//! rmmod - remove a module from the Linux kernel
//!
//! Remove a loadable module from the kernel.

use crate::applets::get_arg;
use crate::sys;

/// rmmod - remove a module from the Linux kernel
///
/// # Synopsis
/// ```text
/// rmmod module
/// ```
///
/// # Description
/// Remove a loadable module from the kernel. Requires root.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn rmmod(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let name = unsafe { get_arg(argv, 1).unwrap() };
    let mut name_buf = [0u8; 256];
    name_buf[..name.len()].copy_from_slice(name);

    if unsafe { libc::syscall(libc::SYS_delete_module, name_buf.as_ptr(), 0) } != 0 {
        sys::perror(b"rmmod");
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
    fn test_rmmod_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["rmmod"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_rmmod_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rmmod", "nonexistent_module"])
            .output()
            .unwrap();

        // Should fail - module doesn't exist or permission denied
        assert_eq!(output.status.code(), Some(1));
    }
}
