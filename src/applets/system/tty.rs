//! tty - print the file name of the terminal
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tty.html

use crate::io;

/// tty - print the file name of the terminal
///
/// # Synopsis
/// ```text
/// tty
/// ```
///
/// # Description
/// Print the file name of the terminal connected to standard input.
///
/// # Exit Status
/// - 0: Standard input is a terminal
/// - 1: Standard input is not a terminal
/// - >1: An error occurred
pub fn tty(_argc: i32, _argv: *const *const u8) -> i32 {
    let name = unsafe { libc::ttyname(0) };
    if !name.is_null() {
        io::write_all(1, unsafe { io::cstr_to_slice(name as *const u8) });
        io::write_str(1, b"\n");
        0
    } else {
        io::write_str(1, b"not a tty\n");
        1
    }
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
    fn test_tty_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["tty"]).output().unwrap();

        // Exit code 0 or 1 is valid (depending on whether stdin is a tty)
        assert!(output.status.code() == Some(0) || output.status.code() == Some(1));
    }

    #[test]
    fn test_tty_no_tty_in_pipe() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // When run in a pipe/subprocess, stdin is not a tty
        let output = Command::new(&armybox).args(["tty"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should either be a tty path or "not a tty"
        assert!(stdout.contains("/") || stdout.contains("not a tty"));
    }
}
