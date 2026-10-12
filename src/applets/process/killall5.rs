//! killall5 - send a signal to all processes
//!
//! SystemV killall command - send a signal to all processes except init.

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

/// killall5 - send a signal to all processes
///
/// # Synopsis
/// ```text
/// killall5 [-SIGNAL]
/// ```
///
/// # Description
/// Send a signal to all processes except init (PID 1) and the calling process.
/// This is typically used during system shutdown.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn killall5(argc: i32, argv: *const *const u8) -> i32 {
    let mut signal = libc::SIGTERM;

    if argc > 1 {
        if let Some(arg) = unsafe { get_arg(argv, 1) } {
            if let Some(&first) = arg.first() {
                if first == b'-' && arg.len() > 1 {
                    match parse_signal(&arg[1..]) {
                        Some(sig) => signal = sig,
                        None => {
                            io::write_str(2, b"killall5: invalid signal\n");
                            return 1;
                        }
                    }
                }
            }
        }
    }

    let my_pid = unsafe { libc::getpid() };

    // Send signal to all processes except init and ourselves
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
                if let Some(pid) = sys::parse_i64(name) {
                    if pid > 1 && pid as i32 != my_pid {
                        let _ = unsafe { libc::kill(pid as i32, signal) };
                    }
                }
            }
            offset += dirent.d_reclen as usize;
        }
    }
    io::close(fd);
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_killall5_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Test with signal 0 (check permissions only, no actual signal)
        let output = Command::new(&armybox)
            .args(["killall5", "-0"])
            .output()
            .unwrap();

        // Should exit 0 regardless of whether it could signal all processes
        assert_eq!(output.status.code(), Some(0));
    }
}
