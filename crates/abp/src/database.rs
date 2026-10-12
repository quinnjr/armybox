//! ABP Package Database
//!
//! Manages the package database using redb (when available) or a simple
//! file-based fallback.
//!
//! # Database Schema
//! ```text
//! PACKAGES:    "name-version" -> PackageRecord
//! FILES:       "/path/to/file" -> "package-name"
//! REQUIRED_BY: "package" -> Vec<dependents>
//! REPOS:       "repo-name" -> RepoRecord
//! AVAILABLE:   "package" -> AvailablePackage
//! ```

extern crate alloc;

use crate::format::{Manifest, PackageMetadata};
use crate::io;
use alloc::string::String;
use alloc::vec::Vec;

/// Database paths
pub const DB_PATH: &[u8] = b"/var/lib/abp/abp.db";
pub const DB_DIR: &[u8] = b"/var/lib/abp";
pub const CACHE_DIR: &[u8] = b"/var/cache/abp";

/// Package record stored in database
#[derive(Clone, Debug)]
pub struct PackageRecord {
    pub name: String,
    pub version: String,
    pub description: String,
    pub install_time: u64,
    pub install_size: u64,
    pub depends: Vec<String>,
    pub provides: Vec<String>,
    pub files: Vec<String>,
    /// Per-file SHA-256 checksums, parallel to `files` (same order). Populated
    /// from the package manifest at install time so `abp verify` can detect
    /// content tampering, not just missing files. May be empty for records
    /// written by an older abp that did not persist checksums (the trailing
    /// checksum block is optional and absent records deserialize with an empty
    /// vec, keeping the on-disk format backward-compatible in both directions).
    pub checksums: Vec<[u8; 32]>,
}

impl PackageRecord {
    /// Create from metadata and manifest
    pub fn from_metadata(meta: &PackageMetadata, manifest: &Manifest, install_time: u64) -> Self {
        PackageRecord {
            name: meta.name.clone(),
            version: meta.version.clone(),
            description: meta.description.clone(),
            install_time,
            install_size: meta.install_size,
            depends: meta.depends.clone(),
            provides: meta.provides.clone(),
            files: manifest.entries.iter().map(|e| e.path.clone()).collect(),
            checksums: manifest.entries.iter().map(|e| e.checksum).collect(),
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Format: name\0version\0description\0time\0size\0depends\0provides\0files
        buf.extend_from_slice(self.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.version.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.description.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&self.install_time.to_le_bytes());
        buf.extend_from_slice(&self.install_size.to_le_bytes());

        // Number of depends
        buf.extend_from_slice(&(self.depends.len() as u32).to_le_bytes());
        for dep in &self.depends {
            buf.extend_from_slice(&(dep.len() as u16).to_le_bytes());
            buf.extend_from_slice(dep.as_bytes());
        }

        // Number of provides
        buf.extend_from_slice(&(self.provides.len() as u32).to_le_bytes());
        for prov in &self.provides {
            buf.extend_from_slice(&(prov.len() as u16).to_le_bytes());
            buf.extend_from_slice(prov.as_bytes());
        }

        // Number of files
        buf.extend_from_slice(&(self.files.len() as u32).to_le_bytes());
        for file in &self.files {
            buf.extend_from_slice(&(file.len() as u16).to_le_bytes());
            buf.extend_from_slice(file.as_bytes());
        }

        // Per-file checksums (trailing, optional block). Older readers stop
        // after the file list and simply ignore these bytes; newer readers
        // that reach end-of-data before this block treat it as absent.
        buf.extend_from_slice(&(self.checksums.len() as u32).to_le_bytes());
        for c in &self.checksums {
            buf.extend_from_slice(c);
        }

        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut pos = 0;

        let name = read_cstring(data, &mut pos)?;
        let version = read_cstring(data, &mut pos)?;
        let description = read_cstring(data, &mut pos)?;

        if pos + 16 > data.len() {
            return None;
        }

        let install_time = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);
        pos += 8;

        let install_size = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);
        pos += 8;

        let depends = read_string_list(data, &mut pos)?;
        let provides = read_string_list(data, &mut pos)?;
        let files = read_string_list(data, &mut pos)?;

        // Trailing checksum block is optional: records written by an older abp
        // end right after the file list. `read_checksum_list` returns an empty
        // vec when there is no (or an incomplete) block, so both formats parse.
        let checksums = read_checksum_list(data, &mut pos);

        Some(PackageRecord {
            name,
            version,
            description,
            install_time,
            install_size,
            depends,
            provides,
            files,
            checksums,
        })
    }
}

/// Read an optional trailing per-file checksum block: a u32 count followed by
/// that many 32-byte hashes. Returns an empty vec if the block is absent or
/// truncated, so legacy records (no block) and forward reads both stay safe.
fn read_checksum_list(data: &[u8], pos: &mut usize) -> Vec<[u8; 32]> {
    if *pos + 4 > data.len() {
        return Vec::new();
    }
    let count =
        u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]) as usize;
    *pos += 4;

    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        if *pos + 32 > data.len() {
            break;
        }
        let mut c = [0u8; 32];
        c.copy_from_slice(&data[*pos..*pos + 32]);
        *pos += 32;
        list.push(c);
    }
    list
}

fn read_cstring(data: &[u8], pos: &mut usize) -> Option<String> {
    let start = *pos;
    while *pos < data.len() && data[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= data.len() {
        return None;
    }
    let s = core::str::from_utf8(&data[start..*pos]).ok()?;
    *pos += 1; // Skip null terminator
    Some(String::from(s))
}

fn read_string_list(data: &[u8], pos: &mut usize) -> Option<Vec<String>> {
    if *pos + 4 > data.len() {
        return None;
    }
    let count =
        u32::from_le_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]) as usize;
    *pos += 4;

    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        if *pos + 2 > data.len() {
            return None;
        }
        let len = u16::from_le_bytes([data[*pos], data[*pos + 1]]) as usize;
        *pos += 2;

        if *pos + len > data.len() {
            return None;
        }
        let s = core::str::from_utf8(&data[*pos..*pos + len]).ok()?;
        list.push(String::from(s));
        *pos += len;
    }
    Some(list)
}

/// Repository record
#[derive(Clone, Debug)]
pub struct RepoRecord {
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub last_update: u64,
}

impl RepoRecord {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(self.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.url.as_bytes());
        buf.push(0);
        buf.push(if self.enabled { 1 } else { 0 });
        buf.extend_from_slice(&self.last_update.to_le_bytes());
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut pos = 0;
        let name = read_cstring(data, &mut pos)?;
        let url = read_cstring(data, &mut pos)?;

        if pos + 9 > data.len() {
            return None;
        }

        let enabled = data[pos] != 0;
        pos += 1;

        let last_update = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);

        Some(RepoRecord {
            name,
            url,
            enabled,
            last_update,
        })
    }
}

/// Available package (from repository)
#[derive(Clone, Debug)]
pub struct AvailablePackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub depends: Vec<String>,
    pub size: u64,
    pub checksum: [u8; 32],
}

impl AvailablePackage {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(self.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.version.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.description.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.repo.as_bytes());
        buf.push(0);

        buf.extend_from_slice(&(self.depends.len() as u32).to_le_bytes());
        for dep in &self.depends {
            buf.extend_from_slice(&(dep.len() as u16).to_le_bytes());
            buf.extend_from_slice(dep.as_bytes());
        }

        buf.extend_from_slice(&self.size.to_le_bytes());
        buf.extend_from_slice(&self.checksum);
        buf
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let mut pos = 0;
        let name = read_cstring(data, &mut pos)?;
        let version = read_cstring(data, &mut pos)?;
        let description = read_cstring(data, &mut pos)?;
        let repo = read_cstring(data, &mut pos)?;

        let depends = read_string_list(data, &mut pos)?;

        if pos + 8 + 32 > data.len() {
            return None;
        }

        let size = u64::from_le_bytes([
            data[pos],
            data[pos + 1],
            data[pos + 2],
            data[pos + 3],
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]);
        pos += 8;

        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&data[pos..pos + 32]);

        Some(AvailablePackage {
            name,
            version,
            description,
            repo,
            depends,
            size,
            checksum,
        })
    }
}

/// Simple file-based database
///
/// Uses a directory structure:
/// ```text
/// /var/lib/abp/
///   packages/       # Package records (one file per package)
///   files.idx       # File -> package mapping
///   repos/          # Repository records
///   available/      # Available packages index
/// ```
pub struct Database {
    path: Vec<u8>,
}

impl Database {
    /// Open or create database
    pub fn open() -> Option<Self> {
        // Create directories if needed
        ensure_dir(DB_DIR);
        ensure_dir(CACHE_DIR);

        let mut packages_dir = DB_DIR.to_vec();
        packages_dir.extend_from_slice(b"/packages");
        ensure_dir(&packages_dir);

        let mut repos_dir = DB_DIR.to_vec();
        repos_dir.extend_from_slice(b"/repos");
        ensure_dir(&repos_dir);

        let mut available_dir = DB_DIR.to_vec();
        available_dir.extend_from_slice(b"/available");
        ensure_dir(&available_dir);

        Some(Database {
            path: DB_DIR.to_vec(),
        })
    }

    /// Get a package record by name
    pub fn get_package(&self, name: &str) -> Option<PackageRecord> {
        if !valid_pkg_name(name.as_bytes()) {
            return None;
        }
        let mut path = self.path.clone();
        path.extend_from_slice(b"/packages/");
        path.extend_from_slice(name.as_bytes());

        let fd = io::open(&path, libc::O_RDONLY, 0);
        if fd < 0 {
            return None;
        }

        let data = io::read_all(fd);
        io::close(fd);

        PackageRecord::from_bytes(&data)
    }

    /// Store a package record
    pub fn put_package(&self, record: &PackageRecord) -> bool {
        if !valid_pkg_name(record.name.as_bytes()) {
            return false;
        }
        let mut path = self.path.clone();
        path.extend_from_slice(b"/packages/");
        path.extend_from_slice(record.name.as_bytes());

        let data = record.to_bytes();

        atomic_write(&path, &data)
    }

    /// Delete a package record
    pub fn delete_package(&self, name: &str) -> bool {
        if !valid_pkg_name(name.as_bytes()) {
            return false;
        }
        let mut path = self.path.clone();
        path.extend_from_slice(b"/packages/");
        path.extend_from_slice(name.as_bytes());

        io::unlink(&path) == 0
    }

    /// List all installed packages
    pub fn list_packages(&self) -> Vec<PackageRecord> {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/packages");

        let mut packages = Vec::new();
        let dir = io::opendir(&path);
        if dir.is_null() {
            return packages;
        }

        loop {
            let entry = io::readdir(dir);
            if entry.is_null() {
                break;
            }

            let (name, _) = unsafe { io::dirent_name(entry) };
            if name == b"." || name == b".." {
                continue;
            }

            let name_str = match core::str::from_utf8(name) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(pkg) = self.get_package(name_str) {
                packages.push(pkg);
            }
        }

        io::closedir(dir);
        packages
    }

    /// Register file ownership
    pub fn register_file(&self, path: &str, package: &str) -> bool {
        // Append to files index
        let mut idx_path = self.path.clone();
        idx_path.extend_from_slice(b"/files.idx");

        let fd = io::open(
            &idx_path,
            libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
            0o644,
        );
        if fd < 0 {
            return false;
        }

        let mut entry = Vec::new();
        entry.extend_from_slice(path.as_bytes());
        entry.push(b'\t');
        entry.extend_from_slice(package.as_bytes());
        entry.push(b'\n');

        let written = io::write_all(fd, &entry);
        io::close(fd);

        written == entry.len() as isize
    }

    /// Get package owning a file
    pub fn get_file_owner(&self, path: &str) -> Option<String> {
        let mut idx_path = self.path.clone();
        idx_path.extend_from_slice(b"/files.idx");

        let fd = io::open(&idx_path, libc::O_RDONLY, 0);
        if fd < 0 {
            return None;
        }

        let data = io::read_all(fd);
        io::close(fd);

        let text = core::str::from_utf8(&data).ok()?;
        for line in text.lines() {
            if let Some((file_path, pkg_name)) = line.split_once('\t')
                && file_path == path
            {
                return Some(String::from(pkg_name));
            }
        }

        None
    }

    /// Unregister files for a package
    pub fn unregister_files(&self, package: &str) -> bool {
        let mut idx_path = self.path.clone();
        idx_path.extend_from_slice(b"/files.idx");

        let fd = io::open(&idx_path, libc::O_RDONLY, 0);
        if fd < 0 {
            return true; // No index file is OK
        }

        let data = io::read_all(fd);
        io::close(fd);

        let text = match core::str::from_utf8(&data) {
            Ok(t) => t,
            Err(_) => return false,
        };

        // Rebuild without the package's entries
        let mut new_data = Vec::new();
        for line in text.lines() {
            if let Some((_, pkg_name)) = line.split_once('\t')
                && pkg_name != package
            {
                new_data.extend_from_slice(line.as_bytes());
                new_data.push(b'\n');
            }
        }

        // Atomically replace the whole index: this rewrites ownership data for
        // ALL packages, so a crash mid-write must not corrupt it.
        atomic_write(&idx_path, &new_data)
    }

    /// Unregister a single file from the index
    pub fn unregister_file(&self, file_path: &str) -> bool {
        let mut idx_path = self.path.clone();
        idx_path.extend_from_slice(b"/files.idx");

        let fd = io::open(&idx_path, libc::O_RDONLY, 0);
        if fd < 0 {
            return true; // No index file is OK
        }

        let data = io::read_all(fd);
        io::close(fd);

        let text = match core::str::from_utf8(&data) {
            Ok(t) => t,
            Err(_) => return false,
        };

        // Rebuild without the file's entry
        let mut new_data = Vec::new();
        for line in text.lines() {
            if let Some((path, _)) = line.split_once('\t')
                && path != file_path
            {
                new_data.extend_from_slice(line.as_bytes());
                new_data.push(b'\n');
            }
        }

        // Atomically replace the index to avoid corrupting it on a crash.
        atomic_write(&idx_path, &new_data)
    }

    /// Get repository record
    pub fn get_repo(&self, name: &str) -> Option<RepoRecord> {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/repos/");
        path.extend_from_slice(name.as_bytes());

        let fd = io::open(&path, libc::O_RDONLY, 0);
        if fd < 0 {
            return None;
        }

        let data = io::read_all(fd);
        io::close(fd);

        RepoRecord::from_bytes(&data)
    }

    /// Store repository record
    pub fn put_repo(&self, record: &RepoRecord) -> bool {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/repos/");
        path.extend_from_slice(record.name.as_bytes());

        let data = record.to_bytes();

        let fd = io::open(&path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
        if fd < 0 {
            return false;
        }

        let written = io::write_all(fd, &data);
        io::close(fd);

        written == data.len() as isize
    }

    /// Delete repository
    pub fn delete_repo(&self, name: &str) -> bool {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/repos/");
        path.extend_from_slice(name.as_bytes());

        io::unlink(&path) == 0
    }

    /// List repositories
    pub fn list_repos(&self) -> Vec<RepoRecord> {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/repos");

        let mut repos = Vec::new();
        let dir = io::opendir(&path);
        if dir.is_null() {
            return repos;
        }

        loop {
            let entry = io::readdir(dir);
            if entry.is_null() {
                break;
            }

            let (name, _) = unsafe { io::dirent_name(entry) };
            if name == b"." || name == b".." {
                continue;
            }

            let name_str = match core::str::from_utf8(name) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(repo) = self.get_repo(name_str) {
                repos.push(repo);
            }
        }

        io::closedir(dir);
        repos
    }

    /// Store available package
    pub fn put_available(&self, pkg: &AvailablePackage) -> bool {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/available/");
        path.extend_from_slice(pkg.name.as_bytes());

        let data = pkg.to_bytes();

        let fd = io::open(&path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
        if fd < 0 {
            return false;
        }

        let written = io::write_all(fd, &data);
        io::close(fd);

        written == data.len() as isize
    }

    /// Get available package
    pub fn get_available(&self, name: &str) -> Option<AvailablePackage> {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/available/");
        path.extend_from_slice(name.as_bytes());

        let fd = io::open(&path, libc::O_RDONLY, 0);
        if fd < 0 {
            return None;
        }

        let data = io::read_all(fd);
        io::close(fd);

        AvailablePackage::from_bytes(&data)
    }

    /// List available packages
    pub fn list_available(&self) -> Vec<AvailablePackage> {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/available");

        let mut packages = Vec::new();
        let dir = io::opendir(&path);
        if dir.is_null() {
            return packages;
        }

        loop {
            let entry = io::readdir(dir);
            if entry.is_null() {
                break;
            }

            let (name, _) = unsafe { io::dirent_name(entry) };
            if name == b"." || name == b".." {
                continue;
            }

            let name_str = match core::str::from_utf8(name) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Some(pkg) = self.get_available(name_str) {
                packages.push(pkg);
            }
        }

        io::closedir(dir);
        packages
    }

    /// Clear available packages
    pub fn clear_available(&self) -> bool {
        let mut path = self.path.clone();
        path.extend_from_slice(b"/available");

        let dir = io::opendir(&path);
        if dir.is_null() {
            return true;
        }

        loop {
            let entry = io::readdir(dir);
            if entry.is_null() {
                break;
            }

            let (name, _) = unsafe { io::dirent_name(entry) };
            if name == b"." || name == b".." {
                continue;
            }

            let mut file_path = path.clone();
            file_path.push(b'/');
            file_path.extend_from_slice(name);
            io::unlink(&file_path);
        }

        io::closedir(dir);
        true
    }

    /// Get packages that depend on the given package
    pub fn get_dependents(&self, package: &str) -> Vec<String> {
        let mut dependents = Vec::new();

        for pkg in self.list_packages() {
            for dep in &pkg.depends {
                // Parse dependency and check if it matches
                let dep_name = dep.split(['>', '<', '=']).next().unwrap_or(dep);
                if dep_name == package {
                    dependents.push(pkg.name.clone());
                    break;
                }
            }
        }

        dependents
    }

    /// Check if a package is installed
    pub fn is_installed(&self, name: &str) -> bool {
        self.get_package(name).is_some()
    }
}

/// Ensure directory exists
fn ensure_dir(path: &[u8]) {
    io::mkdir(path, 0o755);
}

/// Validate a package/record name used to build an on-disk path.
///
/// Prevents path traversal: `name` is concatenated onto a database
/// subdirectory, so an unvalidated `../../etc/...` would escape the
/// database. A valid name is non-empty, contains only
/// `[A-Za-z0-9._+-]` (no `/`), does not start with `.`, and contains
/// no `..` sequence.
fn valid_pkg_name(name: &[u8]) -> bool {
    if name.is_empty() {
        return false;
    }
    if name[0] == b'.' {
        return false;
    }
    for (i, &c) in name.iter().enumerate() {
        let ok = c.is_ascii_alphanumeric() || c == b'.' || c == b'_' || c == b'+' || c == b'-';
        if !ok {
            return false;
        }
        // Reject any ".." sequence.
        if c == b'.' && i + 1 < name.len() && name[i + 1] == b'.' {
            return false;
        }
    }
    true
}

/// Atomically write `data` to `path`.
///
/// Writes to a temporary file (`<path>.tmp.<pid>`) then `rename()`s it
/// over the target, which is atomic on POSIX. On any failure the temp
/// file is unlinked and `false` is returned, leaving the original
/// target untouched. This prevents a crash mid-write from corrupting
/// the record.
fn atomic_write(path: &[u8], data: &[u8]) -> bool {
    let pid = unsafe { libc::getpid() } as u64;

    let mut tmp = Vec::with_capacity(path.len() + 20);
    tmp.extend_from_slice(path);
    tmp.extend_from_slice(b".tmp.");
    // Append the PID as decimal digits (no_std / no format machinery).
    let mut n = pid;
    if n == 0 {
        tmp.push(b'0');
    } else {
        let mut digits = [0u8; 20];
        let mut i = digits.len();
        while n > 0 {
            i -= 1;
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        tmp.extend_from_slice(&digits[i..]);
    }

    let fd = io::open(&tmp, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if fd < 0 {
        return false;
    }

    let written = io::write_all(fd, data);
    io::close(fd);

    if written != data.len() as isize {
        io::unlink(&tmp);
        return false;
    }

    if io::rename(&tmp, path) != 0 {
        io::unlink(&tmp);
        return false;
    }

    true
}

/// Get current timestamp
pub fn current_time() -> u64 {
    let mut tv: libc::timeval = unsafe { core::mem::zeroed() };
    unsafe { libc::gettimeofday(&mut tv, core::ptr::null_mut()) };
    tv.tv_sec as u64
}

#[cfg(test)]
mod tests {
    use super::valid_pkg_name;
    use super::*;

    #[test]
    fn test_record_checksum_roundtrip() {
        let rec = PackageRecord {
            name: String::from("pkg"),
            version: String::from("1.0"),
            description: String::from("desc"),
            install_time: 123,
            install_size: 456,
            depends: alloc_vec(&["dep1"]),
            provides: alloc_vec(&["prov1"]),
            files: alloc_vec(&["/usr/bin/a", "/usr/bin/b"]),
            checksums: vec![[7u8; 32], [9u8; 32]],
        };
        let bytes = rec.to_bytes();
        let parsed = PackageRecord::from_bytes(&bytes).expect("must parse");
        assert_eq!(parsed.files, rec.files);
        assert_eq!(parsed.checksums, rec.checksums);
    }

    #[test]
    fn test_record_backward_compat_no_checksums() {
        // Simulate a legacy record: serialize a record with an empty checksum
        // block, then strip the trailing 4-byte count to mimic a record from an
        // abp version that never wrote the block at all. Both must parse with
        // an empty checksums vec (and identical files).
        let rec = PackageRecord {
            name: String::from("legacy"),
            version: String::from("2.0"),
            description: String::from("d"),
            install_time: 1,
            install_size: 2,
            depends: Vec::new(),
            provides: Vec::new(),
            files: alloc_vec(&["/x"]),
            checksums: Vec::new(),
        };
        let mut bytes = rec.to_bytes();
        // Drop the trailing empty-checksum-count word (4 bytes) => truly legacy.
        bytes.truncate(bytes.len() - 4);
        let parsed = PackageRecord::from_bytes(&bytes).expect("legacy record must parse");
        assert_eq!(parsed.files, rec.files);
        assert!(parsed.checksums.is_empty());
    }

    fn alloc_vec(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| String::from(*s)).collect()
    }

    #[test]
    fn test_valid_pkg_name_accepts_normal_names() {
        assert!(valid_pkg_name(b"ripgrep"));
        assert!(valid_pkg_name(b"musl-dev"));
        assert!(valid_pkg_name(b"gcc-13.2.0"));
        assert!(valid_pkg_name(b"lib_foo"));
        assert!(valid_pkg_name(b"g++"));
        assert!(valid_pkg_name(b"a"));
    }

    #[test]
    fn test_valid_pkg_name_rejects_traversal() {
        // Empty
        assert!(!valid_pkg_name(b""));
        // Path separators escape the packages dir.
        assert!(!valid_pkg_name(b"../../etc/cron.d/x"));
        assert!(!valid_pkg_name(b"foo/bar"));
        assert!(!valid_pkg_name(b"/etc/passwd"));
        // Parent-dir components.
        assert!(!valid_pkg_name(b".."));
        assert!(!valid_pkg_name(b"foo..bar"));
        // Leading dot (hidden / relative).
        assert!(!valid_pkg_name(b"."));
        assert!(!valid_pkg_name(b".hidden"));
        // Disallowed characters.
        assert!(!valid_pkg_name(b"foo bar"));
        assert!(!valid_pkg_name(b"foo\0bar"));
        assert!(!valid_pkg_name(b"foo$bar"));
    }
}
