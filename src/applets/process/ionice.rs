//! ionice - set or get process I/O scheduling class and priority
//!
//! Get or set the I/O scheduling class and priority for a process.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

// I/O scheduling classes
const IOPRIO_CLASS_NONE: i32 = 0;
const IOPRIO_CLASS_RT: i32 = 1;
const IOPRIO_CLASS_BE: i32 = 2;
const IOPRIO_CLASS_IDLE: i32 = 3;

const IOPRIO_WHO_PROCESS: i32 = 1;

fn ioprio_prio_value(class: i32, data: i32) -> i32 {
    (class << 13) | data
}

fn ioprio_prio_class(mask: i32) -> i32 {
    mask >> 13
}

fn ioprio_prio_data(mask: i32) -> i32 {
    mask & 0x1fff
}

/// ionice - set or get process I/O scheduling class and priority
///
/// # Synopsis
/// ```text
/// ionice [-c class] [-n level] [-p pid] [command [args]]
/// ```
///
/// # Description
/// Get or set the I/O scheduling class and priority for a process.
///
/// # Options
/// - `-c class`: Scheduling class (0=none, 1=realtime, 2=best-effort, 3=idle)
/// - `-n level`: Priority level (0-7, lower is higher priority)
/// - `-p pid`: Operate on existing process
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn ionice(argc: i32, argv: *const *const u8) -> i32 {
    let mut class: i32 = -1;
    let mut level: i32 = -1;
    let mut pid: i32 = 0;
    let mut cmd_idx: i32 = -1;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-c" && i + 1 < argc {
                if let Some(c) = unsafe { get_arg(argv, i + 1) } {
                    class = sys::parse_i64(c).unwrap_or(2) as i32;
                }
                i += 2;
                continue;
            } else if arg == b"-n" && i + 1 < argc {
                if let Some(n) = unsafe { get_arg(argv, i + 1) } {
                    level = sys::parse_i64(n).unwrap_or(4) as i32;
                }
                i += 2;
                continue;
            } else if arg == b"-p" && i + 1 < argc {
                if let Some(p) = unsafe { get_arg(argv, i + 1) } {
                    pid = sys::parse_i64(p).unwrap_or(0) as i32;
                }
                i += 2;
                continue;
            } else if arg[0] != b'-' {
                cmd_idx = i;
                break;
            }
        }
        i += 1;
    }

    // If no class/level specified and no command, show current priority
    if class < 0 && level < 0 && cmd_idx < 0 {
        if pid == 0 {
            pid = unsafe { libc::getpid() };
        }

        let prio = unsafe { libc::syscall(libc::SYS_ioprio_get, IOPRIO_WHO_PROCESS, pid) } as i32;
        if prio < 0 {
            sys::perror(b"ioprio_get");
            return 1;
        }

        let cur_class = ioprio_prio_class(prio);
        let cur_data = ioprio_prio_data(prio);

        let class_names = [b"none" as &[u8], b"realtime", b"best-effort", b"idle"];
        let class_name = if cur_class < 4 {
            class_names[cur_class as usize]
        } else {
            b"unknown"
        };

        io::write_all(1, class_name);
        if cur_class == IOPRIO_CLASS_RT || cur_class == IOPRIO_CLASS_BE {
            io::write_str(1, b": prio ");
            io::write_signed(1, cur_data as i64);
        }
        io::write_str(1, b"\n");
        return 0;
    }

    // Set defaults
    if class < 0 {
        class = IOPRIO_CLASS_BE;
    }
    if level < 0 {
        level = 4;
    }

    let prio = ioprio_prio_value(class, level);

    if cmd_idx < 0 {
        // Set priority on existing process
        if pid == 0 {
            pid = unsafe { libc::getpid() };
        }

        if unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, pid, prio) } != 0 {
            sys::perror(b"ioprio_set");
            return 1;
        }
        return 0;
    }

    // Fork and exec command with new priority
    let child_pid = io::fork();
    if child_pid == 0 {
        // Child - set priority and exec
        if unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0, prio) } != 0 {
            sys::perror(b"ioprio_set");
            io::exit(1);
        }

        // Build argv for exec
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
        io::exit(127);
    }

    // Parent - wait for child
    let mut status = 0;
    io::waitpid(child_pid, &mut status, 0);

    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ionice_show_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ionice"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should show a class name
        assert!(
            stdout.contains("best-effort")
                || stdout.contains("none")
                || stdout.contains("realtime")
                || stdout.contains("idle")
        );
    }

    #[test]
    fn test_ionice_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ionice", "-c", "2", "-n", "7", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
