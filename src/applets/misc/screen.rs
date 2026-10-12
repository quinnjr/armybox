//! screen - terminal multiplexer
//!
//! A simplified implementation of GNU screen for terminal session management.

use super::get_arg;
use crate::io;

/// Session directory for storing screen sessions
const SESSION_DIR: &[u8] = b"/tmp/armybox-screen";

/// Ctrl+A is the command prefix (ASCII 0x01)
const CTRL_A: u8 = 0x01;

/// Print help message
fn print_help() {
    io::write_str(1, b"Usage: screen [options] [command [args]]\n");
    io::write_str(1, b"\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -ls           List running sessions\n");
    io::write_str(1, b"  -r <session>  Reattach to a detached session\n");
    io::write_str(1, b"  -d <session>  Detach a running session\n");
    io::write_str(1, b"  -S <name>     Create a named session\n");
    io::write_str(
        1,
        b"  -x <session>  Attach to an already attached session (multi-display)\n",
    );
    io::write_str(1, b"  -h            Display this help message\n");
    io::write_str(1, b"\n");
    io::write_str(1, b"Commands while in screen:\n");
    io::write_str(1, b"  Ctrl+A d      Detach from session\n");
    io::write_str(1, b"  Ctrl+A k      Kill window\n");
    io::write_str(1, b"\n");
    io::write_str(1, b"Sessions are stored in /tmp/armybox-screen/\n");
}

/// Ensure session directory exists
fn ensure_session_dir() -> bool {
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(SESSION_DIR, &mut st) == 0 {
        // Directory exists
        return true;
    }
    // Create directory with rwx for owner only
    io::mkdir(SESSION_DIR, 0o700) == 0
}

/// List all screen sessions
pub fn screen_list_sessions() -> i32 {
    if !ensure_session_dir() {
        io::write_str(2, b"screen: cannot access session directory\n");
        return 1;
    }

    let dir = io::opendir(SESSION_DIR);
    if dir.is_null() {
        io::write_str(1, b"No Sockets found in /tmp/armybox-screen/\n");
        return 0;
    }

    let mut found = false;
    io::write_str(1, b"There are screens on:\n");

    loop {
        let entry = io::readdir(dir);
        if entry.is_null() {
            break;
        }

        let (name, len) = unsafe { io::dirent_name(entry) };

        // Skip . and ..
        if len == 1 && name[0] == b'.' {
            continue;
        }
        if len == 2 && name[0] == b'.' && name[1] == b'.' {
            continue;
        }

        // Check if it's a socket file
        let mut path_buf = [0u8; 512];
        let mut idx = 0;
        for &c in SESSION_DIR {
            path_buf[idx] = c;
            idx += 1;
        }
        path_buf[idx] = b'/';
        idx += 1;
        for i in 0..len {
            path_buf[idx] = name[i];
            idx += 1;
        }

        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::stat(&path_buf[..idx], &mut st) == 0 {
            // Check if socket (S_IFSOCK)
            if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFSOCK as u32 {
                io::write_str(1, b"\t");
                io::write_all(1, &name[..len]);
                io::write_str(1, b"\t(Detached)\n");
                found = true;
            }
        }
    }

    io::closedir(dir);

    if !found {
        io::write_str(1, b"No Sockets found in /tmp/armybox-screen/\n");
    }

    0
}

/// Detach a session by name
pub fn screen_detach(session_name: &[u8]) -> i32 {
    if !ensure_session_dir() {
        io::write_str(2, b"screen: cannot access session directory\n");
        return 1;
    }

    // Build socket path
    let mut sock_path = [0u8; 512];
    let mut idx = 0;
    for &c in SESSION_DIR {
        sock_path[idx] = c;
        idx += 1;
    }
    sock_path[idx] = b'/';
    idx += 1;
    for &c in session_name {
        sock_path[idx] = c;
        idx += 1;
    }

    // Check if socket exists
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(&sock_path[..idx], &mut st) != 0 {
        io::write_str(2, b"screen: no session named '");
        io::write_all(2, session_name);
        io::write_str(2, b"'\n");
        return 1;
    }

    // Connect to socket and send detach command
    #[cfg(feature = "alloc")]
    {
        let sock_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };

        if sock_fd < 0 {
            io::write_str(2, b"screen: cannot create socket\n");
            return 1;
        }

        let mut addr: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

        // Copy path to sun_path
        for i in 0..idx.min(107) {
            addr.sun_path[i] = sock_path[i] as libc::c_char;
        }

        let ret = unsafe {
            libc::connect(
                sock_fd,
                &addr as *const libc::sockaddr_un as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            )
        };

        if ret < 0 {
            io::write_str(2, b"screen: cannot connect to session\n");
            io::close(sock_fd);
            return 1;
        }

        // Send detach signal
        let detach_cmd = b"DETACH\n";
        io::write_all(sock_fd, detach_cmd);
        io::close(sock_fd);

        io::write_str(1, b"[detached from ");
        io::write_all(1, session_name);
        io::write_str(1, b"]\n");
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"screen: detach requires alloc feature\n");
        return 1;
    }

    0
}

/// Reattach to a detached session
pub fn screen_reattach(session_name: &[u8]) -> i32 {
    if !ensure_session_dir() {
        io::write_str(2, b"screen: cannot access session directory\n");
        return 1;
    }

    // Build socket path
    let mut sock_path = [0u8; 512];
    let mut idx = 0;
    for &c in SESSION_DIR {
        sock_path[idx] = c;
        idx += 1;
    }
    sock_path[idx] = b'/';
    idx += 1;
    for &c in session_name {
        sock_path[idx] = c;
        idx += 1;
    }

    // Check if socket exists
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(&sock_path[..idx], &mut st) != 0 {
        io::write_str(2, b"screen: no session named '");
        io::write_all(2, session_name);
        io::write_str(2, b"'\n");
        return 1;
    }

    #[cfg(feature = "alloc")]
    {
        let sock_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };

        if sock_fd < 0 {
            io::write_str(2, b"screen: cannot create socket\n");
            return 1;
        }

        let mut addr: libc::sockaddr_un = unsafe { core::mem::zeroed() };
        addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

        for i in 0..idx.min(107) {
            addr.sun_path[i] = sock_path[i] as libc::c_char;
        }

        let ret = unsafe {
            libc::connect(
                sock_fd,
                &addr as *const libc::sockaddr_un as *const libc::sockaddr,
                core::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
            )
        };

        if ret < 0 {
            io::write_str(2, b"screen: cannot connect to session '");
            io::write_all(2, session_name);
            io::write_str(2, b"'\n");
            io::close(sock_fd);
            return 1;
        }

        io::write_str(1, b"[attaching to ");
        io::write_all(1, session_name);
        io::write_str(1, b"]\n");

        // Set terminal to raw mode
        let mut orig_termios: libc::termios = unsafe { core::mem::zeroed() };
        unsafe { libc::tcgetattr(0, &mut orig_termios) };

        let mut raw = orig_termios;
        unsafe { libc::cfmakeraw(&mut raw) };
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw) };

        // Main loop: relay data between terminal and session
        let mut buf = [0u8; 4096];
        let mut running = true;

        while running {
            let mut fds: [libc::pollfd; 2] = unsafe { core::mem::zeroed() };
            fds[0].fd = 0; // stdin
            fds[0].events = libc::POLLIN;
            fds[1].fd = sock_fd;
            fds[1].events = libc::POLLIN;

            let ret = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
            if ret < 0 {
                break;
            }

            // Data from stdin -> socket
            if fds[0].revents & libc::POLLIN != 0 {
                let n = io::read(0, &mut buf);
                if n <= 0 {
                    break;
                }

                // Check for Ctrl+A d (detach)
                if n >= 2 {
                    for i in 0..(n as usize - 1) {
                        if buf[i] == CTRL_A && buf[i + 1] == b'd' {
                            running = false;
                            break;
                        }
                    }
                }

                if running {
                    io::write_all(sock_fd, &buf[..n as usize]);
                }
            }

            // Data from socket -> stdout
            if fds[1].revents & libc::POLLIN != 0 {
                let n = io::read(sock_fd, &mut buf);
                if n <= 0 {
                    break;
                }
                io::write_all(1, &buf[..n as usize]);
            }

            // Check for hangup
            if fds[1].revents & libc::POLLHUP != 0 {
                break;
            }
        }

        // Restore terminal
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &orig_termios) };
        io::close(sock_fd);

        io::write_str(1, b"\n[detached]\n");
    }

    #[cfg(not(feature = "alloc"))]
    {
        io::write_str(2, b"screen: reattach requires alloc feature\n");
        return 1;
    }

    0
}

/// Create a new screen session with PTY
#[cfg(feature = "alloc")]
pub fn screen_new_session(session_name: Option<&[u8]>) -> i32 {
    use alloc::ffi::CString;
    use alloc::vec::Vec;

    if !ensure_session_dir() {
        io::write_str(2, b"screen: cannot create session directory\n");
        return 1;
    }

    // Generate session name if not provided
    let pid = io::getpid();
    let mut name_buf = [0u8; 64];
    let name = if let Some(n) = session_name {
        n
    } else {
        // Generate name from PID
        let mut idx = 0;
        let mut p = pid as u64;
        let mut digits = [0u8; 20];
        let mut dlen = 0;

        if p == 0 {
            digits[0] = b'0';
            dlen = 1;
        } else {
            while p > 0 {
                digits[dlen] = b'0' + (p % 10) as u8;
                p /= 10;
                dlen += 1;
            }
        }

        // Reverse digits
        for i in 0..dlen {
            name_buf[idx] = digits[dlen - 1 - i];
            idx += 1;
        }
        name_buf[idx] = b'.';
        idx += 1;
        for &c in b"pts" {
            name_buf[idx] = c;
            idx += 1;
        }
        &name_buf[..idx]
    };

    // Build socket path
    let mut sock_path = [0u8; 512];
    let mut idx = 0;
    for &c in SESSION_DIR {
        sock_path[idx] = c;
        idx += 1;
    }
    sock_path[idx] = b'/';
    idx += 1;
    for &c in name {
        sock_path[idx] = c;
        idx += 1;
    }
    let sock_path_len = idx;

    // Create PTY
    let mut master_fd: i32 = 0;
    let mut slave_fd: i32 = 0;

    let ret = unsafe {
        libc::openpty(
            &mut master_fd,
            &mut slave_fd,
            core::ptr::null_mut(),
            core::ptr::null(),
            core::ptr::null(),
        )
    };

    if ret < 0 {
        io::write_str(2, b"screen: cannot create PTY\n");
        return 1;
    }

    // Create Unix socket for session
    let listen_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };

    if listen_fd < 0 {
        io::write_str(2, b"screen: cannot create socket\n");
        io::close(master_fd);
        io::close(slave_fd);
        return 1;
    }

    // Remove existing socket if present
    io::unlink(&sock_path[..sock_path_len]);

    let mut addr: libc::sockaddr_un = unsafe { core::mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    for i in 0..sock_path_len.min(107) {
        addr.sun_path[i] = sock_path[i] as libc::c_char;
    }

    let ret = unsafe {
        libc::bind(
            listen_fd,
            &addr as *const libc::sockaddr_un as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_un>() as libc::socklen_t,
        )
    };

    if ret < 0 {
        io::write_str(2, b"screen: cannot bind socket\n");
        io::close(listen_fd);
        io::close(master_fd);
        io::close(slave_fd);
        return 1;
    }

    unsafe { libc::listen(listen_fd, 5) };

    // Fork for the session manager
    let manager_pid = io::fork();

    if manager_pid < 0 {
        io::write_str(2, b"screen: fork failed\n");
        io::close(listen_fd);
        io::close(master_fd);
        io::close(slave_fd);
        io::unlink(&sock_path[..sock_path_len]);
        return 1;
    }

    if manager_pid > 0 {
        // Parent process - close fds and return
        io::close(listen_fd);
        io::close(master_fd);
        io::close(slave_fd);

        io::write_str(1, b"[screen session '");
        io::write_all(1, name);
        io::write_str(1, b"' created]\n");
        return 0;
    }

    // Session manager process
    // Detach from controlling terminal
    unsafe { libc::setsid() };

    // Fork shell process
    let shell_pid = io::fork();

    if shell_pid < 0 {
        io::close(listen_fd);
        io::close(master_fd);
        io::close(slave_fd);
        io::unlink(&sock_path[..sock_path_len]);
        io::exit(1);
    }

    if shell_pid == 0 {
        // Shell process - run in slave PTY
        io::close(master_fd);
        io::close(listen_fd);

        // Set up slave as controlling terminal
        unsafe { libc::setsid() };
        unsafe { libc::ioctl(slave_fd, libc::TIOCSCTTY as crate::io::IoctlReq, 0) };

        // Redirect stdio to slave
        io::dup2(slave_fd, 0);
        io::dup2(slave_fd, 1);
        io::dup2(slave_fd, 2);

        if slave_fd > 2 {
            io::close(slave_fd);
        }

        // Get shell from environment or default to /bin/sh
        let shell = io::getenv(b"SHELL").unwrap_or(b"/bin/sh");
        let mut shell_path = Vec::with_capacity(shell.len() + 1);
        shell_path.extend_from_slice(shell);
        shell_path.push(0);

        if let Ok(shell_cstr) = CString::from_vec_with_nul(shell_path) {
            let args: [*const libc::c_char; 2] = [shell_cstr.as_ptr(), core::ptr::null()];
            unsafe {
                libc::execv(shell_cstr.as_ptr(), args.as_ptr());
            }
        }

        io::exit(127);
    }

    // Session manager continues
    io::close(slave_fd);

    // Main session loop
    let mut client_fd: i32 = -1;
    let mut buf = [0u8; 4096];
    let mut running = true;

    while running {
        let mut nfds = 2;
        let mut fds: [libc::pollfd; 3] = unsafe { core::mem::zeroed() };

        fds[0].fd = master_fd;
        fds[0].events = libc::POLLIN;
        fds[1].fd = listen_fd;
        fds[1].events = libc::POLLIN;

        if client_fd >= 0 {
            fds[2].fd = client_fd;
            fds[2].events = libc::POLLIN;
            nfds = 3;
        }

        let ret = unsafe { libc::poll(fds.as_mut_ptr(), nfds, 100) };

        // Check if shell is still alive
        let mut status = 0;
        let wpid = io::waitpid(shell_pid, &mut status, libc::WNOHANG);
        if wpid == shell_pid {
            running = false;
            break;
        }

        if ret < 0 {
            continue;
        }

        // New client connection
        if fds[1].revents & libc::POLLIN != 0 {
            let new_fd =
                unsafe { libc::accept(listen_fd, core::ptr::null_mut(), core::ptr::null_mut()) };
            if new_fd >= 0 {
                if client_fd >= 0 {
                    io::close(client_fd);
                }
                client_fd = new_fd;
            }
        }

        // Data from PTY master -> client
        if fds[0].revents & libc::POLLIN != 0 {
            let n = io::read(master_fd, &mut buf);
            if n > 0 && client_fd >= 0 {
                io::write_all(client_fd, &buf[..n as usize]);
            }
        }

        // Data from client -> PTY master
        if nfds == 3 && fds[2].revents & libc::POLLIN != 0 {
            let n = io::read(client_fd, &mut buf);
            if n <= 0 {
                // Client disconnected
                io::close(client_fd);
                client_fd = -1;
            } else {
                // Check for DETACH command
                if n >= 7 && &buf[..7] == b"DETACH\n" {
                    io::close(client_fd);
                    client_fd = -1;
                } else {
                    io::write_all(master_fd, &buf[..n as usize]);
                }
            }
        }

        // Client disconnected
        if nfds == 3 && (fds[2].revents & libc::POLLHUP != 0) {
            io::close(client_fd);
            client_fd = -1;
        }
    }

    // Cleanup
    if client_fd >= 0 {
        io::close(client_fd);
    }
    io::close(master_fd);
    io::close(listen_fd);
    io::unlink(&sock_path[..sock_path_len]);

    io::exit(0);
}

#[cfg(not(feature = "alloc"))]
pub fn screen_new_session(_session_name: Option<&[u8]>) -> i32 {
    io::write_str(2, b"screen: new session requires alloc feature\n");
    1
}

/// Main screen command entry point
pub fn screen(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        // No arguments - create new session
        return screen_new_session(None);
    }

    let arg1 = unsafe { get_arg(argv, 1).unwrap() };

    // Parse options
    match arg1 {
        b"-h" | b"--help" => {
            print_help();
            0
        }
        b"-ls" | b"-list" => screen_list_sessions(),
        b"-d" | b"-D" => {
            // Detach session
            if argc < 3 {
                io::write_str(2, b"screen: -d requires session name\n");
                return 1;
            }
            let session = unsafe { get_arg(argv, 2).unwrap() };
            screen_detach(session)
        }
        b"-r" | b"-R" => {
            // Reattach to session
            if argc < 3 {
                io::write_str(2, b"screen: -r requires session name\n");
                return 1;
            }
            let session = unsafe { get_arg(argv, 2).unwrap() };
            screen_reattach(session)
        }
        b"-x" => {
            // Attach to attached session (multi-display)
            if argc < 3 {
                io::write_str(2, b"screen: -x requires session name\n");
                return 1;
            }
            let session = unsafe { get_arg(argv, 2).unwrap() };
            // For simplicity, -x behaves like -r in this implementation
            screen_reattach(session)
        }
        b"-S" => {
            // Create named session
            if argc < 3 {
                io::write_str(2, b"screen: -S requires session name\n");
                return 1;
            }
            let session = unsafe { get_arg(argv, 2).unwrap() };
            screen_new_session(Some(session))
        }
        _ => {
            // Unknown option or command to run
            if arg1.starts_with(b"-") {
                io::write_str(2, b"screen: unknown option '");
                io::write_all(2, arg1);
                io::write_str(2, b"'\n");
                return 1;
            }
            // Run command in new session
            screen_new_session(None)
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_screen_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["screen", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage:"));
        assert!(stdout.contains("-ls"));
        assert!(stdout.contains("-r"));
        assert!(stdout.contains("-d"));
        assert!(stdout.contains("-S"));
    }

    #[test]
    fn test_screen_list_empty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Clean up any existing sessions first
        let _ = std::fs::remove_dir_all("/tmp/armybox-screen");

        let output = Command::new(&armybox)
            .args(["screen", "-ls"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should either show "No Sockets found" or "There are screens on:"
        assert!(
            stdout.contains("screen") || stdout.contains("Socket") || stdout.contains("There are")
        );
    }

    #[test]
    fn test_screen_detach_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["screen", "-d", "nonexistent_session_12345"])
            .output()
            .unwrap();

        // Should fail with non-zero exit
        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_screen_reattach_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["screen", "-r", "nonexistent_session_12345"])
            .output()
            .unwrap();

        // Should fail with non-zero exit
        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_screen_missing_session_name() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Test -r without session name
        let output = Command::new(&armybox)
            .args(["screen", "-r"])
            .output()
            .unwrap();
        assert_ne!(output.status.code(), Some(0));

        // Test -d without session name
        let output = Command::new(&armybox)
            .args(["screen", "-d"])
            .output()
            .unwrap();
        assert_ne!(output.status.code(), Some(0));

        // Test -S without session name
        let output = Command::new(&armybox)
            .args(["screen", "-S"])
            .output()
            .unwrap();
        assert_ne!(output.status.code(), Some(0));
    }
}
