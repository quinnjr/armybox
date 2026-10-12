//! chrt - manipulate the real-time attributes of a process
//!
//! Set or retrieve the real-time scheduling attributes of a process.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// chrt - manipulate the real-time attributes of a process
///
/// # Synopsis
/// ```text
/// chrt [options] priority command [args]
/// chrt [options] -p [priority] pid
/// ```
///
/// # Description
/// Set or retrieve the real-time scheduling attributes of an existing
/// process or run command with the given attributes.
///
/// # Options
/// - `-p`: Operate on existing process
/// - `-f`: Set SCHED_FIFO scheduling
/// - `-r`: Set SCHED_RR scheduling (default)
/// - `-o`: Set SCHED_OTHER scheduling
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn chrt(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"chrt: usage: chrt [-p] priority pid|command\n");
        return 1;
    }

    let mut policy = libc::SCHED_RR;
    let mut show_pid = false;
    let mut idx = 1;

    // Parse options
    while idx < argc {
        if let Some(arg) = unsafe { get_arg(argv, idx) } {
            if arg == b"-p" {
                show_pid = true;
            } else if arg == b"-f" {
                policy = libc::SCHED_FIFO;
            } else if arg == b"-r" {
                policy = libc::SCHED_RR;
            } else if arg == b"-o" {
                policy = sys::SCHED_OTHER;
            } else if arg[0] != b'-' {
                break;
            }
        }
        idx += 1;
    }

    if show_pid {
        // Show or set priority for existing PID
        if idx >= argc {
            io::write_str(2, b"chrt: missing pid\n");
            return 1;
        }

        let pid_str = unsafe { get_arg(argv, idx).unwrap() };
        let pid = sys::parse_i64(pid_str).unwrap_or(0) as i32;

        if pid <= 0 {
            io::write_str(2, b"chrt: invalid pid\n");
            return 1;
        }

        // Get current scheduling
        let current_policy = unsafe { libc::sched_getscheduler(pid) };
        if current_policy < 0 {
            sys::perror(b"sched_getscheduler");
            return 1;
        }

        let mut param: libc::sched_param = unsafe { core::mem::zeroed() };
        if unsafe { libc::sched_getparam(pid, &mut param) } != 0 {
            sys::perror(b"sched_getparam");
            return 1;
        }

        let policy_name = match current_policy {
            sys::SCHED_OTHER => b"SCHED_OTHER" as &[u8],
            libc::SCHED_FIFO => b"SCHED_FIFO",
            libc::SCHED_RR => b"SCHED_RR",
            libc::SCHED_BATCH => b"SCHED_BATCH",
            libc::SCHED_IDLE => b"SCHED_IDLE",
            _ => b"unknown",
        };

        io::write_str(1, b"pid ");
        io::write_signed(1, pid as i64);
        io::write_str(1, b"'s current scheduling policy: ");
        io::write_all(1, policy_name);
        io::write_str(1, b"\n");
        io::write_str(1, b"pid ");
        io::write_signed(1, pid as i64);
        io::write_str(1, b"'s current scheduling priority: ");
        io::write_signed(1, param.sched_priority as i64);
        io::write_str(1, b"\n");

        return 0;
    }

    // Need at least priority and command
    if idx + 1 >= argc {
        io::write_str(2, b"chrt: missing priority or command\n");
        return 1;
    }

    let prio_str = unsafe { get_arg(argv, idx).unwrap() };
    let priority = sys::parse_i64(prio_str).unwrap_or(0) as i32;
    idx += 1;

    // Fork and exec
    let pid = io::fork();
    if pid == 0 {
        // Child - set scheduling and exec
        let mut param: libc::sched_param = unsafe { core::mem::zeroed() };
        param.sched_priority = priority;

        if unsafe { libc::sched_setscheduler(0, policy, &param) } != 0 {
            sys::perror(b"sched_setscheduler");
            io::exit(1);
        }

        let cmd = unsafe { get_arg(argv, idx).unwrap() };
        let mut cmd_buf = [0u8; 4096];
        cmd_buf[..cmd.len()].copy_from_slice(cmd);

        unsafe {
            libc::execvp(
                cmd_buf.as_ptr() as *const libc::c_char,
                argv.add(idx as usize) as *const *const libc::c_char,
            );
        }
        sys::perror(cmd);
        io::exit(127);
    }

    // Parent - wait
    let mut status = 0;
    io::waitpid(pid, &mut status, 0);

    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::AtomicUsize;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_chrt_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chrt"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_chrt_show_pid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["chrt", "-p", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("scheduling"));
    }
}
