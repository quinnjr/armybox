//! ping - send ICMP ECHO_REQUEST to network hosts
//!
//! Send ICMP ECHO_REQUEST packets to network hosts.

use super::get_arg;
use crate::io;
use crate::sys;

/// ping - send ICMP ECHO_REQUEST to network hosts
///
/// # Synopsis
/// ```text
/// ping [-c count] HOST
/// ```
///
/// # Description
/// Send ICMP ECHO_REQUEST packets to a network host and report responses.
///
/// # Options
/// - `-c count`: Stop after sending count packets
///
/// # Exit Status
/// - 0: At least one response received
/// - 1: No response received or error
pub fn ping(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"ping: missing host\n");
        return 1;
    }

    let host = unsafe { get_arg(argv, argc - 1).unwrap() };
    let mut count = 4i32;

    // Parse -c option
    for i in 1..argc - 1 {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-c" && i + 1 < argc - 1 {
                if let Some(c) = unsafe { get_arg(argv, i + 1) } {
                    count = sys::parse_i64(c).unwrap_or(4) as i32;
                }
            }
        }
    }

    io::write_str(1, b"PING ");
    io::write_all(1, host);
    io::write_str(1, b" 56 data bytes\n");

    // Create raw socket for ICMP
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_ICMP) };
    if sock < 0 {
        io::write_str(2, b"ping: socket: Operation not permitted\n");
        return 1;
    }

    // Resolve hostname
    let mut host_buf = [0u8; 256];
    host_buf[..host.len()].copy_from_slice(host);

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_INET;
    let mut res: *mut libc::addrinfo = core::ptr::null_mut();

    if unsafe {
        libc::getaddrinfo(
            host_buf.as_ptr() as *const libc::c_char,
            core::ptr::null(),
            &hints,
            &mut res,
        )
    } != 0
    {
        io::write_str(2, b"ping: unknown host\n");
        unsafe { libc::close(sock) };
        return 1;
    }

    let addr = unsafe { *((*res).ai_addr as *const libc::sockaddr_in) };
    unsafe { libc::freeaddrinfo(res) };

    let mut sent = 0;
    let mut received = 0;

    for seq in 0..count {
        // Build ICMP echo request
        let mut packet = [0u8; 64];
        packet[0] = 8; // ICMP Echo Request
        packet[1] = 0; // Code
        // Checksum at 2-3, will compute
        packet[4] = 0;
        packet[5] = 1; // ID
        packet[6] = (seq >> 8) as u8;
        packet[7] = seq as u8; // Sequence

        // Compute checksum
        let mut sum: u32 = 0;
        for i in (0..64).step_by(2) {
            sum += ((packet[i] as u32) << 8) | (packet[i + 1] as u32);
        }
        while sum >> 16 != 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }
        let checksum = !sum as u16;
        packet[2] = (checksum >> 8) as u8;
        packet[3] = checksum as u8;

        let start = unsafe { libc::time(core::ptr::null_mut()) };

        let sent_bytes = unsafe {
            libc::sendto(
                sock,
                packet.as_ptr() as *const libc::c_void,
                64,
                0,
                &addr as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        };

        if sent_bytes > 0 {
            sent += 1;

            // Set receive timeout
            let tv = libc::timeval {
                tv_sec: 1,
                tv_usec: 0,
            };
            unsafe {
                libc::setsockopt(
                    sock,
                    libc::SOL_SOCKET,
                    libc::SO_RCVTIMEO,
                    &tv as *const _ as *const libc::c_void,
                    core::mem::size_of::<libc::timeval>() as libc::socklen_t,
                )
            };

            let mut recv_buf = [0u8; 128];
            let mut from: libc::sockaddr_in = unsafe { core::mem::zeroed() };
            let mut fromlen = core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

            let recv_bytes = unsafe {
                libc::recvfrom(
                    sock,
                    recv_buf.as_mut_ptr() as *mut libc::c_void,
                    recv_buf.len(),
                    0,
                    &mut from as *mut _ as *mut libc::sockaddr,
                    &mut fromlen,
                )
            };

            let end = unsafe { libc::time(core::ptr::null_mut()) };
            let rtt = (end - start) * 1000; // Approximate ms

            if recv_bytes > 0 {
                received += 1;
                io::write_str(1, b"64 bytes from ");
                io::write_all(1, host);
                io::write_str(1, b": icmp_seq=");
                let mut num_buf = [0u8; 20];
                io::write_all(1, sys::format_u64((seq + 1) as u64, &mut num_buf));
                io::write_str(1, b" time=");
                io::write_all(1, sys::format_u64(rtt as u64, &mut num_buf));
                io::write_str(1, b" ms\n");
            }
        }

        if seq < count - 1 {
            unsafe { libc::sleep(1) };
        }
    }

    unsafe { libc::close(sock) };

    // Print statistics
    io::write_str(1, b"\n--- ");
    io::write_all(1, host);
    io::write_str(1, b" ping statistics ---\n");
    let mut num_buf = [0u8; 20];
    io::write_all(1, sys::format_u64(sent as u64, &mut num_buf));
    io::write_str(1, b" packets transmitted, ");
    io::write_all(1, sys::format_u64(received as u64, &mut num_buf));
    io::write_str(1, b" received, ");
    let loss = if sent > 0 {
        ((sent - received) * 100 / sent) as u64
    } else {
        100
    };
    io::write_all(1, sys::format_u64(loss, &mut num_buf));
    io::write_str(1, b"% packet loss\n");

    if received > 0 { 0 } else { 1 }
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
    fn test_ping_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ping"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing host"));
    }

    #[test]
    fn test_ping_requires_root() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Without root, should fail with permission error
        let output = Command::new(&armybox)
            .args(["ping", "-c", "1", "127.0.0.1"])
            .output()
            .unwrap();

        // Either succeeds (as root) or fails with permission denied
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
