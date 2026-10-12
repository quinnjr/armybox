//! ifconfig - configure network interfaces
//!
//! Display or configure network interface parameters.

use super::get_arg;
use crate::io;

/// ifconfig - configure network interfaces
///
/// # Synopsis
/// ```text
/// ifconfig [INTERFACE]
/// ```
///
/// # Description
/// Display information about network interfaces. If INTERFACE
/// is specified, show only that interface.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ifconfig(argc: i32, argv: *const *const u8) -> i32 {
    // Read from /sys/class/net
    let fd = io::open(b"/sys/class/net", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        io::write_str(2, b"ifconfig: cannot read interfaces\n");
        return 1;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        let mut offset = 0;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };

            if name != b"." && name != b".." {
                // If specific interface requested, filter
                if argc > 1 {
                    let req = unsafe { get_arg(argv, 1).unwrap() };
                    if name != req {
                        offset += dirent.d_reclen as usize;
                        continue;
                    }
                }

                io::write_all(1, name);
                io::write_str(1, b": ");

                // Read flags
                let mut path = [0u8; 128];
                let mut pi = 0;
                for &c in b"/sys/class/net/" {
                    path[pi] = c;
                    pi += 1;
                }
                for &c in name {
                    path[pi] = c;
                    pi += 1;
                }

                let path_base = pi;

                // Read operstate
                for &c in b"/operstate\0" {
                    path[pi] = c;
                    pi += 1;
                }
                let state_fd = io::open(&path, libc::O_RDONLY, 0);
                if state_fd >= 0 {
                    let mut state_buf = [0u8; 32];
                    let sn = io::read(state_fd, &mut state_buf);
                    io::close(state_fd);
                    if sn > 0 {
                        let state = &state_buf[..sn as usize];
                        let state = state.split(|&c| c == b'\n').next().unwrap_or(state);
                        io::write_str(1, b"flags=<");
                        if state == b"up" {
                            io::write_str(1, b"UP,RUNNING");
                        } else {
                            io::write_str(1, b"DOWN");
                        }
                        io::write_str(1, b">\n");
                    }
                }

                // Read address
                pi = path_base;
                for &c in b"/address\0" {
                    path[pi] = c;
                    pi += 1;
                }
                let addr_fd = io::open(&path, libc::O_RDONLY, 0);
                if addr_fd >= 0 {
                    let mut addr_buf = [0u8; 64];
                    let an = io::read(addr_fd, &mut addr_buf);
                    io::close(addr_fd);
                    if an > 0 {
                        io::write_str(1, b"        ether ");
                        io::write_all(1, &addr_buf[..an as usize - 1]); // Remove newline
                        io::write_str(1, b"\n");
                    }
                }

                // Read MTU
                pi = path_base;
                for &c in b"/mtu\0" {
                    path[pi] = c;
                    pi += 1;
                }
                let mtu_fd = io::open(&path, libc::O_RDONLY, 0);
                if mtu_fd >= 0 {
                    let mut mtu_buf = [0u8; 16];
                    let mn = io::read(mtu_fd, &mut mtu_buf);
                    io::close(mtu_fd);
                    if mn > 0 {
                        io::write_str(1, b"        mtu ");
                        io::write_all(1, &mtu_buf[..mn as usize - 1]);
                        io::write_str(1, b"\n");
                    }
                }

                io::write_str(1, b"\n");
            }
            offset += dirent.d_reclen as usize;
        }
    }
    io::close(fd);
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
    fn test_ifconfig_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ifconfig"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should show at least lo interface
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("lo") || stdout.contains("eth") || stdout.contains("en"));
    }

    #[test]
    fn test_ifconfig_lo() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ifconfig", "lo"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
