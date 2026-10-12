//! POSIX.1-2017 compliance tests for cmp
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cmp.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The cmp utility shall compare two files"
#[test]
fn posix_cmp_identical() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "identical content").unwrap();
    fs::write(&file2, "identical content").unwrap();

    let result = run(&["cmp", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_eq!(result.0, 0);
    assert!(result.1.is_empty()); // No output for identical files
}

/// POSIX: cmp different files
#[test]
fn posix_cmp_different() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "content A").unwrap();
    fs::write(&file2, "content B").unwrap();

    let result = run(&["cmp", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_eq!(result.0, 1);
    // Should report location of first difference
    assert!(result.1.contains("differ"));
}

/// POSIX: cmp -l verbose output
#[test]
fn posix_cmp_verbose() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "abc").unwrap();
    fs::write(&file2, "aXc").unwrap();

    let result = run(&[
        "cmp",
        "-l",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_eq!(result.0, 1);
    // Should list byte positions
}

/// POSIX: cmp -s silent mode
#[test]
fn posix_cmp_silent() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "content A").unwrap();
    fs::write(&file2, "content B").unwrap();

    let result = run(&[
        "cmp",
        "-s",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_eq!(result.0, 1);
    assert!(result.1.is_empty()); // No output in silent mode
}

/// POSIX: Exit status 0 for identical
#[test]
fn posix_cmp_exit_identical() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "same").unwrap();
    fs::write(&file2, "same").unwrap();

    let result = run(&["cmp", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status 1 for different
#[test]
fn posix_cmp_exit_different() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "A").unwrap();
    fs::write(&file2, "B").unwrap();

    let result = run(&["cmp", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_eq!(result.0, 1);
}

/// POSIX: Exit status >1 on error
#[test]
fn posix_cmp_exit_error() {
    let result = run(&["cmp", "/nonexistent/file1", "/nonexistent/file2"]);
    assert!(result.0 > 1);
}

/// POSIX: cmp with stdin
#[test]
fn posix_cmp_stdin() {
    let dir = setup_test_env();
    let file = dir.path().join("file");
    fs::write(&file, "content").unwrap();

    let result = run_with_stdin(&["cmp", file.to_str().unwrap(), "-"], b"content");
    assert_eq!(result.0, 0);
}
