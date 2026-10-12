//! Common constants, error types, and predefined tables for zstd

#[cfg(feature = "alloc")]
extern crate alloc;

/// Frame magic number
pub const ZSTD_MAGIC: u32 = 0xFD2FB528;

/// Skippable frame magic range
pub const SKIPPABLE_MAGIC_LOW: u32 = 0x184D2A50;
pub const SKIPPABLE_MAGIC_HIGH: u32 = 0x184D2A5F;

/// Maximum block size (128 KB)
pub const MAX_BLOCK_SIZE: usize = 1 << 17;

/// Minimum match length
pub const MIN_MATCH: usize = 3;

/// Maximum window log
pub const MAX_WINDOW_LOG: u32 = 30;

/// Block types (2-bit field in block header)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockType {
    Raw = 0,
    Rle = 1,
    Compressed = 2,
    Reserved = 3,
}

impl BlockType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => BlockType::Raw,
            1 => BlockType::Rle,
            2 => BlockType::Compressed,
            _ => BlockType::Reserved,
        }
    }
}

/// Literals section type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiteralsType {
    Raw = 0,
    Rle = 1,
    Compressed = 2,
    Treeless = 3,
}

impl LiteralsType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => LiteralsType::Raw,
            1 => LiteralsType::Rle,
            2 => LiteralsType::Compressed,
            _ => LiteralsType::Treeless,
        }
    }
}

/// Sequence compression mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeqMode {
    Predefined = 0,
    Rle = 1,
    FseCompressed = 2,
    Repeat = 3,
}

impl SeqMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => SeqMode::Predefined,
            1 => SeqMode::Rle,
            2 => SeqMode::FseCompressed,
            _ => SeqMode::Repeat,
        }
    }
}

/// Error types for zstd operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZstdError {
    CorruptData,
    WindowTooLarge,
    UnsupportedDictionary,
    ChecksumMismatch,
    OutputTooLarge,
}

/// Literal length code baselines and extra bits (codes 0-35)
/// Per RFC 8878 Table 15
pub const LL_BASELINES: [u32; 36] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32, 40, 48, 64,
    128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];

pub const LL_EXTRA_BITS: [u8; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15, 16,
];

/// Match length code baselines and extra bits (codes 0-52)
/// Per RFC 8878 and reference zstd implementation
/// Note: After code 42 (bits=5), bits jump to 7 and then increment by 1
pub const ML_BASELINES: [u32; 53] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 33, 34, 35, 37, 39, 41, 43, 47, 51, 59, 67, 83, 99, 131, 259, 515, 1027,
    2051, 4099, 8195, 16387, 32771, 65539,
];

pub const ML_EXTRA_BITS: [u8; 53] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];

/// Offset codes extra bits (code N uses N extra bits)
/// The baseline for offset code N is (1 << N)

/// Predefined FSE distribution for literal lengths (accuracy log 6)
pub const LL_DEFAULT_DIST: [i16; 36] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1,
    -1, -1, -1, -1,
];
pub const LL_DEFAULT_AL: u8 = 6;

/// Predefined FSE distribution for match lengths (accuracy log 6)
pub const ML_DEFAULT_DIST: [i16; 53] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];
pub const ML_DEFAULT_AL: u8 = 6;

/// Predefined FSE distribution for offsets (accuracy log 5)
pub const OF_DEFAULT_DIST: [i16; 29] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];
pub const OF_DEFAULT_AL: u8 = 5;

/// Number of literal length symbols
pub const LL_MAX_SYMBOL: usize = 35;
/// Max accuracy log for literal lengths
pub const LL_MAX_AL: u8 = 9;

/// Number of match length symbols
pub const ML_MAX_SYMBOL: usize = 52;
/// Max accuracy log for match lengths
pub const ML_MAX_AL: u8 = 9;

/// Number of offset symbols
pub const OF_MAX_SYMBOL: usize = 31;
/// Max accuracy log for offsets
pub const OF_MAX_AL: u8 = 8;

/// Get literal length code for a given literal length value
pub fn ll_code(ll: u32) -> u8 {
    if ll <= 15 {
        return ll as u8;
    }
    // Search backward through baselines to find the right code
    let mut code = 35u8;
    while code > 16 {
        if ll >= LL_BASELINES[code as usize] {
            return code;
        }
        code -= 1;
    }
    16
}

/// Get match length code for a given match length value (ml >= 3)
pub fn ml_code(ml: u32) -> u8 {
    // ML_BASELINES[0] = 3, so code 0 = match length 3
    if ml <= 34 {
        // Codes 0-31: match lengths 3-34 (direct mapping)
        return (ml - 3) as u8;
    }
    // Search backward through baselines for codes 32-52
    let mut code = 52u8;
    while code > 32 {
        if ml >= ML_BASELINES[code as usize] {
            return code;
        }
        code -= 1;
    }
    32
}

/// Get offset code for a given offset value
pub fn of_code(offset: u32) -> u8 {
    if offset == 0 {
        return 0;
    }
    (32 - offset.leading_zeros() - 1) as u8
}

/// Helper to read a little-endian u32 from a byte slice
#[inline]
pub fn read_le32(data: &[u8], offset: usize) -> u32 {
    if offset + 4 > data.len() {
        return 0;
    }
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Helper to read a little-endian u64 from a byte slice
#[inline]
pub fn read_le64(data: &[u8], offset: usize) -> u64 {
    if offset + 8 > data.len() {
        return 0;
    }
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}
