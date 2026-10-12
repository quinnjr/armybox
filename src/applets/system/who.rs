//! who - show who is logged in
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/who.html

use super::get_arg;
use crate::io;
use crate::sys;

// utmp entry types
const EMPTY: i16 = 0;
const RUN_LVL: i16 = 1;
const BOOT_TIME: i16 = 2;
const NEW_TIME: i16 = 3;
const OLD_TIME: i16 = 4;
const INIT_PROCESS: i16 = 5;
const LOGIN_PROCESS: i16 = 6;
const USER_PROCESS: i16 = 7;
const DEAD_PROCESS: i16 = 8;

// utmp structure for Linux (glibc compatible)
#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct Utmp {
    ut_type: i16,
    ut_pid: i32,
    ut_line: [u8; 32],
    ut_id: [u8; 4],
    ut_user: [u8; 32],
    ut_host: [u8; 256],
    ut_exit: ExitStatus,
    ut_session: i32,
    ut_tv: Timeval,
    ut_addr_v6: [i32; 4],
    __unused: [u8; 20],
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct ExitStatus {
    e_termination: i16,
    e_exit: i16,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct Timeval {
    tv_sec: i32,
    tv_usec: i32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const UTMP_SIZE: usize = core::mem::size_of::<Utmp>();

/// who - show who is logged in
///
/// # Synopsis
/// ```text
/// who [-a] [-b] [-d] [-H] [-l] [-m] [-p] [-q] [-r] [-s] [-t] [-T] [-u]
/// who am i
/// ```
///
/// # Description
/// Display information about currently logged-in users.
///
/// # Options
/// - `-a`: All - same as -b -d -l -p -r -t -T -u
/// - `-b`: Time of last system boot
/// - `-d`: Print dead processes
/// - `-H`: Print column headers
/// - `-l`: Print login processes
/// - `-m`: Only hostname and user for stdin
/// - `-q`: Quick mode - only names and count
/// - `-r`: Print current runlevel
/// - `-s`: Short format (default)
/// - `-t`: Print last system clock change
/// - `-T`, `-w`: Show terminal write status (+, -, ?)
/// - `-u`: Show idle time
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn who(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_header = false;
    let mut show_boot = false;
    let mut show_dead = false;
    let mut show_login = false;
    let mut show_runlevel = false;
    let mut show_time_change = false;
    let mut show_idle = false;
    let mut show_mesg = false;
    let mut quick_mode = false;
    let mut am_i = false;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"am" {
            // Check for "who am i"
            if i + 1 < argc as usize {
                if let Some(next) = unsafe { get_arg(argv, (i + 1) as i32) } {
                    if next == b"i" || next == b"I" {
                        am_i = true;
                        i += 1;
                    }
                }
            }
        } else if arg.starts_with(b"-") && arg.len() > 1 {
            for &c in &arg[1..] {
                match c {
                    b'a' => {
                        show_boot = true;
                        show_dead = true;
                        show_login = true;
                        show_runlevel = true;
                        show_time_change = true;
                        show_idle = true;
                        show_mesg = true;
                    }
                    b'b' => show_boot = true,
                    b'd' => show_dead = true,
                    b'H' => show_header = true,
                    b'l' => show_login = true,
                    b'm' => am_i = true,
                    b'q' => quick_mode = true,
                    b'r' => show_runlevel = true,
                    b's' => {} // Short format (default)
                    b't' => show_time_change = true,
                    b'T' | b'w' => show_mesg = true,
                    b'u' => show_idle = true,
                    _ => {}
                }
            }
        }
        i += 1;
    }

    // Get my tty for "who am i"
    let my_tty = if am_i { get_my_tty() } else { None };

    // Open utmp file
    let mut fd = io::open(b"/var/run/utmp\0", libc::O_RDONLY, 0);
    if fd < 0 {
        // Try alternate location
        fd = io::open(b"/run/utmp\0", libc::O_RDONLY, 0);
        // No utmp at all just means no login records (as with getutxent);
        // process_utmp's read on fd -1 fails and yields an empty listing
        if fd < 0 && sys::errno() != libc::ENOENT {
            io::write_str(2, b"who: cannot open utmp\n");
            return 1;
        }
    }

    process_utmp(
        fd,
        show_header,
        show_boot,
        show_dead,
        show_login,
        show_runlevel,
        show_time_change,
        show_idle,
        show_mesg,
        quick_mode,
        my_tty,
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_utmp(
    fd: i32,
    show_header: bool,
    show_boot: bool,
    show_dead: bool,
    show_login: bool,
    show_runlevel: bool,
    show_time_change: bool,
    show_idle: bool,
    show_mesg: bool,
    quick_mode: bool,
    my_tty: Option<[u8; 32]>,
) -> i32 {
    if show_header && !quick_mode {
        io::write_str(1, b"NAME     ");
        if show_mesg {
            io::write_str(1, b"S ");
        }
        io::write_str(1, b"LINE         TIME");
        if show_idle {
            io::write_str(1, b"         IDLE");
        }
        io::write_str(1, b"             COMMENT\n");
    }

    let mut buf = [0u8; UTMP_SIZE];
    let mut count = 0;
    let mut first_quick = true;

    loop {
        // Compare as isize: a failed read (-1) must end the loop too
        let n = io::read(fd, &mut buf);
        if n < UTMP_SIZE as isize {
            break;
        }

        // SAFETY: buf holds UTMP_SIZE initialized bytes; the byte buffer has
        // alignment 1, so copy the record out instead of referencing it
        let entry: Utmp = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const Utmp) };

        // Filter by type
        let show = match entry.ut_type {
            USER_PROCESS => true,
            BOOT_TIME => show_boot,
            DEAD_PROCESS => show_dead,
            LOGIN_PROCESS => show_login,
            RUN_LVL => show_runlevel,
            NEW_TIME | OLD_TIME => show_time_change,
            _ => false,
        };

        if !show {
            continue;
        }

        // Filter by tty for "who am i"
        if let Some(ref tty) = my_tty {
            if entry.ut_line != *tty {
                continue;
            }
        }

        // Get user name
        let user_len = entry.ut_user.iter().position(|&c| c == 0).unwrap_or(32);
        let user = &entry.ut_user[..user_len];

        if quick_mode {
            if !first_quick {
                io::write_str(1, b" ");
            }
            io::write_all(1, user);
            first_quick = false;
            count += 1;
            continue;
        }

        // Print entry
        match entry.ut_type {
            BOOT_TIME => {
                io::write_str(1, b"         system boot  ");
            }
            RUN_LVL => {
                io::write_str(1, b"         run-level ");
                let level = (entry.ut_pid & 0xFF) as u8;
                io::write_all(1, &[level]);
                io::write_str(1, b"  ");
            }
            NEW_TIME => {
                io::write_str(1, b"         new time     ");
            }
            OLD_TIME => {
                io::write_str(1, b"         old time     ");
            }
            _ => {
                // User name (8 chars padded)
                io::write_all(1, user);
                for _ in user_len..9 {
                    io::write_str(1, b" ");
                }
            }
        }

        // Terminal write status
        if show_mesg {
            let line_len = entry.ut_line.iter().position(|&c| c == 0).unwrap_or(32);
            let mesg = get_mesg_status(&entry.ut_line[..line_len]);
            io::write_all(1, &[mesg, b' ']);
        }

        // Line (terminal)
        let line_len = entry.ut_line.iter().position(|&c| c == 0).unwrap_or(32);
        io::write_all(1, &entry.ut_line[..line_len]);
        for _ in line_len..13 {
            io::write_str(1, b" ");
        }

        // Time
        format_time(entry.ut_tv.tv_sec as i64);

        // Idle time
        if show_idle && entry.ut_type == USER_PROCESS {
            io::write_str(1, b"   ");
            let idle = get_idle_time(&entry.ut_line[..line_len]);
            format_idle(idle);
        }

        // Host (comment)
        let host_len = entry.ut_host.iter().position(|&c| c == 0).unwrap_or(256);
        if host_len > 0 {
            io::write_str(1, b" (");
            io::write_all(1, &entry.ut_host[..host_len]);
            io::write_str(1, b")");
        }

        io::write_str(1, b"\n");
        count += 1;
    }

    if fd >= 0 {
        io::close(fd);
    }

    if quick_mode {
        io::write_str(1, b"\n# users=");
        let mut buf = [0u8; 16];
        io::write_all(1, sys::format_u64(count, &mut buf));
        io::write_str(1, b"\n");
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_my_tty() -> Option<[u8; 32]> {
    let mut tty_buf = [0u8; 256];

    // Get tty name from /proc/self/fd/0
    let n = unsafe {
        libc::readlink(
            b"/proc/self/fd/0\0".as_ptr() as *const libc::c_char,
            tty_buf.as_mut_ptr() as *mut libc::c_char,
            tty_buf.len() - 1,
        )
    };

    if n <= 0 {
        return None;
    }

    let tty_path = &tty_buf[..n as usize];

    // Extract line name (strip /dev/)
    let line = if tty_path.starts_with(b"/dev/") {
        &tty_path[5..]
    } else {
        tty_path
    };

    let mut result = [0u8; 32];
    let len = core::cmp::min(line.len(), 31);
    result[..len].copy_from_slice(&line[..len]);

    Some(result)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_mesg_status(line: &[u8]) -> u8 {
    let mut path = [0u8; 64];
    path[..5].copy_from_slice(b"/dev/");
    let len = core::cmp::min(line.len(), 58);
    path[5..5 + len].copy_from_slice(&line[..len]);

    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    let ret = unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) };

    if ret < 0 {
        return b'?';
    }

    if stat.st_mode as u32 & libc::S_IWGRP as u32 != 0 {
        b'+'
    } else {
        b'-'
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_idle_time(line: &[u8]) -> i64 {
    let mut path = [0u8; 64];
    path[..5].copy_from_slice(b"/dev/");
    let len = core::cmp::min(line.len(), 58);
    path[5..5 + len].copy_from_slice(&line[..len]);

    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    let ret = unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) };

    if ret < 0 {
        return -1;
    }

    let mut now: libc::timespec = unsafe { core::mem::zeroed() };
    unsafe { libc::clock_gettime(libc::CLOCK_REALTIME, &mut now) };

    now.tv_sec as i64 - stat.st_atime as i64
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn format_time(timestamp: i64) {
    // Convert timestamp to broken-down time
    let tm = unsafe {
        let t = timestamp as libc::time_t;
        libc::localtime(&t)
    };

    if tm.is_null() {
        io::write_str(1, b"????-??-?? ??:??");
        return;
    }

    let tm = unsafe { &*tm };
    let mut buf = [0u8; 20];

    // Format: YYYY-MM-DD HH:MM
    let year = (tm.tm_year + 1900) as u64;
    let mon = (tm.tm_mon + 1) as u64;
    let day = tm.tm_mday as u64;
    let hour = tm.tm_hour as u64;
    let min = tm.tm_min as u64;

    // Year
    buf[0] = b'0' + ((year / 1000) % 10) as u8;
    buf[1] = b'0' + ((year / 100) % 10) as u8;
    buf[2] = b'0' + ((year / 10) % 10) as u8;
    buf[3] = b'0' + (year % 10) as u8;
    buf[4] = b'-';
    buf[5] = b'0' + ((mon / 10) % 10) as u8;
    buf[6] = b'0' + (mon % 10) as u8;
    buf[7] = b'-';
    buf[8] = b'0' + ((day / 10) % 10) as u8;
    buf[9] = b'0' + (day % 10) as u8;
    buf[10] = b' ';
    buf[11] = b'0' + ((hour / 10) % 10) as u8;
    buf[12] = b'0' + (hour % 10) as u8;
    buf[13] = b':';
    buf[14] = b'0' + ((min / 10) % 10) as u8;
    buf[15] = b'0' + (min % 10) as u8;

    io::write_all(1, &buf[..16]);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn format_idle(seconds: i64) {
    if seconds < 0 {
        io::write_str(1, b"  ?  ");
        return;
    }

    if seconds < 60 {
        io::write_str(1, b"  .  ");
        return;
    }

    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        let mut buf = [0u8; 8];
        let s = sys::format_u64(days as u64, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b"days");
    } else if hours > 0 {
        let mut buf = [0u8; 8];
        let h = hours % 24;
        let m = minutes % 60;
        if h < 10 {
            io::write_str(1, b" ");
        }
        io::write_all(1, sys::format_u64(h as u64, &mut buf));
        io::write_str(1, b":");
        if m < 10 {
            io::write_str(1, b"0");
        }
        io::write_all(1, sys::format_u64(m as u64, &mut buf));
    } else {
        let mut buf = [0u8; 8];
        let m = minutes % 60;
        if m < 10 {
            io::write_str(1, b" ");
        }
        io::write_all(1, sys::format_u64(m as u64, &mut buf));
        io::write_str(1, b":00");
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn who(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"who: only available on Linux\n");
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
    fn test_who_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["who"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_who_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["who", "-H"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("NAME"));
    }

    #[test]
    fn test_who_quick() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["who", "-q"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("# users="));
    }
}
