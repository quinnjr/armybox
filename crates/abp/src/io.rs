//! Low-level I/O primitives using libc
//!
//! Subset of armybox's io module used by the ABP package manager.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::ptr;

/// Maximum path length supported (matches PATH_MAX on Linux)
pub const PATH_MAX: usize = 4096;

/// Copy a path into a null-terminated buffer
///
/// Returns `true` if successful, `false` if path is too long.
#[inline]
pub fn path_to_cstr(path: &[u8], buf: &mut [u8; PATH_MAX]) -> bool {
    if path.len() >= PATH_MAX {
        return false;
    }
    buf[..path.len()].copy_from_slice(path);
    buf[path.len()] = 0;
    true
}

/// Current value of the C `errno`.
///
/// Bionic exposes errno as `__errno()`; glibc and musl as `__errno_location()`.
pub fn errno() -> i32 {
    #[cfg(target_os = "android")]
    unsafe {
        *libc::__errno()
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        *libc::__errno_location()
    }
}

/// Set the C `errno`.
pub fn set_errno(value: i32) {
    #[cfg(target_os = "android")]
    unsafe {
        *libc::__errno() = value;
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        *libc::__errno_location() = value;
    }
}

/// Does `st_mode` describe a directory?
///
/// The `u32` parameter (with casts at the call site) sidesteps `mode_t`/`S_IFMT`
/// width differences across glibc, musl, and Bionic.
#[inline]
pub fn is_dir(mode: u32) -> bool {
    (mode & libc::S_IFMT as u32) == libc::S_IFDIR as u32
}

/// Does `st_mode` describe a regular file?
#[inline]
pub fn is_reg(mode: u32) -> bool {
    (mode & libc::S_IFMT as u32) == libc::S_IFREG as u32
}

/// Write all bytes to a file descriptor
pub fn write_all(fd: i32, buf: &[u8]) -> isize {
    let mut written = 0;
    while written < buf.len() {
        let ret = unsafe {
            libc::write(
                fd,
                buf[written..].as_ptr() as *const libc::c_void,
                buf.len() - written,
            )
        };
        if ret < 0 {
            return ret;
        }
        written += ret as usize;
    }
    written as isize
}

/// Write a string literal to fd
pub fn write_str(fd: i32, s: &[u8]) -> isize {
    write_all(fd, s)
}

/// Write a number to fd
pub fn write_num(fd: i32, mut n: u64) -> isize {
    if n == 0 {
        return write_str(fd, b"0");
    }

    let mut buf = [0u8; 20];
    let mut i = buf.len();

    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    write_all(fd, &buf[i..])
}

/// Read from file descriptor into buffer
pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) }
}

/// Read entire file into Vec
#[cfg(feature = "alloc")]
pub fn read_all(fd: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let mut buf = [0u8; 4096];

    loop {
        let n = read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        result.extend_from_slice(&buf[..n as usize]);
    }

    result
}

/// Open a file
pub fn open(path: &[u8], flags: i32, mode: u32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::open(path_buf.as_ptr() as *const libc::c_char, flags, mode) }
}

/// Close a file descriptor
pub fn close(fd: i32) -> i32 {
    unsafe { libc::close(fd) }
}

/// Create a zeroed stat buffer
#[inline]
pub fn stat_zeroed() -> libc::stat {
    unsafe { core::mem::zeroed() }
}

/// Get file status
pub fn stat(path: &[u8], buf: &mut libc::stat) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::stat(path_buf.as_ptr() as *const libc::c_char, buf) }
}

/// Get file status from fd
pub fn fstat(fd: i32, buf: &mut libc::stat) -> i32 {
    unsafe { libc::fstat(fd, buf) }
}

/// Create a directory
pub fn mkdir(path: &[u8], mode: u32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe {
        libc::mkdir(
            path_buf.as_ptr() as *const libc::c_char,
            mode as libc::mode_t,
        )
    }
}

/// Remove a directory
pub fn rmdir(path: &[u8]) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::rmdir(path_buf.as_ptr() as *const libc::c_char) }
}

/// Unlink (remove) a file
pub fn unlink(path: &[u8]) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::unlink(path_buf.as_ptr() as *const libc::c_char) }
}

/// Rename a file
pub fn rename(old: &[u8], new: &[u8]) -> i32 {
    let mut old_buf = [0u8; 4096];
    let mut new_buf = [0u8; 4096];

    if old.len() >= old_buf.len() || new.len() >= new_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }

    old_buf[..old.len()].copy_from_slice(old);
    old_buf[old.len()] = 0;
    new_buf[..new.len()].copy_from_slice(new);
    new_buf[new.len()] = 0;

    unsafe {
        libc::rename(
            old_buf.as_ptr() as *const libc::c_char,
            new_buf.as_ptr() as *const libc::c_char,
        )
    }
}

/// Create a symlink
pub fn symlink(target: &[u8], linkpath: &[u8]) -> i32 {
    let mut target_buf = [0u8; 4096];
    let mut link_buf = [0u8; 4096];

    if target.len() >= target_buf.len() || linkpath.len() >= link_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }

    target_buf[..target.len()].copy_from_slice(target);
    target_buf[target.len()] = 0;
    link_buf[..linkpath.len()].copy_from_slice(linkpath);
    link_buf[linkpath.len()] = 0;

    unsafe {
        libc::symlink(
            target_buf.as_ptr() as *const libc::c_char,
            link_buf.as_ptr() as *const libc::c_char,
        )
    }
}

/// Change file permissions
pub fn chmod(path: &[u8], mode: u32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe {
        libc::chmod(
            path_buf.as_ptr() as *const libc::c_char,
            mode as libc::mode_t,
        )
    }
}

/// Check file access permissions (POSIX access())
pub fn access(path: &[u8], mode: i32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::access(path_buf.as_ptr() as *const libc::c_char, mode) }
}

/// Open directory for reading
pub fn opendir(path: &[u8]) -> *mut libc::DIR {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        set_errno(libc::ENAMETOOLONG);
        return ptr::null_mut();
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::opendir(path_buf.as_ptr() as *const libc::c_char) }
}

/// Read directory entry
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn readdir(dir: *mut libc::DIR) -> *mut libc::dirent {
    unsafe { libc::readdir(dir) }
}

/// Close directory
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn closedir(dir: *mut libc::DIR) -> i32 {
    unsafe { libc::closedir(dir) }
}

/// Seek in file
pub fn lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    unsafe { libc::lseek(fd, offset as libc::off_t, whence) as i64 }
}

/// Get C string length with a safety limit
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn strlen(s: *const u8) -> usize {
    const MAX_STRLEN: usize = 1024 * 1024; // 1MB safety limit
    let mut len = 0;
    while len < MAX_STRLEN {
        if unsafe { *s.add(len) } == 0 {
            break;
        }
        len += 1;
    }
    len
}

/// Convert C string pointer to slice
///
/// # Safety
/// `s` must be non-null and point to a NUL-terminated string that stays
/// valid and unmodified for as long as the returned slice is used.
pub unsafe fn cstr_to_slice(s: *const u8) -> &'static [u8] {
    debug_assert!(!s.is_null(), "cstr_to_slice called with null pointer");
    let len = strlen(s);
    unsafe { core::slice::from_raw_parts(s, len) }
}

/// Get dirent name as u8 slice
///
/// # Safety
/// `entry` must be a non-null `dirent` (e.g. from `readdir`) that stays
/// valid for as long as the returned slice is used, i.e. until the next
/// `readdir`/`closedir` on its stream.
pub unsafe fn dirent_name(entry: *const libc::dirent) -> (&'static [u8], usize) {
    debug_assert!(!entry.is_null(), "dirent_name called with null pointer");
    unsafe {
        let name_ptr = (*entry).d_name.as_ptr();
        let mut len = 0;
        while len < 255 && *name_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = core::slice::from_raw_parts(name_ptr as *const u8, len);
        (slice, len)
    }
}
