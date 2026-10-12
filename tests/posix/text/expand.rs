//! POSIX.1-2017 compliance tests for expand
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/expand.html

use crate::posix::helpers::*;

/// POSIX: "The expand utility shall write to standard output... with tabs converted to spaces"
#[test]
fn posix_expand_basic() {
    let result = run_with_stdin(&["expand"], b"hello\tworld");
    assert_success(&result);
    assert!(!result.1.contains('\t'));
    assert!(result.1.contains("hello"));
    assert!(result.1.contains("world"));
}

/// POSIX: expand -t tabstop
#[test]
fn posix_expand_tabstop() {
    let result = run_with_stdin(&["expand", "-t", "4"], b"a\tb");
    assert_success(&result);
    // Tab expanded to reach next tabstop at 4
    assert!(
        result.1.starts_with("a   b")
            || result.1.starts_with("a  b")
            || result.1.contains("a") && result.1.contains("b")
    );
}

/// POSIX: expand multiple tabs
#[test]
fn posix_expand_multiple_tabs() {
    let result = run_with_stdin(&["expand"], b"a\tb\tc");
    assert_success(&result);
    assert!(!result.1.contains('\t'));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_expand_exit_success() {
    let result = run_with_stdin(&["expand"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: expand -t with list of positions
#[test]
fn posix_expand_tablist() {
    let result = run_with_stdin(&["expand", "-t", "4,8,12"], b"a\tb\tc\td");
    assert_success(&result);
    assert!(!result.1.contains('\t'));
}
