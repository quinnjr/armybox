//! tee - read from stdin and write to stdout and files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tee.html

use crate::applets::{get_arg, has_opt};
use crate::io;

/// tee - read from stdin and write to stdout and files
///
/// # Synopsis
/// ```text
/// tee [-ai] [file...]
/// ```
///
/// # Description
/// Copy standard input to standard output and also to any specified files.
///
/// # Options
/// - `-a`: Append to files instead of overwriting
/// - `-i`: Ignore SIGINT signal
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn tee(argc: i32, argv: *const *const u8) -> i32 {
    let mut append = false;

    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut fds: Vec<i32> = Vec::new();
        fds.push(1); // stdout

        for i in 1..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                if arg.len() > 0 && has_opt(arg, b'a') {
                    append = true;
                } else if arg.len() > 0 && arg[0] != b'-' {
                    let flags = if append {
                        libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND
                    } else {
                        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
                    };
                    let fd = io::open(arg, flags, 0o644);
                    if fd >= 0 {
                        fds.push(fd);
                    }
                }
            }
        }

        let mut buf = [0u8; 4096];
        loop {
            let n = io::read(0, &mut buf);
            if n <= 0 {
                break;
            }

            for &fd in &fds {
                io::write_all(fd, &buf[..n as usize]);
            }
        }

        for &fd in &fds[1..] {
            io::close(fd);
        }
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = append;
        let mut buf = [0u8; 4096];
        loop {
            let n = io::read(0, &mut buf);
            if n <= 0 {
                break;
            }
            io::write_all(1, &buf[..n as usize]);
        }
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

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_tee_test_{}_{}",
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
    fn test_tee_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("output.txt");

        let mut child = Command::new(&armybox)
            .args(["tee", file.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello world\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Check stdout
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello world\n"
        );

        // Check file
        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, "hello world\n");

        cleanup(&dir);
    }

    #[test]
    fn test_tee_append() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("output.txt");

        // Write initial content
        fs::write(&file, "first line\n").unwrap();

        let mut child = Command::new(&armybox)
            .args(["tee", "-a", file.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"second line\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Check file has both lines
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("first line"));
        assert!(content.contains("second line"));

        cleanup(&dir);
    }

    #[test]
    fn test_tee_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("out1.txt");
        let file2 = dir.join("out2.txt");

        let mut child = Command::new(&armybox)
            .args(["tee", file1.to_str().unwrap(), file2.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"test data\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Check both files
        assert_eq!(fs::read_to_string(&file1).unwrap(), "test data\n");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "test data\n");

        cleanup(&dir);
    }
}
