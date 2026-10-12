//! compress - compress files using LZW
//!
//! Unix compress utility (legacy format).

use super::{get_arg, open_read, open_write_create};
use crate::io;
use alloc::vec;
use alloc::vec::Vec;

// LZW constants
const MAGIC: [u8; 2] = [0x1f, 0x9d];
const MIN_BITS: u8 = 9;
const MAX_BITS: u8 = 16;
const CLEAR_CODE: u16 = 256;
const FIRST_CODE: u16 = 257;

/// compress - compress files using LZW
///
/// # Synopsis
/// ```text
/// compress [-dckf] [FILE...]
/// ```
///
/// # Description
/// Compress files using LZW algorithm.
///
/// # Options
/// - `-d`: Decompress
/// - `-c`: Write to stdout
/// - `-k`: Keep original files
/// - `-f`: Force overwrite
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn compress(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress = false;
    let mut stdout_mode = false;
    let mut keep = false;
    let mut files: Vec<&[u8]> = Vec::new();

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") {
                for &c in &arg[1..] {
                    match c {
                        b'd' => decompress = true,
                        b'c' => stdout_mode = true,
                        b'k' => keep = true,
                        b'f' => {} // Force, ignored for now
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
            decompress_stream(0, 1)
        } else {
            compress_stream(0, 1)
        }
    } else {
        for &file in &files {
            if stdout_mode {
                let fd = open_read(file);
                if fd < 0 {
                    io::write_str(2, b"compress: cannot open file\n");
                    return 1;
                }
                let result = if decompress {
                    decompress_stream(fd, 1)
                } else {
                    compress_stream(fd, 1)
                };
                io::close(fd);
                if result != 0 {
                    return result;
                }
            } else {
                if decompress {
                    if decompress_file(file, keep) != 0 {
                        return 1;
                    }
                } else {
                    if compress_file(file, keep) != 0 {
                        return 1;
                    }
                }
            }
        }
        0
    }
}

/// uncompress - uncompress LZW compressed files
pub fn uncompress(argc: i32, argv: *const *const u8) -> i32 {
    // uncompress is compress -d
    let mut new_argv: Vec<*const u8> = Vec::new();
    new_argv.push(b"uncompress\0".as_ptr());
    new_argv.push(b"-d\0".as_ptr());
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            new_argv.push(arg.as_ptr());
        }
    }
    compress(new_argv.len() as i32, new_argv.as_ptr())
}

fn compress_stream(input_fd: i32, output_fd: i32) -> i32 {
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

    // Write header
    let max_bits = MAX_BITS;
    let header = [MAGIC[0], MAGIC[1], max_bits | 0x80]; // 0x80 = block mode
    io::write_all(output_fd, &header);

    // LZW compress
    let compressed = lzw_compress(&data, max_bits);

    // Write compressed data
    io::write_all(output_fd, &compressed);

    0
}

fn decompress_stream(input_fd: i32, output_fd: i32) -> i32 {
    // Read header
    let mut header = [0u8; 3];
    if io::read(input_fd, &mut header) != 3 {
        io::write_str(2, b"compress: truncated header\n");
        return 1;
    }

    // Verify magic
    if header[0] != MAGIC[0] || header[1] != MAGIC[1] {
        io::write_str(2, b"compress: not in compressed format\n");
        return 1;
    }

    let max_bits = header[2] & 0x1f;
    let block_mode = header[2] & 0x80 != 0;

    if max_bits < MIN_BITS || max_bits > MAX_BITS {
        io::write_str(2, b"compress: invalid bit size\n");
        return 1;
    }

    // Read compressed data
    let mut compressed = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(input_fd, &mut buf);
        if n <= 0 {
            break;
        }
        compressed.extend_from_slice(&buf[..n as usize]);
    }

    // LZW decompress
    let decompressed = lzw_decompress(&compressed, max_bits, block_mode);

    io::write_all(output_fd, &decompressed);

    0
}

fn compress_file(path: &[u8], keep: bool) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"compress: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    // Create output path with .Z extension
    let mut out_path = Vec::new();
    out_path.extend_from_slice(path);
    out_path.extend_from_slice(b".Z\0");

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"compress: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = compress_stream(fd, out_fd);

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

fn decompress_file(path: &[u8], keep: bool) -> i32 {
    let fd = open_read(path);
    if fd < 0 {
        io::write_str(2, b"compress: cannot open ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    // Create output path without .Z extension
    let mut out_path = Vec::new();
    if path.ends_with(b".Z") {
        out_path.extend_from_slice(&path[..path.len() - 2]);
    } else {
        out_path.extend_from_slice(path);
        out_path.extend_from_slice(b".out");
    }
    out_path.push(0);

    let out_fd = open_write_create(&out_path, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"compress: cannot create output\n");
        io::close(fd);
        return 1;
    }

    let result = decompress_stream(fd, out_fd);

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

/// LZW compression
fn lzw_compress(data: &[u8], max_bits: u8) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let max_code = (1u32 << max_bits) - 1;
    let mut output = Vec::new();
    let mut bit_buffer: u32 = 0;
    let mut bits_in_buffer: u8 = 0;
    let mut current_bits: u8 = MIN_BITS;

    // Dictionary: maps byte sequences to codes
    // For efficiency, we use a simple hash table
    let mut dict: Vec<Option<(Vec<u8>, u16)>> = vec![None; 65536];
    let mut next_code: u16 = FIRST_CODE;

    fn hash_key(prefix: &[u8]) -> usize {
        let mut h: u32 = 0;
        for &b in prefix {
            h = h.wrapping_mul(31).wrapping_add(b as u32);
        }
        (h as usize) % 65536
    }

    fn find_code(dict: &[Option<(Vec<u8>, u16)>], key: &[u8]) -> Option<u16> {
        let mut idx = hash_key(key);
        for _ in 0..1000 {
            match &dict[idx] {
                Some((k, code)) if k == key => return Some(*code),
                None => return None,
                _ => idx = (idx + 1) % 65536,
            }
        }
        None
    }

    fn insert_code(dict: &mut [Option<(Vec<u8>, u16)>], key: Vec<u8>, code: u16) {
        let mut idx = hash_key(&key);
        for _ in 0..1000 {
            if dict[idx].is_none() {
                dict[idx] = Some((key, code));
                return;
            }
            idx = (idx + 1) % 65536;
        }
    }

    // Initialize dictionary with single bytes
    for i in 0..256u16 {
        insert_code(&mut dict, vec![i as u8], i);
    }

    let emit_code = |code: u16,
                     output: &mut Vec<u8>,
                     bit_buffer: &mut u32,
                     bits_in_buffer: &mut u8,
                     current_bits: u8| {
        *bit_buffer |= (code as u32) << *bits_in_buffer;
        *bits_in_buffer += current_bits;
        while *bits_in_buffer >= 8 {
            output.push(*bit_buffer as u8);
            *bit_buffer >>= 8;
            *bits_in_buffer -= 8;
        }
    };

    let mut current = vec![data[0]];
    for &byte in &data[1..] {
        let mut next = current.clone();
        next.push(byte);

        if find_code(&dict, &next).is_some() {
            current = next;
        } else {
            // Output code for current
            if let Some(code) = find_code(&dict, &current) {
                emit_code(
                    code,
                    &mut output,
                    &mut bit_buffer,
                    &mut bits_in_buffer,
                    current_bits,
                );
            }

            // Add next to dictionary
            if (next_code as u32) <= max_code {
                insert_code(&mut dict, next, next_code);
                next_code += 1;

                // Increase bit width if needed
                if next_code > (1 << current_bits) && current_bits < max_bits {
                    current_bits += 1;
                }
            }

            current = vec![byte];
        }
    }

    // Output final code
    if let Some(code) = find_code(&dict, &current) {
        emit_code(
            code,
            &mut output,
            &mut bit_buffer,
            &mut bits_in_buffer,
            current_bits,
        );
    }

    // Flush remaining bits
    if bits_in_buffer > 0 {
        output.push(bit_buffer as u8);
    }

    output
}

/// LZW decompression
fn lzw_decompress(data: &[u8], max_bits: u8, block_mode: bool) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::new();
    let mut bit_pos: usize = 0;
    let mut current_bits: u8 = MIN_BITS;

    // Dictionary: code -> byte sequence
    let mut dict: Vec<Vec<u8>> = Vec::with_capacity(65536);

    // Initialize with single bytes
    for i in 0..256u16 {
        dict.push(vec![i as u8]);
    }

    // Reserve slot for CLEAR_CODE if block mode
    if block_mode {
        dict.push(Vec::new()); // 256 = clear code
    }

    fn get_code(data: &[u8], bit_pos: &mut usize, bits: u8) -> Option<u16> {
        let mut code: u16 = 0;
        for i in 0..bits {
            let byte_idx = *bit_pos / 8;
            let bit_idx = *bit_pos % 8;
            if byte_idx >= data.len() {
                return None;
            }
            if data[byte_idx] & (1 << bit_idx) != 0 {
                code |= 1 << i;
            }
            *bit_pos += 1;
        }
        Some(code)
    }

    let mut prev: Option<Vec<u8>> = None;
    let mut next_code = if block_mode { FIRST_CODE } else { 256 };

    loop {
        let code = match get_code(data, &mut bit_pos, current_bits) {
            Some(c) => c,
            None => break,
        };

        // Handle clear code in block mode
        if block_mode && code == CLEAR_CODE {
            dict.truncate(257);
            next_code = FIRST_CODE;
            current_bits = MIN_BITS;
            prev = None;
            continue;
        }

        let entry = if (code as usize) < dict.len() {
            dict[code as usize].clone()
        } else if code as usize == dict.len() {
            // Special case: code not yet in dictionary
            if let Some(ref p) = prev {
                let mut e = p.clone();
                e.push(p[0]);
                e
            } else {
                break;
            }
        } else {
            break;
        };

        output.extend_from_slice(&entry);

        // Add to dictionary
        if let Some(ref p) = prev {
            if (next_code as u32) < (1u32 << max_bits) {
                let mut new_entry = p.clone();
                new_entry.push(entry[0]);
                if dict.len() <= next_code as usize {
                    dict.push(new_entry);
                }
                next_code += 1;

                // Increase bit width if needed
                if next_code >= (1 << current_bits) && current_bits < max_bits {
                    current_bits += 1;
                }
            }
        }

        prev = Some(entry);
    }

    output
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
    fn test_compress_stdin() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        // Compress some data
        let mut child = Command::new(&armybox)
            .args(["compress", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(b"Hello, World! This is a test of compress.")
                .unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Verify magic bytes
        assert!(output.stdout.len() >= 3);
        assert_eq!(output.stdout[0], 0x1f);
        assert_eq!(output.stdout[1], 0x9d);
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let test_data = b"ABABABABABABABABABABABABABABABABABAB";

        // Compress
        let mut compress = Command::new(&armybox)
            .args(["compress", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = compress.stdin.as_mut().unwrap();
            stdin.write_all(test_data).unwrap();
        }

        let compressed = compress.wait_with_output().unwrap();
        assert_eq!(compressed.status.code(), Some(0));

        // Decompress
        let mut decompress = Command::new(&armybox)
            .args(["compress", "-d", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = decompress.stdin.as_mut().unwrap();
            stdin.write_all(&compressed.stdout).unwrap();
        }

        let decompressed = decompress.wait_with_output().unwrap();
        assert_eq!(decompressed.status.code(), Some(0));
        assert_eq!(&decompressed.stdout[..], test_data);
    }

    #[test]
    fn test_uncompress_alias() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["uncompress", "-c"])
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
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not in compressed format"));
    }
}
