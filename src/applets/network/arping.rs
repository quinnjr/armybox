//! arping - send ARP requests to a neighbor host
//!
//! ARP-level ping utility for network discovery.

use super::get_arg;
use crate::io;
use crate::sys;

// ARP constants
const ARPHRD_ETHER: u16 = 1;
const ARPOP_REQUEST: u16 = 1;
const ARPOP_REPLY: u16 = 2;
const ETH_P_ARP: u16 = 0x0806;
const ETH_ALEN: usize = 6;
const IP_ALEN: usize = 4;

/// arping - send ARP requests to a neighbor host
///
/// # Synopsis
/// ```text
/// arping [-c COUNT] [-I IFACE] [-w TIMEOUT] HOST
/// ```
///
/// # Description
/// Send ARP REQUEST packets to discover hosts on the network.
/// Useful for checking if a host is up at the link layer.
///
/// # Options
/// - `-c COUNT`: Stop after sending COUNT requests
/// - `-I IFACE`: Network interface to use
/// - `-w TIMEOUT`: Timeout in seconds
///
/// # Exit Status
/// - 0: At least one reply received
/// - 1: No reply received or error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn arping(argc: i32, argv: *const *const u8) -> i32 {
    let mut count: Option<u32> = None;
    let mut interface: &[u8] = b"eth0";
    let mut timeout: u32 = 1;
    let mut target: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-c" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                count = sys::parse_u64(val).map(|n| n as u32);
            }
        } else if arg == b"-I" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                interface = val;
            }
        } else if arg == b"-w" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                timeout = sys::parse_u64(val).unwrap_or(1) as u32;
            }
        } else if !arg.starts_with(b"-") {
            target = Some(arg);
        }
        i += 1;
    }

    let target_ip = match target {
        Some(t) => match parse_ipv4(t) {
            Some(ip) => ip,
            None => {
                io::write_str(2, b"arping: invalid IP address\n");
                return 1;
            }
        },
        None => {
            io::write_str(
                2,
                b"Usage: arping [-c COUNT] [-I IFACE] [-w TIMEOUT] HOST\n",
            );
            return 1;
        }
    };

    // Get interface information
    let (src_mac, src_ip, if_index) = match get_interface_info(interface) {
        Some(info) => info,
        None => {
            io::write_str(2, b"arping: cannot get interface info for ");
            io::write_all(2, interface);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Create raw socket for ARP
    let sock = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, (ETH_P_ARP as i32).to_be()) };

    if sock < 0 {
        io::write_str(
            2,
            b"arping: cannot create raw socket (need CAP_NET_RAW or root)\n",
        );
        return 1;
    }

    // Set timeout
    let tv = libc::timeval {
        tv_sec: timeout as libc::time_t,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const _ as *const libc::c_void,
            core::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
    }

    // Bind to interface
    let mut sll: libc::sockaddr_ll = unsafe { core::mem::zeroed() };
    sll.sll_family = libc::AF_PACKET as u16;
    sll.sll_protocol = (ETH_P_ARP as u16).to_be();
    sll.sll_ifindex = if_index;

    unsafe {
        libc::bind(
            sock,
            &sll as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
        );
    }

    io::write_str(1, b"ARPING ");
    print_ip(target_ip);
    io::write_str(1, b" from ");
    print_ip(src_ip);
    io::write_str(1, b" ");
    io::write_all(1, interface);
    io::write_str(1, b"\n");

    let mut sent = 0u32;
    let mut received = 0u32;
    let max_count = count.unwrap_or(u32::MAX);

    while sent < max_count {
        // Build ARP request packet
        let mut packet = [0u8; 42]; // Ethernet header (14) + ARP (28)

        // Ethernet header
        // Destination: broadcast
        packet[0..6].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        // Source
        packet[6..12].copy_from_slice(&src_mac);
        // Type: ARP
        packet[12] = (ETH_P_ARP >> 8) as u8;
        packet[13] = (ETH_P_ARP & 0xFF) as u8;

        // ARP header
        let arp = &mut packet[14..];
        // Hardware type: Ethernet
        arp[0] = (ARPHRD_ETHER >> 8) as u8;
        arp[1] = (ARPHRD_ETHER & 0xFF) as u8;
        // Protocol type: IPv4
        arp[2] = 0x08;
        arp[3] = 0x00;
        // Hardware address length
        arp[4] = ETH_ALEN as u8;
        // Protocol address length
        arp[5] = IP_ALEN as u8;
        // Operation: request
        arp[6] = (ARPOP_REQUEST >> 8) as u8;
        arp[7] = (ARPOP_REQUEST & 0xFF) as u8;
        // Sender hardware address
        arp[8..14].copy_from_slice(&src_mac);
        // Sender protocol address
        arp[14] = ((src_ip >> 24) & 0xFF) as u8;
        arp[15] = ((src_ip >> 16) & 0xFF) as u8;
        arp[16] = ((src_ip >> 8) & 0xFF) as u8;
        arp[17] = (src_ip & 0xFF) as u8;
        // Target hardware address (zeros for request)
        arp[18..24].copy_from_slice(&[0, 0, 0, 0, 0, 0]);
        // Target protocol address
        arp[24] = ((target_ip >> 24) & 0xFF) as u8;
        arp[25] = ((target_ip >> 16) & 0xFF) as u8;
        arp[26] = ((target_ip >> 8) & 0xFF) as u8;
        arp[27] = (target_ip & 0xFF) as u8;

        // Get send time
        let mut start_tv: libc::timeval = unsafe { core::mem::zeroed() };
        unsafe { libc::gettimeofday(&mut start_tv, core::ptr::null_mut()) };

        // Send packet
        let mut dest_sll: libc::sockaddr_ll = unsafe { core::mem::zeroed() };
        dest_sll.sll_family = libc::AF_PACKET as u16;
        dest_sll.sll_protocol = (ETH_P_ARP as u16).to_be();
        dest_sll.sll_ifindex = if_index;
        dest_sll.sll_halen = ETH_ALEN as u8;
        dest_sll.sll_addr[0..6].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

        let ret = unsafe {
            libc::sendto(
                sock,
                packet.as_ptr() as *const libc::c_void,
                packet.len(),
                0,
                &dest_sll as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            )
        };

        if ret < 0 {
            io::write_str(2, b"arping: send failed\n");
            break;
        }

        sent += 1;

        // Wait for reply
        let mut recv_buf = [0u8; 60];
        let mut from: libc::sockaddr_ll = unsafe { core::mem::zeroed() };
        let mut from_len: libc::socklen_t =
            core::mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t;

        let n = unsafe {
            libc::recvfrom(
                sock,
                recv_buf.as_mut_ptr() as *mut libc::c_void,
                recv_buf.len(),
                0,
                &mut from as *mut _ as *mut libc::sockaddr,
                &mut from_len,
            )
        };

        let mut end_tv: libc::timeval = unsafe { core::mem::zeroed() };
        unsafe { libc::gettimeofday(&mut end_tv, core::ptr::null_mut()) };

        if n >= 42 {
            // Check if it's an ARP reply from our target
            let recv_arp = &recv_buf[14..];
            let op = ((recv_arp[6] as u16) << 8) | (recv_arp[7] as u16);
            let sender_ip = ((recv_arp[14] as u32) << 24)
                | ((recv_arp[15] as u32) << 16)
                | ((recv_arp[16] as u32) << 8)
                | (recv_arp[17] as u32);

            if op == ARPOP_REPLY && sender_ip == target_ip {
                received += 1;

                // Calculate RTT
                let rtt_us = ((end_tv.tv_sec - start_tv.tv_sec) * 1_000_000
                    + (end_tv.tv_usec - start_tv.tv_usec)) as u64;

                io::write_str(1, b"Unicast reply from ");
                print_ip(sender_ip);
                io::write_str(1, b" [");
                print_mac(&recv_arp[8..14]);
                io::write_str(1, b"]  ");
                io::write_num(1, rtt_us / 1000);
                io::write_str(1, b".");
                io::write_num(1, (rtt_us % 1000) / 100);
                io::write_str(1, b"ms\n");
            }
        }

        // Sleep between requests (unless last one)
        if sent < max_count {
            unsafe { libc::sleep(1) };
        }
    }

    unsafe { libc::close(sock) };

    // Print summary
    io::write_str(1, b"Sent ");
    io::write_num(1, sent as u64);
    io::write_str(1, b" probes (");
    io::write_num(1, received as u64);
    io::write_str(1, b" received)\n");

    if received > 0 { 0 } else { 1 }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn arping(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"arping: only available on Linux\n");
    1
}

/// Get interface MAC address, IP address, and index
#[cfg(any(target_os = "linux", target_os = "android"))]
fn get_interface_info(name: &[u8]) -> Option<([u8; 6], u32, i32)> {
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        return None;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    let name_len = core::cmp::min(name.len(), libc::IFNAMSIZ - 1);
    unsafe {
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            ifr.ifr_name.as_mut_ptr() as *mut u8,
            name_len,
        );
    }

    // Get interface index
    let if_index = unsafe {
        if libc::ioctl(sock, libc::SIOCGIFINDEX as crate::io::IoctlReq, &mut ifr) < 0 {
            libc::close(sock);
            return None;
        }
        ifr.ifr_ifru.ifru_ifindex
    };

    // Get MAC address
    let mac = unsafe {
        if libc::ioctl(sock, libc::SIOCGIFHWADDR as crate::io::IoctlReq, &mut ifr) < 0 {
            libc::close(sock);
            return None;
        }
        let mut m = [0u8; 6];
        m.copy_from_slice(
            &ifr.ifr_ifru.ifru_hwaddr.sa_data[0..6]
                .iter()
                .map(|&x| x as u8)
                .collect::<alloc::vec::Vec<u8>>(),
        );
        m
    };

    // Get IP address
    let ip = unsafe {
        if libc::ioctl(sock, libc::SIOCGIFADDR as crate::io::IoctlReq, &mut ifr) < 0 {
            libc::close(sock);
            return None;
        }
        let sin = &*(&ifr.ifr_ifru.ifru_addr as *const libc::sockaddr as *const libc::sockaddr_in);
        u32::from_be(sin.sin_addr.s_addr)
    };

    unsafe { libc::close(sock) };

    Some((mac, ip, if_index))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn parse_ipv4(s: &[u8]) -> Option<u32> {
    let mut parts = [0u8; 4];
    let mut part_idx = 0;
    let mut current: u16 = 0;
    let mut has_digit = false;

    for &c in s {
        if c == b'.' {
            if !has_digit || part_idx >= 3 || current > 255 {
                return None;
            }
            parts[part_idx] = current as u8;
            part_idx += 1;
            current = 0;
            has_digit = false;
        } else if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u16;
            has_digit = true;
            if current > 255 {
                return None;
            }
        } else {
            return None;
        }
    }

    if !has_digit || part_idx != 3 || current > 255 {
        return None;
    }
    parts[3] = current as u8;

    Some(
        ((parts[0] as u32) << 24)
            | ((parts[1] as u32) << 16)
            | ((parts[2] as u32) << 8)
            | (parts[3] as u32),
    )
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_ip(ip: u32) {
    let mut buf = [0u8; 16];
    io::write_all(1, sys::format_u64(((ip >> 24) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(((ip >> 16) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(((ip >> 8) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64((ip & 0xFF) as u64, &mut buf));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_mac(mac: &[u8]) {
    let mut buf = [0u8; 8];
    for (i, &b) in mac.iter().enumerate() {
        if i > 0 {
            io::write_str(1, b":");
        }
        let hex = sys::format_hex(b as u64, &mut buf);
        if hex.len() < 2 {
            io::write_str(1, b"0");
        }
        io::write_all(1, hex);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_arping_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["arping"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_parse_ipv4() {
        assert_eq!(super::parse_ipv4(b"192.168.1.1"), Some(0xC0A80101));
        assert_eq!(super::parse_ipv4(b"invalid"), None);
    }
}
