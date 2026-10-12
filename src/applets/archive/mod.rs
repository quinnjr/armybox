//! Archive utilities

use super::get_arg;
use crate::io;

// Extracted modules
#[cfg(feature = "bzip2")]
mod bzip2;
#[cfg(feature = "compress")]
mod compress;
#[cfg(feature = "cpio")]
mod cpio;
#[cfg(feature = "gzip")]
mod gzip;
#[cfg(feature = "tar")]
mod tar;
#[cfg(feature = "unzip")]
mod unzip;
#[cfg(feature = "xz")]
mod xz;
#[cfg(feature = "zstd")]
mod zstd_cmd;

// Re-export all utilities
#[cfg(feature = "bzip2")]
pub use bzip2::{bunzip2, bzcat, bzip2};
#[cfg(feature = "compress")]
pub use compress::{compress, uncompress};
#[cfg(feature = "cpio")]
pub use cpio::cpio;
#[cfg(feature = "gzip")]
pub use gzip::{gunzip, gzip, zcat};
#[cfg(feature = "tar")]
pub use tar::tar;
#[cfg(feature = "unzip")]
pub use unzip::unzip;
#[cfg(feature = "xz")]
pub use xz::{lzcat, lzma, unlzma, unxz, xz, xzcat};
#[cfg(feature = "zstd")]
pub use zstd_cmd::{unzstd, zstd, zstdcat};

// Shared helper functions used by multiple archive utilities

#[cfg(any(feature = "tar", feature = "cpio", feature = "unzip"))]
pub(crate) fn open_read(path: &[u8]) -> i32 {
    io::open(path, libc::O_RDONLY, 0)
}

#[cfg(any(feature = "tar", feature = "cpio", feature = "unzip"))]
pub(crate) fn open_write_create(path: &[u8], mode: i32) -> i32 {
    io::open(
        path,
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
        mode as u32,
    )
}

/// Create all parent directories for `path`.
///
/// Returns `false` (and creates nothing) if `path` is longer than
/// `io::PATH_MAX`, instead of silently truncating it (which previously
/// could create directories under the wrong, truncated name).
#[cfg(any(feature = "tar", feature = "cpio", feature = "unzip"))]
pub(crate) fn create_parent_dirs(path: &[u8]) -> bool {
    let mut buf = [0u8; io::PATH_MAX];
    if !io::path_to_cstr(path, &mut buf) {
        return false;
    }
    let len = path.len();

    // Find each / and create directories
    for i in 0..len {
        if buf[i] == b'/' {
            buf[i] = 0;
            unsafe { libc::mkdir(buf.as_ptr() as *const libc::c_char, 0o755) };
            buf[i] = b'/';
        }
    }
    true
}

#[cfg(any(feature = "tar", feature = "cpio"))]
pub(crate) fn mode_string(mode: u32) -> [u8; 9] {
    let mut s = [b'-'; 9];
    if mode & 0o400 != 0 {
        s[0] = b'r';
    }
    if mode & 0o200 != 0 {
        s[1] = b'w';
    }
    if mode & 0o100 != 0 {
        s[2] = b'x';
    }
    if mode & 0o040 != 0 {
        s[3] = b'r';
    }
    if mode & 0o020 != 0 {
        s[4] = b'w';
    }
    if mode & 0o010 != 0 {
        s[5] = b'x';
    }
    if mode & 0o004 != 0 {
        s[6] = b'r';
    }
    if mode & 0o002 != 0 {
        s[7] = b'w';
    }
    if mode & 0o001 != 0 {
        s[8] = b'x';
    }
    s
}
