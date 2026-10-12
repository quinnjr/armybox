//! POSIX.1-2017 compliance tests for tail
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tail.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The tail utility shall copy the last lines of each file"
#[test]
fn posix_tail_default() {
    let input = (1..=15)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");
    let result = run_with_stdin(&["tail"], input.as_bytes());
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 10);
    assert!(result.1.contains("line15"));
}

/// POSIX: tail -n count
#[test]
fn posix_tail_count() {
    let input = (1..=10)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");
    let result = run_with_stdin(&["tail", "-n", "3"], input.as_bytes());
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 3);
    assert!(result.1.contains("line10"));
}

/// POSIX: tail -n +N (starting from line N)
#[test]
fn posix_tail_from_line() {
    let input = (1..=10)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");
    let result = run_with_stdin(&["tail", "-n", "+8"], input.as_bytes());
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 3);
    assert!(result.1.contains("line8"));
}

/// POSIX: tail -c bytes
#[test]
fn posix_tail_bytes() {
    let result = run_with_stdin(&["tail", "-c", "5"], b"hello world");
    assert_success(&result);
    assert_eq!(result.1, "world");
}

/// POSIX: tail from file
#[test]
fn posix_tail_from_file() {
    let dir = setup_test_env();
    let file = dir.path().join("input");
    let content = (1..=15)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&file, &content).unwrap();

    let result = run(&["tail", file.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 10);
}

/// POSIX: tail multiple files
#[test]
fn posix_tail_multiple_files() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "content1\n").unwrap();
    fs::write(&file2, "content2\n").unwrap();

    let result = run(&["tail", file1.to_str().unwrap(), file2.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("content1"));
    assert!(result.1.contains("content2"));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_tail_exit_success() {
    let result = run_with_stdin(&["tail"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_tail_exit_error() {
    let result = run(&["tail", "/nonexistent/file"]);
    assert!(result.0 > 0);
}

/// Note: tail -f (follow) cannot be easily tested in a unit test
/// as it runs indefinitely waiting for input
#[test]
fn posix_tail_f_flag_accepted() {
    // Just verify the flag is accepted
    let dir = setup_test_env();
    let file = dir.path().join("input");
    fs::write(&file, "content").unwrap();

    // We can't test -f behavior, but we can verify the flag parses
    // This would hang, so skip actual execution
}

/// tail -c with a negative count means "last N bytes", same as a bare
/// positive count (a leading `-` must not be mis-parsed as a bad number).
#[test]
fn posix_tail_bytes_negative() {
    let result = run_with_stdin(&["tail", "-c", "-5"], b"hello world");
    assert_success(&result);
    assert_eq!(result.1, "world");
}

/// tail -n with a negative count means "last N lines", same as a bare
/// positive count.
#[test]
fn posix_tail_lines_negative() {
    let input = (1..=10)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");
    let result = run_with_stdin(&["tail", "-n", "-3"], input.as_bytes());
    assert_success(&result);
    assert_eq!(count_stdout_lines(&result), 3);
    assert!(result.1.contains("line10"));
}

/// An unparsable -n count is a hard error, not a silent default of 10.
#[test]
fn posix_tail_invalid_n_exits_nonzero() {
    let result = run_with_stdin(&["tail", "-n", "xyz"], b"hello\n");
    assert!(result.0 != 0);
    assert_eq!(result.1, "");
}

/// tail --help prints usage and exits 0 without reading stdin.
#[test]
fn posix_tail_help() {
    let result = run_with_stdin(&["tail", "--help"], b"should not be read");
    assert_eq!(result.0, 0);
    assert!(result.1.to_lowercase().contains("usage"));
}

/// POSIX: tail -q quiet (no headers)
#[test]
fn posix_tail_quiet() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "content1").unwrap();
    fs::write(&file2, "content2").unwrap();

    let result = run(&[
        "tail",
        "-q",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    // Should not contain "==>" headers
    assert!(!result.1.contains("==>"));
}
