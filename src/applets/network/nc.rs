//! nc - TCP/IP swiss army knife
//!
//! Netcat - arbitrary TCP and UDP connections and listens.

use super::get_arg;
use crate::io;

/// nc - TCP/IP swiss army knife
///
/// # Synopsis
/// ```text
/// nc HOST PORT
/// ```
///
/// # Description
/// Connect to a TCP host and port, relaying stdin to the socket
/// and socket data to stdout.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn nc(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"nc: usage: nc HOST PORT\n");
        return 1;
    }

    let host = unsafe { get_arg(argv, 1).unwrap() };
    let port = unsafe { get_arg(argv, 2).unwrap() };

    let mut host_buf = [0u8; 256];
    let mut port_buf = [0u8; 16];
    host_buf[..host.len()].copy_from_slice(host);
    port_buf[..port.len()].copy_from_slice(port);

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_INET;
    hints.ai_socktype = libc::SOCK_STREAM;
    let mut res: *mut libc::addrinfo = core::ptr::null_mut();

    if unsafe {
        libc::getaddrinfo(
            host_buf.as_ptr() as *const libc::c_char,
            port_buf.as_ptr() as *const libc::c_char,
            &hints,
            &mut res,
        )
    } != 0
    {
        io::write_str(2, b"nc: cannot resolve host\n");
        return 1;
    }

    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        unsafe { libc::freeaddrinfo(res) };
        io::write_str(2, b"nc: socket failed\n");
        return 1;
    }

    let info = unsafe { &*res };
    if unsafe { libc::connect(sock, info.ai_addr, info.ai_addrlen) } < 0 {
        unsafe {
            libc::close(sock);
            libc::freeaddrinfo(res)
        };
        io::write_str(2, b"nc: connection failed\n");
        return 1;
    }

    unsafe { libc::freeaddrinfo(res) };

    // Simple relay: stdin -> socket, socket -> stdout
    let mut buf = [0u8; 4096];

    // Set socket non-blocking
    unsafe { libc::fcntl(sock, libc::F_SETFL, libc::O_NONBLOCK) };
    unsafe { libc::fcntl(0, libc::F_SETFL, libc::O_NONBLOCK) };

    loop {
        // Read from stdin, write to socket
        let n = io::read(0, &mut buf);
        if n > 0 {
            let _ = unsafe { libc::send(sock, buf.as_ptr() as *const libc::c_void, n as usize, 0) };
        }

        // Read from socket, write to stdout
        let n = unsafe { libc::recv(sock, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if n > 0 {
            io::write_all(1, &buf[..n as usize]);
        } else if n == 0 {
            break; // Connection closed
        }

        unsafe { libc::usleep(10000) }; // 10ms
    }

    unsafe { libc::close(sock) };
    0
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
    fn test_nc_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nc"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_nc_connection_refused() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Try to connect to a port that's likely not listening
        let output = Command::new(&armybox)
            .args(["nc", "127.0.0.1", "65534"])
            .output()
            .unwrap();

        // Should fail with connection refused
        assert_eq!(output.status.code(), Some(1));
    }
}
