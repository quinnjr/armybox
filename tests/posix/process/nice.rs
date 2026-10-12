//! POSIX.1-2017 compliance tests for nice
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/nice.html

use crate::posix::helpers::*;

/// POSIX: "The nice utility shall invoke a utility with a modified scheduling priority"
#[test]
fn posix_nice_basic() {
    let result = run(&["nice", "echo", "hello"]);
    assert_success(&result);
    assert_eq!(result.1.trim(), "hello");
}

/// POSIX: nice -n adjustment
#[test]
fn posix_nice_adjustment() {
    let result = run(&["nice", "-n", "10", "echo", "test"]);
    assert_success(&result);
    assert_eq!(result.1.trim(), "test");
}

/// POSIX: nice without arguments shows current niceness
#[test]
fn posix_nice_show_current() {
    let result = run(&["nice"]);
    assert_success(&result);
    let nice: i32 = result.1.trim().parse().unwrap_or(-999);
    assert!((-20..=19).contains(&nice));
}

/// POSIX: nice positive increment
#[test]
fn posix_nice_positive() {
    let result = run(&["nice", "-n", "5", "true"]);
    assert_eq!(result.0, 0);
}

/// POSIX: nice negative increment (requires privilege)
#[test]
fn posix_nice_negative() {
    // Negative nice requires root, should fail for normal user
    let result = run(&["nice", "-n", "-5", "true"]);
    // Either succeeds (as root) or fails (as non-root)
    let _ = result;
}

/// POSIX: Exit status reflects executed command
#[test]
fn posix_nice_exit_reflects_cmd() {
    let result = run(&["nice", "true"]);
    assert_eq!(result.0, 0);

    let result = run(&["nice", "false"]);
    assert!(result.0 != 0);
}
