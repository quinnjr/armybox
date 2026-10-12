//! tail - output the last part of files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tail.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// What to count when trimming output, and whether it is anchored to the
/// end (last N) or to the start (`+N` forms, meaning "starting at N").
enum Mode {
    /// Last N lines.
    LastLines(u64),
    /// Starting at line N (1-based).
    FromLine(u64),
    /// Last N bytes.
    LastBytes(u64),
    /// Starting at byte N (1-based).
    FromByte(u64),
}

/// Parse a `-n`/`-c` argument, handling the `+N` "from start" form and the
/// `-N` "last N" form (a bare leading `-`, e.g. from `-n -5`, still means
/// "last N" — it is not a parse error). Returns `None` if the numeric part
/// cannot be parsed at all, so callers can diagnose and exit rather than
/// silently defaulting.
fn parse_count(s: &[u8], last: fn(u64) -> Mode, start: fn(u64) -> Mode) -> Option<Mode> {
    if s.is_empty() {
        return None;
    }
    if s[0] == b'+' {
        sys::parse_u64(&s[1..]).map(start)
    } else if s[0] == b'-' {
        sys::parse_u64(&s[1..]).map(last)
    } else {
        sys::parse_u64(s).map(last)
    }
}

/// tail - output the last part of files
///
/// # Synopsis
/// ```text
/// tail [-f] [-q] [-n number | -c number] [file...]
/// ```
///
/// # Description
/// Copy the last N lines (default 10) of each FILE to standard output.
/// With no FILE, or when FILE is -, read standard input. When more than
/// one FILE is given, a `==> FILE <==` header is printed before each
/// file's output, unless `-q` is given.
///
/// # Options
/// - `-n N`: Print the last N lines instead of the last 10. `-n +N`
///   prints starting at line N (1-based) instead.
/// - `-c N`: Print the last N bytes instead of lines. `-c +N` prints
///   starting at byte N (1-based) instead.
/// - `-N`: Legacy form of `-n N`.
/// - `-f`: Follow the file (output appended data as the file grows). With
///   multiple files, only the last file is followed after all files'
///   initial output has been printed.
/// - `-q`: Never print file name headers.
/// - `--help`: Print usage information and exit.
///
/// # Exit Status
/// - 0: Success
/// - 1: An error occurred reading one or more files, or an option's count
///   could not be parsed
/// - 2: An unrecognized long option was given
pub fn tail(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = Mode::LastLines(10);
    let mut follow = false;
    let mut quiet = false;

    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut files: Vec<&'static [u8]> = Vec::new();

        let mut i = 1;
        while i < argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                if arg == b"--help" {
                    print_usage();
                    return 0;
                } else if arg.len() > 2 && arg[0] == b'-' && arg[1] == b'-' {
                    io::write_str(2, b"tail: unrecognized option '");
                    io::write_all(2, arg);
                    io::write_str(2, b"'\n");
                    return 2;
                } else if arg.len() > 1 && arg[0] == b'-' && arg != b"-" {
                    if arg[1] == b'n' {
                        let spec: &[u8] = if arg.len() > 2 {
                            &arg[2..]
                        } else if i + 1 < argc {
                            i += 1;
                            match unsafe { get_arg(argv, i) } {
                                Some(n) => n,
                                None => b"",
                            }
                        } else {
                            b""
                        };
                        match parse_count(spec, Mode::LastLines, Mode::FromLine) {
                            Some(m) => mode = m,
                            None => return invalid_number(b"lines", spec),
                        }
                    } else if arg[1] == b'c' {
                        let spec: &[u8] = if arg.len() > 2 {
                            &arg[2..]
                        } else if i + 1 < argc {
                            i += 1;
                            match unsafe { get_arg(argv, i) } {
                                Some(n) => n,
                                None => b"",
                            }
                        } else {
                            b""
                        };
                        match parse_count(spec, Mode::LastBytes, Mode::FromByte) {
                            Some(m) => mode = m,
                            None => return invalid_number(b"bytes", spec),
                        }
                    } else if arg[1] >= b'0' && arg[1] <= b'9' {
                        // Legacy -N form
                        match sys::parse_u64(&arg[1..]) {
                            Some(n) => mode = Mode::LastLines(n),
                            None => return invalid_number(b"lines", &arg[1..]),
                        }
                    } else {
                        for &b in &arg[1..] {
                            match b {
                                b'f' => follow = true,
                                b'q' => quiet = true,
                                _ => {}
                            }
                        }
                    }
                } else if let Some(path) = unsafe { get_arg(argv, i) } {
                    files.push(path);
                }
            }
            i += 1;
        }

        let mut exit_code = 0;

        if files.is_empty() {
            if !tail_fd(0, &mode) {
                exit_code = 1;
            }
            follow_fd(0, follow);
        } else {
            let multiple = files.len() > 1 && !quiet;
            let last_idx = files.len() - 1;
            for (idx, &path) in files.iter().enumerate() {
                if idx > 0 && multiple {
                    if io::write_str(1, b"\n") < 0 {
                        exit_code = 1;
                    }
                }

                if path == b"-" {
                    if multiple && !print_header(b"standard input") {
                        exit_code = 1;
                    }
                    if !tail_fd(0, &mode) {
                        exit_code = 1;
                    }
                    // Only follow the last file when several are given;
                    // GNU tail's true multi-file follow multiplexes all of
                    // them, but a single followed stream keeps this
                    // implementation simple and predictable.
                    if idx == last_idx {
                        follow_fd(0, follow);
                    }
                } else {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        io::write_str(2, b"tail: ");
                        sys::perror(path);
                        exit_code = 1;
                        continue;
                    }
                    if multiple && !print_header(path) {
                        exit_code = 1;
                    }
                    if !tail_fd(fd, &mode) {
                        exit_code = 1;
                    }
                    if idx == last_idx {
                        follow_fd(fd, follow);
                    }
                    io::close(fd);
                }
            }
        }

        exit_code
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = argc;
        let _ = argv;
        let _ = &mode;
        let _ = follow;
        let _ = quiet;
        io::write_str(2, b"tail: requires alloc feature\n");
        1
    }
}

/// Print an "invalid number of X: 'Y'" diagnostic to stderr and return the
/// exit code (1) callers should propagate, matching GNU tail's behavior of
/// rejecting unparsable counts instead of silently defaulting.
#[cfg(feature = "alloc")]
fn invalid_number(kind: &[u8], spec: &[u8]) -> i32 {
    io::write_str(2, b"tail: invalid number of ");
    io::write_all(2, kind);
    io::write_str(2, b": '");
    io::write_all(2, spec);
    io::write_str(2, b"'\n");
    1
}

fn print_usage() {
    io::write_str(
        1,
        b"Usage: tail [-f] [-q] [-n number | -c number] [file...]\n",
    );
    io::write_str(
        1,
        b"  -n N  Print the last N lines instead of the last 10. -n +N prints from line N.\n",
    );
    io::write_str(
        1,
        b"  -c N  Print the last N bytes instead of lines. -c +N prints from byte N.\n",
    );
    io::write_str(1, b"  -f    Follow the file as it grows.\n");
    io::write_str(1, b"  -q    Never print file name headers.\n");
    io::write_str(1, b"  --help  Display this help and exit.\n");
}

fn print_header(name: &[u8]) -> bool {
    let mut ok = io::write_str(1, b"==> ") >= 0;
    ok &= io::write_all(1, name) >= 0;
    ok &= io::write_str(1, b" <==\n") >= 0;
    ok
}

fn tail_fd(fd: i32, mode: &Mode) -> bool {
    match mode {
        Mode::LastLines(n) => tail_last_lines(fd, *n),
        Mode::FromLine(n) => tail_from_line(fd, *n),
        Mode::LastBytes(n) => tail_last_bytes(fd, *n),
        Mode::FromByte(n) => tail_from_byte(fd, *n),
    }
}

/// If `-f` was requested and the fd supports it, keep reading and echoing
/// appended data. `fd <= 0` (stdin) is followed too, matching GNU tail's
/// behavior for pipes.
fn follow_fd(fd: i32, follow: bool) {
    if !follow {
        return;
    }
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n > 0 {
            if io::write_all(1, &buf[..n as usize]) < 0 {
                // stdout is gone (e.g. the reader closed its pipe); there
                // is nothing more we can usefully do.
                return;
            }
        } else {
            unsafe { libc::usleep(100_000) };
        }
    }
}

/// Print the last `n` lines of `fd`.
///
/// Reads the whole input, keeping only the most recent `n` complete
/// (or final, possibly-unterminated) lines in a ring buffer, then
/// writes them out in one batch.
#[cfg(feature = "alloc")]
fn tail_last_lines(fd: i32, lines: u64) -> bool {
    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    let cap = lines as usize;
    // Don't pre-reserve `cap` up front: `lines` is a user-supplied count,
    // so grow the buffer on demand instead of allocating a
    // caller-controlled amount before any input has been read.
    let mut line_buf: VecDeque<Vec<u8>> = VecDeque::new();
    let mut current_line = Vec::new();
    let mut buf = [0u8; 4096];

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        for j in 0..n as usize {
            current_line.push(buf[j]);
            if buf[j] == b'\n' {
                if cap > 0 {
                    if line_buf.len() >= cap {
                        line_buf.pop_front();
                    }
                    line_buf.push_back(core::mem::take(&mut current_line));
                } else {
                    current_line.clear();
                }
            }
        }
    }

    if !current_line.is_empty() && cap > 0 {
        if line_buf.len() >= cap {
            line_buf.pop_front();
        }
        line_buf.push_back(current_line);
    }

    let mut ok = true;
    for line in line_buf {
        if io::write_all(1, &line) < 0 {
            ok = false;
        }
    }
    ok
}

/// Print lines starting at 1-based line number `n` (tail -n +N).
fn tail_from_line(fd: i32, n: u64) -> bool {
    let target = if n == 0 { 1 } else { n };
    let mut line_no: u64 = 1;
    let mut buf = [0u8; 4096];
    let mut out_buf = [0u8; 8192];
    let mut out_len = 0usize;

    loop {
        let r = io::read(fd, &mut buf);
        if r <= 0 {
            break;
        }
        for &b in &buf[..r as usize] {
            if line_no >= target {
                if out_len == out_buf.len() {
                    if io::write_all(1, &out_buf[..out_len]) < 0 {
                        return false;
                    }
                    out_len = 0;
                }
                out_buf[out_len] = b;
                out_len += 1;
            }
            if b == b'\n' {
                line_no += 1;
            }
        }
    }

    if out_len > 0 {
        return io::write_all(1, &out_buf[..out_len]) >= 0;
    }
    true
}

/// Print the last `n` bytes of `fd`.
#[cfg(feature = "alloc")]
fn tail_last_bytes(fd: i32, n: u64) -> bool {
    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    let cap = n as usize;
    // Don't pre-reserve `cap` up front; grow the ring on demand so a large
    // user-supplied `n` cannot force an unbounded allocation before any
    // input has been read.
    let mut ring: VecDeque<u8> = VecDeque::new();
    let mut buf = [0u8; 4096];

    loop {
        let r = io::read(fd, &mut buf);
        if r <= 0 {
            break;
        }
        for &b in &buf[..r as usize] {
            if cap > 0 {
                if ring.len() >= cap {
                    ring.pop_front();
                }
                ring.push_back(b);
            }
        }
    }

    let out: Vec<u8> = ring.into_iter().collect();
    io::write_all(1, &out) >= 0
}

/// Print bytes starting at 1-based byte offset `n` (tail -c +N).
fn tail_from_byte(fd: i32, n: u64) -> bool {
    let target = if n == 0 { 1 } else { n };
    let mut byte_no: u64 = 1;
    let mut buf = [0u8; 4096];
    let mut out_buf = [0u8; 8192];
    let mut out_len = 0usize;

    loop {
        let r = io::read(fd, &mut buf);
        if r <= 0 {
            break;
        }
        for &b in &buf[..r as usize] {
            if byte_no >= target {
                if out_len == out_buf.len() {
                    if io::write_all(1, &out_buf[..out_len]) < 0 {
                        return false;
                    }
                    out_len = 0;
                }
                out_buf[out_len] = b;
                out_len += 1;
            }
            byte_no += 1;
        }
    }

    if out_len > 0 {
        return io::write_all(1, &out_buf[..out_len]) >= 0;
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

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_tail_test_{}_{}",
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
    fn test_tail_default() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        let content = (1..=20)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        fs::write(&file, &content).unwrap();

        let output = Command::new(&armybox)
            .args(["tail", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 10);
        assert!(stdout.contains("line 20"));
        cleanup(&dir);
    }

    #[test]
    fn test_tail_n_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        let content = (1..=20)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        fs::write(&file, &content).unwrap();

        let output = Command::new(&armybox)
            .args(["tail", "-n", "5", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 5);
        assert!(stdout.contains("line 16"));
        cleanup(&dir);
    }

    #[test]
    fn test_tail_short_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        let output = Command::new(&armybox)
            .args(["tail", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 3);
        cleanup(&dir);
    }
}
