//! printenv - print environment variables
//!
//! Print the values of the specified environment variables.

use super::env::env;
use crate::applets::get_arg;
use crate::io;

/// printenv - print environment variables
///
/// # Synopsis
/// ```text
/// printenv [name...]
/// ```
///
/// # Description
/// Print the values of the specified environment variables.
/// If no variables are specified, print all environment variables.
///
/// # Exit Status
/// - 0: Success
/// - 1: Variable not found
/// - >1: An error occurred
pub fn printenv(argc: i32, argv: *const *const u8) -> i32 {
    if argc > 1 {
        if let Some(name) = unsafe { get_arg(argv, 1) } {
            let val = unsafe { libc::getenv(name.as_ptr() as *const libc::c_char) };
            if !val.is_null() {
                io::write_all(1, unsafe { io::cstr_to_slice(val as *const u8) });
                io::write_str(1, b"\n");
                return 0;
            }
            return 1;
        }
    }
    env(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_printenv_path() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printenv", "PATH"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.trim().is_empty());
    }

    #[test]
    fn test_printenv_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printenv", "NONEXISTENT_VAR_12345"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_printenv_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["printenv"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should print all environment variables
        assert!(stdout.contains("PATH="));
    }
}
