//! POSIX.1-2017 compliance tests for ln
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/ln.html

use crate::posix::helpers::*;
use std::fs;

/// POSIX: "The ln utility shall create a new link to an existing file"
#[test]
fn posix_ln_hard_link() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("hardlink");
    fs::write(&target, "content").unwrap();

    let result = run(&["ln", target.to_str().unwrap(), link.to_str().unwrap()]);
    assert_success(&result);
    assert!(link.exists());
    assert_eq!(fs::read_to_string(&link).unwrap(), "content");
}

/// POSIX: ln -s creates symbolic link
#[test]
fn posix_ln_symbolic() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("symlink");
    fs::write(&target, "content").unwrap();

    let result = run(&["ln", "-s", target.to_str().unwrap(), link.to_str().unwrap()]);
    assert_success(&result);
    assert!(link.is_symlink());
    assert_eq!(fs::read_to_string(&link).unwrap(), "content");
}

/// POSIX: ln -f force overwrite existing link
#[test]
fn posix_ln_force() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, "new content").unwrap();
    fs::write(&link, "old content").unwrap();

    let result = run(&["ln", "-f", target.to_str().unwrap(), link.to_str().unwrap()]);
    assert_success(&result);
    assert_eq!(fs::read_to_string(&link).unwrap(), "new content");
}

/// POSIX: ln to directory
#[test]
fn posix_ln_to_directory() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let target_dir = dir.path().join("linkdir");
    fs::write(&target, "content").unwrap();
    fs::create_dir(&target_dir).unwrap();

    let result = run(&[
        "ln",
        "-s",
        target.to_str().unwrap(),
        target_dir.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(target_dir.join("target").is_symlink());
}

/// POSIX: Exit status 0 on success
#[test]
fn posix_ln_exit_success() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, "x").unwrap();

    let result = run(&["ln", "-s", target.to_str().unwrap(), link.to_str().unwrap()]);
    assert_eq!(result.0, 0);
}

/// POSIX: Exit status >0 on error
#[test]
fn posix_ln_exit_error() {
    let result = run(&["ln", "/nonexistent/target", "/nonexistent/link"]);
    assert!(result.0 > 0);
}

/// POSIX: ln without -s creates hard link
#[test]
fn posix_ln_hard_link_inode() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("hardlink");
    fs::write(&target, "content").unwrap();

    let result = run(&["ln", target.to_str().unwrap(), link.to_str().unwrap()]);
    assert_success(&result);

    // Hard links share inode
    use std::os::unix::fs::MetadataExt;
    let target_ino = fs::metadata(&target).unwrap().ino();
    let link_ino = fs::metadata(&link).unwrap().ino();
    assert_eq!(target_ino, link_ino);
}

/// POSIX: ln -s with relative path
#[test]
fn posix_ln_relative_symlink() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, "content").unwrap();

    let result = run(&["ln", "-s", "target", link.to_str().unwrap()]);
    assert_success(&result);
    assert!(link.is_symlink());
}

/// POSIX: ln multiple sources to directory
#[test]
fn posix_ln_multiple() {
    let dir = setup_test_env();
    let t1 = dir.path().join("t1");
    let t2 = dir.path().join("t2");
    let target_dir = dir.path().join("linkdir");
    fs::write(&t1, "a").unwrap();
    fs::write(&t2, "b").unwrap();
    fs::create_dir(&target_dir).unwrap();

    let result = run(&[
        "ln",
        "-s",
        t1.to_str().unwrap(),
        t2.to_str().unwrap(),
        target_dir.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(target_dir.join("t1").is_symlink());
    assert!(target_dir.join("t2").is_symlink());
}

/// POSIX: ln -sf combined flags
#[test]
fn posix_ln_sf() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let link = dir.path().join("link");
    fs::write(&target, "new").unwrap();
    fs::write(&link, "old").unwrap();

    let result = run(&[
        "ln",
        "-sf",
        target.to_str().unwrap(),
        link.to_str().unwrap(),
    ]);
    assert_success(&result);
    assert!(link.is_symlink());
}

/// POSIX: ln (hard link, no -s) into a directory target names the link
/// after the source's basename and does not truncate the destination path.
#[test]
fn posix_ln_hard_link_to_directory() {
    let dir = setup_test_env();
    let target = dir.path().join("target");
    let target_dir = dir.path().join("linkdir");
    fs::write(&target, "content").unwrap();
    fs::create_dir(&target_dir).unwrap();

    let result = run(&["ln", target.to_str().unwrap(), target_dir.to_str().unwrap()]);
    assert_success(&result);
    let link = target_dir.join("target");
    assert!(link.exists());
    assert_eq!(fs::read_to_string(&link).unwrap(), "content");

    use std::os::unix::fs::MetadataExt;
    let target_ino = fs::metadata(&target).unwrap().ino();
    let link_ino = fs::metadata(&link).unwrap().ino();
    assert_eq!(target_ino, link_ino);
}
