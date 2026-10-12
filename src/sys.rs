//! System utilities and helpers

use crate::io;

/// Parse decimal number from bytes
pub fn parse_u64(s: &[u8]) -> Option<u64> {
    if s.is_empty() {
        return None;
    }

    let mut result: u64 = 0;
    for &c in s {
        if c < b'0' || c > b'9' {
            return None;
        }
        result = result.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(result)
}

/// Parse signed number from bytes
pub fn parse_i64(s: &[u8]) -> Option<i64> {
    if s.is_empty() {
        return None;
    }

    if s[0] == b'-' {
        parse_u64(&s[1..]).map(|n| -(n as i64))
    } else {
        parse_u64(s).map(|n| n as i64)
    }
}

/// Parse octal number from bytes
pub fn parse_octal(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }

    let mut result: u32 = 0;
    for &c in s {
        if c < b'0' || c > b'7' {
            return None;
        }
        result = result.checked_mul(8)?.checked_add((c - b'0') as u32)?;
    }
    Some(result)
}

/// Format number into buffer, returns slice of used bytes
pub fn format_u64(n: u64, buf: &mut [u8]) -> &[u8] {
    if n == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return &buf[..1];
        }
        return &[];
    }

    let mut n = n;
    let mut i = buf.len();

    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    &buf[i..]
}

/// Format signed number
pub fn format_i64(n: i64, buf: &mut [u8]) -> &[u8] {
    if n < 0 {
        if buf.len() < 2 {
            return &[];
        }
        // Format unsigned portion first
        let abs_n = (-n) as u64;
        let len = {
            let temp = format_u64(abs_n, &mut buf[1..]);
            temp.len()
        };
        let start = buf.len() - 1 - len;
        buf[start] = b'-';
        &buf[start..]
    } else {
        format_u64(n as u64, buf)
    }
}

/// Format octal number
pub fn format_octal(n: u32, buf: &mut [u8]) -> &[u8] {
    if n == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return &buf[..1];
        }
        return &[];
    }

    let mut n = n;
    let mut i = buf.len();

    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 8) as u8;
        n /= 8;
    }

    &buf[i..]
}

/// Format hex number
pub fn format_hex(n: u64, buf: &mut [u8]) -> &[u8] {
    const HEX: &[u8] = b"0123456789abcdef";

    if n == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return &buf[..1];
        }
        return &[];
    }

    let mut n = n;
    let mut i = buf.len();

    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = HEX[(n % 16) as usize];
        n /= 16;
    }

    &buf[i..]
}

/// Human-readable size
pub fn format_size(bytes: u64, buf: &mut [u8]) -> &[u8] {
    const UNITS: &[u8] = b"BKMGTPE";

    let mut size = bytes;
    let mut unit = 0;

    while size >= 1024 && unit < UNITS.len() - 1 {
        size /= 1024;
        unit += 1;
    }

    // Format number first to get its length
    let num_str = format_u64(size, buf);
    let num_len = num_str.len();

    // If we have a unit suffix (not bytes), we need to add it
    if unit > 0 && buf.len() > num_len {
        // Find start of the formatted number
        let start = buf.len() - num_len;

        // Shift number left by 1 to make room for unit suffix
        if start > 0 {
            for i in 0..num_len {
                buf[start - 1 + i] = buf[start + i];
            }
            buf[start - 1 + num_len] = UNITS[unit];
            return &buf[start - 1..start + num_len];
        }
    }

    &buf[buf.len() - num_len..]
}

/// File mode to permission string
pub fn format_mode(mode: u32, buf: &mut [u8; 10]) {
    // File type
    buf[0] = match mode & libc::S_IFMT as u32 {
        t if t == libc::S_IFDIR as u32 => b'd',
        t if t == libc::S_IFLNK as u32 => b'l',
        t if t == libc::S_IFCHR as u32 => b'c',
        t if t == libc::S_IFBLK as u32 => b'b',
        t if t == libc::S_IFIFO as u32 => b'p',
        t if t == libc::S_IFSOCK as u32 => b's',
        _ => b'-',
    };

    // User permissions
    buf[1] = if mode & libc::S_IRUSR as u32 != 0 { b'r' } else { b'-' };
    buf[2] = if mode & libc::S_IWUSR as u32 != 0 { b'w' } else { b'-' };
    buf[3] = if mode & libc::S_ISUID as u32 != 0 {
        if mode & libc::S_IXUSR as u32 != 0 { b's' } else { b'S' }
    } else {
        if mode & libc::S_IXUSR as u32 != 0 { b'x' } else { b'-' }
    };

    // Group permissions
    buf[4] = if mode & libc::S_IRGRP as u32 != 0 { b'r' } else { b'-' };
    buf[5] = if mode & libc::S_IWGRP as u32 != 0 { b'w' } else { b'-' };
    buf[6] = if mode & libc::S_ISGID as u32 != 0 {
        if mode & libc::S_IXGRP as u32 != 0 { b's' } else { b'S' }
    } else {
        if mode & libc::S_IXGRP as u32 != 0 { b'x' } else { b'-' }
    };

    // Other permissions
    buf[7] = if mode & libc::S_IROTH as u32 != 0 { b'r' } else { b'-' };
    buf[8] = if mode & libc::S_IWOTH as u32 != 0 { b'w' } else { b'-' };
    buf[9] = if mode & libc::S_ISVTX as u32 != 0 {
        if mode & libc::S_IXOTH as u32 != 0 { b't' } else { b'T' }
    } else {
        if mode & libc::S_IXOTH as u32 != 0 { b'x' } else { b'-' }
    };
}

/// Create device number from major/minor - portable across glibc, musl, and Bionic
/// This replaces libc::makedev which may have different definitions
#[inline]
pub fn makedev(major: u32, minor: u32) -> libc::dev_t {
    // Linux-compatible makedev formula:
    // dev = ((major & 0xfff) << 8) | (minor & 0xff) | ((minor & 0xfff00) << 12) | ((major & 0xfffff000) << 32)
    // Simplified for common cases (major/minor < 256):
    #[cfg(target_os = "linux")]
    {
        ((major as libc::dev_t & 0xfff) << 8)
            | (minor as libc::dev_t & 0xff)
            | ((minor as libc::dev_t & 0xfff00) << 12)
            | ((major as libc::dev_t & 0xfffff000) << 32)
    }
    #[cfg(target_os = "android")]
    {
        ((major as libc::dev_t & 0xfff) << 8)
            | (minor as libc::dev_t & 0xff)
            | ((minor as libc::dev_t & 0xfff00) << 12)
            | ((major as libc::dev_t & 0xfffff000) << 32)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        // Fallback: simple formula for other platforms
        ((major as libc::dev_t) << 8) | (minor as libc::dev_t)
    }
}

/// Get errno - works on glibc, musl, and Bionic
pub fn errno() -> i32 {
    // Bionic exposes errno as `__errno()`; glibc and musl as `__errno_location()`.
    #[cfg(target_os = "android")]
    unsafe { *libc::__errno() }

    #[cfg(target_os = "linux")]
    unsafe { *libc::__errno_location() }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    unsafe { *libc::__error() }
}

/// Clear errno
pub fn clear_errno() {
    #[cfg(target_os = "android")]
    unsafe { *libc::__errno() = 0; }

    #[cfg(target_os = "linux")]
    unsafe { *libc::__errno_location() = 0; }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    unsafe { *libc::__error() = 0; }
}

/// Constants Bionic has but the libc crate does not bind for Android
/// (`SCHED_OTHER` in sched.h, `_PC_FILESIZEBITS` in unistd.h). Applets
/// reference these via `sys::` so a single code path works everywhere.
#[cfg(not(target_os = "android"))]
pub use libc::{SCHED_OTHER, _PC_FILESIZEBITS};
#[cfg(target_os = "android")]
pub const SCHED_OTHER: libc::c_int = 0;
#[cfg(target_os = "android")]
pub const _PC_FILESIZEBITS: libc::c_int = 0;

/// reboot(2) commands; kernel ABI from linux/reboot.h, unbound in the libc
/// crate for Android.
#[cfg(not(target_os = "android"))]
pub use libc::{RB_AUTOBOOT, RB_HALT_SYSTEM, RB_POWER_OFF};
#[cfg(target_os = "android")]
pub const RB_AUTOBOOT: libc::c_int = 0x01234567u32 as i32;
#[cfg(target_os = "android")]
pub const RB_HALT_SYSTEM: libc::c_int = 0xcdef0123u32 as i32;
#[cfg(target_os = "android")]
pub const RB_POWER_OFF: libc::c_int = 0x4321fedcu32 as i32;

/// reboot(2). Bionic has the syscall but the libc crate binds no wrapper for
/// Android, so issue it directly there.
pub fn reboot(cmd: libc::c_int) -> i32 {
    #[cfg(not(target_os = "android"))]
    unsafe {
        libc::reboot(cmd)
    }
    #[cfg(target_os = "android")]
    unsafe {
        const LINUX_REBOOT_MAGIC1: libc::c_uint = 0xfee1dead;
        const LINUX_REBOOT_MAGIC2: libc::c_uint = 672274793;
        libc::syscall(libc::SYS_reboot, LINUX_REBOOT_MAGIC1, LINUX_REBOOT_MAGIC2, cmd, 0) as i32
    }
}

/// gethostid(3). Bionic has no gethostid at all; there, mirror glibc's
/// storage semantics: the 4 bytes in /etc/hostid if present, else 0.
pub fn gethostid() -> i64 {
    #[cfg(not(target_os = "android"))]
    unsafe {
        libc::gethostid() as i64
    }
    #[cfg(target_os = "android")]
    {
        let fd = io::open(b"/etc/hostid", libc::O_RDONLY, 0);
        if fd < 0 {
            return 0;
        }
        let mut buf = [0u8; 4];
        let n = io::read(fd, &mut buf);
        io::close(fd);
        if n == 4 {
            u32::from_ne_bytes(buf) as i64
        } else {
            0
        }
    }
}

/// Routing-table ABI from linux/route.h, which Bionic's net/route.h includes
/// verbatim; the libc crate does not bind these for Android. The struct layout
/// matches libc's rtentry field-for-field (on LP64 the rt_tos/rt_class/rt_pad4
/// group occupies exactly the kernel's `void *rt_pad4` slot), so route and
/// ifup share one construction path on every platform.
#[cfg(not(target_os = "android"))]
pub use libc::{rtentry, RTF_GATEWAY, RTF_HOST, RTF_UP};
#[cfg(target_os = "android")]
pub const RTF_UP: libc::c_ushort = 0x0001;
#[cfg(target_os = "android")]
pub const RTF_GATEWAY: libc::c_ushort = 0x0002;
#[cfg(target_os = "android")]
pub const RTF_HOST: libc::c_ushort = 0x0004;

#[cfg(target_os = "android")]
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct rtentry {
    pub rt_pad1: libc::c_ulong,
    pub rt_dst: libc::sockaddr,
    pub rt_gateway: libc::sockaddr,
    pub rt_genmask: libc::sockaddr,
    pub rt_flags: libc::c_ushort,
    pub rt_pad2: libc::c_short,
    pub rt_pad3: libc::c_ulong,
    pub rt_tos: libc::c_uchar,
    pub rt_class: libc::c_uchar,
    #[cfg(target_pointer_width = "64")]
    pub rt_pad4: [libc::c_short; 3usize],
    #[cfg(not(target_pointer_width = "64"))]
    pub rt_pad4: [libc::c_short; 1usize],
    pub rt_metric: libc::c_short,
    pub rt_dev: *mut libc::c_char,
    pub rt_mtu: libc::c_ulong,
    pub rt_window: libc::c_ulong,
    pub rt_irtt: libc::c_ushort,
}

/// Parse size with optional suffix (K, M, G, T, P, E)
pub fn parse_size(s: &[u8]) -> Option<u64> {
    if s.is_empty() {
        return None;
    }

    // Find where digits end
    let mut num_end = s.len();
    for (i, &c) in s.iter().enumerate() {
        if !(c >= b'0' && c <= b'9') {
            num_end = i;
            break;
        }
    }

    if num_end == 0 {
        return None;
    }

    let base = parse_u64(&s[..num_end])?;

    if num_end >= s.len() {
        return Some(base);
    }

    let multiplier = match s[num_end] {
        b'k' | b'K' => 1024u64,
        b'm' | b'M' => 1024 * 1024,
        b'g' | b'G' => 1024 * 1024 * 1024,
        b't' | b'T' => 1024 * 1024 * 1024 * 1024,
        b'p' | b'P' => 1024 * 1024 * 1024 * 1024 * 1024,
        b'e' | b'E' => 1024 * 1024 * 1024 * 1024 * 1024 * 1024,
        _ => return None,
    };

    base.checked_mul(multiplier)
}

/// Format number to buffer and return slice - safer version
pub fn format_num_buf(n: u64, buf: &mut [u8]) -> usize {
    if n == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
            return 1;
        }
        return 0;
    }

    let mut n = n;
    let mut i = buf.len();

    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    // Shift to beginning
    let len = buf.len() - i;
    for j in 0..len {
        buf[j] = buf[i + j];
    }
    len
}

/// Format number to string (simple version that writes directly)
///
/// # Safety Warning
/// This function uses a static buffer and is NOT thread-safe.
/// The returned slice is only valid until the next call to this function.
/// Prefer `format_u64()` with a caller-provided buffer for thread-safe usage.
///
/// # Example
/// ```ignore
/// // WRONG - data race if called from multiple threads
/// let a = format_num(123);
/// let b = format_num(456); // 'a' is now invalid!
///
/// // RIGHT - use immediately
/// io::write_all(1, format_num(123));
/// ```
#[deprecated(note = "Use format_u64() with a local buffer instead - this function is not thread-safe")]
pub fn format_num(n: u64) -> &'static [u8] {
    // Use format_u64 which already exists
    // SAFETY: This uses a static mutable buffer which creates a data race
    // if called from multiple threads. Caller must use the result immediately
    // before any other call to this function.
    static mut BUF: [u8; 24] = [0; 24];
    #[allow(static_mut_refs)]
    unsafe {
        let s = format_u64(n, &mut BUF);
        // Return pointer to static - caller must use immediately
        core::slice::from_raw_parts(s.as_ptr(), s.len())
    }
}

/// Print errno message
pub fn perror(prefix: &[u8]) {
    let e = errno();
    io::write_all(2, prefix);
    io::write_str(2, b": ");

    // Common errno messages
    let msg = match e {
        libc::ENOENT => b"No such file or directory" as &[u8],
        libc::EACCES => b"Permission denied",
        libc::EEXIST => b"File exists",
        libc::EISDIR => b"Is a directory",
        libc::ENOTDIR => b"Not a directory",
        libc::ENOTEMPTY => b"Directory not empty",
        libc::EBUSY => b"Device or resource busy",
        libc::EINVAL => b"Invalid argument",
        libc::ENOMEM => b"Out of memory",
        libc::ENOSPC => b"No space left on device",
        libc::EPERM => b"Operation not permitted",
        libc::EROFS => b"Read-only file system",
        libc::EMFILE => b"Too many open files",
        libc::ENFILE => b"File table overflow",
        libc::EBADF => b"Bad file descriptor",
        libc::EAGAIN => b"Resource temporarily unavailable",
        libc::EINTR => b"Interrupted system call",
        libc::EIO => b"Input/output error",
        libc::ENODEV => b"No such device",
        libc::ENXIO => b"No such device or address",
        _ => b"Unknown error",
    };

    io::write_all(2, msg);
    io::write_str(2, b" (errno ");
    io::write_num(2, e as u64);
    io::write_str(2, b")\n");
}
