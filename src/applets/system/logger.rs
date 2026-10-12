//! logger - enter messages into the system log
//!
//! Make entries in the system log using syslog.

extern crate alloc;

use super::super::get_arg;
use crate::io;
use alloc::vec::Vec;

// Syslog priorities (facility | level)
const LOG_KERN: i32 = 0 << 3;
const LOG_USER: i32 = 1 << 3;
const LOG_MAIL: i32 = 2 << 3;
const LOG_DAEMON: i32 = 3 << 3;
const LOG_AUTH: i32 = 4 << 3;
const LOG_SYSLOG: i32 = 5 << 3;
const LOG_LPR: i32 = 6 << 3;
const LOG_NEWS: i32 = 7 << 3;
const LOG_UUCP: i32 = 8 << 3;
const LOG_CRON: i32 = 9 << 3;
const LOG_LOCAL0: i32 = 16 << 3;
const LOG_LOCAL1: i32 = 17 << 3;
const LOG_LOCAL2: i32 = 18 << 3;
const LOG_LOCAL3: i32 = 19 << 3;
const LOG_LOCAL4: i32 = 20 << 3;
const LOG_LOCAL5: i32 = 21 << 3;
const LOG_LOCAL6: i32 = 22 << 3;
const LOG_LOCAL7: i32 = 23 << 3;

// Syslog severity levels
const LOG_EMERG: i32 = 0;
const LOG_ALERT: i32 = 1;
const LOG_CRIT: i32 = 2;
const LOG_ERR: i32 = 3;
const LOG_WARNING: i32 = 4;
const LOG_NOTICE: i32 = 5;
const LOG_INFO: i32 = 6;
const LOG_DEBUG: i32 = 7;

/// logger - enter messages into the system log
///
/// # Synopsis
/// ```text
/// logger [-is] [-f file] [-p priority] [-t tag] [message...]
/// ```
///
/// # Description
/// Make entries in the system log.
///
/// # Options
/// - `-i`: Log the process ID
/// - `-s`: Log to stderr as well as syslog
/// - `-f file`: Log each line from file
/// - `-p priority`: Specify priority (facility.level)
/// - `-t tag`: Specify tag for the log entry
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn logger(argc: i32, argv: *const *const u8) -> i32 {
    let mut log_pid = false;
    let mut log_stderr = false;
    let mut priority = LOG_USER | LOG_NOTICE;
    let mut tag: Option<&[u8]> = None;
    let mut file: Option<&[u8]> = None;
    let mut messages: Vec<&[u8]> = Vec::new();

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-i" {
                log_pid = true;
            } else if arg == b"-s" {
                log_stderr = true;
            } else if arg == b"-f" {
                i += 1;
                file = unsafe { get_arg(argv, i) };
            } else if arg == b"-p" {
                i += 1;
                if let Some(p) = unsafe { get_arg(argv, i) } {
                    priority = parse_priority(p);
                }
            } else if arg == b"-t" {
                i += 1;
                tag = unsafe { get_arg(argv, i) };
            } else if arg == b"--help" || arg == b"-h" {
                print_help();
                return 0;
            } else if arg == b"--" {
                // End of options, rest are messages
                i += 1;
                while i < argc {
                    if let Some(msg) = unsafe { get_arg(argv, i) } {
                        messages.push(msg);
                    }
                    i += 1;
                }
                break;
            } else if !arg.starts_with(b"-") {
                messages.push(arg);
            }
        }
        i += 1;
    }

    // Build tag with optional PID
    let pid = if log_pid {
        unsafe { libc::getpid() }
    } else {
        0
    };

    // Get default tag (program name)
    let default_tag = b"logger";
    let actual_tag = tag.unwrap_or(default_tag);

    // If reading from file
    if let Some(filepath) = file {
        return log_from_file(filepath, priority, actual_tag, pid, log_stderr);
    }

    // If no message, read from stdin
    if messages.is_empty() {
        return log_from_stdin(priority, actual_tag, pid, log_stderr);
    }

    // Log the message(s)
    let mut message = Vec::new();
    for (idx, msg) in messages.iter().enumerate() {
        if idx > 0 {
            message.push(b' ');
        }
        message.extend_from_slice(msg);
    }

    log_message(priority, actual_tag, pid, &message, log_stderr);

    0
}

/// Parse priority string (facility.level or numeric)
fn parse_priority(s: &[u8]) -> i32 {
    // Check for numeric
    if let Some(n) = crate::sys::parse_u64(s) {
        return n as i32;
    }

    // Parse facility.level format
    let mut facility = LOG_USER;
    let mut level = LOG_NOTICE;

    if let Some(dot_pos) = s.iter().position(|&c| c == b'.') {
        let fac_str = &s[..dot_pos];
        let lev_str = &s[dot_pos + 1..];

        facility = parse_facility(fac_str);
        level = parse_level(lev_str);
    } else {
        // Could be just a level or facility
        let parsed_level = parse_level(s);
        if parsed_level >= 0 {
            level = parsed_level;
        } else {
            facility = parse_facility(s);
        }
    }

    facility | level
}

/// Parse facility name
fn parse_facility(s: &[u8]) -> i32 {
    match s {
        b"kern" => LOG_KERN,
        b"user" => LOG_USER,
        b"mail" => LOG_MAIL,
        b"daemon" => LOG_DAEMON,
        b"auth" | b"security" => LOG_AUTH,
        b"syslog" => LOG_SYSLOG,
        b"lpr" => LOG_LPR,
        b"news" => LOG_NEWS,
        b"uucp" => LOG_UUCP,
        b"cron" => LOG_CRON,
        b"local0" => LOG_LOCAL0,
        b"local1" => LOG_LOCAL1,
        b"local2" => LOG_LOCAL2,
        b"local3" => LOG_LOCAL3,
        b"local4" => LOG_LOCAL4,
        b"local5" => LOG_LOCAL5,
        b"local6" => LOG_LOCAL6,
        b"local7" => LOG_LOCAL7,
        _ => LOG_USER,
    }
}

/// Parse level name
fn parse_level(s: &[u8]) -> i32 {
    match s {
        b"emerg" | b"panic" => LOG_EMERG,
        b"alert" => LOG_ALERT,
        b"crit" => LOG_CRIT,
        b"err" | b"error" => LOG_ERR,
        b"warning" | b"warn" => LOG_WARNING,
        b"notice" => LOG_NOTICE,
        b"info" => LOG_INFO,
        b"debug" => LOG_DEBUG,
        _ => -1,
    }
}

/// Log a single message
fn log_message(priority: i32, tag: &[u8], pid: i32, message: &[u8], to_stderr: bool) {
    // Try to write to /dev/log (syslog socket) or /var/log/messages
    let mut buf = Vec::with_capacity(1024);

    // Build syslog format: <priority>tag[pid]: message
    buf.push(b'<');
    let mut num_buf = [0u8; 16];
    let pri_str = crate::sys::format_u64(priority as u64, &mut num_buf);
    buf.extend_from_slice(pri_str);
    buf.push(b'>');

    // Add timestamp (Month Day HH:MM:SS)
    let now = unsafe { libc::time(core::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }

    const MONTHS: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];

    buf.extend_from_slice(MONTHS[tm.tm_mon as usize]);
    buf.push(b' ');

    let day_str = crate::sys::format_u64(tm.tm_mday as u64, &mut num_buf);
    if day_str.len() < 2 {
        buf.push(b' ');
    }
    buf.extend_from_slice(day_str);
    buf.push(b' ');

    let hour_str = crate::sys::format_u64(tm.tm_hour as u64, &mut num_buf);
    if hour_str.len() < 2 {
        buf.push(b'0');
    }
    buf.extend_from_slice(hour_str);
    buf.push(b':');

    let min_str = crate::sys::format_u64(tm.tm_min as u64, &mut num_buf);
    if min_str.len() < 2 {
        buf.push(b'0');
    }
    buf.extend_from_slice(min_str);
    buf.push(b':');

    let sec_str = crate::sys::format_u64(tm.tm_sec as u64, &mut num_buf);
    if sec_str.len() < 2 {
        buf.push(b'0');
    }
    buf.extend_from_slice(sec_str);
    buf.push(b' ');

    // Hostname (simplified)
    buf.extend_from_slice(b"localhost ");

    // Tag
    buf.extend_from_slice(tag);
    if pid > 0 {
        buf.push(b'[');
        let pid_str = crate::sys::format_u64(pid as u64, &mut num_buf);
        buf.extend_from_slice(pid_str);
        buf.push(b']');
    }
    buf.extend_from_slice(b": ");

    // Message
    buf.extend_from_slice(message);
    buf.push(b'\n');

    // Try /dev/log (Unix domain socket for syslog)
    let dev_log = b"/dev/log\0";
    let sock = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_DGRAM, 0) };

    if sock >= 0 {
        let mut addr: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        addr.sun_family = libc::AF_UNIX as u16;
        // Copy path
        for (i, &b) in dev_log.iter().take(107).enumerate() {
            addr.sun_path[i] = b as libc::c_char;
        }

        let ret = unsafe {
            libc::connect(
                sock,
                &addr as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            )
        };

        if ret == 0 {
            unsafe {
                libc::send(sock, buf.as_ptr() as *const _, buf.len(), 0);
            }
        }

        unsafe {
            libc::close(sock);
        }
    }

    // Fallback: write to /var/log/messages or syslog
    let fallback_paths = [b"/var/log/messages\0" as &[u8], b"/var/log/syslog\0"];

    for path in &fallback_paths {
        let fd = io::open(&path[..path.len() - 1], libc::O_WRONLY | libc::O_APPEND, 0);
        if fd >= 0 {
            io::write_all(fd, &buf);
            io::close(fd);
            break;
        }
    }

    // Also log to stderr if requested
    if to_stderr {
        // For stderr, use simpler format: tag[pid]: message
        let mut stderr_buf = Vec::new();
        stderr_buf.extend_from_slice(tag);
        if pid > 0 {
            stderr_buf.push(b'[');
            let pid_str = crate::sys::format_u64(pid as u64, &mut num_buf);
            stderr_buf.extend_from_slice(pid_str);
            stderr_buf.push(b']');
        }
        stderr_buf.extend_from_slice(b": ");
        stderr_buf.extend_from_slice(message);
        stderr_buf.push(b'\n');
        io::write_all(2, &stderr_buf);
    }
}

/// Log from file
fn log_from_file(path: &[u8], priority: i32, tag: &[u8], pid: i32, to_stderr: bool) -> i32 {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"logger: ");
        io::write_all(2, path);
        io::write_str(2, b": No such file or directory\n");
        return 1;
    }

    let content = io::read_all(fd);
    io::close(fd);

    // Log each line
    for line in content.split(|&c| c == b'\n') {
        if !line.is_empty() {
            log_message(priority, tag, pid, line, to_stderr);
        }
    }

    0
}

/// Log from stdin
fn log_from_stdin(priority: i32, tag: &[u8], pid: i32, to_stderr: bool) -> i32 {
    let content = io::read_all(0);

    // Log each line
    for line in content.split(|&c| c == b'\n') {
        if !line.is_empty() {
            log_message(priority, tag, pid, line, to_stderr);
        }
    }

    0
}

fn print_help() {
    io::write_str(1, b"Usage: logger [OPTIONS] [message]\n\n");
    io::write_str(1, b"Write message to system log.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -i          Log the process ID\n");
    io::write_str(1, b"  -s          Log to stderr as well\n");
    io::write_str(1, b"  -f FILE     Log each line from FILE\n");
    io::write_str(1, b"  -p PRIO     Priority (facility.level)\n");
    io::write_str(1, b"  -t TAG      Tag for log entries\n\n");
    io::write_str(
        1,
        b"Facilities: kern, user, mail, daemon, auth, syslog, lpr,\n",
    );
    io::write_str(1, b"            news, uucp, cron, local0-7\n\n");
    io::write_str(
        1,
        b"Levels: emerg, alert, crit, err, warning, notice, info, debug\n",
    );
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_logger_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["logger", "test message"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_logger_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["logger", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_logger_stderr() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["logger", "-s", "-t", "test", "hello world"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("test"));
        assert!(stderr.contains("hello world"));
    }
}
