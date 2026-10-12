//! gpioget - read values of GPIO lines
//!
//! Read values from GPIO lines using Linux GPIO character device interface.

#![allow(dead_code)]

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

// GPIO ioctl commands (from linux/gpio.h)
const GPIO_GET_CHIPINFO_IOCTL: crate::io::IoctlReq = 0x8044B401u32 as crate::io::IoctlReq;
const GPIO_GET_LINEINFO_IOCTL: crate::io::IoctlReq = 0xC048B402u32 as crate::io::IoctlReq;
const GPIO_GET_LINEHANDLE_IOCTL: crate::io::IoctlReq = 0xC16CB403u32 as crate::io::IoctlReq;
const GPIOHANDLE_GET_LINE_VALUES_IOCTL: crate::io::IoctlReq = 0xC040B408u32 as crate::io::IoctlReq;

const GPIOHANDLE_REQUEST_INPUT: u32 = 0x01;
const GPIOHANDLES_MAX: usize = 64;

/// GPIO chip info structure
#[repr(C)]
struct GpioChipInfo {
    name: [u8; 32],
    label: [u8; 32],
    lines: u32,
}

/// GPIO line info structure
#[repr(C)]
struct GpioLineInfo {
    line_offset: u32,
    flags: u32,
    name: [u8; 32],
    consumer: [u8; 32],
}

/// GPIO handle request structure
#[repr(C)]
struct GpioHandleRequest {
    lineoffsets: [u32; GPIOHANDLES_MAX],
    flags: u32,
    default_values: [u8; GPIOHANDLES_MAX],
    consumer_label: [u8; 32],
    lines: u32,
    fd: i32,
}

/// GPIO handle data structure
#[repr(C)]
struct GpioHandleData {
    values: [u8; GPIOHANDLES_MAX],
}

/// gpioget - read values of GPIO lines
///
/// # Synopsis
/// ```text
/// gpioget [-a] [-l] CHIP LINE...
/// ```
///
/// # Description
/// Read values from one or more GPIO lines on a chip.
///
/// # Options
/// - `-a`: Use active-low mode
/// - `-l`: Request lines for input
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn gpioget(argc: i32, argv: *const *const u8) -> i32 {
    // Check for --help first
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-h" || arg == b"--help" {
                io::write_str(1, b"Usage: gpioget [OPTIONS] CHIP LINE...\n");
                io::write_str(1, b"Read values from GPIO lines\n\n");
                io::write_str(1, b"Options:\n");
                io::write_str(1, b"  -a    Use active-low mode\n");
                return 0;
            }
        }
    }

    if argc < 3 {
        io::write_str(2, b"Usage: gpioget [OPTIONS] CHIP LINE...\n");
        io::write_str(2, b"Read values from GPIO lines\n\n");
        io::write_str(2, b"Options:\n");
        io::write_str(2, b"  -a    Use active-low mode\n");
        return 1;
    }

    let mut active_low = false;
    let mut chip: Option<&[u8]> = None;
    let mut lines: Vec<u32> = Vec::new();

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-a" || arg == b"--active-low" {
                active_low = true;
            } else if chip.is_none() {
                chip = Some(arg);
            } else {
                // Parse line number
                if let Some(line) = sys::parse_u64(arg) {
                    lines.push(line as u32);
                } else {
                    io::write_str(2, b"gpioget: invalid line number: ");
                    io::write_all(2, arg);
                    io::write_str(2, b"\n");
                    return 1;
                }
            }
        }
        i += 1;
    }

    let chip = match chip {
        Some(c) => c,
        None => {
            io::write_str(2, b"gpioget: chip name required\n");
            return 1;
        }
    };

    if lines.is_empty() {
        io::write_str(2, b"gpioget: at least one line number required\n");
        return 1;
    }

    if lines.len() > GPIOHANDLES_MAX {
        io::write_str(2, b"gpioget: too many lines\n");
        return 1;
    }

    // Build device path
    let mut path = Vec::with_capacity(64);
    if chip.starts_with(b"/") {
        path.extend_from_slice(chip);
    } else {
        path.extend_from_slice(b"/dev/");
        path.extend_from_slice(chip);
    }
    path.push(0);

    // Open GPIO chip
    let fd = unsafe { libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY) };
    if fd < 0 {
        io::write_str(2, b"gpioget: cannot open ");
        io::write_all(2, &path[..path.len() - 1]);
        io::write_str(2, b"\n");
        return 1;
    }

    // Request lines for input
    let mut req: GpioHandleRequest = unsafe { core::mem::zeroed() };
    req.flags = GPIOHANDLE_REQUEST_INPUT;
    req.lines = lines.len() as u32;

    for (idx, &line) in lines.iter().enumerate() {
        req.lineoffsets[idx] = line;
    }

    // Set consumer label
    let label = b"gpioget";
    req.consumer_label[..label.len()].copy_from_slice(label);

    let ret = unsafe {
        libc::ioctl(
            fd,
            GPIO_GET_LINEHANDLE_IOCTL,
            &mut req as *mut GpioHandleRequest,
        )
    };

    if ret < 0 || req.fd < 0 {
        // Fallback: try to read from sysfs
        unsafe {
            libc::close(fd);
        }
        return read_gpio_sysfs(chip, &lines, active_low);
    }

    unsafe {
        libc::close(fd);
    }

    // Read values
    let mut data: GpioHandleData = unsafe { core::mem::zeroed() };
    let ret = unsafe {
        libc::ioctl(
            req.fd,
            GPIOHANDLE_GET_LINE_VALUES_IOCTL,
            &mut data as *mut GpioHandleData,
        )
    };

    if ret < 0 {
        unsafe {
            libc::close(req.fd);
        }
        io::write_str(2, b"gpioget: failed to read GPIO values\n");
        return 1;
    }

    unsafe {
        libc::close(req.fd);
    }

    // Output values
    for idx in 0..lines.len() {
        let mut val = data.values[idx];
        if active_low {
            val = if val == 0 { 1 } else { 0 };
        }
        io::write_num(1, val as u64);
        if idx < lines.len() - 1 {
            io::write_str(1, b" ");
        }
    }
    io::write_str(1, b"\n");

    0
}

/// Fallback: read GPIO values from sysfs
fn read_gpio_sysfs(chip: &[u8], lines: &[u32], active_low: bool) -> i32 {
    // Extract chip number
    let chip_num = if chip.starts_with(b"gpiochip") {
        sys::parse_u64(&chip[8..]).unwrap_or(0) as u32
    } else {
        sys::parse_u64(chip).unwrap_or(0) as u32
    };

    // Get base GPIO number from sysfs
    let mut base_path = Vec::new();
    base_path.extend_from_slice(b"/sys/class/gpio/gpiochip");
    let mut num_buf = [0u8; 16];
    let num_str = sys::format_u64(chip_num as u64, &mut num_buf);
    base_path.extend_from_slice(num_str);
    base_path.extend_from_slice(b"/base");

    let fd = io::open(&base_path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"gpioget: cannot access GPIO chip\n");
        return 1;
    }

    let mut buf = [0u8; 16];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    let base = if n > 0 {
        let end = buf
            .iter()
            .position(|&c| c == b'\n' || c == 0)
            .unwrap_or(n as usize);
        sys::parse_u64(&buf[..end]).unwrap_or(0) as u32
    } else {
        chip_num * 32 // Estimate
    };

    // Read each line
    for (idx, &line) in lines.iter().enumerate() {
        let gpio_num = base + line;

        // Export GPIO if needed
        let mut export_path = b"/sys/class/gpio/export".to_vec();
        export_path.push(0);
        let export_fd =
            unsafe { libc::open(export_path.as_ptr() as *const libc::c_char, libc::O_WRONLY) };
        if export_fd >= 0 {
            let gpio_str = sys::format_u64(gpio_num as u64, &mut num_buf);
            unsafe {
                libc::write(export_fd, gpio_str.as_ptr() as *const _, gpio_str.len());
                libc::close(export_fd);
            }
        }

        // Read value
        let mut value_path = Vec::new();
        value_path.extend_from_slice(b"/sys/class/gpio/gpio");
        let gpio_str = sys::format_u64(gpio_num as u64, &mut num_buf);
        value_path.extend_from_slice(gpio_str);
        value_path.extend_from_slice(b"/value");

        let fd = io::open(&value_path, libc::O_RDONLY, 0);
        let mut val = 0u8;
        if fd >= 0 {
            let mut buf = [0u8; 4];
            let n = io::read(fd, &mut buf);
            io::close(fd);
            if n > 0 && buf[0] == b'1' {
                val = 1;
            }
        }

        if active_low {
            val = if val == 0 { 1 } else { 0 };
        }

        io::write_num(1, val as u64);
        if idx < lines.len() - 1 {
            io::write_str(1, b" ");
        }
    }
    io::write_str(1, b"\n");

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
    fn test_gpioget_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["gpioget"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_gpioget_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpioget", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_gpioget_nonexistent_chip() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpioget", "nonexistent_chip", "0"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
