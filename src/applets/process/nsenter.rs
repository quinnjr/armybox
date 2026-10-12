//! nsenter - run program in different namespaces
//!
//! Enter the namespaces of another process.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// nsenter - run program in different namespaces
///
/// # Synopsis
/// ```text
/// nsenter [options] [-t pid] command [args]
/// ```
///
/// # Description
/// Run a program in the namespaces of another process.
///
/// # Options
/// - `-t pid`: Target process to get namespaces from
/// - `-m`: Enter mount namespace
/// - `-u`: Enter UTS namespace
/// - `-i`: Enter IPC namespace
/// - `-n`: Enter network namespace
/// - `-p`: Enter PID namespace
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn nsenter(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"nsenter: usage: nsenter -t pid [options] command\n");
        return 1;
    }

    let mut target_pid: i32 = 0;
    let mut enter_mount = false;
    let mut enter_uts = false;
    let mut enter_ipc = false;
    let mut enter_net = false;
    let mut enter_pid = false;
    let mut cmd_idx: i32 = -1;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-t" && i + 1 < argc {
                if let Some(pid_str) = unsafe { get_arg(argv, i + 1) } {
                    target_pid = sys::parse_i64(pid_str).unwrap_or(0) as i32;
                }
                i += 2;
                continue;
            } else if arg == b"-m" {
                enter_mount = true;
            } else if arg == b"-u" {
                enter_uts = true;
            } else if arg == b"-i" {
                enter_ipc = true;
            } else if arg == b"-n" {
                enter_net = true;
            } else if arg == b"-p" {
                enter_pid = true;
            } else if arg[0] != b'-' {
                cmd_idx = i;
                break;
            }
        }
        i += 1;
    }

    if target_pid <= 0 {
        io::write_str(2, b"nsenter: missing or invalid target pid\n");
        return 1;
    }

    if cmd_idx < 0 {
        io::write_str(2, b"nsenter: missing command\n");
        return 1;
    }

    // Enter namespaces
    let ns_types = [
        (enter_mount, b"mnt" as &[u8]),
        (enter_uts, b"uts"),
        (enter_ipc, b"ipc"),
        (enter_net, b"net"),
        (enter_pid, b"pid"),
    ];

    for (should_enter, ns_name) in ns_types.iter() {
        if !should_enter {
            continue;
        }

        let mut ns_path = [0u8; 64];
        let mut pi = 0;
        for &c in b"/proc/" {
            ns_path[pi] = c;
            pi += 1;
        }

        let mut pid_buf = [0u8; 16];
        let pid_str = sys::format_i64(target_pid as i64, &mut pid_buf);
        for &c in pid_str {
            ns_path[pi] = c;
            pi += 1;
        }

        for &c in b"/ns/" {
            ns_path[pi] = c;
            pi += 1;
        }
        for &c in *ns_name {
            ns_path[pi] = c;
            pi += 1;
        }

        let ns_fd = io::open(&ns_path[..pi], libc::O_RDONLY, 0);
        if ns_fd < 0 {
            sys::perror(&ns_path[..pi]);
            return 1;
        }

        if unsafe { libc::setns(ns_fd, 0) } != 0 {
            sys::perror(b"setns");
            io::close(ns_fd);
            return 1;
        }
        io::close(ns_fd);
    }

    // Exec the command
    let cmd = unsafe { get_arg(argv, cmd_idx).unwrap() };
    let mut cmd_buf = [0u8; 4096];
    cmd_buf[..cmd.len()].copy_from_slice(cmd);

    unsafe {
        libc::execvp(
            cmd_buf.as_ptr() as *const libc::c_char,
            argv.add(cmd_idx as usize) as *const *const libc::c_char,
        );
    }
    sys::perror(cmd);
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_nsenter_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nsenter"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_nsenter_missing_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nsenter", "-t", "1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
