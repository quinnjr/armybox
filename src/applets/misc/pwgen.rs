//! pwgen - generate random passwords
//!
//! Generate pronounceable or secure passwords.

use super::get_arg;
use crate::io;
use crate::sys;

/// pwgen - generate pronounceable passwords
///
/// # Synopsis
/// ```text
/// pwgen [-s] [length] [count]
/// ```
///
/// # Description
/// Generates random passwords using characters from /dev/urandom.
///
/// # Options
/// - `-s, --secure`: Include symbols in passwords
/// - `-h, --help`: Show help
///
/// # Arguments
/// - `length`: Password length (default: 8)
/// - `count`: Number of passwords to generate (default: 1)
///
/// # Exit Status
/// - 0: Success
pub fn pwgen(argc: i32, argv: *const *const u8) -> i32 {
    const DEFAULT_LENGTH: usize = 8;
    const DEFAULT_COUNT: usize = 1;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    let mut length = DEFAULT_LENGTH;
    let mut count = DEFAULT_COUNT;
    let mut secure = false;

    // Parse arguments
    let mut positional = 0;
    for i in 1..argc as usize {
        if let Some(arg) = unsafe { get_arg(argv, i as i32) } {
            if arg == b"-s" || arg == b"--secure" {
                secure = true;
            } else if arg == b"-h" || arg == b"--help" {
                io::write_str(1, b"Usage: pwgen [-s] [length] [count]\n");
                io::write_str(1, b"  -s  Include symbols\n");
                return 0;
            } else if let Some(n) = sys::parse_u64(arg) {
                if positional == 0 {
                    length = n as usize;
                    if length == 0 {
                        length = DEFAULT_LENGTH;
                    }
                } else {
                    count = n as usize;
                    if count == 0 {
                        count = 1;
                    }
                }
                positional += 1;
            }
        }
    }

    let charset: &[u8] = if secure {
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-="
    } else {
        CHARS
    };

    // Read random bytes
    let fd = io::open(b"/dev/urandom", libc::O_RDONLY, 0);
    let use_urandom = fd >= 0;

    // Fallback PRNG state
    let mut rng = if !use_urandom {
        let t = unsafe { libc::time(core::ptr::null_mut()) } as u64;
        let pid = unsafe { libc::getpid() } as u64;
        t ^ (pid << 16)
    } else {
        0
    };

    for pw_idx in 0..count {
        for _ in 0..length {
            let rand_byte = if use_urandom {
                let mut b = [0u8; 1];
                io::read(fd, &mut b);
                b[0]
            } else {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                (rng >> 56) as u8
            };
            io::write_all(1, &[charset[rand_byte as usize % charset.len()]]);
        }

        // Print space between passwords or newline at end
        if pw_idx + 1 < count {
            io::write_str(1, b" ");
        }
    }
    io::write_str(1, b"\n");

    if use_urandom {
        io::close(fd);
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_pwgen() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["pwgen"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().len() >= 8);
    }

    #[test]
    fn test_pwgen_length() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["pwgen", "16"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim().len(), 16);
    }

    #[test]
    fn test_pwgen_count() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["pwgen", "8", "3"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let passwords: std::vec::Vec<_> = stdout.trim().split(' ').collect();
        assert_eq!(passwords.len(), 3);
    }
}
