//! Variable expansion for shell
//!
//! This module handles shell variable expansion including:
//! - Simple variable expansion: `$VAR` and `${VAR}`
//! - Special variables: `$?` (last exit status) and `$$` (process ID)
//! - Command substitution: `$(cmd)`
//! - Arithmetic expansion: `$((expr))`
//! - Word splitting for expanded values

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use super::arithmetic::eval_arithmetic;
#[cfg(feature = "alloc")]
use super::state::Shell;
use crate::io;

#[cfg(feature = "alloc")]
use super::util::{format_number, format_signed};

/// Expand a single $ expression starting at the given position
///
/// Handles:
/// - `$VAR` - simple variable
/// - `${VAR}` - braced variable
/// - `$?` - last exit status
/// - `$$` - current process ID
/// - `$(cmd)` - command substitution
/// - `$((expr))` - arithmetic expansion
///
/// Returns the expanded bytes and the new position after the expansion.
#[cfg(feature = "alloc")]
pub(super) fn expand_dollar(shell: &Shell, input: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut pos = start + 1; // skip $

    if pos >= input.len() {
        return (b"$".to_vec(), pos);
    }

    let c = input[pos];

    // $? — last exit status
    if c == b'?' {
        let mut buf = [0u8; 16];
        let s = format_number(shell.last_status as u64, &mut buf);
        return (s.to_vec(), pos + 1);
    }

    // $$ — current PID
    if c == b'$' {
        let pid = unsafe { libc::getpid() };
        let mut buf = [0u8; 16];
        let s = format_number(pid as u64, &mut buf);
        return (s.to_vec(), pos + 1);
    }

    // $# — number of positional parameters
    if c == b'#' {
        let mut buf = [0u8; 16];
        let s = format_number(shell.param_count() as u64, &mut buf);
        return (s.to_vec(), pos + 1);
    }

    // $0-$9 — positional parameters
    if c >= b'0' && c <= b'9' {
        let idx = (c - b'0') as usize;
        let value = shell.get_positional(idx).unwrap_or(b"");
        return (value.to_vec(), pos + 1);
    }

    // $@ — all positional parameters as separate words
    if c == b'@' {
        let mut result = Vec::new();
        let count = shell.param_count();
        for i in 1..=count {
            if i > 1 {
                result.push(b' ');
            }
            if let Some(p) = shell.get_positional(i) {
                result.extend_from_slice(p);
            }
        }
        return (result, pos + 1);
    }

    // $* — all positional parameters as a single string
    if c == b'*' {
        let mut result = Vec::new();
        let count = shell.param_count();
        for i in 1..=count {
            if i > 1 {
                result.push(b' ');
            }
            if let Some(p) = shell.get_positional(i) {
                result.extend_from_slice(p);
            }
        }
        return (result, pos + 1);
    }

    // $((arithmetic))
    if c == b'(' && pos + 1 < input.len() && input[pos + 1] == b'(' {
        pos += 2;
        let start_expr = pos;
        let mut depth = 2;
        while pos < input.len() && depth > 0 {
            if input[pos] == b'(' {
                depth += 1;
            } else if input[pos] == b')' {
                depth -= 1;
            }
            pos += 1;
        }
        let expr = &input[start_expr..pos - 2];
        let result = eval_arithmetic(shell, expr);
        let mut buf = [0u8; 20];
        let s = format_signed(result, &mut buf);
        return (s.to_vec(), pos);
    }

    // $(command)
    if c == b'(' {
        pos += 1;
        let start_cmd = pos;
        let mut depth = 1;
        while pos < input.len() && depth > 0 {
            if input[pos] == b'(' {
                depth += 1;
            } else if input[pos] == b')' {
                depth -= 1;
            }
            pos += 1;
        }
        let cmd = &input[start_cmd..pos - 1];
        let output = execute_capture(shell, cmd);
        // Trim trailing newlines
        let output = output.trim_ascii_end().to_vec();
        return (output, pos);
    }

    // ${VAR} or ${VAR:-default} or $VAR
    let mut var_name = Vec::new();
    if c == b'{' {
        pos += 1;
        // Read variable name until we hit }, :, -, +, =, ?, or #, %
        // Also handle positional: ${1}, ${10}, ${#}, ${@}, ${*}
        let brace_start = pos;

        // Special: ${#VAR} for string length
        let length_op = pos < input.len()
            && input[pos] == b'#'
            && pos + 1 < input.len()
            && input[pos + 1] != b'}';

        if length_op {
            pos += 1; // skip #
        }

        // Read variable name
        while pos < input.len()
            && input[pos] != b'}'
            && input[pos] != b':'
            && input[pos] != b'-'
            && input[pos] != b'+'
            && input[pos] != b'='
            && !(input[pos] == b'?' && !var_name.is_empty())
        {
            var_name.push(input[pos]);
            pos += 1;
        }

        // Check for operator (:-  :+  :=  :?  -  +  =  ?)
        let mut op: u8 = 0;
        let mut colon_variant = false;
        if pos < input.len() && input[pos] != b'}' {
            if input[pos] == b':' {
                colon_variant = true;
                pos += 1;
                if pos < input.len() && input[pos] != b'}' {
                    op = input[pos];
                    pos += 1;
                }
            } else {
                op = input[pos];
                pos += 1;
            }

            // Read the operand (everything until matching })
            let mut operand = Vec::new();
            let mut depth = 1;
            while pos < input.len() && depth > 0 {
                if input[pos] == b'{' {
                    depth += 1;
                } else if input[pos] == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                operand.push(input[pos]);
                pos += 1;
            }
            // Skip closing }
            if pos < input.len() && input[pos] == b'}' {
                pos += 1;
            }

            // Expand the operand
            let expanded_operand = expand_string(shell, &operand);

            // Look up the variable value
            let value = lookup_var(shell, &var_name);

            // Check "unset or null" (colon variant) vs just "unset"
            let is_unset_or_null = match &value {
                None => true,
                Some(v) if colon_variant && v.is_empty() => true,
                _ => false,
            };

            if length_op {
                // ${#VAR} — length of value
                let len = value.map(|v| v.len()).unwrap_or(0);
                let mut buf = [0u8; 16];
                let s = format_number(len as u64, &mut buf);
                return (s.to_vec(), pos);
            }

            match op {
                b'-' => {
                    // ${VAR:-default} / ${VAR-default}
                    if is_unset_or_null {
                        return (expanded_operand, pos);
                    } else {
                        return (value.unwrap_or_default(), pos);
                    }
                }
                b'+' => {
                    // ${VAR:+alt} / ${VAR+alt}
                    if is_unset_or_null {
                        return (Vec::new(), pos);
                    } else {
                        return (expanded_operand, pos);
                    }
                }
                b'=' => {
                    // ${VAR:=default} / ${VAR=default}
                    if is_unset_or_null {
                        // Can't modify shell here since we only have &Shell
                        // Return the default value (assignment side-effect not supported in expand_dollar)
                        return (expanded_operand, pos);
                    } else {
                        return (value.unwrap_or_default(), pos);
                    }
                }
                b'?'
                    // ${VAR:?msg} / ${VAR?msg}
                    if is_unset_or_null => {
                        io::write_str(2, b"sh: ");
                        io::write_all(2, &var_name);
                        io::write_str(2, b": ");
                        if expanded_operand.is_empty() {
                            io::write_str(2, b"parameter not set\n");
                        } else {
                            io::write_all(2, &expanded_operand);
                            io::write_str(2, b"\n");
                        }
                        return (Vec::new(), pos);
                    }
                _ => {
                    // Unknown operator, just return value
                    return (value.unwrap_or_default(), pos);
                }
            }
        } else {
            // No operator — simple ${VAR} or ${#VAR}
            // Skip closing }
            if pos < input.len() && input[pos] == b'}' {
                pos += 1;
            }

            if length_op {
                let value = lookup_var(shell, &var_name);
                let len = value.map(|v| v.len()).unwrap_or(0);
                let mut buf = [0u8; 16];
                let s = format_number(len as u64, &mut buf);
                return (s.to_vec(), pos);
            }
        }
    } else {
        while pos < input.len() && (input[pos].is_ascii_alphanumeric() || input[pos] == b'_') {
            var_name.push(input[pos]);
            pos += 1;
        }
    }

    if var_name.is_empty() {
        return (b"$".to_vec(), start + 1);
    }

    // Look up variable (includes positional params for numeric names)
    match lookup_var(shell, &var_name) {
        Some(v) => (v, pos),
        None => (Vec::new(), pos),
    }
}

/// Look up a variable or positional parameter by name.
///
/// Handles:
/// - Numeric names ("0", "1", ...) as positional parameters
/// - "#" as param count, "@" and "*" as all params
/// - Regular variable names (shell locals, then environment)
#[cfg(feature = "alloc")]
fn lookup_var(shell: &Shell, name: &[u8]) -> Option<Vec<u8>> {
    // Positional: single digit
    if name.len() == 1 && name[0] >= b'0' && name[0] <= b'9' {
        let idx = (name[0] - b'0') as usize;
        return shell.get_positional(idx).map(|v| v.to_vec());
    }

    // Multi-digit positional: "10", "11", etc.
    if !name.is_empty() && name.iter().all(|&c| c >= b'0' && c <= b'9') {
        let idx = parse_usize(name);
        return shell.get_positional(idx).map(|v| v.to_vec());
    }

    // Special: # @ *
    if name == b"#" {
        let mut buf = [0u8; 16];
        let s = format_number(shell.param_count() as u64, &mut buf);
        return Some(s.to_vec());
    }
    if name == b"@" || name == b"*" {
        let mut result = Vec::new();
        let count = shell.param_count();
        for i in 1..=count {
            if i > 1 {
                result.push(b' ');
            }
            if let Some(p) = shell.get_positional(i) {
                result.extend_from_slice(p);
            }
        }
        return Some(result);
    }
    if name == b"?" {
        let mut buf = [0u8; 16];
        let s = format_number(shell.last_status as u64, &mut buf);
        return Some(s.to_vec());
    }

    // Shell local variable
    if let Some(value) = shell.get_var(name) {
        return Some(value.to_vec());
    }

    // Environment variable
    if let Some(value) = io::getenv(name) {
        return Some(value.to_vec());
    }

    None
}

/// Parse a byte slice as a usize
#[cfg(feature = "alloc")]
fn parse_usize(s: &[u8]) -> usize {
    let mut n: usize = 0;
    for &c in s {
        if c >= b'0' && c <= b'9' {
            n = n.wrapping_mul(10).wrapping_add((c - b'0') as usize);
        }
    }
    n
}

/// Expand all $ expressions in a string
///
/// Iterates through the input and expands each `$` using `expand_dollar`.
#[cfg(feature = "alloc")]
pub(super) fn expand_string(shell: &Shell, input: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        if input[pos] == b'$' {
            let (expanded, new_pos) = expand_dollar(shell, input, pos);
            result.extend_from_slice(&expanded);
            pos = new_pos;
        } else {
            result.push(input[pos]);
            pos += 1;
        }
    }

    result
}

/// Execute a command and capture its output
///
/// Used for command substitution `$(cmd)`. Forks a subshell,
/// captures stdout via a pipe, and returns the output bytes.
#[cfg(feature = "alloc")]
pub(super) fn execute_capture(shell: &Shell, cmd: &[u8]) -> Vec<u8> {
    // Create pipe
    let mut pipe_fds = [-1i32; 2];
    unsafe {
        if libc::pipe(pipe_fds.as_mut_ptr()) < 0 {
            return Vec::new();
        }
    }

    let pid = io::fork();
    if pid == 0 {
        // Child
        io::close(pipe_fds[0]);
        io::dup2(pipe_fds[1], 1);
        io::close(pipe_fds[1]);

        // Execute in subshell
        let mut subshell = Shell::new(false);
        // Copy variables
        for (k, v) in &shell.variables {
            subshell.variables.insert(k.clone(), v.clone());
        }
        // Copy positional parameters
        subshell.positional_params = shell.positional_params.clone();
        super::execute_script(&mut subshell, cmd);
        io::exit(subshell.last_status);
    }

    // Parent
    io::close(pipe_fds[1]);

    let mut output = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(pipe_fds[0], &mut buf);
        if n <= 0 {
            break;
        }
        output.extend_from_slice(&buf[..n as usize]);
    }
    io::close(pipe_fds[0]);

    let mut status = 0;
    io::waitpid(pid, &mut status, 0);

    output
}

/// Split a string into words
///
/// Splits the input on whitespace, respecting shell quoting rules.
/// Uses `skip_whitespace_and_comments` and `parse_word` from the parent module.
#[cfg(feature = "alloc")]
pub(super) fn split_words(shell: &Shell, s: &[u8]) -> Vec<Vec<u8>> {
    let mut words = Vec::new();
    let mut pos = 0;

    while pos < s.len() {
        pos = super::skip_whitespace_and_comments(s, pos);
        if pos >= s.len() {
            break;
        }

        let (word, new_pos) = super::parse_word(shell, s, pos);
        if !word.is_empty() {
            words.push(word);
        }
        pos = new_pos;
    }

    words
}
