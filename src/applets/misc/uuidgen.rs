//! uuidgen/mcookie - generate random identifiers
//!
//! Generate UUIDs and magic cookies for authentication.

use super::get_arg;
use crate::io;

/// uuidgen - generate UUID (version 4, random)
///
/// Generates a random UUID conforming to RFC 4122 version 4.
/// Reads random bytes from /dev/urandom.
///
/// # Synopsis
/// ```text
/// uuidgen [-r]
/// ```
///
/// # Options
/// - `-r, --random`: Generate random-based UUID (default)
/// - `-h, --help`: Show help
///
/// # Exit Status
/// - 0: Success
pub fn uuidgen(argc: i32, argv: *const *const u8) -> i32 {
    let mut random_output = false;

    // Parse options
    for i in 1..argc as usize {
        if let Some(arg) = unsafe { get_arg(argv, i as i32) } {
            if arg == b"-r" || arg == b"--random" {
                random_output = true;
            } else if arg == b"-h" || arg == b"--help" {
                io::write_str(1, b"Usage: uuidgen [-r]\n");
                io::write_str(1, b"  -r  Generate random-based UUID (default)\n");
                return 0;
            }
        }
    }
    let _ = random_output; // Always use random for now

    // Read 16 random bytes from /dev/urandom
    let mut uuid = [0u8; 16];
    let fd = io::open(b"/dev/urandom", libc::O_RDONLY, 0);
    if fd < 0 {
        // Fallback to time-based if /dev/urandom unavailable
        let t = unsafe { libc::time(core::ptr::null_mut()) } as u64;
        let pid = unsafe { libc::getpid() } as u64;
        let mut rng = t ^ (pid << 16);
        for i in 0..16 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            uuid[i] = (rng >> 56) as u8;
        }
    } else {
        io::read(fd, &mut uuid);
        io::close(fd);
    }

    // Set version 4 (random) - bits 12-15 of time_hi_and_version
    uuid[6] = (uuid[6] & 0x0F) | 0x40;

    // Set variant (RFC 4122) - bits 6-7 of clock_seq_hi_and_reserved
    uuid[8] = (uuid[8] & 0x3F) | 0x80;

    // Print in canonical format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    let hex = b"0123456789abcdef";
    let mut out = [0u8; 36];
    let mut j = 0;

    for (i, &b) in uuid.iter().enumerate() {
        out[j] = hex[(b >> 4) as usize];
        out[j + 1] = hex[(b & 0xf) as usize];
        j += 2;

        if i == 3 || i == 5 || i == 7 || i == 9 {
            out[j] = b'-';
            j += 1;
        }
    }

    io::write_all(1, &out);
    io::write_str(1, b"\n");
    0
}

/// mcookie - generate magic cookie for X authentication
///
/// Generates a 128-bit random hexadecimal number.
/// Used by xauth for generating magic cookies.
///
/// # Synopsis
/// ```text
/// mcookie
/// ```
///
/// # Exit Status
/// - 0: Success
pub fn mcookie(_argc: i32, _argv: *const *const u8) -> i32 {
    // Read 16 random bytes from /dev/urandom
    let mut cookie = [0u8; 16];
    let fd = io::open(b"/dev/urandom", libc::O_RDONLY, 0);
    if fd < 0 {
        // Fallback to time-based if /dev/urandom unavailable
        let t = unsafe { libc::time(core::ptr::null_mut()) } as u64;
        let pid = unsafe { libc::getpid() } as u64;
        let mut rng = t ^ (pid << 16) ^ 0xCAFEBABE;
        for i in 0..16 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            cookie[i] = (rng >> 56) as u8;
        }
    } else {
        io::read(fd, &mut cookie);
        io::close(fd);
    }

    // Print as 32 hex characters
    let hex = b"0123456789abcdef";
    for &b in &cookie {
        io::write_all(1, &[hex[(b >> 4) as usize], hex[(b & 0xf) as usize]]);
    }
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_uuidgen() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["uuidgen"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        assert_eq!(stdout.trim().len(), 36);
        assert!(stdout.contains("-"));
    }

    #[test]
    fn test_mcookie() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mcookie"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // mcookie is 32 hex chars
        assert_eq!(stdout.trim().len(), 32);
    }
}
