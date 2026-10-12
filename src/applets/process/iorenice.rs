//! iorenice - change I/O scheduling priority of a running process
//!
//! Change the I/O scheduling class and priority of a process.

use super::get_arg;
use crate::io;
use crate::sys;

// ioprio syscall numbers
const SYS_IOPRIO_GET: i64 = 252;
const SYS_IOPRIO_SET: i64 = 251;

// I/O priority classes
const IOPRIO_CLASS_RT: i32 = 1;
const IOPRIO_CLASS_BE: i32 = 2;
const IOPRIO_CLASS_IDLE: i32 = 3;

const IOPRIO_WHO_PROCESS: i32 = 1;

/// iorenice - change I/O scheduling priority
///
/// # Synopsis
/// ```text
/// iorenice PID [CLASS [PRIORITY]]
/// ```
///
/// # Description
/// Change the I/O scheduling class and priority of a running process.
///
/// # Classes
/// - 1 or rt: Real-time
/// - 2 or be: Best-effort (default)
/// - 3 or idle: Idle
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iorenice(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"iorenice: usage: iorenice PID [CLASS [PRIORITY]]\n");
        return 1;
    }

    let pid = if let Some(arg) = unsafe { get_arg(argv, 1) } {
        sys::parse_i64(arg).unwrap_or(0) as i32
    } else {
        0
    };

    if pid <= 0 {
        io::write_str(2, b"iorenice: invalid PID\n");
        return 1;
    }

    if argc < 3 {
        // Just show current priority
        let ioprio =
            unsafe { libc::syscall(SYS_IOPRIO_GET as libc::c_long, IOPRIO_WHO_PROCESS, pid) }
                as i32;

        if ioprio < 0 {
            sys::perror(b"ioprio_get");
            return 1;
        }

        let class = (ioprio >> 13) & 0x3;
        let data = ioprio & 0x1fff;

        io::write_str(1, b"class: ");
        match class {
            1 => io::write_str(1, b"realtime"),
            2 => io::write_str(1, b"best-effort"),
            3 => io::write_str(1, b"idle"),
            _ => io::write_str(1, b"none"),
        };
        io::write_str(1, b", priority: ");
        io::write_num(1, data as u64);
        io::write_str(1, b"\n");

        return 0;
    }

    // Parse class
    let class = if let Some(arg) = unsafe { get_arg(argv, 2) } {
        match arg {
            b"rt" | b"1" => IOPRIO_CLASS_RT,
            b"be" | b"2" => IOPRIO_CLASS_BE,
            b"idle" | b"3" => IOPRIO_CLASS_IDLE,
            _ => {
                io::write_str(2, b"iorenice: invalid class\n");
                return 1;
            }
        }
    } else {
        IOPRIO_CLASS_BE
    };

    // Parse priority (0-7)
    let priority = if argc > 3 {
        if let Some(arg) = unsafe { get_arg(argv, 3) } {
            sys::parse_i64(arg).unwrap_or(4) as i32
        } else {
            4
        }
    } else {
        4
    };

    let ioprio = (class << 13) | (priority & 0x7);

    let result = unsafe {
        libc::syscall(
            SYS_IOPRIO_SET as libc::c_long,
            IOPRIO_WHO_PROCESS,
            pid,
            ioprio,
        )
    };

    if result < 0 {
        sys::perror(b"ioprio_set");
        return 1;
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
    fn test_iorenice_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["iorenice"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_iorenice_show_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["iorenice", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("class:"));
    }
}
