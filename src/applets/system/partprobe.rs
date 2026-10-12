//! partprobe - inform the kernel of partition table changes
//!
//! Inform the OS kernel of partition table changes.

use super::get_arg;
use crate::io;
use crate::sys;

// BLKRRPART ioctl - reread partition table
const BLKRRPART: crate::io::IoctlReq = 0x125fu32 as crate::io::IoctlReq;

/// partprobe - inform the kernel of partition table changes
///
/// # Synopsis
/// ```text
/// partprobe [DEVICE...]
/// ```
///
/// # Description
/// Inform the kernel of partition table changes on the
/// specified devices. If no devices specified, probe all
/// disk devices.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn partprobe(argc: i32, argv: *const *const u8) -> i32 {
    let mut exit_code = 0;

    if argc < 2 {
        // Probe all disk devices
        let disks = [
            b"/dev/sda" as &[u8],
            b"/dev/sdb",
            b"/dev/sdc",
            b"/dev/sdd",
            b"/dev/nvme0n1",
            b"/dev/vda",
            b"/dev/hda",
        ];

        for disk in disks.iter() {
            let fd = io::open(disk, libc::O_RDONLY, 0);
            if fd >= 0 {
                if unsafe { libc::ioctl(fd, BLKRRPART) } < 0 {
                    // Silently ignore errors for non-existent devices
                }
                io::close(fd);
            }
        }

        return 0;
    }

    // Probe specified devices
    for i in 1..argc {
        if let Some(device) = unsafe { get_arg(argv, i) } {
            let fd = io::open(device, libc::O_RDONLY, 0);
            if fd < 0 {
                sys::perror(device);
                exit_code = 1;
                continue;
            }

            if unsafe { libc::ioctl(fd, BLKRRPART) } < 0 {
                io::write_str(2, b"partprobe: ");
                io::write_all(2, device);
                io::write_str(2, b": failed to reread partition table\n");
                exit_code = 1;
            }

            io::close(fd);
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_partprobe_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["partprobe"]).output().unwrap();

        // Should succeed (silently ignores missing devices)
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_partprobe_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["partprobe", "/dev/nonexistent_device_xyz"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
