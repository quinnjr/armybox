//! diff - compare files line by line
//!
//! This utility compares two files line by line using the LCS (Longest Common
//! Subsequence) algorithm. Output is in normal diff format with lines starting
//! with `<` for deletions, `>` for additions, and `---` as separator between
//! deletion and addition blocks.
//!
//! # Exit Status
//! - 0: Files are identical
//! - 1: Files differ
//! - 2: Error (missing arguments, file not found, etc.)

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use super::get_arg;
use crate::io;
use crate::sys;

/// Compare two files line by line using LCS algorithm
///
/// # Arguments
/// - `argc`: Argument count (must be at least 3)
/// - `argv`: Argument vector (argv[1] = file1, argv[2] = file2)
///
/// # Returns
/// - `0` if files are identical
/// - `1` if files differ
/// - `2` on error
#[cfg(feature = "alloc")]
pub fn diff(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        return 2;
    }

    let path1 = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => return 2,
    };
    let path2 = match unsafe { get_arg(argv, 2) } {
        Some(p) => p,
        None => return 2,
    };

    // Read first file
    let fd1 = io::open(path1, libc::O_RDONLY, 0);
    if fd1 < 0 {
        io::write_str(2, b"diff: ");
        io::write_all(2, path1);
        io::write_str(2, b": No such file or directory\n");
        return 2;
    }
    let content1 = io::read_all(fd1);
    io::close(fd1);

    // Read second file
    let fd2 = io::open(path2, libc::O_RDONLY, 0);
    if fd2 < 0 {
        io::write_str(2, b"diff: ");
        io::write_all(2, path2);
        io::write_str(2, b": No such file or directory\n");
        return 2;
    }
    let content2 = io::read_all(fd2);
    io::close(fd2);

    // Split into lines
    let lines1: Vec<&[u8]> = content1.split(|&c| c == b'\n').collect();
    let lines2: Vec<&[u8]> = content2.split(|&c| c == b'\n').collect();

    // Simple LCS-based diff using dynamic programming
    let m = lines1.len();
    let n = lines2.len();

    // Build LCS table
    let mut lcs = Vec::new();
    lcs.resize((m + 1) * (n + 1), 0usize);

    for i in 1..=m {
        for j in 1..=n {
            if lines1[i - 1] == lines2[j - 1] {
                lcs[i * (n + 1) + j] = lcs[(i - 1) * (n + 1) + (j - 1)] + 1;
            } else {
                lcs[i * (n + 1) + j] =
                    core::cmp::max(lcs[(i - 1) * (n + 1) + j], lcs[i * (n + 1) + (j - 1)]);
            }
        }
    }

    // Trace back to find differences
    // type: -1=del, +1=add, 0=same, line1_idx, line2_idx
    let mut changes: Vec<(i32, usize, usize)> = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && lines1[i - 1] == lines2[j - 1] {
            changes.push((0, i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i * (n + 1) + (j - 1)] >= lcs[(i - 1) * (n + 1) + j]) {
            changes.push((1, 0, j - 1)); // addition
            j -= 1;
        } else if i > 0 {
            changes.push((-1, i - 1, 0)); // deletion
            i -= 1;
        }
    }

    changes.reverse();

    // Output in normal diff format
    let mut has_diff = false;
    let mut i = 0;
    while i < changes.len() {
        let (change_type, idx1, idx2) = changes[i];

        if change_type == 0 {
            i += 1;
            continue;
        }

        has_diff = true;

        // Find range of consecutive changes
        let start_i = i;
        let mut del_start = if change_type == -1 { idx1 + 1 } else { 0 };
        let mut del_end = del_start;
        let mut add_start = if change_type == 1 { idx2 + 1 } else { 0 };
        let mut add_end = add_start;

        while i < changes.len() {
            let (ct, i1, i2) = changes[i];
            if ct == 0 {
                break;
            }
            if ct == -1 {
                if del_start == 0 {
                    del_start = i1 + 1;
                }
                del_end = i1 + 1;
            } else {
                if add_start == 0 {
                    add_start = i2 + 1;
                }
                add_end = i2 + 1;
            }
            i += 1;
        }

        // Output header
        let mut num_buf = [0u8; 20];

        if del_start > 0 && add_start > 0 {
            // Change
            if del_start == del_end {
                io::write_all(1, sys::format_u64(del_start as u64, &mut num_buf));
            } else {
                io::write_all(1, sys::format_u64(del_start as u64, &mut num_buf));
                io::write_str(1, b",");
                io::write_all(1, sys::format_u64(del_end as u64, &mut num_buf));
            }
            io::write_str(1, b"c");
            if add_start == add_end {
                io::write_all(1, sys::format_u64(add_start as u64, &mut num_buf));
            } else {
                io::write_all(1, sys::format_u64(add_start as u64, &mut num_buf));
                io::write_str(1, b",");
                io::write_all(1, sys::format_u64(add_end as u64, &mut num_buf));
            }
            io::write_str(1, b"\n");
        } else if del_start > 0 {
            // Deletion
            if del_start == del_end {
                io::write_all(1, sys::format_u64(del_start as u64, &mut num_buf));
            } else {
                io::write_all(1, sys::format_u64(del_start as u64, &mut num_buf));
                io::write_str(1, b",");
                io::write_all(1, sys::format_u64(del_end as u64, &mut num_buf));
            }
            io::write_str(1, b"d");
            io::write_all(
                1,
                sys::format_u64(add_start.saturating_sub(1).max(1) as u64, &mut num_buf),
            );
            io::write_str(1, b"\n");
        } else {
            // Addition
            io::write_all(
                1,
                sys::format_u64(del_start.saturating_sub(1).max(1) as u64, &mut num_buf),
            );
            io::write_str(1, b"a");
            if add_start == add_end {
                io::write_all(1, sys::format_u64(add_start as u64, &mut num_buf));
            } else {
                io::write_all(1, sys::format_u64(add_start as u64, &mut num_buf));
                io::write_str(1, b",");
                io::write_all(1, sys::format_u64(add_end as u64, &mut num_buf));
            }
            io::write_str(1, b"\n");
        }

        // Output deleted lines
        for k in start_i..i {
            let (ct, i1, _i2) = changes[k];
            if ct == -1 {
                io::write_str(1, b"< ");
                io::write_all(1, lines1[i1]);
                io::write_str(1, b"\n");
            }
        }

        // Output separator if we have both deletions and additions
        if del_start > 0 && add_start > 0 {
            io::write_str(1, b"---\n");
        }

        // Output added lines
        for k in start_i..i {
            let (ct, _i1, i2) = changes[k];
            if ct == 1 {
                io::write_str(1, b"> ");
                io::write_all(1, lines2[i2]);
                io::write_str(1, b"\n");
            }
        }
    }

    if has_diff { 1 } else { 0 }
}

/// Stub implementation for non-alloc builds
#[cfg(not(feature = "alloc"))]
pub fn diff(_argc: i32, _argv: *const *const u8) -> i32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create argv-style arguments for testing
    #[cfg(feature = "alloc")]
    fn make_argv(args: &[&[u8]]) -> (Vec<*const u8>, i32) {
        let ptrs: Vec<*const u8> = args.iter().map(|s| s.as_ptr()).collect();
        let argc = args.len() as i32;
        (ptrs, argc)
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_diff_identical_files() {
        use std::io::Write;

        // Create two identical temporary files
        let dir = std::env::temp_dir();
        let path1 = dir.join("diff_test_identical1.txt");
        let path2 = dir.join("diff_test_identical2.txt");

        let content = b"line 1\nline 2\nline 3\n";
        std::fs::write(&path1, content).unwrap();
        std::fs::write(&path2, content).unwrap();

        // Convert paths to null-terminated byte strings
        let path1_bytes: Vec<u8> = path1
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();
        let path2_bytes: Vec<u8> = path2
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();

        let args: [&[u8]; 3] = [b"diff\0", &path1_bytes, &path2_bytes];
        let (argv, argc) = make_argv(&args);

        let result = diff(argc, argv.as_ptr());

        // Clean up
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);

        assert_eq!(result, 0, "Identical files should return 0");
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_diff_different_files() {
        // Create two different temporary files
        let dir = std::env::temp_dir();
        let path1 = dir.join("diff_test_different1.txt");
        let path2 = dir.join("diff_test_different2.txt");

        let content1 = b"line 1\nline 2\nline 3\n";
        let content2 = b"line 1\nmodified line\nline 3\n";
        std::fs::write(&path1, content1).unwrap();
        std::fs::write(&path2, content2).unwrap();

        // Convert paths to null-terminated byte strings
        let path1_bytes: Vec<u8> = path1
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();
        let path2_bytes: Vec<u8> = path2
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();

        let args: [&[u8]; 3] = [b"diff\0", &path1_bytes, &path2_bytes];
        let (argv, argc) = make_argv(&args);

        let result = diff(argc, argv.as_ptr());

        // Clean up
        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);

        assert_eq!(result, 1, "Different files should return 1");
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_diff_nonexistent_file() {
        let dir = std::env::temp_dir();
        let path1 = dir.join("diff_test_nonexistent_abc123xyz.txt");
        let path2 = dir.join("diff_test_existing.txt");

        // Only create the second file
        std::fs::write(&path2, b"test content\n").unwrap();

        // Convert paths to null-terminated byte strings
        let path1_bytes: Vec<u8> = path1
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();
        let path2_bytes: Vec<u8> = path2
            .to_str()
            .unwrap()
            .as_bytes()
            .iter()
            .copied()
            .chain(core::iter::once(0))
            .collect();

        let args: [&[u8]; 3] = [b"diff\0", &path1_bytes, &path2_bytes];
        let (argv, argc) = make_argv(&args);

        let result = diff(argc, argv.as_ptr());

        // Clean up
        let _ = std::fs::remove_file(&path2);

        assert_eq!(result, 2, "Nonexistent file should return 2");
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_diff_insufficient_args() {
        let args: [&[u8]; 2] = [b"diff\0", b"onlyonefile\0"];
        let (argv, argc) = make_argv(&args);

        let result = diff(argc, argv.as_ptr());

        assert_eq!(result, 2, "Insufficient arguments should return 2");
    }

    #[test]
    #[cfg(not(feature = "alloc"))]
    fn test_diff_no_alloc() {
        let result = diff(3, core::ptr::null());
        assert_eq!(result, 2, "Non-alloc build should return 2");
    }
}
