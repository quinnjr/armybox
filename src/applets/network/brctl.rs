//! brctl - ethernet bridge administration
//!
//! Manage ethernet bridges.

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

// Bridge ioctl commands
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCBRADDBR: crate::io::IoctlReq = 0x89a0u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCBRDELBR: crate::io::IoctlReq = 0x89a1u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCBRADDIF: crate::io::IoctlReq = 0x89a2u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCBRDELIF: crate::io::IoctlReq = 0x89a3u32 as crate::io::IoctlReq;

// Network interface ioctl
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCGIFINDEX: crate::io::IoctlReq = 0x8933u32 as crate::io::IoctlReq;

/// brctl - ethernet bridge administration
///
/// # Synopsis
/// ```text
/// brctl COMMAND [ARGS]
/// ```
///
/// # Description
/// Administer ethernet bridges.
///
/// # Commands
/// - `addbr BRIDGE`: Add a bridge
/// - `delbr BRIDGE`: Delete a bridge
/// - `addif BRIDGE IFACE`: Add interface to bridge
/// - `delif BRIDGE IFACE`: Remove interface from bridge
/// - `show [BRIDGE]`: Show bridge info
/// - `stp BRIDGE on|off`: Enable/disable STP
/// - `setfd BRIDGE TIME`: Set forward delay (seconds)
/// - `sethello BRIDGE TIME`: Set hello time (seconds)
/// - `setmaxage BRIDGE TIME`: Set max message age (seconds)
/// - `setageing BRIDGE TIME`: Set ageing time (seconds)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn brctl(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        print_usage();
        return 1;
    }

    let cmd = match unsafe { get_arg(argv, 1) } {
        Some(c) => c,
        None => {
            print_usage();
            return 1;
        }
    };

    if cmd == b"addbr" {
        if argc < 3 {
            io::write_str(2, b"Usage: brctl addbr BRIDGE\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        addbr(bridge)
    } else if cmd == b"delbr" {
        if argc < 3 {
            io::write_str(2, b"Usage: brctl delbr BRIDGE\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        delbr(bridge)
    } else if cmd == b"addif" {
        if argc < 4 {
            io::write_str(2, b"Usage: brctl addif BRIDGE IFACE\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        let iface = match unsafe { get_arg(argv, 3) } {
            Some(i) => i,
            None => return 1,
        };
        addif(bridge, iface)
    } else if cmd == b"delif" {
        if argc < 4 {
            io::write_str(2, b"Usage: brctl delif BRIDGE IFACE\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        let iface = match unsafe { get_arg(argv, 3) } {
            Some(i) => i,
            None => return 1,
        };
        delif(bridge, iface)
    } else if cmd == b"show" {
        let bridge = if argc >= 3 {
            unsafe { get_arg(argv, 2) }
        } else {
            None
        };
        show(bridge)
    } else if cmd == b"stp" {
        if argc < 4 {
            io::write_str(2, b"Usage: brctl stp BRIDGE on|off\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        let state = match unsafe { get_arg(argv, 3) } {
            Some(s) => s,
            None => return 1,
        };
        stp(bridge, state)
    } else if cmd == b"setfd" || cmd == b"sethello" || cmd == b"setmaxage" || cmd == b"setageing" {
        if argc < 4 {
            io::write_str(2, b"Usage: brctl ");
            io::write_all(2, cmd);
            io::write_str(2, b" BRIDGE TIME\n");
            return 1;
        }
        let bridge = match unsafe { get_arg(argv, 2) } {
            Some(b) => b,
            None => return 1,
        };
        let time = match unsafe { get_arg(argv, 3) }.and_then(|v| sys::parse_u64(v)) {
            Some(t) => t,
            None => {
                io::write_str(2, b"brctl: invalid time value\n");
                return 1;
            }
        };
        set_bridge_param(bridge, cmd, time)
    } else {
        io::write_str(2, b"brctl: unknown command: ");
        io::write_all(2, cmd);
        io::write_str(2, b"\n");
        print_usage();
        return 1;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn brctl(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"brctl: only available on Linux\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn addbr(bridge: &[u8]) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"brctl: cannot create socket\n");
        return 1;
    }

    // Create null-terminated bridge name
    let mut name = [0u8; 32];
    let len = core::cmp::min(bridge.len(), 31);
    name[..len].copy_from_slice(&bridge[..len]);

    let ret = unsafe { libc::ioctl(sock, SIOCBRADDBR as crate::io::IoctlReq, name.as_ptr()) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"brctl: cannot add bridge ");
        io::write_all(2, bridge);
        io::write_str(2, b": ");
        print_errno();
        return 1;
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn delbr(bridge: &[u8]) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"brctl: cannot create socket\n");
        return 1;
    }

    let mut name = [0u8; 32];
    let len = core::cmp::min(bridge.len(), 31);
    name[..len].copy_from_slice(&bridge[..len]);

    let ret = unsafe { libc::ioctl(sock, SIOCBRDELBR as crate::io::IoctlReq, name.as_ptr()) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"brctl: cannot delete bridge ");
        io::write_all(2, bridge);
        io::write_str(2, b": ");
        print_errno();
        return 1;
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn addif(bridge: &[u8], iface: &[u8]) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"brctl: cannot create socket\n");
        return 1;
    }

    // Get interface index
    let if_index = match get_if_index(sock, iface) {
        Some(idx) => idx,
        None => {
            io::write_str(2, b"brctl: interface ");
            io::write_all(2, iface);
            io::write_str(2, b" not found\n");
            unsafe { libc::close(sock) };
            return 1;
        }
    };

    // Prepare ifreq for bridge
    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    let len = core::cmp::min(bridge.len(), libc::IFNAMSIZ - 1);
    for j in 0..len {
        ifr.ifr_name[j] = bridge[j] as libc::c_char;
    }
    ifr.ifr_ifru.ifru_ifindex = if_index;

    let ret = unsafe { libc::ioctl(sock, SIOCBRADDIF as crate::io::IoctlReq, &ifr) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"brctl: cannot add ");
        io::write_all(2, iface);
        io::write_str(2, b" to ");
        io::write_all(2, bridge);
        io::write_str(2, b": ");
        print_errno();
        return 1;
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn delif(bridge: &[u8], iface: &[u8]) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"brctl: cannot create socket\n");
        return 1;
    }

    let if_index = match get_if_index(sock, iface) {
        Some(idx) => idx,
        None => {
            io::write_str(2, b"brctl: interface ");
            io::write_all(2, iface);
            io::write_str(2, b" not found\n");
            unsafe { libc::close(sock) };
            return 1;
        }
    };

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    let len = core::cmp::min(bridge.len(), libc::IFNAMSIZ - 1);
    for j in 0..len {
        ifr.ifr_name[j] = bridge[j] as libc::c_char;
    }
    ifr.ifr_ifru.ifru_ifindex = if_index;

    let ret = unsafe { libc::ioctl(sock, SIOCBRDELIF as crate::io::IoctlReq, &ifr) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"brctl: cannot remove ");
        io::write_all(2, iface);
        io::write_str(2, b" from ");
        io::write_all(2, bridge);
        io::write_str(2, b": ");
        print_errno();
        return 1;
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show(bridge: Option<&[u8]>) -> i32 {
    io::write_str(1, b"bridge name\tbridge id\t\tSTP enabled\tinterfaces\n");

    // Read /sys/class/net/*/bridge to find bridges
    let fd = io::open(b"/sys/class/net", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        io::write_str(2, b"brctl: cannot read /sys/class/net\n");
        return 1;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };

        if n <= 0 {
            break;
        }

        let mut offset = 0;
        while offset < n as usize {
            let entry = unsafe {
                // buf is a byte array (alignment 1), so copy the header out
                core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const LinuxDirent64)
            };
            let name_len = entry.d_reclen as usize - 19; // header size
            let name = &buf[offset + 19..offset + 19 + name_len];

            // Skip . and ..
            if name.first() != Some(&b'.') {
                // Find null terminator
                let name_end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
                let name = &name[..name_end];

                // Check if it's a bridge
                if is_bridge(name) {
                    if bridge.is_none() || bridge == Some(name) {
                        show_bridge_info(name);
                    }
                }
            }

            offset += entry.d_reclen as usize;
        }
    }

    io::close(fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct LinuxDirent64 {
    d_ino: u64,
    d_off: i64,
    d_reclen: u16,
    d_type: u8,
    // d_name follows
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_bridge(name: &[u8]) -> bool {
    let mut path = Vec::new();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(name);
    path.extend_from_slice(b"/bridge");
    path.push(0);

    let mut stat: libc::stat = unsafe { core::mem::zeroed() };
    unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat) == 0 }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show_bridge_info(name: &[u8]) {
    io::write_all(1, name);
    io::write_str(1, b"\t\t");

    // Read bridge ID
    let mut path = Vec::new();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(name);
    path.extend_from_slice(b"/bridge/bridge_id");
    path.push(0);

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd >= 0 {
        let mut buf = [0u8; 32];
        let n = io::read(fd, &mut buf);
        if n > 0 {
            // Trim newline
            let end = buf[..n as usize]
                .iter()
                .position(|&c| c == b'\n')
                .unwrap_or(n as usize);
            io::write_all(1, &buf[..end]);
        }
        io::close(fd);
    } else {
        io::write_str(1, b"unknown");
    }
    io::write_str(1, b"\t");

    // Read STP state
    path.clear();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(name);
    path.extend_from_slice(b"/bridge/stp_state");
    path.push(0);

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd >= 0 {
        let mut buf = [0u8; 8];
        let n = io::read(fd, &mut buf);
        io::close(fd);
        if n > 0 && buf[0] != b'0' {
            io::write_str(1, b"yes");
        } else {
            io::write_str(1, b"no");
        }
    } else {
        io::write_str(1, b"no");
    }
    io::write_str(1, b"\t\t");

    // List interfaces
    path.clear();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(name);
    path.extend_from_slice(b"/brif");
    path.push(0);

    let dir_fd = io::open(&path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if dir_fd >= 0 {
        let mut buf = [0u8; 1024];
        let n = unsafe { libc::syscall(libc::SYS_getdents64, dir_fd, buf.as_mut_ptr(), buf.len()) };

        if n > 0 {
            let mut offset = 0;
            let mut first = true;
            while offset < n as usize {
                let entry = unsafe {
                    // buf is a byte array (alignment 1), so copy the header out
                    core::ptr::read_unaligned(buf.as_ptr().add(offset) as *const LinuxDirent64)
                };
                let if_name = &buf[offset + 19..];
                let if_name_end = if_name.iter().position(|&c| c == 0).unwrap_or(0);
                let if_name = &if_name[..if_name_end];

                if !if_name.is_empty() && if_name[0] != b'.' {
                    if !first {
                        io::write_str(1, b"\n\t\t\t\t\t\t\t");
                    }
                    io::write_all(1, if_name);
                    first = false;
                }

                offset += entry.d_reclen as usize;
            }
        }
        io::close(dir_fd);
    }

    io::write_str(1, b"\n");
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stp(bridge: &[u8], state: &[u8]) -> i32 {
    let value = if state == b"on" || state == b"yes" || state == b"1" {
        b"1"
    } else if state == b"off" || state == b"no" || state == b"0" {
        b"0"
    } else {
        io::write_str(2, b"brctl: invalid STP state (use on/off)\n");
        return 1;
    };

    let mut path = Vec::new();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(bridge);
    path.extend_from_slice(b"/bridge/stp_state");
    path.push(0);

    let fd = io::open(&path, libc::O_WRONLY, 0);
    if fd < 0 {
        io::write_str(2, b"brctl: cannot set STP for ");
        io::write_all(2, bridge);
        io::write_str(2, b"\n");
        return 1;
    }

    io::write_all(fd, value);
    io::close(fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_bridge_param(bridge: &[u8], param: &[u8], value: u64) -> i32 {
    let sysfs_name = if param == b"setfd" {
        b"forward_delay" as &[u8]
    } else if param == b"sethello" {
        b"hello_time" as &[u8]
    } else if param == b"setmaxage" {
        b"max_age" as &[u8]
    } else if param == b"setageing" {
        b"ageing_time" as &[u8]
    } else {
        io::write_str(2, b"brctl: unknown parameter\n");
        return 1;
    };

    let mut path = Vec::new();
    path.extend_from_slice(b"/sys/class/net/");
    path.extend_from_slice(bridge);
    path.extend_from_slice(b"/bridge/");
    path.extend_from_slice(sysfs_name);
    path.push(0);

    let fd = io::open(&path, libc::O_WRONLY, 0);
    if fd < 0 {
        io::write_str(2, b"brctl: cannot set ");
        io::write_all(2, sysfs_name);
        io::write_str(2, b" for ");
        io::write_all(2, bridge);
        io::write_str(2, b"\n");
        return 1;
    }

    // Convert seconds to jiffies (HZ = 100 typically, but sysfs expects centiseconds)
    let centisecs = value * 100;
    let mut buf = [0u8; 16];
    let s = sys::format_u64(centisecs, &mut buf);
    io::write_all(fd, s);
    io::close(fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_if_index(sock: i32, iface: &[u8]) -> Option<libc::c_int> {
    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    let len = core::cmp::min(iface.len(), libc::IFNAMSIZ - 1);
    for j in 0..len {
        ifr.ifr_name[j] = iface[j] as libc::c_char;
    }

    let ret = unsafe { libc::ioctl(sock, SIOCGIFINDEX as crate::io::IoctlReq, &mut ifr) };
    if ret < 0 {
        return None;
    }

    Some(unsafe { ifr.ifr_ifru.ifru_ifindex })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_errno() {
    let errno = crate::sys::errno();
    let msg = match errno {
        libc::EEXIST => b"already exists" as &[u8],
        libc::ENOENT => b"not found" as &[u8],
        libc::EBUSY => b"device is busy" as &[u8],
        libc::EPERM => b"permission denied" as &[u8],
        libc::ENODEV => b"no such device" as &[u8],
        _ => b"operation failed" as &[u8],
    };
    io::write_all(2, msg);
    io::write_str(2, b"\n");
}

fn print_usage() {
    io::write_str(1, b"Usage: brctl COMMAND [ARGS]\n\n");
    io::write_str(1, b"Commands:\n");
    io::write_str(1, b"  addbr BRIDGE          Add bridge\n");
    io::write_str(1, b"  delbr BRIDGE          Delete bridge\n");
    io::write_str(1, b"  addif BRIDGE IFACE    Add interface to bridge\n");
    io::write_str(1, b"  delif BRIDGE IFACE    Remove interface from bridge\n");
    io::write_str(1, b"  show [BRIDGE]         Show bridge info\n");
    io::write_str(1, b"  stp BRIDGE on|off     Enable/disable STP\n");
    io::write_str(1, b"  setfd BRIDGE TIME     Set forward delay (seconds)\n");
    io::write_str(1, b"  sethello BRIDGE TIME  Set hello time (seconds)\n");
    io::write_str(
        1,
        b"  setmaxage BRIDGE TIME Set max message age (seconds)\n",
    );
    io::write_str(1, b"  setageing BRIDGE TIME Set ageing time (seconds)\n");
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
    fn test_brctl_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["brctl"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_brctl_show() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["brctl", "show"])
            .output()
            .unwrap();

        // Should succeed and show header even if no bridges
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("bridge name"));
    }
}
