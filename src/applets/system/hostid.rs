//! hostid - print the numeric identifier for the current host
//!
//! Print the numeric identifier (host id) of the current host.

use crate::io;

/// hostid - print the numeric identifier for the current host
///
/// # Synopsis
/// ```text
/// hostid
/// ```
///
/// # Description
/// Print the numeric identifier (a 32-bit number) of the current host.
///
/// # Exit Status
/// - 0: Success
pub fn hostid(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    io::write_num(1, crate::sys::gethostid() as u64);
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_hostid_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["hostid"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should output a number
        assert!(stdout.trim().parse::<u64>().is_ok());
    }
}
