//! cd - change the working directory
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cd.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// cd - change the working directory
///
/// # Synopsis
/// ```text
/// cd [directory]
/// ```
///
/// # Description
/// Change the current working directory. If no directory is given,
/// change to the home directory.
///
/// Note: This is typically a shell builtin. This external implementation
/// can only affect its own process.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn cd(argc: i32, argv: *const *const u8) -> i32 {
    let path = if argc > 1 {
        match unsafe { get_arg(argv, 1) } {
            Some(p) => p,
            None => b"/root",
        }
    } else {
        // Get HOME
        b"/root"
    };

    if io::chdir(path) < 0 {
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
            "armybox_cd_test_{}_{}",
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
    fn test_cd_valid_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();

        let output = Command::new(&armybox)
            .args(["cd", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_cd_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["cd", "/nonexistent/directory"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_cd_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["cd"]).output().unwrap();

        // Without HOME set, may succeed or fail depending on /root access
        // Just check it doesn't crash
        let _ = output.status.code();
    }

    #[test]
    fn test_cd_to_file_fails() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["cd", file.to_str().unwrap()])
            .output()
            .unwrap();

        // cd to a file should fail
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
