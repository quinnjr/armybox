//! watch - execute a program periodically
//!
//! Run a command repeatedly and display its output.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// watch - execute a program periodically
///
/// # Synopsis
/// ```text
/// watch [-n seconds] command
/// ```
///
/// # Description
/// Execute a command repeatedly, displaying output each time.
/// The screen is cleared before each execution.
///
/// # Options
/// - `-n seconds`: Specify update interval (default: 2)
///
/// # Exit Status
/// - 0: Success (never returns normally)
/// - 1: Error
pub fn watch(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"watch: missing command\n");
        return 1;
    }

    let mut interval = 2u64;
    let mut cmd_start = 1;

    // Parse -n option
    if let Some(arg) = unsafe { get_arg(argv, 1) } {
        if arg == b"-n" && argc > 2 {
            if let Some(n) = unsafe { get_arg(argv, 2) } {
                interval = sys::parse_i64(n).unwrap_or(2) as u64;
            }
            cmd_start = 3;
        }
    }

    if cmd_start >= argc {
        return 1;
    }

    loop {
        // Clear screen
        io::write_str(1, b"\x1b[H\x1b[2J");

        // Run command
        let pid = io::fork();
        if pid == 0 {
            let cmd = unsafe { get_arg(argv, cmd_start).unwrap() };
            let mut cmd_buf = [0u8; 4096];
            cmd_buf[..cmd.len()].copy_from_slice(cmd);
            let shell = b"/bin/sh\0";
            let c_flag = b"-c\0";
            let argv_ptrs = [
                shell.as_ptr() as *const libc::c_char,
                c_flag.as_ptr() as *const libc::c_char,
                cmd_buf.as_ptr() as *const libc::c_char,
                core::ptr::null(),
            ];
            unsafe { libc::execv(shell.as_ptr() as *const libc::c_char, argv_ptrs.as_ptr()) };
            io::exit(1);
        }

        let mut status = 0;
        io::waitpid(pid, &mut status, 0);

        unsafe { libc::sleep(interval as u32) };
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_watch_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["watch"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing command"));
    }

    // Note: Can't easily test watch in a unit test since it runs forever
    // In real testing, would need to use timeout or signal handling
}
