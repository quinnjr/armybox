//! Huffman coding for zstd literals section
//!
//! Zstd uses canonical Huffman coding for literals. Weights are stored
//! either as FSE-compressed or as direct 4-bit nibbles.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::bitstream::BitReader;
use crate::common::ZstdError;
use crate::fse::FseTable;

/// Maximum Huffman weight / code length
const MAX_BITS: u8 = 11;

/// Maximum number of symbols
// ============================================================================
// Decoding
// ============================================================================

/// Huffman decoding table entry
#[derive(Clone, Copy)]
struct HuffDecEntry {
    symbol: u8,
    num_bits: u8,
}

/// Huffman decoding table for literals
pub struct HuffTable {
    /// Single-level lookup table, indexed by MAX_BITS bits
    table: Vec<HuffDecEntry>,
    /// Maximum code length
    max_bits: u8,
}

impl HuffTable {
    /// Build a Huffman decoding table from symbol weights.
    ///
    /// Weights are 0..=MAX_BITS where 0 means the symbol is not present,
    /// and weight W means the symbol's code length is (max_bits + 1 - W).
    pub fn build(weights: &[u8]) -> Result<Self, ZstdError> {
        if weights.is_empty() {
            return Err(ZstdError::CorruptData);
        }

        // Find max weight and compute weight counts
        let mut weight_counts = [0u32; MAX_BITS as usize + 2];
        let mut max_weight = 0u8;
        for &w in weights {
            if w > MAX_BITS + 1 {
                return Err(ZstdError::CorruptData);
            }
            weight_counts[w as usize] += 1;
            if w > max_weight {
                max_weight = w;
            }
        }

        if max_weight == 0 {
            return Err(ZstdError::CorruptData);
        }

        // Compute weight value sum using spec formula: sum(2^(w-1)) for w > 0
        // This must be completable to a power of 2 with the implied last weight
        let mut weight_sum = 0u32;
        for w in 1..=max_weight {
            weight_sum += weight_counts[w as usize] << (w - 1);
        }

        // Find the target power of 2 (= 1 << max_bits where max_bits = tableLog)
        // tableLog is the smallest value such that 2^tableLog >= weight_sum
        let max_bits = if weight_sum == 0 {
            return Err(ZstdError::CorruptData);
        } else {
            // Find next power of 2 >= weight_sum + 1 (need room for implied)
            let mut log = max_weight;
            while (1u32 << log) <= weight_sum {
                log += 1;
                if log > MAX_BITS + 1 {
                    return Err(ZstdError::CorruptData);
                }
            }
            log
        };

        let target = 1u32 << max_bits;
        let remainder = target - weight_sum;

        // remainder must be a power of 2 (for the implied last weight)
        if remainder == 0 || (remainder & (remainder - 1)) != 0 {
            return Err(ZstdError::CorruptData);
        }

        // Implied last weight: 2^(implied_w - 1) = remainder
        // So implied_w = log2(remainder) + 1
        let implied_weight = (32 - remainder.leading_zeros()) as u8; // log2(remainder) + 1
        // Table size = 1 << max_bits
        let table_size = 1usize << max_bits;
        let mut table = vec![
            HuffDecEntry {
                symbol: 0,
                num_bits: 0
            };
            table_size
        ];

        // Build full weight array including implied last symbol
        let implied_sym = weights.len();

        // Build lookup table using the zstd convention:
        // Longest codes (lowest weight) get the lowest table indices.
        // This is the reverse of the typical canonical Huffman ordering.
        //
        // rank_indexes[max_bits] = 0 (longest codes start at index 0)
        // rank_indexes[bits-1] = rank_indexes[bits] + count[bits] * (1 << (max_bits - bits))
        let mut bit_ranks = [0u32; MAX_BITS as usize + 2];
        for &w in weights {
            if w > 0 {
                let bits = max_bits + 1 - w;
                bit_ranks[bits as usize] += 1;
            }
        }
        // Add implied symbol
        {
            let bits = max_bits + 1 - implied_weight;
            bit_ranks[bits as usize] += 1;
        }

        // Compute starting index for each bit length (longest first)
        let mut rank_indexes = [0usize; MAX_BITS as usize + 2];
        rank_indexes[max_bits as usize] = 0;
        for bits in (1..=max_bits).rev() {
            rank_indexes[bits as usize - 1] = rank_indexes[bits as usize]
                + bit_ranks[bits as usize] as usize * (1usize << (max_bits - bits));
        }

        // Fill the table - explicit weights
        for (sym, &w) in weights.iter().enumerate() {
            if w == 0 {
                continue;
            }
            let bits = (max_bits + 1 - w) as usize;
            let base_idx = rank_indexes[bits];
            let num_entries = 1usize << (max_bits as usize - bits);

            for j in 0..num_entries {
                let idx = base_idx + j;
                if idx < table_size {
                    table[idx] = HuffDecEntry {
                        symbol: sym as u8,
                        num_bits: bits as u8,
                    };
                }
            }
            rank_indexes[bits] += num_entries;
        }
        // Fill implied symbol
        {
            let bits = (max_bits + 1 - implied_weight) as usize;
            let base_idx = rank_indexes[bits];
            let num_entries = 1usize << (max_bits as usize - bits);
            for j in 0..num_entries {
                let idx = base_idx + j;
                if idx < table_size {
                    table[idx] = HuffDecEntry {
                        symbol: implied_sym as u8,
                        num_bits: bits as u8,
                    };
                }
            }
        }

        Ok(HuffTable { table, max_bits })
    }

    /// Read Huffman weights from compressed data.
    ///
    /// Returns (weights, bytes_consumed).
    pub fn read_weights(data: &[u8]) -> Result<(Vec<u8>, usize), ZstdError> {
        if data.is_empty() {
            return Err(ZstdError::CorruptData);
        }

        let header_byte = data[0];

        if header_byte < 128 {
            // FSE-compressed weights
            let compressed_size = header_byte as usize;
            if compressed_size == 0 || 1 + compressed_size > data.len() {
                return Err(ZstdError::CorruptData);
            }

            let compressed = &data[1..1 + compressed_size];
            let weights = decompress_fse_weights(compressed)?;
            Ok((weights, 1 + compressed_size))
        } else {
            // Direct encoding: 4-bit nibbles
            let num_symbols = (header_byte as usize) - 127;
            let num_bytes = (num_symbols + 1) / 2;
            if 1 + num_bytes > data.len() {
                return Err(ZstdError::CorruptData);
            }

            let mut weights = Vec::with_capacity(num_symbols);
            for i in 0..num_symbols {
                let byte = data[1 + i / 2];
                let w = if i % 2 == 0 { byte >> 4 } else { byte & 0x0F };
                weights.push(w);
            }

            Ok((weights, 1 + num_bytes))
        }
    }

    /// Decode a single symbol from a forward bitstream
    pub fn decode_symbol(&self, bits: u32) -> (u8, u8) {
        let idx = (bits & ((1u32 << self.max_bits) - 1)) as usize;
        let entry = &self.table[idx];
        (entry.symbol, entry.num_bits)
    }
}

/// Decompress FSE-compressed Huffman weights
fn decompress_fse_weights(data: &[u8]) -> Result<Vec<u8>, ZstdError> {
    if data.is_empty() {
        return Err(ZstdError::CorruptData);
    }

    // Read the FSE table description for weights (max symbol = 12, max AL = 6)
    let (probs, accuracy_log, header_size) = FseTable::read_table_description(data, 12, 6)?;
    let table = FseTable::build(&probs, accuracy_log)?;

    // The remaining data is a backward bitstream with 2 interleaved FSE streams
    let stream_data = &data[header_size..];
    if stream_data.is_empty() {
        return Err(ZstdError::CorruptData);
    }

    let mut reader = BitReader::init(stream_data)?;

    // Initialize two states
    let al = accuracy_log as u32;
    let mut state1 = reader.read_bits(al)?;
    let mut state2 = reader.read_bits(al)?;

    let mut weights = Vec::new();

    // Decode alternating between the two streams.
    // The termination logic follows the zstd specification (and matches ruzstd):
    // - Output symbol from state, then update state (reading bits from stream)
    // - After update, check if bitstream is exhausted
    // - If exhausted after state1 update: push final symbol from state2, break
    // - If exhausted after state2 update: push final symbol from UPDATED state1, break
    loop {
        // Stream 1: output symbol
        let sym1 = table.peek_symbol(state1);
        weights.push(sym1);

        let entry1 = &table.entries[state1 as usize];
        let nb1 = entry1.num_bits as u32;

        // Update state1 - check if enough bits first
        if reader.remaining() < nb1 {
            // Can't update state1; emit final symbol from state2 and stop
            let sym2 = table.peek_symbol(state2);
            weights.push(sym2);
            break;
        }
        table.decode(&mut state1, &mut reader)?;

        // Stream 2: output symbol
        let sym2 = table.peek_symbol(state2);
        weights.push(sym2);

        let entry2 = &table.entries[state2 as usize];
        let nb2 = entry2.num_bits as u32;

        // Update state2 - check if enough bits first
        if reader.remaining() < nb2 {
            // Can't update state2; emit final symbol from ALREADY-UPDATED state1
            let final_sym = table.peek_symbol(state1);
            weights.push(final_sym);
            break;
        }
        table.decode(&mut state2, &mut reader)?;
    }

    Ok(weights)
}

/// Decompress a 1-stream Huffman-coded literals section
pub fn decompress_1stream(
    table: &HuffTable,
    data: &[u8],
    regen_size: usize,
) -> Result<Vec<u8>, ZstdError> {
    if data.is_empty() {
        return Err(ZstdError::CorruptData);
    }

    let mut reader = BitReader::init(data)?;
    let mut output = Vec::with_capacity(regen_size);

    while output.len() < regen_size {
        // When fewer than max_bits remain, pad with zeros at LSB (matching zstd spec behavior).
        // The table is MSB-aligned, so short peeks must be shifted left.
        let remaining = reader.remaining();
        let max = table.max_bits as u32;
        let bits = if remaining >= max {
            reader.peek_bits(max)?
        } else if remaining > 0 {
            // Shift left to MSB-align the available bits
            reader.peek_bits(remaining)? << (max - remaining)
        } else {
            0
        };
        let (sym, num_bits) = table.decode_symbol(bits);
        let consume = core::cmp::min(num_bits as u32, remaining);
        reader.consume(consume);
        output.push(sym);
    }

    Ok(output)
}

/// Decompress a 4-stream Huffman-coded literals section
pub fn decompress_4streams(
    table: &HuffTable,
    data: &[u8],
    regen_size: usize,
) -> Result<Vec<u8>, ZstdError> {
    // First 6 bytes: 3 x u16 LE = compressed sizes of streams 1-3
    // Stream 4 size is implicit (rest of data)
    if data.len() < 6 {
        return Err(ZstdError::CorruptData);
    }

    let csize1 = u16::from_le_bytes([data[0], data[1]]) as usize;
    let csize2 = u16::from_le_bytes([data[2], data[3]]) as usize;
    let csize3 = u16::from_le_bytes([data[4], data[5]]) as usize;

    let start1 = 6;
    let start2 = start1 + csize1;
    let start3 = start2 + csize2;
    let start4 = start3 + csize3;

    if start4 > data.len() {
        return Err(ZstdError::CorruptData);
    }

    // Each stream decompresses to regen_size/4 bytes (last one gets remainder)
    let seg_size = (regen_size + 3) / 4;
    let sizes = [
        core::cmp::min(seg_size, regen_size),
        core::cmp::min(seg_size, regen_size.saturating_sub(seg_size)),
        core::cmp::min(seg_size, regen_size.saturating_sub(seg_size * 2)),
        regen_size.saturating_sub(seg_size * 3),
    ];

    let streams: [&[u8]; 4] = [
        &data[start1..start2],
        &data[start2..start3],
        &data[start3..start4],
        &data[start4..],
    ];

    let mut output = Vec::with_capacity(regen_size);

    for i in 0..4 {
        if sizes[i] == 0 {
            continue;
        }
        let decoded = decompress_1stream(table, streams[i], sizes[i])?;
        output.extend_from_slice(&decoded);
    }

    Ok(output)
}

// ============================================================================
// Encoding
// ============================================================================

/// Huffman encoding table
#[cfg(feature = "alloc")]
pub struct HuffEncTable {
    /// code[symbol] = (code_value, code_length)
    pub codes: Vec<(u32, u8)>,
    pub max_bits: u8,
    pub weights: Vec<u8>,
}

/// Build a Huffman encoding table from symbol frequencies
#[cfg(feature = "alloc")]
pub fn build_huff_enc_table(histogram: &[u32; 256]) -> Option<HuffEncTable> {
    // Count non-zero symbols
    let mut num_symbols = 0;
    for &count in histogram {
        if count > 0 {
            num_symbols += 1;
        }
    }

    if num_symbols == 0 {
        return None;
    }

    if num_symbols == 1 {
        // Special case: single symbol
        let mut weights = vec![0u8; 256];
        let mut codes = vec![(0u32, 0u8); 256];
        for (i, &count) in histogram.iter().enumerate() {
            if count > 0 {
                weights[i] = 1;
                codes[i] = (0, 1);
                break;
            }
        }
        return Some(HuffEncTable {
            codes,
            max_bits: 1,
            weights,
        });
    }

    // Build Huffman tree using simple length-limited approach
    // Collect symbols with their frequencies
    let mut symbols: Vec<(usize, u32)> = Vec::new();
    for (i, &count) in histogram.iter().enumerate() {
        if count > 0 {
            symbols.push((i, count));
        }
    }

    // Sort by frequency (ascending)
    symbols.sort_by_key(|&(_, freq)| freq);

    // Build code lengths using package-merge-like approach (simplified)
    // For simplicity, use a greedy assignment that limits to MAX_BITS
    let mut lengths = vec![0u8; 256];
    assign_code_lengths(&symbols, &mut lengths, MAX_BITS);

    // Build canonical codes
    let max_bits = *lengths.iter().max().unwrap_or(&0);
    if max_bits == 0 {
        return None;
    }

    // Count codes of each length
    let mut bl_count = [0u32; MAX_BITS as usize + 2];
    for &len in &lengths {
        if len > 0 {
            bl_count[len as usize] += 1;
        }
    }

    // Compute starting code for each length
    let mut next_code = [0u32; MAX_BITS as usize + 2];
    let mut code = 0u32;
    for bits in 1..=max_bits {
        code = (code + bl_count[bits as usize - 1]) << 1;
        next_code[bits as usize] = code;
    }

    // Assign codes
    let mut codes = vec![(0u32, 0u8); 256];
    for (sym, &len) in lengths.iter().enumerate() {
        if len > 0 {
            // Store code MSB-first in the lower `len` bits, but for table lookup
            // we need it in a specific format
            codes[sym] = (next_code[len as usize], len);
            next_code[len as usize] += 1;
        }
    }

    // Build weights from lengths: weight = max_bits + 1 - length (for length > 0)
    let mut weights = vec![0u8; 256];
    for (i, &len) in lengths.iter().enumerate() {
        if len > 0 {
            weights[i] = max_bits + 1 - len;
        }
    }

    Some(HuffEncTable {
        codes,
        max_bits,
        weights,
    })
}

/// Assign code lengths to symbols using a simple heuristic
#[cfg(feature = "alloc")]
fn assign_code_lengths(symbols: &[(usize, u32)], lengths: &mut [u8], max_bits: u8) {
    let n = symbols.len();
    if n <= 1 {
        if n == 1 {
            lengths[symbols[0].0] = 1;
        }
        return;
    }

    // Use a simple bottom-up Huffman tree construction
    // with length limiting
    let tree: Vec<u64> = symbols.iter().map(|&(_, freq)| freq as u64).collect();
    let mut depth = vec![0u8; n];

    // Simulate tree building
    let mut leaves = tree.clone();
    while leaves.len() > 1 {
        leaves.sort();
        let a = leaves[0];
        let b = leaves[1];
        leaves.remove(0);
        leaves[0] = a + b;

        // Increment depth for all symbols with frequency <= a or b
        for (i, &freq) in tree.iter().enumerate() {
            if freq <= a {
                depth[i] += 1;
            }
        }
    }

    // This simple approach doesn't produce optimal results, so use a different method:
    // Assign lengths based on log2 of relative frequency
    let total: u64 = symbols.iter().map(|&(_, f)| f as u64).sum();
    if total == 0 {
        return;
    }

    for (i, &(sym, freq)) in symbols.iter().enumerate() {
        if freq == 0 {
            continue;
        }
        // Ideal length = -log2(freq/total) = log2(total) - log2(freq)
        let ideal = if freq == 0 {
            max_bits
        } else {
            let log_total = 64 - total.leading_zeros();
            let log_freq = 64 - (freq as u64).leading_zeros();
            let len = if log_total > log_freq {
                (log_total - log_freq) as u8
            } else {
                1
            };
            len.clamp(1, max_bits)
        };
        lengths[sym] = ideal;
        let _ = i; // suppress unused
    }

    // Adjust to satisfy Kraft inequality: sum(2^(-len_i)) <= 1
    // Which in integer form: sum(2^(max_bits - len_i)) <= 2^max_bits
    loop {
        let kraft_sum: u64 = lengths
            .iter()
            .filter(|&&l| l > 0)
            .map(|&l| 1u64 << (max_bits - l))
            .sum();

        let target = 1u64 << max_bits;

        if kraft_sum == target {
            break;
        } else if kraft_sum < target {
            // Under-full: shorten the longest codes
            let mut max_len_sym = 0;
            let mut max_len = 0u8;
            for (i, &l) in lengths.iter().enumerate() {
                if l > max_len {
                    max_len = l;
                    max_len_sym = i;
                }
            }
            if max_len > 1 {
                lengths[max_len_sym] -= 1;
            } else {
                break;
            }
        } else {
            // Over-full: lengthen the shortest codes
            let mut min_len_sym = 0;
            let mut min_len = max_bits + 1;
            for (i, &l) in lengths.iter().enumerate() {
                if l > 0 && l < min_len {
                    min_len = l;
                    min_len_sym = i;
                }
            }
            if min_len < max_bits {
                lengths[min_len_sym] += 1;
            } else {
                break;
            }
        }
    }
}

/// Write Huffman weights header
#[cfg(feature = "alloc")]
pub fn write_weights(weights: &[u8]) -> Vec<u8> {
    // Find last non-zero weight
    let num_symbols = weights.iter().rposition(|&w| w > 0).map_or(0, |p| p + 1);

    if num_symbols == 0 {
        return vec![0];
    }

    // Use direct 4-bit encoding (simpler, good enough for our compressor)
    let header_byte = (num_symbols + 127) as u8;
    let num_bytes = (num_symbols + 1) / 2;
    let mut out = Vec::with_capacity(1 + num_bytes);
    out.push(header_byte);

    for i in (0..num_symbols).step_by(2) {
        let hi = weights[i];
        let lo = if i + 1 < num_symbols {
            weights[i + 1]
        } else {
            0
        };
        out.push((hi << 4) | lo);
    }

    out
}

/// Compress literals using Huffman coding.
///
/// Returns (compressed_data, huffman_table_for_reuse) or None if incompressible.
#[cfg(feature = "alloc")]
pub fn compress_literals(data: &[u8]) -> Option<(Vec<u8>, HuffEncTable)> {
    if data.is_empty() {
        return None;
    }

    // Build histogram
    let mut histogram = [0u32; 256];
    for &b in data {
        histogram[b as usize] += 1;
    }

    let enc_table = build_huff_enc_table(&histogram)?;

    // Encode literals
    let encoded = encode_huffman_stream(&enc_table, data);

    // Only use if compression is beneficial
    if encoded.len() + write_weights(&enc_table.weights).len() >= data.len() {
        return None;
    }

    Some((encoded, enc_table))
}

/// Encode data using the given Huffman table.
/// For data >= 256 bytes, uses 4 streams; otherwise 1 stream.
#[cfg(feature = "alloc")]
fn encode_huffman_stream(table: &HuffEncTable, data: &[u8]) -> Vec<u8> {
    if data.len() >= 256 {
        encode_4streams(table, data)
    } else {
        encode_1stream(table, data)
    }
}

/// Encode a single Huffman stream (bits written MSB-first, backward bitstream)
#[cfg(feature = "alloc")]
fn encode_1stream(table: &HuffEncTable, data: &[u8]) -> Vec<u8> {
    // Write bits forward, then reverse
    let mut bits: u64 = 0;
    let mut nbits: u32 = 0;
    let mut output = Vec::new();

    for &byte in data.iter().rev() {
        let (code, len) = table.codes[byte as usize];
        if len == 0 {
            continue;
        }

        // Reverse the code bits for our forward writer
        let reversed = reverse_bits(code, len);
        bits |= (reversed as u64) << nbits;
        nbits += len as u32;

        while nbits >= 8 {
            output.push(bits as u8);
            bits >>= 8;
            nbits -= 8;
        }
    }

    // Add sentinel bit
    bits |= 1u64 << nbits;
    nbits += 1;

    while nbits > 0 {
        output.push(bits as u8);
        bits >>= 8;
        nbits = nbits.saturating_sub(8);
    }

    output.reverse();
    output
}

/// Encode 4 Huffman streams
#[cfg(feature = "alloc")]
fn encode_4streams(table: &HuffEncTable, data: &[u8]) -> Vec<u8> {
    let seg = (data.len() + 3) / 4;
    let segs = [
        &data[..core::cmp::min(seg, data.len())],
        &data[core::cmp::min(seg, data.len())..core::cmp::min(seg * 2, data.len())],
        &data[core::cmp::min(seg * 2, data.len())..core::cmp::min(seg * 3, data.len())],
        &data[core::cmp::min(seg * 3, data.len())..],
    ];

    let s1 = encode_1stream(table, segs[0]);
    let s2 = encode_1stream(table, segs[1]);
    let s3 = encode_1stream(table, segs[2]);
    let s4 = encode_1stream(table, segs[3]);

    // 6-byte jump table: 3 x u16 LE
    let mut output = Vec::with_capacity(6 + s1.len() + s2.len() + s3.len() + s4.len());
    output.extend_from_slice(&(s1.len() as u16).to_le_bytes());
    output.extend_from_slice(&(s2.len() as u16).to_le_bytes());
    output.extend_from_slice(&(s3.len() as u16).to_le_bytes());
    output.extend_from_slice(&s1);
    output.extend_from_slice(&s2);
    output.extend_from_slice(&s3);
    output.extend_from_slice(&s4);

    output
}

/// Reverse the bottom `len` bits of a value
fn reverse_bits(val: u32, len: u8) -> u32 {
    let mut result = 0u32;
    let mut v = val;
    for _ in 0..len {
        result = (result << 1) | (v & 1);
        v >>= 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_direct_weights() {
        // Header byte >= 128: direct 4-bit weights
        // 128 + 4 - 127 = 5, no: header_byte = num_symbols + 127
        // For 4 symbols: header = 4 + 127 = 131
        let data = [131, 0x21, 0x30]; // weights: 2,1, 3,0
        let (weights, consumed) = HuffTable::read_weights(&data).unwrap();
        assert_eq!(weights, vec![2, 1, 3, 0]);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn test_build_hufftable() {
        // Valid weights: weight_sum = 2+1 = 3, target = 4, remainder = 1 (power of 2)
        let weights = vec![2u8, 1];
        let table = HuffTable::build(&weights).unwrap();
        assert!(table.max_bits > 0);

        // Valid weights: weight_sum = 2+2+2 = 6, target = 8, remainder = 2
        let weights = vec![2u8, 2, 2];
        let table = HuffTable::build(&weights).unwrap();
        assert!(table.max_bits > 0);

        // Valid weights: weight_sum = 4+2+1 = 7, target = 8, remainder = 1
        let weights = vec![3u8, 2, 1];
        let table = HuffTable::build(&weights).unwrap();
        assert!(table.max_bits > 0);
    }

    #[test]
    fn test_reverse_bits() {
        assert_eq!(reverse_bits(0b110, 3), 0b011);
        assert_eq!(reverse_bits(0b1010, 4), 0b0101);
    }
}
