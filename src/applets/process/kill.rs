//! kill - send a signal to a process
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/kill.html

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

fn list_signals() {
    for (n, val) in SIGNALS.iter() {
        let mut buf = [0u8; 16];
        let mut i = 0;
        let mut v = *val;
        if v == 0 {
            buf[i] = b'0';
            i += 1;
        }
        let start = i;
        while v > 0 {
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
            i += 1;
        }
        buf[start..i].reverse();
        io::write_str(1, &buf[..i]);
        io::write_str(1, b") ");
        io::write_str(1, n);
        io::write_str(1, b"\n");
    }
}

/// kill - send a signal to a process
///
/// # Synopsis
/// ```text
/// kill [-s signal_name] pid...
/// kill -signal_number pid...
/// kill -signal_name pid...
/// kill -l
/// ```
///
/// # Description
/// Send the specified signal to the specified processes.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred, or an invalid signal/pid was given
pub fn kill(argc: i32, argv: *const *const u8) -> i32 {
    let mut signal = libc::SIGTERM;
    let mut failed = false;
    let mut any_pid = false;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if let Some(&first) = arg.first() {
                if first == b'-' && arg.len() > 1 {
                    if arg == b"-l" {
                        list_signals();
                        return 0;
                    }
                    if arg == b"-s" {
                        i += 1;
                        if i >= argc {
                            io::write_str(2, b"kill: option requires an argument -- 's'\n");
                            return 1;
                        }
                        let name_arg = unsafe { get_arg(argv, i) };
                        match name_arg.and_then(parse_signal) {
                            Some(sig) => signal = sig,
                            None => {
                                io::write_str(2, b"kill: invalid signal\n");
                                return 1;
                            }
                        }
                    } else {
                        match parse_signal(&arg[1..]) {
                            Some(sig) => signal = sig,
                            None => {
                                io::write_str(2, b"kill: invalid signal\n");
                                return 1;
                            }
                        }
                    }
                } else {
                    any_pid = true;
                    match sys::parse_i64(arg) {
                        Some(pid) => {
                            if unsafe { libc::kill(pid as i32, signal) } < 0 {
                                sys::perror(arg);
                                failed = true;
                            }
                        }
                        None => {
                            io::write_str(2, b"kill: invalid pid\n");
                            failed = true;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    if !any_pid {
        io::write_str(2, b"kill: usage: kill [-s signal|-signal] pid...\n");
        return 1;
    }

    if failed { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::AtomicUsize;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
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
    fn test_kill_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Kill with signal 0 (check if process exists) on our own PID
        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["kill", "-0", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_kill_nonexistent_pid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Try to kill a very high PID that doesn't exist
        let output = Command::new(&armybox)
            .args(["kill", "999999"])
            .output()
            .unwrap();

        // The kill(2) call fails (ESRCH) so kill should report failure.
        assert_eq!(output.status.code(), Some(1));
    }
}
