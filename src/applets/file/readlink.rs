//! readlink - print resolved symbolic links or canonical file names
//!
//! GNU coreutils compatible implementation.

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// readlink - print resolved symbolic links or canonical file names
///
/// # Synopsis
/// ```text
/// readlink [-f] file...
/// ```
///
/// # Description
/// Print the value of a symbolic link or canonical file name.
///
/// # Options
/// - `-f`: Canonicalize by following every symlink in every component,
///         recursively; all but the last component must exist
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn readlink(argc: i32, argv: *const *const u8) -> i32 {
    let mut canonicalize = false;
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'f') {
                    canonicalize = true;
                }
            } else {
                let mut buf = [0u8; 4096];
                let n = if canonicalize {
                    io::realpath(arg, &mut buf)
                } else {
                    io::readlink(arg, &mut buf)
                };
                if n > 0 {
                    io::write_all(1, &buf[..n as usize]);
                    io::write_str(1, b"\n");
                } else {
                    sys::perror(arg);
                    exit_code = 1;
                }
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
            "armybox_readlink_test_{}_{}",
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
    fn test_readlink_symlink() {
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
            .args(["readlink", link.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().contains("target.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_readlink_canonicalize() {
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
            .args(["readlink", "-f", link.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should be absolute path
        assert!(stdout.trim().starts_with('/'));
        cleanup(&dir);
    }

    #[test]
    fn test_readlink_regular_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        // readlink on regular file should fail
        let output = Command::new(&armybox)
            .args(["readlink", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_readlink_canonicalize_regular() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        // readlink -f on regular file should work (returns canonical path)
        let output = Command::new(&armybox)
            .args(["readlink", "-f", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
