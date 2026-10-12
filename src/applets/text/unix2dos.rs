//! unix2dos - convert Unix line endings to DOS
//!
//! Converts LF line endings to CRLF.

use crate::applets::get_arg;
use crate::io;

/// unix2dos - convert Unix line endings to DOS
///
/// # Synopsis
/// ```text
/// unix2dos file...
/// ```
///
/// # Description
/// Convert Unix line endings (LF) to DOS/Windows line endings (CRLF).
/// Files are modified in place.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn unix2dos(argc: i32, argv: *const *const u8) -> i32 {
    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if path.len() > 0 && path[0] != b'-' {
                #[cfg(feature = "alloc")]
                {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        continue;
                    }

                    let content = io::read_all(fd);
                    io::close(fd);

                    let fd = io::open(path, libc::O_WRONLY | libc::O_TRUNC, 0);
                    if fd < 0 {
                        continue;
                    }

                    for &c in &content {
                        if c == b'\n' {
                            io::write_str(fd, b"\r\n");
                        } else if c != b'\r' {
                            io::write_all(fd, &[c]);
                        }
                    }
                    io::close(fd);
                }
            }
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
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_unix2dos_test_{}_{}",
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
    fn test_unix2dos_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line1\nline2\n").unwrap();

        let output = Command::new(&armybox)
            .args(["unix2dos", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\r\nline2\r\n");
        cleanup(&dir);
    }

    #[test]
    fn test_unix2dos_already_dos() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "already\r\ndos\r\n").unwrap();

        let output = Command::new(&armybox)
            .args(["unix2dos", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        // Should not double up CRs
        assert_eq!(content, b"already\r\ndos\r\n");
        cleanup(&dir);
    }

    #[test]
    fn test_unix2dos_no_newlines() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "no newline").unwrap();

        let output = Command::new(&armybox)
            .args(["unix2dos", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"no newline");
        cleanup(&dir);
    }
}
