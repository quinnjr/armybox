//! Package Removal
//!
//! Implements the `abp del` command for removing packages.

extern crate alloc;

use crate::database::Database;
use crate::io;
use alloc::string::String;
use alloc::vec::Vec;

/// Remove packages
pub fn remove_packages(packages: &[&[u8]], purge: bool, force: bool) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let mut to_remove: Vec<String> = Vec::new();

    // Validate packages
    for &pkg_name in packages {
        let name = match core::str::from_utf8(pkg_name) {
            Ok(s) => s,
            Err(_) => {
                io::write_str(2, b"abp: invalid package name\n");
                return 1;
            }
        };

        if !db.is_installed(name) {
            io::write_str(2, b"abp: package '");
            io::write_all(2, pkg_name);
            io::write_str(2, b"' is not installed\n");
            return 1;
        }

        to_remove.push(String::from(name));
    }

    if to_remove.is_empty() {
        io::write_str(2, b"abp: no packages to remove\n");
        return 1;
    }

    // Check dependencies (unless force)
    if !force {
        for name in &to_remove {
            let dependents = db.get_dependents(name);
            let blocking: Vec<_> = dependents
                .iter()
                .filter(|d| !to_remove.contains(d))
                .collect();

            if !blocking.is_empty() {
                io::write_str(2, b"abp: cannot remove '");
                io::write_all(2, name.as_bytes());
                io::write_str(2, b"': required by:\n");
                for dep in blocking {
                    io::write_str(2, b"  ");
                    io::write_all(2, dep.as_bytes());
                    io::write_str(2, b"\n");
                }
                io::write_str(2, b"Use --force to remove anyway.\n");
                return 1;
            }
        }
    }

    // Show summary
    io::write_str(1, b"Packages to remove:\n");
    for name in &to_remove {
        if let Some(pkg) = db.get_package(name) {
            io::write_str(1, b"  ");
            io::write_all(1, name.as_bytes());
            io::write_str(1, b"-");
            io::write_all(1, pkg.version.as_bytes());
            io::write_str(1, b"\n");
        }
    }
    io::write_str(1, b"\n");

    // Remove packages
    let mut success = true;
    for name in &to_remove {
        if !remove_single_package(&db, name, purge) {
            success = false;
            break;
        }
    }

    if success {
        io::write_str(1, b"Removal complete.\n");
        0
    } else {
        io::write_str(2, b"Removal failed.\n");
        1
    }
}

/// Remove a single package
pub(crate) fn remove_single_package(db: &Database, name: &str, purge: bool) -> bool {
    let pkg = match db.get_package(name) {
        Some(p) => p,
        None => {
            io::write_str(2, b"abp: package not found in database\n");
            return false;
        }
    };

    io::write_str(1, b"Removing ");
    io::write_all(1, name.as_bytes());
    io::write_str(1, b"-");
    io::write_all(1, pkg.version.as_bytes());
    io::write_str(1, b"...\n");

    // Remove files (in reverse order to handle directories properly)
    let mut files = pkg.files.clone();
    files.sort();
    files.reverse();

    for file in &files {
        let path = file.as_bytes();

        // Skip config files unless purging
        if !purge && is_config_file(path) {
            io::write_str(1, b"  Keeping config: ");
            io::write_all(1, path);
            io::write_str(1, b"\n");
            continue;
        }

        // Check if file exists
        if io::access(path, libc::F_OK) != 0 {
            continue;
        }

        // Check if directory
        let mut stat_buf = io::stat_zeroed();
        if io::stat(path, &mut stat_buf) == 0 {
            if io::is_dir(stat_buf.st_mode as u32) {
                // Try to remove directory (will fail if not empty)
                io::rmdir(path);
            } else {
                // Remove file
                io::unlink(path);
            }
        }
    }

    // Unregister files from database
    db.unregister_files(name);

    // Delete package record
    if !db.delete_package(name) {
        io::write_str(2, b"abp: failed to remove package from database\n");
        return false;
    }

    io::write_str(1, b"  Done.\n");
    true
}

/// Check if path is a config file
fn is_config_file(path: &[u8]) -> bool {
    // Config files are typically in /etc
    if path.starts_with(b"/etc/") {
        return true;
    }

    // Also check for common config extensions
    if path.ends_with(b".conf") || path.ends_with(b".cfg") || path.ends_with(b".ini") {
        return true;
    }

    false
}
