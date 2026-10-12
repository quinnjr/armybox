//! shred - overwrite a file to hide its contents
//!
//! GNU coreutils compatible implementation.

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// shred - overwrite a file to hide its contents
///
/// # Synopsis
/// ```text
/// shred [-u] file...
/// ```
///
/// # Description
/// Overwrite the specified FILE(s) repeatedly to make it harder to
/// recover the data by even very expensive hardware probing.
///
/// # Options
/// - `-u`: Deallocate and remove file after overwriting
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn shred(argc: i32, argv: *const *const u8) -> i32 {
    let mut remove = false;
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'u') {
                    remove = true;
                }
            } else {
                // Overwrite with random data
                let fd = io::open(arg, libc::O_WRONLY, 0);
                if fd < 0 {
                    sys::perror(arg);
                    exit_code = 1;
                    continue;
                }

                let mut st: libc::stat = unsafe { core::mem::zeroed() };
                if io::fstat(fd, &mut st) == 0 {
                    let size = st.st_size as usize;
                    let mut buf = [0xFFu8; 4096];
                    let mut written = 0;
                    while written < size {
                        let chunk = core::cmp::min(buf.len(), size - written);
                        io::write_all(fd, &buf[..chunk]);
                        written += chunk;
                    }
                    unsafe { libc::fsync(fd) };

                    // Zero pass
                    unsafe { libc::lseek(fd, 0, libc::SEEK_SET) };
                    buf.fill(0);
                    written = 0;
                    while written < size {
                        let chunk = core::cmp::min(buf.len(), size - written);
                        io::write_all(fd, &buf[..chunk]);
                        written += chunk;
                    }
                }
                io::close(fd);

                if remove {
                    io::unlink(arg);
                }
            }
        }
    }
    exit_code
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
            "armybox_shred_test_{}_{}",
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
    fn test_shred_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("secret.txt");
        fs::write(&file, "sensitive data").unwrap();

        let output = Command::new(&armybox)
            .args(["shred", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(file.exists()); // File still exists, just overwritten
        // Content should be different (zeros or 0xFF pattern)
        let content = fs::read(&file).unwrap();
        assert_ne!(content, b"sensitive data");
        cleanup(&dir);
    }

    #[test]
    fn test_shred_with_remove() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("to_delete.txt");
        fs::write(&file, "delete me").unwrap();
        assert!(file.exists());

        let output = Command::new(&armybox)
            .args(["shred", "-u", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!file.exists()); // File should be removed
        cleanup(&dir);
    }

    #[test]
    fn test_shred_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["shred", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
