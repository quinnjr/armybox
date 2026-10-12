//! POSIX test helpers
//!
//! Common utilities for running armybox and verifying POSIX-compliant behavior.

// A shared toolkit: not every helper is used by the current tests
#![allow(dead_code)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Get the path to the armybox binary.
///
/// Uses ARMYBOX_PATH if set (for CI or custom setups), otherwise the binary
/// Cargo built for this test run's profile.
pub fn get_armybox_path() -> PathBuf {
    if let Ok(path) = std::env::var("ARMYBOX_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return p;
        }
        panic!("ARMYBOX_PATH set to '{}' but file does not exist", path);
    }

    // Absolute, so tests that change the child's working directory still work
    PathBuf::from(env!("CARGO_BIN_EXE_armybox"))
}

/// Create a test environment with a temporary directory.
pub fn setup_test_env() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// Run armybox utility and return (exit_code, stdout, stderr).
pub fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(get_armybox_path())
        .args(args)
        .output()
        .expect("Failed to execute armybox");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

/// Run armybox with stdin input and return (exit_code, stdout, stderr).
pub fn run_with_stdin(args: &[&str], input: &[u8]) -> (i32, String, String) {
    let mut child = Command::new(get_armybox_path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn armybox");

    {
        let stdin = child.stdin.as_mut().unwrap();
        // armybox may exit before reading all input (e.g. on a usage error);
        // that's a broken pipe, not a test failure
        if let Err(e) = stdin.write_all(input) {
            assert_eq!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe,
                "Failed to write to stdin: {e}"
            );
        }
    }

    let output = child.wait_with_output().expect("Failed to wait on child");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

/// Run armybox and return raw bytes (for binary tests).
pub fn run_binary(args: &[&str]) -> (i32, Vec<u8>, Vec<u8>) {
    let output = Command::new(get_armybox_path())
        .args(args)
        .output()
        .expect("Failed to execute armybox");

    let code = output.status.code().unwrap_or(-1);
    (code, output.stdout, output.stderr)
}

/// Run armybox with stdin and return raw bytes.
pub fn run_binary_with_stdin(args: &[&str], input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(get_armybox_path())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn armybox");

    {
        let stdin = child.stdin.as_mut().unwrap();
        // armybox may exit before reading all input (e.g. on a usage error);
        // that's a broken pipe, not a test failure
        if let Err(e) = stdin.write_all(input) {
            assert_eq!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe,
                "Failed to write to stdin: {e}"
            );
        }
    }

    let output = child.wait_with_output().expect("Failed to wait on child");
    let code = output.status.code().unwrap_or(-1);
    (code, output.stdout, output.stderr)
}

/// Run armybox in a specific directory.
pub fn run_in_dir(args: &[&str], dir: &std::path::Path) -> (i32, String, String) {
    let output = Command::new(get_armybox_path())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("Failed to execute armybox");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

/// Run armybox with custom environment variables.
pub fn run_with_env(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(get_armybox_path());
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute armybox");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

/// Run armybox with cleared environment (only specified vars).
pub fn run_with_clean_env(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(get_armybox_path());
    cmd.args(args);
    cmd.env_clear();
    for (key, value) in env {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute armybox");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

/// Assert that exit code is 0 (success).
#[track_caller]
pub fn assert_success(result: &(i32, String, String)) {
    assert_eq!(
        result.0, 0,
        "Expected exit code 0, got {}. stderr: {}",
        result.0, result.2
    );
}

/// Assert that exit code is non-zero (error).
#[track_caller]
pub fn assert_error(result: &(i32, String, String)) {
    assert!(
        result.0 != 0,
        "Expected non-zero exit code, got 0. stdout: {}",
        result.1
    );
}

/// Assert stdout equals expected value.
#[track_caller]
pub fn assert_stdout(result: &(i32, String, String), expected: &str) {
    assert_eq!(
        result.1, expected,
        "stdout mismatch.\nExpected: {:?}\nActual: {:?}",
        expected, result.1
    );
}

/// Assert stdout contains expected substring.
#[track_caller]
pub fn assert_stdout_contains(result: &(i32, String, String), expected: &str) {
    assert!(
        result.1.contains(expected),
        "stdout does not contain {:?}.\nActual stdout: {:?}",
        expected,
        result.1
    );
}

/// Assert stderr contains expected substring.
#[track_caller]
pub fn assert_stderr_contains(result: &(i32, String, String), expected: &str) {
    assert!(
        result.2.contains(expected),
        "stderr does not contain {:?}.\nActual stderr: {:?}",
        expected,
        result.2
    );
}

/// Assert stdout is empty.
#[track_caller]
pub fn assert_stdout_empty(result: &(i32, String, String)) {
    assert!(
        result.1.is_empty(),
        "Expected empty stdout, got: {:?}",
        result.1
    );
}

/// Assert stderr is empty.
#[track_caller]
pub fn assert_stderr_empty(result: &(i32, String, String)) {
    assert!(
        result.2.is_empty(),
        "Expected empty stderr, got: {:?}",
        result.2
    );
}

/// Assert stdout lines match expected lines (order matters).
#[track_caller]
pub fn assert_stdout_lines(result: &(i32, String, String), expected: &[&str]) {
    let actual: Vec<&str> = result.1.lines().collect();
    assert_eq!(
        actual, expected,
        "stdout lines mismatch.\nExpected: {:?}\nActual: {:?}",
        expected, actual
    );
}

/// Assert stdout contains all expected lines (order doesn't matter).
#[track_caller]
pub fn assert_stdout_contains_all(result: &(i32, String, String), expected: &[&str]) {
    for line in expected {
        assert!(
            result.1.contains(line),
            "stdout missing expected content: {:?}\nActual stdout: {:?}",
            line,
            result.1
        );
    }
}

/// Count lines in stdout.
pub fn count_stdout_lines(result: &(i32, String, String)) -> usize {
    result.1.lines().count()
}
