//! Boolean utilities - true, false, colon
//!
//! Simple utilities that return fixed exit codes.

/// true - return true (exit 0)
pub fn r#true(_argc: i32, _argv: *const *const u8) -> i32 {
    0
}

/// false - return false (exit 1)
pub fn r#false(_argc: i32, _argv: *const *const u8) -> i32 {
    1
}

/// colon - null command (exit 0)
pub fn colon(_argc: i32, _argv: *const *const u8) -> i32 {
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_true_returns_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["true"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_false_returns_one() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["false"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
