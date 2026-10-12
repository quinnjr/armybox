//! iproute - show/manipulate routing table
//!
//! Alias for 'ip route'.

use super::ip::ip;

/// iproute - show/manipulate routing table
///
/// # Synopsis
/// ```text
/// iproute [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip route' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iproute(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_iproute_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iproute"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
