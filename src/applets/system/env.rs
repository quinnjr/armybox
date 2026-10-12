//! env - run a program in a modified environment
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/env.html

use crate::io;

/// env - run a program in a modified environment
///
/// # Synopsis
/// ```text
/// env [name=value]... [utility [argument...]]
/// ```
///
/// # Description
/// When called with no arguments, print the environment.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn env(argc: i32, argv: *const *const u8) -> i32 {
    unsafe extern "C" {
        static environ: *const *const libc::c_char;
    }

    // NUL-terminate a byte slice for the C environment functions.
    fn cstr(bytes: &[u8]) -> alloc::vec::Vec<u8> {
        let mut v = alloc::vec::Vec::with_capacity(bytes.len() + 1);
        v.extend_from_slice(bytes);
        v.push(0);
        v
    }

    let mut i = 1;

    // --- Options: [-i] [-u NAME]... [--] ---
    while i < argc {
        let arg = match unsafe { super::get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if arg == b"-i" || arg == b"--ignore-environment" || arg == b"-" {
            unsafe { libc::clearenv() };
            i += 1;
        } else if arg == b"--" {
            i += 1;
            break;
        } else if arg == b"-u" || arg == b"--unset" {
            // Name is the following operand.
            i += 1;
            if let Some(name) = unsafe { super::get_arg(argv, i) } {
                let c = cstr(name);
                unsafe { libc::unsetenv(c.as_ptr() as *const libc::c_char) };
                i += 1;
            } else {
                io::write_str(2, b"env: option requires an argument -- 'u'\n");
                return 2;
            }
        } else if arg.len() > 2 && arg[0] == b'-' && arg[1] == b'u' {
            // -uNAME form.
            let c = cstr(&arg[2..]);
            unsafe { libc::unsetenv(c.as_ptr() as *const libc::c_char) };
            i += 1;
        } else {
            break;
        }
    }

    // --- Assignments: NAME=VALUE ... ---
    while i < argc {
        let arg = match unsafe { super::get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        // An operand is an assignment iff it contains '='. The first operand
        // without '=' is the utility to execute.
        if let Some(eq) = arg.iter().position(|&b| b == b'=') {
            let name = cstr(&arg[..eq]);
            let value = cstr(&arg[eq + 1..]);
            unsafe {
                libc::setenv(
                    name.as_ptr() as *const libc::c_char,
                    value.as_ptr() as *const libc::c_char,
                    1,
                )
            };
            i += 1;
        } else {
            break;
        }
    }

    // --- Utility, or list the environment ---
    if i < argc {
        // Exec the utility with the remaining argv (already NUL-terminated by
        // the OS) in the (possibly modified) environment.
        let file = unsafe { *argv.add(i as usize) };
        unsafe {
            libc::execvp(
                file as *const libc::c_char,
                argv.add(i as usize) as *const *const libc::c_char,
            )
        };
        // execvp only returns on failure.
        let err = crate::sys::errno();
        let name = unsafe { super::get_arg(argv, i) }.unwrap_or(b"");
        io::write_str(2, b"env: ");
        io::write_all(2, name);
        io::write_str(2, b": command not found\n");
        return if err == libc::ENOENT { 127 } else { 126 };
    }

    // List mode.
    unsafe {
        let mut j = 0;
        while !(*environ.add(j)).is_null() {
            let e = io::cstr_to_slice(*environ.add(j) as *const u8);
            io::write_all(1, e);
            io::write_str(1, b"\n");
            j += 1;
        }
    }
    0
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
    fn test_env_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["env"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_env_contains_path() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["env"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Environment should contain PATH
        assert!(stdout.contains("PATH="));
    }

    #[test]
    fn test_env_format() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["env"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Each line should contain = for variable assignments
        for line in stdout.lines() {
            if !line.is_empty() {
                assert!(line.contains("="), "Line missing '=': {}", line);
            }
        }
    }
}
