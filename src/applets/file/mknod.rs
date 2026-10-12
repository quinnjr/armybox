//! mknod - make block or character special files
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mknod.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// mknod - make block or character special files
///
/// # Synopsis
/// ```text
/// mknod name type [major minor]
/// ```
///
/// # Description
/// Create a special file (block or character device, or named pipe).
///
/// # Arguments
/// - `name`: The pathname of the special file
/// - `type`: The type of file (`b` for block, `c` for character, `p` for FIFO)
/// - `major`: Major device number (for block/character devices)
/// - `minor`: Minor device number (for block/character devices)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn mknod(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"mknod: missing operand\n");
        return 1;
    }

    let path = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => {
            io::write_str(2, b"mknod: missing operand\n");
            return 1;
        }
    };

    let type_arg = match unsafe { get_arg(argv, 2) } {
        Some(t) => t,
        None => {
            io::write_str(2, b"mknod: missing operand\n");
            return 1;
        }
    };

    let (mode, dev) = if type_arg == b"p" {
        (libc::S_IFIFO | 0o666, 0)
    } else if argc >= 5 {
        let major = sys::parse_u64(unsafe { get_arg(argv, 3).unwrap_or(b"0") }).unwrap_or(0) as u32;
        let minor = sys::parse_u64(unsafe { get_arg(argv, 4).unwrap_or(b"0") }).unwrap_or(0) as u32;
        let m = if type_arg == b"b" {
            libc::S_IFBLK
        } else {
            libc::S_IFCHR
        };
        (m | 0o666, sys::makedev(major, minor))
    } else {
        io::write_str(2, b"mknod: missing major/minor\n");
        return 1;
    };

    if unsafe { libc::mknod(path.as_ptr() as *const libc::c_char, mode, dev) } < 0 {
        sys::perror(path);
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::os::unix::fs::FileTypeExt;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_mknod_test_{}_{}",
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
    fn test_mknod_fifo() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let fifo = dir.join("myfifo");

        let output = Command::new(&armybox)
            .args(["mknod", fifo.to_str().unwrap(), "p"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(fifo.exists());
        let metadata = fs::metadata(&fifo).unwrap();
        assert!(metadata.file_type().is_fifo());
        cleanup(&dir);
    }

    #[test]
    fn test_mknod_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mknod"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_mknod_missing_type() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let node = dir.join("mynode");

        let output = Command::new(&armybox)
            .args(["mknod", node.to_str().unwrap()])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_mknod_block_missing_major_minor() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let node = dir.join("mynode");

        let output = Command::new(&armybox)
            .args(["mknod", node.to_str().unwrap(), "b"])
            .output()
            .unwrap();

        // Should fail without major/minor for block device
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }
}
