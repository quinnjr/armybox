//! httpd - simple HTTP daemon
//!
//! Minimal HTTP server.

extern crate alloc;
use alloc::vec::Vec;
use crate::io;
use crate::sys;
use super::get_arg;

/// httpd - simple HTTP daemon
///
/// # Synopsis
/// ```text
/// httpd [-f] [-p PORT] [-h HOME]
/// ```
///
/// # Description
/// Simple HTTP web server daemon. Serves static files from the specified
/// directory (default: current directory).
///
/// # Options
/// - `-f`: Don't daemonize, stay in foreground
/// - `-p PORT`: Listen on PORT (default: 80)
/// - `-h HOME`: Document root directory (default: .)
/// - `-v`: Verbose mode
///
/// # Supported Features
/// - GET requests
/// - Directory index (index.html)
/// - MIME type detection
/// - Basic error pages (404, 403)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn httpd(argc: i32, argv: *const *const u8) -> i32 {
    let mut foreground = false;
    let mut port: u16 = 80;
    let mut doc_root: &[u8] = b".";
    let mut verbose = false;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-f" {
            foreground = true;
        } else if arg == b"-p" {
            i += 1;
            if let Some(p) = unsafe { get_arg(argv, i as i32) } {
                port = sys::parse_u64(p).unwrap_or(80) as u16;
            }
        } else if arg == b"-h" {
            i += 1;
            if let Some(h) = unsafe { get_arg(argv, i as i32) } {
                doc_root = h;
            }
        } else if arg == b"-v" {
            verbose = true;
        } else if arg == b"--help" {
            print_usage();
            return 0;
        }
        i += 1;
    }

    // Change to document root
    if io::chdir(doc_root) < 0 {
        io::write_str(2, b"httpd: cannot chdir to ");
        io::write_all(2, doc_root);
        io::write_str(2, b"\n");
        return 1;
    }

    // Create listening socket
    let listen_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if listen_fd < 0 {
        io::write_str(2, b"httpd: socket failed\n");
        return 1;
    }

    // Set SO_REUSEADDR
    let opt: i32 = 1;
    unsafe {
        libc::setsockopt(listen_fd, libc::SOL_SOCKET, libc::SO_REUSEADDR,
                         &opt as *const _ as *const libc::c_void,
                         core::mem::size_of::<i32>() as libc::socklen_t);
    }

    // Bind
    let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    addr.sin_family = libc::AF_INET as u16;
    addr.sin_port = port.to_be();
    addr.sin_addr.s_addr = 0; // INADDR_ANY

    if unsafe { libc::bind(listen_fd, &addr as *const _ as *const libc::sockaddr,
                           core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t) } < 0 {
        io::write_str(2, b"httpd: bind failed (port ");
        let mut buf = [0u8; 16];
        io::write_all(2, sys::format_u64(port as u64, &mut buf));
        io::write_str(2, b")\n");
        io::close(listen_fd);
        return 1;
    }

    // Listen
    if unsafe { libc::listen(listen_fd, 10) } < 0 {
        io::write_str(2, b"httpd: listen failed\n");
        io::close(listen_fd);
        return 1;
    }

    // Daemonize unless -f
    if !foreground {
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            io::write_str(2, b"httpd: fork failed\n");
            io::close(listen_fd);
            return 1;
        }
        if pid > 0 {
            // Parent exits
            return 0;
        }

        // Create new session
        unsafe { libc::setsid() };

        // Close standard streams
        io::close(0);
        io::close(1);
        io::close(2);

        // Redirect to /dev/null
        let null_fd = io::open(b"/dev/null", libc::O_RDWR, 0);
        if null_fd >= 0 {
            io::dup2(null_fd, 0);
            io::dup2(null_fd, 1);
            io::dup2(null_fd, 2);
            if null_fd > 2 {
                io::close(null_fd);
            }
        }
    } else if verbose {
        io::write_str(1, b"httpd: listening on port ");
        let mut buf = [0u8; 16];
        io::write_all(1, sys::format_u64(port as u64, &mut buf));
        io::write_str(1, b"\n");
    }

    // Accept loop
    loop {
        let mut client_addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        let mut addr_len = core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

        let client_fd = unsafe {
            libc::accept(listen_fd, &mut client_addr as *mut _ as *mut libc::sockaddr, &mut addr_len)
        };

        if client_fd < 0 {
            let e = sys::errno();
            if e == libc::EINTR || e == libc::ECONNABORTED {
                continue;
            } else if e == libc::EMFILE || e == libc::ENFILE {
                // Fd exhaustion: back off briefly instead of busy-spinning.
                unsafe { libc::usleep(100_000) };
                continue;
            } else {
                io::write_str(2, b"httpd: accept failed\n");
                io::close(listen_fd);
                return 1;
            }
        }

        // Fork to handle request
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // Child - handle request
            io::close(listen_fd);
            handle_request(client_fd, verbose);
            io::close(client_fd);
            unsafe { libc::_exit(0) };
        } else {
            // Parent - close client fd and continue
            io::close(client_fd);

            // Reap zombies
            unsafe {
                libc::waitpid(-1, core::ptr::null_mut(), libc::WNOHANG);
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn handle_request(fd: i32, _verbose: bool) {
    let mut buf = [0u8; 4096];
    let n = io::read(fd, &mut buf);
    if n <= 0 {
        return;
    }

    // Parse request line: GET /path HTTP/1.x
    let request = &buf[..n as usize];

    // Find method
    let mut pos = 0;
    while pos < request.len() && request[pos] != b' ' {
        pos += 1;
    }
    let method = &request[..pos];

    // Skip space
    while pos < request.len() && request[pos] == b' ' {
        pos += 1;
    }

    // Find path
    let path_start = pos;
    while pos < request.len() && request[pos] != b' ' && request[pos] != b'?' {
        pos += 1;
    }
    let path = &request[path_start..pos];

    // Only support GET
    if method != b"GET" {
        send_error(fd, 405, b"Method Not Allowed");
        return;
    }

    // Decode path (basic - just handle %20 for spaces)
    let decoded_path = decode_url(path);

    // Security: reject paths with ..
    if has_dotdot(&decoded_path) {
        send_error(fd, 403, b"Forbidden");
        return;
    }

    // Strip ALL leading slashes (not just one) so requests like
    // "GET //etc/passwd" or "GET /%2Fetc/passwd" cannot decode into an
    // absolute path that escapes the document root.
    let mut stripped_start = 0;
    while stripped_start < decoded_path.len() && decoded_path[stripped_start] == b'/' {
        stripped_start += 1;
    }
    let stripped = &decoded_path[stripped_start..];

    // Reject anything that is still absolute after stripping (shouldn't
    // happen, but be defensive) and anything containing a ".." component.
    if stripped.first() == Some(&b'/') || has_dotdot(stripped) {
        send_error(fd, 403, b"Forbidden");
        return;
    }

    // Remove leading slash and handle root
    let file_path = if stripped.is_empty() {
        b"index.html".to_vec()
    } else {
        stripped.to_vec()
    };

    // Check if directory, append index.html
    let final_path = if is_directory(&file_path) {
        let mut p = file_path.clone();
        if !p.ends_with(b"/") {
            p.push(b'/');
        }
        p.extend_from_slice(b"index.html");
        p
    } else {
        file_path
    };

    // Try to open file
    let file_fd = io::open(&final_path, libc::O_RDONLY, 0);
    if file_fd < 0 {
        send_error(fd, 404, b"Not Found");
        return;
    }

    // Get file size
    let mut stat_buf = io::stat_zeroed();
    if io::fstat(file_fd, &mut stat_buf) < 0 {
        io::close(file_fd);
        send_error(fd, 500, b"Internal Server Error");
        return;
    }

    let content_length = stat_buf.st_size as u64;
    let content_type = get_mime_type(&final_path);

    // Send response headers
    io::write_all(fd, b"HTTP/1.0 200 OK\r\n");
    io::write_all(fd, b"Server: armybox httpd\r\n");
    io::write_all(fd, b"Content-Type: ");
    io::write_all(fd, content_type);
    io::write_all(fd, b"\r\n");
    io::write_all(fd, b"Content-Length: ");
    let mut len_buf = [0u8; 20];
    io::write_all(fd, sys::format_u64(content_length, &mut len_buf));
    io::write_all(fd, b"\r\n");
    io::write_all(fd, b"Connection: close\r\n");
    io::write_all(fd, b"\r\n");

    // Send file content
    let mut send_buf = [0u8; 8192];
    loop {
        let n = io::read(file_fd, &mut send_buf);
        if n <= 0 {
            break;
        }
        io::write_all(fd, &send_buf[..n as usize]);
    }

    io::close(file_fd);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn send_error(fd: i32, code: u32, message: &[u8]) {
    let mut buf = [0u8; 16];

    io::write_all(fd, b"HTTP/1.0 ");
    io::write_all(fd, sys::format_u64(code as u64, &mut buf));
    io::write_all(fd, b" ");
    io::write_all(fd, message);
    io::write_all(fd, b"\r\n");
    io::write_all(fd, b"Content-Type: text/html\r\n");
    io::write_all(fd, b"Connection: close\r\n");
    io::write_all(fd, b"\r\n");
    io::write_all(fd, b"<html><body><h1>");
    io::write_all(fd, sys::format_u64(code as u64, &mut buf));
    io::write_all(fd, b" ");
    io::write_all(fd, message);
    io::write_all(fd, b"</h1></body></html>\n");
}

fn decode_url(url: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(url.len());
    let mut i = 0;
    while i < url.len() {
        if url[i] == b'%' && i + 2 < url.len() {
            // Decode hex
            let h1 = hex_digit(url[i + 1]);
            let h2 = hex_digit(url[i + 2]);
            if let (Some(d1), Some(d2)) = (h1, h2) {
                result.push((d1 << 4) | d2);
                i += 3;
                continue;
            }
        }
        result.push(url[i]);
        i += 1;
    }
    result
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn has_dotdot(path: &[u8]) -> bool {
    let mut i = 0;
    while i < path.len() {
        if path[i] == b'.' && i + 1 < path.len() && path[i + 1] == b'.' {
            // Check if it's a path component
            let before_ok = i == 0 || path[i - 1] == b'/';
            let after_ok = i + 2 >= path.len() || path[i + 2] == b'/';
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_directory(path: &[u8]) -> bool {
    let mut stat_buf = io::stat_zeroed();
    if io::stat(path, &mut stat_buf) < 0 {
        return false;
    }
    (stat_buf.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32
}

fn get_mime_type(path: &[u8]) -> &'static [u8] {
    // Find extension
    let mut ext_start = path.len();
    for i in (0..path.len()).rev() {
        if path[i] == b'.' {
            ext_start = i + 1;
            break;
        }
        if path[i] == b'/' {
            break;
        }
    }

    if ext_start >= path.len() {
        return b"application/octet-stream";
    }

    let ext = &path[ext_start..];

    // Common MIME types
    if ext == b"html" || ext == b"htm" {
        b"text/html"
    } else if ext == b"css" {
        b"text/css"
    } else if ext == b"js" {
        b"application/javascript"
    } else if ext == b"json" {
        b"application/json"
    } else if ext == b"txt" {
        b"text/plain"
    } else if ext == b"xml" {
        b"application/xml"
    } else if ext == b"png" {
        b"image/png"
    } else if ext == b"jpg" || ext == b"jpeg" {
        b"image/jpeg"
    } else if ext == b"gif" {
        b"image/gif"
    } else if ext == b"svg" {
        b"image/svg+xml"
    } else if ext == b"ico" {
        b"image/x-icon"
    } else if ext == b"pdf" {
        b"application/pdf"
    } else if ext == b"zip" {
        b"application/zip"
    } else if ext == b"tar" {
        b"application/x-tar"
    } else if ext == b"gz" {
        b"application/gzip"
    } else if ext == b"mp3" {
        b"audio/mpeg"
    } else if ext == b"mp4" {
        b"video/mp4"
    } else if ext == b"webm" {
        b"video/webm"
    } else if ext == b"woff" {
        b"font/woff"
    } else if ext == b"woff2" {
        b"font/woff2"
    } else {
        b"application/octet-stream"
    }
}

fn print_usage() {
    io::write_str(1, b"Usage: httpd [-f] [-p PORT] [-h HOME]\n\n");
    io::write_str(1, b"Simple HTTP server.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -f        Stay in foreground\n");
    io::write_str(1, b"  -p PORT   Listen on PORT (default: 80)\n");
    io::write_str(1, b"  -h HOME   Document root (default: .)\n");
    io::write_str(1, b"  -v        Verbose mode\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn httpd(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"httpd: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::process::Command;
    use std::path::PathBuf;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_httpd_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["httpd", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_mime_types() {
        use super::get_mime_type;

        assert_eq!(get_mime_type(b"index.html"), b"text/html");
        assert_eq!(get_mime_type(b"style.css"), b"text/css");
        assert_eq!(get_mime_type(b"app.js"), b"application/javascript");
        assert_eq!(get_mime_type(b"image.png"), b"image/png");
        assert_eq!(get_mime_type(b"data.json"), b"application/json");
    }

    #[test]
    fn test_has_dotdot() {
        use super::has_dotdot;

        assert!(has_dotdot(b".."));
        assert!(has_dotdot(b"/../"));
        assert!(has_dotdot(b"/foo/../bar"));
        assert!(!has_dotdot(b"/foo/bar"));
        assert!(!has_dotdot(b"/foo..bar"));
    }

    #[test]
    fn test_decode_url() {
        use super::decode_url;

        assert_eq!(decode_url(b"hello%20world"), b"hello world");
        assert_eq!(decode_url(b"test%2Fpath"), b"test/path");
        assert_eq!(decode_url(b"normal"), b"normal");
    }
}
