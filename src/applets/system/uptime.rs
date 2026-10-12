//! uptime - show how long the system has been running
//!
//! Display how long the system has been running.

use crate::io;

/// uptime - show how long the system has been running
///
/// # Synopsis
/// ```text
/// uptime
/// ```
///
/// # Description
/// Print how long the system has been running.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn uptime(_argc: i32, _argv: *const *const u8) -> i32 {
    let fd = io::open(b"/proc/uptime", libc::O_RDONLY, 0);
    if fd >= 0 {
        let mut buf = [0u8; 64];
        let n = io::read(fd, &mut buf);
        io::close(fd);
        if n > 0 {
            io::write_str(1, b"up ");
            io::write_all(1, &buf[..n as usize]);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_uptime_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["uptime"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_uptime_contains_up() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["uptime"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("up"));
    }
}
