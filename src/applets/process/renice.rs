//! renice - alter priority of running processes
//!
//! Change the scheduling priority of running processes.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// renice - alter priority of running processes
///
/// # Synopsis
/// ```text
/// renice [-n] priority [-p] pid...
/// renice [-n] priority -g pgrp...
/// renice [-n] priority -u user...
/// ```
///
/// # Description
/// Alter the scheduling priority of one or more running processes,
/// process groups, or all processes owned by one or more users.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred (e.g. bad ID, or setpriority failed for any target)
pub fn renice(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"renice: missing operand\n");
        return 1;
    }

    let mut idx: i32 = 1;

    // Optional -n flag before the priority value
    if let Some(arg) = unsafe { get_arg(argv, idx) } {
        if arg == b"-n" {
            idx += 1;
        }
    }

    if idx >= argc {
        io::write_str(2, b"renice: missing operand\n");
        return 1;
    }

    let prio_arg = unsafe { get_arg(argv, idx).unwrap() };
    let priority = match sys::parse_i64(prio_arg) {
        Some(p) => p as i32,
        None => {
            io::write_str(2, b"renice: invalid priority '");
            io::write_all(2, prio_arg);
            io::write_str(2, b"'\n");
            return 1;
        }
    };
    idx += 1;

    if idx >= argc {
        io::write_str(2, b"renice: missing operand\n");
        return 1;
    }

    // Default target type is PRIO_PROCESS unless -p/-g/-u is given.
    let mut mode = libc::PRIO_PROCESS;
    let mut had_targets = false;
    let mut ok = true;

    while idx < argc {
        let arg = match unsafe { get_arg(argv, idx) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-p" {
            mode = libc::PRIO_PROCESS;
            idx += 1;
            continue;
        } else if arg == b"-g" {
            mode = libc::PRIO_PGRP;
            idx += 1;
            continue;
        } else if arg == b"-u" {
            mode = libc::PRIO_USER;
            idx += 1;
            continue;
        }

        // arg is an id (pid, pgrp, or user)
        let id: u32 = if mode == libc::PRIO_USER {
            match sys::parse_i64(arg) {
                Some(n) if n >= 0 => n as u32,
                _ => match lookup_uid(arg) {
                    Some(uid) => uid,
                    None => {
                        io::write_str(2, b"renice: invalid user '");
                        io::write_all(2, arg);
                        io::write_str(2, b"'\n");
                        ok = false;
                        idx += 1;
                        continue;
                    }
                },
            }
        } else {
            match sys::parse_i64(arg) {
                Some(n) if n >= 0 => n as u32,
                _ => {
                    io::write_str(2, b"renice: invalid number '");
                    io::write_all(2, arg);
                    io::write_str(2, b"'\n");
                    ok = false;
                    idx += 1;
                    continue;
                }
            }
        };

        had_targets = true;
        if unsafe { libc::setpriority(mode, id, priority) } != 0 {
            sys::perror(b"renice");
            ok = false;
        }
        idx += 1;
    }

    if !had_targets {
        io::write_str(2, b"renice: missing operand\n");
        return 1;
    }

    if ok { 0 } else { 1 }
}

/// Look up a UID by username using getpwnam
fn lookup_uid(name: &[u8]) -> Option<u32> {
    let mut buf = [0u8; 256];
    let len = core::cmp::min(name.len(), buf.len() - 1);
    buf[..len].copy_from_slice(&name[..len]);
    buf[len] = 0;

    let pw = unsafe { libc::getpwnam(buf.as_ptr() as *const libc::c_char) };
    if pw.is_null() {
        None
    } else {
        Some(unsafe { (*pw).pw_uid })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
    fn test_renice_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["renice"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_renice_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        // Raising the nice value (lowering priority) is always permitted for
        // an unprivileged process; lowering it requires privilege. Use a
        // higher nice value so the call is always allowed.
        let output = Command::new(&armybox)
            .args(["renice", "5", &pid.to_string()])
            .output()
            .unwrap();

        // Succeeds when permitted; if the environment still denies it (e.g.
        // RLIMIT_NICE), the diagnostic must say so rather than silently pass.
        let code = output.status.code();
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(
            code == Some(0) || stderr.contains("Permission denied"),
            "unexpected renice outcome: code={:?} stderr={}",
            code,
            stderr
        );
    }
}
