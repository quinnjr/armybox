//! fsync - synchronize a file's in-core state with storage device
//!
//! Flush file system buffers for specified files.

use crate::applets::get_arg;
use crate::io;

/// fsync - synchronize a file's in-core state with storage device
///
/// # Synopsis
/// ```text
/// fsync file...
/// ```
///
/// # Description
/// Force changed blocks to disk, update the super block.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn fsync_cmd(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"fsync: missing operand\n");
        return 1;
    }

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            let fd = io::open(path, libc::O_RDONLY, 0);
            if fd >= 0 {
                unsafe { libc::fsync(fd) };
                io::close(fd);
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

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_fsync_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["fsync"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_fsync_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_file = std::env::temp_dir().join(format!(
            "armybox_fsync_test_{}_{}",
            std::process::id(),
            counter
        ));
        fs::write(&test_file, "test").unwrap();

        let output = Command::new(&armybox)
            .args(["fsync", test_file.to_str().unwrap()])
            .output()
            .unwrap();

        let _ = fs::remove_file(&test_file);
        assert_eq!(output.status.code(), Some(0));
    }
}
