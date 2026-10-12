//! cal - display a calendar
//!
//! Display a calendar for the current month.

use crate::io;

/// cal - display a calendar
///
/// # Synopsis
/// ```text
/// cal
/// ```
///
/// # Description
/// Display a calendar for the current month.
/// This is a simplified implementation.
///
/// # Exit Status
/// - 0: Success
pub fn cal(_argc: i32, _argv: *const *const u8) -> i32 {
    // Get current time
    let now = unsafe { libc::time(core::ptr::null_mut()) };
    let tm = unsafe { libc::localtime(&now) };

    if tm.is_null() {
        io::write_str(1, b"Su Mo Tu We Th Fr Sa\n");
        return 0;
    }

    let tm = unsafe { &*tm };

    // Month names
    let months = [
        b"January" as &[u8],
        b"February",
        b"March",
        b"April",
        b"May",
        b"June",
        b"July",
        b"August",
        b"September",
        b"October",
        b"November",
        b"December",
    ];

    let month = tm.tm_mon as usize;
    let year = tm.tm_year + 1900;
    let mday = tm.tm_mday;

    // Print header
    io::write_str(1, b"     ");
    io::write_all(1, months[month]);
    io::write_str(1, b" ");
    io::write_signed(1, year as i64);
    io::write_str(1, b"\n");
    io::write_str(1, b"Su Mo Tu We Th Fr Sa\n");

    // Calculate first day of month
    // Go back to day 1 of current month
    let days_back = mday - 1;
    let first_wday = (tm.tm_wday - days_back % 7 + 7) % 7;

    // Days in each month (non-leap year base)
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut days = days_in_month[month];

    // Check for leap year
    if month == 1 && (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)) {
        days = 29;
    }

    // Print leading spaces
    for _ in 0..first_wday {
        io::write_str(1, b"   ");
    }

    // Print days
    let mut wday = first_wday;
    for day in 1..=days {
        if day < 10 {
            io::write_str(1, b" ");
        }
        io::write_signed(1, day as i64);

        wday += 1;
        if wday == 7 {
            io::write_str(1, b"\n");
            wday = 0;
        } else {
            io::write_str(1, b" ");
        }
    }

    if wday != 0 {
        io::write_str(1, b"\n");
    }

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
    fn test_cal_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["cal"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain day headers
        assert!(stdout.contains("Su Mo Tu We Th Fr Sa"));
    }

    #[test]
    fn test_cal_has_month_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["cal"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain a year
        assert!(stdout.contains("202") || stdout.contains("203"));
    }
}
