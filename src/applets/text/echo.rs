//! echo - display a line of text
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/echo.html

use crate::applets::get_arg;
use crate::io;

/// echo - display a line of text
///
/// # Synopsis
/// ```text
/// echo [-n] [string...]
/// ```
///
/// # Description
/// Write arguments to standard output, separated by spaces, followed by a newline.
///
/// # Options
/// - `-n`: Do not output the trailing newline
///
/// # Exit Status
/// - 0: Success (always)
pub fn echo(argc: i32, argv: *const *const u8) -> i32 {
    let mut newline = true;
    let mut start = 1;

    if argc > 1 {
        if let Some(arg) = unsafe { get_arg(argv, 1) } {
            if arg == b"-n" {
                newline = false;
                start = 2;
            }
        }
    }

    for i in start..argc {
        if i > start {
            io::write_str(1, b" ");
        }
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            io::write_all(1, arg);
        }
    }

    if newline {
        io::write_str(1, b"\n");
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_echo_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["echo", "hello", "world"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello world\n"
        );
    }

    #[test]
    fn test_echo_no_newline() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["echo", "-n", "hello"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello"
        );
    }

    #[test]
    fn test_echo_empty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["echo"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(std::string::String::from_utf8_lossy(&output.stdout), "\n");
    }

    #[test]
    fn test_echo_special_chars() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["echo", "hello\tworld"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.len() > 0);
    }
}
