//! file - determine file type
//!
//! Simplified implementation of the file command.

use crate::applets::get_arg;
use crate::io;

/// file - determine file type
///
/// # Synopsis
/// ```text
/// file file...
/// ```
///
/// # Description
/// Determine the type of each FILE argument.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn file(argc: i32, argv: *const *const u8) -> i32 {
    let mut exit_code = 0;

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if path.len() > 0 && path[0] == b'-' {
                continue;
            }

            io::write_all(1, path);
            io::write_str(1, b": ");

            let mut st: libc::stat = unsafe { core::mem::zeroed() };
            if io::lstat(path, &mut st) < 0 {
                io::write_str(1, b"cannot stat\n");
                exit_code = 1;
                continue;
            }

            match st.st_mode as u32 & libc::S_IFMT as u32 {
                t if t == libc::S_IFDIR as u32 => {
                    io::write_str(1, b"directory\n");
                }
                t if t == libc::S_IFLNK as u32 => {
                    io::write_str(1, b"symbolic link\n");
                }
                t if t == libc::S_IFIFO as u32 => {
                    io::write_str(1, b"fifo (named pipe)\n");
                }
                t if t == libc::S_IFSOCK as u32 => {
                    io::write_str(1, b"socket\n");
                }
                t if t == libc::S_IFBLK as u32 => {
                    io::write_str(1, b"block special\n");
                }
                t if t == libc::S_IFCHR as u32 => {
                    io::write_str(1, b"character special\n");
                }
                t if t == libc::S_IFREG as u32 => {
                    // Check magic bytes
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd >= 0 {
                        let mut magic = [0u8; 8];
                        let n = io::read(fd, &mut magic);
                        io::close(fd);

                        if n >= 4 {
                            if magic[0..4] == [0x7F, b'E', b'L', b'F'] {
                                io::write_str(1, b"ELF executable\n");
                            } else if magic[0..2] == *b"#!" {
                                io::write_str(1, b"script\n");
                            } else if magic[0..4] == [0x1F, 0x8B, 0x08, 0x00] {
                                io::write_str(1, b"gzip compressed\n");
                            } else if n >= 3 && magic[0..3] == *b"BZh" {
                                io::write_str(1, b"bzip2 compressed\n");
                            } else if st.st_size == 0 {
                                io::write_str(1, b"empty\n");
                            } else {
                                io::write_str(1, b"data\n");
                            }
                        } else {
                            io::write_str(1, b"empty\n");
                        }
                    } else {
                        io::write_str(1, b"regular file\n");
                    }
                }
                _ => {
                    io::write_str(1, b"unknown\n");
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
            "armybox_file_cmd_test_{}_{}",
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
    fn test_file_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();

        let output = Command::new(&armybox)
            .args(["file", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("directory"));
        cleanup(&dir);
    }

    #[test]
    fn test_file_regular() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("regular.txt");
        fs::write(&file, "some content").unwrap();

        let output = Command::new(&armybox)
            .args(["file", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("data"));
        cleanup(&dir);
    }

    #[test]
    fn test_file_empty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("empty.txt");
        fs::write(&file, "").unwrap();

        let output = Command::new(&armybox)
            .args(["file", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("empty"));
        cleanup(&dir);
    }

    #[test]
    fn test_file_symlink() {
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
            .args(["file", link.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("symbolic link"));
        cleanup(&dir);
    }

    #[test]
    fn test_file_script() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let script = dir.join("script.sh");
        fs::write(&script, "#!/bin/sh\necho hello").unwrap();

        let output = Command::new(&armybox)
            .args(["file", script.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("script"));
        cleanup(&dir);
    }

    #[test]
    fn test_file_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["file", "/nonexistent/path"])
            .output()
            .unwrap();

        // Should still succeed but report "cannot stat"
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("cannot stat"));
    }
}
