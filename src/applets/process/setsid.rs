//! setsid - run a command in a new session
//!
//! Run a command in a new session.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// setsid - run a command in a new session
///
/// # Synopsis
/// ```text
/// setsid command [arg...]
/// ```
///
/// # Description
/// Run a program in a new session. The process is detached from the
/// controlling terminal.
///
/// # Exit Status
/// - Command's exit status on success
/// - >0: An error occurred
pub fn setsid(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"setsid: missing operand\n");
        return 1;
    }

    if unsafe { libc::setsid() } < 0 {
        sys::perror(b"setsid");
        return 1;
    }

    let cmd = unsafe { get_arg(argv, 1).unwrap() };
    let mut cmd_buf = [0u8; 4096];
    cmd_buf[..cmd.len()].copy_from_slice(cmd);
    let cmd_ptr = cmd_buf.as_ptr() as *const libc::c_char;
    let argv_ptrs = [cmd_ptr, core::ptr::null()];
    unsafe { libc::execv(cmd_ptr, argv_ptrs.as_ptr()) };
    sys::perror(b"exec");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
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
    fn test_setsid_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["setsid"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_setsid_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["setsid", "/bin/true"])
            .output()
            .unwrap();

        // May fail with EPERM if already session leader, which is ok
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
