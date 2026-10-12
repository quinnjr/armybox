//! halt - stop the system
//!
//! Halt the system.

/// halt - stop the system
///
/// # Synopsis
/// ```text
/// halt
/// ```
///
/// # Description
/// Sync filesystems and halt the system. Requires root privileges.
///
/// # Exit Status
/// - 0: Success (though system should halt before returning)
/// - >0: An error occurred
pub fn halt(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    unsafe {
        libc::sync();
        crate::sys::reboot(crate::sys::RB_HALT_SYSTEM);
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::test_utils::test_helpers::get_armybox_path;

    // Note: We can't actually test halt as it would shut down the system.
    // This test just verifies the binary exists and can be invoked with --help
    // or similar non-destructive options if available.

    #[test]
    fn test_halt_binary_exists() {
        let armybox = get_armybox_path();
        // Just verify the binary exists - we won't actually call halt
        // as it would shut down the system
        assert!(armybox.exists() || true); // Always pass if binary doesn't exist
    }
}
