//! Repository Management
//!
//! Implements repository commands for ABP.

extern crate alloc;

use crate::database::{Database, RepoRecord, current_time};
use crate::fetch::fetch_index;
use crate::io;
use alloc::string::String;

/// Add a repository
pub fn add_repo(url: &[u8], name: &[u8]) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let url_str = match core::str::from_utf8(url) {
        Ok(s) => String::from(s),
        Err(_) => {
            io::write_str(2, b"abp: invalid URL\n");
            return 1;
        }
    };

    let name_str = match core::str::from_utf8(name) {
        Ok(s) => String::from(s),
        Err(_) => {
            io::write_str(2, b"abp: invalid repository name\n");
            return 1;
        }
    };

    // Check if repo already exists
    if db.get_repo(&name_str).is_some() {
        io::write_str(2, b"abp: repository '");
        io::write_all(2, name);
        io::write_str(2, b"' already exists\n");
        return 1;
    }

    let record = RepoRecord {
        name: name_str.clone(),
        url: url_str,
        enabled: true,
        last_update: 0,
    };

    if !db.put_repo(&record) {
        io::write_str(2, b"abp: failed to add repository\n");
        return 1;
    }

    io::write_str(1, b"Repository '");
    io::write_all(1, name);
    io::write_str(1, b"' added.\n");
    io::write_str(1, b"Run 'abp update' to fetch package index.\n");

    0
}

/// Remove a repository
pub fn remove_repo(name: &[u8]) -> i32 {
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
            io::write_str(2, b"abp: invalid repository name\n");
            return 1;
        }
    };

    if db.get_repo(name_str).is_none() {
        io::write_str(2, b"abp: repository '");
        io::write_all(2, name);
        io::write_str(2, b"' not found\n");
        return 1;
    }

    if !db.delete_repo(name_str) {
        io::write_str(2, b"abp: failed to remove repository\n");
        return 1;
    }

    io::write_str(1, b"Repository '");
    io::write_all(1, name);
    io::write_str(1, b"' removed.\n");

    0
}

/// List repositories
pub fn list_repos() -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let repos = db.list_repos();

    if repos.is_empty() {
        io::write_str(1, b"No repositories configured.\n");
        io::write_str(1, b"Use 'abp repo add <url> <name>' to add one.\n");
        return 0;
    }

    io::write_str(1, b"Configured repositories:\n\n");

    for repo in &repos {
        let status = if repo.enabled {
            "[enabled]"
        } else {
            "[disabled]"
        };
        io::write_all(1, status.as_bytes());
        io::write_str(1, b" ");
        io::write_all(1, repo.name.as_bytes());
        io::write_str(1, b"\n");
        io::write_str(1, b"    URL: ");
        io::write_all(1, repo.url.as_bytes());
        io::write_str(1, b"\n");
        if repo.last_update > 0 {
            io::write_str(1, b"    Last update: ");
            format_time(repo.last_update);
            io::write_str(1, b"\n");
        }
        io::write_str(1, b"\n");
    }

    0
}

/// Update repository indexes
pub fn update_repos() -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let repos = db.list_repos();

    if repos.is_empty() {
        io::write_str(1, b"No repositories configured.\n");
        io::write_str(1, b"Use 'abp repo add <url> <name>' to add one.\n");
        return 0;
    }

    // Clear old available packages
    db.clear_available();

    let mut errors = 0;

    for repo in &repos {
        if !repo.enabled {
            continue;
        }

        io::write_str(1, b"Updating ");
        io::write_all(1, repo.name.as_bytes());
        io::write_str(1, b"... ");

        match fetch_index(&repo.url) {
            Ok(packages) => {
                for pkg in packages {
                    db.put_available(&pkg);
                }
                io::write_str(1, b"OK\n");

                // Update last_update time
                let updated = RepoRecord {
                    name: repo.name.clone(),
                    url: repo.url.clone(),
                    enabled: repo.enabled,
                    last_update: current_time(),
                };
                db.put_repo(&updated);
            }
            Err(e) => {
                io::write_str(1, b"FAILED\n");
                io::write_str(2, b"  Error: ");
                io::write_all(2, e.as_bytes());
                io::write_str(2, b"\n");
                errors += 1;
            }
        }
    }

    if errors > 0 {
        io::write_str(2, b"\nSome repositories failed to update.\n");
        return 1;
    }

    io::write_str(1, b"\nPackage index updated.\n");
    0
}

fn format_time(timestamp: u64) {
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
