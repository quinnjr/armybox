//! mkdir - make directories
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mkdir.html

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// mkdir - make directories
///
/// # Synopsis
/// ```text
/// mkdir [-p] [-m mode] dir...
/// ```
///
/// # Description
/// Creates the specified directories.
///
/// # Options
/// - `-m mode`: Set file permission bits (octal)
/// - `-p`: Create intermediate directories as required
///
/// # Exit Status
/// - 0: All directories created successfully
/// - >0: An error occurred
pub fn mkdir(argc: i32, argv: *const *const u8) -> i32 {
    let mut parents = false;
    let mut mode = 0o755u32;
    let mut exit_code = 0;
    let mut skip_next = false;

    for i in 1..argc {
        if skip_next {
            skip_next = false;
            continue;
        }

        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'p') {
                    parents = true;
                }
                if has_opt(arg, b'm') && i + 1 < argc {
                    if let Some(m) = unsafe { get_arg(argv, i + 1) } {
                        mode = sys::parse_octal(m).unwrap_or(0o755);
                        skip_next = true;
                    }
                }
            } else {
                // This is a directory path
                let result = if parents {
                    mkdir_parents(arg, mode)
                } else {
                    if io::mkdir(arg, mode) < 0 {
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

/// Create directory and all parent directories as needed
pub(crate) fn mkdir_parents(path: &[u8], mode: u32) -> i32 {
    let mut partial = [0u8; 4096];
    let mut len = 0;

    for &c in path {
        if c == b'/' && len > 0 {
            // Try to create intermediate directory (ignore errors if exists)
            let _ = io::mkdir(&partial[..len], mode);
        }
        if len < partial.len() - 1 {
            partial[len] = c;
            len += 1;
        }
    }

    // Create final directory
    if io::mkdir(&partial[..len], mode) < 0 {
        // Check if it already exists and is a directory
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::stat(&partial[..len], &mut st) == 0 {
            if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
                return 0; // Already exists as directory, OK
            }
        }
        sys::perror(path);
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    //! Unit tests for mkdir utility

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
            "armybox_mkdir_test_{}_{}",
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
    fn test_mkdir_single_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let new_dir = dir.join("newdir");

        let output = Command::new(&armybox)
            .args(["mkdir", new_dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_multiple_directories() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let dir1 = dir.join("dir1");
        let dir2 = dir.join("dir2");

        let output = Command::new(&armybox)
            .args(["mkdir", dir1.to_str().unwrap(), dir2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dir1.exists());
        assert!(dir2.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_parents() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let nested = dir.join("a/b/c/d");

        let output = Command::new(&armybox)
            .args(["mkdir", "-p", nested.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(nested.exists());
        assert!(nested.is_dir());
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_existing_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let existing = dir.join("existing");
        fs::create_dir(&existing).unwrap();

        // Without -p, should fail
        let output = Command::new(&armybox)
            .args(["mkdir", existing.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_parents_existing() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let existing = dir.join("existing");
        fs::create_dir(&existing).unwrap();

        // With -p, should succeed even if exists
        let output = Command::new(&armybox)
            .args(["mkdir", "-p", existing.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_mode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let new_dir = dir.join("modedir");

        let output = Command::new(&armybox)
            .args(["mkdir", "-m", "700", new_dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(new_dir.exists());

        // Check permissions
        let metadata = fs::metadata(&new_dir).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        cleanup(&dir);
    }

    #[test]
    fn test_mkdir_no_permission() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Skip test if running as root (can create dirs anywhere)
        if std::env::var("USER").map(|u| u == "root").unwrap_or(false) {
            return;
        }

        // Try to create in root (should fail for non-root)
        let output = Command::new(&armybox)
            .args(["mkdir", "/test_no_permission_dir"])
            .output()
            .unwrap();

        // Should fail for non-root users
        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_mkdir_parents_partial_exists() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        // Create partial path
        fs::create_dir(dir.join("a")).unwrap();
        fs::create_dir(dir.join("a/b")).unwrap();
        let nested = dir.join("a/b/c/d");

        let output = Command::new(&armybox)
            .args(["mkdir", "-p", nested.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(nested.exists());
        cleanup(&dir);
    }
}
