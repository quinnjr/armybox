//! killall - kill processes by name
//!
//! Send a signal to all processes with the given name.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// Table of symbolic signal names (without the "SIG" prefix) to numbers.
const SIGNALS: &[(&[u8], i32)] = &[
    (b"HUP", libc::SIGHUP),
    (b"INT", libc::SIGINT),
    (b"QUIT", libc::SIGQUIT),
    (b"KILL", libc::SIGKILL),
    (b"USR1", libc::SIGUSR1),
    (b"USR2", libc::SIGUSR2),
    (b"PIPE", libc::SIGPIPE),
    (b"ALRM", libc::SIGALRM),
    (b"TERM", libc::SIGTERM),
    (b"STOP", libc::SIGSTOP),
    (b"CONT", libc::SIGCONT),
];

/// Parse a signal specification: a bare number, a bare name (HUP), or a
/// name with the "SIG" prefix (SIGHUP). Case-insensitive.
fn parse_signal(spec: &[u8]) -> Option<i32> {
    if spec.is_empty() {
        return None;
    }
    if spec.iter().all(|c| c.is_ascii_digit()) {
        return sys::parse_i64(spec).map(|v| v as i32);
    }
    let name = if spec.len() > 3
        && spec[0].eq_ignore_ascii_case(&b'S')
        && spec[1].eq_ignore_ascii_case(&b'I')
        && spec[2].eq_ignore_ascii_case(&b'G')
    {
        &spec[3..]
    } else {
        spec
    };
    for (n, val) in SIGNALS.iter() {
        if n.eq_ignore_ascii_case(name) {
            return Some(*val);
        }
    }
    None
}

/// killall - kill processes by name
///
/// # Synopsis
/// ```text
/// killall [-SIGNAL] name
/// ```
///
/// # Description
/// Send a signal to all processes with the given name.
///
/// # Exit Status
/// - 0: At least one process was killed
/// - 1: No processes matched or an error occurred
pub fn killall(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"killall: no process name specified\n");
        return 1;
    }

    let mut signal = libc::SIGTERM;
    let mut name_idx = 1;

    // Parse signal argument
    if let Some(arg) = unsafe { get_arg(argv, 1) } {
        if let Some(&first) = arg.first() {
            if first == b'-' && arg.len() > 1 {
                match parse_signal(&arg[1..]) {
                    Some(sig) => signal = sig,
                    None => {
                        io::write_str(2, b"killall: invalid signal\n");
                        return 1;
                    }
                }
                name_idx = 2;
            }
        }
    }

    if name_idx >= argc {
        io::write_str(2, b"killall: no process name specified\n");
        return 1;
    }

    let target_name = unsafe { get_arg(argv, name_idx).unwrap() };
    let mut killed = 0;

    // Scan /proc for processes
    let fd = io::open(b"/proc", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        return 1;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        let mut offset = 0;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };

            if !name.is_empty() && name[0] >= b'0' && name[0] <= b'9' {
                // Read /proc/PID/comm
                let mut path = [0u8; 64];
                let mut pi = 0;
                for &c in b"/proc/" {
                    path[pi] = c;
                    pi += 1;
                }
                for &c in name {
                    path[pi] = c;
                    pi += 1;
                }
                for &c in b"/comm\0" {
                    path[pi] = c;
                    pi += 1;
                }

                let comm_fd = io::open(&path, libc::O_RDONLY, 0);
                if comm_fd >= 0 {
                    let mut comm_buf = [0u8; 256];
                    let n = io::read(comm_fd, &mut comm_buf);
                    io::close(comm_fd);

                    if n > 0 {
                        let comm = &comm_buf[..n as usize];
                        let comm = comm.split(|&c| c == b'\n').next().unwrap_or(comm);

                        if comm == target_name {
                            if let Some(pid) = sys::parse_i64(name) {
                                if unsafe { libc::kill(pid as i32, signal) } == 0 {
                                    killed += 1;
                                }
                            }
                        }
                    }
                }
            }
            offset += dirent.d_reclen as usize;
        }
    }
    io::close(fd);

    if killed == 0 { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_killall_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["killall"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("no process name"));
    }

    #[test]
    fn test_killall_nonexistent_process() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["killall", "nonexistent_process_name_12345"])
            .output()
            .unwrap();

        // Should fail because no such process exists
        assert_eq!(output.status.code(), Some(1));
    }
}
