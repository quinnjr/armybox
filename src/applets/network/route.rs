//! route - show/manipulate IP routing table
//!
//! Display or modify the IP routing table.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

use crate::sys::{RTF_GATEWAY, RTF_HOST, RTF_UP, rtentry};

/// route - show/manipulate IP routing table
///
/// # Synopsis
/// ```text
/// route [-n] [-e]
/// route add -net|-host DEST [gw GW] [netmask MASK] [dev IF]
/// route del -net|-host DEST
/// ```
///
/// # Description
/// Display or manipulate the kernel IP routing table.
///
/// # Options
/// - `-n`: Show numerical addresses instead of resolving hostnames
/// - `-e`: Display detailed information (extended format)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn route(argc: i32, argv: *const *const u8) -> i32 {
    let mut numeric = false;
    let mut extended = false;
    let mut command: Option<&[u8]> = None;
    let mut cmd_args: Vec<&[u8]> = Vec::new();

    // Parse arguments
    let mut i = 1i32;
    while i < argc {
        let Some(arg) = (unsafe { get_arg(argv, i) }) else {
            i += 1;
            continue;
        };

        if arg == b"-n" {
            numeric = true;
        } else if arg == b"-e" || arg == b"--extended" {
            extended = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_help();
            return 0;
        } else if arg == b"add" || arg == b"del" || arg == b"delete" {
            command = Some(arg);
            // Collect remaining args for the command
            i += 1;
            while i < argc {
                if let Some(a) = unsafe { get_arg(argv, i) } {
                    cmd_args.push(a);
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }

    // If no command, just display routing table
    if command.is_none() {
        return show_routes(numeric, extended);
    }

    // Handle add/del commands
    match command {
        Some(b"add") => add_route(&cmd_args),
        Some(b"del") | Some(b"delete") => del_route(&cmd_args),
        _ => {
            io::write_str(2, b"route: unknown command\n");
            1
        }
    }
}

fn print_help() {
    io::write_str(1, b"Usage: route [-n] [-e]\n");
    io::write_str(
        1,
        b"       route add -net|-host DEST [gw GW] [netmask MASK] [dev IF]\n",
    );
    io::write_str(1, b"       route del -net|-host DEST\n");
    io::write_str(1, b"\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -n    Show numerical addresses\n");
    io::write_str(1, b"  -e    Display extended information\n");
}

/// Display the routing table
fn show_routes(numeric: bool, extended: bool) -> i32 {
    io::write_str(1, b"Kernel IP routing table\n");

    if extended {
        io::write_str(
            1,
            b"Destination     Gateway         Genmask         Flags   MSS Window  irtt Iface\n",
        );
    } else {
        io::write_str(
            1,
            b"Destination     Gateway         Genmask         Flags Metric Ref    Use Iface\n",
        );
    }

    let fd = io::open(b"/proc/net/route", libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"route: cannot open /proc/net/route\n");
        return 1;
    }

    let mut buf = [0u8; 8192];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    let content = &buf[..n as usize];

    // Skip header line and process each route
    for (i, line) in content.split(|&c| c == b'\n').enumerate() {
        if i == 0 {
            continue;
        } // Skip header
        if line.is_empty() {
            continue;
        }

        if let Some(entry) = parse_route_line(line) {
            print_route_entry(&entry, numeric, extended);
        }
    }

    0
}

/// Parsed route entry
struct RouteEntry<'a> {
    iface: &'a [u8],
    dest: u32,
    gateway: u32,
    flags: u16,
    refcnt: u32,
    use_count: u32,
    metric: u32,
    mask: u32,
    mtu: u32,
    window: u32,
    irtt: u32,
}

/// Parse a line from /proc/net/route
fn parse_route_line(line: &[u8]) -> Option<RouteEntry<'_>> {
    // Format: Iface Destination Gateway Flags RefCnt Use Metric Mask MTU Window IRTT
    let fields: Vec<&[u8]> = line
        .split(|&c| c == b'\t' || c == b' ')
        .filter(|s| !s.is_empty())
        .collect();

    if fields.len() < 11 {
        return None;
    }

    Some(RouteEntry {
        iface: fields[0],
        dest: parse_hex_u32(fields[1])?,
        gateway: parse_hex_u32(fields[2])?,
        flags: parse_hex_u32(fields[3])? as u16,
        refcnt: parse_hex_u32(fields[4]).unwrap_or(0),
        use_count: parse_hex_u32(fields[5]).unwrap_or(0),
        metric: parse_hex_u32(fields[6]).unwrap_or(0),
        mask: parse_hex_u32(fields[7])?,
        mtu: parse_hex_u32(fields[8]).unwrap_or(0),
        window: parse_hex_u32(fields[9]).unwrap_or(0),
        irtt: parse_hex_u32(fields[10]).unwrap_or(0),
    })
}

/// Parse hex string to u32
fn parse_hex_u32(s: &[u8]) -> Option<u32> {
    let mut result: u32 = 0;
    for &c in s {
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        result = result.checked_mul(16)?.checked_add(digit as u32)?;
    }
    Some(result)
}

/// Print a route entry
fn print_route_entry(entry: &RouteEntry, _numeric: bool, extended: bool) {
    // Destination (padded to 16 chars)
    print_ip_padded(entry.dest, 16);

    // Gateway
    print_ip_padded(entry.gateway, 16);

    // Netmask
    print_ip_padded(entry.mask, 16);

    // Flags
    let mut flags_str = [b' '; 6];
    let mut pos = 0;

    // RTF_UP = 0x0001
    if entry.flags & 0x0001 != 0 {
        flags_str[pos] = b'U';
        pos += 1;
    }
    // RTF_GATEWAY = 0x0002
    if entry.flags & 0x0002 != 0 {
        flags_str[pos] = b'G';
        pos += 1;
    }
    // RTF_HOST = 0x0004
    if entry.flags & 0x0004 != 0 {
        flags_str[pos] = b'H';
        pos += 1;
    }
    // RTF_REINSTATE = 0x0008
    if entry.flags & 0x0008 != 0 {
        flags_str[pos] = b'R';
        pos += 1;
    }
    // RTF_DYNAMIC = 0x0010
    if entry.flags & 0x0010 != 0 {
        flags_str[pos] = b'D';
        pos += 1;
    }
    // RTF_MODIFIED = 0x0020
    if entry.flags & 0x0020 != 0 {
        flags_str[pos] = b'M';
        pos += 1;
    }

    io::write_all(1, &flags_str);

    if extended {
        // MSS (MTU-40 for TCP)
        let mss = if entry.mtu > 40 { entry.mtu - 40 } else { 0 };
        print_number_padded(mss as u64, 6);
        // Window
        print_number_padded(entry.window as u64, 7);
        // IRTT
        print_number_padded(entry.irtt as u64, 6);
    } else {
        // Metric
        print_number_padded(entry.metric as u64, 7);
        // Ref
        print_number_padded(entry.refcnt as u64, 4);
        // Use
        print_number_padded(entry.use_count as u64, 7);
    }

    io::write_str(1, b" ");
    io::write_all(1, entry.iface);
    io::write_str(1, b"\n");
}

/// Print IP address in dotted quad notation, padded to width
fn print_ip_padded(ip: u32, width: usize) {
    // IP is in host byte order (little-endian on x86), but /proc/net/route
    // stores it as little-endian hex, so bytes are already in network order
    let b0 = (ip & 0xFF) as u8;
    let b1 = ((ip >> 8) & 0xFF) as u8;
    let b2 = ((ip >> 16) & 0xFF) as u8;
    let b3 = ((ip >> 24) & 0xFF) as u8;

    let mut buf = [0u8; 16];
    let mut pos = 0;

    // Build IP string
    let mut num_buf = [0u8; 4];

    let s = sys::format_u64(b0 as u64, &mut num_buf);
    buf[pos..pos + s.len()].copy_from_slice(s);
    pos += s.len();
    buf[pos] = b'.';
    pos += 1;

    let s = sys::format_u64(b1 as u64, &mut num_buf);
    buf[pos..pos + s.len()].copy_from_slice(s);
    pos += s.len();
    buf[pos] = b'.';
    pos += 1;

    let s = sys::format_u64(b2 as u64, &mut num_buf);
    buf[pos..pos + s.len()].copy_from_slice(s);
    pos += s.len();
    buf[pos] = b'.';
    pos += 1;

    let s = sys::format_u64(b3 as u64, &mut num_buf);
    buf[pos..pos + s.len()].copy_from_slice(s);
    pos += s.len();

    // Special case: 0.0.0.0 -> "*" or "default"
    let ip_str = if ip == 0 {
        b"0.0.0.0" as &[u8]
    } else {
        &buf[..pos]
    };

    io::write_all(1, ip_str);

    // Pad with spaces
    for _ in ip_str.len()..width {
        io::write_str(1, b" ");
    }
}

/// Print number right-padded to width
fn print_number_padded(n: u64, width: usize) {
    let mut buf = [0u8; 16];
    let s = sys::format_u64(n, &mut buf);

    // Pad with spaces before number
    for _ in s.len()..width {
        io::write_str(1, b" ");
    }
    io::write_all(1, s);
}

/// Add a route
fn add_route(args: &[&[u8]]) -> i32 {
    let mut dest: Option<&[u8]> = None;
    let mut gateway: Option<&[u8]> = None;
    let mut netmask: Option<&[u8]> = None;
    let mut device: Option<&[u8]> = None;
    let mut is_host = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == b"-net" {
            is_host = false;
        } else if arg == b"-host" {
            is_host = true;
        } else if arg == b"gw" || arg == b"gateway" {
            i += 1;
            if i < args.len() {
                gateway = Some(args[i]);
            }
        } else if arg == b"netmask" {
            i += 1;
            if i < args.len() {
                netmask = Some(args[i]);
            }
        } else if arg == b"dev" {
            i += 1;
            if i < args.len() {
                device = Some(args[i]);
            }
        } else if dest.is_none() {
            dest = Some(arg);
        }
        i += 1;
    }

    let Some(dest_addr) = dest else {
        io::write_str(2, b"route: destination required\n");
        return 1;
    };

    // Build rtentry structure for ioctl
    let mut rt: rtentry = unsafe { core::mem::zeroed() };

    // Set destination
    let dest_sin = unsafe { &mut *(&mut rt.rt_dst as *mut _ as *mut libc::sockaddr_in) };
    dest_sin.sin_family = libc::AF_INET as u16;
    dest_sin.sin_addr.s_addr = if dest_addr == b"default" || dest_addr == b"0.0.0.0" {
        0
    } else {
        parse_ipv4(dest_addr).unwrap_or(0)
    };

    // Set gateway if provided
    if let Some(gw) = gateway {
        let gw_sin = unsafe { &mut *(&mut rt.rt_gateway as *mut _ as *mut libc::sockaddr_in) };
        gw_sin.sin_family = libc::AF_INET as u16;
        gw_sin.sin_addr.s_addr = parse_ipv4(gw).unwrap_or(0);
        rt.rt_flags |= RTF_GATEWAY as u16;
    }

    // Set netmask
    let mask_sin = unsafe { &mut *(&mut rt.rt_genmask as *mut _ as *mut libc::sockaddr_in) };
    mask_sin.sin_family = libc::AF_INET as u16;
    mask_sin.sin_addr.s_addr = if let Some(mask) = netmask {
        parse_ipv4(mask).unwrap_or(0xFFFFFFFF)
    } else if is_host {
        0xFFFFFFFF
    } else if dest_addr == b"default" {
        0
    } else {
        0xFFFFFF00 // Default /24
    };

    // Set flags
    rt.rt_flags |= RTF_UP as u16;
    if is_host {
        rt.rt_flags |= RTF_HOST as u16;
    }

    // Set device if provided
    if let Some(dev) = device {
        let mut dev_buf = [0u8; 16];
        let len = core::cmp::min(dev.len(), 15);
        dev_buf[..len].copy_from_slice(&dev[..len]);
        rt.rt_dev = dev_buf.as_ptr() as *mut libc::c_char;
    }

    // Open socket for ioctl
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        io::write_str(2, b"route: socket failed\n");
        return 1;
    }

    let ret = unsafe { libc::ioctl(sock, libc::SIOCADDRT as crate::io::IoctlReq, &rt) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        let err = sys::errno();
        io::write_str(2, b"route: ");
        if err == libc::EEXIST {
            io::write_str(2, b"route already exists\n");
        } else if err == libc::ENETUNREACH {
            io::write_str(2, b"network unreachable\n");
        } else if err == libc::EPERM {
            io::write_str(2, b"operation not permitted\n");
        } else {
            sys::perror(b"SIOCADDRT");
        }
        return 1;
    }

    0
}

/// Delete a route
fn del_route(args: &[&[u8]]) -> i32 {
    let mut dest: Option<&[u8]> = None;
    let mut netmask: Option<&[u8]> = None;
    let mut is_host = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == b"-net" {
            is_host = false;
        } else if arg == b"-host" {
            is_host = true;
        } else if arg == b"netmask" {
            i += 1;
            if i < args.len() {
                netmask = Some(args[i]);
            }
        } else if dest.is_none() {
            dest = Some(arg);
        }
        i += 1;
    }

    let Some(dest_addr) = dest else {
        io::write_str(2, b"route: destination required\n");
        return 1;
    };

    // Build rtentry structure
    let mut rt: rtentry = unsafe { core::mem::zeroed() };

    // Set destination
    let dest_sin = unsafe { &mut *(&mut rt.rt_dst as *mut _ as *mut libc::sockaddr_in) };
    dest_sin.sin_family = libc::AF_INET as u16;
    dest_sin.sin_addr.s_addr = if dest_addr == b"default" || dest_addr == b"0.0.0.0" {
        0
    } else {
        parse_ipv4(dest_addr).unwrap_or(0)
    };

    // Set netmask
    let mask_sin = unsafe { &mut *(&mut rt.rt_genmask as *mut _ as *mut libc::sockaddr_in) };
    mask_sin.sin_family = libc::AF_INET as u16;
    mask_sin.sin_addr.s_addr = if let Some(mask) = netmask {
        parse_ipv4(mask).unwrap_or(0xFFFFFFFF)
    } else if is_host {
        0xFFFFFFFF
    } else if dest_addr == b"default" {
        0
    } else {
        0xFFFFFF00
    };

    rt.rt_flags = RTF_UP as u16;
    if is_host {
        rt.rt_flags |= RTF_HOST as u16;
    }

    // Open socket for ioctl
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        io::write_str(2, b"route: socket failed\n");
        return 1;
    }

    let ret = unsafe { libc::ioctl(sock, libc::SIOCDELRT as crate::io::IoctlReq, &rt) };
    unsafe { libc::close(sock) };

    if ret < 0 {
        let err = sys::errno();
        io::write_str(2, b"route: ");
        if err == libc::ESRCH {
            io::write_str(2, b"no such route\n");
        } else if err == libc::EPERM {
            io::write_str(2, b"operation not permitted\n");
        } else {
            sys::perror(b"SIOCDELRT");
        }
        return 1;
    }

    0
}

/// Parse IPv4 address from dotted quad notation
fn parse_ipv4(s: &[u8]) -> Option<u32> {
    let mut octets = [0u8; 4];
    let mut octet_idx = 0;
    let mut current: u32 = 0;

    for &c in s {
        if c == b'.' {
            if octet_idx >= 3 || current > 255 {
                return None;
            }
            octets[octet_idx] = current as u8;
            octet_idx += 1;
            current = 0;
        } else if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u32;
        } else {
            return None;
        }
    }

    if octet_idx != 3 || current > 255 {
        return None;
    }
    octets[3] = current as u8;

    // Return in network byte order (big-endian)
    Some(u32::from_ne_bytes(octets))
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
    fn test_route_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["route"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Destination") || stdout.contains("routing"));
    }

    #[test]
    fn test_route_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["route", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.starts_with(b"Usage:"));
    }
}
