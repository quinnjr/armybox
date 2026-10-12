//! install - copy files and set attributes
//!
//! GNU coreutils compatible implementation.

use super::{copy_file, mkdir_parents};
use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// install - copy files and set attributes
///
/// # Synopsis
/// ```text
/// install [OPTIONS] SOURCE DEST
/// install -d DIRECTORY...
/// ```
///
/// # Description
/// Copy files and set attributes. Used primarily for installing files.
///
/// # Options
/// - `-d`: Create directories
/// - `-m MODE`: Set permission mode
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn install(argc: i32, argv: *const *const u8) -> i32 {
    let mut dir_mode = false;
    let mut mode = 0o755u32;

    // First pass: parse options
    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'd') {
                    dir_mode = true;
                }
                if has_opt(arg, b'm') && i + 1 < argc {
                    if let Some(m) = unsafe { get_arg(argv, i + 1) } {
                        mode = sys::parse_octal(m).unwrap_or(0o755);
                        i += 1;
                    }
                }
            }
        }
        i += 1;
    }

    if dir_mode {
        // Create directories
        for j in 1..argc {
            if let Some(path) = unsafe { get_arg(argv, j) } {
                if path.len() > 0 && path[0] != b'-' {
                    // Skip mode argument
                    if j > 1 {
                        if let Some(prev) = unsafe { get_arg(argv, j - 1) } {
                            if prev.len() >= 2 && prev[0] == b'-' && has_opt(prev, b'm') {
                                continue;
                            }
                        }
                    }
                    mkdir_parents(path, mode);
                }
            }
        }
    } else if argc >= 3 {
        // Find source and destination (skip options)
        let mut src: Option<&[u8]> = None;
        let mut dest: Option<&[u8]> = None;

        let mut j = 1;
        while j < argc {
            if let Some(arg) = unsafe { get_arg(argv, j) } {
                if arg.len() > 0 && arg[0] == b'-' {
                    if has_opt(arg, b'm') {
                        j += 1; // Skip mode argument
                    }
                } else {
                    if src.is_none() {
                        src = Some(arg);
                    } else {
                        dest = Some(arg);
                    }
                }
            }
            j += 1;
        }

        if let (Some(s), Some(d)) = (src, dest) {
            if copy_file(s, d, true) != 0 {
                return 1;
            }
            io::chmod(d, mode);
        } else {
            io::write_str(2, b"install: missing operand\n");
            return 1;
        }
    } else {
        io::write_str(2, b"install: missing operand\n");
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
    use std::os::unix::fs::PermissionsExt;
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
            "armybox_install_test_{}_{}",
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
    fn test_install_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let src = dir.join("source.txt");
        let dest = dir.join("dest.txt");
        fs::write(&src, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["install", src.to_str().unwrap(), dest.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "content");
        cleanup(&dir);
    }

    #[test]
    fn test_install_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let newdir = dir.join("newdir");

        let output = Command::new(&armybox)
            .args(["install", "-d", newdir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(newdir.is_dir());
        cleanup(&dir);
    }

    #[test]
    fn test_install_with_mode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let src = dir.join("source.txt");
        let dest = dir.join("dest.txt");
        fs::write(&src, "content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "install",
                "-m",
                "644",
                src.to_str().unwrap(),
                dest.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dest.exists());
        let mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
        cleanup(&dir);
    }

    #[test]
    fn test_install_nested_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let nested = dir.join("a/b/c");

        let output = Command::new(&armybox)
            .args(["install", "-d", nested.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(nested.is_dir());
        cleanup(&dir);
    }

    #[test]
    fn test_install_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["install"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
