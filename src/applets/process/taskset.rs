//! taskset - set or retrieve a process's CPU affinity
//!
//! Set or retrieve a process's CPU affinity.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// Parse a hexadecimal string to u64
fn parse_hex(s: &[u8]) -> Option<u64> {
    if s.is_empty() {
        return None;
    }

    let mut val: u64 = 0;
    let mut start = 0;

    // Skip 0x prefix if present
    if s.len() > 2 && s[0] == b'0' && (s[1] == b'x' || s[1] == b'X') {
        start = 2;
    }

    for &c in &s[start..] {
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        val = val.checked_mul(16)?.checked_add(digit as u64)?;
    }

    Some(val)
}

/// taskset - set or retrieve a process's CPU affinity
///
/// # Synopsis
/// ```text
/// taskset [-p] [mask] pid
/// taskset mask command [args]
/// ```
///
/// # Description
/// Set or retrieve a process's CPU affinity.
///
/// # Options
/// - `-p`: Operate on an existing PID
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn taskset(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"taskset: usage: taskset [-p] [mask] pid\n");
        return 1;
    }

    let mut idx: i32 = 1;
    let mut show_only = false;

    // Check for -p flag
    if let Some(arg) = unsafe { get_arg(argv, 1) } {
        if arg == b"-p" {
            show_only = true;
            idx = 2;
        }
    }

    if idx >= argc {
        io::write_str(2, b"taskset: missing pid\n");
        return 1;
    }

    // Get PID (last argument)
    let pid_str = unsafe { get_arg(argv, argc - 1).unwrap() };
    let pid = sys::parse_i64(pid_str).unwrap_or(0) as i32;

    if pid <= 0 {
        io::write_str(2, b"taskset: invalid pid\n");
        return 1;
    }

    // Get current affinity
    let mut mask: libc::cpu_set_t = unsafe { core::mem::zeroed() };

    if unsafe { libc::sched_getaffinity(pid, core::mem::size_of::<libc::cpu_set_t>(), &mut mask) }
        != 0
    {
        sys::perror(b"sched_getaffinity");
        return 1;
    }

    if show_only || idx == argc - 1 {
        // Just show current affinity
        io::write_str(1, b"pid ");
        io::write_signed(1, pid as i64);
        io::write_str(1, b"'s current affinity mask: ");

        // Convert CPU set to hex mask
        let mut affinity: u64 = 0;
        for i in 0..64 {
            if unsafe { libc::CPU_ISSET(i, &mask) } {
                affinity |= 1 << i;
            }
        }

        // Format as hex
        let mut hex_buf = [0u8; 20];
        let hex_str = sys::format_hex(affinity, &mut hex_buf);
        io::write_all(1, hex_str);
        io::write_str(1, b"\n");
        return 0;
    }

    // Set new affinity
    let mask_str = unsafe { get_arg(argv, idx).unwrap() };
    let new_mask = parse_hex(mask_str).unwrap_or(0xffffffff);

    unsafe { libc::CPU_ZERO(&mut mask) };
    for i in 0..64 {
        if new_mask & (1 << i) != 0 {
            unsafe { libc::CPU_SET(i, &mut mask) };
        }
    }

    if unsafe { libc::sched_setaffinity(pid, core::mem::size_of::<libc::cpu_set_t>(), &mask) } != 0
    {
        sys::perror(b"sched_setaffinity");
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
    fn test_taskset_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["taskset"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_taskset_show_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["taskset", "-p", &pid.to_string()])
            .output()
            .unwrap();

        // Should succeed and show affinity mask
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("affinity mask"));
    }
}
