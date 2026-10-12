//! dos2unix - convert DOS line endings to Unix
//!
//! Converts CRLF line endings to LF.

use crate::applets::get_arg;
use crate::io;

/// dos2unix - convert DOS line endings to Unix
///
/// # Synopsis
/// ```text
/// dos2unix file...
/// ```
///
/// # Description
/// Convert DOS/Windows line endings (CRLF) to Unix line endings (LF).
/// Files are modified in place.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn dos2unix(argc: i32, argv: *const *const u8) -> i32 {
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
                        if c != b'\r' {
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
            "armybox_dos2unix_test_{}_{}",
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
    fn test_dos2unix_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line1\r\nline2\r\n").unwrap();

        let output = Command::new(&armybox)
            .args(["dos2unix", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"line1\nline2\n");
        cleanup(&dir);
    }

    #[test]
    fn test_dos2unix_no_crlf() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "already unix\n").unwrap();

        let output = Command::new(&armybox)
            .args(["dos2unix", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"already unix\n");
        cleanup(&dir);
    }

    #[test]
    fn test_dos2unix_mixed_endings() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "dos\r\nunix\nmixed\r\n").unwrap();

        let output = Command::new(&armybox)
            .args(["dos2unix", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read(&file).unwrap();
        assert_eq!(content, b"dos\nunix\nmixed\n");
        cleanup(&dir);
    }
}
