//! dirname - strip last component from file name
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/dirname.html

use crate::applets::get_arg;
use crate::io;

/// dirname - strip last component from file name
///
/// # Synopsis
/// ```text
/// dirname string
/// ```
///
/// # Description
/// Output each NAME with its last non-slash component and trailing slashes
/// removed. If NAME contains no slashes, output '.' (the current directory).
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn dirname(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"dirname: missing operand\n");
        return 1;
    }

    let path = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => {
            io::write_str(2, b"dirname: missing operand\n");
            return 1;
        }
    };

    // Handle empty path
    if path.is_empty() {
        io::write_str(1, b".\n");
        return 0;
    }

    let mut end = path.len();

    // Remove trailing slashes
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }

    // Handle root case (all slashes)
    if end == 0 {
        io::write_str(1, b"/\n");
        return 0;
    }

    // Find last /
    let mut last_slash = None;
    for i in 0..end {
        if path[i] == b'/' {
            last_slash = Some(i);
        }
    }

    match last_slash {
        Some(0) => {
            // Path like /foo -> /
            io::write_str(1, b"/\n");
        }
        Some(i) => {
            // Remove trailing slashes from directory part
            let mut dir_end = i;
            while dir_end > 1 && path[dir_end - 1] == b'/' {
                dir_end -= 1;
            }
            io::write_all(1, &path[..dir_end]);
            io::write_str(1, b"\n");
        }
        None => {
            // No slash in path
            io::write_str(1, b".\n");
        }
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_dirname_simple() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dirname", "/usr/bin/sort"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "/usr/bin"
        );
    }

    #[test]
    fn test_dirname_no_slash() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dirname", "file.txt"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "."
        );
    }

    #[test]
    fn test_dirname_root_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dirname", "/file"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "/"
        );
    }

    #[test]
    fn test_dirname_root() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dirname", "/"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "/"
        );
    }

    #[test]
    fn test_dirname_trailing_slash() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dirname", "/path/to/dir/"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout).trim(),
            "/path/to"
        );
    }

    #[test]
    fn test_dirname_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["dirname"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
