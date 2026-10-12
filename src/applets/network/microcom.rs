//! microcom - minimalist terminal emulator
//!
//! Simple serial terminal.

use super::get_arg;
use crate::io;
use crate::sys;

/// microcom - minimalist terminal emulator
///
/// # Synopsis
/// ```text
/// microcom [-s SPEED] TTY
/// ```
///
/// # Description
/// Simple terminal emulator for serial ports. Connect to a serial device
/// and pass data bidirectionally between the terminal and the device.
///
/// # Options
/// - `-s SPEED`: Set baud rate (default: 115200)
///
/// # Escape sequence
/// Press Ctrl+X to exit
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn microcom(argc: i32, argv: *const *const u8) -> i32 {
    let mut speed: u32 = 115200;
    let mut device: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-s" {
            i += 1;
            if i < argc as usize {
                if let Some(s) = unsafe { get_arg(argv, i as i32) } {
                    speed = sys::parse_u64(s).unwrap_or(115200) as u32;
                }
            }
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            device = Some(arg);
        }
        i += 1;
    }

    let device = match device {
        Some(d) => d,
        None => {
            io::write_str(2, b"microcom: missing TTY device\n");
            print_usage();
            return 1;
        }
    };

    // Open serial device
    let fd = io::open(device, libc::O_RDWR | libc::O_NOCTTY, 0);
    if fd < 0 {
        io::write_str(2, b"microcom: cannot open ");
        io::write_all(2, device);
        io::write_str(2, b"\n");
        return 1;
    }

    // Configure serial port
    let mut termios: libc::termios = unsafe { core::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } < 0 {
        io::write_str(2, b"microcom: cannot get terminal attributes\n");
        io::close(fd);
        return 1;
    }

    // Raw mode for serial
    termios.c_iflag = 0;
    termios.c_oflag = 0;
    termios.c_lflag = 0;
    termios.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL;
    termios.c_cc[libc::VMIN] = 1;
    termios.c_cc[libc::VTIME] = 0;

    // Set speed
    let speed_const = match speed {
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
            io::write_str(2, b"microcom: unsupported baud rate, using 115200\n");
            libc::B115200
        }
    };

    unsafe {
        libc::cfsetispeed(&mut termios, speed_const);
        libc::cfsetospeed(&mut termios, speed_const);
    }

    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } < 0 {
        io::write_str(2, b"microcom: cannot set terminal attributes\n");
        io::close(fd);
        return 1;
    }

    // Save and configure stdin terminal
    let mut orig_termios: libc::termios = unsafe { core::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut orig_termios) } < 0 {
        io::write_str(2, b"microcom: cannot get stdin attributes\n");
        io::close(fd);
        return 1;
    }

    let mut raw = orig_termios;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
    raw.c_iflag &= !(libc::IXON | libc::ICRNL | libc::BRKINT | libc::INPCK | libc::ISTRIP);
    raw.c_oflag &= !libc::OPOST;
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;

    if unsafe { libc::tcsetattr(0, libc::TCSAFLUSH, &raw) } < 0 {
        io::write_str(2, b"microcom: cannot set stdin to raw mode\n");
        io::close(fd);
        return 1;
    }

    // Set non-blocking for select
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);

        let flags = libc::fcntl(0, libc::F_GETFL, 0);
        libc::fcntl(0, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    io::write_str(1, b"Connected to ");
    io::write_all(1, device);
    io::write_str(1, b" at ");
    let mut buf = [0u8; 16];
    io::write_all(1, sys::format_u64(speed as u64, &mut buf));
    io::write_str(1, b" baud\n");
    io::write_str(1, b"Press Ctrl+X to exit\n\n");

    let mut running = true;

    while running {
        let mut read_fds: libc::fd_set = unsafe { core::mem::zeroed() };
        unsafe {
            libc::FD_ZERO(&mut read_fds);
            libc::FD_SET(0, &mut read_fds);
            libc::FD_SET(fd, &mut read_fds);
        }

        let mut timeout = libc::timeval {
            tv_sec: 1,
            tv_usec: 0,
        };

        let nfds = if fd > 0 { fd + 1 } else { 1 };
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

        // Data from serial port -> stdout
        if unsafe { libc::FD_ISSET(fd, &read_fds) } {
            let mut rbuf = [0u8; 256];
            let n = io::read(fd, &mut rbuf);
            if n > 0 {
                io::write_all(1, &rbuf[..n as usize]);
            }
        }

        // Data from stdin -> serial port
        if unsafe { libc::FD_ISSET(0, &read_fds) } {
            let mut rbuf = [0u8; 256];
            let n = io::read(0, &mut rbuf);
            if n > 0 {
                for j in 0..n as usize {
                    if rbuf[j] == 0x18 {
                        // Ctrl+X
                        running = false;
                        break;
                    }
                }
                if running {
                    io::write_all(fd, &rbuf[..n as usize]);
                }
            }
        }
    }

    // Restore terminal
    unsafe {
        libc::tcsetattr(0, libc::TCSAFLUSH, &orig_termios);
    }
    io::close(fd);

    io::write_str(1, b"\nDisconnected\n");
    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn microcom(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"microcom: only available on Linux\n");
    1
}

fn print_usage() {
    io::write_str(1, b"Usage: microcom [-s SPEED] TTY\n\n");
    io::write_str(1, b"Simple serial terminal emulator.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -s SPEED   Set baud rate (default: 115200)\n");
    io::write_str(1, b"  -h         Show this help\n\n");
    io::write_str(1, b"Press Ctrl+X to exit.\n");
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_microcom_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["microcom", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_microcom_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["microcom"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing TTY"));
    }
}
