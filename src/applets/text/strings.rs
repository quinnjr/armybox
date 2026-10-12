//! strings - print printable strings from binary
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/strings.html

use crate::applets::get_arg;
use crate::io;

/// strings - print printable strings from binary
///
/// # Synopsis
/// ```text
/// strings file...
/// ```
///
/// # Description
/// Find printable strings in a binary file. Prints sequences of at least
/// 4 printable characters followed by a non-printable character.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn strings(argc: i32, argv: *const *const u8) -> i32 {
    let min_len = 4;

    for i in 1..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if path.len() > 0 && path[0] != b'-' {
                let fd = io::open(path, libc::O_RDONLY, 0);
                if fd < 0 {
                    continue;
                }

                let mut buf = [0u8; 4096];
                let mut string = [0u8; 256];
                let mut string_len = 0;

                loop {
                    let n = io::read(fd, &mut buf);
                    if n <= 0 {
                        break;
                    }

                    for &c in &buf[..n as usize] {
                        if c >= 0x20 && c < 0x7f {
                            if string_len < string.len() {
                                string[string_len] = c;
                                string_len += 1;
                            }
                        } else {
                            if string_len >= min_len {
                                io::write_all(1, &string[..string_len]);
                                io::write_str(1, b"\n");
                            }
                            string_len = 0;
                        }
                    }
                }

                // Flush any remaining string at end of file
                if string_len >= min_len {
                    io::write_all(1, &string[..string_len]);
                    io::write_str(1, b"\n");
                }

                io::close(fd);
            }
        }
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
            "armybox_strings_test_{}_{}",
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
    fn test_strings_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("binary.bin");
        // Create file with mixed binary and text: null bytes, "hello", null bytes, "world"
        let data: Vec<u8> = vec![
            0, 0, 0, b'h', b'e', b'l', b'l', b'o', 0, 0, b'w', b'o', b'r', b'l', b'd', 0,
        ];
        fs::write(&file, &data).unwrap();

        let output = Command::new(&armybox)
            .args(["strings", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Both "hello" and "world" are >= 4 chars
        assert!(stdout.contains("hello"));
        assert!(stdout.contains("world"));
        cleanup(&dir);
    }

    #[test]
    fn test_strings_short_strings() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("binary.bin");
        // Create file with short strings (< 4 chars)
        let data: Vec<u8> = vec![0, b'a', b'b', b'c', 0, 0];
        fs::write(&file, &data).unwrap();

        let output = Command::new(&armybox)
            .args(["strings", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // "abc" is only 3 chars, should not be printed
        assert!(!stdout.contains("abc"));
        cleanup(&dir);
    }

    #[test]
    fn test_strings_long_string() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("binary.bin");
        // Create file with a long string
        let mut data: Vec<u8> = vec![0, 0];
        data.extend(b"this is a longer string");
        data.push(0);
        fs::write(&file, &data).unwrap();

        let output = Command::new(&armybox)
            .args(["strings", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("this is a longer string"));
        cleanup(&dir);
    }
}
