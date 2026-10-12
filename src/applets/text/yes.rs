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

    use std::process::{Command, Stdio};

    /// Read stdout until `line` appears (or EOF / 4 KiB). A single read() can
    /// return a partial line, since yes may write the text and the newline
    /// separately
    fn read_until(child: &mut std::process::Child, line: &[u8]) -> std::vec::Vec<u8> {
        let mut buf = std::vec::Vec::new();
        let mut chunk = [0u8; 256];
        if let Some(ref mut stdout) = child.stdout {
            while buf.len() < 4096 && !buf.windows(line.len()).any(|w| w == line) {
                match stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
        }
        buf
    }

    use crate::test_utils::test_helpers::get_armybox_path;

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

        let buf = read_until(&mut child, b"y\n");

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

        let buf = read_until(&mut child, b"hello\n");

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
