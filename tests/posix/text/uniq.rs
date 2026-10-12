//! POSIX.1-2017 compliance tests for uniq
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/uniq.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The uniq utility shall read input... comparing adjacent lines"
#[test]
fn posix_uniq_basic() {
    let result = run_with_stdin(&["uniq"], b"apple\napple\nbanana\nbanana\ncherry");
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\ncherry\n");
}

/// POSIX: uniq -c count occurrences
#[test]
fn posix_uniq_count() {
    let result = run_with_stdin(&["uniq", "-c"], b"apple\napple\nbanana");
    assert_success(&result);
    assert!(result.1.contains("2") && result.1.contains("apple"));
    assert!(result.1.contains("1") && result.1.contains("banana"));
}

/// POSIX: uniq -d only duplicates
#[test]
fn posix_uniq_duplicates() {
    let result = run_with_stdin(&["uniq", "-d"], b"apple\napple\nbanana\ncherry\ncherry");
    assert_success(&result);
    assert!(result.1.contains("apple"));
    assert!(result.1.contains("cherry"));
    assert!(!result.1.contains("banana"));
}

/// POSIX: uniq -u only unique
#[test]
fn posix_uniq_unique() {
    let result = run_with_stdin(&["uniq", "-u"], b"apple\napple\nbanana\ncherry\ncherry");
    assert_success(&result);
    assert!(!result.1.contains("apple"));
    assert!(!result.1.contains("cherry"));
    assert!(result.1.contains("banana"));
}

/// POSIX: uniq -f skip fields
#[test]
fn posix_uniq_skip_fields() {
    let result = run_with_stdin(&["uniq", "-f1"], b"a apple\nb apple\nc banana");
    assert_success(&result);
    assert_eq!(result.1, "a apple\nc banana\n");
}

/// POSIX: uniq -s skip chars
#[test]
fn posix_uniq_skip_chars() {
    let result = run_with_stdin(&["uniq", "-s1"], b"aapple\nbapple\ncbanana");
    assert_success(&result);
    assert_eq!(result.1, "aapple\ncbanana\n");
}

/// POSIX: uniq from file
#[test]
fn posix_uniq_from_file() {
    let dir = setup_test_env();
    let file = dir.path().join("input");
    fs::write(&file, "apple\napple\nbanana").unwrap();

    let result = run(&["uniq", file.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\n");
}

/// POSIX: uniq output to file
#[test]
fn posix_uniq_output() {
    let dir = setup_test_env();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    fs::write(&input, "apple\napple\nbanana").unwrap();

    let result = run(&["uniq", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&output).unwrap(), "apple\nbanana\n");
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_uniq_exit_success() {
    let result = run_with_stdin(&["uniq"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: uniq -i ignore case
#[test]
fn posix_uniq_ignore_case() {
    let result = run_with_stdin(&["uniq", "-i"], b"Apple\napple\nBANANA");
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 2);
}
