//! POSIX.1-2017 compliance tests for sort
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sort.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The sort utility shall sort, merge, or sequence check text files"
#[test]
fn posix_sort_basic() {
    let result = run_with_stdin(&["sort"], b"cherry\napple\nbanana");
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\ncherry\n");
}

/// POSIX: sort -r reverse
#[test]
fn posix_sort_reverse() {
    let result = run_with_stdin(&["sort", "-r"], b"apple\nbanana\ncherry");
    assert_success(&result);
    assert_eq!(result.1, "cherry\nbanana\napple\n");
}

/// POSIX: sort -n numeric
#[test]
fn posix_sort_numeric() {
    let result = run_with_stdin(&["sort", "-n"], b"10\n2\n1\n20");
    assert_success(&result);
    assert_eq!(result.1, "1\n2\n10\n20\n");
}

/// POSIX: sort -u unique
#[test]
fn posix_sort_unique() {
    let result = run_with_stdin(&["sort", "-u"], b"apple\nbanana\napple\ncherry");
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\ncherry\n");
}

/// POSIX: sort -f fold case
#[test]
fn posix_sort_fold_case() {
    let result = run_with_stdin(&["sort", "-f"], b"Apple\napple\nBanana");
    assert_success(&result);
    // Case-folded sort groups same words together
    let lines: Vec<&str> = result.1.lines().collect();
    assert!(lines[0] == "Apple" || lines[0] == "apple");
}

/// POSIX: sort -k key field
#[test]
fn posix_sort_key() {
    let result = run_with_stdin(&["sort", "-k2"], b"a 3\nb 1\nc 2");
    assert_success(&result);
    assert_eq!(result.1, "b 1\nc 2\na 3\n");
}

/// POSIX: sort -k with numeric
#[test]
fn posix_sort_key_numeric() {
    let result = run_with_stdin(&["sort", "-k2", "-n"], b"a 10\nb 2\nc 1");
    assert_success(&result);
    assert_eq!(result.1, "c 1\nb 2\na 10\n");
}

/// POSIX: sort -t field separator
#[test]
fn posix_sort_separator() {
    let result = run_with_stdin(&["sort", "-t:", "-k2"], b"a:3\nb:1\nc:2");
    assert_success(&result);
    assert_eq!(result.1, "b:1\nc:2\na:3\n");
}

/// POSIX: sort -b ignore leading blanks
#[test]
fn posix_sort_ignore_blanks() {
    // Without -b, leading blanks participate in the byte comparison, so
    // "  cherry" < " banana" < "apple". With -b, blanks are skipped and the
    // lines sort by their first non-blank character: apple < banana < cherry.
    let result = run_with_stdin(&["sort", "-b"], b"  cherry\n banana\napple");
    assert_success(&result);
    assert_eq!(result.1, "apple\n banana\n  cherry\n");
}

/// POSIX: sort -c check sorted
#[test]
fn posix_sort_check() {
    let result = run_with_stdin(&["sort", "-c"], b"apple\nbanana\ncherry");
    assert_eq!(result.0, 0);

    let result = run_with_stdin(&["sort", "-c"], b"cherry\napple\nbanana");
    assert!(result.0 != 0);
}

/// POSIX: sort -m merge
#[test]
fn posix_sort_merge() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "apple\ncherry").unwrap();
    fs::write(&file2, "banana\ndate").unwrap();

    let result = run(&[
        "sort",
        "-m",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\ncherry\ndate\n");
}

/// POSIX: sort from file
#[test]
fn posix_sort_from_file() {
    let dir = setup_test_env();
    let file = dir.path().join("input");
    fs::write(&file, "cherry\napple\nbanana").unwrap();

    let result = run(&["sort", file.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(result.1, "apple\nbanana\ncherry\n");
}

/// POSIX: sort -o output to file
#[test]
fn posix_sort_output() {
    let dir = setup_test_env();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    fs::write(&input, "cherry\napple\nbanana").unwrap();

    let result = run(&[
        "sort",
        "-o",
        output.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "apple\nbanana\ncherry\n"
    );
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_sort_exit_success() {
    let result = run_with_stdin(&["sort"], b"test");
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_sort_exit_error() {
    let result = run(&["sort", "/nonexistent/file"]);
    assert!(result.0 > 0);
}

/// POSIX: sort stable with equal keys
#[test]
fn posix_sort_stable() {
    let result = run_with_stdin(&["sort", "-s", "-k1,1"], b"a 1\na 2\na 3");
    assert_success(&result);
    // Stable sort preserves original order for equal keys
    assert_eq!(result.1, "a 1\na 2\na 3\n");
}

/// POSIX: sort key with start and end positions
#[test]
fn posix_sort_key_positions() {
    // -k1.2,1.3 compares characters 2 through 3 of field 1 (the whole line,
    // since there's no field separator here). All three lines are only two
    // characters long, so the effective key is just the second character.
    let result = run_with_stdin(&["sort", "-k1.2,1.3"], b"ab\naa\nac");
    assert_success(&result);
    assert_eq!(result.1, "aa\nab\nac\n");
}

/// POSIX: sort -k with a character-offset range spanning more than one
/// character within the key field.
#[test]
fn posix_sort_key_char_offset() {
    // Key is characters 2..3 of the (whole-line) field: "34", "12", "90".
    let result = run_with_stdin(&["sort", "-k1.2,1.3"], b"a34z\nb12z\nc90z");
    assert_success(&result);
    assert_eq!(result.1, "b12z\na34z\nc90z\n");
}

/// POSIX: sort -o writes sorted output to the given file, independent of
/// any other options in effect.
#[test]
fn posix_sort_output_file() {
    let dir = setup_test_env();
    let input = dir.path().join("nums.txt");
    let output = dir.path().join("sorted.txt");
    fs::write(&input, "10\n2\n33\n1").unwrap();

    let result = run(&[
        "sort",
        "-n",
        "-o",
        output.to_str().unwrap(),
        input.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert_eq!(result.1, "");
    assert_eq!(fs::read_to_string(&output).unwrap(), "1\n2\n10\n33\n");
}
