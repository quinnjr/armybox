//! Zstandard compression (RFC 8878)
//!
//! Implements a level-1 equivalent greedy compressor with hash-table match finding.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::bitstream::BitWriter;
use crate::common::*;
use crate::fse::{FseTable, normalize_counts, write_table_description};
use crate::xxhash::xxhash64;

/// Compress data using zstandard format
pub fn compress(input: &[u8], level: u32) -> Vec<u8> {
    let mut output = Vec::new();
    encode_frame(input, level, &mut output);
    output
}

/// Encode a complete zstd frame
fn encode_frame(input: &[u8], _level: u32, output: &mut Vec<u8>) {
    // Magic number
    output.extend_from_slice(&ZSTD_MAGIC.to_le_bytes());

    // Frame header descriptor
    let fcs_flag = if input.len() <= 0xFF {
        0u8
    } else if input.len() <= 0xFFFF + 256 {
        1u8
    } else if input.len() as u64 <= 0xFFFFFFFF {
        2u8
    } else {
        3u8
    };

    let single_segment = true;
    let content_checksum = true;

    let descriptor =
        (fcs_flag << 6) | ((single_segment as u8) << 5) | ((content_checksum as u8) << 2);

    output.push(descriptor);

    // Frame content size
    match fcs_flag {
        0 => {
            output.push(input.len() as u8);
        }
        1 => {
            let val = (input.len() as u16).wrapping_sub(256);
            output.extend_from_slice(&val.to_le_bytes());
        }
        2 => {
            output.extend_from_slice(&(input.len() as u32).to_le_bytes());
        }
        _ => {
            output.extend_from_slice(&(input.len() as u64).to_le_bytes());
        }
    }

    // Encode blocks
    if input.is_empty() {
        let block_header: u32 = 1; // last=1, type=raw(0), size=0
        output.push(block_header as u8);
        output.push((block_header >> 8) as u8);
        output.push((block_header >> 16) as u8);
    } else {
        let mut pos = 0;
        while pos < input.len() {
            let end = core::cmp::min(pos + MAX_BLOCK_SIZE, input.len());
            let block = &input[pos..end];
            let is_last = end == input.len();
            encode_block(block, is_last, output);
            pos = end;
        }
    }

    // Content checksum (lower 32 bits of xxHash64)
    let hash = (xxhash64(input) & 0xFFFFFFFF) as u32;
    output.extend_from_slice(&hash.to_le_bytes());
}

/// Encode a single block
fn encode_block(data: &[u8], is_last: bool, output: &mut Vec<u8>) {
    // Try compressed encoding
    if let Some(compressed) = encode_compressed_block(data)
        && compressed.len() < data.len()
    {
        let block_header = (is_last as u32)
            | ((BlockType::Compressed as u32) << 1)
            | ((compressed.len() as u32) << 3);
        output.push(block_header as u8);
        output.push((block_header >> 8) as u8);
        output.push((block_header >> 16) as u8);
        output.extend_from_slice(&compressed);
        return;
    }

    // Fall back to raw block
    let block_header =
        (is_last as u32) | ((BlockType::Raw as u32) << 1) | ((data.len() as u32) << 3);
    output.push(block_header as u8);
    output.push((block_header >> 8) as u8);
    output.push((block_header >> 16) as u8);
    output.extend_from_slice(data);
}

/// A sequence before encoding
struct RawSequence {
    lit_len: u32,
    match_len: u32,
    offset: u32,
}

/// Try to encode a compressed block
fn encode_compressed_block(data: &[u8]) -> Option<Vec<u8>> {
    let (sequences, literals) = find_matches(data);

    if sequences.is_empty() {
        return encode_literals_only_block(&literals);
    }

    let mut block = Vec::new();

    // Encode literals section
    encode_literals_section(&literals, &mut block);

    // Encode sequences section
    encode_sequences_section(&sequences, &mut block)?;

    Some(block)
}

/// Greedy match finder with hash table
fn find_matches(data: &[u8]) -> (Vec<RawSequence>, Vec<u8>) {
    let mut sequences = Vec::new();
    let mut literals = Vec::new();
    let mut offsets = [1u32, 4, 8];

    if data.len() < MIN_MATCH {
        literals.extend_from_slice(data);
        return (sequences, literals);
    }

    let hash_bits = 16u32;
    let hash_size = 1usize << hash_bits;
    let mut hash_table = vec![0u32; hash_size];
    let mut pos = 0;
    let mut lit_start = 0;

    while pos + MIN_MATCH <= data.len() {
        let mut best_len = 0u32;
        let mut best_offset = 0u32;
        let lit_len = (pos - lit_start) as u32;

        // Check repeat offsets
        for (rep_idx, &rep_off) in offsets.iter().enumerate() {
            if rep_off as usize <= pos && rep_off > 0 {
                let match_start = pos - rep_off as usize;
                let len = count_match(data, match_start, pos);
                if len >= MIN_MATCH as u32 && len > best_len {
                    best_len = len;
                    if lit_len > 0 {
                        best_offset = rep_idx as u32 + 1;
                    } else {
                        // litlen==0 shifts meanings
                        best_offset = match rep_idx {
                            1 => 1,
                            2 => 2,
                            _ => continue,
                        };
                    }
                }
            }
        }

        // Hash-based match
        if pos + 4 <= data.len() {
            let h = hash4(data, pos, hash_bits);
            let candidate = hash_table[h] as usize;
            hash_table[h] = pos as u32;

            if candidate < pos && pos - candidate <= (1 << 20) && candidate + 3 < data.len() {
                let len = count_match(data, candidate, pos);
                if len >= MIN_MATCH as u32 && len > best_len {
                    best_len = len;
                    let actual_offset = (pos - candidate) as u32;
                    best_offset = actual_offset + 3;
                }
            }
        }

        if best_len >= MIN_MATCH as u32 {
            literals.extend_from_slice(&data[lit_start..pos]);

            sequences.push(RawSequence {
                lit_len,
                match_len: best_len,
                offset: best_offset,
            });

            // Update repeat offsets
            if best_offset > 3 {
                offsets[2] = offsets[1];
                offsets[1] = offsets[0];
                offsets[0] = best_offset - 3;
            } else if lit_len > 0 {
                match best_offset {
                    2 => {
                        offsets.swap(0, 1);
                    }
                    3 => {
                        let tmp = offsets[2];
                        offsets[2] = offsets[1];
                        offsets[1] = offsets[0];
                        offsets[0] = tmp;
                    }
                    _ => {}
                }
            } else {
                match best_offset {
                    1 => {
                        offsets.swap(0, 1);
                    }
                    2 => {
                        let tmp = offsets[2];
                        offsets[2] = offsets[1];
                        offsets[1] = offsets[0];
                        offsets[0] = tmp;
                    }
                    _ => {}
                }
            }

            // Update hash table for match bytes
            let match_end = pos + best_len as usize;
            pos += 1;
            while pos < match_end && pos + 4 <= data.len() {
                let h = hash4(data, pos, hash_bits);
                hash_table[h] = pos as u32;
                pos += 1;
            }
            pos = match_end;
            lit_start = pos;
        } else {
            pos += 1;
        }
    }

    if lit_start < data.len() {
        literals.extend_from_slice(&data[lit_start..]);
    }

    (sequences, literals)
}

fn count_match(data: &[u8], pos1: usize, pos2: usize) -> u32 {
    let max_len = data.len() - pos2;
    let mut len = 0u32;
    while (len as usize) < max_len && data[pos1 + len as usize] == data[pos2 + len as usize] {
        len += 1;
    }
    len
}

fn hash4(data: &[u8], pos: usize, bits: u32) -> usize {
    let val = read_le32(data, pos);
    ((val.wrapping_mul(0x9E3779B1)) >> (32 - bits)) as usize
}

/// Encode literals section
fn encode_literals_section(literals: &[u8], output: &mut Vec<u8>) {
    let regen_size = literals.len();

    // Use raw literals — simpler and always correct
    // Huffman compression can be added later as an optimization
    if regen_size < 32 {
        // 1-byte header, size_format=0
        let header = (LiteralsType::Raw as u8) | ((regen_size as u8) << 3);
        output.push(header);
    } else if regen_size < 4096 {
        // 2-byte header, size_format=1
        let header = (LiteralsType::Raw as u16) | (1u16 << 2) | ((regen_size as u16) << 4);
        output.push(header as u8);
        output.push((header >> 8) as u8);
    } else {
        // 3-byte header, size_format=3
        let header = (LiteralsType::Raw as u32) | (3u32 << 2) | ((regen_size as u32) << 4);
        output.push(header as u8);
        output.push((header >> 8) as u8);
        output.push((header >> 16) as u8);
    }
    output.extend_from_slice(literals);
}

/// Encode the sequences section
fn encode_sequences_section(sequences: &[RawSequence], output: &mut Vec<u8>) -> Option<()> {
    let num_seq = sequences.len();

    // Sequence count
    if num_seq < 128 {
        output.push(num_seq as u8);
    } else if num_seq < 0x7F00 {
        output.push(((num_seq >> 8) + 128) as u8);
        output.push((num_seq & 0xFF) as u8);
    } else {
        output.push(255);
        let val = num_seq - 0x7F00;
        output.push(val as u8);
        output.push((val >> 8) as u8);
    }

    // Build histograms
    let mut ll_hist = [0u32; 36];
    let mut ml_hist = [0u32; 53];
    let mut of_hist = [0u32; 32];
    let mut max_ll = 0usize;
    let mut max_ml = 0usize;
    let mut max_of = 0usize;

    for seq in sequences {
        let ll = ll_code(seq.lit_len) as usize;
        let ml = ml_code(seq.match_len) as usize;
        let of = of_code(seq.offset) as usize;
        ll_hist[ll] += 1;
        ml_hist[ml] += 1;
        of_hist[of] += 1;
        if ll > max_ll {
            max_ll = ll;
        }
        if ml > max_ml {
            max_ml = ml;
        }
        if of > max_of {
            max_of = of;
        }
    }

    // For each table, determine mode and build table
    let ll_info = build_enc_table(&ll_hist, max_ll, LL_MAX_AL);
    let of_info = build_enc_table(&of_hist, max_of, OF_MAX_AL);
    let ml_info = build_enc_table(&ml_hist, max_ml, ML_MAX_AL);

    // Compression modes byte
    let modes_byte = (ll_info.mode << 6) | (of_info.mode << 4) | (ml_info.mode << 2);
    output.push(modes_byte);

    // Write table descriptions
    output.extend_from_slice(&ll_info.header);
    output.extend_from_slice(&of_info.header);
    output.extend_from_slice(&ml_info.header);

    // Encode sequences into backward bitstream
    let bitstream = encode_seq_bitstream(sequences, &ll_info, &of_info, &ml_info)?;
    output.extend_from_slice(&bitstream);

    Some(())
}

struct EncTableInfo {
    mode: u8,
    header: Vec<u8>,
    dec_table: FseTable,
    // For encoding: maps symbol -> list of (state_idx, target_state, num_bits)
    enc_map: Vec<Vec<EncMapEntry>>,
}

#[derive(Clone)]
struct EncMapEntry {
    target_state: u16,
}

/// Build encoding info for a sequence table
fn build_enc_table(hist: &[u32], max_sym: usize, max_al: u8) -> EncTableInfo {
    // Count distinct symbols
    let distinct: usize = hist[..=max_sym].iter().filter(|&&c| c > 0).count();

    if distinct <= 1 {
        // RLE mode
        let sym = hist.iter().position(|&c| c > 0).unwrap_or(0) as u8;
        let mut probs = vec![0i16; sym as usize + 1];
        probs[sym as usize] = 1;
        let dec_table = FseTable::build(&probs, 0).unwrap();

        // Build enc_map for single symbol
        let mut enc_map = vec![Vec::new(); sym as usize + 1];
        enc_map[sym as usize].push(EncMapEntry { target_state: 0 });

        return EncTableInfo {
            mode: SeqMode::Rle as u8,
            header: vec![sym],
            dec_table,
            enc_map,
        };
    }

    // FSE compressed mode
    let table_log = compute_table_log(hist, max_sym, max_al);
    let probs = normalize_counts(hist, max_sym, table_log);
    let header = write_table_description(&probs, table_log);
    let dec_table = FseTable::build(&probs, table_log).unwrap();

    // Build encoding map from the decoding table
    let num_symbols = probs.len();
    let mut enc_map = vec![Vec::new(); num_symbols];

    // For each symbol, collect the states that decode to it
    // and compute the encoding parameters
    let mut symbol_count = vec![0u16; num_symbols];
    for sym in 0..num_symbols {
        if probs[sym] == -1 {
            symbol_count[sym] = 1;
        } else if probs[sym] > 0 {
            symbol_count[sym] = probs[sym] as u16;
        }
    }

    // Build reverse mapping: for each state, record what symbol it decodes to
    // The encoding for symbol S with count C works as follows:
    // There are C cells in the table for symbol S
    // To encode symbol S from state `st`:
    //   nb = num_bits for the cell
    //   Output low `nb` bits of `st`
    //   New state = cell's baseline + (st >> nb)
    // But that's the wrong direction. For encoding:
    // Given that we want to output symbol S and transition from one valid state to another:
    // We need to find which cell to use and what bits to emit.
    //
    // The correct FSE encoding works as:
    // 1. Look up the entry for current state (which gives symbol, num_bits, baseline)
    // 2. But encoding is the reverse: given a symbol, find which state to transition to
    //
    // A simpler approach: for each symbol, we track a "next_state_index" that cycles
    // through the available states for that symbol.
    // For symbol S:
    //   Collect all states [s0, s1, ..., sN-1] where entries[si].symbol == S
    //   When encoding S, use the next state in round-robin fashion
    //   The bits to emit are entries[target_state].num_bits bits of the current state

    // Collect states per symbol (sorted by state index)
    let mut states_for_sym: Vec<Vec<usize>> = vec![Vec::new(); num_symbols];
    for (state, entry) in dec_table.entries.iter().enumerate() {
        states_for_sym[entry.symbol as usize].push(state);
    }

    for sym in 0..num_symbols {
        let states = &states_for_sym[sym];
        for &target in states {
            enc_map[sym].push(EncMapEntry {
                target_state: target as u16,
            });
        }
    }

    EncTableInfo {
        mode: SeqMode::FseCompressed as u8,
        header,
        dec_table,
        enc_map,
    }
}

fn compute_table_log(hist: &[u32], max_sym: usize, max_al: u8) -> u8 {
    let total: u32 = hist[..=max_sym].iter().sum();
    if total == 0 {
        return 5;
    }

    let log = if total < 16 {
        5
    } else if total < 64 {
        6
    } else if total < 256 {
        7
    } else {
        8
    };

    let min_log = if max_sym < 2 {
        1
    } else {
        (32 - (max_sym as u32).leading_zeros()) as u8
    };

    core::cmp::max(log, min_log + 1).min(max_al)
}

/// Encode sequences into a backward bitstream.
///
/// FSE state transitions are computed in REVERSE order (last sequence first)
/// to guarantee valid transitions. For any target state, there exists exactly
/// one source state per symbol whose transition range includes that target.
/// This property is guaranteed by FSE table construction.
///
/// After computing all states backward, the bitstream is written forward
/// in the order the decoder expects.
fn encode_seq_bitstream(
    sequences: &[RawSequence],
    ll_info: &EncTableInfo,
    of_info: &EncTableInfo,
    ml_info: &EncTableInfo,
) -> Option<Vec<u8>> {
    let ll_al = ll_info.dec_table.accuracy_log;
    let of_al = of_info.dec_table.accuracy_log;
    let ml_al = ml_info.dec_table.accuracy_log;

    let n = sequences.len();

    // Pre-compute codes for all sequences
    let mut seq_codes: Vec<(u8, u8, u8)> = Vec::with_capacity(n);
    for seq in sequences {
        seq_codes.push((
            ll_code(seq.lit_len),
            of_code(seq.offset),
            ml_code(seq.match_len),
        ));
    }

    // --- Reverse-order FSE state computation ---
    //
    // The decoder processes sequences forward:
    //   state_0 = init_state (read from bitstream)
    //   For each seq i: symbol = entries[state_i].symbol
    //                    bits = read(entries[state_i].num_bits)
    //                    state_{i+1} = entries[state_i].baseline + bits
    //
    // The encoder works backward:
    //   1. Choose state_{N-1} (arbitrary state for last sequence's symbol)
    //   2. For i = N-2..0: find state_i such that state_{i+1} is in
    //      [entries[state_i].baseline, entries[state_i].baseline + 2^num_bits)
    //   3. state_0 becomes the init state for the decoder

    // Choose arbitrary valid states for the last sequence's symbols
    let mut ll_state = find_state_for_symbol(&ll_info.enc_map, seq_codes[n - 1].0);
    let mut of_state = find_state_for_symbol(&of_info.enc_map, seq_codes[n - 1].1);
    let mut ml_state = find_state_for_symbol(&ml_info.enc_map, seq_codes[n - 1].2);

    struct SeqUpdate {
        ll_bits: u32,
        ll_nbits: u8,
        of_bits: u32,
        of_nbits: u8,
        ml_bits: u32,
        ml_nbits: u8,
    }

    let mut updates: Vec<SeqUpdate> = (0..n)
        .map(|_| SeqUpdate {
            ll_bits: 0,
            ll_nbits: 0,
            of_bits: 0,
            of_nbits: 0,
            ml_bits: 0,
            ml_nbits: 0,
        })
        .collect();

    // Process backward: for each seq[i], find the source state whose
    // transition range includes the current target (*_state).
    for i in (0..n - 1).rev() {
        let (src_ll, ll_nb, ll_bv) = find_source_state(
            &ll_info.dec_table,
            &ll_info.enc_map,
            seq_codes[i].0,
            ll_state,
        );
        let (src_of, of_nb, of_bv) = find_source_state(
            &of_info.dec_table,
            &of_info.enc_map,
            seq_codes[i].1,
            of_state,
        );
        let (src_ml, ml_nb, ml_bv) = find_source_state(
            &ml_info.dec_table,
            &ml_info.enc_map,
            seq_codes[i].2,
            ml_state,
        );

        updates[i] = SeqUpdate {
            ll_bits: ll_bv,
            ll_nbits: ll_nb,
            of_bits: of_bv,
            of_nbits: of_nb,
            ml_bits: ml_bv,
            ml_nbits: ml_nb,
        };

        ll_state = src_ll;
        of_state = src_of;
        ml_state = src_ml;
    }

    // After backward processing, *_state holds the init states for the decoder
    let init_ll = ll_state;
    let init_of = of_state;
    let init_ml = ml_state;

    // --- Write bitstream in forward order ---
    let mut writer = BitWriter::new();

    // Init states
    writer.write_bits(init_ll as u32, ll_al as u32);
    writer.write_bits(init_of as u32, of_al as u32);
    writer.write_bits(init_ml as u32, ml_al as u32);

    for (i, seq) in sequences.iter().enumerate() {
        let (ll_c, of_c, ml_c) = seq_codes[i];

        // OF extra bits
        let of_nbits = of_c as u32;
        if of_nbits > 0 {
            let of_extra = seq.offset - (1u32 << of_nbits);
            writer.write_bits(of_extra, of_nbits);
        }

        // ML extra bits
        let ml_eb = ML_EXTRA_BITS[ml_c as usize] as u32;
        if ml_eb > 0 {
            let ml_extra = seq.match_len - ML_BASELINES[ml_c as usize];
            writer.write_bits(ml_extra, ml_eb);
        }

        // LL extra bits
        let ll_eb = LL_EXTRA_BITS[ll_c as usize] as u32;
        if ll_eb > 0 {
            let ll_extra = seq.lit_len - LL_BASELINES[ll_c as usize];
            writer.write_bits(ll_extra, ll_eb);
        }

        // State updates (not for last sequence)
        if i < n - 1 {
            let upd = &updates[i];
            writer.write_bits(upd.ll_bits, upd.ll_nbits as u32);
            writer.write_bits(upd.ml_bits, upd.ml_nbits as u32);
            writer.write_bits(upd.of_bits, upd.of_nbits as u32);
        }
    }

    Some(writer.finish())
}

/// Find a valid state index for the given symbol
fn find_state_for_symbol(enc_map: &[Vec<EncMapEntry>], symbol: u8) -> u16 {
    let sym = symbol as usize;
    if sym < enc_map.len() && !enc_map[sym].is_empty() {
        enc_map[sym][0].target_state
    } else {
        0
    }
}

/// Find the source state for the given symbol whose transition range includes `target_state`.
///
/// The decoder does:
///   new_state = entries[source].baseline + read_bits(entries[source].num_bits)
///
/// So we need a source state where:
///   entries[source].symbol == symbol
///   entries[source].baseline <= target_state < entries[source].baseline + (1 << entries[source].num_bits)
///
/// By FSE table construction, for any (symbol, target_state) pair, exactly one
/// source state exists whose range covers the target. This makes reverse-order
/// encoding always succeed.
///
/// Returns (source_state, num_bits, bits_value) where bits_value = target_state - baseline
fn find_source_state(
    dec_table: &FseTable,
    enc_map: &[Vec<EncMapEntry>],
    symbol: u8,
    target_state: u16,
) -> (u16, u8, u32) {
    let sym = symbol as usize;
    if sym >= enc_map.len() || enc_map[sym].is_empty() {
        return (0, 0, 0);
    }

    let target = target_state as u32;
    for cand in &enc_map[sym] {
        let source = cand.target_state;
        let entry = &dec_table.entries[source as usize];
        let baseline = entry.baseline as u32;
        let num_bits = entry.num_bits;
        let range_end = baseline + (1u32 << num_bits);

        if target >= baseline && target < range_end {
            return (source, num_bits, target - baseline);
        }
    }

    // Fallback: should never happen with correctly built FSE tables
    let first = enc_map[sym][0].target_state;
    (first, 0, 0)
}

/// Encode a block with only literals (no sequences)
fn encode_literals_only_block(literals: &[u8]) -> Option<Vec<u8>> {
    let mut block = Vec::new();
    encode_literals_section(literals, &mut block);
    block.push(0); // num_sequences = 0
    Some(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash4() {
        let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let h1 = hash4(&data, 0, 16);
        let h2 = hash4(&data, 1, 16);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_count_match() {
        let data = b"abcabcdef";
        assert_eq!(count_match(data, 0, 3), 3);
    }

    #[test]
    fn test_find_matches_no_matches() {
        let data = b"abcd";
        let (seqs, lits) = find_matches(data);
        assert!(seqs.is_empty());
        assert_eq!(lits, data);
    }

    #[test]
    fn test_compress_empty() {
        let compressed = compress(b"", 1);
        assert!(compressed.len() > 4); // At least magic + header
        assert_eq!(read_le32(&compressed, 0), ZSTD_MAGIC);
    }

    #[test]
    fn test_ll_code() {
        assert_eq!(ll_code(0), 0);
        assert_eq!(ll_code(15), 15);
        assert_eq!(ll_code(16), 16);
    }

    #[test]
    fn test_ml_code() {
        assert_eq!(ml_code(3), 0);
        assert_eq!(ml_code(4), 1);
    }

    #[test]
    fn test_of_code() {
        assert_eq!(of_code(1), 0);
        assert_eq!(of_code(2), 1);
        assert_eq!(of_code(4), 2);
    }
}

#[test]
fn test_compress_decompress_abc() {
    let input = b"ABCABCABCABCABCABCABCABCABCABC"; // 30 bytes, repeated pattern
    let compressed = compress(input, 3);
    let decompressed = crate::decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}

#[test]
fn test_compress_decompress_zeros_1k() {
    let input = vec![0u8; 1024];
    let compressed = compress(&input, 3);
    let decompressed = crate::decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}

#[test]
fn test_compress_decompress_zeros_10k() {
    let input = vec![0u8; 10240];
    let compressed = compress(&input, 3);
    let decompressed = crate::decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}

#[test]
fn test_compress_decompress_tarlike() {
    // Simulate tar-like data: some ASCII + lots of null padding
    let mut input = Vec::new();
    input.extend_from_slice(b"./\x00");
    input.extend_from_slice(&[0u8; 509]); // pad to 512
    input.extend_from_slice(b"./usr/bin/test.sh\x00");
    input.extend_from_slice(&[0u8; 494]); // pad to 512
    input.extend_from_slice(b"#!/bin/sh\necho hello\n");
    input.extend_from_slice(&[0u8; 491]); // pad to 512
    input.extend_from_slice(&[0u8; 1024]); // end of archive

    let compressed = compress(&input, 3);
    let decompressed = crate::decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}

#[test]
fn test_compress_decompress_mixed_4k() {
    // Mixed pattern: repeated text + nulls
    let mut input = Vec::new();
    for i in 0..100 {
        input.extend_from_slice(format!("line {:04}\n", i).as_bytes());
        input.extend_from_slice(&[0u8; 30]);
    }

    let compressed = compress(&input, 3);
    let decompressed = crate::decompress(&compressed).unwrap();
    assert_eq!(decompressed, input);
}
