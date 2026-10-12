//! chmod - change file mode bits
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/chmod.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// chmod - change file mode bits
///
/// # Synopsis
/// ```text
/// chmod [-R] mode file...
/// ```
///
/// # Description
/// Changes the file mode bits of each given file according to mode.
///
/// # Options
/// - `-R`: Recursive - change files and directories recursively
///
/// # Mode
/// Octal mode (e.g., 755, 644) or symbolic mode (e.g., u+x, go-w, a=rx)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn chmod(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"chmod: missing operand\n");
        return 1;
    }

    let mut recursive = false;
    let mut mode_arg_idx = 1;

    // Parse options
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-R" {
                recursive = true;
                mode_arg_idx = i + 1;
            } else if arg == b"--" {
                mode_arg_idx = i + 1;
                break;
            } else if arg.len() > 1 && arg[0] == b'-' && arg[1].is_ascii_alphabetic() {
                // Other options - skip
                mode_arg_idx = i + 1;
            } else {
                mode_arg_idx = i;
                break;
            }
        }
    }

    let mode_str = match unsafe { get_arg(argv, mode_arg_idx) } {
        Some(m) => m,
        None => {
            io::write_str(2, b"chmod: missing mode\n");
            return 1;
        }
    };

    // Try octal first
    let mode_spec = if let Some(octal) = sys::parse_octal(mode_str) {
        ModeSpec::Absolute(octal)
    } else {
        match parse_symbolic_mode(mode_str) {
            Some(spec) => spec,
            None => {
                io::write_str(2, b"chmod: invalid mode: '");
                io::write_all(2, mode_str);
                io::write_str(2, b"'\n");
                return 1;
            }
        }
    };

    let mut exit_code = 0;
    let files_start = mode_arg_idx + 1;

    if files_start >= argc {
        io::write_str(2, b"chmod: missing operand\n");
        return 1;
    }

    for i in files_start..argc {
        if let Some(path) = unsafe { get_arg(argv, i) } {
            if chmod_path(path, &mode_spec, recursive) != 0 {
                exit_code = 1;
            }
        }
    }
    exit_code
}

/// Mode specification - either absolute octal or symbolic operations
enum ModeSpec {
    Absolute(u32),
    Symbolic(SymbolicOps),
}

/// A set of symbolic mode operations
struct SymbolicOps {
    ops: [SymbolicOp; 8],
    count: usize,
}

/// A single symbolic mode operation
#[derive(Copy, Clone)]
struct SymbolicOp {
    who: u32,   // bitmask: 4=user, 2=group, 1=other, 7=all
    action: u8, // b'+', b'-', b'='
    perms: u32, // permission bits (rwxXst)
}

/// Parse symbolic mode string like "u+x,go-w,a=rx"
fn parse_symbolic_mode(s: &[u8]) -> Option<ModeSpec> {
    let mut ops = SymbolicOps {
        ops: [SymbolicOp {
            who: 0,
            action: 0,
            perms: 0,
        }; 8],
        count: 0,
    };

    // Split by commas and parse each clause
    let mut start = 0;
    let mut i = 0;
    while i <= s.len() {
        if i == s.len() || s[i] == b',' {
            if i > start {
                if !parse_symbolic_clause(&s[start..i], &mut ops) {
                    return None;
                }
            }
            start = i + 1;
        }
        i += 1;
    }

    if ops.count == 0 {
        return None;
    }

    Some(ModeSpec::Symbolic(ops))
}

/// Parse a single symbolic clause like "u+x" or "go-w" or "a=rx"
fn parse_symbolic_clause(s: &[u8], ops: &mut SymbolicOps) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut pos = 0;
    let mut who: u32 = 0;

    // Parse who part (ugoa)
    while pos < s.len() {
        match s[pos] {
            b'u' => who |= 4,
            b'g' => who |= 2,
            b'o' => who |= 1,
            b'a' => who |= 7,
            _ => break,
        }
        pos += 1;
    }

    // If no who specified, default to 'a' (all)
    if who == 0 {
        who = 7;
    }

    // Parse one or more action+perm pairs
    while pos < s.len() {
        let action = s[pos];
        if action != b'+' && action != b'-' && action != b'=' {
            return false;
        }
        pos += 1;

        let mut perms: u32 = 0;
        while pos < s.len() && s[pos] != b'+' && s[pos] != b'-' && s[pos] != b'=' && s[pos] != b','
        {
            match s[pos] {
                b'r' => perms |= 4,
                b'w' => perms |= 2,
                b'x' => perms |= 1,
                b'X' => perms |= 0o10, // special: exec only if dir or already exec
                b's' => perms |= 0o20, // setuid/setgid
                b't' => perms |= 0o40, // sticky
                _ => return false,
            }
            pos += 1;
        }

        if ops.count >= ops.ops.len() {
            return false;
        }
        ops.ops[ops.count] = SymbolicOp { who, action, perms };
        ops.count += 1;
    }

    true
}

/// Apply a mode spec to get a new mode from current mode
fn apply_mode(current: u32, spec: &ModeSpec, is_dir: bool) -> u32 {
    match spec {
        ModeSpec::Absolute(mode) => *mode,
        ModeSpec::Symbolic(ops) => {
            let mut mode = current & 0o7777;
            for i in 0..ops.count {
                let op = &ops.ops[i];
                let mut bits: u32 = 0;

                // Build the actual permission bits for the who mask
                let base_perms = op.perms & 0o7; // rwx

                if op.who & 4 != 0 {
                    bits |= base_perms << 6;
                    if op.perms & 0o20 != 0 {
                        bits |= 0o4000;
                    } // setuid
                }
                if op.who & 2 != 0 {
                    bits |= base_perms << 3;
                    if op.perms & 0o20 != 0 {
                        bits |= 0o2000;
                    } // setgid
                }
                if op.who & 1 != 0 {
                    bits |= base_perms;
                    if op.perms & 0o40 != 0 {
                        bits |= 0o1000;
                    } // sticky
                }

                // Handle X (conditional execute)
                if op.perms & 0o10 != 0 {
                    if is_dir || (current & 0o111) != 0 {
                        if op.who & 4 != 0 {
                            bits |= 0o100;
                        }
                        if op.who & 2 != 0 {
                            bits |= 0o010;
                        }
                        if op.who & 1 != 0 {
                            bits |= 0o001;
                        }
                    }
                }

                match op.action {
                    b'+' => mode |= bits,
                    b'-' => mode &= !bits,
                    b'=' => {
                        // Clear the who bits first, then set
                        let mut clear_mask: u32 = 0;
                        if op.who & 4 != 0 {
                            clear_mask |= 0o4700;
                        }
                        if op.who & 2 != 0 {
                            clear_mask |= 0o2070;
                        }
                        if op.who & 1 != 0 {
                            clear_mask |= 0o1007;
                        }
                        mode &= !clear_mask;
                        mode |= bits;
                    }
                    _ => {}
                }
            }
            mode
        }
    }
}

/// Apply chmod to a path, optionally recursing
fn chmod_path(path: &[u8], spec: &ModeSpec, recursive: bool) -> i32 {
    let mut st: libc::stat = unsafe { core::mem::zeroed() };
    if io::stat(path, &mut st) < 0 {
        sys::perror(path);
        return 1;
    }

    let is_dir = (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32;
    let new_mode = apply_mode(st.st_mode & 0o7777, spec, is_dir);

    if io::chmod(path, new_mode) < 0 {
        sys::perror(path);
        return 1;
    }

    if recursive && is_dir {
        return chmod_recursive(path, spec);
    }

    0
}

/// Recursively chmod a directory's contents
fn chmod_recursive(dir: &[u8], spec: &ModeSpec) -> i32 {
    let fd = io::open(dir, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        sys::perror(dir);
        return 1;
    }

    let mut exit_code = 0;
    let mut buf = [0u8; 4096];

    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }

        let mut offset = 0;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };

            if name != b"." && name != b".." {
                let mut full_path = [0u8; 4096];
                let mut len = 0;
                for &c in dir {
                    if len < full_path.len() - 1 {
                        full_path[len] = c;
                        len += 1;
                    }
                }
                if len < full_path.len() - 1 {
                    full_path[len] = b'/';
                    len += 1;
                }
                for &c in name {
                    if len < full_path.len() - 1 {
                        full_path[len] = c;
                        len += 1;
                    }
                }

                if chmod_path(&full_path[..len], spec, true) != 0 {
                    exit_code = 1;
                }
            }

            offset += dirent.d_reclen as usize;
        }
    }

    io::close(fd);
    exit_code
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let release = PathBuf::from("target/release/armybox");
        if release.exists() {
            return release;
        }
        PathBuf::from("target/debug/armybox")
    }

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_chmod_test_{}_{}",
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
    fn test_chmod_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "755", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_restrictive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "600", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");
        fs::write(&file1, "content").unwrap();
        fs::write(&file2, "content").unwrap();

        let output = Command::new(&armybox)
            .args([
                "chmod",
                "700",
                file1.to_str().unwrap(),
                file2.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            fs::metadata(&file1).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file2).unwrap().permissions().mode() & 0o777,
            0o700
        );
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_missing_operand() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chmod"]).output().unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_chmod_nonexistent_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["chmod", "755", "/nonexistent/file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }

    #[test]
    fn test_chmod_symbolic_add_execute() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "u+x", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o744);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_symbolic_remove_write() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "go-w", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_symbolic_set_all() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "a=rx", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o555);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_symbolic_comma_separated() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("file.txt");
        fs::write(&file, "content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "u=rwx,g=rx,o=r", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o754);
        cleanup(&dir);
    }

    #[test]
    fn test_chmod_recursive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let subdir = dir.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let file = subdir.join("file.txt");
        fs::write(&file, "content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        let output = Command::new(&armybox)
            .args(["chmod", "-R", "755", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(&subdir).unwrap().permissions().mode() & 0o777,
            0o755
        );
        cleanup(&dir);
    }
}
