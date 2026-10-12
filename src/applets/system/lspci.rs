//! lspci - list all PCI devices
//!
//! Display information about PCI buses and devices.

use crate::io;

/// lspci - list all PCI devices
///
/// # Synopsis
/// ```text
/// lspci
/// ```
///
/// # Description
/// List all PCI devices by reading /sys/bus/pci/devices.
///
/// # Exit Status
/// - 0: Success
pub fn lspci(_argc: i32, _argv: *const *const u8) -> i32 {
    let dir = io::opendir(b"/sys/bus/pci/devices");
    if dir.is_null() {
        io::write_str(1, b"00:00.0 Host bridge: Unknown\n");
        return 0;
    }

    loop {
        let entry = io::readdir(dir);
        if entry.is_null() {
            break;
        }

        let d_name = unsafe { &(*entry).d_name };
        let name_len = d_name.iter().position(|&c| c == 0).unwrap_or(d_name.len());
        let name = &d_name[..name_len];
        let name_u8: &[u8] = unsafe { core::mem::transmute(name) };

        // Skip . and ..
        if name_u8 == b"." || name_u8 == b".." {
            continue;
        }

        // Format: DDDD:BB:SS.F -> BB:SS.F
        // Skip the domain prefix if present
        let display_name = if name_u8.len() > 8 && name_u8[4] == b':' {
            &name_u8[5..]
        } else {
            name_u8
        };

        // Read class
        let mut class_path = [0u8; 256];
        let mut pi = 0;
        for &c in b"/sys/bus/pci/devices/" {
            class_path[pi] = c;
            pi += 1;
        }
        for &c in name_u8 {
            class_path[pi] = c;
            pi += 1;
        }
        for &c in b"/class" {
            class_path[pi] = c;
            pi += 1;
        }

        let class_fd = io::open(&class_path[..pi], libc::O_RDONLY, 0);
        let class_code = if class_fd >= 0 {
            let mut buf = [0u8; 16];
            let n = io::read(class_fd, &mut buf);
            io::close(class_fd);

            if n > 2 {
                // Parse hex class code (0xNNNNNN)
                let code_str = &buf[2..n as usize];
                let mut code: u32 = 0;
                for &c in code_str {
                    if c == b'\n' || c == 0 {
                        break;
                    }
                    let digit = match c {
                        b'0'..=b'9' => c - b'0',
                        b'a'..=b'f' => c - b'a' + 10,
                        b'A'..=b'F' => c - b'A' + 10,
                        _ => break,
                    };
                    code = code * 16 + digit as u32;
                }
                code >> 8 // Use top 16 bits for class
            } else {
                0
            }
        } else {
            0
        };

        let class_name = match class_code >> 8 {
            0x00 => b"Unclassified device" as &[u8],
            0x01 => b"Mass storage controller",
            0x02 => b"Network controller",
            0x03 => b"Display controller",
            0x04 => b"Multimedia controller",
            0x05 => b"Memory controller",
            0x06 => b"Bridge",
            0x07 => b"Communication controller",
            0x08 => b"System peripheral",
            0x09 => b"Input device controller",
            0x0a => b"Docking station",
            0x0b => b"Processor",
            0x0c => b"Serial bus controller",
            0x0d => b"Wireless controller",
            0x0e => b"Intelligent controller",
            0x0f => b"Satellite controller",
            0x10 => b"Encryption controller",
            0x11 => b"Signal processing controller",
            _ => b"Unknown device",
        };

        io::write_all(1, display_name);
        io::write_str(1, b" ");
        io::write_all(1, class_name);
        io::write_str(1, b"\n");
    }

    io::closedir(dir);
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_lspci_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["lspci"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should produce some output
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty());
    }
}
