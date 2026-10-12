//! freeramdisk - free all memory used by a ramdisk
//!
//! Release memory used by a specified ramdisk device.

use super::get_arg;
use crate::io;
use crate::sys;

// BLKFLSBUF ioctl - flush buffer cache
const BLKFLSBUF: crate::io::IoctlReq = 0x1261u32 as crate::io::IoctlReq;

/// freeramdisk - free all memory used by a ramdisk
///
/// # Synopsis
/// ```text
/// freeramdisk DEVICE
/// ```
///
/// # Description
/// Free all memory used by a ramdisk device.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn freeramdisk(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"freeramdisk: usage: freeramdisk DEVICE\n");
        return 1;
    }

    let device = unsafe { get_arg(argv, 1).unwrap() };

    let fd = io::open(device, libc::O_RDWR, 0);
    if fd < 0 {
        sys::perror(device);
        return 1;
    }

    // Flush buffers - this frees the ramdisk memory
    if unsafe { libc::ioctl(fd, BLKFLSBUF) } < 0 {
        sys::perror(b"BLKFLSBUF");
        io::close(fd);
        return 1;
    }

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
    fn test_freeramdisk_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["freeramdisk"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_freeramdisk_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["freeramdisk", "/dev/nonexistent_ram"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
