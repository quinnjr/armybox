//! traceroute6 - print the route packets trace to network host (IPv6)
//!
//! Alias for traceroute.

use super::traceroute::traceroute;

/// traceroute6 - print the route packets trace to network host (IPv6)
///
/// # Synopsis
/// ```text
/// traceroute6 HOST
/// ```
///
/// # Description
/// Alias for the traceroute command.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn traceroute6(argc: i32, argv: *const *const u8) -> i32 {
    traceroute(argc, argv)
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
    fn test_traceroute6_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["traceroute6"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
