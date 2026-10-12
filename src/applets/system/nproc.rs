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

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

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
