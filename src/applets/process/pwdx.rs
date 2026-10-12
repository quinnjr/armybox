//! pwdx - report current working directory of a process
//!
//! Print the current working directory of a process.

use crate::applets::get_arg;
use crate::io;

/// pwdx - report current working directory of a process
///
/// # Synopsis
/// ```text
/// pwdx pid
/// ```
///
/// # Description
/// Print the current working directory of the process with the given PID.
///
/// # Exit Status
/// - 0: Success
/// - 1: An error occurred (invalid PID, permission denied, etc.)
pub fn pwdx(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let pid = unsafe { get_arg(argv, 1).unwrap() };

    let mut path = [0u8; 64];
    let mut pi = 0;
    for &c in b"/proc/" {
        path[pi] = c;
        pi += 1;
    }
    for &c in pid {
        path[pi] = c;
        pi += 1;
    }
    for &c in b"/cwd\0" {
        path[pi] = c;
        pi += 1;
    }

    let mut link_buf = [0u8; 4096];
    let n = unsafe {
        libc::readlink(
            path.as_ptr() as *const libc::c_char,
            link_buf.as_mut_ptr() as *mut libc::c_char,
            link_buf.len() - 1,
        )
    };
    if n > 0 {
        io::write_all(1, pid);
        io::write_str(1, b": ");
        io::write_all(1, &link_buf[..n as usize]);
        io::write_str(1, b"\n");
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::AtomicUsize;

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
    fn test_pwdx_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["pwdx"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_pwdx_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Get the cwd of our own process
        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["pwdx", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(&pid.to_string()));
        assert!(stdout.contains(":"));
    }

    #[test]
    fn test_pwdx_invalid_pid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["pwdx", "999999999"])
            .output()
            .unwrap();

        // Should fail - no such process
        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_pwdx_pid_1() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // PID 1 always exists, but reading its cwd may require root
        let output = Command::new(&armybox).args(["pwdx", "1"]).output().unwrap();

        // May succeed (if root) or fail (permission denied)
        // Just check it runs without crashing
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));

        // If it succeeded, verify output format
        if output.status.code() == Some(0) {
            let stdout = std::string::String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("1:"));
        }
    }
}
