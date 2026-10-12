//! w - show who is logged in and what they are doing
//!
//! Shows who is logged in and what they are doing.

extern crate alloc;

use crate::io;
use crate::sys;
use alloc::string::String;
use alloc::vec::Vec;

/// w - show who is logged in and what they are doing
///
/// # Synopsis
/// ```text
/// w [-h] [-s] [user]
/// ```
///
/// # Description
/// Display information about currently logged-in users and their processes.
///
/// # Options
/// - `-h`: Don't print the header
/// - `-s`: Short format
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn w(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_header = true;
    let mut short_format = false;
    let mut filter_user: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { super::super::get_arg(argv, i) } {
            if arg == b"-h" {
                show_header = false;
            } else if arg == b"-s" {
                short_format = true;
            } else if !arg.starts_with(b"-") {
                filter_user = Some(arg);
            }
        }
        i += 1;
    }

    if show_header {
        print_header();
    }

    // Print column header
    if short_format {
        io::write_str(1, b"USER     TTY      IDLE WHAT\n");
    } else {
        io::write_str(
            1,
            b"USER     TTY      FROM             LOGIN@   IDLE   JCPU   PCPU WHAT\n",
        );
    }

    // Get logged-in users from utmp
    let users = get_logged_in_users();

    for user in &users {
        if let Some(filter) = filter_user {
            if user.name.as_bytes() != filter {
                continue;
            }
        }

        if short_format {
            print_user_short(user);
        } else {
            print_user_long(user);
        }
    }

    0
}

/// User session information
struct UserSession {
    name: String,
    tty: String,
    host: String,
    login_time: i64,
    pid: i32,
}

/// Print the header line with uptime info
fn print_header() {
    // Get current time
    let now = unsafe { libc::time(core::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    unsafe {
        libc::localtime_r(&now, &mut tm);
    }

    // Format current time
    io::write_str(1, b" ");
    let mut buf = [0u8; 16];
    let hour = sys::format_u64(tm.tm_hour as u64, &mut buf);
    if hour.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, hour);
    io::write_str(1, b":");
    let min = sys::format_u64(tm.tm_min as u64, &mut buf);
    if min.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, min);
    io::write_str(1, b":");
    let sec = sys::format_u64(tm.tm_sec as u64, &mut buf);
    if sec.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, sec);

    // Get uptime
    let uptime = get_uptime();
    io::write_str(1, b" up ");
    format_uptime(uptime);

    // Get number of users
    let users = get_logged_in_users();
    io::write_str(1, b",  ");
    let num = sys::format_u64(users.len() as u64, &mut buf);
    io::write_all(1, num);
    if users.len() == 1 {
        io::write_str(1, b" user");
    } else {
        io::write_str(1, b" users");
    }

    // Get load averages
    io::write_str(1, b",  load average: ");
    let (load1, load5, load15) = get_load_average();
    io::write_all(1, &load1);
    io::write_str(1, b", ");
    io::write_all(1, &load5);
    io::write_str(1, b", ");
    io::write_all(1, &load15);

    io::write_str(1, b"\n");
}

/// Get system uptime in seconds
fn get_uptime() -> u64 {
    let fd = io::open(b"/proc/uptime", libc::O_RDONLY, 0);
    if fd < 0 {
        return 0;
    }

    let mut buf = [0u8; 64];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    // Parse first number (uptime in seconds)
    let content = &buf[..n as usize];
    let end = content
        .iter()
        .position(|&c| c == b' ' || c == b'.')
        .unwrap_or(content.len());
    sys::parse_u64(&content[..end]).unwrap_or(0)
}

/// Format uptime as "X days, HH:MM" or "HH:MM"
fn format_uptime(seconds: u64) {
    let mut buf = [0u8; 16];

    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;

    if days > 0 {
        let d = sys::format_u64(days, &mut buf);
        io::write_all(1, d);
        if days == 1 {
            io::write_str(1, b" day, ");
        } else {
            io::write_str(1, b" days, ");
        }
    }

    let h = sys::format_u64(hours, &mut buf);
    if h.len() < 2 {
        io::write_str(1, b" ");
    }
    io::write_all(1, h);
    io::write_str(1, b":");
    let m = sys::format_u64(mins, &mut buf);
    if m.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, m);
}

/// Get load averages from /proc/loadavg
fn get_load_average() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let fd = io::open(b"/proc/loadavg", libc::O_RDONLY, 0);
    if fd < 0 {
        return (b"0.00".to_vec(), b"0.00".to_vec(), b"0.00".to_vec());
    }

    let mut buf = [0u8; 128];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return (b"0.00".to_vec(), b"0.00".to_vec(), b"0.00".to_vec());
    }

    let content = &buf[..n as usize];
    let parts: Vec<&[u8]> = content.split(|&c| c == b' ').collect();

    let load1 = parts.first().copied().unwrap_or(b"0.00").to_vec();
    let load5 = parts.get(1).copied().unwrap_or(b"0.00").to_vec();
    let load15 = parts.get(2).copied().unwrap_or(b"0.00").to_vec();

    (load1, load5, load15)
}

/// Get logged-in users from /var/run/utmp or /run/utmp
fn get_logged_in_users() -> Vec<UserSession> {
    let mut users = Vec::new();

    // Try to read utmp
    let utmp_paths = [b"/var/run/utmp" as &[u8], b"/run/utmp"];

    let mut fd = -1;
    for path in &utmp_paths {
        fd = io::open(path, libc::O_RDONLY, 0);
        if fd >= 0 {
            break;
        }
    }

    if fd < 0 {
        // Fallback: try to get info from /proc
        return get_users_from_proc();
    }

    // Read utmp entries
    // utmp structure size varies by platform, but is typically 384 bytes on Linux x86_64
    const UTMP_SIZE: usize = 384;
    let mut entry = [0u8; UTMP_SIZE];

    while io::read(fd, &mut entry) == UTMP_SIZE as isize {
        // Check ut_type: USER_PROCESS = 7
        let ut_type = i16::from_ne_bytes([entry[0], entry[1]]) as i32;
        if ut_type != 7 {
            continue;
        }

        // Parse fields (Linux x86_64 offsets)
        // ut_user: offset 4, 32 bytes
        // ut_line: offset 40, 32 bytes
        // ut_host: offset 76, 256 bytes
        // ut_tv.tv_sec: offset 340, 4 bytes
        // ut_pid: offset 36, 4 bytes

        let name = extract_string(&entry[4..36]);
        let tty = extract_string(&entry[40..72]);
        let host = extract_string(&entry[76..332]);
        let login_time =
            i32::from_ne_bytes([entry[340], entry[341], entry[342], entry[343]]) as i64;
        let pid = i32::from_ne_bytes([entry[36], entry[37], entry[38], entry[39]]);

        if !name.is_empty() {
            users.push(UserSession {
                name,
                tty,
                host,
                login_time,
                pid,
            });
        }
    }

    io::close(fd);
    users
}

/// Extract null-terminated string from byte array
fn extract_string(data: &[u8]) -> String {
    let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).into_owned()
}

/// Fallback: get users from /proc
fn get_users_from_proc() -> Vec<UserSession> {
    let mut users = Vec::new();

    // Look for login shells in /proc
    // This is a fallback when utmp is not available

    // Try to read /proc entries
    let fd = io::open(b"/proc", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        return users;
    }
    io::close(fd);

    // Get current user as fallback
    let uid = unsafe { libc::getuid() };
    let name = get_username(uid);

    // Get current tty
    let tty = get_current_tty();

    users.push(UserSession {
        name,
        tty,
        host: String::new(),
        login_time: unsafe { libc::time(core::ptr::null_mut()) } as i64,
        pid: unsafe { libc::getpid() },
    });

    users
}

/// Get username from UID
fn get_username(uid: libc::uid_t) -> String {
    // Try to read /etc/passwd
    let fd = io::open(b"/etc/passwd", libc::O_RDONLY, 0);
    if fd < 0 {
        let mut buf = [0u8; 16];
        let s = sys::format_u64(uid as u64, &mut buf);
        return String::from_utf8_lossy(s).into_owned();
    }

    let content = io::read_all(fd);
    io::close(fd);

    // Parse passwd entries
    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&[u8]> = line.split(|&c| c == b':').collect();
        if parts.len() >= 3 {
            if let Some(entry_uid) = sys::parse_u64(parts[2]) {
                if entry_uid as u32 == uid {
                    return String::from_utf8_lossy(parts[0]).into_owned();
                }
            }
        }
    }

    let mut buf = [0u8; 16];
    let s = sys::format_u64(uid as u64, &mut buf);
    String::from_utf8_lossy(s).into_owned()
}

/// Get current tty name
fn get_current_tty() -> String {
    // Try /proc/self/fd/0
    let mut link_buf = [0u8; 256];
    let n = unsafe {
        libc::readlink(
            b"/proc/self/fd/0\0".as_ptr() as *const libc::c_char,
            link_buf.as_mut_ptr() as *mut libc::c_char,
            link_buf.len(),
        )
    };

    if n > 0 {
        let path = &link_buf[..n as usize];
        // Extract tty name (e.g., /dev/pts/0 -> pts/0)
        if path.starts_with(b"/dev/") {
            return String::from_utf8_lossy(&path[5..]).into_owned();
        }
        return String::from_utf8_lossy(path).into_owned();
    }

    String::from("?")
}

/// Get user's idle time
fn get_idle_time(tty: &str) -> u64 {
    let mut path = Vec::with_capacity(32);
    path.extend_from_slice(b"/dev/");
    path.extend_from_slice(tty.as_bytes());

    let mut stat_buf = io::stat_zeroed();
    if io::stat(&path, &mut stat_buf) != 0 {
        return 0;
    }

    let now = unsafe { libc::time(core::ptr::null_mut()) };
    (now - stat_buf.st_atime).max(0) as u64
}

/// Format idle time
fn format_idle(seconds: u64) -> Vec<u8> {
    let mut buf = [0u8; 16];

    if seconds < 60 {
        // Less than a minute: show seconds or nothing
        if seconds == 0 {
            return b"  .  ".to_vec();
        }
        let s = sys::format_u64(seconds, &mut buf);
        let mut result = Vec::new();
        for _ in 0..(5 - s.len()) {
            result.push(b' ');
        }
        result.extend_from_slice(s);
        result.push(b's');
        return result;
    }

    if seconds < 3600 {
        // Less than an hour: show minutes
        let mins = seconds / 60;
        let secs = seconds % 60;
        let mut result = Vec::new();
        let m = sys::format_u64(mins, &mut buf);
        for _ in 0..(2 - m.len()) {
            result.push(b' ');
        }
        result.extend_from_slice(m);
        result.push(b':');
        let s = sys::format_u64(secs, &mut buf);
        if s.len() < 2 {
            result.push(b'0');
        }
        result.extend_from_slice(s);
        return result;
    }

    if seconds < 86400 {
        // Less than a day: show hours:minutes
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        let mut result = Vec::new();
        let h = sys::format_u64(hours, &mut buf);
        result.extend_from_slice(h);
        result.push(b':');
        let m = sys::format_u64(mins, &mut buf);
        if m.len() < 2 {
            result.push(b'0');
        }
        result.extend_from_slice(m);
        result.push(b'm');
        return result;
    }

    // Days
    let days = seconds / 86400;
    let mut result = Vec::new();
    let d = sys::format_u64(days, &mut buf);
    result.extend_from_slice(d);
    result.extend_from_slice(b"days");
    result
}

/// Get the current process command for a user
fn get_user_what(pid: i32) -> String {
    // Try to read /proc/[pid]/cmdline
    let mut path = Vec::with_capacity(32);
    path.extend_from_slice(b"/proc/");
    let mut buf = [0u8; 16];
    let pid_str = sys::format_u64(pid as u64, &mut buf);
    path.extend_from_slice(pid_str);
    path.extend_from_slice(b"/cmdline");

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd < 0 {
        return String::from("-");
    }

    let mut cmdline = [0u8; 256];
    let n = io::read(fd, &mut cmdline);
    io::close(fd);

    if n <= 0 {
        return String::from("-");
    }

    // Replace nulls with spaces, get just the command name
    let cmd = &cmdline[..n as usize];
    let end = cmd.iter().position(|&c| c == 0).unwrap_or(cmd.len());
    let cmd_str = &cmd[..end];

    // Extract just the command name (basename)
    let start = cmd_str
        .iter()
        .rposition(|&c| c == b'/')
        .map(|p| p + 1)
        .unwrap_or(0);
    String::from_utf8_lossy(&cmd_str[start..]).into_owned()
}

/// Print user in short format
fn print_user_short(user: &UserSession) {
    // USER (8)
    io::write_all(1, user.name.as_bytes());
    for _ in user.name.len()..9 {
        io::write_str(1, b" ");
    }

    // TTY (8)
    let tty_display = if user.tty.starts_with("pts/") {
        &user.tty[4..]
    } else {
        &user.tty
    };
    io::write_all(1, tty_display.as_bytes());
    for _ in tty_display.len()..9 {
        io::write_str(1, b" ");
    }

    // IDLE
    let idle = get_idle_time(&user.tty);
    let idle_str = format_idle(idle);
    io::write_all(1, &idle_str);
    io::write_str(1, b" ");

    // WHAT
    let what = get_user_what(user.pid);
    io::write_all(1, what.as_bytes());
    io::write_str(1, b"\n");
}

/// Print user in long format
fn print_user_long(user: &UserSession) {
    let mut buf = [0u8; 16];

    // USER (8)
    io::write_all(1, user.name.as_bytes());
    for _ in user.name.len()..9 {
        io::write_str(1, b" ");
    }

    // TTY (8)
    let tty_display = if user.tty.starts_with("pts/") {
        &user.tty[4..]
    } else {
        &user.tty
    };
    io::write_all(1, tty_display.as_bytes());
    for _ in tty_display.len()..9 {
        io::write_str(1, b" ");
    }

    // FROM (16)
    let host = if user.host.is_empty() {
        "-"
    } else {
        &user.host
    };
    let host_display = if host.len() > 16 { &host[..16] } else { host };
    io::write_all(1, host_display.as_bytes());
    for _ in host_display.len()..17 {
        io::write_str(1, b" ");
    }

    // LOGIN@ (8)
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    let login_time = user.login_time as libc::time_t;
    unsafe {
        libc::localtime_r(&login_time, &mut tm);
    }
    let h = sys::format_u64(tm.tm_hour as u64, &mut buf);
    if h.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, h);
    io::write_str(1, b":");
    let m = sys::format_u64(tm.tm_min as u64, &mut buf);
    if m.len() < 2 {
        io::write_str(1, b"0");
    }
    io::write_all(1, m);
    io::write_str(1, b"    ");

    // IDLE (6)
    let idle = get_idle_time(&user.tty);
    let idle_str = format_idle(idle);
    io::write_all(1, &idle_str);
    io::write_str(1, b" ");

    // JCPU and PCPU (placeholders)
    io::write_str(1, b"  0.00s   0.00s ");

    // WHAT
    let what = get_user_what(user.pid);
    io::write_all(1, what.as_bytes());
    io::write_str(1, b"\n");
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_w_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["w"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_w_produces_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["w"]).output().unwrap();

        assert!(!output.stdout.is_empty());
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should have uptime info
        assert!(stdout.contains("up") || stdout.contains("USER"));
    }

    #[test]
    fn test_w_no_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["w", "-h"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should NOT have uptime line when -h is used
        assert!(!stdout.starts_with(" ") || stdout.starts_with("USER"));
    }
}
