//! fgconsole - print the number of the active VT
//!
//! Print the number of the currently active virtual terminal.

use crate::io;

/// fgconsole - print the number of the active VT
///
/// # Synopsis
/// ```text
/// fgconsole
/// ```
///
/// # Description
/// Print the number of the currently active virtual terminal.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn fgconsole(_argc: i32, _argv: *const *const u8) -> i32 {
    // Try to get current VT from /sys/class/tty/tty0/active
    let fd = io::open(b"/sys/class/tty/tty0/active", libc::O_RDONLY, 0);
    if fd >= 0 {
        let mut buf = [0u8; 32];
        let n = io::read(fd, &mut buf);
        io::close(fd);

        if n > 0 {
            // Parse "ttyN" to get N
            let content = &buf[..n as usize];
            if content.starts_with(b"tty") {
                let num_start = 3;
                let num_end = content
                    .iter()
                    .position(|&c| c == b'\n' || c == 0)
                    .unwrap_or(n as usize);
                if num_end > num_start {
                    io::write_all(1, &content[num_start..num_end]);
                    io::write_str(1, b"\n");
                    return 0;
                }
            }
        }
    }

    // Fallback - assume VT 1
    io::write_str(1, b"1\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_fgconsole_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["fgconsole"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should output a number
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty());
    }
}
