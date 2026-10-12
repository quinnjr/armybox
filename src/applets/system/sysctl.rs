//! sysctl - configure kernel parameters at runtime
//!
//! Read and write kernel parameters.

use crate::applets::get_arg;
use crate::io;

/// sysctl - configure kernel parameters at runtime
///
/// # Synopsis
/// ```text
/// sysctl [parameter]
/// ```
///
/// # Description
/// Read kernel parameters from /proc/sys. Without arguments, shows
/// the hostname.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn sysctl(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        // List all sysctl values - just show a few common ones
        io::write_str(1, b"kernel.hostname = ");
        let mut buf = [0u8; 256];
        if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
            io::write_all(1, unsafe { io::cstr_to_slice(buf.as_ptr()) });
        }
        io::write_str(1, b"\n");
        return 0;
    }

    let param = unsafe { get_arg(argv, 1).unwrap() };

    // Convert sysctl name to /proc/sys path
    let mut path = [0u8; 256];
    let mut pi = 0;
    for &c in b"/proc/sys/" {
        path[pi] = c;
        pi += 1;
    }
    for &c in param {
        if c == b'.' {
            path[pi] = b'/';
        } else {
            path[pi] = c;
        }
        pi += 1;
    }

    let fd = io::open(&path[..pi], libc::O_RDONLY, 0);
    if fd >= 0 {
        io::write_all(1, param);
        io::write_str(1, b" = ");
        let mut buf = [0u8; 4096];
        let n = io::read(fd, &mut buf);
        if n > 0 {
            io::write_all(1, &buf[..n as usize]);
        }
        io::close(fd);
        0
    } else {
        1
    }
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
    fn test_sysctl_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["sysctl"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("kernel.hostname"));
    }

    #[test]
    fn test_sysctl_hostname() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["sysctl", "kernel.hostname"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("kernel.hostname"));
    }
}
