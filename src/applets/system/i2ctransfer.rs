//! i2ctransfer - send user-defined I2C messages
//!
//! Execute I2C message transfers.

#![allow(dead_code)]

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec;
use alloc::vec::Vec;

// I2C ioctl commands
const I2C_RDWR: crate::io::IoctlReq = 0x0707u32 as crate::io::IoctlReq;
const I2C_M_RD: u16 = 0x0001;
const I2C_M_TEN: u16 = 0x0010;
const I2C_M_RECV_LEN: u16 = 0x0400;
const I2C_M_NO_RD_ACK: u16 = 0x0800;
const I2C_M_IGNORE_NAK: u16 = 0x1000;
const I2C_M_REV_DIR_ADDR: u16 = 0x2000;
const I2C_M_NOSTART: u16 = 0x4000;
const I2C_M_STOP: u16 = 0x8000;

/// i2c_msg structure (matches kernel definition)
#[repr(C)]
struct I2cMsg {
    addr: u16,    // slave address
    flags: u16,   // message flags
    len: u16,     // message length
    buf: *mut u8, // pointer to data buffer
}

/// i2c_rdwr_ioctl_data structure
#[repr(C)]
struct I2cRdwrIoctlData {
    msgs: *mut I2cMsg,
    nmsgs: u32,
}

/// i2ctransfer - send user-defined I2C messages
///
/// # Synopsis
/// ```text
/// i2ctransfer [-y] [-v] [-V] BUS DESC [DATA...] [DESC [DATA...]]...
/// ```
///
/// # Description
/// Execute user-defined I2C message transfers.
/// DESC format: {r|w}LENGTH[@ADDRESS]
///   - r: read operation
///   - w: write operation
///   - LENGTH: number of bytes to read/write
///   - ADDRESS: 7-bit I2C address (hex with 0x prefix)
///
/// # Options
/// - `-y`: Disable interactive mode
/// - `-v`: Verbose output
/// - `-V`: Print version and exit
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn i2ctransfer(argc: i32, argv: *const *const u8) -> i32 {
    let mut bus: Option<i32> = None;
    let mut verbose = false;
    let mut force = false;
    let mut descs: Vec<MsgDesc> = Vec::new();

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-y" || arg == b"-f" {
            force = true;
        } else if arg == b"-v" {
            verbose = true;
        } else if arg == b"-V" || arg == b"--version" {
            io::write_str(1, b"i2ctransfer from armybox\n");
            return 0;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if bus.is_none() {
            // First non-option is bus number
            bus = Some(sys::parse_i64(arg).unwrap_or(-1) as i32);
        } else {
            // Parse message descriptor or data
            if let Some(desc) = parse_desc(arg) {
                descs.push(desc);
            } else if !descs.is_empty() {
                // This is data for the last write message
                if let Some(last) = descs.last_mut() {
                    if !last.is_read {
                        if let Some(byte) = parse_hex_byte(arg) {
                            last.data.push(byte);
                        } else {
                            io::write_str(2, b"i2ctransfer: invalid data byte: ");
                            io::write_all(2, arg);
                            io::write_str(2, b"\n");
                            return 1;
                        }
                    }
                }
            } else {
                io::write_str(2, b"i2ctransfer: unexpected argument: ");
                io::write_all(2, arg);
                io::write_str(2, b"\n");
                return 1;
            }
        }
        i += 1;
    }

    let bus = match bus {
        Some(b) if b >= 0 => b,
        _ => {
            io::write_str(2, b"i2ctransfer: missing or invalid bus number\n");
            print_usage();
            return 1;
        }
    };

    if descs.is_empty() {
        io::write_str(2, b"i2ctransfer: no messages specified\n");
        print_usage();
        return 1;
    }

    // Validate descriptors
    let mut current_addr: Option<u16> = None;
    for desc in &mut descs {
        if let Some(a) = desc.addr {
            current_addr = Some(a);
        } else {
            if let Some(a) = current_addr {
                desc.addr = Some(a);
            } else {
                io::write_str(2, b"i2ctransfer: no address specified\n");
                return 1;
            }
        }

        // Validate write data length
        if !desc.is_read && desc.data.len() != desc.length as usize {
            io::write_str(2, b"i2ctransfer: write data length mismatch\n");
            return 1;
        }
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

    // Build I2C messages
    let mut msgs: Vec<I2cMsg> = Vec::new();
    let mut buffers: Vec<Vec<u8>> = Vec::new();

    for desc in &descs {
        let addr = desc.addr.unwrap_or(0);
        let flags = if desc.is_read { I2C_M_RD } else { 0 };

        let mut buf = if desc.is_read {
            vec![0u8; desc.length as usize]
        } else {
            desc.data.clone()
        };

        let buf_ptr = buf.as_mut_ptr();
        buffers.push(buf);

        msgs.push(I2cMsg {
            addr,
            flags,
            len: desc.length,
            buf: buf_ptr,
        });
    }

    // Update buffer pointers after collecting all buffers
    for (i, msg) in msgs.iter_mut().enumerate() {
        msg.buf = buffers[i].as_mut_ptr();
    }

    let mut rdwr_data = I2cRdwrIoctlData {
        msgs: msgs.as_mut_ptr(),
        nmsgs: msgs.len() as u32,
    };

    // Execute transfer
    if unsafe { libc::ioctl(fd, I2C_RDWR, &mut rdwr_data) } < 0 {
        sys::perror(b"i2ctransfer");
        io::close(fd);
        return 1;
    }

    // Print results
    for (i, desc) in descs.iter().enumerate() {
        if verbose {
            io::write_str(1, b"msg ");
            let mut num_buf = [0u8; 8];
            io::write_all(1, sys::format_u64(i as u64, &mut num_buf));
            io::write_str(
                1,
                if desc.is_read {
                    b": read "
                } else {
                    b": write "
                },
            );
            io::write_all(1, sys::format_u64(desc.length as u64, &mut num_buf));
            io::write_str(1, b" bytes at 0x");
            let addr = desc.addr.unwrap_or(0);
            print_hex(1, addr as u8);
            io::write_str(1, b"\n");
        }

        if desc.is_read {
            // Print read data
            for j in 0..desc.length as usize {
                if j > 0 {
                    io::write_str(1, b" ");
                }
                io::write_str(1, b"0x");
                print_hex(1, buffers[i][j]);
            }
            io::write_str(1, b"\n");
        }
    }

    io::close(fd);
    0
}

#[derive(Clone)]
struct MsgDesc {
    is_read: bool,
    length: u16,
    addr: Option<u16>,
    data: Vec<u8>,
}

fn parse_desc(s: &[u8]) -> Option<MsgDesc> {
    if s.is_empty() {
        return None;
    }

    let is_read = match s[0] {
        b'r' | b'R' => true,
        b'w' | b'W' => false,
        _ => return None,
    };

    // Parse length and optional address: LENGTH[@ADDRESS]
    let rest = &s[1..];

    let (length_part, addr) = if let Some(at_pos) = rest.iter().position(|&c| c == b'@') {
        let length_str = &rest[..at_pos];
        let addr_str = &rest[at_pos + 1..];
        let addr = parse_hex_or_dec(addr_str)?;
        (length_str, Some(addr as u16))
    } else {
        (rest, None)
    };

    let length = sys::parse_u64(length_part)? as u16;
    if length == 0 {
        return None;
    }

    Some(MsgDesc {
        is_read,
        length,
        addr,
        data: Vec::new(),
    })
}

fn parse_hex_or_dec(s: &[u8]) -> Option<u32> {
    if s.len() >= 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        // Hex
        let hex_str = &s[2..];
        let mut val = 0u32;
        for &c in hex_str {
            val = val.checked_mul(16)?;
            let digit = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return None,
            };
            val = val.checked_add(digit as u32)?;
        }
        Some(val)
    } else {
        // Decimal
        sys::parse_u64(s).map(|v| v as u32)
    }
}

fn parse_hex_byte(s: &[u8]) -> Option<u8> {
    let val = parse_hex_or_dec(s)?;
    if val > 255 {
        return None;
    }
    Some(val as u8)
}

fn print_hex(fd: i32, val: u8) {
    let hex_chars = b"0123456789abcdef";
    let buf = [
        hex_chars[(val >> 4) as usize],
        hex_chars[(val & 0x0f) as usize],
    ];
    io::write_all(fd, &buf);
}

fn print_usage() {
    io::write_str(
        1,
        b"Usage: i2ctransfer [-y] [-v] BUS DESC [DATA...] ...\n\n",
    );
    io::write_str(1, b"Execute I2C message transfers.\n\n");
    io::write_str(1, b"DESC format: {r|w}LENGTH[@ADDRESS]\n");
    io::write_str(1, b"  r: read operation\n");
    io::write_str(1, b"  w: write operation\n");
    io::write_str(1, b"  LENGTH: number of bytes\n");
    io::write_str(1, b"  ADDRESS: 7-bit I2C address (0x00-0x7f)\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -y   Disable interactive mode\n");
    io::write_str(1, b"  -v   Verbose output\n\n");
    io::write_str(1, b"Examples:\n");
    io::write_str(1, b"  i2ctransfer -y 1 w2@0x50 0x00 0x10 r4\n");
    io::write_str(1, b"  i2ctransfer -y 0 w1@0x68 0x0f r1\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn i2ctransfer(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"i2ctransfer: only available on Linux\n");
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
    fn test_i2ctransfer_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2ctransfer", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_i2ctransfer_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["i2ctransfer"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing") || stderr.contains("bus"));
    }

    #[test]
    fn test_parse_desc() {
        use super::parse_desc;

        let desc = parse_desc(b"r4@0x50").unwrap();
        assert!(desc.is_read);
        assert_eq!(desc.length, 4);
        assert_eq!(desc.addr, Some(0x50));

        let desc = parse_desc(b"w2@0x68").unwrap();
        assert!(!desc.is_read);
        assert_eq!(desc.length, 2);
        assert_eq!(desc.addr, Some(0x68));

        let desc = parse_desc(b"r1").unwrap();
        assert!(desc.is_read);
        assert_eq!(desc.length, 1);
        assert_eq!(desc.addr, None);
    }
}
