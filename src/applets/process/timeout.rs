//! timeout - run a command with a time limit
//!
//! Start a command and kill it if still running after a specified time.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// timeout - run a command with a time limit
///
/// # Synopsis
/// ```text
/// timeout duration command [arg...]
/// ```
///
/// # Description
/// Start a command and kill it if still running after duration seconds.
///
/// # Exit Status
/// - 124: Command timed out
/// - 127: Command not found
/// - Command's exit status otherwise
pub fn timeout(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"timeout: missing operand\n");
        return 1;
    }

    let duration = sys::parse_i64(unsafe { get_arg(argv, 1).unwrap() }).unwrap_or(0);
    let cmd = unsafe { get_arg(argv, 2).unwrap() };

    let pid = io::fork();
    if pid == 0 {
        // Child: execute command
        let mut cmd_buf = [0u8; 4096];
        cmd_buf[..cmd.len()].copy_from_slice(cmd);
        let cmd_ptr = cmd_buf.as_ptr() as *const libc::c_char;
        let argv_ptrs = [cmd_ptr, core::ptr::null()];
        unsafe { libc::execv(cmd_ptr, argv_ptrs.as_ptr()) };
        io::exit(127);
    }

    // Parent: wait with timeout
    unsafe { libc::alarm(duration as u32) };

    let mut status = 0;
    let ret = io::waitpid(pid, &mut status, 0);

    unsafe { libc::alarm(0) }; // Cancel alarm

    if ret < 0 {
        // Likely timed out, kill the child
        unsafe { libc::kill(pid, libc::SIGKILL) };
        io::waitpid(pid, &mut status, 0);
        return 124; // Timeout exit code
    }

    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else {
        128 + libc::WTERMSIG(status)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;
    use std::time::Instant;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_timeout_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["timeout"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_timeout_fast_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let start = Instant::now();
        let output = Command::new(&armybox)
            .args(["timeout", "10", "/bin/true"])
            .output()
            .unwrap();

        // Should complete quickly (not wait for timeout)
        assert!(start.elapsed().as_secs() < 5);
        assert_eq!(output.status.code(), Some(0));
    }
}
