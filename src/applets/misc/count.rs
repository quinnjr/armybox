//! count - count bytes from stdin
//!
//! A simple utility to count bytes read from standard input.

use crate::io;

/// count - count bytes from stdin
///
/// # Synopsis
/// ```text
/// count
/// ```
///
/// # Description
/// Counts the number of bytes read from stdin and prints the total.
///
/// # Exit Status
/// - 0: Success
pub fn count(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut count = 0u64;
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(0, &mut buf);
        if n <= 0 {
            break;
        }
        count += n as u64;
    }
    io::write_num(1, count);
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_count() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["count"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello world").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "11");
    }
}
