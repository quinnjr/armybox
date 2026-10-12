//! POSIX.1-2017 compliance tests for cp
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/cp.html

use crate::posix::helpers::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// POSIX: "The cp utility shall copy... source_file to target"
#[test]
fn posix_cp_basic() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "content").unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "content");
}

/// POSIX: cp to existing directory
#[test]
fn posix_cp_to_directory() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::write(&src, "content").unwrap();
    fs::create_dir(&target_dir).unwrap();

    let result = run(&["cp", src.to_str().unwrap(), target_dir.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(
        fs::read_to_string(target_dir.join("source")).unwrap(),
        "content"
    );
}

/// POSIX: cp -f force overwrite
#[test]
fn posix_cp_force() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "new content").unwrap();
    fs::write(&dst, "old content").unwrap();
    // Make dest read-only
    fs::set_permissions(&dst, fs::Permissions::from_mode(0o444)).unwrap();

    let result = run(&["cp", "-f", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new content");
}

/// POSIX: cp -R recursive copy
#[test]
fn posix_cp_recursive() {
    let dir = setup_test_env();
    let src_dir = dir.path().join("src");
    let dst_dir = dir.path().join("dst");
    fs::create_dir(&src_dir).unwrap();
    fs::write(src_dir.join("file1"), "content1").unwrap();
    fs::create_dir(src_dir.join("subdir")).unwrap();
    fs::write(src_dir.join("subdir/file2"), "content2").unwrap();

    let result = run(&[
        "cp",
        "-R",
        src_dir.to_str().unwrap(),
        dst_dir.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(dst_dir.join("file1").exists());
    assert!(dst_dir.join("subdir/file2").exists());
}

/// POSIX: cp -p preserve attributes
#[test]
fn posix_cp_preserve() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "content").unwrap();
    fs::set_permissions(&src, fs::Permissions::from_mode(0o755)).unwrap();

    let result = run(&["cp", "-p", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);

    let src_mode = fs::metadata(&src).unwrap().permissions().mode() & 0o777;
    let dst_mode = fs::metadata(&dst).unwrap().permissions().mode() & 0o777;
    assert_eq!(src_mode, dst_mode);
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_cp_exit_success() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "x").unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_cp_exit_error() {
    let result = run(&["cp", "/nonexistent/source", "/tmp/dest"]);
    assert!(result.0 > 0);
}

/// POSIX: cp multiple sources to directory
#[test]
fn posix_cp_multiple_to_dir() {
    let dir = setup_test_env();
    let src1 = dir.path().join("file1");
    let src2 = dir.path().join("file2");
    let target = dir.path().join("target");
    fs::write(&src1, "content1").unwrap();
    fs::write(&src2, "content2").unwrap();
    fs::create_dir(&target).unwrap();

    let result = run(&[
        "cp",
        src1.to_str().unwrap(),
        src2.to_str().unwrap(),
        target.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(target.join("file1").exists());
    assert!(target.join("file2").exists());
}

/// POSIX: cp preserves binary content
#[test]
fn posix_cp_binary() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    let binary: Vec<u8> = (0..=255).collect();
    fs::write(&src, &binary).unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read(&dst).unwrap(), binary);
}

/// POSIX: cp empty file
#[test]
fn posix_cp_empty() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "").unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "");
}

/// POSIX: cp -r is synonym for -R
#[test]
fn posix_cp_lowercase_r() {
    let dir = setup_test_env();
    let src_dir = dir.path().join("src");
    let dst_dir = dir.path().join("dst");
    fs::create_dir(&src_dir).unwrap();
    fs::write(src_dir.join("file"), "content").unwrap();

    let result = run(&[
        "cp",
        "-r",
        src_dir.to_str().unwrap(),
        dst_dir.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(dst_dir.join("file").exists());
}

/// POSIX: cp with -i (interactive) flag accepted
#[test]
fn posix_cp_interactive_flag() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "content").unwrap();

    // -i should be accepted (won't prompt in non-interactive)
    let result = run(&["cp", "-i", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
}

/// POSIX: cp -L follows symbolic links
#[test]
fn posix_cp_follow_symlinks() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    let dst = dir.path().join("dest");
    fs::write(&target, "content").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let result = run(&["cp", "-L", link.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    // Destination should be a regular file, not a symlink
    assert!(dst.is_file());
    assert!(!dst.is_symlink());
}

/// POSIX: cp -P preserves symbolic links
#[test]
fn posix_cp_preserve_symlinks() {
    let dir = setup_test_env();
    let src_dir = dir.path().join("src");
    let dst_dir = dir.path().join("dst");
    fs::create_dir(&src_dir).unwrap();
    let target = src_dir.join("target");
    let link = src_dir.join("link");
    fs::write(&target, "content").unwrap();
    std::os::unix::fs::symlink("target", &link).unwrap();

    let result = run(&[
        "cp",
        "-R",
        "-P",
        src_dir.to_str().unwrap(),
        dst_dir.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(dst_dir.join("link").is_symlink());
}

/// POSIX: cp large file
#[test]
fn posix_cp_large_file() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    let large: Vec<u8> = vec![0x55; 1024 * 1024]; // 1MB
    fs::write(&src, &large).unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::metadata(&dst).unwrap().len(), 1024 * 1024);
}

/// POSIX: cp overwrites existing file by default
#[test]
fn posix_cp_overwrite() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "new").unwrap();
    fs::write(&dst, "old").unwrap();

    let result = run(&["cp", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new");
}

/// POSIX: cp -n no-clobber (if supported)
#[test]
fn posix_cp_no_clobber() {
    let dir = setup_test_env();
    let src = dir.path().join("source");
    let dst = dir.path().join("dest");
    fs::write(&src, "new").unwrap();
    fs::write(&dst, "old").unwrap();

    // -n never overwrites an existing destination and is not an error.
    let result = run(&["cp", "-n", src.to_str().unwrap(), dst.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "old");
}
