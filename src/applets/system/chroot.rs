//! chroot - run command with a different root directory
//!
//! Change the root directory and run a command.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// chroot - run command with a different root directory
///
/// # Synopsis
/// ```text
/// chroot newroot [command]
/// ```
///
/// # Description
/// Change the root directory to newroot and run command (default: /bin/sh).
/// Requires root privileges.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn chroot(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"chroot: missing operand\n");
        return 1;
    }
    let newroot = unsafe { get_arg(argv, 1).unwrap() };
    let mut root_buf = [0u8; 4096];
    root_buf[..newroot.len()].copy_from_slice(newroot);

    if unsafe { libc::chroot(root_buf.as_ptr() as *const libc::c_char) } != 0 {
        sys::perror(b"chroot");
        return 1;
    }
    if unsafe { libc::chdir(b"/\0".as_ptr() as *const libc::c_char) } != 0 {
        sys::perror(b"chdir");
        return 1;
    }

    // Execute command if provided
    if argc > 2 {
        let cmd = unsafe { get_arg(argv, 2).unwrap() };
        let mut cmd_buf = [0u8; 4096];
        cmd_buf[..cmd.len()].copy_from_slice(cmd);
        let cmd_ptr = cmd_buf.as_ptr() as *const libc::c_char;
        let argv_ptrs = [cmd_ptr, core::ptr::null()];
        unsafe { libc::execv(cmd_ptr, argv_ptrs.as_ptr()) };
        sys::perror(b"exec");
        return 1;
    }

    // Default: run /bin/sh
    let shell = b"/bin/sh\0";
    let argv_ptrs = [shell.as_ptr() as *const libc::c_char, core::ptr::null()];
    unsafe { libc::execv(shell.as_ptr() as *const libc::c_char, argv_ptrs.as_ptr()) };
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
    fn test_chroot_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chroot"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing operand"));
    }

    #[test]
    fn test_chroot_requires_root() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // This should fail without root privileges
        let output = Command::new(&armybox)
            .args(["chroot", "/tmp"])
            .output()
            .unwrap();

        // Should fail (permission denied)
        assert_eq!(output.status.code(), Some(1));
    }
}
