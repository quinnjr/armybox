//! swapoff - disable devices and files for paging and swapping
//!
//! Disable swap on a device or file.

use crate::applets::get_arg;
use crate::sys;

/// swapoff - disable devices and files for paging and swapping
///
/// # Synopsis
/// ```text
/// swapoff device
/// ```
///
/// # Description
/// Disable swapping on the specified device or file. Requires root.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn swapoff(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let path = unsafe { get_arg(argv, 1).unwrap() };
    let mut path_buf = [0u8; 4096];
    path_buf[..path.len()].copy_from_slice(path);

    if unsafe { libc::swapoff(path_buf.as_ptr() as *const libc::c_char) } != 0 {
        sys::perror(b"swapoff");
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_swapoff_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["swapoff"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_swapoff_invalid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["swapoff", "/nonexistent"])
            .output()
            .unwrap();

        // Should fail - not a swap device
        assert_eq!(output.status.code(), Some(1));
    }
}
