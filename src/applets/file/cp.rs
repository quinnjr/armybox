//! cp - copy files and directories
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cp.html

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// Hard cap on recursive-copy depth. Symlink cycles are already broken by
/// recreating links instead of following them; this bounds genuinely deep real
/// trees so `cp -R` can't exhaust the stack and crash (SIGSEGV).
const MAX_DEPTH: usize = 4096;

/// cp - copy files and directories
///
/// # Synopsis
/// ```text
/// cp [-fip] [-R [-H|-L|-P]] source_file target_file
/// cp [-fip] [-R [-H|-L|-P]] source_file... target
/// ```
///
/// # Description
/// Copies files. If target is a directory, copies source files into it.
/// If target is a file, source must be a single file.
///
/// # Options
/// - `-f`: Force - remove existing destination files if needed
/// - `-i`: Interactive - prompt before overwrite (not implemented)
/// - `-n`: No-clobber - never overwrite an existing destination
/// - `-p`: Preserve - preserve file mode, ownership, and timestamps
/// - `-P`: Never follow symbolic links in source (copy the link itself)
/// - `-L`: Follow symbolic links in source (default)
/// - `-R`, `-r`: Recursive - copy directories recursively
///
/// # Exit Status
/// - 0: All files copied successfully
/// - >0: An error occurred
pub fn cp(argc: i32, argv: *const *const u8) -> i32 {
    let mut recursive = false;
    let mut force = false;
    let mut _interactive = false;
    let mut preserve = false;
    let mut no_deref = false;
    let mut deref_specified = false;
    let mut no_clobber = false;
    let mut files_start = 1;

    // Parse options
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'r') || has_opt(arg, b'R') {
                    recursive = true;
                }
                if has_opt(arg, b'f') {
                    force = true;
                }
                if has_opt(arg, b'i') {
                    _interactive = true;
                }
                if has_opt(arg, b'p') {
                    preserve = true;
                }
                if has_opt(arg, b'P') {
                    no_deref = true;
                    deref_specified = true;
                }
                if has_opt(arg, b'L') {
                    no_deref = false;
                    deref_specified = true;
                }
                if has_opt(arg, b'H') {
                    deref_specified = true;
                }
                if has_opt(arg, b'n') {
                    no_clobber = true;
                }
                files_start = i + 1;
            } else {
                break;
            }
        }
    }

    // With -R and no explicit -H/-L/-P, symlinks in the tree are recreated as
    // symlinks (as GNU/BSD cp do), which also prevents symlink-cycle recursion.
    if recursive && !deref_specified {
        no_deref = true;
    }

    let file_count = argc - files_start;
    if file_count < 2 {
        io::write_str(2, b"cp: missing operand\n");
        return 1;
    }

    let dest = match unsafe { get_arg(argv, argc - 1) } {
        Some(d) => d,
        None => {
            io::write_str(2, b"cp: missing destination\n");
            return 1;
        }
    };

    // Check if destination is a directory
    let dest_is_dir = is_directory(dest);

    // If multiple sources, destination must be a directory
    if file_count > 2 && !dest_is_dir {
        io::write_str(2, b"cp: target '");
        io::write_all(2, dest);
        io::write_str(2, b"' is not a directory\n");
        return 1;
    }

    let mut exit_code = 0;

    for i in files_start..(argc - 1) {
        if let Some(src) = unsafe { get_arg(argv, i) } {
            let result = if dest_is_dir {
                // Copy into directory
                let mut dest_path = [0u8; 4096];
                let dest_len = build_dest_path(dest, src, &mut dest_path);
                copy_item(
                    src,
                    &dest_path[..dest_len],
                    recursive,
                    force,
                    preserve,
                    no_deref,
                    no_clobber,
                    0,
                )
            } else {
                copy_item(
                    src, dest, recursive, force, preserve, no_deref, no_clobber, 0,
                )
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

/// Copy a file or directory
fn copy_item(
    src: &[u8],
    dest: &[u8],
    recursive: bool,
    force: bool,
    preserve: bool,
    no_deref: bool,
    no_clobber: bool,
    depth: usize,
) -> i32 {
    // -n: if the destination already exists at all, skip this source
    // entirely without treating it as an error.
    if no_clobber {
        let mut dst_st: libc::stat = unsafe { core::mem::zeroed() };
        if io::lstat(dest, &mut dst_st) == 0 {
            return 0;
        }
    }

    // When -P is in effect, symbolic links are recreated as symbolic
    // links instead of being followed.
    if no_deref {
        let mut lst: libc::stat = unsafe { core::mem::zeroed() };
        if io::lstat(src, &mut lst) == 0
            && (lst.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFLNK as u32
        {
            return copy_symlink(src, dest, force);
        }
    }

    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(src, &mut st) < 0 {
        sys::perror(src);
        return 1;
    }

    if (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
        if !recursive {
            io::write_str(2, b"cp: omitting directory '");
            io::write_all(2, src);
            io::write_str(2, b"'\n");
            return 1;
        }
        copy_directory(src, dest, force, preserve, no_deref, no_clobber, depth)
    } else {
        copy_file_opts(src, dest, force, preserve)
    }
}

/// Recreate a symbolic link at `dest` pointing at the same target as `src`.
fn copy_symlink(src: &[u8], dest: &[u8], force: bool) -> i32 {
    let mut buf = [0u8; 4096];
    let n = io::readlink(src, &mut buf);
    if n < 0 {
        sys::perror(src);
        return 1;
    }

    if force {
        let _ = io::unlink(dest);
    }

    if io::symlink(&buf[..n as usize], dest) < 0 {
        sys::perror(dest);
        return 1;
    }
    0
}

/// Copy a single file
pub(crate) fn copy_file(src: &[u8], dest: &[u8], force: bool) -> i32 {
    copy_file_opts(src, dest, force, false)
}

/// Copy a single file with optional attribute preservation
fn copy_file_opts(src: &[u8], dest: &[u8], force: bool, preserve: bool) -> i32 {
    let src_fd = io::open(src, libc::O_RDONLY, 0);
    if src_fd < 0 {
        sys::perror(src);
        return 1;
    }

    // Get source stats for preserve mode
    let mut src_st: libc::stat = unsafe { core::mem::zeroed() };
    let has_stat = io::stat(src, &mut src_st) == 0;

    // Refuse to copy a file onto itself (POSIX requirement): if both source
    // and destination exist and refer to the same file (same device and
    // inode), report the error and skip the copy without truncating.
    let mut dest_st: libc::stat = unsafe { core::mem::zeroed() };
    let dest_has_stat = io::stat(dest, &mut dest_st) == 0;
    if has_stat
        && dest_has_stat
        && src_st.st_dev == dest_st.st_dev
        && src_st.st_ino == dest_st.st_ino
    {
        io::close(src_fd);
        io::write_str(2, b"cp: '");
        io::write_all(2, src);
        io::write_str(2, b"' and '");
        io::write_all(2, dest);
        io::write_str(2, b"' are the same file\n");
        return 1;
    }

    // If force and dest exists, try to remove it first
    if force {
        let _ = io::unlink(dest);
    }

    let create_mode = if preserve && has_stat {
        src_st.st_mode & 0o7777
    } else {
        0o644
    };

    let dest_fd = io::open(
        dest,
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        create_mode,
    );
    if dest_fd < 0 {
        io::close(src_fd);
        sys::perror(dest);
        return 1;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(src_fd, &mut buf);
        if n <= 0 {
            break;
        }
        if io::write_all(dest_fd, &buf[..n as usize]) < 0 {
            io::close(src_fd);
            io::close(dest_fd);
            sys::perror(dest);
            return 1;
        }
    }

    io::close(src_fd);
    io::close(dest_fd);

    // Preserve attributes
    if preserve && has_stat {
        // Preserve mode
        io::chmod(dest, src_st.st_mode & 0o7777);

        // Preserve ownership (may fail if not root)
        unsafe {
            libc::chown(
                dest.as_ptr() as *const libc::c_char,
                src_st.st_uid,
                src_st.st_gid,
            );
        }

        // Preserve timestamps
        let times = [
            libc::timeval {
                tv_sec: src_st.st_atime,
                tv_usec: 0,
            },
            libc::timeval {
                tv_sec: src_st.st_mtime,
                tv_usec: 0,
            },
        ];
        unsafe {
            libc::utimes(dest.as_ptr() as *const libc::c_char, times.as_ptr());
        }
    }

    0
}

/// Copy a directory recursively
fn copy_directory(
    src: &[u8],
    dest: &[u8],
    force: bool,
    preserve: bool,
    no_deref: bool,
    no_clobber: bool,
    depth: usize,
) -> i32 {
    // Bound genuine recursion depth so a pathologically deep tree stops with a
    // diagnostic and a nonzero exit instead of overflowing the stack.
    if depth >= MAX_DEPTH {
        io::write_str(2, b"cp: '");
        io::write_all(2, src);
        io::write_str(2, b"': directory too deep\n");
        return 1;
    }

    // Create destination directory
    if io::mkdir(dest, 0o755) < 0 {
        // Check if it already exists
        if !is_directory(dest) {
            sys::perror(dest);
            return 1;
        }
    }

    // Open source directory
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
                // Build full paths on the heap so arbitrarily deep trees can't
                // overflow a fixed buffer.
                let mut src_path: alloc::vec::Vec<u8> =
                    alloc::vec::Vec::with_capacity(src.len() + 1 + name.len());
                src_path.extend_from_slice(src);
                src_path.push(b'/');
                src_path.extend_from_slice(name);

                let mut dest_path: alloc::vec::Vec<u8> =
                    alloc::vec::Vec::with_capacity(dest.len() + 1 + name.len());
                dest_path.extend_from_slice(dest);
                dest_path.push(b'/');
                dest_path.extend_from_slice(name);

                if copy_item(
                    &src_path,
                    &dest_path,
                    true,
                    force,
                    preserve,
                    no_deref,
                    no_clobber,
                    depth + 1,
                ) != 0
                {
                    exit_code = 1;
                }
            }

            offset += dirent.d_reclen as usize;
        }
    }

    io::close(fd);

    // Preserve directory attributes
    if preserve {
        let mut src_st: libc::stat = unsafe { core::mem::zeroed() };
        if io::stat(src, &mut src_st) == 0 {
            io::chmod(dest, src_st.st_mode & 0o7777);
            unsafe {
                libc::chown(
                    dest.as_ptr() as *const libc::c_char,
                    src_st.st_uid,
                    src_st.st_gid,
                );
            }
            let times = [
                libc::timeval {
                    tv_sec: src_st.st_atime,
                    tv_usec: 0,
                },
                libc::timeval {
                    tv_sec: src_st.st_mtime,
                    tv_usec: 0,
                },
            ];
            unsafe {
                libc::utimes(dest.as_ptr() as *const libc::c_char, times.as_ptr());
            }
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    //! Unit tests for cp utility

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
            "armybox_cp_test_{}_{}",
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
    fn test_cp_single_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("source.txt"), "hello world").unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                dir.join("source.txt").to_str().unwrap(),
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dir.join("dest.txt").exists());
        assert_eq!(
            fs::read_to_string(dir.join("dest.txt")).unwrap(),
            "hello world"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cp_to_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("source.txt"), "content").unwrap();
        fs::create_dir(dir.join("destdir")).unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                dir.join("source.txt").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dir.join("destdir/source.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_cp_multiple_files_to_directory() {
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
                "cp",
                dir.join("file1.txt").to_str().unwrap(),
                dir.join("file2.txt").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dir.join("destdir/file1.txt").exists());
        assert!(dir.join("destdir/file2.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_cp_nonexistent_source() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();

        let output = Command::new(&armybox)
            .args([
                "cp",
                "/nonexistent/file",
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_cp_recursive_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir_all(dir.join("srcdir/subdir")).unwrap();
        fs::write(dir.join("srcdir/file.txt"), "content").unwrap();
        fs::write(dir.join("srcdir/subdir/nested.txt"), "nested").unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                "-r",
                dir.join("srcdir").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(dir.join("destdir/file.txt").exists());
        assert!(dir.join("destdir/subdir/nested.txt").exists());
        cleanup(&dir);
    }

    #[test]
    fn test_cp_directory_without_recursive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("srcdir")).unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                dir.join("srcdir").to_str().unwrap(),
                dir.join("destdir").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        // Should fail - can't copy directory without -r
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_cp_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["cp"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
        assert!(std::string::String::from_utf8_lossy(&output.stderr).contains("missing operand"));
    }

    #[test]
    fn test_cp_force_overwrite() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("source.txt"), "new content").unwrap();
        fs::write(dir.join("dest.txt"), "old content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                "-f",
                dir.join("source.txt").to_str().unwrap(),
                dir.join("dest.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            fs::read_to_string(dir.join("dest.txt")).unwrap(),
            "new content"
        );
        cleanup(&dir);
    }

    #[test]
    fn test_cp_preserves_content() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let binary_data: Vec<u8> = (0..=255).collect();
        fs::write(dir.join("binary.bin"), &binary_data).unwrap();

        let output = Command::new(&armybox)
            .args([
                "cp",
                dir.join("binary.bin").to_str().unwrap(),
                dir.join("copy.bin").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::read(dir.join("copy.bin")).unwrap(), binary_data);
        cleanup(&dir);
    }
}
