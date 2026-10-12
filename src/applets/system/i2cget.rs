//! i2cget - read from I2C device
//!
//! Read a register from an I2C device.

use super::get_arg;
use crate::io;
use crate::sys;

// I2C ioctl
const I2C_SLAVE: crate::io::IoctlReq = 0x0703u32 as crate::io::IoctlReq;

/// i2cget - read from I2C device
///
/// # Synopsis
/// ```text
/// i2cget [-y] BUS ADDRESS [REGISTER]
/// ```
///
/// # Description
/// Read a byte from an I2C device register.
///
/// # Options
/// - `-y`: Disable interactive mode
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn i2cget(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"i2cget: usage: i2cget [-y] BUS ADDRESS [REGISTER]\n");
        return 1;
    }

    let mut bus: i32 = -1;
    let mut addr: i32 = -1;
    let mut reg: Option<u8> = None;
    let mut arg_idx = 1;

    while arg_idx < argc {
        if let Some(arg) = unsafe { get_arg(argv, arg_idx) } {
            if arg == b"-y" {
                // Skip -y flag
            } else if bus < 0 {
                bus = sys::parse_i64(arg).unwrap_or(-1) as i32;
            } else if addr < 0 {
                addr = parse_hex_or_dec(arg) as i32;
            } else if reg.is_none() {
                reg = Some(parse_hex_or_dec(arg) as u8);
            }
        }
        arg_idx += 1;
    }

    if bus < 0 || addr < 0 {
        io::write_str(2, b"i2cget: invalid bus or address\n");
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

    let mut value = [0u8; 1];

    if let Some(r) = reg {
        // Write register address first
        let reg_buf = [r];
        if unsafe { libc::write(fd, reg_buf.as_ptr() as *const libc::c_void, 1) } != 1 {
            sys::perror(b"write");
            io::close(fd);
            return 1;
        }
    }

    // Read value
    if unsafe { libc::read(fd, value.as_mut_ptr() as *mut libc::c_void, 1) } != 1 {
        sys::perror(b"read");
        io::close(fd);
        return 1;
    }

    io::write_str(1, b"0x");
    let mut hex_buf = [0u8; 4];
    let hex = sys::format_hex(value[0] as u64, &mut hex_buf);
    if hex.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, hex);
    io::write_str(1, b"\n");

    io::close(fd);
    0
}

fn parse_hex_or_dec(s: &[u8]) -> u64 {
    if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        let mut result: u64 = 0;
        for &c in &s[2..] {
            let digit = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return 0,
            };
            result = result * 16 + digit as u64;
        }
        result
    } else {
        sys::parse_u64(s).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_i2cget_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["i2cget"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_i2cget_nonexistent_bus() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2cget", "-y", "99", "0x50"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
