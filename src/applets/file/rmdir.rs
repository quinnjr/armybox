//! rmdir - remove directories
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/rmdir.html

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// rmdir - remove directories
///
/// # Synopsis
/// ```text
/// rmdir [-p] dir...
/// ```
///
/// # Description
/// Removes empty directories. The directories must be empty for the
/// operation to succeed.
///
/// # Options
/// - `-p`: Remove directory and its ancestors (parents)
///
/// # Exit Status
/// - 0: All directories removed successfully
/// - >0: An error occurred
pub fn rmdir(argc: i32, argv: *const *const u8) -> i32 {
    let mut parents = false;
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'p') {
                    parents = true;
                }
            } else {
                // This is a directory path
                let result = if parents {
                    rmdir_parents(arg)
                } else {
                    if io::rmdir(arg) < 0 {
                        sys::perror(arg);
                        1
                    } else {
                        0
                    }
                };
                if result != 0 {
                    exit_code = 1;
                }
            }
        }
    }
    exit_code
}

/// Remove directory and its empty parent directories
fn rmdir_parents(path: &[u8]) -> i32 {
    let mut current_path = [0u8; 4096];
    let mut len = path.len().min(current_path.len() - 1);

    // Copy path
    for (i, &c) in path.iter().enumerate() {
        if i < current_path.len() - 1 {
            current_path[i] = c;
        }
    }

    // First, remove the specified directory (must succeed)
    if io::rmdir(&current_path[..len]) < 0 {
        sys::perror(&current_path[..len]);
        return 1;
    }

    // Then try to remove parent directories, stopping on any failure
    loop {
        // Find parent by removing last component
        while len > 0 && current_path[len - 1] != b'/' {
            len -= 1;
        }
        // Remove trailing slash
        while len > 0 && current_path[len - 1] == b'/' {
            len -= 1;
        }

        // Stop if we've reached root or empty
        if len == 0 {
            break;
        }

        // Try to remove parent - stop silently on failure
        // (parent may be non-empty or we lack permission)
        if io::rmdir(&current_path[..len]) < 0 {
            break;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    //! Unit tests for rmdir utility

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
        let release = PathBuf::from("target/release/armybox");
        if release.exists() {
            return release;
        }
        PathBuf::from("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_rmdir_test_{}_{}",
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
    fn test_rmdir_empty_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let empty_dir = dir.join("empty");
        fs::create_dir(&empty_dir).unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", empty_dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!empty_dir.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rmdir_multiple_directories() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let dir1 = dir.join("dir1");
        let dir2 = dir.join("dir2");
        fs::create_dir(&dir1).unwrap();
        fs::create_dir(&dir2).unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", dir1.to_str().unwrap(), dir2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir1.exists());
        assert!(!dir2.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rmdir_nonempty_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let nonempty = dir.join("nonempty");
        fs::create_dir(&nonempty).unwrap();
        fs::write(nonempty.join("file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", nonempty.to_str().unwrap()])
            .output()
            .unwrap();

        // Should fail - directory not empty
        assert_ne!(output.status.code(), Some(0));
        assert!(nonempty.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rmdir_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rmdir", "/nonexistent/directory"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_rmdir_parents() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let nested = dir.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", "-p", nested.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!nested.exists());
        assert!(!dir.join("a/b").exists());
        assert!(!dir.join("a").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rmdir_parents_partial() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let nested = dir.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        // Put a file in 'a' so it can't be removed
        fs::write(dir.join("a/file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", "-p", nested.to_str().unwrap()])
            .output()
            .unwrap();

        // Should succeed - removes what it can, stops silently on non-empty
        assert_eq!(output.status.code(), Some(0));
        // c and b should be gone
        assert!(!nested.exists());
        assert!(!dir.join("a/b").exists());
        // a should still exist (has file)
        assert!(dir.join("a").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rmdir_file_not_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rmdir", file.to_str().unwrap()])
            .output()
            .unwrap();

        // Should fail - not a directory
        assert_ne!(output.status.code(), Some(0));
        assert!(file.exists());
        cleanup(&dir);
    }
}
