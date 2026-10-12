//! ipneigh - show/manipulate neighbor/ARP cache
//!
//! Alias for 'ip neigh'.

use super::ip::ip;

/// ipneigh - show/manipulate neighbor/ARP cache
///
/// # Synopsis
/// ```text
/// ipneigh [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip neigh' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ipneigh(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ipneigh_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ipneigh"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
