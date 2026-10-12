//! readahead - initiate file readahead into page cache
//!
//! Preload file content into the page cache.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// readahead - initiate file readahead into page cache
///
/// # Synopsis
/// ```text
/// readahead file...
/// ```
///
/// # Description
/// Initiate readahead on files to preload them into the page cache.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn readahead_cmd(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"readahead: missing file operand\n");
        return 1;
    }

    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd < 0 {
                sys::perror(path);
                exit_code = 1;
                continue;
            }

            // Get file size
            let size = unsafe { libc::lseek(fd, 0, libc::SEEK_END) };
            if size > 0 {
                // Call readahead syscall
                unsafe {
                    libc::syscall(libc::SYS_readahead, fd, 0i64, size as usize);
                }
            }

            io::close(fd);
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
    fn test_readahead_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["readahead"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_readahead_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let test_file = std::env::temp_dir().join(std::format!("armybox_readahead_test_{}", pid));
        fs::write(&test_file, "test content for readahead").unwrap();

        let output = Command::new(&armybox)
            .args(["readahead", test_file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        let _ = fs::remove_file(test_file);
    }
}
