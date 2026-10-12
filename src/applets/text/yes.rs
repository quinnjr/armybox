//! yes - output a string repeatedly
//!
//! Non-POSIX utility (common in Unix systems).
//! Repeatedly outputs a string (default "y").

use crate::applets::get_arg;
use crate::io;

/// yes - output a string repeatedly
///
/// # Synopsis
/// ```text
/// yes [string]
/// ```
///
/// # Description
/// Repeatedly output a line with all specified STRING(s), or 'y'.
/// Note: This command runs indefinitely until killed or pipe breaks.
///
/// # Exit Status
/// - 0: Success (when terminated by broken pipe)
/// - >0: An error occurred
pub fn yes(argc: i32, argv: *const *const u8) -> i32 {
    let text = if argc > 1 {
        unsafe { get_arg(argv, 1).unwrap_or(b"y") }
    } else {
        b"y"
    };

    loop {
        io::write_all(1, text);
        io::write_str(1, b"\n");
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Read;
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
    fn test_yes_default() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["yes"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        // Read a small amount of output
        let mut buf = [0u8; 100];
        if let Some(ref mut stdout) = child.stdout {
            let _ = stdout.read(&mut buf);
        }

        // Kill the process
        let _ = child.kill();
        let _ = child.wait();

        // Verify output contains "y"
        let output = std::string::String::from_utf8_lossy(&buf);
        assert!(output.contains("y\n"));
    }

    #[test]
    fn test_yes_custom_string() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["yes", "hello"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        // Read a small amount of output
        let mut buf = [0u8; 100];
        if let Some(ref mut stdout) = child.stdout {
            let _ = stdout.read(&mut buf);
        }

        // Kill the process
        let _ = child.kill();
        let _ = child.wait();

        // Verify output contains "hello"
        let output = std::string::String::from_utf8_lossy(&buf);
        assert!(output.contains("hello\n"));
    }

    #[test]
    fn test_yes_with_head() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Use head to limit output
        let output = Command::new("sh")
            .args(["-c", &format!("{} yes | head -n 5", armybox.display())])
            .output()
            .unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 5);
        for line in lines {
            assert_eq!(line, "y");
        }
    }
}
