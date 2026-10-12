//! bzip2 - block-sorting file compressor
//!
//! Compress files using bzip2 algorithm.

use super::{get_arg, open_read, open_write_create};
use crate::io;
use alloc::vec;
use alloc::vec::Vec;

// bzip2 magic bytes
const MAGIC: [u8; 2] = *b"BZ";
const BLOCK_MAGIC: [u8; 6] = [0x31, 0x41, 0x59, 0x26, 0x53, 0x59]; // pi digits
const STREAM_END_MAGIC: [u8; 6] = [0x17, 0x72, 0x45, 0x38, 0x50, 0x90]; // sqrt(pi) digits

/// bzip2 - block-sorting file compressor
///
/// # Synopsis
/// ```text
/// bzip2 [-dkczf] [-1..-9] [FILE...]
/// ```
///
/// # Description
/// Compress files using bzip2 algorithm (Burrows-Wheeler transform).
///
/// # Options
/// - `-d`: Decompress
/// - `-k`: Keep original files
/// - `-c`: Write to stdout
/// - `-z`: Compress (default)
/// - `-f`: Force overwrite
/// - `-1` to `-9`: Block size (100k to 900k)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn bzip2(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress = false;
    let mut keep = false;
    let mut stdout_mode = false;
    let mut block_size: u8 = 9;
    let mut files: Vec<&[u8]> = Vec::new();

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") {
                for &c in &arg[1..] {
                    match c {
                        b'd' => decompress = true,
                        b'k' => keep = true,
                        b'c' => stdout_mode = true,
                        b'z' => decompress = false,
                        b'f' => {}
                        b'1'..=b'9' => block_size = c - b'0',
                        _ => {}
                    }
                }
            } else {
                files.push(arg);
            }
        }
    }

    if files.is_empty() {
        if decompress {
            bunzip2_stream(0, 1)
        } else {
            bzip2_stream(0, 1, block_size)
        }
    } else {
        for &file in &files {
            if stdout_mode {
                let fd = open_read(file);
                if fd < 0 {
                    io::write_str(2, b"bzip2: cannot open file\n");
                    return 1;
                }
                let result = if decompress {
                    bunzip2_stream(fd, 1)
                } else {
                    bzip2_stream(fd, 1, block_size)
                };
                io::close(fd);
                if result != 0 {
                    return result;
                }
            } else {
                if decompress {
                    if bunzip2_file(file, keep) != 0 {
                        return 1;
                    }
                } else {
                    if bzip2_file(file, keep, block_size) != 0 {
                        return 1;
                    }
                }
            }
        }
        0
    }
}

/// bunzip2 - decompress bzip2 files
pub fn bunzip2(argc: i32, argv: *const *const u8) -> i32 {
    let mut new_argv: Vec<*const u8> = Vec::new();
    new_argv.push(b"bunzip2\0".as_ptr());
    new_argv.push(b"-d\0".as_ptr());
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            new_argv.push(arg.as_ptr());
        }
    }
    bzip2(new_argv.len() as i32, new_argv.as_ptr())
}

/// bzcat - decompress bzip2 to stdout
pub fn bzcat(argc: i32, argv: *const *const u8) -> i32 {
    let mut new_argv: Vec<*const u8> = Vec::new();
    new_argv.push(b"bzcat\0".as_ptr());
    new_argv.push(b"-dc\0".as_ptr());
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            new_argv.push(arg.as_ptr());
        }
    }
    bzip2(new_argv.len() as i32, new_argv.as_ptr())
}

fn bzip2_stream(input_fd: i32, output_fd: i32, block_size: u8) -> i32 {
    // Read all input
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(input_fd, &mut buf);
        if n <= 0 {
            break;
        }
        data.extend_from_slice(&buf[..n as usize]);
    }

    // Write stream header: BZh<block_size>
    let header = [MAGIC[0], MAGIC[1], b'h', b'0' + block_size];
    io::write_all(output_fd, &header);

    let block_bytes = (block_size as usize) * 100000;

    // Process in blocks
    let mut bit_writer = BitWriter::new();
    let mut combined_crc: u32 = 0;

    for chunk in data.chunks(block_bytes) {
        // Write block header
        for &b in &BLOCK_MAGIC {
            bit_writer.write_bits(b as u32, 8);
        }

        // Calculate and write block CRC
        let block_crc = bzip2_crc32(chunk);
        bit_writer.write_bits(block_crc, 32);

        // Update combined CRC
        combined_crc = (combined_crc << 1) | (combined_crc >> 31);
        combined_crc ^= block_crc;

        // Randomized flag (not used)
        bit_writer.write_bits(0, 1);

        // Compress block
        compress_block(chunk, &mut bit_writer);
    }

    // Write stream footer
    for &b in &STREAM_END_MAGIC {
        bit_writer.write_bits(b as u32, 8);
    }
    bit_writer.write_bits(combined_crc, 32);

    // Flush bits
    bit_writer.flush();
    io::write_all(output_fd, bit_writer.data());

    0
}

fn bunzip2_stream(input_fd: i32, output_fd: i32) -> i32 {
    // Read stream header
    let mut header = [0u8; 4];
    if io::read(input_fd, &mut header) != 4 {
        io::write_str(2, b"bzip2: truncated header\n");
        return 1;
    }

    if header[0] != MAGIC[0] || header[1] != MAGIC[1] || header[2] != b'h' {
        io::write_str(2, b"bzip2: not bzip2 format\n");
        return 1;
    }

    let block_size = header[3] - b'0';
    if block_size < 1 || block_size > 9 {
        io::write_str(2, b"bzip2: invalid block size\n");
        return 1;
    }

    // Read remaining data
    let mut compressed = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(input_fd, &mut buf);
        if n <= 0 {
            break;
        }
        compressed.extend_from_slice(&buf[..n as usize]);
    }

    // Decompress
    let mut bit_reader = BitReader::new(&compressed);

    loop {
        // Read block header
        let mut magic = [0u8; 6];
        for i in 0..6 {
            magic[i] = bit_reader.read_bits(8) as u8;
        }

        if magic == STREAM_END_MAGIC {
            // Stream end
            let _combined_crc = bit_reader.read_bits(32);
            break;
        }

        if magic != BLOCK_MAGIC {
            io::write_str(2, b"bzip2: invalid block header\n");
            return 1;
        }

        // Read block CRC
        let _block_crc = bit_reader.read_bits(32);

        // Read randomized flag
        let _randomized = bit_reader.read_bits(1);

        // Decompress block
        let block_data = decompress_block(&mut bit_reader);
        io::write_all(output_fd, &block_data);
    }

    0
}

fn bzip2_file(path: &[u8], keep: bool, block_size: u8) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"bzip2: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    let mut out_path = Vec::new();
    out_path.extend_from_slice(path);
    out_path.extend_from_slice(b".bz2\0");

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"bzip2: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = bzip2_stream(fd, out_fd, block_size);

    io::close(fd);
    io::close(out_fd);

    if result == 0 && !keep {
        let mut path_z = [0u8; 256];
        let len = path.len().min(255);
        path_z[..len].copy_from_slice(&path[..len]);
        unsafe { libc::unlink(path_z.as_ptr() as *const libc::c_char) };
    }

    result
}

fn bunzip2_file(path: &[u8], keep: bool) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"bzip2: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    let mut out_path = Vec::new();
    if path.ends_with(b".bz2") {
        out_path.extend_from_slice(&path[..path.len() - 4]);
    } else if path.ends_with(b".bz") {
        out_path.extend_from_slice(&path[..path.len() - 3]);
    } else {
        out_path.extend_from_slice(path);
        out_path.extend_from_slice(b".out");
    }
    out_path.push(0);

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"bzip2: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = bunzip2_stream(fd, out_fd);

    io::close(fd);
    io::close(out_fd);

    if result == 0 && !keep {
        let mut path_z = [0u8; 256];
        let len = path.len().min(255);
        path_z[..len].copy_from_slice(&path[..len]);
        unsafe { libc::unlink(path_z.as_ptr() as *const libc::c_char) };
    }

    result
}

// Compress a single block using simplified BWT + MTF + RLE + Huffman
fn compress_block(data: &[u8], writer: &mut BitWriter) {
    if data.is_empty() {
        // Write origin pointer (24 bits)
        writer.write_bits(0, 24);
        // Write empty symbol map
        for _ in 0..16 {
            writer.write_bits(0, 1);
        }
        // Write selectors
        writer.write_bits(2, 3); // 2 groups
        writer.write_bits(1, 15); // 1 selector
        writer.write_bits(0, 1); // MTF delta 0
        // Write Huffman tables
        writer.write_bits(5, 5); // length 5 for both symbols
        writer.write_bits(0, 1); // delta 0
        writer.write_bits(0, 1);
        writer.write_bits(5, 5);
        writer.write_bits(0, 1);
        writer.write_bits(0, 1);
        return;
    }

    // Step 1: RLE encoding (run-length encode runs of identical bytes)
    let rle_data = rle1_encode(data);

    // Step 2: Burrows-Wheeler Transform
    let (bwt_data, origin_ptr) = bwt_transform(&rle_data);

    // Write origin pointer (24 bits)
    writer.write_bits(origin_ptr as u32, 24);

    // Build symbol map BEFORE MTF (MTF needs to use the same symbol ordering)
    let mut used = [false; 256];
    for &b in &bwt_data {
        used[b as usize] = true;
    }
    let symbol_map: Vec<u8> = (0u8..=255).filter(|&b| used[b as usize]).collect();

    // Step 3: Move-to-front transform (using symbol map)
    let mtf_data = mtf_encode_with_map(&bwt_data, &symbol_map);

    // Step 4: Zero-run-length encoding
    let zrle_data = zrle_encode(&mtf_data);

    // Write symbol map (16 groups of 16)
    let mut map_l1 = 0u16;
    for i in 0..16 {
        let mut has_any = false;
        for j in 0..16 {
            if used[i * 16 + j] {
                has_any = true;
                break;
            }
        }
        if has_any {
            map_l1 |= 1 << (15 - i);
        }
    }

    for i in 0..16 {
        writer.write_bits(if map_l1 & (1 << (15 - i)) != 0 { 1 } else { 0 }, 1);
    }

    for i in 0..16 {
        if map_l1 & (1 << (15 - i)) != 0 {
            for j in 0..16 {
                writer.write_bits(if used[i * 16 + j] { 1 } else { 0 }, 1);
            }
        }
    }

    // For simplicity, use a single Huffman table with fixed codes
    // Real bzip2 uses multiple tables selected by selectors

    // Count symbols for Huffman coding: unique bytes + RUNA + RUNB + EOB
    // But RUNA/RUNB replace the first symbol, so it's: symbol_map.len() + 2
    let num_symbols = symbol_map.len() + 2;

    // Number of Huffman groups (1-6)
    let num_groups = core::cmp::min(6, core::cmp::max(2, (zrle_data.len() / 50) + 1));
    writer.write_bits(num_groups as u32, 3);

    // Number of selectors
    let num_selectors = (zrle_data.len() + 49) / 50;
    writer.write_bits(num_selectors as u32, 15);

    // Write selectors (all using group 0 for simplicity)
    for _ in 0..num_selectors {
        writer.write_bits(0, 1); // MTF delta of 0
    }

    // Write Huffman table (simplified: all symbols have same length)
    // Calculate code length needed to represent all symbols
    let code_len = {
        let mut len = 1u8;
        let mut max_val = 2usize; // 2^1 = 2
        while max_val < num_symbols {
            len += 1;
            max_val <<= 1;
        }
        len.max(1).min(20) // bzip2 allows 1-20 bits
    };

    for _ in 0..num_groups {
        writer.write_bits(code_len as u32, 5);
        for _ in 0..num_symbols {
            writer.write_bits(0, 1); // delta = 0, keep same length
        }
    }

    // Write compressed data using simple fixed-length codes
    for &sym in &zrle_data {
        writer.write_bits(sym as u32, code_len as usize);
    }

    // Write end-of-block symbol
    writer.write_bits((num_symbols - 1) as u32, code_len as usize);
}

// Decompress a single block
fn decompress_block(reader: &mut BitReader) -> Vec<u8> {
    // Read origin pointer
    let origin_ptr = reader.read_bits(24) as usize;

    // Read symbol map
    let mut map_l1 = 0u16;
    for i in 0..16 {
        if reader.read_bits(1) != 0 {
            map_l1 |= 1 << (15 - i);
        }
    }

    let mut symbol_map: Vec<u8> = Vec::new();
    for i in 0..16 {
        if map_l1 & (1 << (15 - i)) != 0 {
            for j in 0..16 {
                if reader.read_bits(1) != 0 {
                    symbol_map.push((i * 16 + j) as u8);
                }
            }
        }
    }

    if symbol_map.is_empty() {
        return Vec::new();
    }

    let num_symbols = symbol_map.len() + 2;

    // Read number of groups
    let num_groups = reader.read_bits(3) as usize;
    if num_groups < 2 || num_groups > 6 {
        return Vec::new();
    }

    // Read number of selectors
    let num_selectors = reader.read_bits(15) as usize;

    // Read selectors
    let mut selectors = Vec::with_capacity(num_selectors);
    let mut mtf_selectors: Vec<u8> = (0..num_groups as u8).collect();
    for _ in 0..num_selectors {
        let mut j = 0;
        while reader.read_bits(1) != 0 {
            j += 1;
            if j >= num_groups {
                break;
            }
        }
        let sel = mtf_selectors[j];
        // Move to front
        for k in (1..=j).rev() {
            mtf_selectors[k] = mtf_selectors[k - 1];
        }
        mtf_selectors[0] = sel;
        selectors.push(sel);
    }

    // Read Huffman tables
    let mut tables: Vec<Vec<u8>> = Vec::with_capacity(num_groups);
    for _ in 0..num_groups {
        let mut lengths = Vec::with_capacity(num_symbols);
        let mut curr_len = reader.read_bits(5) as u8;
        for _ in 0..num_symbols {
            loop {
                let bit = reader.read_bits(1);
                if bit == 0 {
                    break;
                }
                let bit2 = reader.read_bits(1);
                if bit2 == 0 {
                    curr_len += 1;
                } else {
                    curr_len = curr_len.saturating_sub(1);
                }
            }
            lengths.push(curr_len);
        }
        tables.push(lengths);
    }

    // Build Huffman decoders
    let decoders: Vec<HuffmanDecoder> = tables
        .iter()
        .map(|t| HuffmanDecoder::from_lengths(t))
        .collect();

    // Decode symbols
    let mut zrle_data = Vec::new();
    let mut selector_idx = 0;
    let mut group_count = 0;

    loop {
        // Use current selector, or last one if we've run out
        let effective_selector = if selector_idx < selectors.len() {
            selectors[selector_idx] as usize
        } else if !selectors.is_empty() {
            selectors[selectors.len() - 1] as usize
        } else {
            break;
        };

        let decoder = &decoders[effective_selector];
        let symbol = decoder.decode(reader);

        if symbol == num_symbols - 1 {
            // End of block
            break;
        }

        zrle_data.push(symbol as u8);

        group_count += 1;
        if group_count >= 50 {
            group_count = 0;
            selector_idx += 1;
        }
    }

    // Inverse zero-run-length encoding
    let mtf_data = zrle_decode(&zrle_data);

    // Inverse move-to-front transform
    let bwt_data = mtf_decode(&mtf_data, &symbol_map);

    // Inverse Burrows-Wheeler transform
    let rle_data = bwt_inverse(&bwt_data, origin_ptr);

    // Inverse RLE
    rle1_decode(&rle_data)
}

// RLE1: encode runs of 4+ identical bytes
fn rle1_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let byte = data[i];
        let mut run_len = 1;
        while i + run_len < data.len() && data[i + run_len] == byte && run_len < 255 {
            run_len += 1;
        }
        if run_len >= 4 {
            output.extend_from_slice(&[byte, byte, byte, byte]);
            output.push((run_len - 4) as u8);
            i += run_len;
        } else {
            output.push(byte);
            i += 1;
        }
    }
    output
}

fn rle1_decode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < data.len() {
        output.push(data[i]);
        if i + 3 < data.len()
            && data[i] == data[i + 1]
            && data[i] == data[i + 2]
            && data[i] == data[i + 3]
        {
            output.extend_from_slice(&[data[i], data[i], data[i]]);
            i += 4;
            if i < data.len() {
                let repeat = data[i] as usize;
                for _ in 0..repeat {
                    output.push(data[i - 4]);
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    output
}

// Simplified BWT (Burrows-Wheeler Transform)
fn bwt_transform(data: &[u8]) -> (Vec<u8>, usize) {
    if data.is_empty() {
        return (Vec::new(), 0);
    }

    let n = data.len();
    let mut indices: Vec<usize> = (0..n).collect();

    // Sort rotations
    indices.sort_by(|&a, &b| {
        for i in 0..n {
            let ca = data[(a + i) % n];
            let cb = data[(b + i) % n];
            if ca != cb {
                return ca.cmp(&cb);
            }
        }
        core::cmp::Ordering::Equal
    });

    // Extract last column and find origin
    let mut output = Vec::with_capacity(n);
    let mut origin = 0;
    for (i, &idx) in indices.iter().enumerate() {
        output.push(data[(idx + n - 1) % n]);
        if idx == 0 {
            origin = i;
        }
    }

    (output, origin)
}

fn bwt_inverse(data: &[u8], origin: usize) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let n = data.len();

    // Build transformation vector using counting sort
    let mut count = [0usize; 256];
    for &b in data {
        count[b as usize] += 1;
    }

    let mut cumsum = [0usize; 256];
    let mut sum = 0;
    for i in 0..256 {
        cumsum[i] = sum;
        sum += count[i];
    }

    let mut transform = vec![0usize; n];
    let mut count2 = [0usize; 256];
    for (i, &b) in data.iter().enumerate() {
        transform[cumsum[b as usize] + count2[b as usize]] = i;
        count2[b as usize] += 1;
    }

    // Build sorted data (first column F)
    let mut sorted_data = data.to_vec();
    sorted_data.sort();

    // Reconstruct original by reading from F and following FL transform
    let mut output = Vec::with_capacity(n);
    let mut idx = origin;
    for _ in 0..n {
        output.push(sorted_data[idx]);
        idx = transform[idx];
    }

    output
}

// Move-to-front encoding with symbol map (for encoder)
fn mtf_encode_with_map(data: &[u8], symbol_map: &[u8]) -> Vec<u8> {
    let mut mtf: Vec<u8> = symbol_map.to_vec();
    let mut output = Vec::with_capacity(data.len());

    for &byte in data {
        let pos = mtf.iter().position(|&x| x == byte).unwrap();
        output.push(pos as u8);
        // Move to front
        for i in (1..=pos).rev() {
            mtf[i] = mtf[i - 1];
        }
        mtf[0] = byte;
    }

    output
}

fn mtf_decode(data: &[u8], symbol_map: &[u8]) -> Vec<u8> {
    let mut mtf: Vec<u8> = symbol_map.to_vec();
    let mut output = Vec::with_capacity(data.len());

    for &pos in data {
        let pos = pos as usize;
        if pos < mtf.len() {
            let byte = mtf[pos];
            output.push(byte);
            // Move to front
            for i in (1..=pos).rev() {
                mtf[i] = mtf[i - 1];
            }
            mtf[0] = byte;
        }
    }

    output
}

// Zero-run-length encoding (RUNA=0, RUNB=1)
// Uses bijective base-2 numeration: digits are 1 (RUNA) and 2 (RUNB)
// run = sum of (digit * 2^position) where digit is 1 for RUNA, 2 for RUNB
fn zrle_encode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if data[i] == 0 {
            // Count zeros
            let mut run = 1;
            while i + run < data.len() && data[i + run] == 0 {
                run += 1;
            }
            // Encode run using bijective base-2
            let mut n = run;
            while n > 0 {
                if n % 2 == 1 {
                    output.push(0); // RUNA = digit 1
                    n = (n - 1) / 2;
                } else {
                    output.push(1); // RUNB = digit 2
                    n = (n - 2) / 2;
                }
            }
            i += run;
        } else {
            output.push(data[i] + 1);
            i += 1;
        }
    }

    output
}

fn zrle_decode(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if data[i] == 0 || data[i] == 1 {
            // Decode RUNA/RUNB sequence
            let mut run = 0;
            let mut power = 1;
            while i < data.len() && (data[i] == 0 || data[i] == 1) {
                run += power * (data[i] as usize + 1);
                power <<= 1;
                i += 1;
            }
            for _ in 0..run {
                output.push(0);
            }
        } else {
            output.push(data[i] - 1);
            i += 1;
        }
    }

    output
}

// Bit writer for output
struct BitWriter {
    data: Vec<u8>,
    buffer: u32,
    bits: u8,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            data: Vec::new(),
            buffer: 0,
            bits: 0,
        }
    }

    fn write_bits(&mut self, value: u32, count: usize) {
        for i in (0..count).rev() {
            self.buffer = (self.buffer << 1) | ((value >> i) & 1);
            self.bits += 1;
            if self.bits == 8 {
                self.data.push(self.buffer as u8);
                self.buffer = 0;
                self.bits = 0;
            }
        }
    }

    fn flush(&mut self) {
        if self.bits > 0 {
            self.data.push((self.buffer << (8 - self.bits)) as u8);
            self.buffer = 0;
            self.bits = 0;
        }
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}

// Bit reader for input
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buffer: u32,
    bits: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            buffer: 0,
            bits: 0,
        }
    }

    fn read_bits(&mut self, count: usize) -> u32 {
        let mut result = 0;
        for _ in 0..count {
            if self.bits == 0 {
                if self.pos < self.data.len() {
                    self.buffer = self.data[self.pos] as u32;
                    self.pos += 1;
                    self.bits = 8;
                } else {
                    return result;
                }
            }
            self.bits -= 1;
            result = (result << 1) | ((self.buffer >> self.bits) & 1);
        }
        result
    }
}

// Simple Huffman decoder
struct HuffmanDecoder {
    max_len: u8,
    codes: Vec<(u32, u8, usize)>, // (code, length, symbol)
}

impl HuffmanDecoder {
    fn from_lengths(lengths: &[u8]) -> Self {
        let max_len = *lengths.iter().max().unwrap_or(&0);
        let mut codes = Vec::new();

        // Build canonical codes
        let mut bl_count = vec![0u32; max_len as usize + 1];
        for &len in lengths {
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        let mut next_code = vec![0u32; max_len as usize + 1];
        let mut code = 0u32;
        for bits in 1..=max_len {
            code = (code + bl_count[bits as usize - 1]) << 1;
            next_code[bits as usize] = code;
        }

        for (symbol, &len) in lengths.iter().enumerate() {
            if len > 0 {
                codes.push((next_code[len as usize], len, symbol));
                next_code[len as usize] += 1;
            }
        }

        HuffmanDecoder { max_len, codes }
    }

    fn decode(&self, reader: &mut BitReader) -> usize {
        let mut code = 0u32;
        for len in 1..=self.max_len {
            code = (code << 1) | reader.read_bits(1);
            for &(c, l, sym) in &self.codes {
                if l == len && c == code {
                    return sym;
                }
            }
        }
        0
    }
}

// bzip2 CRC32 (different polynomial from gzip)
fn bzip2_crc32(data: &[u8]) -> u32 {
    static CRC_TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = (i as u32) << 24;
            let mut j = 0;
            while j < 8 {
                if c & 0x80000000 != 0 {
                    c = (c << 1) ^ 0x04c11db7;
                } else {
                    c <<= 1;
                }
                j += 1;
            }
            table[i] = c;
            i += 1;
        }
        table
    };

    let mut crc = 0xffffffff_u32;
    for &b in data {
        crc = (crc << 8) ^ CRC_TABLE[((crc >> 24) ^ (b as u32)) as usize];
    }
    !crc
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_bzip2_compress() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["bzip2", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"Hello, World!").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.len() >= 4);
        assert_eq!(&output.stdout[0..2], b"BZ");
    }

    #[test]
    fn test_bzip2_invalid_input() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["bzip2", "-d", "-c"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"invalid").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
    }
}
