//! iproute - show/manipulate routing table
//!
//! Alias for 'ip route'.

use super::ip::ip;

/// iproute - show/manipulate routing table
///
/// # Synopsis
/// ```text
/// iproute [OPTIONS]
/// ```
///
/// # Description
/// Alias for 'ip route' command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iproute(argc: i32, argv: *const *const u8) -> i32 {
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
    fn test_iproute_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iproute"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
