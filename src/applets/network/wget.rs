//! wget - retrieve files via HTTP
//!
//! Simple HTTP file downloader.

use super::get_arg;
use crate::io;

/// wget - retrieve files via HTTP
///
/// # Synopsis
/// ```text
/// wget URL
/// ```
///
/// # Description
/// Download files from HTTP servers.
/// Only http:// URLs are supported (no https).
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn wget(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"wget: missing URL\n");
        return 1;
    }

    let url = unsafe { get_arg(argv, argc - 1).unwrap() };

    // Parse URL (simplified - only http)
    if !url.starts_with(b"http://") {
        io::write_str(2, b"wget: only http:// URLs supported\n");
        return 1;
    }

    let url_rest = &url[7..]; // Skip "http://"

    // Find host and path
    let (host_port, path) = if let Some(pos) = url_rest.iter().position(|&c| c == b'/') {
        (&url_rest[..pos], &url_rest[pos..])
    } else {
        (url_rest, b"/".as_slice())
    };

    // Parse host:port
    let (host, port) = if let Some(pos) = host_port.iter().position(|&c| c == b':') {
        (&host_port[..pos], &host_port[pos + 1..])
    } else {
        (host_port, b"80".as_slice())
    };

    // Connect
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
        io::write_str(2, b"wget: cannot resolve host\n");
        return 1;
    }

    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        unsafe { libc::freeaddrinfo(res) };
        return 1;
    }

    let info = unsafe { &*res };
    if unsafe { libc::connect(sock, info.ai_addr, info.ai_addrlen) } < 0 {
        unsafe {
            libc::close(sock);
            libc::freeaddrinfo(res)
        };
        io::write_str(2, b"wget: connection failed\n");
        return 1;
    }

    unsafe { libc::freeaddrinfo(res) };

    // Send HTTP request
    let mut request = [0u8; 1024];
    let mut ri = 0;
    for &c in b"GET " {
        request[ri] = c;
        ri += 1;
    }
    for &c in path {
        request[ri] = c;
        ri += 1;
    }
    for &c in b" HTTP/1.0\r\nHost: " {
        request[ri] = c;
        ri += 1;
    }
    for &c in host {
        request[ri] = c;
        ri += 1;
    }
    for &c in b"\r\nConnection: close\r\n\r\n" {
        request[ri] = c;
        ri += 1;
    }

    let _ = unsafe { libc::send(sock, request.as_ptr() as *const libc::c_void, ri, 0) };

    // Determine output filename
    let filename = if let Some(pos) = path.iter().rposition(|&c| c == b'/') {
        if pos + 1 < path.len() {
            &path[pos + 1..]
        } else {
            b"index.html"
        }
    } else {
        b"index.html"
    };

    let out_fd = io::open(
        filename,
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        0o644,
    );
    if out_fd < 0 {
        unsafe { libc::close(sock) };
        io::write_str(2, b"wget: cannot create output file\n");
        return 1;
    }

    // Receive response
    let mut buf = [0u8; 4096];
    let mut header_done = false;
    let mut body_start = 0usize;

    loop {
        let n = unsafe { libc::recv(sock, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if n <= 0 {
            break;
        }

        let data = &buf[..n as usize];

        if !header_done {
            // Find end of headers
            for i in 0..data.len().saturating_sub(3) {
                if data[i..].starts_with(b"\r\n\r\n") {
                    header_done = true;
                    body_start = i + 4;
                    break;
                }
            }
            if header_done && body_start < data.len() {
                io::write_all(out_fd, &data[body_start..]);
            }
        } else {
            io::write_all(out_fd, data);
        }
    }

    io::close(out_fd);
    unsafe { libc::close(sock) };

    io::write_str(2, b"'");
    io::write_all(2, filename);
    io::write_str(2, b"' saved\n");
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
    fn test_wget_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["wget"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing URL"));
    }

    #[test]
    fn test_wget_https_not_supported() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["wget", "https://example.com"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("only http://"));
    }

    #[test]
    fn test_wget_invalid_url() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["wget", "ftp://example.com"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
