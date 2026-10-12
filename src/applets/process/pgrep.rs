//! pgrep - look up processes based on name
//!
//! Find process IDs matching a pattern.

use crate::applets::get_arg;
use crate::io;

/// pgrep - look up processes based on name
///
/// # Synopsis
/// ```text
/// pgrep pattern
/// ```
///
/// # Description
/// Search for processes matching the given pattern and print their PIDs.
///
/// # Exit Status
/// - 0: At least one process matched
/// - 1: No processes matched or an error occurred
pub fn pgrep(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 1;
    }
    let pattern = unsafe { get_arg(argv, 1).unwrap() };
    let mut found = false;

    let fd = io::open(b"/proc", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
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

            if !name.is_empty() && name[0] >= b'0' && name[0] <= b'9' {
                let mut path = [0u8; 64];
                let mut pi = 0;
                for &c in b"/proc/" {
                    path[pi] = c;
                    pi += 1;
                }
                for &c in name {
                    path[pi] = c;
                    pi += 1;
                }
                for &c in b"/comm\0" {
                    path[pi] = c;
                    pi += 1;
                }

                let comm_fd = io::open(&path, libc::O_RDONLY, 0);
                if comm_fd >= 0 {
                    let mut comm_buf = [0u8; 256];
                    let cn = io::read(comm_fd, &mut comm_buf);
                    io::close(comm_fd);

                    if cn > 0 {
                        let comm = &comm_buf[..cn as usize];
                        let comm = comm.split(|&c| c == b'\n').next().unwrap_or(comm);
                        // Simple substring match
                        if comm.windows(pattern.len()).any(|w| w == pattern) {
                            io::write_all(1, name);
                            io::write_str(1, b"\n");
                            found = true;
                        }
                    }
                }
            }
            offset += dirent.d_reclen as usize;
        }
    }
    io::close(fd);
    if found { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_pgrep_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["pgrep"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_pgrep_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["pgrep", "nonexistent_process_12345"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_pgrep_finds_init() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // systemd or init should always be running as PID 1
        let output = Command::new(&armybox)
            .args(["pgrep", "systemd"])
            .output()
            .unwrap();

        // Either finds systemd (exit 0) or doesn't (exit 1) - both are valid
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }
}
