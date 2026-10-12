//! paste - merge lines of files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/paste.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// paste - merge lines of files
///
/// # Synopsis
/// ```text
/// paste [-s] [-d list] file...
/// ```
///
/// # Description
/// Merge corresponding or subsequent lines of files.
///
/// # Options
/// - `-d list`: Replace default delimiter TAB with a cycling list of
///   characters. Recognizes the escapes `\n`, `\t`, `\\`, and `\0`
///   (an empty delimiter, not a NUL byte).
/// - `-s`: Paste one file at a time instead of in parallel
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn paste(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        // 0u8 is used internally as a marker for an empty ("\0") delimiter
        // slot; it is never written to output.
        let mut delims: Vec<u8> = alloc::vec![b'\t'];
        let mut have_custom_delim = false;
        let mut serial = false;
        let mut files: Vec<&[u8]> = Vec::new();

        let mut i = 1;
        while i < argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                if arg == b"--help" {
                    io::write_str(1, b"usage: paste [-s] [-d list] file...\n");
                    return 0;
                }
                if arg.len() > 2 && arg.starts_with(b"--") {
                    io::write_str(2, b"paste: unrecognized option '");
                    io::write_all(2, arg);
                    io::write_str(2, b"'\n");
                    return 2;
                }
                if arg == b"-" {
                    files.push(b"-");
                } else if arg.starts_with(b"-") && arg.len() > 1 {
                    if arg == b"-s" {
                        serial = true;
                    } else if arg == b"-d" || arg.starts_with(b"-d") {
                        let value: Option<&[u8]> = if arg.len() > 2 {
                            Some(&arg[2..])
                        } else if i + 1 < argc {
                            i += 1;
                            unsafe { get_arg(argv, i) }
                        } else {
                            None
                        };

                        if let Some(v) = value {
                            delims = parse_delims(v);
                            have_custom_delim = true;
                        }
                    }
                } else {
                    files.push(arg);
                }
            }
            i += 1;
        }

        if have_custom_delim && delims.is_empty() {
            // -d '' means "no delimiter", represented by our empty marker.
            delims.push(0u8);
        }

        if files.is_empty() {
            files.push(b"-");
        }

        let mut had_error = false;

        if serial {
            for &file in &files {
                let fd = if file == b"-" {
                    0
                } else {
                    io::open(file, libc::O_RDONLY, 0)
                };

                if fd < 0 {
                    io::write_str(2, b"paste: ");
                    sys::perror(file);
                    had_error = true;
                    continue;
                }

                let content = io::read_all(fd);
                if fd != 0 {
                    io::close(fd);
                }

                let lines = split_lines(&content);
                for (idx, line) in lines.iter().enumerate() {
                    if idx > 0 && !write_delim(&delims, idx - 1) {
                        io::write_str(2, b"paste: write error\n");
                        return 1;
                    }
                    if io::write_all(1, line) < 0 {
                        io::write_str(2, b"paste: write error\n");
                        return 1;
                    }
                }
                if io::write_str(1, b"\n") < 0 {
                    io::write_str(2, b"paste: write error\n");
                    return 1;
                }
            }
        } else {
            let mut contents: Vec<Vec<u8>> = Vec::new();

            for &file in &files {
                let fd = if file == b"-" {
                    0
                } else {
                    io::open(file, libc::O_RDONLY, 0)
                };

                if fd < 0 {
                    io::write_str(2, b"paste: ");
                    sys::perror(file);
                    had_error = true;
                    contents.push(Vec::new());
                    continue;
                }

                let content = io::read_all(fd);
                if fd != 0 {
                    io::close(fd);
                }
                contents.push(content);
            }

            let file_lines: Vec<Vec<&[u8]>> = contents.iter().map(|c| split_lines(c)).collect();
            let max_lines = file_lines.iter().map(|l| l.len()).max().unwrap_or(0);

            for line_idx in 0..max_lines {
                for (file_idx, lines) in file_lines.iter().enumerate() {
                    if file_idx > 0 && !write_delim(&delims, file_idx - 1) {
                        io::write_str(2, b"paste: write error\n");
                        return 1;
                    }
                    if line_idx < lines.len() && io::write_all(1, &lines[line_idx]) < 0 {
                        io::write_str(2, b"paste: write error\n");
                        return 1;
                    }
                }
                if io::write_str(1, b"\n") < 0 {
                    io::write_str(2, b"paste: write error\n");
                    return 1;
                }
            }
        }

        if had_error {
            return 1;
        }
        return 0;
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = (argc, argv);
        io::write_str(2, b"paste: requires alloc feature\n");
        1
    }
}

/// Write the cycling delimiter for gap index `gap` (0-based gap between
/// consecutive fields). A marker byte of 0 means "no delimiter".
#[cfg(feature = "alloc")]
fn write_delim(delims: &[u8], gap: usize) -> bool {
    if delims.is_empty() {
        return true;
    }
    let d = delims[gap % delims.len()];
    if d != 0 {
        return io::write_all(1, &[d]) >= 0;
    }
    true
}

/// Parse a `-d` delimiter list, expanding `\n`, `\t`, `\\`, and `\0`
/// escapes. `\0` is represented internally as a literal 0 byte, which
/// `write_delim` treats as "write nothing".
#[cfg(feature = "alloc")]
fn parse_delims(v: &[u8]) -> alloc::vec::Vec<u8> {
    use alloc::vec::Vec;

    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        if v[i] == b'\\' && i + 1 < v.len() {
            match v[i + 1] {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'\\' => out.push(b'\\'),
                b'0' => out.push(0u8),
                other => out.push(other),
            }
            i += 2;
        } else {
            out.push(v[i]);
            i += 1;
        }
    }
    out
}

/// Split content into lines, dropping the trailing empty segment produced
/// when the content ends with a newline (so a trailing newline does not
/// produce a spurious blank final line).
#[cfg(feature = "alloc")]
fn split_lines(content: &[u8]) -> alloc::vec::Vec<&[u8]> {
    let mut lines: alloc::vec::Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
    if content.last() == Some(&b'\n') {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_paste_test_{}_{}",
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
    fn test_paste_two_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "a\nb\nc\n").unwrap();
        fs::write(&file2, "1\n2\n3\n").unwrap();

        let output = Command::new(&armybox)
            .args(["paste", file1.to_str().unwrap(), file2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("a\t1"));
        assert!(stdout.contains("b\t2"));
        assert!(stdout.contains("c\t3"));
        cleanup(&dir);
    }

    #[test]
    fn test_paste_serial() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "a\nb\nc\n").unwrap();

        let output = Command::new(&armybox)
            .args(["paste", "-s", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("a\tb\tc"));
        cleanup(&dir);
    }

    #[test]
    fn test_paste_custom_delimiter() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "a\nb\n").unwrap();
        fs::write(&file2, "1\n2\n").unwrap();

        let output = Command::new(&armybox)
            .args([
                "paste",
                "-d",
                ",",
                file1.to_str().unwrap(),
                file2.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("a,1"));
        assert!(stdout.contains("b,2"));
        cleanup(&dir);
    }
}
