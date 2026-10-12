//! pivot_root - change the root filesystem
//!
//! Move the root filesystem to a new location.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// pivot_root - change the root filesystem
///
/// # Synopsis
/// ```text
/// pivot_root new_root put_old
/// ```
///
/// # Description
/// Move the root filesystem of the calling process to the directory put_old
/// and make new_root the new root filesystem.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn pivot_root(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"pivot_root: usage: pivot_root new_root put_old\n");
        return 1;
    }

    let new_root = unsafe { get_arg(argv, 1).unwrap() };
    let put_old = unsafe { get_arg(argv, 2).unwrap() };

    // Create null-terminated strings
    let mut new_root_buf = [0u8; 4096];
    let mut put_old_buf = [0u8; 4096];

    new_root_buf[..new_root.len()].copy_from_slice(new_root);
    put_old_buf[..put_old.len()].copy_from_slice(put_old);

    // Call pivot_root syscall
    if unsafe {
        libc::syscall(
            libc::SYS_pivot_root,
            new_root_buf.as_ptr(),
            put_old_buf.as_ptr(),
        )
    } != 0
    {
        sys::perror(b"pivot_root");
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
    fn test_pivot_root_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["pivot_root"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_pivot_root_invalid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Should fail with invalid paths (needs root and proper setup)
        let output = Command::new(&armybox)
            .args(["pivot_root", "/nonexistent", "/also_nonexistent"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
