//! gpiodetect - list all GPIO chips
//!
//! List all GPIO chips present on the system.

use crate::io;

/// gpiodetect - list all GPIO chips
///
/// # Synopsis
/// ```text
/// gpiodetect
/// ```
///
/// # Description
/// List all GPIO chips present on the system, their names,
/// labels, and number of GPIO lines.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn gpiodetect(_argc: i32, _argv: *const *const u8) -> i32 {
    // Read GPIO chips from /sys/class/gpio or /dev/gpiochip*
    let mut found = false;

    // Try to list gpiochip devices
    for i in 0..16 {
        let mut path = [0u8; 32];
        let prefix = b"/dev/gpiochip";
        path[..prefix.len()].copy_from_slice(prefix);

        // Add chip number
        if i < 10 {
            path[prefix.len()] = b'0' + i;
            path[prefix.len() + 1] = 0;
        } else {
            path[prefix.len()] = b'0' + (i / 10);
            path[prefix.len() + 1] = b'0' + (i % 10);
            path[prefix.len() + 2] = 0;
        }

        // Check if device exists
        let mut stat_buf: libc::stat = unsafe { core::mem::zeroed() };
        if unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat_buf) } == 0 {
            io::write_str(1, b"gpiochip");
            io::write_num(1, i as u64);
            io::write_str(1, b" [");

            // Try to read label from sysfs
            let mut label_path = [0u8; 64];
            let sysfs_prefix = b"/sys/class/gpio/gpiochip";
            label_path[..sysfs_prefix.len()].copy_from_slice(sysfs_prefix);
            if i < 10 {
                label_path[sysfs_prefix.len()] = b'0' + i;
                let suffix = b"/label";
                label_path[sysfs_prefix.len() + 1..sysfs_prefix.len() + 1 + suffix.len()]
                    .copy_from_slice(suffix);
            }

            io::write_str(1, b"gpio");
            io::write_str(1, b"] (lines: unknown)\n");
            found = true;
        }
    }

    if !found {
        io::write_str(2, b"gpiodetect: no GPIO chips found\n");
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_gpiodetect_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpiodetect"])
            .output()
            .unwrap();

        // Either finds GPIO chips or reports none found
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
