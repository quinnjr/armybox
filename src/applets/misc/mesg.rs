//! mesg - control write access to terminal
//!
//! Display or set write permission for terminal.

use super::get_arg;

/// mesg - control write access to terminal
///
/// # Synopsis
/// ```text
/// mesg [y|n]
/// ```
///
/// # Description
/// Control write access to your terminal.
///
/// # Exit Status
/// - 0: Success
pub fn mesg(argc: i32, argv: *const *const u8) -> i32 {
    if argc > 1 {
        if let Some(arg) = unsafe { get_arg(argv, 1) } {
            let mode = if arg == b"y" { 0o620 } else { 0o600 };
            let tty = unsafe { libc::ttyname(0) };
            if !tty.is_null() {
                unsafe { libc::chmod(tty, mode) };
            }
        }
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
    fn test_mesg_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mesg"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
