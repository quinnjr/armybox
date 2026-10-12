//! uuencode/uudecode - encode and decode binary files (POSIX compliant)
//!
//! Converts binary files to/from ASCII representation.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// uuencode - encode binary to ASCII (POSIX compliant)
///
/// POSIX: Converts binary file to ASCII representation using uuencoding.
/// Format: "begin MODE FILENAME", encoded lines (up to 45 bytes per line), "end"
///
/// # Synopsis
/// ```text
/// uuencode [-m] [file] name
/// ```
///
/// # Options
/// - `-m`: Use base64 encoding instead of traditional uuencoding
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn uuencode(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = 0o644u32;
    let mut decode_name: Option<&[u8]> = None;
    let mut input_file: Option<&[u8]> = None;
    let mut use_base64 = false;
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-m" {
            use_base64 = true;
        } else if decode_name.is_none() {
            decode_name = Some(arg);
        } else if input_file.is_none() {
            input_file = Some(decode_name.unwrap());
            decode_name = Some(arg);
        }
        i += 1;
    }

    let name = match decode_name {
        Some(n) => n,
        None => {
            io::write_str(2, b"uuencode: missing operand\n");
            io::write_str(2, b"Usage: uuencode [-m] [file] name\n");
            return 1;
        }
    };

    // Open input file or use stdin
    let fd = match input_file {
        Some(path) => {
            // Get file mode if it exists
            let mut stat_buf = io::stat_zeroed();
            if io::stat(path, &mut stat_buf) == 0 {
                mode = stat_buf.st_mode & 0o777;
            }
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"uuencode: ");
                io::write_all(2, path);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            fd
        }
        None => 0, // stdin
    };

    // Read all input
    let content = io::read_all(fd);
    if fd > 0 {
        io::close(fd);
    }

    if use_base64 {
        // Base64 encoding (MIME format)
        io::write_str(1, b"begin-base64 ");
        let mut mode_buf = [0u8; 8];
        let mode_str = sys::format_octal(mode, &mut mode_buf);
        io::write_all(1, mode_str);
        io::write_str(1, b" ");
        io::write_all(1, name);
        io::write_str(1, b"\n");

        encode_base64(&content);

        io::write_str(1, b"====\n");
    } else {
        // Traditional uuencoding
        io::write_str(1, b"begin ");
        let mut mode_buf = [0u8; 8];
        let mode_str = sys::format_octal(mode, &mut mode_buf);
        io::write_all(1, mode_str);
        io::write_str(1, b" ");
        io::write_all(1, name);
        io::write_str(1, b"\n");

        encode_uuencode(&content);

        io::write_str(1, b"`\nend\n");
    }

    0
}

/// uudecode - decode uuencoded file (POSIX compliant)
///
/// POSIX: Decodes uuencoded (or base64) files back to binary.
/// Parses header for filename and mode, writes decoded output.
///
/// # Synopsis
/// ```text
/// uudecode [-o outfile] [file]
/// ```
///
/// # Options
/// - `-o outfile`: Write output to outfile instead of embedded filename
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn uudecode(argc: i32, argv: *const *const u8) -> i32 {
    let mut output_file: Option<&[u8]> = None;
    let mut input_file: Option<&[u8]> = None;
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-o" {
            i += 1;
            output_file = unsafe { get_arg(argv, i as i32) };
        } else if input_file.is_none() {
            input_file = Some(arg);
        }
        i += 1;
    }

    // Open input file or use stdin
    let fd = match input_file {
        Some(path) => {
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"uudecode: ");
                io::write_all(2, path);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            fd
        }
        None => 0,
    };

    // Read all input
    let content = io::read_all(fd);
    if fd > 0 {
        io::close(fd);
    }

    // Find "begin" line
    let mut pos = 0;
    let mut is_base64 = false;
    let mut mode = 0o644u32;
    let mut filename: Vec<u8> = Vec::new();

    // Skip to "begin" or "begin-base64" line
    while pos < content.len() {
        let line_start = pos;
        while pos < content.len() && content[pos] != b'\n' {
            pos += 1;
        }
        let line = &content[line_start..pos];
        if pos < content.len() {
            pos += 1; // Skip newline
        }

        if line.starts_with(b"begin-base64 ") {
            is_base64 = true;
            // Parse mode and filename
            let rest = &line[13..];
            if let Some((m, f)) = parse_begin_line(rest) {
                mode = m;
                filename = f;
            }
            break;
        } else if line.starts_with(b"begin ") {
            // Parse mode and filename
            let rest = &line[6..];
            if let Some((m, f)) = parse_begin_line(rest) {
                mode = m;
                filename = f;
            }
            break;
        }
    }

    if filename.is_empty() {
        io::write_str(2, b"uudecode: no 'begin' line found\n");
        return 1;
    }

    // Use output file override if provided
    let out_name = match output_file {
        Some(name) => name,
        None => &filename,
    };

    // Decode content
    let mut decoded: Vec<u8> = Vec::new();

    if is_base64 {
        // Base64 decoding
        while pos < content.len() {
            let line_start = pos;
            while pos < content.len() && content[pos] != b'\n' {
                pos += 1;
            }
            let line = &content[line_start..pos];
            if pos < content.len() {
                pos += 1;
            }

            if line.starts_with(b"====") {
                break;
            }

            decode_base64_line(line, &mut decoded);
        }
    } else {
        // Traditional uudecoding
        while pos < content.len() {
            let line_start = pos;
            while pos < content.len() && content[pos] != b'\n' {
                pos += 1;
            }
            let line = &content[line_start..pos];
            if pos < content.len() {
                pos += 1;
            }

            if line.is_empty() || line == b"`" || line == b"end" {
                break;
            }

            decode_uuencode_line(line, &mut decoded);
        }
    }

    // Write output
    if out_name == b"-" || out_name == b"/dev/stdout" {
        io::write_all(1, &decoded);
    } else {
        let fd = io::open(
            out_name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            mode,
        );
        if fd < 0 {
            io::write_str(2, b"uudecode: ");
            io::write_all(2, out_name);
            io::write_str(2, b": cannot create\n");
            return 1;
        }
        io::write_all(fd, &decoded);
        io::close(fd);
    }

    0
}

/// Encode a byte as uuencode character
fn uu_encode_char(b: u8) -> u8 {
    if b == 0 {
        b'`' // Backtick for zero
    } else {
        b + 32 // Add space offset
    }
}

/// Encode data using traditional uuencoding
fn encode_uuencode(data: &[u8]) {
    const LINE_BYTES: usize = 45; // 45 bytes per line (60 encoded chars)

    let mut pos = 0;
    while pos < data.len() {
        let chunk_len = core::cmp::min(LINE_BYTES, data.len() - pos);
        let chunk = &data[pos..pos + chunk_len];

        // Line length character
        io::write_all(1, &[uu_encode_char(chunk_len as u8)]);

        // Encode in groups of 3 bytes -> 4 characters
        let mut i = 0;
        while i < chunk_len {
            let b0 = chunk[i];
            let b1 = if i + 1 < chunk_len { chunk[i + 1] } else { 0 };
            let b2 = if i + 2 < chunk_len { chunk[i + 2] } else { 0 };

            let c0 = (b0 >> 2) & 0x3F;
            let c1 = ((b0 << 4) | (b1 >> 4)) & 0x3F;
            let c2 = ((b1 << 2) | (b2 >> 6)) & 0x3F;
            let c3 = b2 & 0x3F;

            io::write_all(
                1,
                &[
                    uu_encode_char(c0),
                    uu_encode_char(c1),
                    uu_encode_char(c2),
                    uu_encode_char(c3),
                ],
            );

            i += 3;
        }

        io::write_str(1, b"\n");
        pos += chunk_len;
    }
}

/// Encode data using base64
fn encode_base64(data: &[u8]) {
    const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const LINE_WIDTH: usize = 76;

    let mut line_pos = 0;
    let mut pos = 0;

    while pos < data.len() {
        let b0 = data[pos];
        let b1 = if pos + 1 < data.len() {
            data[pos + 1]
        } else {
            0
        };
        let b2 = if pos + 2 < data.len() {
            data[pos + 2]
        } else {
            0
        };

        let c0 = BASE64[((b0 >> 2) & 0x3F) as usize];
        let c1 = BASE64[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize];
        let c2 = if pos + 1 < data.len() {
            BASE64[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize]
        } else {
            b'='
        };
        let c3 = if pos + 2 < data.len() {
            BASE64[(b2 & 0x3F) as usize]
        } else {
            b'='
        };

        io::write_all(1, &[c0, c1, c2, c3]);
        line_pos += 4;

        if line_pos >= LINE_WIDTH {
            io::write_str(1, b"\n");
            line_pos = 0;
        }

        pos += 3;
    }

    if line_pos > 0 {
        io::write_str(1, b"\n");
    }
}

/// Parse "MODE FILENAME" from begin line
fn parse_begin_line(s: &[u8]) -> Option<(u32, Vec<u8>)> {
    let mut pos = 0;
    // Parse octal mode
    let mut mode = 0u32;
    while pos < s.len() && s[pos] >= b'0' && s[pos] <= b'7' {
        mode = mode * 8 + (s[pos] - b'0') as u32;
        pos += 1;
    }

    // Skip space
    while pos < s.len() && s[pos] == b' ' {
        pos += 1;
    }

    // Rest is filename
    let filename: Vec<u8> = s[pos..].to_vec();
    if filename.is_empty() {
        return None;
    }

    Some((mode, filename))
}

/// Decode uuencode character
fn uu_decode_char(c: u8) -> u8 {
    if c == b'`' || c == b' ' {
        0
    } else {
        (c - 32) & 0x3F
    }
}

/// Decode a single uuencode line
fn decode_uuencode_line(line: &[u8], output: &mut Vec<u8>) {
    if line.is_empty() {
        return;
    }

    let len = uu_decode_char(line[0]) as usize;
    if len == 0 || line.len() < 2 {
        return;
    }

    let mut pos = 1;
    let mut decoded = 0;

    while decoded < len && pos + 3 < line.len() {
        let c0 = uu_decode_char(line[pos]);
        let c1 = uu_decode_char(line[pos + 1]);
        let c2 = uu_decode_char(line[pos + 2]);
        let c3 = uu_decode_char(line[pos + 3]);

        let b0 = (c0 << 2) | (c1 >> 4);
        let b1 = (c1 << 4) | (c2 >> 2);
        let b2 = (c2 << 6) | c3;

        if decoded < len {
            output.push(b0);
            decoded += 1;
        }
        if decoded < len {
            output.push(b1);
            decoded += 1;
        }
        if decoded < len {
            output.push(b2);
            decoded += 1;
        }

        pos += 4;
    }
}

/// Decode base64 character
fn base64_decode_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        b'=' => None, // Padding
        _ => None,
    }
}

/// Decode a single base64 line
fn decode_base64_line(line: &[u8], output: &mut Vec<u8>) {
    let mut pos = 0;

    while pos + 3 < line.len() {
        let c0 = match base64_decode_char(line[pos]) {
            Some(v) => v,
            None => {
                pos += 1;
                continue;
            }
        };
        let c1 = match base64_decode_char(line[pos + 1]) {
            Some(v) => v,
            None => break,
        };

        output.push((c0 << 2) | (c1 >> 4));

        if line[pos + 2] != b'=' {
            if let Some(c2) = base64_decode_char(line[pos + 2]) {
                output.push((c1 << 4) | (c2 >> 2));

                if line[pos + 3] != b'=' {
                    if let Some(c3) = base64_decode_char(line[pos + 3]) {
                        output.push((c2 << 6) | c3);
                    }
                }
            }
        }

        pos += 4;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_uuencode_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["uuencode", "test"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"Hello, World!").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.starts_with("begin "));
        assert!(stdout.contains("end"));
    }
}
