//! ulimit - get and set user resource limits
//!
//! Display or modify shell resource limits.

use super::get_arg;
use crate::io;
use crate::sys;

/// ulimit - get and set user resource limits
///
/// # Synopsis
/// ```text
/// ulimit [-SHacdfelmqrstuv] [limit]
/// ```
///
/// # Description
/// Display or modify shell resource limits.
///
/// # Options
/// - `-S`: Use soft limit
/// - `-H`: Use hard limit
/// - `-a`: Report all limits
/// - `-c`: Core file size
/// - `-d`: Data segment size
/// - `-f`: File size
/// - `-l`: Locked memory size
/// - `-m`: Resident set size
/// - `-n`: Number of open files
/// - `-s`: Stack size
/// - `-t`: CPU time
/// - `-u`: Number of processes
/// - `-v`: Virtual memory size
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn ulimit(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_all = false;
    let mut use_hard = false;
    let mut resource = libc::RLIMIT_FSIZE; // Default: file size
    let mut resource_name = b"file size" as &[u8];
    let mut divisor: u64 = 512; // Default: 512-byte blocks

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() >= 2 && arg[0] == b'-' {
                for &c in &arg[1..] {
                    match c {
                        b'a' => show_all = true,
                        b'H' => use_hard = true,
                        b'S' => use_hard = false,
                        b'c' => {
                            resource = libc::RLIMIT_CORE;
                            resource_name = b"core file size";
                            divisor = 512;
                        }
                        b'd' => {
                            resource = libc::RLIMIT_DATA;
                            resource_name = b"data seg size";
                            divisor = 1024;
                        }
                        b'f' => {
                            resource = libc::RLIMIT_FSIZE;
                            resource_name = b"file size";
                            divisor = 512;
                        }
                        b'l' => {
                            resource = libc::RLIMIT_MEMLOCK;
                            resource_name = b"locked memory";
                            divisor = 1024;
                        }
                        b'm' => {
                            resource = libc::RLIMIT_RSS;
                            resource_name = b"resident set";
                            divisor = 1024;
                        }
                        b'n' => {
                            resource = libc::RLIMIT_NOFILE;
                            resource_name = b"open files";
                            divisor = 1;
                        }
                        b's' => {
                            resource = libc::RLIMIT_STACK;
                            resource_name = b"stack size";
                            divisor = 1024;
                        }
                        b't' => {
                            resource = libc::RLIMIT_CPU;
                            resource_name = b"cpu time";
                            divisor = 1;
                        }
                        b'u' => {
                            resource = libc::RLIMIT_NPROC;
                            resource_name = b"max processes";
                            divisor = 1;
                        }
                        b'v' => {
                            resource = libc::RLIMIT_AS;
                            resource_name = b"virtual memory";
                            divisor = 1024;
                        }
                        _ => {}
                    }
                }
            }
        }
        i += 1;
    }

    if show_all {
        print_all_limits(use_hard);
        return 0;
    }

    // Get current limit
    let mut rlim: libc::rlimit = unsafe { core::mem::zeroed() };
    if unsafe { libc::getrlimit(resource as _, &mut rlim) } != 0 {
        sys::perror(resource_name);
        return 1;
    }

    let value = if use_hard {
        rlim.rlim_max
    } else {
        rlim.rlim_cur
    };

    if value == libc::RLIM_INFINITY {
        io::write_str(1, b"unlimited\n");
    } else {
        io::write_num(1, value as u64 / divisor);
        io::write_str(1, b"\n");
    }

    0
}

fn print_all_limits(use_hard: bool) {
    let limits = [
        (
            libc::RLIMIT_CORE,
            b"core file size          (blocks, -c) " as &[u8],
            512u64,
        ),
        (
            libc::RLIMIT_DATA,
            b"data seg size           (kbytes, -d) ",
            1024,
        ),
        (
            libc::RLIMIT_FSIZE,
            b"file size               (blocks, -f) ",
            512,
        ),
        (
            libc::RLIMIT_MEMLOCK,
            b"max locked memory       (kbytes, -l) ",
            1024,
        ),
        (
            libc::RLIMIT_RSS,
            b"max resident set size   (kbytes, -m) ",
            1024,
        ),
        (
            libc::RLIMIT_NOFILE,
            b"open files                      (-n) ",
            1,
        ),
        (
            libc::RLIMIT_STACK,
            b"stack size              (kbytes, -s) ",
            1024,
        ),
        (
            libc::RLIMIT_CPU,
            b"cpu time                (seconds, -t) ",
            1,
        ),
        (
            libc::RLIMIT_NPROC,
            b"max user processes              (-u) ",
            1,
        ),
        (
            libc::RLIMIT_AS,
            b"virtual memory          (kbytes, -v) ",
            1024,
        ),
    ];

    for (resource, name, divisor) in limits.iter() {
        io::write_all(1, name);

        let mut rlim: libc::rlimit = unsafe { core::mem::zeroed() };
        if unsafe { libc::getrlimit(*resource as _, &mut rlim) } == 0 {
            let value = if use_hard {
                rlim.rlim_max
            } else {
                rlim.rlim_cur
            };
            if value == libc::RLIM_INFINITY {
                io::write_str(1, b"unlimited\n");
            } else {
                io::write_num(1, value as u64 / divisor);
                io::write_str(1, b"\n");
            }
        } else {
            io::write_str(1, b"error\n");
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_ulimit_default() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ulimit"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should output a number or "unlimited"
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty());
    }

    #[test]
    fn test_ulimit_all() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ulimit", "-a"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("core file size"));
        assert!(stdout.contains("open files"));
    }

    #[test]
    fn test_ulimit_open_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ulimit", "-n"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should be a number (e.g., 1024) or unlimited
        assert!(stdout.trim().parse::<u64>().is_ok() || stdout.contains("unlimited"));
    }

    #[test]
    fn test_ulimit_hard_limit() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ulimit", "-Hn"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
