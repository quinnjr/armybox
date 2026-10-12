//! telnet - user interface to the TELNET protocol
//!
//! Simple telnet client for remote terminal connections.

use super::get_arg;
use crate::io;
use crate::sys;

// Telnet protocol constants
const IAC: u8 = 255; // Interpret As Command
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250; // Subnegotiation Begin
const SE: u8 = 240; // Subnegotiation End

// Telnet options
const TELOPT_ECHO: u8 = 1;
const TELOPT_SGA: u8 = 3; // Suppress Go Ahead
const TELOPT_TTYPE: u8 = 24; // Terminal Type
const TELOPT_NAWS: u8 = 31; // Window Size

/// telnet - user interface to the TELNET protocol
///
/// # Synopsis
/// ```text
/// telnet HOST [PORT]
/// ```
///
/// # Description
/// Connect to a host using the TELNET protocol for remote terminal access.
/// Supports basic telnet option negotiation.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn telnet(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"Usage: telnet HOST [PORT]\n");
        return 1;
    }

    let host = match unsafe { get_arg(argv, 1) } {
        Some(h) => h,
        None => {
            io::write_str(2, b"telnet: missing host\n");
            return 1;
        }
    };

    let port: u16 = if argc > 2 {
        match unsafe { get_arg(argv, 2) }.and_then(|p| sys::parse_u64(p)) {
            Some(p) => p as u16,
            None => 23,
        }
    } else {
        23 // Default telnet port
    };

    // Resolve host
    let addr = match resolve_host(host, port) {
        Some(a) => a,
        None => {
            io::write_str(2, b"telnet: cannot resolve ");
            io::write_all(2, host);
            io::write_str(2, b"\n");
            return 1;
        }
    };

    // Create socket
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if sock < 0 {
        io::write_str(2, b"telnet: cannot create socket\n");
        return 1;
    }

    io::write_str(1, b"Trying ");
    print_addr(&addr);
    io::write_str(1, b"...\n");

    // Connect
    let ret = unsafe {
        libc::connect(
            sock,
            &addr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };

    if ret < 0 {
        io::write_str(
            2,
            b"telnet: Unable to connect to remote host: Connection refused\n",
        );
        unsafe { libc::close(sock) };
        return 1;
    }

    io::write_str(1, b"Connected to ");
    io::write_all(1, host);
    io::write_str(1, b".\nEscape character is '^]'.\n");

    // Set terminal to raw mode
    let mut orig_termios: libc::termios = unsafe { core::mem::zeroed() };
    unsafe {
        libc::tcgetattr(0, &mut orig_termios);
        let mut raw = orig_termios;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
        raw.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);
        raw.c_oflag &= !libc::OPOST;
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        libc::tcsetattr(0, libc::TCSAFLUSH, &raw);
    }

    // Set socket to non-blocking for select
    unsafe {
        let flags = libc::fcntl(sock, libc::F_GETFL, 0);
        libc::fcntl(sock, libc::F_SETFL, flags | libc::O_NONBLOCK);

        let flags = libc::fcntl(0, libc::F_GETFL, 0);
        libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let mut telnet_state = TelnetState::new();
    let mut running = true;

    while running {
        // Use select to wait for input from either stdin or socket
        let mut read_fds: libc::fd_set = unsafe { core::mem::zeroed() };
        unsafe {
            libc::FD_ZERO(&mut read_fds);
            libc::FD_SET(0, &mut read_fds);
            libc::FD_SET(sock, &mut read_fds);
        }

        let mut timeout = libc::timeval {
            tv_sec: 1,
            tv_usec: 0,
        };

        let nfds = sock + 1;
        let ret = unsafe {
            libc::select(
                nfds,
                &mut read_fds,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut timeout,
            )
        };

        if ret < 0 {
            break;
        }

        // Check for input from socket
        if unsafe { libc::FD_ISSET(sock, &read_fds) } {
            let mut buf = [0u8; 1024];
            let n =
                unsafe { libc::recv(sock, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };

            if n <= 0 {
                io::write_str(1, b"\nConnection closed by foreign host.\n");
                running = false;
            } else {
                // Process telnet protocol and output data
                let mut i = 0;
                while i < n as usize {
                    if buf[i] == IAC && i + 1 < n as usize {
                        // Telnet command
                        let cmd = buf[i + 1];
                        match cmd {
                            IAC => {
                                // Escaped 255
                                io::write_all(1, &[255]);
                                i += 2;
                            }
                            DO | DONT | WILL | WONT if i + 2 < n as usize => {
                                let opt = buf[i + 2];
                                handle_telnet_option(sock, cmd, opt, &mut telnet_state);
                                i += 3;
                            }
                            SB => {
                                // Subnegotiation - find SE
                                let mut j = i + 2;
                                while j + 1 < n as usize {
                                    if buf[j] == IAC && buf[j + 1] == SE {
                                        // Handle subnegotiation (skip for now)
                                        j += 2;
                                        break;
                                    }
                                    j += 1;
                                }
                                i = j;
                            }
                            _ => {
                                i += 2;
                            }
                        }
                    } else {
                        // Regular data
                        io::write_all(1, &buf[i..i + 1]);
                        i += 1;
                    }
                }
            }
        }

        // Check for input from stdin
        if unsafe { libc::FD_ISSET(0, &read_fds) } {
            let mut buf = [0u8; 256];
            let n = io::read(0, &mut buf);

            if n > 0 {
                for i in 0..n as usize {
                    if buf[i] == 0x1D {
                        // Ctrl+] - escape character
                        io::write_str(1, b"\ntelnet> ");
                        // Read command
                        let mut cmd_buf = [0u8; 64];

                        // Temporarily restore terminal
                        unsafe { libc::tcsetattr(0, libc::TCSAFLUSH, &orig_termios) };

                        let cn = io::read(0, &mut cmd_buf);

                        // Back to raw mode
                        unsafe {
                            let mut raw = orig_termios;
                            raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG);
                            raw.c_iflag &= !(libc::IXON | libc::ICRNL);
                            libc::tcsetattr(0, libc::TCSAFLUSH, &raw);
                        }

                        if cn > 0 {
                            let cmd = &cmd_buf[..cn as usize];
                            if cmd.starts_with(b"quit") || cmd.starts_with(b"q") || cmd == b"\n" {
                                running = false;
                            } else if cmd.starts_with(b"close") || cmd.starts_with(b"c") {
                                running = false;
                            }
                        }
                    } else {
                        // Send to remote
                        let byte = buf[i];
                        if byte == IAC {
                            // Escape IAC
                            let escaped = [IAC, IAC];
                            unsafe {
                                libc::send(sock, escaped.as_ptr() as *const libc::c_void, 2, 0);
                            }
                        } else {
                            unsafe {
                                libc::send(sock, &byte as *const u8 as *const libc::c_void, 1, 0);
                            }
                        }
                    }
                }
            }
        }
    }

    // Restore terminal
    unsafe {
        libc::tcsetattr(0, libc::TCSAFLUSH, &orig_termios);
        libc::close(sock);
    }

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn telnet(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"telnet: only available on Linux\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
struct TelnetState {
    echo: bool,
    sga: bool,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl TelnetState {
    fn new() -> Self {
        TelnetState {
            echo: false,
            sga: false,
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn handle_telnet_option(sock: i32, cmd: u8, opt: u8, state: &mut TelnetState) {
    let response = match cmd {
        DO => {
            // Server asks us to do something
            match opt {
                TELOPT_TTYPE | TELOPT_NAWS => {
                    // We can do these
                    [IAC, WILL, opt]
                }
                _ => {
                    // We won't do other things
                    [IAC, WONT, opt]
                }
            }
        }
        DONT => [IAC, WONT, opt],
        WILL => {
            // Server will do something
            match opt {
                TELOPT_ECHO => {
                    state.echo = true;
                    [IAC, DO, opt]
                }
                TELOPT_SGA => {
                    state.sga = true;
                    [IAC, DO, opt]
                }
                _ => [IAC, DONT, opt],
            }
        }
        WONT => {
            match opt {
                TELOPT_ECHO => state.echo = false,
                TELOPT_SGA => state.sga = false,
                _ => {}
            }
            [IAC, DONT, opt]
        }
        _ => return,
    };

    unsafe {
        libc::send(sock, response.as_ptr() as *const libc::c_void, 3, 0);
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn resolve_host(host: &[u8], port: u16) -> Option<libc::sockaddr_in> {
    // Try to parse as IP address first
    if let Some(ip) = parse_ipv4(host) {
        let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        addr.sin_family = libc::AF_INET as u16;
        addr.sin_port = port.to_be();
        addr.sin_addr.s_addr = ip.to_be();
        return Some(addr);
    }

    // Use getaddrinfo for DNS resolution
    let mut host_cstr = [0u8; 256];
    let len = core::cmp::min(host.len(), 255);
    host_cstr[..len].copy_from_slice(&host[..len]);
    host_cstr[len] = 0;

    let mut hints: libc::addrinfo = unsafe { core::mem::zeroed() };
    hints.ai_family = libc::AF_INET;
    hints.ai_socktype = libc::SOCK_STREAM;

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
            let mut sin = *(ai.ai_addr as *const libc::sockaddr_in);
            sin.sin_port = port.to_be();
            Some(sin)
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn print_addr(addr: &libc::sockaddr_in) {
    let ip = u32::from_be(addr.sin_addr.s_addr);
    let port = u16::from_be(addr.sin_port);
    let mut buf = [0u8; 16];

    io::write_all(1, sys::format_u64(((ip >> 24) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(((ip >> 16) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64(((ip >> 8) & 0xFF) as u64, &mut buf));
    io::write_str(1, b".");
    io::write_all(1, sys::format_u64((ip & 0xFF) as u64, &mut buf));
    io::write_str(1, b":");
    io::write_all(1, sys::format_u64(port as u64, &mut buf));
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_telnet_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["telnet"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_parse_ipv4() {
        assert_eq!(super::parse_ipv4(b"127.0.0.1"), Some(0x7F000001));
        assert_eq!(super::parse_ipv4(b"192.168.1.1"), Some(0xC0A80101));
        assert_eq!(super::parse_ipv4(b"invalid"), None);
    }
}
