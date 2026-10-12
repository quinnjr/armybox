//! tr - translate characters
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/tr.html

use crate::applets::{get_arg, has_opt};
use crate::io;

/// tr - translate or delete characters
///
/// # Synopsis
/// ```text
/// tr [-c|-C] [-s] string1 string2
/// tr -s [-c|-C] string1
/// tr -d [-c|-C] string1
/// tr -ds [-c|-C] string1 string2
/// ```
///
/// # Description
/// Translate characters from standard input, writing to standard output.
///
/// # Options
/// - `-c` / `-C`: Complement the set of characters in string1
/// - `-d`: Delete characters in string1
/// - `-s`: Replace repeated occurrences of characters with a single occurrence
/// - `-t`: Truncate string1 to the length of string2
///
/// # Exit Status
/// - 0: Success
/// - 2: Usage error (missing operand)
pub fn tr(argc: i32, argv: *const *const u8) -> i32 {
    #[cfg(feature = "alloc")]
    {
        let mut delete = false;
        let mut squeeze = false;
        let mut complement = false;
        let mut truncate = false;
        let mut set1_idx = 0;
        let mut set2_idx = 0;

        for i in 1..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                if arg == b"--help" {
                    io::write_str(1, b"usage: tr [-cCdst] string1 [string2]\n");
                    return 0;
                }
                if arg.len() > 2 && arg.starts_with(b"--") {
                    io::write_str(2, b"tr: unrecognized option '");
                    io::write_all(2, arg);
                    io::write_str(2, b"'\n");
                    return 2;
                }
                if arg.len() > 1 && arg[0] == b'-' {
                    if has_opt(arg, b'd') {
                        delete = true;
                    }
                    if has_opt(arg, b's') {
                        squeeze = true;
                    }
                    if has_opt(arg, b'c') || has_opt(arg, b'C') {
                        complement = true;
                    }
                    if has_opt(arg, b't') {
                        truncate = true;
                    }
                } else if set1_idx == 0 {
                    set1_idx = i;
                } else if set2_idx == 0 {
                    set2_idx = i;
                }
            }
        }

        if set1_idx == 0 {
            io::write_str(2, b"tr: missing operand\n");
            return 2;
        }

        let set1 = unsafe { get_arg(argv, set1_idx).unwrap() };
        let set2 = if set2_idx > 0 {
            unsafe { get_arg(argv, set2_idx) }
        } else {
            None
        };

        // Translate requires two operands. If neither -d nor -s is given, a
        // second operand is mandatory.
        if set2.is_none() && !delete && !squeeze {
            io::write_str(2, b"tr: missing operand\n");
            return 2;
        }

        // --- Build all lookup tables ONCE, before the per-byte loop. ---

        let set1_expanded = expand_set(set1);
        let set2_expanded = set2.map(expand_set);

        // Translate only when a second operand exists and we are not deleting.
        let translate = !delete && set2_expanded.is_some();

        // Deletion membership table.
        let mut delete_set = [false; 256];
        if delete {
            for &b in &set1_expanded {
                delete_set[b as usize] = true;
            }
            if complement {
                for j in 0..256 {
                    delete_set[j] = !delete_set[j];
                }
            }
        }

        // Translation map (identity by default).
        let mut map = [0u8; 256];
        for i in 0..256 {
            map[i] = i as u8;
        }
        if translate {
            let s2e = set2_expanded.as_ref().unwrap();
            if complement {
                // Map every byte NOT in set1 to the last char of set2.
                let mut in1 = [false; 256];
                for &b in &set1_expanded {
                    in1[b as usize] = true;
                }
                let last = if s2e.is_empty() {
                    0
                } else {
                    s2e[s2e.len() - 1]
                };
                for j in 0..256 {
                    if !in1[j] {
                        map[j] = last;
                    }
                }
            } else {
                let limit = if truncate {
                    core::cmp::min(set1_expanded.len(), s2e.len())
                } else {
                    set1_expanded.len()
                };
                for i in 0..limit {
                    let c = set1_expanded[i];
                    let r = if i < s2e.len() {
                        s2e[i]
                    } else if !s2e.is_empty() {
                        s2e[s2e.len() - 1]
                    } else {
                        c
                    };
                    map[c as usize] = r;
                }
            }
        }

        // Squeeze membership table (applies to the last relevant set).
        let mut squeeze_set = [false; 256];
        if squeeze {
            if translate {
                // Squeeze the (translated) output set == set2.
                for &b in set2_expanded.as_ref().unwrap() {
                    squeeze_set[b as usize] = true;
                }
            } else if delete {
                if let Some(ref s2e) = set2_expanded {
                    // -ds: delete via set1, squeeze via set2.
                    for &b in s2e {
                        squeeze_set[b as usize] = true;
                    }
                } else {
                    for &b in &set1_expanded {
                        squeeze_set[b as usize] = true;
                    }
                    if complement {
                        for j in 0..256 {
                            squeeze_set[j] = !squeeze_set[j];
                        }
                    }
                }
            } else {
                // -s alone: squeeze set1.
                for &b in &set1_expanded {
                    squeeze_set[b as usize] = true;
                }
                if complement {
                    for j in 0..256 {
                        squeeze_set[j] = !squeeze_set[j];
                    }
                }
            }
        }

        // Per-byte processing (uses only pre-built tables). Transformed
        // bytes are appended to an output buffer that is flushed in blocks,
        // rather than issuing one write syscall per byte.
        let process = |c: u8, out_buf: &mut alloc::vec::Vec<u8>, last_out: &mut i32| {
            if delete && delete_set[c as usize] {
                return;
            }
            let out = if translate { map[c as usize] } else { c };
            if squeeze && squeeze_set[out as usize] && *last_out == out as i32 {
                return;
            }
            out_buf.push(out);
            *last_out = out as i32;
        };

        // --- Stream stdin -> stdout, byte-transparent (no synthetic
        // trailing newline is added; output is exactly the transformed
        // input bytes, matching POSIX tr behavior). ---
        let mut buf = [0u8; 4096];
        let mut out_buf: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(4096);
        let mut last_out: i32 = -1;

        loop {
            let n = io::read(0, &mut buf);
            if n <= 0 {
                break;
            }
            let n = n as usize;
            for k in 0..n {
                process(buf[k], &mut out_buf, &mut last_out);
            }
            if out_buf.len() >= 4096 {
                if io::write_all(1, &out_buf) < 0 {
                    io::write_str(2, b"tr: write error\n");
                    return 1;
                }
                out_buf.clear();
            }
        }

        if !out_buf.is_empty() && io::write_all(1, &out_buf) < 0 {
            io::write_str(2, b"tr: write error\n");
            return 1;
        }

        return 0;
    }

    #[cfg(not(feature = "alloc"))]
    {
        let _ = argc;
        let _ = argv;
        io::write_str(2, b"tr: requires alloc feature\n");
        return 1;
    }
}

/// Decode a single (possibly escaped) character starting at `i`.
/// Returns the byte and the index just past the consumed input.
#[cfg(feature = "alloc")]
fn decode_char(s: &[u8], i: usize) -> (u8, usize) {
    if s[i] == b'\\' && i + 1 < s.len() {
        let c = s[i + 1];
        match c {
            b'n' => (b'\n', i + 2),
            b't' => (b'\t', i + 2),
            b'r' => (b'\r', i + 2),
            b'\\' => (b'\\', i + 2),
            b'a' => (7, i + 2),
            b'b' => (8, i + 2),
            b'f' => (12, i + 2),
            b'v' => (11, i + 2),
            b'0'..=b'7' => {
                // Up to 3 octal digits.
                let mut val: u32 = 0;
                let mut j = i + 1;
                let mut cnt = 0;
                while j < s.len() && cnt < 3 && (b'0'..=b'7').contains(&s[j]) {
                    val = val * 8 + (s[j] - b'0') as u32;
                    j += 1;
                    cnt += 1;
                }
                ((val & 0xff) as u8, j)
            }
            _ => (c, i + 2),
        }
    } else {
        (s[i], i + 1)
    }
}

/// Append the inclusive byte range `a..=b` to `out`.
#[cfg(feature = "alloc")]
fn push_range(out: &mut alloc::vec::Vec<u8>, a: u8, b: u8) {
    for c in a..=b {
        out.push(c);
    }
}

/// Append the members of a POSIX character class named `name` to `out`.
#[cfg(feature = "alloc")]
fn push_class(out: &mut alloc::vec::Vec<u8>, name: &[u8]) {
    match name {
        b"alpha" => {
            push_range(out, b'A', b'Z');
            push_range(out, b'a', b'z');
        }
        b"digit" => push_range(out, b'0', b'9'),
        b"alnum" => {
            push_range(out, b'0', b'9');
            push_range(out, b'A', b'Z');
            push_range(out, b'a', b'z');
        }
        b"upper" => push_range(out, b'A', b'Z'),
        b"lower" => push_range(out, b'a', b'z'),
        b"space" => {
            // tab, newline, vtab, formfeed, carriage return, space
            for &c in &[9u8, 10, 11, 12, 13, 32] {
                out.push(c);
            }
        }
        b"blank" => {
            out.push(9);
            out.push(32);
        }
        b"punct" => {
            push_range(out, 33, 47);
            push_range(out, 58, 64);
            push_range(out, 91, 96);
            push_range(out, 123, 126);
        }
        b"cntrl" => {
            push_range(out, 0, 31);
            out.push(127);
        }
        b"print" => push_range(out, 32, 126),
        b"graph" => push_range(out, 33, 126),
        b"xdigit" => {
            push_range(out, b'0', b'9');
            push_range(out, b'A', b'F');
            push_range(out, b'a', b'f');
        }
        _ => {
            // Unknown class: treat the whole token as literal bytes.
            out.push(b'[');
            out.push(b':');
            out.extend_from_slice(name);
            out.push(b':');
            out.push(b']');
        }
    }
}

/// Expand a SET operand into an explicit list of bytes, handling ranges
/// (`a-z`), POSIX character classes (`[:alpha:]`), and escapes (`\n`, `\NNN`).
#[cfg(feature = "alloc")]
fn expand_set(s: &[u8]) -> alloc::vec::Vec<u8> {
    use alloc::vec::Vec;
    let mut result = Vec::new();
    let mut i = 0;

    while i < s.len() {
        // POSIX character class: [:name:]
        if s[i] == b'[' && i + 1 < s.len() && s[i + 1] == b':' {
            let mut k = i + 2;
            let mut found = None;
            while k + 1 < s.len() {
                if s[k] == b':' && s[k + 1] == b']' {
                    found = Some(k);
                    break;
                }
                k += 1;
            }
            if let Some(end) = found {
                push_class(&mut result, &s[i + 2..end]);
                i = end + 2;
                continue;
            }
        }

        let (c, ni) = decode_char(s, i);

        // Range: c-c2
        if ni < s.len() && s[ni] == b'-' && ni + 1 < s.len() {
            let (c2, ni2) = decode_char(s, ni + 1);
            if c <= c2 {
                for x in c..=c2 {
                    result.push(x);
                }
            } else {
                let mut x = c;
                loop {
                    result.push(x);
                    if x == c2 {
                        break;
                    }
                    x -= 1;
                }
            }
            i = ni2;
        } else {
            result.push(c);
            i = ni;
        }
    }
    result
}

#[cfg(not(feature = "alloc"))]
fn expand_set(s: &[u8]) -> &[u8] {
    // Without alloc, just return as-is (limited functionality)
    s
}

#[cfg(test)]
mod tests {
    extern crate std;
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

    #[test]
    fn test_tr_lowercase_to_uppercase() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["tr", "a-z", "A-Z"])
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
        assert_eq!(stdout, "HELLO WORLD\n");
    }

    #[test]
    fn test_tr_delete() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["tr", "-d", "aeiou"])
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
        assert_eq!(stdout, "hll wrld\n");
    }

    #[test]
    fn test_tr_squeeze() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["tr", "-s", " ", " "])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"hello    world\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "hello world\n");
    }

    #[test]
    fn test_tr_simple_replace() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["tr", "abc", "xyz"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"abc\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, "xyz\n");
    }
}
