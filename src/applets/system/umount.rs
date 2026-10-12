//! umount - unmount a filesystem
//!
//! Unmount a mounted filesystem.

use crate::applets::get_arg;
use crate::sys;

/// umount - unmount a filesystem
///
/// # Synopsis
/// ```text
/// umount target
/// ```
///
/// # Description
/// Unmount the filesystem mounted at the specified target directory.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn umount(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let target = unsafe { get_arg(argv, 1).unwrap() };
    let mut target_buf = [0u8; 4096];
    target_buf[..target.len()].copy_from_slice(target);

    let ret = unsafe { libc::umount(target_buf.as_ptr() as *const libc::c_char) };
    if ret != 0 {
        sys::perror(b"umount");
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_umount_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["umount"]).output().unwrap();

        // Should fail without arguments
        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_umount_not_mounted() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["umount", "/nonexistent/mount/point"])
            .output()
            .unwrap();

        // Should fail - not mounted
        assert_eq!(output.status.code(), Some(1));
    }
}
