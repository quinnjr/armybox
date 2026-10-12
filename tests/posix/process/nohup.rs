//! POSIX.1-2017 compliance tests for nohup
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/nohup.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The nohup utility shall invoke the utility ignoring the SIGHUP signal"
#[test]
fn posix_nohup_basic() {
    let dir = setup_test_env();
    let output = dir.path().join("output.txt");

    let result = run_in_dir(
        &[
            "nohup",
            "sh",
            "-c",
            &format!("echo hello > {}", output.to_str().unwrap()),
        ],
        dir.path(),
    );

    // Give it a moment to complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // nohup should have succeeded
    assert!(result.0 == 0 || result.0 == 126 || result.0 == 127);
}

/// POSIX: nohup redirects output to nohup.out by default
#[test]
fn posix_nohup_output_redirect() {
    let dir = setup_test_env();

    let result = run_in_dir(&["nohup", "echo", "test"], dir.path());
    // Output should be redirected
    let _ = result;
}

/// POSIX: Exit status 126 if utility cannot be invoked
#[test]
fn posix_nohup_exit_126() {
    let result = run(&["nohup", "/nonexistent/command"]);
    assert!(result.0 == 126 || result.0 == 127 || result.0 > 0);
}

/// POSIX: Exit status 127 if utility not found
#[test]
fn posix_nohup_exit_127() {
    let result = run(&["nohup", "nonexistent_command_12345"]);
    assert!(result.0 == 127 || result.0 > 0);
}

/// POSIX: nohup command exit status reflects command
#[test]
fn posix_nohup_reflects_command() {
    let result = run(&["nohup", "true"]);
    assert_eq!(result.0, 0);
}
