//! xz - compress files using LZMA
//!
//! Compress files using LZMA/XZ algorithm.

extern crate alloc;
use super::get_arg;
use crate::io;
use alloc::vec;
use alloc::vec::Vec;

/// Read a file into a Vec
fn read_file(path: &[u8]) -> Option<Vec<u8>> {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }
    let data = io::read_all(fd);
    io::close(fd);
    Some(data)
}

/// Write data to a file
fn write_file(path: &[u8], data: &[u8]) -> bool {
    let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if fd < 0 {
        return false;
    }
    let written = io::write_all(fd, data);
    io::close(fd);
    written == data.len() as isize
}

/// Check if a file exists
fn file_exists(path: &[u8]) -> bool {
    io::access(path, libc::F_OK) == 0
}

/// Remove a file
fn remove_file(path: &[u8]) {
    io::unlink(path);
}

// XZ magic bytes
const XZ_MAGIC: [u8; 6] = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
const XZ_FOOTER_MAGIC: [u8; 2] = [0x59, 0x5A];

// LZMA constants
const LZMA_LC: u32 = 3; // Literal context bits
const LZMA_LP: u32 = 0; // Literal position bits
const LZMA_PB: u32 = 2; // Position bits

const NUM_STATES: usize = 12;
const NUM_POS_BITS_MAX: usize = 4;
const NUM_POS_STATES_MAX: usize = 1 << NUM_POS_BITS_MAX;
const LEN_LOW_BITS: usize = 3;
const LEN_MID_BITS: usize = 3;
const LEN_HIGH_BITS: usize = 8;
const LEN_LOW_SYMBOLS: usize = 1 << LEN_LOW_BITS;
const LEN_MID_SYMBOLS: usize = 1 << LEN_MID_BITS;
const LEN_HIGH_SYMBOLS: usize = 1 << LEN_HIGH_BITS;
const MATCH_LEN_MIN: usize = 2;

const END_POS_MODEL_INDEX: usize = 14;
const NUM_FULL_DISTANCES: usize = 1 << (END_POS_MODEL_INDEX >> 1);
const NUM_ALIGN_BITS: usize = 4;
const ALIGN_TABLE_SIZE: usize = 1 << NUM_ALIGN_BITS;

const RC_TOP_VALUE: u32 = 1 << 24;
const RC_BIT_MODEL_TOTAL_BITS: u32 = 11;
const RC_BIT_MODEL_TOTAL: u32 = 1 << RC_BIT_MODEL_TOTAL_BITS;
const RC_MOVE_BITS: u32 = 5;

// CRC32 for XZ (standard polynomial)
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF;
    for &byte in data {
        crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

// CRC64 for XZ
const CRC64_TABLE: [u64; 256] = {
    let mut table = [0u64; 256];
    let poly: u64 = 0xC96C5795D7870F42;
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u64;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ poly;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

fn crc64(data: &[u8]) -> u64 {
    let mut crc: u64 = 0xFFFFFFFFFFFFFFFF;
    for &byte in data {
        crc = CRC64_TABLE[((crc ^ byte as u64) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Range encoder for LZMA
struct RangeEncoder {
    low: u64,
    range: u32,
    cache_size: u32,
    cache: u8,
    output: Vec<u8>,
}

impl RangeEncoder {
    fn new() -> Self {
        Self {
            low: 0,
            range: 0xFFFFFFFF,
            cache_size: 1,
            cache: 0,
            output: Vec::new(),
        }
    }

    fn shift_low(&mut self) {
        let low_hi = (self.low >> 32) as u8;
        if low_hi != 0 || self.low < 0xFF000000 {
            let mut temp = self.cache;
            loop {
                self.output.push(temp.wrapping_add(low_hi));
                temp = 0xFF;
                self.cache_size -= 1;
                if self.cache_size == 0 {
                    break;
                }
            }
            self.cache = ((self.low >> 24) & 0xFF) as u8;
        }
        self.cache_size += 1;
        self.low = (self.low << 8) & 0xFFFFFFFF;
    }

    fn encode_bit(&mut self, prob: &mut u16, bit: u32) {
        let bound = (self.range >> RC_BIT_MODEL_TOTAL_BITS) * (*prob as u32);
        if bit == 0 {
            self.range = bound;
            *prob += ((RC_BIT_MODEL_TOTAL - *prob as u32) >> RC_MOVE_BITS) as u16;
        } else {
            self.low += bound as u64;
            self.range -= bound;
            *prob -= (*prob >> RC_MOVE_BITS) as u16;
        }
        self.normalize();
    }

    fn normalize(&mut self) {
        while self.range < RC_TOP_VALUE {
            self.range <<= 8;
            self.shift_low();
        }
    }

    fn encode_direct(&mut self, value: u32, bits: usize) {
        for i in (0..bits).rev() {
            self.range >>= 1;
            self.low += (self.range as u64) * ((value >> i) & 1) as u64;
            self.normalize();
        }
    }

    fn finish(&mut self) {
        for _ in 0..5 {
            self.shift_low();
        }
    }
}

/// Range decoder for LZMA
struct RangeDecoder<'a> {
    range: u32,
    code: u32,
    input: &'a [u8],
    pos: usize,
}

impl<'a> RangeDecoder<'a> {
    fn new(input: &'a [u8]) -> Option<Self> {
        if input.len() < 5 {
            return None;
        }
        let mut decoder = Self {
            range: 0xFFFFFFFF,
            code: 0,
            input,
            pos: 0,
        };
        // First byte must be 0
        if input[0] != 0 {
            return None;
        }
        decoder.pos = 1;
        for _ in 0..4 {
            decoder.code = (decoder.code << 8) | decoder.read_byte() as u32;
        }
        Some(decoder)
    }

    fn read_byte(&mut self) -> u8 {
        if self.pos < self.input.len() {
            let b = self.input[self.pos];
            self.pos += 1;
            b
        } else {
            0
        }
    }

    fn decode_bit(&mut self, prob: &mut u16) -> u32 {
        let bound = (self.range >> RC_BIT_MODEL_TOTAL_BITS) * (*prob as u32);

        let bit = if self.code < bound {
            self.range = bound;
            *prob += ((RC_BIT_MODEL_TOTAL - *prob as u32) >> RC_MOVE_BITS) as u16;
            0
        } else {
            self.range -= bound;
            self.code -= bound;
            *prob -= (*prob >> RC_MOVE_BITS) as u16;
            1
        };
        self.normalize();
        bit
    }

    fn normalize(&mut self) {
        if self.range < RC_TOP_VALUE {
            self.range <<= 8;
            self.code = (self.code << 8) | self.read_byte() as u32;
        }
    }

    fn decode_direct(&mut self, bits: usize) -> u32 {
        let mut result = 0u32;
        for _ in 0..bits {
            self.range >>= 1;
            let t = (self.code.wrapping_sub(self.range)) >> 31;
            self.code = self.code.wrapping_sub(self.range & (t.wrapping_sub(1)));
            result = (result << 1) | (1 - t);
            self.normalize();
        }
        result
    }
}

/// LZMA state transitions
fn state_update_literal(state: usize) -> usize {
    if state < 4 {
        0
    } else if state < 10 {
        state - 3
    } else {
        state - 6
    }
}

fn state_update_match(state: usize) -> usize {
    if state < 7 { 7 } else { 10 }
}

fn state_update_rep(state: usize) -> usize {
    if state < 7 { 8 } else { 11 }
}

fn state_update_short_rep(state: usize) -> usize {
    if state < 7 { 9 } else { 11 }
}

fn is_lit_state(state: usize) -> bool {
    state < 7
}

/// Probability initialization
fn init_probs(probs: &mut [u16]) {
    for p in probs.iter_mut() {
        *p = (RC_BIT_MODEL_TOTAL >> 1) as u16;
    }
}

/// LZMA encoder
struct LzmaEncoder {
    // Probabilities
    is_match: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    is_rep: [u16; NUM_STATES],
    is_rep0: [u16; NUM_STATES],
    is_rep0_long: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    is_rep1: [u16; NUM_STATES],
    is_rep2: [u16; NUM_STATES],
    pos_slot: [[u16; 64]; 4], // 4 len categories, 64 slots
    pos_special: [u16; NUM_FULL_DISTANCES - END_POS_MODEL_INDEX],
    pos_align: [u16; ALIGN_TABLE_SIZE],
    len_enc: LenEncoder,
    rep_len_enc: LenEncoder,
    literal: Vec<u16>, // Literal probabilities

    // State
    state: usize,
    reps: [u32; 4],
    dict_size: u32,
    lc: u32,
    lp: u32,
    pb: u32,
}

struct LenEncoder {
    choice: u16,
    choice2: u16,
    low: [[u16; LEN_LOW_SYMBOLS]; NUM_POS_STATES_MAX],
    mid: [[u16; LEN_MID_SYMBOLS]; NUM_POS_STATES_MAX],
    high: [u16; LEN_HIGH_SYMBOLS],
}

impl LenEncoder {
    fn new() -> Self {
        let mut enc = Self {
            choice: (RC_BIT_MODEL_TOTAL >> 1) as u16,
            choice2: (RC_BIT_MODEL_TOTAL >> 1) as u16,
            low: [[0; LEN_LOW_SYMBOLS]; NUM_POS_STATES_MAX],
            mid: [[0; LEN_MID_SYMBOLS]; NUM_POS_STATES_MAX],
            high: [0; LEN_HIGH_SYMBOLS],
        };
        for ps in 0..NUM_POS_STATES_MAX {
            init_probs(&mut enc.low[ps]);
            init_probs(&mut enc.mid[ps]);
        }
        init_probs(&mut enc.high);
        enc
    }

    fn encode(&mut self, rc: &mut RangeEncoder, len: usize, pos_state: usize) {
        let len = len - MATCH_LEN_MIN;
        if len < LEN_LOW_SYMBOLS {
            rc.encode_bit(&mut self.choice, 0);
            encode_tree(rc, &mut self.low[pos_state], LEN_LOW_BITS, len as u32);
        } else if len < LEN_LOW_SYMBOLS + LEN_MID_SYMBOLS {
            rc.encode_bit(&mut self.choice, 1);
            rc.encode_bit(&mut self.choice2, 0);
            encode_tree(
                rc,
                &mut self.mid[pos_state],
                LEN_MID_BITS,
                (len - LEN_LOW_SYMBOLS) as u32,
            );
        } else {
            rc.encode_bit(&mut self.choice, 1);
            rc.encode_bit(&mut self.choice2, 1);
            encode_tree(
                rc,
                &mut self.high,
                LEN_HIGH_BITS,
                (len - LEN_LOW_SYMBOLS - LEN_MID_SYMBOLS) as u32,
            );
        }
    }
}

struct LenDecoder {
    choice: u16,
    choice2: u16,
    low: [[u16; LEN_LOW_SYMBOLS]; NUM_POS_STATES_MAX],
    mid: [[u16; LEN_MID_SYMBOLS]; NUM_POS_STATES_MAX],
    high: [u16; LEN_HIGH_SYMBOLS],
}

impl LenDecoder {
    fn new() -> Self {
        let mut dec = Self {
            choice: (RC_BIT_MODEL_TOTAL >> 1) as u16,
            choice2: (RC_BIT_MODEL_TOTAL >> 1) as u16,
            low: [[0; LEN_LOW_SYMBOLS]; NUM_POS_STATES_MAX],
            mid: [[0; LEN_MID_SYMBOLS]; NUM_POS_STATES_MAX],
            high: [0; LEN_HIGH_SYMBOLS],
        };
        for ps in 0..NUM_POS_STATES_MAX {
            init_probs(&mut dec.low[ps]);
            init_probs(&mut dec.mid[ps]);
        }
        init_probs(&mut dec.high);
        dec
    }

    fn decode(&mut self, rc: &mut RangeDecoder, pos_state: usize) -> usize {
        if rc.decode_bit(&mut self.choice) == 0 {
            decode_tree(rc, &mut self.low[pos_state], LEN_LOW_BITS) + MATCH_LEN_MIN
        } else if rc.decode_bit(&mut self.choice2) == 0 {
            decode_tree(rc, &mut self.mid[pos_state], LEN_MID_BITS)
                + MATCH_LEN_MIN
                + LEN_LOW_SYMBOLS
        } else {
            decode_tree(rc, &mut self.high, LEN_HIGH_BITS)
                + MATCH_LEN_MIN
                + LEN_LOW_SYMBOLS
                + LEN_MID_SYMBOLS
        }
    }
}

fn encode_tree(rc: &mut RangeEncoder, probs: &mut [u16], bits: usize, value: u32) {
    let mut symbol = 1u32;
    for i in (0..bits).rev() {
        let bit = (value >> i) & 1;
        rc.encode_bit(&mut probs[symbol as usize], bit);
        symbol = (symbol << 1) | bit;
    }
}

fn decode_tree(rc: &mut RangeDecoder, probs: &mut [u16], bits: usize) -> usize {
    let mut symbol = 1usize;
    for _ in 0..bits {
        symbol = (symbol << 1) | rc.decode_bit(&mut probs[symbol]) as usize;
    }
    symbol - (1 << bits)
}

fn decode_tree_reverse(rc: &mut RangeDecoder, probs: &mut [u16], bits: usize) -> usize {
    let mut symbol = 1usize;
    let mut result = 0usize;
    for i in 0..bits {
        let bit = rc.decode_bit(&mut probs[symbol]) as usize;
        symbol = (symbol << 1) | bit;
        result |= bit << i;
    }
    result
}

fn encode_tree_reverse(rc: &mut RangeEncoder, probs: &mut [u16], bits: usize, value: u32) {
    let mut symbol = 1u32;
    for i in 0..bits {
        let bit = (value >> i) & 1;
        rc.encode_bit(&mut probs[symbol as usize], bit);
        symbol = (symbol << 1) | bit;
    }
}

impl LzmaEncoder {
    fn new(dict_size: u32, lc: u32, lp: u32, pb: u32) -> Self {
        let lit_size = 0x300 << (lc + lp);
        let mut literal = vec![0u16; lit_size];
        init_probs(&mut literal);

        let mut enc = Self {
            is_match: [[0; NUM_POS_STATES_MAX]; NUM_STATES],
            is_rep: [0; NUM_STATES],
            is_rep0: [0; NUM_STATES],
            is_rep0_long: [[0; NUM_POS_STATES_MAX]; NUM_STATES],
            is_rep1: [0; NUM_STATES],
            is_rep2: [0; NUM_STATES],
            pos_slot: [[0; 64]; 4],
            pos_special: [0; NUM_FULL_DISTANCES - END_POS_MODEL_INDEX],
            pos_align: [0; ALIGN_TABLE_SIZE],
            len_enc: LenEncoder::new(),
            rep_len_enc: LenEncoder::new(),
            literal,
            state: 0,
            reps: [0; 4],
            dict_size,
            lc,
            lp,
            pb,
        };

        for s in 0..NUM_STATES {
            init_probs(&mut enc.is_match[s]);
            init_probs(&mut enc.is_rep0_long[s]);
        }
        init_probs(&mut enc.is_rep);
        init_probs(&mut enc.is_rep0);
        init_probs(&mut enc.is_rep1);
        init_probs(&mut enc.is_rep2);
        for i in 0..4 {
            init_probs(&mut enc.pos_slot[i]);
        }
        init_probs(&mut enc.pos_special);
        init_probs(&mut enc.pos_align);

        enc
    }

    fn get_pos_slot(dist: u32) -> u32 {
        if dist < 4 {
            dist
        } else {
            let bsr = 31 - dist.leading_zeros();
            ((bsr << 1) + ((dist >> (bsr - 1)) & 1)) as u32
        }
    }

    fn encode_literal(
        &mut self,
        rc: &mut RangeEncoder,
        prev_byte: u8,
        byte: u8,
        pos: usize,
        match_byte: u8,
    ) {
        let _pos_state = pos & ((1 << self.lp) - 1);
        let lit_state = ((pos & ((1 << self.lp) - 1)) << self.lc as usize)
            | ((prev_byte >> (8 - self.lc)) as usize);
        let probs = &mut self.literal[lit_state * 0x300..][..0x300];

        if is_lit_state(self.state) {
            // Normal literal
            let mut symbol = 1u32;
            for i in (0..8).rev() {
                let bit = ((byte >> i) & 1) as u32;
                rc.encode_bit(&mut probs[symbol as usize], bit);
                symbol = (symbol << 1) | bit;
            }
        } else {
            // Literal after match - use match byte context
            let mut symbol = 1u32;
            let mut match_bit;
            let mut offset = 0x100usize;
            for i in (0..8).rev() {
                match_bit = ((match_byte >> i) & 1) as usize;
                let bit = ((byte >> i) & 1) as u32;
                rc.encode_bit(&mut probs[offset + (match_bit << 8) + symbol as usize], bit);
                symbol = (symbol << 1) | bit;
                offset &= (0usize.wrapping_sub(((match_bit as u32) ^ bit) as usize)) & 0x100;
            }
        }
        self.state = state_update_literal(self.state);
    }

    fn encode_match(&mut self, rc: &mut RangeEncoder, dist: u32, len: usize, pos: usize) {
        let pos_state = pos & ((1 << self.pb) - 1);

        rc.encode_bit(&mut self.is_match[self.state][pos_state], 1);
        rc.encode_bit(&mut self.is_rep[self.state], 0);

        self.len_enc.encode(rc, len, pos_state);

        let len_cat = if len < 6 { len - 2 } else { 3 };
        let pos_slot = Self::get_pos_slot(dist);
        encode_tree(rc, &mut self.pos_slot[len_cat], 6, pos_slot);

        if pos_slot >= 4 {
            let footer_bits = ((pos_slot >> 1) - 1) as usize;
            let base = (2 | (pos_slot & 1)) << footer_bits;
            let pos_reduced = dist - base;

            if pos_slot < END_POS_MODEL_INDEX as u32 {
                // Index into pos_special: base - pos_slot (not base - pos_slot - 1)
                encode_tree_reverse(
                    rc,
                    &mut self.pos_special[(base - pos_slot) as usize..],
                    footer_bits,
                    pos_reduced,
                );
            } else {
                rc.encode_direct(pos_reduced >> NUM_ALIGN_BITS, footer_bits - NUM_ALIGN_BITS);
                encode_tree_reverse(
                    rc,
                    &mut self.pos_align,
                    NUM_ALIGN_BITS,
                    pos_reduced & ((1 << NUM_ALIGN_BITS) - 1),
                );
            }
        }

        // Update reps (store as 1-based: 1 = 1 byte back, to match decoder)
        self.reps[3] = self.reps[2];
        self.reps[2] = self.reps[1];
        self.reps[1] = self.reps[0];
        self.reps[0] = dist + 1;
        self.state = state_update_match(self.state);
    }

    fn encode_rep(&mut self, rc: &mut RangeEncoder, rep_index: usize, len: usize, pos: usize) {
        let pos_state = pos & ((1 << self.pb) - 1);

        rc.encode_bit(&mut self.is_match[self.state][pos_state], 1);
        rc.encode_bit(&mut self.is_rep[self.state], 1);

        if rep_index == 0 {
            rc.encode_bit(&mut self.is_rep0[self.state], 0);
            if len == 1 {
                rc.encode_bit(&mut self.is_rep0_long[self.state][pos_state], 0);
                self.state = state_update_short_rep(self.state);
                return;
            }
            rc.encode_bit(&mut self.is_rep0_long[self.state][pos_state], 1);
        } else {
            rc.encode_bit(&mut self.is_rep0[self.state], 1);
            if rep_index == 1 {
                rc.encode_bit(&mut self.is_rep1[self.state], 0);
            } else {
                rc.encode_bit(&mut self.is_rep1[self.state], 1);
                rc.encode_bit(&mut self.is_rep2[self.state], (rep_index - 2) as u32);
            }
            // Move rep to front
            let dist = self.reps[rep_index];
            for i in (1..=rep_index).rev() {
                self.reps[i] = self.reps[i - 1];
            }
            self.reps[0] = dist;
        }

        self.rep_len_enc.encode(rc, len, pos_state);
        self.state = state_update_rep(self.state);
    }
}

/// Find best match using hash chain
fn find_match(data: &[u8], pos: usize, dict_size: usize, reps: &[u32; 4]) -> (usize, u32, usize) {
    // Returns: (length, distance, rep_index) where rep_index < 4 means use rep, else use match
    let max_len = core::cmp::min(273, data.len() - pos);
    if max_len < MATCH_LEN_MIN {
        return (0, 0, 4);
    }

    // First check rep distances
    let mut best_len = 1;
    let mut best_dist = 0u32;
    let mut best_rep = 4usize;

    for (rep_idx, &rep_dist) in reps.iter().enumerate() {
        // rep_dist is 1-based: 1 = 1 byte back, 2 = 2 bytes back, etc.
        // 0 means no match stored yet
        if rep_dist == 0 || rep_dist as usize > pos {
            continue;
        }
        let start = pos - rep_dist as usize;

        let mut len = 0;
        while len < max_len && data[pos + len] == data[start + len] {
            len += 1;
        }

        if len >= MATCH_LEN_MIN && len > best_len {
            best_len = len;
            best_dist = rep_dist;
            best_rep = rep_idx;
        }
    }

    // Simple hash-based match finding
    let search_start = if pos > dict_size { pos - dict_size } else { 0 };

    for search_pos in (search_start..pos).rev().take(1024) {
        if data[search_pos] != data[pos] {
            continue;
        }

        let mut len = 0;
        while len < max_len && data[search_pos + len] == data[pos + len] {
            len += 1;
        }

        if len >= MATCH_LEN_MIN && len > best_len {
            // LZMA distance: 0 = 1 byte back, so dist = actual_distance - 1
            let dist = (pos - search_pos - 1) as u32;
            best_len = len;
            best_dist = dist;
            best_rep = 4; // Not a rep match
        }
    }

    if best_len < MATCH_LEN_MIN {
        (0, 0, 4)
    } else {
        (best_len, best_dist, best_rep)
    }
}

/// LZMA compress
fn lzma_compress(data: &[u8], dict_size: u32) -> Vec<u8> {
    let mut rc = RangeEncoder::new();
    let mut enc = LzmaEncoder::new(dict_size, LZMA_LC, LZMA_LP, LZMA_PB);

    // Write LZMA header (properties + dict size + uncompressed size)
    let props = (LZMA_PB * 5 + LZMA_LP) * 9 + LZMA_LC;
    let mut header = Vec::with_capacity(13);
    header.push(props as u8);
    for i in 0..4 {
        header.push((dict_size >> (i * 8)) as u8);
    }
    // Uncompressed size (8 bytes, little endian)
    let size = data.len() as u64;
    for i in 0..8 {
        header.push((size >> (i * 8)) as u8);
    }

    let mut pos = 0usize;
    let mut prev_byte = 0u8;

    while pos < data.len() {
        let (match_len, match_dist, rep_idx) = find_match(data, pos, dict_size as usize, &enc.reps);
        let pos_state = pos & ((1 << enc.pb) - 1);

        if match_len >= MATCH_LEN_MIN {
            if rep_idx < 4 {
                // Rep match
                enc.encode_rep(&mut rc, rep_idx, match_len, pos);
            } else {
                // Regular match
                enc.encode_match(&mut rc, match_dist, match_len, pos);
            }
            for _ in 0..match_len {
                prev_byte = data[pos];
                pos += 1;
            }
        } else {
            // Literal
            rc.encode_bit(&mut enc.is_match[enc.state][pos_state], 0);
            let match_byte = if enc.reps[0] > 0 && pos >= enc.reps[0] as usize {
                data[pos - enc.reps[0] as usize]
            } else {
                0
            };
            enc.encode_literal(&mut rc, prev_byte, data[pos], pos, match_byte);
            prev_byte = data[pos];
            pos += 1;
        }
    }

    // Encode end marker
    let pos_state = pos & ((1 << enc.pb) - 1);
    rc.encode_bit(&mut enc.is_match[enc.state][pos_state], 1);
    rc.encode_bit(&mut enc.is_rep[enc.state], 0);
    enc.len_enc.encode(&mut rc, MATCH_LEN_MIN, pos_state);
    encode_tree(&mut rc, &mut enc.pos_slot[0], 6, 63); // Slot 63 is end marker
    rc.encode_direct(0x3FFFFFF, 26); // End marker distance
    encode_tree_reverse(&mut rc, &mut enc.pos_align, NUM_ALIGN_BITS, 0xF);

    rc.finish();

    let mut result = header;
    result.extend_from_slice(&rc.output);
    result
}

/// LZMA decoder
struct LzmaDecoder {
    is_match: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    is_rep: [u16; NUM_STATES],
    is_rep0: [u16; NUM_STATES],
    is_rep0_long: [[u16; NUM_POS_STATES_MAX]; NUM_STATES],
    is_rep1: [u16; NUM_STATES],
    is_rep2: [u16; NUM_STATES],
    pos_slot: [[u16; 64]; 4],
    pos_special: [u16; NUM_FULL_DISTANCES - END_POS_MODEL_INDEX],
    pos_align: [u16; ALIGN_TABLE_SIZE],
    len_dec: LenDecoder,
    rep_len_dec: LenDecoder,
    literal: Vec<u16>,

    state: usize,
    reps: [u32; 4],
    dict_size: u32,
    lc: u32,
    lp: u32,
    pb: u32,
}

impl LzmaDecoder {
    fn new(props: u8, dict_size: u32) -> Self {
        let lc = (props % 9) as u32;
        let props = props / 9;
        let lp = (props % 5) as u32;
        let pb = (props / 5) as u32;

        let lit_size = 0x300 << (lc + lp);
        let mut literal = vec![0u16; lit_size];
        init_probs(&mut literal);

        let mut dec = Self {
            is_match: [[0; NUM_POS_STATES_MAX]; NUM_STATES],
            is_rep: [0; NUM_STATES],
            is_rep0: [0; NUM_STATES],
            is_rep0_long: [[0; NUM_POS_STATES_MAX]; NUM_STATES],
            is_rep1: [0; NUM_STATES],
            is_rep2: [0; NUM_STATES],
            pos_slot: [[0; 64]; 4],
            pos_special: [0; NUM_FULL_DISTANCES - END_POS_MODEL_INDEX],
            pos_align: [0; ALIGN_TABLE_SIZE],
            len_dec: LenDecoder::new(),
            rep_len_dec: LenDecoder::new(),
            literal,
            state: 0,
            reps: [0; 4],
            dict_size,
            lc,
            lp,
            pb,
        };

        for s in 0..NUM_STATES {
            init_probs(&mut dec.is_match[s]);
            init_probs(&mut dec.is_rep0_long[s]);
        }
        init_probs(&mut dec.is_rep);
        init_probs(&mut dec.is_rep0);
        init_probs(&mut dec.is_rep1);
        init_probs(&mut dec.is_rep2);
        for i in 0..4 {
            init_probs(&mut dec.pos_slot[i]);
        }
        init_probs(&mut dec.pos_special);
        init_probs(&mut dec.pos_align);

        dec
    }

    fn decode_literal(&mut self, rc: &mut RangeDecoder, output: &[u8], pos: usize) -> Option<u8> {
        let prev_byte = if pos > 0 { output[pos - 1] } else { 0 };
        let lit_state = ((pos & ((1 << self.lp) - 1)) << self.lc as usize)
            | ((prev_byte >> (8 - self.lc)) as usize);
        let probs = &mut self.literal[lit_state * 0x300..][..0x300];

        if is_lit_state(self.state) {
            let mut symbol = 1usize;
            for _ in 0..8 {
                symbol = (symbol << 1) | rc.decode_bit(&mut probs[symbol]) as usize;
            }
            Some((symbol - 0x100) as u8)
        } else {
            let match_byte = if self.reps[0] > 0 && pos >= self.reps[0] as usize {
                output[pos - self.reps[0] as usize]
            } else {
                return None;
            };

            let mut symbol = 1usize;
            let mut offset = 0x100usize;
            for i in (0..8).rev() {
                let match_bit = ((match_byte >> i) & 1) as usize;
                let bit = rc.decode_bit(&mut probs[offset + (match_bit << 8) + symbol]) as usize;
                symbol = (symbol << 1) | bit;
                offset &= (0usize.wrapping_sub((match_bit ^ bit) as usize)) & 0x100;
            }
            Some((symbol - 0x100) as u8)
        }
    }

    fn decode_distance(&mut self, rc: &mut RangeDecoder, len: usize) -> Option<u32> {
        let len_cat = if len < 6 { len - 2 } else { 3 };
        let pos_slot = decode_tree(rc, &mut self.pos_slot[len_cat], 6) as u32;

        if pos_slot < 4 {
            return Some(pos_slot);
        }

        let num_direct_bits = ((pos_slot >> 1) - 1) as usize;
        let mut dist = (2 | (pos_slot & 1)) << num_direct_bits;

        if pos_slot < END_POS_MODEL_INDEX as u32 {
            // Index into pos_special: dist - pos_slot (not dist - pos_slot - 1)
            let base_idx = (dist - pos_slot) as usize;
            if base_idx + (1 << num_direct_bits) > self.pos_special.len() {
                return None;
            }
            dist +=
                decode_tree_reverse(rc, &mut self.pos_special[base_idx..], num_direct_bits) as u32;
        } else {
            dist += (rc.decode_direct(num_direct_bits - NUM_ALIGN_BITS) << NUM_ALIGN_BITS) as u32;
            dist += decode_tree_reverse(rc, &mut self.pos_align, NUM_ALIGN_BITS) as u32;
        }

        Some(dist)
    }
}

/// LZMA decompress
fn lzma_decompress(data: &[u8], expected_size: Option<u64>) -> Option<Vec<u8>> {
    if data.len() < 13 {
        return None;
    }

    let props = data[0];
    let dict_size = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let uncompressed_size = u64::from_le_bytes([
        data[5], data[6], data[7], data[8], data[9], data[10], data[11], data[12],
    ]);

    let expected = expected_size.unwrap_or(uncompressed_size);
    let known_size = expected != u64::MAX;

    let mut rc = RangeDecoder::new(&data[13..])?;
    let mut dec = LzmaDecoder::new(props, dict_size);
    let mut output: Vec<u8> = if known_size {
        Vec::with_capacity(expected as usize)
    } else {
        Vec::new()
    };

    loop {
        if known_size && output.len() >= expected as usize {
            break;
        }

        let pos_state = output.len() & ((1 << dec.pb) - 1);

        if rc.decode_bit(&mut dec.is_match[dec.state][pos_state]) == 0 {
            // Literal
            let byte = dec.decode_literal(&mut rc, &output, output.len())?;
            output.push(byte);
            dec.state = state_update_literal(dec.state);
        } else if rc.decode_bit(&mut dec.is_rep[dec.state]) == 0 {
            // Match
            let len = dec.len_dec.decode(&mut rc, pos_state);
            let dist = dec.decode_distance(&mut rc, len)?;

            // End marker check
            if dist == 0xFFFFFFFF {
                break;
            }

            // Copy from dictionary
            if dist as usize >= output.len() {
                return None;
            }

            dec.reps[3] = dec.reps[2];
            dec.reps[2] = dec.reps[1];
            dec.reps[1] = dec.reps[0];
            dec.reps[0] = dist + 1;
            dec.state = state_update_match(dec.state);

            let src = output.len() - dist as usize - 1;
            for i in 0..len {
                let byte = output[src + i];
                output.push(byte);
            }
        } else {
            // Rep match
            let (len, rep_idx) = if rc.decode_bit(&mut dec.is_rep0[dec.state]) == 0 {
                if rc.decode_bit(&mut dec.is_rep0_long[dec.state][pos_state]) == 0 {
                    // Short rep
                    dec.state = state_update_short_rep(dec.state);
                    if dec.reps[0] as usize > output.len() {
                        return None;
                    }
                    let byte = output[output.len() - dec.reps[0] as usize];
                    output.push(byte);
                    continue;
                }
                (dec.rep_len_dec.decode(&mut rc, pos_state), 0)
            } else if rc.decode_bit(&mut dec.is_rep1[dec.state]) == 0 {
                (dec.rep_len_dec.decode(&mut rc, pos_state), 1)
            } else if rc.decode_bit(&mut dec.is_rep2[dec.state]) == 0 {
                (dec.rep_len_dec.decode(&mut rc, pos_state), 2)
            } else {
                (dec.rep_len_dec.decode(&mut rc, pos_state), 3)
            };

            // Move rep to front
            let dist = dec.reps[rep_idx];
            for i in (1..=rep_idx).rev() {
                dec.reps[i] = dec.reps[i - 1];
            }
            dec.reps[0] = dist;
            dec.state = state_update_rep(dec.state);

            if dist as usize > output.len() {
                return None;
            }
            let src = output.len() - dist as usize;
            for i in 0..len {
                let byte = output[src + i];
                output.push(byte);
            }
        }
    }

    Some(output)
}

/// XZ block header flags
const XZ_CHECK_NONE: u8 = 0x00;
const XZ_CHECK_CRC32: u8 = 0x01;
const XZ_CHECK_CRC64: u8 = 0x04;
const XZ_CHECK_SHA256: u8 = 0x0A;

/// XZ filter IDs
const XZ_FILTER_LZMA2: u64 = 0x21;

/// Encode multibyte integer (XZ format)
fn encode_vli(value: u64, buf: &mut Vec<u8>) {
    let mut v = value;
    loop {
        let byte = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            buf.push(byte);
            break;
        }
        buf.push(byte | 0x80);
    }
}

/// Decode multibyte integer
fn decode_vli(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return None;
        }
        let byte = data[*pos];
        *pos += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    Some(result)
}

/// LZMA2 encoder (simplified - wraps LZMA data in LZMA2 chunks)
fn lzma2_encode(data: &[u8], dict_size: u32) -> Vec<u8> {
    if data.is_empty() {
        return vec![0x00]; // End marker
    }

    let mut result = Vec::new();
    let chunk_size = 65536; // 64KB chunks
    let mut pos = 0;

    while pos < data.len() {
        let end = core::cmp::min(pos + chunk_size, data.len());
        let chunk = &data[pos..end];
        let compressed = lzma_compress(chunk, dict_size);

        // Check if compression is beneficial
        if compressed.len() < chunk.len() {
            // Compressed chunk
            // Control byte: 0b111xxxxx where xxxxx encodes (dict_size_bits - 12)
            // For simplicity, we'll use a different approach
            // LZMA2 control byte format:
            // 0x00 = end marker
            // 0x01 = uncompressed, dictionary reset
            // 0x02 = uncompressed, no reset
            // 0x80-0xFF = LZMA compressed

            // Control byte for LZMA chunk with state/prop reset
            let ctrl = 0xE0u8; // LZMA chunk, reset state, reset props
            result.push(ctrl);

            // Uncompressed size - 1 (2 bytes, big endian)
            let uncomp_size = (chunk.len() - 1) as u16;
            result.push((uncomp_size >> 8) as u8);
            result.push((uncomp_size & 0xFF) as u8);

            // Compressed size - 1 (2 bytes, big endian)
            // Skip the 13-byte LZMA header, just use the compressed data
            let lzma_data = &compressed[13..];
            let comp_size = (lzma_data.len() - 1) as u16;
            result.push((comp_size >> 8) as u8);
            result.push((comp_size & 0xFF) as u8);

            // Properties byte
            let props = (LZMA_PB * 5 + LZMA_LP) * 9 + LZMA_LC;
            result.push(props as u8);

            // Compressed data (without LZMA header)
            result.extend_from_slice(lzma_data);
        } else {
            // Uncompressed chunk
            let ctrl = 0x01u8; // Uncompressed, reset dictionary
            result.push(ctrl);

            let size = (chunk.len() - 1) as u16;
            result.push((size >> 8) as u8);
            result.push((size & 0xFF) as u8);

            result.extend_from_slice(chunk);
        }

        pos = end;
    }

    result.push(0x00); // End marker
    result
}

/// LZMA2 decoder
fn lzma2_decode(data: &[u8], dict_size: u32) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut pos = 0;
    let mut props = 0u8;
    let mut need_props = true;

    while pos < data.len() {
        let ctrl = data[pos];
        pos += 1;

        if ctrl == 0x00 {
            // End marker
            break;
        }

        if ctrl < 0x80 {
            // Uncompressed chunk
            if pos + 2 > data.len() {
                return None;
            }
            let size = (((data[pos] as usize) << 8) | (data[pos + 1] as usize)) + 1;
            pos += 2;

            if pos + size > data.len() {
                return None;
            }
            output.extend_from_slice(&data[pos..pos + size]);
            pos += size;
            need_props = ctrl == 0x01;
        } else {
            // LZMA compressed chunk
            if pos + 4 > data.len() {
                return None;
            }

            let uncomp_size = (((data[pos] as usize) << 8) | (data[pos + 1] as usize)) + 1;
            pos += 2;
            let comp_size = (((data[pos] as usize) << 8) | (data[pos + 1] as usize)) + 1;
            pos += 2;

            let reset_props = ctrl >= 0xC0;
            if reset_props || need_props {
                if pos >= data.len() {
                    return None;
                }
                props = data[pos];
                pos += 1;
                need_props = false;
            }

            if pos + comp_size > data.len() {
                return None;
            }

            // Build LZMA header
            let mut lzma_data = Vec::with_capacity(13 + comp_size);
            lzma_data.push(props);
            for i in 0..4 {
                lzma_data.push((dict_size >> (i * 8)) as u8);
            }
            let size = uncomp_size as u64;
            for i in 0..8 {
                lzma_data.push((size >> (i * 8)) as u8);
            }
            lzma_data.extend_from_slice(&data[pos..pos + comp_size]);

            let decoded = lzma_decompress(&lzma_data, Some(uncomp_size as u64))?;
            output.extend_from_slice(&decoded);
            pos += comp_size;
        }
    }

    Some(output)
}

/// XZ compress
fn xz_compress(data: &[u8], check_type: u8, dict_size: u32) -> Vec<u8> {
    let mut result = Vec::new();

    // Stream header
    result.extend_from_slice(&XZ_MAGIC);
    let stream_flags = [0x00, check_type];
    result.extend_from_slice(&stream_flags);
    let header_crc = crc32(&stream_flags);
    result.extend_from_slice(&header_crc.to_le_bytes());

    // Block
    let lzma2_data = lzma2_encode(data, dict_size);

    // Block header
    let mut block_header = Vec::new();
    block_header.push(0x00); // Placeholder for size
    block_header.push(0x00); // Block flags: 1 filter, no compressed/uncompressed size

    // Filter: LZMA2
    encode_vli(XZ_FILTER_LZMA2, &mut block_header);

    // LZMA2 properties: dictionary size
    // Dict size is encoded as: 2^(props/2 + 12) for even props, 3*2^((props-1)/2 + 11) for odd
    let dict_bits = 32 - dict_size.leading_zeros();
    let dict_prop = if dict_bits <= 12 {
        0
    } else {
        ((dict_bits - 12) * 2) as u8
    };
    encode_vli(1, &mut block_header); // Properties size
    block_header.push(dict_prop);

    // Pad to multiple of 4
    while block_header.len() % 4 != 0 {
        block_header.push(0x00);
    }

    // Set block header size: (actual_size / 4) - 1
    block_header[0] = ((block_header.len() / 4) as u8) - 1;

    // Block header CRC
    let block_header_crc = crc32(&block_header);
    result.extend_from_slice(&block_header);
    result.extend_from_slice(&block_header_crc.to_le_bytes());

    // Compressed data
    let comp_start = result.len();
    result.extend_from_slice(&lzma2_data);

    // Padding to multiple of 4
    while (result.len() - comp_start) % 4 != 0 {
        result.push(0x00);
    }

    // Check value
    match check_type {
        XZ_CHECK_CRC32 => {
            let check = crc32(data);
            result.extend_from_slice(&check.to_le_bytes());
        }
        XZ_CHECK_CRC64 => {
            let check = crc64(data);
            result.extend_from_slice(&check.to_le_bytes());
        }
        _ => {}
    }

    // Index
    let index_start = result.len();
    result.push(0x00); // Index indicator
    encode_vli(1, &mut result); // Number of records
    encode_vli((result.len() - comp_start) as u64, &mut result); // Unpadded size (approx)
    encode_vli(data.len() as u64, &mut result); // Uncompressed size

    // Pad index to multiple of 4
    while (result.len() - index_start) % 4 != 0 {
        result.push(0x00);
    }

    // Index CRC
    let index_crc = crc32(&result[index_start..]);
    result.extend_from_slice(&index_crc.to_le_bytes());

    // Stream footer
    let footer_crc_data = [
        ((result.len() - index_start) / 4 - 1) as u8,
        (((result.len() - index_start) / 4 - 1) >> 8) as u8,
        0x00,
        check_type,
    ];
    let footer_crc = crc32(&footer_crc_data);
    result.extend_from_slice(&footer_crc.to_le_bytes());
    result.push(footer_crc_data[0]);
    result.push(footer_crc_data[1]);
    result.push(footer_crc_data[2]);
    result.push(footer_crc_data[3]);
    result.extend_from_slice(&XZ_FOOTER_MAGIC);

    result
}

/// XZ decompress
fn xz_decompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 24 {
        return None;
    }

    // Check magic
    if &data[0..6] != &XZ_MAGIC {
        return None;
    }

    // Stream flags
    let stream_flags = &data[6..8];
    if stream_flags[0] != 0x00 {
        return None;
    }
    let check_type = stream_flags[1];

    // Verify header CRC
    let header_crc = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    if crc32(stream_flags) != header_crc {
        return None;
    }

    // Check footer magic
    if &data[data.len() - 2..] != &XZ_FOOTER_MAGIC {
        return None;
    }

    let mut pos = 12; // After stream header
    let mut output = Vec::new();

    // Process blocks
    while pos < data.len() - 12 {
        // Check for index (starts with 0x00)
        if data[pos] == 0x00 {
            break;
        }

        // Block header size
        let block_header_size = ((data[pos] as usize) + 1) * 4;
        if pos + block_header_size + 4 > data.len() {
            return None;
        }

        let block_header = &data[pos..pos + block_header_size];
        let block_header_crc = u32::from_le_bytes([
            data[pos + block_header_size],
            data[pos + block_header_size + 1],
            data[pos + block_header_size + 2],
            data[pos + block_header_size + 3],
        ]);

        if crc32(block_header) != block_header_crc {
            return None;
        }

        let block_flags = block_header[1];
        let num_filters = (block_flags & 0x03) + 1;

        let mut hdr_pos = 2usize;

        // Skip compressed size if present
        if block_flags & 0x40 != 0 {
            decode_vli(block_header, &mut hdr_pos)?;
        }

        // Skip uncompressed size if present
        if block_flags & 0x80 != 0 {
            decode_vli(block_header, &mut hdr_pos)?;
        }

        // Parse filters
        let mut dict_size = 8 * 1024 * 1024u32; // Default 8MB
        for _ in 0..num_filters {
            let filter_id = decode_vli(block_header, &mut hdr_pos)?;
            let props_size = decode_vli(block_header, &mut hdr_pos)? as usize;

            if filter_id == XZ_FILTER_LZMA2 && props_size >= 1 {
                let dict_prop = block_header[hdr_pos];
                dict_size = if dict_prop == 40 {
                    u32::MAX
                } else {
                    let base = 2u32 | ((dict_prop as u32) & 1);
                    let bits = ((dict_prop as u32) / 2) + 11;
                    base << bits
                };
            }
            hdr_pos += props_size;
        }

        pos += block_header_size + 4;
        let comp_start = pos; // Remember where compressed data starts

        // Find compressed data end (scan for end marker or use known size)
        // For simplicity, we'll look for the LZMA2 end marker
        let mut comp_end = pos;
        while comp_end < data.len() - 12 {
            if data[comp_end] == 0x00 {
                comp_end += 1;
                break;
            }
            // Skip LZMA2 chunk
            let ctrl = data[comp_end];
            comp_end += 1;

            if ctrl < 0x80 {
                if comp_end + 2 > data.len() {
                    return None;
                }
                let size = (((data[comp_end] as usize) << 8) | (data[comp_end + 1] as usize)) + 1;
                comp_end += 2 + size;
            } else {
                if comp_end + 4 > data.len() {
                    return None;
                }
                comp_end += 2;
                let comp_size =
                    (((data[comp_end] as usize) << 8) | (data[comp_end + 1] as usize)) + 1;
                comp_end += 2;
                if ctrl >= 0xC0 {
                    comp_end += 1; // Properties byte
                }
                comp_end += comp_size;
            }
        }

        // Decompress LZMA2 data
        let lzma2_data = &data[pos..comp_end];
        let block_output = lzma2_decode(lzma2_data, dict_size)?;
        output.extend_from_slice(&block_output);

        // Skip padding (compressed data + padding must be multiple of 4 bytes)
        pos = comp_end;
        while pos < data.len() && (pos - comp_start) % 4 != 0 {
            pos += 1;
        }

        // Skip check value
        match check_type {
            XZ_CHECK_CRC32 => pos += 4,
            XZ_CHECK_CRC64 => pos += 8,
            XZ_CHECK_SHA256 => pos += 32,
            _ => {}
        }
    }

    Some(output)
}

/// xz - compress files using LZMA
///
/// # Synopsis
/// ```text
/// xz [OPTIONS] [FILE...]
/// ```
///
/// # Description
/// Compress or decompress files using LZMA/XZ algorithm.
///
/// # Options
/// - `-d, --decompress`: Decompress
/// - `-k, --keep`: Keep input files
/// - `-c, --stdout`: Write to stdout
/// - `-f, --force`: Force overwrite
/// - `-0...-9`: Compression level (default 6)
/// - `-h, --help`: Show help
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn xz(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress = false;
    let mut keep = false;
    let mut to_stdout = false;
    let mut force = false;
    let mut level = 6u8;
    let mut files: Vec<&[u8]> = Vec::new();

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-d" || arg == b"--decompress" {
            decompress = true;
        } else if arg == b"-k" || arg == b"--keep" {
            keep = true;
        } else if arg == b"-c" || arg == b"--stdout" {
            to_stdout = true;
        } else if arg == b"-f" || arg == b"--force" {
            force = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if arg.len() == 2 && arg[0] == b'-' && arg[1] >= b'0' && arg[1] <= b'9' {
            level = arg[1] - b'0';
        } else if !arg.starts_with(b"-") {
            files.push(arg);
        }
        i += 1;
    }

    // Check invocation name
    let prog_name = unsafe { get_arg(argv, 0) }.unwrap_or(b"xz");
    if prog_name.ends_with(b"unxz") || prog_name.ends_with(b"xzcat") {
        decompress = true;
    }
    if prog_name.ends_with(b"xzcat") {
        to_stdout = true;
    }

    if files.is_empty() {
        // Read from stdin
        let input = io::read_all(0);

        let output = if decompress {
            match xz_decompress(&input) {
                Some(d) => d,
                None => {
                    io::write_str(2, b"xz: decompression failed\n");
                    return 1;
                }
            }
        } else {
            let dict_size = match level {
                0 => 256 * 1024,
                1 => 1024 * 1024,
                2 => 2 * 1024 * 1024,
                3 => 4 * 1024 * 1024,
                4 => 4 * 1024 * 1024,
                5 => 8 * 1024 * 1024,
                6 => 8 * 1024 * 1024,
                7 => 16 * 1024 * 1024,
                8 => 32 * 1024 * 1024,
                _ => 64 * 1024 * 1024,
            };
            xz_compress(&input, XZ_CHECK_CRC64, dict_size)
        };

        io::write_all(1, &output);
        return 0;
    }

    let mut status = 0;

    for file in files {
        let input = match read_file(file) {
            Some(d) => d,
            None => {
                io::write_str(2, b"xz: cannot read ");
                io::write_all(2, file);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }
        };

        let (output, out_name) = if decompress {
            let decompressed = match xz_decompress(&input) {
                Some(d) => d,
                None => {
                    io::write_str(2, b"xz: ");
                    io::write_all(2, file);
                    io::write_str(2, b": decompression failed\n");
                    status = 1;
                    continue;
                }
            };

            // Remove .xz extension
            let name = if file.ends_with(b".xz") {
                &file[..file.len() - 3]
            } else {
                io::write_str(2, b"xz: ");
                io::write_all(2, file);
                io::write_str(2, b": unknown suffix -- ignored\n");
                status = 1;
                continue;
            };

            (decompressed, name.to_vec())
        } else {
            let dict_size = match level {
                0 => 256 * 1024,
                1 => 1024 * 1024,
                2 => 2 * 1024 * 1024,
                3 => 4 * 1024 * 1024,
                4 => 4 * 1024 * 1024,
                5 => 8 * 1024 * 1024,
                6 => 8 * 1024 * 1024,
                7 => 16 * 1024 * 1024,
                8 => 32 * 1024 * 1024,
                _ => 64 * 1024 * 1024,
            };
            let compressed = xz_compress(&input, XZ_CHECK_CRC64, dict_size);

            // Add .xz extension
            let mut name = file.to_vec();
            name.extend_from_slice(b".xz");

            (compressed, name)
        };

        if to_stdout {
            io::write_all(1, &output);
        } else {
            // Check if output exists
            if !force && file_exists(&out_name) {
                io::write_str(2, b"xz: ");
                io::write_all(2, &out_name);
                io::write_str(2, b" already exists; use -f to overwrite\n");
                status = 1;
                continue;
            }

            if !write_file(&out_name, &output) {
                io::write_str(2, b"xz: cannot write ");
                io::write_all(2, &out_name);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }

            // Remove input file unless -k
            if !keep {
                remove_file(file);
            }
        }
    }

    status
}

/// unxz - decompress XZ files
pub fn unxz(argc: i32, argv: *const *const u8) -> i32 {
    xz(argc, argv)
}

/// xzcat - decompress XZ to stdout
pub fn xzcat(argc: i32, argv: *const *const u8) -> i32 {
    xz(argc, argv)
}

/// lzma - compress files using LZMA (legacy format)
pub fn lzma(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress = false;
    let mut keep = false;
    let mut to_stdout = false;
    let mut force = false;
    let mut level = 6u8;
    let mut files: Vec<&[u8]> = Vec::new();

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-d" || arg == b"--decompress" {
            decompress = true;
        } else if arg == b"-k" || arg == b"--keep" {
            keep = true;
        } else if arg == b"-c" || arg == b"--stdout" {
            to_stdout = true;
        } else if arg == b"-f" || arg == b"--force" {
            force = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: lzma [OPTIONS] [FILE...]\n");
            io::write_str(1, b"Compress or decompress files using LZMA.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(1, b"  -d, --decompress  Decompress\n");
            io::write_str(1, b"  -k, --keep        Keep input files\n");
            io::write_str(1, b"  -c, --stdout      Write to stdout\n");
            io::write_str(1, b"  -f, --force       Force overwrite\n");
            io::write_str(1, b"  -0...-9           Compression level\n");
            io::write_str(1, b"  -h, --help        Show this help\n");
            return 0;
        } else if arg.len() == 2 && arg[0] == b'-' && arg[1] >= b'0' && arg[1] <= b'9' {
            level = arg[1] - b'0';
        } else if !arg.starts_with(b"-") {
            files.push(arg);
        }
        i += 1;
    }

    // Check invocation name
    let prog_name = unsafe { get_arg(argv, 0) }.unwrap_or(b"lzma");
    if prog_name.ends_with(b"unlzma") || prog_name.ends_with(b"lzcat") {
        decompress = true;
    }
    if prog_name.ends_with(b"lzcat") {
        to_stdout = true;
    }

    if files.is_empty() {
        let input = io::read_all(0);

        let output = if decompress {
            match lzma_decompress(&input, None) {
                Some(d) => d,
                None => {
                    io::write_str(2, b"lzma: decompression failed\n");
                    return 1;
                }
            }
        } else {
            let dict_size = match level {
                0..=3 => 1 << (level + 16),
                _ => 1 << 23,
            };
            lzma_compress(&input, dict_size)
        };

        io::write_all(1, &output);
        return 0;
    }

    let mut status = 0;

    for file in files {
        let input = match read_file(file) {
            Some(d) => d,
            None => {
                io::write_str(2, b"lzma: cannot read ");
                io::write_all(2, file);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }
        };

        let (output, out_name) = if decompress {
            let decompressed = match lzma_decompress(&input, None) {
                Some(d) => d,
                None => {
                    io::write_str(2, b"lzma: ");
                    io::write_all(2, file);
                    io::write_str(2, b": decompression failed\n");
                    status = 1;
                    continue;
                }
            };

            let name = if file.ends_with(b".lzma") {
                &file[..file.len() - 5]
            } else {
                io::write_str(2, b"lzma: ");
                io::write_all(2, file);
                io::write_str(2, b": unknown suffix -- ignored\n");
                status = 1;
                continue;
            };

            (decompressed, name.to_vec())
        } else {
            let dict_size = match level {
                0..=3 => 1 << (level + 16),
                _ => 1 << 23,
            };
            let compressed = lzma_compress(&input, dict_size);

            let mut name = file.to_vec();
            name.extend_from_slice(b".lzma");

            (compressed, name)
        };

        if to_stdout {
            io::write_all(1, &output);
        } else {
            if !force && file_exists(&out_name) {
                io::write_str(2, b"lzma: ");
                io::write_all(2, &out_name);
                io::write_str(2, b" already exists; use -f to overwrite\n");
                status = 1;
                continue;
            }

            if !write_file(&out_name, &output) {
                io::write_str(2, b"lzma: cannot write ");
                io::write_all(2, &out_name);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }

            if !keep {
                remove_file(file);
            }
        }
    }

    status
}

/// unlzma - decompress LZMA files
pub fn unlzma(argc: i32, argv: *const *const u8) -> i32 {
    lzma(argc, argv)
}

/// lzcat - decompress LZMA to stdout
pub fn lzcat(argc: i32, argv: *const *const u8) -> i32 {
    lzma(argc, argv)
}

fn print_usage() {
    io::write_str(1, b"Usage: xz [OPTIONS] [FILE...]\n\n");
    io::write_str(1, b"Compress or decompress files using LZMA/XZ.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -d, --decompress  Decompress\n");
    io::write_str(1, b"  -k, --keep        Keep input files\n");
    io::write_str(1, b"  -c, --stdout      Write to stdout\n");
    io::write_str(1, b"  -f, --force       Force overwrite\n");
    io::write_str(1, b"  -0...-9           Compression level (default: 6)\n");
    io::write_str(1, b"  -h, --help        Show this help\n\n");
    io::write_str(1, b"With no FILE, read from stdin.\n");
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    #[test]
    fn test_lzma_roundtrip() {
        let data = b"Hello, World! This is a test of LZMA compression.";
        let compressed = lzma_compress(data, 1 << 20);
        let decompressed = lzma_decompress(&compressed, None).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn test_lzma_empty() {
        let data = b"";
        let compressed = lzma_compress(data, 1 << 20);
        let decompressed = lzma_decompress(&compressed, None).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn test_lzma_repeated() {
        let data = [b'A'; 1000];
        let compressed = lzma_compress(&data, 1 << 20);
        // LZMA should compress repeated data well
        assert!(compressed.len() < data.len() / 2);
        let decompressed = lzma_decompress(&compressed, None).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn test_xz_roundtrip() {
        let data = b"XZ compression test data with some repeated patterns patterns patterns.";
        let compressed = xz_compress(data, XZ_CHECK_CRC64, 1 << 20);

        // Check magic
        assert_eq!(&compressed[0..6], &XZ_MAGIC);

        let decompressed = xz_decompress(&compressed).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn test_xz_empty() {
        let data = b"";
        let compressed = xz_compress(data, XZ_CHECK_CRC32, 1 << 20);
        let decompressed = xz_decompress(&compressed).unwrap();
        assert_eq!(&decompressed[..], &data[..]);
    }

    #[test]
    fn test_crc32() {
        // Test against known values
        assert_eq!(crc32(b""), 0x00000000);
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_range_coder_basic() {
        let mut enc = RangeEncoder::new();
        let mut prob = (RC_BIT_MODEL_TOTAL >> 1) as u16;

        // Encode some bits
        enc.encode_bit(&mut prob, 0);
        enc.encode_bit(&mut prob, 1);
        enc.encode_bit(&mut prob, 0);
        enc.encode_bit(&mut prob, 1);
        enc.finish();

        // Should produce some output
        assert!(!enc.output.is_empty());
    }

    #[test]
    fn test_vli_encoding() {
        let mut buf = Vec::new();
        encode_vli(0, &mut buf);
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        encode_vli(127, &mut buf);
        assert_eq!(buf, vec![0x7F]);

        buf.clear();
        encode_vli(128, &mut buf);
        assert_eq!(buf, vec![0x80, 0x01]);

        buf.clear();
        encode_vli(16384, &mut buf);
        let mut pos = 0;
        assert_eq!(decode_vli(&buf, &mut pos), Some(16384));
    }
}
