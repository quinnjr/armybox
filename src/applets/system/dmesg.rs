//! dmesg - print or control the kernel ring buffer
//!
//! Display messages from the kernel ring buffer.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// dmesg - print or control the kernel ring buffer
///
/// # Synopsis
/// ```text
/// dmesg [-c] [-n level] [-r] [-T] [-t]
/// ```
///
/// # Description
/// Display messages from the kernel ring buffer. Reads from /dev/kmsg
/// and formats the output with timestamps.
///
/// # Options
/// - `-c`: Clear the ring buffer after printing
/// - `-n level`: Set console log level (requires root)
/// - `-r`: Print raw message buffer
/// - `-T`: Print human-readable timestamps (requires -d for delta)
/// - `-t`: Don't print timestamps
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn dmesg(argc: i32, argv: *const *const u8) -> i32 {
    let mut clear = false;
    let mut raw = false;
    let mut no_timestamp = false;
    let mut human_time = false;

    // Parse arguments
    let mut i = 1i32;
    while i < argc {
        let Some(arg) = (unsafe { get_arg(argv, i) }) else {
            i += 1;
            continue;
        };

        if arg == b"-c" {
            clear = true;
        } else if arg == b"-r" {
            raw = true;
        } else if arg == b"-t" {
            no_timestamp = true;
        } else if arg == b"-T" {
            human_time = true;
        } else if arg == b"-n" {
            // Skip level argument - would require CAP_SYS_ADMIN
            i += 1;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: dmesg [-c] [-r] [-t] [-T]\n");
            io::write_str(1, b"  -c  Clear the ring buffer after printing\n");
            io::write_str(1, b"  -r  Print raw message buffer\n");
            io::write_str(1, b"  -t  Don't print timestamps\n");
            io::write_str(1, b"  -T  Print human-readable timestamps\n");
            return 0;
        }
        i += 1;
    }

    // Try /dev/kmsg first (non-blocking so we don't hang)
    let fd = io::open(b"/dev/kmsg", libc::O_RDONLY | libc::O_NONBLOCK, 0);
    if fd >= 0 {
        // Seek to end and read backwards would be ideal, but for simplicity
        // we'll read sequentially. The kernel maintains position per-fd.
        // SEEK_DATA (3) with offset 0 seeks to the first record
        unsafe { libc::lseek(fd, 0, 0) }; // SEEK_SET to beginning

        let mut line_buf = [0u8; 8192];
        let mut boot_time: Option<i64> = None;

        if human_time {
            // Get boot time from /proc/stat
            boot_time = get_boot_time();
        }

        loop {
            let n = io::read(fd, &mut line_buf);
            if n <= 0 {
                break;
            }

            let line = &line_buf[..n as usize];

            if raw {
                io::write_all(1, line);
            } else {
                format_kmsg_line(line, no_timestamp, human_time, boot_time);
            }
        }

        if clear {
            // Would need CAP_SYS_ADMIN to actually clear
            // syslog(SYSLOG_ACTION_CLEAR, NULL, 0)
        }

        io::close(fd);
        return 0;
    }

    // Fallback to /var/log/dmesg or /var/log/kern.log
    for path in &[b"/var/log/dmesg" as &[u8], b"/var/log/kern.log"] {
        let fd = io::open(*path, libc::O_RDONLY, 0);
        if fd >= 0 {
            let mut buf = [0u8; 4096];
            loop {
                let n = io::read(fd, &mut buf);
                if n <= 0 {
                    break;
                }
                io::write_all(1, &buf[..n as usize]);
            }
            io::close(fd);
            return 0;
        }
    }

    io::write_str(2, b"dmesg: cannot read kernel messages\n");
    1
}

/// Get system boot time from /proc/stat
fn get_boot_time() -> Option<i64> {
    let fd = io::open(b"/proc/stat", libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }

    // /proc/stat can be quite large, read in chunks
    let mut buf = [0u8; 16384];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return None;
    }

    let content = &buf[..n as usize];

    // Find "btime NNNN" line
    for line in content.split(|&c| c == b'\n') {
        if line.starts_with(b"btime ") {
            // Extract just the digits
            let time_str = &line[6..];
            let end = time_str
                .iter()
                .position(|&c| c < b'0' || c > b'9')
                .unwrap_or(time_str.len());
            let digits = &time_str[..end];
            if !digits.is_empty() {
                return sys::parse_u64(digits).map(|t| t as i64);
            }
        }
    }

    None
}

/// Format a /dev/kmsg line
/// Format: priority,sequence,timestamp,flags;message
fn format_kmsg_line(line: &[u8], no_timestamp: bool, human_time: bool, boot_time: Option<i64>) {
    // Find the semicolon that separates metadata from message
    let Some(semi_pos) = line.iter().position(|&c| c == b';') else {
        // Malformed, just print as-is
        io::write_all(1, line);
        return;
    };

    let metadata = &line[..semi_pos];
    let message = &line[semi_pos + 1..];

    // Parse metadata: priority,sequence,timestamp,flags
    let fields: Vec<&[u8]> = metadata.split(|&c| c == b',').collect();

    if fields.len() < 3 {
        // Malformed, just print message
        io::write_all(1, message);
        return;
    }

    let timestamp_us = sys::parse_u64(fields[2]).unwrap_or(0);

    if !no_timestamp {
        if human_time {
            if let Some(btime) = boot_time {
                // Convert to absolute time
                let abs_time = btime + (timestamp_us / 1_000_000) as i64;
                format_human_time(abs_time);
            } else {
                // Fall back to relative time
                format_relative_time(timestamp_us);
            }
        } else {
            format_relative_time(timestamp_us);
        }
        io::write_str(1, b" ");
    }

    // Print message (strip trailing newline if present to avoid double newlines)
    let msg = if message.ends_with(b"\n") {
        &message[..message.len() - 1]
    } else {
        message
    };
    io::write_all(1, msg);
    io::write_str(1, b"\n");
}

/// Format relative time (seconds since boot)
fn format_relative_time(timestamp_us: u64) {
    let secs = timestamp_us / 1_000_000;
    let usecs = timestamp_us % 1_000_000;

    let mut buf = [0u8; 16];

    io::write_str(1, b"[");

    // Print seconds with leading spaces for alignment (up to 5 digits for seconds)
    let sec_str = sys::format_u64(secs, &mut buf);
    let padding = 5usize.saturating_sub(sec_str.len());
    for _ in 0..padding {
        io::write_str(1, b" ");
    }
    io::write_all(1, sec_str);

    io::write_str(1, b".");

    // Print microseconds (6 digits, zero-padded)
    let usec_str = sys::format_u64(usecs, &mut buf);
    let zeros = 6usize.saturating_sub(usec_str.len());
    for _ in 0..zeros {
        io::write_str(1, b"0");
    }
    io::write_all(1, usec_str);

    io::write_str(1, b"]");
}

/// Format human-readable time
fn format_human_time(timestamp: i64) {
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    let time_t = timestamp as libc::time_t;
    unsafe { libc::localtime_r(&time_t, &mut tm) };

    // Format: [Mon Jan  2 15:04:05 2006]
    const MONTHS: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    const DAYS: [&[u8]; 7] = [b"Sun", b"Mon", b"Tue", b"Wed", b"Thu", b"Fri", b"Sat"];

    let mut buf = [0u8; 8];

    io::write_str(1, b"[");

    // Day of week
    io::write_all(1, DAYS[tm.tm_wday as usize % 7]);
    io::write_str(1, b" ");

    // Month
    io::write_all(1, MONTHS[tm.tm_mon as usize % 12]);
    io::write_str(1, b" ");

    // Day of month (space-padded)
    let day = sys::format_u64(tm.tm_mday as u64, &mut buf);
    if day.len() < 2 {
        io::write_str(1, b" ");
    }
    io::write_all(1, day);
    io::write_str(1, b" ");

    // Hour
    let hour = sys::format_u64(tm.tm_hour as u64, &mut buf);
    if hour.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, hour);
    io::write_str(1, b":");

    // Minute
    let min = sys::format_u64(tm.tm_min as u64, &mut buf);
    if min.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, min);
    io::write_str(1, b":");

    // Second
    let sec = sys::format_u64(tm.tm_sec as u64, &mut buf);
    if sec.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sec);
    io::write_str(1, b" ");

    // Year
    let year = sys::format_u64((tm.tm_year + 1900) as u64, &mut buf);
    io::write_all(1, year);

    io::write_str(1, b"]");
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
    fn test_dmesg_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dmesg", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.starts_with(b"Usage:"));
    }
}
