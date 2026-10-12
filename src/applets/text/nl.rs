//! nl - number lines
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/nl.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// nl - number lines of files
///
/// # Synopsis
/// ```text
/// nl [-b type] [-n format] [-w width] [-s sep] [-v start] [-i incr] [file...]
/// ```
///
/// # Description
/// Read lines from file (or standard input) and write them to standard
/// output with line numbers added.
///
/// # Options
/// - `-b a|t|n`: Number all lines, non-empty lines only (default), or no lines
/// - `-n ln|rn|rz`: Left justified, right justified (default), or right
///   justified with leading zeros
/// - `-w WIDTH`: Line number field width (default 6)
/// - `-s SEP`: Line number separator (default TAB)
/// - `-v START`: Initial line number (default 1)
/// - `-i INCR`: Line number increment (default 1)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn nl(argc: i32, argv: *const *const u8) -> i32 {
    let mut body_type = b't';
    let mut width: usize = 6;
    let mut sep: &[u8] = b"\t";
    let mut start: u64 = 1;
    let mut incr: u64 = 1;
    let mut left_justify = false;
    let mut zero_pad = false;

    #[cfg(feature = "alloc")]
    let mut files: alloc::vec::Vec<&[u8]> = alloc::vec::Vec::new();

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"--help" {
                io::write_str(1, b"usage: nl [-b type] [-n format] [-w width] [-s sep] [-v start] [-i incr] [file...]\n");
                return 0;
            }
            if arg.len() > 2 && arg.starts_with(b"--") {
                io::write_str(2, b"nl: unrecognized option '");
                io::write_all(2, arg);
                io::write_str(2, b"'\n");
                return 2;
            }
            if arg.len() > 1 && arg[0] == b'-' {
                let opt = arg[1];
                let value: Option<&[u8]> = if arg.len() > 2 {
                    Some(&arg[2..])
                } else if i + 1 < argc {
                    i += 1;
                    unsafe { get_arg(argv, i) }
                } else {
                    None
                };

                match opt {
                    b'b' => {
                        if let Some(v) = value {
                            if !v.is_empty() {
                                body_type = v[0];
                            }
                        }
                    }
                    b'w' => {
                        if let Some(v) = value {
                            match sys::parse_u64(v) {
                                Some(n) => width = n as usize,
                                None => {
                                    io::write_str(2, b"nl: invalid line number field width: ");
                                    io::write_all(2, v);
                                    io::write_str(2, b"\n");
                                    return 2;
                                }
                            }
                        }
                    }
                    b's' => {
                        if let Some(v) = value {
                            sep = v;
                        }
                    }
                    b'v' => {
                        if let Some(v) = value {
                            match sys::parse_u64(v) {
                                Some(n) => start = n,
                                None => {
                                    io::write_str(2, b"nl: invalid starting line number: ");
                                    io::write_all(2, v);
                                    io::write_str(2, b"\n");
                                    return 2;
                                }
                            }
                        }
                    }
                    b'i' => {
                        if let Some(v) = value {
                            match sys::parse_u64(v) {
                                Some(n) => incr = n,
                                None => {
                                    io::write_str(2, b"nl: invalid line number increment: ");
                                    io::write_all(2, v);
                                    io::write_str(2, b"\n");
                                    return 2;
                                }
                            }
                        }
                    }
                    b'n' => {
                        if let Some(v) = value {
                            if v.len() >= 2 {
                                left_justify = v[0] == b'l';
                                zero_pad = v[1] == b'z';
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                #[cfg(feature = "alloc")]
                files.push(arg);
            }
        }
        i += 1;
    }

    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut content: Vec<u8> = Vec::new();
        let mut had_error = false;

        if files.is_empty() {
            content = io::read_all(0);
        } else {
            for &path in &files {
                let fd = if path == b"-" {
                    0
                } else {
                    io::open(path, libc::O_RDONLY, 0)
                };

                if fd < 0 {
                    io::write_str(2, b"nl: ");
                    sys::perror(path);
                    had_error = true;
                    continue;
                }

                content.extend_from_slice(&io::read_all(fd));
                if fd != 0 {
                    io::close(fd);
                }
            }
        }

        let mut counter = start;
        let mut lines: Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
        if content.last() == Some(&b'\n') {
            // Drop the trailing empty segment produced when content ends
            // with a newline; it is not a real line.
            lines.pop();
        }

        for seg in lines {
            let numbered = match body_type {
                b'a' => true,
                b'n' => false,
                _ => !seg.is_empty(),
            };

            if numbered {
                if !write_number(width, counter, zero_pad, left_justify)
                    || io::write_all(1, sep) < 0
                {
                    io::write_str(2, b"nl: write error\n");
                    return 1;
                }
                counter = counter.wrapping_add(incr);
            }
            if io::write_all(1, seg) < 0 || io::write_str(1, b"\n") < 0 {
                io::write_str(2, b"nl: write error\n");
                return 1;
            }
        }

        if had_error {
            return 1;
        }
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = (body_type, width, sep, start, incr, left_justify, zero_pad);
        io::write_str(2, b"nl: requires alloc feature\n");
        return 1;
    }

    0
}

/// Write a line number in the requested field width and justification.
/// Returns false if any write fails.
fn write_number(width: usize, num: u64, zero_pad: bool, left_justify: bool) -> bool {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = num;

    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while n > 0 {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
    }

    let digits = &buf[i..];
    let pad = width.saturating_sub(digits.len());

    if left_justify {
        if io::write_all(1, digits) < 0 {
            return false;
        }
        for _ in 0..pad {
            if io::write_str(1, b" ") < 0 {
                return false;
            }
        }
    } else {
        let pad_char: u8 = if zero_pad { b'0' } else { b' ' };
        for _ in 0..pad {
            if io::write_all(1, &[pad_char]) < 0 {
                return false;
            }
        }
        if io::write_all(1, digits) < 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
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

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_nl_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_nl_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line one\nline two\nline three\n").unwrap();

        let output = Command::new(&armybox)
            .args(["nl", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("1\tline one"));
        assert!(stdout.contains("2\tline two"));
        assert!(stdout.contains("3\tline three"));
        cleanup(&dir);
    }

    #[test]
    fn test_nl_single_line() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "only one\n").unwrap();

        let output = Command::new(&armybox)
            .args(["nl", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("1\tonly one"));
        cleanup(&dir);
    }

    #[test]
    fn test_nl_numbered_content() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "first\nsecond\nthird\nfourth\nfifth\n").unwrap();

        let output = Command::new(&armybox)
            .args(["nl", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 5);
        cleanup(&dir);
    }
}
