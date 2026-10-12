//! hostname - show or set the system hostname
//!
//! Shows or sets the system's hostname.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// hostname - show or set the system hostname
///
/// # Synopsis
/// ```text
/// hostname [-F file] [name]
/// ```
///
/// # Description
/// With no arguments, print the current hostname.
/// With `-F file`, read the hostname from the specified file.
/// With a plain argument, set the hostname (requires root).
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn hostname(argc: i32, argv: *const *const u8) -> i32 {
    let mut i = 1;
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => {
                i += 1;
                continue;
            }
        };

        if arg == b"-F" || arg == b"-f" || arg == b"--file" {
            // Read hostname from file
            i += 1;
            if i >= argc {
                io::write_str(2, b"hostname: option requires an argument -- 'F'\n");
                return 1;
            }
            let file = match unsafe { get_arg(argv, i) } {
                Some(f) => f,
                None => return 1,
            };
            let fd = io::open(file, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"hostname: ");
                io::write_all(2, file);
                io::write_str(2, b": No such file or directory\n");
                return 1;
            }
            let content = io::read_all(fd);
            io::close(fd);
            // Trim trailing whitespace/newlines
            let name = content.trim_ascii();
            if !name.is_empty() {
                if unsafe { libc::sethostname(name.as_ptr() as *const libc::c_char, name.len()) }
                    < 0
                {
                    sys::perror(b"sethostname");
                    return 1;
                }
            }
            return 0;
        } else if arg == b"-s" || arg == b"--short" {
            // Print short hostname (up to first dot)
            let mut buf = [0u8; 256];
            if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
                let full = unsafe { io::cstr_to_slice(buf.as_ptr()) };
                if let Some(dot) = full.iter().position(|&c| c == b'.') {
                    io::write_all(1, &full[..dot]);
                } else {
                    io::write_all(1, full);
                }
                io::write_str(1, b"\n");
            }
            return 0;
        } else if arg == b"-d" || arg == b"--domain" {
            // Print domain name
            let mut buf = [0u8; 256];
            if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
                let full = unsafe { io::cstr_to_slice(buf.as_ptr()) };
                if let Some(dot) = full.iter().position(|&c| c == b'.') {
                    io::write_all(1, &full[dot + 1..]);
                }
                io::write_str(1, b"\n");
            }
            return 0;
        } else if arg[0] != b'-' {
            // Set hostname directly
            if unsafe { libc::sethostname(arg.as_ptr() as *const libc::c_char, arg.len()) } < 0 {
                sys::perror(b"sethostname");
                return 1;
            }
            return 0;
        }

        i += 1;
    }

    // No arguments - print current hostname
    let mut buf = [0u8; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0 {
        io::write_all(1, unsafe { io::cstr_to_slice(buf.as_ptr()) });
        io::write_str(1, b"\n");
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
    fn test_hostname_show() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["hostname"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.trim().is_empty());
    }

    #[test]
    fn test_hostname_matches_system() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Compare with system hostname command
        let system_output = Command::new("hostname").output().unwrap();
        let system_hostname = std::string::String::from_utf8_lossy(&system_output.stdout);

        let output = Command::new(&armybox).args(["hostname"]).output().unwrap();

        let armybox_hostname = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(armybox_hostname.trim(), system_hostname.trim());
    }
}
