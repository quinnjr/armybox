//! ip - show/manipulate routing, devices, policy routing and tunnels
//!
//! Show/manipulate network configuration.

use super::arp::arp;
use super::get_arg;
use super::ifconfig::ifconfig;
use super::route::route;
use crate::io;

/// ip - show/manipulate routing, devices, policy routing and tunnels
///
/// # Synopsis
/// ```text
/// ip OBJECT { COMMAND }
/// ```
///
/// # Description
/// Show or manipulate routing, network devices, interfaces and tunnels.
///
/// # Objects
/// - link: Network device
/// - addr: Protocol address on a device
/// - route: Routing table entry
/// - neigh: ARP/NDISC cache entry
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ip(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"Usage: ip OBJECT { COMMAND }\n");
        io::write_str(2, b"OBJECT := { link | addr | route | neigh }\n");
        return 1;
    }

    let obj = unsafe { get_arg(argv, 1).unwrap() };

    if obj == b"link" || obj == b"l" {
        return ifconfig(1, argv);
    }

    if obj == b"addr" || obj == b"a" {
        return ifconfig(1, argv);
    }

    if obj == b"route" || obj == b"r" {
        return route(1, argv);
    }

    if obj == b"neigh" || obj == b"n" {
        return arp(1, argv);
    }

    io::write_str(2, b"ip: unknown object\n");
    1
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
    fn test_ip_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ip"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage") || stderr.contains("OBJECT"));
    }

    #[test]
    fn test_ip_link() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ip", "link"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_ip_addr() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ip", "addr"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_ip_route() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ip", "route"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
