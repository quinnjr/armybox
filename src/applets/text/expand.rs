//! expand - convert tabs to spaces
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/expand.html

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// Tab stop configuration, as controlled by `-t`.
enum TabStops {
    /// A single repeating interval (e.g. every 8th column).
    Uniform(usize),
    /// An explicit, increasing list of tab stop columns. Beyond the last
    /// listed stop, tabs advance one column at a time (POSIX behavior).
    List(Vec<usize>),
}

/// Compute the column of the next tab stop after `col`.
fn next_tabstop(col: usize, stops: &TabStops) -> usize {
    match stops {
        TabStops::Uniform(n) => {
            if *n == 0 {
                col + 1
            } else {
                col + (n - (col % n))
            }
        }
        TabStops::List(list) => {
            for &stop in list {
                if stop > col {
                    return stop;
                }
            }
            col + 1
        }
    }
}

/// Parse a `-t` TABLIST argument: either a single positive number (a
/// uniform interval) or a comma-separated increasing list of columns.
fn parse_tablist(s: &[u8]) -> Option<TabStops> {
    if s.is_empty() {
        return None;
    }

    let parts: Vec<&[u8]> = s.split(|&c| c == b',').collect();
    if parts.len() == 1 {
        let n = sys::parse_u64(parts[0])?;
        if n == 0 {
            return None;
        }
        return Some(TabStops::Uniform(n as usize));
    }

    let mut list = Vec::new();
    for p in parts {
        let n = sys::parse_u64(p)?;
        list.push(n as usize);
    }
    Some(TabStops::List(list))
}

/// Expand tabs in a single input stream to standard output.
fn process(fd: i32, stops: &TabStops, leading_only: bool) {
    let mut buf = [0u8; 4096];
    let mut col: usize = 0;
    let mut at_line_start = true;

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        for &c in &buf[..n as usize] {
            match c {
                b'\t' => {
                    if leading_only && !at_line_start {
                        io::write_all(1, &[c]);
                    } else {
                        let next = next_tabstop(col, stops);
                        let spaces = next - col;
                        for _ in 0..spaces {
                            io::write_str(1, b" ");
                        }
                        col = next;
                    }
                }
                b'\n' => {
                    io::write_str(1, b"\n");
                    col = 0;
                    at_line_start = true;
                }
                0x08 => {
                    // Backspace: move the tracked column back, pass through as-is.
                    if col > 0 {
                        col -= 1;
                    }
                    io::write_all(1, &[c]);
                }
                _ => {
                    io::write_all(1, &[c]);
                    col += 1;
                    if c != b' ' {
                        at_line_start = false;
                    }
                }
            }
        }
    }
}

/// expand - convert tabs to spaces
///
/// # Synopsis
/// ```text
/// expand [-i] [-t tablist] [file...]
/// ```
///
/// # Description
/// Convert tabs in each FILE (or standard input) to spaces, writing to
/// standard output. Tab stops default to every 8th column.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn expand(argc: i32, argv: *const *const u8) -> i32 {
    let mut stops = TabStops::Uniform(8);
    let mut leading_only = false;
    let mut files: Vec<&[u8]> = Vec::new();
    let mut only_files = false;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => {
                i += 1;
                continue;
            }
        };

        if only_files {
            files.push(arg);
        } else if arg == b"--" {
            only_files = true;
        } else if arg == b"-i" || arg == b"--initial" {
            leading_only = true;
        } else if arg == b"-t" || arg == b"--tabs" {
            i += 1;
            let t = match unsafe { get_arg(argv, i) } {
                Some(t) => t,
                None => {
                    io::write_str(2, b"expand: option requires an argument -- 't'\n");
                    return 2;
                }
            };
            match parse_tablist(t) {
                Some(s) => stops = s,
                None => {
                    io::write_str(2, b"expand: invalid tab list '");
                    io::write_all(2, t);
                    io::write_str(2, b"'\n");
                    return 1;
                }
            }
        } else if arg.starts_with(b"-t") && arg.len() > 2 {
            match parse_tablist(&arg[2..]) {
                Some(s) => stops = s,
                None => {
                    io::write_str(2, b"expand: invalid tab list '");
                    io::write_all(2, &arg[2..]);
                    io::write_str(2, b"'\n");
                    return 1;
                }
            }
        } else if arg.starts_with(b"--tabs=") {
            match parse_tablist(&arg[7..]) {
                Some(s) => stops = s,
                None => {
                    io::write_str(2, b"expand: invalid tab list '");
                    io::write_all(2, &arg[7..]);
                    io::write_str(2, b"'\n");
                    return 1;
                }
            }
        } else if arg.len() > 1 && arg[0] == b'-' && arg != b"-" {
            io::write_str(2, b"expand: invalid option -- '");
            io::write_all(2, &arg[1..]);
            io::write_str(2, b"'\n");
            return 2;
        } else {
            files.push(arg);
        }

        i += 1;
    }

    if files.is_empty() {
        files.push(b"-");
    }

    let mut exit_code = 0;
    for f in files {
        let fd = if f == b"-" {
            0
        } else {
            io::open(f, libc::O_RDONLY, 0)
        };

        if fd < 0 {
            sys::perror(f);
            exit_code = 1;
            continue;
        }

        process(fd, &stops, leading_only);

        if fd != 0 {
            io::close(fd);
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    extern crate std;
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

    #[test]
    fn test_expand_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["expand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\tb\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // 'a' is at column 0, tab expands to column 8
        assert_eq!(stdout, "a       b\n");
    }

    #[test]
    fn test_expand_multiple_tabs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["expand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"\t\t\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Two tabs = 16 spaces
        assert_eq!(stdout, "                \n");
    }

    #[test]
    fn test_expand_no_tabs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["expand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"no tabs here\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "no tabs here\n");
    }

    #[test]
    fn test_expand_custom_tabstop() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["expand", "-t", "4"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\tb\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "a   b\n");
    }
}
