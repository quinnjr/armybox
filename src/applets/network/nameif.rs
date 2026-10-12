//! nameif - name network interfaces based on MAC addresses
//!
//! Assign names to network interfaces.

extern crate alloc;
use super::get_arg;
use crate::io;
use alloc::vec::Vec;

/// nameif - name network interfaces based on MAC addresses
///
/// # Synopsis
/// ```text
/// nameif [-s] [-c FILE] [IFACE MAC]
/// ```
///
/// # Description
/// Rename network interfaces based on MAC addresses.
/// If no arguments given, reads from /etc/mactab.
///
/// # Options
/// - `-s`: Use syslog for logging
/// - `-c FILE`: Use FILE instead of /etc/mactab
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn nameif(argc: i32, argv: *const *const u8) -> i32 {
    let mut config_file: &[u8] = b"/etc/mactab";
    let mut use_syslog = false;
    let mut iface: Option<&[u8]> = None;
    let mut mac: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-s" {
            use_syslog = true;
        } else if arg == b"-c" {
            i += 1;
            if let Some(f) = unsafe { get_arg(argv, i as i32) } {
                config_file = f;
            }
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            if iface.is_none() {
                iface = Some(arg);
            } else if mac.is_none() {
                mac = Some(arg);
            }
        }
        i += 1;
    }

    // If both iface and mac provided, rename that interface
    if let (Some(iface_name), Some(mac_addr)) = (iface, mac) {
        return rename_interface_by_mac(iface_name, mac_addr, use_syslog);
    }

    // Otherwise, read from config file
    let fd = io::open(config_file, libc::O_RDONLY, 0);
    if fd < 0 {
        // No config file - nothing to do
        return 0;
    }

    let content = io::read_all(fd);
    io::close(fd);

    let mut exit_code = 0;

    for line in content.split(|&c| c == b'\n') {
        let line = trim(line);
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        let parts: Vec<&[u8]> = line
            .split(|&c| c == b' ' || c == b'\t')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() >= 2 {
            if rename_interface_by_mac(parts[0], parts[1], use_syslog) != 0 {
                exit_code = 1;
            }
        }
    }

    exit_code
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn rename_interface_by_mac(new_name: &[u8], target_mac: &[u8], _use_syslog: bool) -> i32 {
    // Parse target MAC address
    let target = match parse_mac(target_mac) {
        Some(m) => m,
        None => {
            io::write_str(2, b"nameif: invalid MAC address: ");
            io::write_all(2, target_mac);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Find interface with matching MAC
    let old_name = match find_interface_by_mac(&target) {
        Some(name) => name,
        None => {
            // Interface with this MAC not found - not an error
            return 0;
        }
    };

    // Check if already has correct name
    if old_name == new_name {
        return 0;
    }

    // Rename interface
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        io::write_str(2, b"nameif: socket failed\n");
        return 1;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    copy_iface_name(&mut ifr.ifr_name, &old_name);

    // Put new name in ifr_ifru.ifru_newname
    // This is a union, we need to access it correctly
    let new_name_ptr = unsafe { &mut ifr.ifr_ifru.ifru_newname as *mut [libc::c_char; 16] };
    copy_iface_name(unsafe { &mut *new_name_ptr }, new_name);

    // SIOCSIFNAME = 0x8923
    const SIOCSIFNAME: crate::io::IoctlReq = 0x8923u32 as crate::io::IoctlReq;

    let result = if unsafe { libc::ioctl(fd, SIOCSIFNAME, &ifr) } < 0 {
        io::write_str(2, b"nameif: cannot rename ");
        io::write_all(2, &old_name);
        io::write_str(2, b" to ");
        io::write_all(2, new_name);
        io::write_str(2, b"\n");
        1
    } else {
        0
    };

    io::close(fd);
    result
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn find_interface_by_mac(target: &[u8; 6]) -> Option<Vec<u8>> {
    // Read /sys/class/net to find all interfaces
    let dir_fd = io::open(b"/sys/class/net", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if dir_fd < 0 {
        return None;
    }

    // Read directory entries
    let mut buf = [0u8; 4096];
    let mut found_name: Option<Vec<u8>> = None;

    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, dir_fd, buf.as_mut_ptr(), buf.len()) };

        if n <= 0 {
            break;
        }

        let mut offset = 0usize;
        while offset < n as usize {
            let entry = &buf[offset..];
            // d_reclen is at offset 16 (2 bytes)
            let reclen = u16::from_ne_bytes([entry[16], entry[17]]) as usize;
            // d_type is at offset 18
            // d_name starts at offset 19
            let name_start = 19;
            let name_end = entry[name_start..]
                .iter()
                .position(|&c| c == 0)
                .map(|p| name_start + p)
                .unwrap_or(reclen);
            let name = &entry[name_start..name_end];

            if name != b"." && name != b".." {
                // Read MAC address from /sys/class/net/<iface>/address
                let mut path = Vec::new();
                path.extend_from_slice(b"/sys/class/net/");
                path.extend_from_slice(name);
                path.extend_from_slice(b"/address");
                path.push(0);

                let addr_fd = io::open(&path, libc::O_RDONLY, 0);
                if addr_fd >= 0 {
                    let mut mac_buf = [0u8; 32];
                    let n = io::read(addr_fd, &mut mac_buf);
                    io::close(addr_fd);

                    if n > 0 {
                        let mac_str = &mac_buf[..n as usize];
                        if let Some(mac) = parse_mac(mac_str) {
                            if mac == *target {
                                found_name = Some(name.to_vec());
                                break;
                            }
                        }
                    }
                }
            }

            offset += reclen;
        }

        if found_name.is_some() {
            break;
        }
    }

    io::close(dir_fd);
    found_name
}

fn parse_mac(s: &[u8]) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut idx = 0;
    let mut current = 0u8;
    let mut digit_count = 0;

    for &c in s {
        if c == b':' || c == b'-' {
            if digit_count == 0 {
                return None;
            }
            if idx >= 5 {
                return None;
            }
            mac[idx] = current;
            idx += 1;
            current = 0;
            digit_count = 0;
        } else if c >= b'0' && c <= b'9' {
            if digit_count >= 2 {
                return None;
            }
            current = current * 16 + (c - b'0');
            digit_count += 1;
        } else if c >= b'a' && c <= b'f' {
            if digit_count >= 2 {
                return None;
            }
            current = current * 16 + (c - b'a' + 10);
            digit_count += 1;
        } else if c >= b'A' && c <= b'F' {
            if digit_count >= 2 {
                return None;
            }
            current = current * 16 + (c - b'A' + 10);
            digit_count += 1;
        } else if c == b'\n' || c == b'\r' || c == b' ' || c == b'\t' {
            // Ignore trailing whitespace
            break;
        } else {
            return None;
        }
    }

    if digit_count == 0 || idx != 5 {
        return None;
    }
    mac[5] = current;

    Some(mac)
}

fn copy_iface_name(dest: &mut [libc::c_char; 16], src: &[u8]) {
    let len = src.len().min(15);
    for i in 0..len {
        dest[i] = src[i] as libc::c_char;
    }
    dest[len] = 0;
}

fn trim(s: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = s.len();
    while start < end && (s[start] == b' ' || s[start] == b'\t') {
        start += 1;
    }
    while end > start && (s[end - 1] == b' ' || s[end - 1] == b'\t' || s[end - 1] == b'\r') {
        end -= 1;
    }
    &s[start..end]
}

fn print_usage() {
    io::write_str(1, b"Usage: nameif [-s] [-c FILE] [IFACE MAC]\n\n");
    io::write_str(1, b"Rename network interfaces based on MAC addresses.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -s        Use syslog for logging\n");
    io::write_str(1, b"  -c FILE   Use FILE instead of /etc/mactab\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn nameif(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"nameif: only available on Linux\n");
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
    fn test_nameif_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nameif", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_nameif_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nameif"]).output().unwrap();

        // Should succeed (no /etc/mactab is not an error)
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_parse_mac() {
        use super::parse_mac;

        assert_eq!(
            parse_mac(b"00:11:22:33:44:55"),
            Some([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])
        );
        assert_eq!(
            parse_mac(b"aa:bb:cc:dd:ee:ff"),
            Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
        );
        assert_eq!(
            parse_mac(b"AA:BB:CC:DD:EE:FF"),
            Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
        );
        assert_eq!(
            parse_mac(b"00-11-22-33-44-55"),
            Some([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])
        );
        assert_eq!(parse_mac(b"invalid"), None);
        assert_eq!(parse_mac(b"00:11:22:33:44"), None);
    }
}
