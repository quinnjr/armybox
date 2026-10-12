//! linux32 - set execution domain to 32-bit
//!
//! Run a program in 32-bit personality mode.

use super::get_arg;
use crate::sys;

// Personality constants
const PER_LINUX32: u64 = 0x0008;

/// linux32 - set execution domain to 32-bit
///
/// # Synopsis
/// ```text
/// linux32 [PROGRAM [ARGS...]]
/// ```
///
/// # Description
/// Set the execution domain to 32-bit Linux and optionally
/// execute a program. Used to run 32-bit programs that check
/// `uname -m`.
///
/// # Exit Status
/// - Exit status of PROGRAM, or 1 on error
pub fn linux32(argc: i32, argv: *const *const u8) -> i32 {
    // Set personality to 32-bit
    let result = unsafe { libc::personality(PER_LINUX32 as _) };
    if result < 0 {
        sys::perror(b"personality");
        return 1;
    }

    if argc < 2 {
        // No program specified, exec shell
        let shell = b"/bin/sh\0";
        let args: [*const u8; 2] = [shell.as_ptr(), core::ptr::null()];
        unsafe {
            libc::execv(
                shell.as_ptr() as *const libc::c_char,
                args.as_ptr() as *const *const libc::c_char,
            );
        }
        sys::perror(b"/bin/sh");
        return 1;
    }

    // Build argv for exec
    let prog = unsafe { get_arg(argv, 1).unwrap() };

    // Need null-terminated strings
    let mut prog_buf = [0u8; 256];
    let len = prog.len().min(prog_buf.len() - 1);
    prog_buf[..len].copy_from_slice(&prog[..len]);

    // Execute the program
    unsafe {
        libc::execvp(
            prog_buf.as_ptr() as *const libc::c_char,
            argv.add(1) as *const *const libc::c_char,
        );
    }

    sys::perror(prog);
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_linux32_with_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["linux32", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_linux32_uname() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["linux32", "uname", "-m"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // On 64-bit system with linux32, should show i686 or similar
        assert!(!stdout.is_empty());
    }
}
