//! link - create a hard link to a file
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/link.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// link - create a hard link to a file
///
/// # Synopsis
/// ```text
/// link file1 file2
/// ```
///
/// # Description
/// Create a hard link to a file. Unlike ln, link is a minimal interface
/// that takes exactly two operands.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn link(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"link: missing operand\n");
        return 1;
    }

    let target = match unsafe { get_arg(argv, 1) } {
        Some(t) => t,
        None => {
            io::write_str(2, b"link: missing operand\n");
            return 1;
        }
    };

    let link_name = match unsafe { get_arg(argv, 2) } {
        Some(l) => l,
        None => {
            io::write_str(2, b"link: missing operand\n");
            return 1;
        }
    };

    if io::link(target, link_name) < 0 {
        sys::perror(link_name);
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

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_link_cmd_test_{}_{}",
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
    fn test_link_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let target = dir.join("target.txt");
        let link_name = dir.join("link.txt");
        fs::write(&target, "content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "link",
                target.to_str().unwrap(),
                link_name.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(link_name.exists());
        // Hard links share the same content
        assert_eq!(fs::read_to_string(&link_name).unwrap(), "content");
        cleanup(&dir);
    }

    #[test]
    fn test_link_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["link"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_link_missing_second_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let target = dir.join("target.txt");
        fs::write(&target, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["link", target.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_link_nonexistent_target() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let link_name = dir.join("link.txt");

        let output = Command::new(&armybox)
            .args(["link", "/nonexistent/file", link_name.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
