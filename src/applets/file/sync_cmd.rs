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
