//! flock - manage locks from shell scripts
//!
//! Manage file locks from shell scripts.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// flock - manage locks from shell scripts
///
/// # Synopsis
/// ```text
/// flock lockfile command [arg...]
/// ```
///
/// # Description
/// Acquire an exclusive lock on a file and run a command while holding
/// the lock.
///
/// # Exit Status
/// - Command's exit status on success
/// - >0: An error occurred
pub fn flock(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"flock: missing operand\n");
        return 1;
    }

    let lock_file = unsafe { get_arg(argv, 1).unwrap() };
    let cmd = unsafe { get_arg(argv, 2).unwrap() };

    let mut lock_buf = [0u8; 4096];
    lock_buf[..lock_file.len()].copy_from_slice(lock_file);

    let fd = io::open(lock_file, libc::O_RDWR | libc::O_CREAT, 0o644);
    if fd < 0 {
        sys::perror(b"flock");
        return 1;
    }

    // Get exclusive lock
    if unsafe { libc::flock(fd, libc::LOCK_EX) } != 0 {
        io::close(fd);
        sys::perror(b"flock");
        return 1;
    }

    // Execute command
    let mut cmd_buf = [0u8; 4096];
    cmd_buf[..cmd.len()].copy_from_slice(cmd);
    let cmd_ptr = cmd_buf.as_ptr() as *const libc::c_char;
    let argv_ptrs = [cmd_ptr, core::ptr::null()];
    unsafe { libc::execv(cmd_ptr, argv_ptrs.as_ptr()) };
    io::close(fd);
    sys::perror(b"exec");
    1
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
    fn test_flock_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["flock"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_flock_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let lock_file = std::env::temp_dir().join(format!("armybox_flock_test_{}.lock", pid));

        let output = Command::new(&armybox)
            .args(["flock", lock_file.to_str().unwrap(), "/bin/true"])
            .output()
            .unwrap();

        let _ = std::fs::remove_file(&lock_file);
        assert_eq!(output.status.code(), Some(0));
    }
}
