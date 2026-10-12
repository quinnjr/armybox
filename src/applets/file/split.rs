//! split - split a file into pieces
//!
//! GNU coreutils compatible implementation.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec;
use alloc::vec::Vec;

/// split - split a file into pieces
///
/// # Synopsis
/// ```text
/// split [-l lines] [-b bytes] [-a suffix_len] [file [prefix]]
/// ```
///
/// # Description
/// Split a file into pieces. Output pieces are named PREFIXaa, PREFIXab, etc.
///
/// # Options
/// - `-l N`: Put N lines per output file (default: 1000)
/// - `-b N`: Put N bytes per output file (supports K, M, G suffixes)
/// - `-a N`: Use N characters for suffix (default: 2)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn split(argc: i32, argv: *const *const u8) -> i32 {
    let mut lines: Option<usize> = None;
    let mut bytes: Option<usize> = None;
    let mut suffix_len = 2usize;
    let mut prefix = b"x".as_slice();
    let mut input: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 1 && arg[0] == b'-' {
                match arg[1] {
                    b'l' => {
                        // -lN or -l N
                        if arg.len() > 2 {
                            lines = parse_size(&arg[2..]);
                        } else if i + 1 < argc {
                            i += 1;
                            if let Some(n) = unsafe { get_arg(argv, i) } {
                                lines = parse_size(n);
                            }
                        }
                    }
                    b'b' => {
                        // -bN or -b N
                        if arg.len() > 2 {
                            bytes = parse_size(&arg[2..]);
                        } else if i + 1 < argc {
                            i += 1;
                            if let Some(n) = unsafe { get_arg(argv, i) } {
                                bytes = parse_size(n);
                            }
                        }
                    }
                    b'a' => {
                        // -aN or -a N
                        if arg.len() > 2 {
                            suffix_len = sys::parse_u64(&arg[2..]).unwrap_or(2) as usize;
                        } else if i + 1 < argc {
                            i += 1;
                            if let Some(n) = unsafe { get_arg(argv, i) } {
                                suffix_len = sys::parse_u64(n).unwrap_or(2) as usize;
                            }
                        }
                    }
                    b'-' => {
                        // Long options
                        if arg == b"--help" {
                            print_help();
                            return 0;
                        } else if arg.starts_with(b"--lines=") {
                            lines = parse_size(&arg[8..]);
                        } else if arg.starts_with(b"--bytes=") {
                            bytes = parse_size(&arg[8..]);
                        } else if arg.starts_with(b"--suffix-length=") {
                            suffix_len = sys::parse_u64(&arg[16..]).unwrap_or(2) as usize;
                        }
                    }
                    b'0'..=b'9' => {
                        // -N shorthand for -lN
                        lines = parse_size(&arg[1..]);
                    }
                    _ => {}
                }
            } else if arg == b"-" {
                input = None; // stdin
            } else if input.is_none() {
                input = Some(arg);
            } else {
                prefix = arg;
            }
        }
        i += 1;
    }

    // Default to lines mode if neither specified
    if lines.is_none() && bytes.is_none() {
        lines = Some(1000);
    }

    // Open input
    let fd = match input {
        Some(p) => {
            let f = io::open(p, libc::O_RDONLY, 0);
            if f < 0 {
                io::write_str(2, b"split: ");
                io::write_all(2, p);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            f
        }
        None => 0, // stdin
    };

    let result = if let Some(n) = bytes {
        split_by_bytes(fd, prefix, n, suffix_len)
    } else {
        split_by_lines(fd, prefix, lines.unwrap_or(1000), suffix_len)
    };

    if fd != 0 {
        io::close(fd);
    }

    result
}

/// Parse size with optional K, M, G suffix
fn parse_size(s: &[u8]) -> Option<usize> {
    if s.is_empty() {
        return None;
    }

    let (num_part, multiplier) = match s.last() {
        Some(b'K') | Some(b'k') => (&s[..s.len() - 1], 1024),
        Some(b'M') | Some(b'm') => (&s[..s.len() - 1], 1024 * 1024),
        Some(b'G') | Some(b'g') => (&s[..s.len() - 1], 1024 * 1024 * 1024),
        _ => (s, 1),
    };

    sys::parse_u64(num_part).map(|n| (n as usize) * multiplier)
}

/// Split by number of lines
fn split_by_lines(fd: i32, prefix: &[u8], lines_per_file: usize, suffix_len: usize) -> i32 {
    let mut suffix = SuffixGenerator::new(suffix_len);
    let mut line_count = 0;
    let mut out_fd: i32 = -1;
    let mut buf = [0u8; 4096];
    let mut line_buf: Vec<u8> = Vec::new();

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        for &byte in &buf[..n as usize] {
            line_buf.push(byte);

            if byte == b'\n' {
                // Write the line
                if out_fd < 0 || line_count >= lines_per_file {
                    // Open new output file
                    if out_fd >= 0 {
                        io::close(out_fd);
                    }

                    let filename = make_filename(prefix, suffix.next());
                    out_fd = io::open(
                        &filename,
                        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                        0o644,
                    );
                    if out_fd < 0 {
                        io::write_str(2, b"split: cannot create output file\n");
                        return 1;
                    }
                    line_count = 0;
                }

                io::write_all(out_fd, &line_buf);
                line_buf.clear();
                line_count += 1;
            }
        }
    }

    // Write any remaining data
    if !line_buf.is_empty() {
        if out_fd < 0 || line_count >= lines_per_file {
            if out_fd >= 0 {
                io::close(out_fd);
            }
            let filename = make_filename(prefix, suffix.next());
            out_fd = io::open(
                &filename,
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o644,
            );
            if out_fd < 0 {
                io::write_str(2, b"split: cannot create output file\n");
                return 1;
            }
        }
        io::write_all(out_fd, &line_buf);
    }

    if out_fd >= 0 {
        io::close(out_fd);
    }

    0
}

/// Split by number of bytes
fn split_by_bytes(fd: i32, prefix: &[u8], bytes_per_file: usize, suffix_len: usize) -> i32 {
    let mut suffix = SuffixGenerator::new(suffix_len);
    let mut byte_count = 0;
    let mut out_fd: i32 = -1;
    let mut buf = [0u8; 4096];

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        let data = &buf[..n as usize];
        let mut offset = 0;

        while offset < data.len() {
            if out_fd < 0 || byte_count >= bytes_per_file {
                // Open new output file
                if out_fd >= 0 {
                    io::close(out_fd);
                }

                let filename = make_filename(prefix, suffix.next());
                out_fd = io::open(
                    &filename,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                    0o644,
                );
                if out_fd < 0 {
                    io::write_str(2, b"split: cannot create output file\n");
                    return 1;
                }
                byte_count = 0;
            }

            let remaining_in_file = bytes_per_file - byte_count;
            let remaining_in_buf = data.len() - offset;
            let to_write = core::cmp::min(remaining_in_file, remaining_in_buf);

            io::write_all(out_fd, &data[offset..offset + to_write]);
            byte_count += to_write;
            offset += to_write;
        }
    }

    if out_fd >= 0 {
        io::close(out_fd);
    }

    0
}

/// Generate output filename
fn make_filename(prefix: &[u8], suffix: &[u8]) -> Vec<u8> {
    let mut name = Vec::with_capacity(prefix.len() + suffix.len());
    name.extend_from_slice(prefix);
    name.extend_from_slice(suffix);
    name
}

/// Suffix generator (aa, ab, ..., az, ba, bb, ..., zz, aaa, ...)
struct SuffixGenerator {
    current: Vec<u8>,
    previous: Vec<u8>,
}

impl SuffixGenerator {
    fn new(len: usize) -> Self {
        let len = if len < 2 { 2 } else { len };
        let current = vec![b'a'; len];
        SuffixGenerator {
            current: current.clone(),
            previous: current,
        }
    }

    fn next(&mut self) -> &[u8] {
        // Store current as previous (this is what we'll return)
        self.previous = self.current.clone();

        // Increment current for next call
        let mut carry = true;
        for i in (0..self.current.len()).rev() {
            if carry {
                if self.current[i] == b'z' {
                    self.current[i] = b'a';
                } else {
                    self.current[i] += 1;
                    carry = false;
                }
            }
        }

        if carry {
            // Extend suffix length (all z's wrapped around)
            self.current.insert(0, b'a');
        }

        // Return the previous value (before increment)
        &self.previous
    }
}

fn print_help() {
    io::write_str(1, b"Usage: split [OPTION]... [FILE [PREFIX]]\n");
    io::write_str(1, b"Output pieces of FILE to PREFIXaa, PREFIXab, ...\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -l N, --lines=N       put N lines per output file\n");
    io::write_str(
        1,
        b"  -b N, --bytes=N       put N bytes per output file (K, M, G suffixes)\n",
    );
    io::write_str(
        1,
        b"  -a N, --suffix-length=N  use suffixes of length N (default 2)\n",
    );
    io::write_str(1, b"  --help                display this help and exit\n\n");
    io::write_str(
        1,
        b"With no FILE, or when FILE is -, read standard input.\n",
    );
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
            "armybox_split_test_{}_{}",
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
    fn test_split_by_lines() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let input = dir.join("input.txt");
        let mut content = std::string::String::new();
        for i in 1..=10 {
            content.push_str(&format!("line {}\n", i));
        }
        fs::write(&input, &content).unwrap();

        let output = Command::new(&armybox)
            .current_dir(&dir)
            .args(["split", "-l", "3", input.to_str().unwrap(), "out_"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // Should create 4 files: out_aa (3 lines), out_ab (3), out_ac (3), out_ad (1)
        assert!(dir.join("out_aa").exists());
        assert!(dir.join("out_ab").exists());
        assert!(dir.join("out_ac").exists());
        assert!(dir.join("out_ad").exists());

        cleanup(&dir);
    }

    #[test]
    fn test_split_by_bytes() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let input = dir.join("input.txt");
        fs::write(&input, "0123456789").unwrap();

        let output = Command::new(&armybox)
            .current_dir(&dir)
            .args(["split", "-b", "3", input.to_str().unwrap(), "chunk_"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // Should create 4 files: chunk_aa (3), chunk_ab (3), chunk_ac (3), chunk_ad (1)
        assert!(dir.join("chunk_aa").exists());
        let content = fs::read_to_string(dir.join("chunk_aa")).unwrap();
        assert_eq!(content, "012");

        cleanup(&dir);
    }

    #[test]
    fn test_split_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["split", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
