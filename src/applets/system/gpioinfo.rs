//! gpioinfo - display information about GPIO chips and lines
//!
//! Display information about GPIO chips and their lines using Linux GPIO chardev.

extern crate alloc;

use crate::io;
use crate::sys;
use alloc::vec::Vec;

// GPIO ioctl commands (from linux/gpio.h)
const GPIO_GET_CHIPINFO_IOCTL: crate::io::IoctlReq = 0x8044B401u32 as crate::io::IoctlReq;
const GPIO_GET_LINEINFO_IOCTL: crate::io::IoctlReq = 0xC048B402u32 as crate::io::IoctlReq;

// Line flags
const GPIOLINE_FLAG_KERNEL: u32 = 1 << 0;
const GPIOLINE_FLAG_IS_OUT: u32 = 1 << 1;
const GPIOLINE_FLAG_ACTIVE_LOW: u32 = 1 << 2;
const GPIOLINE_FLAG_OPEN_DRAIN: u32 = 1 << 3;
const GPIOLINE_FLAG_OPEN_SOURCE: u32 = 1 << 4;

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

/// gpioinfo - display information about GPIO chips and lines
///
/// # Synopsis
/// ```text
/// gpioinfo [CHIP...]
/// ```
///
/// # Description
/// Display information about GPIO chips and their lines.
/// If no chip specified, show all chips.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn gpioinfo(argc: i32, argv: *const *const u8) -> i32 {
    let mut found = false;

    if argc < 2 {
        // Show all chips
        for i in 0..32 {
            if show_chip_info(i) {
                found = true;
            }
        }
    } else {
        // Show specified chips
        for i in 1..argc {
            if let Some(chip) = unsafe { super::get_arg(argv, i) } {
                if chip == b"-h" || chip == b"--help" {
                    io::write_str(1, b"Usage: gpioinfo [OPTIONS] [CHIP...]\n");
                    io::write_str(1, b"Display information about GPIO chips and lines\n\n");
                    io::write_str(1, b"If no chip specified, show all chips.\n");
                    return 0;
                }

                // Parse chip - could be number, gpiochipN, or /dev/gpiochipN
                let chip_num = parse_chip_name(chip);
                if show_chip_info(chip_num) {
                    found = true;
                }
            }
        }
    }

    if !found {
        io::write_str(2, b"gpioinfo: no GPIO chips found\n");
        return 1;
    }

    0
}

/// Parse chip name to chip number
fn parse_chip_name(name: &[u8]) -> u8 {
    // /dev/gpiochipN
    if name.starts_with(b"/dev/gpiochip") {
        return sys::parse_u64(&name[13..]).unwrap_or(0) as u8;
    }
    // gpiochipN
    if name.starts_with(b"gpiochip") {
        return sys::parse_u64(&name[8..]).unwrap_or(0) as u8;
    }
    // Just a number
    sys::parse_u64(name).unwrap_or(0) as u8
}

fn show_chip_info(chip_num: u8) -> bool {
    // Build path
    let mut path = Vec::with_capacity(32);
    path.extend_from_slice(b"/dev/gpiochip");
    let mut num_buf = [0u8; 8];
    let num_str = sys::format_u64(chip_num as u64, &mut num_buf);
    path.extend_from_slice(num_str);
    path.push(0);

    // Open the device
    let fd = unsafe { libc::open(path.as_ptr() as *const libc::c_char, libc::O_RDONLY) };
    if fd < 0 {
        return false;
    }

    // Get chip info
    let mut chip_info: GpioChipInfo = unsafe { core::mem::zeroed() };
    let ret = unsafe {
        libc::ioctl(
            fd,
            GPIO_GET_CHIPINFO_IOCTL,
            &mut chip_info as *mut GpioChipInfo,
        )
    };

    if ret < 0 {
        unsafe {
            libc::close(fd);
        }
        return false;
    }

    // Print chip header
    let name_end = chip_info
        .name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(chip_info.name.len());
    let label_end = chip_info
        .label
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(chip_info.label.len());

    io::write_all(1, &chip_info.name[..name_end]);
    io::write_str(1, b" - ");
    io::write_num(1, chip_info.lines as u64);
    io::write_str(1, b" lines");

    if label_end > 0 && chip_info.label[0] != 0 {
        io::write_str(1, b" (");
        io::write_all(1, &chip_info.label[..label_end]);
        io::write_str(1, b")");
    }
    io::write_str(1, b":\n");

    // Print each line
    for line in 0..chip_info.lines {
        let mut line_info: GpioLineInfo = unsafe { core::mem::zeroed() };
        line_info.line_offset = line;

        let ret = unsafe {
            libc::ioctl(
                fd,
                GPIO_GET_LINEINFO_IOCTL,
                &mut line_info as *mut GpioLineInfo,
            )
        };

        if ret < 0 {
            continue;
        }

        io::write_str(1, b"\tline ");

        // Right-align line number
        if line < 10 {
            io::write_str(1, b"  ");
        } else if line < 100 {
            io::write_str(1, b" ");
        }
        io::write_num(1, line as u64);

        io::write_str(1, b": ");

        // Line name
        let name_end = line_info
            .name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(line_info.name.len());
        if name_end > 0 && line_info.name[0] != 0 {
            io::write_str(1, b"\"");
            io::write_all(1, &line_info.name[..name_end]);
            io::write_str(1, b"\"");
        } else {
            io::write_str(1, b"unnamed");
        }

        // Pad to column
        let name_len = if name_end > 0 && line_info.name[0] != 0 {
            name_end + 2
        } else {
            7
        };
        for _ in name_len..24 {
            io::write_str(1, b" ");
        }

        // Consumer
        let consumer_end = line_info
            .consumer
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(line_info.consumer.len());
        if consumer_end > 0 && line_info.consumer[0] != 0 {
            io::write_str(1, b"\"");
            io::write_all(1, &line_info.consumer[..consumer_end]);
            io::write_str(1, b"\"");
        } else {
            io::write_str(1, b"unused");
        }

        // Pad to column
        let consumer_len = if consumer_end > 0 && line_info.consumer[0] != 0 {
            consumer_end + 2
        } else {
            6
        };
        for _ in consumer_len..20 {
            io::write_str(1, b" ");
        }

        // Flags
        if line_info.flags & GPIOLINE_FLAG_IS_OUT != 0 {
            io::write_str(1, b" output");
        } else {
            io::write_str(1, b" input");
        }

        if line_info.flags & GPIOLINE_FLAG_ACTIVE_LOW != 0 {
            io::write_str(1, b" active-low");
        }

        if line_info.flags & GPIOLINE_FLAG_OPEN_DRAIN != 0 {
            io::write_str(1, b" open-drain");
        }

        if line_info.flags & GPIOLINE_FLAG_OPEN_SOURCE != 0 {
            io::write_str(1, b" open-source");
        }

        if line_info.flags & GPIOLINE_FLAG_KERNEL != 0 {
            io::write_str(1, b" [kernel]");
        }

        io::write_str(1, b"\n");
    }

    unsafe {
        libc::close(fd);
    }
    true
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
    fn test_gpioinfo_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["gpioinfo"]).output().unwrap();

        // Either shows info or reports no chips
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }

    #[test]
    fn test_gpioinfo_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpioinfo", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_gpioinfo_specific_chip() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpioinfo", "gpiochip0"])
            .output()
            .unwrap();

        // Either shows info or chip doesn't exist
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
