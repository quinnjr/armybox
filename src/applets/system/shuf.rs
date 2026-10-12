//! shuf - generate random permutations
//!
//! Write a random permutation of the input lines to standard output.

use crate::applets::get_arg;
use crate::io;
use alloc::vec::Vec;

/// shuf - generate random permutations
///
/// # Synopsis
/// ```text
/// shuf [file]
/// ```
///
/// # Description
/// Write a random permutation of the input lines to standard output.
/// If no file is specified, reads from standard input.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn shuf(argc: i32, argv: *const *const u8) -> i32 {
    // Read lines from stdin or file
    let fd = if argc > 1 {
        let path = unsafe { get_arg(argv, 1).unwrap() };
        if path[0] != b'-' {
            io::open(path, libc::O_RDONLY, 0)
        } else {
            0
        }
    } else {
        0
    };

    if fd < 0 {
        return 1;
    }

    let content = io::read_all(fd);
    if fd > 0 {
        io::close(fd);
    }

    let mut lines: Vec<&[u8]> = content
        .split(|&c| c == b'\n')
        .filter(|l| !l.is_empty())
        .collect();

    // Fisher-Yates shuffle using simple PRNG seeded from time
    let mut seed = unsafe { libc::time(core::ptr::null_mut()) } as u64;
    let n = lines.len();

    for i in (1..n).rev() {
        // Simple LCG PRNG
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (seed >> 33) as usize % (i + 1);
        lines.swap(i, j);
    }

    for line in lines {
        io::write_all(1, line);
        io::write_str(1, b"\n");
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

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
    fn test_shuf_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let pid = std::process::id();
        let test_file = std::env::temp_dir().join(std::format!("armybox_shuf_test_{}", pid));
        fs::write(&test_file, "a\nb\nc\nd\ne\n").unwrap();

        let output = Command::new(&armybox)
            .args(["shuf", test_file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // Verify output contains all lines (in some order)
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let mut lines: std::vec::Vec<&str> = stdout.lines().collect();
        lines.sort();
        assert_eq!(lines, std::vec!["a", "b", "c", "d", "e"]);

        let _ = fs::remove_file(test_file);
    }

    #[test]
    fn test_shuf_stdin() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["shuf"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"1\n2\n3\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Verify output contains all lines
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let mut lines: std::vec::Vec<&str> = stdout.lines().collect();
        lines.sort();
        assert_eq!(lines, std::vec!["1", "2", "3"]);
    }
}
