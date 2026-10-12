//! POSIX.1-2017 compliance tests for uname
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/uname.html

use crate::posix::helpers::*;

/// POSIX: "The uname utility shall write information identifying the current system"
#[test]
fn posix_uname_basic() {
    let result = run(&["uname"]);
    assert_success(&result);
    assert!(!result.1.is_empty());
}

/// POSIX: uname -s system name
#[test]
fn posix_uname_system() {
    let result = run(&["uname", "-s"]);
    assert_success(&result);
    assert!(result.1.trim() == "Linux" || !result.1.is_empty());
}

/// POSIX: uname -n node name
#[test]
fn posix_uname_nodename() {
    let result = run(&["uname", "-n"]);
    assert_success(&result);
    assert!(!result.1.trim().is_empty());
}

/// POSIX: uname -r release
#[test]
fn posix_uname_release() {
    let result = run(&["uname", "-r"]);
    assert_success(&result);
    assert!(!result.1.trim().is_empty());
}

/// POSIX: uname -v version
#[test]
fn posix_uname_version() {
    let result = run(&["uname", "-v"]);
    assert_success(&result);
    assert!(!result.1.trim().is_empty());
}

/// POSIX: uname -m machine
#[test]
fn posix_uname_machine() {
    let result = run(&["uname", "-m"]);
    assert_success(&result);
    assert!(!result.1.trim().is_empty());
}

/// POSIX: uname -a all information
#[test]
fn posix_uname_all() {
    let result = run(&["uname", "-a"]);
    assert_success(&result);
    // Should have multiple space-separated fields
    let fields: Vec<&str> = result.1.split_whitespace().collect();
    assert!(fields.len() >= 3);
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_uname_exit_success() {
    let result = run(&["uname"]);
    assert_eq!(result.0, 0);
}
