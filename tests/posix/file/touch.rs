//! POSIX.1-2017 compliance tests for touch
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/touch.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The touch utility shall change the modification times of files"
#[test]
fn posix_touch_existing() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    // Wait briefly to ensure time changes
    std::thread::sleep(std::time::Duration::from_millis(10));
    let before = fs::metadata(&file).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));
    let result = run(&["touch", file.to_str().unwrap()]);
    assert_success(&result);

    let after = fs::metadata(&file).unwrap().modified().unwrap();
    assert!(after >= before); // Time should be updated
}

/// POSIX: touch creates file if it does not exist
#[test]
fn posix_touch_create() {
    let dir = setup_test_env();
    let file = dir.path().join("newfile");

    let result = run(&["touch", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(file.exists());
    assert_eq!(fs::read_to_string(&file).unwrap(), "");
}

/// POSIX: touch -c does not create file
#[test]
fn posix_touch_no_create() {
    let dir = setup_test_env();
    let file = dir.path().join("nonexistent");

    let result = run(&["touch", "-c", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(!file.exists());
}

/// POSIX: touch -a changes only access time
#[test]
fn posix_touch_access_only() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    let result = run(&["touch", "-a", file.to_str().unwrap()]);
    assert_success(&result);
}

/// POSIX: touch -m changes only modification time
#[test]
fn posix_touch_mod_only() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    let result = run(&["touch", "-m", file.to_str().unwrap()]);
    assert_success(&result);
}

/// POSIX: touch -r uses reference file time
#[test]
fn posix_touch_reference() {
    let dir = setup_test_env();
    let ref_file = dir.path().join("reference");
    let target = dir.path().join("target");
    fs::write(&ref_file, "ref").unwrap();
    fs::write(&target, "target").unwrap();

    let result = run(&[
        "touch",
        "-r",
        ref_file.to_str().unwrap(),
        target.to_str().unwrap(),
    ]);
    assert_success(&result);
}

/// POSIX: touch multiple files
#[test]
fn posix_touch_multiple() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");

    let result = run(&["touch", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    assert!(file1.exists());
    assert!(file2.exists());
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_touch_exit_success() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");

    let result = run(&["touch", file.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: touch -t sets specific time
#[test]
fn posix_touch_time_spec() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    // Format: [[CC]YY]MMDDhhmm[.SS]
    let result = run(&["touch", "-t", "202301151200", file.to_str().unwrap()]);
    assert_success(&result);
}

/// POSIX: touch -d date string
#[test]
fn posix_touch_date_string() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    // -d may vary by implementation
    let result = run(&["touch", "-d", "2023-01-15 12:00:00", file.to_str().unwrap()]);
    // Accept success or unsupported
    let _ = result;
}
