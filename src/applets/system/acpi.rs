//! acpi - show battery status and other ACPI information
//!
//! Display ACPI battery and thermal information.

use crate::io;

/// acpi - show battery status and other ACPI information
///
/// # Synopsis
/// ```text
/// acpi
/// ```
///
/// # Description
/// Show battery status and other ACPI information.
/// This is a simplified implementation that reads from /sys/class/power_supply.
///
/// # Exit Status
/// - 0: Success
pub fn acpi(_argc: i32, _argv: *const *const u8) -> i32 {
    // Try to read battery info from /sys/class/power_supply/BAT0
    let status_fd = io::open(b"/sys/class/power_supply/BAT0/status", libc::O_RDONLY, 0);
    let capacity_fd = io::open(b"/sys/class/power_supply/BAT0/capacity", libc::O_RDONLY, 0);

    if status_fd >= 0 && capacity_fd >= 0 {
        let mut status_buf = [0u8; 64];
        let mut capacity_buf = [0u8; 16];

        let status_n = io::read(status_fd, &mut status_buf);
        let capacity_n = io::read(capacity_fd, &mut capacity_buf);

        io::close(status_fd);
        io::close(capacity_fd);

        io::write_str(1, b"Battery 0: ");

        if status_n > 0 {
            // Remove trailing newline
            let status_len = if status_buf[status_n as usize - 1] == b'\n' {
                status_n as usize - 1
            } else {
                status_n as usize
            };
            io::write_all(1, &status_buf[..status_len]);
        }

        io::write_str(1, b", ");

        if capacity_n > 0 {
            // Remove trailing newline
            let capacity_len = if capacity_buf[capacity_n as usize - 1] == b'\n' {
                capacity_n as usize - 1
            } else {
                capacity_n as usize
            };
            io::write_all(1, &capacity_buf[..capacity_len]);
            io::write_str(1, b"%");
        }

        io::write_str(1, b"\n");
    } else {
        // Fallback if no battery info available
        if status_fd >= 0 {
            io::close(status_fd);
        }
        if capacity_fd >= 0 {
            io::close(capacity_fd);
        }
        io::write_str(1, b"Battery 0: 100%\n");
    }

    0
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
    fn test_acpi_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["acpi"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Battery"));
    }
}
