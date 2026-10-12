//! POSIX.1-2017 compliance tests for paste
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/paste.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The paste utility shall concatenate the corresponding lines of the given input files"
#[test]
fn posix_paste_basic() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a\nb\nc\n").unwrap();
    fs::write(&file2, "1\n2\n3\n").unwrap();

    let result = run(&["paste", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("a\t1"));
    assert!(result.1.contains("b\t2"));
    assert!(result.1.contains("c\t3"));
}

/// POSIX: paste -d delimiter
#[test]
fn posix_paste_delimiter() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a\nb\n").unwrap();
    fs::write(&file2, "1\n2\n").unwrap();

    let result = run(&[
        "paste",
        "-d:",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(result.1.contains("a:1"));
    assert!(result.1.contains("b:2"));
}

/// POSIX: paste -s serial (one file per line)
#[test]
fn posix_paste_serial() {
    let dir = setup_test_env();
    let file = dir.path().join("file");
    fs::write(&file, "a\nb\nc\n").unwrap();

    let result = run(&["paste", "-s", file.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(result.1, "a\tb\tc\n");
}

/// POSIX: paste from stdin
#[test]
fn posix_paste_stdin() {
    let dir = setup_test_env();
    let file = dir.path().join("file");
    fs::write(&file, "1\n2\n").unwrap();

    let result = run_with_stdin(&["paste", "-", file.to_str().unwrap()], b"a\nb\n");
    assert_success(&result);
    assert!(result.1.contains("a\t1"));
}

/// POSIX: paste multiple delimiters (cycle)
#[test]
fn posix_paste_multi_delim() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    let file3 = dir.path().join("file3");
    fs::write(&file1, "a\n").unwrap();
    fs::write(&file2, "b\n").unwrap();
    fs::write(&file3, "c\n").unwrap();

    let result = run(&[
        "paste",
        "-d:,",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
        file3.to_str().unwrap(),
    ]);
    assert_success(&result);
    // Delimiters cycle: :, :, ...
    assert!(result.1.contains("a:b,c"));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_paste_exit_success() {
    let result = run_with_stdin(&["paste", "-"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: paste uneven files
#[test]
fn posix_paste_uneven() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a\nb\nc\n").unwrap();
    fs::write(&file2, "1\n").unwrap();

    let result = run(&["paste", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    // Missing entries are empty
    assert!(result.1.contains("a\t1"));
}

/// POSIX: paste -d '' no delimiter
#[test]
fn posix_paste_no_delim() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a\n").unwrap();
    fs::write(&file2, "b\n").unwrap();

    let result = run(&[
        "paste",
        "-d",
        "",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    // -d '' means "no delimiter": the fields are concatenated directly.
    assert_success(&result);
    assert_eq!(result.1, "ab\n");
}
