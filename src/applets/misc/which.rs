//! which - locate a command
//!
//! Shows the full path of shell commands.

use super::get_arg;
use crate::io;

/// which - locate a command
///
/// # Synopsis
/// ```text
/// which COMMAND
/// ```
///
/// # Description
/// Shows the full path of (shell) commands.
///
/// # Exit Status
/// - 0: Found
/// - 1: Not found
pub fn which(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }

    let cmd = unsafe { get_arg(argv, 1).unwrap() };
    let path_env = unsafe { libc::getenv(b"PATH\0".as_ptr() as *const libc::c_char) };

    if path_env.is_null() {
        return 1;
    }

    let path = unsafe { io::cstr_to_slice(path_env as *const u8) };

    for dir in path.split(|&c| c == b':') {
        let mut full_path = [0u8; 512];
        let mut len = 0;
        for &c in dir {
            full_path[len] = c;
            len += 1;
        }
        full_path[len] = b'/';
        len += 1;
        for &c in cmd {
            full_path[len] = c;
            len += 1;
        }

        if unsafe { libc::access(full_path.as_ptr() as *const libc::c_char, libc::X_OK) } == 0 {
            io::write_all(1, &full_path[..len]);
            io::write_str(1, b"\n");
            return 0;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_which_ls() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["which", "ls"])
            .output()
            .unwrap();

        // ls should exist in PATH
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("/ls"));
    }

    #[test]
    fn test_which_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["which", "nonexistent_command_12345"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_which_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["which"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
