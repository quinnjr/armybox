//! host - DNS lookup utility
//!
//! Perform DNS lookups.

use super::get_arg;
use crate::io;
use crate::sys;

/// host - DNS lookup utility
///
/// # Synopsis
/// ```text
/// host HOSTNAME
/// ```
///
/// # Description
/// Look up the IP address of a hostname using DNS.
///
/// # Exit Status
/// - 0: Success
/// - 1: Host not found or error
pub fn host(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"host: missing hostname\n");
        return 1;
    }

    let hostname = unsafe { get_arg(argv, 1).unwrap() };
    let mut host_buf = [0u8; 256];
    host_buf[..hostname.len()].copy_from_slice(hostname);

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_UNSPEC;
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
        io::write_str(2, b"host: ");
        io::write_all(2, hostname);
        io::write_str(2, b" not found\n");
        return 1;
    }

    io::write_all(1, hostname);
    io::write_str(1, b" has address ");

    let mut current = res;
    while !current.is_null() {
        let info = unsafe { &*current };
        if info.ai_family == libc::AF_INET {
            let addr = unsafe { &*(info.ai_addr as *const libc::sockaddr_in) };
            let ip = addr.sin_addr.s_addr.to_be();
            let mut num_buf = [0u8; 20];
            io::write_all(1, sys::format_u64(((ip >> 24) & 0xff) as u64, &mut num_buf));
            io::write_str(1, b".");
            io::write_all(1, sys::format_u64(((ip >> 16) & 0xff) as u64, &mut num_buf));
            io::write_str(1, b".");
            io::write_all(1, sys::format_u64(((ip >> 8) & 0xff) as u64, &mut num_buf));
            io::write_str(1, b".");
            io::write_all(1, sys::format_u64((ip & 0xff) as u64, &mut num_buf));
            io::write_str(1, b"\n");
            break;
        }
        current = info.ai_next;
    }

    unsafe { libc::freeaddrinfo(res) };
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_host_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["host"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing"));
    }

    #[test]
    fn test_host_localhost() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["host", "localhost"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("127.0.0.1") || stdout.contains("address"));
    }
}
