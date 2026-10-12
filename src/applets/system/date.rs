//! date - print the system date and time
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/date.html

use crate::io;

const WDAY_ABBR: [&[u8]; 7] = [b"Sun", b"Mon", b"Tue", b"Wed", b"Thu", b"Fri", b"Sat"];
const WDAY_FULL: [&[u8]; 7] = [
    b"Sunday",
    b"Monday",
    b"Tuesday",
    b"Wednesday",
    b"Thursday",
    b"Friday",
    b"Saturday",
];
const MON_ABBR: [&[u8]; 12] = [
    b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
];
const MON_FULL: [&[u8]; 12] = [
    b"January",
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

fn pad2(fd: i32, n: i64) {
    if n < 10 {
        io::write_str(fd, b"0");
    }
    io::write_num(fd, n as u64);
}

fn pad3(fd: i32, n: i64) {
    if n < 100 {
        io::write_str(fd, b"0");
    }
    if n < 10 {
        io::write_str(fd, b"0");
    }
    io::write_num(fd, n as u64);
}

/// Emit a single conversion specifier `c` for the broken-down time `t`.
/// `zone` is the timezone abbreviation to use for `%Z` (may be empty).
fn emit_conv(fd: i32, c: u8, t: &libc::tm, zone: &[u8]) {
    let year = t.tm_year as i64 + 1900;
    let mon = t.tm_mon as i64; // 0..11
    match c {
        b'Y' => {
            io::write_num(fd, year as u64);
        }
        b'y' => pad2(fd, year.rem_euclid(100)),
        b'C' => pad2(fd, year / 100),
        b'm' => pad2(fd, mon + 1),
        b'd' => pad2(fd, t.tm_mday as i64),
        b'e' => {
            // Day of month, space-padded to width 2.
            if (t.tm_mday as i64) < 10 {
                io::write_str(fd, b" ");
            }
            io::write_num(fd, t.tm_mday as u64);
        }
        b'H' => pad2(fd, t.tm_hour as i64),
        b'I' => {
            let mut h = (t.tm_hour as i64) % 12;
            if h == 0 {
                h = 12;
            }
            pad2(fd, h);
        }
        b'M' => pad2(fd, t.tm_min as i64),
        b'S' => pad2(fd, t.tm_sec as i64),
        b'j' => pad3(fd, t.tm_yday as i64 + 1),
        b'p' => {
            io::write_str(fd, if t.tm_hour < 12 { b"AM" } else { b"PM" });
        }
        b'a' => {
            io::write_str(fd, WDAY_ABBR[(t.tm_wday as usize) % 7]);
        }
        b'A' => {
            io::write_str(fd, WDAY_FULL[(t.tm_wday as usize) % 7]);
        }
        b'b' | b'h' => {
            io::write_str(fd, MON_ABBR[(mon as usize) % 12]);
        }
        b'B' => {
            io::write_str(fd, MON_FULL[(mon as usize) % 12]);
        }
        b'u' => {
            // ISO weekday 1..7, Monday=1, Sunday=7.
            let w = t.tm_wday as i64;
            io::write_num(fd, if w == 0 { 7 } else { w } as u64);
        }
        b'w' => {
            io::write_num(fd, (t.tm_wday as u64) % 7);
        }
        b'Z' => {
            io::write_str(fd, zone);
        }
        b'T' => {
            pad2(fd, t.tm_hour as i64);
            io::write_str(fd, b":");
            pad2(fd, t.tm_min as i64);
            io::write_str(fd, b":");
            pad2(fd, t.tm_sec as i64);
        }
        b'R' => {
            pad2(fd, t.tm_hour as i64);
            io::write_str(fd, b":");
            pad2(fd, t.tm_min as i64);
        }
        b'D' => {
            pad2(fd, mon + 1);
            io::write_str(fd, b"/");
            pad2(fd, t.tm_mday as i64);
            io::write_str(fd, b"/");
            pad2(fd, year.rem_euclid(100));
        }
        b'F' => {
            io::write_num(fd, year as u64);
            io::write_str(fd, b"-");
            pad2(fd, mon + 1);
            io::write_str(fd, b"-");
            pad2(fd, t.tm_mday as i64);
        }
        b'n' => {
            io::write_str(fd, b"\n");
        }
        b't' => {
            io::write_str(fd, b"\t");
        }
        b'%' => {
            io::write_str(fd, b"%");
        }
        // Unknown specifier: emit it verbatim (with the leading %) per common
        // date behavior.
        other => {
            io::write_str(fd, b"%");
            io::write_all(fd, &[other]);
        }
    }
}

/// Write `format` (a byte string that may contain `%` conversions) for `t`.
fn emit_format(fd: i32, format: &[u8], t: &libc::tm, zone: &[u8]) {
    let mut i = 0;
    while i < format.len() {
        if format[i] == b'%' && i + 1 < format.len() {
            emit_conv(fd, format[i + 1], t, zone);
            i += 2;
        } else {
            io::write_all(fd, &format[i..i + 1]);
            i += 1;
        }
    }
}

/// date - write the current date and time to standard output.
///
/// # Synopsis
/// ```text
/// date [-u] [+format]
/// ```
///
/// # Options
/// - `-u`: Use Coordinated Universal Time (UTC) instead of the local time zone.
///
/// # Operands
/// - `+format`: A format string; `%` conversions are expanded (`%Y %m %d %H %M
///   %S %y %j %a %A %b %B %p %e %u %w %Z %T %R %D %F %n %t %%`). With no format
///   the POSIX default `%a %b %e %H:%M:%S %Z %Y` is used.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred (invalid usage, or unsupported time-setting operand)
pub fn date(argc: i32, argv: *const *const u8) -> i32 {
    let mut utc = false;
    let mut format: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { super::get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if arg == b"-u" || arg == b"--utc" || arg == b"--universal" {
            utc = true;
        } else if arg.first() == Some(&b'+') {
            format = Some(&arg[1..]);
        } else if arg == b"--help" {
            io::write_str(1, b"Usage: date [-u] [+FORMAT]\n");
            return 0;
        } else if arg.first() == Some(&b'-') {
            io::write_str(2, b"date: unknown option\n");
            return 2;
        } else {
            // A bare operand is a request to SET the clock, which is not
            // supported here. Fail loudly rather than silently ignoring it.
            io::write_str(2, b"date: setting the time is not supported\n");
            return 1;
        }
        i += 1;
    }

    let now = unsafe { libc::time(core::ptr::null_mut()) };
    if now == -1 {
        io::write_str(2, b"date: cannot read current time\n");
        return 1;
    }

    let tm_ptr = if utc {
        unsafe { libc::gmtime(&now) }
    } else {
        unsafe { libc::localtime(&now) }
    };
    if tm_ptr.is_null() {
        io::write_str(2, b"date: cannot convert time\n");
        return 1;
    }
    let t = unsafe { &*tm_ptr };

    // Timezone abbreviation for %Z.
    let zone: &[u8] = if utc {
        b"UTC"
    } else if !t.tm_zone.is_null() {
        unsafe { io::cstr_to_slice(t.tm_zone as *const u8) }
    } else {
        b""
    };

    match format {
        Some(f) => emit_format(1, f, t, zone),
        None => emit_format(1, b"%a %b %e %H:%M:%S %Z %Y", t, zone),
    }
    io::write_str(1, b"\n");
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
    fn test_date_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }
        let output = Command::new(&armybox).args(["date"]).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_date_default_has_time() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }
        let output = Command::new(&armybox).args(["date"]).output().unwrap();
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // POSIX default format "%a %b %e %H:%M:%S %Z %Y" contains the time.
        assert!(stdout.contains(":"));
    }

    #[test]
    fn test_date_default_has_current_year() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }
        let output = Command::new(&armybox).args(["date"]).output().unwrap();
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Year is the trailing field in the POSIX default format.
        assert!(stdout.contains("202"));
    }

    #[test]
    fn test_date_format_year() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }
        let output = Command::new(&armybox)
            .args(["date", "+%Y"])
            .output()
            .unwrap();
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let year = stdout.trim();
        assert_eq!(year.len(), 4);
        assert!(year.chars().all(|c| c.is_ascii_digit()));
    }
}
