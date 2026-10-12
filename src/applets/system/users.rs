//! users - print logged in users
//!
//! Prints the login names of users currently logged in.

use crate::io;

// utmp entry types
const USER_PROCESS: i16 = 7;

// utmp structure for Linux (glibc compatible)
#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct Utmp {
    ut_type: i16,
    ut_pid: i32,
    ut_line: [u8; 32],
    ut_id: [u8; 4],
    ut_user: [u8; 32],
    ut_host: [u8; 256],
    ut_exit: ExitStatus,
    ut_session: i32,
    ut_tv: Timeval,
    ut_addr_v6: [i32; 4],
    __unused: [u8; 20],
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct ExitStatus {
    e_termination: i16,
    e_exit: i16,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct Timeval {
    tv_sec: i32,
    tv_usec: i32,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const UTMP_SIZE: usize = core::mem::size_of::<Utmp>();

/// users - print logged in users
///
/// # Synopsis
/// ```text
/// users [FILE]
/// ```
///
/// # Description
/// Print login names of users currently logged in, separated by spaces.
/// If FILE is not specified, /var/run/utmp is used.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn users(argc: i32, argv: *const *const u8) -> i32 {
    // Determine utmp file to use
    let utmp_path = if argc > 1 {
        match unsafe { super::get_arg(argv, 1) } {
            Some(p) => p,
            None => b"/var/run/utmp\0" as &[u8],
        }
    } else {
        b"/var/run/utmp\0"
    };

    // Try to open utmp file
    let fd = io::open(utmp_path, libc::O_RDONLY, 0);
    let fd = if fd < 0 {
        // Try alternate location
        let fd2 = io::open(b"/run/utmp\0", libc::O_RDONLY, 0);
        if fd2 < 0 {
            // No utmp available - this is OK, just no users
            io::write_str(1, b"\n");
            return 0;
        }
        fd2
    } else {
        fd
    };

    // Collect usernames
    let mut buf = [0u8; UTMP_SIZE];
    let mut first = true;

    loop {
        let n = io::read(fd, &mut buf);
        if (n as usize) < UTMP_SIZE {
            break;
        }

        // SAFETY: buf is properly sized and aligned for Utmp
        let entry: &Utmp = unsafe { &*(buf.as_ptr() as *const Utmp) };

        // Only show logged-in users
        if entry.ut_type != USER_PROCESS {
            continue;
        }

        // Get user name (null-terminated)
        let user_len = entry.ut_user.iter().position(|&c| c == 0).unwrap_or(32);
        if user_len == 0 {
            continue;
        }

        // Print separator
        if !first {
            io::write_str(1, b" ");
        }
        first = false;

        // Print username
        io::write_all(1, &entry.ut_user[..user_len]);
    }

    io::close(fd);

    // Print newline
    io::write_str(1, b"\n");

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn users(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"users: only available on Linux\n");
    1
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
    fn test_users_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["users"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_users_produces_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["users"]).output().unwrap();

        // Should at least print a newline
        assert!(!output.stdout.is_empty());
    }
}
