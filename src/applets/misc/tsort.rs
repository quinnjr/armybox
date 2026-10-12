//! tsort - topological sort (POSIX compliant)
//!
//! Reads pairs of strings and outputs topological order.

extern crate alloc;

use super::get_arg;
use crate::io;
use alloc::collections::BTreeMap;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

/// tsort - topological sort (POSIX compliant)
///
/// POSIX: Reads pairs of strings from stdin or file, outputs topological order.
/// Each pair "a b" means "a" depends on "b" (b must come before a).
/// Detects cycles and reports them to stderr.
///
/// # Synopsis
/// ```text
/// tsort [file]
/// ```
///
/// # Exit Status
/// - 0: Success
/// - >0: Cycle detected or error
pub fn tsort(argc: i32, argv: *const *const u8) -> i32 {
    // Open input file or use stdin
    let fd = if argc > 1 {
        match unsafe { get_arg(argv, 1) } {
            Some(filename) if filename == b"-" => 0,
            Some(filename) => {
                let fd = io::open(filename, libc::O_RDONLY, 0);
                if fd < 0 {
                    io::write_str(2, b"tsort: ");
                    io::write_all(2, filename);
                    io::write_str(2, b": No such file or directory\n");
                    return 1;
                }
                fd
            }
            None => 0,
        }
    } else {
        0 // stdin
    };

    // Read all input
    let content = io::read_all(fd);
    if fd > 0 {
        io::close(fd);
    }

    // Parse pairs and build graph
    // Graph: node -> set of nodes it depends on (must come before it)
    let mut nodes: BTreeSet<Vec<u8>> = BTreeSet::new();
    let mut edges: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new(); // node -> predecessors
    let mut successors: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = BTreeMap::new(); // node -> what depends on it

    let mut i = 0;
    while i < content.len() {
        // Skip whitespace
        while i < content.len()
            && (content[i] == b' '
                || content[i] == b'\t'
                || content[i] == b'\n'
                || content[i] == b'\r')
        {
            i += 1;
        }
        if i >= content.len() {
            break;
        }

        // Read first word
        let start1 = i;
        while i < content.len() && content[i] != b' ' && content[i] != b'\t' && content[i] != b'\n'
        {
            i += 1;
        }
        let word1: Vec<u8> = content[start1..i].to_vec();

        // Skip whitespace between words
        while i < content.len() && (content[i] == b' ' || content[i] == b'\t') {
            i += 1;
        }

        // Read second word (if present on same line)
        let start2 = i;
        while i < content.len() && content[i] != b' ' && content[i] != b'\t' && content[i] != b'\n'
        {
            i += 1;
        }
        let word2: Vec<u8> = content[start2..i].to_vec();

        // Add nodes
        if !word1.is_empty() {
            nodes.insert(word1.clone());
            if !edges.contains_key(&word1) {
                edges.insert(word1.clone(), BTreeSet::new());
            }
            if !successors.contains_key(&word1) {
                successors.insert(word1.clone(), BTreeSet::new());
            }
        }
        if !word2.is_empty() {
            nodes.insert(word2.clone());
            if !edges.contains_key(&word2) {
                edges.insert(word2.clone(), BTreeSet::new());
            }
            if !successors.contains_key(&word2) {
                successors.insert(word2.clone(), BTreeSet::new());
            }
        }

        // Add edge: word1 depends on word2 (word2 must come before word1)
        if !word1.is_empty() && !word2.is_empty() && word1 != word2 {
            edges.get_mut(&word1).unwrap().insert(word2.clone());
            successors.get_mut(&word2).unwrap().insert(word1.clone());
        }
    }

    // Kahn's algorithm for topological sort
    let mut result: Vec<Vec<u8>> = Vec::new();
    let mut in_degree: BTreeMap<Vec<u8>, usize> = BTreeMap::new();

    // Calculate in-degrees
    for node in &nodes {
        let deg = edges.get(node).map(|s| s.len()).unwrap_or(0);
        in_degree.insert(node.clone(), deg);
    }

    // Find all nodes with no dependencies
    let mut queue: Vec<Vec<u8>> = Vec::new();
    for node in &nodes {
        if *in_degree.get(node).unwrap_or(&0) == 0 {
            queue.push(node.clone());
        }
    }

    // Sort queue for deterministic output
    queue.sort();

    while !queue.is_empty() {
        // Take first (alphabetically) for deterministic output
        let node = queue.remove(0);
        result.push(node.clone());

        // Remove this node from graph
        if let Some(succs) = successors.get(&node) {
            for succ in succs.iter() {
                if let Some(deg) = in_degree.get_mut(succ) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        // Insert in sorted position for determinism
                        let pos = queue.iter().position(|x| x > succ).unwrap_or(queue.len());
                        queue.insert(pos, succ.clone());
                    }
                }
            }
        }
    }

    // Check for cycles
    let has_cycle = result.len() != nodes.len();

    // Output results
    for node in &result {
        io::write_all(1, node);
        io::write_str(1, b"\n");
    }

    if has_cycle {
        io::write_str(2, b"tsort: input contains a loop\n");
        // Output remaining nodes (in cycle)
        for node in &nodes {
            if !result.contains(node) {
                io::write_all(1, node);
                io::write_str(1, b"\n");
            }
        }
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_tsort_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["tsort"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(b"a b\nb c\n").unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("c"));
    }
}
