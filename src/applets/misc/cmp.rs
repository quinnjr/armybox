//! cmp - compare two files byte by byte
//!
//! Compare two files and report the first difference.

use super::get_arg;
use crate::io;
use crate::sys;

/// cmp - compare two files
///
/// # Synopsis
/// ```text
/// cmp [-s|-l] FILE1 FILE2
/// ```
///
/// # Description
/// Compare two files byte by byte.
///
/// # Options
/// - `-s`: Silent mode; print nothing, only report the exit status.
/// - `-l`: List every differing byte offset (decimal) and value (octal) in
///   each file, rather than stopping at the first difference.
///
/// # Exit Status
/// - 0: Files are identical
/// - 1: Files differ
/// - 2: Error (e.g. a file could not be opened)
pub fn cmp(argc: i32, argv: *const *const u8) -> i32 {
    let mut silent = false;
    let mut list = false;
    let mut path1: Option<&[u8]> = None;
    let mut path2: Option<&[u8]> = None;

    for i in 1..argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => continue,
        };

        if arg.len() >= 2 && arg[0] == b'-' && arg != b"-" {
            for &c in &arg[1..] {
                match c {
                    b's' => silent = true,
                    b'l' => list = true,
                    _ => {
                        if !silent {
                            io::write_str(2, b"cmp: invalid option -- '");
                            io::write_all(2, &[c]);
                            io::write_str(2, b"'\n");
                        }
                        return 2;
                    }
                }
            }
        } else if path1.is_none() {
            path1 = Some(arg);
        } else if path2.is_none() {
            path2 = Some(arg);
        }
    }

    let (path1, path2) = match (path1, path2) {
        (Some(p1), Some(p2)) => (p1, p2),
        _ => {
            if !silent {
                io::write_str(2, b"cmp: missing operand\n");
            }
            return 2;
        }
    };

    let fd1 = if path1 == b"-" {
        0
    } else {
        io::open(path1, libc::O_RDONLY, 0)
    };
    if fd1 < 0 {
        if !silent {
            sys::perror(path1);
        }
        return 2;
    }

    let fd2 = if path2 == b"-" {
        0
    } else {
        io::open(path2, libc::O_RDONLY, 0)
    };
    if fd2 < 0 {
        if !silent {
            sys::perror(path2);
        }
        if fd1 != 0 {
            io::close(fd1);
        }
        return 2;
    }

    let mut buf1 = [0u8; 4096];
    let mut buf2 = [0u8; 4096];
    let mut pos1 = 0usize;
    let mut pos2 = 0usize;
    let mut len1 = 0isize;
    let mut len2 = 0isize;
    let mut have1 = true; // whether buf1 needs (re)filling on first iteration
    let mut have2 = true;

    let mut byte_num = 0u64;
    let mut line_num = 1u64;
    let mut differ = false;
    let mut io_error = false;

    loop {
        if have1 {
            let n = io::read(fd1, &mut buf1);
            if n < 0 {
                io_error = true;
                break;
            }
            len1 = n;
            pos1 = 0;
            have1 = false;
        }
        if have2 {
            let n = io::read(fd2, &mut buf2);
            if n < 0 {
                io_error = true;
                break;
            }
            len2 = n;
            pos2 = 0;
            have2 = false;
        }

        let eof1 = len1 == 0;
        let eof2 = len2 == 0;

        if eof1 && eof2 {
            break;
        }
        if eof1 || eof2 {
            differ = true;
            if !silent {
                let shorter = if eof1 { path1 } else { path2 };
                io::write_str(2, b"cmp: EOF on ");
                io::write_all(2, shorter);
                io::write_str(2, b"\n");
            }
            break;
        }

        let b1 = buf1[pos1];
        let b2 = buf2[pos2];
        pos1 += 1;
        pos2 += 1;
        byte_num += 1;

        if b1 != b2 {
            differ = true;
            if list {
                if !silent {
                    let mut nb = [0u8; 20];
                    let mut ob1 = [0u8; 4];
                    let mut ob2 = [0u8; 4];
                    io::write_all(1, sys::format_u64(byte_num, &mut nb));
                    io::write_str(1, b" ");
                    io::write_all(1, sys::format_octal(b1 as u32, &mut ob1));
                    io::write_str(1, b" ");
                    io::write_all(1, sys::format_octal(b2 as u32, &mut ob2));
                    io::write_str(1, b"\n");
                }
            } else {
                if !silent {
                    let mut nb = [0u8; 20];
                    let mut lb = [0u8; 20];
                    io::write_all(1, path1);
                    io::write_str(1, b" ");
                    io::write_all(1, path2);
                    io::write_str(1, b" differ: char ");
                    io::write_all(1, sys::format_u64(byte_num, &mut nb));
                    io::write_str(1, b", line ");
                    io::write_all(1, sys::format_u64(line_num, &mut lb));
                    io::write_str(1, b"\n");
                }
                break;
            }
        }

        if b1 == b'\n' {
            line_num += 1;
        }

        if pos1 as isize >= len1 {
            have1 = true;
        }
        if pos2 as isize >= len2 {
            have2 = true;
        }
    }

    if fd1 != 0 {
        io::close(fd1);
    }
    if fd2 != 0 {
        io::close(fd2);
    }

    if io_error {
        2
    } else if differ {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
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

    #[test]
    fn test_cmp_identical() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = std::env::temp_dir().join("armybox_test_cmp");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("file1.txt"), "hello world").unwrap();
        fs::write(dir.join("file2.txt"), "hello world").unwrap();

        let output = Command::new(&armybox)
            .args([
                "cmp",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cmp_different() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = std::env::temp_dir().join("armybox_test_cmp2");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("file1.txt"), "hello").unwrap();
        fs::write(dir.join("file2.txt"), "world").unwrap();

        let output = Command::new(&armybox)
            .args([
                "cmp",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("differ"));
        let _ = fs::remove_dir_all(&dir);
    }
}
