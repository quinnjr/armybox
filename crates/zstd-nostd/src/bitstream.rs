//! Backward bitstream reader and forward bitstream writer for zstd
//!
//! Zstd uses a backward bitstream for sequence encoding. The stream is stored
//! as bytes with a sentinel bit in the last byte. Reading proceeds from the
//! end of the buffer toward the start.

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::vec;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::common::ZstdError;

/// Backward bitstream reader for zstd sequence decoding.
///
/// The last byte of the stream contains a sentinel: the highest set bit
/// marks the start, and bits below it (plus all preceding bytes) are data.
/// Reading proceeds from the last byte toward the first, consuming bits
/// from MSB to LSB within the logical stream.
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Bit position within the entire buffer (counts down from total bits)
    bit_pos: isize,
}

impl<'a> BitReader<'a> {
    /// Initialize a backward bitstream reader.
    pub fn init(data: &'a [u8]) -> Result<Self, ZstdError> {
        if data.is_empty() {
            return Err(ZstdError::CorruptData);
        }

        let last = data[data.len() - 1];
        if last == 0 {
            return Err(ZstdError::CorruptData);
        }

        // Find sentinel: highest set bit in last byte
        let sentinel_bit = 7 - last.leading_zeros() as isize;
        // Total data bits = (len-1)*8 + sentinel_bit
        // (sentinel_bit bits below the sentinel in the last byte, plus all preceding bytes)
        let total_bits = ((data.len() - 1) as isize) * 8 + sentinel_bit;

        Ok(BitReader {
            data,
            bit_pos: total_bits,
        })
    }

    /// Read n bits from the bitstream
    pub fn read_bits(&mut self, n: u32) -> Result<u32, ZstdError> {
        if n == 0 {
            return Ok(0);
        }
        let n = n as isize;
        self.bit_pos -= n;
        if self.bit_pos < 0 {
            return Err(ZstdError::CorruptData);
        }
        Ok(self.extract_bits(self.bit_pos, n as u32))
    }

    /// Peek at n bits without consuming
    pub fn peek_bits(&self, n: u32) -> Result<u32, ZstdError> {
        if n == 0 {
            return Ok(0);
        }
        let pos = self.bit_pos - n as isize;
        if pos < 0 {
            return Err(ZstdError::CorruptData);
        }
        Ok(self.extract_bits(pos, n))
    }

    /// Consume n bits (after peeking)
    pub fn consume(&mut self, n: u32) {
        self.bit_pos -= n as isize;
    }

    /// Extract n bits starting at bit position `pos` (0 = LSB of first byte)
    fn extract_bits(&self, pos: isize, n: u32) -> u32 {
        let mut result = 0u32;
        for i in 0..n {
            let bit_idx = (pos + i as isize) as usize;
            let byte_idx = bit_idx / 8;
            let bit_in_byte = bit_idx % 8;
            if byte_idx < self.data.len() {
                result |= (((self.data[byte_idx] >> bit_in_byte) & 1) as u32) << i;
            }
        }
        result
    }

    /// Check if all bits have been consumed
    pub fn is_done(&self) -> bool {
        self.bit_pos <= 0
    }

    /// Remaining bits in the stream
    pub fn remaining(&self) -> u32 {
        if self.bit_pos < 0 {
            0
        } else {
            self.bit_pos as u32
        }
    }
}

/// Forward bitstream writer for zstd compression.
///
/// Bits written first will be read first by BitReader (they end up at the
/// "top" of the backward stream, i.e., read first when reading from the end).
#[cfg(feature = "alloc")]
pub struct BitWriter {
    /// All bits collected, stored as (value, num_bits) pairs
    bits: Vec<(u32, u32)>,
    total_bits: u64,
}

#[cfg(feature = "alloc")]
impl BitWriter {
    pub fn new() -> Self {
        BitWriter {
            bits: Vec::new(),
            total_bits: 0,
        }
    }

    /// Write n bits to the stream.
    pub fn write_bits(&mut self, value: u32, n: u32) {
        if n == 0 {
            return;
        }
        let masked = value & ((1u64 << n) as u32 - 1);
        self.bits.push((masked, n));
        self.total_bits += n as u64;
    }

    /// Finalize the stream: produce the byte array with sentinel.
    ///
    /// The output byte array is structured so that BitReader::init() + read_bits()
    /// reproduces the bits in the order they were written.
    pub fn finish(self) -> Vec<u8> {
        let total = self.total_bits as usize;
        if total == 0 {
            // Just sentinel
            return vec![0x01];
        }

        // Total bytes needed: ceil((total + 1) / 8) where +1 is for sentinel
        let total_with_sentinel = total + 1;
        let num_bytes = total_with_sentinel.div_ceil(8);
        let mut buf = vec![0u8; num_bytes];

        // The sentinel bit goes at position `total` (0-indexed from LSB of byte 0)
        let sentinel_pos = total;
        let sentinel_byte = sentinel_pos / 8;
        let sentinel_bit = sentinel_pos % 8;
        buf[sentinel_byte] |= 1u8 << sentinel_bit;

        // Write data bits: first-written bits go at highest positions.
        // BitReader reads n bits: decrements bit_pos by n, then extracts bits
        // from bit_pos (LSB) to bit_pos+n-1 (MSB). So the value's LSB should be
        // at the lower position and MSB at the higher position.
        // We write MSB first (at higher position) decrementing toward LSB.
        let mut pos = total; // next bit position to write at (decrements)
        for &(value, n) in &self.bits {
            // Write MSB of this value first (at higher position)
            for i in (0..n).rev() {
                pos -= 1;
                let bit = (value >> i) & 1;
                let byte_idx = pos / 8;
                let bit_idx = pos % 8;
                buf[byte_idx] |= (bit as u8) << bit_idx;
            }
        }

        buf
    }
}

impl Default for BitWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitwriter_reader_roundtrip() {
        let mut w = BitWriter::new();
        w.write_bits(0b101, 3);
        w.write_bits(0b1100, 4);
        w.write_bits(0xFF, 8);
        w.write_bits(0, 1);
        let data = w.finish();

        let mut r = BitReader::init(&data).unwrap();
        assert_eq!(r.read_bits(3).unwrap(), 0b101);
        assert_eq!(r.read_bits(4).unwrap(), 0b1100);
        assert_eq!(r.read_bits(8).unwrap(), 0xFF);
        assert_eq!(r.read_bits(1).unwrap(), 0);
    }

    #[test]
    fn test_single_bit() {
        let mut w = BitWriter::new();
        w.write_bits(1, 1);
        let data = w.finish();

        let mut r = BitReader::init(&data).unwrap();
        assert_eq!(r.read_bits(1).unwrap(), 1);
    }

    #[test]
    fn test_empty_stream() {
        let w = BitWriter::new();
        let data = w.finish();
        assert!(!data.is_empty());
        let r = BitReader::init(&data).unwrap();
        assert!(r.is_done());
    }

    #[test]
    fn test_many_bits() {
        let mut w = BitWriter::new();
        for i in 0..20u32 {
            w.write_bits(i, 5);
        }
        let data = w.finish();

        let mut r = BitReader::init(&data).unwrap();
        for i in 0..20u32 {
            assert_eq!(r.read_bits(5).unwrap(), i);
        }
    }

    #[test]
    fn test_32bit_values() {
        let mut w = BitWriter::new();
        w.write_bits(0xDEAD, 16);
        w.write_bits(0xBEEF, 16);
        let data = w.finish();

        let mut r = BitReader::init(&data).unwrap();
        assert_eq!(r.read_bits(16).unwrap(), 0xDEAD);
        assert_eq!(r.read_bits(16).unwrap(), 0xBEEF);
    }
}
