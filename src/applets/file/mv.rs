//! mv - move (rename) files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mv.html

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// mv - move (rename) files
///
/// # Synopsis
/// ```text
/// mv [-fi] source_file target_file
/// mv [-fi] source_file... target_dir
/// ```
///
/// # Description
/// Moves files or directories. First tries rename(2), falls back to
/// copy and delete if rename fails (e.g., across filesystems).
///
/// # Options
/// - `-f`: Force - do not prompt for confirmation
/// - `-i`: Interactive - prompt before overwrite (not implemented)
///
/// # Exit Status
/// - 0: All files moved successfully
/// - >0: An error occurred
pub fn mv(argc: i32, argv: *const *const u8) -> i32 {
    let mut force = false;
    let mut _interactive = false;
    let mut files_start = 1;

    // Parse options
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'f') {
                    force = true;
                }
                if has_opt(arg, b'i') {
                    _interactive = true;
                }
                files_start = i + 1;
            } else {
                break;
            }
        }
    }

    let file_count = argc - files_start;
    if file_count < 2 {
        io::write_str(2, b"mv: missing operand\n");
        return 1;
    }

    let dest = match unsafe { get_arg(argv, argc - 1) } {
        Some(d) => d,
        None => {
            io::write_str(2, b"mv: missing destination\n");
            return 1;
        }
    };

    // Check if destination is a directory
    let dest_is_dir = is_directory(dest);

    // If multiple sources, destination must be a directory
    if file_count > 2 && !dest_is_dir {
        io::write_str(2, b"mv: target '");
        io::write_all(2, dest);
        io::write_str(2, b"' is not a directory\n");
        return 1;
    }

    let mut exit_code = 0;

    for i in files_start..(argc - 1) {
        if let Some(src) = unsafe { get_arg(argv, i) } {
            let result = if dest_is_dir {
                // Move into directory
                let mut dest_path = [0u8; 4096];
                let dest_len = build_dest_path(dest, src, &mut dest_path);
                move_item(src, &dest_path[..dest_len], force)
            } else {
                move_item(src, dest, force)
            };

            if result != 0 {
                exit_code = 1;
            }
        }
    }

    exit_code
}

/// Check if path is a directory
fn is_directory(path: &[u8]) -> bool {
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(path, &mut st) < 0 {
        return false;
    }
    (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32
}

/// Build destination path by appending basename of source to dest directory
fn build_dest_path(dest_dir: &[u8], src: &[u8], buf: &mut [u8]) -> usize {
    let mut len = 0;

    // Copy destination directory
    for &c in dest_dir {
        if len < buf.len() - 1 {
            buf[len] = c;
            len += 1;
        }
    }

    // Add separator if needed
    if len > 0 && buf[len - 1] != b'/' {
        if len < buf.len() - 1 {
            buf[len] = b'/';
            len += 1;
        }
    }

    // Find basename of source
    let basename_start = src
        .iter()
        .rposition(|&c| c == b'/')
        .map(|p| p + 1)
        .unwrap_or(0);

    // Append basename
    for &c in &src[basename_start..] {
        if len < buf.len() - 1 {
            buf[len] = c;
            len += 1;
        }
    }

    len
}

/// Move a file or directory
fn move_item(src: &[u8], dest: &[u8], force: bool) -> i32 {
    // If force, try to remove destination first
    if force {
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::lstat(dest, &mut st) == 0 {
            if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
                // Can't force-remove a directory this way
            } else {
                let _ = io::unlink(dest);
            }
        }
    }

    // Try rename first (works within same filesystem)
    if io::rename(src, dest) == 0 {
        return 0;
    }

    // If rename failed, fall back to copy + remove
    // This handles cross-filesystem moves
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(src, &mut st) < 0 {
        sys::perror(src);
        return 1;
    }

    if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
        // For directories, we need recursive copy then remove
        if copy_directory(src, dest) != 0 {
            return 1;
        }
        remove_recursive(src);
    } else {
        // For regular files, simple copy then remove
        if copy_file(src, dest) != 0 {
            return 1;
        }
        if io::unlink(src) < 0 {
            sys::perror(src);
            return 1;
        }
    }

    0
}

/// Copy a single file
fn copy_file(src: &[u8], dest: &[u8]) -> i32 {
    let src_fd = io::open(src, libc::O_RDONLY, 0);
    if src_fd < 0 {
        sys::perror(src);
        return 1;
    }

    let dest_fd = io::open(dest, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if dest_fd < 0 {
        io::close(src_fd);
        sys::perror(dest);
        return 1;
    }

    let mut buf = [0u8; 4096];
    let mut copy_failed = false;
    loop {
        let n = io::read(src_fd, &mut buf);
        if n < 0 {
            sys::perror(src);
            copy_failed = true;
            break;
        }
        if n == 0 {
            break;
        }
        if io::write_all(dest_fd, &buf[..n as usize]) < 0 {
            sys::perror(dest);
            copy_failed = true;
            break;
        }
    }

    io::close(src_fd);
    if io::close(dest_fd) < 0 && !copy_failed {
        sys::perror(dest);
        copy_failed = true;
    }

    if copy_failed {
        io::write_str(2, b"mv: failed to copy '");
        io::write_all(2, src);
        io::write_str(2, b"' to '");
        io::write_all(2, dest);
        io::write_str(2, b"'\n");
        // Best-effort cleanup of the partial/incomplete destination so a
        // failed copy does not masquerade as a complete file.
        let _ = io::unlink(dest);
        return 1;
    }

    0
}

/// Copy a directory recursively
fn copy_directory(src: &[u8], dest: &[u8]) -> i32 {
    // Create destination directory
    if io::mkdir(dest, 0o755) < 0 {
        if !is_directory(dest) {
            sys::perror(dest);
            return 1;
        }
    }

    let fd = io::open(src, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        sys::perror(src);
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
                let mut src_path = [0u8; 4096];
                let mut src_len = 0;
                for &c in src {
                    src_path[src_len] = c;
                    src_len += 1;
                }
                src_path[src_len] = b'/';
                src_len += 1;
                for &c in name {
                    src_path[src_len] = c;
                    src_len += 1;
                }

                let mut dest_path = [0u8; 4096];
                let mut dest_len = 0;
                for &c in dest {
                    dest_path[dest_len] = c;
                    dest_len += 1;
                }
                dest_path[dest_len] = b'/';
                dest_len += 1;
                for &c in name {
                    dest_path[dest_len] = c;
                    dest_len += 1;
                }

                let mut st: libc::stat = unsafe { core::mem::zeroed() };
                if io::stat(&src_path[..src_len], &mut st) == 0 {
                    if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
                        if copy_directory(&src_path[..src_len], &dest_path[..dest_len]) != 0 {
                            exit_code = 1;
                        }
                    } else if copy_file(&src_path[..src_len], &dest_path[..dest_len]) != 0 {
                        exit_code = 1;
                    }
                }
            }

            offset += dirent.d_reclen as usize;
        }
    }

    io::close(fd);
    exit_code
}

/// Remove a file or directory recursively
fn remove_recursive(path: &[u8]) {
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(path, &mut st) < 0 {
        return;
    }

    if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
        let fd = io::open(path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
        if fd < 0 {
            return;
        }

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
                    let mut full_path = [0u8; 512];
                    let mut len = 0;
                    for &c in path {
                        full_path[len] = c;
                        len += 1;
                    }
                    full_path[len] = b'/';
                    len += 1;
                    for &c in name {
                        full_path[len] = c;
                        len += 1;
                    }
                    remove_recursive(&full_path[..len]);
                }

                offset += dirent.d_reclen as usize;
            }
        }
        io::close(fd);
        io::rmdir(path);
    } else {
        io::unlink(path);
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for mv utility

    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let release = PathBuf::from("target/release/armybox");
        if release.exists() {
            return release;
        }
        PathBuf::from("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_mv_test_{}_{}",
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
    fn test_mv_rename_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("old.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                dir.join("old.txt").to_str().unwrap(),
                dir.join("new.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("old.txt").exists());
        assert!(dir.join("new.txt").exists());
        assert_eq!(fs::read_to_string(dir.join("new.txt")).unwrap(), "content");
        cleanup(&dir);
    }

    #[test]
    fn test_mv_to_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file.txt"), "content").unwrap();
        fs::create_dir(dir.join("destdir")).unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                dir.join("file.txt").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("file.txt").exists());
        assert!(dir.join("destdir/file.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_mv_multiple_files_to_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "content1").unwrap();
        fs::write(dir.join("file2.txt"), "content2").unwrap();
        fs::create_dir(dir.join("destdir")).unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("file1.txt").exists());
        assert!(!dir.join("file2.txt").exists());
        assert!(dir.join("destdir/file1.txt").exists());
        assert!(dir.join("destdir/file2.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_mv_nonexistent_source() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();

        let output = Command::new(&armybox)
            .args([
                "mv",
                "/nonexistent/file",
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_mv_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mv"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
        assert!(std::string::String::from_utf8_lossy(&output.stderr).contains("missing operand"));
    }

    #[test]
    fn test_mv_rename_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("olddir")).unwrap();
        fs::write(dir.join("olddir/file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                dir.join("olddir").to_str().unwrap(),
                dir.join("newdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("olddir").exists());
        assert!(dir.join("newdir").exists());
        assert!(dir.join("newdir/file.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_mv_overwrite_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("source.txt"), "new content").unwrap();
        fs::write(dir.join("dest.txt"), "old content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                dir.join("source.txt").to_str().unwrap(),
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("source.txt").exists());
        assert_eq!(
            fs::read_to_string(dir.join("dest.txt")).unwrap(),
            "new content"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_mv_force_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("source.txt"), "content").unwrap();
        fs::write(dir.join("dest.txt"), "existing").unwrap();

        let output = Command::new(&armybox)
            .args([
                "mv",
                "-f",
                dir.join("source.txt").to_str().unwrap(),
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!dir.join("source.txt").exists());
        assert_eq!(fs::read_to_string(dir.join("dest.txt")).unwrap(), "content");
        cleanup(&dir);
    }
}
