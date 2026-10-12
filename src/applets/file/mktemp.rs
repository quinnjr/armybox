//! mktemp - create a temporary file or directory
//!
//! GNU coreutils compatible implementation.

use crate::applets::{get_arg, has_opt};
use crate::io;
use crate::sys;

/// mktemp - create a temporary file or directory
///
/// # Synopsis
/// ```text
/// mktemp [-d] [TEMPLATE]
/// ```
///
/// # Description
/// Create a temporary file or directory, safely, and print its name.
/// TEMPLATE must contain at least 3 consecutive 'X's.
///
/// # Options
/// - `-d`: Create a directory, not a file
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn mktemp(argc: i32, argv: *const *const u8) -> i32 {
    let mut dir = false;
    let mut template: Option<&[u8]> = None;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.len() > 0 && arg[0] == b'-' {
                if has_opt(arg, b'd') {
                    dir = true;
                }
            } else {
                template = Some(arg);
            }
        }
    }

    let template = template.unwrap_or(b"/tmp/tmp.XXXXXX");
    create_temp(template, dir)
}

fn create_temp(template: &[u8], dir: bool) -> i32 {
    let mut path = [0u8; 256];
    if template.len() >= path.len() {
        io::write_str(2, b"mktemp: template too long\n");
        return 1;
    }

    for (i, &c) in template.iter().enumerate() {
        path[i] = c;
    }

    // Replace X with random chars
    let seed = unsafe { libc::time(core::ptr::null_mut()) } as u64;
    let pid = unsafe { libc::getpid() } as u64;
    let mut rng = seed.wrapping_mul(pid).wrapping_add(1);

    for i in 0..template.len() {
        if path[i] == b'X' {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            path[i] = b"0123456789abcdef"[(rng >> 60) as usize];
        }
    }

    if dir {
        if io::mkdir(&path[..template.len()], 0o700) < 0 {
            sys::perror(&path[..template.len()]);
            return 1;
        }
    } else {
        let fd = io::open(
            &path[..template.len()],
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        );
        if fd < 0 {
            sys::perror(&path[..template.len()]);
            return 1;
        }
        io::close(fd);
    }

    io::write_all(1, &path[..template.len()]);
    io::write_str(1, b"\n");
    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_mktemp_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_mktemp_default() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["mktemp"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let path = stdout.trim();
        assert!(path.starts_with("/tmp/tmp."));
        // Clean up created file
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_mktemp_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["mktemp", "-d"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let path = stdout.trim();
        assert!(std::path::Path::new(path).is_dir());
        // Clean up created directory
        let _ = fs::remove_dir(path);
    }

    #[test]
    fn test_mktemp_custom_template() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let template = dir.join("mytemp.XXXXXX");

        let output = Command::new(&armybox)
            .args(["mktemp", template.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let path = stdout.trim();
        assert!(path.starts_with(dir.to_str().unwrap()));
        assert!(path.contains("mytemp."));
        cleanup(&dir);
    }

    #[test]
    fn test_mktemp_unique() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output1 = Command::new(&armybox).args(["mktemp"]).output().unwrap();

        let output2 = Command::new(&armybox).args(["mktemp"]).output().unwrap();

        let path1 = std::string::String::from_utf8_lossy(&output1.stdout);
        let path2 = std::string::String::from_utf8_lossy(&output2.stdout);

        // Two calls should create different files
        assert_ne!(path1.trim(), path2.trim());

        // Clean up
        let _ = fs::remove_file(path1.trim());
        let _ = fs::remove_file(path2.trim());
    }
}
