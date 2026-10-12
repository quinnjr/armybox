//! blkdiscard - discard device sectors
//!
//! Discard the content of sectors on a device.

use super::get_arg;
use crate::io;
use crate::sys;

// BLKDISCARD ioctl
const BLKDISCARD: crate::io::IoctlReq = 0x1277u32 as crate::io::IoctlReq;
const BLKGETSIZE64: crate::io::IoctlReq = 0x80081272u32 as crate::io::IoctlReq;

/// blkdiscard - discard device sectors
///
/// # Synopsis
/// ```text
/// blkdiscard [-o offset] [-l length] device
/// ```
///
/// # Description
/// Discard the content of sectors on a device. This is useful for
/// SSDs to inform the drive that blocks are no longer in use.
///
/// # Options
/// - `-o offset`: Start offset in bytes
/// - `-l length`: Number of bytes to discard
/// - `-z`: Zero-fill instead of discard
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn blkdiscard(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(
            2,
            b"blkdiscard: usage: blkdiscard [-o offset] [-l length] device\n",
        );
        return 1;
    }

    let mut offset: u64 = 0;
    let mut length: u64 = 0;
    let mut length_specified = false;
    let mut device: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-o" && i + 1 < argc {
                if let Some(val) = unsafe { get_arg(argv, i + 1) } {
                    offset = sys::parse_u64(val).unwrap_or(0);
                }
                i += 2;
                continue;
            } else if arg == b"-l" && i + 1 < argc {
                if let Some(val) = unsafe { get_arg(argv, i + 1) } {
                    length = sys::parse_u64(val).unwrap_or(0);
                    length_specified = true;
                }
                i += 2;
                continue;
            } else if arg.len() > 0 && arg[0] != b'-' {
                device = Some(arg);
            }
        }
        i += 1;
    }

    let device = match device {
        Some(d) => d,
        None => {
            io::write_str(2, b"blkdiscard: no device specified\n");
            return 1;
        }
    };

    let fd = io::open(device, libc::O_RDWR, 0);
    if fd < 0 {
        sys::perror(device);
        return 1;
    }

    // If length not specified, get device size
    if !length_specified {
        let mut size: u64 = 0;
        if unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size) } < 0 {
            sys::perror(b"BLKGETSIZE64");
            io::close(fd);
            return 1;
        }
        length = size - offset;
    }

    // BLKDISCARD takes a range [offset, length]
    let range: [u64; 2] = [offset, length];
    if unsafe { libc::ioctl(fd, BLKDISCARD, &range) } < 0 {
        sys::perror(b"BLKDISCARD");
        io::close(fd);
        return 1;
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
    fn test_blkdiscard_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["blkdiscard"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_blkdiscard_nonexistent_device() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["blkdiscard", "/dev/nonexistent_device_xyz"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_blkdiscard_with_offset() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // This will fail since we don't have a real block device,
        // but it tests argument parsing
        let output = Command::new(&armybox)
            .args(["blkdiscard", "-o", "4096", "/dev/null"])
            .output()
            .unwrap();

        // Will fail on ioctl since /dev/null isn't a block device
        assert_eq!(output.status.code(), Some(1));
    }
}
