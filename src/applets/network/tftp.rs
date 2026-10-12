//! tftp - trivial file transfer protocol client
//!
//! Simple TFTP client for file transfers.

use super::get_arg;
use crate::io;

// TFTP opcodes
const TFTP_RRQ: u16 = 1; // Read request
const TFTP_WRQ: u16 = 2; // Write request
const TFTP_DATA: u16 = 3; // Data packet
const TFTP_ACK: u16 = 4; // Acknowledgment
const TFTP_ERROR: u16 = 5; // Error packet

// TFTP constants
const TFTP_PORT: u16 = 69;
const TFTP_BLOCK_SIZE: usize = 512;

/// tftp - trivial file transfer protocol client
///
/// # Synopsis
/// ```text
/// tftp [-g|-p] -l LOCAL -r REMOTE HOST
/// ```
///
/// # Description
/// Transfer files using TFTP protocol.
///
/// # Options
/// - `-g`: Get file from server
/// - `-p`: Put file to server
/// - `-l LOCAL`: Local filename
/// - `-r REMOTE`: Remote filename
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn tftp(argc: i32, argv: *const *const u8) -> i32 {
    let mut get_mode = false;
    let mut put_mode = false;
    let mut local_file: Option<&[u8]> = None;
    let mut remote_file: Option<&[u8]> = None;
    let mut host: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-g" {
            get_mode = true;
        } else if arg == b"-p" {
            put_mode = true;
        } else if arg == b"-l" {
            i += 1;
            local_file = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-r" {
            i += 1;
            remote_file = unsafe { get_arg(argv, i as i32) };
        } else if !arg.starts_with(b"-") {
            host = Some(arg);
        }
        i += 1;
    }

    let host = match host {
        Some(h) => h,
        None => {
            io::write_str(2, b"Usage: tftp [-g|-p] -l LOCAL -r REMOTE HOST\n");
            return 1;
        }
    };

    if !get_mode && !put_mode {
        io::write_str(2, b"tftp: must specify -g (get) or -p (put)\n");
        return 1;
    }

    let local = match local_file {
        Some(f) => f,
        None => {
            io::write_str(2, b"tftp: missing local filename (-l)\n");
            return 1;
        }
    };

    let remote = match remote_file {
        Some(f) => f,
        None => local,
    };

    // Resolve host
    let server_addr = match resolve_host(host) {
        Some(a) => a,
        None => {
            io::write_str(2, b"tftp: cannot resolve host\n");
            return 1;
        }
    };

    // Create UDP socket
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if sock < 0 {
        io::write_str(2, b"tftp: cannot create socket\n");
        return 1;
    }

    // Set receive timeout
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

    let result = if get_mode {
        tftp_get(sock, &server_addr, remote, local)
    } else {
        tftp_put(sock, &server_addr, remote, local)
    };

    unsafe { libc::close(sock) };

    result
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn tftp(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"tftp: only available on Linux\n");
    1
}

/// Download file from TFTP server
#[cfg(any(target_os = "linux", target_os = "android"))]
fn tftp_get(sock: i32, server: &libc::sockaddr_in, remote: &[u8], local: &[u8]) -> i32 {
    // Build RRQ packet
    let mut packet = [0u8; 516];
    packet[0] = 0;
    packet[1] = TFTP_RRQ as u8;

    let mut pos = 2;
    for &b in remote {
        if pos < 512 {
            packet[pos] = b;
            pos += 1;
        }
    }
    packet[pos] = 0;
    pos += 1;

    for &b in b"octet" {
        packet[pos] = b;
        pos += 1;
    }
    packet[pos] = 0;
    pos += 1;

    // Send RRQ
    let mut dest = *server;
    dest.sin_port = TFTP_PORT.to_be();

    let sent = unsafe {
        libc::sendto(
            sock,
            packet.as_ptr() as *const libc::c_void,
            pos,
            0,
            &dest as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    if sent < 0 {
        io::write_str(2, b"tftp: send failed\n");
        return 1;
    }

    // Open local file
    let fd = io::open(local, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if fd < 0 {
        io::write_str(2, b"tftp: cannot create local file\n");
        return 1;
    }

    let mut expected_block: u16 = 1;
    let mut total_bytes: u64 = 0;

    loop {
        let mut recv_buf = [0u8; 516];
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

        if n < 4 {
            io::write_str(2, b"tftp: timeout or invalid packet\n");
            io::close(fd);
            return 1;
        }

        let opcode = ((recv_buf[0] as u16) << 8) | (recv_buf[1] as u16);
        let block_num = ((recv_buf[2] as u16) << 8) | (recv_buf[3] as u16);

        if opcode == TFTP_ERROR {
            io::write_str(2, b"tftp: server error\n");
            io::close(fd);
            return 1;
        }

        if opcode != TFTP_DATA || block_num != expected_block {
            continue;
        }

        let data_len = (n as usize) - 4;
        if data_len > 0 {
            io::write_all(fd, &recv_buf[4..4 + data_len]);
            total_bytes += data_len as u64;
        }

        // Send ACK
        let ack = [0u8, TFTP_ACK as u8, recv_buf[2], recv_buf[3]];
        unsafe {
            libc::sendto(
                sock,
                ack.as_ptr() as *const libc::c_void,
                4,
                0,
                &from as *const _ as *const libc::sockaddr,
                from_len,
            );
        }

        if data_len < TFTP_BLOCK_SIZE {
            break;
        }

        expected_block = expected_block.wrapping_add(1);
    }

    io::close(fd);

    io::write_str(1, b"Received ");
    io::write_num(1, total_bytes);
    io::write_str(1, b" bytes\n");

    0
}

/// Upload file to TFTP server
#[cfg(any(target_os = "linux", target_os = "android"))]
fn tftp_put(sock: i32, server: &libc::sockaddr_in, remote: &[u8], local: &[u8]) -> i32 {
    let fd = io::open(local, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"tftp: cannot open local file\n");
        return 1;
    }

    // Build WRQ packet
    let mut packet = [0u8; 516];
    packet[0] = 0;
    packet[1] = TFTP_WRQ as u8;

    let mut pos = 2;
    for &b in remote {
        if pos < 512 {
            packet[pos] = b;
            pos += 1;
        }
    }
    packet[pos] = 0;
    pos += 1;

    for &b in b"octet" {
        packet[pos] = b;
        pos += 1;
    }
    packet[pos] = 0;
    pos += 1;

    let mut dest = *server;
    dest.sin_port = TFTP_PORT.to_be();

    unsafe {
        libc::sendto(
            sock,
            packet.as_ptr() as *const libc::c_void,
            pos,
            0,
            &dest as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );
    }

    // Wait for ACK 0
    let mut recv_buf = [0u8; 516];
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

    if n < 4 {
        io::write_str(2, b"tftp: no response from server\n");
        io::close(fd);
        return 1;
    }

    let opcode = ((recv_buf[0] as u16) << 8) | (recv_buf[1] as u16);
    if opcode != TFTP_ACK {
        io::write_str(2, b"tftp: server error\n");
        io::close(fd);
        return 1;
    }

    let mut block_num: u16 = 1;
    let mut total_bytes: u64 = 0;
    let mut data_buf = [0u8; TFTP_BLOCK_SIZE];

    loop {
        let n_read = io::read(fd, &mut data_buf);
        let data_len = if n_read > 0 { n_read as usize } else { 0 };

        packet[0] = 0;
        packet[1] = TFTP_DATA as u8;
        packet[2] = (block_num >> 8) as u8;
        packet[3] = (block_num & 0xFF) as u8;

        if data_len > 0 {
            packet[4..4 + data_len].copy_from_slice(&data_buf[..data_len]);
        }

        unsafe {
            libc::sendto(
                sock,
                packet.as_ptr() as *const libc::c_void,
                4 + data_len,
                0,
                &from as *const _ as *const libc::sockaddr,
                from_len,
            );
        }

        total_bytes += data_len as u64;

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

        if n < 4 {
            io::write_str(2, b"tftp: timeout\n");
            io::close(fd);
            return 1;
        }

        let opcode = ((recv_buf[0] as u16) << 8) | (recv_buf[1] as u16);
        let ack_block = ((recv_buf[2] as u16) << 8) | (recv_buf[3] as u16);

        if opcode != TFTP_ACK || ack_block != block_num {
            io::write_str(2, b"tftp: invalid ACK\n");
            io::close(fd);
            return 1;
        }

        if data_len < TFTP_BLOCK_SIZE {
            break;
        }

        block_num = block_num.wrapping_add(1);
    }

    io::close(fd);

    io::write_str(1, b"Sent ");
    io::write_num(1, total_bytes);
    io::write_str(1, b" bytes\n");

    0
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
    fn test_tftp_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["tftp"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_tftp_missing_mode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["tftp", "-l", "file", "-r", "file", "host"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("must specify -g"));
    }
}
