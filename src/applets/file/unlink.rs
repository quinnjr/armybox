//! unlink - remove a directory entry
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/unlink.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// unlink - remove a directory entry
///
/// # Synopsis
/// ```text
/// unlink file
/// ```
///
/// # Description
/// Remove the directory entry specified by file. Unlike rm, unlink is a
/// minimal interface that takes exactly one operand and does not support
/// options.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn unlink(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"unlink: missing operand\n");
        return 1;
    }

    let path = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => {
            io::write_str(2, b"unlink: missing operand\n");
            return 1;
        }
    };

    if io::unlink(path) < 0 {
        sys::perror(path);
        return 1;
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
            "armybox_unlink_test_{}_{}",
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
    fn test_unlink_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();
        assert!(file.exists());

        let output = Command::new(&armybox)
            .args(["unlink", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!file.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_unlink_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["unlink"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_unlink_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["unlink", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_unlink_directory_fails() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let subdir = dir.join("subdir");
        fs::create_dir(&subdir).unwrap();

        let output = Command::new(&armybox)
            .args(["unlink", subdir.to_str().unwrap()])
            .output()
            .unwrap();

        // unlink on directory should fail
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
