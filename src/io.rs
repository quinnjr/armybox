//! Low-level I/O primitives using libc
//!
//! This module provides basic I/O operations without std dependency.
//!
//! # Safety Architecture
//!
//! This module wraps unsafe libc functions to provide a safer Rust interface.
//! The general safety principles followed are:
//!
//! 1. **Path handling**: All path arguments are `&[u8]` and are copied into
//!    a null-terminated buffer before passing to libc. Paths longer than
//!    `PATH_MAX` (4096 bytes) are rejected with an error return.
//!
//! 2. **Buffer management**: Functions that fill buffers (like `read`, `getcwd`)
//!    take mutable slices and respect their bounds.
//!
//! 3. **Pointer validity**: Functions returning pointers (`getenv`, `ttyname`)
//!    return `Option` types and document lifetime constraints.
//!
//! 4. **Error handling**: System call errors are propagated as negative return
//!    values or `None`, matching POSIX conventions.
//!
//! # Thread Safety
//!
//! Most functions in this module are thread-safe as they wrap thread-safe
//! POSIX functions. Exceptions are documented on the individual functions.
//! Notable non-thread-safe operations include:
//! - `getenv()` - returned data can be invalidated by concurrent `setenv()`
//! - Functions using `readdir()` on the same `DIR*` from multiple threads

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::ptr;

/// Portable ioctl request type.
///
/// musl libc and Bionic define `ioctl(fd, request: c_int, ...)` while glibc
/// uses `ioctl(fd, request: c_ulong, ...)`. This type alias allows ioctl
/// request constants to compile correctly on all targets.
#[cfg(any(target_env = "musl", target_os = "android"))]
pub type IoctlReq = libc::c_int;
/// Portable ioctl request type (c_ulong on glibc, c_int on musl/Bionic).
#[cfg(not(any(target_env = "musl", target_os = "android")))]
pub type IoctlReq = libc::c_ulong;

/// Maximum path length supported (matches PATH_MAX on Linux)
pub const PATH_MAX: usize = 4096;

/// Copy a path into a null-terminated buffer
///
/// Returns `true` if successful, `false` if path is too long.
/// This is a safe helper that ensures proper null termination.
#[inline]
pub fn path_to_cstr(path: &[u8], buf: &mut [u8; PATH_MAX]) -> bool {
    if path.len() >= PATH_MAX {
        return false;
    }
    buf[..path.len()].copy_from_slice(path);
    buf[path.len()] = 0;
    true
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

/// Write all bytes to fd (alias for write_all returning isize)
pub fn write_all_fd(fd: i32, buf: &[u8]) -> isize {
    write_all(fd, buf)
}

/// Write all bytes and return count written
pub fn write_all_count(fd: i32, buf: &[u8]) -> usize {
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
            break;
        }
        written += ret as usize;
    }
    written
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

/// Write a signed number to fd
pub fn write_signed(fd: i32, n: i64) -> isize {
    if n < 0 {
        write_str(fd, b"-");
        write_num(fd, (-n) as u64)
    } else {
        write_num(fd, n as u64)
    }
}

/// Read from file descriptor into buffer
pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    unsafe {
        libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
    }
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
    // Ensure null-terminated path
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
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
///
/// This is safe because `libc::stat` is a POD type (Plain Old Data)
/// that can be safely zero-initialized. Using this helper centralizes
/// the unsafe `mem::zeroed()` call.
#[inline]
pub fn stat_zeroed() -> libc::stat {
    // SAFETY: libc::stat is a C struct with no invariants that would be
    // violated by zero initialization. All fields are numeric types.
    unsafe { core::mem::zeroed() }
}

/// Get file status
pub fn stat(path: &[u8], buf: &mut libc::stat) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::stat(path_buf.as_ptr() as *const libc::c_char, buf) }
}

/// Get file status (no follow symlinks)
pub fn lstat(path: &[u8], buf: &mut libc::stat) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::lstat(path_buf.as_ptr() as *const libc::c_char, buf) }
}

/// Get file status from fd
pub fn fstat(fd: i32, buf: &mut libc::stat) -> i32 {
    unsafe { libc::fstat(fd, buf) }
}

/// Create a directory
pub fn mkdir(path: &[u8], mode: u32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::mkdir(path_buf.as_ptr() as *const libc::c_char, mode as libc::mode_t) }
}

/// Remove a directory
pub fn rmdir(path: &[u8]) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
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
        return -1;
    }

    old_buf[..old.len()].copy_from_slice(old);
    old_buf[old.len()] = 0;
    new_buf[..new.len()].copy_from_slice(new);
    new_buf[new.len()] = 0;

    unsafe { libc::rename(old_buf.as_ptr() as *const libc::c_char, new_buf.as_ptr() as *const libc::c_char) }
}

/// Create a symlink
pub fn symlink(target: &[u8], linkpath: &[u8]) -> i32 {
    let mut target_buf = [0u8; 4096];
    let mut link_buf = [0u8; 4096];

    if target.len() >= target_buf.len() || linkpath.len() >= link_buf.len() {
        return -1;
    }

    target_buf[..target.len()].copy_from_slice(target);
    target_buf[target.len()] = 0;
    link_buf[..linkpath.len()].copy_from_slice(linkpath);
    link_buf[linkpath.len()] = 0;

    unsafe { libc::symlink(target_buf.as_ptr() as *const libc::c_char, link_buf.as_ptr() as *const libc::c_char) }
}

/// Create a hard link
pub fn link(old: &[u8], new: &[u8]) -> i32 {
    let mut old_buf = [0u8; 4096];
    let mut new_buf = [0u8; 4096];

    if old.len() >= old_buf.len() || new.len() >= new_buf.len() {
        return -1;
    }

    old_buf[..old.len()].copy_from_slice(old);
    old_buf[old.len()] = 0;
    new_buf[..new.len()].copy_from_slice(new);
    new_buf[new.len()] = 0;

    unsafe { libc::link(old_buf.as_ptr() as *const libc::c_char, new_buf.as_ptr() as *const libc::c_char) }
}

/// Change file permissions
pub fn chmod(path: &[u8], mode: u32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::chmod(path_buf.as_ptr() as *const libc::c_char, mode as libc::mode_t) }
}

/// Change working directory
pub fn chdir(path: &[u8]) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::chdir(path_buf.as_ptr() as *const libc::c_char) }
}

/// Get current working directory
#[cfg(feature = "alloc")]
pub fn getcwd() -> Option<Vec<u8>> {
    let mut buf = [0u8; 4096];
    let ret = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

    if ret.is_null() {
        None
    } else {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(buf[..len].to_vec())
    }
}

/// Get current working directory (no alloc version)
#[cfg(not(feature = "alloc"))]
pub fn getcwd(buf: &mut [u8]) -> bool {
    let ret = unsafe { libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    !ret.is_null()
}

/// Read symbolic link
pub fn readlink(path: &[u8], buf: &mut [u8]) -> isize {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() { return -1; }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::readlink(path_buf.as_ptr() as *const libc::c_char, buf.as_mut_ptr() as *mut libc::c_char, buf.len()) }
}

/// Get canonical path
pub fn realpath(path: &[u8], buf: &mut [u8]) -> isize {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() { return -1; }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    let ret = unsafe { libc::realpath(path_buf.as_ptr() as *const libc::c_char, buf.as_mut_ptr() as *mut libc::c_char) };
    if ret.is_null() {
        -1
    } else {
        strlen_arr(buf) as isize
    }
}

/// Open directory for reading
pub fn opendir(path: &[u8]) -> *mut libc::DIR {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
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

/// Get user ID
pub fn getuid() -> u32 {
    unsafe { libc::getuid() }
}

/// Get effective user ID
pub fn geteuid() -> u32 {
    unsafe { libc::geteuid() }
}

/// Get group ID
pub fn getgid() -> u32 {
    unsafe { libc::getgid() }
}

/// Get effective group ID
pub fn getegid() -> u32 {
    unsafe { libc::getegid() }
}

/// Check file access permissions (POSIX access())
pub fn access(path: &[u8], mode: i32) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::access(path_buf.as_ptr() as *const libc::c_char, mode) }
}

/// Get process ID
pub fn getpid() -> i32 {
    unsafe { libc::getpid() }
}

/// Get parent process ID
pub fn getppid() -> i32 {
    unsafe { libc::getppid() }
}

/// Get hostname
#[cfg(feature = "alloc")]
pub fn gethostname() -> Option<Vec<u8>> {
    let mut buf = [0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

    if ret != 0 {
        None
    } else {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(buf[..len].to_vec())
    }
}

/// Sleep for seconds
pub fn sleep(secs: u32) {
    unsafe { libc::sleep(secs); }
}

/// Sleep for microseconds
pub fn usleep(usecs: u32) {
    unsafe { libc::usleep(usecs); }
}

/// Duplicate file descriptor
pub fn dup(fd: i32) -> i32 {
    unsafe { libc::dup(fd) }
}

/// Duplicate file descriptor to specific fd
pub fn dup2(old: i32, new: i32) -> i32 {
    unsafe { libc::dup2(old, new) }
}

/// Seek in file
///
/// Bionic's `off_t` is 32-bit on 32-bit targets, so Android uses the `*64`
/// calls to keep offsets past 2 GiB from wrapping. glibc (64-bit) and musl
/// `off_t` are already 64-bit.
pub fn lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    #[cfg(target_os = "android")]
    unsafe {
        libc::lseek64(fd, offset, whence)
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        libc::lseek(fd, offset as libc::off_t, whence) as i64
    }
}

/// Truncate file
pub fn ftruncate(fd: i32, length: i64) -> i32 {
    #[cfg(target_os = "android")]
    unsafe {
        libc::ftruncate64(fd, length)
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        libc::ftruncate(fd, length as libc::off_t)
    }
}

/// Truncate file by path
pub fn truncate(path: &[u8], length: i64) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    #[cfg(target_os = "android")]
    unsafe {
        libc::truncate64(path_buf.as_ptr() as *const libc::c_char, length)
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        libc::truncate(path_buf.as_ptr() as *const libc::c_char, length as libc::off_t)
    }
}

/// Sync filesystem
pub fn sync() {
    unsafe { libc::sync(); }
}

/// Check if fd is a tty
pub fn isatty(fd: i32) -> bool {
    unsafe { libc::isatty(fd) != 0 }
}

/// Get terminal name
#[cfg(feature = "alloc")]
pub fn ttyname(fd: i32) -> Option<Vec<u8>> {
    let ptr = unsafe { libc::ttyname(fd) };
    if ptr.is_null() {
        None
    } else {
        let mut len = 0;
        while unsafe { *ptr.add(len) } != 0 {
            len += 1;
        }
        let slice = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
        Some(slice.to_vec())
    }
}

/// Get environment variable
///
/// # Safety Warning
/// The returned slice points to libc-managed memory. This memory can be
/// invalidated by any call to `setenv`, `unsetenv`, or `putenv`. The caller
/// must copy the data if it needs to persist across such calls.
///
/// The `'static` lifetime is a lie - the data is only valid until the
/// environment is modified. This is an inherent limitation of the POSIX API.
///
/// # Example
/// ```ignore
/// // SAFE: immediate use
/// if let Some(val) = getenv(b"PATH") {
///     io::write_all(1, val);
/// }
///
/// // DANGEROUS: storing reference across setenv
/// let path = getenv(b"PATH");
/// setenv(b"FOO", b"bar"); // path may now be invalid!
/// ```
pub fn getenv(name: &[u8]) -> Option<&'static [u8]> {
    let mut name_buf = [0u8; 256];
    if name.len() >= name_buf.len() {
        return None;
    }
    name_buf[..name.len()].copy_from_slice(name);
    name_buf[name.len()] = 0;

    // SAFETY: libc::getenv returns a pointer to environment memory.
    // This memory is managed by libc and can be invalidated by setenv/unsetenv.
    let ptr = unsafe { libc::getenv(name_buf.as_ptr() as *const libc::c_char) };
    if ptr.is_null() {
        None
    } else {
        // SAFETY: We scan for null terminator with a reasonable limit (via strlen)
        let len = strlen(ptr as *const u8);
        // SAFETY: The slice is valid as long as no environment modification occurs.
        // The 'static lifetime is technically incorrect but matches POSIX semantics.
        Some(unsafe { core::slice::from_raw_parts(ptr as *const u8, len) })
    }
}

/// Set an environment variable
///
/// Sets the environment variable `name` to `value`. If `overwrite` is true,
/// an existing variable will be replaced; otherwise it will be left unchanged.
///
/// # Returns
/// 0 on success, -1 on error (e.g., out of memory or invalid name).
///
/// # Safety Note
/// This function invalidates any pointers previously returned by `getenv()`
/// for any environment variable, not just the one being set.
pub fn setenv(name: &[u8], value: &[u8], overwrite: bool) -> i32 {
    // Validate name doesn't contain '=' or is empty
    if name.is_empty() || name.contains(&b'=') {
        return -1;
    }

    let mut name_buf = [0u8; 256];
    let mut value_buf = [0u8; 4096];

    if name.len() >= name_buf.len() || value.len() >= value_buf.len() {
        return -1;
    }

    name_buf[..name.len()].copy_from_slice(name);
    name_buf[name.len()] = 0;
    value_buf[..value.len()].copy_from_slice(value);
    value_buf[value.len()] = 0;

    // SAFETY: Both buffers are properly null-terminated and within bounds.
    // The libc function copies the data, so our stack buffers can go out of scope.
    unsafe {
        libc::setenv(
            name_buf.as_ptr() as *const libc::c_char,
            value_buf.as_ptr() as *const libc::c_char,
            if overwrite { 1 } else { 0 },
        )
    }
}

/// Unset (remove) an environment variable
///
/// # Returns
/// 0 on success, -1 on error.
///
/// # Safety Note
/// This function may invalidate pointers previously returned by `getenv()`.
pub fn unsetenv(name: &[u8]) -> i32 {
    if name.is_empty() || name.contains(&b'=') {
        return -1;
    }

    let mut name_buf = [0u8; 256];
    if name.len() >= name_buf.len() {
        return -1;
    }

    name_buf[..name.len()].copy_from_slice(name);
    name_buf[name.len()] = 0;

    // SAFETY: Buffer is properly null-terminated.
    unsafe { libc::unsetenv(name_buf.as_ptr() as *const libc::c_char) }
}

/// Send signal to process
pub fn kill(pid: i32, sig: i32) -> i32 {
    unsafe { libc::kill(pid, sig) }
}

/// Execute program
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn execve(path: &[u8], argv: *const *const libc::c_char, envp: *const *const libc::c_char) -> i32 {
    let mut path_buf = [0u8; 4096];
    if path.len() >= path_buf.len() {
        return -1;
    }
    path_buf[..path.len()].copy_from_slice(path);
    path_buf[path.len()] = 0;

    unsafe { libc::execve(path_buf.as_ptr() as *const libc::c_char, argv, envp) }
}

/// Fork process
pub fn fork() -> i32 {
    unsafe { libc::fork() }
}

/// Wait for child process
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn wait(status: *mut i32) -> i32 {
    unsafe { libc::wait(status) }
}

/// Wait for specific process
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32 {
    unsafe { libc::waitpid(pid, status, options) }
}

/// Get uname info
pub fn uname(buf: &mut libc::utsname) -> i32 {
    unsafe { libc::uname(buf) }
}

/// Exit process
pub fn exit(code: i32) -> ! {
    unsafe { libc::_exit(code); }
}

// ============================================================================
// Helper functions for argument parsing
// ============================================================================

/// Get C string length with a safety limit
///
/// This function limits scanning to 1MB to prevent runaway reads on
/// non-terminated strings. Returns the length excluding null terminator.
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

/// String length for array
pub fn strlen_arr(s: &[u8]) -> usize {
    s.iter().position(|&c| c == 0).unwrap_or(s.len())
}

/// Convert C string pointer to slice
///
/// # Safety
/// The caller must ensure:
/// - `s` is a valid, non-null pointer to a null-terminated C string
/// - The memory remains valid for the `'static` lifetime (or the caller
///   must ensure the slice is not used after the memory is freed)
/// - The string is properly null-terminated within allocated bounds
///
/// The `'static` lifetime is often incorrect - callers should typically
/// constrain the actual lifetime based on the source of the pointer.
pub unsafe fn cstr_to_slice(s: *const u8) -> &'static [u8] {
    debug_assert!(!s.is_null(), "cstr_to_slice called with null pointer");
    let len = strlen(s);
    // SAFETY: Caller guarantees pointer validity and null termination.
    // The 'static lifetime is the caller's responsibility to honor.
    unsafe { core::slice::from_raw_parts(s, len) }
}

/// Compare byte slices
pub fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y)
}

/// Check if slice starts with prefix
pub fn starts_with(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && &s[..prefix.len()] == prefix
}

/// Get dirent name as u8 slice
///
/// On Linux, d_name is [i8; 256], we need to convert to u8.
///
/// # Safety
/// The caller must ensure:
/// - `entry` is a valid pointer to a `libc::dirent` structure
/// - The dirent was obtained from a valid `readdir()` call
/// - The pointer remains valid (not invalidated by `closedir` or another `readdir`)
///
/// The returned slice is only valid until the next `readdir()` call on the
/// same directory stream, or until `closedir()` is called.
///
/// # Returns
/// A tuple of (name_slice, length). The slice does NOT include the null terminator.
pub unsafe fn dirent_name(entry: *const libc::dirent) -> (&'static [u8], usize) {
    debug_assert!(!entry.is_null(), "dirent_name called with null pointer");
    // SAFETY: Caller guarantees entry is a valid dirent pointer.
    // d_name is a fixed-size array within the struct, so access is safe.
    unsafe {
        let name_ptr = (*entry).d_name.as_ptr();
        let mut len = 0;
        // Limit to 255 to stay within d_name bounds (256 bytes including null)
        while len < 255 && *name_ptr.add(len) != 0 {
            len += 1;
        }
        let slice = core::slice::from_raw_parts(name_ptr as *const u8, len);
        (slice, len)
    }
}
