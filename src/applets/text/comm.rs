//! comm - compare two sorted files line by line
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/comm.html

use crate::applets::get_arg;
use crate::io;

/// comm - compare two sorted files line by line
///
/// # Synopsis
/// ```text
/// comm [-123] file1 file2
/// ```
///
/// # Description
/// Compare two sorted files line by line. Produces three text columns:
/// - Column 1: lines only in file1
/// - Column 2: lines only in file2
/// - Column 3: lines in both files
///
/// # Options
/// - `-1`: Suppress column 1 (lines only in file1)
/// - `-2`: Suppress column 2 (lines only in file2)
/// - `-3`: Suppress column 3 (lines in both)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn comm(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        use alloc::vec::Vec;

        let mut suppress_col1 = false;
        let mut suppress_col2 = false;
        let mut suppress_col3 = false;
        let mut file1: Option<&[u8]> = None;
        let mut file2: Option<&[u8]> = None;

        for i in 1..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                if arg.starts_with(b"-") && arg.len() > 1 && arg[1] != b'-' {
                    for &c in &arg[1..] {
                        match c {
                            b'1' => suppress_col1 = true,
                            b'2' => suppress_col2 = true,
                            b'3' => suppress_col3 = true,
                            _ => {}
                        }
                    }
                } else if file1.is_none() {
                    file1 = Some(arg);
                } else if file2.is_none() {
                    file2 = Some(arg);
                }
            }
        }

        let file1 = match file1 {
            Some(f) => f,
            None => {
                io::write_str(2, b"comm: missing operand\n");
                return 1;
            }
        };
        let file2 = match file2 {
            Some(f) => f,
            None => {
                io::write_str(2, b"comm: missing operand\n");
                return 1;
            }
        };

        // Read both files
        let fd1 = if file1 == b"-" {
            0
        } else {
            io::open(file1, libc::O_RDONLY, 0)
        };
        if fd1 < 0 && file1 != b"-" {
            io::write_str(2, b"comm: cannot open file1\n");
            return 1;
        }
        let content1 = io::read_all(fd1);
        if fd1 > 0 {
            io::close(fd1);
        }

        let fd2 = if file2 == b"-" {
            0
        } else {
            io::open(file2, libc::O_RDONLY, 0)
        };
        if fd2 < 0 && file2 != b"-" {
            io::write_str(2, b"comm: cannot open file2\n");
            return 1;
        }
        let content2 = io::read_all(fd2);
        if fd2 > 0 {
            io::close(fd2);
        }

        let lines1: Vec<&[u8]> = content1
            .split(|&c| c == b'\n')
            .filter(|l| !l.is_empty())
            .collect();
        let lines2: Vec<&[u8]> = content2
            .split(|&c| c == b'\n')
            .filter(|l| !l.is_empty())
            .collect();

        let mut i = 0;
        let mut j = 0;

        while i < lines1.len() || j < lines2.len() {
            if i >= lines1.len() {
                // Only file2 has remaining lines
                if !suppress_col2 {
                    if !suppress_col1 {
                        io::write_str(1, b"\t");
                    }
                    io::write_all(1, lines2[j]);
                    io::write_str(1, b"\n");
                }
                j += 1;
            } else if j >= lines2.len() {
                // Only file1 has remaining lines
                if !suppress_col1 {
                    io::write_all(1, lines1[i]);
                    io::write_str(1, b"\n");
                }
                i += 1;
            } else {
                let cmp = cmp_bytes(lines1[i], lines2[j]);
                if cmp < 0 {
                    // Line only in file1
                    if !suppress_col1 {
                        io::write_all(1, lines1[i]);
                        io::write_str(1, b"\n");
                    }
                    i += 1;
                } else if cmp > 0 {
                    // Line only in file2
                    if !suppress_col2 {
                        if !suppress_col1 {
                            io::write_str(1, b"\t");
                        }
                        io::write_all(1, lines2[j]);
                        io::write_str(1, b"\n");
                    }
                    j += 1;
                } else {
                    // Line in both files
                    if !suppress_col3 {
                        if !suppress_col1 {
                            io::write_str(1, b"\t");
                        }
                        if !suppress_col2 {
                            io::write_str(1, b"\t");
                        }
                        io::write_all(1, lines1[i]);
                        io::write_str(1, b"\n");
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"comm: requires alloc feature\n");
        return 1;
    }

    0
}

fn cmp_bytes(a: &[u8], b: &[u8]) -> i32 {
    let min_len = a.len().min(b.len());
    for i in 0..min_len {
        if a[i] < b[i] {
            return -1;
        }
        if a[i] > b[i] {
            return 1;
        }
    }
    if a.len() < b.len() {
        -1
    } else if a.len() > b.len() {
        1
    } else {
        0
    }
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
            "armybox_comm_test_{}_{}",
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
    fn test_comm_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "a\nb\nc\n").unwrap();
        fs::write(&file2, "b\nc\nd\n").unwrap();

        let output = Command::new(&armybox)
            .args(["comm", file1.to_str().unwrap(), file2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // a is only in file1 (col 1), b/c are in both (col 3), d is only in file2 (col 2)
        assert!(stdout.contains("a\n"));
        assert!(stdout.contains("\t\tb\n"));
        assert!(stdout.contains("\t\tc\n"));
        assert!(stdout.contains("\td\n"));
        cleanup(&dir);
    }

    #[test]
    fn test_comm_suppress_col1() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "a\nb\n").unwrap();
        fs::write(&file2, "b\nc\n").unwrap();

        let output = Command::new(&armybox)
            .args([
                "comm",
                "-1",
                file1.to_str().unwrap(),
                file2.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should not contain 'a' (col 1 suppressed)
        assert!(!stdout.starts_with("a"));
        cleanup(&dir);
    }

    #[test]
    fn test_comm_suppress_col3() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "a\nb\n").unwrap();
        fs::write(&file2, "b\nc\n").unwrap();

        let output = Command::new(&armybox)
            .args([
                "comm",
                "-3",
                file1.to_str().unwrap(),
                file2.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain 'a' and 'c' but not 'b' (col 3 suppressed)
        assert!(stdout.contains("a"));
        assert!(stdout.contains("c"));
        cleanup(&dir);
    }
}
