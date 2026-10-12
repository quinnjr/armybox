//! mkswap - set up a Linux swap area
//!
//! Set up a Linux swap area on a device or file.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// mkswap - set up a Linux swap area
///
/// # Synopsis
/// ```text
/// mkswap device
/// ```
///
/// # Description
/// Set up a Linux swap area on a device or file.
/// This writes a swap signature to the device.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn mkswap(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"mkswap: missing device\n");
        return 1;
    }

    let device = unsafe { get_arg(argv, 1).unwrap() };

    // Open device
    let fd = io::open(device, libc::O_RDWR, 0);
    if fd < 0 {
        sys::perror(device);
        return 1;
    }

    // Get device size
    let size = io::lseek(fd, 0, libc::SEEK_END);
    if size < 0 {
        sys::perror(b"lseek");
        io::close(fd);
        return 1;
    }

    // Minimum swap size is 10 pages (typically 40KB)
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as i64;
    if size < page_size * 10 {
        io::write_str(2, b"mkswap: swap area too small\n");
        io::close(fd);
        return 1;
    }

    // Write swap signature at end of first page
    // The magic is "SWAPSPACE2" at offset pagesize - 10
    let magic = b"SWAPSPACE2";
    let magic_offset = page_size - 10;

    if io::lseek(fd, magic_offset, libc::SEEK_SET) < 0 {
        sys::perror(b"lseek");
        io::close(fd);
        return 1;
    }

    if io::write_all(fd, magic) != magic.len() as isize {
        sys::perror(b"write");
        io::close(fd);
        return 1;
    }

    io::close(fd);

    io::write_str(1, b"Setting up swapspace version 1, size = ");
    io::write_signed(1, size / 1024);
    io::write_str(1, b" KiB\n");

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
    fn test_mkswap_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mkswap"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing device"));
    }

    #[test]
    fn test_mkswap_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["mkswap", "/nonexistent/device"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
