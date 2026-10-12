//! getopt - parse command options (POSIX compliant)
//!
//! Parses command-line options and outputs them in normalized form.

extern crate alloc;

use super::get_arg;
use crate::io;
use alloc::vec::Vec;

/// getopt - parse command options (POSIX compliant)
///
/// POSIX: Parses command-line options and outputs them in normalized form.
/// Usage: getopt optstring parameters...
/// Outputs: normalized options, then "--", then non-option arguments.
///
/// # Synopsis
/// ```text
/// getopt optstring parameters...
/// ```
///
/// # Exit Status
/// - 0: Success
/// - >0: Invalid option encountered
pub fn getopt(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"getopt: missing optstring\n");
        return 1;
    }

    let optstring = match unsafe { get_arg(argv, 1) } {
        Some(s) => s,
        None => return 1,
    };

    let mut options: Vec<Vec<u8>> = Vec::new();
    let mut operands: Vec<&[u8]> = Vec::new();
    let mut i = 2;
    let mut error = false;

    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"--" {
            // End of options
            i += 1;
            while i < argc as usize {
                if let Some(a) = unsafe { get_arg(argv, i as i32) } {
                    operands.push(a);
                }
                i += 1;
            }
            break;
        } else if arg.starts_with(b"-") && arg.len() > 1 && arg[1] != b'-' {
            // Short options
            let mut j = 1;
            while j < arg.len() {
                let opt = arg[j];
                let opt_pos = optstring.iter().position(|&c| c == opt);

                match opt_pos {
                    Some(pos) => {
                        // Check if option takes an argument
                        let takes_arg = pos + 1 < optstring.len() && optstring[pos + 1] == b':';

                        if takes_arg {
                            if j + 1 < arg.len() {
                                // Argument is attached: -oARG
                                let mut opt_str = Vec::with_capacity(3);
                                opt_str.push(b'-');
                                opt_str.push(opt);
                                options.push(opt_str);
                                let arg_val: Vec<u8> = arg[j + 1..].to_vec();
                                options.push(arg_val);
                                break;
                            } else {
                                // Argument is next parameter
                                i += 1;
                                let mut opt_str = Vec::with_capacity(3);
                                opt_str.push(b'-');
                                opt_str.push(opt);
                                options.push(opt_str);

                                if let Some(next_arg) = unsafe { get_arg(argv, i as i32) } {
                                    options.push(next_arg.to_vec());
                                } else {
                                    io::write_str(2, b"getopt: option requires an argument -- ");
                                    io::write_all(2, &[opt]);
                                    io::write_str(2, b"\n");
                                    error = true;
                                }
                                break;
                            }
                        } else {
                            // Option without argument
                            let mut opt_str = Vec::with_capacity(3);
                            opt_str.push(b'-');
                            opt_str.push(opt);
                            options.push(opt_str);
                        }
                    }
                    None => {
                        io::write_str(2, b"getopt: invalid option -- ");
                        io::write_all(2, &[opt]);
                        io::write_str(2, b"\n");
                        error = true;
                    }
                }
                j += 1;
            }
        } else {
            // Non-option argument
            operands.push(arg);
        }
        i += 1;
    }

    // Output options
    let mut first = true;
    for opt in &options {
        if !first {
            io::write_str(1, b" ");
        }
        // Quote if contains spaces
        if opt.contains(&b' ') || opt.contains(&b'\t') {
            io::write_str(1, b"'");
            io::write_all(1, opt);
            io::write_str(1, b"'");
        } else {
            io::write_all(1, opt);
        }
        first = false;
    }

    // Output separator
    if !first {
        io::write_str(1, b" ");
    }
    io::write_str(1, b"--");

    // Output operands
    for op in &operands {
        io::write_str(1, b" ");
        if op.contains(&b' ') || op.contains(&b'\t') {
            io::write_str(1, b"'");
            io::write_all(1, op);
            io::write_str(1, b"'");
        } else {
            io::write_all(1, op);
        }
    }

    io::write_str(1, b"\n");

    if error { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

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
    fn test_getopt_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["getopt", "ab:c", "-a", "-b", "val", "arg"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("-a"));
        assert!(stdout.contains("-b"));
        assert!(stdout.contains("--"));
    }
}
