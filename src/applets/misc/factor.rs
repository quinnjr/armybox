//! factor - factor numbers
//!
//! Print the prime factors of each number.

use super::get_arg;
use crate::io;
use crate::sys;

/// factor - factor numbers
///
/// # Synopsis
/// ```text
/// factor [NUMBER]...
/// ```
///
/// # Description
/// Print the prime factors of each NUMBER.
///
/// # Exit Status
/// - 0: Success
/// - 1: No arguments
pub fn factor(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            let mut n = sys::parse_u64(arg).unwrap_or(0);
            io::write_num(1, n);
            io::write_str(1, b":");

            let mut d = 2u64;
            while d * d <= n {
                while n % d == 0 {
                    io::write_str(1, b" ");
                    io::write_num(1, d);
                    n /= d;
                }
                d += 1;
            }
            if n > 1 {
                io::write_str(1, b" ");
                io::write_num(1, n);
            }
            io::write_str(1, b"\n");
        }
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
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
    fn test_factor_prime() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["factor", "17"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "17: 17");
    }

    #[test]
    fn test_factor_composite() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["factor", "12"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "12: 2 2 3");
    }

    #[test]
    fn test_factor_large() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["factor", "100"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "100: 2 2 5 5");
    }
}
