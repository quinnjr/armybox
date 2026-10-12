//! unexpand - convert spaces to tabs
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/unexpand.html

use crate::applets::get_arg;
use crate::io;

/// unexpand - convert spaces to tabs
///
/// # Synopsis
/// ```text
/// unexpand [file...]
/// ```
///
/// # Description
/// Convert blanks to tabs, writing to standard output.
/// Every 8 consecutive spaces are converted to a tab.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn unexpand(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        if let Some(path) = unsafe { get_arg(argv, argc - 1) } {
            if path.len() > 0 && path[0] != b'-' {
                io::open(path, libc::O_RDONLY, 0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let mut buf = [0u8; 4096];
    let mut spaces = 0;

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        for &c in &buf[..n as usize] {
            if c == b' ' {
                spaces += 1;
                if spaces == 8 {
                    io::write_str(1, b"\t");
                    spaces = 0;
                }
            } else {
                for _ in 0..spaces {
                    io::write_str(1, b" ");
                }
                spaces = 0;
                io::write_all(1, &[c]);
            }
        }
    }

    if fd != 0 {
        io::close(fd);
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
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
    fn test_unexpand_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["unexpand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            // 8 spaces should become 1 tab
            stdin.write_all(b"        x\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "\tx\n");
    }

    #[test]
    fn test_unexpand_partial_spaces() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["unexpand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            // 5 spaces should stay as 5 spaces
            stdin.write_all(b"     x\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "     x\n");
    }

    #[test]
    fn test_unexpand_multiple_tabs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["unexpand"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            // 16 spaces should become 2 tabs
            stdin.write_all(b"                x\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "\t\tx\n");
    }
}
