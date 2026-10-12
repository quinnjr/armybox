//! i2cdetect - detect I2C devices
//!
//! Detect I2C devices on a bus.

use super::get_arg;
use crate::io;
use crate::sys;

// I2C ioctl commands
const I2C_SLAVE: crate::io::IoctlReq = 0x0703u32 as crate::io::IoctlReq;
const I2C_SMBUS: crate::io::IoctlReq = 0x0720u32 as crate::io::IoctlReq;

/// i2cdetect - detect I2C devices
///
/// # Synopsis
/// ```text
/// i2cdetect [-y] [-a] [-q|-r] BUS
/// ```
///
/// # Description
/// Detect I2C devices on a specified bus.
///
/// # Options
/// - `-y`: Disable interactive mode (assume yes)
/// - `-a`: Force scanning all addresses
/// - `-q`: Use quick write for probing
/// - `-r`: Use read byte for probing
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn i2cdetect(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"i2cdetect: usage: i2cdetect [-y] [-a] BUS\n");
        return 1;
    }

    let mut bus: i32 = -1;
    let mut _force_yes = false;
    let mut scan_all = false;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-y" {
                _force_yes = true;
            } else if arg == b"-a" {
                scan_all = true;
            } else if arg.len() > 0 && arg[0] != b'-' {
                bus = sys::parse_i64(arg).unwrap_or(-1) as i32;
            }
        }
    }

    if bus < 0 {
        io::write_str(2, b"i2cdetect: no bus specified\n");
        return 1;
    }

    // Open I2C device
    let mut path = [0u8; 32];
    let prefix = b"/dev/i2c-";
    path[..prefix.len()].copy_from_slice(prefix);

    let mut num_buf = [0u8; 12];
    let num_str = sys::format_i64(bus as i64, &mut num_buf);
    path[prefix.len()..prefix.len() + num_str.len()].copy_from_slice(num_str);

    let fd = io::open(&path, libc::O_RDWR, 0);
    if fd < 0 {
        sys::perror(&path);
        return 1;
    }

    // Print header
    io::write_str(1, b"     0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f\n");

    let start_addr: u8 = if scan_all { 0x00 } else { 0x03 };
    let end_addr: u8 = if scan_all { 0x7f } else { 0x77 };

    for row in 0..8 {
        // Print row header
        io::write_num(1, (row * 16) as u64);
        io::write_str(1, b"0:");

        for col in 0..16 {
            let addr = row * 16 + col;

            if addr < start_addr || addr > end_addr {
                io::write_str(1, b"   ");
                continue;
            }

            // Try to probe this address
            if unsafe { libc::ioctl(fd, I2C_SLAVE, addr as i32) } < 0 {
                io::write_str(1, b" --");
                continue;
            }

            // Try a simple read to detect device
            let mut buf = [0u8; 1];
            let result = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 1) };

            if result >= 0 {
                // Device responded
                io::write_str(1, b" ");
                let mut hex_buf = [0u8; 4];
                let hex = sys::format_hex(addr as u64, &mut hex_buf);
                // Pad to 2 chars
                if hex.len() < 2 {
                    io::write_str(1, b"0");
                }
                io::write_all(1, hex);
            } else {
                io::write_str(1, b" --");
            }
        }
        io::write_str(1, b"\n");
    }

    io::close(fd);
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_i2cdetect_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["i2cdetect"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_i2cdetect_no_bus() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2cdetect", "-y"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_i2cdetect_nonexistent_bus() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2cdetect", "-y", "99"])
            .output()
            .unwrap();

        // Will fail because /dev/i2c-99 doesn't exist
        assert_eq!(output.status.code(), Some(1));
    }
}
