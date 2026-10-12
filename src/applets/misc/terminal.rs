//! Terminal utilities - clear, reset
//!
//! Terminal control utilities.

use crate::io;

/// clear - clear the terminal screen
pub fn clear(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"\x1b[H\x1b[2J");
    0
}

/// reset - reset the terminal
pub fn reset(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"\x1bc");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_clear_outputs_escape() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["clear"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Check for escape sequence
        assert!(output.stdout.starts_with(b"\x1b[H"));
    }

    #[test]
    fn test_reset_outputs_escape() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["reset"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.starts_with(b"\x1bc"));
    }
}
