//! POSIX.1-2017 compliance tests for cat
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cat.html

use crate::posix::{get_armybox_path, setup_test_env};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

/// POSIX: "The cat utility shall read files in sequence..."
#[test]
fn posix_cat_sequential_read() {
    let dir = setup_test_env();
    fs::write(dir.path().join("a"), "AAA").unwrap();
    fs::write(dir.path().join("b"), "BBB").unwrap();
    fs::write(dir.path().join("c"), "CCC").unwrap();

    let output = Command::new(get_armybox_path())
        .args([
            "cat",
            dir.path().join("a").to_str().unwrap(),
            dir.path().join("b").to_str().unwrap(),
            dir.path().join("c").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(String::from_utf8_lossy(&output.stdout), "AAABBBCCC");
}

/// POSIX: "If no file operands are specified, cat shall read from stdin"
#[test]
fn posix_cat_stdin_default() {
    let mut child = Command::new(get_armybox_path())
        .args(["cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"test input").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "test input");
}

/// POSIX: "If a file is '-', cat shall read from stdin at that point"
#[test]
fn posix_cat_stdin_dash() {
    let dir = setup_test_env();
    fs::write(dir.path().join("file"), "FILE").unwrap();

    let mut child = Command::new(get_armybox_path())
        .args(["cat", dir.path().join("file").to_str().unwrap(), "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"STDIN").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "FILESTDIN");
}

/// POSIX: "-u option shall have no effect (always unbuffered)"
#[test]
fn posix_cat_unbuffered() {
    let dir = setup_test_env();
    fs::write(dir.path().join("file"), "content").unwrap();

    let output = Command::new(get_armybox_path())
        .args(["cat", "-u", dir.path().join("file").to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "content");
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_cat_exit_success() {
    let dir = setup_test_env();
    fs::write(dir.path().join("file"), "x").unwrap();

    let output = Command::new(get_armybox_path())
        .args(["cat", dir.path().join("file").to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_cat_exit_error() {
    let output = Command::new(get_armybox_path())
        .args(["cat", "/nonexistent/path/file"])
        .output()
        .unwrap();

    assert!(output.status.code().unwrap() > 0);
}

/// Test that cat handles multiple stdin dashes
#[test]
fn posix_cat_multiple_stdin_dash() {
    let mut child = Command::new(get_armybox_path())
        .args(["cat", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"input").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    // Per POSIX, stdin is read once and then EOF, so second dash reads nothing
    assert_eq!(String::from_utf8_lossy(&output.stdout), "input");
}

/// Test cat with empty file
#[test]
fn posix_cat_empty_file() {
    let dir = setup_test_env();
    fs::write(dir.path().join("empty"), "").unwrap();

    let output = Command::new(get_armybox_path())
        .args(["cat", dir.path().join("empty").to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout.len(), 0);
}

/// Test cat preserves binary data
#[test]
fn posix_cat_binary_data() {
    let dir = setup_test_env();
    let binary_data: Vec<u8> = (0..=255).collect();
    fs::write(dir.path().join("binary"), &binary_data).unwrap();

    let output = Command::new(get_armybox_path())
        .args(["cat", dir.path().join("binary").to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, binary_data);
}
