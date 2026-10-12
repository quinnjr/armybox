//! pmap - report memory map of a process
//!
//! Display the memory map of a process.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// pmap - report memory map of a process
///
/// # Synopsis
/// ```text
/// pmap pid
/// ```
///
/// # Description
/// Display the memory map of a process by reading /proc/pid/maps.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn pmap(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"pmap: missing pid\n");
        return 1;
    }

    let pid_str = unsafe { get_arg(argv, 1).unwrap() };
    let pid = sys::parse_i64(pid_str).unwrap_or(0);

    if pid <= 0 {
        io::write_str(2, b"pmap: invalid pid\n");
        return 1;
    }

    // Build path to /proc/pid/maps
    let mut path = [0u8; 64];
    let mut pi = 0;
    for &c in b"/proc/" {
        path[pi] = c;
        pi += 1;
    }

    let mut pid_buf = [0u8; 16];
    let pid_s = sys::format_i64(pid, &mut pid_buf);
    for &c in pid_s {
        path[pi] = c;
        pi += 1;
    }

    for &c in b"/maps" {
        path[pi] = c;
        pi += 1;
    }

    let fd = io::open(&path[..pi], libc::O_RDONLY, 0);
    if fd < 0 {
        sys::perror(&path[..pi]);
        return 1;
    }

    // Print header
    io::write_signed(1, pid);
    io::write_str(1, b":   ");

    // Get process name from /proc/pid/comm
    let mut comm_path = [0u8; 64];
    pi = 0;
    for &c in b"/proc/" {
        comm_path[pi] = c;
        pi += 1;
    }
    for &c in pid_s {
        comm_path[pi] = c;
        pi += 1;
    }
    for &c in b"/comm" {
        comm_path[pi] = c;
        pi += 1;
    }

    let comm_fd = io::open(&comm_path[..pi], libc::O_RDONLY, 0);
    if comm_fd >= 0 {
        let mut comm_buf = [0u8; 64];
        let n = io::read(comm_fd, &mut comm_buf);
        io::close(comm_fd);
        if n > 0 {
            let len = if comm_buf[n as usize - 1] == b'\n' {
                n as usize - 1
            } else {
                n as usize
            };
            io::write_all(1, &comm_buf[..len]);
        }
    }
    io::write_str(1, b"\n");

    // Read and display maps
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        io::write_all(1, &buf[..n as usize]);
    }

    io::close(fd);
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
    fn test_pmap_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["pmap"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing pid"));
    }

    #[test]
    fn test_pmap_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let output = Command::new(&armybox)
            .args(["pmap", &pid.to_string()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should show memory mappings
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.is_empty());
    }
}
