//! nohup - run a command immune to hangups
//!
//! Run a command immune to hangups, with output to a non-tty.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// nohup - run a command immune to hangups
///
/// # Synopsis
/// ```text
/// nohup command [arg...]
/// ```
///
/// # Description
/// Run command immune to hangups. If stdout is a terminal, redirect it
/// to nohup.out. SIGHUP is ignored.
///
/// # Exit Status
/// - 126: Utility found but could not be executed
/// - 127: Utility not found
/// - Command's exit status otherwise
pub fn nohup(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"nohup: missing operand\n");
        return 1;
    }

    // Ignore SIGHUP
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }

    // Redirect stdout to nohup.out if it's a tty (best-effort)
    if unsafe { libc::isatty(1) } != 0 {
        let fd = io::open(
            b"nohup.out",
            libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
            0o644,
        );
        if fd >= 0 {
            io::dup2(fd, 1);
            io::close(fd);
        }
    }

    let cmd = unsafe { get_arg(argv, 1).unwrap() };
    let mut cmd_buf = [0u8; 4096];
    cmd_buf[..cmd.len()].copy_from_slice(cmd);
    cmd_buf[cmd.len()] = 0;

    unsafe {
        libc::execvp(
            cmd_buf.as_ptr() as *const libc::c_char,
            argv.add(1) as *const *const libc::c_char,
        );
    }

    let err = sys::errno();
    sys::perror(cmd);
    if err == libc::ENOENT { 127 } else { 126 }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_nohup_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nohup"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_nohup_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nohup", "/bin/true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
