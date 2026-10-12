//! head - output the first part of files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/head.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// What to count when trimming output.
enum Mode {
    Lines(i64),
    Bytes(i64),
}

/// head - output the first part of files
///
/// # Synopsis
/// ```text
/// head [-n number | -c number] [file...]
/// ```
///
/// # Description
/// Copy the first N lines (default 10) of each FILE to standard output.
/// With no FILE, or when FILE is -, read standard input. When more than
/// one FILE is given, a `==> FILE <==` header is printed before each
/// file's output, separated by a blank line.
///
/// # Options
/// - `-n N`: Print the first N lines instead of the first 10. A negative
///   N means "all but the last N lines".
/// - `-c N`: Print the first N bytes instead of lines. A negative N means
///   "all but the last N bytes".
/// - `-N`: Legacy form of `-n N`.
/// - `--help`: Print usage information and exit.
///
/// # Exit Status
/// - 0: Success
/// - 1: An error occurred reading one or more files, or an option's count
///   could not be parsed
/// - 2: An unrecognized long option was given
pub fn head(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = Mode::Lines(10);

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
                    io::write_str(2, b"head: unrecognized option '");
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
                        match sys::parse_i64(spec) {
                            Some(n) => mode = Mode::Lines(n),
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
                        match sys::parse_i64(spec) {
                            Some(n) => mode = Mode::Bytes(n),
                            None => return invalid_number(b"bytes", spec),
                        }
                    } else if arg[1] >= b'0' && arg[1] <= b'9' {
                        // Legacy -N form
                        match sys::parse_i64(&arg[1..]) {
                            Some(n) => mode = Mode::Lines(n),
                            None => return invalid_number(b"lines", &arg[1..]),
                        }
                    }
                    // Unknown short option flag(s) are otherwise ignored.
                } else if let Some(path) = unsafe { get_arg(argv, i) } {
                    files.push(path);
                }
            }
            i += 1;
        }

        let mut exit_code = 0;

        if files.is_empty() {
            if !head_fd(0, &mode) {
                exit_code = 1;
            }
        } else {
            let multiple = files.len() > 1;
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
                    if !head_fd(0, &mode) {
                        exit_code = 1;
                    }
                } else {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        io::write_str(2, b"head: ");
                        sys::perror(path);
                        exit_code = 1;
                        continue;
                    }
                    if multiple && !print_header(path) {
                        exit_code = 1;
                    }
                    if !head_fd(fd, &mode) {
                        exit_code = 1;
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
        io::write_str(2, b"head: requires alloc feature\n");
        1
    }
}

/// Print an "invalid number of X: 'Y'" diagnostic to stderr and return the
/// exit code (1) callers should propagate, matching GNU head's behavior of
/// rejecting unparsable counts instead of silently defaulting.
#[cfg(feature = "alloc")]
fn invalid_number(kind: &[u8], spec: &[u8]) -> i32 {
    io::write_str(2, b"head: invalid number of ");
    io::write_all(2, kind);
    io::write_str(2, b": '");
    io::write_all(2, spec);
    io::write_str(2, b"'\n");
    1
}

fn print_usage() {
    io::write_str(1, b"Usage: head [-n number | -c number] [file...]\n");
    io::write_str(
        1,
        b"  -n N  Print the first N lines instead of the first 10.\n",
    );
    io::write_str(1, b"  -c N  Print the first N bytes instead of lines.\n");
    io::write_str(1, b"  --help  Display this help and exit.\n");
}

fn print_header(name: &[u8]) -> bool {
    let mut ok = io::write_str(1, b"==> ") >= 0;
    ok &= io::write_all(1, name) >= 0;
    ok &= io::write_str(1, b" <==\n") >= 0;
    ok
}

/// Buffer used for batching output writes.
const OUT_BUF_SIZE: usize = 8192;

fn head_fd(fd: i32, mode: &Mode) -> bool {
    match mode {
        Mode::Bytes(n) => {
            if *n >= 0 {
                head_bytes(fd, *n)
            } else {
                head_bytes_all_but_last(fd, (-*n) as u64)
            }
        }
        Mode::Lines(n) => {
            if *n >= 0 {
                head_lines(fd, *n as u64)
            } else {
                head_lines_all_but_last(fd, (-*n) as u64)
            }
        }
    }
}

/// Print the first `n` bytes of `fd`, buffering output.
fn head_bytes(fd: i32, n: i64) -> bool {
    if n <= 0 {
        return true;
    }
    let mut remaining = n as u64;
    let mut in_buf = [0u8; 4096];
    let mut out_buf = [0u8; OUT_BUF_SIZE];
    let mut out_len = 0usize;

    while remaining > 0 {
        let r = io::read(fd, &mut in_buf);
        if r <= 0 {
            break;
        }
        let mut take = r as usize;
        if take as u64 > remaining {
            take = remaining as usize;
        }
        let mut off = 0usize;
        while off < take {
            let space = OUT_BUF_SIZE - out_len;
            let chunk = core::cmp::min(space, take - off);
            out_buf[out_len..out_len + chunk].copy_from_slice(&in_buf[off..off + chunk]);
            out_len += chunk;
            off += chunk;
            if out_len == OUT_BUF_SIZE {
                if io::write_all(1, &out_buf[..out_len]) < 0 {
                    return false;
                }
                out_len = 0;
            }
        }
        remaining -= take as u64;
    }

    if out_len > 0 {
        return io::write_all(1, &out_buf[..out_len]) >= 0;
    }
    true
}

/// Print the first `n` lines of `fd`, buffering output rather than
/// writing byte-by-byte.
fn head_lines(fd: i32, mut n: u64) -> bool {
    if n == 0 {
        return true;
    }
    let mut in_buf = [0u8; 4096];
    let mut out_buf = [0u8; OUT_BUF_SIZE];
    let mut out_len = 0usize;

    'outer: loop {
        let r = io::read(fd, &mut in_buf);
        if r <= 0 {
            break;
        }
        for &b in &in_buf[..r as usize] {
            if out_len == OUT_BUF_SIZE {
                if io::write_all(1, &out_buf[..out_len]) < 0 {
                    return false;
                }
                out_len = 0;
            }
            out_buf[out_len] = b;
            out_len += 1;
            if b == b'\n' {
                n -= 1;
                if n == 0 {
                    break 'outer;
                }
            }
        }
    }

    if out_len > 0 {
        return io::write_all(1, &out_buf[..out_len]) >= 0;
    }
    true
}

/// Print all but the last `n` bytes of `fd` (head -c -N).
///
/// Streams input through a bounded ring buffer of `n` bytes: once the
/// buffer is full, each newly read byte evicts (and emits) the oldest
/// buffered byte, so at EOF only the last `n` bytes remain unwritten.
#[cfg(feature = "alloc")]
fn head_bytes_all_but_last(fd: i32, n: u64) -> bool {
    use alloc::collections::VecDeque;

    if n == 0 {
        // Nothing to hold back; print everything.
        return head_bytes(fd, i64::MAX);
    }

    let cap = n as usize;
    // Do not pre-reserve `cap` up front: `n` comes straight from user
    // input, so grow the ring on demand instead of allocating a
    // caller-controlled amount before any input has been read.
    let mut ring: VecDeque<u8> = VecDeque::new();
    let mut in_buf = [0u8; 4096];
    let mut out_buf = [0u8; OUT_BUF_SIZE];
    let mut out_len = 0usize;

    loop {
        let r = io::read(fd, &mut in_buf);
        if r <= 0 {
            break;
        }
        for &b in &in_buf[..r as usize] {
            ring.push_back(b);
            if ring.len() > cap {
                if let Some(front) = ring.pop_front() {
                    if out_len == OUT_BUF_SIZE {
                        if io::write_all(1, &out_buf[..out_len]) < 0 {
                            return false;
                        }
                        out_len = 0;
                    }
                    out_buf[out_len] = front;
                    out_len += 1;
                }
            }
        }
    }

    if out_len > 0 {
        return io::write_all(1, &out_buf[..out_len]) >= 0;
    }
    true
}

#[cfg(not(feature = "alloc"))]
fn head_bytes_all_but_last(_fd: i32, _n: u64) -> bool {
    io::write_str(2, b"head: -c negative requires alloc feature\n");
    false
}

/// Print all but the last `n` lines of `fd` (head -n -N).
///
/// Reads the whole input, buffering complete lines, then holds back the
/// last `n` of them.
#[cfg(feature = "alloc")]
fn head_lines_all_but_last(fd: i32, n: u64) -> bool {
    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    if n == 0 {
        // Nothing to hold back; print everything.
        return head_lines(fd, u64::MAX);
    }

    let cap = n as usize;
    // Do not pre-reserve `cap` up front; grow the queue on demand so a
    // large user-supplied `n` cannot force an unbounded allocation before
    // any input has been read.
    let mut queue: VecDeque<Vec<u8>> = VecDeque::new();
    let mut current_line = Vec::new();
    let mut buf = [0u8; 4096];
    let mut ok = true;

    loop {
        let r = io::read(fd, &mut buf);
        if r <= 0 {
            break;
        }
        for &b in &buf[..r as usize] {
            current_line.push(b);
            if b == b'\n' {
                queue.push_back(core::mem::take(&mut current_line));
                if queue.len() > cap {
                    if let Some(line) = queue.pop_front() {
                        if io::write_all(1, &line) < 0 {
                            ok = false;
                        }
                    }
                }
            }
        }
    }

    if !current_line.is_empty() {
        queue.push_back(current_line);
        if queue.len() > cap {
            if let Some(line) = queue.pop_front() {
                if io::write_all(1, &line) < 0 {
                    ok = false;
                }
            }
        }
    }
    // Any lines left in `queue` are the last `n` lines; discard them.
    ok
}

#[cfg(not(feature = "alloc"))]
fn head_lines_all_but_last(_fd: i32, _n: u64) -> bool {
    io::write_str(2, b"head: -n negative requires alloc feature\n");
    false
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
            "armybox_head_test_{}_{}",
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
    fn test_head_default() {
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
            .args(["head", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 10);
        assert!(stdout.starts_with("line 1\n"));
        cleanup(&dir);
    }

    #[test]
    fn test_head_n_option() {
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
            .args(["head", "-n", "5", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 5);
        cleanup(&dir);
    }

    #[test]
    fn test_head_short_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        let output = Command::new(&armybox)
            .args(["head", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 3);
        cleanup(&dir);
    }

    #[test]
    fn test_head_numeric_option() {
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
            .args(["head", "-3", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.lines().count(), 3);
        cleanup(&dir);
    }
}
