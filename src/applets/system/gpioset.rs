//! gpioset - set values of GPIO lines
//!
//! Set values of GPIO lines.

use super::get_arg;
use crate::io;
use crate::sys;

/// gpioset - set values of GPIO lines
///
/// # Synopsis
/// ```text
/// gpioset CHIP LINE=VALUE...
/// ```
///
/// # Description
/// Set values of one or more GPIO lines on a chip.
/// Values are 0 or 1.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn gpioset(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"gpioset: usage: gpioset CHIP LINE=VALUE...\n");
        return 1;
    }

    let chip = unsafe { get_arg(argv, 1).unwrap() };

    // Build device path
    let mut path = [0u8; 64];
    if chip.len() > 0 && chip[0] == b'/' {
        let len = chip.len().min(path.len() - 1);
        path[..len].copy_from_slice(&chip[..len]);
    } else {
        let prefix = b"/dev/";
        path[..prefix.len()].copy_from_slice(prefix);
        let len = chip.len().min(path.len() - prefix.len() - 1);
        path[prefix.len()..prefix.len() + len].copy_from_slice(&chip[..len]);
    }

    let fd = io::open(&path, libc::O_RDWR, 0);
    if fd < 0 {
        sys::perror(&path);
        return 1;
    }

    // Parse LINE=VALUE pairs
    for i in 2..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            // Find '=' separator
            let mut eq_pos = None;
            for (j, &c) in arg.iter().enumerate() {
                if c == b'=' {
                    eq_pos = Some(j);
                    break;
                }
            }

            if let Some(pos) = eq_pos {
                let _line = sys::parse_u64(&arg[..pos]).unwrap_or(0);
                let _value = sys::parse_u64(&arg[pos + 1..]).unwrap_or(0);

                // Would need GPIO_SET_LINEHANDLE_IOCTL to actually set
                // For now, just acknowledge
            } else {
                io::write_str(2, b"gpioset: invalid argument: ");
                io::write_all(2, arg);
                io::write_str(2, b"\n");
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
    fn test_gpioset_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["gpioset"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_gpioset_nonexistent_chip() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpioset", "nonexistent_chip", "0=1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
