//! prlimit - get and set process resource limits
//!
//! Get and set resource limits for a process.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// The kernel's `struct rlimit64`. `prlimit64` always writes 16 bytes, but
/// `libc::rlimit` is only 8 where `rlim_t` is 32-bit (e.g. 32-bit Bionic).
#[repr(C)]
struct Rlimit64 {
    rlim_cur: u64,
    rlim_max: u64,
}

const RLIM64_INFINITY: u64 = !0;

/// prlimit - get and set process resource limits
///
/// # Synopsis
/// ```text
/// prlimit [-p pid] [resource=value]
/// ```
///
/// # Description
/// Get and set resource limits. If no arguments are given,
/// displays all limits for the current process.
///
/// # Options
/// - `-p pid`: Operate on the specified process
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn prlimit(argc: i32, argv: *const *const u8) -> i32 {
    let mut pid: i32 = 0; // 0 means current process

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-p" && i + 1 < argc {
                if let Some(pid_str) = unsafe { get_arg(argv, i + 1) } {
                    pid = sys::parse_i64(pid_str).unwrap_or(0) as i32;
                }
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    // Display all limits
    let resources = [
        (libc::RLIMIT_AS, b"AS" as &[u8]),
        (libc::RLIMIT_CORE, b"CORE"),
        (libc::RLIMIT_CPU, b"CPU"),
        (libc::RLIMIT_DATA, b"DATA"),
        (libc::RLIMIT_FSIZE, b"FSIZE"),
        (libc::RLIMIT_NOFILE, b"NOFILE"),
        (libc::RLIMIT_STACK, b"STACK"),
        (libc::RLIMIT_NPROC, b"NPROC"),
    ];

    io::write_str(1, b"RESOURCE   SOFT       HARD\n");

    for (resource, name) in resources.iter() {
        let mut rlim = Rlimit64 {
            rlim_cur: 0,
            rlim_max: 0,
        };

        // prlimit64 with pid 0 targets the calling process
        let result = unsafe {
            libc::syscall(
                libc::SYS_prlimit64,
                pid,
                *resource,
                core::ptr::null::<Rlimit64>(),
                &mut rlim,
            ) as i32
        };

        if result != 0 {
            continue;
        }

        io::write_all(1, name);
        // Padding
        for _ in name.len()..11 {
            io::write_str(1, b" ");
        }

        if rlim.rlim_cur == RLIM64_INFINITY {
            io::write_str(1, b"unlimited  ");
        } else {
            io::write_num(1, rlim.rlim_cur);
            io::write_str(1, b" ");
        }

        if rlim.rlim_max == RLIM64_INFINITY {
            io::write_str(1, b"unlimited");
        } else {
            io::write_num(1, rlim.rlim_max);
        }

        io::write_str(1, b"\n");
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::AtomicUsize;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
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
    fn test_prlimit_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["prlimit"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("RESOURCE"));
    }

    #[test]
    fn test_prlimit_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["prlimit", "-p", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
