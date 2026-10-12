//! gzip - compress or decompress files
//!
//! GNU zip compression utility.

use super::{get_arg, open_read, open_write_create};
use crate::io;
use alloc::vec;
use alloc::vec::Vec;

/// gzip - compress or decompress files
///
/// # Synopsis
/// ```text
/// gzip [-dkc] [FILE...]
/// ```
///
/// # Description
/// Compress files using DEFLATE algorithm.
///
/// # Options
/// - `-d`: Decompress
/// - `-k`: Keep original file
/// - `-c`: Write to stdout
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn gzip(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress = false;
    let mut keep = false;
    let mut stdout_mode = false;
    let mut files: Vec<&[u8]> = Vec::new();

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") {
                for &c in &arg[1..] {
                    match c {
                        b'd' => decompress = true,
                        b'k' => keep = true,
                        b'c' => stdout_mode = true,
                        _ => {}
                    }
                }
            } else {
                files.push(arg);
            }
        }
    }

    if files.is_empty() {
        // Read from stdin, write to stdout
        if decompress {
            gunzip_stream(0, 1)
        } else {
            gzip_stream(0, 1)
        }
    } else {
        for &file in &files {
            if stdout_mode {
                let fd = open_read(file);
                if fd < 0 {
                    io::write_str(2, b"gzip: cannot open file\n");
                    return 1;
                }
                let result = if decompress {
                    gunzip_stream(fd, 1)
                } else {
                    gzip_stream(fd, 1)
                };
                io::close(fd);
                if result != 0 {
                    return result;
                }
            } else {
                if decompress {
                    if gunzip_file(file, keep) != 0 {
                        return 1;
                    }
                } else {
                    if gzip_file(file, keep) != 0 {
                        return 1;
                    }
                }
            }
        }
        0
    }
}

/// gunzip - decompress files
pub fn gunzip(argc: i32, argv: *const *const u8) -> i32 {
    // gunzip is gzip -d
    let mut new_argv: Vec<*const u8> = Vec::new();
    new_argv.push(b"gunzip\0".as_ptr());
    new_argv.push(b"-d\0".as_ptr());
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            new_argv.push(arg.as_ptr());
        }
    }
    gzip(new_argv.len() as i32, new_argv.as_ptr())
}

/// zcat - decompress to stdout
pub fn zcat(argc: i32, argv: *const *const u8) -> i32 {
    // zcat is gzip -dc
    let mut new_argv: Vec<*const u8> = Vec::new();
    new_argv.push(b"zcat\0".as_ptr());
    new_argv.push(b"-dc\0".as_ptr());
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            new_argv.push(arg.as_ptr());
        }
    }
    gzip(new_argv.len() as i32, new_argv.as_ptr())
}

fn gzip_stream(input_fd: i32, output_fd: i32) -> i32 {
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

    // Compute CRC32
    let crc = crc32(&data);
    let size = data.len() as u32;

    // Write gzip header
    let header = [
        0x1f, 0x8b, // Magic
        0x08, // Compression method (deflate)
        0x00, // Flags
        0, 0, 0, 0,    // Mtime
        0x00, // Extra flags
        0xff, // OS (unknown)
    ];
    io::write_all(output_fd, &header);

    // Write DEFLATE compressed data (using stored blocks for simplicity)
    deflate_stored(&data, output_fd);

    // Write CRC32 and original size
    io::write_all(output_fd, &crc.to_le_bytes());
    io::write_all(output_fd, &size.to_le_bytes());

    0
}

fn deflate_stored(data: &[u8], fd: i32) {
    // Use stored blocks (no compression) - valid DEFLATE but not efficient
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_size = remaining.min(65535);
        let is_final = offset + block_size >= data.len();

        // Block header: BFINAL (1 bit) + BTYPE=00 (2 bits) = stored block
        let header_byte = if is_final { 0x01 } else { 0x00 };
        io::write_all(fd, &[header_byte]);

        // LEN and NLEN (little-endian)
        let len = block_size as u16;
        let nlen = !len;
        io::write_all(fd, &len.to_le_bytes());
        io::write_all(fd, &nlen.to_le_bytes());

        // Data
        io::write_all(fd, &data[offset..offset + block_size]);

        offset += block_size;
    }
}

fn gunzip_stream(input_fd: i32, output_fd: i32) -> i32 {
    // Read gzip header
    let mut header = [0u8; 10];
    if io::read(input_fd, &mut header) != 10 {
        io::write_str(2, b"gzip: truncated header\n");
        return 1;
    }

    // Verify magic
    if header[0] != 0x1f || header[1] != 0x8b {
        io::write_str(2, b"gzip: not gzip format\n");
        return 1;
    }

    // Verify compression method is deflate
    if header[2] != 0x08 {
        io::write_str(2, b"gzip: unsupported compression method\n");
        return 1;
    }

    let flags = header[3];

    // Skip optional fields
    if flags & 0x04 != 0 {
        // FEXTRA
        let mut len_buf = [0u8; 2];
        io::read(input_fd, &mut len_buf);
        let len = u16::from_le_bytes(len_buf) as usize;
        let mut skip = vec![0u8; len];
        io::read(input_fd, &mut skip);
    }
    if flags & 0x08 != 0 {
        // FNAME - skip null-terminated string
        let mut b = [0u8; 1];
        loop {
            io::read(input_fd, &mut b);
            if b[0] == 0 {
                break;
            }
        }
    }
    if flags & 0x10 != 0 {
        // FCOMMENT - skip null-terminated string
        let mut b = [0u8; 1];
        loop {
            io::read(input_fd, &mut b);
            if b[0] == 0 {
                break;
            }
        }
    }
    if flags & 0x02 != 0 {
        // FHCRC
        let mut crc16 = [0u8; 2];
        io::read(input_fd, &mut crc16);
    }

    // Read remaining compressed data
    let mut compressed = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(input_fd, &mut buf);
        if n <= 0 {
            break;
        }
        compressed.extend_from_slice(&buf[..n as usize]);
    }

    // The last 8 bytes are CRC32 (LE) and ISIZE (LE); capture them before truncating
    if compressed.len() < 8 {
        io::write_str(2, b"gzip: truncated file\n");
        return 1;
    }

    let trailer_start = compressed.len() - 8;
    let expected_crc = u32::from_le_bytes([
        compressed[trailer_start],
        compressed[trailer_start + 1],
        compressed[trailer_start + 2],
        compressed[trailer_start + 3],
    ]);
    let expected_isize = u32::from_le_bytes([
        compressed[trailer_start + 4],
        compressed[trailer_start + 5],
        compressed[trailer_start + 6],
        compressed[trailer_start + 7],
    ]);
    compressed.truncate(trailer_start);

    // Decompress DEFLATE data
    let decompressed = match inflate(&compressed) {
        Ok(d) => d,
        Err(()) => {
            io::write_str(2, b"gunzip: invalid compressed data--crc error\n");
            return 1;
        }
    };

    // Validate the gzip trailer (CRC32 + ISIZE) against the decompressed data
    // before trusting it or handing it to the caller.
    let actual_crc = crc32(&decompressed);
    let actual_isize = decompressed.len() as u32;
    if actual_crc != expected_crc || actual_isize != expected_isize {
        io::write_str(2, b"gunzip: invalid compressed data--crc error\n");
        return 1;
    }

    io::write_all(output_fd, &decompressed);

    0
}

fn gzip_file(path: &[u8], keep: bool) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"gzip: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    // Create output path with .gz extension
    let mut out_path = Vec::new();
    out_path.extend_from_slice(path);
    out_path.extend_from_slice(b".gz\0");

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"gzip: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = gzip_stream(fd, out_fd);

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

fn gunzip_file(path: &[u8], keep: bool) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"gzip: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    // Create output path without .gz extension
    let mut out_path = Vec::new();
    if path.ends_with(b".gz") {
        out_path.extend_from_slice(&path[..path.len() - 3]);
    } else {
        out_path.extend_from_slice(path);
        out_path.extend_from_slice(b".out");
    }
    out_path.push(0);

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"gzip: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = gunzip_stream(fd, out_fd);

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

// DEFLATE decompression (inflate)
//
// Returns `Err(())` on any malformed input (invalid block type, truncated
// bitstream, invalid Huffman code, or an out-of-range back-reference) instead
// of silently truncating the output, so callers can detect corrupt archives.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, ()> {
    let mut output = Vec::new();
    let mut bit_pos = 0usize;

    fn get_bits(data: &[u8], bit_pos: &mut usize, count: usize) -> Option<u32> {
        let mut result = 0u32;
        for i in 0..count {
            let byte_idx = *bit_pos / 8;
            if byte_idx >= data.len() {
                return None;
            }
            let bit_idx = *bit_pos % 8;
            if data[byte_idx] & (1 << bit_idx) != 0 {
                result |= 1 << i;
            }
            *bit_pos += 1;
        }
        Some(result)
    }

    loop {
        let bfinal = get_bits(data, &mut bit_pos, 1).ok_or(())?;
        let btype = get_bits(data, &mut bit_pos, 2).ok_or(())?;

        match btype {
            0 => {
                // Stored block
                bit_pos = (bit_pos + 7) & !7;
                let len = get_bits(data, &mut bit_pos, 16).ok_or(())? as u16;
                let nlen = get_bits(data, &mut bit_pos, 16).ok_or(())? as u16;
                if nlen != !len {
                    return Err(());
                }
                let len = len as usize;

                let byte_pos = bit_pos / 8;
                if byte_pos + len > data.len() {
                    return Err(());
                }
                output.extend_from_slice(&data[byte_pos..byte_pos + len]);
                bit_pos += len * 8;
            }
            1 => {
                // Fixed Huffman
                inflate_fixed_huffman(data, &mut bit_pos, &mut output)?;
            }
            2 => {
                // Dynamic Huffman
                inflate_dynamic_huffman(data, &mut bit_pos, &mut output)?;
            }
            _ => {
                // BTYPE 3 is reserved/invalid
                return Err(());
            }
        }

        if bfinal != 0 {
            break;
        }
    }

    Ok(output)
}

fn inflate_fixed_huffman(data: &[u8], bit_pos: &mut usize, output: &mut Vec<u8>) -> Result<(), ()> {
    fn get_bits(data: &[u8], bit_pos: &mut usize, count: usize) -> Option<u32> {
        let mut result = 0u32;
        for i in 0..count {
            let byte_idx = *bit_pos / 8;
            if byte_idx >= data.len() {
                return None;
            }
            let bit_idx = *bit_pos % 8;
            if data[byte_idx] & (1 << bit_idx) != 0 {
                result |= 1 << i;
            }
            *bit_pos += 1;
        }
        Some(result)
    }

    fn get_bits_rev(data: &[u8], bit_pos: &mut usize, count: usize) -> Option<u32> {
        let mut result = 0u32;
        for _ in 0..count {
            result <<= 1;
            let byte_idx = *bit_pos / 8;
            if byte_idx >= data.len() {
                return None;
            }
            let bit_idx = *bit_pos % 8;
            if data[byte_idx] & (1 << bit_idx) != 0 {
                result |= 1;
            }
            *bit_pos += 1;
        }
        Some(result)
    }

    let length_bases: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    let length_extra: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    let dist_bases: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    let dist_extra: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    loop {
        let mut code = get_bits_rev(data, bit_pos, 7).ok_or(())?;

        let symbol = if code <= 0b0010111 {
            code + 256
        } else {
            code = (code << 1) | get_bits_rev(data, bit_pos, 1).ok_or(())?;
            if code <= 0b10111111 {
                code - 0b00110000
            } else if code <= 0b11000111 {
                code - 0b11000000 + 280
            } else {
                code = (code << 1) | get_bits_rev(data, bit_pos, 1).ok_or(())?;
                code - 0b110010000 + 144
            }
        };

        if symbol < 256 {
            output.push(symbol as u8);
        } else if symbol == 256 {
            break;
        } else {
            let length_idx = (symbol - 257) as usize;
            if length_idx >= 29 {
                return Err(());
            }
            let length = length_bases[length_idx] as usize
                + get_bits(data, bit_pos, length_extra[length_idx] as usize).ok_or(())? as usize;

            let dist_code = get_bits_rev(data, bit_pos, 5).ok_or(())? as usize;
            if dist_code >= 30 {
                return Err(());
            }
            let distance = dist_bases[dist_code] as usize
                + get_bits(data, bit_pos, dist_extra[dist_code] as usize).ok_or(())? as usize;

            if distance == 0 || distance > output.len() {
                return Err(());
            }
            let start = output.len() - distance;
            for i in 0..length {
                let idx = start + (i % distance);
                output.push(output[idx]);
            }
        }
    }

    Ok(())
}

fn inflate_dynamic_huffman(
    data: &[u8],
    bit_pos: &mut usize,
    output: &mut Vec<u8>,
) -> Result<(), ()> {
    fn get_bits(data: &[u8], bit_pos: &mut usize, count: usize) -> Option<u32> {
        let mut result = 0u32;
        for i in 0..count {
            let byte_idx = *bit_pos / 8;
            if byte_idx >= data.len() {
                return None;
            }
            let bit_idx = *bit_pos % 8;
            if data[byte_idx] & (1 << bit_idx) != 0 {
                result |= 1 << i;
            }
            *bit_pos += 1;
        }
        Some(result)
    }

    let hlit = get_bits(data, bit_pos, 5).ok_or(())? as usize + 257;
    let hdist = get_bits(data, bit_pos, 5).ok_or(())? as usize + 1;
    let hclen = get_bits(data, bit_pos, 4).ok_or(())? as usize + 4;

    let order: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let mut code_length_lengths = [0u8; 19];
    for i in 0..hclen {
        code_length_lengths[order[i]] = get_bits(data, bit_pos, 3).ok_or(())? as u8;
    }

    let code_length_tree = build_huffman_tree(&code_length_lengths);

    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < hlit + hdist {
        let sym = decode_huffman(data, bit_pos, &code_length_tree).ok_or(())?;
        match sym {
            0..=15 => {
                lengths[i] = sym as u8;
                i += 1;
            }
            16 => {
                let repeat = get_bits(data, bit_pos, 2).ok_or(())? as usize + 3;
                let val = if i > 0 { lengths[i - 1] } else { 0 };
                for _ in 0..repeat {
                    if i >= lengths.len() {
                        return Err(());
                    }
                    lengths[i] = val;
                    i += 1;
                }
            }
            17 => {
                let repeat = get_bits(data, bit_pos, 3).ok_or(())? as usize + 3;
                for _ in 0..repeat {
                    if i >= lengths.len() {
                        return Err(());
                    }
                    lengths[i] = 0;
                    i += 1;
                }
            }
            18 => {
                let repeat = get_bits(data, bit_pos, 7).ok_or(())? as usize + 11;
                for _ in 0..repeat {
                    if i >= lengths.len() {
                        return Err(());
                    }
                    lengths[i] = 0;
                    i += 1;
                }
            }
            _ => return Err(()),
        }
    }

    let lit_tree = build_huffman_tree(&lengths[..hlit]);
    let dist_tree = build_huffman_tree(&lengths[hlit..]);

    let length_bases: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    let length_extra: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    let dist_bases: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    let dist_extra: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    loop {
        let symbol = decode_huffman(data, bit_pos, &lit_tree).ok_or(())?;

        if symbol < 256 {
            output.push(symbol as u8);
        } else if symbol == 256 {
            break;
        } else {
            let length_idx = (symbol - 257) as usize;
            if length_idx >= 29 {
                return Err(());
            }
            let length = length_bases[length_idx] as usize
                + get_bits(data, bit_pos, length_extra[length_idx] as usize).ok_or(())? as usize;

            let dist_code = decode_huffman(data, bit_pos, &dist_tree).ok_or(())? as usize;
            if dist_code >= 30 {
                return Err(());
            }
            let distance = dist_bases[dist_code] as usize
                + get_bits(data, bit_pos, dist_extra[dist_code] as usize).ok_or(())? as usize;

            if distance == 0 || distance > output.len() {
                return Err(());
            }
            let start = output.len() - distance;
            for i in 0..length {
                let idx = start + (i % distance);
                output.push(output[idx]);
            }
        }
    }

    Ok(())
}

struct HuffmanTree {
    codes: Vec<(u16, u8, u16)>,
    max_bits: u8,
}

fn build_huffman_tree(lengths: &[u8]) -> HuffmanTree {
    let max_bits = *lengths.iter().max().unwrap_or(&0);
    if max_bits == 0 {
        return HuffmanTree {
            codes: Vec::new(),
            max_bits: 0,
        };
    }

    let mut bl_count = vec![0u16; max_bits as usize + 1];
    for &len in lengths {
        if len > 0 {
            bl_count[len as usize] += 1;
        }
    }

    let mut next_code = vec![0u16; max_bits as usize + 1];
    let mut code = 0u16;
    for bits in 1..=max_bits as usize {
        code = (code + bl_count[bits - 1]) << 1;
        next_code[bits] = code;
    }

    let mut codes = Vec::new();
    for (symbol, &len) in lengths.iter().enumerate() {
        if len > 0 {
            let c = next_code[len as usize];
            next_code[len as usize] += 1;
            codes.push((c, len, symbol as u16));
        }
    }

    HuffmanTree { codes, max_bits }
}

fn decode_huffman(data: &[u8], bit_pos: &mut usize, tree: &HuffmanTree) -> Option<u16> {
    if tree.codes.is_empty() {
        return None;
    }

    let mut code = 0u16;
    for len in 1..=tree.max_bits {
        let byte_idx = *bit_pos / 8;
        if byte_idx >= data.len() {
            return None;
        }
        let bit_idx = *bit_pos % 8;
        code <<= 1;
        if data[byte_idx] & (1 << bit_idx) != 0 {
            code |= 1;
        }
        *bit_pos += 1;

        for &(c, l, sym) in &tree.codes {
            if l == len && c == code {
                return Some(sym);
            }
        }
    }

    None
}

pub fn crc32(data: &[u8]) -> u32 {
    static CRC_TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut j = 0;
            while j < 8 {
                if c & 1 != 0 {
                    c = 0xedb88320 ^ (c >> 1);
                } else {
                    c >>= 1;
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
        crc = CRC_TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_gzip_not_gzip_format() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["gzip", "-d"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"not gzip data").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not gzip format") || stderr.contains("truncated"));
    }

    #[test]
    fn test_gunzip_alias() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // gunzip should work like gzip -d
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["gunzip"])
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

    #[test]
    fn test_zcat_alias() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["zcat"])
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
