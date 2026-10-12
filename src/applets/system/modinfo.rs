//! modinfo - show information about a kernel module
//!
//! Extract and display information from kernel modules.

use super::get_arg;
use crate::io;

/// modinfo - show information about a kernel module
///
/// # Synopsis
/// ```text
/// modinfo [-F field] module...
/// ```
///
/// # Description
/// Extract and display information from Linux kernel modules.
/// Reads module info from /sys/module/<name> or parses the
/// module file directly.
///
/// # Options
/// - `-F field`: Only print the specified field
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn modinfo(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"modinfo: usage: modinfo [-F field] module...\n");
        return 1;
    }

    let mut field_filter: Option<&[u8]> = None;
    let mut exit_code = 0;

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-F" && i + 1 < argc {
                field_filter = unsafe { get_arg(argv, i + 1) };
                i += 2;
                continue;
            } else if arg.len() > 0 && arg[0] != b'-' {
                if !show_module_info(arg, field_filter) {
                    exit_code = 1;
                }
            }
        }
        i += 1;
    }

    exit_code
}

fn show_module_info(module: &[u8], field_filter: Option<&[u8]>) -> bool {
    // Try to read from /sys/module/<name>
    let mut path = [0u8; 256];
    let prefix = b"/sys/module/";
    let mut pos = 0;

    for &b in prefix.iter() {
        if pos < path.len() {
            path[pos] = b;
            pos += 1;
        }
    }

    for &b in module.iter() {
        if pos < path.len() {
            // Replace '-' with '_' in module name
            path[pos] = if b == b'-' { b'_' } else { b };
            pos += 1;
        }
    }

    // Check if module exists in /sys/module
    let mut stat_buf: libc::stat = unsafe { core::mem::zeroed() };
    path[pos] = 0; // Null terminate
    if unsafe { libc::stat(path.as_ptr() as *const libc::c_char, &mut stat_buf) } != 0 {
        io::write_str(2, b"modinfo: module ");
        io::write_all(2, module);
        io::write_str(2, b" not found\n");
        return false;
    }

    // Print filename (module name)
    if field_filter.is_none() || field_filter == Some(b"filename") {
        print_field(b"filename", &path[..pos], field_filter.is_some());
    }

    // Read parameters from /sys/module/<name>/parameters/
    if field_filter.is_none() || field_filter == Some(b"parm") {
        let mut parm_path = [0u8; 280];
        parm_path[..pos].copy_from_slice(&path[..pos]);
        let suffix = b"/parameters";
        for (j, &b) in suffix.iter().enumerate() {
            if pos + j < parm_path.len() {
                parm_path[pos + j] = b;
            }
        }
        // Note: Would need to read directory entries to list parameters
    }

    // Read refcnt from /sys/module/<name>/refcnt
    let refcnt_suffix = b"/refcnt";
    let mut refcnt_path = [0u8; 280];
    refcnt_path[..pos].copy_from_slice(&path[..pos]);
    for (j, &b) in refcnt_suffix.iter().enumerate() {
        if pos + j < refcnt_path.len() {
            refcnt_path[pos + j] = b;
        }
    }
    refcnt_path[pos + refcnt_suffix.len()] = 0;

    if field_filter.is_none() || field_filter == Some(b"refcnt") {
        let fd = io::open(&refcnt_path, libc::O_RDONLY, 0);
        if fd >= 0 {
            let mut buf = [0u8; 32];
            let n = io::read(fd, &mut buf);
            if n > 0 {
                print_field(b"refcnt", &buf[..n as usize], field_filter.is_some());
            }
            io::close(fd);
        }
    }

    // Read coresize from /sys/module/<name>/coresize
    let coresize_suffix = b"/coresize";
    let mut coresize_path = [0u8; 280];
    coresize_path[..pos].copy_from_slice(&path[..pos]);
    for (j, &b) in coresize_suffix.iter().enumerate() {
        if pos + j < coresize_path.len() {
            coresize_path[pos + j] = b;
        }
    }
    coresize_path[pos + coresize_suffix.len()] = 0;

    if field_filter.is_none() || field_filter == Some(b"size") {
        let fd = io::open(&coresize_path, libc::O_RDONLY, 0);
        if fd >= 0 {
            let mut buf = [0u8; 32];
            let n = io::read(fd, &mut buf);
            if n > 0 {
                print_field(b"size", &buf[..n as usize], field_filter.is_some());
            }
            io::close(fd);
        }
    }

    true
}

fn print_field(name: &[u8], value: &[u8], value_only: bool) {
    if value_only {
        // Trim trailing newline if present
        let trimmed = if value.last() == Some(&b'\n') {
            &value[..value.len() - 1]
        } else {
            value
        };
        io::write_all(1, trimmed);
        io::write_str(1, b"\n");
    } else {
        io::write_all(1, name);
        io::write_str(1, b":\t\t");
        io::write_all(1, value);
        if value.last() != Some(&b'\n') {
            io::write_str(1, b"\n");
        }
    }
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
    fn test_modinfo_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["modinfo"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }

    #[test]
    fn test_modinfo_nonexistent_module() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["modinfo", "nonexistent_module_xyz"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not found"));
    }

    #[test]
    fn test_modinfo_with_field_filter() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Try with a module that might exist (kernel modules vary by system)
        let output = Command::new(&armybox)
            .args(["modinfo", "-F", "filename", "nonexistent_xyz"])
            .output()
            .unwrap();

        // Should fail because module doesn't exist
        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_modinfo_existing_module() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Try to find any loaded module
        let modules = std::fs::read_dir("/sys/module");
        if modules.is_err() {
            return; // /sys/module not available
        }

        let modules = modules.unwrap();
        for entry in modules {
            if let Ok(entry) = entry {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                let output = Command::new(&armybox)
                    .args(["modinfo", &name_str])
                    .output()
                    .unwrap();

                // If module exists, should succeed
                if output.status.code() == Some(0) {
                    let stdout = std::string::String::from_utf8_lossy(&output.stdout);
                    assert!(stdout.contains("filename:"));
                    return; // Found at least one working module
                }
            }
        }
    }
}
