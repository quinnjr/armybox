//! find - search for files in a directory hierarchy
//!
//! POSIX.1-2017 compatible implementation.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::vec::Vec;

/// A parsed predicate (leaf of the expression tree).
enum Pred {
    /// `-name PATTERN` — basename matches shell glob (`*`, `?`, `[..]`).
    Name(&'static [u8]),
    /// `-type c` — file is of the given type character.
    Type(u8),
    /// `-print` — print the path (action).
    Print,
    /// `-prune` — do not descend into this directory.
    Prune,
    /// `-exec CMD ... ;` — run command per match, substituting `{}` (action).
    Exec(Vec<&'static [u8]>),
    /// `-newer FILE` — modified more recently than the reference (sec, nsec).
    Newer(i64, i64),
    /// `-size N[ckMG...]` — size comparison: (cmp, unit, count).
    Size { cmp: i8, unit: u64, count: u64 },
    /// `-perm [-/]MODE` — permission bits.
    Perm { mode: u32, kind: u8 },
    /// Matches everything (empty expression).
    True,
}

/// Expression tree with POSIX operator precedence: `!` > implicit/`-a` > `-o`.
enum Expr {
    Pred(Pred),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

/// Global traversal configuration (from `-depth` / `-maxdepth`).
struct Cfg {
    depth_first: bool,
    maxdepth: usize,
}

/// Hard cap on real directory recursion depth. Symlink cycles are already
/// avoided via `lstat`; this only bounds genuinely deep trees so a pathological
/// hierarchy can't exhaust the stack and crash (SIGSEGV).
const MAX_DEPTH: usize = 4096;

/// find - search for files in a directory hierarchy
///
/// # Synopsis
/// ```text
/// find [path...] [expression]
/// ```
///
/// # Exit Status
/// - 0: Success
/// - 1: A runtime error occurred (e.g. an unreadable directory)
/// - 2: Usage error (e.g. an unknown predicate)
pub fn find(argc: i32, argv: *const *const u8) -> i32 {
    // Collect leading path operands: everything up to the first token that
    // begins with '-' or is '!' / '(' starts the expression.
    let mut paths: Vec<&'static [u8]> = Vec::new();
    let mut i = 1i32;
    while i < argc {
        let a = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if !a.is_empty() && a[0] == b'-' {
            break;
        }
        if a == b"!" || a == b"(" {
            break;
        }
        paths.push(a);
        i += 1;
    }
    if paths.is_empty() {
        paths.push(b".");
    }

    // Pre-pass over expression tokens: extract global options (-depth,
    // -maxdepth) and gather the remaining predicate/operator tokens.
    let mut cfg = Cfg {
        depth_first: false,
        maxdepth: usize::MAX,
    };
    let mut toks: Vec<&'static [u8]> = Vec::new();
    while i < argc {
        let a = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if a == b"-depth" {
            cfg.depth_first = true;
            i += 1;
            continue;
        }
        if a == b"-maxdepth" {
            i += 1;
            let n = match unsafe { get_arg(argv, i) }.and_then(parse_usize) {
                Some(n) => n,
                None => {
                    io::write_str(2, b"find: invalid argument to -maxdepth\n");
                    return 2;
                }
            };
            cfg.maxdepth = n;
            i += 1;
            continue;
        }
        toks.push(a);
        i += 1;
    }

    // Parse the expression.
    let mut parser = Parser {
        toks: &toks,
        pos: 0,
        has_action: false,
        error: false,
    };
    let expr = match parser.parse() {
        Some(e) if !parser.error && parser.pos == toks.len() => e,
        _ => {
            io::write_str(2, b"find: invalid expression\n");
            return 2;
        }
    };

    // POSIX: if no action is given, the whole expression is `( expr ) -a -print`.
    let expr = if parser.has_action {
        expr
    } else {
        Expr::And(Box::new(expr), Box::new(Expr::Pred(Pred::Print)))
    };

    let mut ret = 0i32;
    let mut abort = false;
    for p in &paths {
        if abort {
            break;
        }
        process(p, basename(p), 0, &cfg, &expr, &mut ret, &mut abort);
    }
    ret
}

/// Recursively process a single path entry.
fn process(
    path: &[u8],
    name: &[u8],
    depth: usize,
    cfg: &Cfg,
    expr: &Expr,
    ret: &mut i32,
    abort: &mut bool,
) {
    let mut st = io::stat_zeroed();
    if io::lstat(path, &mut st) < 0 {
        diag(path);
        *ret = 1;
        return;
    }

    let is_dir = (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32;
    let mut can_descend = is_dir && depth < cfg.maxdepth;

    // Bound genuine recursion depth so a pathologically deep tree stops the
    // descent with a diagnostic and a nonzero exit instead of overflowing the
    // stack.
    if can_descend && depth >= MAX_DEPTH {
        io::write_str(2, b"find: '");
        io::write_all(2, path);
        io::write_str(2, b"': directory too deep\n");
        *ret = 1;
        can_descend = false;
    }

    if cfg.depth_first {
        if can_descend {
            descend(path, depth, cfg, expr, ret, abort);
        }
        if *abort {
            return;
        }
        let mut prune = false;
        eval(expr, path, name, &st, &mut prune, ret, abort);
    } else {
        let mut prune = false;
        eval(expr, path, name, &st, &mut prune, ret, abort);
        if can_descend && !prune && !*abort {
            descend(path, depth, cfg, expr, ret, abort);
        }
    }
}

/// Read a directory and process each child.
fn descend(path: &[u8], depth: usize, cfg: &Cfg, expr: &Expr, ret: &mut i32, abort: &mut bool) {
    let fd = io::open(path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        diag(path);
        *ret = 1;
        return;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n < 0 {
            diag(path);
            *ret = 1;
            break;
        }
        if n == 0 {
            break;
        }

        let mut offset = 0usize;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };

            if name != b"." && name != b".." {
                // Build the child path on the heap: no fixed-size buffer, so
                // arbitrarily deep trees can never overflow or panic.
                let mut child = Vec::with_capacity(path.len() + 1 + name.len());
                child.extend_from_slice(path);
                if !path.is_empty() && path[path.len() - 1] != b'/' {
                    child.push(b'/');
                }
                child.extend_from_slice(name);

                process(&child, name, depth + 1, cfg, expr, ret, abort);
                if *abort {
                    break;
                }
            }

            offset += dirent.d_reclen as usize;
        }
        if *abort {
            break;
        }
    }
    io::close(fd);
}

/// Evaluate the expression tree against one entry (with short-circuiting so
/// action side effects only fire when POSIX says they should).
fn eval(
    expr: &Expr,
    path: &[u8],
    name: &[u8],
    st: &libc::stat,
    prune: &mut bool,
    ret: &mut i32,
    abort: &mut bool,
) -> bool {
    match expr {
        Expr::And(a, b) => {
            if eval(a, path, name, st, prune, ret, abort) {
                eval(b, path, name, st, prune, ret, abort)
            } else {
                false
            }
        }
        Expr::Or(a, b) => {
            if eval(a, path, name, st, prune, ret, abort) {
                true
            } else {
                eval(b, path, name, st, prune, ret, abort)
            }
        }
        Expr::Not(a) => !eval(a, path, name, st, prune, ret, abort),
        Expr::Pred(p) => eval_pred(p, path, name, st, prune, ret, abort),
    }
}

fn eval_pred(
    p: &Pred,
    path: &[u8],
    name: &[u8],
    st: &libc::stat,
    prune: &mut bool,
    ret: &mut i32,
    abort: &mut bool,
) -> bool {
    match p {
        Pred::True => true,
        Pred::Name(pat) => glob_match(pat, name),
        Pred::Type(c) => type_match(*c, st.st_mode as libc::mode_t),
        Pred::Print => {
            // A failed stdout write (e.g. EPIPE from `find | head`) must set a
            // nonzero exit and stop the traversal rather than silently exit 0.
            if io::write_all(1, path) < 0 || io::write_str(1, b"\n") < 0 {
                *ret = 1;
                *abort = true;
            }
            true
        }
        Pred::Prune => {
            *prune = true;
            true
        }
        Pred::Exec(cmd) => run_exec(cmd, path, ret),
        Pred::Newer(sec, nsec) => {
            let e = (st.st_mtime as i64, st.st_mtime_nsec as i64);
            e > (*sec, *nsec)
        }
        Pred::Size { cmp, unit, count } => {
            let bytes = if st.st_size < 0 {
                0u64
            } else {
                st.st_size as u64
            };
            // Round the size up to whole units (POSIX/GNU semantics).
            let units = if *unit <= 1 {
                bytes
            } else {
                (bytes + *unit - 1) / *unit
            };
            match cmp {
                1 => units > *count,
                -1 => units < *count,
                _ => units == *count,
            }
        }
        Pred::Perm { mode, kind } => {
            let bits = st.st_mode & 0o7777;
            match kind {
                1 => (bits & mode) == *mode,           // -MODE : all listed bits set
                2 => *mode == 0 || (bits & mode) != 0, // /MODE : any listed bit set
                _ => bits == *mode,                    // MODE : exact
            }
        }
    }
}

/// Match `-type` character against `st_mode`.
fn type_match(c: u8, mode: libc::mode_t) -> bool {
    let fmt = mode & libc::S_IFMT;
    match c {
        b'f' => fmt == libc::S_IFREG,
        b'd' => fmt == libc::S_IFDIR,
        b'l' => fmt == libc::S_IFLNK,
        b'b' => fmt == libc::S_IFBLK,
        b'c' => fmt == libc::S_IFCHR,
        b'p' => fmt == libc::S_IFIFO,
        b's' => fmt == libc::S_IFSOCK,
        _ => false,
    }
}

/// Fork and exec `-exec` command with `{}` substituted for `path`.
/// Returns true when the command exits 0 (POSIX).
fn run_exec(template: &[&[u8]], path: &[u8], ret: &mut i32) -> bool {
    let mut args: Vec<Vec<u8>> = Vec::with_capacity(template.len());
    for t in template {
        let mut v = Vec::with_capacity(t.len());
        let mut idx = 0;
        while idx < t.len() {
            if t[idx..].starts_with(b"{}") {
                v.extend_from_slice(path);
                idx += 2;
            } else {
                v.push(t[idx]);
                idx += 1;
            }
        }
        args.push(v);
    }

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        io::write_str(2, b"find: fork failed\n");
        *ret = 1;
        return false;
    }
    if pid == 0 {
        // Child.
        let mut cstrings: Vec<CString> = Vec::with_capacity(args.len());
        for a in &args {
            let mut v = Vec::with_capacity(a.len() + 1);
            v.extend_from_slice(a);
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
        if !ptrs.is_empty() {
            unsafe {
                libc::execvp(ptrs[0], ptrs.as_ptr());
            }
        }
        io::write_str(2, b"find: exec failed\n");
        unsafe {
            libc::_exit(127);
        }
    }

    let mut status = 0i32;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }
    let ok = libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
    if !ok {
        *ret = 1;
    }
    ok
}

/// Write a `find: 'PATH': strerror` diagnostic to stderr.
fn diag(path: &[u8]) {
    let errno = crate::sys::errno();
    io::write_str(2, b"find: '");
    io::write_all(2, path);
    io::write_str(2, b"': ");
    let s = unsafe { libc::strerror(errno) };
    if !s.is_null() {
        let slice = unsafe { io::cstr_to_slice(s as *const u8) };
        io::write_all(2, slice);
    }
    io::write_str(2, b"\n");
}

/// Return the basename (final component) of a path.
fn basename(p: &[u8]) -> &[u8] {
    match p.iter().rposition(|&c| c == b'/') {
        Some(idx) if idx + 1 < p.len() => &p[idx + 1..],
        Some(_) => p, // trailing slash or "/" — keep as-is
        None => p,
    }
}

/// Parse a base-10 unsigned integer.
fn parse_usize(b: &[u8]) -> Option<usize> {
    if b.is_empty() {
        return None;
    }
    let mut n: usize = 0;
    for &c in b {
        if !c.is_ascii_digit() {
            return None;
        }
        n = n.checked_mul(10)?.checked_add((c - b'0') as usize)?;
    }
    Some(n)
}

// ---------------------------------------------------------------------------
// Glob matching (shell filename patterns: '*', '?', '[..]').
// ---------------------------------------------------------------------------

/// Iterative two-pointer glob matcher. A single remembered backtrack point for
/// the most recent `*` collapses the search to O(pat * name) time, so adversarial
/// patterns like `a*a*a*...` can never trigger catastrophic (exponential)
/// backtracking as the previous recursive matcher could.
fn glob_match(pat: &[u8], name: &[u8]) -> bool {
    let mut p = 0usize; // current index into pat
    let mut s = 0usize; // current index into name
    let mut star_p: Option<usize> = None; // pat index of the last '*' seen
    let mut star_s = 0usize; // name index when that '*' was matched

    while s < name.len() {
        let mut matched_here = false;
        let mut pat_adv = 0usize;
        if p < pat.len() {
            match pat[p] {
                b'*' => {
                    // Record the backtrack point and consume the '*'; it can
                    // still absorb more of `name` on a later mismatch.
                    star_p = Some(p);
                    star_s = s;
                    p += 1;
                    continue;
                }
                b'?' => {
                    matched_here = true;
                    pat_adv = 1;
                }
                b'[' => match match_bracket(&pat[p..], name[s]) {
                    Some((m, next)) => {
                        matched_here = m;
                        pat_adv = next;
                    }
                    None => {
                        // No closing bracket — treat '[' as a literal.
                        matched_here = name[s] == b'[';
                        pat_adv = 1;
                    }
                },
                c => {
                    matched_here = name[s] == c;
                    pat_adv = 1;
                }
            }
        }

        if matched_here {
            p += pat_adv;
            s += 1;
        } else if let Some(sp) = star_p {
            // Backtrack: let the last '*' swallow one more character.
            p = sp + 1;
            star_s += 1;
            s = star_s;
        } else {
            return false;
        }
    }

    // Name exhausted: any remaining pattern must be all '*' to match.
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

/// Match a `[..]` bracket expression against `ch`.
/// Returns `(matched, index_after_class)` or `None` if unterminated.
fn match_bracket(pat: &[u8], ch: u8) -> Option<(bool, usize)> {
    // pat[0] == '['
    let mut i = 1;
    let mut negate = false;
    if i < pat.len() && (pat[i] == b'!' || pat[i] == b'^') {
        negate = true;
        i += 1;
    }
    let start = i;
    let mut matched = false;
    loop {
        if i >= pat.len() {
            return None;
        }
        if pat[i] == b']' && i > start {
            break;
        }
        if i + 2 < pat.len() && pat[i + 1] == b'-' && pat[i + 2] != b']' {
            let lo = pat[i];
            let hi = pat[i + 2];
            if ch >= lo && ch <= hi {
                matched = true;
            }
            i += 3;
        } else {
            if pat[i] == ch {
                matched = true;
            }
            i += 1;
        }
    }
    Some((matched ^ negate, i + 1))
}

// ---------------------------------------------------------------------------
// Expression parser (recursive descent).
// ---------------------------------------------------------------------------

struct Parser<'a> {
    toks: &'a [&'static [u8]],
    pos: usize,
    has_action: bool,
    error: bool,
}

impl<'a> Parser<'a> {
    fn parse(&mut self) -> Option<Expr> {
        if self.toks.is_empty() {
            return Some(Expr::Pred(Pred::True));
        }
        self.or_expr()
    }

    fn peek(&self) -> Option<&'static [u8]> {
        self.toks.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<&'static [u8]> {
        let t = self.toks.get(self.pos).copied();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn fail(&mut self) -> Option<Expr> {
        self.error = true;
        None
    }

    fn or_expr(&mut self) -> Option<Expr> {
        let mut left = self.and_expr()?;
        while matches!(self.peek(), Some(t) if t == b"-o" || t == b"-or") {
            self.pos += 1;
            let right = self.and_expr()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn and_expr(&mut self) -> Option<Expr> {
        let mut left = self.not_expr()?;
        loop {
            match self.peek() {
                Some(t) if t == b"-a" || t == b"-and" => {
                    self.pos += 1;
                    let right = self.not_expr()?;
                    left = Expr::And(Box::new(left), Box::new(right));
                }
                Some(t) if t == b"-o" || t == b"-or" || t == b")" => break,
                None => break,
                Some(_) => {
                    // Implicit AND between adjacent predicates.
                    let right = self.not_expr()?;
                    left = Expr::And(Box::new(left), Box::new(right));
                }
            }
        }
        Some(left)
    }

    fn not_expr(&mut self) -> Option<Expr> {
        if matches!(self.peek(), Some(t) if t == b"!" || t == b"-not") {
            self.pos += 1;
            let e = self.not_expr()?;
            return Some(Expr::Not(Box::new(e)));
        }
        self.term()
    }

    fn term(&mut self) -> Option<Expr> {
        match self.peek() {
            Some(t) if t == b"(" => {
                self.pos += 1;
                let e = self.or_expr()?;
                if self.peek() != Some(b")".as_slice()) {
                    return self.fail();
                }
                self.pos += 1;
                Some(e)
            }
            Some(_) => self.predicate(),
            None => self.fail(),
        }
    }

    fn predicate(&mut self) -> Option<Expr> {
        let t = self.next()?;
        let pred = if t == b"-name" {
            Pred::Name(self.next()?)
        } else if t == b"-type" {
            let a = self.next()?;
            if a.len() != 1 {
                return self.fail();
            }
            Pred::Type(a[0])
        } else if t == b"-print" {
            self.has_action = true;
            Pred::Print
        } else if t == b"-prune" {
            Pred::Prune
        } else if t == b"-exec" {
            let mut cmd: Vec<&'static [u8]> = Vec::new();
            loop {
                match self.next() {
                    Some(x) if x == b";" || x == b"\\;" => break,
                    Some(x) => cmd.push(x),
                    None => return self.fail(),
                }
            }
            if cmd.is_empty() {
                return self.fail();
            }
            self.has_action = true;
            Pred::Exec(cmd)
        } else if t == b"-newer" {
            let f = self.next()?;
            let mut st = io::stat_zeroed();
            if io::stat(f, &mut st) < 0 {
                return self.fail();
            }
            Pred::Newer(st.st_mtime as i64, st.st_mtime_nsec as i64)
        } else if t == b"-size" {
            match parse_size(self.next()?) {
                Some(p) => p,
                None => return self.fail(),
            }
        } else if t == b"-perm" {
            match parse_perm(self.next()?) {
                Some(p) => p,
                None => return self.fail(),
            }
        } else {
            // Unknown predicate — usage error.
            return self.fail();
        };
        Some(Expr::Pred(pred))
    }
}

/// Parse a `-size` argument: `[+-]N[bcwkMG]`.
fn parse_size(b: &[u8]) -> Option<Pred> {
    if b.is_empty() {
        return None;
    }
    let mut i = 0;
    let cmp: i8 = match b[0] {
        b'+' => {
            i += 1;
            1
        }
        b'-' => {
            i += 1;
            -1
        }
        _ => 0,
    };
    let num_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == num_start {
        return None;
    }
    let count = parse_usize(&b[num_start..i])? as u64;
    // Suffix determines the unit size in bytes; default is 512-byte blocks.
    let unit: u64 = if i == b.len() {
        512
    } else if i + 1 == b.len() {
        match b[i] {
            b'b' => 512,
            b'c' => 1,
            b'w' => 2,
            b'k' => 1024,
            b'M' => 1024 * 1024,
            b'G' => 1024 * 1024 * 1024,
            _ => return None,
        }
    } else {
        return None;
    };
    Some(Pred::Size { cmp, unit, count })
}

/// Parse a `-perm` argument: `MODE`, `-MODE`, or `/MODE` (octal).
fn parse_perm(b: &[u8]) -> Option<Pred> {
    if b.is_empty() {
        return None;
    }
    let (kind, digits) = match b[0] {
        b'-' => (1u8, &b[1..]),
        b'/' => (2u8, &b[1..]),
        _ => (0u8, b),
    };
    if digits.is_empty() {
        return None;
    }
    let mut mode: u32 = 0;
    for &c in digits {
        if !(b'0'..=b'7').contains(&c) {
            return None;
        }
        mode = mode * 8 + (c - b'0') as u32;
    }
    Some(Pred::Perm { mode, kind })
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_find_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_find_all() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "").unwrap();
        fs::write(dir.join("file2.txt"), "").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();
        fs::write(dir.join("subdir/file3.txt"), "").unwrap();

        let output = Command::new(&armybox)
            .args(["find", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("file1.txt"));
        assert!(stdout.contains("file2.txt"));
        assert!(stdout.contains("file3.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_find_by_name() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "").unwrap();
        fs::write(dir.join("file2.rs"), "").unwrap();

        let output = Command::new(&armybox)
            .args(["find", dir.to_str().unwrap(), "-name", "*.txt"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("file1.txt"));
        assert!(!stdout.contains("file2.rs"));
        cleanup(&dir);
    }

    #[test]
    fn test_find_by_type() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file.txt"), "").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let output = Command::new(&armybox)
            .args(["find", dir.to_str().unwrap(), "-type", "d"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("subdir"));
        assert!(!stdout.contains("file.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_find_default_path() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["find"]).output().unwrap();

        // Should search current directory
        assert_eq!(output.status.code(), Some(0));
    }
}
