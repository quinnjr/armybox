//! arch - print machine hardware name
//!
//! Prints the machine hardware name (same as uname -m).

use crate::io;

/// arch - print machine hardware name
///
/// # Synopsis
/// ```text
/// arch
/// ```
///
/// # Description
/// Print the machine hardware name, equivalent to `uname -m`.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn arch(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut uts: libc::utsname = unsafe { core::mem::zeroed() };
    if io::uname(&mut uts) != 0 {
        return 1;
    }
    io::write_all(1, unsafe {
        io::cstr_to_slice(uts.machine.as_ptr() as *const u8)
    });
    io::write_str(1, b"\n");
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
    fn test_arch_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["arch"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_arch_matches_uname_m() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let arch_output = Command::new(&armybox).args(["arch"]).output().unwrap();

        let uname_output = Command::new(&armybox)
            .args(["uname", "-m"])
            .output()
            .unwrap();

        let arch_str = std::string::String::from_utf8_lossy(&arch_output.stdout);
        let uname_str = std::string::String::from_utf8_lossy(&uname_output.stdout);
        assert_eq!(arch_str.trim(), uname_str.trim());
    }
}
