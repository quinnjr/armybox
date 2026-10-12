//! seq - print sequence of numbers
//!
//! Non-POSIX utility (GNU coreutils).
//! Prints a sequence of numbers.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// seq - print a sequence of numbers
///
/// # Synopsis
/// ```text
/// seq [first [increment]] last
/// ```
///
/// # Description
/// Print numbers from FIRST to LAST, in steps of INCREMENT.
/// If FIRST or INCREMENT is omitted, it defaults to 1.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn seq(argc: i32, argv: *const *const u8) -> i32 {
    let (first, last, incr) = match argc {
        2 => {
            let last = sys::parse_i64(unsafe { get_arg(argv, 1).unwrap() }).unwrap_or(1);
            (1i64, last, 1i64)
        }
        3 => {
            let first = sys::parse_i64(unsafe { get_arg(argv, 1).unwrap() }).unwrap_or(1);
            let last = sys::parse_i64(unsafe { get_arg(argv, 2).unwrap() }).unwrap_or(1);
            (first, last, 1)
        }
        _ if argc >= 4 => {
            let first = sys::parse_i64(unsafe { get_arg(argv, 1).unwrap() }).unwrap_or(1);
            let incr = sys::parse_i64(unsafe { get_arg(argv, 2).unwrap() }).unwrap_or(1);
            let last = sys::parse_i64(unsafe { get_arg(argv, 3).unwrap() }).unwrap_or(1);
            (first, last, incr)
        }
        _ => (1, 10, 1),
    };

    let mut n = first;
    if incr > 0 {
        while n <= last {
            io::write_signed(1, n);
            io::write_str(1, b"\n");
            n += incr;
        }
    } else if incr < 0 {
        while n >= last {
            io::write_signed(1, n);
            io::write_str(1, b"\n");
            n += incr;
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
    fn test_seq_single_arg() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["seq", "5"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["1", "2", "3", "4", "5"]);
    }

    #[test]
    fn test_seq_two_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["seq", "3", "7"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["3", "4", "5", "6", "7"]);
    }

    #[test]
    fn test_seq_three_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["seq", "1", "2", "9"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["1", "3", "5", "7", "9"]);
    }

    #[test]
    fn test_seq_decreasing() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["seq", "5", "-1", "1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["5", "4", "3", "2", "1"]);
    }

    #[test]
    fn test_seq_single_number() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["seq", "1"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "1");
    }
}
