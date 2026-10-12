//! Shell built-in commands
//!
//! This module implements the built-in commands for the shell, including:
//! - exit: Exit the shell with optional status code
//! - cd: Change current directory
//! - pwd: Print working directory
//! - export: Export variables to environment
//! - unset: Remove variables
//! - echo: Print arguments
//! - test/[: Evaluate conditional expressions
//! - true/false/:: Return success/failure/no-op
//! - source/.: Execute script in current environment
//! - read: Read line from stdin into variable
//! - exec: Replace shell with command
//! - set: Set shell options
//! - shift: Shift positional parameters
//! - return: Return from function or sourced script

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use crate::io;
#[cfg(feature = "alloc")]
use crate::sys;

#[cfg(feature = "alloc")]
use super::execute::{execute_command, execute_script};
#[cfg(feature = "alloc")]
use super::parser::Command;
#[cfg(feature = "alloc")]
use super::state::Shell;
#[cfg(feature = "alloc")]
use super::util::{
    file_exists, file_size, is_directory, is_executable, is_readable, is_regular_file, is_symlink,
    is_writable,
};

/// Execute a built-in command if recognized
///
/// Returns true if the command was handled as a builtin, false otherwise.
/// Handles: exit, cd, export, unset, set, pwd, echo, test, [, true, false, :,
/// read, shift, return, break, continue, exec, eval, source, .
#[cfg(feature = "alloc")]
pub(super) fn execute_builtin(shell: &mut Shell, cmd: &Command) -> bool {
    if cmd.args.is_empty() {
        return true;
    }

    let name = &cmd.args[0];

    if name == b"exit" {
        shell.should_exit = true;
        shell.exit_code = if cmd.args.len() > 1 {
            sys::parse_i64(&cmd.args[1]).unwrap_or(0) as i32
        } else {
            shell.last_status
        };
        return true;
    }

    if name == b"cd" {
        let path = if cmd.args.len() > 1 {
            &cmd.args[1]
        } else if let Some(home) = io::getenv(b"HOME") {
            home
        } else {
            b"/root"
        };

        if io::chdir(path) < 0 {
            io::write_str(2, b"cd: ");
            io::write_all(2, path);
            io::write_str(2, b": No such directory\n");
            shell.last_status = 1;
        } else {
            shell.last_status = 0;
        }
        return true;
    }

    if name == b"pwd" {
        let mut buf = [0u8; 4096];
        unsafe {
            if !libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, buf.len()).is_null() {
                let len = io::strlen(buf.as_ptr());
                io::write_all(1, &buf[..len]);
                io::write_str(1, b"\n");
                shell.last_status = 0;
            } else {
                shell.last_status = 1;
            }
        }
        return true;
    }

    if name == b"export" {
        if cmd.args.len() > 1 {
            let arg = &cmd.args[1];
            if let Some(eq_pos) = arg.iter().position(|&c| c == b'=') {
                let name_part = &arg[..eq_pos];
                let value_part = &arg[eq_pos + 1..];
                // Use io::setenv which handles null-termination safely
                io::setenv(name_part, value_part, true);
            } else {
                // Export existing variable
                if let Some(value) = shell.get_var(arg) {
                    io::setenv(arg, value, true);
                }
            }
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"unset" {
        if cmd.args.len() > 1 {
            let var = &cmd.args[1];
            shell.variables.remove(var);
            // Use io::unsetenv which handles null-termination safely
            io::unsetenv(var);
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"echo" {
        let mut newline = true;
        let mut start = 1;

        if cmd.args.len() > 1 && cmd.args[1] == b"-n" {
            newline = false;
            start = 2;
        }

        let mut first = true;
        for arg in &cmd.args[start..] {
            if !first {
                io::write_str(1, b" ");
            }
            io::write_all(1, arg);
            first = false;
        }
        if newline {
            io::write_str(1, b"\n");
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"test" || name == b"[" {
        shell.last_status = execute_test(&cmd.args[1..]);
        return true;
    }

    if name == b"true" {
        shell.last_status = 0;
        return true;
    }

    if name == b"false" {
        shell.last_status = 1;
        return true;
    }

    if name == b":" {
        shell.last_status = 0;
        return true;
    }

    if name == b"source" || name == b"." {
        if cmd.args.len() > 1 {
            let fd = io::open(&cmd.args[1], libc::O_RDONLY, 0);
            if fd >= 0 {
                let content = io::read_all(fd);
                io::close(fd);
                execute_script(shell, &content);
            } else {
                io::write_str(2, b"sh: cannot open ");
                io::write_all(2, &cmd.args[1]);
                io::write_str(2, b"\n");
                shell.last_status = 1;
            }
        }
        return true;
    }

    if name == b"read" {
        if cmd.args.len() > 1 {
            let var_name = &cmd.args[1];
            let mut line = Vec::new();
            let mut buf = [0u8; 1];
            loop {
                let n = io::read(0, &mut buf);
                if n <= 0 || buf[0] == b'\n' {
                    break;
                }
                line.push(buf[0]);
            }
            shell.set_var(var_name, &line);
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"exec" {
        if cmd.args.len() > 1 {
            let mut cmd_exec = Command::new();
            for arg in &cmd.args[1..] {
                cmd_exec.args.push(arg.clone());
            }
            execute_command(&cmd_exec);
            shell.last_status = 127;
        }
        return true;
    }

    if name == b"set" {
        if cmd.args.len() > 1 && cmd.args[1] == b"-e" {
            // set -e: exit on error (just acknowledge, don't implement fully)
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"shift" {
        // shift [n] - shift positional parameters left by n (default 1)
        let n = if cmd.args.len() > 1 {
            let mut val = 0usize;
            for &c in cmd.args[1].iter() {
                if c >= b'0' && c <= b'9' {
                    val = val * 10 + (c - b'0') as usize;
                }
            }
            if val == 0 { 1 } else { val }
        } else {
            1
        };

        // $0 is at index 0 and is NOT shifted; we shift $1, $2, ...
        let param_count = shell.param_count();
        if n > param_count {
            shell.last_status = 1;
        } else {
            // Remove n items starting at index 1
            for _ in 0..n {
                if shell.positional_params.len() > 1 {
                    shell.positional_params.remove(1);
                }
            }
            shell.last_status = 0;
        }
        return true;
    }

    if name == b"return" {
        shell.last_status = if cmd.args.len() > 1 {
            sys::parse_i64(&cmd.args[1]).unwrap_or(0) as i32
        } else {
            0
        };
        return true;
    }

    if name == b"eval" {
        if cmd.args.len() > 1 {
            // Concatenate all args with spaces
            let mut line = Vec::new();
            for (i, arg) in cmd.args[1..].iter().enumerate() {
                if i > 0 {
                    line.push(b' ');
                }
                line.extend_from_slice(arg);
            }
            execute_script(shell, &line);
        }
        return true;
    }

    if name == b"alias" {
        if cmd.args.len() == 1 {
            // Print all aliases
            for (name, value) in &shell.aliases {
                io::write_all(1, name);
                io::write_str(1, b"='");
                io::write_all(1, value);
                io::write_str(1, b"'\n");
            }
        } else {
            for arg in &cmd.args[1..] {
                if let Some(eq_pos) = arg.iter().position(|&c| c == b'=') {
                    let alias_name = &arg[..eq_pos];
                    let alias_value = &arg[eq_pos + 1..];
                    shell
                        .aliases
                        .insert(alias_name.to_vec(), alias_value.to_vec());
                } else {
                    // Print specific alias
                    if let Some(value) = shell.aliases.get(arg.as_slice()) {
                        io::write_all(1, arg);
                        io::write_str(1, b"='");
                        io::write_all(1, value);
                        io::write_str(1, b"'\n");
                    } else {
                        io::write_str(2, b"sh: alias: ");
                        io::write_all(2, arg);
                        io::write_str(2, b": not found\n");
                        shell.last_status = 1;
                        return true;
                    }
                }
            }
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"unalias" {
        if cmd.args.len() > 1 {
            if cmd.args[1] == b"-a" {
                shell.aliases.clear();
            } else {
                for arg in &cmd.args[1..] {
                    shell.aliases.remove(arg.as_slice());
                }
            }
        }
        shell.last_status = 0;
        return true;
    }

    if name == b"type" {
        if cmd.args.len() > 1 {
            let target = &cmd.args[1];
            // Check builtins
            let builtins: &[&[u8]] = &[
                b"exit", b"cd", b"pwd", b"export", b"unset", b"echo", b"test", b"[", b"true",
                b"false", b":", b"source", b".", b"read", b"exec", b"set", b"shift", b"return",
                b"eval", b"alias", b"unalias", b"type",
            ];
            if builtins.iter().any(|b| *b == target.as_slice()) {
                io::write_all(1, target);
                io::write_str(1, b" is a shell builtin\n");
            } else if shell.aliases.contains_key(target.as_slice()) {
                io::write_all(1, target);
                io::write_str(1, b" is an alias for '");
                io::write_all(1, shell.aliases.get(target.as_slice()).unwrap());
                io::write_str(1, b"'\n");
            } else {
                io::write_all(1, target);
                io::write_str(1, b" is an external command\n");
            }
        }
        shell.last_status = 0;
        return true;
    }

    false
}

/// Execute test command (test/[)
///
/// Implements conditional expressions:
/// - Unary tests: -e, -f, -d, -r, -w, -x, -s, -n, -z, -L/-h
/// - Binary tests: =, ==, !=, -eq, -ne, -lt, -gt, -le, -ge
///
/// Returns 0 for true, 1 for false.
#[cfg(feature = "alloc")]
pub(super) fn execute_test(args: &[Vec<u8>]) -> i32 {
    if args.is_empty() {
        return 1;
    }

    // Remove trailing ] if present
    let args: Vec<&[u8]> = args
        .iter()
        .map(|a| a.as_slice())
        .filter(|a| *a != b"]")
        .collect();

    if args.is_empty() {
        return 1;
    }

    // Single arg: true if non-empty
    if args.len() == 1 {
        return if args[0].is_empty() { 1 } else { 0 };
    }

    // Two args: unary operators
    if args.len() == 2 {
        let op = args[0];
        let arg = args[1];

        if op == b"-n" {
            return if arg.is_empty() { 1 } else { 0 };
        }
        if op == b"-z" {
            return if arg.is_empty() { 0 } else { 1 };
        }
        if op == b"-e" || op == b"-a" {
            return if file_exists(arg) { 0 } else { 1 };
        }
        if op == b"-f" {
            return if is_regular_file(arg) { 0 } else { 1 };
        }
        if op == b"-d" {
            return if is_directory(arg) { 0 } else { 1 };
        }
        if op == b"-r" {
            return if is_readable(arg) { 0 } else { 1 };
        }
        if op == b"-w" {
            return if is_writable(arg) { 0 } else { 1 };
        }
        if op == b"-x" {
            return if is_executable(arg) { 0 } else { 1 };
        }
        if op == b"-s" {
            return if file_size(arg) > 0 { 0 } else { 1 };
        }
        if op == b"-L" || op == b"-h" {
            return if is_symlink(arg) { 0 } else { 1 };
        }
        if op == b"!" {
            return execute_test(&[arg.to_vec()]) ^ 1;
        }
    }

    // Three args: binary operators
    if args.len() == 3 {
        let left = args[0];
        let op = args[1];
        let right = args[2];

        if op == b"=" || op == b"==" {
            return if left == right { 0 } else { 1 };
        }
        if op == b"!=" {
            return if left != right { 0 } else { 1 };
        }

        // Numeric comparisons
        let ln = sys::parse_i64(left).unwrap_or(0);
        let rn = sys::parse_i64(right).unwrap_or(0);

        if op == b"-eq" {
            return if ln == rn { 0 } else { 1 };
        }
        if op == b"-ne" {
            return if ln != rn { 0 } else { 1 };
        }
        if op == b"-lt" {
            return if ln < rn { 0 } else { 1 };
        }
        if op == b"-le" {
            return if ln <= rn { 0 } else { 1 };
        }
        if op == b"-gt" {
            return if ln > rn { 0 } else { 1 };
        }
        if op == b"-ge" {
            return if ln >= rn { 0 } else { 1 };
        }
    }

    1
}
