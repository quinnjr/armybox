//! slattach - attach a network interface to a serial line
//!
//! SLIP/CSLIP attachment utility.

use super::get_arg;
use crate::io;
use crate::sys;

// Line discipline constants
#[cfg(any(target_os = "linux", target_os = "android"))]
const N_SLIP: libc::c_int = 1; // Serial Line IP
#[cfg(any(target_os = "linux", target_os = "android"))]
const N_CSLIP: libc::c_int = 2; // SLIP + VJ header compression
#[cfg(any(target_os = "linux", target_os = "android"))]
const N_SLIP6: libc::c_int = 3; // 6-bit SLIP
#[cfg(any(target_os = "linux", target_os = "android"))]
const N_CSLIP6: libc::c_int = 4; // 6-bit SLIP + VJ compression
#[cfg(any(target_os = "linux", target_os = "android"))]
const N_PPP: libc::c_int = 3; // PPP (same as SLIP6 on some systems)

// TTY ioctl for setting line discipline
#[cfg(any(target_os = "linux", target_os = "android"))]
const TIOCSETD: crate::io::IoctlReq = 0x5423u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const TIOCGETD: crate::io::IoctlReq = 0x5424u32 as crate::io::IoctlReq;

/// slattach - attach a network interface to a serial line
///
/// # Synopsis
/// ```text
/// slattach [-p PROTO] [-s SPEED] [-e] [-L] TTY
/// ```
///
/// # Description
/// Attach a serial line to a network interface using SLIP protocol.
///
/// # Options
/// - `-p PROTO`: Protocol (slip, cslip, slip6, cslip6, ppp) [default: cslip]
/// - `-s SPEED`: Line speed (baud rate)
/// - `-e`: Exit after setting up line
/// - `-L`: Enable 3-wire mode (no modem control)
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn slattach(argc: i32, argv: *const *const u8) -> i32 {
    let mut protocol = N_CSLIP;
    let mut speed: Option<u32> = None;
    let mut exit_after_setup = false;
    let mut local_mode = false;
    let mut tty: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-p" {
            i += 1;
            if i < argc as usize {
                if let Some(proto) = unsafe { get_arg(argv, i as i32) } {
                    protocol = match proto {
                        b"slip" => N_SLIP,
                        b"cslip" => N_CSLIP,
                        b"slip6" => N_SLIP6,
                        b"cslip6" => N_CSLIP6,
                        b"ppp" => N_PPP,
                        _ => {
                            io::write_str(2, b"slattach: unknown protocol: ");
                            io::write_all(2, proto);
                            io::write_str(2, b"\n");
                            return 1;
                        }
                    };
                }
            }
        } else if arg == b"-s" {
            i += 1;
            if i < argc as usize {
                if let Some(s) = unsafe { get_arg(argv, i as i32) } {
                    speed = sys::parse_u64(s).map(|v| v as u32);
                }
            }
        } else if arg == b"-e" {
            exit_after_setup = true;
        } else if arg == b"-L" {
            local_mode = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            tty = Some(arg);
        }
        i += 1;
    }

    let tty = match tty {
        Some(t) => t,
        None => {
            io::write_str(2, b"slattach: missing TTY device\n");
            print_usage();
            return 1;
        }
    };

    // Open TTY device
    let fd = io::open(tty, libc::O_RDWR | libc::O_NOCTTY, 0);
    if fd < 0 {
        io::write_str(2, b"slattach: cannot open ");
        io::write_all(2, tty);
        io::write_str(2, b"\n");
        return 1;
    }

    // Get current terminal attributes
    let mut termios: libc::termios = unsafe { core::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } < 0 {
        io::write_str(2, b"slattach: cannot get terminal attributes\n");
        io::close(fd);
        return 1;
    }

    // Configure for raw mode
    termios.c_iflag = 0;
    termios.c_oflag = 0;
    termios.c_lflag = 0;
    termios.c_cflag = libc::CS8 | libc::CREAD | libc::HUPCL;

    if local_mode {
        termios.c_cflag |= libc::CLOCAL;
    }

    // Set speed if specified
    if let Some(baud) = speed {
        let speed_const = match baud {
            50 => libc::B50,
            75 => libc::B75,
            110 => libc::B110,
            134 => libc::B134,
            150 => libc::B150,
            200 => libc::B200,
            300 => libc::B300,
            600 => libc::B600,
            1200 => libc::B1200,
            1800 => libc::B1800,
            2400 => libc::B2400,
            4800 => libc::B4800,
            9600 => libc::B9600,
            19200 => libc::B19200,
            38400 => libc::B38400,
            57600 => libc::B57600,
            115200 => libc::B115200,
            230400 => libc::B230400,
            460800 => libc::B460800,
            500000 => libc::B500000,
            576000 => libc::B576000,
            921600 => libc::B921600,
            1000000 => libc::B1000000,
            1152000 => libc::B1152000,
            1500000 => libc::B1500000,
            2000000 => libc::B2000000,
            2500000 => libc::B2500000,
            3000000 => libc::B3000000,
            3500000 => libc::B3500000,
            4000000 => libc::B4000000,
            _ => {
                io::write_str(2, b"slattach: unsupported baud rate\n");
                io::close(fd);
                return 1;
            }
        };

        unsafe {
            libc::cfsetispeed(&mut termios, speed_const);
            libc::cfsetospeed(&mut termios, speed_const);
        }
    }

    // Apply terminal settings
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } < 0 {
        io::write_str(2, b"slattach: cannot set terminal attributes\n");
        io::close(fd);
        return 1;
    }

    // Set line discipline
    let ret = unsafe { libc::ioctl(fd, TIOCSETD as crate::io::IoctlReq, &protocol) };
    if ret < 0 {
        io::write_str(2, b"slattach: cannot set line discipline\n");
        io::close(fd);
        return 1;
    }

    // Print status
    io::write_str(1, b"Attached ");
    io::write_all(1, tty);
    io::write_str(1, b" with protocol ");
    let proto_name = match protocol {
        1 => b"slip" as &[u8],
        2 => b"cslip" as &[u8],
        3 => b"slip6" as &[u8],
        4 => b"cslip6" as &[u8],
        _ => b"unknown" as &[u8],
    };
    io::write_all(1, proto_name);
    io::write_str(1, b"\n");

    if exit_after_setup {
        // Exit immediately, keeping fd open through the process
        // The line discipline persists until the fd is closed
        return 0;
    }

    // Stay running and hold the line open
    // Wait for signals to terminate
    io::write_str(1, b"Press Ctrl+C to detach\n");

    loop {
        unsafe {
            libc::sleep(3600); // Sleep for an hour, wake on signals
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn slattach(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"slattach: only available on Linux\n");
    1
}

fn print_usage() {
    io::write_str(1, b"Usage: slattach [OPTIONS] TTY\n\n");
    io::write_str(1, b"Attach a serial line to a network interface.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(
        1,
        b"  -p PROTO   Protocol: slip, cslip, slip6, cslip6, ppp [default: cslip]\n",
    );
    io::write_str(1, b"  -s SPEED   Line speed (baud rate)\n");
    io::write_str(1, b"  -e         Exit after setting up line\n");
    io::write_str(1, b"  -L         Enable 3-wire mode (no modem control)\n");
    io::write_str(1, b"  -h         Show this help\n");
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
    fn test_slattach_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["slattach", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_slattach_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["slattach"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing TTY"));
    }
}
