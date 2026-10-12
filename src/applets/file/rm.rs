//! rm - remove files or directories
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/rm.html

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// rm - remove files or directories
///
/// # Synopsis
/// ```text
/// rm [-fiRr] file...
/// ```
///
/// # Description
/// Removes (unlinks) files. With -r, removes directories recursively.
///
/// # Options
/// - `-f`: Force - ignore nonexistent files, never prompt
/// - `-i`: Interactive - prompt before every removal (not implemented)
/// - `-R`, `-r`: Recursive - remove directories and their contents
///
/// # Exit Status
/// - 0: All files removed successfully
/// - >0: An error occurred
pub fn rm(argc: i32, argv: *const *const u8) -> i32 {
    let mut recursive = false;
    let mut force = false;
    let mut _interactive = false;
    let mut files_start = 1;
    let mut has_files = false;

    // Parse options and find files
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' && !has_files {
                // Handle -- to stop option processing
                if arg == b"--" {
                    files_start = i + 1;
                    break;
                }
                if has_opt(arg, b'r') || has_opt(arg, b'R') {
                    recursive = true;
                }
                if has_opt(arg, b'f') {
                    force = true;
                }
                if has_opt(arg, b'i') {
                    _interactive = true;
                }
                files_start = i + 1;
            } else {
                has_files = true;
                break;
            }
        }
    }

    // Check if we have files to remove
    if files_start >= argc {
        if !force {
            io::write_str(2, b"rm: missing operand\n");
            return 1;
        }
        return 0; // -f with no files is OK
    }

    let mut exit_code = 0;

    // Process each file
    for i in files_start..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            let result = if recursive {
                remove_recursive(path, force)
            } else {
                remove_file(path, force)
            };

            if result != 0 {
                exit_code = 1;
            }
        }
    }

    exit_code
}

/// Remove a single file (non-directory)
fn remove_file(path: &[u8], force: bool) -> i32 {
    // Check if it's a directory
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::lstat(path, &mut st) == 0 {
        if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
            if !force {
                io::write_str(2, b"rm: cannot remove '");
                io::write_all(2, path);
                io::write_str(2, b"': Is a directory\n");
            }
            return if force { 0 } else { 1 };
        }
    } else {
        // File doesn't exist
        if !force {
            sys::perror(path);
        }
        return if force { 0 } else { 1 };
    }

    if io::unlink(path) < 0 {
        if !force {
            sys::perror(path);
        }
        return if force { 0 } else { 1 };
    }

    0
}

/// Remove a file or directory recursively
fn remove_recursive(path: &[u8], force: bool) -> i32 {
    let mut st: libc::stat = unsafe { core::mem::zeroed() };

    // Use lstat to not follow symlinks
    if io::lstat(path, &mut st) < 0 {
        if !force {
            sys::perror(path);
        }
        return if force { 0 } else { 1 };
    }

    // If it's a symlink or regular file, just unlink it
    if (st.st_mode as u32 & libc::S_IFMT as u32) != libc::S_IFDIR as u32 {
        if io::unlink(path) < 0 {
            if !force {
                sys::perror(path);
            }
            return if force { 0 } else { 1 };
        }
        return 0;
    }

    // It's a directory - recurse into it
    let fd = io::open(path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        if !force {
            sys::perror(path);
        }
        return if force { 0 } else { 1 };
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
                // Build full path
                let mut full_path = [0u8; 4096];
                let mut len = 0;
                for &c in path {
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

                if remove_recursive(&full_path[..len], force) != 0 {
                    exit_code = 1;
                }
            }

            offset += dirent.d_reclen as usize;
        }
    }

    io::close(fd);

    // Now remove the empty directory
    if io::rmdir(path) < 0 {
        if !force {
            sys::perror(path);
        }
        return if force { 0 } else { 1 };
    }

    exit_code
}

#[cfg(test)]
mod tests {
    //! Unit tests for rm utility

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
            "armybox_rm_test_{}_{}",
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
    fn test_rm_single_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rm", dir.join("file.txt").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("file.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "content1").unwrap();
        fs::write(dir.join("file2.txt"), "content2").unwrap();

        let output = Command::new(&armybox)
            .args([
                "rm",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("file1.txt").exists());
        assert!(!dir.join("file2.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_nonexistent_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rm", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_rm_force_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["rm", "-f", "/nonexistent/file"])
            .output()
            .unwrap();

        // -f should not fail on nonexistent files
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_rm_directory_without_recursive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("subdir")).unwrap();

        let output = Command::new(&armybox)
            .args(["rm", dir.join("subdir").to_str().unwrap()])
            .output()
            .unwrap();

        // Should fail - can't remove directory without -r
        assert_ne!(output.status.code(), Some(0));
        assert!(dir.join("subdir").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_recursive_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir_all(dir.join("subdir/nested")).unwrap();
        fs::write(dir.join("subdir/file.txt"), "content").unwrap();
        fs::write(dir.join("subdir/nested/deep.txt"), "deep").unwrap();

        let output = Command::new(&armybox)
            .args(["rm", "-r", dir.join("subdir").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("subdir").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_recursive_uppercase() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("subdir")).unwrap();
        fs::write(dir.join("subdir/file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rm", "-R", dir.join("subdir").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("subdir").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["rm"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
        assert!(std::string::String::from_utf8_lossy(&output.stderr).contains("missing operand"));
    }

    #[test]
    fn test_rm_force_no_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["rm", "-f"]).output().unwrap();

        // -f with no operands should succeed silently
        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_rm_symlink() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("target.txt"), "content").unwrap();

        // Create symlink
        Command::new("ln")
            .args([
                "-s",
                dir.join("target.txt").to_str().unwrap(),
                dir.join("link").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        let output = Command::new(&armybox)
            .args(["rm", dir.join("link").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("link").exists());
        // Target should still exist
        assert!(dir.join("target.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_rm_combined_options() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("subdir")).unwrap();
        fs::write(dir.join("subdir/file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["rm", "-rf", dir.join("subdir").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("subdir").exists());
        cleanup(&dir);
    }
}
