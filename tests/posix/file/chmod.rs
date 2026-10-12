//! POSIX.1-2017 compliance tests for chmod
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/chmod.html

use crate::posix::helpers::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// POSIX: "The chmod utility shall change... file mode bits"
#[test]
fn posix_chmod_octal() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    let result = run(&["chmod", "755", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o755);
}

/// POSIX: chmod with symbolic mode u+x
#[test]
fn posix_chmod_symbolic_plus() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

    let result = run(&["chmod", "u+x", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert!(
        (perms.mode() & 0o100) != 0,
        "user execute bit should be set"
    );
}

/// POSIX: chmod with symbolic mode a-w
#[test]
fn posix_chmod_symbolic_minus() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();

    let result = run(&["chmod", "a-w", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o222, 0, "write bits should be cleared");
}

/// POSIX: chmod with symbolic mode g=rx
#[test]
fn posix_chmod_symbolic_equals() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();

    let result = run(&["chmod", "g=rx", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o070, 0o050, "group should be r-x");
}

/// POSIX: chmod -R recursive
#[test]
fn posix_chmod_recursive() {
    let dir = setup_test_env();
    let subdir = dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let file = subdir.join("file");
    fs::write(&file, "content").unwrap();

    let result = run(&["chmod", "-R", "755", dir.path().to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o755);
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_chmod_exit_success() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();

    let result = run(&["chmod", "644", file.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_chmod_exit_error() {
    let result = run(&["chmod", "755", "/nonexistent/path/file"]);
    assert!(result.0 > 0);
}

/// POSIX: Multiple files
#[test]
fn posix_chmod_multiple_files() {
    let dir = setup_test_env();
    let file1 = dir.path().join("file1");
    let file2 = dir.path().join("file2");
    fs::write(&file1, "a").unwrap();
    fs::write(&file2, "b").unwrap();

    let result = run(&[
        "chmod",
        "755",
        file1.to_str().unwrap(),
        file2.to_str().unwrap(),
    ]);
    assert_success(&result);

    assert_eq!(
        fs::metadata(&file1).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        fs::metadata(&file2).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

/// POSIX: Symbolic mode with multiple who symbols
#[test]
fn posix_chmod_symbolic_ug() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();

    let result = run(&["chmod", "ug+rw", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o660, 0o660);
}

/// POSIX: Symbolic mode chain (a+r,u+w)
#[test]
fn posix_chmod_symbolic_chain() {
    let dir = setup_test_env();
    let file = dir.path().join("testfile");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();

    let result = run(&["chmod", "a+r,u+w", file.to_str().unwrap()]);
    assert_success(&result);

    let perms = fs::metadata(&file).unwrap().permissions();
    assert_eq!(perms.mode() & 0o644, 0o644);
}
