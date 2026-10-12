//! POSIX.1-2017 compliance tests for grep
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/grep.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The grep utility shall search... for the pattern"
#[test]
fn posix_grep_basic() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nworld\nhello world").unwrap();

    let result = run(&["grep", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("hello"));
}

/// POSIX: grep -i case insensitive
#[test]
fn posix_grep_case_insensitive() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "Hello\nWORLD\nhello").unwrap();

    let result = run(&["grep", "-i", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("Hello"));
    assert!(result.1.contains("hello"));
}

/// POSIX: grep -v invert match
#[test]
fn posix_grep_invert() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nworld\nfoo").unwrap();

    let result = run(&["grep", "-v", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(!result.1.contains("hello"));
    assert!(result.1.contains("world"));
    assert!(result.1.contains("foo"));
}

/// POSIX: grep -c count matches
#[test]
fn posix_grep_count() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nhello\nworld").unwrap();

    let result = run(&["grep", "-c", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.trim() == "2");
}

/// POSIX: grep -l list files with matches
#[test]
fn posix_grep_list_files() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "hello").unwrap();
    fs::write(&file2, "world").unwrap();

    let result = run(&[
        "grep",
        "-l",
        "hello",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(result.1.contains("file1"));
    assert!(!result.1.contains("file2"));
}

/// POSIX: grep -n show line numbers
#[test]
fn posix_grep_line_numbers() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "foo\nhello\nbar").unwrap();

    let result = run(&["grep", "-n", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("2:")); // Line 2
}

/// POSIX: grep -q quiet mode
#[test]
fn posix_grep_quiet() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello world").unwrap();

    let result = run(&["grep", "-q", "hello", file.to_str().unwrap()]);
    assert_eq!(result.0, 0);
    assert!(result.1.is_empty());
}

/// POSIX: grep -E extended regex
#[test]
fn posix_grep_extended() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nworld\nhelloworld").unwrap();

    let result = run(&["grep", "-E", "hello|world", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("hello"));
    assert!(result.1.contains("world"));
}

/// POSIX: grep -F fixed strings
#[test]
fn posix_grep_fixed() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello.world\nhelloXworld").unwrap();

    let result = run(&["grep", "-F", "hello.world", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("hello.world"));
    assert!(!result.1.contains("helloXworld"));
}

/// POSIX: grep from stdin
#[test]
fn posix_grep_stdin() {
    let result = run_with_stdin(&["grep", "hello"], b"hello\nworld\nhello world");
    assert_success(&result);
    assert!(result.1.contains("hello"));
}

/// POSIX: Exit status 0 when match found
#[test]
fn posix_grep_exit_match() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello").unwrap();

    let result = run(&["grep", "hello", file.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status 1 when no match
#[test]
fn posix_grep_exit_no_match() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello").unwrap();

    let result = run(&["grep", "notfound", file.to_str().unwrap()]);
    assert_eq!(result.0, 1);
}

/// POSIX: Exit status >1 on error
#[test]
fn posix_grep_exit_error() {
    let result = run(&["grep", "pattern", "/nonexistent/file"]);
    assert!(result.0 > 1);
}

/// POSIX: grep multiple files
#[test]
fn posix_grep_multiple_files() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "hello file1").unwrap();
    fs::write(&file2, "hello file2").unwrap();

    let result = run(&[
        "grep",
        "hello",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(result.1.contains("file1"));
    assert!(result.1.contains("file2"));
}

/// POSIX: grep -e pattern
#[test]
fn posix_grep_e_option() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nworld").unwrap();

    let result = run(&["grep", "-e", "hello", "-e", "world", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("hello"));
    assert!(result.1.contains("world"));
}

/// POSIX: grep -f pattern file
#[test]
fn posix_grep_pattern_file() {
    let dir = setup_test_env();
    let data = dir.path().join("data");
    let patterns = dir.path().join("patterns");
    fs::write(&data, "hello\nworld\nfoo").unwrap();
    fs::write(&patterns, "hello\nfoo").unwrap();

    let result = run(&[
        "grep",
        "-f",
        patterns.to_str().unwrap(),
        data.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(result.1.contains("hello"));
    assert!(result.1.contains("foo"));
    assert!(!result.1.contains("world"));
}

/// POSIX: grep regex anchors
#[test]
fn posix_grep_anchors() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nworld hello\nhello world").unwrap();

    let result = run(&["grep", "^hello", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.lines().all(|l| l.starts_with("hello")));
}

/// POSIX: grep word boundary -w
#[test]
fn posix_grep_word() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nhelloworld\nworld hello world").unwrap();

    let result = run(&["grep", "-w", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    // Matches the standalone word "hello" but never inside "helloworld".
    assert_eq!(result.1, "hello\nworld hello world\n");
}

/// POSIX: grep -x whole-line match
#[test]
fn posix_grep_whole_line() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "hello\nhello world\nsay hello").unwrap();

    let result = run(&["grep", "-x", "hello", file.to_str().unwrap()]);
    assert_success(&result);
    // Only the line that is exactly "hello".
    assert_eq!(result.1, "hello\n");
}

/// POSIX: ERE interval bounds {n,m}
#[test]
fn posix_grep_interval_ere() {
    let result = run_with_stdin(&["grep", "-E", "a{2,3}"], b"a\naa\naaa\naaaa\n");
    assert_success(&result);
    // {2,3} matches lines containing 2, 3, or (a run of) 4 a's; not a single 'a'.
    assert_eq!(result.1, "aa\naaa\naaaa\n");
}

/// POSIX: BRE interval bounds \{n,m\}
#[test]
fn posix_grep_interval_bre() {
    let result = run_with_stdin(&["grep", "x\\{2,3\\}"], b"x\nxx\nxxx\nxxxx\n");
    assert_success(&result);
    assert_eq!(result.1, "xx\nxxx\nxxxx\n");
}

/// POSIX: BRE exact interval \{n\}
#[test]
fn posix_grep_interval_exact() {
    let result = run_with_stdin(&["grep", "^a\\{3\\}$"], b"aa\naaa\naaaa\n");
    assert_success(&result);
    assert_eq!(result.1, "aaa\n");
}

/// POSIX: bracket character class [[:digit:]]
#[test]
fn posix_grep_class_digit() {
    let result = run_with_stdin(&["grep", "[[:digit:]]"], b"abc\nab3\nxyz\n42\n");
    assert_success(&result);
    assert_eq!(result.1, "ab3\n42\n");
}

/// POSIX: bracket range [a-c]
#[test]
fn posix_grep_class_range() {
    let result = run_with_stdin(&["grep", "[a-c]"], b"dog\ncat\nzoo\nbee\n");
    assert_success(&result);
    // "cat" has 'a'/'c', "bee" has 'b'; "dog"/"zoo" have none of a-c.
    assert_eq!(result.1, "cat\nbee\n");
}

/// POSIX: negated bracket class [^...]
#[test]
fn posix_grep_class_negate() {
    let result = run_with_stdin(&["grep", "^[^abc]"], b"apple\nbanana\ncherry\ndate\n");
    assert_success(&result);
    // Only lines whose first char is not a, b, or c.
    assert_eq!(result.1, "date\n");
}

/// POSIX: BRE alternation \| and grouping \(...\)
#[test]
fn posix_grep_bre_alternation() {
    let result = run_with_stdin(&["grep", "^\\(cat\\|dog\\)$"], b"cat\ndog\ncatdog\nbird\n");
    assert_success(&result);
    assert_eq!(result.1, "cat\ndog\n");
}

/// POSIX: BRE grouping with a quantifier \(ab\)*
#[test]
fn posix_grep_bre_group_repeat() {
    let result = run_with_stdin(&["grep", "^\\(ab\\)\\{2\\}$"], b"ab\nabab\nababab\n");
    assert_success(&result);
    assert_eq!(result.1, "abab\n");
}

/// POSIX: bad regex (unmatched '[') exits 2
#[test]
fn posix_grep_bad_regex_bracket() {
    let result = run_with_stdin(&["grep", "[abc"], b"abc\n");
    assert_eq!(result.0, 2);
}

/// POSIX: bad regex (unmatched BRE group '\(') exits 2
#[test]
fn posix_grep_bad_regex_group() {
    let result = run_with_stdin(&["grep", "\\(abc"], b"abc\n");
    assert_eq!(result.0, 2);
}

/// POSIX: out-of-range interval {n,m} with n > m exits 2
#[test]
fn posix_grep_bad_interval() {
    let result = run_with_stdin(&["grep", "-E", "a{3,2}"], b"aaa\n");
    assert_eq!(result.0, 2);
}

/// POSIX: -f with a nonexistent pattern file errors
#[test]
fn posix_grep_missing_pattern_file() {
    let result = run(&["grep", "-f", "/nonexistent/patterns", "/dev/null"]);
    assert!(result.0 > 1);
}

/// POSIX: invalid option names the offending character
#[test]
fn posix_grep_invalid_option() {
    let result = run_with_stdin(&["grep", "-Z", "x"], b"x\n");
    assert_eq!(result.0, 2);
    assert!(result.2.contains("'Z'"));
}
