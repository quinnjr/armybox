//! stat - display file or file system status
//!
//! GNU coreutils compatible implementation.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// stat - display file or file system status
///
/// # Synopsis
/// ```text
/// stat [options] file...
/// ```
///
/// # Description
/// Display file or file system status including size, blocks, inode, etc.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn stat(argc: i32, argv: *const *const u8) -> i32 {
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if path.len() > 0 && path[0] == b'-' {
                continue;
            }

            let mut st: libc::stat = unsafe { core::mem::zeroed() };
            if io::stat(path, &mut st) < 0 {
                sys::perror(path);
                exit_code = 1;
                continue;
            }

            io::write_str(1, b"  File: ");
            io::write_all(1, path);
            io::write_str(1, b"\n  Size: ");
            io::write_num(1, st.st_size as u64);
            io::write_str(1, b"\tBlocks: ");
            io::write_num(1, st.st_blocks as u64);
            io::write_str(1, b"\nDevice: ");
            io::write_num(1, st.st_dev as u64);
            io::write_str(1, b"\tInode: ");
            io::write_num(1, st.st_ino as u64);
            io::write_str(1, b"\tLinks: ");
            io::write_num(1, st.st_nlink as u64);
            io::write_str(1, b"\nAccess: ");
            let mut mode_buf = [0u8; 10];
            sys::format_mode(st.st_mode as u32, &mut mode_buf);
            io::write_all(1, &mode_buf);
            io::write_str(1, b"\n");
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
            "armybox_stat_test_{}_{}",
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
    fn test_stat_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "hello world").unwrap();

        let output = Command::new(&armybox)
            .args(["stat", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("File:"));
        assert!(stdout.contains("Size:"));
        assert!(stdout.contains("Inode:"));
        cleanup(&dir);
    }

    #[test]
    fn test_stat_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();

        let output = Command::new(&armybox)
            .args(["stat", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("File:"));
        cleanup(&dir);
    }

    #[test]
    fn test_stat_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["stat", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_stat_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let output = Command::new(&armybox)
            .args(["stat", file1.to_str().unwrap(), file2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should have output for both files
        assert!(stdout.matches("File:").count() >= 2);
        cleanup(&dir);
    }
}
