//! Shell tokenization and parsing
//!
//! This module provides tokenization and parsing functionality for the shell,
//! including handling of quotes, escapes, redirections, and shell operators.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use super::expand::expand_dollar;
#[cfg(feature = "alloc")]
use super::state::Shell;

/// Token types for shell parsing
#[cfg(feature = "alloc")]
#[derive(Clone, PartialEq)]
pub(super) enum Token {
    Word(Vec<u8>),
    Pipe,
    AndIf, // &&
    OrIf,  // ||
    RedirectOut,
    RedirectAppend,
    RedirectIn,
    RedirectErr,
    Background,
}

/// Command structure representing a parsed shell command
#[cfg(feature = "alloc")]
pub(super) struct Command {
    pub(super) args: Vec<Vec<u8>>,
    pub(super) stdin_file: Option<Vec<u8>>,
    pub(super) stdout_file: Option<Vec<u8>>,
    pub(super) stdout_append: bool,
    pub(super) stderr_file: Option<Vec<u8>>,
    pub(super) background: bool,
}

#[cfg(feature = "alloc")]
impl Command {
    pub(super) fn new() -> Self {
        Command {
            args: Vec::new(),
            stdin_file: None,
            stdout_file: None,
            stdout_append: false,
            stderr_file: None,
            background: false,
        }
    }
}

/// Parse assignment value handling quotes and parentheses
#[cfg(feature = "alloc")]
pub(super) fn parse_assignment_value(s: &[u8]) -> (Vec<u8>, &[u8]) {
    let mut value = Vec::new();
    let mut pos = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut paren_depth = 0;

    while pos < s.len() {
        let c = s[pos];

        if in_single_quote {
            if c == b'\'' {
                in_single_quote = false;
            } else {
                value.push(c);
            }
            pos += 1;
        } else if in_double_quote {
            if c == b'"' {
                in_double_quote = false;
            } else if c == b'\\' && pos + 1 < s.len() {
                pos += 1;
                value.push(s[pos]);
            } else {
                value.push(c);
            }
            pos += 1;
        } else if c == b'\'' {
            in_single_quote = true;
            pos += 1;
        } else if c == b'"' {
            in_double_quote = true;
            pos += 1;
        } else if c == b'(' {
            paren_depth += 1;
            value.push(c);
            pos += 1;
        } else if c == b')' {
            if paren_depth > 0 {
                paren_depth -= 1;
            }
            value.push(c);
            pos += 1;
        } else if (c == b' ' || c == b'\t') && paren_depth == 0 {
            break;
        } else {
            value.push(c);
            pos += 1;
        }
    }

    (value, &s[pos..])
}

/// Tokenize input into shell tokens
#[cfg(feature = "alloc")]
pub(super) fn tokenize(shell: &Shell, input: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut pos = 0;

    while pos < input.len() {
        let c = input[pos];

        if c == b' ' || c == b'\t' {
            pos += 1;
            continue;
        }

        if c == b'#' {
            break;
        }

        if c == b'|' {
            if pos + 1 < input.len() && input[pos + 1] == b'|' {
                tokens.push(Token::OrIf);
                pos += 2;
            } else {
                tokens.push(Token::Pipe);
                pos += 1;
            }
            continue;
        }

        if c == b'&' {
            if pos + 1 < input.len() && input[pos + 1] == b'&' {
                tokens.push(Token::AndIf);
                pos += 2;
            } else {
                tokens.push(Token::Background);
                pos += 1;
            }
            continue;
        }

        if c == b'>' {
            if pos + 1 < input.len() && input[pos + 1] == b'>' {
                tokens.push(Token::RedirectAppend);
                pos += 2;
            } else {
                tokens.push(Token::RedirectOut);
                pos += 1;
            }
            continue;
        }

        if c == b'<' {
            tokens.push(Token::RedirectIn);
            pos += 1;
            continue;
        }

        if c == b'2' && pos + 1 < input.len() && input[pos + 1] == b'>' {
            tokens.push(Token::RedirectErr);
            pos += 2;
            continue;
        }

        // Parse word
        let (word, new_pos) = parse_word(shell, input, pos);
        if !word.is_empty() {
            tokens.push(Token::Word(word));
        }
        pos = new_pos;
    }

    tokens
}

/// Parse a word (handling quotes and expansions)
#[cfg(feature = "alloc")]
pub(super) fn parse_word(shell: &Shell, input: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut word = Vec::new();
    let mut pos = start;

    while pos < input.len() {
        let c = input[pos];

        if c == b' '
            || c == b'\t'
            || c == b'\n'
            || c == b';'
            || c == b'|'
            || c == b'&'
            || c == b'>'
            || c == b'<'
            || c == b'#'
        {
            break;
        }

        if c == b'\'' {
            // Single quote - literal
            pos += 1;
            while pos < input.len() && input[pos] != b'\'' {
                word.push(input[pos]);
                pos += 1;
            }
            if pos < input.len() {
                pos += 1;
            }
        } else if c == b'"' {
            // Double quote - allow expansions
            pos += 1;
            while pos < input.len() && input[pos] != b'"' {
                if input[pos] == b'\\' && pos + 1 < input.len() {
                    pos += 1;
                    word.push(input[pos]);
                    pos += 1;
                } else if input[pos] == b'$' {
                    let (expanded, new_pos) = expand_dollar(shell, input, pos);
                    word.extend_from_slice(&expanded);
                    pos = new_pos;
                } else {
                    word.push(input[pos]);
                    pos += 1;
                }
            }
            if pos < input.len() {
                pos += 1;
            }
        } else if c == b'\\' && pos + 1 < input.len() {
            pos += 1;
            word.push(input[pos]);
            pos += 1;
        } else if c == b'$' {
            let (expanded, new_pos) = expand_dollar(shell, input, pos);
            word.extend_from_slice(&expanded);
            pos = new_pos;
        } else {
            word.push(c);
            pos += 1;
        }
    }

    (word, pos)
}
