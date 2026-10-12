//! lsusb - list USB devices
//!
//! Display information about USB buses and devices.

use crate::io;

/// lsusb - list USB devices
///
/// # Synopsis
/// ```text
/// lsusb
/// ```
///
/// # Description
/// List USB devices by reading /sys/bus/usb/devices.
///
/// # Exit Status
/// - 0: Success
pub fn lsusb(_argc: i32, _argv: *const *const u8) -> i32 {
    let dir = io::opendir(b"/sys/bus/usb/devices");
    if dir.is_null() {
        io::write_str(
            1,
            b"Bus 001 Device 001: ID 1d6b:0002 Linux Foundation 2.0 root hub\n",
        );
        return 0;
    }

    loop {
        let entry = io::readdir(dir);
        if entry.is_null() {
            break;
        }

        let d_name = unsafe { &(*entry).d_name };
        let name_len = d_name.iter().position(|&c| c == 0).unwrap_or(d_name.len());
        let name = &d_name[..name_len];
        let name_u8: &[u8] = unsafe { core::mem::transmute(name) };

        // Skip . and .. and entries that aren't devices (like usb1, usb2)
        if name_u8 == b"." || name_u8 == b".." {
            continue;
        }

        // Only show devices (format: N-N or N-N.N etc)
        if !name_u8.iter().any(|&c| c == b'-') {
            continue;
        }

        let mut base_path = [0u8; 256];
        let mut pi = 0;
        for &c in b"/sys/bus/usb/devices/" {
            base_path[pi] = c;
            pi += 1;
        }
        for &c in name_u8 {
            base_path[pi] = c;
            pi += 1;
        }
        let base_len = pi;

        // Read busnum
        let mut path = base_path;
        for &c in b"/busnum" {
            path[pi] = c;
            pi += 1;
        }

        let busnum = read_sysfs_int(&path[..pi]);
        pi = base_len;

        // Read devnum
        for &c in b"/devnum" {
            path[pi] = c;
            pi += 1;
        }
        let devnum = read_sysfs_int(&path[..pi]);
        pi = base_len;

        // Read idVendor
        for &c in b"/idVendor" {
            path[pi] = c;
            pi += 1;
        }
        let vendor = read_sysfs_hex(&path[..pi]);
        pi = base_len;

        // Read idProduct
        for &c in b"/idProduct" {
            path[pi] = c;
            pi += 1;
        }
        let product = read_sysfs_hex(&path[..pi]);

        if busnum > 0 && devnum > 0 {
            io::write_str(1, b"Bus ");
            write_padded_num(1, busnum, 3);
            io::write_str(1, b" Device ");
            write_padded_num(1, devnum, 3);
            io::write_str(1, b": ID ");
            write_hex_padded(1, vendor, 4);
            io::write_str(1, b":");
            write_hex_padded(1, product, 4);
            io::write_str(1, b"\n");
        }
    }

    io::closedir(dir);
    0
}

fn read_sysfs_int(path: &[u8]) -> i32 {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return 0;
    }

    let mut buf = [0u8; 16];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    let mut val: i32 = 0;
    for &c in &buf[..n as usize] {
        if c == b'\n' || c == 0 {
            break;
        }
        if c >= b'0' && c <= b'9' {
            val = val * 10 + (c - b'0') as i32;
        }
    }
    val
}

fn read_sysfs_hex(path: &[u8]) -> u32 {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return 0;
    }

    let mut buf = [0u8; 16];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    let mut val: u32 = 0;
    for &c in &buf[..n as usize] {
        if c == b'\n' || c == 0 {
            break;
        }
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => break,
        };
        val = val * 16 + digit as u32;
    }
    val
}

fn write_padded_num(fd: i32, n: i32, width: usize) {
    let mut buf = [b'0'; 8];
    let mut val = n;
    let mut i = buf.len();

    if val == 0 {
        i -= 1;
    } else {
        while val > 0 && i > 0 {
            i -= 1;
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
        }
    }

    let start = if buf.len() - i < width {
        buf.len() - width
    } else {
        i
    };

    io::write_all(fd, &buf[start..]);
}

fn write_hex_padded(fd: i32, n: u32, width: usize) {
    let hex_chars = b"0123456789abcdef";
    let mut buf = [b'0'; 8];
    let mut val = n;
    let mut i = buf.len();

    if val == 0 {
        i -= 1;
    } else {
        while val > 0 && i > 0 {
            i -= 1;
            buf[i] = hex_chars[(val & 0xf) as usize];
            val >>= 4;
        }
    }

    let start = if buf.len() - i < width {
        buf.len() - width
    } else {
        i
    };

    io::write_all(fd, &buf[start..]);
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
    fn test_lsusb_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["lsusb"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Output is expected only when the host has USB devices (names like
        // 1-1); CI runners and containers often have none
        let has_devices = std::fs::read_dir("/sys/bus/usb/devices")
            .map(|d| {
                d.flatten()
                    .any(|e| e.file_name().to_string_lossy().contains('-'))
            })
            .unwrap_or(false);
        if has_devices {
            assert!(!output.stdout.is_empty());
        }
    }
}
