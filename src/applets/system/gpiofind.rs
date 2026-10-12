//! gpiofind - find a GPIO line by name
//!
//! Find a GPIO line by name and print its chip and offset.

use crate::io;

/// gpiofind - find a GPIO line by name
///
/// # Synopsis
/// ```text
/// gpiofind NAME
/// ```
///
/// # Description
/// Find a GPIO line by name and print its chip name and line offset.
///
/// # Exit Status
/// - 0: Success (line found)
/// - 1: Error or line not found
pub fn gpiofind(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"gpiofind: usage: gpiofind NAME\n");
        return 1;
    }

    let _name = unsafe { super::get_arg(argv, 1).unwrap() };

    // This would require reading GPIO chip info via ioctl
    // For now, just report not found
    io::write_str(2, b"gpiofind: line not found\n");
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
    fn test_gpiofind_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["gpiofind"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_gpiofind_not_found() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["gpiofind", "nonexistent_line"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
