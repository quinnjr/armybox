//! Shell control flow structures
//!
//! This module implements POSIX shell control flow constructs:
//! - `if/then/else/elif/fi` - conditional execution
//! - `while/do/done` - loop while condition is true
//! - `until/do/done` - loop until condition is true
//! - `for/in/do/done` - iterate over word list
//! - `case/in/esac` - pattern matching

#[cfg(feature = "alloc")]
use super::expand::split_words;
#[cfg(feature = "alloc")]
use super::parser::parse_word;
#[cfg(feature = "alloc")]
use super::state::Shell;
#[cfg(feature = "alloc")]
use super::util::{
    find_keyword, find_matching_done, find_word_end, is_keyword_boundary, skip_to_next_token,
    skip_whitespace_and_comments,
};

use crate::io;

/// Execute if/then/else/elif/fi
///
/// Evaluates the condition and executes the appropriate branch.
/// Supports nested if statements and elif chains.
///
/// Returns the position after the closing `fi`.
#[cfg(feature = "alloc")]
pub(super) fn execute_if(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let mut pos = start + 2; // skip "if"

    // Find "then"
    let then_pos = find_keyword(script, pos, b"then");
    if then_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'then'\n");
        return script.len();
    }
    let then_pos = then_pos.unwrap();

    // Execute condition
    let condition = &script[pos..then_pos];
    super::execute_script(shell, condition.trim_ascii());
    let condition_result = shell.last_status == 0;

    pos = then_pos + 4; // skip "then"

    // Find matching fi, tracking first elif/else at depth 1 only
    let mut depth = 1;
    let mut first_else_pos: Option<usize> = None;
    let mut first_else_is_elif = false;
    let mut fi_pos: Option<usize> = None;
    let mut scan = pos;

    while scan < script.len() && depth > 0 {
        scan = skip_whitespace_and_comments(script, scan);
        if scan >= script.len() {
            break;
        }

        let word_end = find_word_end(script, scan);
        let word = &script[scan..word_end];
        let at_boundary = is_keyword_boundary(script, word_end);

        if word == b"if" && at_boundary {
            depth += 1;
            scan = word_end;
        } else if word == b"fi" && at_boundary {
            depth -= 1;
            if depth == 0 {
                fi_pos = Some(scan);
            }
            scan = word_end;
        } else if word == b"else" && depth == 1 && at_boundary && first_else_pos.is_none() {
            first_else_pos = Some(scan);
            first_else_is_elif = false;
            scan = word_end;
        } else if word == b"elif" && depth == 1 && at_boundary && first_else_pos.is_none() {
            first_else_pos = Some(scan);
            first_else_is_elif = true;
            scan = word_end;
        } else {
            scan = skip_to_next_token(script, scan);
        }
    }

    if fi_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'fi'\n");
        return script.len();
    }
    let fi_pos = fi_pos.unwrap();

    if condition_result {
        // Execute then branch
        let end = first_else_pos.unwrap_or(fi_pos);
        let then_body = &script[pos..end];
        super::execute_script(shell, then_body);
    } else if let Some(else_start) = first_else_pos {
        if first_else_is_elif {
            // elif: reconstruct as "if <rest> fi" and execute recursively
            // The content from elif to fi is: "elif cond; then body [elif|else ...] fi"
            // We treat it as: "if cond; then body [elif|else ...] fi"
            let elif_body = &script[else_start + 2..fi_pos + 2]; // skip "el", keep "if...fi"
            super::execute_script(shell, elif_body.trim_ascii());
        } else {
            // else: execute the body between "else" and "fi"
            let else_body = &script[else_start + 4..fi_pos]; // skip "else"
            super::execute_script(shell, else_body);
        }
    }

    fi_pos + 2 // skip "fi"
}

/// Execute while/do/done
///
/// Repeatedly executes the body while the condition returns success (exit 0).
///
/// Returns the position after the closing `done`.
#[cfg(feature = "alloc")]
pub(super) fn execute_while(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let mut pos = start + 5; // skip "while"

    // Find "do"
    let do_pos = find_keyword(script, pos, b"do");
    if do_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'do'\n");
        return script.len();
    }
    let do_pos = do_pos.unwrap();
    let condition = &script[pos..do_pos];

    pos = do_pos + 2; // skip "do"

    // Find matching "done"
    let done_pos = find_matching_done(script, pos);
    if done_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'done'\n");
        return script.len();
    }
    let done_pos = done_pos.unwrap();

    let body = &script[pos..done_pos];

    // Execute while loop
    loop {
        super::execute_script(shell, condition.trim_ascii());
        if shell.last_status != 0 {
            break;
        }
        super::execute_script(shell, body);
        if shell.should_exit {
            break;
        }
    }

    done_pos + 4 // skip "done"
}

/// Execute until/do/done (opposite of while)
///
/// Repeatedly executes the body until the condition returns success (exit 0).
///
/// Returns the position after the closing `done`.
#[cfg(feature = "alloc")]
pub(super) fn execute_until(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let mut pos = start + 5; // skip "until"

    let do_pos = find_keyword(script, pos, b"do");
    if do_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'do'\n");
        return script.len();
    }
    let do_pos = do_pos.unwrap();
    let condition = &script[pos..do_pos];

    pos = do_pos + 2;

    let done_pos = find_matching_done(script, pos);
    if done_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'done'\n");
        return script.len();
    }
    let done_pos = done_pos.unwrap();

    let body = &script[pos..done_pos];

    loop {
        super::execute_script(shell, condition.trim_ascii());
        if shell.last_status == 0 {
            break;
        } // opposite of while
        super::execute_script(shell, body);
        if shell.should_exit {
            break;
        }
    }

    done_pos + 4
}

/// Execute for/in/do/done
///
/// Iterates over a list of words, setting the loop variable for each iteration.
///
/// Returns the position after the closing `done`.
#[cfg(feature = "alloc")]
pub(super) fn execute_for(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let mut pos = start + 3; // skip "for"
    pos = skip_whitespace_and_comments(script, pos);

    // Get variable name
    let var_end = find_word_end(script, pos);
    let var_name = script[pos..var_end].to_vec();
    pos = var_end;

    pos = skip_whitespace_and_comments(script, pos);

    // Check for "in" keyword
    let in_end = find_word_end(script, pos);
    if &script[pos..in_end] != b"in" || !is_keyword_boundary(script, in_end) {
        io::write_str(2, b"sh: syntax error: expected 'in'\n");
        return script.len();
    }
    pos = in_end;

    // Get word list (until "do" or newline/semicolon)
    let do_pos = find_keyword(script, pos, b"do");
    if do_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'do'\n");
        return script.len();
    }
    let do_pos = do_pos.unwrap();

    let words_str = &script[pos..do_pos];
    let words = split_words(shell, words_str);

    pos = do_pos + 2;

    let done_pos = find_matching_done(script, pos);
    if done_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'done'\n");
        return script.len();
    }
    let done_pos = done_pos.unwrap();

    let body = &script[pos..done_pos];

    // Execute for each word
    for word in words {
        shell.set_var(&var_name, &word);
        super::execute_script(shell, body);
        if shell.should_exit {
            break;
        }
    }

    done_pos + 4
}

/// Find matching esac for a case statement, tracking nested case/esac depth
#[cfg(feature = "alloc")]
fn find_matching_esac(script: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    let mut depth = 1;

    while pos < script.len() && depth > 0 {
        pos = skip_whitespace_and_comments(script, pos);
        if pos >= script.len() {
            return None;
        }

        let word_end = find_word_end(script, pos);
        let word = &script[pos..word_end];

        if word == b"case" && is_keyword_boundary(script, word_end) {
            depth += 1;
        } else if word == b"esac" && is_keyword_boundary(script, word_end) {
            depth -= 1;
            if depth == 0 {
                return Some(pos);
            }
        }

        pos = skip_to_next_token(script, pos);
    }
    None
}

/// Execute case/in/esac
///
/// Matches a word against patterns and executes the corresponding commands.
/// Patterns are separated by `|` and terminated by `)`.
/// Pattern branches are terminated by `;;`.
///
/// Returns the position after the closing `esac`.
#[cfg(feature = "alloc")]
pub(super) fn execute_case(shell: &mut Shell, script: &[u8], start: usize) -> usize {
    let mut pos = start + 4; // skip "case"
    pos = skip_whitespace_and_comments(script, pos);

    // Get the word to match
    let (match_word, new_pos) = parse_word(shell, script, pos);
    pos = new_pos;

    pos = skip_whitespace_and_comments(script, pos);

    // Expect "in"
    let word_end = find_word_end(script, pos);
    if &script[pos..word_end] != b"in" || !is_keyword_boundary(script, word_end) {
        io::write_str(2, b"sh: syntax error: expected 'in'\n");
        return script.len();
    }
    pos = word_end;

    // Find matching esac (with depth tracking)
    let esac_pos = find_matching_esac(script, pos);
    if esac_pos.is_none() {
        io::write_str(2, b"sh: syntax error: expected 'esac'\n");
        return script.len();
    }
    let esac_pos = esac_pos.unwrap();

    // Parse and execute case patterns
    let mut matched = false;
    while pos < esac_pos && !matched {
        pos = skip_whitespace_and_comments(script, pos);
        if pos >= esac_pos {
            break;
        }

        // Find pattern(s) ending with )
        let paren_pos = script[pos..esac_pos].iter().position(|&c| c == b')');
        if paren_pos.is_none() {
            break;
        }
        let paren_pos = pos + paren_pos.unwrap();

        let patterns = &script[pos..paren_pos];
        pos = paren_pos + 1;

        // Find ;; or esac
        let end_pos = find_case_end(script, pos, esac_pos);
        let body = &script[pos..end_pos];

        // Check if any pattern matches
        for pattern in patterns.split(|&c| c == b'|') {
            let pattern = pattern.trim_ascii();
            if pattern_matches(&match_word, pattern) {
                super::execute_script(shell, body);
                matched = true;
                break;
            }
        }

        pos = end_pos;
        if pos < script.len() && script[pos..].starts_with(b";;") {
            pos += 2;
        }
    }

    esac_pos + 4
}

/// Check if word matches a shell pattern
///
/// Supports glob patterns with:
/// - `*` - matches any sequence of characters
/// - `?` - matches any single character
///
/// Returns true if the word matches the pattern.
#[cfg(feature = "alloc")]
pub(super) fn pattern_matches(word: &[u8], pattern: &[u8]) -> bool {
    if pattern == b"*" {
        return true;
    }

    let mut wi = 0;
    let mut pi = 0;
    let mut star_pi: Option<usize> = None;
    let mut star_wi: usize = 0;

    while wi < word.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == word[wi]) {
            wi += 1;
            pi += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = Some(pi);
            star_wi = wi;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_wi += 1;
            wi = star_wi;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}

/// Find end of case branch
///
/// Scans for `;;` (case branch terminator) or the esac position.
///
/// Returns the position of `;;` or `esac_pos` if not found.
#[cfg(feature = "alloc")]
pub(super) fn find_case_end(script: &[u8], start: usize, esac_pos: usize) -> usize {
    let mut pos = start;
    while pos < esac_pos {
        if script[pos..].starts_with(b";;") {
            return pos;
        }
        pos += 1;
    }
    esac_pos
}
