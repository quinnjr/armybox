//! usleep - sleep for microseconds
//!
//! Suspend execution for a specified number of microseconds.

use crate::applets::get_arg;
use crate::sys;

/// usleep - sleep for microseconds
///
/// # Synopsis
/// ```text
/// usleep microseconds
/// ```
///
/// # Description
/// Suspend execution for the specified number of microseconds.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn usleep(argc: i32, argv: *const *const u8) -> i32 {
    if argc > 1 {
        if let Some(arg) = unsafe { get_arg(argv, 1) } {
            let usecs = sys::parse_u64(arg).unwrap_or(0) as u32;
            unsafe { libc::usleep(usecs) };
        }
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;
    use std::time::Instant;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_usleep_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let start = Instant::now();
        let output = Command::new(&armybox)
            .args(["usleep", "0"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should complete quickly
        assert!(start.elapsed().as_millis() < 500);
    }

    #[test]
    fn test_usleep_100ms() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let start = Instant::now();
        let output = Command::new(&armybox)
            .args(["usleep", "100000"]) // 100,000 microseconds = 100ms
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should take at least 100ms
        assert!(start.elapsed().as_millis() >= 90);
    }
}
