//! FSE (Finite State Entropy) — tANS variant
//!
//! The core entropy coder used in zstd for encoding sequence headers
//! (literal lengths, match lengths, offsets).

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::bitstream::{BitReader, BitWriter};
use crate::common::ZstdError;

// ============================================================================
// Decoding
// ============================================================================

/// A single entry in an FSE decoding table
#[derive(Clone, Copy, Debug)]
pub struct FseEntry {
    pub symbol: u8,
    pub num_bits: u8,
    pub baseline: u16,
}

/// FSE decoding table
pub struct FseTable {
    pub entries: Vec<FseEntry>,
    pub accuracy_log: u8,
}

impl FseTable {
    /// Build a decoding table from a probability distribution.
    ///
    /// `probs[i]` = probability of symbol i. -1 means "less than 1" (the symbol
    /// gets exactly one cell and uses max bits).
    pub fn build(probs: &[i16], accuracy_log: u8) -> Result<Self, ZstdError> {
        let table_size = 1usize << accuracy_log;
        let mut entries = vec![
            FseEntry {
                symbol: 0,
                num_bits: 0,
                baseline: 0
            };
            table_size
        ];

        // Step 1: place "less than 1" symbols at the end
        let mut high_threshold = table_size - 1;
        for (symbol, &prob) in probs.iter().enumerate() {
            if prob == -1 {
                entries[high_threshold].symbol = symbol as u8;
                high_threshold -= 1;
            }
        }

        // Step 2: spread symbols using the step formula
        let step = (table_size >> 1) + (table_size >> 3) + 3;
        let mask = table_size - 1;
        let mut pos = 0usize;

        for (symbol, &prob) in probs.iter().enumerate() {
            if prob <= 0 {
                continue; // skip "less than 1" (placed above) and zero-probability
            }
            for _ in 0..prob as usize {
                entries[pos].symbol = symbol as u8;
                pos = (pos + step) & mask;
                // Skip positions occupied by "less than 1" symbols
                while pos > high_threshold {
                    pos = (pos + step) & mask;
                }
            }
        }

        // Verify: pos should be 0 after placing all symbols
        if pos != 0 {
            return Err(ZstdError::CorruptData);
        }

        // Step 3: build decoding metadata (num_bits and baseline)
        // Count how many times each symbol appears
        let mut symbol_next = vec![0u16; probs.len()];
        for (sym, &prob) in probs.iter().enumerate() {
            if prob == -1 {
                symbol_next[sym] = 1;
            } else if prob > 0 {
                symbol_next[sym] = prob as u16;
            }
        }

        for i in 0..table_size {
            let sym = entries[i].symbol as usize;
            let next_state = symbol_next[sym];
            symbol_next[sym] += 1;

            let nb = accuracy_log as u32 - (32 - (next_state as u32).leading_zeros() - 1);
            entries[i].num_bits = nb as u8;
            entries[i].baseline =
                ((next_state as u16) << nb as u16).wrapping_sub(table_size as u16);
        }

        Ok(FseTable {
            entries,
            accuracy_log,
        })
    }

    /// Decode one symbol using the current state, then update state
    pub fn decode(&self, state: &mut u32, reader: &mut BitReader) -> Result<u8, ZstdError> {
        let entry = &self.entries[*state as usize];
        let symbol = entry.symbol;
        let bits = reader.read_bits(entry.num_bits as u32)?;
        *state = entry.baseline as u32 + bits;
        Ok(symbol)
    }

    /// Peek at the symbol for the current state without updating
    pub fn peek_symbol(&self, state: u32) -> u8 {
        self.entries[state as usize].symbol
    }

    /// Read an FSE table description from a forward bitstream (compressed header).
    ///
    /// Returns (table, bytes_consumed).
    pub fn read_table_description(
        data: &[u8],
        max_symbol: usize,
        max_accuracy_log: u8,
    ) -> Result<(Vec<i16>, u8, usize), ZstdError> {
        if data.is_empty() {
            return Err(ZstdError::CorruptData);
        }

        // Read accuracy log from first 4 bits
        let accuracy_log = (data[0] & 0x0F) + 5;
        if accuracy_log > max_accuracy_log {
            return Err(ZstdError::CorruptData);
        }

        let table_size = 1i32 << accuracy_log;
        let mut remaining = table_size + 1;
        let mut probs = Vec::new();

        // Forward bit reading for the header
        let mut bit_pos: usize = 4; // start after the 4-bit accuracy log

        // Match zstd reference: threshold = tableSize, nbBits = tableLog + 1
        let mut threshold = table_size;
        let mut nb_bits = accuracy_log as u32 + 1;

        loop {
            if probs.len() > max_symbol {
                break;
            }

            // Special: if remaining is small enough, the rest are zeros
            if remaining <= 1 {
                // Fill remaining symbols with 0
                while probs.len() <= max_symbol {
                    probs.push(0);
                }
                break;
            }

            // Read bits from the forward bitstream
            let byte_idx = bit_pos / 8;
            let bit_idx = bit_pos % 8;

            if byte_idx >= data.len() {
                return Err(ZstdError::CorruptData);
            }

            // Read up to 24 bits starting from current position
            let mut raw = data[byte_idx] as u32 >> bit_idx;
            let mut available = 8 - bit_idx as u32;
            if byte_idx + 1 < data.len() {
                raw |= (data[byte_idx + 1] as u32) << available;
                available += 8;
            }
            if byte_idx + 2 < data.len() {
                raw |= (data[byte_idx + 2] as u32) << available;
            }

            // Variable-length decoding per zstd spec:
            // max = 2*threshold - 1 - remaining
            // Short form: low bits (nb_bits-1 bits) < max → count = low bits, consume nb_bits-1
            // Long form: read nb_bits bits, if value >= threshold then subtract max
            let max_val = (2 * threshold - 1 - remaining) as u32;
            let low_mask = (1u32 << (nb_bits - 1)) - 1;
            let low_val = raw & low_mask;

            let (count, bits_used) = if low_val < max_val {
                // Short form
                (low_val, (nb_bits - 1) as usize)
            } else {
                // Long form
                let high_val = raw & ((1u32 << nb_bits) - 1);
                if high_val >= threshold as u32 {
                    (high_val - max_val, nb_bits as usize)
                } else {
                    (high_val, nb_bits as usize)
                }
            };

            bit_pos += bits_used;

            // count: 0 means probability = -1 (less than 1)
            // 1 means probability = 0
            // n means probability = n-1
            let prob = if count == 0 {
                -1i16
            } else {
                (count as i16) - 1
            };

            probs.push(prob);

            // Update remaining
            if prob == -1 {
                remaining -= 1;
            } else {
                remaining -= prob as i32;
            }

            if remaining < 0 {
                return Err(ZstdError::CorruptData);
            }

            while remaining < threshold {
                nb_bits -= 1;
                threshold >>= 1;
            }

            // Check for zero-repeat encoding
            if prob == 0 {
                // Read repeat count
                loop {
                    let byte_idx = bit_pos / 8;
                    let bit_idx = bit_pos % 8;
                    if byte_idx >= data.len() {
                        break;
                    }
                    // Read 2 bits, handling byte boundary spanning
                    let mut raw = (data[byte_idx] as u32) >> bit_idx;
                    if bit_idx >= 7 && byte_idx + 1 < data.len() {
                        raw |= (data[byte_idx + 1] as u32) << (8 - bit_idx);
                    }
                    let repeat_bits = raw & 3;
                    bit_pos += 2;

                    for _ in 0..repeat_bits {
                        probs.push(0);
                    }

                    if repeat_bits < 3 {
                        break;
                    }
                }
            }
        }

        // Truncate to max_symbol + 1
        probs.truncate(max_symbol + 1);

        let bytes_consumed = (bit_pos + 7) / 8;
        Ok((probs, accuracy_log as u8, bytes_consumed))
    }
}

/// Build a predefined FSE decoding table from the default distributions
pub fn build_predefined_table(probs: &[i16], accuracy_log: u8) -> Result<FseTable, ZstdError> {
    FseTable::build(probs, accuracy_log)
}

// ============================================================================
// Encoding
// ============================================================================

/// FSE encoding table entry
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub struct FseEncEntry {
    /// Number of bits to output
    pub num_bits: u8,
    /// New state delta
    pub new_state: u16,
}

/// FSE encoding table
#[cfg(feature = "alloc")]
pub struct FseEncTable {
    /// For each symbol, a list of encoding entries indexed by sub-state
    pub symbol_table: Vec<Vec<FseEncEntry>>,
    pub accuracy_log: u8,
}

#[cfg(feature = "alloc")]
impl FseEncTable {
    /// Build an encoding table from normalized probabilities
    pub fn build(probs: &[i16], accuracy_log: u8) -> Result<Self, ZstdError> {
        // First build the decoding table to get the symbol-to-state mapping
        let dec_table = FseTable::build(probs, accuracy_log)?;

        // For each symbol, collect the states that map to it (in order)
        let num_symbols = probs.len();
        let mut symbol_states: Vec<Vec<u16>> = vec![Vec::new(); num_symbols];
        for (state, entry) in dec_table.entries.iter().enumerate() {
            symbol_states[entry.symbol as usize].push(state as u16);
        }

        // Build encoding table: for each symbol, we need to know how to
        // reach each state that has that symbol
        let mut symbol_table = Vec::with_capacity(num_symbols);

        for (sym, &prob) in probs.iter().enumerate() {
            let count = if prob == -1 {
                1
            } else if prob > 0 {
                prob as usize
            } else {
                0
            };

            let mut entries = Vec::with_capacity(count);

            if count > 0 {
                // The states for this symbol, sorted
                let states = &symbol_states[sym];

                for (idx, &target_state) in states.iter().enumerate() {
                    // When encoding symbol `sym` from sub-state `idx`:
                    // We need to emit enough bits to make the transition deterministic
                    let nb =
                        accuracy_log as u32 - (32 - ((idx + count) as u32).leading_zeros() - 1);
                    let new_state = target_state;
                    entries.push(FseEncEntry {
                        num_bits: nb as u8,
                        new_state,
                    });
                }
            }

            symbol_table.push(entries);
        }

        Ok(FseEncTable {
            symbol_table,
            accuracy_log,
        })
    }

    /// Encode a symbol: emit bits and return new state
    pub fn encode(&self, state: &mut u32, symbol: u8, writer: &mut BitWriter) {
        let sym = symbol as usize;
        if sym >= self.symbol_table.len() || self.symbol_table[sym].is_empty() {
            return;
        }

        let entries = &self.symbol_table[sym];
        let count = entries.len() as u32;

        // Determine how many bits to output and which sub-entry to use
        // State encodes which sub-entry: sub_state = state / (table_size / count)
        let nb = self.accuracy_log as u32 - (32 - count.leading_zeros() - 1);
        let low_bits = *state & ((1u32 << nb) - 1);
        writer.write_bits(low_bits, nb);

        let sub_state = (*state >> nb) as usize;
        if sub_state < entries.len() {
            *state = entries[sub_state].new_state as u32;
        }
    }

    /// Initialize state for encoding (use the first valid state for the given symbol)
    pub fn init_state(&self, symbol: u8) -> u32 {
        let sym = symbol as usize;
        if sym < self.symbol_table.len() && !self.symbol_table[sym].is_empty() {
            self.symbol_table[sym][0].new_state as u32
        } else {
            0
        }
    }
}

/// Normalize a histogram to sum to `table_size = 1 << table_log`.
///
/// Returns normalized probabilities where -1 means "less than 1".
#[cfg(feature = "alloc")]
pub fn normalize_counts(counts: &[u32], max_symbol: usize, table_log: u8) -> Vec<i16> {
    let table_size = 1u32 << table_log;
    let total: u32 = counts[..=max_symbol].iter().sum();

    if total == 0 {
        return vec![0i16; max_symbol + 1];
    }

    let mut probs = vec![0i16; max_symbol + 1];
    let mut distributed = 0i32;
    let mut largest_idx = 0usize;
    let mut largest_count = 0u32;

    for i in 0..=max_symbol {
        if counts[i] == 0 {
            probs[i] = 0;
            continue;
        }

        if counts[i] > largest_count {
            largest_count = counts[i];
            largest_idx = i;
        }

        // Scale proportionally
        let prob = ((counts[i] as u64 * table_size as u64) / total as u64) as i16;
        if prob == 0 {
            // Symbol appears but rounds to 0 — assign -1 (less than 1)
            probs[i] = -1;
            distributed += 1;
        } else {
            probs[i] = prob;
            distributed += prob as i32;
        }
    }

    // Adjust largest symbol to make sum == table_size
    let correction = table_size as i32 - distributed;
    probs[largest_idx] += correction as i16;
    if probs[largest_idx] <= 0 {
        probs[largest_idx] = 1;
    }

    probs
}

/// Write normalized probabilities as an FSE table description.
/// Returns the serialized bytes.
#[cfg(feature = "alloc")]
pub fn write_table_description(probs: &[i16], accuracy_log: u8) -> Vec<u8> {
    let table_size = 1i32 << accuracy_log;
    let mut output = Vec::new();
    let mut bit_buf: u64 = 0;
    let mut bit_count: u32 = 0;

    // First 4 bits: accuracy_log - 5
    bit_buf |= ((accuracy_log - 5) as u64) & 0x0F;
    bit_count += 4;

    // Match zstd reference: threshold = tableSize, nbBits = tableLog + 1
    let mut remaining = table_size + 1;
    let mut threshold = table_size;
    let mut nb_bits = accuracy_log as u32 + 1;

    let mut i = 0;
    while i < probs.len() {
        // Stop when remaining probability budget is exhausted
        if remaining <= 1 {
            break;
        }

        let prob = probs[i];

        // The count to write: prob == -1 => 0, prob == 0 => 1, prob == n => n+1
        let count = if prob == -1 { 0u32 } else { (prob + 1) as u32 };

        // Variable-length encoding per zstd spec:
        // max = 2*threshold - 1 - remaining
        // Short form (nb_bits-1 bits): count < max
        // Long form (nb_bits bits): count >= max
        let max_val = (2 * threshold - 1 - remaining) as u32;

        if count < max_val {
            // Short form: write count in nb_bits-1 bits
            bit_buf |= (count as u64) << bit_count;
            bit_count += nb_bits - 1;
        } else {
            // Long form: write in nb_bits bits
            let adjusted = if count < threshold as u32 {
                count
            } else {
                count + max_val
            };
            bit_buf |= (adjusted as u64) << bit_count;
            bit_count += nb_bits;
        }

        // Flush complete bytes
        while bit_count >= 8 {
            output.push(bit_buf as u8);
            bit_buf >>= 8;
            bit_count -= 8;
        }

        // Update remaining
        if prob == -1 {
            remaining -= 1;
        } else if prob > 0 {
            remaining -= prob as i32;
        }

        while remaining < threshold {
            nb_bits -= 1;
            threshold >>= 1;
        }

        // Handle zero-repeat encoding
        if prob == 0 {
            // Count consecutive zeros
            let mut repeat = 0u32;
            i += 1;
            while i < probs.len() && probs[i] == 0 {
                repeat += 1;
                i += 1;
            }

            // Write repeat count in groups of 2 bits (0-2 = exact, 3 = continue)
            loop {
                let chunk = core::cmp::min(repeat, 3);
                bit_buf |= (chunk as u64) << bit_count;
                bit_count += 2;
                while bit_count >= 8 {
                    output.push(bit_buf as u8);
                    bit_buf >>= 8;
                    bit_count -= 8;
                }
                if chunk < 3 {
                    break;
                }
                repeat -= 3;
            }
            continue;
        }

        i += 1;
    }

    // Flush remaining bits
    if bit_count > 0 {
        output.push(bit_buf as u8);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_build_predefined_ll_table() {
        let table = FseTable::build(&common::LL_DEFAULT_DIST, common::LL_DEFAULT_AL).unwrap();
        assert_eq!(table.entries.len(), 1 << common::LL_DEFAULT_AL);
    }

    #[test]
    fn test_build_predefined_ml_table() {
        let table = FseTable::build(&common::ML_DEFAULT_DIST, common::ML_DEFAULT_AL).unwrap();
        assert_eq!(table.entries.len(), 1 << common::ML_DEFAULT_AL);
    }

    #[test]
    fn test_build_predefined_of_table() {
        let table = FseTable::build(&common::OF_DEFAULT_DIST, common::OF_DEFAULT_AL).unwrap();
        assert_eq!(table.entries.len(), 1 << common::OF_DEFAULT_AL);
    }

    #[test]
    fn test_normalize_counts() {
        let counts = [10u32, 20, 5, 1, 0, 0];
        let probs = normalize_counts(&counts, 3, 6);
        let sum: i32 = probs
            .iter()
            .map(|&p| if p == -1 { 1 } else { p as i32 })
            .sum();
        assert_eq!(sum, 64); // 2^6
    }
}

#[test]
fn test_fse_table_description_roundtrip() {
    // accuracy_log must be >= 5 for zstd (4 bits encode al-5)
    // table_size = 2^6 = 64. Sum of probs (with -1 counting as 1) must equal 64
    let probs_in = vec![
        10i16, 8, 6, 5, 5, 4, 4, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, -1, -1,
    ];
    // sum: 10+8+6+5+5+4+4+3+3+3+2+2+2+2+1+1+1+1+1 = 64 ✓
    let al = 6u8;
    let serialized = write_table_description(&probs_in, al);

    let (probs_out, al_out, consumed) =
        FseTable::read_table_description(&serialized, 20, 9).unwrap();

    assert_eq!(al_out, al, "accuracy log mismatch");
    assert_eq!(
        consumed,
        serialized.len(),
        "bytes consumed must match serialized length"
    );
    for (i, &p) in probs_in.iter().enumerate() {
        assert_eq!(
            probs_out.get(i).copied().unwrap_or(0),
            p,
            "prob mismatch at index {}: expected {}, got {:?}",
            i,
            p,
            probs_out.get(i)
        );
    }
}

#[test]
fn test_fse_table_description_roundtrip_with_zeros() {
    // Distribution with zeros (zero-repeat encoding)
    // table_size = 2^5 = 32
    let probs_in = vec![8i16, 4, 0, 0, 0, 3, 3, 2, 2, 2, 2, 1, 1, 1, -1, -1, -1];
    // sum: 8+4+0+0+0+3+3+2+2+2+2+1+1+1+1+1+1 = 32 ✓
    let al = 5u8;
    let serialized = write_table_description(&probs_in, al);

    let (probs_out, al_out, consumed) =
        FseTable::read_table_description(&serialized, 20, 9).unwrap();

    assert_eq!(al_out, al, "accuracy log mismatch");
    assert_eq!(
        consumed,
        serialized.len(),
        "bytes consumed must match serialized length"
    );
    for (i, &p) in probs_in.iter().enumerate() {
        assert_eq!(
            probs_out.get(i).copied().unwrap_or(0),
            p,
            "prob mismatch at index {}: expected {}, got {:?}",
            i,
            p,
            probs_out.get(i)
        );
    }
}

#[test]
fn test_fse_table_description_roundtrip_sparse() {
    // Sparse distribution with many zeros - exactly the ML table from the failing test
    // table_size = 2^7 = 128
    let probs_in = vec![
        1i16, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
        1, 0, 0, 0, 1, 123,
    ];
    // sum: 1+1+1+1+1+123 = 128 ✓ (5 probs of value 1, one of 123, rest are 0)
    let al = 7u8;
    let serialized = write_table_description(&probs_in, al);

    let (probs_out, al_out, consumed) =
        FseTable::read_table_description(&serialized, 52, 9).unwrap();

    assert_eq!(al_out, al, "accuracy log mismatch");
    assert_eq!(
        consumed,
        serialized.len(),
        "bytes consumed must match serialized length"
    );
    for (i, &p) in probs_in.iter().enumerate() {
        assert_eq!(
            probs_out.get(i).copied().unwrap_or(0),
            p,
            "prob mismatch at index {}: expected {}, got {:?}",
            i,
            p,
            probs_out.get(i)
        );
    }
}
