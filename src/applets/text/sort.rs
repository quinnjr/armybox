//! sort - sort lines of text files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sort.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use core::cmp::Ordering;

#[inline]
fn is_blank(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

#[inline]
fn to_lower(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

#[inline]
fn imin(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

/// A `-k` key definition: start/end fields (1-based) and optional character
/// offsets within those fields (1-based; 0 means unspecified).
struct KeyDef {
    start_field: usize,
    start_char: usize,
    end_field: usize,
    end_char: usize,
}

fn parse_pos(s: &[u8]) -> (usize, usize) {
    let mut i = 0;
    let mut f: usize = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        f = f.saturating_mul(10).saturating_add((s[i] - b'0') as usize);
        i += 1;
    }
    let mut c: usize = 0;
    if i < s.len() && s[i] == b'.' {
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            c = c.saturating_mul(10).saturating_add((s[i] - b'0') as usize);
            i += 1;
        }
    }
    (f, c)
}

fn parse_keydef(s: &[u8]) -> KeyDef {
    let mut comma = s.len();
    for (idx, &c) in s.iter().enumerate() {
        if c == b',' {
            comma = idx;
            break;
        }
    }
    let (sf, sc) = parse_pos(&s[..comma]);
    let (ef, ec) = if comma < s.len() {
        parse_pos(&s[comma + 1..])
    } else {
        (0, 0)
    };
    KeyDef {
        start_field: if sf == 0 { 1 } else { sf },
        start_char: sc,
        end_field: ef,
        end_char: ec,
    }
}

/// Byte index of the start of the field reached after skipping `skip` fields.
fn field_start(line: &[u8], skip: usize, sep: Option<u8>) -> usize {
    let mut pos = 0;
    match sep {
        Some(s) => {
            let mut k = 0;
            while k < skip && pos < line.len() {
                while pos < line.len() && line[pos] != s {
                    pos += 1;
                }
                if pos < line.len() {
                    pos += 1;
                } // consume separator
                k += 1;
            }
        }
        None => {
            for _ in 0..skip {
                while pos < line.len() && is_blank(line[pos]) {
                    pos += 1;
                }
                while pos < line.len() && !is_blank(line[pos]) {
                    pos += 1;
                }
            }
        }
    }
    pos
}

/// Byte index (exclusive) of the end of field number `field` (1-based).
fn field_end(line: &[u8], field: usize, sep: Option<u8>) -> usize {
    match sep {
        Some(s) => {
            let start = field_start(line, field - 1, Some(s));
            let mut pos = start;
            while pos < line.len() && line[pos] != s {
                pos += 1;
            }
            pos
        }
        None => field_start(line, field, None),
    }
}

/// Parse a POSIX numeric prefix: leading blanks, optional sign, digits and an
/// optional decimal fraction. Non-numeric input yields 0.
fn parse_num(s: &[u8]) -> f64 {
    let mut i = 0;
    while i < s.len() && is_blank(s[i]) {
        i += 1;
    }
    let mut sign = 1.0f64;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        if s[i] == b'-' {
            sign = -1.0;
        }
        i += 1;
    }
    let mut val = 0.0f64;
    let mut any = false;
    while i < s.len() && s[i].is_ascii_digit() {
        val = val * 10.0 + (s[i] - b'0') as f64;
        i += 1;
        any = true;
    }
    if i < s.len() && s[i] == b'.' {
        i += 1;
        let mut frac = 0.1f64;
        while i < s.len() && s[i].is_ascii_digit() {
            val += (s[i] - b'0') as f64 * frac;
            frac *= 0.1;
            i += 1;
            any = true;
        }
    }
    if any { sign * val } else { 0.0 }
}

fn num_cmp(a: &[u8], b: &[u8]) -> Ordering {
    let x = parse_num(a);
    let y = parse_num(b);
    x.partial_cmp(&y).unwrap_or(Ordering::Equal)
}

fn fold_cmp(a: &[u8], b: &[u8]) -> Ordering {
    let n = imin(a.len(), b.len());
    let mut i = 0;
    while i < n {
        let ca = to_lower(a[i]);
        let cb = to_lower(b[i]);
        if ca != cb {
            return ca.cmp(&cb);
        }
        i += 1;
    }
    a.len().cmp(&b.len())
}

fn print_usage() {
    io::write_str(
        1,
        b"Usage: sort [-cmnrufb] [-o output] [-t char] [-k keydef] [file...]\n",
    );
    io::write_str(1, b"Sort, merge, or sequence check text files.\n\n");
    io::write_str(
        1,
        b"  -n            compare according to string numerical value\n",
    );
    io::write_str(1, b"  -r            reverse the result of comparisons\n");
    io::write_str(
        1,
        b"  -u            output only the first of an equal run\n",
    );
    io::write_str(1, b"  -b            ignore leading blanks\n");
    io::write_str(1, b"  -f            fold lowercase to uppercase\n");
    io::write_str(
        1,
        b"  -c            check whether input is already sorted\n",
    );
    io::write_str(1, b"  -k KEYDEF     sort key definition\n");
    io::write_str(1, b"  -t CHAR       field separator character\n");
    io::write_str(1, b"  -o FILE       write output to FILE\n");
    io::write_str(1, b"  --help        display this help and exit\n");
}

/// sort - sort lines of text files
///
/// # Synopsis
/// ```text
/// sort [-cmnrufb] [-o output] [-t char] [-k keydef] [file...]
/// ```
///
/// # Description
/// Sort, merge, or sequence check text files.
///
/// # Options
/// - `-n`: Compare according to string numerical value
/// - `-r`: Reverse the result of comparisons
/// - `-u`: Output only the first of an equal run
/// - `-b`: Ignore leading blanks
/// - `-f`: Fold lowercase to uppercase (case-insensitive)
/// - `-c`: Check whether the input is already sorted
/// - `-k`: Sort key definition
/// - `-t`: Field separator character
/// - `-o`: Write output to the given file
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn sort(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut reverse = false;
        let mut numeric = false;
        let mut unique = false;
        let mut ignore_blanks = false;
        let mut fold = false;
        let mut check = false;
        let mut sep: Option<u8> = None;
        let mut keydef: Option<KeyDef> = None;
        let mut output: Option<&[u8]> = None;
        let mut files: Vec<&[u8]> = Vec::new();

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
                io::write_str(2, b"sort: unrecognized option '");
                io::write_all(2, arg);
                io::write_str(2, b"'\n");
                return 2;
            }
            if arg.len() >= 2 && arg[0] == b'-' {
                let mut j = 1;
                while j < arg.len() {
                    match arg[j] {
                        b'r' => reverse = true,
                        b'n' => numeric = true,
                        b'u' => unique = true,
                        b'b' => ignore_blanks = true,
                        b'f' => fold = true,
                        b'c' | b'C' => check = true,
                        b'm' | b's' => {} // merge / stable: sort is already stable
                        b'o' => {
                            let rest = &arg[j + 1..];
                            if !rest.is_empty() {
                                output = Some(rest);
                            } else {
                                i += 1;
                                output = unsafe { get_arg(argv, i) };
                            }
                            break;
                        }
                        b'k' => {
                            let rest = &arg[j + 1..];
                            let kd = if !rest.is_empty() {
                                rest
                            } else {
                                i += 1;
                                unsafe { get_arg(argv, i) }.unwrap_or(b"")
                            };
                            keydef = Some(parse_keydef(kd));
                            break;
                        }
                        b't' => {
                            let rest = &arg[j + 1..];
                            let val = if !rest.is_empty() {
                                rest
                            } else {
                                i += 1;
                                unsafe { get_arg(argv, i) }.unwrap_or(b"")
                            };
                            if !val.is_empty() {
                                sep = Some(val[0]);
                            }
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
            } else {
                files.push(arg);
            }
            i += 1;
        }

        // Read each input source into its own buffer, splitting into lines so
        // that files without trailing newlines don't merge across boundaries.
        let mut buffers: Vec<Vec<u8>> = Vec::new();
        if files.is_empty() {
            buffers.push(io::read_all(0));
        } else {
            for &path in &files {
                if path == b"-" {
                    buffers.push(io::read_all(0));
                } else {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        io::write_str(2, b"sort: ");
                        sys::perror(path);
                        return 1;
                    }
                    buffers.push(io::read_all(fd));
                    io::close(fd);
                }
            }
        }

        let mut lines: Vec<&[u8]> = Vec::new();
        for buf in &buffers {
            let mut parts: Vec<&[u8]> = buf.split(|&c| c == b'\n').collect();
            // Drop only the single trailing empty element from a trailing newline.
            if let Some(last) = parts.last() {
                if last.is_empty() {
                    parts.pop();
                }
            }
            lines.extend_from_slice(&parts);
        }

        // Extract the comparison key range [start, end) for a line.
        let keyrange = |line: &[u8]| -> (usize, usize) {
            match &keydef {
                None => {
                    let mut s = 0;
                    if ignore_blanks {
                        while s < line.len() && is_blank(line[s]) {
                            s += 1;
                        }
                    }
                    (s, line.len())
                }
                Some(k) => {
                    let mut s = field_start(line, k.start_field - 1, sep);
                    if ignore_blanks {
                        while s < line.len() && is_blank(line[s]) {
                            s += 1;
                        }
                    }
                    if k.start_char > 1 {
                        s = imin(line.len(), s + (k.start_char - 1));
                    }
                    let e = if k.end_field == 0 {
                        line.len()
                    } else if k.end_char == 0 {
                        field_end(line, k.end_field, sep)
                    } else {
                        let mut base = field_start(line, k.end_field - 1, sep);
                        if ignore_blanks {
                            while base < line.len() && is_blank(line[base]) {
                                base += 1;
                            }
                        }
                        imin(line.len(), base + k.end_char)
                    };
                    (s, if e < s { s } else { e })
                }
            }
        };

        let cmp = |a: &[u8], b: &[u8]| -> Ordering {
            let (as_, ae) = keyrange(a);
            let (bs, be) = keyrange(b);
            let ka = &a[as_..ae];
            let kb = &b[bs..be];
            if numeric {
                num_cmp(ka, kb)
            } else if fold {
                fold_cmp(ka, kb)
            } else {
                ka.cmp(kb)
            }
        };

        // Check-sorted mode: report disorder without producing output.
        if check {
            for w in 1..lines.len() {
                let ord = cmp(lines[w - 1], lines[w]);
                let bad = if reverse {
                    ord == Ordering::Less
                } else {
                    ord == Ordering::Greater
                };
                if bad {
                    io::write_str(2, b"sort: input not sorted\n");
                    return 1;
                }
            }
            return 0;
        }

        // Apply reverse in the comparator (a stable sort) so lines with equal
        // keys keep their original relative order, matching GNU sort -r.
        if reverse {
            lines.sort_by(|a, b| cmp(*b, *a));
        } else {
            lines.sort_by(|a, b| cmp(*a, *b));
        }

        if unique {
            lines.dedup_by(|a, b| cmp(*a, *b) == Ordering::Equal);
        }

        // Determine output file descriptor.
        let out_fd = if let Some(path) = output {
            let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
            if fd < 0 {
                io::write_str(2, b"sort: ");
                sys::perror(path);
                return 1;
            }
            fd
        } else {
            1
        };

        let mut exit_code = 0;
        for line in &lines {
            if io::write_all(out_fd, line) < 0 || io::write_str(out_fd, b"\n") < 0 {
                sys::perror(b"sort: write error");
                exit_code = 1;
                break;
            }
        }

        if out_fd != 1 {
            io::close(out_fd);
        }

        return exit_code;
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"sort: requires alloc feature\n");
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
            "armybox_sort_test_{}_{}",
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
    fn test_sort_alphabetic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "cherry\napple\nbanana\n").unwrap();

        let output = Command::new(&armybox)
            .args(["sort", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["apple", "banana", "cherry"]);
        cleanup(&dir);
    }

    #[test]
    fn test_sort_numeric() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["sort", "-n"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"10\n2\n1\n20\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["1", "2", "10", "20"]);
    }

    #[test]
    fn test_sort_reverse() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["sort", "-r"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\nb\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_sort_unique() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["sort", "-u"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\nb\na\nb\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
    }
}
