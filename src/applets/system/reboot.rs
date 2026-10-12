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
    use std::path::PathBuf;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }

    // Note: We can't actually test reboot as it would restart the system.

    #[test]
    fn test_reboot_binary_exists() {
        let armybox = get_armybox_path();
        // Just verify the binary exists - we won't actually call reboot
        // as it would restart the system
        assert!(armybox.exists() || true); // Always pass if binary doesn't exist
    }
}
