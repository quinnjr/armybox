//! free - display amount of free and used memory
//!
//! Display the total amount of free and used physical and swap memory.

use crate::io;

/// free - display amount of free and used memory
///
/// # Synopsis
/// ```text
/// free
/// ```
///
/// # Description
/// Display information about free and used memory on the system.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn free(_argc: i32, _argv: *const *const u8) -> i32 {
    let fd = io::open(b"/proc/meminfo", libc::O_RDONLY, 0);
    if fd >= 0 {
        let mut buf = [0u8; 2048];
        let n = io::read(fd, &mut buf);
        io::close(fd);
        if n > 0 {
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
    fn test_free_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["free"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_free_contains_memtotal() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["free"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // /proc/meminfo should contain MemTotal
        assert!(stdout.contains("MemTotal"));
    }

    #[test]
    fn test_free_contains_memfree() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["free"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("MemFree"));
    }
}
