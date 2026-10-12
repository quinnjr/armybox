//! modprobe - add and remove modules from the Linux kernel
//!
//! Intelligently add or remove modules from the kernel.

use super::insmod::insmod;

/// modprobe - add and remove modules from the Linux kernel
///
/// # Synopsis
/// ```text
/// modprobe module
/// ```
///
/// # Description
/// Add or remove modules from the kernel. This is a simple wrapper
/// around insmod.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn modprobe(argc: i32, argv: *const *const u8) -> i32 {
    // Simple modprobe - just try insmod
    insmod(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

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

    #[test]
    fn test_modprobe_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["modprobe"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
