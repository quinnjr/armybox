//! Package Fetching
//!
//! Handles downloading packages and repository indexes.

extern crate alloc;

use crate::database::AvailablePackage;
use crate::io;
use alloc::string::String;
use alloc::vec::Vec;

/// Fetch repository index
pub fn fetch_index(base_url: &str) -> Result<Vec<AvailablePackage>, String> {
    // Build index URL
    let mut url = String::from(base_url);
    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str("APKINDEX");

    // For now, we only support file:// URLs for testing
    if let Some(path) = url.strip_prefix("file://") {
        return fetch_file_index(path);
    }

    // HTTP/HTTPS not yet implemented
    Err(String::from("HTTP fetching not yet implemented"))
}

/// Fetch index from local file
fn fetch_file_index(path: &str) -> Result<Vec<AvailablePackage>, String> {
    let fd = io::open(path.as_bytes(), libc::O_RDONLY, 0);
    if fd < 0 {
        return Err(String::from("cannot open index file"));
    }

    let data = io::read_all(fd);
    io::close(fd);

    parse_index(&data)
}

/// Parse repository index
///
/// Index format (similar to Alpine APKINDEX):
/// ```text
/// P:package-name
/// V:1.0.0
/// T:Package description
/// D:dep1 dep2 dep3
/// S:12345
/// C:sha256checksum
///
/// P:another-package
/// ...
/// ```
fn parse_index(data: &[u8]) -> Result<Vec<AvailablePackage>, String> {
    let text = match core::str::from_utf8(data) {
        Ok(t) => t,
        Err(_) => return Err(String::from("invalid UTF-8 in index")),
    };

    let mut packages = Vec::new();
    let mut current = AvailablePackage {
        name: String::new(),
        version: String::new(),
        description: String::new(),
        repo: String::new(),
        depends: Vec::new(),
        size: 0,
        checksum: [0; 32],
    };

    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() {
            // End of package entry
            if !current.name.is_empty() && !current.version.is_empty() {
                packages.push(current.clone());
            }
            current = AvailablePackage {
                name: String::new(),
                version: String::new(),
                description: String::new(),
                repo: String::new(),
                depends: Vec::new(),
                size: 0,
                checksum: [0; 32],
            };
            continue;
        }

        if line.len() < 2 || line.as_bytes()[1] != b':' {
            continue;
        }

        let key = line.as_bytes()[0];
        let value = &line[2..];

        match key {
            b'P' => current.name = String::from(value),
            b'V' => current.version = String::from(value),
            b'T' => current.description = String::from(value),
            b'D' => {
                current.depends = value.split_whitespace().map(String::from).collect();
            }
            b'S' => current.size = value.parse().unwrap_or(0),
            b'C' => {
                if let Some(checksum) = parse_hex_checksum(value) {
                    current.checksum = checksum;
                }
            }
            b'R' => current.repo = String::from(value),
            _ => {}
        }
    }

    // Don't forget the last entry
    if !current.name.is_empty() && !current.version.is_empty() {
        packages.push(current);
    }

    Ok(packages)
}

fn parse_hex_checksum(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }

    let mut result = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hi = hex_digit(chunk[0])?;
        let lo = hex_digit(chunk[1])?;
        result[i] = (hi << 4) | lo;
    }
    Some(result)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Download a package
pub fn download_package(url: &str, dest: &[u8]) -> Result<(), String> {
    // For now, only support file:// URLs
    if let Some(path) = url.strip_prefix("file://") {
        return copy_file(path.as_bytes(), dest);
    }

    Err(String::from("HTTP downloading not yet implemented"))
}

fn copy_file(src: &[u8], dest: &[u8]) -> Result<(), String> {
    let src_fd = io::open(src, libc::O_RDONLY, 0);
    if src_fd < 0 {
        return Err(String::from("cannot open source file"));
    }

    let dest_fd = io::open(dest, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if dest_fd < 0 {
        io::close(src_fd);
        return Err(String::from("cannot create destination file"));
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(src_fd, &mut buf);
        if n <= 0 {
            break;
        }
        io::write_all(dest_fd, &buf[..n as usize]);
    }

    io::close(src_fd);
    io::close(dest_fd);
    Ok(())
}

/// Get cache path for a package
pub fn cache_path(name: &str, version: &str) -> Vec<u8> {
    let mut path = Vec::from(super::database::CACHE_DIR);
    path.push(b'/');
    path.extend_from_slice(name.as_bytes());
    path.push(b'-');
    path.extend_from_slice(version.as_bytes());
    path.extend_from_slice(b".abp");
    path
}
