//! nslookup - query DNS servers
//!
//! Query DNS servers interactively.

use super::host::host;

/// nslookup - query DNS servers
///
/// # Synopsis
/// ```text
/// nslookup HOSTNAME
/// ```
///
/// # Description
/// Query DNS servers for hostname information.
/// Wraps the host command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn nslookup(argc: i32, argv: *const *const u8) -> i32 {
    host(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_nslookup_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nslookup"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_nslookup_localhost() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nslookup", "localhost"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
