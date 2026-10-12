//! expr - evaluate expressions
//!
//! Evaluate an expression given as separate argv tokens and print the result.
//! Implements a recursive-descent parser over the tokens honoring POSIX
//! precedence, plus the common `length`, `substr`, `index` and `match`
//! string functions. The `:` / `match` regex operators delegate to the shared
//! BRE/ERE engine (`crate::applets::text::regex`).

extern crate alloc;

use alloc::vec::Vec;

use super::get_arg;
use crate::io;

/// A value in `expr` is always a string; numeric operators parse it as i64.
type Value = Vec<u8>;

/// Error exit code produced when the expression cannot be evaluated.
const E_SYNTAX: i32 = 2;
const E_MATH: i32 = 2;
/// Error exit code produced when writing the result fails.
const E_IO: i32 = 2;

/// A value is "false" (null or zero) when it is the empty string or when it
/// parses as an integer equal to zero.
fn is_falsey(v: &[u8]) -> bool {
    if v.is_empty() {
        return true;
    }
    matches!(to_int(v), Some(0))
}

/// Format a signed integer into a freshly allocated byte vector.
fn int_to_value(n: i64) -> Value {
    let mut buf = [0u8; 24];
    let neg = n < 0;
    // Use u64 to safely handle i64::MIN.
    let mut m: u64 = if neg {
        (n as i128).unsigned_abs() as u64
    } else {
        n as u64
    };
    let mut i = buf.len();
    if m == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while m > 0 {
            i -= 1;
            buf[i] = b'0' + (m % 10) as u8;
            m /= 10;
        }
    }
    if neg {
        i -= 1;
        buf[i] = b'-';
    }
    buf[i..].to_vec()
}

/// Parse a value as an integer, requiring the entire string to be a valid
/// optionally-signed decimal integer.
///
/// Magnitudes outside the `i64` range are rejected (returning `None`) so that a
/// too-large literal is treated as a string operand rather than overflowing.
/// Negatives are accumulated directly so that `i64::MIN` is representable
/// without the `-(i64::MIN)` panic that a parse-then-negate approach hits.
fn to_int(v: &[u8]) -> Option<i64> {
    if v.is_empty() {
        return None;
    }
    let (neg, digits) = if v[0] == b'-' {
        (true, &v[1..])
    } else {
        (false, v)
    };
    if digits.is_empty() {
        return None;
    }
    let mut acc: i64 = 0;
    for &c in digits {
        if !c.is_ascii_digit() {
            return None;
        }
        let d = (c - b'0') as i64;
        acc = acc.checked_mul(10)?;
        acc = if neg {
            acc.checked_sub(d)?
        } else {
            acc.checked_add(d)?
        };
    }
    Some(acc)
}

// ------------------------------------------------------------------------
// Recursive-descent parser
// ------------------------------------------------------------------------

struct Parser<'a> {
    toks: &'a [&'a [u8]],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a [u8]> {
        self.toks.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<&'a [u8]> {
        let t = self.toks.get(self.pos).copied();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn remaining(&self) -> usize {
        self.toks.len().saturating_sub(self.pos)
    }

    /// Lowest precedence: `|`.
    fn parse_or(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_and()?;
        while let Some(t) = self.peek() {
            if t == b"|" {
                self.advance();
                let right = self.parse_and()?;
                // `|`: left if it is neither null nor zero, otherwise right.
                if is_falsey(&left) {
                    left = right;
                }
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// `&`.
    fn parse_and(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_rel()?;
        while let Some(t) = self.peek() {
            if t == b"&" {
                self.advance();
                let right = self.parse_rel()?;
                // `&`: left if both operands are neither null nor zero, else 0.
                if is_falsey(&left) || is_falsey(&right) {
                    left = b"0".to_vec();
                }
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// Relational operators: `= > >= < <= !=`.
    fn parse_rel(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_add()?;
        while let Some(t) = self.peek() {
            let op: &[u8] = match t {
                b"=" | b">" | b">=" | b"<" | b"<=" | b"!=" => t,
                _ => break,
            };
            self.advance();
            let right = self.parse_add()?;
            let res = compare(&left, &right, op);
            left = int_to_value(if res { 1 } else { 0 });
        }
        Ok(left)
    }

    /// Additive operators: `+ -`.
    fn parse_add(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_mul()?;
        while let Some(t) = self.peek() {
            let sub = match t {
                b"+" => false,
                b"-" => true,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            let a = to_int(&left).ok_or(E_MATH)?;
            let b = to_int(&right).ok_or(E_MATH)?;
            let r = if sub {
                a.checked_sub(b)
            } else {
                a.checked_add(b)
            };
            let r = match r {
                Some(r) => r,
                None => {
                    io::write_str(2, b"expr: overflow\n");
                    return Err(E_MATH);
                }
            };
            left = int_to_value(r);
        }
        Ok(left)
    }

    /// Multiplicative operators: `* / %`.
    fn parse_mul(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_match()?;
        while let Some(t) = self.peek() {
            let kind = match t {
                b"*" => 0,
                b"/" => 1,
                b"%" => 2,
                _ => break,
            };
            self.advance();
            let right = self.parse_match()?;
            let a = to_int(&left).ok_or(E_MATH)?;
            let b = to_int(&right).ok_or(E_MATH)?;
            let checked = match kind {
                0 => a.checked_mul(b),
                _ => {
                    if b == 0 {
                        io::write_str(2, b"expr: division by zero\n");
                        return Err(E_MATH);
                    }
                    if kind == 1 {
                        a.checked_div(b)
                    } else {
                        a.checked_rem(b)
                    }
                }
            };
            let r = match checked {
                Some(r) => r,
                None => {
                    io::write_str(2, b"expr: overflow\n");
                    return Err(E_MATH);
                }
            };
            left = int_to_value(r);
        }
        Ok(left)
    }

    /// Anchored regex match: `:`.
    fn parse_match(&mut self) -> Result<Value, i32> {
        let mut left = self.parse_primary()?;
        while let Some(t) = self.peek() {
            if t == b":" {
                self.advance();
                let right = self.parse_primary()?;
                left = regex_match(&left, &right)?;
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// Parentheses, string functions, and bare operands.
    fn parse_primary(&mut self) -> Result<Value, i32> {
        let t = self.peek().ok_or(E_SYNTAX)?;

        if t == b"(" {
            self.advance();
            let v = self.parse_or()?;
            match self.advance() {
                Some(b")") => return Ok(v),
                _ => return Err(E_SYNTAX),
            }
        }

        // String functions (GNU extensions). Only treated as functions when
        // enough operands follow; otherwise the keyword is a literal string.
        match t {
            b"length" if self.remaining() >= 2 => {
                self.advance();
                let s = self.parse_primary()?;
                return Ok(int_to_value(s.len() as i64));
            }
            b"substr" if self.remaining() >= 4 => {
                self.advance();
                let s = self.parse_primary()?;
                let pos = self.parse_primary()?;
                let len = self.parse_primary()?;
                return Ok(substr(&s, &pos, &len));
            }
            b"index" if self.remaining() >= 3 => {
                self.advance();
                let s = self.parse_primary()?;
                let chars = self.parse_primary()?;
                return Ok(int_to_value(index_of(&s, &chars)));
            }
            b"match" if self.remaining() >= 3 => {
                self.advance();
                let s = self.parse_primary()?;
                let re = self.parse_primary()?;
                return regex_match(&s, &re);
            }
            _ => {}
        }

        // Bare operand.
        let tok = self.advance().ok_or(E_SYNTAX)?;
        Ok(tok.to_vec())
    }
}

/// Compare two values with the given relational operator. Comparison is numeric
/// when both operands are integers, otherwise lexicographic on bytes.
fn compare(a: &[u8], b: &[u8], op: &[u8]) -> bool {
    let ord = match (to_int(a), to_int(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.cmp(b),
    };
    use core::cmp::Ordering::*;
    match op {
        b"=" => ord == Equal,
        b"!=" => ord != Equal,
        b">" => ord == Greater,
        b">=" => ord != Less,
        b"<" => ord == Less,
        b"<=" => ord != Greater,
        _ => false,
    }
}

/// `substr STR POS LEN`: 1-based substring. Out-of-range yields empty string.
fn substr(s: &[u8], pos: &[u8], len: &[u8]) -> Value {
    let p = to_int(pos).unwrap_or(0);
    let l = to_int(len).unwrap_or(0);
    if p < 1 || l < 1 {
        return Vec::new();
    }
    let start = (p - 1) as usize;
    if start >= s.len() {
        return Vec::new();
    }
    let end = start.saturating_add(l as usize).min(s.len());
    s[start..end].to_vec()
}

/// `index STR CHARS`: 1-based position of the first char of STR present in
/// CHARS, or 0 if none.
fn index_of(s: &[u8], chars: &[u8]) -> i64 {
    for (i, c) in s.iter().enumerate() {
        if chars.contains(c) {
            return (i + 1) as i64;
        }
    }
    0
}

// ------------------------------------------------------------------------
// Regex matching for `:` and `match` (delegates to the shared BRE engine)
// ------------------------------------------------------------------------

/// Does `pattern` contain a `\(...\)` capture group?
///
/// POSIX `expr` distinguishes patterns with a subexpression (result is the
/// captured substring) from those without (result is the match length). A
/// group is opened by an *unescaped* `\(`; a `\(` appearing inside a bracket
/// expression is literal (a backslash is an ordinary character within `[...]`
/// in a BRE), matching the shared engine's own group counting.
fn pattern_has_group(pattern: &[u8]) -> bool {
    let mut i = 0;
    while i < pattern.len() {
        match pattern[i] {
            b'\\' if i + 1 < pattern.len() => {
                if pattern[i + 1] == b'(' {
                    return true;
                }
                // Skip the escaped pair (e.g. `\\` then a following `(`).
                i += 2;
            }
            b'[' => i = skip_bracket(pattern, i),
            _ => i += 1,
        }
    }
    false
}

/// Return the index just past a `[...]` bracket expression beginning at `start`
/// (the `[`). A `]` immediately after `[` or `[^` is a literal member rather
/// than the terminator, mirroring BRE bracket parsing.
fn skip_bracket(pattern: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    if i < pattern.len() && pattern[i] == b'^' {
        i += 1;
    }
    if i < pattern.len() && pattern[i] == b']' {
        i += 1;
    }
    while i < pattern.len() && pattern[i] != b']' {
        i += 1;
    }
    if i < pattern.len() {
        i += 1; // consume the closing `]`
    }
    i
}

/// Anchored regex match used by both `:` and `match`.
///
/// POSIX `expr` regexes are Basic Regular Expressions implicitly anchored at
/// the start of the string, so the pattern is compiled as a BRE and only a
/// match beginning at position 0 is accepted (the shared engine's `search` is
/// leftmost, so a returned match that does not start at 0 means nothing
/// matches there). Without a `\(...\)` group the result is the number of bytes
/// matched, as a decimal string ("0" if no match); with a group it is the
/// substring captured by group 1 ("" if no match). A malformed pattern is a
/// syntax error (exit 2).
fn regex_match(s: &[u8], pattern: &[u8]) -> Result<Value, i32> {
    use crate::applets::regex::{Regex, Syntax};

    let syntax = Syntax {
        ere: false,
        icase: false,
        translate_escapes: false,
    };
    let re = Regex::compile(pattern, syntax).map_err(|()| E_SYNTAX)?;

    let has_group = pattern_has_group(pattern);
    // expr anchors at the start of the string: accept only a match at index 0.
    let matched = re.search(s, 0).filter(|c| c.start() == 0);

    let result = if has_group {
        match &matched {
            Some(c) => c.group(1, s).unwrap_or(&[]).to_vec(),
            None => Vec::new(),
        }
    } else {
        match matched {
            Some(c) => int_to_value((c.end() - c.start()) as i64),
            None => b"0".to_vec(),
        }
    };
    Ok(result)
}

// ------------------------------------------------------------------------
// Entry point
// ------------------------------------------------------------------------

/// expr - evaluate expressions
///
/// # Synopsis
/// ```text
/// expr EXPRESSION
/// ```
///
/// # Exit Status
/// - 0: result is neither null nor zero
/// - 1: result is null or zero
/// - 2: invalid expression or evaluation error
pub fn expr(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"expr: missing operand\n");
        return E_SYNTAX;
    }

    // Collect tokens (argv[1..argc]).
    let mut toks: Vec<&[u8]> = Vec::new();
    for i in 1..argc {
        match unsafe { get_arg(argv, i) } {
            Some(t) => toks.push(t),
            None => break,
        }
    }

    let mut parser = Parser {
        toks: &toks,
        pos: 0,
    };
    match parser.parse_or() {
        Ok(v) => {
            if parser.pos != parser.toks.len() {
                io::write_str(2, b"expr: syntax error\n");
                return E_SYNTAX;
            }
            if io::write_all(1, &v) < 0 || io::write_str(1, b"\n") < 0 {
                io::write_str(2, b"expr: I/O error\n");
                return E_IO;
            }
            if is_falsey(&v) { 1 } else { 0 }
        }
        Err(code) => code,
    }
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
    fn test_expr_addition() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["expr", "5", "+", "3"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "8");
    }

    #[test]
    fn test_expr_subtraction() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["expr", "10", "-", "3"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "7");
    }

    #[test]
    fn test_expr_multiplication() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["expr", "6", "*", "7"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "42");
    }

    #[test]
    fn test_expr_division() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["expr", "20", "/", "4"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "5");
    }

    #[test]
    fn test_expr_single_value() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["expr", "42"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "42");
    }
}
