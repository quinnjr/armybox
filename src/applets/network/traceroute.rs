//! traceroute - print the route packets trace to network host
//!
//! Trace the route to a network host using UDP or ICMP probes.

use super::get_arg;
use crate::io;
use crate::sys;

/// traceroute - print the route packets trace to network host
///
/// # Synopsis
/// ```text
/// traceroute [-m max_ttl] [-p port] [-q nqueries] [-w wait] HOST
/// ```
///
/// # Description
/// Print the route packets take to reach a network host.
/// Shows each hop along the path with latency information.
///
/// # Options
/// - `-m max_ttl`: Set maximum TTL (default: 30)
/// - `-p port`: Set destination port (default: 33434)
/// - `-q nqueries`: Set number of probes per hop (default: 3)
/// - `-w wait`: Set timeout in seconds (default: 5)
/// - `-n`: Don't resolve addresses to hostnames
/// - `-I`: Use ICMP ECHO instead of UDP
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn traceroute(argc: i32, argv: *const *const u8) -> i32 {
    let mut max_ttl: u8 = 30;
    let mut port: u16 = 33434;
    let mut nqueries: u8 = 3;
    let mut wait_time: u32 = 5;
    let mut numeric_only = false;
    let mut use_icmp = false;
    let mut target: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-m" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                max_ttl = sys::parse_u64(val).unwrap_or(30) as u8;
            }
        } else if arg == b"-p" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                port = sys::parse_u64(val).unwrap_or(33434) as u16;
            }
        } else if arg == b"-q" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                nqueries = sys::parse_u64(val).unwrap_or(3) as u8;
            }
        } else if arg == b"-w" {
            i += 1;
            if let Some(val) = unsafe { get_arg(argv, i as i32) } {
                wait_time = sys::parse_u64(val).unwrap_or(5) as u32;
            }
        } else if arg == b"-n" {
            numeric_only = true;
        } else if arg == b"-I" {
            use_icmp = true;
        } else if !arg.starts_with(b"-") {
            target = Some(arg);
        }
        i += 1;
    }

    let host = match target {
        Some(h) => h,
        None => {
            io::write_str(2, b"traceroute: missing host operand\n");
            io::write_str(2, b"Usage: traceroute [-m max_ttl] [-p port] [-q nqueries] [-w wait] [-n] [-I] HOST\n");
            return 1;
        }
    };

    // Resolve hostname to IP address
    let dest_addr = match resolve_host(host) {
        Some(addr) => addr,
        None => {
            io::write_str(2, b"traceroute: cannot resolve ");
            io::write_all(2, host);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Print header
    io::write_str(1, b"traceroute to ");
    io::write_all(1, host);
    io::write_str(1, b" (");
    print_ip(dest_addr);
    io::write_str(1, b"), ");
    io::write_num(1, max_ttl as u64);
    io::write_str(1, b" hops max\n");

    // Create send socket
    let send_sock = if use_icmp {
        unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_ICMP) }
    } else {
        unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_UDP) }
    };

    if send_sock < 0 {
        io::write_str(
            2,
            b"traceroute: cannot create send socket (need CAP_NET_RAW or root)\n",
        );
        return 1;
    }

    // Create receive socket for ICMP responses
    let recv_sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_ICMP) };
    if recv_sock < 0 {
        io::write_str(
            2,
            b"traceroute: cannot create receive socket (need CAP_NET_RAW or root)\n",
        );
        unsafe { libc::close(send_sock) };
        return 1;
    }

    // Set receive timeout
    let tv = libc::timeval {
        tv_sec: wait_time as libc::time_t,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            recv_sock,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const _ as *const libc::c_void,
            core::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
    }

    // Trace route
    let mut reached = false;
    for ttl in 1..=max_ttl {
        // Set TTL on send socket
        let ttl_val = ttl as libc::c_int;
        unsafe {
            libc::setsockopt(
                send_sock,
                libc::IPPROTO_IP,
                libc::IP_TTL,
                &ttl_val as *const _ as *const libc::c_void,
                core::mem::size_of::<libc::c_int>() as libc::socklen_t,
            );
        }

        // Print TTL number (right-aligned)
        if ttl < 10 {
            io::write_str(1, b" ");
        }
        io::write_num(1, ttl as u64);
        io::write_str(1, b"  ");

        let mut last_addr: u32 = 0;
        let mut got_response = false;

        for q in 0..nqueries {
            // Prepare destination address
            let mut dest: libc::sockaddr_in = unsafe { core::mem::zeroed() };
            dest.sin_family = libc::AF_INET as u16;
            dest.sin_addr.s_addr = dest_addr.to_be();
            dest.sin_port = (port + (ttl as u16) + (q as u16)).to_be();

            // Get start time
            let mut start_tv: libc::timeval = unsafe { core::mem::zeroed() };
            unsafe { libc::gettimeofday(&mut start_tv, core::ptr::null_mut()) };

            // Send probe
            if use_icmp {
                // ICMP Echo Request
                let mut packet = [0u8; 64];
                packet[0] = 8; // ICMP Echo Request
                packet[1] = 0; // Code
                // Checksum will be at [2..4]
                packet[4] = ((ttl as u16) >> 8) as u8; // Identifier high
                packet[5] = ttl; // Identifier low
                packet[6] = 0; // Sequence high (q is small, always 0)
                packet[7] = q; // Sequence low

                // Calculate checksum
                let cksum = icmp_checksum(&packet[..8]);
                packet[2] = (cksum >> 8) as u8;
                packet[3] = (cksum & 0xFF) as u8;

                unsafe {
                    libc::sendto(
                        send_sock,
                        packet.as_ptr() as *const libc::c_void,
                        8,
                        0,
                        &dest as *const _ as *const libc::sockaddr,
                        core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                    );
                }
            } else {
                // UDP probe
                let probe_data = [0u8; 32];
                unsafe {
                    libc::sendto(
                        send_sock,
                        probe_data.as_ptr() as *const libc::c_void,
                        probe_data.len(),
                        0,
                        &dest as *const _ as *const libc::sockaddr,
                        core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                    );
                }
            }

            // Wait for ICMP response
            let mut recv_buf = [0u8; 512];
            let mut from: libc::sockaddr_in = unsafe { core::mem::zeroed() };
            let mut from_len: libc::socklen_t =
                core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

            let n = unsafe {
                libc::recvfrom(
                    recv_sock,
                    recv_buf.as_mut_ptr() as *mut libc::c_void,
                    recv_buf.len(),
                    0,
                    &mut from as *mut _ as *mut libc::sockaddr,
                    &mut from_len,
                )
            };

            // Get end time
            let mut end_tv: libc::timeval = unsafe { core::mem::zeroed() };
            unsafe { libc::gettimeofday(&mut end_tv, core::ptr::null_mut()) };

            if n > 0 {
                got_response = true;
                let from_addr = u32::from_be(from.sin_addr.s_addr);

                // Print hostname/IP if different from last
                if from_addr != last_addr {
                    if last_addr != 0 {
                        io::write_str(1, b"\n    ");
                    }
                    if !numeric_only {
                        // Try reverse DNS (simplified - just print IP for now)
                        print_ip(from_addr);
                    } else {
                        print_ip(from_addr);
                    }
                    io::write_str(1, b" ");
                    last_addr = from_addr;
                }

                // Calculate and print RTT
                let rtt_us = ((end_tv.tv_sec - start_tv.tv_sec) * 1_000_000
                    + (end_tv.tv_usec - start_tv.tv_usec)) as u64;
                let rtt_ms = rtt_us / 1000;
                let rtt_frac = (rtt_us % 1000) / 100;

                io::write_str(1, b" ");
                io::write_num(1, rtt_ms);
                io::write_str(1, b".");
                io::write_num(1, rtt_frac);
                io::write_str(1, b" ms");

                // Check if we reached the destination
                // ICMP type 0 = Echo Reply, or from_addr == dest_addr
                if from_addr == dest_addr {
                    reached = true;
                }

                // Check ICMP type (after IP header, typically 20 bytes)
                if n >= 28 {
                    let icmp_type = recv_buf[20];
                    if icmp_type == 0 || icmp_type == 3 {
                        // Echo Reply or Destination Unreachable
                        reached = true;
                    }
                }
            } else {
                // Timeout
                io::write_str(1, b" *");
            }
        }

        io::write_str(1, b"\n");

        if reached {
            break;
        }
    }

    unsafe {
        libc::close(send_sock);
        libc::close(recv_sock);
    }

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn traceroute(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"traceroute: only available on Linux\n");
    1
}

/// Resolve hostname to IPv4 address
#[cfg(any(target_os = "linux", target_os = "android"))]
fn resolve_host(host: &[u8]) -> Option<u32> {
    // First try parsing as IP address
    if let Some(addr) = parse_ipv4(host) {
        return Some(addr);
    }

    // Use getaddrinfo for DNS resolution
    let mut host_cstr = [0u8; 256];
    let len = core::cmp::min(host.len(), 255);
    host_cstr[..len].copy_from_slice(&host[..len]);
    host_cstr[len] = 0;

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_INET;
    hints.ai_socktype = libc::SOCK_DGRAM;

    let mut result: *mut libc::addrinfo = core::ptr::null_mut();

    let ret = unsafe {
        libc::getaddrinfo(
            host_cstr.as_ptr() as *const libc::c_char,
            core::ptr::null(),
            &hints,
            &mut result,
        )
    };

    if ret != 0 || result.is_null() {
        return None;
    }

    let addr = unsafe {
        let ai = &*result;
        if ai.ai_family == libc::AF_INET && !ai.ai_addr.is_null() {
            let sin = &*(ai.ai_addr as *const libc::sockaddr_in);
            Some(u32::from_be(sin.sin_addr.s_addr))
        } else {
            None
        }
    };

    unsafe { libc::freeaddrinfo(result) };

    addr
}

/// Parse IPv4 address string
fn parse_ipv4(s: &[u8]) -> Option<u32> {
    let mut parts = [0u8; 4];
    let mut part_idx = 0;
    let mut current: u16 = 0;
    let mut has_digit = false;

    for &c in s {
        if c == b'.' {
            if !has_digit || part_idx >= 3 {
                return None;
            }
            if current > 255 {
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

    if !has_digit || part_idx != 3 {
        return None;
    }
    if current > 255 {
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

/// Print IPv4 address
fn print_ip(addr: u32) {
    let mut buf = [0u8; 16];

    let a = ((addr >> 24) & 0xFF) as u64;
    let b = ((addr >> 16) & 0xFF) as u64;
    let c = ((addr >> 8) & 0xFF) as u64;
    let d = (addr & 0xFF) as u64;

    io::write_all(1, sys::format_u64(a, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(b, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(c, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(d, &mut buf));
}

/// Calculate ICMP checksum
#[cfg(any(target_os = "linux", target_os = "android"))]
fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;

    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }

    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
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
    fn test_traceroute_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["traceroute"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing host"));
    }

    #[test]
    fn test_parse_ipv4() {
        assert_eq!(super::parse_ipv4(b"127.0.0.1"), Some(0x7F000001));
        assert_eq!(super::parse_ipv4(b"192.168.1.1"), Some(0xC0A80101));
        assert_eq!(super::parse_ipv4(b"8.8.8.8"), Some(0x08080808));
        assert_eq!(super::parse_ipv4(b"invalid"), None);
        assert_eq!(super::parse_ipv4(b"256.1.1.1"), None);
        assert_eq!(super::parse_ipv4(b"1.2.3"), None);
    }
}
