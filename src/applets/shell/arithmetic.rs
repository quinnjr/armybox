//! Arithmetic evaluation for shell
//!
//! This module provides arithmetic expression evaluation for shell `$((expr))`
//! syntax. It supports:
//! - Basic operators: `+`, `-`, `*`, `/`, `%`
//! - Parentheses for grouping
//! - Negative numbers
//! - Variable references (both `$var` and bare `var`)
//!
//! The parser uses recursive descent with three precedence levels:
//! - Expression level: `+`, `-` (lowest precedence)
//! - Term level: `*`, `/`, `%`
//! - Factor level: numbers, parentheses, variables (highest precedence)

#[cfg(feature = "alloc")]
use crate::sys;

#[cfg(feature = "alloc")]
use super::expand_string;
#[cfg(feature = "alloc")]
use super::state::Shell;

/// Evaluate an arithmetic expression with variable expansion.
///
/// This is the main entry point for arithmetic evaluation. It first expands
/// any `$variable` references in the expression, then evaluates the result.
#[cfg(feature = "alloc")]
pub(super) fn eval_arithmetic(shell: &Shell, expr: &[u8]) -> i64 {
    // Expand $variables first
    let expanded = expand_string(shell, expr);
    // Then evaluate, passing shell for bare variable names
    eval_arith_expr_with_shell(shell, &expanded, 0).0
}

/// Arithmetic expression parser with shell variable lookup for bare names.
///
/// Handles addition and subtraction (lowest precedence).
/// Returns the evaluated value and the position after parsing.
#[cfg(feature = "alloc")]
pub(super) fn eval_arith_expr_with_shell(shell: &Shell, expr: &[u8], pos: usize) -> (i64, usize) {
    let (mut left, mut pos) = eval_arith_term_with_shell(shell, expr, pos);

    loop {
        pos = skip_arith_ws(expr, pos);
        if pos >= expr.len() {
            break;
        }

        let op = expr[pos];
        if op == b'+' {
            let (right, new_pos) = eval_arith_term_with_shell(shell, expr, pos + 1);
            left += right;
            pos = new_pos;
        } else if op == b'-' {
            let (right, new_pos) = eval_arith_term_with_shell(shell, expr, pos + 1);
            left -= right;
            pos = new_pos;
        } else {
            break;
        }
    }

    (left, pos)
}

/// Arithmetic term parser with shell variable lookup.
///
/// Handles multiplication, division, and modulo (medium precedence).
#[cfg(feature = "alloc")]
pub(super) fn eval_arith_term_with_shell(shell: &Shell, expr: &[u8], pos: usize) -> (i64, usize) {
    let (mut left, mut pos) = eval_arith_factor_with_shell(shell, expr, pos);

    loop {
        pos = skip_arith_ws(expr, pos);
        if pos >= expr.len() {
            break;
        }

        let op = expr[pos];
        if op == b'*' {
            let (right, new_pos) = eval_arith_factor_with_shell(shell, expr, pos + 1);
            left *= right;
            pos = new_pos;
        } else if op == b'/' {
            let (right, new_pos) = eval_arith_factor_with_shell(shell, expr, pos + 1);
            if right != 0 {
                left /= right;
            }
            pos = new_pos;
        } else if op == b'%' {
            let (right, new_pos) = eval_arith_factor_with_shell(shell, expr, pos + 1);
            if right != 0 {
                left %= right;
            }
            pos = new_pos;
        } else {
            break;
        }
    }

    (left, pos)
}

/// Arithmetic factor parser with shell variable lookup.
///
/// Handles numbers, parenthesized expressions, negative signs, and bare
/// variable names (highest precedence).
#[cfg(feature = "alloc")]
pub(super) fn eval_arith_factor_with_shell(shell: &Shell, expr: &[u8], pos: usize) -> (i64, usize) {
    let mut pos = skip_arith_ws(expr, pos);
    if pos >= expr.len() {
        return (0, pos);
    }

    // Handle parentheses
    if expr[pos] == b'(' {
        let (val, new_pos) = eval_arith_expr_with_shell(shell, expr, pos + 1);
        let mut pos = skip_arith_ws(expr, new_pos);
        if pos < expr.len() && expr[pos] == b')' {
            pos += 1;
        }
        return (val, pos);
    }

    // Handle negative
    if expr[pos] == b'-' {
        let (val, new_pos) = eval_arith_factor_with_shell(shell, expr, pos + 1);
        return (-val, new_pos);
    }

    // Parse number
    if expr[pos] >= b'0' && expr[pos] <= b'9' {
        let mut num: i64 = 0;
        while pos < expr.len() && expr[pos] >= b'0' && expr[pos] <= b'9' {
            num = num * 10 + (expr[pos] - b'0') as i64;
            pos += 1;
        }
        return (num, pos);
    }

    // Parse bare variable name
    if expr[pos].is_ascii_alphabetic() || expr[pos] == b'_' {
        let start = pos;
        while pos < expr.len() && (expr[pos].is_ascii_alphanumeric() || expr[pos] == b'_') {
            pos += 1;
        }
        let var_name = &expr[start..pos];
        // Look up variable and parse as number
        if let Some(value) = shell.get_var(var_name) {
            let num = sys::parse_i64(value).unwrap_or(0);
            return (num, pos);
        }
        return (0, pos);
    }

    (0, pos)
}

/// Simple arithmetic expression parser (without shell context).
///
/// Handles addition and subtraction. This version does not support
/// variable lookups.
#[cfg(feature = "alloc")]
#[allow(dead_code)]
pub(super) fn eval_arith_expr(expr: &[u8], pos: usize) -> (i64, usize) {
    let (mut left, mut pos) = eval_arith_term(expr, pos);

    loop {
        pos = skip_arith_ws(expr, pos);
        if pos >= expr.len() {
            break;
        }

        let op = expr[pos];
        if op == b'+' {
            let (right, new_pos) = eval_arith_term(expr, pos + 1);
            left += right;
            pos = new_pos;
        } else if op == b'-' {
            let (right, new_pos) = eval_arith_term(expr, pos + 1);
            left -= right;
            pos = new_pos;
        } else {
            break;
        }
    }

    (left, pos)
}

/// Simple arithmetic term parser (without shell context).
///
/// Handles multiplication, division, and modulo.
#[cfg(feature = "alloc")]
#[allow(dead_code)]
pub(super) fn eval_arith_term(expr: &[u8], pos: usize) -> (i64, usize) {
    let (mut left, mut pos) = eval_arith_factor(expr, pos);

    loop {
        pos = skip_arith_ws(expr, pos);
        if pos >= expr.len() {
            break;
        }

        let op = expr[pos];
        if op == b'*' {
            let (right, new_pos) = eval_arith_factor(expr, pos + 1);
            left *= right;
            pos = new_pos;
        } else if op == b'/' {
            let (right, new_pos) = eval_arith_factor(expr, pos + 1);
            if right != 0 {
                left /= right;
            }
            pos = new_pos;
        } else if op == b'%' {
            let (right, new_pos) = eval_arith_factor(expr, pos + 1);
            if right != 0 {
                left %= right;
            }
            pos = new_pos;
        } else {
            break;
        }
    }

    (left, pos)
}

/// Simple arithmetic factor parser (without shell context).
///
/// Handles numbers, parenthesized expressions, and negative signs.
/// Does not support variable lookups.
#[cfg(feature = "alloc")]
#[allow(dead_code)]
pub(super) fn eval_arith_factor(expr: &[u8], pos: usize) -> (i64, usize) {
    let mut pos = skip_arith_ws(expr, pos);
    if pos >= expr.len() {
        return (0, pos);
    }

    // Handle parentheses
    if expr[pos] == b'(' {
        let (val, new_pos) = eval_arith_expr(expr, pos + 1);
        let mut pos = skip_arith_ws(expr, new_pos);
        if pos < expr.len() && expr[pos] == b')' {
            pos += 1;
        }
        return (val, pos);
    }

    // Handle negative
    if expr[pos] == b'-' {
        let (val, new_pos) = eval_arith_factor(expr, pos + 1);
        return (-val, new_pos);
    }

    // Parse number
    let mut num: i64 = 0;
    while pos < expr.len() && expr[pos] >= b'0' && expr[pos] <= b'9' {
        num = num * 10 + (expr[pos] - b'0') as i64;
        pos += 1;
    }

    (num, pos)
}

/// Skip whitespace in arithmetic expressions.
///
/// Advances past spaces and tabs, returning the new position.
#[cfg(feature = "alloc")]
pub(super) fn skip_arith_ws(expr: &[u8], pos: usize) -> usize {
    let mut pos = pos;
    while pos < expr.len() && (expr[pos] == b' ' || expr[pos] == b'\t') {
        pos += 1;
    }
    pos
}
