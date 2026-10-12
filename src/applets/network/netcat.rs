//! netcat - TCP/IP swiss army knife
//!
//! Alias for nc.

use super::nc::nc;

/// netcat - TCP/IP swiss army knife
///
/// # Synopsis
/// ```text
/// netcat HOST PORT
/// ```
///
/// # Description
/// Alias for the nc command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn netcat(argc: i32, argv: *const *const u8) -> i32 {
    nc(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_netcat_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["netcat"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
