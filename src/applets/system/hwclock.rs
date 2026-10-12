//! hwclock - query or set the hardware clock (RTC)
//!
//! Access the hardware real-time clock.

use super::get_arg;
use crate::io;
use crate::sys;

// RTC ioctl commands
const RTC_RD_TIME: crate::io::IoctlReq = 0x80247009u32 as crate::io::IoctlReq; // Read RTC time
const RTC_SET_TIME: crate::io::IoctlReq = 0x4024700au32 as crate::io::IoctlReq; // Set RTC time

/// RTC time structure (matches struct rtc_time)
#[repr(C)]
#[derive(Default)]
struct RtcTime {
    tm_sec: i32,   // Seconds (0-59)
    tm_min: i32,   // Minutes (0-59)
    tm_hour: i32,  // Hours (0-23)
    tm_mday: i32,  // Day of month (1-31)
    tm_mon: i32,   // Month (0-11)
    tm_year: i32,  // Year - 1900
    tm_wday: i32,  // Day of week (0-6, Sunday = 0)
    tm_yday: i32,  // Day of year (0-365)
    tm_isdst: i32, // Daylight saving time flag
}

/// hwclock - query or set the hardware clock (RTC)
///
/// # Synopsis
/// ```text
/// hwclock [-r|-w|-s] [-u] [-f device]
/// ```
///
/// # Description
/// Query or set the hardware real-time clock.
///
/// # Options
/// - `-r, --show`: Read hardware clock and print (default)
/// - `-w, --systohc`: Set hardware clock from system time
/// - `-s, --hctosys`: Set system time from hardware clock
/// - `-u, --utc`: Hardware clock is in UTC
/// - `-l, --localtime`: Hardware clock is in local time
/// - `-f, --rtc <dev>`: Use specified RTC device (default: /dev/rtc0)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn hwclock(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = Mode::Show;
    let mut utc = true;
    let mut device: &[u8] = b"/dev/rtc0";

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-r" || arg == b"--show" || arg == b"--read" {
            mode = Mode::Show;
        } else if arg == b"-w" || arg == b"--systohc" {
            mode = Mode::SysToHc;
        } else if arg == b"-s" || arg == b"--hctosys" {
            mode = Mode::HcToSys;
        } else if arg == b"-u" || arg == b"--utc" {
            utc = true;
        } else if arg == b"-l" || arg == b"--localtime" {
            utc = false;
        } else if arg == b"-f" || arg == b"--rtc" {
            i += 1;
            if let Some(d) = unsafe { get_arg(argv, i as i32) } {
                device = d;
            }
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        }
        i += 1;
    }

    // Open RTC device
    let fd = io::open(device, libc::O_RDONLY, 0);
    if fd < 0 {
        // Try fallback devices
        let fd = io::open(b"/dev/rtc", libc::O_RDONLY, 0);
        if fd < 0 {
            sys::perror(device);
            return 1;
        }
    }

    let result = match mode {
        Mode::Show => show_rtc_time(fd, utc),
        Mode::SysToHc => sys_to_hc(fd, utc),
        Mode::HcToSys => hc_to_sys(fd, utc),
    };

    io::close(fd);
    result
}

#[cfg(any(target_os = "linux", target_os = "android"))]
enum Mode {
    Show,
    SysToHc,
    HcToSys,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show_rtc_time(fd: i32, utc: bool) -> i32 {
    let mut rtc_tm = RtcTime::default();

    if unsafe { libc::ioctl(fd, RTC_RD_TIME, &mut rtc_tm) } < 0 {
        sys::perror(b"RTC_RD_TIME");
        return 1;
    }

    // Format and print the time
    let year = rtc_tm.tm_year + 1900;
    let month = rtc_tm.tm_mon + 1;

    let mut buf = [0u8; 16];

    // Print date
    io::write_all(1, sys::format_u64(year as u64, &mut buf));
    io::write_str(1, b"-");
    if month < 10 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sys::format_u64(month as u64, &mut buf));
    io::write_str(1, b"-");
    if rtc_tm.tm_mday < 10 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sys::format_u64(rtc_tm.tm_mday as u64, &mut buf));
    io::write_str(1, b" ");

    // Print time
    if rtc_tm.tm_hour < 10 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sys::format_u64(rtc_tm.tm_hour as u64, &mut buf));
    io::write_str(1, b":");
    if rtc_tm.tm_min < 10 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sys::format_u64(rtc_tm.tm_min as u64, &mut buf));
    io::write_str(1, b":");
    if rtc_tm.tm_sec < 10 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sys::format_u64(rtc_tm.tm_sec as u64, &mut buf));

    if utc {
        io::write_str(1, b" UTC");
    }
    io::write_str(1, b"\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn sys_to_hc(fd: i32, utc: bool) -> i32 {
    // Get current system time
    let mut tv: libc::timeval = unsafe { core::mem::zeroed() };
    if unsafe { libc::gettimeofday(&mut tv, core::ptr::null_mut()) } < 0 {
        sys::perror(b"gettimeofday");
        return 1;
    }

    // Convert to struct tm
    let time_t = tv.tv_sec;
    let tm = if utc {
        unsafe { libc::gmtime(&time_t) }
    } else {
        unsafe { libc::localtime(&time_t) }
    };

    if tm.is_null() {
        io::write_str(2, b"hwclock: cannot convert time\n");
        return 1;
    }

    let tm = unsafe { &*tm };

    let rtc_tm = RtcTime {
        tm_sec: tm.tm_sec,
        tm_min: tm.tm_min,
        tm_hour: tm.tm_hour,
        tm_mday: tm.tm_mday,
        tm_mon: tm.tm_mon,
        tm_year: tm.tm_year,
        tm_wday: tm.tm_wday,
        tm_yday: tm.tm_yday,
        tm_isdst: tm.tm_isdst,
    };

    // Need write access
    io::close(fd);
    let fd = io::open(b"/dev/rtc0", libc::O_WRONLY, 0);
    if fd < 0 {
        sys::perror(b"/dev/rtc0");
        return 1;
    }

    if unsafe { libc::ioctl(fd, RTC_SET_TIME, &rtc_tm) } < 0 {
        sys::perror(b"RTC_SET_TIME");
        io::close(fd);
        return 1;
    }

    io::close(fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn hc_to_sys(fd: i32, utc: bool) -> i32 {
    let mut rtc_tm = RtcTime::default();

    if unsafe { libc::ioctl(fd, RTC_RD_TIME, &mut rtc_tm) } < 0 {
        sys::perror(b"RTC_RD_TIME");
        return 1;
    }

    // Convert rtc_time to struct tm
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    tm.tm_sec = rtc_tm.tm_sec;
    tm.tm_min = rtc_tm.tm_min;
    tm.tm_hour = rtc_tm.tm_hour;
    tm.tm_mday = rtc_tm.tm_mday;
    tm.tm_mon = rtc_tm.tm_mon;
    tm.tm_year = rtc_tm.tm_year;
    tm.tm_isdst = -1; // Let mktime determine DST

    // Convert to time_t
    let time_t = if utc {
        unsafe { libc::timegm(&mut tm) }
    } else {
        unsafe { libc::mktime(&mut tm) }
    };

    if time_t < 0 {
        io::write_str(2, b"hwclock: cannot convert RTC time\n");
        return 1;
    }

    // Set system time
    let tv = libc::timeval {
        tv_sec: time_t,
        tv_usec: 0,
    };

    if unsafe { libc::settimeofday(&tv, core::ptr::null()) } < 0 {
        sys::perror(b"settimeofday");
        return 1;
    }

    0
}

fn print_usage() {
    io::write_str(1, b"Usage: hwclock [OPTIONS]\n\n");
    io::write_str(1, b"Query or set the hardware clock (RTC).\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(
        1,
        b"  -r, --show      Read and print hardware clock (default)\n",
    );
    io::write_str(
        1,
        b"  -w, --systohc   Set hardware clock from system time\n",
    );
    io::write_str(
        1,
        b"  -s, --hctosys   Set system time from hardware clock\n",
    );
    io::write_str(1, b"  -u, --utc       Hardware clock is in UTC (default)\n");
    io::write_str(1, b"  -l, --localtime Hardware clock is in local time\n");
    io::write_str(1, b"  -f, --rtc DEV   Use specified RTC device\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn hwclock(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"hwclock: only available on Linux\n");
    1
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
    fn test_hwclock_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["hwclock"]).output().unwrap();

        // hwclock requires root access to /dev/rtc
        // If we're not root, it will fail with permission denied
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Permission denied") {
            // Expected when not running as root
            return;
        }

        assert_eq!(output.status.code(), Some(0));
        // Should produce some output (date/time)
        assert!(!output.stdout.is_empty());
    }
}
