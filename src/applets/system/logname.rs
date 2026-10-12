//! logname - print user's login name
//!
//! Print the name of the current user.

use super::whoami::whoami;

/// logname - print user's login name
///
/// # Synopsis
/// ```text
/// logname
/// ```
///
/// # Description
/// Print the name of the current user as found in the LOGNAME
/// environment variable or by other means.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn logname(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    whoami(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_logname_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["logname"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.trim().is_empty());
    }
}
