//! test - evaluate conditional expressions
//!
//! POSIX.1-2017 conditional expression evaluation for `test` and `[`.

extern crate alloc;

use alloc::vec::Vec;

use super::get_arg;
use crate::io;
use crate::sys;

// Exit codes.
const TRUE: i32 = 0;
const FALSE: i32 = 1;
const ERROR: i32 = 2;

/// test - evaluate conditional expression
///
/// # Synopsis
/// ```text
/// test EXPRESSION
/// [ EXPRESSION ]
/// ```
///
/// # Exit Status
/// - 0: expression is true
/// - 1: expression is false (or empty)
/// - 2: a syntax or usage error occurred
pub fn test(argc: i32, argv: *const *const u8) -> i32 {
    // Collect the applet name and the expression arguments.
    let name = unsafe { get_arg(argv, 0) }.unwrap_or(b"test");

    let mut args: Vec<&[u8]> = Vec::new();
    let mut i = 1;
    while i < argc {
        args.push(unsafe { get_arg(argv, i) }.unwrap_or(b""));
        i += 1;
    }

    // When invoked as `[`, a trailing `]` is mandatory and is stripped.
    if is_bracket(name) {
        match args.last() {
            Some(last) if *last == b"]" => {
                args.pop();
            }
            _ => return ERROR,
        }
    }

    match eval(&args) {
        Ok(true) => TRUE,
        Ok(false) => FALSE,
        Err(()) => ERROR,
    }
}

/// bracket - alias for test ([ command)
pub fn bracket(argc: i32, argv: *const *const u8) -> i32 {
    test(argc, argv)
}

/// True if the applet name is `[` (possibly with a leading path).
fn is_bracket(name: &[u8]) -> bool {
    match name.iter().rposition(|&c| c == b'/') {
        Some(idx) => &name[idx + 1..] == b"[",
        None => name == b"[",
    }
}

/// Evaluate an expression using the POSIX argument-count algorithm.
fn eval(args: &[&[u8]]) -> Result<bool, ()> {
    match args.len() {
        0 => Ok(false),
        1 => Ok(!args[0].is_empty()),
        2 => two_args(args[0], args[1]),
        3 => three_args(args[0], args[1], args[2]),
        4 => four_args(args),
        _ => {
            // General grammar for expressions of five or more arguments.
            let mut p = Parser { args, pos: 0 };
            let val = p.parse_expr()?;
            if p.pos != args.len() {
                return Err(());
            }
            Ok(val)
        }
    }
}

fn two_args(a: &[u8], b: &[u8]) -> Result<bool, ()> {
    if a == b"!" {
        // Negation of the one-argument test of `b`.
        return Ok(b.is_empty());
    }
    if is_unary_op(a) {
        return unary(a, b);
    }
    Err(())
}

fn three_args(a: &[u8], b: &[u8], c: &[u8]) -> Result<bool, ()> {
    if is_binary_op(b) {
        return binary(a, b, c);
    }
    if a == b"!" {
        return two_args(b, c).map(|v| !v);
    }
    if a == b"(" && c == b")" {
        return Ok(!b.is_empty());
    }
    if b == b"-a" {
        return Ok(!a.is_empty() && !c.is_empty());
    }
    if b == b"-o" {
        return Ok(!a.is_empty() || !c.is_empty());
    }
    Err(())
}

fn four_args(args: &[&[u8]]) -> Result<bool, ()> {
    let (a, b, c, d) = (args[0], args[1], args[2], args[3]);
    if a == b"!" {
        return three_args(b, c, d).map(|v| !v);
    }
    if a == b"(" && d == b")" {
        return two_args(b, c);
    }
    // Fall back to the general grammar.
    let mut p = Parser { args, pos: 0 };
    let val = p.parse_expr()?;
    if p.pos != args.len() {
        return Err(());
    }
    Ok(val)
}

// =============================================================================
// Operator classification
// =============================================================================

fn is_binary_op(op: &[u8]) -> bool {
    matches!(
        op,
        b"=" | b"=="
            | b"!="
            | b"-eq"
            | b"-ne"
            | b"-lt"
            | b"-le"
            | b"-gt"
            | b"-ge"
            | b"-nt"
            | b"-ot"
            | b"-ef"
    )
}

fn is_unary_op(op: &[u8]) -> bool {
    matches!(
        op,
        b"-b"
            | b"-c"
            | b"-d"
            | b"-e"
            | b"-f"
            | b"-g"
            | b"-h"
            | b"-L"
            | b"-k"
            | b"-p"
            | b"-r"
            | b"-s"
            | b"-S"
            | b"-t"
            | b"-u"
            | b"-w"
            | b"-x"
            | b"-O"
            | b"-G"
            | b"-n"
            | b"-z"
    )
}

// =============================================================================
// Primary evaluation
// =============================================================================

fn unary(op: &[u8], arg: &[u8]) -> Result<bool, ()> {
    match op {
        b"-n" => return Ok(!arg.is_empty()),
        b"-z" => return Ok(arg.is_empty()),
        b"-t" => {
            let fd = match sys::parse_i64(arg) {
                Some(n) => n as i32,
                None => return Ok(false),
            };
            return Ok(unsafe { libc::isatty(fd) } == 1);
        }
        b"-r" => return Ok(io::access(arg, libc::R_OK) == 0),
        b"-w" => return Ok(io::access(arg, libc::W_OK) == 0),
        b"-x" => return Ok(io::access(arg, libc::X_OK) == 0),
        b"-h" | b"-L" => {
            // Symlink test operates on the link itself.
            let mut st = io::stat_zeroed();
            let ok = io::lstat(arg, &mut st) == 0;
            return Ok(ok && (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFLNK as u32);
        }
        _ => {}
    }

    // Remaining primaries stat the target (following symlinks).
    let mut st = io::stat_zeroed();
    let ok = io::stat(arg, &mut st) == 0;
    if !ok {
        // Non-existent path: every remaining primary is false.
        return Ok(false);
    }
    let mode = st.st_mode as u32;
    let ifmt = mode & libc::S_IFMT as u32;

    let result = match op {
        b"-e" => true,
        b"-f" => ifmt == libc::S_IFREG as u32,
        b"-d" => ifmt == libc::S_IFDIR as u32,
        b"-b" => ifmt == libc::S_IFBLK as u32,
        b"-c" => ifmt == libc::S_IFCHR as u32,
        b"-p" => ifmt == libc::S_IFIFO as u32,
        b"-S" => ifmt == libc::S_IFSOCK as u32,
        b"-s" => st.st_size > 0,
        b"-g" => (mode & libc::S_ISGID as u32) != 0,
        b"-u" => (mode & libc::S_ISUID as u32) != 0,
        b"-k" => (mode & libc::S_ISVTX as u32) != 0,
        b"-O" => st.st_uid == unsafe { libc::geteuid() },
        b"-G" => st.st_gid == unsafe { libc::getegid() },
        _ => return Err(()),
    };
    Ok(result)
}

fn binary(left: &[u8], op: &[u8], right: &[u8]) -> Result<bool, ()> {
    match op {
        b"=" | b"==" => return Ok(left == right),
        b"!=" => return Ok(left != right),
        b"-eq" | b"-ne" | b"-lt" | b"-le" | b"-gt" | b"-ge" => {
            let l = sys::parse_i64(left).ok_or(())?;
            let r = sys::parse_i64(right).ok_or(())?;
            return Ok(match op {
                b"-eq" => l == r,
                b"-ne" => l != r,
                b"-lt" => l < r,
                b"-le" => l <= r,
                b"-gt" => l > r,
                b"-ge" => l >= r,
                _ => unreachable!(),
            });
        }
        b"-nt" | b"-ot" | b"-ef" => {
            let mut ls = io::stat_zeroed();
            let mut rs = io::stat_zeroed();
            let lok = io::stat(left, &mut ls) == 0;
            let rok = io::stat(right, &mut rs) == 0;
            return Ok(match op {
                // file1 is newer than file2, or file1 exists and file2 does not.
                b"-nt" => lok && (!rok || ls.st_mtime > rs.st_mtime),
                // file1 is older than file2, or file2 exists and file1 does not.
                b"-ot" => rok && (!lok || ls.st_mtime < rs.st_mtime),
                // file1 and file2 refer to the same device and inode.
                b"-ef" => lok && rok && ls.st_dev == rs.st_dev && ls.st_ino == rs.st_ino,
                _ => unreachable!(),
            });
        }
        _ => Err(()),
    }
}

// =============================================================================
// General expression grammar (5+ arguments)
//
//   expr   := term   { -o term }
//   term   := factor { -a factor }
//   factor := ! factor | ( expr ) | primary
// =============================================================================

struct Parser<'a> {
    args: &'a [&'a [u8]],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a [u8]> {
        self.args.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<&'a [u8]> {
        let a = self.args.get(self.pos).copied();
        if a.is_some() {
            self.pos += 1;
        }
        a
    }

    fn parse_expr(&mut self) -> Result<bool, ()> {
        let mut val = self.parse_term()?;
        while matches!(self.peek(), Some(x) if x == b"-o") {
            self.advance();
            let rhs = self.parse_term()?;
            val = val || rhs;
        }
        Ok(val)
    }

    fn parse_term(&mut self) -> Result<bool, ()> {
        let mut val = self.parse_factor()?;
        while matches!(self.peek(), Some(x) if x == b"-a") {
            self.advance();
            let rhs = self.parse_factor()?;
            val = val && rhs;
        }
        Ok(val)
    }

    fn parse_factor(&mut self) -> Result<bool, ()> {
        match self.peek() {
            Some(b"!") => {
                self.advance();
                Ok(!self.parse_factor()?)
            }
            Some(b"(") => {
                self.advance();
                let val = self.parse_expr()?;
                match self.advance() {
                    Some(x) if x == b")" => {}
                    _ => return Err(()),
                }
                Ok(val)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<bool, ()> {
        let a = self.advance().ok_or(())?;

        // Binary primary: arg OP arg.
        if let Some(op) = self.peek() {
            if is_binary_op(op) {
                self.advance();
                let c = self.advance().ok_or(())?;
                return binary(a, op, c);
            }
        }

        // Unary primary: OP arg.
        if is_unary_op(a) {
            if let Some(arg) = self.peek() {
                self.advance();
                return unary(a, arg);
            }
            // Dangling unary operator behaves as a non-empty string.
            return Ok(!a.is_empty());
        }

        // Bare string: true when non-empty.
        Ok(!a.is_empty())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::fs;
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
    fn test_file_exists() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = std::env::temp_dir().join("armybox_test_test");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("file.txt"), "test").unwrap();

        let output = Command::new(&armybox)
            .args(["test", "-e", dir.join("file.txt").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_not_exists() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["test", "-e", "/nonexistent/path/file"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_string_equal() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["test", "foo", "=", "foo"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_string_not_equal() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["test", "foo", "!=", "bar"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_numeric_equal() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["test", "42", "-eq", "42"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_numeric_less_than() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["test", "5", "-lt", "10"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
