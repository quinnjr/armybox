//! sntp - simple NTP client
//!
//! Query NTP servers for time synchronization.

use super::get_arg;
use crate::io;
use crate::sys;

// NTP constants
const NTP_PORT: u16 = 123;
const NTP_PACKET_SIZE: usize = 48;
const NTP_EPOCH_OFFSET: u64 = 2208988800; // Seconds from 1900 to 1970

/// sntp - simple NTP client
///
/// # Synopsis
/// ```text
/// sntp [-s] [-p] SERVER
/// ```
///
/// # Description
/// Query NTP servers to get current time. Can optionally set system time.
///
/// # Options
/// - `-s`: Set system time (requires root)
/// - `-p`: Print time only (default)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn sntp(argc: i32, argv: *const *const u8) -> i32 {
    let mut set_time = false;
    let mut server: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-s" {
            set_time = true;
        } else if arg == b"-p" {
            set_time = false;
        } else if !arg.starts_with(b"-") {
            server = Some(arg);
        }
        i += 1;
    }

    let server = match server {
        Some(s) => s,
        None => {
            io::write_str(2, b"Usage: sntp [-s] [-p] SERVER\n");
            io::write_str(2, b"  -s: Set system time\n");
            io::write_str(2, b"  -p: Print time only (default)\n");
            return 1;
        }
    };

    // Resolve server
    let server_addr = match resolve_host(server) {
        Some(a) => a,
        None => {
            io::write_str(2, b"sntp: cannot resolve ");
            io::write_all(2, server);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Create UDP socket
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        io::write_str(2, b"sntp: cannot create socket\n");
        return 1;
    }

    // Set timeout
    let tv = libc::timeval {
        tv_sec: 5,
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

    // Build NTP request packet
    // LI=0, VN=3, Mode=3 (client) -> 0x1B
    let mut packet = [0u8; NTP_PACKET_SIZE];
    packet[0] = 0x1B; // LI=0, VN=3, Mode=3

    // Get transmit timestamp
    let mut now_tv: libc::timeval = unsafe { core::mem::zeroed() };
    unsafe { libc::gettimeofday(&mut now_tv, core::ptr::null_mut()) };

    // Store transmit timestamp (for calculating roundtrip)
    let t1_sec = now_tv.tv_sec as u64 + NTP_EPOCH_OFFSET;
    let t1_frac = ((now_tv.tv_usec as u64) << 32) / 1_000_000;

    // Set transmit timestamp in packet (bytes 40-47)
    packet[40] = ((t1_sec >> 24) & 0xFF) as u8;
    packet[41] = ((t1_sec >> 16) & 0xFF) as u8;
    packet[42] = ((t1_sec >> 8) & 0xFF) as u8;
    packet[43] = (t1_sec & 0xFF) as u8;
    packet[44] = ((t1_frac >> 24) & 0xFF) as u8;
    packet[45] = ((t1_frac >> 16) & 0xFF) as u8;
    packet[46] = ((t1_frac >> 8) & 0xFF) as u8;
    packet[47] = (t1_frac & 0xFF) as u8;

    // Send request
    let mut dest = server_addr;
    dest.sin_port = NTP_PORT.to_be();

    let sent = unsafe {
        libc::sendto(
            sock,
            packet.as_ptr() as *const libc::c_void,
            NTP_PACKET_SIZE,
            0,
            &dest as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    if sent < 0 {
        io::write_str(2, b"sntp: send failed\n");
        unsafe { libc::close(sock) };
        return 1;
    }

    // Receive response
    let mut recv_buf = [0u8; NTP_PACKET_SIZE];
    let mut from: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    let mut from_len: libc::socklen_t =
        core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

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

    // Get receive timestamp
    let mut recv_tv: libc::timeval = unsafe { core::mem::zeroed() };
    unsafe { libc::gettimeofday(&mut recv_tv, core::ptr::null_mut()) };

    unsafe { libc::close(sock) };

    if n < NTP_PACKET_SIZE as isize {
        io::write_str(2, b"sntp: timeout or invalid response\n");
        return 1;
    }

    // Parse response
    // Check mode (should be 4 = server)
    let mode = recv_buf[0] & 0x07;
    if mode != 4 {
        io::write_str(2, b"sntp: unexpected response mode\n");
        return 1;
    }

    // Extract transmit timestamp from server (bytes 40-47)
    // This is the time when the server sent the response
    let tx_sec = ((recv_buf[40] as u64) << 24)
        | ((recv_buf[41] as u64) << 16)
        | ((recv_buf[42] as u64) << 8)
        | (recv_buf[43] as u64);

    let tx_frac = ((recv_buf[44] as u64) << 24)
        | ((recv_buf[45] as u64) << 16)
        | ((recv_buf[46] as u64) << 8)
        | (recv_buf[47] as u64);

    // Convert to Unix timestamp
    let unix_sec = tx_sec.saturating_sub(NTP_EPOCH_OFFSET);
    let unix_usec = (tx_frac * 1_000_000) >> 32;

    // Calculate offset (simplified - just use transmit time)
    // Real SNTP would calculate: offset = ((t2-t1) + (t3-t4)) / 2

    // Print the time
    io::write_all(1, server);
    io::write_str(1, b": ");

    // Format time
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    let time_t = unix_sec as libc::time_t;
    unsafe { libc::gmtime_r(&time_t, &mut tm) };

    let mut buf = [0u8; 16];

    // YYYY-MM-DD HH:MM:SS.usec UTC
    io::write_all(1, sys::format_u64((tm.tm_year + 1900) as u64, &mut buf));
    io::write_str(1, b"-");
    print_padded((tm.tm_mon + 1) as u64, 2, &mut buf);
    io::write_str(1, b"-");
    print_padded(tm.tm_mday as u64, 2, &mut buf);
    io::write_str(1, b" ");
    print_padded(tm.tm_hour as u64, 2, &mut buf);
    io::write_str(1, b":");
    print_padded(tm.tm_min as u64, 2, &mut buf);
    io::write_str(1, b":");
    print_padded(tm.tm_sec as u64, 2, &mut buf);
    io::write_str(1, b".");
    print_padded(unix_usec / 1000, 3, &mut buf);
    io::write_str(1, b" UTC\n");

    // Set system time if requested
    if set_time {
        let new_tv = libc::timeval {
            tv_sec: unix_sec as libc::time_t,
            tv_usec: unix_usec as libc::suseconds_t,
        };

        let ret = unsafe { libc::settimeofday(&new_tv, core::ptr::null()) };
        if ret < 0 {
            io::write_str(2, b"sntp: cannot set time (need root)\n");
            return 1;
        }
        io::write_str(1, b"System time set.\n");
    }

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn sntp(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"sntp: only available on Linux\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_padded(val: u64, width: usize, buf: &mut [u8; 16]) {
    let s = sys::format_u64(val, buf);
    for _ in 0..(width.saturating_sub(s.len())) {
        io::write_str(1, b"0");
    }
    io::write_all(1, s);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn resolve_host(host: &[u8]) -> Option<libc::sockaddr_in> {
    if let Some(ip) = parse_ipv4(host) {
        let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        addr.sin_family = libc::AF_INET as u16;
        addr.sin_addr.s_addr = ip.to_be();
        return Some(addr);
    }

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
            Some(*(ai.ai_addr as *const libc::sockaddr_in))
        } else {
            None
        }
    };

    unsafe { libc::freeaddrinfo(result) };
    addr
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

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_sntp_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["sntp"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }
}
