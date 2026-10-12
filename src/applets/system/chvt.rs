//! chvt - change foreground virtual terminal
//!
//! Change the foreground virtual terminal.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

// VT_ACTIVATE ioctl
const VT_ACTIVATE: crate::io::IoctlReq = 0x5606u32 as crate::io::IoctlReq;
const VT_WAITACTIVE: crate::io::IoctlReq = 0x5607u32 as crate::io::IoctlReq;

/// chvt - change foreground virtual terminal
///
/// # Synopsis
/// ```text
/// chvt N
/// ```
///
/// # Description
/// Change the foreground virtual terminal to /dev/ttyN.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn chvt(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"chvt: missing VT number\n");
        return 1;
    }

    let vt_str = unsafe { get_arg(argv, 1).unwrap() };
    let vt_num = sys::parse_i64(vt_str).unwrap_or(0) as i32;

    if vt_num <= 0 {
        io::write_str(2, b"chvt: invalid VT number\n");
        return 1;
    }

    // Open /dev/console or /dev/tty0
    let fd = io::open(b"/dev/console", libc::O_RDWR, 0);
    let fd = if fd < 0 {
        io::open(b"/dev/tty0", libc::O_RDWR, 0)
    } else {
        fd
    };

    if fd < 0 {
        sys::perror(b"chvt");
        return 1;
    }

    // Activate the VT
    if unsafe { libc::ioctl(fd, VT_ACTIVATE, vt_num) } < 0 {
        sys::perror(b"VT_ACTIVATE");
        io::close(fd);
        return 1;
    }

    // Wait for VT to become active
    if unsafe { libc::ioctl(fd, VT_WAITACTIVE, vt_num) } < 0 {
        sys::perror(b"VT_WAITACTIVE");
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
    fn test_chvt_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chvt"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing"));
    }

    #[test]
    fn test_chvt_invalid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chvt", "0"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
