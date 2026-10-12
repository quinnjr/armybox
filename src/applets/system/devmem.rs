//! devmem - access physical memory
//!
//! Read/write from physical address space via /dev/mem.

use super::get_arg;
use crate::io;
use crate::sys;

/// devmem - access physical memory
///
/// # Synopsis
/// ```text
/// devmem ADDRESS [WIDTH [VALUE]]
/// ```
///
/// # Description
/// Read/write from physical memory. Requires root privileges.
///
/// # Arguments
/// - ADDRESS: Physical address to access (hex)
/// - WIDTH: Access width (8, 16, 32, 64) default 32
/// - VALUE: Value to write (if specified)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn devmem(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"devmem: usage: devmem ADDRESS [WIDTH [VALUE]]\n");
        return 1;
    }

    let addr_str = unsafe { get_arg(argv, 1).unwrap() };
    let addr = parse_hex(addr_str).unwrap_or(0) as usize;

    let width: usize = if argc > 2 {
        if let Some(w) = unsafe { get_arg(argv, 2) } {
            sys::parse_u64(w).unwrap_or(32) as usize
        } else {
            32
        }
    } else {
        32
    };

    let write_value: Option<u64> = if argc > 3 {
        if let Some(v) = unsafe { get_arg(argv, 3) } {
            Some(parse_hex(v).unwrap_or(0))
        } else {
            None
        }
    } else {
        None
    };

    // Validate width
    if width != 8 && width != 16 && width != 32 && width != 64 {
        io::write_str(2, b"devmem: invalid width (use 8, 16, 32, or 64)\n");
        return 1;
    }

    // Open /dev/mem
    let fd = io::open(b"/dev/mem", libc::O_RDWR | libc::O_SYNC, 0);
    if fd < 0 {
        sys::perror(b"/dev/mem");
        return 1;
    }

    // Calculate page-aligned address
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
    let page_mask = page_size - 1;
    let map_base = addr & !page_mask;
    let map_offset = addr & page_mask;

    // Map the memory. Bionic's 32-bit off_t would turn physical addresses at
    // or above 2 GiB negative, so Android goes through mmap64.
    #[cfg(target_os = "android")]
    let map = unsafe {
        libc::mmap64(
            core::ptr::null_mut(),
            page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            map_base as libc::off64_t,
        )
    };
    #[cfg(not(target_os = "android"))]
    let map = unsafe {
        libc::mmap(
            core::ptr::null_mut(),
            page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd,
            map_base as i64,
        )
    };

    if map == libc::MAP_FAILED {
        sys::perror(b"mmap");
        io::close(fd);
        return 1;
    }

    let virt_addr = (map as usize + map_offset) as *mut u8;

    if let Some(value) = write_value {
        // Write operation
        unsafe {
            match width {
                8 => *(virt_addr as *mut u8) = value as u8,
                16 => *(virt_addr as *mut u16) = value as u16,
                32 => *(virt_addr as *mut u32) = value as u32,
                64 => *(virt_addr as *mut u64) = value,
                _ => {}
            }
        }
    } else {
        // Read operation
        let value: u64 = unsafe {
            match width {
                8 => *(virt_addr as *const u8) as u64,
                16 => *(virt_addr as *const u16) as u64,
                32 => *(virt_addr as *const u32) as u64,
                64 => *(virt_addr as *const u64),
                _ => 0,
            }
        };

        io::write_str(1, b"0x");
        let mut buf = [0u8; 20];
        let hex = sys::format_hex(value, &mut buf);
        io::write_all(1, hex);
        io::write_str(1, b"\n");
    }

    unsafe { libc::munmap(map, page_size) };
    io::close(fd);
    0
}

fn parse_hex(s: &[u8]) -> Option<u64> {
    let s = if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        &s[2..]
    } else {
        s
    };

    let mut result: u64 = 0;
    for &c in s {
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        result = result.checked_mul(16)?.checked_add(digit as u64)?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_devmem_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["devmem"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_devmem_invalid_width() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["devmem", "0x1000", "12"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("invalid width"));
    }

    #[test]
    fn test_devmem_permission_denied() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // This should fail without root
        let output = Command::new(&armybox)
            .args(["devmem", "0x1000"])
            .output()
            .unwrap();

        // Will fail with permission denied (unless running as root)
        assert_eq!(output.status.code(), Some(1));
    }
}
