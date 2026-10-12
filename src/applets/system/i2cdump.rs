//! i2cdump - dump I2C device registers
//!
//! Examine I2C registers on a connected device.

use super::get_arg;
use crate::io;
use crate::sys;

// I2C ioctl
const I2C_SLAVE: crate::io::IoctlReq = 0x0703u32 as crate::io::IoctlReq;

/// i2cdump - dump I2C device registers
///
/// # Synopsis
/// ```text
/// i2cdump [-y] BUS ADDRESS [MODE]
/// ```
///
/// # Description
/// Dump registers from an I2C device.
///
/// # Options
/// - `-y`: Disable interactive mode
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn i2cdump(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"i2cdump: usage: i2cdump [-y] BUS ADDRESS\n");
        return 1;
    }

    let mut bus: i32 = -1;
    let mut addr: i32 = -1;
    let mut arg_idx = 1;

    while arg_idx < argc {
        if let Some(arg) = unsafe { get_arg(argv, arg_idx) } {
            if arg == b"-y" {
                // Skip -y flag
            } else if bus < 0 {
                bus = sys::parse_i64(arg).unwrap_or(-1) as i32;
            } else if addr < 0 {
                addr = parse_addr(arg);
            }
        }
        arg_idx += 1;
    }

    if bus < 0 || addr < 0 {
        io::write_str(2, b"i2cdump: invalid bus or address\n");
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

    if unsafe { libc::ioctl(fd, I2C_SLAVE, addr) } < 0 {
        sys::perror(b"I2C_SLAVE");
        io::close(fd);
        return 1;
    }

    // Print header
    io::write_str(1, b"     0  1  2  3  4  5  6  7  8  9  a  b  c  d  e  f\n");

    // Dump all 256 registers
    for row in 0..16 {
        // Row header
        let mut hex_buf = [0u8; 4];
        let hex = sys::format_hex((row * 16) as u64, &mut hex_buf);
        if hex.len() < 2 {
            io::write_str(1, b"0");
        }
        io::write_all(1, hex);
        io::write_str(1, b":");

        for col in 0..16 {
            let reg = row * 16 + col;
            let mut buf = [0u8; 1];

            // Read register (write address, then read)
            let reg_byte = [reg as u8];
            if unsafe { libc::write(fd, reg_byte.as_ptr() as *const libc::c_void, 1) } == 1 {
                if unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 1) } == 1 {
                    io::write_str(1, b" ");
                    let hex = sys::format_hex(buf[0] as u64, &mut hex_buf);
                    if hex.len() < 2 {
                        io::write_str(1, b"0");
                    }
                    io::write_all(1, hex);
                } else {
                    io::write_str(1, b" XX");
                }
            } else {
                io::write_str(1, b" XX");
            }
        }
        io::write_str(1, b"\n");
    }

    io::close(fd);
    0
}

fn parse_addr(s: &[u8]) -> i32 {
    if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        // Hex
        let mut result: i32 = 0;
        for &c in &s[2..] {
            let digit = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return -1,
            };
            result = result * 16 + digit as i32;
        }
        result
    } else {
        sys::parse_i64(s).unwrap_or(-1) as i32
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
    fn test_i2cdump_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["i2cdump"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_i2cdump_nonexistent_bus() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2cdump", "-y", "99", "0x50"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
