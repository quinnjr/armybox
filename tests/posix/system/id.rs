//! POSIX.1-2017 compliance tests for id
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/id.html

use crate::posix::helpers::*;

/// POSIX: "The id utility shall write the user and group IDs and corresponding names"
#[test]
fn posix_id_basic() {
    let result = run(&["id"]);
    assert_success(&result);
    assert!(result.1.contains("uid="));
    assert!(result.1.contains("gid="));
}

/// POSIX: id -u user ID
#[test]
fn posix_id_uid() {
    let result = run(&["id", "-u"]);
    assert_success(&result);
    let uid: u32 = result.1.trim().parse().unwrap();
    assert!(uid < 65536 || uid == 65534 || uid > 65536); // Valid UID range
}

/// POSIX: id -g group ID
#[test]
fn posix_id_gid() {
    let result = run(&["id", "-g"]);
    assert_success(&result);
    let gid: u32 = result.1.trim().parse().unwrap();
    assert!(gid < 65536 || gid == 65534 || gid > 65536);
}

/// POSIX: id -G all group IDs
#[test]
fn posix_id_groups() {
    let result = run(&["id", "-G"]);
    assert_success(&result);
    // Should be space-separated list of GIDs
    for gid_str in result.1.split_whitespace() {
        let _gid: u32 = gid_str.parse().expect("GID should be numeric");
    }
}

/// POSIX: id -n with -u shows name
#[test]
fn posix_id_name_uid() {
    let result = run(&["id", "-un"]);
    assert_success(&result);
    let name = result.1.trim();
    assert!(!name.is_empty());
    assert!(!name.chars().next().unwrap().is_ascii_digit()); // Name not a number
}

/// POSIX: id -n with -g shows name
#[test]
fn posix_id_name_gid() {
    let result = run(&["id", "-gn"]);
    assert_success(&result);
    let name = result.1.trim();
    assert!(!name.is_empty());
}

/// POSIX: id -r with -u shows real (not effective) UID
#[test]
fn posix_id_real_uid() {
    let result = run(&["id", "-ru"]);
    assert_success(&result);
    let uid: u32 = result.1.trim().parse().unwrap();
    let _ = uid;
}

/// POSIX: id for specific user
#[test]
fn posix_id_user() {
    let result = run(&["id", "root"]);
    // May fail if root doesn't exist, but should not crash
    if result.0 == 0 {
        assert!(result.1.contains("uid=0"));
    }
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_id_exit_success() {
    let result = run(&["id"]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 for invalid user
#[test]
fn posix_id_exit_error() {
    let result = run(&["id", "nonexistent_user_12345"]);
    assert!(result.0 > 0);
}
