//! iprule - show/manipulate routing policy rules
//!
//! Alias for 'ip rule'.

use super::ip::ip;

/// iprule - show/manipulate routing policy rules
///
/// # Synopsis
/// ```text
/// iprule [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip rule' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iprule(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_iprule_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iprule"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
