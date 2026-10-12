//! POSIX.1-2017 compliance tests for comm
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/comm.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The comm utility shall read file1 and file2... and produce three-column output"
#[test]
fn posix_comm_basic() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\nbanana\ncherry\n").unwrap();
    fs::write(&file2, "banana\ndate\n").unwrap();

    let result = run(&["comm", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    // Column 1: only in file1
    // Column 2: only in file2
    // Column 3: in both
}

/// POSIX: comm -1 suppress column 1
#[test]
fn posix_comm_suppress_1() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\nbanana\n").unwrap();
    fs::write(&file2, "banana\ndate\n").unwrap();

    let result = run(&[
        "comm",
        "-1",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    // apple should not appear (column 1 suppressed)
}

/// POSIX: comm -2 suppress column 2
#[test]
fn posix_comm_suppress_2() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\nbanana\n").unwrap();
    fs::write(&file2, "banana\ndate\n").unwrap();

    let result = run(&[
        "comm",
        "-2",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    // date should not appear (column 2 suppressed)
}

/// POSIX: comm -3 suppress column 3
#[test]
fn posix_comm_suppress_3() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\nbanana\n").unwrap();
    fs::write(&file2, "banana\ndate\n").unwrap();

    let result = run(&[
        "comm",
        "-3",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    // banana should not appear (column 3 suppressed)
}

/// POSIX: comm -12 show only common lines
#[test]
fn posix_comm_common_only() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\nbanana\ncherry\n").unwrap();
    fs::write(&file2, "banana\ncherry\ndate\n").unwrap();

    let result = run(&[
        "comm",
        "-12",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(result.1.contains("banana"));
    assert!(result.1.contains("cherry"));
    assert!(!result.1.contains("apple"));
    assert!(!result.1.contains("date"));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_comm_exit_success() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a\n").unwrap();
    fs::write(&file2, "b\n").unwrap();

    let result = run(&["comm", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}
