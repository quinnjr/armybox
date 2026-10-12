//! truncate - shrink or extend the size of a file
//!
//! GNU coreutils compatible implementation.

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// truncate - shrink or extend the size of a file
///
/// # Synopsis
/// ```text
/// truncate [-c] -s SIZE file...
/// ```
///
/// # Description
/// Shrink or extend the size of each FILE to the specified size.
///
/// # Options
/// - `-c`, `--no-create`: Do not create files that don't exist
/// - `-s SIZE`: Set or adjust the file size
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn truncate(argc: i32, argv: *const *const u8) -> i32 {
    let mut size: i64 = 0;
    let mut size_set = false;
    let mut size_parse_failed = false;
    let mut no_create = false;
    let mut exit_code = 0;

    // First pass: find size option
    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if arg == b"--no-create" || (arg.get(1) != Some(&b'-') && has_opt(arg, b'c')) {
                    no_create = true;
                }
                if has_opt(arg, b's') && i + 1 < argc {
                    if let Some(s) = unsafe { get_arg(argv, i + 1) } {
                        match sys::parse_u64(s) {
                            Some(v) => {
                                size = v as i64;
                                size_set = true;
                            }
                            None => {
                                size_parse_failed = true;
                            }
                        }
                        i += 1;
                    }
                }
            }
        }
        i += 1;
    }

    if size_parse_failed {
        io::write_str(2, b"truncate: invalid size specified for -s\n");
        return 1;
    }

    // Refuse to truncate anything unless a size was explicitly given via
    // -s; without it there is no safe target size, and defaulting to 0
    // would silently zero every file argument.
    if !size_set {
        io::write_str(2, b"truncate: you must specify -s SIZE\n");
        io::write_str(2, b"usage: truncate -s SIZE file...\n");
        return 1;
    }

    // Second pass: process files
    for j in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, j) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b's') {
                    continue; // Skip the -s and its argument
                }
                continue;
            }

            // Check if this is the size argument
            if j > 1 {
                if let Some(prev) = unsafe { get_arg(argv, j - 1) } {
                    if prev.len() >= 2 && prev[0] == b'-' && has_opt(prev, b's') {
                        continue; // This is the size argument, skip it
                    }
                }
            }
            // Like GNU truncate: create missing files unless -c, which skips them
            if no_create {
                if io::access(arg, libc::F_OK) != 0 {
                    continue;
                }
            } else {
                let fd = io::open(arg, libc::O_WRONLY | libc::O_CREAT, 0o666);
                if fd >= 0 {
                    io::close(fd);
                }
            }

            if io::truncate(arg, size) < 0 {
                sys::perror(arg);
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
            "armybox_truncate_test_{}_{}",
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
    fn test_truncate_shrink() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "hello world!").unwrap();
        assert_eq!(fs::metadata(&file).unwrap().len(), 12);

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "5", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 5);
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello");
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_extend() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "hi").unwrap();
        assert_eq!(fs::metadata(&file).unwrap().len(), 2);

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 10);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_to_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "0", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 0);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_truncate_creates_missing_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("new.txt");

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 10);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_no_create_skips_missing_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let existing = dir.join("existing.txt");
        fs::write(&existing, "hello world!").unwrap();

        for flag in ["-c", "--no-create"] {
            let missing = dir.join("missing.txt");
            let output = Command::new(&armybox)
                .args([
                    "truncate",
                    flag,
                    "-s",
                    "5",
                    missing.to_str().unwrap(),
                    existing.to_str().unwrap(),
                ])
                .output()
                .unwrap();

            assert_eq!(output.status.code(), Some(0), "{flag}");
            assert!(!missing.exists(), "{flag} created the file");
        }
        assert_eq!(fs::metadata(&existing).unwrap().len(), 5);
        cleanup(&dir);
    }
}
