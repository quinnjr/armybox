//! su - run a command with substitute user and group ID
//!
//! Change the effective user ID and group ID.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;
// Shared password-verification primitives live in the login applet.
// NOTE: this makes the `su` feature depend on the `login` module being
// compiled in (add `su = ["login"]` in Cargo.toml).
use super::login;

/// User info from /etc/passwd
struct UserInfo {
    uid: u32,
    gid: u32,
    home: Vec<u8>,
    shell: Vec<u8>,
}

/// su - run a command with substitute user and group ID
///
/// # Synopsis
/// ```text
/// su [-] [-l] [-c command] [user]
/// ```
///
/// # Description
/// Change the effective user ID and group ID to that of user.
/// If no user is specified, root is assumed.
///
/// # Options
/// - `-`, `-l`, `--login`: Start a login shell
/// - `-c command`: Pass command to the shell
/// - `-s shell`: Use specified shell instead of user's shell
/// - `-p`, `--preserve-environment`: Preserve environment variables
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn su(argc: i32, argv: *const *const u8) -> i32 {
    let mut user = b"root" as &[u8];
    let mut command: Option<&[u8]> = None;
    let mut login_shell = false;
    let mut preserve_env = false;
    let mut shell_override: Option<&[u8]> = None;
    let mut i = 1;

    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-c" && i + 1 < argc {
                command = unsafe { get_arg(argv, i + 1) };
                i += 2;
                continue;
            } else if arg == b"-s" && i + 1 < argc {
                shell_override = unsafe { get_arg(argv, i + 1) };
                i += 2;
                continue;
            } else if arg == b"-" || arg == b"-l" || arg == b"--login" {
                login_shell = true;
            } else if arg == b"-p" || arg == b"--preserve-environment" {
                preserve_env = true;
            } else if arg == b"-h" || arg == b"--help" {
                print_help();
                return 0;
            } else if !arg.starts_with(b"-") {
                user = arg;
            }
        }
        i += 1;
    }

    // Look up user in /etc/passwd
    let user_info = match lookup_user(user) {
        Some(info) => info,
        None => {
            io::write_str(2, b"su: user ");
            io::write_all(2, user);
            io::write_str(2, b" does not exist\n");
            return 1;
        }
    };

    // Authenticate the caller BEFORE changing any credentials.
    // Root (real uid 0) may su to anyone without a password; every other
    // caller must prove knowledge of the target user's password.
    if unsafe { libc::getuid() } != 0 {
        // Fetch the target user's password hash from /etc/shadow.
        let hash = login::get_shadow_hash(user);

        // Prompt with terminal echo disabled.
        io::write_str(2, b"Password: ");
        let old_termios = login::disable_echo();
        let password = match login::read_line() {
            Some(p) => p,
            None => {
                login::restore_termios(&old_termios);
                io::write_str(2, b"\n");
                io::write_str(2, b"su: Authentication failure\n");
                return 1;
            }
        };
        login::restore_termios(&old_termios);
        io::write_str(2, b"\n");

        // An empty/locked hash makes verify_password return false, so a
        // missing shadow entry is rejected rather than treated as no password.
        if !login::verify_password(&password, &hash) {
            io::write_str(2, b"su: Authentication failure\n");
            return 1;
        }
    }

    let shell = shell_override
        .map(|s| s.to_vec())
        .unwrap_or(user_info.shell.clone());

    // Set group ID first (must be done before setuid)
    if unsafe { libc::setgid(user_info.gid) } != 0 {
        sys::perror(b"setgid");
        return 1;
    }

    // Initialize supplementary groups
    let mut user_buf = [0u8; 256];
    let user_len = user.len().min(user_buf.len() - 1);
    user_buf[..user_len].copy_from_slice(&user[..user_len]);
    unsafe {
        libc::initgroups(
            user_buf.as_ptr() as *const libc::c_char,
            user_info.gid as libc::gid_t,
        );
    }

    // Set user ID
    if unsafe { libc::setuid(user_info.uid) } != 0 {
        sys::perror(b"setuid");
        return 1;
    }

    // Set environment if login shell
    if login_shell && !preserve_env {
        // Set HOME
        let mut home_env = b"HOME=".to_vec();
        home_env.extend_from_slice(&user_info.home);
        home_env.push(0);
        unsafe {
            libc::putenv(home_env.as_ptr() as *mut libc::c_char);
        }
        core::mem::forget(home_env); // Don't free the env string

        // Set SHELL
        let mut shell_env = b"SHELL=".to_vec();
        shell_env.extend_from_slice(&shell);
        shell_env.push(0);
        unsafe {
            libc::putenv(shell_env.as_ptr() as *mut libc::c_char);
        }
        core::mem::forget(shell_env);

        // Set USER
        let mut user_env = b"USER=".to_vec();
        user_env.extend_from_slice(user);
        user_env.push(0);
        unsafe {
            libc::putenv(user_env.as_ptr() as *mut libc::c_char);
        }
        core::mem::forget(user_env);

        // Set LOGNAME
        let mut logname_env = b"LOGNAME=".to_vec();
        logname_env.extend_from_slice(user);
        logname_env.push(0);
        unsafe {
            libc::putenv(logname_env.as_ptr() as *mut libc::c_char);
        }
        core::mem::forget(logname_env);

        // Change to home directory
        let mut home_buf = [0u8; 4096];
        let home_len = user_info.home.len().min(home_buf.len() - 1);
        home_buf[..home_len].copy_from_slice(&user_info.home[..home_len]);
        unsafe {
            libc::chdir(home_buf.as_ptr() as *const libc::c_char);
        }
    }

    // Build shell path
    let mut shell_buf = [0u8; 256];
    let shell_len = shell.len().min(shell_buf.len() - 1);
    shell_buf[..shell_len].copy_from_slice(&shell[..shell_len]);

    // Execute command or shell
    if let Some(cmd) = command {
        let mut cmd_buf = [0u8; 4096];
        let cmd_len = cmd.len().min(cmd_buf.len() - 1);
        cmd_buf[..cmd_len].copy_from_slice(&cmd[..cmd_len]);

        let c_flag = b"-c\0";
        let argv_ptrs = [
            shell_buf.as_ptr() as *const libc::c_char,
            c_flag.as_ptr() as *const libc::c_char,
            cmd_buf.as_ptr() as *const libc::c_char,
            core::ptr::null(),
        ];

        unsafe {
            libc::execv(
                shell_buf.as_ptr() as *const libc::c_char,
                argv_ptrs.as_ptr(),
            );
        }
    } else {
        // Build shell name for argv[0] (with or without leading -)
        let mut shell_name = [0u8; 256];
        let shell_basename = shell
            .iter()
            .rposition(|&c| c == b'/')
            .map(|p| &shell[p + 1..])
            .unwrap_or(&shell);

        let start = if login_shell {
            shell_name[0] = b'-';
            1
        } else {
            0
        };

        let name_len = shell_basename.len().min(shell_name.len() - start - 1);
        shell_name[start..start + name_len].copy_from_slice(&shell_basename[..name_len]);

        let argv_ptrs = [
            shell_name.as_ptr() as *const libc::c_char,
            core::ptr::null(),
        ];

        unsafe {
            libc::execv(
                shell_buf.as_ptr() as *const libc::c_char,
                argv_ptrs.as_ptr(),
            );
        }
    }

    sys::perror(b"exec");
    1
}

fn print_help() {
    io::write_str(1, b"Usage: su [OPTIONS] [USER]\n\n");
    io::write_str(1, b"Run shell as another user.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -, -l, --login    Start a login shell\n");
    io::write_str(1, b"  -c COMMAND        Run COMMAND instead of shell\n");
    io::write_str(
        1,
        b"  -s SHELL          Use SHELL instead of user's shell\n",
    );
    io::write_str(1, b"  -p                Preserve environment\n");
    io::write_str(1, b"  -h, --help        Show this help\n");
}

/// Look up user in /etc/passwd
/// Format: name:password:uid:gid:gecos:home:shell
fn lookup_user(username: &[u8]) -> Option<UserInfo> {
    let fd = io::open(b"/etc/passwd", libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }

    let content = io::read_all(fd);
    io::close(fd);

    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        let fields: Vec<&[u8]> = line.split(|&c| c == b':').collect();
        if fields.len() < 7 {
            continue;
        }

        let name = fields[0];
        if name != username {
            continue;
        }

        // Parse uid and gid. Reject a malformed entry outright instead of
        // silently falling back to uid/gid 0 (root), which would be a
        // privilege-escalation fail-open.
        let uid = {
            let v = sys::parse_u64(fields[2])?;
            v as u32
        };
        let gid = {
            let v = sys::parse_u64(fields[3])?;
            v as u32
        };
        let home = fields[5].to_vec();
        let shell = if fields[6].is_empty() {
            b"/bin/sh".to_vec()
        } else {
            fields[6].to_vec()
        };

        return Some(UserInfo {
            uid,
            gid,
            home,
            shell,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_su_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["su", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_su_invalid_user() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["su", "nonexistent_user_12345"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("does not exist"));
    }
}
