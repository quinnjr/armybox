//! iconv - character set conversion (POSIX compliant)
//!
//! Converts text between character encodings.

extern crate alloc;

use super::get_arg;
use crate::io;
use alloc::vec::Vec;

/// iconv - character set conversion (POSIX compliant)
///
/// POSIX: Converts text between character encodings.
/// Supported: ASCII, UTF-8, ISO-8859-1 (Latin-1)
///
/// # Synopsis
/// ```text
/// iconv [-f from-code] [-t to-code] [-l] [file...]
/// ```
///
/// # Options
/// - `-f from-code`: Source encoding (default: UTF-8)
/// - `-t to-code`: Target encoding (default: UTF-8)
/// - `-l, --list`: List available encodings
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn iconv(argc: i32, argv: *const *const u8) -> i32 {
    let mut from_code: &[u8] = b"UTF-8";
    let mut to_code: &[u8] = b"UTF-8";
    let mut input_files: Vec<&[u8]> = Vec::new();
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-f" || arg == b"--from-code" {
            i += 1;
            if let Some(code) = unsafe { get_arg(argv, i as i32) } {
                from_code = code;
            }
        } else if arg == b"-t" || arg == b"--to-code" {
            i += 1;
            if let Some(code) = unsafe { get_arg(argv, i as i32) } {
                to_code = code;
            }
        } else if arg == b"-l" || arg == b"--list" {
            io::write_str(1, b"ASCII\nUTF-8\nISO-8859-1\nLATIN1\n");
            return 0;
        } else if !arg.starts_with(b"-") {
            input_files.push(arg);
        }
        i += 1;
    }

    // Normalize encoding names (case-insensitive)
    let from_enc = normalize_encoding(from_code);
    let to_enc = normalize_encoding(to_code);

    // Process files or stdin
    if input_files.is_empty() {
        let content = io::read_all(0);
        convert_and_output(&content, from_enc, to_enc);
    } else {
        for path in input_files {
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"iconv: ");
                io::write_all(2, path);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            let content = io::read_all(fd);
            io::close(fd);
            convert_and_output(&content, from_enc, to_enc);
        }
    }

    0
}

/// Encoding types we support
#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Ascii,
    Utf8,
    Latin1,
}

/// Normalize encoding name
fn normalize_encoding(name: &[u8]) -> Encoding {
    // Case-insensitive comparison
    let upper: Vec<u8> = name.iter().map(|c| c.to_ascii_uppercase()).collect();

    if upper == b"ASCII" || upper == b"US-ASCII" {
        Encoding::Ascii
    } else if upper == b"UTF-8" || upper == b"UTF8" {
        Encoding::Utf8
    } else if upper == b"ISO-8859-1"
        || upper == b"ISO88591"
        || upper == b"LATIN1"
        || upper == b"LATIN-1"
    {
        Encoding::Latin1
    } else {
        // Default to UTF-8
        Encoding::Utf8
    }
}

/// Convert and output data
fn convert_and_output(data: &[u8], from: Encoding, to: Encoding) {
    // First decode to Unicode codepoints
    let codepoints = decode_to_codepoints(data, from);

    // Then encode to target
    let output = encode_from_codepoints(&codepoints, to);

    io::write_all(1, &output);
}

/// Decode bytes to Unicode codepoints
fn decode_to_codepoints(data: &[u8], enc: Encoding) -> Vec<u32> {
    let mut result: Vec<u32> = Vec::new();

    match enc {
        Encoding::Ascii => {
            for &b in data {
                if b < 128 {
                    result.push(b as u32);
                } else {
                    result.push(0xFFFD); // Replacement character
                }
            }
        }
        Encoding::Latin1 => {
            for &b in data {
                result.push(b as u32); // ISO-8859-1 maps directly to Unicode
            }
        }
        Encoding::Utf8 => {
            let mut i = 0;
            while i < data.len() {
                let b = data[i];
                if b < 0x80 {
                    result.push(b as u32);
                    i += 1;
                } else if b < 0xC0 {
                    result.push(0xFFFD);
                    i += 1;
                } else if b < 0xE0 {
                    if i + 1 < data.len() && data[i + 1] & 0xC0 == 0x80 {
                        let cp = ((b as u32 & 0x1F) << 6) | (data[i + 1] as u32 & 0x3F);
                        result.push(cp);
                        i += 2;
                    } else {
                        result.push(0xFFFD);
                        i += 1;
                    }
                } else if b < 0xF0 {
                    if i + 2 < data.len()
                        && data[i + 1] & 0xC0 == 0x80
                        && data[i + 2] & 0xC0 == 0x80
                    {
                        let cp = ((b as u32 & 0x0F) << 12)
                            | ((data[i + 1] as u32 & 0x3F) << 6)
                            | (data[i + 2] as u32 & 0x3F);
                        result.push(cp);
                        i += 3;
                    } else {
                        result.push(0xFFFD);
                        i += 1;
                    }
                } else if b < 0xF8 {
                    if i + 3 < data.len()
                        && data[i + 1] & 0xC0 == 0x80
                        && data[i + 2] & 0xC0 == 0x80
                        && data[i + 3] & 0xC0 == 0x80
                    {
                        let cp = ((b as u32 & 0x07) << 18)
                            | ((data[i + 1] as u32 & 0x3F) << 12)
                            | ((data[i + 2] as u32 & 0x3F) << 6)
                            | (data[i + 3] as u32 & 0x3F);
                        result.push(cp);
                        i += 4;
                    } else {
                        result.push(0xFFFD);
                        i += 1;
                    }
                } else {
                    result.push(0xFFFD);
                    i += 1;
                }
            }
        }
    }

    result
}

/// Encode codepoints to bytes
fn encode_from_codepoints(codepoints: &[u32], enc: Encoding) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();

    match enc {
        Encoding::Ascii => {
            for &cp in codepoints {
                if cp < 128 {
                    result.push(cp as u8);
                } else {
                    result.push(b'?'); // Replacement for non-ASCII
                }
            }
        }
        Encoding::Latin1 => {
            for &cp in codepoints {
                if cp < 256 {
                    result.push(cp as u8);
                } else {
                    result.push(b'?'); // Replacement for non-Latin1
                }
            }
        }
        Encoding::Utf8 => {
            for &cp in codepoints {
                if cp < 0x80 {
                    result.push(cp as u8);
                } else if cp < 0x800 {
                    result.push(0xC0 | ((cp >> 6) as u8));
                    result.push(0x80 | ((cp & 0x3F) as u8));
                } else if cp < 0x10000 {
                    result.push(0xE0 | ((cp >> 12) as u8));
                    result.push(0x80 | (((cp >> 6) & 0x3F) as u8));
                    result.push(0x80 | ((cp & 0x3F) as u8));
                } else if cp < 0x110000 {
                    result.push(0xF0 | ((cp >> 18) as u8));
                    result.push(0x80 | (((cp >> 12) & 0x3F) as u8));
                    result.push(0x80 | (((cp >> 6) & 0x3F) as u8));
                    result.push(0x80 | ((cp & 0x3F) as u8));
                }
            }
        }
    }

    result
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
    fn test_iconv_list() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["iconv", "-l"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("UTF-8"));
    }
}
