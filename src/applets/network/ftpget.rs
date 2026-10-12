//! ftpget - retrieve files via FTP
//!
//! FTP file download utility.

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;

/// ftpget - retrieve files via FTP
///
/// # Synopsis
/// ```text
/// ftpget [OPTIONS] HOST REMOTE [LOCAL]
/// ```
///
/// # Description
/// Download files from FTP servers using passive mode.
///
/// # Options
/// - `-u USER`: FTP username (default: anonymous)
/// - `-p PASS`: FTP password (default: ftp@)
/// - `-P PORT`: FTP control port (default: 21)
/// - `-v`: Verbose output
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ftpget(argc: i32, argv: *const *const u8) -> i32 {
    let mut user: &[u8] = b"anonymous";
    let mut pass: &[u8] = b"ftp@";
    let mut port: u16 = 21;
    let mut verbose = false;
    let mut host: Option<&[u8]> = None;
    let mut remote: Option<&[u8]> = None;
    let mut local: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-u" {
            i += 1;
            if let Some(u) = unsafe { get_arg(argv, i as i32) } {
                user = u;
            }
        } else if arg == b"-p" {
            i += 1;
            if let Some(p) = unsafe { get_arg(argv, i as i32) } {
                pass = p;
            }
        } else if arg == b"-P" {
            i += 1;
            if let Some(p) = unsafe { get_arg(argv, i as i32) } {
                port = sys::parse_u64(p).unwrap_or(21) as u16;
            }
        } else if arg == b"-v" {
            verbose = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            if host.is_none() {
                host = Some(arg);
            } else if remote.is_none() {
                remote = Some(arg);
            } else if local.is_none() {
                local = Some(arg);
            }
        }
        i += 1;
    }

    let host = match host {
        Some(h) => h,
        None => {
            io::write_str(2, b"ftpget: missing host\n");
            print_usage();
            return 1;
        }
    };

    let remote = match remote {
        Some(r) => r,
        None => {
            io::write_str(2, b"ftpget: missing remote file\n");
            return 1;
        }
    };

    // If no local name, use the basename of remote
    let local_name = match local {
        Some(l) => l.to_vec(),
        None => {
            // Get basename
            let mut start = 0;
            for (i, &c) in remote.iter().enumerate() {
                if c == b'/' {
                    start = i + 1;
                }
            }
            remote[start..].to_vec()
        }
    };

    // Resolve hostname
    let addr = match resolve_host(host, port) {
        Some(a) => a,
        None => {
            io::write_str(2, b"ftpget: cannot resolve ");
            io::write_all(2, host);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Connect control socket
    let ctrl_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if ctrl_fd < 0 {
        io::write_str(2, b"ftpget: socket failed\n");
        return 1;
    }

    if unsafe {
        libc::connect(
            ctrl_fd,
            &addr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        io::write_str(2, b"ftpget: connect failed\n");
        io::close(ctrl_fd);
        return 1;
    }

    // FTP session
    let result = ftp_download(ctrl_fd, user, pass, remote, &local_name, verbose);

    io::close(ctrl_fd);
    result
}

fn resolve_host(host: &[u8], port: u16) -> Option<libc::sockaddr_in> {
    // Try parsing as IP first
    if let Some(ip) = parse_ipv4(host) {
        let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        addr.sin_family = libc::AF_INET as u16;
        addr.sin_port = port.to_be();
        addr.sin_addr.s_addr = ip;
        return Some(addr);
    }

    // DNS lookup using getaddrinfo
    let mut host_cstr = [0u8; 256];
    let len = host.len().min(255);
    host_cstr[..len].copy_from_slice(&host[..len]);
    host_cstr[len] = 0;

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_INET;
    hints.ai_socktype = libc::SOCK_STREAM;

    let mut res: *mut libc::addrinfo = core::ptr::null_mut();
    if unsafe {
        libc::getaddrinfo(
            host_cstr.as_ptr() as *const libc::c_char,
            core::ptr::null(),
            &hints,
            &mut res,
        )
    } != 0
    {
        return None;
    }

    if res.is_null() {
        return None;
    }

    let ai = unsafe { &*res };
    if ai.ai_addr.is_null()
        || ai.ai_addrlen < core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
    {
        unsafe { libc::freeaddrinfo(res) };
        return None;
    }

    let mut addr = unsafe { *(ai.ai_addr as *const libc::sockaddr_in) };
    addr.sin_port = port.to_be();

    unsafe { libc::freeaddrinfo(res) };
    Some(addr)
}

fn parse_ipv4(s: &[u8]) -> Option<u32> {
    let mut octets = [0u8; 4];
    let mut octet_idx = 0;
    let mut current = 0u32;
    let mut has_digits = false;

    for &c in s {
        if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u32;
            if current > 255 {
                return None;
            }
            has_digits = true;
        } else if c == b'.' {
            if !has_digits || octet_idx >= 3 {
                return None;
            }
            octets[octet_idx] = current as u8;
            octet_idx += 1;
            current = 0;
            has_digits = false;
        } else {
            return None;
        }
    }

    if !has_digits || octet_idx != 3 {
        return None;
    }
    octets[3] = current as u8;

    Some(u32::from_ne_bytes(octets))
}

fn ftp_download(
    ctrl_fd: i32,
    user: &[u8],
    pass: &[u8],
    remote: &[u8],
    local: &[u8],
    verbose: bool,
) -> i32 {
    // Read initial banner
    let mut buf = [0u8; 1024];
    let code = read_response(ctrl_fd, &mut buf, verbose);
    if code != 220 {
        io::write_str(2, b"ftpget: bad server response\n");
        return 1;
    }

    // USER
    send_command(ctrl_fd, b"USER ", user);
    let code = read_response(ctrl_fd, &mut buf, verbose);
    if code != 331 && code != 230 {
        io::write_str(2, b"ftpget: login failed\n");
        return 1;
    }

    // PASS (if needed)
    if code == 331 {
        send_command(ctrl_fd, b"PASS ", pass);
        let code = read_response(ctrl_fd, &mut buf, verbose);
        if code != 230 {
            io::write_str(2, b"ftpget: authentication failed\n");
            return 1;
        }
    }

    // TYPE I (binary mode)
    send_command(ctrl_fd, b"TYPE I", b"");
    let code = read_response(ctrl_fd, &mut buf, verbose);
    if code != 200 {
        io::write_str(2, b"ftpget: cannot set binary mode\n");
        return 1;
    }

    // PASV (passive mode)
    send_command(ctrl_fd, b"PASV", b"");
    let n = read_response_line(ctrl_fd, &mut buf);
    if n <= 0 {
        io::write_str(2, b"ftpget: PASV failed\n");
        return 1;
    }

    if verbose {
        io::write_str(1, b"< ");
        io::write_all(1, &buf[..n as usize]);
    }

    // Parse PASV response: 227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)
    let pasv_addr = match parse_pasv_response(&buf[..n as usize]) {
        Some(a) => a,
        None => {
            io::write_str(2, b"ftpget: cannot parse PASV response\n");
            return 1;
        }
    };

    // Connect data socket
    let data_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if data_fd < 0 {
        io::write_str(2, b"ftpget: data socket failed\n");
        return 1;
    }

    if unsafe {
        libc::connect(
            data_fd,
            &pasv_addr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        io::write_str(2, b"ftpget: data connect failed\n");
        io::close(data_fd);
        return 1;
    }

    // RETR
    send_command(ctrl_fd, b"RETR ", remote);
    let code = read_response(ctrl_fd, &mut buf, verbose);
    if code != 150 && code != 125 {
        io::write_str(2, b"ftpget: RETR failed\n");
        io::close(data_fd);
        return 1;
    }

    // Open local file
    let out_fd = io::open(local, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if out_fd < 0 {
        io::write_str(2, b"ftpget: cannot create ");
        io::write_all(2, local);
        io::write_str(2, b"\n");
        io::close(data_fd);
        return 1;
    }

    // Read data
    let mut total = 0usize;
    let mut data_buf = [0u8; 8192];
    loop {
        let n = io::read(data_fd, &mut data_buf);
        if n <= 0 {
            break;
        }
        io::write_all(out_fd, &data_buf[..n as usize]);
        total += n as usize;
    }

    io::close(data_fd);
    io::close(out_fd);

    // Read completion response
    let code = read_response(ctrl_fd, &mut buf, verbose);
    if code != 226 {
        io::write_str(2, b"ftpget: transfer incomplete\n");
        return 1;
    }

    // QUIT
    send_command(ctrl_fd, b"QUIT", b"");
    let _ = read_response(ctrl_fd, &mut buf, verbose);

    if verbose {
        io::write_str(1, b"Downloaded ");
        let mut num_buf = [0u8; 20];
        io::write_all(1, sys::format_u64(total as u64, &mut num_buf));
        io::write_str(1, b" bytes\n");
    }

    0
}

fn send_command(fd: i32, cmd: &[u8], arg: &[u8]) {
    io::write_all(fd, cmd);
    if !arg.is_empty() {
        io::write_all(fd, arg);
    }
    io::write_all(fd, b"\r\n");
}

fn read_response_line(fd: i32, buf: &mut [u8]) -> isize {
    let mut pos = 0usize;
    while pos < buf.len() - 1 {
        let n = io::read(fd, &mut buf[pos..pos + 1]);
        if n <= 0 {
            break;
        }
        if buf[pos] == b'\n' {
            pos += 1;
            break;
        }
        pos += 1;
    }
    pos as isize
}

fn read_response(fd: i32, buf: &mut [u8], verbose: bool) -> u32 {
    // Read response lines until we get a final response (no -)
    let mut code = 0u32;
    loop {
        let n = read_response_line(fd, buf);
        if n <= 0 {
            return 0;
        }

        if verbose {
            io::write_str(1, b"< ");
            io::write_all(1, &buf[..n as usize]);
        }

        // Parse code
        if n >= 3 {
            let c =
                (buf[0] - b'0') as u32 * 100 + (buf[1] - b'0') as u32 * 10 + (buf[2] - b'0') as u32;
            code = c;

            // Check if continuation line (code followed by -)
            if n >= 4 && buf[3] != b'-' {
                break;
            }
        }
    }
    code
}

fn parse_pasv_response(resp: &[u8]) -> Option<libc::sockaddr_in> {
    // Find opening paren
    let start = resp.iter().position(|&c| c == b'(')?;
    let end = resp.iter().position(|&c| c == b')')?;

    if start >= end {
        return None;
    }

    let nums = &resp[start + 1..end];
    let mut values = [0u32; 6];
    let mut idx = 0;
    let mut current = 0u32;

    for &c in nums {
        if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u32;
        } else if c == b',' {
            if idx >= 6 {
                return None;
            }
            values[idx] = current;
            idx += 1;
            current = 0;
        }
    }
    if idx < 5 {
        return None;
    }
    values[5] = current;

    let ip = ((values[0] as u32) << 0)
        | ((values[1] as u32) << 8)
        | ((values[2] as u32) << 16)
        | ((values[3] as u32) << 24);
    let port = (values[4] as u16) << 8 | (values[5] as u16);

    let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    addr.sin_family = libc::AF_INET as u16;
    addr.sin_port = port.to_be();
    addr.sin_addr.s_addr = ip;

    Some(addr)
}

fn print_usage() {
    io::write_str(1, b"Usage: ftpget [OPTIONS] HOST REMOTE [LOCAL]\n\n");
    io::write_str(1, b"Download file from FTP server.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -u USER   Username (default: anonymous)\n");
    io::write_str(1, b"  -p PASS   Password (default: ftp@)\n");
    io::write_str(1, b"  -P PORT   Control port (default: 21)\n");
    io::write_str(1, b"  -v        Verbose output\n");
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ftpget_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ftpget", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_ftpget_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ftpget"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing host"));
    }

    #[test]
    fn test_parse_pasv() {
        use super::parse_pasv_response;

        let resp = b"227 Entering Passive Mode (192,168,1,1,200,100)";
        let addr = parse_pasv_response(resp).unwrap();

        // Port should be 200*256 + 100 = 51300
        let port = u16::from_be(addr.sin_port);
        assert_eq!(port, 51300);
    }
}
