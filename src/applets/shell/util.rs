//! Shell utility functions
//!
//! This module contains utility functions for parsing and file operations
//! used by the shell implementation.

use crate::io;

/// Skip whitespace, semicolons, and comments, return new position
#[cfg(feature = "alloc")]
pub(super) fn skip_whitespace_and_comments(script: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < script.len() {
        let c = script[pos];
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' || c == b';' {
            pos += 1;
        } else if c == b'#' {
            // Skip to end of line
            while pos < script.len() && script[pos] != b'\n' {
                pos += 1;
            }
        } else {
            break;
        }
    }
    pos
}

/// Find end of word (alphanumeric + underscore)
#[cfg(feature = "alloc")]
pub(super) fn find_word_end(script: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < script.len() {
        let c = script[pos];
        if c.is_ascii_alphanumeric() || c == b'_' {
            pos += 1;
        } else {
            break;
        }
    }
    pos
}

/// Check if a position is a valid keyword boundary
///
/// A keyword is only valid if it's followed by whitespace, a command separator,
/// an operator, end-of-input, or a comment. Characters like `=`, `/`, `-` indicate
/// the word is part of a larger token (e.g. `if=/dev/urandom` in `dd if=...`).
#[cfg(feature = "alloc")]
pub(super) fn is_keyword_boundary(script: &[u8], pos: usize) -> bool {
    if pos >= script.len() {
        return true;
    }
    let c = script[pos];
    matches!(
        c,
        b' ' | b'\t' | b'\n' | b'\r' | b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')' | b'#'
    )
}

/// Find a keyword at word boundary
#[cfg(feature = "alloc")]
pub(super) fn find_keyword(script: &[u8], start: usize, keyword: &[u8]) -> Option<usize> {
    let mut pos = start;
    while pos < script.len() {
        pos = skip_whitespace_and_comments(script, pos);
        if pos >= script.len() {
            return None;
        }

        let word_end = find_word_end(script, pos);
        if &script[pos..word_end] == keyword && is_keyword_boundary(script, word_end) {
            return Some(pos);
        }

        pos = skip_to_next_token(script, pos);
    }
    None
}

/// Find matching done for while/for/until
#[cfg(feature = "alloc")]
pub(super) fn find_matching_done(script: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    let mut depth = 1;

    while pos < script.len() && depth > 0 {
        pos = skip_whitespace_and_comments(script, pos);
        if pos >= script.len() {
            return None;
        }

        let word_end = find_word_end(script, pos);
        let word = &script[pos..word_end];

        if (word == b"while" || word == b"for" || word == b"until")
            && is_keyword_boundary(script, word_end)
        {
            depth += 1;
        } else if word == b"done" && is_keyword_boundary(script, word_end) {
            depth -= 1;
            if depth == 0 {
                return Some(pos);
            }
        }

        pos = skip_to_next_token(script, pos);
    }
    None
}

/// Skip to next token
#[cfg(feature = "alloc")]
pub(super) fn skip_to_next_token(script: &[u8], start: usize) -> usize {
    let mut pos = start;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while pos < script.len() {
        let c = script[pos];

        if in_single_quote {
            if c == b'\'' {
                in_single_quote = false;
            }
            pos += 1;
        } else if in_double_quote {
            if c == b'"' {
                in_double_quote = false;
            } else if c == b'\\' && pos + 1 < script.len() {
                pos += 1;
            }
            pos += 1;
        } else if c == b'\'' {
            in_single_quote = true;
            pos += 1;
        } else if c == b'"' {
            in_double_quote = true;
            pos += 1;
        } else if c == b' ' || c == b'\t' || c == b'\n' || c == b';' {
            break;
        } else {
            pos += 1;
        }
    }
    pos
}

// ============================================================================
// File test functions - using io module's safe wrappers
// ============================================================================

/// Check if file exists
///
/// Uses the io module's stat wrapper which handles path null-termination safely.
#[cfg(feature = "alloc")]
pub(super) fn file_exists(path: &[u8]) -> bool {
    let mut stat_buf = io::stat_zeroed();
    io::stat(path, &mut stat_buf) == 0
}

/// Check if path is a regular file
#[cfg(feature = "alloc")]
pub(super) fn is_regular_file(path: &[u8]) -> bool {
    let mut stat_buf = io::stat_zeroed();
    if io::stat(path, &mut stat_buf) != 0 {
        return false;
    }
    (stat_buf.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFREG as u32
}

/// Check if path is a directory
#[cfg(feature = "alloc")]
pub(super) fn is_directory(path: &[u8]) -> bool {
    let mut stat_buf = io::stat_zeroed();
    if io::stat(path, &mut stat_buf) != 0 {
        return false;
    }
    (stat_buf.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32
}

/// Check if path is readable
#[cfg(feature = "alloc")]
pub(super) fn is_readable(path: &[u8]) -> bool {
    io::access(path, libc::R_OK) == 0
}

/// Check if path is writable
#[cfg(feature = "alloc")]
pub(super) fn is_writable(path: &[u8]) -> bool {
    io::access(path, libc::W_OK) == 0
}

/// Check if path is executable
#[cfg(feature = "alloc")]
pub(super) fn is_executable(path: &[u8]) -> bool {
    io::access(path, libc::X_OK) == 0
}

/// Check if path is a symbolic link
#[cfg(feature = "alloc")]
pub(super) fn is_symlink(path: &[u8]) -> bool {
    let mut stat_buf = io::stat_zeroed();
    if io::lstat(path, &mut stat_buf) != 0 {
        return false;
    }
    (stat_buf.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFLNK as u32
}

/// Get file size in bytes
#[cfg(feature = "alloc")]
pub(super) fn file_size(path: &[u8]) -> i64 {
    let mut stat_buf = io::stat_zeroed();
    if io::stat(path, &mut stat_buf) != 0 {
        return 0;
    }
    stat_buf.st_size
}

// ============================================================================
// Number formatting utilities
// ============================================================================

/// Format an unsigned number into a byte buffer
///
/// Returns a slice of the buffer containing the formatted number.
/// The number is written right-aligned in the buffer.
pub(super) fn format_number(mut n: u64, buf: &mut [u8]) -> &[u8] {
    if buf.is_empty() {
        return &[];
    }
    if n == 0 {
        buf[0] = b'0';
        return &buf[..1];
    }
    let mut i = buf.len();
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    &buf[i..]
}

/// Format a signed number into a byte buffer
///
/// Returns a slice of the buffer containing the formatted number.
/// Handles negative numbers by prepending a '-' sign.
pub(super) fn format_signed(n: i64, buf: &mut [u8]) -> &[u8] {
    if buf.len() < 2 {
        return &[];
    }
    if n < 0 {
        let buf_len = buf.len();
        let s_len = format_number((-n) as u64, &mut buf[1..]).len();
        if s_len == 0 {
            return &[];
        }
        let start = buf_len - s_len - 1;
        buf[start] = b'-';
        &buf[start..]
    } else {
        format_number(n as u64, buf)
    }
}
