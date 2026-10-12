//! Shell entry points
//!
//! This module contains the main entry points for the shell applet:
//! - `sh` - The main POSIX shell entry point
//! - `ash` - Alias for sh (Almquist shell compatible)
//! - `dash` - Alias for sh (Debian Almquist shell compatible)
//!
//! It also contains the interactive loop and minimal shell implementations.

use super::get_arg;
use crate::io;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use super::state::Shell;

#[cfg(feature = "alloc")]
use super::execute::execute_script;

/// Main shell entry point
pub fn sh(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        let mut script_file: Option<&[u8]> = None;
        let mut command_string: Option<&[u8]> = None;
        let mut login_shell = false;
        let mut script_args: Vec<&[u8]> = Vec::new();

        // Get $0 (the shell name itself)
        let shell_name = unsafe { get_arg(argv, 0) }.unwrap_or(b"sh");

        // Parse arguments
        let mut i = 1;
        while i < argc {
            let arg = match unsafe { get_arg(argv, i) } {
                Some(a) => a,
                None => {
                    i += 1;
                    continue;
                }
            };

            // If we already found a script file, everything else is a
            // positional parameter for that script ($1, $2, ...)
            if script_file.is_some() {
                script_args.push(arg);
                i += 1;
                continue;
            }

            if arg == b"-c" {
                // Execute command string
                if i + 1 < argc {
                    command_string = unsafe { get_arg(argv, i + 1) };
                    i += 1;
                }
                // After -c "cmd", the next arg (if any) becomes $0,
                // and remaining args become $1, $2, ...
                i += 1;
                if command_string.is_some() {
                    // Collect remaining args as positional params for -c
                    while i < argc {
                        if let Some(a) = unsafe { get_arg(argv, i) } {
                            script_args.push(a);
                        }
                        i += 1;
                    }
                }
                break;
            } else if arg == b"-l" || arg == b"--login" {
                login_shell = true;
            } else if arg == b"-s"
                || arg == b"-i"
                || arg == b"-e"
                || arg == b"-x"
                || arg == b"-v"
                || arg == b"-n"
            {
                // Known flags, skip
            } else if !arg.is_empty() && arg[0] == b'-' {
                // Unknown flag, skip
            } else {
                // First non-option argument is the script file
                script_file = Some(arg);
                // All subsequent args are positional params — handled above
            }
            i += 1;
        }

        if let Some(cmd) = command_string {
            // Execute -c command
            let mut shell = Shell::new(false);
            if login_shell {
                source_profile(&mut shell);
            }
            // For -c: if there are extra args, first is $0, rest are $1..
            if !script_args.is_empty() {
                let dollar0 = script_args[0];
                let params: Vec<&[u8]> = script_args[1..].to_vec();
                shell.set_positional_params(dollar0, &params);
            } else {
                shell.set_positional_params(shell_name, &[]);
            }
            execute_script(&mut shell, cmd);
            return shell.last_status;
        }

        if let Some(file) = script_file {
            // Execute script file
            let fd = io::open(file, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"sh: cannot open ");
                io::write_all(2, file);
                io::write_str(2, b"\n");
                return 127;
            }
            let content = io::read_all(fd);
            io::close(fd);

            let mut shell = Shell::new(false);
            if login_shell {
                source_profile(&mut shell);
            }
            // $0 = script file, $1.. = remaining args
            shell.set_positional_params(file, &script_args);
            execute_script(&mut shell, &content);
            return shell.last_status;
        }

        // Interactive mode
        let interactive = io::isatty(0);
        let mut shell = Shell::new(interactive);
        shell.set_positional_params(shell_name, &[]);

        // Source profile for login shells or interactive sessions
        if login_shell || interactive {
            source_profile(&mut shell);
        }

        if interactive {
            io::write_str(1, b"ArmyBox sh\n");
        }

        interactive_loop(&mut shell);
        return shell.exit_code;
    }

    #[cfg(not(feature = "alloc"))]
    {
        // Minimal shell without alloc
        io::write_str(1, b"ArmyBox sh (minimal)\n");
        minimal_shell();
        0
    }
}

/// Ash shell entry point (alias for sh)
pub fn ash(argc: i32, argv: *const *const u8) -> i32 {
    sh(argc, argv)
}

/// Dash shell entry point (alias for sh)
pub fn dash(argc: i32, argv: *const *const u8) -> i32 {
    sh(argc, argv)
}

/// Source /etc/profile if it exists
///
/// This loads the system-wide shell profile, setting up variables and aliases.
#[cfg(feature = "alloc")]
fn source_profile(shell: &mut Shell) {
    let profile = b"/etc/profile";
    let fd = io::open(profile, libc::O_RDONLY, 0);
    if fd >= 0 {
        let content = io::read_all(fd);
        io::close(fd);
        execute_script(shell, &content);
    }
}

/// Minimal shell for no-alloc builds
///
/// This provides a very basic shell implementation that works without
/// heap allocation. It supports:
/// - Simple command execution via fork/exec
/// - The `cd` builtin
/// - The `exit` builtin
#[cfg(not(feature = "alloc"))]
pub(super) fn minimal_shell() {
    let mut line_buf = [0u8; 1024];
    let mut pos = 0;

    loop {
        if io::isatty(0) {
            io::write_str(1, b"$ ");
        }

        pos = 0;
        loop {
            let mut c = [0u8; 1];
            let n = io::read(0, &mut c);
            if n <= 0 {
                if pos == 0 {
                    return;
                }
                break;
            }
            if c[0] == b'\n' {
                break;
            }
            if pos < line_buf.len() - 1 {
                line_buf[pos] = c[0];
                pos += 1;
            }
        }

        if pos == 0 {
            continue;
        }

        let line = &line_buf[..pos];
        if line == b"exit" {
            return;
        }

        // Handle cd
        if line.starts_with(b"cd ") {
            let path = &line[3..];
            let mut path_buf = [0u8; 256];
            let plen = core::cmp::min(path.len(), path_buf.len() - 1);
            path_buf[..plen].copy_from_slice(&path[..plen]);
            unsafe {
                if libc::chdir(path_buf.as_ptr() as *const libc::c_char) != 0 {
                    io::write_str(2, b"cd: failed\n");
                }
            }
            continue;
        }

        // Fork and exec
        let pid = io::fork();
        if pid == 0 {
            let mut args: [*const libc::c_char; 32] = [core::ptr::null(); 32];
            let mut arg_count = 0;
            let mut start = 0;
            let mut in_word = false;

            for i in 0..=pos {
                if i == pos || line_buf[i] == b' ' || line_buf[i] == b'\t' {
                    if in_word && arg_count < 31 {
                        line_buf[i] = 0;
                        args[arg_count] = line_buf[start..].as_ptr() as *const libc::c_char;
                        arg_count += 1;
                        in_word = false;
                    }
                } else if !in_word {
                    start = i;
                    in_word = true;
                }
            }

            if arg_count > 0 {
                unsafe {
                    libc::execvp(args[0], args.as_ptr());
                }
                io::write_str(2, b"sh: command not found\n");
            }
            io::exit(127);
        }

        let mut status: i32 = 0;
        io::waitpid(pid, &mut status, 0);
    }
}

/// Interactive shell loop
///
/// Reads commands from stdin and executes them until EOF or the shell
/// is instructed to exit.
#[cfg(feature = "alloc")]
pub(super) fn interactive_loop(shell: &mut Shell) {
    let mut line_buf = Vec::new();

    loop {
        if shell.should_exit {
            return;
        }

        if shell.interactive {
            io::write_str(1, b"$ ");
        }

        line_buf.clear();
        loop {
            let mut c = [0u8; 1];
            let n = io::read(0, &mut c);
            if n <= 0 {
                if line_buf.is_empty() {
                    shell.should_exit = true;
                    return;
                }
                break;
            }
            if c[0] == b'\n' {
                break;
            }
            line_buf.push(c[0]);
        }

        if line_buf.is_empty() {
            continue;
        }

        execute_script(shell, &line_buf);
    }
}
