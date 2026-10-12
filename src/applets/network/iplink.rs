//! iplink - show/manipulate network devices
//!
//! Alias for 'ip link'.

use super::ip::ip;

/// iplink - show/manipulate network devices
///
/// # Synopsis
/// ```text
/// iplink [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip link' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iplink(argc: i32, argv: *const *const u8) -> i32 {
    ip(argc, argv)
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
    fn test_iplink_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iplink"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
