//! uname - print system information
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/uname.html

use crate::applets::{get_arg, has_opt};
use crate::io;

/// uname - print system information
///
/// # Synopsis
/// ```text
/// uname [-amnrsv]
/// ```
///
/// # Description
/// Print system information.
///
/// # Options
/// - `-a`: Print all information
/// - `-m`: Print machine hardware name
/// - `-n`: Print network node hostname
/// - `-r`: Print operating system release
/// - `-s`: Print operating system name (default)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn uname(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_all = false;
    let mut show_s = false;
    let mut show_n = false;
    let mut show_r = false;
    let mut show_m = false;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if has_opt(arg, b'a') {
                show_all = true;
            }
            if has_opt(arg, b's') {
                show_s = true;
            }
            if has_opt(arg, b'n') {
                show_n = true;
            }
            if has_opt(arg, b'r') {
                show_r = true;
            }
            if has_opt(arg, b'm') {
                show_m = true;
            }
        }
    }

    if show_all {
        show_s = true;
        show_n = true;
        show_r = true;
        show_m = true;
    }
    if !show_s && !show_n && !show_r && !show_m {
        show_s = true;
    }

    let mut uts: libc::utsname = unsafe { core::mem::zeroed() };
    if io::uname(&mut uts) != 0 {
        return 1;
    }

    if show_s {
        io::write_all(1, unsafe {
            io::cstr_to_slice(uts.sysname.as_ptr() as *const u8)
        });
        io::write_str(1, b" ");
    }
    if show_n {
        io::write_all(1, unsafe {
            io::cstr_to_slice(uts.nodename.as_ptr() as *const u8)
        });
        io::write_str(1, b" ");
    }
    if show_r {
        io::write_all(1, unsafe {
            io::cstr_to_slice(uts.release.as_ptr() as *const u8)
        });
        io::write_str(1, b" ");
    }
    if show_m {
        io::write_all(1, unsafe {
            io::cstr_to_slice(uts.machine.as_ptr() as *const u8)
        });
    }
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_uname_default() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["uname"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Linux"));
    }

    #[test]
    fn test_uname_all() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["uname", "-a"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain kernel name, hostname, release, machine
        assert!(stdout.contains("Linux"));
    }

    #[test]
    fn test_uname_machine() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["uname", "-m"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain architecture like x86_64 or aarch64
        assert!(!stdout.trim().is_empty());
    }
}
