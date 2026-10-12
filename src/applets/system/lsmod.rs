//! lsmod - show the status of modules in the Linux kernel
//!
//! Show the status of loaded kernel modules.

extern crate alloc;

use crate::io;
use alloc::vec::Vec;

/// lsmod - show the status of modules in the Linux kernel
///
/// # Synopsis
/// ```text
/// lsmod
/// ```
///
/// # Description
/// Display information about loaded kernel modules by reading /proc/modules
/// and formatting the output in columns.
///
/// # Output Format
/// Module: module name
/// Size: memory size in bytes
/// Used by: use count and list of modules depending on this one
///
/// # Exit Status
/// - 0: Success
pub fn lsmod(_argc: i32, _argv: *const *const u8) -> i32 {
    // Print header with proper column widths
    io::write_str(1, b"Module                  Size  Used by\n");

    let fd = io::open(b"/proc/modules", libc::O_RDONLY, 0);
    if fd < 0 {
        return 1;
    }

    let mut buf = [0u8; 65536];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    let content = &buf[..n as usize];

    // Parse each line
    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() {
            continue;
        }

        // /proc/modules format:
        // name size refcount dependencies state offset
        // Example: nbd 73728 3 - Live 0x0000000000000000
        let fields: Vec<&[u8]> = line
            .split(|&c| c == b' ')
            .filter(|s| !s.is_empty())
            .collect();

        if fields.len() < 3 {
            continue;
        }

        let name = fields[0];
        let size = fields[1];
        let refcount = fields[2];
        let deps = if fields.len() > 3 { fields[3] } else { b"-" };

        // Print module name (left-aligned, 19 chars wide + space)
        io::write_all(1, name);
        let name_padding = 20usize.saturating_sub(name.len());
        for _ in 0..name_padding {
            io::write_str(1, b" ");
        }

        // Print size (right-aligned, 8 chars wide)
        let size_padding = 8usize.saturating_sub(size.len());
        for _ in 0..size_padding {
            io::write_str(1, b" ");
        }
        io::write_all(1, size);
        io::write_str(1, b"  ");

        // Print refcount
        io::write_all(1, refcount);

        // Print dependencies (if any, excluding "-")
        if deps != b"-" && !deps.is_empty() {
            io::write_str(1, b" ");
            // Remove trailing comma if present
            if deps.ends_with(b",") {
                io::write_all(1, &deps[..deps.len() - 1]);
            } else {
                io::write_all(1, deps);
            }
        }

        io::write_str(1, b"\n");
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_lsmod_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["lsmod"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Module"));
    }

    #[test]
    fn test_lsmod_has_columns() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["lsmod"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Module"));
        assert!(stdout.contains("Size"));
        assert!(stdout.contains("Used by"));
    }
}
