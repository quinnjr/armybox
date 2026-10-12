//! Shared BRE/ERE regex engine for `grep` and `sed`.
//!
//! A self-contained, `#[no_std]` (alloc) regex engine supporting POSIX Basic
//! Regular Expressions (BRE, default) and Extended Regular Expressions (ERE).
//! It is a capture-capable bytecode VM: the same compiled program serves both
//! grep's boolean search (with `-w` word and `-x` whole-line match modes) and
//! sed's leftmost search with capture-group spans (for `\1`..`\9` and `&` in
//! the replacement).
//!
//! The matcher is an **iterative priority-thread Pike VM** (a Thompson NFA
//! simulation with capture tracking). It runs in `O(program × text)` time and
//! `O(program)` space with no recursion whose depth is tied to the input
//! length, so it cannot overflow the stack on a very long line and has no
//! exponential-backtracking blow-up. Threads are kept in strict backtracking
//! priority order — the higher-priority alternative / greedy-repeat branch is
//! explored first, and the first thread to reach `Match` wins — which
//! reproduces the exact leftmost-greedy submatch results of a classic
//! backtracker (as opposed to POSIX leftmost-longest, which would differ on
//! e.g. `a|ab` against "ab"). Empty-loop safety (`\(a*\)*`) comes from a
//! per-input-position dedup set: each program counter is added to the thread
//! list at most once per position, so nullable stars terminate.
//!
//! Hardening (all covered by the grep/sed test suites):
//!   * `{n,m}` counts are capped at [`RE_DUP_MAX`] with a compile *error* (never
//!     a panic) on overflow or inverted bounds, and a [`MAX_PROG_LEN`] backstop
//!     on total interval-expanded program size.
//!   * A parser nesting cap ([`MAX_NEST`]) bounds group depth.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

/// POSIX RE_DUP_MAX: maximum permitted count in a `{n,m}` interval.
const RE_DUP_MAX: usize = 32767;
/// Backstop cap on the total number of compiled VM instructions. Interval
/// expansion multiplies instruction counts, so nested bounds are capped here
/// even when each individual bound is within `RE_DUP_MAX`.
const MAX_PROG_LEN: usize = 1 << 20;
/// Maximum parser nesting depth for groups (parse-error backstop).
const MAX_NEST: usize = 1000;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Regex dialect / flags selected at compile time.
#[derive(Clone, Copy)]
pub(crate) struct Syntax {
    /// Extended Regular Expressions (`-E`/`-r`) when true; Basic otherwise.
    pub ere: bool,
    /// Case-insensitive matching.
    pub icase: bool,
    /// Translate `\n`/`\t`/`\r` in the pattern to the control byte (sed) rather
    /// than to the literal following character (grep). This is the one behavior
    /// that genuinely differs between the two callers.
    pub translate_escapes: bool,
}

/// A successful match: the overall span (slots 0/1) plus capture-group spans.
pub(crate) struct Captures {
    slots: Vec<Option<usize>>,
}

impl Captures {
    /// Start offset of the overall match.
    pub fn start(&self) -> usize {
        self.slots[0].unwrap()
    }
    /// End offset (exclusive) of the overall match.
    pub fn end(&self) -> usize {
        self.slots[1].unwrap()
    }
    /// The overall matched slice (`&` in a sed replacement).
    pub fn whole<'t>(&self, text: &'t [u8]) -> &'t [u8] {
        &text[self.start()..self.end()]
    }
    /// The slice captured by group `n` (`\n` in a sed replacement), if any.
    /// Group `0` is the whole match.
    pub fn group<'t>(&self, n: usize, text: &'t [u8]) -> Option<&'t [u8]> {
        let i = 2 * n;
        if i + 1 < self.slots.len() {
            if let (Some(s), Some(e)) = (self.slots[i], self.slots[i + 1]) {
                return Some(&text[s..e]);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Ast {
    Empty,
    Lit(u8),
    Any,
    Class([u8; 32], bool), // membership bitmap, negate flag
    Start,
    End,
    Concat(Vec<Ast>),
    Alt(Vec<Ast>),
    Group(usize, Box<Ast>),
    Star(Box<Ast>),
    Plus(Box<Ast>),
    Quest(Box<Ast>),
    Repeat(Box<Ast>, usize, Option<usize>),
}

// ---------------------------------------------------------------------------
// Token helpers (BRE/ERE dialect differences)
// ---------------------------------------------------------------------------

fn is_alt_op(b: &[u8], p: usize, ere: bool) -> bool {
    p < b.len()
        && ((ere && b[p] == b'|') || (!ere && b[p] == b'\\' && p + 1 < b.len() && b[p + 1] == b'|'))
}

fn is_group_close(b: &[u8], p: usize, ere: bool) -> bool {
    p < b.len()
        && ((ere && b[p] == b')') || (!ere && b[p] == b'\\' && p + 1 < b.len() && b[p + 1] == b')'))
}

fn is_group_open(b: &[u8], p: usize, ere: bool) -> bool {
    p < b.len()
        && ((ere && b[p] == b'(') || (!ere && b[p] == b'\\' && p + 1 < b.len() && b[p + 1] == b'('))
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    b: &'a [u8],
    pos: usize,
    ere: bool,
    translate: bool,
    gc: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn alt(&mut self) -> Result<Ast, ()> {
        let mut br: Vec<Ast> = Vec::new();
        br.push(self.concat()?);
        while is_alt_op(self.b, self.pos, self.ere) {
            self.pos += if self.ere { 1 } else { 2 };
            br.push(self.concat()?);
        }
        Ok(if br.len() == 1 {
            br.pop().unwrap()
        } else {
            Ast::Alt(br)
        })
    }

    /// End-of-branch context for a `$` anchor in BRE: end of pattern, or
    /// immediately before `\)` / `\|`.
    fn bre_dollar_is_anchor(&self) -> bool {
        let nx = self.pos + 1;
        nx >= self.b.len() || is_alt_op(self.b, nx, false) || is_group_close(self.b, nx, false)
    }

    fn concat(&mut self) -> Result<Ast, ()> {
        let mut items: Vec<Ast> = Vec::new();
        let mut first = true;
        while self.pos < self.b.len()
            && !is_alt_op(self.b, self.pos, self.ere)
            && !is_group_close(self.b, self.pos, self.ere)
        {
            let c = self.b[self.pos];
            // '^' anchor: always in ERE; only at branch start in BRE.
            if c == b'^' && (self.ere || first) {
                self.pos += 1;
                items.push(Ast::Start);
                first = false;
                continue;
            }
            // '$' anchor: always in ERE; only at branch end in BRE.
            if c == b'$' && (self.ere || self.bre_dollar_is_anchor()) {
                self.pos += 1;
                items.push(Ast::End);
                first = false;
                continue;
            }
            let a = self.atom()?;
            let a = self.quant(a)?;
            items.push(a);
            first = false;
        }
        Ok(if items.is_empty() {
            Ast::Empty
        } else if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Ast::Concat(items)
        })
    }

    fn atom(&mut self) -> Result<Ast, ()> {
        let b = self.b;
        if is_group_open(b, self.pos, self.ere) {
            self.pos += if self.ere { 1 } else { 2 };
            self.depth += 1;
            if self.depth > MAX_NEST {
                return Err(());
            }
            self.gc += 1;
            let idx = self.gc;
            let inner = self.alt()?;
            if is_group_close(b, self.pos, self.ere) {
                self.pos += if self.ere { 1 } else { 2 };
            } else {
                // Unterminated group: `\(` (BRE) or `(` (ERE) with no close.
                return Err(());
            }
            self.depth -= 1;
            return Ok(Ast::Group(idx, Box::new(inner)));
        }
        let c = b[self.pos];
        if c == b'.' {
            self.pos += 1;
            return Ok(Ast::Any);
        }
        if c == b'[' {
            return self.class();
        }
        if c == b'\\' {
            if self.pos + 1 < b.len() {
                let nx = b[self.pos + 1];
                self.pos += 2;
                let ch = if self.translate {
                    match nx {
                        b'n' => b'\n',
                        b't' => b'\t',
                        b'r' => b'\r',
                        _ => nx,
                    }
                } else {
                    nx
                };
                return Ok(Ast::Lit(ch));
            }
            // Trailing backslash: literal backslash.
            self.pos += 1;
            return Ok(Ast::Lit(b'\\'));
        }
        self.pos += 1;
        Ok(Ast::Lit(c))
    }

    fn quant(&mut self, mut a: Ast) -> Result<Ast, ()> {
        loop {
            if self.pos >= self.b.len() {
                break;
            }
            let c = self.b[self.pos];
            if c == b'*' {
                self.pos += 1;
                a = Ast::Star(Box::new(a));
                continue;
            }
            if self.ere {
                if c == b'+' {
                    self.pos += 1;
                    a = Ast::Plus(Box::new(a));
                    continue;
                }
                if c == b'?' {
                    self.pos += 1;
                    a = Ast::Quest(Box::new(a));
                    continue;
                }
                if c == b'{' {
                    if let Some((mn, mx)) = self.interval(self.pos + 1, false)? {
                        a = Ast::Repeat(Box::new(a), mn, mx);
                        continue;
                    }
                }
            } else if c == b'\\' && self.pos + 1 < self.b.len() {
                let nx = self.b[self.pos + 1];
                if nx == b'+' {
                    self.pos += 2;
                    a = Ast::Plus(Box::new(a));
                    continue;
                }
                if nx == b'?' {
                    self.pos += 2;
                    a = Ast::Quest(Box::new(a));
                    continue;
                }
                if nx == b'{' {
                    if let Some((mn, mx)) = self.interval(self.pos + 2, true)? {
                        a = Ast::Repeat(Box::new(a), mn, mx);
                        continue;
                    }
                }
            }
            break;
        }
        Ok(a)
    }

    /// Parse a `{n}` / `{n,}` / `{n,m}` interval starting at `start`.
    /// `Ok(None)` means "not a well-formed interval here" (treat `{` literally);
    /// `Err(())` means a syntactically-present interval with an illegal bound
    /// (overflow / count > RE_DUP_MAX / n > m).
    fn interval(&mut self, start: usize, bre: bool) -> Result<Option<(usize, Option<usize>)>, ()> {
        let b = self.b;
        let mut p = start;
        let mut mn: usize = 0;
        let mut mn_overflow = false;
        let mut got = false;
        while p < b.len() && b[p].is_ascii_digit() {
            mn = mn.saturating_mul(10).saturating_add((b[p] - b'0') as usize);
            if mn > RE_DUP_MAX {
                mn_overflow = true;
            }
            p += 1;
            got = true;
        }
        if !got {
            return Ok(None);
        }
        let mut mx_overflow = false;
        let mx;
        if p < b.len() && b[p] == b',' {
            p += 1;
            if p < b.len() && b[p].is_ascii_digit() {
                let mut m: usize = 0;
                while p < b.len() && b[p].is_ascii_digit() {
                    m = m.saturating_mul(10).saturating_add((b[p] - b'0') as usize);
                    if m > RE_DUP_MAX {
                        mx_overflow = true;
                    }
                    p += 1;
                }
                mx = Some(m);
            } else {
                mx = None;
            }
        } else {
            mx = Some(mn);
        }
        // Require the closing brace before deciding this is an interval at all.
        if bre {
            if p + 1 < b.len() && b[p] == b'\\' && b[p + 1] == b'}' {
                p += 2;
            } else {
                return Ok(None);
            }
        } else if p < b.len() && b[p] == b'}' {
            p += 1;
        } else {
            return Ok(None);
        }
        // Syntactically an interval: now validate the bounds.
        if mn_overflow || mx_overflow {
            return Err(());
        }
        if let Some(m) = mx {
            if mn > m {
                return Err(());
            }
        }
        self.pos = p;
        Ok(Some((mn, mx)))
    }

    fn class(&mut self) -> Result<Ast, ()> {
        // Consume '['.
        self.pos += 1;
        let mut negate = false;
        if self.b.get(self.pos).copied() == Some(b'^') {
            negate = true;
            self.pos += 1;
        }
        let mut bm = [0u8; 32];
        let set = |bm: &mut [u8; 32], c: u8| bm[(c >> 3) as usize] |= 1u8 << (c & 7);

        let mut first = true;
        loop {
            let c = match self.b.get(self.pos).copied() {
                Some(c) => c,
                None => return Err(()), // unterminated
            };
            if c == b']' && !first {
                self.pos += 1;
                break;
            }

            // [:class:] character class.
            if c == b'[' && self.pos + 1 < self.b.len() && self.b[self.pos + 1] == b':' {
                let mut k = self.pos + 2;
                let start = k;
                while k + 1 < self.b.len() && !(self.b[k] == b':' && self.b[k + 1] == b']') {
                    k += 1;
                }
                if k + 1 >= self.b.len() {
                    return Err(());
                }
                let name = &self.b[start..k];
                add_named_class(&mut bm, name)?;
                self.pos = k + 2;
                first = false;
                continue;
            }

            // Collating [.x.] / equivalence [=x=]: take the enclosed byte literally.
            if c == b'['
                && self.pos + 1 < self.b.len()
                && (self.b[self.pos + 1] == b'.' || self.b[self.pos + 1] == b'=')
            {
                let delim = self.b[self.pos + 1];
                let mut k = self.pos + 2;
                let start = k;
                while k + 1 < self.b.len() && !(self.b[k] == delim && self.b[k + 1] == b']') {
                    k += 1;
                }
                if k + 1 >= self.b.len() {
                    return Err(());
                }
                for &byte in &self.b[start..k] {
                    set(&mut bm, byte);
                }
                self.pos = k + 2;
                first = false;
                continue;
            }

            // Range a-z (only when '-' is not the last char before ']').
            if self.pos + 2 < self.b.len()
                && self.b[self.pos + 1] == b'-'
                && self.b[self.pos + 2] != b']'
            {
                let lo = c;
                let hi = self.b[self.pos + 2];
                if lo <= hi {
                    let mut x = lo;
                    loop {
                        set(&mut bm, x);
                        if x == hi {
                            break;
                        }
                        x += 1;
                    }
                } else {
                    // Invalid range: treat literally.
                    set(&mut bm, lo);
                    set(&mut bm, b'-');
                    set(&mut bm, hi);
                }
                self.pos += 3;
                first = false;
                continue;
            }

            set(&mut bm, c);
            self.pos += 1;
            first = false;
        }
        Ok(Ast::Class(bm, negate))
    }
}

fn add_named_class(bm: &mut [u8; 32], name: &[u8]) -> Result<(), ()> {
    // Validate the class name once.
    match name {
        b"alpha" | b"digit" | b"alnum" | b"upper" | b"lower" | b"space" | b"blank" | b"punct"
        | b"cntrl" | b"graph" | b"print" | b"xdigit" => {}
        _ => return Err(()),
    }
    for v in 0u16..256 {
        let c = v as u8;
        let member = match name {
            b"alpha" => c.is_ascii_alphabetic(),
            b"digit" => c.is_ascii_digit(),
            b"alnum" => c.is_ascii_alphanumeric(),
            b"upper" => c.is_ascii_uppercase(),
            b"lower" => c.is_ascii_lowercase(),
            b"space" => matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c),
            b"blank" => c == b' ' || c == b'\t',
            b"punct" => c.is_ascii_punctuation(),
            b"cntrl" => c.is_ascii_control(),
            b"graph" => c.is_ascii_graphic(),
            b"print" => c.is_ascii_graphic() || c == b' ',
            b"xdigit" => c.is_ascii_hexdigit(),
            _ => false,
        };
        if member {
            bm[(c >> 3) as usize] |= 1u8 << (c & 7);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Compiled program (Pike VM instructions)
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Inst {
    Char(u8),
    Any,
    Class([u8; 32], bool),
    Save(usize),
    Jmp(usize),
    Split(usize, usize),
    Start,
    End,
    Match,
}

fn emit(ast: &Ast, prog: &mut Vec<Inst>) {
    // Stop growing the program once the cap is exceeded; `compile` turns the
    // oversize program into a compile error.
    if prog.len() > MAX_PROG_LEN {
        return;
    }
    match ast {
        Ast::Empty => {}
        Ast::Lit(c) => prog.push(Inst::Char(*c)),
        Ast::Any => prog.push(Inst::Any),
        Ast::Class(bm, neg) => prog.push(Inst::Class(*bm, *neg)),
        Ast::Start => prog.push(Inst::Start),
        Ast::End => prog.push(Inst::End),
        Ast::Concat(v) => {
            for a in v {
                emit(a, prog);
            }
        }
        Ast::Group(n, inner) => {
            prog.push(Inst::Save(2 * n));
            emit(inner, prog);
            prog.push(Inst::Save(2 * n + 1));
        }
        Ast::Alt(v) => {
            let mut jmps: Vec<usize> = Vec::new();
            for (i, a) in v.iter().enumerate() {
                if i + 1 < v.len() {
                    let sp = prog.len();
                    prog.push(Inst::Split(0, 0));
                    let astart = prog.len();
                    emit(a, prog);
                    let jp = prog.len();
                    prog.push(Inst::Jmp(0));
                    jmps.push(jp);
                    let next = prog.len();
                    prog[sp] = Inst::Split(astart, next);
                } else {
                    emit(a, prog);
                }
            }
            let end = prog.len();
            for jp in jmps {
                prog[jp] = Inst::Jmp(end);
            }
        }
        Ast::Star(inner) => {
            let sp = prog.len();
            prog.push(Inst::Split(0, 0));
            let body = prog.len();
            emit(inner, prog);
            prog.push(Inst::Jmp(sp));
            let out = prog.len();
            prog[sp] = Inst::Split(body, out);
        }
        Ast::Plus(inner) => {
            let body = prog.len();
            emit(inner, prog);
            let sp = prog.len();
            prog.push(Inst::Split(0, 0));
            prog[sp] = Inst::Split(body, prog.len());
        }
        Ast::Quest(inner) => {
            let sp = prog.len();
            prog.push(Inst::Split(0, 0));
            let body = prog.len();
            emit(inner, prog);
            let out = prog.len();
            prog[sp] = Inst::Split(body, out);
        }
        Ast::Repeat(inner, mn, mx) => {
            for _ in 0..*mn {
                emit(inner, prog);
            }
            match mx {
                None => emit(&Ast::Star(inner.clone()), prog),
                Some(m) => {
                    for _ in *mn..*m {
                        emit(&Ast::Quest(inner.clone()), prog);
                    }
                }
            }
        }
    }
}

#[inline]
fn other_case(x: u8) -> u8 {
    if x.is_ascii_lowercase() {
        x - 32
    } else if x.is_ascii_uppercase() {
        x + 32
    } else {
        x
    }
}

#[inline]
fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

#[inline]
fn class_contains(bm: &[u8; 32], c: u8) -> bool {
    (bm[(c >> 3) as usize] >> (c & 7)) & 1 == 1
}

#[inline]
fn class_match(bm: &[u8; 32], neg: bool, c: u8, ic: bool) -> bool {
    let mut m = class_contains(bm, c);
    if ic {
        m |= class_contains(bm, other_case(c));
    }
    m ^ neg
}

// ---------------------------------------------------------------------------
// Pike VM matcher
// ---------------------------------------------------------------------------

/// A single NFA thread: a program counter plus its capture slots.
///
/// The slots array carries this thread's tentative submatch offsets; slot 0 is
/// always the thread's start position (set by `Save(0)` at seed time), which
/// grep's `-w`/`-x` acceptance consults.
#[derive(Clone)]
struct Thread {
    pc: usize,
    saves: Vec<Option<usize>>,
}

/// An ordered, priority-preserving list of threads for one input position.
///
/// Threads are kept in strict backtracking priority order (index 0 is highest
/// priority). The allocation is reused across positions via [`ThreadList::clear`].
struct ThreadList {
    threads: Vec<Thread>,
}

impl ThreadList {
    fn new() -> ThreadList {
        ThreadList {
            threads: Vec::new(),
        }
    }
    #[inline]
    fn clear(&mut self) {
        self.threads.clear();
    }
    #[inline]
    fn len(&self) -> usize {
        self.threads.len()
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }
}

/// Per-position "already added" set over program counters, with O(1) clear.
///
/// A program counter is added to a given [`ThreadList`] at most once per input
/// position. This is what makes the Pike VM terminate on nullable stars such as
/// `\(a*\)*` (an empty loop revisits its own `Split` at the same position and is
/// dropped) and keeps the priority ordering deterministic (the first, i.e.
/// highest-priority, path to reach a pc wins its captures). Clearing walks only
/// the marked entries, so the cost is O(threads added), never O(program).
struct SeenSet {
    mark: Vec<bool>,
    marked: Vec<usize>,
}

impl SeenSet {
    fn new(n: usize) -> SeenSet {
        SeenSet {
            mark: vec![false; n],
            marked: Vec::new(),
        }
    }
    fn clear(&mut self) {
        for &p in &self.marked {
            self.mark[p] = false;
        }
        self.marked.clear();
    }
    /// Returns true if `pc` was already present; otherwise records it.
    #[inline]
    fn test_and_set(&mut self, pc: usize) -> bool {
        if self.mark[pc] {
            true
        } else {
            self.mark[pc] = true;
            self.marked.push(pc);
            false
        }
    }
}

impl Regex {
    /// Follow the epsilon-closure from `(pc0, saves0)` at input position `pos`,
    /// appending the reached consuming/`Match` instructions to `list` as new
    /// threads, in priority (backtracking) order.
    ///
    /// Iterative (explicit stack), so its depth is bounded by the program size
    /// and never by the input length. `Split(a, b)` explores `a` before `b`
    /// (the greedy / first-alternative branch first); `seen` deduplicates on the
    /// program counter so each pc is added at most once per position.
    fn add_thread(
        &self,
        list: &mut ThreadList,
        seen: &mut SeenSet,
        pc0: usize,
        saves0: Vec<Option<usize>>,
        pos: usize,
        len: usize,
    ) {
        // DFS with an explicit stack. To visit `a` before `b` under LIFO order,
        // push `b` first and `a` second so `a` pops (and closes) first.
        let mut stack: Vec<(usize, Vec<Option<usize>>)> = vec![(pc0, saves0)];
        while let Some((pc, saves)) = stack.pop() {
            if seen.test_and_set(pc) {
                continue;
            }
            match &self.prog[pc] {
                Inst::Jmp(x) => stack.push((*x, saves)),
                Inst::Split(a, b) => {
                    stack.push((*b, saves.clone()));
                    stack.push((*a, saves));
                }
                Inst::Save(k) => {
                    let mut s2 = saves;
                    if *k < s2.len() {
                        s2[*k] = Some(pos);
                    }
                    stack.push((pc + 1, s2));
                }
                Inst::Start => {
                    if pos == 0 {
                        stack.push((pc + 1, saves));
                    }
                }
                Inst::End => {
                    if pos == len {
                        stack.push((pc + 1, saves));
                    }
                }
                // Consuming instructions and Match become live threads.
                Inst::Char(_) | Inst::Any | Inst::Class(_, _) | Inst::Match => {
                    list.threads.push(Thread { pc, saves });
                }
            }
        }
    }

    /// grep `-x` (whole-line) and `-w` (word-boundary) acceptance for a match
    /// spanning `start..end`. Evaluated per thread when it reaches `Match`, so a
    /// rejected thread simply dies and lower-priority threads keep running (as a
    /// backtracker would keep searching) — never a post-hoc filter that would
    /// clobber the priority-thread ordering.
    #[inline]
    fn accept(&self, text: &[u8], start: usize, end: usize, whole_line: bool, word: bool) -> bool {
        let len = text.len();
        if whole_line && !(start == 0 && end == len) {
            return false;
        }
        if word {
            let before = start == 0 || !is_word(text[start - 1]);
            let after = end == len || !is_word(text[end]);
            if !(before && after) {
                return false;
            }
        }
        true
    }

    /// The core Pike VM. Returns the winning thread's capture slots, or `None`.
    ///
    /// Seeds a start thread at every input position from `from` onward (lowest
    /// priority at each position, so an earlier start out-ranks a later one and
    /// the result is leftmost), stopping once a match is recorded. When
    /// `anchored`, only a single start thread is seeded at `from` (used by grep
    /// `-x`). When a thread reaches `Match`, its captures are recorded and the
    /// remaining lower-priority threads at that position are cut; higher-priority
    /// threads already carried forward keep running and may overwrite the record
    /// with a more-preferred (greedier / earlier-starting) match.
    fn exec(
        &self,
        text: &[u8],
        from: usize,
        anchored: bool,
        whole_line: bool,
        word: bool,
    ) -> Option<Vec<Option<usize>>> {
        let len = text.len();
        if from > len {
            return None;
        }
        let mut clist = ThreadList::new();
        let mut nlist = ThreadList::new();
        let mut seen_c = SeenSet::new(self.prog.len());
        let mut seen_n = SeenSet::new(self.prog.len());
        let mut matched: Option<Vec<Option<usize>>> = None;

        let mut pos = from;
        loop {
            // Seed a fresh start thread at this position, at lowest priority
            // (appended after any threads carried forward from the previous
            // step, which share `seen_c`). Stop seeding once matched, or after
            // the first position when anchored.
            if matched.is_none() && (!anchored || pos == from) {
                let saves = vec![None; self.nslots];
                self.add_thread(&mut clist, &mut seen_c, 0, saves, pos, len);
            }

            // Nothing left to advance and no more seeds to add: done.
            if clist.is_empty() && (matched.is_some() || anchored || pos >= len) {
                break;
            }

            // Build the next position's thread list.
            seen_n.clear();
            nlist.clear();
            let mut i = 0;
            while i < clist.len() {
                let pc = clist.threads[i].pc;
                match &self.prog[pc] {
                    Inst::Char(c) => {
                        if pos < len
                            && (text[pos] == *c || (self.icase && other_case(text[pos]) == *c))
                        {
                            let saves = clist.threads[i].saves.clone();
                            self.add_thread(&mut nlist, &mut seen_n, pc + 1, saves, pos + 1, len);
                        }
                    }
                    Inst::Any => {
                        if pos < len {
                            let saves = clist.threads[i].saves.clone();
                            self.add_thread(&mut nlist, &mut seen_n, pc + 1, saves, pos + 1, len);
                        }
                    }
                    Inst::Class(bm, neg) => {
                        if pos < len && class_match(bm, *neg, text[pos], self.icase) {
                            let saves = clist.threads[i].saves.clone();
                            self.add_thread(&mut nlist, &mut seen_n, pc + 1, saves, pos + 1, len);
                        }
                    }
                    Inst::Match => {
                        let start = clist.threads[i].saves[0].unwrap_or(pos);
                        if self.accept(text, start, pos, whole_line, word) {
                            matched = Some(clist.threads[i].saves.clone());
                            // Cut all lower-priority threads at this position.
                            break;
                        }
                    }
                    // Epsilon instructions never appear as live threads: they are
                    // resolved during `add_thread`.
                    _ => {}
                }
                i += 1;
            }

            if pos >= len {
                break;
            }
            core::mem::swap(&mut clist, &mut nlist);
            core::mem::swap(&mut seen_c, &mut seen_n);
            pos += 1;
        }

        matched
    }
}

// ---------------------------------------------------------------------------
// Regex
// ---------------------------------------------------------------------------

pub(crate) struct Regex {
    prog: Vec<Inst>,
    nslots: usize,
    icase: bool,
}

impl Regex {
    pub fn compile(pat: &[u8], syntax: Syntax) -> Result<Regex, ()> {
        let mut p = Parser {
            b: pat,
            pos: 0,
            ere: syntax.ere,
            translate: syntax.translate_escapes,
            gc: 0,
            depth: 0,
        };
        let ast = p.alt()?;
        // Leftover input means an unmatched group close (e.g. a stray `\)`/`)`).
        if p.pos != pat.len() {
            return Err(());
        }
        let ngroups = p.gc;
        let mut prog: Vec<Inst> = Vec::new();
        prog.push(Inst::Save(0));
        emit(&ast, &mut prog);
        prog.push(Inst::Save(1));
        // Backstop: interval expansion can multiply the instruction count.
        if prog.len() > MAX_PROG_LEN {
            return Err(());
        }
        prog.push(Inst::Match);
        Ok(Regex {
            prog,
            nslots: 2 * (ngroups + 1),
            icase: syntax.icase,
        })
    }

    /// Leftmost match at or after `from`, with overall + capture-group spans.
    /// Used by sed for `s///` and address matching.
    pub fn search(&self, text: &[u8], from: usize) -> Option<Captures> {
        self.exec(text, from, false, false, false)
            .map(|slots| Captures { slots })
    }

    /// Boolean search used by grep. `whole_line` implements `-x` and `word`
    /// implements `-w`.
    pub fn is_match(&self, text: &[u8], whole_line: bool, word: bool) -> bool {
        // For -x the match must begin at column 0, so seed a single anchored
        // start thread; `accept` additionally requires it to end at the line end.
        self.exec(text, 0, whole_line, whole_line, word).is_some()
    }
}
