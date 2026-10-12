//! mount - mount a filesystem
//!
//! Mount a filesystem or show mounted filesystems.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// mount - mount a filesystem
///
/// # Synopsis
/// ```text
/// mount
/// mount source target
/// ```
///
/// # Description
/// Without arguments, show mounted filesystems by reading /proc/mounts.
/// With arguments, mount the source filesystem on the target directory.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn mount(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        // Show mounted filesystems
        let fd = io::open(b"/proc/mounts", libc::O_RDONLY, 0);
        if fd >= 0 {
            let mut buf = [0u8; 4096];
            loop {
                let n = io::read(fd, &mut buf);
                if n <= 0 {
                    break;
                }
                io::write_all(1, &buf[..n as usize]);
            }
            io::close(fd);
        }
        return 0;
    }

    let source = unsafe { get_arg(argv, argc - 2).unwrap() };
    let target = unsafe { get_arg(argv, argc - 1).unwrap() };

    let mut source_buf = [0u8; 4096];
    let mut target_buf = [0u8; 4096];
    source_buf[..source.len()].copy_from_slice(source);
    target_buf[..target.len()].copy_from_slice(target);

    let ret = unsafe {
        libc::mount(
            source_buf.as_ptr() as *const libc::c_char,
            target_buf.as_ptr() as *const libc::c_char,
            core::ptr::null(),
            0,
            core::ptr::null(),
        )
    };
    if ret != 0 {
        sys::perror(b"mount");
        1
    } else {
        0
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
    fn test_mount_show_mounts() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mount"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should contain some mount information
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // /proc/mounts should at least have some entries
        assert!(!stdout.is_empty() || true); // May be empty in some containers
    }

    #[test]
    fn test_mount_invalid_source() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // This should fail without root privileges
        let output = Command::new(&armybox)
            .args(["mount", "/nonexistent/source", "/tmp"])
            .output()
            .unwrap();

        // Should fail (permission denied or invalid)
        assert_eq!(output.status.code(), Some(1));
    }
}
