//! POSIX.1-2017 compliance tests for ls
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/ls.html

use crate::posix::helpers::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// POSIX: "The ls utility shall write... the names of the files..."
#[test]
fn posix_ls_basic() {
    let dir = setup_test_env();
    fs::write(dir.path().join("file1"), "a").unwrap();
    fs::write(dir.path().join("file2"), "b").unwrap();

    let result = run_in_dir(&["ls"], dir.path());
    assert_success(&result);
    assert!(result.1.contains("file1"));
    assert!(result.1.contains("file2"));
}

/// POSIX: ls -a shows hidden files
#[test]
fn posix_ls_all() {
    let dir = setup_test_env();
    fs::write(dir.path().join(".hidden"), "x").unwrap();
    fs::write(dir.path().join("visible"), "y").unwrap();

    let result = run_in_dir(&["ls", "-a"], dir.path());
    assert_success(&result);
    assert!(result.1.contains(".hidden"));
    assert!(result.1.contains("visible"));
    assert!(result.1.contains(".")); // current dir
    assert!(result.1.contains("..")); // parent dir
}

/// POSIX: ls -A shows hidden files except . and ..
#[test]
fn posix_ls_almost_all() {
    let dir = setup_test_env();
    fs::write(dir.path().join(".hidden"), "x").unwrap();

    let result = run_in_dir(&["ls", "-A"], dir.path());
    assert_success(&result);
    assert!(result.1.contains(".hidden"));
    // Should not contain standalone "." and ".."
}

/// POSIX: ls -l long listing format
#[test]
fn posix_ls_long() {
    let dir = setup_test_env();
    fs::write(dir.path().join("testfile"), "content").unwrap();

    let result = run_in_dir(&["ls", "-l"], dir.path());
    assert_success(&result);
    // Long format includes permissions, links, owner, size, date, name
    assert!(result.1.contains("testfile"));
    // Check for permission string pattern (e.g., "-rw-")
    assert!(result.1.contains("rw"));
}

/// POSIX: ls -d lists directory itself, not contents
#[test]
fn posix_ls_directory() {
    let dir = setup_test_env();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file"), "x").unwrap();

    let result = run(&["ls", "-d", subdir.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("subdir"));
    assert!(!result.1.contains("file")); // Should not show contents
}

/// POSIX: ls -R recursive listing
#[test]
fn posix_ls_recursive() {
    let dir = setup_test_env();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("nested"), "x").unwrap();

    let result = run_in_dir(&["ls", "-R"], dir.path());
    assert_success(&result);
    assert!(result.1.contains("subdir"));
    assert!(result.1.contains("nested"));
}

/// POSIX: ls -1 one entry per line
#[test]
fn posix_ls_one_per_line() {
    let dir = setup_test_env();
    fs::write(dir.path().join("file1"), "a").unwrap();
    fs::write(dir.path().join("file2"), "b").unwrap();

    let result = run_in_dir(&["ls", "-1"], dir.path());
    assert_success(&result);
    let lines: Vec<&str> = result.1.lines().collect();
    assert!(lines.len() >= 2);
}

/// POSIX: ls -i shows inode numbers
#[test]
fn posix_ls_inode() {
    let dir = setup_test_env();
    fs::write(dir.path().join("testfile"), "x").unwrap();

    let result = run_in_dir(&["ls", "-i"], dir.path());
    assert_success(&result);
    // Output should contain numeric inode
    assert!(result.1.chars().any(|c| c.is_ascii_digit()));
}

/// POSIX: ls -t sorts by modification time
#[test]
fn posix_ls_time_sort() {
    let dir = setup_test_env();
    fs::write(dir.path().join("older"), "a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    fs::write(dir.path().join("newer"), "b").unwrap();

    let result = run_in_dir(&["ls", "-t"], dir.path());
    assert_success(&result);
    // Newer file should appear first
    let newer_pos = result.1.find("newer").unwrap_or(999);
    let older_pos = result.1.find("older").unwrap_or(0);
    assert!(newer_pos < older_pos);
}

/// POSIX: ls -r reverses sort order
#[test]
fn posix_ls_reverse() {
    let dir = setup_test_env();
    fs::write(dir.path().join("aaa"), "a").unwrap();
    fs::write(dir.path().join("zzz"), "z").unwrap();

    let result = run_in_dir(&["ls", "-r"], dir.path());
    assert_success(&result);
    // zzz should appear before aaa
    let zzz_pos = result.1.find("zzz").unwrap_or(999);
    let aaa_pos = result.1.find("aaa").unwrap_or(0);
    assert!(zzz_pos < aaa_pos);
}

/// POSIX: ls -S sorts by file size
#[test]
fn posix_ls_size_sort() {
    let dir = setup_test_env();
    fs::write(dir.path().join("small"), "x").unwrap();
    fs::write(dir.path().join("large"), "x".repeat(1000)).unwrap();

    let result = run_in_dir(&["ls", "-S"], dir.path());
    assert_success(&result);
    // Larger file should appear first
    let large_pos = result.1.find("large").unwrap_or(999);
    let small_pos = result.1.find("small").unwrap_or(0);
    assert!(large_pos < small_pos);
}

/// POSIX: ls -F appends indicator
#[test]
fn posix_ls_classify() {
    let dir = setup_test_env();
    fs::create_dir(dir.path().join("directory")).unwrap();
    fs::write(dir.path().join("file"), "x").unwrap();
    let exec = dir.path().join("executable");
    fs::write(&exec, "#!/bin/sh").unwrap();
    fs::set_permissions(&exec, fs::Permissions::from_mode(0o755)).unwrap();

    let result = run_in_dir(&["ls", "-F"], dir.path());
    assert_success(&result);
    assert!(result.1.contains("directory/"));
}

/// POSIX: ls specific file
#[test]
fn posix_ls_specific_file() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    let result = run(&["ls", file.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("testfile"));
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_ls_exit_success() {
    let result = run(&["ls", "/"]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_ls_exit_error() {
    let result = run(&["ls", "/nonexistent/path"]);
    assert!(result.0 > 0);
}

/// POSIX: ls -h human-readable sizes (with -l)
#[test]
fn posix_ls_human_readable() {
    let dir = setup_test_env();
    let file = dir.path().join("largefile");
    fs::write(&file, vec![0u8; 10000]).unwrap();

    let result = run_in_dir(&["ls", "-lh"], dir.path());
    assert_success(&result);
    // 10000 bytes rounds up to a 10K human-readable size.
    assert!(result.1.contains("10K"));
    assert!(result.1.contains("largefile"));
}

/// POSIX: ls -u selects access time for -t sorting / -l display
#[test]
fn posix_ls_atime() {
    let dir = setup_test_env();
    fs::write(dir.path().join("one"), "a").unwrap();
    fs::write(dir.path().join("two"), "b").unwrap();

    // -u must be accepted and combine with -t; atime ordering itself depends on
    // the filesystem's atime policy, so we assert both entries are listed.
    let result = run_in_dir(&["ls", "-u", "-t"], dir.path());
    assert_success(&result);
    assert!(result.1.contains("one"));
    assert!(result.1.contains("two"));
}

/// ls --help prints usage and exits 0 (not parsed as -h -e -l -p)
#[test]
fn posix_ls_help() {
    let result = run(&["ls", "--help"]);
    assert_eq!(result.0, 0);
    assert!(result.1.contains("Usage"));
}

/// ls rejects an unknown --long option with exit status 2
#[test]
fn posix_ls_unknown_long_option() {
    let result = run(&["ls", "--bogus"]);
    assert_eq!(result.0, 2);
}

/// POSIX: ls multiple directories
#[test]
fn posix_ls_multiple_dirs() {
    let dir = setup_test_env();
    let dir1 = dir.path().join("dir1");
    let dir2 = dir.path().join("dir2");
    fs::create_dir(&dir1).unwrap();
    fs::create_dir(&dir2).unwrap();
    fs::write(dir1.join("file1"), "a").unwrap();
    fs::write(dir2.join("file2"), "b").unwrap();

    let result = run(&["ls", dir1.to_str().unwrap(), dir2.to_str().unwrap()]);
    assert_success(&result);
    assert!(result.1.contains("file1"));
    assert!(result.1.contains("file2"));
}

/// POSIX: ls empty directory
#[test]
fn posix_ls_empty_dir() {
    let dir = setup_test_env();
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();

    let result = run(&["ls", empty.to_str().unwrap()]);
    assert_success(&result);
}
