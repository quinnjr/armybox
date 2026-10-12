//! iplink - show/manipulate network devices
//!
//! Alias for 'ip link'.

use super::ip::ip;

/// iplink - show/manipulate network devices
///
/// # Synopsis
/// ```text
/// iplink [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip link' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iplink(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_iplink_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iplink"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
