//! grep - search for patterns in files
//!
//! POSIX.1-2017 compliant implementation. Basic Regular Expressions (BRE,
//! default) and Extended Regular Expressions (ERE, `-E`) are handled by the
//! shared engine in `super::regex`; a fast fixed-string path (`-F`) lives here.
//!
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/grep.html

extern crate alloc;

use alloc::vec::Vec;

use crate::applets::get_arg;
use crate::applets::regex::{Regex, Syntax};
use crate::io;

// ===========================================================================
// Options
// ===========================================================================

#[derive(Clone, Copy)]
struct Opts {
    ere: bool,
    fixed: bool,
    ignore_case: bool,
    invert: bool,
    count_only: bool,
    list_files: bool,
    line_numbers: bool,
    quiet: bool,
    suppress_errors: bool,
    whole_line: bool,
    word: bool,
}

impl Opts {
    fn new(ere: bool, fixed: bool) -> Self {
        Opts {
            ere,
            fixed,
            ignore_case: false,
            invert: false,
            count_only: false,
            list_files: false,
            line_numbers: false,
            quiet: false,
            suppress_errors: false,
            whole_line: false,
            word: false,
        }
    }
}

// ===========================================================================
// Fixed-string matching (`-F`)
// ===========================================================================

#[inline]
fn to_lower(c: u8) -> u8 {
    if c.is_ascii_uppercase() { c + 32 } else { c }
}

#[inline]
fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn slice_eq_ci(a: &[u8], b: &[u8], ic: bool) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if ic {
            if to_lower(a[i]) != to_lower(b[i]) {
                return false;
            }
        } else if a[i] != b[i] {
            return false;
        }
    }
    true
}

fn fixed_match(line: &[u8], pat: &[u8], opts: &Opts) -> bool {
    if opts.whole_line {
        return slice_eq_ci(line, pat, opts.ignore_case);
    }
    if pat.is_empty() {
        return true;
    }
    if line.len() < pat.len() {
        return false;
    }
    for i in 0..=(line.len() - pat.len()) {
        if slice_eq_ci(&line[i..i + pat.len()], pat, opts.ignore_case) {
            if opts.word {
                let before = i == 0 || !is_word(line[i - 1]);
                let after = i + pat.len() == line.len() || !is_word(line[i + pat.len()]);
                if before && after {
                    return true;
                }
            } else {
                return true;
            }
        }
    }
    false
}

enum Compiled {
    Regex(Vec<Regex>),
    Fixed(Vec<Vec<u8>>),
}

impl Compiled {
    fn matches(&self, line: &[u8], opts: &Opts) -> bool {
        match self {
            Compiled::Regex(progs) => progs
                .iter()
                .any(|p| p.is_match(line, opts.whole_line, opts.word)),
            Compiled::Fixed(pats) => pats.iter().any(|p| fixed_match(line, p, opts)),
        }
    }
}

// ===========================================================================
// Pattern collection
// ===========================================================================

/// Split a raw pattern blob on newlines into individual patterns.
/// A single trailing newline does not create an empty trailing pattern.
fn split_patterns(blob: &[u8], out: &mut Vec<Vec<u8>>) {
    let mut start = 0;
    let len = blob.len();
    let mut i = 0;
    while i < len {
        if blob[i] == b'\n' {
            out.push(blob[start..i].to_vec());
            start = i + 1;
        }
        i += 1;
    }
    if start < len {
        out.push(blob[start..len].to_vec());
    }
    // If blob is empty, treat as a single empty pattern (matches everything).
    if len == 0 {
        out.push(Vec::new());
    }
}

// ===========================================================================
// Line iteration over a byte buffer (no fixed line buffer)
// ===========================================================================

/// Call `f(line_bytes)` for each line in `data`. A trailing newline does not
/// yield an empty final line.
fn for_each_line<F: FnMut(&[u8])>(data: &[u8], mut f: F) {
    let mut start = 0;
    let len = data.len();
    let mut i = 0;
    while i < len {
        if data[i] == b'\n' {
            f(&data[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    if start < len {
        f(&data[start..len]);
    }
}

// ===========================================================================
// Entry points
// ===========================================================================

/// grep - search a file for a pattern (BRE by default).
pub fn grep(argc: i32, argv: *const *const u8) -> i32 {
    grep_main(argc, argv, false, false)
}

/// egrep - grep with Extended Regular Expressions (`-E`).
pub fn egrep(argc: i32, argv: *const *const u8) -> i32 {
    grep_main(argc, argv, true, false)
}

/// fgrep - grep with fixed strings (`-F`).
pub fn fgrep(argc: i32, argv: *const *const u8) -> i32 {
    grep_main(argc, argv, false, true)
}

fn grep_main(argc: i32, argv: *const *const u8, default_ere: bool, default_fixed: bool) -> i32 {
    let mut opts = Opts::new(default_ere, default_fixed);
    let mut patterns: Vec<Vec<u8>> = Vec::new();
    let mut have_pattern_source = false;
    let mut operands: Vec<&[u8]> = Vec::new();
    // Filename display override: Some(true) for -H, Some(false) for -h.
    let mut with_filename: Option<bool> = None;
    let mut no_more_opts = false;
    let mut had_error = false;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => {
                i += 1;
                continue;
            }
        };

        if !no_more_opts && arg == b"--" {
            no_more_opts = true;
            i += 1;
            continue;
        }

        if !no_more_opts && arg.len() > 1 && arg[0] == b'-' {
            let mut j = 1;
            while j < arg.len() {
                let c = arg[j];
                match c {
                    b'E' => {
                        opts.ere = true;
                        opts.fixed = false;
                    }
                    b'F' => {
                        opts.fixed = true;
                        opts.ere = false;
                    }
                    b'i' => opts.ignore_case = true,
                    b'v' => opts.invert = true,
                    b'c' => opts.count_only = true,
                    b'l' => opts.list_files = true,
                    b'n' => opts.line_numbers = true,
                    b'q' => opts.quiet = true,
                    b's' => opts.suppress_errors = true,
                    b'x' => opts.whole_line = true,
                    b'w' => opts.word = true,
                    b'H' => with_filename = Some(true),
                    b'h' => with_filename = Some(false),
                    b'e' | b'f' => {
                        // Option value: rest of this cluster, else next argument.
                        let val: Option<&[u8]> = if j + 1 < arg.len() {
                            let v = &arg[j + 1..];
                            j = arg.len(); // consume remainder
                            Some(v)
                        } else {
                            i += 1;
                            unsafe { get_arg(argv, i) }
                        };
                        let Some(v) = val else {
                            io::write_str(2, b"grep: option requires an argument\n");
                            return 2;
                        };
                        if c == b'e' {
                            split_patterns(v, &mut patterns);
                        } else {
                            let fd = io::open(v, libc::O_RDONLY, 0);
                            if fd < 0 {
                                if !opts.suppress_errors {
                                    io::write_str(2, b"grep: ");
                                    io::write_all(2, v);
                                    io::write_str(2, b": No such file or directory\n");
                                }
                                return 2;
                            }
                            let data = io::read_all(fd);
                            if fd != 0 {
                                io::close(fd);
                            }
                            split_patterns(&data, &mut patterns);
                        }
                        have_pattern_source = true;
                        break; // done with this cluster
                    }
                    _ => {
                        io::write_str(2, b"grep: invalid option -- '");
                        io::write_all(2, &[c]);
                        io::write_str(2, b"'\n");
                        return 2;
                    }
                }
                j += 1;
            }
        } else {
            operands.push(arg);
        }
        i += 1;
    }

    // Determine the pattern source if no -e/-f were given.
    if !have_pattern_source {
        if operands.is_empty() {
            io::write_str(2, b"grep: missing pattern\n");
            return 2;
        }
        let pat = operands.remove(0);
        split_patterns(pat, &mut patterns);
    }

    // Compile patterns.
    let compiled = if opts.fixed {
        Compiled::Fixed(patterns)
    } else {
        let mut progs = Vec::with_capacity(patterns.len());
        let syntax = Syntax {
            ere: opts.ere,
            icase: opts.ignore_case,
            translate_escapes: false,
        };
        for p in &patterns {
            match Regex::compile(p, syntax) {
                Ok(prog) => progs.push(prog),
                Err(_) => {
                    io::write_str(2, b"grep: invalid regular expression\n");
                    return 2;
                }
            }
        }
        Compiled::Regex(progs)
    };

    let files = operands;
    let multiple_files = files.len() > 1;
    let show_name = with_filename.unwrap_or(multiple_files);

    let mut found_any = false;

    if files.is_empty() {
        // stdin
        let data = io::read_all(0);
        let r = process_source(
            &data,
            b"(standard input)",
            show_name,
            &compiled,
            &opts,
            &mut found_any,
        );
        if r {
            found_any = true;
        }
    } else {
        for &f in &files {
            let is_stdin = f == b"-";
            let fd = if is_stdin {
                0
            } else {
                io::open(f, libc::O_RDONLY, 0)
            };
            if fd < 0 {
                if !opts.suppress_errors {
                    io::write_str(2, b"grep: ");
                    io::write_all(2, f);
                    io::write_str(2, b": No such file or directory\n");
                }
                had_error = true;
                continue;
            }
            let data = io::read_all(fd);
            if fd != 0 {
                io::close(fd);
            }
            let name: &[u8] = if is_stdin { b"(standard input)" } else { f };
            let r = process_source(&data, name, show_name, &compiled, &opts, &mut found_any);
            if r {
                found_any = true;
            }
        }
    }

    if had_error {
        2
    } else if found_any {
        0
    } else {
        1
    }
}

/// Process one input source. Returns true if any line was selected.
///
/// Handles -q (immediate exit), -l, -c and normal line output.
fn process_source(
    data: &[u8],
    name: &[u8],
    show_name: bool,
    compiled: &Compiled,
    opts: &Opts,
    found_any: &mut bool,
) -> bool {
    let mut count: u64 = 0;
    let mut line_no: u64 = 0;
    let mut any = false;

    for_each_line(data, |line| {
        line_no += 1;
        let selected = compiled.matches(line, opts) != opts.invert;
        if !selected {
            return;
        }
        any = true;
        *found_any = true;
        count += 1;

        if opts.quiet {
            // Match found: exit immediately with success.
            io::exit(0);
        }
        if opts.list_files || opts.count_only {
            // Output deferred until after the loop.
            return;
        }
        // Normal line output.
        if show_name {
            io::write_all(1, name);
            io::write_str(1, b":");
        }
        if opts.line_numbers {
            io::write_num(1, line_no);
            io::write_str(1, b":");
        }
        io::write_all(1, line);
        io::write_str(1, b"\n");
    });

    if !opts.quiet {
        if opts.list_files {
            if any {
                io::write_all(1, name);
                io::write_str(1, b"\n");
            }
        } else if opts.count_only {
            if show_name {
                io::write_all(1, name);
                io::write_str(1, b":");
            }
            io::write_num(1, count);
            io::write_str(1, b"\n");
        }
    }

    any
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

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

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_grep_test_{}_{}",
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
    fn test_grep_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "hello world\nfoo bar\nhello again\n").unwrap();

        let output = Command::new(&armybox)
            .args(["grep", "hello", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("hello"));
        assert!(lines[1].contains("hello"));
        cleanup(&dir);
    }

    #[test]
    fn test_grep_line_numbers() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        fs::write(&file, "one\ntwo\nthree\ntwo again\n").unwrap();

        let output = Command::new(&armybox)
            .args(["grep", "-n", "two", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("2:two"));
        assert!(stdout.contains("4:two again"));
        cleanup(&dir);
    }

    #[test]
    fn test_grep_count() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["grep", "-c", "test"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(b"test one\nno match\ntest two\ntest three\n")
                .unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout.trim(), "3");
    }

    #[test]
    fn test_grep_invert() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["grep", "-v", "skip"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin
                .write_all(b"keep this\nskip this\nkeep that\n")
                .unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines, vec!["keep this", "keep that"]);
    }

    #[test]
    fn test_grep_ignore_case() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["grep", "-i", "hello"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"Hello World\nHELLO\nhello\nhi\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_grep_no_match() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["grep", "notfound"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"some text\nother text\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1)); // No match returns 1
    }
}
