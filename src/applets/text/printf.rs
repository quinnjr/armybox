//! printf - format and print data
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/printf.html

use crate::applets::get_arg;
use crate::io;
use alloc::vec::Vec;

/// Maximum field width we will honor (guards against runaway allocation).
const MAX_WIDTH: usize = 1 << 20;

#[inline]
fn hexval(c: u8) -> u32 {
    match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'a'..=b'f' => (c - b'a' + 10) as u32,
        b'A'..=b'F' => (c - b'A' + 10) as u32,
        _ => 0,
    }
}

/// Decode a backslash escape sequence in `s` starting at index `i`
/// (where `s[i] == b'\\'`). Pushes the decoded byte(s) into `out`.
///
/// Returns `(new_index, stop)` where `new_index` is the position just past
/// the sequence and `stop` is true if a `\c` escape was seen (meaning all
/// further output should be suppressed).
fn decode_escape(s: &[u8], mut i: usize, out: &mut Vec<u8>) -> (usize, bool) {
    // s[i] == b'\\'
    i += 1;
    if i >= s.len() {
        out.push(b'\\');
        return (i, false);
    }
    let c = s[i];
    match c {
        b'\\' => {
            out.push(b'\\');
            (i + 1, false)
        }
        b'a' => {
            out.push(0x07);
            (i + 1, false)
        }
        b'b' => {
            out.push(0x08);
            (i + 1, false)
        }
        b'f' => {
            out.push(0x0c);
            (i + 1, false)
        }
        b'n' => {
            out.push(b'\n');
            (i + 1, false)
        }
        b'r' => {
            out.push(b'\r');
            (i + 1, false)
        }
        b't' => {
            out.push(b'\t');
            (i + 1, false)
        }
        b'v' => {
            out.push(0x0b);
            (i + 1, false)
        }
        b'c' => (i + 1, true),
        b'x' => {
            i += 1;
            let mut val = 0u32;
            let mut n = 0;
            while n < 2 && i < s.len() && s[i].is_ascii_hexdigit() {
                val = val * 16 + hexval(s[i]);
                i += 1;
                n += 1;
            }
            out.push(val as u8);
            (i, false)
        }
        b'0'..=b'7' => {
            // \NNN or \0NNN : up to three octal digits.
            // A leading '0' acts as an introducer for the \0NNN form.
            if s[i] == b'0' {
                i += 1;
            }
            let mut val = 0u32;
            let mut n = 0;
            while n < 3 && i < s.len() && (b'0'..=b'7').contains(&s[i]) {
                val = val * 8 + (s[i] - b'0') as u32;
                i += 1;
                n += 1;
            }
            out.push(val as u8);
            (i, false)
        }
        other => {
            out.push(b'\\');
            out.push(other);
            (i + 1, false)
        }
    }
}

/// Interpret all escape sequences in `s` (used for the `%b` conversion),
/// appending decoded bytes to `out`. Returns true if a `\c` was seen.
fn unescape_all(s: &[u8], out: &mut Vec<u8>) -> bool {
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\\' {
            let (ni, stop) = decode_escape(s, i, out);
            i = ni;
            if stop {
                return true;
            }
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    false
}

/// Format an unsigned value in the given radix into a fresh Vec.
fn digits(mut n: u64, radix: u64, upper: bool) -> Vec<u8> {
    if n == 0 {
        return alloc::vec![b'0'];
    }
    let lut: &[u8] = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    let mut v = Vec::new();
    while n > 0 {
        v.push(lut[(n % radix) as usize]);
        n /= radix;
    }
    v.reverse();
    v
}

/// Pad `prefix`+`body` to `width`, honoring left-justify and zero-fill.
/// Zero-fill (when requested and not left-justified) is inserted between
/// the prefix and the body so signs / base prefixes stay leftmost.
fn pad(out: &mut Vec<u8>, prefix: &[u8], body: &[u8], width: usize, left: bool, zero: bool) {
    let total = prefix.len() + body.len();
    if total >= width {
        out.extend_from_slice(prefix);
        out.extend_from_slice(body);
        return;
    }
    let padlen = width - total;
    if left {
        out.extend_from_slice(prefix);
        out.extend_from_slice(body);
        for _ in 0..padlen {
            out.push(b' ');
        }
    } else if zero {
        out.extend_from_slice(prefix);
        for _ in 0..padlen {
            out.push(b'0');
        }
        out.extend_from_slice(body);
    } else {
        for _ in 0..padlen {
            out.push(b' ');
        }
        out.extend_from_slice(prefix);
        out.extend_from_slice(body);
    }
}

/// Apply an explicit precision (minimum digit count) to a numeric body.
fn apply_precision(body: &mut Vec<u8>, value_is_zero: bool, prec: Option<usize>) {
    if let Some(p) = prec {
        if value_is_zero && p == 0 {
            body.clear();
        } else if body.len() < p {
            let mut nb = Vec::with_capacity(p);
            for _ in 0..(p - body.len()) {
                nb.push(b'0');
            }
            nb.extend_from_slice(body);
            *body = nb;
        }
    }
}

/// Emit a "not a valid number" diagnostic to stderr.
fn numeric_diag(val: &[u8]) {
    io::write_all(2, b"printf: ");
    io::write_all(2, val);
    io::write_all(2, b": expected a numeric value\n");
}

/// Parse a signed integer argument (strtoll, base 0 => decimal/octal/hex).
/// Invalid or partially-valid input sets `*code` and prints a diagnostic.
unsafe fn parse_signed(s: &[u8], code: &mut i32) -> i64 {
    if s.is_empty() {
        return 0;
    }
    let mut buf: Vec<u8> = Vec::with_capacity(s.len() + 1);
    buf.extend_from_slice(s);
    buf.push(0);
    let mut end: *mut libc::c_char = core::ptr::null_mut();
    let v = unsafe {
        libc::strtoll(
            buf.as_ptr() as *const libc::c_char,
            &mut end as *mut *mut libc::c_char,
            0,
        )
    };
    let consumed = end as usize - buf.as_ptr() as usize;
    if end.is_null() || consumed != s.len() {
        numeric_diag(s);
        *code = 1;
    }
    v as i64
}

/// Parse an unsigned integer argument (strtoull, base 0).
unsafe fn parse_unsigned(s: &[u8], code: &mut i32) -> u64 {
    if s.is_empty() {
        return 0;
    }
    let mut buf: Vec<u8> = Vec::with_capacity(s.len() + 1);
    buf.extend_from_slice(s);
    buf.push(0);
    let mut end: *mut libc::c_char = core::ptr::null_mut();
    let v = unsafe {
        libc::strtoull(
            buf.as_ptr() as *const libc::c_char,
            &mut end as *mut *mut libc::c_char,
            0,
        )
    };
    let consumed = end as usize - buf.as_ptr() as usize;
    if end.is_null() || consumed != s.len() {
        numeric_diag(s);
        *code = 1;
    }
    v as u64
}

/// Parse a floating-point argument (strtod).
unsafe fn parse_double(s: &[u8], code: &mut i32) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut buf: Vec<u8> = Vec::with_capacity(s.len() + 1);
    buf.extend_from_slice(s);
    buf.push(0);
    let mut end: *mut libc::c_char = core::ptr::null_mut();
    let v = unsafe {
        libc::strtod(
            buf.as_ptr() as *const libc::c_char,
            &mut end as *mut *mut libc::c_char,
        )
    };
    let consumed = end as usize - buf.as_ptr() as usize;
    if end.is_null() || consumed != s.len() {
        numeric_diag(s);
        *code = 1;
    }
    v
}

/// Format a floating-point value by delegating to libc's `snprintf`, reusing
/// the original conversion specification (flags/width/precision + conv char).
fn format_float(spec: &[u8], val: f64, out: &mut Vec<u8>, width: usize, prec: Option<usize>) {
    let mut cfmt: Vec<u8> = Vec::with_capacity(spec.len() + 1);
    cfmt.extend_from_slice(spec);
    cfmt.push(0);

    let size = width.max(prec.unwrap_or(6)) + 128;
    let mut fbuf: Vec<u8> = Vec::new();
    fbuf.resize(size + 16, 0);

    let n = unsafe {
        libc::snprintf(
            fbuf.as_mut_ptr() as *mut libc::c_char,
            fbuf.len() as libc::size_t,
            cfmt.as_ptr() as *const libc::c_char,
            val,
        )
    };
    if n > 0 {
        let len = (n as usize).min(fbuf.len() - 1);
        out.extend_from_slice(&fbuf[..len]);
    }
}

/// printf - format and print data
///
/// # Synopsis
/// ```text
/// printf format [argument...]
/// ```
///
/// # Description
/// Format and print ARGUMENT(s) according to FORMAT. The FORMAT is reused as
/// often as necessary to consume all ARGUMENTS.
///
/// # Conversions
/// `%d %i` signed, `%u` unsigned, `%o` octal, `%x %X` hex, `%c` char,
/// `%s` string, `%b` string with escapes, `%f %e %g %E %G` float, `%%` literal.
/// Flags (`- + space 0 #`), field width, and precision are supported.
///
/// # Exit Status
/// - 0: Success
/// - >0: A conversion error occurred (e.g. invalid numeric argument)
pub fn printf(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        return 0;
    }

    let fmt = match unsafe { get_arg(argv, 1) } {
        Some(f) => f,
        None => return 0,
    };

    let mut arg_idx: i32 = 2;
    let mut out: Vec<u8> = Vec::new();
    let mut code: i32 = 0;
    let mut stop = false;

    loop {
        let before = arg_idx;
        let mut fi = 0;

        while fi < fmt.len() && !stop {
            let c = fmt[fi];

            if c == b'\\' {
                let (ni, st) = decode_escape(fmt, fi, &mut out);
                fi = ni;
                if st {
                    stop = true;
                }
                continue;
            }

            if c != b'%' {
                out.push(c);
                fi += 1;
                continue;
            }

            // c == b'%'
            if fi + 1 < fmt.len() && fmt[fi + 1] == b'%' {
                out.push(b'%');
                fi += 2;
                continue;
            }

            // Parse a conversion specification: %[flags][width][.prec]conv
            let spec_start = fi;
            let mut j = fi + 1;

            let mut left = false;
            let mut zero = false;
            let mut plus = false;
            let mut space = false;
            let mut alt = false;
            while j < fmt.len() {
                match fmt[j] {
                    b'-' => left = true,
                    b'+' => plus = true,
                    b' ' => space = true,
                    b'0' => zero = true,
                    b'#' => alt = true,
                    _ => break,
                }
                j += 1;
            }

            // Width
            let mut width = 0usize;
            while j < fmt.len() && fmt[j].is_ascii_digit() {
                width = (width * 10 + (fmt[j] - b'0') as usize).min(MAX_WIDTH);
                j += 1;
            }

            // Precision
            let mut prec: Option<usize> = None;
            if j < fmt.len() && fmt[j] == b'.' {
                j += 1;
                let mut p = 0usize;
                while j < fmt.len() && fmt[j].is_ascii_digit() {
                    p = (p * 10 + (fmt[j] - b'0') as usize).min(MAX_WIDTH);
                    j += 1;
                }
                prec = Some(p);
            }

            if j >= fmt.len() {
                // Trailing '%' with no conversion: emit literally.
                out.extend_from_slice(&fmt[spec_start..]);
                fi = fmt.len();
                continue;
            }

            let conv = fmt[j];

            // Fetch next argument (for conversions that consume one).
            let mut take_arg = || -> &[u8] {
                if arg_idx < argc {
                    let a = unsafe { get_arg(argv, arg_idx) }.unwrap_or(b"");
                    arg_idx += 1;
                    a
                } else {
                    b""
                }
            };

            match conv {
                b'd' | b'i' => {
                    let arg = take_arg();
                    let v = unsafe { parse_signed(arg, &mut code) };
                    let neg = v < 0;
                    let uv = if neg {
                        (v as i128).unsigned_abs() as u64
                    } else {
                        v as u64
                    };
                    let mut body = digits(uv, 10, false);
                    apply_precision(&mut body, uv == 0, prec);
                    let mut prefix: Vec<u8> = Vec::new();
                    if neg {
                        prefix.push(b'-');
                    } else if plus {
                        prefix.push(b'+');
                    } else if space {
                        prefix.push(b' ');
                    }
                    let use_zero = zero && !left && prec.is_none();
                    pad(&mut out, &prefix, &body, width, left, use_zero);
                }
                b'u' => {
                    let arg = take_arg();
                    let v = unsafe { parse_unsigned(arg, &mut code) };
                    let mut body = digits(v, 10, false);
                    apply_precision(&mut body, v == 0, prec);
                    let use_zero = zero && !left && prec.is_none();
                    pad(&mut out, &[], &body, width, left, use_zero);
                }
                b'o' => {
                    let arg = take_arg();
                    let v = unsafe { parse_unsigned(arg, &mut code) };
                    let mut body = digits(v, 8, false);
                    apply_precision(&mut body, v == 0, prec);
                    if alt && body.first() != Some(&b'0') {
                        let mut nb = Vec::with_capacity(body.len() + 1);
                        nb.push(b'0');
                        nb.extend_from_slice(&body);
                        body = nb;
                    }
                    let use_zero = zero && !left && prec.is_none();
                    pad(&mut out, &[], &body, width, left, use_zero);
                }
                b'x' | b'X' => {
                    let upper = conv == b'X';
                    let arg = take_arg();
                    let v = unsafe { parse_unsigned(arg, &mut code) };
                    let mut body = digits(v, 16, upper);
                    apply_precision(&mut body, v == 0, prec);
                    let mut prefix: Vec<u8> = Vec::new();
                    if alt && v != 0 {
                        prefix.push(b'0');
                        prefix.push(if upper { b'X' } else { b'x' });
                    }
                    let use_zero = zero && !left && prec.is_none();
                    pad(&mut out, &prefix, &body, width, left, use_zero);
                }
                b'c' => {
                    let arg = take_arg();
                    let mut body: Vec<u8> = Vec::new();
                    if !arg.is_empty() {
                        body.push(arg[0]);
                    }
                    pad(&mut out, &[], &body, width, left, false);
                }
                b's' => {
                    let arg = take_arg();
                    let mut body = arg.to_vec();
                    if let Some(p) = prec {
                        if body.len() > p {
                            body.truncate(p);
                        }
                    }
                    pad(&mut out, &[], &body, width, left, false);
                }
                b'b' => {
                    let arg = take_arg();
                    let mut body: Vec<u8> = Vec::new();
                    let st = unescape_all(arg, &mut body);
                    if let Some(p) = prec {
                        if body.len() > p {
                            body.truncate(p);
                        }
                    }
                    pad(&mut out, &[], &body, width, left, false);
                    if st {
                        stop = true;
                    }
                }
                b'f' | b'F' | b'e' | b'E' | b'g' | b'G' | b'a' | b'A' => {
                    let arg = take_arg();
                    let val = unsafe { parse_double(arg, &mut code) };
                    let spec = &fmt[spec_start..=j];
                    format_float(spec, val, &mut out, width, prec);
                }
                _ => {
                    // Unsupported conversion: diagnose, don't emit stray output.
                    io::write_all(2, b"printf: invalid conversion specification\n");
                    code = 1;
                }
            }

            fi = j + 1;
        }

        if stop {
            break;
        }
        // Reuse the format until arguments are exhausted; stop if a pass
        // consumed no arguments (no arg-consuming conversions) to avoid looping.
        if arg_idx >= argc {
            break;
        }
        if arg_idx == before {
            break;
        }
    }

    let w = io::write_all(1, &out);
    if w < 0 || (w as usize) < out.len() {
        io::write_str(2, b"printf: write error\n");
        return 1;
    }
    code
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_printf_string() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printf", "%s", "hello"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello"
        );
    }

    #[test]
    fn test_printf_integer() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printf", "%d", "42"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(std::string::String::from_utf8_lossy(&output.stdout), "42");
    }

    #[test]
    fn test_printf_newline() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printf", "hello\\n"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "hello\n"
        );
    }

    #[test]
    fn test_printf_multiple_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["printf", "%s %d\\n", "count:", "5"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            std::string::String::from_utf8_lossy(&output.stdout),
            "count: 5\n"
        );
    }
}
