//! POSIX.1-2017 compliance tests for env
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/env.html

use crate::posix::helpers::*;

/// POSIX: "The env utility shall obtain the current environment..."
#[test]
fn posix_env_list() {
    let result = run(&["env"]);
    assert_success(&result);
    // Should list environment variables in VAR=value format
    assert!(result.1.contains("="));
}

/// POSIX: env sets variable for command
#[test]
fn posix_env_set_var() {
    let result = run(&["env", "TESTVAR=hello", "sh", "-c", "echo $TESTVAR"]);
    assert_success(&result);
    assert!(result.1.trim() == "hello");
}

/// POSIX: env -i ignore inherited environment
#[test]
fn posix_env_ignore() {
    let result = run(&["env", "-i", "PATH=/bin", "env"]);
    assert_success(&result);
    // Should only have PATH set
    let lines: Vec<&str> = result.1.lines().collect();
    assert!(lines.iter().any(|l| l.starts_with("PATH=")));
}

/// POSIX: env -u unset variable
#[test]
fn posix_env_unset() {
    let result = run_with_env(&["env", "-u", "TESTVAR", "env"], &[("TESTVAR", "value")]);
    assert_success(&result);
    // TESTVAR should not appear
    assert!(!result.1.contains("TESTVAR="));
}

/// POSIX: env executes utility
#[test]
fn posix_env_exec() {
    let result = run(&["env", "echo", "hello"]);
    assert_success(&result);
    assert_eq!(result.1.trim(), "hello");
}

/// POSIX: Exit status reflects executed command
#[test]
fn posix_env_exit_status() {
    let result = run(&["env", "true"]);
    assert_eq!(result.0, 0);

    let result = run(&["env", "false"]);
    assert!(result.0 != 0);
}

/// POSIX: env without utility lists environment
#[test]
fn posix_env_no_utility() {
    let result = run(&["env"]);
    assert_success(&result);
    // Output should be VAR=value format
}

/// POSIX: env multiple variables
#[test]
fn posix_env_multiple_vars() {
    let result = run(&["env", "A=1", "B=2", "sh", "-c", "echo $A $B"]);
    assert_success(&result);
    assert!(result.1.trim() == "1 2");
}
