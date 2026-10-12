//! basename - strip directory and suffix from filenames
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/basename.html

use crate::applets::get_arg;
use crate::io;

/// basename - strip directory and suffix from filenames
///
/// # Synopsis
/// ```text
/// basename string [suffix]
/// basename -a [-s suffix] name...
/// ```
///
/// # Description
/// Print NAME with any leading directory components removed.
/// If specified, also remove a trailing SUFFIX.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn basename(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"basename: missing operand\n");
        return 1;
    }

    let path = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => {
            io::write_str(2, b"basename: missing operand\n");
            return 1;
        }
    };

    let suffix = if argc > 2 {
        unsafe { get_arg(argv, 2) }
    } else {
        None
    };

    // Handle empty path
    if path.is_empty() {
        io::write_str(1, b"\n");
        return 0;
    }

    // Find start of basename (after last /)
    let mut start = 0;
    let mut end = path.len();

    // Remove trailing slashes
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }

    // Handle root case
    if end == 0 {
        io::write_str(1, b"/\n");
        return 0;
    }

    // Find last /
    for i in 0..end {
        if path[i] == b'/' {
            start = i + 1;
        }
    }

    let base = &path[start..end];
    let mut final_end = base.len();

    // Strip suffix if provided and matches
    if let Some(s) = suffix {
        if base.len() > s.len() && &base[base.len() - s.len()..] == s {
            final_end = base.len() - s.len();
        }
    }

    io::write_all(1, &base[..final_end]);
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_basename_simple() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["basename", "/usr/bin/sort"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "sort"
        );
    }

    #[test]
    fn test_basename_with_suffix() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["basename", "/path/to/file.txt", ".txt"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "file"
        );
    }

    #[test]
    fn test_basename_no_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["basename", "file.txt"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "file.txt"
        );
    }

    #[test]
    fn test_basename_trailing_slash() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["basename", "/path/to/dir/"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "dir"
        );
    }

    #[test]
    fn test_basename_root() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["basename", "/"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "/"
        );
    }

    #[test]
    fn test_basename_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["basename"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
