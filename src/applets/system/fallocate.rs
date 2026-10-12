//! fallocate - preallocate or deallocate space to a file
//!
//! Manipulate file space.

use crate::io;
use crate::sys;
use crate::applets::get_arg;

/// fallocate - preallocate or deallocate space to a file
///
/// # Synopsis
/// ```text
/// fallocate -l SIZE FILE
/// ```
///
/// # Description
/// Preallocate space to a file using ftruncate.
///
/// # Options
/// - `-l SIZE`: Specify the length of the file
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn fallocate(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 4 {
        io::write_str(2, b"fallocate: usage: fallocate -l SIZE FILE\n");
        return 1;
    }

    let mut size = 0i64;
    let file_idx = argc - 1;

    for i in 1..argc-1 {
        if let Some(arg) = unsafe { get_arg(argv, i ) } {
            if arg == b"-l" && i + 1 < argc - 1 {
                if let Some(s) = unsafe { get_arg(argv, i + 1 ) } {
                    size = sys::parse_i64(s).unwrap_or(0);
                }
            }
        }
    }

    let path = unsafe { get_arg(argv, file_idx ).unwrap() };
    let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT, 0o644);
    if fd < 0 {
        sys::perror(path);
        return 1;
    }

    if io::ftruncate(fd, size) != 0 {
        sys::perror(b"fallocate");
        io::close(fd);
        return 1;
    }

    io::close(fd);
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::process::Command;
    use std::path::PathBuf;
    use std::fs;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_fallocate_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["fallocate"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_fallocate_create_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let pid = std::process::id();
        let test_file = std::env::temp_dir().join(std::format!("armybox_fallocate_test_{}", pid));

        let output = Command::new(&armybox)
            .args(["fallocate", "-l", "1024", test_file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // Verify file was created with correct size
        let metadata = fs::metadata(&test_file).unwrap();
        assert_eq!(metadata.len(), 1024);

        let _ = fs::remove_file(test_file);
    }
}
