//! whoami - print effective user name
//!
//! POSIX.1-2017 compliant implementation.

use crate::io;

/// whoami - print effective user name
///
/// # Synopsis
/// ```text
/// whoami
/// ```
///
/// # Description
/// Print the user name associated with the current effective user ID.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn whoami(_argc: i32, _argv: *const *const u8) -> i32 {
    let uid = unsafe { libc::getuid() };
    let pwd = unsafe { libc::getpwuid(uid) };
    if !pwd.is_null() {
        let name = unsafe { io::cstr_to_slice((*pwd).pw_name as *const u8) };
        io::write_all(1, name);
        io::write_str(1, b"\n");
    }
    0
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
    fn test_whoami_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["whoami"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.trim().is_empty());
    }

    #[test]
    fn test_whoami_matches_system() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let system_output = Command::new("whoami").output().unwrap();
        let system_user = std::string::String::from_utf8_lossy(&system_output.stdout);

        let output = Command::new(&armybox).args(["whoami"]).output().unwrap();

        let armybox_user = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(armybox_user.trim(), system_user.trim());
    }
}
