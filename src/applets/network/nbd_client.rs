//! nbd-client - network block device client
//!
//! Connect to NBD servers.

// Allow dead code for NBD protocol constants - defined for completeness
#![allow(dead_code)]

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;

// NBD protocol constants
const NBD_MAGIC: u64 = 0x4e42444d41474943; // "NBDMAGIC"
const NBD_OPTS_MAGIC: u64 = 0x49484156454F5054; // "IHAVEOPT"
const NBD_REPLY_MAGIC: u32 = 0x67446698;
const NBD_REQUEST_MAGIC: u32 = 0x25609513;

// NBD commands
const NBD_CMD_READ: u32 = 0;
const NBD_CMD_WRITE: u32 = 1;
const NBD_CMD_DISC: u32 = 2;
const NBD_CMD_FLUSH: u32 = 3;
const NBD_CMD_TRIM: u32 = 4;

// NBD options (newstyle)
const NBD_OPT_EXPORT_NAME: u32 = 1;
const NBD_OPT_ABORT: u32 = 2;
const NBD_OPT_LIST: u32 = 3;
const NBD_OPT_GO: u32 = 7;

// NBD option replies
const NBD_REP_ACK: u32 = 1;
const NBD_REP_SERVER: u32 = 2;
const NBD_REP_INFO: u32 = 3;
const NBD_REP_FLAG_ERROR: u32 = 1 << 31;
const NBD_REP_ERR_UNSUP: u32 = NBD_REP_FLAG_ERROR | 1;

// NBD ioctls
const NBD_SET_SOCK: crate::io::IoctlReq = 0xab00u32 as crate::io::IoctlReq;
const NBD_SET_BLKSIZE: crate::io::IoctlReq = 0xab01u32 as crate::io::IoctlReq;
const NBD_SET_SIZE: crate::io::IoctlReq = 0xab02u32 as crate::io::IoctlReq;
const NBD_DO_IT: crate::io::IoctlReq = 0xab03u32 as crate::io::IoctlReq;
const NBD_CLEAR_SOCK: crate::io::IoctlReq = 0xab04u32 as crate::io::IoctlReq;
const NBD_CLEAR_QUE: crate::io::IoctlReq = 0xab05u32 as crate::io::IoctlReq;
const NBD_PRINT_DEBUG: crate::io::IoctlReq = 0xab06u32 as crate::io::IoctlReq;
const NBD_SET_SIZE_BLOCKS: crate::io::IoctlReq = 0xab07u32 as crate::io::IoctlReq;
const NBD_DISCONNECT: crate::io::IoctlReq = 0xab08u32 as crate::io::IoctlReq;
const NBD_SET_TIMEOUT: crate::io::IoctlReq = 0xab09u32 as crate::io::IoctlReq;
const NBD_SET_FLAGS: crate::io::IoctlReq = 0xab0au32 as crate::io::IoctlReq;

// NBD flags
const NBD_FLAG_HAS_FLAGS: u16 = 1 << 0;
const NBD_FLAG_READ_ONLY: u16 = 1 << 1;
const NBD_FLAG_SEND_FLUSH: u16 = 1 << 2;
const NBD_FLAG_SEND_FUA: u16 = 1 << 3;
const NBD_FLAG_ROTATIONAL: u16 = 1 << 4;
const NBD_FLAG_SEND_TRIM: u16 = 1 << 5;

// Handshake flags
const NBD_FLAG_FIXED_NEWSTYLE: u16 = 1 << 0;
const NBD_FLAG_NO_ZEROES: u16 = 1 << 1;

// Client flags
const NBD_FLAG_C_FIXED_NEWSTYLE: u32 = 1 << 0;
const NBD_FLAG_C_NO_ZEROES: u32 = 1 << 1;

/// nbd-client - network block device client
///
/// # Synopsis
/// ```text
/// nbd-client HOST PORT DEVICE [-b BLKSIZE] [-t TIMEOUT] [-n NAME]
/// nbd-client -d DEVICE
/// ```
///
/// # Description
/// Connect a local block device to a network block device server.
///
/// # Options
/// - `-b BLKSIZE`: Block size (default: 512)
/// - `-t TIMEOUT`: Timeout in seconds
/// - `-n NAME`: Export name (for newstyle servers)
/// - `-d DEVICE`: Disconnect device
/// - `-p`: Persist connection (don't daemonize)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn nbd_client(argc: i32, argv: *const *const u8) -> i32 {
    let mut host: Option<&[u8]> = None;
    let mut port: u16 = 10809;
    let mut device: Option<&[u8]> = None;
    let mut block_size: u32 = 512;
    let mut timeout: u32 = 0;
    let mut export_name: &[u8] = b"";
    let mut disconnect = false;
    let mut persist = false;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-b" {
            i += 1;
            if let Some(b) = unsafe { get_arg(argv, i as i32) } {
                block_size = sys::parse_u64(b).unwrap_or(512) as u32;
            }
        } else if arg == b"-t" {
            i += 1;
            if let Some(t) = unsafe { get_arg(argv, i as i32) } {
                timeout = sys::parse_u64(t).unwrap_or(0) as u32;
            }
        } else if arg == b"-n" || arg == b"-N" {
            i += 1;
            if let Some(n) = unsafe { get_arg(argv, i as i32) } {
                export_name = n;
            }
        } else if arg == b"-d" {
            disconnect = true;
        } else if arg == b"-p" {
            persist = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            if host.is_none() {
                host = Some(arg);
            } else if device.is_none() {
                // Check if this looks like a port number or device
                if arg.iter().all(|&c| c >= b'0' && c <= b'9') {
                    port = sys::parse_u64(arg).unwrap_or(10809) as u16;
                } else {
                    device = Some(arg);
                }
            } else if device.is_none() {
                device = Some(arg);
            }
        }
        i += 1;
    }

    // Handle disconnect
    if disconnect {
        let dev = match device.or(host) {
            Some(d) => d,
            None => {
                io::write_str(2, b"nbd-client: -d requires device\n");
                return 1;
            }
        };
        return disconnect_device(dev);
    }

    // Validate arguments
    let host = match host {
        Some(h) => h,
        None => {
            io::write_str(2, b"nbd-client: missing host\n");
            print_usage();
            return 1;
        }
    };

    let device = match device {
        Some(d) => d,
        None => {
            io::write_str(2, b"nbd-client: missing device\n");
            return 1;
        }
    };

    // Connect to server
    let sock_fd = connect_to_server(host, port);
    if sock_fd < 0 {
        io::write_str(2, b"nbd-client: cannot connect to ");
        io::write_all(2, host);
        io::write_str(2, b"\n");
        return 1;
    }

    // Perform NBD handshake
    let (size, flags) = match nbd_handshake(sock_fd, export_name) {
        Some(r) => r,
        None => {
            io::write_str(2, b"nbd-client: handshake failed\n");
            io::close(sock_fd);
            return 1;
        }
    };

    // Open NBD device
    let nbd_fd = io::open(device, libc::O_RDWR, 0);
    if nbd_fd < 0 {
        io::write_str(2, b"nbd-client: cannot open ");
        io::write_all(2, device);
        io::write_str(2, b"\n");
        io::close(sock_fd);
        return 1;
    }

    // Configure NBD device
    unsafe {
        if libc::ioctl(nbd_fd, NBD_SET_BLKSIZE, block_size as crate::io::IoctlReq) < 0 {
            io::write_str(2, b"nbd-client: NBD_SET_BLKSIZE failed\n");
            io::close(nbd_fd);
            io::close(sock_fd);
            return 1;
        }

        let size_blocks = size / block_size as u64;
        if libc::ioctl(
            nbd_fd,
            NBD_SET_SIZE_BLOCKS,
            size_blocks as crate::io::IoctlReq,
        ) < 0
        {
            io::write_str(2, b"nbd-client: NBD_SET_SIZE_BLOCKS failed\n");
            io::close(nbd_fd);
            io::close(sock_fd);
            return 1;
        }

        if libc::ioctl(nbd_fd, NBD_CLEAR_SOCK) < 0 {
            io::write_str(2, b"nbd-client: NBD_CLEAR_SOCK failed\n");
            io::close(nbd_fd);
            io::close(sock_fd);
            return 1;
        }

        if timeout > 0 {
            libc::ioctl(nbd_fd, NBD_SET_TIMEOUT, timeout as crate::io::IoctlReq);
        }

        if libc::ioctl(nbd_fd, NBD_SET_FLAGS, flags as crate::io::IoctlReq) < 0 {
            // Non-fatal, older kernels may not support this
        }

        if libc::ioctl(nbd_fd, NBD_SET_SOCK, sock_fd) < 0 {
            io::write_str(2, b"nbd-client: NBD_SET_SOCK failed\n");
            io::close(nbd_fd);
            io::close(sock_fd);
            return 1;
        }
    }

    // Fork to run NBD_DO_IT in background
    if !persist {
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            io::write_str(2, b"nbd-client: fork failed\n");
            io::close(nbd_fd);
            io::close(sock_fd);
            return 1;
        }

        if pid > 0 {
            // Parent - return success
            io::close(nbd_fd);
            io::close(sock_fd);
            return 0;
        }

        // Child - daemonize
        unsafe {
            libc::setsid();
            libc::chdir(b"/\0".as_ptr() as *const libc::c_char);
        }
    }

    // Run NBD_DO_IT (blocks until disconnected)
    unsafe {
        libc::ioctl(nbd_fd, NBD_DO_IT);
        libc::ioctl(nbd_fd, NBD_CLEAR_QUE);
        libc::ioctl(nbd_fd, NBD_CLEAR_SOCK);
    }

    io::close(nbd_fd);
    io::close(sock_fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn connect_to_server(host: &[u8], port: u16) -> i32 {
    // Try parsing as IP first
    if let Some(ip) = parse_ipv4(host) {
        let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        addr.sin_family = libc::AF_INET as u16;
        addr.sin_port = port.to_be();
        addr.sin_addr.s_addr = ip;

        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        if fd < 0 {
            return -1;
        }

        if unsafe {
            libc::connect(
                fd,
                &addr as *const _ as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        } < 0
        {
            io::close(fd);
            return -1;
        }
        return fd;
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
        return -1;
    }

    if res.is_null() {
        return -1;
    }

    let ai = unsafe { &*res };
    if ai.ai_addr.is_null()
        || ai.ai_addrlen < core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t
    {
        unsafe { libc::freeaddrinfo(res) };
        return -1;
    }

    let mut addr = unsafe { *(ai.ai_addr as *const libc::sockaddr_in) };
    addr.sin_port = port.to_be();

    unsafe { libc::freeaddrinfo(res) };

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return -1;
    }

    if unsafe {
        libc::connect(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        io::close(fd);
        return -1;
    }

    fd
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn nbd_handshake(fd: i32, export_name: &[u8]) -> Option<(u64, u16)> {
    let mut buf = [0u8; 256];

    // Read initial magic (old or new style)
    if read_exact(fd, &mut buf[..8]) != 8 {
        return None;
    }

    let magic = u64::from_be_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]);
    if magic != NBD_MAGIC {
        return None;
    }

    // Read second magic or size
    if read_exact(fd, &mut buf[..8]) != 8 {
        return None;
    }

    let second = u64::from_be_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]);

    if second == NBD_OPTS_MAGIC {
        // Newstyle negotiation
        return newstyle_handshake(fd, export_name);
    } else {
        // Oldstyle - second was size
        let size = second;

        // Read flags (4 bytes for old style)
        if read_exact(fd, &mut buf[..4]) != 4 {
            return None;
        }
        let flags = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);

        // Read and discard reserved bytes (124 bytes)
        let mut discard = [0u8; 124];
        if read_exact(fd, &mut discard) != 124 {
            return None;
        }

        return Some((size, flags as u16));
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn newstyle_handshake(fd: i32, export_name: &[u8]) -> Option<(u64, u16)> {
    let mut buf = [0u8; 1024];

    // Read handshake flags (2 bytes)
    if read_exact(fd, &mut buf[..2]) != 2 {
        return None;
    }
    let handshake_flags = u16::from_be_bytes([buf[0], buf[1]]);

    // Send client flags
    let mut client_flags: u32 = 0;
    if handshake_flags & NBD_FLAG_FIXED_NEWSTYLE != 0 {
        client_flags |= NBD_FLAG_C_FIXED_NEWSTYLE;
    }
    if handshake_flags & NBD_FLAG_NO_ZEROES != 0 {
        client_flags |= NBD_FLAG_C_NO_ZEROES;
    }
    let flags_bytes = client_flags.to_be_bytes();
    io::write_all(fd, &flags_bytes);

    // Try NBD_OPT_GO first (newer protocol)
    let size_flags = try_opt_go(fd, export_name);
    if size_flags.is_some() {
        return size_flags;
    }

    // Fall back to NBD_OPT_EXPORT_NAME
    // Send option: IHAVEOPT magic (8 bytes) + option (4 bytes) + length (4 bytes) + name
    let opt_magic = NBD_OPTS_MAGIC.to_be_bytes();
    let opt_type = NBD_OPT_EXPORT_NAME.to_be_bytes();
    let name_len = (export_name.len() as u32).to_be_bytes();

    io::write_all(fd, &opt_magic);
    io::write_all(fd, &opt_type);
    io::write_all(fd, &name_len);
    if !export_name.is_empty() {
        io::write_all(fd, export_name);
    }

    // Read response: size (8 bytes) + flags (2 bytes) + reserved (124 bytes, if NO_ZEROES not set)
    if read_exact(fd, &mut buf[..8]) != 8 {
        return None;
    }
    let size = u64::from_be_bytes([
        buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
    ]);

    if read_exact(fd, &mut buf[..2]) != 2 {
        return None;
    }
    let flags = u16::from_be_bytes([buf[0], buf[1]]);

    // Read reserved bytes if NO_ZEROES not set
    if client_flags & NBD_FLAG_C_NO_ZEROES == 0 {
        let mut discard = [0u8; 124];
        if read_exact(fd, &mut discard) != 124 {
            return None;
        }
    }

    Some((size, flags))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn try_opt_go(fd: i32, export_name: &[u8]) -> Option<(u64, u16)> {
    let mut buf = [0u8; 1024];

    // Send NBD_OPT_GO
    let opt_magic = NBD_OPTS_MAGIC.to_be_bytes();
    let opt_type = NBD_OPT_GO.to_be_bytes();
    // Data: name length (4 bytes) + name + num info requests (2 bytes)
    let data_len = (4 + export_name.len() + 2) as u32;
    let data_len_bytes = data_len.to_be_bytes();
    let name_len = (export_name.len() as u32).to_be_bytes();

    io::write_all(fd, &opt_magic);
    io::write_all(fd, &opt_type);
    io::write_all(fd, &data_len_bytes);
    io::write_all(fd, &name_len);
    if !export_name.is_empty() {
        io::write_all(fd, export_name);
    }
    io::write_all(fd, &[0, 0]); // No info requests

    // Read replies
    let mut size: u64 = 0;
    let mut flags: u16 = 0;

    loop {
        // Reply header: magic (8) + option (4) + reply type (4) + length (4)
        if read_exact(fd, &mut buf[..20]) != 20 {
            return None;
        }

        let reply_magic = u64::from_be_bytes([
            buf[0], buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7],
        ]);
        if reply_magic != 0x3e889045565a9 {
            // NBD_REP_MAGIC in BE
            return None;
        }

        let reply_type = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let reply_len = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);

        if reply_type == NBD_REP_ERR_UNSUP {
            // Server doesn't support OPT_GO, skip remaining data
            if reply_len > 0 && reply_len < 1024 {
                read_exact(fd, &mut buf[..reply_len as usize]);
            }
            return None;
        }

        if reply_type & NBD_REP_FLAG_ERROR != 0 {
            // Error
            if reply_len > 0 && reply_len < 1024 {
                read_exact(fd, &mut buf[..reply_len as usize]);
            }
            return None;
        }

        if reply_type == NBD_REP_ACK {
            // Success, we have all info
            break;
        }

        if reply_type == NBD_REP_INFO {
            // Info reply
            if reply_len >= 2 && reply_len < 1024 {
                if read_exact(fd, &mut buf[..reply_len as usize]) != reply_len as isize {
                    return None;
                }

                let info_type = u16::from_be_bytes([buf[0], buf[1]]);
                if info_type == 0 && reply_len >= 12 {
                    // NBD_INFO_EXPORT
                    flags = u16::from_be_bytes([buf[2], buf[3]]);
                    size = u64::from_be_bytes([
                        buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11],
                    ]);
                }
            }
        } else {
            // Unknown reply, skip
            if reply_len > 0 && reply_len < 1024 {
                read_exact(fd, &mut buf[..reply_len as usize]);
            }
        }
    }

    if size > 0 { Some((size, flags)) } else { None }
}

fn read_exact(fd: i32, buf: &mut [u8]) -> isize {
    let mut total = 0usize;
    while total < buf.len() {
        let n = io::read(fd, &mut buf[total..]);
        if n <= 0 {
            return total as isize;
        }
        total += n as usize;
    }
    total as isize
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn disconnect_device(device: &[u8]) -> i32 {
    let fd = io::open(device, libc::O_RDWR, 0);
    if fd < 0 {
        io::write_str(2, b"nbd-client: cannot open ");
        io::write_all(2, device);
        io::write_str(2, b"\n");
        return 1;
    }

    unsafe {
        libc::ioctl(fd, NBD_DISCONNECT);
        libc::ioctl(fd, NBD_CLEAR_SOCK);
    }

    io::close(fd);
    0
}

fn print_usage() {
    io::write_str(1, b"Usage: nbd-client HOST [PORT] DEVICE [OPTIONS]\n");
    io::write_str(1, b"       nbd-client -d DEVICE\n\n");
    io::write_str(1, b"Connect to NBD server or disconnect device.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -b BLKSIZE  Block size (default: 512)\n");
    io::write_str(1, b"  -t TIMEOUT  Timeout in seconds\n");
    io::write_str(1, b"  -n NAME     Export name (newstyle servers)\n");
    io::write_str(1, b"  -d DEVICE   Disconnect device\n");
    io::write_str(1, b"  -p          Persist (don't daemonize)\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn nbd_client(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"nbd-client: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_nbd_client_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nbd-client", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_nbd_client_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nbd-client"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing host"));
    }
}
