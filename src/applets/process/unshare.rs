//! unshare - run program in new namespaces
//!
//! Unshare namespaces and run a program.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

// Clone flags for namespaces
const CLONE_NEWNS: i32 = 0x00020000;
const CLONE_NEWUTS: i32 = 0x04000000;
const CLONE_NEWIPC: i32 = 0x08000000;
const CLONE_NEWNET: i32 = 0x40000000;
const CLONE_NEWPID: i32 = 0x20000000;
const CLONE_NEWUSER: i32 = 0x10000000;

/// unshare - run program in new namespaces
///
/// # Synopsis
/// ```text
/// unshare [options] command [args]
/// ```
///
/// # Description
/// Create new namespaces and run a program in them.
///
/// # Options
/// - `-m`: Unshare mount namespace
/// - `-u`: Unshare UTS namespace
/// - `-i`: Unshare IPC namespace
/// - `-n`: Unshare network namespace
/// - `-p`: Unshare PID namespace
/// - `-U`: Unshare user namespace
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn unshare(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"unshare: usage: unshare [options] command\n");
        return 1;
    }

    let mut flags: i32 = 0;
    let mut cmd_idx: i32 = -1;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-m" {
                flags |= CLONE_NEWNS;
            } else if arg == b"-u" {
                flags |= CLONE_NEWUTS;
            } else if arg == b"-i" {
                flags |= CLONE_NEWIPC;
            } else if arg == b"-n" {
                flags |= CLONE_NEWNET;
            } else if arg == b"-p" {
                flags |= CLONE_NEWPID;
            } else if arg == b"-U" {
                flags |= CLONE_NEWUSER;
            } else if arg[0] != b'-' {
                cmd_idx = i;
                break;
            }
        }
        i += 1;
    }

    if cmd_idx < 0 {
        io::write_str(2, b"unshare: missing command\n");
        return 1;
    }

    // Call unshare syscall
    if flags != 0 {
        if unsafe { libc::unshare(flags) } != 0 {
            sys::perror(b"unshare");
            return 1;
        }
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
    fn test_unshare_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["unshare"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_unshare_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Run true with no namespace changes
        let output = Command::new(&armybox)
            .args(["unshare", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
