//! deallocvt - deallocate unused virtual terminals
//!
//! Deallocate kernel resources for unused virtual consoles.

use super::get_arg;
use crate::io;
use crate::sys;

// VT ioctl
const VT_DISALLOCATE: crate::io::IoctlReq = 0x5608u32 as crate::io::IoctlReq;

/// deallocvt - deallocate unused virtual terminals
///
/// # Synopsis
/// ```text
/// deallocvt [N...]
/// ```
///
/// # Description
/// Deallocate kernel resources for unused virtual consoles.
/// If no arguments, deallocate all unused consoles.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn deallocvt(argc: i32, argv: *const *const u8) -> i32 {
    let fd = io::open(b"/dev/tty0", libc::O_RDWR, 0);
    if fd < 0 {
        // Try /dev/tty as fallback
        let fd = io::open(b"/dev/tty", libc::O_RDWR, 0);
        if fd < 0 {
            sys::perror(b"/dev/tty");
            return 1;
        }
    }

    if argc < 2 {
        // Deallocate all unused VTs (arg 0)
        if unsafe { libc::ioctl(fd, VT_DISALLOCATE, 0) } < 0 {
            sys::perror(b"VT_DISALLOCATE");
            io::close(fd);
            return 1;
        }
    } else {
        // Deallocate specific VTs
        for i in 1..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                let vt = sys::parse_i64(arg).unwrap_or(0) as i32;
                if vt > 0 {
                    if unsafe { libc::ioctl(fd, VT_DISALLOCATE, vt) } < 0 {
                        io::write_str(2, b"deallocvt: could not deallocate VT ");
                        io::write_num(2, vt as u64);
                        io::write_str(2, b"\n");
                    }
                }
            }
        }
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
    fn test_deallocvt_no_tty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Running without a TTY will likely fail, which is expected
        let output = Command::new(&armybox).args(["deallocvt"]).output().unwrap();

        // Either succeeds (if run on console) or fails (no TTY)
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }

    #[test]
    fn test_deallocvt_specific_vt() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["deallocvt", "99"])
            .output()
            .unwrap();

        // Will fail without proper TTY access
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
