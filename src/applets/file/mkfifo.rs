//! mkfifo - make FIFOs (named pipes)
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/mkfifo.html

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// Get the process umask without permanently changing it.
fn get_umask() -> u32 {
    let mask = unsafe { libc::umask(0) };
    unsafe { libc::umask(mask) };
    mask as u32
}

/// Parse an octal mode string (e.g. "644").
fn parse_octal(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut val: u32 = 0;
    for &c in s {
        if !(b'0'..=b'7').contains(&c) {
            return None;
        }
        val = val * 8 + (c - b'0') as u32;
    }
    Some(val)
}

/// Parse a mode operand, either octal ("644") or symbolic ("u+rwx,go-w"),
/// relative to `base` (the starting mode, used for symbolic clauses).
fn parse_mode(s: &[u8], base: u32) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    if s[0].is_ascii_digit() {
        return parse_octal(s).map(|v| v & 0o7777);
    }

    let mut mode = base;
    for clause in s.split(|&c| c == b',') {
        if clause.is_empty() {
            return None;
        }
        let mut idx = 0;
        let mut who_mask: u32 = 0;
        while idx < clause.len() && matches!(clause[idx], b'u' | b'g' | b'o' | b'a') {
            who_mask |= match clause[idx] {
                b'u' => 0o700,
                b'g' => 0o070,
                b'o' => 0o007,
                b'a' => 0o777,
                _ => 0,
            };
            idx += 1;
        }
        if who_mask == 0 {
            who_mask = 0o777;
        }
        if idx >= clause.len() {
            return None;
        }
        let op = clause[idx];
        if op != b'+' && op != b'-' && op != b'=' {
            return None;
        }
        idx += 1;

        let mut perm_bits: u32 = 0;
        while idx < clause.len() {
            perm_bits |= match clause[idx] {
                b'r' => 0o444,
                b'w' => 0o222,
                b'x' => 0o111,
                _ => return None,
            };
            idx += 1;
        }

        let applied = perm_bits & who_mask;
        match op {
            b'+' => mode |= applied,
            b'-' => mode &= !applied,
            b'=' => {
                mode &= !who_mask;
                mode |= applied;
            }
            _ => {}
        }
    }
    Some(mode & 0o7777)
}

/// mkfifo - make FIFOs (named pipes)
///
/// # Synopsis
/// ```text
/// mkfifo [-m mode] file...
/// ```
///
/// # Description
/// Create FIFO special files (named pipes).
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn mkfifo(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode: u32 = 0o666 & !get_umask();
    let mut files: Vec<&[u8]> = Vec::new();
    let mut only_files = false;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => {
                i += 1;
                continue;
            }
        };

        if only_files {
            files.push(arg);
        } else if arg == b"--" {
            only_files = true;
        } else if arg == b"-m" {
            i += 1;
            let m = match unsafe { get_arg(argv, i) } {
                Some(m) => m,
                None => {
                    io::write_str(2, b"mkfifo: option requires an argument -- 'm'\n");
                    return 2;
                }
            };
            match parse_mode(m, mode) {
                Some(v) => mode = v,
                None => {
                    io::write_str(2, b"mkfifo: invalid mode: '");
                    io::write_all(2, m);
                    io::write_str(2, b"'\n");
                    return 1;
                }
            }
        } else if arg.starts_with(b"-m") && arg.len() > 2 {
            match parse_mode(&arg[2..], mode) {
                Some(v) => mode = v,
                None => {
                    io::write_str(2, b"mkfifo: invalid mode: '");
                    io::write_all(2, &arg[2..]);
                    io::write_str(2, b"'\n");
                    return 1;
                }
            }
        } else if arg.len() > 1 && arg[0] == b'-' {
            io::write_str(2, b"mkfifo: invalid option -- '");
            io::write_all(2, &arg[1..]);
            io::write_str(2, b"'\n");
            io::write_str(2, b"Usage: mkfifo [-m mode] file...\n");
            return 2;
        } else {
            files.push(arg);
        }

        i += 1;
    }

    if files.is_empty() {
        io::write_str(2, b"mkfifo: missing operand\n");
        return 2;
    }

    let mut exit_code = 0;
    for path in files {
        if unsafe { libc::mkfifo(path.as_ptr() as *const libc::c_char, mode as libc::mode_t) } < 0 {
            sys::perror(path);
            exit_code = 1;
        }
    }
    exit_code
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
            "armybox_mkfifo_test_{}_{}",
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
    fn test_mkfifo_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let fifo = dir.join("myfifo");

        let output = Command::new(&armybox)
            .args(["mkfifo", fifo.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(fifo.exists());
        let metadata = fs::metadata(&fifo).unwrap();
        assert!(metadata.file_type().is_fifo());
        cleanup(&dir);
    }

    #[test]
    fn test_mkfifo_multiple() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let fifo1 = dir.join("fifo1");
        let fifo2 = dir.join("fifo2");

        let output = Command::new(&armybox)
            .args(["mkfifo", fifo1.to_str().unwrap(), fifo2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(fs::metadata(&fifo1).unwrap().file_type().is_fifo());
        assert!(fs::metadata(&fifo2).unwrap().file_type().is_fifo());
        cleanup(&dir);
    }

    #[test]
    fn test_mkfifo_exists() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let fifo = dir.join("myfifo");
        // Create the fifo first (need null-terminated path for libc)
        let fifo_cstr = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        unsafe { libc::mkfifo(fifo_cstr.as_ptr(), 0o644) };

        let output = Command::new(&armybox)
            .args(["mkfifo", fifo.to_str().unwrap()])
            .output()
            .unwrap();

        // Should fail if fifo already exists
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_mkfifo_no_permission() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Try creating in /root which should fail for non-root
        if std::env::var("USER").map(|u| u == "root").unwrap_or(false) {
            return; // Skip if root
        }

        let output = Command::new(&armybox)
            .args(["mkfifo", "/root/testfifo"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_mkfifo_mode_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let fifo = dir.join("myfifo");

        let output = Command::new(&armybox)
            .args(["mkfifo", "-m", "600", fifo.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&fifo).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
        cleanup(&dir);
    }
}
