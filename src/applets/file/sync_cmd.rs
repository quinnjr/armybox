//! sync - synchronize cached writes to persistent storage
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sync.html

/// sync - synchronize cached writes to persistent storage
///
/// # Synopsis
/// ```text
/// sync
/// ```
///
/// # Description
/// Force completion of pending disk writes (flush cache).
/// The sync utility writes all information in memory that should be on disk,
/// including modified superblocks, modified inodes, and delayed reads and writes.
///
/// # Exit Status
/// - 0: Success (always)
pub fn sync_cmd(_argc: i32, _argv: *const *const u8) -> i32 {
    unsafe { libc::sync() };
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_sync_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["sync"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_sync_no_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["sync"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // sync should produce no output
        assert!(output.stdout.is_empty());
    }
}
