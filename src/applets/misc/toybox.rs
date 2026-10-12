//! toybox - toybox compatibility
//!
//! Prints a message indicating toybox compatibility.

use crate::io;

/// toybox - toybox compatibility
///
/// # Synopsis
/// ```text
/// toybox
/// ```
///
/// # Description
/// Prints a message indicating toybox compatibility.
///
/// # Exit Status
/// - 0: Success
pub fn toybox(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"armybox (toybox compatible)\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_toybox() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["toybox"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
