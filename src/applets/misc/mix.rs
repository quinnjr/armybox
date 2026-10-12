//! mix - audio mixer interface
//!
//! Interacts with ALSA/OSS audio mixer.

use super::get_arg;
use crate::io;
use crate::sys;

/// mix - audio mixer interface
///
/// # Synopsis
/// ```text
/// mix [channel] [volume]
/// ```
///
/// # Description
/// Interacts with ALSA/OSS audio mixer.
/// Note: Full ALSA support requires libasound. This provides basic OSS compatibility.
///
/// # Exit Status
/// - 0: Success
/// - 1: Cannot open mixer or set volume
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mix(argc: i32, argv: *const *const u8) -> i32 {
    const SOUND_MIXER_READ_VOLUME: crate::io::IoctlReq = 0x80044D00u32 as crate::io::IoctlReq;
    const SOUND_MIXER_WRITE_VOLUME: crate::io::IoctlReq = 0xC0044D00u32 as crate::io::IoctlReq;

    // Try to open OSS mixer
    let fd = io::open(b"/dev/mixer", libc::O_RDWR, 0);
    if fd < 0 {
        io::write_str(2, b"mix: cannot open /dev/mixer\n");
        io::write_str(2, b"Note: This system may use ALSA instead of OSS.\n");
        io::write_str(2, b"Try: amixer or alsamixer instead.\n");
        return 1;
    }

    if argc < 2 {
        // Show current volume
        let mut vol: i32 = 0;
        let ret = unsafe { libc::ioctl(fd, SOUND_MIXER_READ_VOLUME, &mut vol as *mut i32) };
        io::close(fd);

        if ret < 0 {
            io::write_str(2, b"mix: cannot read volume\n");
            return 1;
        }

        let left = vol & 0xFF;
        let right = (vol >> 8) & 0xFF;

        io::write_str(1, b"Master volume: ");
        let mut buf = [0u8; 8];
        let l = sys::format_u64(left as u64, &mut buf);
        io::write_all(1, l);
        io::write_str(1, b"% / ");
        let r = sys::format_u64(right as u64, &mut buf);
        io::write_all(1, r);
        io::write_str(1, b"%\n");
    } else {
        // Set volume
        let vol_arg = match unsafe { get_arg(argv, 1) } {
            Some(v) => v,
            None => {
                io::close(fd);
                return 1;
            }
        };

        let volume = match sys::parse_u64(vol_arg) {
            Some(v) => core::cmp::min(v, 100) as i32,
            None => {
                io::write_str(2, b"mix: invalid volume\n");
                io::close(fd);
                return 1;
            }
        };

        let vol = volume | (volume << 8); // Same for left and right

        let ret = unsafe { libc::ioctl(fd, SOUND_MIXER_WRITE_VOLUME, &vol as *const i32) };
        io::close(fd);

        if ret < 0 {
            io::write_str(2, b"mix: cannot set volume\n");
            return 1;
        }

        io::write_str(1, b"Volume set to ");
        let mut buf = [0u8; 8];
        let v = sys::format_u64(volume as u64, &mut buf);
        io::write_all(1, v);
        io::write_str(1, b"%\n");
    }

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn mix(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"mix: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    // mix requires /dev/mixer which may not be available
}
