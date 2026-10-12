//! unicode - display Unicode character information
//!
//! Shows information about Unicode codepoints.

use super::get_arg;
use crate::io;
use crate::sys;

/// unicode - display Unicode character information
///
/// # Synopsis
/// ```text
/// unicode <codepoint>...
/// ```
///
/// # Description
/// Shows information about Unicode codepoints (decimal, hex, or U+XXXX notation).
///
/// # Examples
/// ```text
/// unicode 65
/// unicode 0x41
/// unicode U+0041
/// ```
///
/// # Exit Status
/// - 0: Success
/// - 1: Invalid codepoint or missing argument
pub fn unicode(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"Usage: unicode <codepoint>...\n");
        io::write_str(2, b"Examples: unicode 65, unicode 0x41, unicode U+0041\n");
        return 1;
    }

    for i in 1..argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => continue,
        };

        let codepoint = parse_codepoint(arg);

        if let Some(cp) = codepoint {
            // Print codepoint info
            let mut buf = [0u8; 16];

            io::write_str(1, b"U+");
            let hex = sys::format_hex(cp as u64, &mut buf);
            for _ in 0..(4usize.saturating_sub(hex.len())) {
                io::write_str(1, b"0");
            }
            io::write_all(1, hex);

            io::write_str(1, b" (");
            let dec = sys::format_u64(cp as u64, &mut buf);
            io::write_all(1, dec);
            io::write_str(1, b") ");

            // Print the character if printable
            if cp < 0x110000 {
                let mut utf8 = [0u8; 4];
                let len = encode_utf8(cp, &mut utf8);
                if len > 0 {
                    io::write_str(1, b"'");
                    io::write_all(1, &utf8[..len]);
                    io::write_str(1, b"'");
                }
            }

            io::write_str(1, b"\n");
        } else {
            io::write_str(2, b"unicode: invalid codepoint: ");
            io::write_all(2, arg);
            io::write_str(2, b"\n");
        }
    }

    0
}

/// Parse codepoint from various formats
fn parse_codepoint(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }

    // U+XXXX format
    if s.len() > 2 && (s[0] == b'U' || s[0] == b'u') && s[1] == b'+' {
        return parse_hex(&s[2..]);
    }

    // 0xXXXX format
    if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        return parse_hex(&s[2..]);
    }

    // Decimal
    sys::parse_u64(s).map(|n| n as u32)
}

/// Parse hex number
fn parse_hex(s: &[u8]) -> Option<u32> {
    let mut result: u32 = 0;
    for &c in s {
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        result = result.checked_mul(16)?.checked_add(digit as u32)?;
    }
    Some(result)
}

/// Encode Unicode codepoint to UTF-8
fn encode_utf8(cp: u32, buf: &mut [u8; 4]) -> usize {
    if cp < 0x80 {
        buf[0] = cp as u8;
        1
    } else if cp < 0x800 {
        buf[0] = 0xC0 | ((cp >> 6) as u8);
        buf[1] = 0x80 | ((cp & 0x3F) as u8);
        2
    } else if cp < 0x10000 {
        buf[0] = 0xE0 | ((cp >> 12) as u8);
        buf[1] = 0x80 | (((cp >> 6) & 0x3F) as u8);
        buf[2] = 0x80 | ((cp & 0x3F) as u8);
        3
    } else if cp < 0x110000 {
        buf[0] = 0xF0 | ((cp >> 18) as u8);
        buf[1] = 0x80 | (((cp >> 12) & 0x3F) as u8);
        buf[2] = 0x80 | (((cp >> 6) & 0x3F) as u8);
        buf[3] = 0x80 | ((cp & 0x3F) as u8);
        4
    } else {
        0
    }
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
    fn test_unicode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["unicode", "65"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("U+0041"));
        assert!(stdout.contains("'A'"));
    }
}
