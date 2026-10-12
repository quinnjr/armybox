//! awk - pattern scanning and processing
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/awk.html
//!
//! This is a real awk: lexer -> parser (AST) -> tree-walking interpreter,
//! with a small ERE regex engine. It implements the POSIX awk language
//! subset needed for typical programs: patterns/actions, BEGIN/END, fields
//! and records, the full expression grammar, control flow, associative
//! arrays, user-defined functions, and the standard built-in functions.

use crate::applets::get_arg;
use crate::io;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;

use libc::{c_char, c_long, c_ulong, size_t};

// ===========================================================================
// Numeric helpers
//
// glibc/musl libc (as exposed by the `libc` crate) does NOT provide the libm
// math functions for the Linux target, so we implement everything we need in
// pure Rust. Only snprintf/strtod/system/time are used from libc.
// ===========================================================================

#[inline]
fn fabs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

/// Truncate toward zero.
fn ftrunc(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    if fabs(x) < 9.0e18 {
        (x as i64) as f64
    } else {
        // beyond 2^53 all f64 values are already integers
        x
    }
}

fn fmod(a: f64, b: f64) -> f64 {
    if b == 0.0 || !a.is_finite() {
        return f64::NAN;
    }
    if !b.is_finite() {
        return a;
    }
    a - ftrunc(a / b) * b
}

const LN2: f64 = core::f64::consts::LN_2;

/// Natural log via mantissa/exponent split and an atanh series.
fn fln(x: f64) -> f64 {
    if x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    let mut bits = x.to_bits();
    // normalise subnormals
    let mut extra = 0i64;
    if (bits >> 52) & 0x7ff == 0 {
        let scaled = x * 18014398509481984.0; // 2^54
        bits = scaled.to_bits();
        extra = -54;
    }
    let e = ((bits >> 52) & 0x7ff) as i64 - 1023 + extra;
    let m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    let t = (m - 1.0) / (m + 1.0);
    let t2 = t * t;
    let mut term = t;
    let mut sum = 0.0;
    let mut k = 1.0;
    for _ in 0..24 {
        sum += term / k;
        term *= t2;
        k += 2.0;
    }
    e as f64 * LN2 + 2.0 * sum
}

/// exp via range reduction to [-ln2/2, ln2/2] and a Taylor series.
fn fexp(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::NEG_INFINITY {
        return 0.0;
    }
    if x == f64::INFINITY {
        return f64::INFINITY;
    }
    let k = ftrunc(x / LN2 + if x >= 0.0 { 0.5 } else { -0.5 });
    let r = x - k * LN2;
    // Taylor for exp(r)
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..20 {
        term *= r / n as f64;
        sum += term;
    }
    // multiply by 2^k
    let ki = k as i64;
    let mut result = sum;
    let mut steps = ki.unsigned_abs();
    let factor = if ki >= 0 { 2.0 } else { 0.5 };
    while steps > 0 {
        result *= factor;
        steps -= 1;
    }
    result
}

fn fpow(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 1.0;
    }
    if let Some(n) = as_int(b) {
        let mut result = 1.0;
        let mut base = a;
        let mut e = n.unsigned_abs();
        while e > 0 {
            if e & 1 == 1 {
                result *= base;
            }
            base *= base;
            e >>= 1;
        }
        return if n < 0 { 1.0 / result } else { result };
    }
    if a < 0.0 {
        return f64::NAN;
    }
    if a == 0.0 {
        return 0.0;
    }
    fexp(b * fln(a))
}

fn fsqrt(x: f64) -> f64 {
    if x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return x;
    }
    let mut g = x;
    for _ in 0..40 {
        g = 0.5 * (g + x / g);
    }
    g
}

const PI: f64 = core::f64::consts::PI;
const TWO_PI: f64 = core::f64::consts::TAU;

fn fsin(mut x: f64) -> f64 {
    if !x.is_finite() {
        return f64::NAN;
    }
    // range reduce to [-pi, pi]
    x -= ftrunc(x / TWO_PI) * TWO_PI;
    if x > PI {
        x -= TWO_PI;
    } else if x < -PI {
        x += TWO_PI;
    }
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut n = 1.0;
    for _ in 0..12 {
        term *= -x2 / ((2.0 * n) * (2.0 * n + 1.0));
        sum += term;
        n += 1.0;
    }
    sum
}

fn fcos(x: f64) -> f64 {
    fsin(x + PI / 2.0)
}

fn fatan(x: f64) -> f64 {
    // atan via series with argument reduction using atan(x)=pi/2-atan(1/x)
    if x.is_nan() {
        return f64::NAN;
    }
    let neg = x < 0.0;
    let mut a = fabs(x);
    let mut comp = false;
    if a > 1.0 {
        a = 1.0 / a;
        comp = true;
    }
    let a2 = a * a;
    let mut term = a;
    let mut sum = 0.0;
    let mut k = 0.0;
    for _ in 0..60 {
        let denom = 2.0 * k + 1.0;
        if k as i64 % 2 == 0 {
            sum += term / denom;
        } else {
            sum -= term / denom;
        }
        term *= a2;
        k += 1.0;
    }
    let mut r = sum;
    if comp {
        r = PI / 2.0 - r;
    }
    if neg { -r } else { r }
}

fn fatan2(y: f64, x: f64) -> f64 {
    if x > 0.0 {
        fatan(y / x)
    } else if x < 0.0 {
        if y >= 0.0 {
            fatan(y / x) + PI
        } else {
            fatan(y / x) - PI
        }
    } else if y > 0.0 {
        PI / 2.0
    } else if y < 0.0 {
        -PI / 2.0
    } else {
        0.0
    }
}

/// Return `Some(i)` if `n` is an exact integer representable as `i64`.
fn as_int(n: f64) -> Option<i64> {
    if !n.is_finite() {
        return None;
    }
    if fabs(n) < 9.0e18 {
        let i = n as i64;
        if i as f64 == n {
            return Some(i);
        }
    }
    None
}

/// Format a non-negative decimal into `out`.
fn push_u(out: &mut Vec<u8>, mut u: u128) {
    if u == 0 {
        out.push(b'0');
        return;
    }
    let start = out.len();
    while u > 0 {
        out.push(b'0' + (u % 10) as u8);
        u /= 10;
    }
    out[start..].reverse();
}

fn i64_bytes(n: i64) -> Vec<u8> {
    let mut v = Vec::new();
    if n < 0 {
        v.push(b'-');
        push_u(&mut v, (n as i128).unsigned_abs());
    } else {
        push_u(&mut v, n as u128);
    }
    v
}

/// Convert a number to its awk string representation.
/// Integers print without a decimal point; otherwise `fmt` (CONVFMT/OFMT).
fn num_to_str(n: f64, fmt: &[u8]) -> Vec<u8> {
    if let Some(i) = as_int(n) {
        return i64_bytes(i);
    }
    if n.is_nan() {
        return b"nan".to_vec();
    }
    if n.is_infinite() {
        return if n < 0.0 {
            b"-inf".to_vec()
        } else {
            b"inf".to_vec()
        };
    }
    snp_double(fmt, n)
}

/// Scan the leading numeric prefix of `s`; return `(consumed_end, saw_digits)`
/// where the number spans `s[start..end]` (start = first non-blank).
fn scan_number(s: &[u8]) -> (usize, usize, bool) {
    let mut i = 0;
    while i < s.len() && (s[i] == b' ' || s[i] == b'\t' || s[i] == b'\n') {
        i += 1;
    }
    let start = i;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    let mut any = false;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
        any = true;
    }
    if i < s.len() && s[i] == b'.' {
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
            any = true;
        }
    }
    if any && i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let save = i;
        i += 1;
        if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
            i += 1;
        }
        let mut ed = false;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
            ed = true;
        }
        if !ed {
            i = save;
        }
    }
    (start, i, any)
}

/// Parse the leading numeric value of a string (awk semantics).
fn str_to_num(s: &[u8]) -> f64 {
    let (start, end, any) = scan_number(s);
    if !any {
        return 0.0;
    }
    let mut buf = s[start..end].to_vec();
    buf.push(0);
    unsafe { libc::strtod(buf.as_ptr() as *const c_char, core::ptr::null_mut()) }
}

/// Does the whole (blank-trimmed) string look like a valid number?
fn looks_numeric(s: &[u8]) -> bool {
    let (_, end, any) = scan_number(s);
    if !any {
        return false;
    }
    let mut i = end;
    while i < s.len() && (s[i] == b' ' || s[i] == b'\t' || s[i] == b'\n') {
        i += 1;
    }
    i == s.len()
}

// --- snprintf wrappers (two-pass sizing via NULL/0) ------------------------

fn snp_long(spec: &[u8], v: c_long) -> Vec<u8> {
    let mut f = spec.to_vec();
    f.push(0);
    unsafe {
        let n = libc::snprintf(core::ptr::null_mut(), 0, f.as_ptr() as *const c_char, v);
        if n < 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize + 1];
        libc::snprintf(
            buf.as_mut_ptr() as *mut c_char,
            n as size_t + 1,
            f.as_ptr() as *const c_char,
            v,
        );
        buf.truncate(n as usize);
        buf
    }
}

fn snp_ulong(spec: &[u8], v: c_ulong) -> Vec<u8> {
    let mut f = spec.to_vec();
    f.push(0);
    unsafe {
        let n = libc::snprintf(core::ptr::null_mut(), 0, f.as_ptr() as *const c_char, v);
        if n < 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize + 1];
        libc::snprintf(
            buf.as_mut_ptr() as *mut c_char,
            n as size_t + 1,
            f.as_ptr() as *const c_char,
            v,
        );
        buf.truncate(n as usize);
        buf
    }
}

fn snp_double(spec: &[u8], v: f64) -> Vec<u8> {
    let mut f = spec.to_vec();
    f.push(0);
    unsafe {
        let n = libc::snprintf(core::ptr::null_mut(), 0, f.as_ptr() as *const c_char, v);
        if n < 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize + 1];
        libc::snprintf(
            buf.as_mut_ptr() as *mut c_char,
            n as size_t + 1,
            f.as_ptr() as *const c_char,
            v,
        );
        buf.truncate(n as usize);
        buf
    }
}

fn snp_str(spec: &[u8], s: &[u8]) -> Vec<u8> {
    let mut f = spec.to_vec();
    f.push(0);
    let mut arg = s.to_vec();
    arg.push(0);
    unsafe {
        let n = libc::snprintf(
            core::ptr::null_mut(),
            0,
            f.as_ptr() as *const c_char,
            arg.as_ptr() as *const c_char,
        );
        if n < 0 {
            return Vec::new();
        }
        let mut buf = vec![0u8; n as usize + 1];
        libc::snprintf(
            buf.as_mut_ptr() as *mut c_char,
            n as size_t + 1,
            f.as_ptr() as *const c_char,
            arg.as_ptr() as *const c_char,
        );
        buf.truncate(n as usize);
        buf
    }
}

// ===========================================================================
// Limits (guard against pathological input / OOM / stack overflow)
// ===========================================================================

/// Recursion-depth cap for the awk-expression parser.
const PARSE_MAX_DEPTH: usize = 2000;
/// Upper bound on NF / a field index, to reject `NF=2000000000` style OOM.
const FIELD_MAX: usize = 1_000_000;
/// Clamp for printf/sprintf field width and precision before snprintf.
const PRINTF_MAX_WIDTH: i64 = 8192;

// ===========================================================================
// Regex engine
// ===========================================================================
//
// awk's regexes are POSIX EREs (case-sensitive) and are backed by the shared
// BRE/ERE bytecode VM in `super::regex`. This thin wrapper adapts that engine's
// capture-returning API to the `(start, end)` span interface the rest of the
// awk interpreter expects, and preserves awk's deferred-error behavior: a
// pattern that fails to compile is flagged and turned into a runtime error only
// when the regex is actually used.

use crate::applets::regex::{Regex as ReEngine, Syntax as ReSyntax};

/// awk ERE syntax: extended, case-sensitive, with escape translation so that
/// regex constants and dynamic regexes interpret `\t`/`\n`/`\r` as control
/// bytes (matching gawk/POSIX awk behavior).
const AWK_SYNTAX: ReSyntax = ReSyntax {
    ere: true,
    icase: false,
    translate_escapes: true,
};

#[derive(Clone)]
struct Regex {
    /// Compiled program, shared (Rc) so the wrapper stays `Clone` for storage in
    /// `Expr::Regex` and cloning in `regex_from`. `None` iff compilation failed.
    re: Option<Rc<ReEngine>>,
    /// Set when the pattern failed to compile. The interpreter turns this into a
    /// runtime error when the regex is actually used.
    error: bool,
}

fn compile_regex(pat: &[u8]) -> Regex {
    match ReEngine::compile(pat, AWK_SYNTAX) {
        Ok(r) => Regex {
            re: Some(Rc::new(r)),
            error: false,
        },
        Err(()) => Regex {
            re: None,
            error: true,
        },
    }
}

/// Leftmost match at or after `from`, as a `(start, end)` byte span.
fn re_find(re: &Regex, t: &[u8], from: usize) -> Option<(usize, usize)> {
    let engine = re.re.as_ref()?;
    engine.search(t, from).map(|c| (c.start(), c.end()))
}

fn re_test(re: &Regex, t: &[u8]) -> bool {
    re_find(re, t, 0).is_some()
}

// ===========================================================================
// Values
// ===========================================================================

#[derive(Clone)]
enum Value {
    Num(f64),
    Str(Vec<u8>),
    StrNum(Vec<u8>),
    Uninit,
}

impl Value {
    fn to_num(&self) -> f64 {
        match self {
            Value::Num(n) => *n,
            Value::Str(s) | Value::StrNum(s) => str_to_num(s),
            Value::Uninit => 0.0,
        }
    }
    fn to_str(&self, fmt: &[u8]) -> Vec<u8> {
        match self {
            Value::Num(n) => num_to_str(*n, fmt),
            Value::Str(s) | Value::StrNum(s) => s.clone(),
            Value::Uninit => Vec::new(),
        }
    }
    fn is_numeric_c(&self) -> bool {
        matches!(self, Value::Num(_) | Value::StrNum(_) | Value::Uninit)
    }
    fn is_true(&self) -> bool {
        match self {
            Value::Num(n) => *n != 0.0,
            Value::StrNum(s) => str_to_num(s) != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Uninit => false,
        }
    }
}

/// Build a value from raw input text (fields, split results, -v, ARGV):
/// numeric-looking text becomes a numeric string.
fn mk_input_value(s: Vec<u8>) -> Value {
    if s.is_empty() {
        Value::Uninit
    } else if looks_numeric(&s) {
        Value::StrNum(s)
    } else {
        Value::Str(s)
    }
}

// ===========================================================================
// AST
// ===========================================================================

#[derive(Clone, Copy, PartialEq)]
enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Clone, Copy)]
enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Copy, PartialEq)]
enum Builtin {
    Length,
    Substr,
    Index,
    Split,
    Sub,
    Gsub,
    Match,
    Sprintf,
    Sin,
    Cos,
    Atan2,
    Exp,
    Log,
    Sqrt,
    Int,
    Rand,
    Srand,
    Tolower,
    Toupper,
    System,
    Close,
    Fflush,
}

#[derive(Clone)]
enum Expr {
    Num(f64),
    Str(Vec<u8>),
    Regex(Regex),
    Var(Vec<u8>),
    Field(Box<Expr>),
    Index(Vec<u8>, Vec<Expr>),
    Assign(AssignOp, Box<Expr>, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Pos(Box<Expr>),
    Not(Box<Expr>),
    PreIncr(bool, Box<Expr>),
    PostIncr(bool, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Match(bool, Box<Expr>, Box<Expr>),
    Compare(CmpOp, Box<Expr>, Box<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    In(Vec<Expr>, Vec<u8>),
    Call(Vec<u8>, Vec<Expr>),
    BuiltinCall(Builtin, Vec<Expr>),
    Grouping(Box<Expr>),
}

#[derive(Clone)]
enum Redir {
    Truncate(Box<Expr>),
    Append(Box<Expr>),
    Pipe(Box<Expr>),
}

#[derive(Clone)]
enum Stmt {
    Print(Vec<Expr>, Option<Redir>),
    Printf(Vec<Expr>, Option<Redir>),
    Expr(Expr),
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(
        Option<Box<Stmt>>,
        Option<Expr>,
        Option<Box<Stmt>>,
        Box<Stmt>,
    ),
    ForIn(Vec<u8>, Vec<u8>, Box<Stmt>),
    Next,
    Exit(Option<Expr>),
    Return(Option<Expr>),
    Break,
    Continue,
    Delete(Vec<u8>, Vec<Expr>),
}

enum Pattern {
    Begin,
    End,
    Expr(Expr),
    Range(Expr, Expr),
}

struct Rule {
    pattern: Option<Pattern>,
    action: Option<Vec<Stmt>>,
}

struct Func {
    params: Vec<Vec<u8>>,
    body: Vec<Stmt>,
}

// ===========================================================================
// Lexer
// ===========================================================================

#[derive(Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(Vec<u8>),
    Ere(Vec<u8>),
    Ident(Vec<u8>),
    FuncName(Vec<u8>),
    Builtin(Builtin),
    Begin,
    End,
    Function,
    If,
    Else,
    While,
    For,
    Do,
    Break,
    Continue,
    Next,
    Exit,
    Return,
    Delete,
    In,
    Print,
    Printf,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBrack,
    RBrack,
    Semi,
    Newline,
    Comma,
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    PowAssign,
    Or,
    And,
    Not,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    MatchOp,
    NotMatch,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Incr,
    Decr,
    Dollar,
    Question,
    Colon,
    Append,
    Pipe,
    Eof,
}

impl Tok {
    /// Can this token end an expression (so a following `/` is division)?
    fn ends_value(&self) -> bool {
        matches!(
            self,
            Tok::Num(_)
                | Tok::Str(_)
                | Tok::Ident(_)
                | Tok::RParen
                | Tok::RBrack
                | Tok::Incr
                | Tok::Decr
                | Tok::Builtin(_)
                | Tok::Dollar
        )
    }
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    prev: Tok,
    toks: Vec<Tok>,
    error: bool,
}

fn builtin_name(name: &[u8]) -> Option<Builtin> {
    Some(match name {
        b"length" => Builtin::Length,
        b"substr" => Builtin::Substr,
        b"index" => Builtin::Index,
        b"split" => Builtin::Split,
        b"sub" => Builtin::Sub,
        b"gsub" => Builtin::Gsub,
        b"match" => Builtin::Match,
        b"sprintf" => Builtin::Sprintf,
        b"sin" => Builtin::Sin,
        b"cos" => Builtin::Cos,
        b"atan2" => Builtin::Atan2,
        b"exp" => Builtin::Exp,
        b"log" => Builtin::Log,
        b"sqrt" => Builtin::Sqrt,
        b"int" => Builtin::Int,
        b"rand" => Builtin::Rand,
        b"srand" => Builtin::Srand,
        b"tolower" => Builtin::Tolower,
        b"toupper" => Builtin::Toupper,
        b"system" => Builtin::System,
        b"close" => Builtin::Close,
        b"fflush" => Builtin::Fflush,
        _ => return None,
    })
}

impl<'a> Lexer<'a> {
    fn new(s: &'a [u8]) -> Self {
        Lexer {
            s,
            i: 0,
            prev: Tok::Newline,
            toks: Vec::new(),
            error: false,
        }
    }

    fn push(&mut self, t: Tok) {
        self.prev = t.clone();
        self.toks.push(t);
    }

    fn run(mut self) -> (Vec<Tok>, bool) {
        while self.i < self.s.len() {
            let c = self.s[self.i];
            match c {
                b' ' | b'\t' => {
                    self.i += 1;
                }
                b'\\' if self.i + 1 < self.s.len() && self.s[self.i + 1] == b'\n' => {
                    self.i += 2; // line continuation
                }
                b'\r' => {
                    self.i += 1;
                }
                b'\n' => {
                    self.i += 1;
                    if !matches!(self.prev, Tok::Newline) {
                        self.push(Tok::Newline);
                    }
                }
                b'#' => {
                    while self.i < self.s.len() && self.s[self.i] != b'\n' {
                        self.i += 1;
                    }
                }
                b'"' => self.lex_string(),
                b'/' => {
                    if self.prev.ends_value() {
                        self.lex_op();
                    } else {
                        self.lex_regex();
                    }
                }
                _ if c.is_ascii_digit()
                    || (c == b'.'
                        && self.s.get(self.i + 1).is_some_and(|d| d.is_ascii_digit())) =>
                {
                    self.lex_number()
                }
                _ if c == b'_' || c.is_ascii_alphabetic() => self.lex_ident(),
                _ => self.lex_op(),
            }
        }
        self.push(Tok::Eof);
        (self.toks, self.error)
    }

    fn lex_string(&mut self) {
        self.i += 1;
        let mut out = Vec::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'"' {
                self.i += 1;
                break;
            }
            if c == b'\\' && self.i + 1 < self.s.len() {
                self.i += 1;
                let e = self.s[self.i];
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'r' => out.push(b'\r'),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    b'/' => out.push(b'/'),
                    b'a' => out.push(0x07),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0c),
                    b'v' => out.push(0x0b),
                    b'0'..=b'7' => {
                        let mut val = 0u32;
                        let mut n = 0;
                        while n < 3
                            && self.i < self.s.len()
                            && (b'0'..=b'7').contains(&self.s[self.i])
                        {
                            val = val * 8 + (self.s[self.i] - b'0') as u32;
                            self.i += 1;
                            n += 1;
                        }
                        out.push(val as u8);
                        continue;
                    }
                    other => {
                        out.push(b'\\');
                        out.push(other);
                    }
                }
                self.i += 1;
            } else {
                out.push(c);
                self.i += 1;
            }
        }
        self.push(Tok::Str(out));
    }

    fn lex_regex(&mut self) {
        self.i += 1; // opening /
        let mut out = Vec::new();
        let mut in_class = false;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'\\' && self.i + 1 < self.s.len() {
                out.push(c);
                out.push(self.s[self.i + 1]);
                self.i += 2;
                continue;
            }
            if c == b'[' {
                in_class = true;
            } else if c == b']' {
                in_class = false;
            } else if c == b'/' && !in_class {
                self.i += 1;
                break;
            } else if c == b'\n' {
                break;
            }
            out.push(c);
            self.i += 1;
        }
        self.push(Tok::Ere(out));
    }

    fn lex_number(&mut self) {
        let start = self.i;
        // hex
        if self.s[self.i] == b'0'
            && self
                .s
                .get(self.i + 1)
                .is_some_and(|&c| c == b'x' || c == b'X')
        {
            self.i += 2;
            while self.i < self.s.len() && self.s[self.i].is_ascii_hexdigit() {
                self.i += 1;
            }
            let mut buf = self.s[start..self.i].to_vec();
            buf.push(0);
            let v = unsafe { libc::strtod(buf.as_ptr() as *const c_char, core::ptr::null_mut()) };
            self.push(Tok::Num(v));
            return;
        }
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i < self.s.len() && self.s[self.i] == b'.' {
            self.i += 1;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
            }
        }
        if self.i < self.s.len() && (self.s[self.i] == b'e' || self.s[self.i] == b'E') {
            let save = self.i;
            self.i += 1;
            if self.i < self.s.len() && (self.s[self.i] == b'+' || self.s[self.i] == b'-') {
                self.i += 1;
            }
            let mut any = false;
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
                any = true;
            }
            if !any {
                self.i = save;
            }
        }
        self.push(Tok::Num(str_to_num(&self.s[start..self.i])));
    }

    fn lex_ident(&mut self) {
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i] == b'_' || self.s[self.i].is_ascii_alphanumeric())
        {
            self.i += 1;
        }
        let name = &self.s[start..self.i];
        let tok = match name {
            b"BEGIN" => Tok::Begin,
            b"END" => Tok::End,
            b"function" | b"func" => Tok::Function,
            b"if" => Tok::If,
            b"else" => Tok::Else,
            b"while" => Tok::While,
            b"for" => Tok::For,
            b"do" => Tok::Do,
            b"break" => Tok::Break,
            b"continue" => Tok::Continue,
            b"next" => Tok::Next,
            b"exit" => Tok::Exit,
            b"return" => Tok::Return,
            b"delete" => Tok::Delete,
            b"in" => Tok::In,
            b"print" => Tok::Print,
            b"printf" => Tok::Printf,
            _ => {
                if let Some(b) = builtin_name(name) {
                    Tok::Builtin(b)
                } else if self.s.get(self.i) == Some(&b'(') {
                    Tok::FuncName(name.to_vec())
                } else {
                    Tok::Ident(name.to_vec())
                }
            }
        };
        self.push(tok);
    }

    fn lex_op(&mut self) {
        let two = if self.i + 1 < self.s.len() {
            &self.s[self.i..self.i + 2]
        } else {
            &self.s[self.i..self.i + 1]
        };
        let (tok, len): (Tok, usize) = match two {
            b"+=" => (Tok::AddAssign, 2),
            b"-=" => (Tok::SubAssign, 2),
            b"*=" => (Tok::MulAssign, 2),
            b"/=" => (Tok::DivAssign, 2),
            b"%=" => (Tok::ModAssign, 2),
            b"^=" => (Tok::PowAssign, 2),
            b"==" => (Tok::Eq, 2),
            b"!=" => (Tok::Ne, 2),
            b"<=" => (Tok::Le, 2),
            b">=" => (Tok::Ge, 2),
            b"&&" => (Tok::And, 2),
            b"||" => (Tok::Or, 2),
            b"!~" => (Tok::NotMatch, 2),
            b"++" => (Tok::Incr, 2),
            b"--" => (Tok::Decr, 2),
            b">>" => (Tok::Append, 2),
            b"**" => (Tok::Caret, 2),
            _ => {
                let c = self.s[self.i];
                let t = match c {
                    b'{' => Tok::LBrace,
                    b'}' => Tok::RBrace,
                    b'(' => Tok::LParen,
                    b')' => Tok::RParen,
                    b'[' => Tok::LBrack,
                    b']' => Tok::RBrack,
                    b';' => Tok::Semi,
                    b',' => Tok::Comma,
                    b'=' => Tok::Assign,
                    b'<' => Tok::Lt,
                    b'>' => Tok::Gt,
                    b'!' => Tok::Not,
                    b'~' => Tok::MatchOp,
                    b'+' => Tok::Plus,
                    b'-' => Tok::Minus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'%' => Tok::Percent,
                    b'^' => Tok::Caret,
                    b'$' => Tok::Dollar,
                    b'?' => Tok::Question,
                    b':' => Tok::Colon,
                    b'|' => Tok::Pipe,
                    _ => {
                        self.error = true;
                        self.i += 1;
                        return;
                    }
                };
                (t, 1)
            }
        };
        self.i += len;
        self.push(tok);
    }
}

// ===========================================================================
// Parser
// ===========================================================================

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    no_gt: bool,
    error: bool,
    depth: usize,
    rules: Vec<Rule>,
    funcs: BTreeMap<Vec<u8>, Func>,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Parser {
            toks,
            pos: 0,
            no_gt: false,
            error: false,
            depth: 0,
            rules: Vec::new(),
            funcs: BTreeMap::new(),
        }
    }

    fn peek(&self) -> &Tok {
        self.toks.get(self.pos).unwrap_or(&Tok::Eof)
    }
    fn peek2(&self) -> &Tok {
        self.toks.get(self.pos + 1).unwrap_or(&Tok::Eof)
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks.get(self.pos).cloned().unwrap_or(Tok::Eof);
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) {
        if !self.eat(&t) {
            self.error = true;
        }
    }
    fn skip_terms(&mut self) {
        while matches!(self.peek(), Tok::Newline | Tok::Semi) {
            self.pos += 1;
        }
    }
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.pos += 1;
        }
    }

    fn parse_program(&mut self) {
        loop {
            self.skip_terms();
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            if matches!(self.peek(), Tok::Function) {
                self.parse_function();
            } else {
                self.parse_rule();
            }
            if self.error {
                break;
            }
        }
    }

    fn parse_function(&mut self) {
        self.bump(); // function
        let name = match self.bump() {
            Tok::Ident(n) => n,
            Tok::FuncName(n) => n,
            _ => {
                self.error = true;
                return;
            }
        };
        self.expect(Tok::LParen);
        let mut params = Vec::new();
        if !matches!(self.peek(), Tok::RParen) {
            loop {
                match self.bump() {
                    Tok::Ident(n) => params.push(n),
                    _ => {
                        self.error = true;
                        break;
                    }
                }
                if !self.eat(&Tok::Comma) {
                    break;
                }
                self.skip_newlines();
            }
        }
        self.expect(Tok::RParen);
        self.skip_newlines();
        self.expect(Tok::LBrace);
        let body = self.parse_stmt_list();
        self.expect(Tok::RBrace);
        self.funcs.insert(name, Func { params, body });
    }

    fn parse_rule(&mut self) {
        let pattern;
        match self.peek() {
            Tok::Begin => {
                self.bump();
                pattern = Some(Pattern::Begin);
            }
            Tok::End => {
                self.bump();
                pattern = Some(Pattern::End);
            }
            Tok::LBrace => {
                pattern = None;
            }
            _ => {
                let e = self.parse_expr();
                if self.eat(&Tok::Comma) {
                    self.skip_newlines();
                    let e2 = self.parse_expr();
                    pattern = Some(Pattern::Range(e, e2));
                } else {
                    pattern = Some(Pattern::Expr(e));
                }
            }
        }
        let action = if matches!(self.peek(), Tok::LBrace) {
            self.bump();
            let body = self.parse_stmt_list();
            self.expect(Tok::RBrace);
            Some(body)
        } else {
            None
        };
        self.rules.push(Rule { pattern, action });
    }

    fn parse_stmt_list(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        loop {
            self.skip_terms();
            if matches!(self.peek(), Tok::RBrace | Tok::Eof) {
                break;
            }
            let s = self.parse_stmt();
            stmts.push(s);
            if self.error {
                break;
            }
        }
        stmts
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek().clone() {
            Tok::LBrace => {
                self.bump();
                let b = self.parse_stmt_list();
                self.expect(Tok::RBrace);
                Stmt::Block(b)
            }
            Tok::Print => self.parse_print(false),
            Tok::Printf => self.parse_print(true),
            Tok::If => self.parse_if(),
            Tok::While => self.parse_while(),
            Tok::Do => self.parse_do(),
            Tok::For => self.parse_for(),
            Tok::Break => {
                self.bump();
                Stmt::Break
            }
            Tok::Continue => {
                self.bump();
                Stmt::Continue
            }
            Tok::Next => {
                self.bump();
                Stmt::Next
            }
            Tok::Exit => {
                self.bump();
                let e = if self.stmt_ends() {
                    None
                } else {
                    Some(self.parse_expr())
                };
                Stmt::Exit(e)
            }
            Tok::Return => {
                self.bump();
                let e = if self.stmt_ends() {
                    None
                } else {
                    Some(self.parse_expr())
                };
                Stmt::Return(e)
            }
            Tok::Delete => {
                self.bump();
                let name = match self.bump() {
                    Tok::Ident(n) => n,
                    _ => {
                        self.error = true;
                        Vec::new()
                    }
                };
                let mut subs = Vec::new();
                if self.eat(&Tok::LBrack) {
                    subs.push(self.parse_expr());
                    while self.eat(&Tok::Comma) {
                        subs.push(self.parse_expr());
                    }
                    self.expect(Tok::RBrack);
                }
                Stmt::Delete(name, subs)
            }
            Tok::Semi => {
                self.bump();
                Stmt::Block(Vec::new())
            }
            _ => {
                let e = self.parse_expr();
                Stmt::Expr(e)
            }
        }
    }

    fn stmt_ends(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Semi | Tok::Newline | Tok::RBrace | Tok::Eof
        )
    }

    fn parse_print(&mut self, is_printf: bool) -> Stmt {
        self.bump();
        let mut args = Vec::new();
        let save_gt = self.no_gt;
        self.no_gt = true;
        if !self.print_ends() {
            args.push(self.parse_expr());
            while self.eat(&Tok::Comma) {
                self.skip_newlines();
                args.push(self.parse_expr());
            }
        }
        self.no_gt = save_gt;
        // Unwrap a single parenthesised list: print (a, b)
        let redir = self.parse_redir();
        if is_printf {
            Stmt::Printf(args, redir)
        } else {
            Stmt::Print(args, redir)
        }
    }

    fn print_ends(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Semi | Tok::Newline | Tok::RBrace | Tok::Eof | Tok::Gt | Tok::Append | Tok::Pipe
        )
    }

    fn parse_redir(&mut self) -> Option<Redir> {
        match self.peek() {
            Tok::Gt => {
                self.bump();
                Some(Redir::Truncate(Box::new(self.parse_expr())))
            }
            Tok::Append => {
                self.bump();
                Some(Redir::Append(Box::new(self.parse_expr())))
            }
            Tok::Pipe => {
                self.bump();
                Some(Redir::Pipe(Box::new(self.parse_expr())))
            }
            _ => None,
        }
    }

    fn parse_if(&mut self) -> Stmt {
        self.bump();
        self.expect(Tok::LParen);
        let cond = self.parse_expr();
        self.expect(Tok::RParen);
        self.skip_newlines();
        let then = Box::new(self.parse_stmt());
        let save = self.pos;
        self.skip_terms();
        let els = if matches!(self.peek(), Tok::Else) {
            self.bump();
            self.skip_newlines();
            Some(Box::new(self.parse_stmt()))
        } else {
            self.pos = save;
            None
        };
        Stmt::If(cond, then, els)
    }

    fn parse_while(&mut self) -> Stmt {
        self.bump();
        self.expect(Tok::LParen);
        let cond = self.parse_expr();
        self.expect(Tok::RParen);
        self.skip_newlines();
        let body = Box::new(self.parse_stmt());
        Stmt::While(cond, body)
    }

    fn parse_do(&mut self) -> Stmt {
        self.bump();
        self.skip_newlines();
        let body = Box::new(self.parse_stmt());
        self.skip_terms();
        self.expect(Tok::While);
        self.expect(Tok::LParen);
        let cond = self.parse_expr();
        self.expect(Tok::RParen);
        Stmt::DoWhile(body, cond)
    }

    fn parse_for(&mut self) -> Stmt {
        self.bump();
        self.expect(Tok::LParen);
        // for (var in array)
        if let (Tok::Ident(_), Tok::In) = (self.peek().clone(), self.peek2().clone()) {
            let var = match self.bump() {
                Tok::Ident(n) => n,
                _ => Vec::new(),
            };
            self.bump(); // in
            let arr = match self.bump() {
                Tok::Ident(n) => n,
                _ => {
                    self.error = true;
                    Vec::new()
                }
            };
            self.expect(Tok::RParen);
            self.skip_newlines();
            let body = Box::new(self.parse_stmt());
            return Stmt::ForIn(var, arr, body);
        }
        let init = if matches!(self.peek(), Tok::Semi) {
            None
        } else {
            Some(Box::new(self.parse_simple_stmt()))
        };
        self.expect(Tok::Semi);
        let cond = if matches!(self.peek(), Tok::Semi) {
            None
        } else {
            Some(self.parse_expr())
        };
        self.expect(Tok::Semi);
        let post = if matches!(self.peek(), Tok::RParen) {
            None
        } else {
            Some(Box::new(self.parse_simple_stmt()))
        };
        self.expect(Tok::RParen);
        self.skip_newlines();
        let body = Box::new(self.parse_stmt());
        Stmt::For(init, cond, post, body)
    }

    fn parse_simple_stmt(&mut self) -> Stmt {
        // used in for(;;) init/post: an expression (with assignments/incr)
        Stmt::Expr(self.parse_expr())
    }

    // --- expression grammar ---

    fn parse_expr(&mut self) -> Expr {
        // Bound expression-nesting recursion (parenthesised sub-expressions
        // recurse through parse_primary -> parse_expr) to avoid a parser stack
        // overflow on adversarial input; past the cap it is a syntax error.
        self.depth += 1;
        if self.depth > PARSE_MAX_DEPTH {
            self.error = true;
            self.depth -= 1;
            return Expr::Num(0.0);
        }
        let e = self.parse_assign();
        self.depth -= 1;
        e
    }

    fn parse_assign(&mut self) -> Expr {
        let left = self.parse_ternary();
        let op = match self.peek() {
            Tok::Assign => AssignOp::Set,
            Tok::AddAssign => AssignOp::Add,
            Tok::SubAssign => AssignOp::Sub,
            Tok::MulAssign => AssignOp::Mul,
            Tok::DivAssign => AssignOp::Div,
            Tok::ModAssign => AssignOp::Mod,
            Tok::PowAssign => AssignOp::Pow,
            _ => return left,
        };
        if !is_lvalue(&left) {
            return left;
        }
        self.bump();
        self.skip_newlines();
        let right = self.parse_assign();
        Expr::Assign(op, Box::new(left), Box::new(right))
    }

    fn parse_ternary(&mut self) -> Expr {
        let cond = self.parse_or();
        if self.eat(&Tok::Question) {
            self.skip_newlines();
            let a = self.parse_assign();
            self.expect(Tok::Colon);
            self.skip_newlines();
            let b = self.parse_assign();
            Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
        } else {
            cond
        }
    }

    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.eat(&Tok::Or) {
            self.skip_newlines();
            let right = self.parse_and();
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_in();
        while self.eat(&Tok::And) {
            self.skip_newlines();
            let right = self.parse_in();
            left = Expr::And(Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_in(&mut self) -> Expr {
        let mut left = self.parse_match();
        while matches!(self.peek(), Tok::In) {
            self.bump();
            let arr = match self.bump() {
                Tok::Ident(n) => n,
                _ => {
                    self.error = true;
                    Vec::new()
                }
            };
            left = Expr::In(vec![left], arr);
        }
        left
    }

    fn parse_match(&mut self) -> Expr {
        let mut left = self.parse_compare();
        loop {
            let positive = match self.peek() {
                Tok::MatchOp => true,
                Tok::NotMatch => false,
                _ => break,
            };
            self.bump();
            let right = self.parse_compare();
            left = Expr::Match(positive, Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_compare(&mut self) -> Expr {
        let mut left = self.parse_concat();
        loop {
            let op = match self.peek() {
                Tok::Lt => CmpOp::Lt,
                Tok::Le => CmpOp::Le,
                Tok::Ne => CmpOp::Ne,
                Tok::Eq => CmpOp::Eq,
                Tok::Ge => CmpOp::Ge,
                Tok::Gt if !self.no_gt => CmpOp::Gt,
                _ => break,
            };
            self.bump();
            let right = self.parse_concat();
            left = Expr::Compare(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_concat(&mut self) -> Expr {
        let mut left = self.parse_additive();
        while self.starts_value() {
            let right = self.parse_additive();
            left = Expr::Concat(Box::new(left), Box::new(right));
        }
        left
    }

    fn starts_value(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Num(_)
                | Tok::Str(_)
                | Tok::Ere(_)
                | Tok::Ident(_)
                | Tok::FuncName(_)
                | Tok::Builtin(_)
                | Tok::Dollar
                | Tok::LParen
                | Tok::Not
                | Tok::Incr
                | Tok::Decr
        )
    }

    fn parse_additive(&mut self) -> Expr {
        let mut left = self.parse_mul();
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_mul();
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_mul(&mut self) -> Expr {
        let mut left = self.parse_unary();
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary();
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        left
    }

    fn parse_unary(&mut self) -> Expr {
        // Bound unary-operator chains (`!!!!x`, `- - - x`, ...) against a
        // parser stack overflow; past the cap it is a syntax error.
        self.depth += 1;
        if self.depth > PARSE_MAX_DEPTH {
            self.error = true;
            self.depth -= 1;
            return Expr::Num(0.0);
        }
        let e = match self.peek() {
            Tok::Not => {
                self.bump();
                Expr::Not(Box::new(self.parse_unary()))
            }
            Tok::Minus => {
                self.bump();
                Expr::Neg(Box::new(self.parse_unary()))
            }
            Tok::Plus => {
                self.bump();
                Expr::Pos(Box::new(self.parse_unary()))
            }
            Tok::Incr => {
                self.bump();
                Expr::PreIncr(true, Box::new(self.parse_unary()))
            }
            Tok::Decr => {
                self.bump();
                Expr::PreIncr(false, Box::new(self.parse_unary()))
            }
            _ => self.parse_pow(),
        };
        self.depth -= 1;
        e
    }

    fn parse_pow(&mut self) -> Expr {
        let base = self.parse_postfix();
        if matches!(self.peek(), Tok::Caret) {
            self.bump();
            let rhs = self.parse_unary();
            Expr::Binary(BinOp::Pow, Box::new(base), Box::new(rhs))
        } else {
            base
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut e = self.parse_primary();
        loop {
            match self.peek() {
                Tok::Incr if is_lvalue(&e) => {
                    self.bump();
                    e = Expr::PostIncr(true, Box::new(e));
                }
                Tok::Decr if is_lvalue(&e) => {
                    self.bump();
                    e = Expr::PostIncr(false, Box::new(e));
                }
                _ => break,
            }
        }
        e
    }

    fn parse_primary(&mut self) -> Expr {
        match self.bump() {
            Tok::Num(n) => Expr::Num(n),
            Tok::Str(s) => Expr::Str(s),
            Tok::Ere(r) => Expr::Regex(compile_regex(&r)),
            Tok::Dollar => {
                let f = self.parse_primary_for_field();
                Expr::Field(Box::new(f))
            }
            Tok::Incr => Expr::PreIncr(true, Box::new(self.parse_unary())),
            Tok::Decr => Expr::PreIncr(false, Box::new(self.parse_unary())),
            Tok::Not => Expr::Not(Box::new(self.parse_unary())),
            Tok::Minus => Expr::Neg(Box::new(self.parse_unary())),
            Tok::Plus => Expr::Pos(Box::new(self.parse_unary())),
            Tok::LParen => {
                let save_gt = self.no_gt;
                self.no_gt = false;
                let first = self.parse_expr();
                if matches!(self.peek(), Tok::Comma) {
                    let mut list = vec![first];
                    while self.eat(&Tok::Comma) {
                        self.skip_newlines();
                        list.push(self.parse_expr());
                    }
                    self.expect(Tok::RParen);
                    self.no_gt = save_gt;
                    if matches!(self.peek(), Tok::In) {
                        self.bump();
                        let arr = match self.bump() {
                            Tok::Ident(n) => n,
                            _ => {
                                self.error = true;
                                Vec::new()
                            }
                        };
                        Expr::In(list, arr)
                    } else {
                        // Grouped list only valid before `in`; treat as last expr.
                        self.error = true;
                        Expr::Grouping(Box::new(list.pop().unwrap()))
                    }
                } else {
                    self.expect(Tok::RParen);
                    self.no_gt = save_gt;
                    Expr::Grouping(Box::new(first))
                }
            }
            Tok::Ident(name) => {
                if matches!(self.peek(), Tok::LBrack) {
                    self.bump();
                    let mut subs = vec![self.parse_expr()];
                    while self.eat(&Tok::Comma) {
                        subs.push(self.parse_expr());
                    }
                    self.expect(Tok::RBrack);
                    Expr::Index(name, subs)
                } else {
                    Expr::Var(name)
                }
            }
            Tok::FuncName(name) => {
                self.expect(Tok::LParen);
                let args = self.parse_arg_list();
                self.expect(Tok::RParen);
                Expr::Call(name, args)
            }
            Tok::Builtin(b) => {
                let args = if matches!(self.peek(), Tok::LParen) {
                    self.bump();
                    let a = self.parse_arg_list();
                    self.expect(Tok::RParen);
                    a
                } else {
                    Vec::new()
                };
                Expr::BuiltinCall(b, args)
            }
            _ => {
                self.error = true;
                Expr::Num(0.0)
            }
        }
    }

    /// Operand of `$`: a tight primary (so `$i`, `$1`, `$(e)`, `$NF`, `$$0`).
    fn parse_primary_for_field(&mut self) -> Expr {
        self.parse_primary()
    }

    fn parse_arg_list(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        self.skip_newlines();
        if matches!(self.peek(), Tok::RParen) {
            return args;
        }
        args.push(self.parse_expr());
        while self.eat(&Tok::Comma) {
            self.skip_newlines();
            args.push(self.parse_expr());
        }
        self.skip_newlines();
        args
    }
}

fn is_lvalue(e: &Expr) -> bool {
    match e {
        Expr::Var(_) | Expr::Field(_) | Expr::Index(_, _) => true,
        Expr::Grouping(inner) => is_lvalue(inner),
        _ => false,
    }
}

// ===========================================================================
// Interpreter
// ===========================================================================

enum Flow {
    Normal,
    Break,
    Continue,
    Next,
    Exit,
    Return(Value),
}

struct Frame {
    vars: BTreeMap<Vec<u8>, Value>,
    arrays: BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, Value>>,
    params: Vec<Vec<u8>>,
}

struct Interp {
    globals: BTreeMap<Vec<u8>, Value>,
    arrays: BTreeMap<Vec<u8>, BTreeMap<Vec<u8>, Value>>,
    frames: Vec<Frame>,
    funcs: BTreeMap<Vec<u8>, Func>,
    record: Vec<u8>,
    fields: Vec<Vec<u8>>,
    nf: usize,
    nr: i64,
    fnr: i64,
    out: Vec<u8>,
    exit_code: i32,
    range_active: Vec<bool>,
    rand_seed: u64,
    rand_prev_seed: u64,
}

impl Interp {
    fn new(funcs: BTreeMap<Vec<u8>, Func>, nrules: usize) -> Self {
        let mut g = BTreeMap::new();
        g.insert(b"FS".to_vec(), Value::Str(b" ".to_vec()));
        g.insert(b"OFS".to_vec(), Value::Str(b" ".to_vec()));
        g.insert(b"ORS".to_vec(), Value::Str(b"\n".to_vec()));
        g.insert(b"RS".to_vec(), Value::Str(b"\n".to_vec()));
        g.insert(b"SUBSEP".to_vec(), Value::Str(vec![0x1c]));
        g.insert(b"CONVFMT".to_vec(), Value::Str(b"%.6g".to_vec()));
        g.insert(b"OFMT".to_vec(), Value::Str(b"%.6g".to_vec()));
        g.insert(b"RSTART".to_vec(), Value::Num(0.0));
        g.insert(b"RLENGTH".to_vec(), Value::Num(-1.0));
        g.insert(b"FILENAME".to_vec(), Value::Str(Vec::new()));
        Interp {
            globals: g,
            arrays: BTreeMap::new(),
            frames: Vec::new(),
            funcs,
            record: Vec::new(),
            fields: Vec::new(),
            nf: 0,
            nr: 0,
            fnr: 0,
            out: Vec::new(),
            exit_code: 0,
            range_active: vec![false; nrules],
            rand_seed: 0x2545_f491_4f6c_dd1d,
            rand_prev_seed: 0,
        }
    }

    fn next_rand(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.rand_seed;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rand_seed = x;
        let v = x.wrapping_mul(0x2545_f491_4f6c_dd1d);
        ((v >> 11) as f64) / ((1u64 << 53) as f64)
    }

    // --- special-variable string reads ---
    fn raw_str(&self, name: &[u8], default: &[u8]) -> Vec<u8> {
        match self.globals.get(name) {
            Some(Value::Str(s)) | Some(Value::StrNum(s)) => s.clone(),
            Some(Value::Num(n)) => num_to_str(*n, b"%.6g"),
            _ => default.to_vec(),
        }
    }
    fn convfmt(&self) -> Vec<u8> {
        self.raw_str(b"CONVFMT", b"%.6g")
    }
    fn ofmt(&self) -> Vec<u8> {
        self.raw_str(b"OFMT", b"%.6g")
    }
    fn ofs(&self) -> Vec<u8> {
        self.raw_str(b"OFS", b" ")
    }
    fn ors(&self) -> Vec<u8> {
        self.raw_str(b"ORS", b"\n")
    }
    fn fs(&self) -> Vec<u8> {
        self.raw_str(b"FS", b" ")
    }
    fn subsep(&self) -> Vec<u8> {
        self.raw_str(b"SUBSEP", &[0x1c])
    }

    fn in_func(&self) -> bool {
        !self.frames.is_empty()
    }
    fn is_param(&self, name: &[u8]) -> bool {
        self.frames
            .last()
            .is_some_and(|f| f.params.iter().any(|p| p == name))
    }

    // --- variable get/set ---
    fn get_var(&mut self, name: &[u8]) -> Value {
        match name {
            b"NF" => return Value::Num(self.nf as f64),
            b"NR" => return Value::Num(self.nr as f64),
            b"FNR" => return Value::Num(self.fnr as f64),
            _ => {}
        }
        if self.in_func() && self.is_param(name) {
            return self
                .frames
                .last()
                .and_then(|f| f.vars.get(name).cloned())
                .unwrap_or(Value::Uninit);
        }
        self.globals.get(name).cloned().unwrap_or(Value::Uninit)
    }

    fn set_var(&mut self, name: &[u8], val: Value) {
        match name {
            b"NF" => {
                let newnf = val.to_num();
                let newnf = if newnf < 0.0 { 0 } else { newnf as usize };
                self.set_nf(newnf);
                return;
            }
            b"NR" => {
                self.nr = val.to_num() as i64;
                return;
            }
            b"FNR" => {
                self.fnr = val.to_num() as i64;
                return;
            }
            _ => {}
        }
        if self.in_func() && self.is_param(name) {
            self.frames
                .last_mut()
                .unwrap()
                .vars
                .insert(name.to_vec(), val);
        } else {
            self.globals.insert(name.to_vec(), val);
        }
    }

    // --- arrays ---
    fn array_mut(&mut self, name: &[u8]) -> &mut BTreeMap<Vec<u8>, Value> {
        if self.in_func() && self.is_param(name) {
            self.frames
                .last_mut()
                .unwrap()
                .arrays
                .entry(name.to_vec())
                .or_default()
        } else {
            self.arrays.entry(name.to_vec()).or_default()
        }
    }
    fn is_array(&self, name: &[u8]) -> bool {
        if self.in_func() && self.is_param(name) {
            self.frames
                .last()
                .is_some_and(|f| f.arrays.contains_key(name))
        } else {
            self.arrays.contains_key(name)
        }
    }

    // --- fields ---
    fn resplit(&mut self) {
        let fs = self.fs();
        let parts = split_fields(&self.record, &fs);
        self.nf = parts.len();
        self.fields = parts;
    }
    fn rebuild_record(&mut self) {
        let ofs = self.ofs();
        let mut rec = Vec::new();
        for (i, f) in self.fields.iter().enumerate() {
            if i > 0 {
                rec.extend_from_slice(&ofs);
            }
            rec.extend_from_slice(f);
        }
        self.record = rec;
    }
    fn set_nf(&mut self, newnf: usize) {
        if newnf > FIELD_MAX {
            self.runtime_error(b"awk: NF set too large\n");
        }
        if newnf < self.nf {
            self.fields.truncate(newnf);
        } else {
            self.fields.resize(newnf, Vec::new());
        }
        self.nf = newnf;
        self.rebuild_record();
    }
    fn get_field(&self, idx: i64) -> Value {
        if idx == 0 {
            return mk_input_value(self.record.clone());
        }
        if idx >= 1 && (idx as usize) <= self.nf {
            return mk_input_value(self.fields[idx as usize - 1].clone());
        }
        Value::Uninit
    }
    fn set_field(&mut self, idx: i64, s: Vec<u8>) {
        if idx == 0 {
            self.record = s;
            self.resplit();
        } else if idx >= 1 {
            let n = idx as usize;
            if n > FIELD_MAX {
                self.runtime_error(b"awk: field index too large\n");
            }
            if n > self.nf {
                self.fields.resize(n, Vec::new());
                self.nf = n;
            }
            self.fields[n - 1] = s;
            self.rebuild_record();
        }
    }

    // --- new-record setup ---
    fn set_record(&mut self, rec: Vec<u8>) {
        self.record = rec;
        self.resplit();
    }

    fn flush(&mut self) {
        if self.out.is_empty() {
            return;
        }
        // Only clear the bytes that were actually written; on a short write
        // keep the remainder and report the failure.
        let want = self.out.len();
        let wrote = io::write_all_count(1, &self.out);
        self.out.drain(..wrote);
        if wrote < want {
            io::write_str(2, b"awk: write error\n");
            if self.exit_code == 0 {
                self.exit_code = 2;
            }
        }
    }

    /// Flush buffered stdout, print `msg` to stderr, and abort the whole
    /// program (traditional awk behaviour for fatal runtime errors). The flush
    /// ensures the user still sees everything printed before the error.
    fn runtime_error(&mut self, msg: &[u8]) -> ! {
        self.flush();
        crate::io::write_str(2, msg);
        unsafe { libc::exit(2) }
    }

    fn div_by_zero(&mut self) -> ! {
        self.runtime_error(b"awk: division by zero\n")
    }

    // --- evaluation ---
    fn eval(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Regex(re) => {
                if re.error {
                    self.runtime_error(b"awk: invalid regular expression\n");
                }
                Value::Num(if re_test(re, &self.record) { 1.0 } else { 0.0 })
            }
            Expr::Grouping(inner) => self.eval(inner),
            Expr::Var(name) => self.get_var(name),
            Expr::Field(ie) => {
                let idx = self.eval(ie).to_num() as i64;
                self.get_field(idx)
            }
            Expr::Index(name, subs) => {
                let key = self.subscript(subs);
                let m = self.array_mut(name);
                m.entry(key).or_insert(Value::Uninit).clone()
            }
            Expr::Assign(op, lhs, rhs) => {
                let rv = self.eval(rhs);
                self.do_assign(*op, lhs, rv)
            }
            Expr::Binary(op, a, b) => {
                let x = self.eval(a).to_num();
                let y = self.eval(b).to_num();
                Value::Num(match op {
                    BinOp::Add => x + y,
                    BinOp::Sub => x - y,
                    BinOp::Mul => x * y,
                    BinOp::Div => {
                        if y == 0.0 {
                            self.div_by_zero();
                        }
                        x / y
                    }
                    BinOp::Mod => {
                        if y == 0.0 {
                            self.div_by_zero();
                        }
                        fmod(x, y)
                    }
                    BinOp::Pow => fpow(x, y),
                })
            }
            Expr::Neg(a) => Value::Num(-self.eval(a).to_num()),
            Expr::Pos(a) => Value::Num(self.eval(a).to_num()),
            Expr::Not(a) => Value::Num(if self.eval(a).is_true() { 0.0 } else { 1.0 }),
            Expr::PreIncr(inc, lhs) => {
                let cur = self.eval_lvalue(lhs);
                let nv = if *inc { cur + 1.0 } else { cur - 1.0 };
                self.do_assign(AssignOp::Set, lhs, Value::Num(nv));
                Value::Num(nv)
            }
            Expr::PostIncr(inc, lhs) => {
                let cur = self.eval_lvalue(lhs);
                let nv = if *inc { cur + 1.0 } else { cur - 1.0 };
                self.do_assign(AssignOp::Set, lhs, Value::Num(nv));
                Value::Num(cur)
            }
            Expr::Ternary(c, a, b) => {
                if self.eval(c).is_true() {
                    self.eval(a)
                } else {
                    self.eval(b)
                }
            }
            Expr::And(a, b) => {
                let r = self.eval(a).is_true() && self.eval(b).is_true();
                Value::Num(if r { 1.0 } else { 0.0 })
            }
            Expr::Or(a, b) => {
                let r = self.eval(a).is_true() || self.eval(b).is_true();
                Value::Num(if r { 1.0 } else { 0.0 })
            }
            Expr::Match(pos, l, r) => {
                let conv = self.convfmt();
                let ls = self.eval(l).to_str(&conv);
                let re = self.regex_from(r);
                let m = re_test(&re, &ls);
                Value::Num(if m == *pos { 1.0 } else { 0.0 })
            }
            Expr::Compare(op, a, b) => {
                let va = self.eval(a);
                let vb = self.eval(b);
                let conv = self.convfmt();
                let ord = cmp_values(&va, &vb, &conv);
                use core::cmp::Ordering::*;
                let r = match op {
                    CmpOp::Lt => ord == Less,
                    CmpOp::Le => ord != Greater,
                    CmpOp::Gt => ord == Greater,
                    CmpOp::Ge => ord != Less,
                    CmpOp::Eq => ord == Equal,
                    CmpOp::Ne => ord != Equal,
                };
                Value::Num(if r { 1.0 } else { 0.0 })
            }
            Expr::Concat(a, b) => {
                let conv = self.convfmt();
                let mut s = self.eval(a).to_str(&conv);
                s.extend_from_slice(&self.eval(b).to_str(&conv));
                Value::Str(s)
            }
            Expr::In(subs, arr) => {
                let key = self.subscript(subs);
                let present = if self.in_func() && self.is_param(arr) {
                    self.frames
                        .last()
                        .is_some_and(|f| f.arrays.get(arr).is_some_and(|m| m.contains_key(&key)))
                } else {
                    self.arrays.get(arr).is_some_and(|m| m.contains_key(&key))
                };
                Value::Num(if present { 1.0 } else { 0.0 })
            }
            Expr::Call(name, args) => self.call_user(name, args),
            Expr::BuiltinCall(b, args) => self.call_builtin(*b, args),
        }
    }

    fn subscript(&mut self, subs: &[Expr]) -> Vec<u8> {
        let conv = self.convfmt();
        if subs.len() == 1 {
            return self.eval(&subs[0]).to_str(&conv);
        }
        let sep = self.subsep();
        let mut key = Vec::new();
        for (i, s) in subs.iter().enumerate() {
            if i > 0 {
                key.extend_from_slice(&sep);
            }
            key.extend_from_slice(&self.eval(s).to_str(&conv));
        }
        key
    }

    fn eval_lvalue(&mut self, e: &Expr) -> f64 {
        self.eval(e).to_num()
    }

    fn do_assign(&mut self, op: AssignOp, lhs: &Expr, rhs: Value) -> Value {
        let newval = if op == AssignOp::Set {
            rhs
        } else {
            let cur = self.eval_lvalue(lhs);
            let r = rhs.to_num();
            if matches!(op, AssignOp::Div | AssignOp::Mod) && r == 0.0 {
                self.div_by_zero();
            }
            Value::Num(match op {
                AssignOp::Add => cur + r,
                AssignOp::Sub => cur - r,
                AssignOp::Mul => cur * r,
                AssignOp::Div => cur / r,
                AssignOp::Mod => fmod(cur, r),
                AssignOp::Pow => fpow(cur, r),
                AssignOp::Set => unreachable!(),
            })
        };
        match lhs {
            Expr::Var(name) => self.set_var(name, newval.clone()),
            Expr::Field(ie) => {
                let idx = self.eval(ie).to_num() as i64;
                let conv = self.convfmt();
                let s = newval.to_str(&conv);
                self.set_field(idx, s);
            }
            Expr::Index(name, subs) => {
                let key = self.subscript(subs);
                self.array_mut(name).insert(key, newval.clone());
            }
            Expr::Grouping(inner) => return self.do_assign(AssignOp::Set, inner, newval),
            _ => {}
        }
        newval
    }

    fn regex_from(&mut self, e: &Expr) -> Regex {
        let re = if let Expr::Regex(r) = e {
            r.clone()
        } else {
            let conv = self.convfmt();
            let s = self.eval(e).to_str(&conv);
            compile_regex(&s)
        };
        if re.error {
            self.runtime_error(b"awk: invalid regular expression\n");
        }
        re
    }

    fn call_user(&mut self, name: &[u8], args: &[Expr]) -> Value {
        let func = match self.funcs.get(name) {
            Some(f) => Func {
                params: f.params.clone(),
                body: f.body.clone(),
            },
            None => return Value::Uninit,
        };
        let mut frame = Frame {
            vars: BTreeMap::new(),
            arrays: BTreeMap::new(),
            params: func.params.clone(),
        };
        // Bind scalar args by value (arrays passed by name are not aliased).
        for (i, p) in func.params.iter().enumerate() {
            if let Some(a) = args.get(i) {
                if let Expr::Var(vn) = a {
                    if self.is_array(vn) {
                        // best-effort: copy array contents
                        let src = if self.in_func() && self.is_param(vn) {
                            self.frames.last().and_then(|f| f.arrays.get(vn)).cloned()
                        } else {
                            self.arrays.get(vn).cloned()
                        };
                        if let Some(m) = src {
                            frame.arrays.insert(p.clone(), m);
                            continue;
                        }
                    }
                }
                let v = self.eval(a);
                frame.vars.insert(p.clone(), v);
            } else {
                frame.vars.insert(p.clone(), Value::Uninit);
            }
        }
        self.frames.push(frame);
        let mut ret = Value::Uninit;
        if let Flow::Return(v) = self.exec_block(&func.body) {
            ret = v;
        }
        self.frames.pop();
        ret
    }

    fn call_builtin(&mut self, b: Builtin, args: &[Expr]) -> Value {
        let conv = self.convfmt();
        match b {
            Builtin::Length => {
                if args.is_empty() {
                    return Value::Num(self.record.len() as f64);
                }
                if let Expr::Var(name) = &args[0] {
                    if self.is_array(name) {
                        let n = if self.in_func() && self.is_param(name) {
                            self.frames
                                .last()
                                .and_then(|f| f.arrays.get(name))
                                .map_or(0, |m| m.len())
                        } else {
                            self.arrays.get(name).map_or(0, |m| m.len())
                        };
                        return Value::Num(n as f64);
                    }
                }
                let s = self.eval(&args[0]).to_str(&conv);
                Value::Num(s.len() as f64)
            }
            Builtin::Substr => {
                let s = self.eval(&args[0]).to_str(&conv);
                let m = ftrunc(self.eval(&args[1]).to_num());
                let len = s.len() as i64;
                let start_pos = m as i64;
                let (from, to) = if args.len() >= 3 {
                    let n = ftrunc(self.eval(&args[2]).to_num()) as i64;
                    let end = start_pos.saturating_add(n);
                    (start_pos, end)
                } else {
                    (start_pos, len + 1)
                };
                let from = from.max(1);
                let to = to.min(len + 1).max(from);
                if from > len || to <= from {
                    Value::Str(Vec::new())
                } else {
                    Value::Str(s[(from - 1) as usize..(to - 1) as usize].to_vec())
                }
            }
            Builtin::Index => {
                let s = self.eval(&args[0]).to_str(&conv);
                let t = self.eval(&args[1]).to_str(&conv);
                if t.is_empty() {
                    return Value::Num(0.0);
                }
                for start in 0..=s.len().saturating_sub(t.len()) {
                    if s[start..].starts_with(&t) {
                        return Value::Num((start + 1) as f64);
                    }
                }
                Value::Num(0.0)
            }
            Builtin::Split => {
                let s = self.eval(&args[0]).to_str(&conv);
                let arr = match &args[1] {
                    Expr::Var(n) => n.clone(),
                    _ => return Value::Num(0.0),
                };
                let parts = if args.len() >= 3 {
                    if let Expr::Regex(re) = &args[2] {
                        if re.error {
                            self.runtime_error(b"awk: invalid regular expression\n");
                        }
                        split_by_regex(&s, re)
                    } else {
                        let fsv = self.eval(&args[2]).to_str(&conv);
                        split_fields(&s, &fsv)
                    }
                } else {
                    let fsv = self.fs();
                    split_fields(&s, &fsv)
                };
                let map = self.array_mut(&arr);
                map.clear();
                for (i, p) in parts.iter().enumerate() {
                    map.insert(i64_bytes((i + 1) as i64), mk_input_value(p.clone()));
                }
                Value::Num(parts.len() as f64)
            }
            Builtin::Sub => self.do_sub(args, false),
            Builtin::Gsub => self.do_sub(args, true),
            Builtin::Match => {
                let s = self.eval(&args[0]).to_str(&conv);
                let re = self.regex_from(&args[1]);
                match re_find(&re, &s, 0) {
                    Some((start, end)) => {
                        self.globals
                            .insert(b"RSTART".to_vec(), Value::Num((start + 1) as f64));
                        self.globals
                            .insert(b"RLENGTH".to_vec(), Value::Num((end - start) as f64));
                        Value::Num((start + 1) as f64)
                    }
                    None => {
                        self.globals.insert(b"RSTART".to_vec(), Value::Num(0.0));
                        self.globals.insert(b"RLENGTH".to_vec(), Value::Num(-1.0));
                        Value::Num(0.0)
                    }
                }
            }
            Builtin::Sprintf => {
                let fmt = self.eval(&args[0]).to_str(&conv);
                let mut vals = Vec::new();
                for a in &args[1..] {
                    vals.push(self.eval(a));
                }
                Value::Str(awk_sprintf(&fmt, &vals, &conv))
            }
            Builtin::Sin => Value::Num(fsin(self.eval(&args[0]).to_num())),
            Builtin::Cos => Value::Num(fcos(self.eval(&args[0]).to_num())),
            Builtin::Atan2 => {
                let y = self.eval(&args[0]).to_num();
                let x = self.eval(&args[1]).to_num();
                Value::Num(fatan2(y, x))
            }
            Builtin::Exp => Value::Num(fexp(self.eval(&args[0]).to_num())),
            Builtin::Log => Value::Num(fln(self.eval(&args[0]).to_num())),
            Builtin::Sqrt => Value::Num(fsqrt(self.eval(&args[0]).to_num())),
            Builtin::Int => Value::Num(ftrunc(self.eval(&args[0]).to_num())),
            Builtin::Rand => Value::Num(self.next_rand()),
            Builtin::Srand => {
                let prev = self.rand_prev_seed;
                let seed = if args.is_empty() {
                    unsafe { libc::time(core::ptr::null_mut()) as u64 }
                } else {
                    self.eval(&args[0]).to_num() as u64
                };
                self.rand_prev_seed = seed;
                self.rand_seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
                Value::Num(prev as f64)
            }
            Builtin::Tolower => {
                let mut s = self.eval(&args[0]).to_str(&conv);
                s.make_ascii_lowercase();
                Value::Str(s)
            }
            Builtin::Toupper => {
                let mut s = self.eval(&args[0]).to_str(&conv);
                s.make_ascii_uppercase();
                Value::Str(s)
            }
            Builtin::System => {
                self.flush();
                let cmd = self.eval(&args[0]).to_str(&conv);
                let mut c = cmd.clone();
                c.push(0);
                let r = unsafe { libc::system(c.as_ptr() as *const c_char) };
                Value::Num((r >> 8) as f64)
            }
            Builtin::Close => Value::Num(0.0),
            Builtin::Fflush => {
                self.flush();
                Value::Num(0.0)
            }
        }
    }

    fn do_sub(&mut self, args: &[Expr], global: bool) -> Value {
        let conv = self.convfmt();
        let re = self.regex_from(&args[0]);
        let repl = self.eval(&args[1]).to_str(&conv);
        let target_expr = if args.len() >= 3 {
            args[2].clone()
        } else {
            Expr::Field(Box::new(Expr::Num(0.0)))
        };
        let subject = self.eval(&target_expr).to_str(&conv);
        let (result, count) = regex_sub(&re, &repl, &subject, global);
        if count > 0 {
            self.do_assign(AssignOp::Set, &target_expr, Value::Str(result));
        }
        Value::Num(count as f64)
    }

    // --- statement execution ---
    fn exec_block(&mut self, stmts: &[Stmt]) -> Flow {
        for s in stmts {
            match self.exec_stmt(s) {
                Flow::Normal => {}
                other => return other,
            }
        }
        Flow::Normal
    }

    fn exec_stmt(&mut self, s: &Stmt) -> Flow {
        match s {
            Stmt::Block(b) => self.exec_block(b),
            Stmt::Expr(e) => {
                self.eval(e);
                Flow::Normal
            }
            Stmt::Print(args, _redir) => {
                self.do_print(args);
                Flow::Normal
            }
            Stmt::Printf(args, _redir) => {
                self.do_printf(args);
                Flow::Normal
            }
            Stmt::If(cond, then, els) => {
                if self.eval(cond).is_true() {
                    self.exec_stmt(then)
                } else if let Some(e) = els {
                    self.exec_stmt(e)
                } else {
                    Flow::Normal
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond).is_true() {
                    match self.exec_stmt(body) {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal => {}
                        other => return other,
                    }
                }
                Flow::Normal
            }
            Stmt::DoWhile(body, cond) => {
                loop {
                    match self.exec_stmt(body) {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal => {}
                        other => return other,
                    }
                    if !self.eval(cond).is_true() {
                        break;
                    }
                }
                Flow::Normal
            }
            Stmt::For(init, cond, post, body) => {
                if let Some(i) = init {
                    match self.exec_stmt(i) {
                        Flow::Normal => {}
                        other => return other,
                    }
                }
                loop {
                    if let Some(c) = cond {
                        if !self.eval(c).is_true() {
                            break;
                        }
                    }
                    match self.exec_stmt(body) {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal => {}
                        other => return other,
                    }
                    if let Some(p) = post {
                        match self.exec_stmt(p) {
                            Flow::Normal => {}
                            other => return other,
                        }
                    }
                }
                Flow::Normal
            }
            Stmt::ForIn(var, arr, body) => {
                let keys: Vec<Vec<u8>> = if self.in_func() && self.is_param(arr) {
                    self.frames
                        .last()
                        .and_then(|f| f.arrays.get(arr))
                        .map_or(Vec::new(), |m| m.keys().cloned().collect())
                } else {
                    self.arrays
                        .get(arr)
                        .map_or(Vec::new(), |m| m.keys().cloned().collect())
                };
                for k in keys {
                    self.set_var(var, mk_input_value(k));
                    match self.exec_stmt(body) {
                        Flow::Break => break,
                        Flow::Continue | Flow::Normal => {}
                        other => return other,
                    }
                }
                Flow::Normal
            }
            Stmt::Next => Flow::Next,
            Stmt::Exit(e) => {
                if let Some(e) = e {
                    self.exit_code = self.eval(e).to_num() as i32;
                }
                Flow::Exit
            }
            Stmt::Return(e) => {
                let v = e.as_ref().map(|x| self.eval(x)).unwrap_or(Value::Uninit);
                Flow::Return(v)
            }
            Stmt::Break => Flow::Break,
            Stmt::Continue => Flow::Continue,
            Stmt::Delete(name, subs) => {
                if subs.is_empty() {
                    if self.in_func() && self.is_param(name) {
                        if let Some(m) = self.frames.last_mut().unwrap().arrays.get_mut(name) {
                            m.clear();
                        }
                    } else if let Some(m) = self.arrays.get_mut(name) {
                        m.clear();
                    }
                } else {
                    let key = self.subscript(subs);
                    self.array_mut(name).remove(&key);
                }
                Flow::Normal
            }
        }
    }

    fn do_print(&mut self, args: &[Expr]) {
        let ofmt = self.ofmt();
        let ofs = self.ofs();
        let ors = self.ors();
        if args.is_empty() {
            let rec = self.record.clone();
            self.out.extend_from_slice(&rec);
            self.out.extend_from_slice(&ors);
            return;
        }
        let mut line = Vec::new();
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                line.extend_from_slice(&ofs);
            }
            let v = self.eval(a);
            line.extend_from_slice(&v.to_str(&ofmt));
        }
        line.extend_from_slice(&ors);
        self.out.extend_from_slice(&line);
    }

    fn do_printf(&mut self, args: &[Expr]) {
        if args.is_empty() {
            return;
        }
        let conv = self.convfmt();
        let fmt = self.eval(&args[0]).to_str(&conv);
        let mut vals = Vec::new();
        for a in &args[1..] {
            vals.push(self.eval(a));
        }
        let s = awk_sprintf(&fmt, &vals, &conv);
        self.out.extend_from_slice(&s);
    }
}

fn cmp_values(a: &Value, b: &Value, conv: &[u8]) -> core::cmp::Ordering {
    use core::cmp::Ordering::*;
    if a.is_numeric_c() && b.is_numeric_c() {
        let x = a.to_num();
        let y = b.to_num();
        if x < y {
            Less
        } else if x > y {
            Greater
        } else {
            Equal
        }
    } else {
        a.to_str(conv).cmp(&b.to_str(conv))
    }
}

// --- sub/gsub / field splitting -------------------------------------------

fn expand_repl(repl: &[u8], matched: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < repl.len() {
        let c = repl[i];
        if c == b'&' {
            out.extend_from_slice(matched);
            i += 1;
        } else if c == b'\\' && i + 1 < repl.len() {
            let n = repl[i + 1];
            if n == b'&' {
                out.push(b'&');
                i += 2;
            } else if n == b'\\' {
                out.push(b'\\');
                i += 2;
            } else {
                out.push(b'\\');
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
}

fn regex_sub(re: &Regex, repl: &[u8], subject: &[u8], global: bool) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut pos = 0;
    let mut count = 0;
    loop {
        match re_find(re, subject, pos) {
            Some((ms, me)) => {
                out.extend_from_slice(&subject[pos..ms]);
                expand_repl(repl, &subject[ms..me], &mut out);
                count += 1;
                if me > ms {
                    pos = me;
                } else {
                    // empty match: emit one char and advance to avoid a loop
                    if me < subject.len() {
                        out.push(subject[me]);
                    }
                    pos = me + 1;
                }
                if !global {
                    if pos <= subject.len() {
                        out.extend_from_slice(&subject[pos.min(subject.len())..]);
                    }
                    return (out, count);
                }
                if pos > subject.len() {
                    return (out, count);
                }
            }
            None => {
                out.extend_from_slice(&subject[pos.min(subject.len())..]);
                return (out, count);
            }
        }
    }
}

fn split_fields(s: &[u8], fs: &[u8]) -> Vec<Vec<u8>> {
    if s.is_empty() {
        return Vec::new();
    }
    if fs == b" " {
        // default: split on runs of blanks, trimming leading/trailing
        let mut out = Vec::new();
        let mut i = 0;
        while i < s.len() {
            while i < s.len() && (s[i] == b' ' || s[i] == b'\t' || s[i] == b'\n') {
                i += 1;
            }
            if i >= s.len() {
                break;
            }
            let start = i;
            while i < s.len() && s[i] != b' ' && s[i] != b'\t' && s[i] != b'\n' {
                i += 1;
            }
            out.push(s[start..i].to_vec());
        }
        return out;
    }
    if fs.len() == 1 {
        let sep = fs[0];
        let mut out = Vec::new();
        let mut start = 0;
        for i in 0..s.len() {
            if s[i] == sep {
                out.push(s[start..i].to_vec());
                start = i + 1;
            }
        }
        out.push(s[start..].to_vec());
        return out;
    }
    let re = compile_regex(fs);
    split_by_regex(s, &re)
}

fn split_by_regex(s: &[u8], re: &Regex) -> Vec<Vec<u8>> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        match re_find(re, s, pos) {
            Some((ms, me)) if me > ms => {
                out.push(s[pos..ms].to_vec());
                pos = me;
                if pos > s.len() {
                    break;
                }
            }
            _ => break,
        }
    }
    out.push(s[pos..].to_vec());
    out
}

// --- sprintf engine --------------------------------------------------------

fn awk_sprintf(fmt: &[u8], args: &[Value], conv: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut ai = 0usize;
    let next = |ai: &mut usize| -> Value {
        let v = args.get(*ai).cloned().unwrap_or(Value::Uninit);
        *ai += 1;
        v
    };
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c != b'%' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        if i >= fmt.len() {
            out.push(b'%');
            break;
        }
        if fmt[i] == b'%' {
            out.push(b'%');
            i += 1;
            continue;
        }
        let mut spec = vec![b'%'];
        while i < fmt.len() && matches!(fmt[i], b'-' | b'+' | b' ' | b'0' | b'#') {
            spec.push(fmt[i]);
            i += 1;
        }
        // width (clamped to keep the value in range for libc snprintf, where
        // out-of-int-range widths are undefined behaviour, and to bound the
        // amount of padding a single conversion can request)
        if i < fmt.len() && fmt[i] == b'*' {
            let mut w = next(&mut ai).to_num() as i64;
            w = w.clamp(-PRINTF_MAX_WIDTH, PRINTF_MAX_WIDTH);
            if w < 0 {
                spec.push(b'-');
                push_u(&mut spec, (-w) as u128);
            } else {
                push_u(&mut spec, w as u128);
            }
            i += 1;
        } else {
            let mut w: i64 = 0;
            let mut any = false;
            while i < fmt.len() && fmt[i].is_ascii_digit() {
                w = w.saturating_mul(10).saturating_add((fmt[i] - b'0') as i64);
                any = true;
                i += 1;
            }
            if any {
                if w > PRINTF_MAX_WIDTH {
                    w = PRINTF_MAX_WIDTH;
                }
                push_u(&mut spec, w as u128);
            }
        }
        // precision (also clamped)
        if i < fmt.len() && fmt[i] == b'.' {
            spec.push(b'.');
            i += 1;
            if i < fmt.len() && fmt[i] == b'*' {
                let mut p = next(&mut ai).to_num() as i64;
                p = p.clamp(0, PRINTF_MAX_WIDTH);
                push_u(&mut spec, p as u128);
                i += 1;
            } else {
                let mut p: i64 = 0;
                while i < fmt.len() && fmt[i].is_ascii_digit() {
                    p = p.saturating_mul(10).saturating_add((fmt[i] - b'0') as i64);
                    i += 1;
                }
                if p > PRINTF_MAX_WIDTH {
                    p = PRINTF_MAX_WIDTH;
                }
                push_u(&mut spec, p as u128);
            }
        }
        if i >= fmt.len() {
            out.extend_from_slice(&spec);
            break;
        }
        let convc = fmt[i];
        i += 1;
        match convc {
            b'd' | b'i' => {
                let v = next(&mut ai).to_num();
                spec.push(b'l');
                spec.push(b'd');
                out.extend_from_slice(&snp_long(&spec, v as c_long));
            }
            b'o' | b'x' | b'X' | b'u' => {
                let v = next(&mut ai).to_num();
                spec.push(b'l');
                spec.push(convc);
                out.extend_from_slice(&snp_ulong(&spec, v as i64 as c_ulong));
            }
            b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'a' | b'A' => {
                let v = next(&mut ai).to_num();
                spec.push(convc);
                out.extend_from_slice(&snp_double(&spec, v));
            }
            b'c' => {
                let val = next(&mut ai);
                let ch: Vec<u8> = match &val {
                    Value::Num(n) => vec![*n as i64 as u8],
                    other => {
                        let s = other.to_str(conv);
                        if s.is_empty() { Vec::new() } else { vec![s[0]] }
                    }
                };
                spec.push(b's');
                out.extend_from_slice(&snp_str(&spec, &ch));
            }
            b's' => {
                let s = next(&mut ai).to_str(conv);
                spec.push(b's');
                out.extend_from_slice(&snp_str(&spec, &s));
            }
            _ => {
                spec.push(convc);
                out.extend_from_slice(&spec);
            }
        }
    }
    out
}

// ===========================================================================
// Escape processing for -v / -F values and command-line assignments
// ===========================================================================

fn process_escapes(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\\' && i + 1 < s.len() {
            i += 1;
            match s[i] {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'r' => out.push(b'\r'),
                b'\\' => out.push(b'\\'),
                b'"' => out.push(b'"'),
                b'/' => out.push(b'/'),
                b'a' => out.push(0x07),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0c),
                b'v' => out.push(0x0b),
                b'0'..=b'7' => {
                    let mut val = 0u32;
                    let mut n = 0;
                    while n < 3 && i < s.len() && (b'0'..=b'7').contains(&s[i]) {
                        val = val * 8 + (s[i] - b'0') as u32;
                        i += 1;
                        n += 1;
                    }
                    out.push(val as u8);
                    continue;
                }
                other => {
                    out.push(b'\\');
                    out.push(other);
                }
            }
            i += 1;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

fn valid_var_name(s: &[u8]) -> bool {
    if s.is_empty() {
        return false;
    }
    if !(s[0] == b'_' || s[0].is_ascii_alphabetic()) {
        return false;
    }
    s.iter().all(|&c| c == b'_' || c.is_ascii_alphanumeric())
}

// ===========================================================================
// Entry point
// ===========================================================================

fn read_fd(fd: i32) -> Vec<u8> {
    io::read_all(fd)
}

fn read_file(path: &[u8]) -> Option<Vec<u8>> {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }
    let data = io::read_all(fd);
    io::close(fd);
    Some(data)
}

/// awk - pattern scanning and processing
///
/// # Synopsis
/// ```text
/// awk [-F sepstring] [-v assignment]... program [argument...]
/// awk [-F sepstring] -f progfile [-f progfile]... [-v assignment]... [argument...]
/// ```
///
/// # Description
/// A tree-walking interpreter for the POSIX awk language: patterns and
/// actions, BEGIN/END, records and fields, the full expression grammar,
/// control flow, associative arrays, user functions, and the standard
/// built-in functions, backed by a small ERE regex engine.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred (e.g. a syntax error or an unreadable file)
pub fn awk(argc: i32, argv: *const *const u8) -> i32 {
    let mut i = 1i32;
    let mut fs: Option<Vec<u8>> = None;
    let mut assigns: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut prog_files: Vec<Vec<u8>> = Vec::new();
    let program_text: Option<Vec<u8>> = None;

    // --- option parsing ---
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if arg == b"--" {
            i += 1;
            break;
        }
        if arg.len() >= 2 && arg[0] == b'-' && arg != b"-" {
            match arg[1] {
                b'F' => {
                    if arg.len() > 2 {
                        fs = Some(arg[2..].to_vec());
                    } else {
                        i += 1;
                        if let Some(v) = unsafe { get_arg(argv, i) } {
                            fs = Some(v.to_vec());
                        }
                    }
                }
                b'v' => {
                    let val = if arg.len() > 2 {
                        arg[2..].to_vec()
                    } else {
                        i += 1;
                        unsafe { get_arg(argv, i) }
                            .map(|v| v.to_vec())
                            .unwrap_or_default()
                    };
                    if let Some(eq) = val.iter().position(|&c| c == b'=') {
                        assigns.push((val[..eq].to_vec(), val[eq + 1..].to_vec()));
                    }
                }
                b'f' => {
                    let f = if arg.len() > 2 {
                        arg[2..].to_vec()
                    } else {
                        i += 1;
                        unsafe { get_arg(argv, i) }
                            .map(|v| v.to_vec())
                            .unwrap_or_default()
                    };
                    prog_files.push(f);
                }
                _ => {
                    // Unrecognized -X option: diagnose and exit rather than
                    // silently treating "-X" as the program text.
                    io::write_str(2, b"awk: unknown option ");
                    io::write_str(2, arg);
                    io::write_str(2, b"\n");
                    return 2;
                }
            }
            i += 1;
        } else {
            break;
        }
    }

    // --- program source ---
    let program: Vec<u8> = if !prog_files.is_empty() {
        let mut src = Vec::new();
        for pf in &prog_files {
            match read_file(pf) {
                Some(data) => {
                    src.extend_from_slice(&data);
                    src.push(b'\n');
                }
                None => {
                    io::write_str(2, b"awk: can't open program file\n");
                    return 2;
                }
            }
        }
        src
    } else {
        match unsafe { get_arg(argv, i) } {
            Some(p) => {
                i += 1;
                p.to_vec()
            }
            None => {
                io::write_str(2, b"awk: no program text\n");
                return 2;
            }
        }
    };

    // remaining operands
    let mut operands: Vec<Vec<u8>> = Vec::new();
    while i < argc {
        if let Some(a) = unsafe { get_arg(argv, i) } {
            operands.push(a.to_vec());
        }
        i += 1;
    }

    // --- lex + parse ---
    let (toks, lex_err) = Lexer::new(&program).run();
    if lex_err {
        io::write_str(2, b"awk: syntax error\n");
        return 2;
    }
    let mut parser = Parser::new(toks);
    parser.parse_program();
    if parser.error {
        io::write_str(2, b"awk: syntax error\n");
        return 2;
    }
    let rules = core::mem::take(&mut parser.rules);
    let funcs = core::mem::take(&mut parser.funcs);
    let nrules = rules.len();

    let mut it = Interp::new(funcs, nrules);

    // FS from -F
    if let Some(f) = fs {
        it.globals
            .insert(b"FS".to_vec(), Value::Str(process_escapes(&f)));
    }
    // -v assignments (before BEGIN)
    for (name, val) in &assigns {
        if valid_var_name(name) {
            it.set_var(name, mk_input_value(process_escapes(val)));
        }
    }

    let has_end = rules
        .iter()
        .any(|r| matches!(r.pattern, Some(Pattern::End)));
    let has_main = rules
        .iter()
        .any(|r| !matches!(r.pattern, Some(Pattern::Begin) | Some(Pattern::End)));

    // `exiting` tracks only whether `exit` fired in BEGIN: that skips the main
    // input loop but END must STILL run (POSIX). An `exit` inside the main loop
    // is handled by process_input returning early; END runs afterwards
    // regardless. An `exit` inside END is handled within the END loop below.
    let mut exiting = false;

    // --- BEGIN ---
    for r in &rules {
        if let Some(Pattern::Begin) = r.pattern {
            if let Some(action) = &r.action {
                if let Flow::Exit = it.exec_block(action) {
                    exiting = true;
                    break;
                }
            }
        }
    }

    // --- main input ---
    if !exiting && (has_main || has_end) {
        // Determine input files: operands that are not var=val assignments.
        let mut files: Vec<Vec<u8>> = Vec::new();
        let mut pending: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for op in &operands {
            if let Some(eq) = op.iter().position(|&c| c == b'=') {
                if valid_var_name(&op[..eq]) {
                    // record assignment position by count of preceding files
                    pending.push((op[..eq].to_vec(), op[eq + 1..].to_vec()));
                    continue;
                }
            }
            files.push(op.clone());
        }
        // Apply any leading assignments that come before first file are handled
        // inline below; for simplicity apply all pending assignments now if
        // there are no files, else interleave in order.
        if files.is_empty() {
            for (n, v) in &pending {
                it.set_var(n, mk_input_value(process_escapes(v)));
            }
            it.globals
                .insert(b"FILENAME".to_vec(), Value::Str(Vec::new()));
            let data = read_fd(0);
            // An exit here stops input, but END still runs below.
            process_input(&mut it, &rules, &data);
        } else {
            // Re-walk operands in order to interleave assignments with files.
            'outer: for op in &operands {
                if let Some(eq) = op.iter().position(|&c| c == b'=') {
                    if valid_var_name(&op[..eq]) {
                        it.set_var(&op[..eq], mk_input_value(process_escapes(&op[eq + 1..])));
                        continue;
                    }
                }
                let data = if op == b"-" {
                    read_fd(0)
                } else {
                    match read_file(op) {
                        Some(d) => d,
                        None => {
                            io::write_str(2, b"awk: can't open file ");
                            io::write_str(2, op);
                            io::write_str(2, b"\n");
                            it.exit_code = 2;
                            continue;
                        }
                    }
                };
                it.globals
                    .insert(b"FILENAME".to_vec(), Value::Str(op.clone()));
                it.fnr = 0;
                if process_input(&mut it, &rules, &data) {
                    // exit fired mid-input: stop reading files, but run END.
                    break 'outer;
                }
            }
        }
    }

    // --- END (runs even after an `exit` in BEGIN or the main loop) ---
    if has_end {
        for r in &rules {
            if let Some(Pattern::End) = r.pattern {
                if let Some(action) = &r.action {
                    if let Flow::Exit = it.exec_block(action) {
                        break;
                    }
                }
            }
        }
    }

    it.flush();
    it.exit_code
}

/// Process one input source (already read into `data`). Returns true if an
/// `exit` statement fired (stop reading further input).
fn process_input(it: &mut Interp, rules: &[Rule], data: &[u8]) -> bool {
    let rs = it.raw_str(b"RS", b"\n");
    let records = split_records(data, &rs);
    for rec in records {
        it.nr += 1;
        it.fnr += 1;
        it.set_record(rec);
        match run_rules(it, rules) {
            Flow::Exit => return true,
            _ => {}
        }
    }
    false
}

fn run_rules(it: &mut Interp, rules: &[Rule]) -> Flow {
    for (idx, r) in rules.iter().enumerate() {
        let run = match &r.pattern {
            None => true,
            Some(Pattern::Begin) | Some(Pattern::End) => false,
            Some(Pattern::Expr(e)) => it.eval(e).is_true(),
            Some(Pattern::Range(a, b)) => {
                if it.range_active[idx] {
                    if it.eval(b).is_true() {
                        it.range_active[idx] = false;
                    }
                    true
                } else if it.eval(a).is_true() {
                    it.range_active[idx] = !it.eval(b).is_true();
                    true
                } else {
                    false
                }
            }
        };
        if !run {
            continue;
        }
        match &r.action {
            None => it.do_print(&[]),
            Some(stmts) => match it.exec_block(stmts) {
                Flow::Next => return Flow::Normal,
                Flow::Exit => return Flow::Exit,
                _ => {}
            },
        }
    }
    Flow::Normal
}

/// Split raw bytes into records using a single-character RS (default "\n").
fn split_records(data: &[u8], rs: &[u8]) -> Vec<Vec<u8>> {
    if data.is_empty() {
        return Vec::new();
    }
    // Paragraph mode: RS == ""
    if rs.is_empty() {
        let mut out = Vec::new();
        let mut i = 0;
        while i < data.len() {
            // skip leading blank lines
            while i + 1 < data.len() && data[i] == b'\n' {
                i += 1;
            }
            if i >= data.len() {
                break;
            }
            let start = i;
            // read until blank line
            while i < data.len() {
                if data[i] == b'\n' && i + 1 < data.len() && data[i + 1] == b'\n' {
                    break;
                }
                i += 1;
            }
            let mut end = i;
            if end > start && data[end.saturating_sub(1)] == b'\n' {
                end -= 1;
            }
            out.push(data[start..end].to_vec());
            i += 1;
        }
        return out;
    }
    let sep = rs[0];
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..data.len() {
        if data[i] == sep {
            out.push(data[start..i].to_vec());
            start = i + 1;
        }
    }
    if start < data.len() {
        out.push(data[start..].to_vec());
    }
    out
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_awk_print_all() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["awk", "{print}"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello world\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "hello world\n");
    }

    #[test]
    fn test_awk_print_field() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["awk", "{print $2}"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"one two three\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "two\n");
    }

    #[test]
    fn test_awk_print_first_field() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["awk", "{print $1}"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(b"alpha beta gamma\ndelta epsilon\n")
                .unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["alpha", "delta"]);
    }

    #[test]
    fn test_awk_print_zero() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["awk", "{print $0}"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"entire line\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "entire line\n");
    }
}
