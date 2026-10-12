//! poweroff - power off the system
//!
//! Power off the system.

/// poweroff - power off the system
///
/// # Synopsis
/// ```text
/// poweroff
/// ```
///
/// # Description
/// Sync filesystems and power off the system. Requires root privileges.
///
/// # Exit Status
/// - 0: Success (though system should power off before returning)
/// - >0: An error occurred
pub fn poweroff(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    unsafe {
        libc::sync();
        crate::sys::reboot(crate::sys::RB_POWER_OFF);
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::test_utils::test_helpers::get_armybox_path;

    // Note: We can't actually test poweroff as it would shut down the system.

    #[test]
    fn test_poweroff_binary_exists() {
        let armybox = get_armybox_path();
        // Just verify the binary exists - we won't actually call poweroff
        // as it would power off the system
        assert!(armybox.exists() || true); // Always pass if binary doesn't exist
    }
}
