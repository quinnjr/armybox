//! cat - concatenate and print files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cat.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// cat - concatenate and print files
///
/// # Synopsis
/// ```text
/// cat [-u] [file...]
/// ```
///
/// # Description
/// Reads files sequentially, writing them to standard output.
/// If no file is specified, or if file is `-`, reads standard input.
///
/// # Options
/// - `-u`: Write bytes unbuffered (no-op, always unbuffered in this implementation)
///
/// # Exit Status
/// - 0: All files read successfully
/// - >0: An error occurred
pub fn cat(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        // Read from stdin
        let mut buf = [0u8; 4096];
        loop {
            let n = io::read(0, &mut buf);
            if n <= 0 {
                break;
            }
            io::write_all(1, &buf[..n as usize]);
        }
        return 0;
    }

    let mut exit_code = 0;

    for i in 1..argc {
        let path = match unsafe { get_arg(argv, i) } {
            Some(p) => p,
            None => continue,
        };

        // Skip -u option (unbuffered, always true for us)
        if path == b"-u" {
            continue;
        }

        if path == b"-" {
            let mut buf = [0u8; 4096];
            loop {
                let n = io::read(0, &mut buf);
                if n <= 0 {
                    break;
                }
                io::write_all(1, &buf[..n as usize]);
            }
            continue;
        }

        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd < 0 {
            sys::perror(path);
            exit_code = 1;
            continue;
        }

        let mut buf = [0u8; 4096];
        loop {
            let n = io::read(fd, &mut buf);
            if n <= 0 {
                break;
            }
            io::write_all(1, &buf[..n as usize]);
        }
        io::close(fd);
    }
    exit_code
}

#[cfg(test)]
mod tests {
    //! Unit tests for cat utility
    //!
    //! These tests run the armybox binary as a subprocess.
    //! The binary must be built before running tests:
    //! `RUSTFLAGS="-C linker=gcc -C link-arg=-lc" cargo build --release`

    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn get_armybox_path() -> PathBuf {
        // Check environment variable first (for CI or custom setups)
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        // Try release first, then debug
        let release = PathBuf::from("target/release/armybox");
        if release.exists() {
            return release;
        }
        PathBuf::from("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_cat_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("file1.txt"), "hello\n").unwrap();
        fs::write(dir.join("file2.txt"), "world\n").unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_cat_single_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();
        let output = Command::new(&armybox)
            .args(["cat", dir.join("file1.txt").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello\n"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cat_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();
        let output = Command::new(&armybox)
            .args([
                "cat",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello\nworld\n"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cat_nonexistent_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let output = Command::new(&armybox)
            .args(["cat", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        assert!(std::string::String::from_utf8_lossy(&output.stderr).contains("No such file"));
    }

    #[test]
    fn test_cat_unbuffered_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();
        let output = Command::new(&armybox)
            .args(["cat", "-u", dir.join("file1.txt").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello\n"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cat_stdin_dash() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        // Test that "-" reads from stdin
        let mut child = Command::new(&armybox)
            .args(["cat", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"stdin test").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "stdin test"
        );
    }

    #[test]
    fn test_cat_empty_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();
        fs::write(dir.join("empty.txt"), "").unwrap();

        let output = Command::new(&armybox)
            .args(["cat", dir.join("empty.txt").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout.len(), 0);
        cleanup(&dir);
    }

    #[test]
    fn test_cat_binary_data() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();
        fs::write(dir.join("binary.bin"), [0u8, 1, 2, 255, 254, 253]).unwrap();

        let output = Command::new(&armybox)
            .args(["cat", dir.join("binary.bin").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, vec![0u8, 1, 2, 255, 254, 253]);
        cleanup(&dir);
    }

    #[test]
    fn test_cat_mixed_stdin_and_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        let dir = setup();

        let mut child = Command::new(&armybox)
            .args(["cat", dir.join("file1.txt").to_str().unwrap(), "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"STDIN").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello\nSTDIN"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cat_no_args_stdin() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            eprintln!("Skipping test: armybox binary not found at {:?}", armybox);
            return;
        }

        // Test that cat with no args reads from stdin
        let mut child = Command::new(&armybox)
            .args(["cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"from stdin").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "from stdin"
        );
    }
}
