//! tunctl - create and manage persistent TUN/TAP interfaces
//!
//! TUN/TAP device management.

use super::get_arg;
use crate::io;
use crate::sys;

// TUN/TAP ioctl constants
#[cfg(any(target_os = "linux", target_os = "android"))]
const TUNSETIFF: crate::io::IoctlReq = 0x400454cau32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const TUNSETPERSIST: crate::io::IoctlReq = 0x400454cbu32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const TUNSETOWNER: crate::io::IoctlReq = 0x400454ccu32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const TUNSETGROUP: crate::io::IoctlReq = 0x400454ceu32 as crate::io::IoctlReq;

// TUN/TAP flags
#[cfg(any(target_os = "linux", target_os = "android"))]
const IFF_TUN: libc::c_short = 0x0001;
#[cfg(any(target_os = "linux", target_os = "android"))]
const IFF_TAP: libc::c_short = 0x0002;
#[cfg(any(target_os = "linux", target_os = "android"))]
const IFF_NO_PI: libc::c_short = 0x1000;

/// tunctl - create and manage persistent TUN/TAP interfaces
///
/// # Synopsis
/// ```text
/// tunctl [-t NAME] [-u UID] [-g GID] [-n] [-d NAME]
/// ```
///
/// # Description
/// Create and manage TUN/TAP network interfaces.
///
/// # Options
/// - `-t NAME`: Create TUN device with specified name (default: tunN)
/// - `-u UID`: Set owner UID
/// - `-g GID`: Set group GID
/// - `-n`: Create a point-to-point TUN device (no PI)
/// - `-d NAME`: Delete the device with specified name
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn tunctl(argc: i32, argv: *const *const u8) -> i32 {
    let mut name: Option<&[u8]> = None;
    let mut uid: Option<libc::uid_t> = None;
    let mut gid: Option<libc::gid_t> = None;
    let mut delete = false;
    let mut no_pi = false;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-t" {
            i += 1;
            if i < argc as usize {
                name = unsafe { get_arg(argv, i as i32) };
            }
        } else if arg == b"-d" {
            i += 1;
            if i < argc as usize {
                name = unsafe { get_arg(argv, i as i32) };
                delete = true;
            }
        } else if arg == b"-u" {
            i += 1;
            if i < argc as usize {
                if let Some(arg) = unsafe { get_arg(argv, i as i32) } {
                    uid = sys::parse_u64(arg).map(|v| v as libc::uid_t);
                }
            }
        } else if arg == b"-g" {
            i += 1;
            if i < argc as usize {
                if let Some(arg) = unsafe { get_arg(argv, i as i32) } {
                    gid = sys::parse_u64(arg).map(|v| v as libc::gid_t);
                }
            }
        } else if arg == b"-n" {
            no_pi = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        }
        i += 1;
    }

    // Open /dev/net/tun
    let fd = io::open(b"/dev/net/tun", libc::O_RDWR, 0);
    if fd < 0 {
        io::write_str(2, b"tunctl: cannot open /dev/net/tun\n");
        return 1;
    }

    // Prepare ifreq struct
    #[repr(C)]
    struct TunReq {
        ifr_name: [libc::c_char; libc::IFNAMSIZ],
        ifr_flags: libc::c_short,
        padding: [u8; 22], // padding to match ifreq size
    }

    let mut ifr = TunReq {
        ifr_name: [0; libc::IFNAMSIZ],
        ifr_flags: IFF_TUN,
        padding: [0; 22],
    };

    if no_pi {
        ifr.ifr_flags |= IFF_NO_PI;
    }

    // Set interface name if provided
    if let Some(n) = name {
        let len = core::cmp::min(n.len(), libc::IFNAMSIZ - 1);
        for j in 0..len {
            ifr.ifr_name[j] = n[j] as libc::c_char;
        }
    }

    // Create/delete the interface
    let ret = unsafe { libc::ioctl(fd, TUNSETIFF as crate::io::IoctlReq, &mut ifr) };
    if ret < 0 {
        io::write_str(2, b"tunctl: TUNSETIFF failed\n");
        io::close(fd);
        return 1;
    }

    if delete {
        // Delete: set not persistent
        let ret =
            unsafe { libc::ioctl(fd, TUNSETPERSIST as crate::io::IoctlReq, 0 as libc::c_int) };
        if ret < 0 {
            io::write_str(2, b"tunctl: cannot delete interface\n");
            io::close(fd);
            return 1;
        }

        io::write_str(1, b"Set '");
        write_ifname(&ifr.ifr_name);
        io::write_str(1, b"' nonpersistent\n");
    } else {
        // Create: set owner if specified
        if let Some(u) = uid {
            let ret =
                unsafe { libc::ioctl(fd, TUNSETOWNER as crate::io::IoctlReq, u as libc::c_int) };
            if ret < 0 {
                io::write_str(2, b"tunctl: cannot set owner\n");
                io::close(fd);
                return 1;
            }
        }

        // Set group if specified
        if let Some(g) = gid {
            let ret =
                unsafe { libc::ioctl(fd, TUNSETGROUP as crate::io::IoctlReq, g as libc::c_int) };
            if ret < 0 {
                io::write_str(2, b"tunctl: cannot set group\n");
                io::close(fd);
                return 1;
            }
        }

        // Make persistent
        let ret =
            unsafe { libc::ioctl(fd, TUNSETPERSIST as crate::io::IoctlReq, 1 as libc::c_int) };
        if ret < 0 {
            io::write_str(2, b"tunctl: cannot make persistent\n");
            io::close(fd);
            return 1;
        }

        io::write_str(1, b"Set '");
        write_ifname(&ifr.ifr_name);
        io::write_str(1, b"' persistent and owned by ");
        if let Some(u) = uid {
            io::write_str(1, b"uid ");
            let mut buf = [0u8; 16];
            io::write_all(1, sys::format_u64(u as u64, &mut buf));
        } else {
            io::write_str(1, b"uid -1");
        }
        io::write_str(1, b"\n");
    }

    io::close(fd);
    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn tunctl(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"tunctl: only available on Linux\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn write_ifname(name: &[libc::c_char; libc::IFNAMSIZ]) {
    for &c in name.iter() {
        if c == 0 {
            break;
        }
        io::write_all(1, &[c as u8]);
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_usage() {
    io::write_str(1, b"Usage: tunctl [OPTIONS]\n");
    io::write_str(1, b"Create and manage TUN/TAP network interfaces.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -t NAME   Create TUN device with specified name\n");
    io::write_str(1, b"  -u UID    Set owner UID\n");
    io::write_str(1, b"  -g GID    Set group GID\n");
    io::write_str(1, b"  -n        Create with no packet info (IFF_NO_PI)\n");
    io::write_str(1, b"  -d NAME   Delete the device with specified name\n");
    io::write_str(1, b"  -h        Show this help\n");
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_tunctl_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["tunctl", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
