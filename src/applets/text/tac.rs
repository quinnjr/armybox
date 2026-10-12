//! tac - concatenate and print files in reverse
//!
//! Non-POSIX utility (GNU coreutils).
//! Prints files with lines in reverse order.

use crate::applets::get_arg;
use crate::io;

/// tac - concatenate and print files in reverse
///
/// # Synopsis
/// ```text
/// tac [file...]
/// ```
///
/// # Description
/// Write each FILE to standard output, last line first.
/// With no FILE, or when FILE is -, read standard input.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn tac(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        for i in 1..argc {
            if let Some(path) = unsafe { get_arg(argv, i) } {
                if path.len() > 0 && path[0] != b'-' {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd >= 0 {
                        let content = io::read_all(fd);
                        io::close(fd);

                        let lines: Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
                        for line in lines.iter().rev() {
                            if !line.is_empty() {
                                io::write_all(1, line);
                                io::write_str(1, b"\n");
                            }
                        }
                    }
                }
            }
        }

        if argc < 2 {
            let content = io::read_all(0);
            let lines: Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
            for line in lines.iter().rev() {
                if !line.is_empty() {
                    io::write_all(1, line);
                    io::write_str(1, b"\n");
                }
            }
        }
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"tac: requires alloc feature\n");
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
            "armybox_tac_test_{}_{}",
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
    fn test_tac_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        let output = Command::new(&armybox)
            .args(["tac", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line 3");
        assert_eq!(lines[1], "line 2");
        assert_eq!(lines[2], "line 1");
        cleanup(&dir);
    }

    #[test]
    fn test_tac_single_line() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "only one line\n").unwrap();

        let output = Command::new(&armybox)
            .args(["tac", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("only one line"));
        cleanup(&dir);
    }

    #[test]
    fn test_tac_numbers() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "1\n2\n3\n4\n5\n").unwrap();

        let output = Command::new(&armybox)
            .args(["tac", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["5", "4", "3", "2", "1"]);
        cleanup(&dir);
    }
}
