//! nbd-server - network block device server
//!
//! Serve block devices over the network.

#![allow(dead_code)]

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

// NBD protocol constants
const NBD_MAGIC: u64 = 0x4e42444d41474943; // "NBDMAGIC"
const NBD_OPTS_MAGIC: u64 = 0x49484156454F5054; // "IHAVEOPT"
const NBD_REP_MAGIC: u64 = 0x3e889045565a9;
const NBD_REQUEST_MAGIC: u32 = 0x25609513;
const NBD_REPLY_MAGIC: u32 = 0x67446698;

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
const NBD_REP_ERR_POLICY: u32 = NBD_REP_FLAG_ERROR | 2;
const NBD_REP_ERR_INVALID: u32 = NBD_REP_FLAG_ERROR | 3;

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

// Error codes
const NBD_EPERM: u32 = 1;
const NBD_EIO: u32 = 5;
const NBD_ENOMEM: u32 = 12;
const NBD_EINVAL: u32 = 22;
const NBD_ENOSPC: u32 = 28;

/// nbd-server - network block device server
///
/// # Synopsis
/// ```text
/// nbd-server PORT FILE [-r] [-c] [-d]
/// ```
///
/// # Description
/// Serve a file or block device over the network using NBD protocol.
///
/// # Options
/// - `-r`: Read-only mode
/// - `-c`: Copy-on-write (changes not saved)
/// - `-d`: Debug mode (don't daemonize)
/// - `-n NAME`: Export name (for newstyle)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn nbd_server(argc: i32, argv: *const *const u8) -> i32 {
    let mut port: u16 = 10809;
    let mut file: Option<&[u8]> = None;
    let mut read_only = false;
    let mut copy_on_write = false;
    let mut debug = false;
    let mut export_name: &[u8] = b"";

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-r" || arg == b"--readonly" {
            read_only = true;
        } else if arg == b"-c" || arg == b"--copy-on-write" {
            copy_on_write = true;
        } else if arg == b"-d" || arg == b"--debug" {
            debug = true;
        } else if arg == b"-n" {
            i += 1;
            if let Some(n) = unsafe { get_arg(argv, i as i32) } {
                export_name = n;
            }
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            if file.is_none() {
                // Check if it looks like a port number
                if arg.iter().all(|&c| c >= b'0' && c <= b'9') {
                    port = sys::parse_u64(arg).unwrap_or(10809) as u16;
                } else {
                    file = Some(arg);
                }
            } else {
                file = Some(arg);
            }
        }
        i += 1;
    }

    let file = match file {
        Some(f) => f,
        None => {
            io::write_str(2, b"nbd-server: missing file argument\n");
            print_usage();
            return 1;
        }
    };

    // Get file size
    let open_flags = if read_only || copy_on_write {
        libc::O_RDONLY
    } else {
        libc::O_RDWR
    };

    let file_fd = io::open(file, open_flags, 0);
    if file_fd < 0 {
        io::write_str(2, b"nbd-server: cannot open ");
        io::write_all(2, file);
        io::write_str(2, b"\n");
        return 1;
    }

    let mut stat_buf = io::stat_zeroed();
    if io::fstat(file_fd, &mut stat_buf) < 0 {
        io::write_str(2, b"nbd-server: cannot stat ");
        io::write_all(2, file);
        io::write_str(2, b"\n");
        io::close(file_fd);
        return 1;
    }

    let file_size = stat_buf.st_size as u64;
    io::close(file_fd);

    // Create server socket
    let server_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    if server_fd < 0 {
        io::write_str(2, b"nbd-server: socket failed\n");
        return 1;
    }

    // Allow address reuse
    let optval: i32 = 1;
    unsafe {
        libc::setsockopt(
            server_fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &optval as *const _ as *const libc::c_void,
            core::mem::size_of::<i32>() as libc::socklen_t,
        );
    }

    // Bind
    let mut addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    addr.sin_family = libc::AF_INET as u16;
    addr.sin_port = port.to_be();
    addr.sin_addr.s_addr = 0; // INADDR_ANY

    if unsafe {
        libc::bind(
            server_fd,
            &addr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    } < 0
    {
        io::write_str(2, b"nbd-server: bind failed\n");
        io::close(server_fd);
        return 1;
    }

    // Listen
    if unsafe { libc::listen(server_fd, 5) } < 0 {
        io::write_str(2, b"nbd-server: listen failed\n");
        io::close(server_fd);
        return 1;
    }

    // Daemonize unless debug mode
    if !debug {
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            io::write_str(2, b"nbd-server: fork failed\n");
            io::close(server_fd);
            return 1;
        }
        if pid > 0 {
            // Parent exits
            io::close(server_fd);
            return 0;
        }

        // Child daemonizes
        unsafe {
            libc::setsid();
            libc::chdir(b"/\0".as_ptr() as *const libc::c_char);
        }
    }

    if debug {
        io::write_str(1, b"nbd-server: listening on port ");
        let mut num_buf = [0u8; 10];
        io::write_all(1, sys::format_u64(port as u64, &mut num_buf));
        io::write_str(1, b"\n");
    }

    // Accept loop
    loop {
        let mut client_addr: libc::sockaddr_in = unsafe { core::mem::zeroed() };
        let mut addr_len: libc::socklen_t =
            core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

        let client_fd = unsafe {
            libc::accept(
                server_fd,
                &mut client_addr as *mut _ as *mut libc::sockaddr,
                &mut addr_len,
            )
        };

        if client_fd < 0 {
            continue;
        }

        // Fork to handle client
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            io::close(client_fd);
            continue;
        }

        if pid > 0 {
            // Parent
            io::close(client_fd);

            // Reap zombies
            unsafe {
                let mut status: i32 = 0;
                while libc::waitpid(-1, &mut status, libc::WNOHANG) > 0 {}
            }
            continue;
        }

        // Child - handle client
        io::close(server_fd);

        let result = handle_client(
            client_fd,
            file,
            file_size,
            read_only,
            copy_on_write,
            export_name,
            debug,
        );

        io::close(client_fd);
        unsafe { libc::_exit(result) };
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn handle_client(
    fd: i32,
    file: &[u8],
    size: u64,
    read_only: bool,
    _copy_on_write: bool,
    _export_name: &[u8],
    _debug: bool,
) -> i32 {
    // Send newstyle handshake
    // Magic (8 bytes) + opts magic (8 bytes) + handshake flags (2 bytes)
    let magic = NBD_MAGIC.to_be_bytes();
    let opts_magic = NBD_OPTS_MAGIC.to_be_bytes();
    let handshake_flags = (NBD_FLAG_FIXED_NEWSTYLE | NBD_FLAG_NO_ZEROES).to_be_bytes();

    io::write_all(fd, &magic);
    io::write_all(fd, &opts_magic);
    io::write_all(fd, &handshake_flags);

    // Read client flags
    let mut buf = [0u8; 4];
    if read_exact(fd, &mut buf) != 4 {
        return 1;
    }
    let client_flags = u32::from_be_bytes(buf);
    let _no_zeroes = client_flags & NBD_FLAG_C_NO_ZEROES != 0;

    // Handle options phase
    let transmission_flags = if read_only {
        NBD_FLAG_HAS_FLAGS | NBD_FLAG_READ_ONLY | NBD_FLAG_SEND_FLUSH
    } else {
        NBD_FLAG_HAS_FLAGS | NBD_FLAG_SEND_FLUSH | NBD_FLAG_SEND_TRIM
    };

    loop {
        // Read option header: magic (8) + option (4) + length (4)
        let mut header = [0u8; 16];
        if read_exact(fd, &mut header) != 16 {
            return 1;
        }

        let opt_magic = u64::from_be_bytes([
            header[0], header[1], header[2], header[3], header[4], header[5], header[6], header[7],
        ]);
        if opt_magic != NBD_OPTS_MAGIC {
            return 1;
        }

        let option = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
        let data_len = u32::from_be_bytes([header[12], header[13], header[14], header[15]]);

        // Read option data
        let mut data = Vec::new();
        if data_len > 0 {
            if data_len > 65536 {
                return 1; // Too large
            }
            data.resize(data_len as usize, 0);
            if read_exact(fd, &mut data) != data_len as isize {
                return 1;
            }
        }

        match option {
            NBD_OPT_EXPORT_NAME => {
                // Client wants to start transmission
                // Send size (8) + flags (2) + zeroes (124, if not NO_ZEROES)
                let size_bytes = size.to_be_bytes();
                let flags_bytes = transmission_flags.to_be_bytes();

                io::write_all(fd, &size_bytes);
                io::write_all(fd, &flags_bytes);

                if !_no_zeroes {
                    let zeroes = [0u8; 124];
                    io::write_all(fd, &zeroes);
                }

                break; // Enter transmission phase
            }
            NBD_OPT_GO => {
                // Send NBD_REP_INFO with export info, then NBD_REP_ACK
                // NBD_INFO_EXPORT = 0
                let info_reply = build_reply(option, NBD_REP_INFO, &{
                    let mut info = Vec::new();
                    info.extend_from_slice(&0u16.to_be_bytes()); // NBD_INFO_EXPORT
                    info.extend_from_slice(&transmission_flags.to_be_bytes());
                    info.extend_from_slice(&size.to_be_bytes());
                    info
                });
                io::write_all(fd, &info_reply);

                // Send ACK
                let ack = build_reply(option, NBD_REP_ACK, &[]);
                io::write_all(fd, &ack);

                break; // Enter transmission phase
            }
            NBD_OPT_LIST => {
                // Send single export (unnamed)
                let export_reply = build_reply(option, NBD_REP_SERVER, &{
                    let mut data = Vec::new();
                    data.extend_from_slice(&0u32.to_be_bytes()); // Name length
                    data
                });
                io::write_all(fd, &export_reply);

                let ack = build_reply(option, NBD_REP_ACK, &[]);
                io::write_all(fd, &ack);
            }
            NBD_OPT_ABORT => {
                let ack = build_reply(option, NBD_REP_ACK, &[]);
                io::write_all(fd, &ack);
                return 0;
            }
            _ => {
                // Unknown option
                let err = build_reply(option, NBD_REP_ERR_UNSUP, &[]);
                io::write_all(fd, &err);
            }
        }
    }

    // Open file for transmission
    let open_flags = if read_only {
        libc::O_RDONLY
    } else {
        libc::O_RDWR
    };

    let file_fd = io::open(file, open_flags, 0);
    if file_fd < 0 {
        return 1;
    }

    // Transmission phase
    let mut req_buf = [0u8; 28];
    let mut data_buf = Vec::new();

    loop {
        // Read request: magic (4) + flags (2) + type (2) + handle (8) + offset (8) + length (4)
        if read_exact(fd, &mut req_buf) != 28 {
            break;
        }

        let magic = u32::from_be_bytes([req_buf[0], req_buf[1], req_buf[2], req_buf[3]]);
        if magic != NBD_REQUEST_MAGIC {
            break;
        }

        let _flags = u16::from_be_bytes([req_buf[4], req_buf[5]]);
        let cmd_type = u16::from_be_bytes([req_buf[6], req_buf[7]]);
        let handle = &req_buf[8..16];
        let offset = u64::from_be_bytes([
            req_buf[16],
            req_buf[17],
            req_buf[18],
            req_buf[19],
            req_buf[20],
            req_buf[21],
            req_buf[22],
            req_buf[23],
        ]);
        let length = u32::from_be_bytes([req_buf[24], req_buf[25], req_buf[26], req_buf[27]]);

        match cmd_type as u32 {
            NBD_CMD_READ => {
                // Seek to offset
                if io::lseek(file_fd, offset as i64, libc::SEEK_SET) < 0 {
                    send_reply(fd, NBD_EIO, handle, &[]);
                    continue;
                }

                // Read data
                data_buf.clear();
                data_buf.resize(length as usize, 0);
                let n = io::read(file_fd, &mut data_buf);
                if n < 0 {
                    send_reply(fd, NBD_EIO, handle, &[]);
                    continue;
                }

                // Pad with zeros if short read
                if (n as usize) < length as usize {
                    for i in (n as usize)..(length as usize) {
                        data_buf[i] = 0;
                    }
                }

                send_reply(fd, 0, handle, &data_buf);
            }
            NBD_CMD_WRITE => {
                if read_only {
                    // Discard write data
                    data_buf.clear();
                    data_buf.resize(length as usize, 0);
                    read_exact(fd, &mut data_buf);
                    send_reply(fd, NBD_EPERM, handle, &[]);
                    continue;
                }

                // Read write data
                data_buf.clear();
                data_buf.resize(length as usize, 0);
                if read_exact(fd, &mut data_buf) != length as isize {
                    send_reply(fd, NBD_EIO, handle, &[]);
                    continue;
                }

                // Seek and write
                if io::lseek(file_fd, offset as i64, libc::SEEK_SET) < 0 {
                    send_reply(fd, NBD_EIO, handle, &[]);
                    continue;
                }

                io::write_all(file_fd, &data_buf);
                send_reply(fd, 0, handle, &[]);
            }
            NBD_CMD_DISC => {
                // Disconnect
                break;
            }
            NBD_CMD_FLUSH => {
                // Sync file
                unsafe { libc::fsync(file_fd) };
                send_reply(fd, 0, handle, &[]);
            }
            NBD_CMD_TRIM => {
                // We don't actually trim, just acknowledge
                send_reply(fd, 0, handle, &[]);
            }
            _ => {
                send_reply(fd, NBD_EINVAL, handle, &[]);
            }
        }
    }

    io::close(file_fd);
    0
}

fn build_reply(option: u32, reply_type: u32, data: &[u8]) -> Vec<u8> {
    let mut reply = Vec::new();
    reply.extend_from_slice(&NBD_REP_MAGIC.to_be_bytes());
    reply.extend_from_slice(&option.to_be_bytes());
    reply.extend_from_slice(&reply_type.to_be_bytes());
    reply.extend_from_slice(&(data.len() as u32).to_be_bytes());
    reply.extend_from_slice(data);
    reply
}

fn send_reply(fd: i32, error: u32, handle: &[u8], data: &[u8]) {
    let mut reply = [0u8; 16];
    reply[0..4].copy_from_slice(&NBD_REPLY_MAGIC.to_be_bytes());
    reply[4..8].copy_from_slice(&error.to_be_bytes());
    reply[8..16].copy_from_slice(handle);

    io::write_all(fd, &reply);
    if !data.is_empty() {
        io::write_all(fd, data);
    }
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

fn print_usage() {
    io::write_str(1, b"Usage: nbd-server [PORT] FILE [OPTIONS]\n\n");
    io::write_str(1, b"Serve a file over the network using NBD protocol.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -r        Read-only mode\n");
    io::write_str(1, b"  -c        Copy-on-write (changes not saved)\n");
    io::write_str(1, b"  -d        Debug mode (don't daemonize)\n");
    io::write_str(1, b"  -n NAME   Export name\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn nbd_server(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"nbd-server: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_nbd_server_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nbd-server", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_nbd_server_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nbd-server"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing file"));
    }
}
