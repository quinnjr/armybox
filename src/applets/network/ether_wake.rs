//! ether-wake - send Wake-on-LAN magic packet
//!
//! Wake up machines via Wake-on-LAN.

use super::get_arg;
use crate::io;

/// ether-wake - send Wake-on-LAN magic packet
///
/// # Synopsis
/// ```text
/// ether-wake [-i IFACE] [-b] MAC
/// ```
///
/// # Description
/// Send Wake-on-LAN magic packet to wake up a remote machine.
/// The magic packet consists of 6 bytes of 0xFF followed by
/// 16 repetitions of the target MAC address.
///
/// # Options
/// - `-i IFACE`: Use interface IFACE
/// - `-b`: Send to broadcast address (default)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn ether_wake(argc: i32, argv: *const *const u8) -> i32 {
    let mut interface: Option<&[u8]> = None;
    let mut mac: Option<[u8; 6]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-i" {
            i += 1;
            if i < argc as usize {
                interface = unsafe { get_arg(argv, i as i32) };
            }
        } else if arg == b"-b" {
            // Broadcast is default, ignore
        } else if !arg.starts_with(b"-") && mac.is_none() {
            mac = parse_mac(arg);
            if mac.is_none() {
                io::write_str(2, b"ether-wake: invalid MAC address\n");
                return 1;
            }
        }
        i += 1;
    }

    let mac = match mac {
        Some(m) => m,
        None => {
            io::write_str(2, b"Usage: ether-wake [-i IFACE] [-b] MAC\n");
            io::write_str(2, b"  MAC format: AA:BB:CC:DD:EE:FF or AA-BB-CC-DD-EE-FF\n");
            return 1;
        }
    };

    // Build magic packet: 6 bytes of 0xFF + 16 repetitions of MAC
    let mut packet = [0u8; 102];

    // Synchronization stream (6 bytes of 0xFF)
    for j in 0..6 {
        packet[j] = 0xFF;
    }

    // 16 repetitions of target MAC
    for rep in 0..16 {
        for j in 0..6 {
            packet[6 + rep * 6 + j] = mac[j];
        }
    }

    // Create UDP socket
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_UDP) };
    if sock < 0 {
        io::write_str(2, b"ether-wake: cannot create socket\n");
        return 1;
    }

    // Enable broadcast
    let broadcast: libc::c_int = 1;
    let ret = unsafe {
        libc::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_BROADCAST,
            &broadcast as *const _ as *const libc::c_void,
            core::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    if ret < 0 {
        io::write_str(2, b"ether-wake: cannot enable broadcast\n");
        unsafe { libc::close(sock) };
        return 1;
    }

    // Bind to interface if specified
    if let Some(iface) = interface {
        let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
        let len = core::cmp::min(iface.len(), ifr.ifr_name.len() - 1);
        for j in 0..len {
            ifr.ifr_name[j] = iface[j] as libc::c_char;
        }

        let ret = unsafe {
            libc::setsockopt(
                sock,
                libc::SOL_SOCKET,
                libc::SO_BINDTODEVICE,
                &ifr as *const _ as *const libc::c_void,
                core::mem::size_of::<libc::ifreq>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            io::write_str(2, b"ether-wake: cannot bind to interface\n");
            unsafe { libc::close(sock) };
            return 1;
        }
    }

    // Send to broadcast address on port 9 (discard)
    let mut dest: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    dest.sin_family = libc::AF_INET as libc::sa_family_t;
    dest.sin_port = 9u16.to_be(); // WOL typically uses port 9
    dest.sin_addr.s_addr = 0xFFFFFFFFu32.to_be(); // 255.255.255.255

    let sent = unsafe {
        libc::sendto(
            sock,
            packet.as_ptr() as *const libc::c_void,
            packet.len(),
            0,
            &dest as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    unsafe { libc::close(sock) };

    if sent < 0 {
        io::write_str(2, b"ether-wake: send failed\n");
        return 1;
    }

    // Print confirmation
    io::write_str(1, b"Sent magic packet to ");
    print_mac(&mac);
    io::write_str(1, b"\n");

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ether_wake(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"ether-wake: only available on Linux\n");
    1
}

/// Parse MAC address in AA:BB:CC:DD:EE:FF or AA-BB-CC-DD-EE-FF format
#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_mac(s: &[u8]) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut idx = 0;
    let mut byte = 0u8;
    let mut nibble_count = 0;

    for &c in s {
        if c == b':' || c == b'-' {
            if nibble_count != 2 {
                return None;
            }
            if idx >= 6 {
                return None;
            }
            mac[idx] = byte;
            idx += 1;
            byte = 0;
            nibble_count = 0;
        } else {
            let digit = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => return None,
            };
            if nibble_count >= 2 {
                return None;
            }
            byte = byte * 16 + digit;
            nibble_count += 1;
        }
    }

    // Handle last byte
    if nibble_count != 2 || idx != 5 {
        return None;
    }
    mac[5] = byte;

    Some(mac)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_mac(mac: &[u8; 6]) {
    let mut buf = [0u8; 3];
    for (i, &b) in mac.iter().enumerate() {
        if i > 0 {
            io::write_str(1, b":");
        }
        let hex = format_hex_byte(b, &mut buf);
        io::write_all(1, hex);
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn format_hex_byte(val: u8, buf: &mut [u8; 3]) -> &[u8] {
    const HEX: &[u8] = b"0123456789abcdef";
    buf[0] = HEX[(val >> 4) as usize];
    buf[1] = HEX[(val & 0x0F) as usize];
    &buf[0..2]
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ether_wake_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ether-wake"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_parse_mac() {
        assert_eq!(
            super::parse_mac(b"aa:bb:cc:dd:ee:ff"),
            Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
        );
        assert_eq!(
            super::parse_mac(b"AA-BB-CC-DD-EE-FF"),
            Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
        );
        assert_eq!(
            super::parse_mac(b"00:11:22:33:44:55"),
            Some([0x00, 0x11, 0x22, 0x33, 0x44, 0x55])
        );
        assert_eq!(super::parse_mac(b"invalid"), None);
        assert_eq!(super::parse_mac(b"aa:bb:cc:dd:ee"), None);
    }
}
