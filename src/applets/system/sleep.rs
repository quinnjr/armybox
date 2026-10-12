//! sleep - delay for a specified amount of time
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sleep.html

use crate::applets::get_arg;
use crate::io;

/// Parse a POSIX sleep time operand into a number of seconds.
///
/// Accepts a non-negative decimal number, optionally fractional, with an
/// optional trailing unit suffix: `s` (seconds, default), `m` (minutes),
/// `h` (hours), or `d` (days). Returns `None` if the operand is not a
/// valid number.
fn parse_seconds(s: &[u8]) -> Option<f64> {
    if s.is_empty() {
        return None;
    }

    let mut end = s.len();
    let multiplier: f64 = match s[end - 1] {
        b's' => {
            end -= 1;
            1.0
        }
        b'm' => {
            end -= 1;
            60.0
        }
        b'h' => {
            end -= 1;
            3600.0
        }
        b'd' => {
            end -= 1;
            86400.0
        }
        _ => 1.0,
    };

    let num = &s[..end];
    if num.is_empty() {
        return None;
    }

    let mut int_val: f64 = 0.0;
    let mut frac_val: f64 = 0.0;
    let mut frac_div: f64 = 1.0;
    let mut seen_dot = false;
    let mut any_digit = false;

    for &c in num {
        if c == b'.' {
            if seen_dot {
                return None;
            }
            seen_dot = true;
            continue;
        }
        if !c.is_ascii_digit() {
            return None;
        }
        any_digit = true;
        if seen_dot {
            frac_div *= 10.0;
            frac_val += (c - b'0') as f64 / frac_div;
        } else {
            int_val = int_val * 10.0 + (c - b'0') as f64;
        }
    }

    if !any_digit {
        return None;
    }

    Some((int_val + frac_val) * multiplier)
}

/// sleep - delay for a specified amount of time
///
/// # Synopsis
/// ```text
/// sleep time
/// ```
///
/// # Description
/// Suspend execution for at least the integral number of seconds
/// specified by the time operand.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn sleep(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"sleep: missing operand\n");
        io::write_str(2, b"Usage: sleep TIME[s|m|h|d]\n");
        return 2;
    }

    let arg = match unsafe { get_arg(argv, 1) } {
        Some(a) => a,
        None => {
            io::write_str(2, b"sleep: missing operand\n");
            return 2;
        }
    };

    let secs = match parse_seconds(arg) {
        Some(v) if v.is_finite() && v >= 0.0 => v,
        _ => {
            io::write_str(2, b"sleep: invalid time interval '");
            io::write_all(2, arg);
            io::write_str(2, b"'\n");
            return 1;
        }
    };

    // Cast-based truncation (fptosi) avoids pulling in libm's trunc() in
    // this no_std binary; `secs` is already checked non-negative above.
    let secs_whole = secs as libc::time_t;
    let frac = secs - secs_whole as f64;
    let nanos = (frac * 1_000_000_000.0) as i64;

    let mut req = libc::timespec {
        tv_sec: secs_whole,
        tv_nsec: nanos as libc::c_long,
    };

    loop {
        let mut rem = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        let ret = unsafe { libc::nanosleep(&req, &mut rem) };
        if ret == 0 {
            break;
        }

        let errno = crate::sys::errno();
        if errno == libc::EINTR {
            // Interrupted by a signal; resume sleeping for the remaining time.
            req = rem;
            continue;
        }

        // Any other error: stop, but sleep already succeeded partially.
        break;
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::Instant;

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
    fn test_sleep_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let start = Instant::now();
        let output = Command::new(&armybox)
            .args(["sleep", "0"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should complete quickly
        assert!(start.elapsed().as_millis() < 500);
    }

    #[test]
    fn test_sleep_one_second() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let start = Instant::now();
        let output = Command::new(&armybox)
            .args(["sleep", "1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should take at least 1 second
        assert!(start.elapsed().as_millis() >= 900);
    }
}
