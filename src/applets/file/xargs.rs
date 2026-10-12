//! xargs - build and execute command lines from standard input
//!
//! GNU coreutils compatible implementation.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::ffi::CString;
use alloc::vec::Vec;

/// xargs - build and execute command lines from standard input
///
/// # Synopsis
/// ```text
/// xargs [OPTIONS] [COMMAND [INITIAL-ARGS]]
/// ```
///
/// # Description
/// Read items from the standard input and execute a command with
/// those items as arguments.
///
/// # Options
/// - `-0, --null`: Input items are separated by a null character
/// - `-d DELIM`: Input items are separated by DELIM character
/// - `-I REPLSTR`: Replace REPLSTR with input items in arguments
/// - `-n MAXARGS`: Use at most MAXARGS arguments per command line
/// - `-P MAXPROCS`: Run up to MAXPROCS processes at a time
/// - `-r, --no-run-if-empty`: Don't run command if input is empty
/// - `-t, --verbose`: Print command before executing
/// - `-x`: Exit if command line length exceeds limit
///
/// # Exit Status
/// - 0: All commands succeeded
/// - 123: Any command returned 1-125
/// - 124: Command exited with status 255
/// - 125: Command was killed by a signal
/// - 126: Command cannot be run
/// - 127: Command not found
/// - 1: Other error
pub fn xargs(argc: i32, argv: *const *const u8) -> i32 {
    let mut null_delimiter = false;
    let mut delimiter = b'\n';
    let mut explicit_delim = false;
    let mut replace_str: Option<&[u8]> = None;
    let mut max_args: Option<usize> = None;
    let mut max_procs: usize = 1;
    let mut no_run_if_empty = false;
    let mut verbose = false;
    let mut exit_on_limit = false;
    let mut cmd_start = 1;

    // Parse options
    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-0" || arg == b"--null" {
                null_delimiter = true;
                delimiter = 0;
                cmd_start = i + 1;
            } else if arg == b"-d" {
                i += 1;
                explicit_delim = true;
                if let Some(d) = unsafe { get_arg(argv, i) } {
                    if !d.is_empty() {
                        delimiter = d[0];
                    }
                }
                cmd_start = i + 1;
            } else if arg == b"-p" || arg == b"--interactive" {
                // Interactive prompting is not implemented; accept and ignore.
                cmd_start = i + 1;
            } else if arg == b"-I" || arg == b"-i" {
                i += 1;
                let repl = unsafe { get_arg(argv, i) };
                if repl.map(|r| r.is_empty()).unwrap_or(true) {
                    io::write_str(2, b"xargs: replstr may not be empty\n");
                    return 2;
                }
                replace_str = repl;
                cmd_start = i + 1;
            } else if arg.starts_with(b"-I") && arg.len() > 2 {
                // Handle -I{} format (no space)
                replace_str = Some(&arg[2..]);
                cmd_start = i + 1;
            } else if arg == b"-n" {
                i += 1;
                if let Some(n) = unsafe { get_arg(argv, i) } {
                    max_args = sys::parse_u64(n).map(|v| v as usize);
                }
                cmd_start = i + 1;
            } else if arg.starts_with(b"-n") && arg.len() > 2 {
                // Handle -nN format (no space)
                max_args = sys::parse_u64(&arg[2..]).map(|v| v as usize);
                cmd_start = i + 1;
            } else if arg == b"-P" {
                i += 1;
                if let Some(p) = unsafe { get_arg(argv, i) } {
                    match sys::parse_u64(p) {
                        Some(v) => max_procs = v as usize,
                        None => {
                            io::write_str(2, b"xargs: invalid number for -P\n");
                            return 2;
                        }
                    }
                }
                cmd_start = i + 1;
            } else if arg.starts_with(b"-P") && arg.len() > 2 {
                // Handle -PN format (no space)
                match sys::parse_u64(&arg[2..]) {
                    Some(v) => max_procs = v as usize,
                    None => {
                        io::write_str(2, b"xargs: invalid number for -P\n");
                        return 2;
                    }
                }
                cmd_start = i + 1;
            } else if arg == b"-r" || arg == b"--no-run-if-empty" {
                no_run_if_empty = true;
                cmd_start = i + 1;
            } else if arg == b"-t" || arg == b"--verbose" {
                verbose = true;
                cmd_start = i + 1;
            } else if arg == b"-x" {
                exit_on_limit = true;
                cmd_start = i + 1;
            } else if arg == b"-h" || arg == b"--help" {
                print_help();
                return 0;
            } else if arg == b"--" {
                cmd_start = i + 1;
                break;
            } else if !arg.starts_with(b"-") {
                cmd_start = i;
                break;
            } else {
                // Check for combined short options like -r0
                let mut valid = true;
                for &c in &arg[1..] {
                    match c {
                        b'0' => {
                            null_delimiter = true;
                            delimiter = 0;
                        }
                        b'r' => no_run_if_empty = true,
                        b't' => verbose = true,
                        b'x' => exit_on_limit = true,
                        _ => {
                            valid = false;
                            break;
                        }
                    }
                }
                if valid {
                    cmd_start = i + 1;
                } else {
                    cmd_start = i;
                    break;
                }
            }
        }
        i += 1;
    }

    // Get command and initial args
    let command = if cmd_start < argc {
        unsafe { get_arg(argv, cmd_start).unwrap_or(b"echo") }
    } else {
        b"echo"
    };

    let mut initial_args: Vec<&[u8]> = Vec::new();
    for j in (cmd_start + 1)..argc {
        if let Some(arg) = unsafe { get_arg(argv, j) } {
            initial_args.push(arg);
        }
    }

    // Read input
    let input = io::read_all(0);

    // Split input into items.
    //
    // - `-0` / `-d`: split raw on the given byte, no quote processing.
    // - `-I`/`-i` (replace mode): each line is a single item (POSIX/GNU
    //   xargs treats -I as implying line-at-a-time, unsplit, input).
    // - default: split on blanks (spaces, tabs, newlines) honoring shell
    //   quoting (', ", and backslash), matching the POSIX xargs default.
    let items: Vec<Vec<u8>> = if null_delimiter {
        input
            .split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_vec())
            .collect()
    } else if explicit_delim {
        input
            .split(|&c| c == delimiter)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_vec())
            .collect()
    } else if replace_str.is_some() {
        split_lines_trimmed(&input)
    } else {
        split_whitespace_quoted(&input)
    };

    // Handle empty input
    if items.is_empty() {
        if no_run_if_empty {
            return 0;
        }
        // With no input and no -r, xargs still runs command once with initial args
        if replace_str.is_none() && max_args.is_none() {
            return run_command(command, &initial_args, verbose);
        }
        return 0;
    }

    // Track exit status
    let mut exit_status = 0;
    let mut running_pids: Vec<libc::pid_t> = Vec::new();

    if let Some(repl) = replace_str {
        // Replace mode: run command once per item, replacing REPLSTR in args
        for item in &items {
            // Wait if we've reached max parallel processes
            while running_pids.len() >= max_procs && max_procs > 0 {
                let status = wait_for_any(&mut running_pids);
                exit_status = update_exit_status(exit_status, status);
            }

            let args = build_replace_args(&initial_args, repl, item);
            let args_refs: Vec<&[u8]> = args.iter().map(|v| v.as_slice()).collect();

            if max_procs > 1 {
                // Run in parallel
                if let Some(pid) = fork_and_run(command, &args_refs, verbose) {
                    running_pids.push(pid);
                }
            } else {
                let status = run_command(command, &args_refs, verbose);
                exit_status = update_exit_status(exit_status, status);
            }
        }
    } else {
        // Normal mode: batch items into command invocations, respecting
        // -n (max args per invocation) and an ARG_MAX-derived byte budget.
        let max_bytes = arg_max_budget(command, &initial_args);
        let batches = compute_batches(&items, max_args, max_bytes);

        for (start, end) in batches {
            let batch_len = end - start;
            // A single oversized item that can't fit within max_bytes on its own.
            let is_single_oversized =
                batch_len == 1 && items[start].len() + 1 > max_bytes && max_args.is_none();
            // A `-n`-requested batch that had to be shrunk below the requested
            // count because it would otherwise exceed max_bytes (there is more
            // input still to come, so this wasn't just running out of items).
            let is_shrunk_n_batch = match max_args {
                Some(m) => batch_len < m && end < items.len(),
                None => false,
            };
            if exit_on_limit && (is_single_oversized || is_shrunk_n_batch) {
                io::write_str(2, b"xargs: argument line too long\n");
                exit_status = 1;
                break;
            }

            // Wait if we've reached max parallel processes
            while running_pids.len() >= max_procs && max_procs > 0 {
                let status = wait_for_any(&mut running_pids);
                exit_status = update_exit_status(exit_status, status);
            }

            let mut args = initial_args.clone();
            args.extend(items[start..end].iter().map(|v| v.as_slice()));

            if max_procs > 1 {
                // Run in parallel
                if let Some(pid) = fork_and_run(command, &args, verbose) {
                    running_pids.push(pid);
                }
            } else {
                let status = run_command(command, &args, verbose);
                exit_status = update_exit_status(exit_status, status);
            }
        }
    }

    // Wait for remaining processes
    while !running_pids.is_empty() {
        let status = wait_for_any(&mut running_pids);
        exit_status = update_exit_status(exit_status, status);
    }

    exit_status
}

/// Split input on unquoted blanks (space, tab, newline), the POSIX xargs
/// default. Single quotes, double quotes, and backslash escapes are
/// honored so that quoted whitespace does not split an item.
fn split_whitespace_quoted(input: &[u8]) -> Vec<Vec<u8>> {
    let mut items: Vec<Vec<u8>> = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut in_word = false;
    let n = input.len();
    let mut i = 0;

    while i < n {
        let c = input[i];
        match c {
            b' ' | b'\t' | b'\n' => {
                if in_word {
                    items.push(core::mem::take(&mut cur));
                    in_word = false;
                }
                i += 1;
            }
            b'\'' => {
                in_word = true;
                i += 1;
                while i < n && input[i] != b'\'' {
                    cur.push(input[i]);
                    i += 1;
                }
                i += 1;
            }
            b'"' => {
                in_word = true;
                i += 1;
                while i < n && input[i] != b'"' {
                    if input[i] == b'\\'
                        && i + 1 < n
                        && (input[i + 1] == b'"' || input[i + 1] == b'\\')
                    {
                        cur.push(input[i + 1]);
                        i += 2;
                    } else {
                        cur.push(input[i]);
                        i += 1;
                    }
                }
                i += 1;
            }
            b'\\' => {
                in_word = true;
                i += 1;
                if i < n {
                    cur.push(input[i]);
                    i += 1;
                }
            }
            _ => {
                in_word = true;
                cur.push(c);
                i += 1;
            }
        }
    }

    if in_word {
        items.push(cur);
    }

    items
}

/// Split input into lines (POSIX xargs `-I` mode: each line is a single,
/// unsplit item), trimming leading/trailing blanks and dropping empty lines.
fn split_lines_trimmed(input: &[u8]) -> Vec<Vec<u8>> {
    input
        .split(|&c| c == b'\n')
        .map(|line| {
            let start = line.iter().position(|&c| c != b' ' && c != b'\t');
            match start {
                Some(start) => {
                    let end = line.iter().rposition(|&c| c != b' ' && c != b'\t').unwrap();
                    line[start..=end].to_vec()
                }
                None => Vec::new(),
            }
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// Estimate an available byte budget for one command invocation's
/// argument list, derived from the system ARG_MAX, minus the space
/// already consumed by the command name and its initial arguments.
fn arg_max_budget(command: &[u8], initial_args: &[&[u8]]) -> usize {
    let sys_max = unsafe { libc::sysconf(libc::_SC_ARG_MAX) };
    let arg_max: usize = if sys_max > 0 {
        sys_max as usize
    } else {
        128 * 1024
    };

    // Leave headroom for environment and the command/initial args already
    // fixed in every invocation.
    let mut used = command.len() + 1;
    for a in initial_args {
        used += a.len() + 1;
    }

    let headroom = arg_max / 4; // conservative safety margin for env, etc.
    arg_max.saturating_sub(used).saturating_sub(headroom).max(1)
}

/// Group items into batches honoring both `-n` (max item count) and a
/// byte-length budget (ARG_MAX-derived). Always makes progress: a single
/// oversized item still forms its own batch.
fn compute_batches(
    items: &[Vec<u8>],
    max_args: Option<usize>,
    max_bytes: usize,
) -> Vec<(usize, usize)> {
    let mut batches = Vec::new();
    let len = items.len();
    let mut start = 0;

    while start < len {
        let mut end = start;
        let mut bytes = 0usize;
        let mut count = 0usize;

        while end < len {
            let item_bytes = items[end].len() + 1;
            if count > 0 {
                if let Some(m) = max_args {
                    if count >= m {
                        break;
                    }
                }
                if bytes + item_bytes > max_bytes {
                    break;
                }
            }
            bytes += item_bytes;
            count += 1;
            end += 1;
        }

        if end == start {
            end = start + 1;
        }

        batches.push((start, end));
        start = end;
    }

    batches
}

/// Build args with replacement. GNU xargs replaces every occurrence of
/// REPLSTR within each argument, not just the first.
fn build_replace_args(args: &[&[u8]], repl: &[u8], item: &[u8]) -> Vec<Vec<u8>> {
    args.iter()
        .map(|arg| replace_all_subsequences(arg, repl, item))
        .collect()
}

/// Replace every non-overlapping occurrence of `repl` in `arg` with `item`.
fn replace_all_subsequences(arg: &[u8], repl: &[u8], item: &[u8]) -> Vec<u8> {
    if repl.is_empty() {
        return arg.to_vec();
    }
    let mut out = Vec::new();
    let mut rest = arg;
    while let Some(pos) = find_subsequence(rest, repl) {
        out.extend_from_slice(&rest[..pos]);
        out.extend_from_slice(item);
        rest = &rest[pos + repl.len()..];
    }
    out.extend_from_slice(rest);
    out
}

/// Find subsequence in slice
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Run a command synchronously
fn run_command(cmd: &[u8], args: &[&[u8]], verbose: bool) -> i32 {
    if verbose {
        print_command(cmd, args);
    }

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        io::write_str(2, b"xargs: fork failed\n");
        return 1;
    }

    if pid == 0 {
        exec_command(cmd, args);
    }

    // Parent - wait for child
    let mut status: libc::c_int = 0;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }

    get_exit_code(status)
}

/// Fork and run command, returning PID
fn fork_and_run(cmd: &[u8], args: &[&[u8]], verbose: bool) -> Option<libc::pid_t> {
    if verbose {
        print_command(cmd, args);
    }

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        io::write_str(2, b"xargs: fork failed\n");
        return None;
    }

    if pid == 0 {
        exec_command(cmd, args);
    }

    Some(pid)
}

/// Exec a command (in child process)
fn exec_command(cmd: &[u8], args: &[&[u8]]) -> ! {
    let mut cstrings: Vec<CString> = Vec::new();

    // Command
    let mut v = Vec::with_capacity(cmd.len() + 1);
    v.extend_from_slice(cmd);
    v.push(0);
    if let Ok(cs) = CString::from_vec_with_nul(v) {
        cstrings.push(cs);
    }

    // Args
    for arg in args {
        let mut v = Vec::with_capacity(arg.len() + 1);
        v.extend_from_slice(arg);
        v.push(0);
        if let Ok(cs) = CString::from_vec_with_nul(v) {
            cstrings.push(cs);
        }
    }

    let ptrs: Vec<*const libc::c_char> = cstrings
        .iter()
        .map(|s| s.as_ptr())
        .chain(core::iter::once(core::ptr::null()))
        .collect();

    unsafe {
        libc::execvp(ptrs[0], ptrs.as_ptr());
    }

    // exec failed
    io::write_str(2, b"xargs: ");
    io::write_all(2, cmd);
    io::write_str(2, b": ");

    let errno = crate::sys::errno();
    if errno == libc::ENOENT {
        io::write_str(2, b"command not found\n");
        unsafe {
            libc::_exit(127);
        }
    } else if errno == libc::EACCES {
        io::write_str(2, b"permission denied\n");
        unsafe {
            libc::_exit(126);
        }
    } else {
        io::write_str(2, b"cannot run command\n");
        unsafe {
            libc::_exit(126);
        }
    }
}

/// Wait for any child process
fn wait_for_any(pids: &mut Vec<libc::pid_t>) -> i32 {
    let mut status: libc::c_int = 0;
    let pid = unsafe { libc::wait(&mut status) };

    if pid > 0 {
        if let Some(idx) = pids.iter().position(|&p| p == pid) {
            pids.remove(idx);
        }
    }

    get_exit_code(status)
}

/// Get exit code from wait status
fn get_exit_code(status: libc::c_int) -> i32 {
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        125 // killed by signal
    } else {
        1
    }
}

/// Update exit status according to xargs rules
fn update_exit_status(current: i32, new: i32) -> i32 {
    match new {
        0 => current,
        255 => 124,
        126 | 127 => new,
        1..=125 if current == 0 => 123,
        _ => {
            if current == 0 {
                new
            } else {
                current
            }
        }
    }
}

/// Print command for verbose mode
fn print_command(cmd: &[u8], args: &[&[u8]]) {
    io::write_all(2, cmd);
    for arg in args {
        io::write_str(2, b" ");
        // Quote if contains spaces
        let needs_quote = arg.iter().any(|&c| c == b' ' || c == b'\t');
        if needs_quote {
            io::write_str(2, b"'");
        }
        io::write_all(2, arg);
        if needs_quote {
            io::write_str(2, b"'");
        }
    }
    io::write_str(2, b"\n");
}

fn print_help() {
    io::write_str(1, b"Usage: xargs [OPTIONS] [COMMAND [INITIAL-ARGS]]\n\n");
    io::write_str(
        1,
        b"Build and execute command lines from standard input.\n\n",
    );
    io::write_str(1, b"Options:\n");
    io::write_str(
        1,
        b"  -0, --null           Items separated by null, not newline\n",
    );
    io::write_str(1, b"  -d DELIM             Items separated by DELIM\n");
    io::write_str(
        1,
        b"  -I REPLSTR           Replace REPLSTR in args with input item\n",
    );
    io::write_str(
        1,
        b"  -n MAXARGS           Use at most MAXARGS per command line\n",
    );
    io::write_str(
        1,
        b"  -P MAXPROCS          Run up to MAXPROCS processes in parallel\n",
    );
    io::write_str(1, b"  -r, --no-run-if-empty  Don't run if input is empty\n");
    io::write_str(
        1,
        b"  -t, --verbose        Print command before executing\n",
    );
    io::write_str(1, b"  -x                   Exit if command line too long\n");
    io::write_str(1, b"  -h, --help           Show this help\n\n");
    io::write_str(1, b"Examples:\n");
    io::write_str(1, b"  find . -name '*.txt' | xargs grep 'pattern'\n");
    io::write_str(1, b"  find . -print0 | xargs -0 rm\n");
    io::write_str(1, b"  echo 'a b c' | xargs -n1 echo\n");
    io::write_str(1, b"  ls | xargs -I{} mv {} {}.bak\n");
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_xargs_echo() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["xargs", "echo"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello\nworld\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
        assert!(stdout.contains("world"));
    }

    #[test]
    fn test_xargs_empty_input() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["xargs", "-r", "echo"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        // With -r, no output when input is empty
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn test_xargs_n1() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["xargs", "-n1", "echo"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a\nb\nc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Each item on its own line
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_xargs_null() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["xargs", "-0", "echo"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello\0world\0").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello"));
        assert!(stdout.contains("world"));
    }

    #[test]
    fn test_xargs_replace() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["xargs", "-I{}", "echo", "item:{}"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"foo\nbar\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("item:foo"));
        assert!(stdout.contains("item:bar"));
    }

    #[test]
    fn test_xargs_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["xargs", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
        assert!(stdout.contains("-I REPLSTR"));
    }
}
