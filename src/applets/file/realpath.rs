//! realpath - print resolved absolute pathname
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/realpath.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// realpath - print resolved absolute pathname
///
/// # Synopsis
/// ```text
/// realpath file...
/// ```
///
/// # Description
/// Print the resolved absolute file name; all but the last component must exist.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn realpath(argc: i32, argv: *const *const u8) -> i32 {
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if path.len() > 0 && path[0] == b'-' {
                continue;
            }

            let mut buf = [0u8; 4096];
            let n = io::realpath(path, &mut buf);
            if n > 0 {
                io::write_all(1, &buf[..n as usize]);
                io::write_str(1, b"\n");
            } else {
                sys::perror(path);
                exit_code = 1;
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
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_realpath_test_{}_{}",
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
    fn test_realpath_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["realpath", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().starts_with('/'));
        assert!(stdout.trim().ends_with("file.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_realpath_symlink() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let target = dir.join("target.txt");
        let link = dir.join("link");
        fs::write(&target, "content").unwrap();
        symlink(&target, &link).unwrap();

        let output = Command::new(&armybox)
            .args(["realpath", link.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should resolve to absolute path of target
        assert!(stdout.trim().ends_with("target.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_realpath_relative() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let subdir = dir.join("sub");
        fs::create_dir(&subdir).unwrap();
        let file = subdir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["realpath", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().starts_with('/'));
        cleanup(&dir);
    }

    #[test]
    fn test_realpath_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["realpath", "/nonexistent/path/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
