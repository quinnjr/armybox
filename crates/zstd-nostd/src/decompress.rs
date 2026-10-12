//! Zstandard decompression (RFC 8878)
//!
//! Supports single and multi-frame decompression with optional checksums.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::bitstream::BitReader;
use crate::common::*;
use crate::fse::{FseTable, build_predefined_table};
use crate::huff::{self, HuffTable};
use crate::xxhash::xxhash64;

/// Decompress a zstd frame (or multiple concatenated frames).
///
/// This grows the output `Vec` without any size limit and is therefore
/// vulnerable to decompression bombs from untrusted input. Prefer
/// [`decompress_bounded`] when the input is not fully trusted.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, ZstdError> {
    decompress_bounded(input, usize::MAX)
}

/// Decompress a zstd frame (or multiple concatenated frames), refusing to
/// produce more than `max_output` bytes.
///
/// Returns [`ZstdError::OutputTooLarge`] as soon as the decoded output would
/// exceed `max_output`, instead of allocating without bound. This is the
/// safe entry point for untrusted input (e.g. package payloads).
pub fn decompress_bounded(input: &[u8], max_output: usize) -> Result<Vec<u8>, ZstdError> {
    let mut output = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        if pos + 4 > input.len() {
            break;
        }

        let magic = read_le32(input, pos);

        if magic == ZSTD_MAGIC {
            let consumed = decode_frame(input, pos, &mut output, max_output)?;
            pos += consumed;
        } else if (SKIPPABLE_MAGIC_LOW..=SKIPPABLE_MAGIC_HIGH).contains(&magic) {
            // Skippable frame
            if pos + 8 > input.len() {
                return Err(ZstdError::CorruptData);
            }
            let frame_size = read_le32(input, pos + 4) as usize;
            pos += 8 + frame_size;
        } else {
            return Err(ZstdError::CorruptData);
        }
    }

    Ok(output)
}

/// Frame header information
#[allow(dead_code)]
struct FrameHeader {
    /// Window size for this frame
    window_size: u64,
    /// Frame content size (if known)
    frame_content_size: Option<u64>,
    /// Whether a content checksum follows the frame
    has_checksum: bool,
    /// Whether this is a single-segment frame
    single_segment: bool,
    /// Dictionary ID (0 if none)
    dict_id: u32,
    /// Total header size (including magic number)
    header_size: usize,
}

/// Parse the frame header
fn parse_frame_header(data: &[u8], offset: usize) -> Result<FrameHeader, ZstdError> {
    if offset + 5 > data.len() {
        return Err(ZstdError::CorruptData);
    }

    // Skip magic (4 bytes)
    let mut pos = offset + 4;

    let descriptor = data[pos];
    pos += 1;

    let frame_content_size_flag = (descriptor >> 6) & 3;
    let single_segment = (descriptor >> 5) & 1 != 0;
    let _reserved = (descriptor >> 3) & 1;
    let content_checksum_flag = (descriptor >> 2) & 1 != 0;
    let dict_id_flag = descriptor & 3;

    // Window descriptor (not present if single_segment)
    let window_size = if single_segment {
        0 // Will be set from FCS
    } else {
        if pos >= data.len() {
            return Err(ZstdError::CorruptData);
        }
        let window_desc = data[pos];
        pos += 1;
        let exponent = (window_desc >> 3) as u32;
        let mantissa = (window_desc & 0x07) as u64;
        let base = 1u64 << (10 + exponent);
        base + (base >> 3) * mantissa
    };

    // Dictionary ID
    let dict_id_sizes = [0, 1, 2, 4];
    let dict_id_size = dict_id_sizes[dict_id_flag as usize];
    if pos + dict_id_size > data.len() {
        return Err(ZstdError::CorruptData);
    }
    let dict_id = match dict_id_size {
        0 => 0,
        1 => data[pos] as u32,
        2 => u16::from_le_bytes([data[pos], data[pos + 1]]) as u32,
        4 => read_le32(data, pos),
        _ => 0,
    };
    pos += dict_id_size;

    if dict_id != 0 {
        return Err(ZstdError::UnsupportedDictionary);
    }

    // Frame content size
    let fcs_sizes = [0, 1, 2, 4, 8]; // index by fcs_flag, but 0 is special
    let fcs_size = if frame_content_size_flag == 0 {
        if single_segment { 1 } else { 0 }
    } else {
        fcs_sizes[frame_content_size_flag as usize + 1]
    };

    if pos + fcs_size > data.len() {
        return Err(ZstdError::CorruptData);
    }

    let frame_content_size = if fcs_size == 0 {
        None
    } else {
        let fcs = match fcs_size {
            1 => data[pos] as u64,
            2 => u16::from_le_bytes([data[pos], data[pos + 1]]) as u64 + 256,
            4 => read_le32(data, pos) as u64,
            8 => read_le64(data, pos),
            _ => 0,
        };
        Some(fcs)
    };
    pos += fcs_size;

    let actual_window = if single_segment {
        frame_content_size.unwrap_or(0)
    } else {
        window_size
    };

    // Limit window size
    if actual_window > (1u64 << MAX_WINDOW_LOG) {
        return Err(ZstdError::WindowTooLarge);
    }

    Ok(FrameHeader {
        window_size: actual_window,
        frame_content_size,
        has_checksum: content_checksum_flag,
        single_segment,
        dict_id,
        header_size: pos - offset,
    })
}

/// Persistent state across blocks within a frame
struct BlockState {
    /// Saved Huffman table for treeless literal blocks
    huff_table: Option<HuffTable>,
    /// Previous FSE tables for repeat mode
    ll_table: Option<FseTable>,
    ml_table: Option<FseTable>,
    of_table: Option<FseTable>,
    /// Repeat offsets (initialized to 1, 4, 8)
    offsets: [u32; 3],
}

impl BlockState {
    fn new() -> Self {
        BlockState {
            huff_table: None,
            ll_table: None,
            ml_table: None,
            of_table: None,
            offsets: [1, 4, 8],
        }
    }
}

/// Decode one frame, return total bytes consumed from input.
///
/// `max_output` caps the total length of `output`; if decoding would exceed
/// it, [`ZstdError::OutputTooLarge`] is returned.
fn decode_frame(
    data: &[u8],
    offset: usize,
    output: &mut Vec<u8>,
    max_output: usize,
) -> Result<usize, ZstdError> {
    let header = parse_frame_header(data, offset)?;
    let mut pos = offset + header.header_size;

    let output_start = output.len();
    let mut state = BlockState::new();

    // Decode blocks
    loop {
        if pos + 3 > data.len() {
            return Err(ZstdError::CorruptData);
        }

        // Block header: 3 bytes
        let block_header =
            data[pos] as u32 | (data[pos + 1] as u32) << 8 | (data[pos + 2] as u32) << 16;
        pos += 3;

        let last_block = block_header & 1 != 0;
        let block_type = BlockType::from_u8(((block_header >> 1) & 3) as u8);
        let block_size = (block_header >> 3) as usize;

        if pos + block_size > data.len() && block_type != BlockType::Rle {
            return Err(ZstdError::CorruptData);
        }

        match block_type {
            BlockType::Raw => {
                if output.len().saturating_add(block_size) > max_output {
                    return Err(ZstdError::OutputTooLarge);
                }
                output.extend_from_slice(&data[pos..pos + block_size]);
                pos += block_size;
            }
            BlockType::Rle => {
                if pos >= data.len() {
                    return Err(ZstdError::CorruptData);
                }
                if output.len().saturating_add(block_size) > max_output {
                    return Err(ZstdError::OutputTooLarge);
                }
                let byte = data[pos];
                pos += 1;
                for _ in 0..block_size {
                    output.push(byte);
                }
            }
            BlockType::Compressed => {
                let block_data = &data[pos..pos + block_size];
                decode_compressed_block(block_data, output, &mut state, max_output)?;
                pos += block_size;
            }
            BlockType::Reserved => {
                return Err(ZstdError::CorruptData);
            }
        }

        if last_block {
            break;
        }
    }

    // Verify checksum if present
    if header.has_checksum {
        if pos + 4 > data.len() {
            return Err(ZstdError::CorruptData);
        }
        let stored_checksum = read_le32(data, pos);
        let computed = (xxhash64(&output[output_start..]) & 0xFFFFFFFF) as u32;
        if stored_checksum != computed {
            return Err(ZstdError::ChecksumMismatch);
        }
        pos += 4;
    }

    // Verify frame content size if known
    if let Some(fcs) = header.frame_content_size {
        let actual_size = (output.len() - output_start) as u64;
        if actual_size != fcs {
            return Err(ZstdError::CorruptData);
        }
    }

    Ok(pos - offset)
}

/// Decode a compressed block
fn decode_compressed_block(
    data: &[u8],
    output: &mut Vec<u8>,
    state: &mut BlockState,
    max_output: usize,
) -> Result<(), ZstdError> {
    let mut pos = 0;

    // 1. Literals section
    let (literals, lit_consumed) = decode_literals_section(data, state)?;
    pos += lit_consumed;

    // 2. Sequences section
    if pos >= data.len() {
        // No sequences — all literals
        if output.len().saturating_add(literals.len()) > max_output {
            return Err(ZstdError::OutputTooLarge);
        }
        output.extend_from_slice(&literals);
        return Ok(());
    }

    let (sequences, seq_consumed) = decode_sequences_section(&data[pos..], state)?;
    let _ = seq_consumed;

    // 3. Execute sequences
    execute_sequences(output, &literals, &sequences, state, max_output)?;

    Ok(())
}

/// A decoded sequence
struct Sequence {
    lit_len: u32,
    match_len: u32,
    offset: u32,
}

/// Decode the literals section of a compressed block
fn decode_literals_section(
    data: &[u8],
    state: &mut BlockState,
) -> Result<(Vec<u8>, usize), ZstdError> {
    if data.is_empty() {
        return Err(ZstdError::CorruptData);
    }

    let header_byte = data[0];
    let lit_type = LiteralsType::from_u8(header_byte & 3);
    let size_format = (header_byte >> 2) & 3;

    match lit_type {
        LiteralsType::Raw => {
            // Raw literals: just copy
            // RFC 8878 §3.1.1.3.1: size_format 0b00 or 0b10 → 1-byte header (5-bit size)
            //                       size_format 0b01         → 2-byte header (12-bit size)
            //                       size_format 0b11         → 3-byte header (20-bit size)
            let (regen_size, header_size) = match size_format {
                0 | 2 => ((header_byte >> 3) as usize, 1),
                1 => {
                    if data.len() < 2 {
                        return Err(ZstdError::CorruptData);
                    }
                    (((header_byte >> 4) as usize) | ((data[1] as usize) << 4), 2)
                }
                _ => {
                    if data.len() < 3 {
                        return Err(ZstdError::CorruptData);
                    }
                    (
                        ((header_byte >> 4) as usize)
                            | ((data[1] as usize) << 4)
                            | ((data[2] as usize) << 12),
                        3,
                    )
                }
            };

            if header_size + regen_size > data.len() {
                return Err(ZstdError::CorruptData);
            }

            let literals = data[header_size..header_size + regen_size].to_vec();
            Ok((literals, header_size + regen_size))
        }
        LiteralsType::Rle => {
            let (regen_size, header_size) = match size_format {
                0 | 2 => ((header_byte >> 3) as usize, 1),
                1 => {
                    if data.len() < 2 {
                        return Err(ZstdError::CorruptData);
                    }
                    (((header_byte >> 4) as usize) | ((data[1] as usize) << 4), 2)
                }
                _ => {
                    if data.len() < 3 {
                        return Err(ZstdError::CorruptData);
                    }
                    (
                        ((header_byte >> 4) as usize)
                            | ((data[1] as usize) << 4)
                            | ((data[2] as usize) << 12),
                        3,
                    )
                }
            };

            if header_size >= data.len() {
                return Err(ZstdError::CorruptData);
            }

            let byte = data[header_size];
            let literals = vec![byte; regen_size];
            Ok((literals, header_size + 1))
        }
        LiteralsType::Compressed | LiteralsType::Treeless => {
            // Parse compressed/treeless header
            let (regen_size, compressed_size, num_streams, header_size) = match size_format {
                0 => {
                    // Both sizes use 10 bits, 1 stream
                    if data.len() < 3 {
                        return Err(ZstdError::CorruptData);
                    }
                    let combined =
                        (data[0] as u32) | ((data[1] as u32) << 8) | ((data[2] as u32) << 16);
                    let regen = ((combined >> 4) & 0x3FF) as usize;
                    let comp = ((combined >> 14) & 0x3FF) as usize;
                    (regen, comp, 1, 3)
                }
                1 => {
                    // Both sizes use 10 bits, 4 streams
                    if data.len() < 3 {
                        return Err(ZstdError::CorruptData);
                    }
                    let combined =
                        (data[0] as u32) | ((data[1] as u32) << 8) | ((data[2] as u32) << 16);
                    let regen = ((combined >> 4) & 0x3FF) as usize;
                    let comp = ((combined >> 14) & 0x3FF) as usize;
                    (regen, comp, 4, 3)
                }
                2 => {
                    // Both sizes use 14 bits, 4 streams
                    if data.len() < 4 {
                        return Err(ZstdError::CorruptData);
                    }
                    let combined = (data[0] as u32)
                        | ((data[1] as u32) << 8)
                        | ((data[2] as u32) << 16)
                        | ((data[3] as u32) << 24);
                    let regen = ((combined >> 4) & 0x3FFF) as usize;
                    let comp = ((combined >> 18) & 0x3FFF) as usize;
                    (regen, comp, 4, 4)
                }
                _ => {
                    // Both sizes use 18 bits, 4 streams
                    if data.len() < 5 {
                        return Err(ZstdError::CorruptData);
                    }
                    let combined = (data[0] as u64)
                        | ((data[1] as u64) << 8)
                        | ((data[2] as u64) << 16)
                        | ((data[3] as u64) << 24)
                        | ((data[4] as u64) << 32);
                    let regen = ((combined >> 4) & 0x3FFFF) as usize;
                    let comp = ((combined >> 22) & 0x3FFFF) as usize;
                    (regen, comp, 4, 5)
                }
            };

            if header_size + compressed_size > data.len() {
                return Err(ZstdError::CorruptData);
            }

            let lit_data = &data[header_size..header_size + compressed_size];

            if lit_type == LiteralsType::Compressed {
                // Read new Huffman tree
                let (weights, weight_bytes) = HuffTable::read_weights(lit_data)?;
                let huff = HuffTable::build(&weights)?;

                let stream_data = &lit_data[weight_bytes..];
                let literals = if num_streams == 1 {
                    huff::decompress_1stream(&huff, stream_data, regen_size)?
                } else {
                    huff::decompress_4streams(&huff, stream_data, regen_size)?
                };

                state.huff_table = Some(huff);
                Ok((literals, header_size + compressed_size))
            } else {
                // Treeless: reuse previous Huffman table
                let huff = state.huff_table.as_ref().ok_or(ZstdError::CorruptData)?;

                let literals = if num_streams == 1 {
                    huff::decompress_1stream(huff, lit_data, regen_size)?
                } else {
                    huff::decompress_4streams(huff, lit_data, regen_size)?
                };

                Ok((literals, header_size + compressed_size))
            }
        }
    }
}

/// Decode the sequences section
fn decode_sequences_section(
    data: &[u8],
    state: &mut BlockState,
) -> Result<(Vec<Sequence>, usize), ZstdError> {
    if data.is_empty() {
        return Ok((Vec::new(), 0));
    }

    let mut pos = 0;

    // Number of sequences
    let first = data[pos] as usize;
    pos += 1;

    let num_sequences = if first == 0 {
        return Ok((Vec::new(), pos));
    } else if first < 128 {
        first
    } else if first < 255 {
        if pos >= data.len() {
            return Err(ZstdError::CorruptData);
        }
        let second = data[pos] as usize;
        pos += 1;
        ((first - 128) << 8) + second
    } else {
        // first == 255
        if pos + 1 >= data.len() {
            return Err(ZstdError::CorruptData);
        }
        let val = (data[pos] as usize) | ((data[pos + 1] as usize) << 8);
        pos += 2;
        val + 0x7F00
    };

    if num_sequences == 0 {
        return Ok((Vec::new(), pos));
    }

    // Symbol compression modes byte
    if pos >= data.len() {
        return Err(ZstdError::CorruptData);
    }
    let modes_byte = data[pos];
    pos += 1;

    let ll_mode = SeqMode::from_u8((modes_byte >> 6) & 3);
    let of_mode = SeqMode::from_u8((modes_byte >> 4) & 3);
    let ml_mode = SeqMode::from_u8((modes_byte >> 2) & 3);

    // Build/reuse FSE tables for each symbol type
    let ll_table = decode_seq_table(
        &data[pos..],
        ll_mode,
        &state.ll_table,
        &LL_DEFAULT_DIST,
        LL_DEFAULT_AL,
        LL_MAX_SYMBOL,
        LL_MAX_AL,
        &mut pos,
    )?;
    let of_table = decode_seq_table(
        &data[pos..],
        of_mode,
        &state.of_table,
        &OF_DEFAULT_DIST,
        OF_DEFAULT_AL,
        OF_MAX_SYMBOL,
        OF_MAX_AL,
        &mut pos,
    )?;
    let ml_table = decode_seq_table(
        &data[pos..],
        ml_mode,
        &state.ml_table,
        &ML_DEFAULT_DIST,
        ML_DEFAULT_AL,
        ML_MAX_SYMBOL,
        ML_MAX_AL,
        &mut pos,
    )?;

    // Decode sequences from backward bitstream
    let bitstream_data = &data[pos..];
    let mut reader = BitReader::init(bitstream_data)?;

    // Initialize states
    let ll_al = ll_table.accuracy_log as u32;
    let of_al = of_table.accuracy_log as u32;
    let ml_al = ml_table.accuracy_log as u32;

    let mut ll_state = reader.read_bits(ll_al)?;
    let mut of_state = reader.read_bits(of_al)?;
    let mut ml_state = reader.read_bits(ml_al)?;

    let mut sequences = Vec::with_capacity(num_sequences);

    for i in 0..num_sequences {
        // Peek symbols
        let of_code = of_table.peek_symbol(of_state);
        let ml_code = ml_table.peek_symbol(ml_state);
        let ll_code = ll_table.peek_symbol(ll_state);

        // Read extra bits for offset
        let of_bits = of_code as u32;
        let of_extra = reader.read_bits(of_bits)?;
        let offset = (1u32 << of_bits) + of_extra;

        // Read extra bits for match length
        let ml_extra_bits = ML_EXTRA_BITS[ml_code as usize] as u32;
        let ml_extra = reader.read_bits(ml_extra_bits)?;
        let match_len = ML_BASELINES[ml_code as usize] + ml_extra;

        // Read extra bits for literal length
        let ll_extra_bits = LL_EXTRA_BITS[ll_code as usize] as u32;
        let ll_extra = reader.read_bits(ll_extra_bits)?;
        let lit_len = LL_BASELINES[ll_code as usize] + ll_extra;

        sequences.push(Sequence {
            lit_len,
            match_len,
            offset,
        });

        // Update states (not for the last sequence)
        if i < num_sequences - 1 {
            ll_table.decode(&mut ll_state, &mut reader)?;
            ml_table.decode(&mut ml_state, &mut reader)?;
            of_table.decode(&mut of_state, &mut reader)?;
        }
    }

    // Save tables for potential repeat mode in next block
    state.ll_table = Some(ll_table);
    state.of_table = Some(of_table);
    state.ml_table = Some(ml_table);

    Ok((sequences, data.len()))
}

/// Decode an FSE table for a specific sequence type
#[allow(clippy::too_many_arguments)] // mirrors the per-type parameters in RFC 8878
fn decode_seq_table(
    data: &[u8],
    mode: SeqMode,
    prev_table: &Option<FseTable>,
    default_probs: &[i16],
    default_al: u8,
    max_symbol: usize,
    max_al: u8,
    pos: &mut usize,
) -> Result<FseTable, ZstdError> {
    match mode {
        SeqMode::Predefined => build_predefined_table(default_probs, default_al),
        SeqMode::Rle => {
            if data.is_empty() {
                return Err(ZstdError::CorruptData);
            }
            let symbol = data[0];
            *pos += 1;
            // Build a table that always returns this symbol
            let mut probs = vec![0i16; symbol as usize + 1];
            probs[symbol as usize] = 1;
            FseTable::build(&probs, 0)
        }
        SeqMode::FseCompressed => {
            let (probs, accuracy_log, consumed) =
                FseTable::read_table_description(data, max_symbol, max_al)?;
            *pos += consumed;
            FseTable::build(&probs, accuracy_log)
        }
        SeqMode::Repeat => {
            // Reuse previous table
            match prev_table {
                Some(prev) => {
                    // Clone the table
                    Ok(FseTable {
                        entries: prev.entries.clone(),
                        accuracy_log: prev.accuracy_log,
                    })
                }
                None => Err(ZstdError::CorruptData),
            }
        }
    }
}

/// Execute decoded sequences to produce the output
fn execute_sequences(
    output: &mut Vec<u8>,
    literals: &[u8],
    sequences: &[Sequence],
    state: &mut BlockState,
    max_output: usize,
) -> Result<(), ZstdError> {
    let mut lit_pos = 0;

    for seq in sequences {
        // Copy literals
        let lit_end = lit_pos + seq.lit_len as usize;
        if lit_end > literals.len() {
            return Err(ZstdError::CorruptData);
        }
        if output.len().saturating_add(seq.lit_len as usize) > max_output {
            return Err(ZstdError::OutputTooLarge);
        }
        output.extend_from_slice(&literals[lit_pos..lit_end]);
        lit_pos = lit_end;

        // Resolve offset (handle repeat offsets)
        let actual_offset = resolve_offset(seq.offset, seq.lit_len, &mut state.offsets)?;

        // Copy match
        if actual_offset == 0 || actual_offset as usize > output.len() {
            return Err(ZstdError::CorruptData);
        }
        if output.len().saturating_add(seq.match_len as usize) > max_output {
            return Err(ZstdError::OutputTooLarge);
        }

        let match_start = output.len() - actual_offset as usize;
        for i in 0..seq.match_len as usize {
            let byte = output[match_start + i];
            output.push(byte);
        }
    }

    // Copy remaining literals after all sequences
    if lit_pos < literals.len() {
        if output.len().saturating_add(literals.len() - lit_pos) > max_output {
            return Err(ZstdError::OutputTooLarge);
        }
        output.extend_from_slice(&literals[lit_pos..]);
    }

    Ok(())
}

/// Resolve an offset value, handling repeat offsets.
///
/// Offset codes in zstd:
/// - offset > 3: actual_offset = offset - 3
/// - offset 1-3: repeat offset with special handling when lit_len == 0
fn resolve_offset(offset: u32, lit_len: u32, offsets: &mut [u32; 3]) -> Result<u32, ZstdError> {
    let actual = if offset > 3 {
        // New offset
        let real_offset = offset - 3;
        // Shift repeat offsets
        offsets[2] = offsets[1];
        offsets[1] = offsets[0];
        offsets[0] = real_offset;
        real_offset
    } else if lit_len > 0 {
        // Repeat offset (standard case)
        match offset {
            1 => {
                // offset1: no change needed
                offsets[0]
            }
            2 => {
                // offset2: swap 1 and 2
                offsets.swap(0, 1);
                offsets[0]
            }
            3 => {
                // offset3: rotate
                let tmp = offsets[2];
                offsets[2] = offsets[1];
                offsets[1] = offsets[0];
                offsets[0] = tmp;
                tmp
            }
            _ => return Err(ZstdError::CorruptData),
        }
    } else {
        // lit_len == 0: special repeat offset meaning
        match offset {
            1 => {
                // offset2
                offsets.swap(0, 1);
                offsets[0]
            }
            2 => {
                // offset3
                let tmp = offsets[2];
                offsets[2] = offsets[1];
                offsets[1] = offsets[0];
                offsets[0] = tmp;
                tmp
            }
            3 => {
                // offset1 - 1
                let tmp = offsets[0].wrapping_sub(1);
                if tmp == 0 {
                    return Err(ZstdError::CorruptData);
                }
                offsets[2] = offsets[1];
                offsets[1] = offsets[0];
                offsets[0] = tmp;
                tmp
            }
            _ => return Err(ZstdError::CorruptData),
        }
    };

    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_offset_new() {
        let mut offsets = [1, 4, 8];
        let actual = resolve_offset(10, 5, &mut offsets).unwrap();
        assert_eq!(actual, 7); // 10 - 3
        assert_eq!(offsets, [7, 1, 4]);
    }

    #[test]
    fn test_resolve_offset_repeat1() {
        let mut offsets = [5, 4, 8];
        let actual = resolve_offset(1, 3, &mut offsets).unwrap();
        assert_eq!(actual, 5); // repeat offset 1
        assert_eq!(offsets, [5, 4, 8]);
    }

    #[test]
    fn test_resolve_offset_repeat2() {
        let mut offsets = [5, 4, 8];
        let actual = resolve_offset(2, 3, &mut offsets).unwrap();
        assert_eq!(actual, 4); // repeat offset 2
        assert_eq!(offsets, [4, 5, 8]);
    }

    #[test]
    fn test_resolve_offset_repeat3() {
        let mut offsets = [5, 4, 8];
        let actual = resolve_offset(3, 3, &mut offsets).unwrap();
        assert_eq!(actual, 8); // repeat offset 3
        assert_eq!(offsets, [8, 5, 4]);
    }

    #[test]
    fn test_resolve_offset_litlen0_offset1() {
        let mut offsets = [5, 4, 8];
        let actual = resolve_offset(1, 0, &mut offsets).unwrap();
        assert_eq!(actual, 4); // offset2 when litlen==0
        assert_eq!(offsets, [4, 5, 8]);
    }

    #[test]
    fn test_predefined_ll_table_entries() {
        use crate::common::*;
        use crate::fse;
        let table = fse::build_predefined_table(&LL_DEFAULT_DIST, LL_DEFAULT_AL).unwrap();
        // Check some key entries
        eprintln!("LL table ({} entries):", table.entries.len());
        for (i, e) in table.entries.iter().enumerate() {
            eprintln!(
                "  state {}: sym={} nb={} baseline={}",
                i, e.symbol, e.num_bits, e.baseline
            );
        }
        // State 24 should map to symbol 2
        assert_eq!(table.entries[24].symbol, 2);
        // State 15 - what symbol?
        eprintln!("State 15 symbol: {}", table.entries[15].symbol);
    }

    #[test]
    fn test_decompress_tar_roundtrip() {
        // Build a realistic tar-like buffer (10240 bytes = 20 tar blocks)
        let mut data = vec![0u8; 10240];
        // Tar header block (512 bytes) with typical fields
        let name = b"./usr/bin/test.sh";
        data[..name.len()].copy_from_slice(name);
        let mode = b"0100755\0";
        data[100..108].copy_from_slice(mode);
        let uid = b"0001000\0";
        data[108..116].copy_from_slice(uid);
        let gid = b"0001000\0";
        data[116..124].copy_from_slice(gid);
        let size = b"00000000023\0";
        data[124..136].copy_from_slice(size);
        let magic = b"ustar\x0000";
        data[257..265].copy_from_slice(magic);
        // File content in second block
        let content = b"#!/bin/sh\necho hello\n";
        data[512..512 + content.len()].copy_from_slice(content);
        // Rest is zeros (tar end-of-archive marker)

        let compressed = crate::compress(&data, 3);
        let result = crate::decompress(&compressed);
        match &result {
            Ok(decompressed) => {
                assert_eq!(decompressed.len(), data.len(), "wrong output length");
                assert_eq!(&decompressed[..], &data[..], "data mismatch");
            }
            Err(e) => {
                panic!("decompress tar roundtrip failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_decompress_level19() {
        let compressed: &[u8] = &[
            0x28, 0xb5, 0x2f, 0xfd, 0x64, 0xb6, 0x02, 0xd5, 0x00, 0x00, 0x98, 0x48, 0x65, 0x6c,
            0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x20, 0x54, 0x65, 0x73, 0x74,
            0x2e, 0x20, 0x01, 0x00, 0x41, 0x5b, 0x35, 0xc3, 0xb6, 0x33, 0xe3, 0x0e,
        ];
        let result = crate::decompress(compressed);
        match &result {
            Ok(data) => {
                assert_eq!(data.len(), 950, "wrong output length: {}", data.len());
            }
            Err(e) => {
                panic!("decompress level 19 failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_decompress_system_zstd_6000bytes() {
        // System zstd v1.5.7 output for A*2000 + B*2000 + A*2000
        let compressed: &[u8] = &[
            0x28, 0xb5, 0x2f, 0xfd, 0x64, 0x70, 0x16, 0x7d, 0x00, 0x00, 0x20, 0x41, 0x41, 0x42,
            0x41, 0x03, 0x14, 0x00, 0x2e, 0xcc, 0x0b, 0xf3, 0x77, 0x79, 0x2c, 0x8c, 0x4f, 0x3e,
            0x6d,
        ];
        let mut expected = Vec::new();
        expected.resize(2000, b'A');
        expected.resize(4000, b'B');
        expected.resize(6000, b'A');

        let result = crate::decompress(compressed);
        match &result {
            Ok(data) => {
                assert_eq!(data.len(), 6000, "wrong output length: {}", data.len());
                assert_eq!(data, &expected);
            }
            Err(e) => {
                panic!("decompress 6000 failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_decompress_system_zstd_text_177() {
        // System zstd v1.5.7, default level, compressing 177 bytes of text
        // This exercises Huffman-compressed literals with FSE-compressed weights
        let compressed: &[u8] = &[
            0x28, 0xb5, 0x2f, 0xfd, 0x24, 0xb1, 0xc5, 0x03, 0x00, 0x22, 0x49, 0x1b, 0x1b, 0x70,
            0xc5, 0x69, 0x03, 0xda, 0x2b, 0x57, 0x6d, 0x37, 0x9e, 0xb6, 0x11, 0x9b, 0xc8, 0x7e,
            0x49, 0xbc, 0x3d, 0xf0, 0x3f, 0x80, 0x28, 0xde, 0x80, 0x2e, 0x9c, 0x17, 0x01, 0x24,
            0x81, 0x38, 0x18, 0x0a, 0x84, 0x81, 0x0f, 0xa5, 0xbe, 0xcc, 0x48, 0x5c, 0x87, 0xa6,
            0xfa, 0x64, 0x1a, 0x78, 0x9c, 0xa8, 0x58, 0x2f, 0xbf, 0x9b, 0x30, 0x7e, 0x80, 0xad,
            0xed, 0x99, 0xa7, 0xd1, 0x7f, 0xfe, 0xc2, 0x47, 0x3a, 0x43, 0x78, 0xee, 0xfe, 0xb9,
            0x37, 0x00, 0x57, 0xe7, 0x53, 0xb2, 0x5c, 0xe3, 0x7a, 0xef, 0x7b, 0x78, 0xbc, 0xa4,
            0x06, 0xef, 0x5e, 0xd6, 0x96, 0xaf, 0x4c, 0xf1, 0xea, 0xd7, 0x89, 0xd5, 0xac, 0xb6,
            0x28, 0x2c, 0x40, 0x2a, 0x71, 0x19, 0x95, 0x3a, 0x12, 0x02, 0x00, 0x59, 0x11, 0xfc,
            0x66, 0x0d, 0x51, 0xbe, 0x14, 0xa1, 0xf3,
        ];
        let expected = b"Hello world! This is a test file with enough content to trigger sequence encoding. We need repetitive patterns for compression to work well. ABCDEFGH ABCDEFGH ABCDEFGH ABCDEFGH\n";
        let result = crate::decompress(compressed);
        match &result {
            Ok(data) => {
                assert_eq!(data.len(), 177, "wrong output length: {}", data.len());
                assert_eq!(data, expected);
            }
            Err(e) => {
                panic!("decompress system zstd text 177 failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_decompress_system_zstd_150bytes() {
        // System zstd v1.5.7 output for A*50 + B*50 + A*50
        let compressed: &[u8] = &[
            0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x96, 0x65, 0x00, 0x00, 0x20, 0x41, 0x41, 0x42, 0x41,
            0x03, 0x14, 0x00, 0x25, 0x8a, 0x37, 0x2c, 0x95, 0xce, 0x1f, 0xae,
        ];
        let mut expected = Vec::new();
        expected.resize(50, b'A');
        expected.resize(100, b'B');
        expected.resize(150, b'A');

        let result = crate::decompress(compressed);
        match &result {
            Ok(data) => {
                assert_eq!(data.len(), 150, "wrong output length: {}", data.len());
                assert_eq!(data, &expected, "output data mismatch");
            }
            Err(e) => {
                panic!("decompress failed: {:?}", e);
            }
        }
    }

    #[test]
    fn test_ruzstd_comparison_177() {
        // Compare ruzstd (known-good) decompression with our weight decoding
        let compressed: &[u8] = &[
            0x28, 0xb5, 0x2f, 0xfd, 0x24, 0xb1, 0xc5, 0x03, 0x00, 0x22, 0x49, 0x1b, 0x1b, 0x70,
            0xc5, 0x69, 0x03, 0xda, 0x2b, 0x57, 0x6d, 0x37, 0x9e, 0xb6, 0x11, 0x9b, 0xc8, 0x7e,
            0x49, 0xbc, 0x3d, 0xf0, 0x3f, 0x80, 0x28, 0xde, 0x80, 0x2e, 0x9c, 0x17, 0x01, 0x24,
            0x81, 0x38, 0x18, 0x0a, 0x84, 0x81, 0x0f, 0xa5, 0xbe, 0xcc, 0x48, 0x5c, 0x87, 0xa6,
            0xfa, 0x64, 0x1a, 0x78, 0x9c, 0xa8, 0x58, 0x2f, 0xbf, 0x9b, 0x30, 0x7e, 0x80, 0xad,
            0xed, 0x99, 0xa7, 0xd1, 0x7f, 0xfe, 0xc2, 0x47, 0x3a, 0x43, 0x78, 0xee, 0xfe, 0xb9,
            0x37, 0x00, 0x57, 0xe7, 0x53, 0xb2, 0x5c, 0xe3, 0x7a, 0xef, 0x7b, 0x78, 0xbc, 0xa4,
            0x06, 0xef, 0x5e, 0xd6, 0x96, 0xaf, 0x4c, 0xf1, 0xea, 0xd7, 0x89, 0xd5, 0xac, 0xb6,
            0x28, 0x2c, 0x40, 0x2a, 0x71, 0x19, 0x95, 0x3a, 0x12, 0x02, 0x00, 0x59, 0x11, 0xfc,
            0x66, 0x0d, 0x51, 0xbe, 0x14, 0xa1, 0xf3,
        ];
        // ruzstd should decompress successfully
        use std::io::Read;
        let mut cursor = std::io::Cursor::new(compressed);
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut cursor).unwrap();
        let mut ruzstd_output = Vec::new();
        decoder.read_to_end(&mut ruzstd_output).unwrap();
        eprintln!("[RUZSTD] decompressed {} bytes", ruzstd_output.len());
        assert_eq!(ruzstd_output.len(), 177);
        let expected = b"Hello world! This is a test file with enough content to trigger sequence encoding. We need repetitive patterns for compression to work well. ABCDEFGH ABCDEFGH ABCDEFGH ABCDEFGH\n";
        assert_eq!(&ruzstd_output, expected);
    }
}
