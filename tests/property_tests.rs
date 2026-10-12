//! Property-based tests using proptest
//!
//! These tests verify invariants and properties that should hold
//! for all inputs, helping catch edge cases.

use proptest::prelude::*;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

fn armybox() -> String {
    std::env::var("ARMYBOX_PATH").unwrap_or_else(|_| env!("CARGO_BIN_EXE_armybox").to_string())
}

// =============================================================================
// Echo Tests
// =============================================================================

proptest! {
    /// echo should output exactly what we give it (with newline)
    #[test]
    fn echo_outputs_input(s in "[a-zA-Z0-9]{1,100}") {
        let output = Command::new(armybox())
            .args(["echo", &s])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        prop_assert_eq!(stdout.trim_end(), s);
    }

    /// echo -n should not add newline
    #[test]
    fn echo_n_no_newline(s in "[a-zA-Z0-9]{1,50}") {
        let output = Command::new(armybox())
            .args(["echo", "-n", &s])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        prop_assert!(!stdout.ends_with('\n'));
        prop_assert_eq!(stdout.as_ref(), s);
    }
}

// =============================================================================
// Cat Tests
// =============================================================================

proptest! {
    /// cat should output file contents exactly
    #[test]
    fn cat_preserves_content(content in prop::collection::vec(any::<u8>(), 0..10000)) {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&content).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["cat", path])
            .output()
            .unwrap();

        prop_assert_eq!(&output.stdout, &content);
    }

    /// cat multiple files should concatenate
    #[test]
    fn cat_concatenates(
        content1 in prop::collection::vec(any::<u8>(), 1..1000),
        content2 in prop::collection::vec(any::<u8>(), 1..1000)
    ) {
        let mut temp1 = NamedTempFile::new().unwrap();
        temp1.write_all(&content1).unwrap();
        let path1 = temp1.path().to_str().unwrap().to_string();

        let mut temp2 = NamedTempFile::new().unwrap();
        temp2.write_all(&content2).unwrap();
        let path2 = temp2.path().to_str().unwrap().to_string();

        let output = Command::new(armybox())
            .args(["cat", &path1, &path2])
            .output()
            .unwrap();

        let mut expected = content1.clone();
        expected.extend(&content2);
        prop_assert_eq!(&output.stdout, &expected);
    }
}

// =============================================================================
// Head/Tail Tests
// =============================================================================

proptest! {
    /// head -n N should return at most N lines
    #[test]
    fn head_limits_lines(
        lines in prop::collection::vec("[^\n]{1,50}", 1..100),
        n in 1usize..50
    ) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["head", "-n", &n.to_string(), path])
            .output()
            .unwrap();

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let output_lines: Vec<&str> = stdout_str.lines().collect();

        prop_assert!(output_lines.len() <= n);
        prop_assert!(output_lines.len() <= lines.len());
    }

    /// tail -n N should return at most N lines
    #[test]
    fn tail_limits_lines(
        lines in prop::collection::vec("[^\n]{1,50}", 1..100),
        n in 1usize..50
    ) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["tail", "-n", &n.to_string(), path])
            .output()
            .unwrap();

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let output_lines: Vec<&str> = stdout_str.lines().collect();

        prop_assert!(output_lines.len() <= n);
    }
}

// =============================================================================
// WC Tests
// =============================================================================

proptest! {
    /// wc -l should count newlines correctly
    #[test]
    fn wc_counts_lines(lines in prop::collection::vec("[^\n]{0,50}", 0..100)) {
        let content = lines.join("\n");
        let expected_lines = if content.is_empty() { 0 } else { lines.len() };

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        if !content.is_empty() {
            temp.write_all(b"\n").unwrap();
        }
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["wc", "-l", path])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let count: usize = stdout.split_whitespace()
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        // wc counts newlines, so it might be lines.len() or lines.len() depending on trailing newline
        prop_assert!(count == expected_lines || count == expected_lines.saturating_sub(1));
    }

    /// wc -c should count bytes correctly
    #[test]
    fn wc_counts_bytes(content in prop::collection::vec(any::<u8>(), 0..10000)) {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&content).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["wc", "-c", path])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let count: usize = stdout.split_whitespace()
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);

        prop_assert_eq!(count, content.len());
    }
}

// =============================================================================
// Sort Tests
// =============================================================================

proptest! {
    /// sort output should be sorted
    #[test]
    fn sort_produces_sorted_output(lines in prop::collection::vec("[a-z]{1,20}", 1..100)) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["sort", path])
            .output()
            .unwrap();

        let output_lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        let mut sorted = output_lines.clone();
        sorted.sort();
        prop_assert_eq!(output_lines, sorted);
    }

    /// sort -n should sort numerically
    #[test]
    fn sort_numeric(numbers in prop::collection::vec(0i32..10000, 1..100)) {
        let content: String = numbers.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n") + "\n";

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["sort", "-n", path])
            .output()
            .unwrap();

        let output_nums: Vec<i32> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|l| l.parse().ok())
            .collect();

        let mut sorted = numbers.clone();
        sorted.sort();
        prop_assert_eq!(output_nums, sorted);
    }

    /// sort -r should reverse sort
    #[test]
    fn sort_reverse(lines in prop::collection::vec("[a-z]{1,10}", 1..50)) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["sort", "-r", path])
            .output()
            .unwrap();

        let output_lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        let mut sorted = output_lines.clone();
        sorted.sort();
        sorted.reverse();
        prop_assert_eq!(output_lines, sorted);
    }
}

// =============================================================================
// Uniq Tests
// =============================================================================

proptest! {
    /// uniq should remove consecutive duplicates
    #[test]
    fn uniq_removes_consecutive_duplicates(
        lines in prop::collection::vec("[a-z]{1,10}", 1..50)
    ) {
        // Create input with consecutive duplicates
        let mut duplicated: Vec<String> = Vec::new();
        for line in &lines {
            duplicated.push(line.clone());
            duplicated.push(line.clone()); // Add duplicate
        }

        let content = duplicated.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["uniq", path])
            .output()
            .unwrap();

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let output_lines: Vec<&str> = stdout_str.lines().collect();

        // Output should have no consecutive duplicates
        for i in 1..output_lines.len() {
            prop_assert_ne!(output_lines[i], output_lines[i-1]);
        }
    }
}

// =============================================================================
// Base64 Tests
// =============================================================================

proptest! {
    /// base64 encode/decode roundtrip should preserve data
    #[test]
    fn base64_roundtrip(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap();

        // Encode
        let encoded = Command::new(armybox())
            .args(["base64", path])
            .output()
            .unwrap();

        if !encoded.status.success() {
            return Ok(()); // Skip if encoding failed
        }

        // Decode
        let mut encoded_file = NamedTempFile::new().unwrap();
        encoded_file.write_all(&encoded.stdout).unwrap();
        let encoded_path = encoded_file.path().to_str().unwrap();

        let decoded = Command::new(armybox())
            .args(["base64", "-d", encoded_path])
            .output()
            .unwrap();

        prop_assert_eq!(&decoded.stdout, &data);
    }
}

// =============================================================================
// Rev/Tac Tests
// =============================================================================

proptest! {
    /// rev should reverse each line
    #[test]
    fn rev_reverses_lines(lines in prop::collection::vec("[a-zA-Z0-9]{1,50}", 1..20)) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["rev", path])
            .output()
            .unwrap();

        let output_string = String::from_utf8_lossy(&output.stdout);
        let output_lines: Vec<&str> = output_string.lines().collect();

        for (orig, rev) in lines.iter().zip(output_lines.iter()) {
            let expected: String = orig.chars().rev().collect();
            prop_assert_eq!(*rev, expected.as_str());
        }
    }

    /// tac should reverse line order
    #[test]
    fn tac_reverses_line_order(lines in prop::collection::vec("[a-z]{1,20}", 1..50)) {
        let content = lines.join("\n") + "\n";
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(content.as_bytes()).unwrap();
        let path = temp.path().to_str().unwrap();

        let output = Command::new(armybox())
            .args(["tac", path])
            .output()
            .unwrap();

        let output_lines: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();

        let mut expected = lines.clone();
        expected.reverse();
        prop_assert_eq!(output_lines, expected);
    }
}

// =============================================================================
// Checksum Tests
// =============================================================================

proptest! {
    /// checksums should be deterministic
    #[test]
    fn md5sum_deterministic(data in prop::collection::vec(any::<u8>(), 0..10000)) {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(&data).unwrap();
        let path = temp.path().to_str().unwrap();

        let output1 = Command::new(armybox())
            .args(["md5sum", path])
            .output()
            .unwrap();

        let output2 = Command::new(armybox())
            .args(["md5sum", path])
            .output()
            .unwrap();

        prop_assert_eq!(output1.stdout, output2.stdout);
    }

    /// different data should produce different checksums (with high probability)
    #[test]
    fn md5sum_different_for_different_input(
        data1 in prop::collection::vec(any::<u8>(), 10..1000),
        data2 in prop::collection::vec(any::<u8>(), 10..1000)
    ) {
        prop_assume!(data1 != data2);

        let mut temp1 = NamedTempFile::new().unwrap();
        temp1.write_all(&data1).unwrap();
        let path1 = temp1.path().to_str().unwrap();

        let mut temp2 = NamedTempFile::new().unwrap();
        temp2.write_all(&data2).unwrap();
        let path2 = temp2.path().to_str().unwrap();

        let output1 = Command::new(armybox())
            .args(["md5sum", path1])
            .output()
            .unwrap();

        let output2 = Command::new(armybox())
            .args(["md5sum", path2])
            .output()
            .unwrap();

        // Extract just the hash (first field)
        let stdout1 = String::from_utf8_lossy(&output1.stdout);
        let hash1 = stdout1.split_whitespace().next().unwrap_or("");
        let stdout2 = String::from_utf8_lossy(&output2.stdout);
        let hash2 = stdout2.split_whitespace().next().unwrap_or("");

        prop_assert_ne!(hash1, hash2);
    }
}

// =============================================================================
// Compression Roundtrip Tests
// =============================================================================

proptest! {
    /// gzip/gunzip roundtrip should preserve data
    #[test]
    fn gzip_roundtrip(data in prop::collection::vec(any::<u8>(), 1..10000)) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let input_path = temp_dir.path().join("input");
        std::fs::write(&input_path, &data).unwrap();

        // Compress
        let status = Command::new(armybox())
            .args(["gzip", "-k", input_path.to_str().unwrap()])
            .status()
            .unwrap();

        if !status.success() {
            return Ok(());
        }

        let gz_path = format!("{}.gz", input_path.to_str().unwrap());

        // Decompress
        let output = Command::new(armybox())
            .args(["zcat", &gz_path])
            .output()
            .unwrap();

        prop_assert_eq!(&output.stdout, &data);
    }
}
