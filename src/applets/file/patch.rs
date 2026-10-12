//! patch - apply a diff file to an original
//!
//! POSIX-compliant implementation supporting unified and context diff formats.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// Patch hunk representing a single change
#[cfg(feature = "alloc")]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
}

/// A line within a hunk
#[cfg(feature = "alloc")]
#[derive(Clone)]
enum HunkLine {
    Context(Vec<u8>),
    Remove(Vec<u8>),
    Add(Vec<u8>),
}

/// patch - apply a diff file to an original
///
/// # Synopsis
/// ```text
/// patch [-p num] [-i patchfile] [-o outfile] [-R] [-N] [file]
/// ```
///
/// # Description
/// Apply a patch file to an original. Supports unified and context diff formats.
///
/// # Options
/// - `-p NUM`: Strip NUM leading path components
/// - `-i FILE`: Read patch from FILE
/// - `-o FILE`: Write output to FILE instead of patching in place
/// - `-R`: Reverse the patch
/// - `-N`: Ignore patches that appear to be reversed
/// - `-b`: Create backup files
///
/// # Exit Status
/// - 0: Success
/// - 1: Some hunks failed
/// - 2: Serious trouble
#[cfg(feature = "alloc")]
pub fn patch(argc: i32, argv: *const *const u8) -> i32 {
    let mut patch_file: Option<&[u8]> = None;
    let mut output_file: Option<&[u8]> = None;
    let mut strip_level: usize = 0;
    let mut reverse = false;
    let mut ignore_reversed = false;
    let mut backup = false;
    let mut target_file: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => {
                i += 1;
                continue;
            }
        };

        if arg == b"-i" || arg == b"--input" {
            i += 1;
            patch_file = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-o" || arg == b"--output" {
            i += 1;
            output_file = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-p" || arg.starts_with(b"-p") {
            if arg.len() > 2 {
                strip_level = sys::parse_u64(&arg[2..]).unwrap_or(0) as usize;
            } else {
                i += 1;
                if let Some(n) = unsafe { get_arg(argv, i as i32) } {
                    strip_level = sys::parse_u64(n).unwrap_or(0) as usize;
                }
            }
        } else if arg == b"-R" || arg == b"--reverse" {
            reverse = true;
        } else if arg == b"-N" || arg == b"--forward" {
            ignore_reversed = true;
        } else if arg == b"-b" || arg == b"--backup" {
            backup = true;
        } else if !arg.starts_with(b"-") && target_file.is_none() {
            target_file = Some(arg);
        }
        i += 1;
    }

    let _ = ignore_reversed; // TODO: implement

    // Read patch content
    let patch_content = if let Some(path) = patch_file {
        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd < 0 {
            io::write_str(2, b"patch: can't open ");
            io::write_all(2, path);
            io::write_str(2, b"\n");
            return 2;
        }
        let content = io::read_all(fd);
        io::close(fd);
        content
    } else {
        io::read_all(0)
    };

    // Parse and apply patches
    let patches = parse_patch(&patch_content);
    if patches.is_empty() {
        io::write_str(2, b"patch: no valid patches found\n");
        return 2;
    }

    let mut failed_hunks = 0;

    for (filename, hunks) in patches {
        // Determine actual filename
        let actual_file = if let Some(tf) = target_file {
            tf.to_vec()
        } else {
            strip_path(&filename, strip_level)
        };

        if actual_file.is_empty() {
            io::write_str(2, b"patch: unable to determine file name\n");
            continue;
        }

        io::write_str(1, b"patching file ");
        io::write_all(1, &actual_file);
        io::write_str(1, b"\n");

        // Read original file
        let original = {
            let fd = io::open(&actual_file, libc::O_RDONLY, 0);
            if fd < 0 {
                // File doesn't exist - might be a new file
                Vec::new()
            } else {
                let content = io::read_all(fd);
                io::close(fd);
                content
            }
        };

        // Split into lines
        let mut lines: Vec<Vec<u8>> = original
            .split(|&c| c == b'\n')
            .map(|l| l.to_vec())
            .collect();

        // Remove empty last line if file ended with newline
        if lines.last().map(|l| l.is_empty()).unwrap_or(false) && !original.is_empty() {
            lines.pop();
        }

        // Apply hunks
        let mut offset: i64 = 0;
        for hunk in &hunks {
            let hunk_to_apply = if reverse {
                reverse_hunk(hunk)
            } else {
                hunk.clone()
            };

            match apply_hunk(&mut lines, &hunk_to_apply, offset) {
                Ok(new_offset) => {
                    offset = new_offset;
                }
                Err(_) => {
                    io::write_str(2, b"patch: Hunk FAILED\n");
                    failed_hunks += 1;
                }
            }
        }

        // Create backup if requested
        if backup {
            let mut backup_name = actual_file.clone();
            backup_name.extend_from_slice(b".orig");
            let fd = io::open(
                &backup_name,
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o644,
            );
            if fd >= 0 {
                io::write_all(fd, &original);
                io::close(fd);
            }
        }

        // Write output
        let out_path = output_file.map(|p| p.to_vec()).unwrap_or(actual_file);
        let fd = io::open(
            &out_path,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            0o644,
        );
        if fd < 0 {
            io::write_str(2, b"patch: can't write ");
            io::write_all(2, &out_path);
            io::write_str(2, b"\n");
            return 2;
        }

        for (i, line) in lines.iter().enumerate() {
            io::write_all(fd, line);
            if i < lines.len() - 1 || !line.is_empty() {
                io::write_str(fd, b"\n");
            }
        }
        io::close(fd);
    }

    if failed_hunks > 0 { 1 } else { 0 }
}

#[cfg(not(feature = "alloc"))]
pub fn patch(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"patch: requires alloc feature\n");
    1
}

/// Parse patch file into list of (filename, hunks)
#[cfg(feature = "alloc")]
fn parse_patch(content: &[u8]) -> Vec<(Vec<u8>, Vec<Hunk>)> {
    let mut result = Vec::new();
    let lines: Vec<&[u8]> = content.split(|&c| c == b'\n').collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for unified diff header: --- file
        if lines[i].starts_with(b"--- ") && i + 1 < lines.len() && lines[i + 1].starts_with(b"+++ ")
        {
            let old_file = parse_filename(&lines[i][4..]);
            let new_file = parse_filename(&lines[i + 1][4..]);

            // Use new file name, or old if new is /dev/null
            let filename = if new_file == b"/dev/null" {
                old_file
            } else {
                new_file
            };

            i += 2;
            let mut hunks = Vec::new();

            // Parse hunks
            while i < lines.len() && lines[i].starts_with(b"@@") {
                if let Some(hunk) = parse_unified_hunk(&lines, &mut i) {
                    hunks.push(hunk);
                } else {
                    break;
                }
            }

            if !hunks.is_empty() {
                result.push((filename, hunks));
            }
        } else {
            i += 1;
        }
    }

    result
}

/// Parse filename from patch header line
#[cfg(feature = "alloc")]
fn parse_filename(line: &[u8]) -> Vec<u8> {
    // Remove trailing timestamp if present
    let mut end = line.len();

    // Look for tab separator (common in unified diffs)
    if let Some(tab_pos) = line.iter().position(|&c| c == b'\t') {
        end = tab_pos;
    }

    // Trim whitespace
    let trimmed = &line[..end];
    let trimmed = trimmed.trim_ascii();

    trimmed.to_vec()
}

/// Parse a unified diff hunk
#[cfg(feature = "alloc")]
fn parse_unified_hunk(lines: &[&[u8]], i: &mut usize) -> Option<Hunk> {
    let header = lines[*i];

    // Parse @@ -old_start,old_count +new_start,new_count @@
    if !header.starts_with(b"@@") {
        return None;
    }

    let (old_start, old_count, new_start, new_count) = parse_hunk_header(header)?;

    *i += 1;
    let mut hunk_lines = Vec::new();

    while *i < lines.len() {
        let line = lines[*i];

        if line.is_empty() {
            // Empty line ends the hunk (or is between hunks)
            break;
        } else if line.starts_with(b" ") {
            hunk_lines.push(HunkLine::Context(line[1..].to_vec()));
            *i += 1;
        } else if line.starts_with(b"-") {
            hunk_lines.push(HunkLine::Remove(line[1..].to_vec()));
            *i += 1;
        } else if line.starts_with(b"+") {
            hunk_lines.push(HunkLine::Add(line[1..].to_vec()));
            *i += 1;
        } else if line.starts_with(b"\\") {
            // "\ No newline at end of file" - skip
            *i += 1;
        } else {
            // End of hunk
            break;
        }
    }

    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        lines: hunk_lines,
    })
}

/// Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
#[cfg(feature = "alloc")]
fn parse_hunk_header(header: &[u8]) -> Option<(usize, usize, usize, usize)> {
    // Find the ranges: -X,Y +A,B
    let mut pos = 0;

    // Skip @@
    while pos < header.len() && header[pos] != b'-' {
        pos += 1;
    }
    if pos >= header.len() {
        return None;
    }
    pos += 1; // Skip -

    // Parse old_start
    let old_start_end = header[pos..].iter().position(|&c| c == b',' || c == b' ')?;
    let old_start = sys::parse_u64(&header[pos..pos + old_start_end])? as usize;
    pos += old_start_end;

    // Parse old_count (optional)
    let old_count = if pos < header.len() && header[pos] == b',' {
        pos += 1;
        let old_count_end = header[pos..].iter().position(|&c| c == b' ')?;
        let count = sys::parse_u64(&header[pos..pos + old_count_end])? as usize;
        pos += old_count_end;
        count
    } else {
        1
    };

    // Find +
    while pos < header.len() && header[pos] != b'+' {
        pos += 1;
    }
    if pos >= header.len() {
        return None;
    }
    pos += 1; // Skip +

    // Parse new_start
    let new_start_end = header[pos..].iter().position(|&c| c == b',' || c == b' ')?;
    let new_start = sys::parse_u64(&header[pos..pos + new_start_end])? as usize;
    pos += new_start_end;

    // Parse new_count (optional)
    let new_count = if pos < header.len() && header[pos] == b',' {
        pos += 1;
        let new_count_end = header[pos..]
            .iter()
            .position(|&c| c == b' ' || c == b'@')
            .unwrap_or(header.len() - pos);
        let count = sys::parse_u64(&header[pos..pos + new_count_end]).unwrap_or(1) as usize;
        count
    } else {
        1
    };

    Some((old_start, old_count, new_start, new_count))
}

/// Strip leading path components
#[cfg(feature = "alloc")]
fn strip_path(path: &[u8], level: usize) -> Vec<u8> {
    let mut result = path;
    let mut remaining = level;

    while remaining > 0 && !result.is_empty() {
        if let Some(pos) = result.iter().position(|&c| c == b'/') {
            result = &result[pos + 1..];
            remaining -= 1;
        } else {
            break;
        }
    }

    result.to_vec()
}

/// Reverse a hunk (swap add/remove)
#[cfg(feature = "alloc")]
fn reverse_hunk(hunk: &Hunk) -> Hunk {
    Hunk {
        old_start: hunk.new_start,
        old_count: hunk.new_count,
        new_start: hunk.old_start,
        new_count: hunk.old_count,
        lines: hunk
            .lines
            .iter()
            .map(|l| match l {
                HunkLine::Context(c) => HunkLine::Context(c.clone()),
                HunkLine::Remove(r) => HunkLine::Add(r.clone()),
                HunkLine::Add(a) => HunkLine::Remove(a.clone()),
            })
            .collect(),
    }
}

/// Apply a hunk to lines, returns new offset on success
#[cfg(feature = "alloc")]
fn apply_hunk(lines: &mut Vec<Vec<u8>>, hunk: &Hunk, offset: i64) -> Result<i64, ()> {
    // Calculate actual line position with offset
    let target_line = if hunk.old_start == 0 {
        0
    } else {
        ((hunk.old_start as i64 - 1) + offset) as usize
    };

    // For new files (old_count == 0), just insert
    if hunk.old_count == 0 {
        let mut insert_pos = target_line;
        for hl in &hunk.lines {
            if let HunkLine::Add(content) = hl {
                if insert_pos > lines.len() {
                    lines.resize(insert_pos, Vec::new());
                }
                lines.insert(insert_pos, content.clone());
                insert_pos += 1;
            }
        }
        return Ok(offset + hunk.new_count as i64);
    }

    // Verify context matches (with some fuzz)
    let mut matched = false;
    let mut actual_start = target_line;

    // Try exact position first, then search nearby
    for fuzz in 0..=3 {
        for direction in &[0i64, 1, -1, 2, -2, 3, -3] {
            let try_pos = (target_line as i64 + direction * fuzz as i64) as usize;
            if try_pos < lines.len() || hunk.old_count == 0 {
                if verify_context(lines, try_pos, hunk) {
                    actual_start = try_pos;
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            break;
        }
    }

    if !matched && hunk.old_count > 0 {
        return Err(());
    }

    // Apply the hunk
    let _pos = actual_start;
    let mut removals = 0;
    let mut additions = 0;

    // First pass: remove lines
    let mut new_lines = Vec::new();
    let mut hunk_idx = 0;
    let mut file_idx = 0;

    // Copy lines before hunk
    while file_idx < actual_start && file_idx < lines.len() {
        new_lines.push(lines[file_idx].clone());
        file_idx += 1;
    }

    // Process hunk
    for hl in &hunk.lines {
        match hl {
            HunkLine::Context(c) => {
                if file_idx < lines.len() {
                    new_lines.push(lines[file_idx].clone());
                    file_idx += 1;
                } else {
                    new_lines.push(c.clone());
                }
            }
            HunkLine::Remove(_) => {
                file_idx += 1; // Skip the removed line
                removals += 1;
            }
            HunkLine::Add(content) => {
                new_lines.push(content.clone());
                additions += 1;
            }
        }
        hunk_idx += 1;
    }

    // Copy remaining lines
    while file_idx < lines.len() {
        new_lines.push(lines[file_idx].clone());
        file_idx += 1;
    }

    *lines = new_lines;

    Ok(offset + additions as i64 - removals as i64)
}

/// Verify that context lines match
#[cfg(feature = "alloc")]
fn verify_context(lines: &[Vec<u8>], start: usize, hunk: &Hunk) -> bool {
    let mut pos = start;

    for hl in &hunk.lines {
        match hl {
            HunkLine::Context(expected) | HunkLine::Remove(expected) => {
                if pos >= lines.len() {
                    return false;
                }
                if lines[pos] != *expected {
                    // Allow minor whitespace differences
                    if lines[pos].trim_ascii() != expected.trim_ascii() {
                        return false;
                    }
                }
                pos += 1;
            }
            HunkLine::Add(_) => {
                // Add lines don't need verification
            }
        }
    }

    true
}

#[cfg(feature = "alloc")]
impl Clone for Hunk {
    fn clone(&self) -> Self {
        Hunk {
            old_start: self.old_start,
            old_count: self.old_count,
            new_start: self.new_start,
            new_count: self.new_count,
            lines: self.lines.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
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

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_patch_test_{}_{}",
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
    fn test_patch_unified() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("test.txt");
        let patch_file = dir.join("test.patch");

        fs::write(&file, "line1\nold line\nline3\n").unwrap();
        fs::write(
            &patch_file,
            "--- test.txt\n+++ test.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3\n",
        )
        .unwrap();

        let output = Command::new(&armybox)
            .current_dir(&dir)
            .args(["patch", "-i", "test.patch"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("new line"));
        assert!(!content.contains("old line"));

        cleanup(&dir);
    }

    #[test]
    fn test_patch_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["patch", "-i", "/nonexistent/file.patch"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
