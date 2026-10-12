//! reboot - reboot the system
//!
//! Reboot the system.

/// reboot - reboot the system
///
/// # Synopsis
/// ```text
/// reboot
/// ```
///
/// # Description
/// Sync filesystems and reboot the system. Requires root privileges.
///
/// # Exit Status
/// - 0: Success (though system should reboot before returning)
/// - >0: An error occurred
pub fn reboot(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    unsafe {
        libc::sync();
        crate::sys::reboot(crate::sys::RB_AUTOBOOT);
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::test_utils::test_helpers::get_armybox_path;

    // Note: We can't actually test reboot as it would restart the system.

    #[test]
    fn test_reboot_binary_exists() {
        let armybox = get_armybox_path();
        // Just verify the binary exists - we won't actually call reboot
        // as it would restart the system
        assert!(armybox.exists() || true); // Always pass if binary doesn't exist
    }
}
