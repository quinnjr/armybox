//! fmt - simple text formatter
//!
//! Simple implementation that wraps lines similar to fold.

use super::fold::fold;

/// fmt - simple text formatter
///
/// # Synopsis
/// ```text
/// fmt [file...]
/// ```
///
/// # Description
/// Simple optimal text formatter. This implementation is a simple
/// wrapper around fold, providing basic line wrapping.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn fmt(argc: i32, argv: *const *const u8) -> i32 {
    fold(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_fmt_wraps_long_line() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["fmt"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            // 90 character line
            stdin.write_all(b"123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        // Default 80 chars, so should wrap
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_fmt_short_line() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["fmt"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"short line\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "short line\n");
    }
}
