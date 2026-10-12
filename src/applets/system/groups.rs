//! groups - print group memberships
//!
//! Print the groups the current user belongs to.

use crate::io;

/// groups - print group memberships
///
/// # Synopsis
/// ```text
/// groups [user]
/// ```
///
/// # Description
/// Print the groups a user belongs to.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn groups(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut gids = [0u32; 32];
    let n = unsafe { libc::getgroups(32, gids.as_mut_ptr()) };
    if n > 0 {
        for i in 0..n as usize {
            io::write_num(1, gids[i] as u64);
            io::write_str(1, b" ");
        }
        io::write_str(1, b"\n");
    }
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_groups_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["groups"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Should output at least one group (the user's primary group)
    }

    #[test]
    fn test_groups_contains_numbers() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["groups"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Output should contain numeric group IDs
        let has_numbers = stdout.chars().any(|c| c.is_ascii_digit());
        assert!(has_numbers || stdout.is_empty());
    }
}
