//! nice - run a command with modified scheduling priority
//!
//! Run a command with an adjusted niceness value.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// nice - run a command with modified scheduling priority
///
/// # Synopsis
/// ```text
/// nice [-n increment] [command [arg...]]
/// ```
///
/// # Description
/// Run a command with an adjusted niceness value. Without arguments,
/// print the current niceness. The niceness ranges from -20 (highest
/// priority) to 19 (lowest priority).
///
/// # Exit Status
/// - 0: Success (when printing niceness)
/// - Command's exit status when running a command
/// - 126: Utility found but could not be executed
/// - 127: Utility not found
pub fn nice(argc: i32, argv: *const *const u8) -> i32 {
    let mut adjustment = 10i32;
    let mut cmd_start = 1;

    if argc > 1 {
        if let Some(arg) = unsafe { get_arg(argv, 1) } {
            if arg.starts_with(b"-n") {
                if arg.len() > 2 {
                    adjustment = sys::parse_i64(&arg[2..]).unwrap_or(10) as i32;
                    cmd_start = 2;
                } else if argc > 2 {
                    if let Some(n) = unsafe { get_arg(argv, 2) } {
                        adjustment = sys::parse_i64(n).unwrap_or(10) as i32;
                        cmd_start = 3;
                    } else {
                        cmd_start = 2;
                    }
                } else {
                    cmd_start = 2;
                }
            }
        }
    }

    if cmd_start >= argc {
        io::write_num(1, unsafe { libc::nice(0) } as u64);
        io::write_str(1, b"\n");
        return 0;
    }

    unsafe { libc::nice(adjustment) };

    let cmd = unsafe { get_arg(argv, cmd_start).unwrap() };
    let mut cmd_buf = [0u8; 4096];
    cmd_buf[..cmd.len()].copy_from_slice(cmd);
    cmd_buf[cmd.len()] = 0;

    unsafe {
        libc::execvp(
            cmd_buf.as_ptr() as *const libc::c_char,
            argv.add(cmd_start as usize) as *const *const libc::c_char,
        );
    }

    let err = sys::errno();
    sys::perror(cmd);
    if err == libc::ENOENT { 127 } else { 126 }
}

#[cfg(test)]
mod tests {
    extern crate std;
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

    #[test]
    fn test_nice_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["nice"]).output().unwrap();

        // Should print the current niceness and exit 0
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should be a number
        assert!(stdout.trim().parse::<i32>().is_ok());
    }

    #[test]
    fn test_nice_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["nice", "/bin/true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
