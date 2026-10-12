//! cut - remove sections from each line of files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cut.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// Sentinel used for the end of an open-ended range (`N-`).
const OPEN_END: usize = usize::MAX;

/// cut - remove sections from each line of files
///
/// # Synopsis
/// ```text
/// cut -b list [-n] [file...]
/// cut -c list [file...]
/// cut -f list [-d delim] [-s] [file...]
/// ```
///
/// # Description
/// Cut out selected bytes, characters, or fields from each line of files.
///
/// A `list` is a comma-separated set of 1-based, inclusive ranges. Each element
/// may be `N`, `N-`, `-M`, or `N-M`. Ranges may overlap and appear in any order;
/// selected bytes/characters/fields are always emitted in increasing position
/// order.
///
/// # Options
/// - `-b list`: Select only these bytes (treated identically to `-c` here)
/// - `-c list`: Select only these characters
/// - `-d delim`: Use DELIM instead of TAB for the field delimiter
/// - `-f list`: Select only these fields
/// - `-n`: Accepted for POSIX compatibility (no-op)
/// - `-s`: Suppress lines with no delimiter (fields mode only)
///
/// # Exit Status
/// - 0: Success
/// - 1: An input file could not be read
/// - 2: Usage error (missing list, conflicting modes, malformed list)
pub fn cut(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut delimiter = b'\t';
        let mut field_mode = false;
        let mut char_mode = false;
        let mut suppress = false;
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut list_given = false;
        let mut files: Vec<i32> = Vec::new();

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
                io::write_str(
                    1,
                    b"usage: cut -b|-c|-f list [-n] [-d delim] [-s] [file...]\n",
                );
                return 0;
            } else if arg == b"-" {
                // Explicit stdin operand.
                files.push(i);
            } else if arg.len() > 2 && arg.starts_with(b"--") {
                io::write_str(2, b"cut: unrecognized option '");
                io::write_all(2, arg);
                io::write_str(2, b"'\n");
                return 2;
            } else if arg.len() >= 2 && arg[0] == b'-' {
                match arg[1] {
                    b'd' => {
                        if arg.len() > 2 {
                            delimiter = arg[2];
                        } else if i + 1 < argc {
                            if let Some(d) = unsafe { get_arg(argv, i + 1) } {
                                if !d.is_empty() {
                                    delimiter = d[0];
                                }
                            }
                            i += 1;
                        }
                    }
                    b'f' | b'c' | b'b' => {
                        if arg[1] == b'f' {
                            field_mode = true;
                        } else {
                            char_mode = true;
                        }
                        let list: Option<&[u8]> = if arg.len() > 2 {
                            Some(&arg[2..])
                        } else if i + 1 < argc {
                            let l = unsafe { get_arg(argv, i + 1) };
                            i += 1;
                            l
                        } else {
                            None
                        };
                        match list {
                            Some(l) if parse_list(l, &mut ranges) => {
                                list_given = true;
                            }
                            _ => {
                                io::write_str(2, b"cut: invalid list of ranges\n");
                                return 2;
                            }
                        }
                    }
                    b's' => {
                        suppress = true;
                    }
                    b'n' => { /* no-op */ }
                    _ => {
                        io::write_str(2, b"cut: invalid option\n");
                        return 2;
                    }
                }
            } else {
                files.push(i);
            }
            i += 1;
        }

        // A list is required, and the two modes are mutually exclusive.
        if !list_given {
            io::write_str(
                2,
                b"cut: you must specify a list of bytes, characters, or fields\n",
            );
            return 2;
        }
        if field_mode && char_mode {
            io::write_str(2, b"cut: only one type of list may be specified\n");
            return 2;
        }

        let mut exit_code = 0;

        if files.is_empty() {
            let content = io::read_all(0);
            if !process_content(&content, field_mode, delimiter, &ranges, suppress) {
                io::write_str(2, b"cut: write error\n");
                return 1;
            }
        } else {
            for &idx in &files {
                let path = match unsafe { get_arg(argv, idx) } {
                    Some(p) => p,
                    None => continue,
                };
                if path == b"-" {
                    let content = io::read_all(0);
                    if !process_content(&content, field_mode, delimiter, &ranges, suppress) {
                        io::write_str(2, b"cut: write error\n");
                        return 1;
                    }
                    continue;
                }
                let fd = io::open(path, libc::O_RDONLY, 0);
                if fd < 0 {
                    io::write_str(2, b"cut: ");
                    sys::perror(path);
                    exit_code = 1;
                    continue;
                }
                let content = io::read_all(fd);
                io::close(fd);
                if !process_content(&content, field_mode, delimiter, &ranges, suppress) {
                    io::write_str(2, b"cut: write error\n");
                    return 1;
                }
            }
        }

        exit_code
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = (argc, argv);
        io::write_str(2, b"cut: requires alloc feature\n");
        1
    }
}

/// Parse a comma-separated list of ranges, appending `(lo, hi)` pairs to `out`.
///
/// Positions are 1-based and inclusive. `hi == OPEN_END` marks an open-ended
/// range (`N-`). Returns `false` on any malformed element.
#[cfg(feature = "alloc")]
fn parse_list(list: &[u8], out: &mut alloc::vec::Vec<(usize, usize)>) -> bool {
    if list.is_empty() {
        return false;
    }
    for part in list.split(|&c| c == b',') {
        if part.is_empty() {
            return false;
        }
        if let Some(dash) = part.iter().position(|&c| c == b'-') {
            let before = &part[..dash];
            let after = &part[dash + 1..];
            let lo = if before.is_empty() {
                1
            } else {
                match sys::parse_u64(before) {
                    Some(n) if n >= 1 => n as usize,
                    _ => return false,
                }
            };
            let hi = if after.is_empty() {
                OPEN_END
            } else {
                match sys::parse_u64(after) {
                    Some(n) if n >= 1 => n as usize,
                    _ => return false,
                }
            };
            if lo > hi {
                return false;
            }
            out.push((lo, hi));
        } else {
            match sys::parse_u64(part) {
                Some(n) if n >= 1 => out.push((n as usize, n as usize)),
                _ => return false,
            }
        }
    }
    true
}

/// Returns true if the 1-based `pos` falls within any range.
#[cfg(feature = "alloc")]
fn is_selected(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|&(lo, hi)| pos >= lo && pos <= hi)
}

/// Split `content` into lines (on `\n`) and cut each one. A final segment
/// lacking a trailing newline is still processed; a trailing newline does not
/// produce a spurious empty line.
#[cfg(feature = "alloc")]
fn process_content(
    content: &[u8],
    field_mode: bool,
    delimiter: u8,
    ranges: &[(usize, usize)],
    suppress: bool,
) -> bool {
    let len = content.len();
    let mut start = 0;
    let mut i = 0;
    while i < len {
        if content[i] == b'\n' {
            if !cut_line(&content[start..i], field_mode, delimiter, ranges, suppress) {
                return false;
            }
            start = i + 1;
        }
        i += 1;
    }
    if start < len {
        return cut_line(
            &content[start..len],
            field_mode,
            delimiter,
            ranges,
            suppress,
        );
    }
    true
}

/// Cut a single line (without its trailing newline) and write the result,
/// including a trailing newline, unless the line is suppressed.
#[cfg(feature = "alloc")]
fn cut_line(
    line: &[u8],
    field_mode: bool,
    delimiter: u8,
    ranges: &[(usize, usize)],
    suppress: bool,
) -> bool {
    if field_mode {
        // If the line contains no delimiter it is passed through whole,
        // unless -s was given, in which case it is dropped entirely.
        if !line.contains(&delimiter) {
            if suppress {
                return true;
            }
            return io::write_all(1, line) >= 0 && io::write_str(1, b"\n") >= 0;
        }

        let len = line.len();
        let mut field_num = 1;
        let mut start = 0;
        let mut first_out = true;
        loop {
            let mut j = start;
            while j < len && line[j] != delimiter {
                j += 1;
            }
            if is_selected(field_num, ranges) {
                if !first_out && io::write_all(1, &[delimiter]) < 0 {
                    return false;
                }
                if io::write_all(1, &line[start..j]) < 0 {
                    return false;
                }
                first_out = false;
            }
            if j >= len {
                break;
            }
            start = j + 1;
            field_num += 1;
        }
        io::write_str(1, b"\n") >= 0
    } else {
        // Byte/character mode: emit selected positions in contiguous runs.
        let len = line.len();
        let mut pos = 1;
        while pos <= len {
            if is_selected(pos, ranges) {
                let run_start = pos;
                while pos <= len && is_selected(pos, ranges) {
                    pos += 1;
                }
                if io::write_all(1, &line[run_start - 1..pos - 1]) < 0 {
                    return false;
                }
            } else {
                pos += 1;
            }
        }
        io::write_str(1, b"\n") >= 0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_cut_field() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["cut", "-f", "2", "-d", ":"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a:b:c\nx:y:z\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "b\ny\n");
    }

    #[test]
    fn test_cut_chars() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["cut", "-c", "1-3"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello\nworld\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "hel\nwor\n");
    }

    #[test]
    fn test_cut_first_field() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["cut", "-f1", "-d,"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"one,two,three\na,b,c\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "one\na\n");
    }

    #[test]
    fn test_cut_tab_delimiter() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["cut", "-f", "2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\tb\tc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "b\n");
    }
}
