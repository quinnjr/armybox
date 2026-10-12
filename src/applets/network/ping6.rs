//! ping6 - send ICMP6 ECHO_REQUEST to network hosts
//!
//! Send ICMPv6 ECHO_REQUEST packets to network hosts.

use super::ping::ping;

/// ping6 - send ICMP6 ECHO_REQUEST to network hosts
///
/// # Synopsis
/// ```text
/// ping6 [-c count] HOST
/// ```
///
/// # Description
/// Send ICMPv6 ECHO_REQUEST packets to a network host.
/// Currently wraps ping for IPv4.
///
/// # Exit Status
/// - 0: At least one response received
/// - 1: No response received or error
pub fn ping6(argc: i32, argv: *const *const u8) -> i32 {
    ping(argc, argv)
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
    fn test_ping6_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ping6"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
