//! ipcalc - IP address calculator
//!
//! Calculate IP network information.

use super::get_arg;
use crate::io;

/// ipcalc - IP address calculator
///
/// # Synopsis
/// ```text
/// ipcalc ADDRESS[/PREFIX]
/// ```
///
/// # Description
/// Calculate and display IP network information including
/// network address, broadcast address, and host range.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ipcalc(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"ipcalc: missing address\n");
        return 1;
    }

    let addr = unsafe { get_arg(argv, 1).unwrap() };

    // Parse address and optional prefix
    let (ip_part, prefix) = if let Some(pos) = addr.iter().position(|&c| c == b'/') {
        (&addr[..pos], Some(&addr[pos + 1..]))
    } else {
        (addr, None)
    };

    // Parse IP address
    let mut octets = [0u32; 4];
    let mut octet_idx = 0;
    let mut current = 0u32;

    for &c in ip_part {
        if c == b'.' {
            if octet_idx >= 4 {
                io::write_str(2, b"ipcalc: invalid address\n");
                return 1;
            }
            octets[octet_idx] = current;
            octet_idx += 1;
            current = 0;
        } else if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u32;
        } else {
            io::write_str(2, b"ipcalc: invalid address\n");
            return 1;
        }
    }

    if octet_idx != 3 {
        io::write_str(2, b"ipcalc: invalid address\n");
        return 1;
    }
    octets[3] = current;

    // Validate octets
    for o in &octets {
        if *o > 255 {
            io::write_str(2, b"ipcalc: invalid address\n");
            return 1;
        }
    }

    let ip = ((octets[0] as u32) << 24)
        | ((octets[1] as u32) << 16)
        | ((octets[2] as u32) << 8)
        | (octets[3] as u32);

    // Parse prefix length
    let prefix_len = if let Some(p) = prefix {
        let mut val = 0u32;
        for &c in p {
            if c >= b'0' && c <= b'9' {
                val = val * 10 + (c - b'0') as u32;
            }
        }
        if val > 32 {
            io::write_str(2, b"ipcalc: invalid prefix\n");
            return 1;
        }
        val
    } else {
        // Default prefix based on class
        if octets[0] < 128 {
            8
        }
        // Class A
        else if octets[0] < 192 {
            16
        }
        // Class B
        else {
            24
        } // Class C
    };

    let mask = if prefix_len == 0 {
        0
    } else {
        !0u32 << (32 - prefix_len)
    };
    let network = ip & mask;
    let broadcast = network | !mask;

    // Output
    let mut num_buf = [0u8; 20];

    io::write_str(1, b"Address:   ");
    write_ip(ip, &mut num_buf);
    io::write_str(1, b"\n");

    io::write_str(1, b"Netmask:   ");
    write_ip(mask, &mut num_buf);
    io::write_str(1, b" = ");
    io::write_all(1, crate::sys::format_u64(prefix_len as u64, &mut num_buf));
    io::write_str(1, b"\n");

    io::write_str(1, b"Network:   ");
    write_ip(network, &mut num_buf);
    io::write_str(1, b"/");
    io::write_all(1, crate::sys::format_u64(prefix_len as u64, &mut num_buf));
    io::write_str(1, b"\n");

    io::write_str(1, b"Broadcast: ");
    write_ip(broadcast, &mut num_buf);
    io::write_str(1, b"\n");

    // Host range
    let hosts = if prefix_len >= 31 {
        0
    } else {
        (1u32 << (32 - prefix_len)) - 2
    };
    io::write_str(1, b"Hosts:     ");
    io::write_all(1, crate::sys::format_u64(hosts as u64, &mut num_buf));
    io::write_str(1, b"\n");

    0
}

fn write_ip(ip: u32, buf: &mut [u8; 20]) {
    io::write_all(1, crate::sys::format_u64(((ip >> 24) & 0xff) as u64, buf));
    io::write_str(1, b".");
    io::write_all(1, crate::sys::format_u64(((ip >> 16) & 0xff) as u64, buf));
    io::write_str(1, b".");
    io::write_all(1, crate::sys::format_u64(((ip >> 8) & 0xff) as u64, buf));
    io::write_str(1, b".");
    io::write_all(1, crate::sys::format_u64((ip & 0xff) as u64, buf));
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
    fn test_ipcalc_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ipcalc"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing address"));
    }

    #[test]
    fn test_ipcalc_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ipcalc", "192.168.1.100/24"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("192.168.1.100"));
        assert!(stdout.contains("Network:"));
        assert!(stdout.contains("Broadcast:"));
    }

    #[test]
    fn test_ipcalc_no_prefix() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ipcalc", "10.0.0.1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("10.0.0.1"));
    }

    #[test]
    fn test_ipcalc_invalid_address() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ipcalc", "999.999.999.999"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_ipcalc_slash_32() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ipcalc", "192.168.1.1/32"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Hosts:     0"));
    }
}
