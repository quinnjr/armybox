//! rev - reverse lines character-wise
//!
//! Non-POSIX utility (common in Unix systems).
//! Reverses the characters in each line.

use crate::applets::get_arg;
use crate::io;

/// rev - reverse lines character-wise
///
/// # Synopsis
/// ```text
/// rev [file...]
/// ```
///
/// # Description
/// Copy the specified files to standard output, reversing the
/// order of characters in every line.
/// With no FILE, or when FILE is -, read standard input.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn rev(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        if let Some(path) = unsafe { get_arg(argv, 1) } {
            if path.len() > 0 && path[0] != b'-' {
                io::open(path, libc::O_RDONLY, 0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let mut buf = [0u8; 4096];
    let mut line = [0u8; 1024];
    let mut line_len = 0;

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        for &c in &buf[..n as usize] {
            if c == b'\n' {
                // Reverse and print
                for i in (0..line_len).rev() {
                    io::write_all(1, &line[i..i + 1]);
                }
                io::write_str(1, b"\n");
                line_len = 0;
            } else if line_len < line.len() {
                line[line_len] = c;
                line_len += 1;
            }
        }
    }

    // Print remaining
    if line_len > 0 {
        for i in (0..line_len).rev() {
            io::write_all(1, &line[i..i + 1]);
        }
    }

    if fd != 0 {
        io::close(fd);
    }
    0
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
            "armybox_rev_test_{}_{}",
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
    fn test_rev_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "hello\n").unwrap();

        let output = Command::new(&armybox)
            .args(["rev", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "olleh");
        cleanup(&dir);
    }

    #[test]
    fn test_rev_multiple_lines() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "abc\n123\nxyz\n").unwrap();

        let output = Command::new(&armybox)
            .args(["rev", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["cba", "321", "zyx"]);
        cleanup(&dir);
    }

    #[test]
    fn test_rev_stdin() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["rev"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"test\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "tset");
    }

    #[test]
    fn test_rev_palindrome() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "racecar\n").unwrap();

        let output = Command::new(&armybox)
            .args(["rev", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "racecar");
        cleanup(&dir);
    }
}
