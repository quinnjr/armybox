//! nproc - print the number of processing units available
//!
//! Print the number of processing units (CPUs) available.

use crate::io;

/// nproc - print the number of processing units available
///
/// # Synopsis
/// ```text
/// nproc
/// ```
///
/// # Description
/// Print the number of processing units (CPUs) available to the
/// current process.
///
/// # Exit Status
/// - 0: Success
pub fn nproc(argc: i32, argv: *const *const u8) -> i32 {
    let _ = argc;
    let _ = argv;
    io::write_num(1, unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }
        as u64);
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
    fn test_nproc_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nproc"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let nproc: u64 = stdout.trim().parse().unwrap();
        // Should have at least 1 CPU
        assert!(nproc >= 1);
    }
}
