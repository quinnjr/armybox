//! rfkill - tool for enabling and disabling wireless devices
//!
//! Query and change the state of rfkill switches.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// rfkill - tool for enabling and disabling wireless devices
///
/// # Synopsis
/// ```text
/// rfkill [list|block|unblock] [type|id]
/// ```
///
/// # Description
/// Tool for enabling and disabling wireless devices.
///
/// # Commands
/// - list: List the current state of rfkill switches
/// - block [type|id]: Disable a type of wireless device or specific device
/// - unblock [type|id]: Enable a type of wireless device or specific device
///
/// # Types
/// - wifi, wlan: Wireless LAN
/// - bluetooth: Bluetooth
/// - uwb: Ultra-Wideband
/// - wimax: Worldwide Interoperability for Microwave Access
/// - wwan: Wireless Wide Area Network
/// - gps: GPS
/// - fm: FM
/// - nfc: Near Field Communication
/// - all: All devices
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn rfkill(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        // Default to list
        return rfkill_list();
    }

    let cmd = match unsafe { get_arg(argv, 1) } {
        Some(c) => c,
        None => return rfkill_list(),
    };

    if cmd == b"list" {
        rfkill_list()
    } else if cmd == b"block" {
        if argc < 3 {
            io::write_str(2, b"rfkill: missing type/id\n");
            return 1;
        }
        let type_arg = unsafe { get_arg(argv, 2).unwrap() };
        rfkill_set(type_arg, true)
    } else if cmd == b"unblock" {
        if argc < 3 {
            io::write_str(2, b"rfkill: missing type/id\n");
            return 1;
        }
        let type_arg = unsafe { get_arg(argv, 2).unwrap() };
        rfkill_set(type_arg, false)
    } else if cmd == b"-h" || cmd == b"--help" {
        print_help();
        0
    } else {
        io::write_str(2, b"rfkill: unknown command: ");
        io::write_all(2, cmd);
        io::write_str(2, b"\n");
        1
    }
}

fn print_help() {
    io::write_str(1, b"Usage: rfkill [command] [type|id]\n\n");
    io::write_str(1, b"Commands:\n");
    io::write_str(1, b"  list           List all devices\n");
    io::write_str(1, b"  block TYPE     Block devices of TYPE\n");
    io::write_str(1, b"  unblock TYPE   Unblock devices of TYPE\n\n");
    io::write_str(1, b"Types: wifi, bluetooth, wwan, gps, nfc, all\n");
}

fn rfkill_list() -> i32 {
    // Read from /sys/class/rfkill
    let dir = io::opendir(b"/sys/class/rfkill");
    if dir.is_null() {
        io::write_str(1, b"No rfkill devices found\n");
        return 0;
    }

    io::write_str(1, b"ID TYPE      DEVICE           SOFT      HARD\n");

    loop {
        let entry = io::readdir(dir);
        if entry.is_null() {
            break;
        }

        let d_name = unsafe { &(*entry).d_name };
        let name_len = d_name.iter().position(|&c| c == 0).unwrap_or(d_name.len());
        let name = &d_name[..name_len];
        let name_u8: &[u8] = unsafe { core::mem::transmute(name) };

        // Skip . and ..
        if name_u8 == b"." || name_u8 == b".." {
            continue;
        }

        // Build base path
        let mut base_path = [0u8; 256];
        let mut pi = 0;
        for &c in b"/sys/class/rfkill/" {
            base_path[pi] = c;
            pi += 1;
        }
        for &c in name_u8 {
            base_path[pi] = c;
            pi += 1;
        }
        let base_len = pi;

        // Read type
        base_path[base_len] = b'/';
        for (i, &c) in b"type".iter().enumerate() {
            base_path[base_len + 1 + i] = c;
        }
        let type_str = read_sysfs_string(&base_path[..base_len + 5]);

        // Read name/device
        for (i, &c) in b"name".iter().enumerate() {
            base_path[base_len + 1 + i] = c;
        }
        let device_str = read_sysfs_string(&base_path[..base_len + 5]);

        // Read soft block state
        for (i, &c) in b"soft".iter().enumerate() {
            base_path[base_len + 1 + i] = c;
        }
        let soft_str = read_sysfs_string(&base_path[..base_len + 5]);
        let soft_blocked = soft_str.first() == Some(&b'1');

        // Read hard block state
        for (i, &c) in b"hard".iter().enumerate() {
            base_path[base_len + 1 + i] = c;
        }
        let hard_str = read_sysfs_string(&base_path[..base_len + 5]);
        let hard_blocked = hard_str.first() == Some(&b'1');

        // Print entry: ID TYPE DEVICE SOFT HARD
        // Extract ID number from name (e.g., "rfkill0" -> "0")
        let id = if name_u8.starts_with(b"rfkill") {
            &name_u8[6..]
        } else {
            name_u8
        };

        // ID (right-aligned in 2 chars)
        if id.len() < 2 {
            io::write_str(1, b" ");
        }
        io::write_all(1, id);
        io::write_str(1, b" ");

        // Type (left-aligned, 9 chars)
        io::write_all(1, &type_str);
        for _ in type_str.len()..10 {
            io::write_str(1, b" ");
        }

        // Device name (left-aligned, 16 chars)
        let dev_name = if device_str.is_empty() {
            name_u8
        } else {
            &device_str
        };
        io::write_all(1, dev_name);
        for _ in dev_name.len()..17 {
            io::write_str(1, b" ");
        }

        // Soft block state
        let soft_state: &[u8] = if soft_blocked {
            b"blocked"
        } else {
            b"unblocked"
        };
        io::write_all(1, soft_state);
        for _ in soft_state.len()..10 {
            io::write_str(1, b" ");
        }

        // Hard block state
        let hard_state: &[u8] = if hard_blocked {
            b"blocked"
        } else {
            b"unblocked"
        };
        io::write_all(1, hard_state);

        io::write_str(1, b"\n");
    }

    io::closedir(dir);
    0
}

/// Read a sysfs string file (strips trailing newline)
fn read_sysfs_string(path: &[u8]) -> Vec<u8> {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return Vec::new();
    }

    let mut buf = [0u8; 64];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return Vec::new();
    }

    let len = if n > 0 && buf[n as usize - 1] == b'\n' {
        n as usize - 1
    } else {
        n as usize
    };

    buf[..len].to_vec()
}

fn rfkill_set(type_arg: &[u8], block: bool) -> i32 {
    // Normalize type name
    let target_type: Option<&[u8]> = match type_arg {
        b"wifi" | b"wlan" => Some(b"wlan"),
        b"bluetooth" => Some(b"bluetooth"),
        b"wwan" => Some(b"wwan"),
        b"gps" => Some(b"gps"),
        b"nfc" => Some(b"nfc"),
        b"uwb" => Some(b"uwb"),
        b"wimax" => Some(b"wimax"),
        b"fm" => Some(b"fm"),
        b"all" => None, // Match all devices
        _ => {
            // Could be a numeric ID, try that
            if sys::parse_u64(type_arg).is_some() {
                // Treat as device ID
                return set_device_by_id(type_arg, block);
            }
            io::write_str(2, b"rfkill: unknown type: ");
            io::write_all(2, type_arg);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Iterate over all rfkill devices
    let dir = io::opendir(b"/sys/class/rfkill");
    if dir.is_null() {
        io::write_str(2, b"rfkill: cannot access /sys/class/rfkill\n");
        return 1;
    }

    let mut found = false;
    let value = if block { b"1" } else { b"0" };

    loop {
        let entry = io::readdir(dir);
        if entry.is_null() {
            break;
        }

        let d_name = unsafe { &(*entry).d_name };
        let name_len = d_name.iter().position(|&c| c == 0).unwrap_or(d_name.len());
        let name = &d_name[..name_len];
        let name_u8: &[u8] = unsafe { core::mem::transmute(name) };

        if name_u8 == b"." || name_u8 == b".." {
            continue;
        }

        // Build path to type file
        let mut type_path = [0u8; 256];
        let mut pi = 0;
        for &c in b"/sys/class/rfkill/" {
            type_path[pi] = c;
            pi += 1;
        }
        for &c in name_u8 {
            type_path[pi] = c;
            pi += 1;
        }
        let base_len = pi;
        type_path[base_len] = b'/';
        for (i, &c) in b"type".iter().enumerate() {
            type_path[base_len + 1 + i] = c;
        }

        // Read device type
        let device_type = read_sysfs_string(&type_path[..base_len + 5]);

        // Check if we should operate on this device
        let should_set = match target_type {
            Some(t) => device_type == t,
            None => true, // "all" matches everything
        };

        if should_set {
            // Write to soft file
            for (i, &c) in b"soft".iter().enumerate() {
                type_path[base_len + 1 + i] = c;
            }

            let fd = io::open(&type_path[..base_len + 5], libc::O_WRONLY, 0);
            if fd >= 0 {
                io::write_all(fd, value);
                io::close(fd);
                found = true;
            }
        }
    }

    io::closedir(dir);

    if !found && target_type.is_some() {
        io::write_str(2, b"rfkill: no devices of type ");
        io::write_all(2, type_arg);
        io::write_str(2, b" found\n");
        return 1;
    }

    0
}

fn set_device_by_id(id: &[u8], block: bool) -> i32 {
    let mut path = [0u8; 256];
    let mut pi = 0;

    for &c in b"/sys/class/rfkill/rfkill" {
        path[pi] = c;
        pi += 1;
    }
    for &c in id {
        path[pi] = c;
        pi += 1;
    }
    for &c in b"/soft" {
        path[pi] = c;
        pi += 1;
    }

    let fd = io::open(&path[..pi], libc::O_WRONLY, 0);
    if fd < 0 {
        io::write_str(2, b"rfkill: device ");
        io::write_all(2, id);
        io::write_str(2, b" not found\n");
        return 1;
    }

    let value = if block { b"1" } else { b"0" };
    io::write_all(fd, value);
    io::close(fd);

    0
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
    fn test_rfkill_list() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rfkill", "list"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("TYPE") || stdout.contains("No rfkill"));
    }

    #[test]
    fn test_rfkill_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["rfkill"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_rfkill_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rfkill", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
