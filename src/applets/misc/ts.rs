//! ts - timestamp input lines
//!
//! Prepends each line from stdin with a timestamp.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// ts - timestamp input lines
///
/// # Synopsis
/// ```text
/// ts [-s] [format]
/// ```
///
/// # Description
/// Prepends each line from stdin with a timestamp.
///
/// # Options
/// - `-s`: Use time since start instead of wall clock
/// - `-i`: Incremental mode (time since last line)
///
/// # Exit Status
/// - 0: Success
pub fn ts(argc: i32, argv: *const *const u8) -> i32 {
    let mut use_elapsed = false;
    let mut format: Option<&[u8]> = None;

    // Parse arguments
    for i in 1..argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => continue,
        };

        if arg == b"-s" {
            use_elapsed = true;
        } else if arg == b"-i" {
            // Incremental mode - ignore for now
        } else if !arg.starts_with(b"-") {
            format = Some(arg);
        }
    }

    let start_time = unsafe { libc::time(core::ptr::null_mut()) };
    let mut line = Vec::new();
    let mut buf = [0u8; 1];

    loop {
        // Read a line
        line.clear();
        loop {
            let n = io::read(0, &mut buf);
            if n <= 0 {
                if line.is_empty() {
                    return 0;
                }
                break;
            }
            if buf[0] == b'\n' {
                break;
            }
            line.push(buf[0]);
        }

        // Print timestamp
        let now = unsafe { libc::time(core::ptr::null_mut()) };

        if use_elapsed {
            let elapsed = (now - start_time) as u64;
            let hours = elapsed / 3600;
            let mins = (elapsed % 3600) / 60;
            let secs = elapsed % 60;

            let mut num_buf = [0u8; 8];

            let h = sys::format_u64(hours, &mut num_buf);
            if h.len() < 2 {
                io::write_str(1, b"0");
            }
            io::write_all(1, h);
            io::write_str(1, b":");

            let m = sys::format_u64(mins, &mut num_buf);
            if m.len() < 2 {
                io::write_str(1, b"0");
            }
            io::write_all(1, m);
            io::write_str(1, b":");

            let s = sys::format_u64(secs, &mut num_buf);
            if s.len() < 2 {
                io::write_str(1, b"0");
            }
            io::write_all(1, s);
        } else {
            // Format wall clock time
            format_timestamp(now as i64, format);
        }

        io::write_str(1, b" ");
        io::write_all(1, &line);
        io::write_str(1, b"\n");
    }
}

/// Format a timestamp
fn format_timestamp(time: i64, _format: Option<&[u8]>) {
    // Simple ISO-ish format: YYYY-MM-DD HH:MM:SS
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    unsafe {
        let t = time as libc::time_t;
        libc::localtime_r(&t, &mut tm);
    }

    let mut buf = [0u8; 8];

    // Year
    let year = sys::format_u64((tm.tm_year + 1900) as u64, &mut buf);
    io::write_all(1, year);
    io::write_str(1, b"-");

    // Month
    let month = sys::format_u64((tm.tm_mon + 1) as u64, &mut buf);
    if month.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, month);
    io::write_str(1, b"-");

    // Day
    let day = sys::format_u64(tm.tm_mday as u64, &mut buf);
    if day.len() < 2 {
        io::write_str(1, b"0");
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
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ts() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["ts"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"test line\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain timestamp and the line
        assert!(stdout.contains("test line"));
        assert!(stdout.contains("-")); // Date separator
    }
}
