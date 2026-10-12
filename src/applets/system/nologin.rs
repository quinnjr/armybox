//! nologin - politely refuse a login
//!
//! Display a message that an account is not available.

use crate::io;

/// nologin - politely refuse a login
///
/// # Synopsis
/// ```text
/// nologin
/// ```
///
/// # Description
/// Display a message that this account is currently not available
/// and exit non-zero. Intended to be used as a shell field for accounts
/// that should not allow interactive logins.
///
/// # Exit Status
/// - 1: Always (login refused)
pub fn nologin(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"This account is currently not available.\n");
    1
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
    fn test_nologin_exits_nonzero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nologin"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_nologin_message() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nologin"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("not available"));
    }
}
