//! vconfig - VLAN configuration utility
//!
//! Configure 802.1Q VLANs.

use super::get_arg;
use crate::io;
use crate::sys;

// VLAN ioctl commands
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCGIFVLAN: crate::io::IoctlReq = 0x8982u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SIOCSIFVLAN: crate::io::IoctlReq = 0x8983u32 as crate::io::IoctlReq;

// VLAN command codes
#[cfg(any(target_os = "linux", target_os = "android"))]
const ADD_VLAN_CMD: libc::c_int = 0;
#[cfg(any(target_os = "linux", target_os = "android"))]
const DEL_VLAN_CMD: libc::c_int = 1;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SET_VLAN_INGRESS_PRIORITY_CMD: libc::c_int = 2;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SET_VLAN_EGRESS_PRIORITY_CMD: libc::c_int = 3;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SET_VLAN_FLAG_CMD: libc::c_int = 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const SET_VLAN_NAME_TYPE_CMD: libc::c_int = 5;

// VLAN name types
#[cfg(any(target_os = "linux", target_os = "android"))]
const VLAN_NAME_TYPE_PLUS_VID: libc::c_int = 0; // vlan0005
#[cfg(any(target_os = "linux", target_os = "android"))]
const VLAN_NAME_TYPE_RAW_PLUS_VID: libc::c_int = 1; // eth0.0005
#[cfg(any(target_os = "linux", target_os = "android"))]
const VLAN_NAME_TYPE_PLUS_VID_NO_PAD: libc::c_int = 2; // vlan5
#[cfg(any(target_os = "linux", target_os = "android"))]
const VLAN_NAME_TYPE_RAW_PLUS_VID_NO_PAD: libc::c_int = 3; // eth0.5

/// vconfig - VLAN configuration utility
///
/// # Synopsis
/// ```text
/// vconfig add IFACE VID
/// vconfig rem IFACE.VID
/// vconfig set_flag IFACE.VID FLAG VALUE
/// vconfig set_egress_map IFACE.VID SKB_PRIO VLAN_QOS
/// vconfig set_ingress_map IFACE.VID SKB_PRIO VLAN_QOS
/// vconfig set_name_type NAME_TYPE
/// ```
///
/// # Description
/// Create and configure 802.1Q VLAN interfaces.
///
/// # Commands
/// - `add IFACE VID`: Add a VLAN device (e.g., vconfig add eth0 10)
/// - `rem IFACE.VID`: Remove a VLAN device (e.g., vconfig rem eth0.10)
/// - `set_flag IFACE.VID FLAG VALUE`: Set a VLAN flag (0=reorder_hdr, 1=gvrp)
/// - `set_egress_map IFACE.VID SKB_PRIO VLAN_QOS`: Set egress priority map
/// - `set_ingress_map IFACE.VID SKB_PRIO VLAN_QOS`: Set ingress priority map
/// - `set_name_type NAME_TYPE`: Set naming type (VLAN_PLUS_VID, DEV_PLUS_VID, etc.)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn vconfig(argc: i32, argv: *const *const u8) -> i32 {
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

    if cmd == b"add" {
        if argc < 4 {
            io::write_str(2, b"Usage: vconfig add IFACE VID\n");
            return 1;
        }
        let iface = match unsafe { get_arg(argv, 2) } {
            Some(i) => i,
            None => return 1,
        };
        let vid = match unsafe { get_arg(argv, 3) }.and_then(|v| sys::parse_u64(v)) {
            Some(v) if v <= 4094 => v as u16,
            _ => {
                io::write_str(2, b"vconfig: invalid VLAN ID (must be 0-4094)\n");
                return 1;
            }
        };
        add_vlan(iface, vid)
    } else if cmd == b"rem" {
        if argc < 3 {
            io::write_str(2, b"Usage: vconfig rem IFACE.VID\n");
            return 1;
        }
        let vlan_iface = match unsafe { get_arg(argv, 2) } {
            Some(i) => i,
            None => return 1,
        };
        rem_vlan(vlan_iface)
    } else if cmd == b"set_flag" {
        if argc < 5 {
            io::write_str(2, b"Usage: vconfig set_flag IFACE.VID FLAG VALUE\n");
            return 1;
        }
        let vlan_iface = match unsafe { get_arg(argv, 2) } {
            Some(i) => i,
            None => return 1,
        };
        let flag = match unsafe { get_arg(argv, 3) }.and_then(|v| sys::parse_u64(v)) {
            Some(f) => f as libc::c_int,
            None => {
                io::write_str(2, b"vconfig: invalid flag\n");
                return 1;
            }
        };
        let value = match unsafe { get_arg(argv, 4) }.and_then(|v| sys::parse_u64(v)) {
            Some(v) => v as libc::c_int,
            None => {
                io::write_str(2, b"vconfig: invalid value\n");
                return 1;
            }
        };
        set_vlan_flag(vlan_iface, flag, value)
    } else if cmd == b"set_egress_map" {
        if argc < 5 {
            io::write_str(
                2,
                b"Usage: vconfig set_egress_map IFACE.VID SKB_PRIO VLAN_QOS\n",
            );
            return 1;
        }
        let vlan_iface = match unsafe { get_arg(argv, 2) } {
            Some(i) => i,
            None => return 1,
        };
        let skb_prio = match unsafe { get_arg(argv, 3) }.and_then(|v| sys::parse_u64(v)) {
            Some(p) => p as u32,
            None => return 1,
        };
        let vlan_qos = match unsafe { get_arg(argv, 4) }.and_then(|v| sys::parse_u64(v)) {
            Some(q) if q <= 7 => q as u16,
            _ => {
                io::write_str(2, b"vconfig: vlan_qos must be 0-7\n");
                return 1;
            }
        };
        set_egress_map(vlan_iface, skb_prio, vlan_qos)
    } else if cmd == b"set_ingress_map" {
        if argc < 5 {
            io::write_str(
                2,
                b"Usage: vconfig set_ingress_map IFACE.VID SKB_PRIO VLAN_QOS\n",
            );
            return 1;
        }
        let vlan_iface = match unsafe { get_arg(argv, 2) } {
            Some(i) => i,
            None => return 1,
        };
        let skb_prio = match unsafe { get_arg(argv, 3) }.and_then(|v| sys::parse_u64(v)) {
            Some(p) => p as u32,
            None => return 1,
        };
        let vlan_qos = match unsafe { get_arg(argv, 4) }.and_then(|v| sys::parse_u64(v)) {
            Some(q) if q <= 7 => q as u16,
            _ => {
                io::write_str(2, b"vconfig: vlan_qos must be 0-7\n");
                return 1;
            }
        };
        set_ingress_map(vlan_iface, skb_prio, vlan_qos)
    } else if cmd == b"set_name_type" {
        if argc < 3 {
            io::write_str(2, b"Usage: vconfig set_name_type NAME_TYPE\n");
            io::write_str(2, b"  NAME_TYPE: VLAN_PLUS_VID, DEV_PLUS_VID, VLAN_PLUS_VID_NO_PAD, DEV_PLUS_VID_NO_PAD\n");
            return 1;
        }
        let name_type = match unsafe { get_arg(argv, 2) } {
            Some(n) => n,
            None => return 1,
        };
        set_name_type(name_type)
    } else {
        io::write_str(2, b"vconfig: unknown command: ");
        io::write_all(2, cmd);
        io::write_str(2, b"\n");
        print_usage();
        return 1;
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn vconfig(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"vconfig: only available on Linux\n");
    1
}

// VLAN ioctl arguments structure
#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct VlanIoctlArgs {
    cmd: libc::c_int,
    device1: [libc::c_char; 24],
    u: VlanIoctlUnion,
    vlan_qos: libc::c_short,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
union VlanIoctlUnion {
    device2: [libc::c_char; 24],
    vid: libc::c_short,
    skb_priority: libc::c_uint,
    name_type: libc::c_uint,
    bind_type: libc::c_uint,
    flag: libc::c_uint,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn add_vlan(iface: &[u8], vid: u16) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = ADD_VLAN_CMD;

    // Copy interface name
    let len = core::cmp::min(iface.len(), 23);
    for j in 0..len {
        args.device1[j] = iface[j] as libc::c_char;
    }

    args.u.vid = vid as libc::c_short;

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to add VLAN device\n");
        return 1;
    }

    io::write_str(1, b"Added VLAN with VID == ");
    let mut buf = [0u8; 16];
    io::write_all(1, sys::format_u64(vid as u64, &mut buf));
    io::write_str(1, b" to IF -:");
    io::write_all(1, iface);
    io::write_str(1, b":-\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn rem_vlan(vlan_iface: &[u8]) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = DEL_VLAN_CMD;

    // Copy interface name
    let len = core::cmp::min(vlan_iface.len(), 23);
    for j in 0..len {
        args.device1[j] = vlan_iface[j] as libc::c_char;
    }

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to remove VLAN device\n");
        return 1;
    }

    io::write_str(1, b"Removed VLAN -:");
    io::write_all(1, vlan_iface);
    io::write_str(1, b":-\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_vlan_flag(vlan_iface: &[u8], flag: libc::c_int, value: libc::c_int) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = SET_VLAN_FLAG_CMD;

    let len = core::cmp::min(vlan_iface.len(), 23);
    for j in 0..len {
        args.device1[j] = vlan_iface[j] as libc::c_char;
    }

    args.u.flag = flag as libc::c_uint;
    args.vlan_qos = value as libc::c_short;

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to set flag\n");
        return 1;
    }

    io::write_str(1, b"Set flag on device -:");
    io::write_all(1, vlan_iface);
    io::write_str(1, b":-\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_egress_map(vlan_iface: &[u8], skb_prio: u32, vlan_qos: u16) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = SET_VLAN_EGRESS_PRIORITY_CMD;

    let len = core::cmp::min(vlan_iface.len(), 23);
    for j in 0..len {
        args.device1[j] = vlan_iface[j] as libc::c_char;
    }

    args.u.skb_priority = skb_prio;
    args.vlan_qos = vlan_qos as libc::c_short;

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to set egress map\n");
        return 1;
    }

    io::write_str(1, b"Set egress mapping on device -:");
    io::write_all(1, vlan_iface);
    io::write_str(1, b":-\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_ingress_map(vlan_iface: &[u8], skb_prio: u32, vlan_qos: u16) -> i32 {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = SET_VLAN_INGRESS_PRIORITY_CMD;

    let len = core::cmp::min(vlan_iface.len(), 23);
    for j in 0..len {
        args.device1[j] = vlan_iface[j] as libc::c_char;
    }

    args.u.skb_priority = skb_prio;
    args.vlan_qos = vlan_qos as libc::c_short;

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to set ingress map\n");
        return 1;
    }

    io::write_str(1, b"Set ingress mapping on device -:");
    io::write_all(1, vlan_iface);
    io::write_str(1, b":-\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_name_type(name_type: &[u8]) -> i32 {
    let type_val = if name_type == b"VLAN_PLUS_VID" {
        VLAN_NAME_TYPE_PLUS_VID
    } else if name_type == b"DEV_PLUS_VID" || name_type == b"VLAN_PLUS_VID_RAW" {
        VLAN_NAME_TYPE_RAW_PLUS_VID
    } else if name_type == b"VLAN_PLUS_VID_NO_PAD" {
        VLAN_NAME_TYPE_PLUS_VID_NO_PAD
    } else if name_type == b"DEV_PLUS_VID_NO_PAD" {
        VLAN_NAME_TYPE_RAW_PLUS_VID_NO_PAD
    } else {
        io::write_str(2, b"vconfig: unknown name type\n");
        io::write_str(2, b"  Valid types: VLAN_PLUS_VID, DEV_PLUS_VID, VLAN_PLUS_VID_NO_PAD, DEV_PLUS_VID_NO_PAD\n");
        return 1;
    };

    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"vconfig: cannot create socket\n");
        return 1;
    }

    let mut args: VlanIoctlArgs = unsafe { core::mem::zeroed() };
    args.cmd = SET_VLAN_NAME_TYPE_CMD;
    args.u.name_type = type_val as libc::c_uint;

    let ret = unsafe { libc::ioctl(sock, SIOCSIFVLAN as crate::io::IoctlReq, &args) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        io::write_str(2, b"vconfig: failed to set name type\n");
        return 1;
    }

    io::write_str(
        1,
        b"Set name-type for VLAN subsystem. Should be visible in /proc/net/vlan/config\n",
    );

    0
}

fn print_usage() {
    io::write_str(1, b"Usage: vconfig COMMAND [OPTIONS]\n\n");
    io::write_str(1, b"Commands:\n");
    io::write_str(
        1,
        b"  add IFACE VID                         Add VLAN device\n",
    );
    io::write_str(
        1,
        b"  rem IFACE.VID                         Remove VLAN device\n",
    );
    io::write_str(
        1,
        b"  set_flag IFACE.VID FLAG VALUE         Set VLAN flag\n",
    );
    io::write_str(
        1,
        b"  set_egress_map IFACE.VID SKB VLAN     Set egress priority\n",
    );
    io::write_str(
        1,
        b"  set_ingress_map IFACE.VID SKB VLAN    Set ingress priority\n",
    );
    io::write_str(
        1,
        b"  set_name_type NAME_TYPE               Set naming convention\n",
    );
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
    fn test_vconfig_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["vconfig"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
