//! ss - socket statistics
//!
//! Display socket statistics.

use super::netstat::netstat;

/// ss - socket statistics
///
/// # Synopsis
/// ```text
/// ss [OPTIONS]
/// ```
///
/// # Description
/// Display socket statistics similar to netstat.
/// Alias for the netstat command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ss(argc: i32, argv: *const *const u8) -> i32 {
    netstat(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ss_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ss"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Active Internet connections"));
    }
}
