//! eject - eject removable media
//!
//! Eject removable media from the system.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

// CD-ROM ioctl commands
const CDROMEJECT: crate::io::IoctlReq = 0x5309u32 as crate::io::IoctlReq;

/// eject - eject removable media
///
/// # Synopsis
/// ```text
/// eject [device]
/// ```
///
/// # Description
/// Eject removable media. If no device is specified, /dev/cdrom is used.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn eject(argc: i32, argv: *const *const u8) -> i32 {
    let device = if argc > 1 {
        unsafe { get_arg(argv, 1).unwrap() }
    } else {
        b"/dev/cdrom" as &[u8]
    };

    let fd = io::open(device, libc::O_RDONLY | libc::O_NONBLOCK, 0);
    if fd < 0 {
        sys::perror(device);
        return 1;
    }

    if unsafe { libc::ioctl(fd, CDROMEJECT) } < 0 {
        sys::perror(b"CDROMEJECT");
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
    fn test_eject_no_device() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Will likely fail without a CD-ROM device
        let output = Command::new(&armybox).args(["eject"]).output().unwrap();

        // Expected to fail on systems without CD-ROM
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }

    #[test]
    fn test_eject_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["eject", "/dev/nonexistent_cdrom"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
