//! fsfreeze - suspend access to a filesystem
//!
//! Suspend and resume access to a filesystem.

use super::get_arg;
use crate::io;
use crate::sys;

// Filesystem freeze ioctls
const FIFREEZE: crate::io::IoctlReq = 0xC0045877u32 as crate::io::IoctlReq;
const FITHAW: crate::io::IoctlReq = 0xC0045878u32 as crate::io::IoctlReq;

/// fsfreeze - suspend access to a filesystem
///
/// # Synopsis
/// ```text
/// fsfreeze -f|-u MOUNTPOINT
/// ```
///
/// # Description
/// Suspend or resume access to a filesystem. Used to ensure
/// consistent snapshots.
///
/// # Options
/// - `-f`: Freeze the filesystem
/// - `-u`: Unfreeze (thaw) the filesystem
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn fsfreeze(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"fsfreeze: usage: fsfreeze -f|-u MOUNTPOINT\n");
        return 1;
    }

    let mut freeze = true;
    let mut mountpoint: Option<&[u8]> = None;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-f" || arg == b"--freeze" {
                freeze = true;
            } else if arg == b"-u" || arg == b"--unfreeze" {
                freeze = false;
            } else if arg.len() > 0 && arg[0] != b'-' {
                mountpoint = Some(arg);
            }
        }
    }

    let mountpoint = match mountpoint {
        Some(m) => m,
        None => {
            io::write_str(2, b"fsfreeze: no mountpoint specified\n");
            return 1;
        }
    };

    let fd = io::open(mountpoint, libc::O_RDONLY, 0);
    if fd < 0 {
        sys::perror(mountpoint);
        return 1;
    }

    let ioctl_cmd = if freeze { FIFREEZE } else { FITHAW };
    let result = unsafe { libc::ioctl(fd, ioctl_cmd, 0) };

    if result < 0 {
        if freeze {
            sys::perror(b"FIFREEZE");
        } else {
            sys::perror(b"FITHAW");
        }
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
    fn test_fsfreeze_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["fsfreeze"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_fsfreeze_no_mountpoint() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["fsfreeze", "-f"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_fsfreeze_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["fsfreeze", "-f", "/nonexistent/path"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
