//! POSIX.1-2017 compliance tests for wc
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/wc.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The wc utility shall read input files... counting lines, words, and bytes"
#[test]
fn posix_wc_basic() {
    let result = run_with_stdin(&["wc"], b"hello world\n");
    assert_success(&result);
    // Default: lines, words, bytes
    assert!(result.1.contains("1")); // 1 line
    assert!(result.1.contains("2")); // 2 words
    assert!(result.1.contains("12")); // 12 bytes
}

/// POSIX: wc -l lines only
#[test]
fn posix_wc_lines() {
    let result = run_with_stdin(&["wc", "-l"], b"line1\nline2\nline3\n");
    assert_success(&result);
    assert!(result.1.trim().starts_with("3"));
}

/// POSIX: wc -w words only
#[test]
fn posix_wc_words() {
    let result = run_with_stdin(&["wc", "-w"], b"one two three four");
    assert_success(&result);
    assert!(result.1.trim().starts_with("4"));
}

/// POSIX: wc -c bytes only
#[test]
fn posix_wc_bytes() {
    let result = run_with_stdin(&["wc", "-c"], b"hello");
    assert_success(&result);
    assert!(result.1.trim().starts_with("5"));
}

/// POSIX: wc -m characters (may differ from bytes for multibyte)
#[test]
fn posix_wc_chars() {
    let result = run_with_stdin(&["wc", "-m"], b"hello");
    assert_success(&result);
    assert!(result.1.trim().starts_with("5"));
}

/// POSIX: wc from file
#[test]
fn posix_wc_from_file() {
    let dir = setup_test_env();
    let file = dir.path().join("input");
    fs::write(&file, "hello world\n").unwrap();

    let result = run(&["wc", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("1")); // 1 line
}

/// POSIX: wc multiple files
#[test]
fn posix_wc_multiple_files() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "hello\n").unwrap();
    fs::write(&file2, "world\n").unwrap();

    let result = run(&["wc", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("total"));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_wc_exit_success() {
    let result = run_with_stdin(&["wc"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_wc_exit_error() {
    let result = run(&["wc", "/nonexistent/file"]);
    assert!(result.0 > 0);
}

/// GNU-style layout: count columns are right-justified to a common width
/// (the digit width of the largest displayed count), so a single-digit count
/// carries a leading space to line up under a two-digit count.
#[test]
fn posix_wc_column_padding() {
    // 5 lines, 5 words, 20 bytes ("one\n","two\n","six\n","ten\n","fun\n").
    // Largest count is 20 (two digits), so every column is width 2.
    let result = run_with_stdin(&["wc"], b"one\ntwo\nsix\nten\nfun\n");
    assert_success(&result);
    assert_eq!(result.1, " 5  5 20\n");
}

/// --help prints usage and exits 0, even though 'l' is a valid short flag.
#[test]
fn posix_wc_help() {
    let result = run(&["wc", "--help"]);
    assert_eq!(result.0, 0);
    assert!(result.1.contains("usage"));
}
