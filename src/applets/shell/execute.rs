//! Shell command execution
//!
//! This module provides the core execution functions for the shell:
//! - Script execution (multiple statements)
//! - Statement dispatch (control flow vs simple commands)
//! - Pipeline handling (pipes, && and ||)
//! - Command execution (fork and exec)

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use crate::io;

#[cfg(feature = "alloc")]
use super::builtins::execute_builtin;
#[cfg(feature = "alloc")]
use super::control::{execute_case, execute_for, execute_if, execute_until, execute_while};
#[cfg(feature = "alloc")]
use super::expand::expand_string;
#[cfg(feature = "alloc")]
use super::parser::{Command, Token, parse_assignment_value, tokenize};
#[cfg(feature = "alloc")]
use super::state::Shell;
#[cfg(feature = "alloc")]
use super::util::{find_word_end, is_keyword_boundary, skip_whitespace_and_comments};

/// Execute a script (multiple lines/statements)
#[cfg(feature = "alloc")]
pub(super) fn execute_script(shell: &mut Shell, script: &[u8]) {
    let mut pos = 0;
    while pos < script.len() && !shell.should_exit {
        pos = execute_statement(shell, script, pos);
    }
}

/// Execute a single statement, return position after it
#[cfg(feature = "alloc")]
pub(super) fn execute_statement(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let pos = skip_whitespace_and_comments(script, start);
    if pos >= script.len() {
        return pos;
    }

    // Check for control structures (keyword must be at a word boundary)
    let word_end = find_word_end(script, pos);
    let first_word = &script[pos..word_end];
    let at_boundary = is_keyword_boundary(script, word_end);

    if at_boundary {
        if first_word == b"if" {
            return execute_if(shell, script, pos);
        }
        if first_word == b"while" {
            return execute_while(shell, script, pos);
        }
        if first_word == b"until" {
            return execute_until(shell, script, pos);
        }
        if first_word == b"for" {
            return execute_for(shell, script, pos);
        }
        if first_word == b"case" {
            return execute_case(shell, script, pos);
        }
    }

    // Regular command or pipeline
    execute_simple_line(shell, script, pos)
}

/// Execute a simple command line (no control structures)
#[cfg(feature = "alloc")]
pub(super) fn execute_simple_line(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let pos = start;

    // Find end of line/statement
    let mut end = pos;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while end < script.len() {
        let c = script[end];
        if in_single_quote {
            if c == b'\'' {
                in_single_quote = false;
            }
            end += 1;
        } else if in_double_quote {
            if c == b'"' {
                in_double_quote = false;
            } else if c == b'\\' && end + 1 < script.len() {
                end += 1;
            }
            end += 1;
        } else if c == b'\'' {
            in_single_quote = true;
            end += 1;
        } else if c == b'"' {
            in_double_quote = true;
            end += 1;
        } else if c == b'\n' || c == b';' {
            break;
        } else {
            end += 1;
        }
    }

    let line = &script[pos..end];
    if !line.trim_ascii().is_empty() {
        execute_line(shell, line);
    }

    if end < script.len() && (script[end] == b'\n' || script[end] == b';') {
        end + 1
    } else {
        end
    }
}

/// Execute a single command line (may contain pipes)
#[cfg(feature = "alloc")]
pub(super) fn execute_line(shell: &mut Shell, line: &[u8]) {
    let line = line.trim_ascii();
    if line.is_empty() || line[0] == b'#' {
        return;
    }

    // Check for variable assignment: VAR=value or VAR=value cmd...
    if let Some(eq_pos) = line.iter().position(|&c| c == b'=') {
        let before_eq = &line[..eq_pos];
        // Check if it's a valid variable name (starts with letter/underscore, contains only alnum/_)
        if !before_eq.is_empty()
            && (before_eq[0].is_ascii_alphabetic() || before_eq[0] == b'_')
            && before_eq
                .iter()
                .all(|&c| c.is_ascii_alphanumeric() || c == b'_')
        {
            // Find end of value (space or end of line)
            let after_eq = &line[eq_pos + 1..];
            let (value, rest) = parse_assignment_value(after_eq);
            let expanded_value = expand_string(shell, &value);

            if rest.trim_ascii().is_empty() {
                // Just assignment, no command
                shell.set_var(before_eq, &expanded_value);
                shell.last_status = 0;
                return;
            } else {
                // Assignment before command - set in environment for child
                shell.set_var(before_eq, &expanded_value);
                // Continue to execute rest as command
                execute_line(shell, rest.trim_ascii());
                return;
            }
        }
    }

    // Check for alias expansion on the first word
    let tokens = tokenize(shell, line);
    if tokens.is_empty() {
        return;
    }

    // If the first token is a word that matches an alias, expand it
    if let Some(Token::Word(first)) = tokens.first() {
        if let Some(alias_value) = shell.aliases.get(first.as_slice()) {
            // Build a new line with the alias value replacing the first word
            let mut new_line = alias_value.clone();
            // Append remaining tokens as-is from the original line
            // Find where the first word ends in the original line
            let mut skip = 0;
            while skip < line.len() && (line[skip] == b' ' || line[skip] == b'\t') {
                skip += 1;
            }
            // Skip past the first word
            while skip < line.len()
                && line[skip] != b' '
                && line[skip] != b'\t'
                && line[skip] != b'\n'
                && line[skip] != b';'
                && line[skip] != b'|'
                && line[skip] != b'&'
                && line[skip] != b'>'
                && line[skip] != b'<'
            {
                skip += 1;
            }
            // Append the rest of the original line
            if skip < line.len() {
                new_line.extend_from_slice(&line[skip..]);
            }
            execute_line(shell, &new_line);
            return;
        }
    }

    execute_pipeline(shell, &tokens);
}

/// Execute a pipeline with && and || logic
#[cfg(feature = "alloc")]
pub(super) fn execute_pipeline(shell: &mut Shell, tokens: &[Token]) {
    // Split into segments by && and ||
    let mut segments: Vec<(Vec<Token>, Option<Token>)> = Vec::new();
    let mut current_segment: Vec<Token> = Vec::new();

    for token in tokens {
        match token {
            Token::AndIf | Token::OrIf => {
                segments.push((current_segment, Some(token.clone())));
                current_segment = Vec::new();
            }
            _ => {
                current_segment.push(token.clone());
            }
        }
    }
    if !current_segment.is_empty() {
        segments.push((current_segment, None));
    }

    // Execute segments with && / || logic
    for (segment, connector) in segments {
        // Check if we should execute based on previous status and connector
        let should_execute = match connector {
            None => true, // Last segment, always execute
            _ => true,    // First segment or continuation
        };

        if !should_execute {
            continue;
        }

        execute_simple_pipeline(shell, &segment);

        // Check if we should continue to next segment
        if let Some(Token::AndIf) = connector {
            if shell.last_status != 0 {
                break; // && failed, stop
            }
        } else if let Some(Token::OrIf) = connector {
            if shell.last_status == 0 {
                break; // || succeeded, stop
            }
        }
    }
}

/// Execute a simple pipeline (no &&/||)
#[cfg(feature = "alloc")]
pub(super) fn execute_simple_pipeline(shell: &mut Shell, tokens: &[Token]) {
    let mut commands: Vec<Command> = Vec::new();
    let mut current = Command::new();

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(w) => {
                current.args.push(w.clone());
            }
            Token::Pipe => {
                if !current.args.is_empty() {
                    commands.push(current);
                    current = Command::new();
                }
            }
            Token::RedirectOut => {
                if i + 1 < tokens.len() {
                    if let Token::Word(f) = &tokens[i + 1] {
                        current.stdout_file = Some(f.clone());
                        current.stdout_append = false;
                        i += 1;
                    }
                }
            }
            Token::RedirectAppend => {
                if i + 1 < tokens.len() {
                    if let Token::Word(f) = &tokens[i + 1] {
                        current.stdout_file = Some(f.clone());
                        current.stdout_append = true;
                        i += 1;
                    }
                }
            }
            Token::RedirectIn => {
                if i + 1 < tokens.len() {
                    if let Token::Word(f) = &tokens[i + 1] {
                        current.stdin_file = Some(f.clone());
                        i += 1;
                    }
                }
            }
            Token::RedirectErr => {
                if i + 1 < tokens.len() {
                    if let Token::Word(f) = &tokens[i + 1] {
                        current.stderr_file = Some(f.clone());
                        i += 1;
                    }
                }
            }
            Token::Background => {
                current.background = true;
            }
            Token::AndIf | Token::OrIf => {
                // Should not happen in simple pipeline
            }
        }
        i += 1;
    }

    if !current.args.is_empty() {
        commands.push(current);
    }

    if commands.is_empty() {
        return;
    }

    // Single command - check for builtins
    if commands.len() == 1 && !commands[0].background {
        let cmd = &commands[0];
        let has_redirects =
            cmd.stdout_file.is_some() || cmd.stdin_file.is_some() || cmd.stderr_file.is_some();

        if has_redirects {
            // Set up redirections for the builtin, then restore
            let saved_stdout = if cmd.stdout_file.is_some() {
                io::dup(1)
            } else {
                -1
            };
            let saved_stdin = if cmd.stdin_file.is_some() {
                io::dup(0)
            } else {
                -1
            };
            let saved_stderr = if cmd.stderr_file.is_some() {
                io::dup(2)
            } else {
                -1
            };

            let mut redirect_ok = true;

            if let Some(ref f) = cmd.stdin_file {
                let fd = io::open(f, libc::O_RDONLY, 0);
                if fd < 0 {
                    io::write_str(2, b"sh: ");
                    io::write_all(2, f);
                    io::write_str(2, b": No such file or directory\n");
                    redirect_ok = false;
                    shell.last_status = 1;
                } else {
                    io::dup2(fd, 0);
                    io::close(fd);
                }
            }

            if redirect_ok {
                if let Some(ref f) = cmd.stdout_file {
                    let flags = if cmd.stdout_append {
                        libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND
                    } else {
                        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
                    };
                    let fd = io::open(f, flags, 0o644);
                    if fd < 0 {
                        io::write_str(2, b"sh: cannot create ");
                        io::write_all(2, f);
                        io::write_str(2, b"\n");
                        redirect_ok = false;
                        shell.last_status = 1;
                    } else {
                        io::dup2(fd, 1);
                        io::close(fd);
                    }
                }
            }

            if redirect_ok {
                if let Some(ref f) = cmd.stderr_file {
                    let fd = io::open(f, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
                    if fd >= 0 {
                        io::dup2(fd, 2);
                        io::close(fd);
                    }
                }
            }

            if redirect_ok {
                execute_builtin(shell, cmd);
            }

            // Restore original fds
            if saved_stdout >= 0 {
                io::dup2(saved_stdout, 1);
                io::close(saved_stdout);
            }
            if saved_stdin >= 0 {
                io::dup2(saved_stdin, 0);
                io::close(saved_stdin);
            }
            if saved_stderr >= 0 {
                io::dup2(saved_stderr, 2);
                io::close(saved_stderr);
            }

            return;
        }

        if execute_builtin(shell, cmd) {
            return;
        }
    }

    // Execute pipeline with fork
    let n = commands.len();
    let mut prev_pipe_read: i32 = -1;
    let mut pids: Vec<i32> = Vec::new();

    for (i, cmd) in commands.iter().enumerate() {
        let is_last = i == n - 1;

        let mut pipe_fds = [-1i32; 2];
        if !is_last {
            unsafe {
                if libc::pipe(pipe_fds.as_mut_ptr()) < 0 {
                    io::write_str(2, b"sh: pipe failed\n");
                    return;
                }
            }
        }

        let pid = io::fork();
        if pid < 0 {
            io::write_str(2, b"sh: fork failed\n");
            return;
        }

        if pid == 0 {
            // Child
            if prev_pipe_read >= 0 {
                io::dup2(prev_pipe_read, 0);
                io::close(prev_pipe_read);
            }

            if !is_last {
                io::close(pipe_fds[0]);
                io::dup2(pipe_fds[1], 1);
                io::close(pipe_fds[1]);
            }

            // Handle redirections
            if let Some(ref f) = cmd.stdin_file {
                let fd = io::open(f, libc::O_RDONLY, 0);
                if fd < 0 {
                    io::write_str(2, f);
                    io::write_str(2, b": No such file or directory\n");
                    io::exit(1);
                }
                io::dup2(fd, 0);
                io::close(fd);
            }

            if let Some(ref f) = cmd.stdout_file {
                let flags = if cmd.stdout_append {
                    libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND
                } else {
                    libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC
                };
                let fd = io::open(f, flags, 0o644);
                if fd < 0 {
                    io::write_str(2, b"sh: cannot create ");
                    io::write_all(2, f);
                    io::write_str(2, b"\n");
                    io::exit(1);
                }
                io::dup2(fd, 1);
                io::close(fd);
            }

            if let Some(ref f) = cmd.stderr_file {
                let fd = io::open(f, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
                if fd >= 0 {
                    io::dup2(fd, 2);
                    io::close(fd);
                }
            }

            // Set local variables in environment
            for (k, v) in &shell.variables {
                let mut name_buf = [0u8; 256];
                let mut value_buf = [0u8; 4096];
                let nlen = core::cmp::min(k.len(), name_buf.len() - 1);
                let vlen = core::cmp::min(v.len(), value_buf.len() - 1);
                name_buf[..nlen].copy_from_slice(&k[..nlen]);
                value_buf[..vlen].copy_from_slice(&v[..vlen]);
                unsafe {
                    libc::setenv(
                        name_buf.as_ptr() as *const libc::c_char,
                        value_buf.as_ptr() as *const libc::c_char,
                        1,
                    );
                }
            }

            execute_command(cmd);
            io::exit(127);
        }

        pids.push(pid);

        if prev_pipe_read >= 0 {
            io::close(prev_pipe_read);
        }

        if !is_last {
            io::close(pipe_fds[1]);
            prev_pipe_read = pipe_fds[0];
        }
    }

    // Wait
    let background = commands.last().map(|c| c.background).unwrap_or(false);
    if !background {
        for pid in pids {
            let mut status: i32 = 0;
            io::waitpid(pid, &mut status, 0);
            shell.last_status = (status >> 8) & 0xff;
        }
    }
}

/// Execute a command (in child process)
#[cfg(feature = "alloc")]
pub(super) fn execute_command(cmd: &Command) {
    if cmd.args.is_empty() {
        return;
    }

    let mut argv_ptrs: Vec<*const libc::c_char> = Vec::new();
    let mut argv_storage: Vec<Vec<u8>> = Vec::new();

    for arg in &cmd.args {
        let mut s = arg.clone();
        s.push(0);
        argv_storage.push(s);
    }

    for s in &argv_storage {
        argv_ptrs.push(s.as_ptr() as *const libc::c_char);
    }
    argv_ptrs.push(core::ptr::null());

    unsafe {
        libc::execvp(argv_ptrs[0], argv_ptrs.as_ptr());
    }

    io::write_str(2, b"sh: ");
    io::write_all(2, &cmd.args[0]);
    io::write_str(2, b": command not found\n");
}
