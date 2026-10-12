//! uniq - report or omit repeated lines
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/uniq.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

#[inline]
fn is_blank(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

#[inline]
fn to_lower(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

/// Compute the comparison start offset for a line after skipping `fields`
/// leading fields (blank-separated) and then `chars` characters.
fn skip_offset(line: &[u8], fields: usize, chars: usize) -> usize {
    let mut pos = 0;
    for _ in 0..fields {
        while pos < line.len() && is_blank(line[pos]) {
            pos += 1;
        }
        while pos < line.len() && !is_blank(line[pos]) {
            pos += 1;
        }
    }
    pos += chars;
    if pos > line.len() { line.len() } else { pos }
}

fn keys_equal(a: &[u8], b: &[u8], fold: bool) -> bool {
    if !fold {
        return a == b;
    }
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if to_lower(a[i]) != to_lower(b[i]) {
            return false;
        }
        i += 1;
    }
    true
}

/// Parse a numeric option value that may be attached (`-f1`) or supplied as the
/// next argument (`-f 1`). Returns the parsed count and the number of extra
/// argv slots consumed (0 or 1).
fn opt_num(rest: &[u8], argv: *const *const u8, next: i32) -> (usize, i32) {
    let digits = if !rest.is_empty() {
        rest
    } else {
        match unsafe { get_arg(argv, next) } {
            Some(a) => return (parse_usize(a), 1),
            None => b"",
        }
    };
    (parse_usize(digits), 0)
}

fn parse_usize(s: &[u8]) -> usize {
    let mut n = 0usize;
    for &c in s {
        if !c.is_ascii_digit() {
            break;
        }
        n = n.saturating_mul(10).saturating_add((c - b'0') as usize);
    }
    n
}

fn print_usage() {
    io::write_str(
        1,
        b"Usage: uniq [-cdui] [-f fields] [-s chars] [input_file [output_file]]\n",
    );
    io::write_str(
        1,
        b"Filter adjacent matching lines from INPUT, writing to OUTPUT.\n\n",
    );
    io::write_str(
        1,
        b"  -c            prefix lines with number of occurrences\n",
    );
    io::write_str(1, b"  -d            only print duplicate lines\n");
    io::write_str(1, b"  -u            only print unique lines\n");
    io::write_str(1, b"  -i            ignore case when comparing\n");
    io::write_str(1, b"  -f N          skip the first N fields\n");
    io::write_str(1, b"  -s N          skip the first N characters\n");
    io::write_str(1, b"  --help        display this help and exit\n");
}

/// uniq - report or omit repeated lines
///
/// # Synopsis
/// ```text
/// uniq [-cdui] [-f fields] [-s chars] [input_file [output_file]]
/// ```
///
/// # Description
/// Filter adjacent matching lines from INPUT, writing to OUTPUT.
///
/// # Options
/// - `-c`: Prefix lines with number of occurrences
/// - `-d`: Only print duplicate lines
/// - `-u`: Only print unique lines
/// - `-i`: Ignore case when comparing
/// - `-f N`: Skip the first N fields
/// - `-s N`: Skip the first N characters (after any skipped fields)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn uniq(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut count = false;
        let mut repeated = false;
        let mut unique_only = false;
        let mut fold = false;
        let mut skip_fields = 0usize;
        let mut skip_chars = 0usize;
        let mut operands: Vec<&[u8]> = Vec::new();

        let mut i = 1;
        while i < argc {
            let arg = match unsafe { get_arg(argv, i) } {
                Some(a) => a,
                None => {
                    i += 1;
                    continue;
                }
            };
            if arg == b"--help" {
                print_usage();
                return 0;
            }
            if arg.len() > 2 && arg[0] == b'-' && arg[1] == b'-' {
                io::write_str(2, b"uniq: unrecognized option '");
                io::write_all(2, arg);
                io::write_str(2, b"'\n");
                return 2;
            }
            if arg.len() >= 2 && arg[0] == b'-' {
                let mut j = 1;
                while j < arg.len() {
                    match arg[j] {
                        b'c' => count = true,
                        b'd' => repeated = true,
                        b'u' => unique_only = true,
                        b'i' => fold = true,
                        b'f' => {
                            let (n, used) = opt_num(&arg[j + 1..], argv, i + 1);
                            skip_fields = n;
                            i += used;
                            break;
                        }
                        b's' => {
                            let (n, used) = opt_num(&arg[j + 1..], argv, i + 1);
                            skip_chars = n;
                            i += used;
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
            } else {
                operands.push(arg);
            }
            i += 1;
        }

        // First operand is the input file, second (if any) is the output file.
        let content = if !operands.is_empty() && operands[0] != b"-" {
            let fd = io::open(operands[0], libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"uniq: ");
                sys::perror(operands[0]);
                return 1;
            }
            let c = io::read_all(fd);
            io::close(fd);
            c
        } else {
            io::read_all(0)
        };

        let out_fd = if operands.len() >= 2 {
            let fd = io::open(
                operands[1],
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o644,
            );
            if fd < 0 {
                io::write_str(2, b"uniq: ");
                sys::perror(operands[1]);
                return 1;
            }
            fd
        } else {
            1
        };

        let mut lines: Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
        // Drop only the single trailing empty element from a trailing newline.
        if let Some(last) = lines.last() {
            if last.is_empty() {
                lines.pop();
            }
        }

        let mut exit_code = 0;
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let key_start = skip_offset(line, skip_fields, skip_chars);
            let key = &line[key_start..];
            let mut cnt = 1;

            while i + cnt < lines.len() {
                let other = lines[i + cnt];
                let other_key = &other[skip_offset(other, skip_fields, skip_chars)..];
                if !keys_equal(key, other_key, fold) {
                    break;
                }
                cnt += 1;
            }

            let should_print = if repeated {
                cnt > 1
            } else if unique_only {
                cnt == 1
            } else {
                true
            };

            if should_print {
                let mut ok = true;
                if count {
                    ok = io::write_num(out_fd, cnt as u64) >= 0 && io::write_str(out_fd, b" ") >= 0;
                }
                ok = ok && io::write_all(out_fd, line) >= 0 && io::write_str(out_fd, b"\n") >= 0;
                if !ok {
                    sys::perror(b"uniq: write error");
                    exit_code = 1;
                    break;
                }
            }

            i += cnt;
        }

        if out_fd != 1 {
            io::close(out_fd);
        }

        return exit_code;
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"uniq: requires alloc feature\n");
        return 1;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

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
            "armybox_uniq_test_{}_{}",
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
    fn test_uniq_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["uniq"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\na\nb\nb\nb\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_uniq_count() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["uniq", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\na\na\nb\nc\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("3 a"));
        assert!(stdout.contains("1 b"));
        assert!(stdout.contains("2 c"));
    }

    #[test]
    fn test_uniq_duplicates_only() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["uniq", "-d"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\na\nb\nc\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["a", "c"]);
    }

    #[test]
    fn test_uniq_unique_only() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["uniq", "-u"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\na\nb\nc\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["b"]);
    }

    #[test]
    fn test_uniq_from_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "x\nx\ny\ny\ny\nz\n").unwrap();

        let output = Command::new(&armybox)
            .args(["uniq", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["x", "y", "z"]);
        cleanup(&dir);
    }
}
