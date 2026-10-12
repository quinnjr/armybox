//! ipaddr - show/manipulate IP addresses
//!
//! Alias for 'ip addr'.

use super::ip::ip;

/// ipaddr - show/manipulate IP addresses
///
/// # Synopsis
/// ```text
/// ipaddr [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip addr' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ipaddr(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ipaddr_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ipaddr"]).output().unwrap();

        // Shows usage when no args
        assert_eq!(output.status.code(), Some(1));
    }
}
