//! truncate - shrink or extend the size of a file
//!
//! GNU coreutils compatible implementation.

use crate::io;
use crate::sys;
use crate::applets::get_arg;
use alloc::vec::Vec;

/// truncate - shrink or extend the size of a file
///
/// # Synopsis
/// ```text
/// truncate [-c] -s SIZE file...
/// ```
///
/// # Description
/// Shrink or extend the size of each FILE to the specified size.
///
/// # Options
/// - `-c`, `--no-create`: Do not create files that don't exist
/// - `-s SIZE`, `--size=SIZE`: Set the file size
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn truncate(argc: i32, argv: *const *const u8) -> i32 {
    let mut size_arg: Option<&[u8]> = None;
    let mut no_create = false;
    let mut files: Vec<i32> = Vec::new();
    let mut exit_code = 0;

    // Parse options GNU-style: `-s SIZE`, `-sSIZE`, `--size=SIZE`,
    // `--size SIZE`, clusters like `-cs SIZE`, and `--` to end options
    let mut opts_done = false;
    let mut i = 1;
    while i < argc {
        let Some(arg) = (unsafe { get_arg(argv, i) }) else {
            i += 1;
            continue;
        };
        i += 1;

        if opts_done || arg.len() < 2 || arg[0] != b'-' {
            files.push(i - 1);
        } else if arg == b"--" {
            opts_done = true;
        } else if arg == b"--no-create" {
            no_create = true;
        } else if let Some(v) = arg.strip_prefix(b"--size=") {
            size_arg = Some(v);
        } else if arg == b"--size" {
            size_arg = unsafe { get_arg(argv, i) };
            if size_arg.is_none() {
                io::write_str(2, b"truncate: option '--size' requires an argument\n");
                return 1;
            }
            i += 1;
        } else if arg.starts_with(b"--") {
            io::write_str(2, b"truncate: unrecognized option '");
            io::write_all(2, arg);
            io::write_str(2, b"'\n");
            return 1;
        } else {
            for (k, &c) in arg.iter().enumerate().skip(1) {
                match c {
                    b'c' => no_create = true,
                    b's' => {
                        // The rest of the cluster, else the next argument
                        if k + 1 < arg.len() {
                            size_arg = Some(&arg[k + 1..]);
                        } else {
                            size_arg = unsafe { get_arg(argv, i) };
                            if size_arg.is_none() {
                                io::write_str(2, b"truncate: option requires an argument -- 's'\n");
                                return 1;
                            }
                            i += 1;
                        }
                        break;
                    }
                    _ => {
                        io::write_str(2, b"truncate: invalid option -- '");
                        io::write_all(2, &[c]);
                        io::write_str(2, b"'\n");
                        return 1;
                    }
                }
            }
        }
    }

    // Refuse to truncate anything unless a size was explicitly given via
    // -s; without it there is no safe target size, and defaulting to 0
    // would silently zero every file argument.
    let Some(size_arg) = size_arg else {
        io::write_str(2, b"truncate: you must specify -s SIZE\n");
        io::write_str(2, b"usage: truncate -s SIZE file...\n");
        return 1;
    };
    let size = match sys::parse_u64(size_arg) {
        Some(v) => v as i64,
        None => {
            io::write_str(2, b"truncate: invalid size specified for -s\n");
            return 1;
        }
    };

    // Second pass: process files
    for &j in files.iter() {
        if let Some(arg) = unsafe { get_arg(argv, j) } {
            // Like GNU truncate, open first: create missing files unless -c,
            // which skips them (any other failure is still an error), and
            // don't block on a FIFO or device node
            let mut flags = libc::O_WRONLY | libc::O_NONBLOCK | libc::O_NOCTTY;
            if !no_create {
                flags |= libc::O_CREAT;
            }
            let fd = io::open(arg, flags, 0o666);
            if fd < 0 {
                if !(no_create && sys::errno() == libc::ENOENT) {
                    sys::perror(arg);
                    exit_code = 1;
                }
                continue;
            }
            io::close(fd);

            if io::truncate(arg, size) < 0 {
                sys::perror(arg);
                exit_code = 1;
            }
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::process::Command;
    use std::fs;
    use std::path::PathBuf;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("armybox_truncate_test_{}_{}",  std::process::id(), counter));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_truncate_shrink() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "hello world!").unwrap();
        assert_eq!(fs::metadata(&file).unwrap().len(), 12);

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "5", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 5);
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello");
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_extend() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "hi").unwrap();
        assert_eq!(fs::metadata(&file).unwrap().len(), 2);

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 10);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_to_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "0", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 0);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_truncate_creates_missing_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("new.txt");

        let output = Command::new(&armybox)
            .args(["truncate", "-s", "10", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(fs::metadata(&file).unwrap().len(), 10);
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_no_create_skips_missing_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let existing = dir.join("existing.txt");
        fs::write(&existing, "hello world!").unwrap();

        for flag in ["-c", "--no-create"] {
            let missing = dir.join("missing.txt");
            let output = Command::new(&armybox)
                .args(["truncate", flag, "-s", "5", missing.to_str().unwrap(), existing.to_str().unwrap()])
                .output()
                .unwrap();

            assert_eq!(output.status.code(), Some(0), "{flag}");
            assert!(!missing.exists(), "{flag} created the file");
        }
        assert_eq!(fs::metadata(&existing).unwrap().len(), 5);
        cleanup(&dir);
    }

    fn run(args: &[&str]) -> std::process::Output {
        Command::new(get_armybox_path())
            .arg("truncate")
            .args(args)
            .output()
            .unwrap()
    }

    #[test]
    fn test_truncate_fifo_does_not_hang() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let fifo = dir.join("fifo");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0);

        // A FIFO with no reader must fail at once (ENXIO), not block in open()
        let mut child = Command::new(&armybox)
            .args(["truncate", "-s", "0", fifo.to_str().unwrap()])
            .spawn()
            .unwrap();
        let mut status = None;
        for _ in 0..50 {
            if let Some(s) = child.try_wait().unwrap() {
                status = Some(s);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            cleanup(&dir);
            panic!("truncate blocked on a FIFO");
        }
        assert_ne!(status.unwrap().code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_no_create_still_reports_other_errors() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }
        // root bypasses the permission check this relies on
        if unsafe { libc::geteuid() } == 0 { return; }

        let dir = setup();
        let locked = dir.join("locked");
        fs::create_dir(&locked).unwrap();
        let c = std::ffi::CString::new(locked.to_str().unwrap()).unwrap();
        unsafe { libc::chmod(c.as_ptr(), 0) };

        let output = run(&["-c", "-s", "1", locked.join("x").to_str().unwrap()]);

        unsafe { libc::chmod(c.as_ptr(), 0o755) };
        assert_ne!(output.status.code(), Some(0));
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_size_option_forms() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("f");
        let f = file.to_str().unwrap();
        for (args, want) in [
            (vec!["-s", "7", f], 7),
            (vec!["-s8", f], 8),
            (vec!["--size=9", f], 9),
            (vec!["--size", "10", f], 10),
            (vec!["-cs", "11", f], 11),
            (vec!["-s", "12", "--", f], 12),
        ] {
            let output = run(&args);
            assert_eq!(output.status.code(), Some(0), "{args:?}");
            assert_eq!(fs::metadata(&file).unwrap().len(), want, "{args:?}");
        }
        cleanup(&dir);
    }

    #[test]
    fn test_truncate_rejects_bad_options() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let dir = setup();
        let file = dir.join("f");
        let f = file.to_str().unwrap();
        // -sc takes "c" as the size, like GNU; -x and --bogus are unknown
        for args in [vec!["-sc", "5", f], vec!["-x", "-s", "1", f], vec!["--bogus", "-s", "1", f], vec!["-s"]] {
            let output = run(&args);
            assert_ne!(output.status.code(), Some(0), "{args:?}");
        }
        assert!(!file.exists());
        cleanup(&dir);
    }
}
