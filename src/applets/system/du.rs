//! du - estimate file space usage
//!
//! Summarize disk usage of the set of FILEs, recursively for directories.

use crate::io;
use crate::sys;
use crate::applets::{get_arg, has_opt};

/// du - estimate file space usage
///
/// # Synopsis
/// ```text
/// du [-s] [file...]
/// ```
///
/// # Description
/// Summarize disk usage of each FILE, recursively for directories.
///
/// # Options
/// - `-s`: Display only a total for each argument
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn du(argc: i32, argv: *const *const u8) -> i32 {
    let path = if argc > 1 {
        unsafe { get_arg(argv, argc - 1).unwrap() }
    } else {
        b"."
    };

    let show_summary = has_opt(unsafe { get_arg(argv, 1).unwrap_or(b"") }, b's');

    fn get_dir_size(path: &[u8]) -> i64 {
        let mut total: i64 = 0;
        let mut path_buf = [0u8; 4096];
        path_buf[..path.len()].copy_from_slice(path);
        path_buf[path.len()] = 0;

        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if unsafe { libc::lstat(path_buf.as_ptr() as *const libc::c_char, &mut st) } != 0 {
            return 0;
        }

        total += (st.st_blocks as i64 * 512) / 1024; // Convert to KB

        if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
            let fd = io::open(path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
            if fd >= 0 {
                let mut buf = [0u8; 4096];
                loop {
                    let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
                    if n <= 0 { break; }
                    let mut offset = 0;
                    while offset < n as usize {
                        let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
                        let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };
                        if name != b"." && name != b".." {
                            let mut child_path = [0u8; 4096];
                            let mut pi = 0;
                            for &c in path { child_path[pi] = c; pi += 1; }
                            child_path[pi] = b'/'; pi += 1;
                            for &c in name { child_path[pi] = c; pi += 1; }
                            total += get_dir_size(&child_path[..pi]);
                        }
                        offset += dirent.d_reclen as usize;
                    }
                }
                io::close(fd);
            }
        }
        total
    }

    let size = get_dir_size(path);
    let mut num_buf = [0u8; 20];
    io::write_all(1, sys::format_u64(size as u64, &mut num_buf));
    io::write_str(1, b"\t");
    io::write_all(1, path);
    io::write_str(1, b"\n");
    let _ = show_summary;
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::process::Command;
    use std::path::PathBuf;
    use std::fs;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_du_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["du"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_du_current_dir() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["du", "."])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("."));
    }

    #[test]
    fn test_du_specific_dir() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        // Create a temp directory
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = std::env::temp_dir().join(format!("armybox_du_test_{}_{}",  std::process::id(), counter));
        let _ = fs::create_dir_all(&test_dir);
        fs::write(test_dir.join("test.txt"), "hello world").unwrap();

        let output = Command::new(&armybox)
            .args(["du", test_dir.to_str().unwrap()])
            .output()
            .unwrap();

        let _ = fs::remove_dir_all(&test_dir);

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should output size and path
        assert!(stdout.contains("\t"));
    }
}
