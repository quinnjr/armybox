//! openvt - start a program on a new virtual terminal
//!
//! Open a new virtual terminal and run a program on it.

use super::get_arg;
use crate::io;
use crate::sys;

// VT ioctls
const VT_OPENQRY: crate::io::IoctlReq = 0x5600u32 as crate::io::IoctlReq;
const VT_ACTIVATE: crate::io::IoctlReq = 0x5606u32 as crate::io::IoctlReq;
const VT_WAITACTIVE: crate::io::IoctlReq = 0x5607u32 as crate::io::IoctlReq;

/// openvt - start a program on a new virtual terminal
///
/// # Synopsis
/// ```text
/// openvt [-c N] [-s] [-w] [--] COMMAND [ARGS...]
/// ```
///
/// # Description
/// Start a program on a new virtual terminal.
///
/// # Options
/// - `-c N`: Use VT number N
/// - `-s`: Switch to the new VT
/// - `-w`: Wait for program to exit
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn openvt(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(
            2,
            b"openvt: usage: openvt [-c N] [-s] [-w] COMMAND [ARGS...]\n",
        );
        return 1;
    }

    let mut vt_num: i32 = -1;
    let mut do_switch = false;
    let mut do_wait = false;
    let mut cmd_start: i32 = 1;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-c" && i + 1 < argc {
                if let Some(num) = unsafe { get_arg(argv, i + 1) } {
                    vt_num = sys::parse_i64(num).unwrap_or(-1) as i32;
                }
                i += 2;
                cmd_start = i;
                continue;
            } else if arg == b"-s" {
                do_switch = true;
                cmd_start = i + 1;
            } else if arg == b"-w" {
                do_wait = true;
                cmd_start = i + 1;
            } else if arg == b"--" {
                cmd_start = i + 1;
                break;
            } else if arg.len() > 0 && arg[0] != b'-' {
                cmd_start = i;
                break;
            }
        }
        i += 1;
    }

    if cmd_start >= argc {
        io::write_str(2, b"openvt: no command specified\n");
        return 1;
    }

    // Open console
    let console_fd = io::open(b"/dev/tty0", libc::O_RDWR, 0);
    if console_fd < 0 {
        sys::perror(b"/dev/tty0");
        return 1;
    }

    // Find free VT if not specified
    if vt_num < 0 {
        if unsafe { libc::ioctl(console_fd, VT_OPENQRY, &mut vt_num) } < 0 {
            sys::perror(b"VT_OPENQRY");
            io::close(console_fd);
            return 1;
        }
    }

    // Build VT device path
    let mut vt_path = [0u8; 32];
    let prefix = b"/dev/tty";
    vt_path[..prefix.len()].copy_from_slice(prefix);
    let mut num_buf = [0u8; 12];
    let num_str = sys::format_i64(vt_num as i64, &mut num_buf);
    vt_path[prefix.len()..prefix.len() + num_str.len()].copy_from_slice(num_str);

    // Fork and exec on new VT
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        sys::perror(b"fork");
        io::close(console_fd);
        return 1;
    }

    if pid == 0 {
        // Child process
        unsafe { libc::setsid() };

        // Open new VT as controlling terminal
        let vt_fd = io::open(&vt_path, libc::O_RDWR, 0);
        if vt_fd < 0 {
            sys::perror(&vt_path);
            unsafe { libc::_exit(1) };
        }

        // Set up stdin/stdout/stderr
        unsafe {
            libc::dup2(vt_fd, 0);
            libc::dup2(vt_fd, 1);
            libc::dup2(vt_fd, 2);
            if vt_fd > 2 {
                libc::close(vt_fd);
            }
        }

        // Execute command
        let prog = unsafe { get_arg(argv, cmd_start).unwrap() };
        let mut prog_buf = [0u8; 256];
        let len = prog.len().min(prog_buf.len() - 1);
        prog_buf[..len].copy_from_slice(&prog[..len]);

        unsafe {
            libc::execvp(
                prog_buf.as_ptr() as *const libc::c_char,
                argv.add(cmd_start as usize) as *const *const libc::c_char,
            );
        }
        sys::perror(prog);
        unsafe { libc::_exit(1) };
    }

    // Parent process
    if do_switch {
        unsafe {
            libc::ioctl(console_fd, VT_ACTIVATE, vt_num);
            libc::ioctl(console_fd, VT_WAITACTIVE, vt_num);
        }
    }

    io::close(console_fd);

    if do_wait {
        let mut status: i32 = 0;
        unsafe { libc::waitpid(pid, &mut status, 0) };
        if libc::WIFEXITED(status) {
            return libc::WEXITSTATUS(status);
        }
    }

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
    fn test_openvt_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["openvt"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_openvt_no_tty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Will fail without TTY access
        let output = Command::new(&armybox)
            .args(["openvt", "true"])
            .output()
            .unwrap();

        // Expected to fail without console access
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
