//! help - show help
//!
//! Displays basic usage information for armybox.

use crate::io;

/// help - show help
///
/// # Synopsis
/// ```text
/// help
/// ```
///
/// # Description
/// Displays basic usage information for armybox.
///
/// # Exit Status
/// - 0: Success
pub fn help(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(
        1,
        b"armybox - BusyBox/Toybox compatible multi-call binary\n",
    );
    io::write_str(1, b"Usage: armybox [APPLET] [ARGS]\n");
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
    fn test_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["help"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("armybox"));
    }
}
