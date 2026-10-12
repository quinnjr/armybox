//! arp - manipulate the system ARP cache
//!
//! Display and modify the ARP cache.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// arp - manipulate the system ARP cache
///
/// # Synopsis
/// ```text
/// arp [-a] [-n] [-v] [-i interface]
/// ```
///
/// # Description
/// Display the system ARP cache from /proc/net/arp.
///
/// # Options
/// - `-a`: BSD style output (default)
/// - `-n`: Don't resolve hostnames (numeric)
/// - `-v`: Verbose output
/// - `-i iface`: Show only entries for specified interface
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn arp(argc: i32, argv: *const *const u8) -> i32 {
    let mut numeric = false;
    let mut verbose = false;
    let mut filter_iface: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-n" {
            numeric = true;
        } else if arg == b"-v" {
            verbose = true;
        } else if arg == b"-a" {
            // Default BSD style, ignore
        } else if arg == b"-i" {
            i += 1;
            filter_iface = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: arp [-a] [-n] [-v] [-i interface]\n");
            io::write_str(1, b"  -a  BSD style output\n");
            io::write_str(1, b"  -n  Don't resolve names\n");
            io::write_str(1, b"  -v  Verbose\n");
            io::write_str(1, b"  -i  Show only specified interface\n");
            return 0;
        }
        i += 1;
    }

    let _ = numeric; // Always numeric for now
    let _ = verbose;

    // Print header matching system arp format
    io::write_str(
        1,
        b"Address                  HWtype  HWaddress           Flags Mask            Iface\n",
    );

    let fd = io::open(b"/proc/net/arp", libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"arp: cannot open /proc/net/arp\n");
        return 1;
    }

    let content = io::read_all(fd);
    io::close(fd);

    for (idx, line) in content.split(|&c| c == b'\n').enumerate() {
        if idx == 0 || line.is_empty() {
            continue; // Skip header and empty lines
        }

        // Parse /proc/net/arp format:
        // IP address       HW type     Flags       HW address            Mask     Device
        // 192.168.1.1      0x1         0x2         00:11:22:33:44:55     *        eth0
        let fields: Vec<&[u8]> = line
            .split(|&c| c == b' ')
            .filter(|s| !s.is_empty())
            .collect();

        if fields.len() < 6 {
            continue;
        }

        let ip_addr = fields[0];
        let hw_type = fields[1];
        let flags = fields[2];
        let hw_addr = fields[3];
        let mask = fields[4];
        let iface = fields[5];

        // Filter by interface if specified
        if let Some(filter) = filter_iface {
            if iface != filter {
                continue;
            }
        }

        // IP address (left-aligned, 24 chars)
        io::write_all(1, ip_addr);
        pad_to(1, ip_addr.len(), 25);

        // HW type - convert to human-readable
        let hw_type_str = match hw_type {
            b"0x1" => b"ether" as &[u8],
            b"0x6" => b"ieee802",
            b"0x17" => b"ax25",
            b"0x19" => b"arcnet",
            _ => hw_type,
        };
        io::write_all(1, hw_type_str);
        pad_to(1, hw_type_str.len(), 8);

        // HW address (17 chars for MAC + padding)
        io::write_all(1, hw_addr);
        pad_to(1, hw_addr.len(), 20);

        // Flags - decode
        let flags_char = decode_arp_flags(flags);
        io::write_all(1, &[flags_char]);
        io::write_str(1, b"     ");

        // Mask
        io::write_all(1, mask);
        pad_to(1, mask.len(), 16);

        // Interface
        io::write_all(1, iface);
        io::write_str(1, b"\n");
    }

    0
}

/// Decode ARP flags to character
fn decode_arp_flags(flags: &[u8]) -> u8 {
    // Parse hex flags
    let val = if flags.starts_with(b"0x") {
        parse_hex(&flags[2..])
    } else {
        sys::parse_u64(flags).unwrap_or(0) as u32
    };

    // ATF_COM (complete) = 0x02, ATF_PERM = 0x04, ATF_PUBL = 0x08
    if val & 0x04 != 0 {
        b'M' // Permanent (manual)
    } else if val & 0x02 != 0 {
        b'C' // Complete
    } else {
        b' '
    }
}

/// Parse hex number
fn parse_hex(s: &[u8]) -> u32 {
    let mut result: u32 = 0;
    for &c in s {
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => break,
        };
        result = result.wrapping_mul(16).wrapping_add(digit as u32);
    }
    result
}

/// Write padding spaces to reach target column
fn pad_to(fd: i32, current: usize, target: usize) {
    for _ in current..target {
        io::write_str(fd, b" ");
    }
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
    fn test_arp_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["arp"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Address"));
        assert!(stdout.contains("HWtype"));
    }

    #[test]
    fn test_arp_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["arp", "-h"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
