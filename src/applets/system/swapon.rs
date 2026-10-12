//! swapon - enable devices and files for paging and swapping
//!
//! Enable swap on a device or file.

use crate::applets::get_arg;
use crate::sys;

/// swapon - enable devices and files for paging and swapping
///
/// # Synopsis
/// ```text
/// swapon device
/// ```
///
/// # Description
/// Enable swapping on the specified device or file. Requires root.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn swapon(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let path = unsafe { get_arg(argv, 1).unwrap() };
    let mut path_buf = [0u8; 4096];
    path_buf[..path.len()].copy_from_slice(path);

    if unsafe { libc::swapon(path_buf.as_ptr() as *const libc::c_char, 0) } != 0 {
        sys::perror(b"swapon");
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
    fn test_swapon_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["swapon"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_swapon_invalid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["swapon", "/nonexistent"])
            .output()
            .unwrap();

        // Should fail - not a swap device or permission denied
        assert_eq!(output.status.code(), Some(1));
    }
}
