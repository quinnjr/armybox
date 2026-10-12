//! chgrp - change group ownership
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/chgrp.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// chgrp - change group ownership
///
/// # Synopsis
/// ```text
/// chgrp [-hR] group file...
/// ```
///
/// # Description
/// Changes the group ownership of each given file.
///
/// # Options
/// - `-h`: Affect symlinks instead of referenced file
/// - `-R`: Recursive - change files and directories recursively
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn chgrp(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"chgrp: missing operand\n");
        return 1;
    }

    let mut recursive = false;
    let mut no_deref = false;
    let mut group_idx = 1;

    // Parse options
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"--" {
                group_idx = i + 1;
                break;
            } else if arg.len() > 1 && arg[0] == b'-' && arg[1].is_ascii_alphabetic() {
                for &c in &arg[1..] {
                    match c {
                        b'R' => recursive = true,
                        b'h' => no_deref = true,
                        _ => {}
                    }
                }
                group_idx = i + 1;
            } else {
                group_idx = i;
                break;
            }
        }
    }

    let group_str = match unsafe { get_arg(argv, group_idx) } {
        Some(g) => g,
        None => {
            io::write_str(2, b"chgrp: missing group\n");
            return 1;
        }
    };

    // Parse group - try numeric first, then name lookup
    let gid = if let Some(n) = sys::parse_u64(group_str) {
        n as u32
    } else {
        match lookup_gid(group_str) {
            Some(g) => g,
            None => {
                io::write_str(2, b"chgrp: invalid group: '");
                io::write_all(2, group_str);
                io::write_str(2, b"'\n");
                return 1;
            }
        }
    };

    let files_start = group_idx + 1;
    if files_start >= argc {
        io::write_str(2, b"chgrp: missing operand\n");
        return 1;
    }

    let mut exit_code = 0;
    for i in files_start..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if chgrp_path(path, gid, no_deref, recursive) != 0 {
                exit_code = 1;
            }
        }
    }
    exit_code
}

/// Look up a GID by group name using getgrnam
fn lookup_gid(name: &[u8]) -> Option<u32> {
    let mut buf = [0u8; 256];
    let len = core::cmp::min(name.len(), buf.len() - 1);
    buf[..len].copy_from_slice(&name[..len]);
    buf[len] = 0;

    let gr = unsafe { libc::getgrnam(buf.as_ptr() as *const libc::c_char) };
    if gr.is_null() {
        None
    } else {
        Some(unsafe { (*gr).gr_gid })
    }
}

/// Apply chgrp to a path, optionally recursing
fn chgrp_path(path: &[u8], gid: u32, no_deref: bool, recursive: bool) -> i32 {
    let ret = if no_deref {
        let mut buf = [0u8; 4096];
        let len = core::cmp::min(path.len(), buf.len() - 1);
        buf[..len].copy_from_slice(&path[..len]);
        unsafe { libc::lchown(buf.as_ptr() as *const libc::c_char, u32::MAX, gid) }
    } else {
        unsafe { libc::chown(path.as_ptr() as *const libc::c_char, u32::MAX, gid) }
    };

    if ret < 0 {
        sys::perror(path);
        return 1;
    }

    if recursive {
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::stat(path, &mut st) == 0
            && (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32
        {
            return chgrp_recursive(path, gid, no_deref);
        }
    }

    0
}

/// Recursively chgrp a directory's contents
fn chgrp_recursive(dir: &[u8], gid: u32, no_deref: bool) -> i32 {
    let fd = io::open(dir, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        sys::perror(dir);
        return 1;
    }

    let mut exit_code = 0;
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
                let mut full_path = [0u8; 4096];
                let mut len = 0;
                for &c in dir {
                    if len < full_path.len() - 1 {
                        full_path[len] = c;
                        len += 1;
                    }
                }
                if len < full_path.len() - 1 {
                    full_path[len] = b'/';
                    len += 1;
                }
                for &c in name {
                    if len < full_path.len() - 1 {
                        full_path[len] = c;
                        len += 1;
                    }
                }

                if chgrp_path(&full_path[..len], gid, no_deref, true) != 0 {
                    exit_code = 1;
                }
            }

            offset += dirent.d_reclen as usize;
        }
    }

    io::close(fd);
    exit_code
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_chgrp_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_chgrp_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chgrp"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_chgrp_nonexistent_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["chgrp", "1000", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_chgrp_no_permission() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Skip if root
        if std::env::var("USER").map(|u| u == "root").unwrap_or(false) {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        // Try to change group to root group (should fail for non-root)
        let output = Command::new(&armybox)
            .args(["chgrp", "0", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_chgrp_invalid_group() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["chgrp", "nonexistent_group_12345", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
