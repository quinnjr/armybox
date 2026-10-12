//! Package Listing and Information
//!
//! Implements the `abp list`, `abp info`, and `abp search` commands.

extern crate alloc;

use crate::database::Database;
use crate::io;

/// List installed packages
pub fn list_installed(verbose: bool) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let packages = db.list_packages();

    if packages.is_empty() {
        io::write_str(1, b"No packages installed.\n");
        return 0;
    }

    let mut sorted = packages;
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for pkg in &sorted {
        io::write_all(1, pkg.name.as_bytes());

        if verbose {
            io::write_str(1, b"-");
            io::write_all(1, pkg.version.as_bytes());

            if !pkg.description.is_empty() {
                io::write_str(1, b" - ");
                io::write_all(1, pkg.description.as_bytes());
            }
        }

        io::write_str(1, b"\n");
    }

    io::write_str(1, b"\n");
    io::write_num(1, sorted.len() as u64);
    io::write_str(1, b" packages installed.\n");

    0
}

/// List available packages
pub fn list_available(verbose: bool) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let packages = db.list_available();

    if packages.is_empty() {
        io::write_str(1, b"No packages available.\n");
        io::write_str(1, b"Run 'abp update' to refresh repository index.\n");
        return 0;
    }

    let mut sorted = packages;
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for pkg in &sorted {
        // Check if installed
        let installed = db.get_package(&pkg.name);
        let marker = if installed.is_some() { "[i] " } else { "    " };

        io::write_all(1, marker.as_bytes());
        io::write_all(1, pkg.name.as_bytes());

        if verbose {
            io::write_str(1, b"-");
            io::write_all(1, pkg.version.as_bytes());

            // Show upgrade indicator
            if let Some(inst) = &installed {
                use crate::format::Version;
                let inst_ver = Version::parse(&inst.version);
                let avail_ver = Version::parse(&pkg.version);
                if avail_ver.compare(&inst_ver) == core::cmp::Ordering::Greater {
                    io::write_str(1, b" [upgradeable from ");
                    io::write_all(1, inst.version.as_bytes());
                    io::write_str(1, b"]");
                }
            }

            if !pkg.description.is_empty() {
                io::write_str(1, b" - ");
                io::write_all(1, pkg.description.as_bytes());
            }
        }

        io::write_str(1, b"\n");
    }

    io::write_str(1, b"\n");
    io::write_num(1, sorted.len() as u64);
    io::write_str(1, b" packages available.\n");

    0
}

/// Show package information
pub fn show_info(name: &[u8]) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let name_str = match core::str::from_utf8(name) {
        Ok(s) => s,
        Err(_) => {
            io::write_str(2, b"abp: invalid package name\n");
            return 1;
        }
    };

    // Try installed first
    if let Some(pkg) = db.get_package(name_str) {
        io::write_str(1, b"Package: ");
        io::write_all(1, pkg.name.as_bytes());
        io::write_str(1, b"\n");

        io::write_str(1, b"Version: ");
        io::write_all(1, pkg.version.as_bytes());
        io::write_str(1, b"\n");

        io::write_str(1, b"Status: installed\n");

        if !pkg.description.is_empty() {
            io::write_str(1, b"Description: ");
            io::write_all(1, pkg.description.as_bytes());
            io::write_str(1, b"\n");
        }

        io::write_str(1, b"Install Size: ");
        format_size(pkg.install_size);
        io::write_str(1, b"\n");

        io::write_str(1, b"Install Time: ");
        format_time(pkg.install_time);
        io::write_str(1, b"\n");

        if !pkg.depends.is_empty() {
            io::write_str(1, b"Depends: ");
            for (i, dep) in pkg.depends.iter().enumerate() {
                if i > 0 {
                    io::write_str(1, b", ");
                }
                io::write_all(1, dep.as_bytes());
            }
            io::write_str(1, b"\n");
        }

        if !pkg.provides.is_empty() {
            io::write_str(1, b"Provides: ");
            for (i, prov) in pkg.provides.iter().enumerate() {
                if i > 0 {
                    io::write_str(1, b", ");
                }
                io::write_all(1, prov.as_bytes());
            }
            io::write_str(1, b"\n");
        }

        io::write_str(1, b"Files: ");
        io::write_num(1, pkg.files.len() as u64);
        io::write_str(1, b"\n");

        return 0;
    }

    // Try available
    if let Some(pkg) = db.get_available(name_str) {
        io::write_str(1, b"Package: ");
        io::write_all(1, pkg.name.as_bytes());
        io::write_str(1, b"\n");

        io::write_str(1, b"Version: ");
        io::write_all(1, pkg.version.as_bytes());
        io::write_str(1, b"\n");

        io::write_str(1, b"Status: available\n");

        io::write_str(1, b"Repository: ");
        io::write_all(1, pkg.repo.as_bytes());
        io::write_str(1, b"\n");

        if !pkg.description.is_empty() {
            io::write_str(1, b"Description: ");
            io::write_all(1, pkg.description.as_bytes());
            io::write_str(1, b"\n");
        }

        io::write_str(1, b"Download Size: ");
        format_size(pkg.size);
        io::write_str(1, b"\n");

        if !pkg.depends.is_empty() {
            io::write_str(1, b"Depends: ");
            for (i, dep) in pkg.depends.iter().enumerate() {
                if i > 0 {
                    io::write_str(1, b", ");
                }
                io::write_all(1, dep.as_bytes());
            }
            io::write_str(1, b"\n");
        }

        return 0;
    }

    io::write_str(2, b"abp: package '");
    io::write_all(2, name);
    io::write_str(2, b"' not found\n");
    1
}

/// Search packages
pub fn search_packages(query: &[u8]) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let query_str = match core::str::from_utf8(query) {
        Ok(s) => s.to_lowercase(),
        Err(_) => {
            io::write_str(2, b"abp: invalid query\n");
            return 1;
        }
    };

    let mut found = 0;

    // Search installed packages
    for pkg in db.list_packages() {
        if matches_query(&pkg.name, &pkg.description, &query_str) {
            io::write_str(1, b"[installed] ");
            io::write_all(1, pkg.name.as_bytes());
            io::write_str(1, b"-");
            io::write_all(1, pkg.version.as_bytes());
            if !pkg.description.is_empty() {
                io::write_str(1, b" - ");
                io::write_all(1, pkg.description.as_bytes());
            }
            io::write_str(1, b"\n");
            found += 1;
        }
    }

    // Search available packages
    for pkg in db.list_available() {
        // Skip if already shown as installed
        if db.is_installed(&pkg.name) {
            continue;
        }

        if matches_query(&pkg.name, &pkg.description, &query_str) {
            io::write_str(1, b"[available] ");
            io::write_all(1, pkg.name.as_bytes());
            io::write_str(1, b"-");
            io::write_all(1, pkg.version.as_bytes());
            if !pkg.description.is_empty() {
                io::write_str(1, b" - ");
                io::write_all(1, pkg.description.as_bytes());
            }
            io::write_str(1, b"\n");
            found += 1;
        }
    }

    if found == 0 {
        io::write_str(1, b"No packages found matching '");
        io::write_all(1, query);
        io::write_str(1, b"'\n");
    } else {
        io::write_str(1, b"\n");
        io::write_num(1, found);
        io::write_str(1, b" packages found.\n");
    }

    0
}

fn matches_query(name: &str, description: &str, query: &str) -> bool {
    name.to_lowercase().contains(query) || description.to_lowercase().contains(query)
}

fn format_size(size: u64) {
    if size >= 1024 * 1024 * 1024 {
        io::write_num(1, size / (1024 * 1024 * 1024));
        io::write_str(1, b" GiB");
    } else if size >= 1024 * 1024 {
        io::write_num(1, size / (1024 * 1024));
        io::write_str(1, b" MiB");
    } else if size >= 1024 {
        io::write_num(1, size / 1024);
        io::write_str(1, b" KiB");
    } else {
        io::write_num(1, size);
        io::write_str(1, b" B");
    }
}

fn format_time(timestamp: u64) {
    // Simple date formatting
    // Days since Unix epoch
    let days = timestamp / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let day_of_year = days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    io::write_num(1, year);
    io::write_str(1, b"-");
    if month < 10 {
        io::write_str(1, b"0");
    }
    io::write_num(1, month);
    io::write_str(1, b"-");
    if day < 10 {
        io::write_str(1, b"0");
    }
    io::write_num(1, day);
}
