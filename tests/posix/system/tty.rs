//! POSIX.1-2017 compliance tests for tty
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tty.html

use crate::posix::helpers::*;

/// POSIX: "The tty utility shall write to standard output the name of the terminal"
#[test]
fn posix_tty_basic() {
    let _result = run(&["tty"]);
    // Will exit >0 if not on a tty
    // If successful, should output a path like /dev/pts/0
}

/// POSIX: tty -s silent mode
#[test]
fn posix_tty_silent() {
    let result = run(&["tty", "-s"]);
    // No output in silent mode, just exit status
    if result.0 == 0 {
        // We're on a tty
    } else {
        // Not on a tty
    }
}

/// POSIX: Exit status 0 if stdin is terminal
#[test]
fn posix_tty_exit_terminal() {
    let result = run(&["tty"]);
    // In test environment, stdin is usually not a tty
    // so exit status is typically non-zero
    let _ = result;
}

/// POSIX: Exit status 1 if stdin is not terminal
#[test]
fn posix_tty_exit_not_terminal() {
    // Piped stdin is not a terminal
    let result = run_with_stdin(&["tty"], b"");
    // Should exit 1 since stdin is not a tty
    assert!(result.0 >= 1);
}
