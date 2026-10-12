//! rtcwake - enter a system sleep state until specified wakeup time
//!
//! Set an RTC alarm and suspend/hibernate the system.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// rtcwake - enter a system sleep state until specified wakeup time
///
/// # Synopsis
/// ```text
/// rtcwake [-m mode] [-s seconds]
/// ```
///
/// # Description
/// Enter a system sleep state until the specified wakeup time.
///
/// # Options
/// - `-m mode`: Standby, mem, disk, off, no, on, disable, show
/// - `-s seconds`: Seconds from now to wake
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn rtcwake(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = b"standby" as &[u8];
    let mut seconds: i64 = 0;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-m" && i + 1 < argc {
                mode = unsafe { get_arg(argv, i + 1).unwrap() };
                i += 2;
                continue;
            } else if arg == b"-s" && i + 1 < argc {
                if let Some(s) = unsafe { get_arg(argv, i + 1) } {
                    seconds = sys::parse_i64(s).unwrap_or(0);
                }
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    // Handle special modes
    if mode == b"show" {
        // Show current RTC time
        let now = unsafe { libc::time(core::ptr::null_mut()) };
        io::write_str(1, b"Current RTC time: ");
        io::write_signed(1, now as i64);
        io::write_str(1, b"\n");
        return 0;
    }

    if mode == b"disable" || mode == b"off" {
        io::write_str(1, b"rtcwake: alarm disabled\n");
        return 0;
    }

    if seconds <= 0 {
        io::write_str(2, b"rtcwake: no wake time specified\n");
        return 1;
    }

    // Calculate wake time
    let wake_time = unsafe { libc::time(core::ptr::null_mut()) } as i64 + seconds;

    io::write_str(1, b"rtcwake: wakeup scheduled for ");
    io::write_signed(1, wake_time);
    io::write_str(1, b"\n");

    // Write wake alarm to /sys/class/rtc/rtc0/wakealarm
    let fd = io::open(b"/sys/class/rtc/rtc0/wakealarm", libc::O_WRONLY, 0);
    if fd >= 0 {
        // Clear existing alarm first
        io::write_all(fd, b"0");
        io::close(fd);

        let fd = io::open(b"/sys/class/rtc/rtc0/wakealarm", libc::O_WRONLY, 0);
        if fd >= 0 {
            let mut buf = [0u8; 32];
            let s = sys::format_i64(wake_time, &mut buf);
            io::write_all(fd, s);
            io::close(fd);
        }
    }

    // Enter sleep state
    let state_path = b"/sys/power/state";
    let fd = io::open(state_path, libc::O_WRONLY, 0);
    if fd < 0 {
        sys::perror(state_path);
        return 1;
    }

    let state: &[u8] = if mode == b"mem" {
        b"mem"
    } else if mode == b"disk" {
        b"disk"
    } else if mode == b"standby" {
        b"standby"
    } else if mode == b"freeze" {
        b"freeze"
    } else {
        io::write_str(2, b"rtcwake: unknown mode\n");
        io::close(fd);
        return 1;
    };

    io::write_all(fd, state);
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
    fn test_rtcwake_show() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rtcwake", "-m", "show"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("RTC time"));
    }

    #[test]
    fn test_rtcwake_no_time() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["rtcwake"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
