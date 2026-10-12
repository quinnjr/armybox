//! base64 - base64 encode/decode
//!
//! Encode or decode data in base64.

use super::has_opt;
use crate::io;

/// base64 - base64 encode/decode
///
/// # Synopsis
/// ```text
/// base64 [-d] [FILE]
/// ```
///
/// # Description
/// Encode or decode base64 data.
///
/// # Options
/// - `-d`: Decode
///
/// # Exit Status
/// - 0: Success
pub fn base64(argc: i32, argv: *const *const u8) -> i32 {
    use alloc::vec::Vec;

    let mut decode = false;
    let mut ignore_garbage = false;
    let mut files: Vec<&[u8]> = Vec::new();

    for i in 1..argc {
        if let Some(arg) = unsafe { super::get_arg(argv, i) } {
            if arg == b"-d" || arg == b"--decode" {
                decode = true;
            } else if arg == b"-i" || arg == b"--ignore-garbage" {
                ignore_garbage = true;
            } else if arg.len() > 1 && arg[0] == b'-' && arg != b"-" {
                // Combined short options, e.g. -di
                if has_opt(arg, b'd') {
                    decode = true;
                }
                if has_opt(arg, b'i') {
                    ignore_garbage = true;
                }
            } else {
                files.push(arg);
            }
        }
    }

    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    // Gather input: concatenate all FILE operands, or stdin if none/`-`.
    let mut content: Vec<u8> = Vec::new();
    if files.is_empty() {
        content = io::read_all(0);
    } else {
        for path in &files {
            if *path == b"-" {
                content.extend_from_slice(&io::read_all(0));
                continue;
            }
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"base64: ");
                io::write_all(2, path);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            content.extend_from_slice(&io::read_all(fd));
            io::close(fd);
        }
    }

    if decode {
        // Decode: skip whitespace/newlines, honor '=' padding, validate alphabet.
        let mut sextets: Vec<u8> = Vec::new();
        let mut pad_count = 0usize;

        for &c in &content {
            match c {
                b'\n' | b'\r' | b' ' | b'\t' => continue,
                b'=' => {
                    pad_count += 1;
                    continue;
                }
                _ => {
                    if pad_count > 0 {
                        // '=' padding followed by more data is invalid.
                        if !ignore_garbage {
                            io::write_str(2, b"base64: invalid input\n");
                            return 1;
                        }
                        continue;
                    }
                    match ALPHABET.iter().position(|&a| a == c) {
                        Some(v) => sextets.push(v as u8),
                        None => {
                            if !ignore_garbage {
                                io::write_str(2, b"base64: invalid input\n");
                                return 1;
                            }
                        }
                    }
                }
            }
        }

        let mut output: Vec<u8> = Vec::new();
        let mut i = 0;
        while i + 4 <= sextets.len() {
            let a = sextets[i] as u32;
            let b = sextets[i + 1] as u32;
            let c = sextets[i + 2] as u32;
            let d = sextets[i + 3] as u32;
            output.push(((a << 2) | (b >> 4)) as u8);
            output.push((((b & 0xf) << 4) | (c >> 2)) as u8);
            output.push((((c & 0x3) << 6) | d) as u8);
            i += 4;
        }
        let rem = sextets.len() - i;
        if rem == 2 {
            let a = sextets[i] as u32;
            let b = sextets[i + 1] as u32;
            output.push(((a << 2) | (b >> 4)) as u8);
        } else if rem == 3 {
            let a = sextets[i] as u32;
            let b = sextets[i + 1] as u32;
            let c = sextets[i + 2] as u32;
            output.push(((a << 2) | (b >> 4)) as u8);
            output.push((((b & 0xf) << 4) | (c >> 2)) as u8);
        } else if rem == 1 {
            if !ignore_garbage {
                io::write_str(2, b"base64: invalid input\n");
                return 1;
            }
        }

        io::write_all(1, &output);
    } else {
        // Encode the whole input at once so 3-byte groups never split
        // across a read boundary. Line-wrap at 76 columns (GNU default).
        let mut out: Vec<u8> = Vec::new();
        let mut col = 0usize;
        let mut i = 0;
        while i + 3 <= content.len() {
            let a = content[i];
            let b = content[i + 1];
            let c = content[i + 2];
            out.push(ALPHABET[(a >> 2) as usize]);
            out.push(ALPHABET[(((a & 0x3) << 4) | (b >> 4)) as usize]);
            out.push(ALPHABET[(((b & 0xf) << 2) | (c >> 6)) as usize]);
            out.push(ALPHABET[(c & 0x3f) as usize]);
            i += 3;
        }
        if i < content.len() {
            let a = content[i];
            let b = if i + 1 < content.len() {
                content[i + 1]
            } else {
                0
            };
            out.push(ALPHABET[(a >> 2) as usize]);
            out.push(ALPHABET[(((a & 0x3) << 4) | (b >> 4)) as usize]);
            if i + 1 < content.len() {
                out.push(ALPHABET[((b & 0xf) << 2) as usize]);
                out.push(b'=');
            } else {
                out.push(b'=');
                out.push(b'=');
            }
        }

        // Write with 76-column line wrapping.
        let mut written: Vec<u8> = Vec::new();
        for &b in &out {
            written.push(b);
            col += 1;
            if col == 76 {
                written.push(b'\n');
                col = 0;
            }
        }
        io::write_all(1, &written);
        if col > 0 || out.is_empty() {
            io::write_str(1, b"\n");
        }
    }
    0
}

/// base32 - base32 encode/decode
///
/// Usage: base32 [-d] [FILE]
/// RFC 4648 Base32 encoding/decoding.
#[cfg(feature = "alloc")]
pub fn base32(argc: i32, argv: *const *const u8) -> i32 {
    use alloc::vec::Vec;

    const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut decode = false;
    let mut input_file: Option<&[u8]> = None;
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { super::get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-d" || arg == b"--decode" {
            decode = true;
        } else if arg == b"-w" || arg == b"--wrap" {
            i += 1; // Skip wrap column argument
        } else if !arg.starts_with(b"-") {
            input_file = Some(arg);
        }
        i += 1;
    }

    // Read input
    let fd = match input_file {
        Some(path) if path != b"-" => {
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"base32: ");
                io::write_all(2, path);
                io::write_str(2, b": No such file\n");
                return 1;
            }
            fd
        }
        _ => 0,
    };

    let content = io::read_all(fd);
    if fd > 0 {
        io::close(fd);
    }

    if decode {
        // Decode
        let mut output: Vec<u8> = Vec::new();
        let mut buffer: u64 = 0;
        let mut bits = 0;

        for &c in &content {
            let val = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a',
                b'2'..=b'7' => c - b'2' + 26,
                b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
                _ => continue,
            };

            buffer = (buffer << 5) | (val as u64);
            bits += 5;

            if bits >= 8 {
                bits -= 8;
                output.push(((buffer >> bits) & 0xFF) as u8);
            }
        }

        io::write_all(1, &output);
    } else {
        // Encode
        let mut i = 0;
        let mut col = 0;

        while i < content.len() {
            // Take up to 5 bytes (40 bits) -> 8 base32 chars
            let mut buf: u64 = 0;
            let mut bytes = 0;

            for j in 0..5 {
                if i + j < content.len() {
                    buf = (buf << 8) | (content[i + j] as u64);
                    bytes += 1;
                } else {
                    buf <<= 8;
                }
            }

            let chars = (bytes * 8 + 4) / 5; // Round up

            for j in 0..8 {
                if j < chars {
                    let idx = ((buf >> (35 - j * 5)) & 0x1F) as usize;
                    io::write_all(1, &[BASE32_ALPHABET[idx]]);
                } else {
                    io::write_str(1, b"=");
                }
                col += 1;
                if col >= 76 {
                    io::write_str(1, b"\n");
                    col = 0;
                }
            }

            i += 5;
        }

        if col > 0 {
            io::write_str(1, b"\n");
        }
    }

    0
}

#[cfg(not(feature = "alloc"))]
pub fn base32(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"base32: requires alloc feature\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_base64_encode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["base64"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        child.stdin.as_mut().unwrap().write_all(b"Hello").unwrap();
        drop(child.stdin.take());

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "SGVsbG8=");
    }

    #[test]
    fn test_base64_decode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["base64", "-d"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"SGVsbG8=")
            .unwrap();
        drop(child.stdin.take());

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(&output.stdout, b"Hello");
    }
}
