//! Compact Binary Package Index (.abpd)
//!
//! A `no_std`-compatible, zero-copy binary format for the ABP package
//! repository index. Designed for fast lookups on resource-constrained
//! systems (embedded, initramfs, `no_std` environments).
//!
//! # Format Overview
//!
//! ```text
//! ┌──────────────────────────────────┐
//! │ Header                  32 bytes │  Magic, version, counts, offsets
//! ├──────────────────────────────────┤
//! │ Name Index       8 × pkg_count  │  Sorted by name for binary search
//! ├──────────────────────────────────┤
//! │ Entry Table     48 × pkg_count  │  Package metadata (StringRefs)
//! ├──────────────────────────────────┤
//! │ String Table          variable   │  Deduplicated UTF-8 strings
//! └──────────────────────────────────┘
//! ```
//!
//! # Usage (Reader — `no_std`, zero-copy)
//!
//! ```ignore
//! let db = PackageIndex::from_bytes(data).unwrap();
//! if let Some(pkg) = db.lookup("ripgrep") {
//!     assert_eq!(pkg.version(), "14.1.1");
//! }
//! for pkg in db.iter() {
//!     // pkg.name(), pkg.version(), pkg.depends(), ...
//! }
//! ```
//!
//! # Usage (Writer — requires `alloc`)
//!
//! ```ignore
//! let mut builder = PackageIndexBuilder::new();
//! builder.add(PackageInfo {
//!     name: "ripgrep",
//!     version: "14.1.1",
//!     description: "Fast grep replacement",
//!     ..Default::default()
//! });
//! let bytes: Vec<u8> = builder.build();
//! ```

// ═══════════════════════════════════════════════════════════════════
// Binary format constants
// ═══════════════════════════════════════════════════════════════════

/// Magic bytes: "ABPD"
pub const MAGIC: [u8; 4] = *b"ABPD";

/// Current format version
pub const VERSION: u16 = 1;

/// Header size in bytes
pub const HEADER_SIZE: usize = 32;

/// Index entry size in bytes
pub const INDEX_ENTRY_SIZE: usize = 8;

/// Package entry size in bytes (8 StringRefs × 6 + 4 padding/flags)
pub const PACKAGE_ENTRY_SIZE: usize = 52;

// ═══════════════════════════════════════════════════════════════════
// On-disk structures (all little-endian)
// ═══════════════════════════════════════════════════════════════════

/// File header — first 32 bytes of the .abpd file
///
/// ```text
/// Offset  Size  Field
///   0       4   magic ("ABPD")
///   4       2   version (1)
///   6       2   flags (reserved, 0)
///   8       4   pkg_count
///  12       4   index_offset
///  16       4   entries_offset
///  20       4   strings_offset
///  24       4   strings_size
///  28       4   total_size
/// ```
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Header {
    pub magic: [u8; 4],
    pub version: u16,
    pub flags: u16,
    pub pkg_count: u32,
    pub index_offset: u32,
    pub entries_offset: u32,
    pub strings_offset: u32,
    pub strings_size: u32,
    pub total_size: u32,
}

/// A reference into the string table: (offset, length)
///
/// 6 bytes on disk. A zero-length ref points to "".
#[derive(Clone, Copy, Debug, Default)]
pub struct StringRef {
    pub offset: u32,
    pub len: u16,
}

/// Name index entry — sorted by name for binary search.
///
/// 8 bytes on disk.
///
/// ```text
/// Offset  Size  Field
///   0       4   name string offset
///   4       2   name string length
///   6       2   entry index (into the entry table)
/// ```
#[derive(Clone, Copy, Debug)]
pub struct IndexEntry {
    pub name_off: u32,
    pub name_len: u16,
    pub entry_idx: u16,
}

/// Package category tag (1 byte)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Category {
    Core = 0,
    System = 1,
    Network = 2,
    Text = 3,
    Archive = 4,
    Devel = 5,
    Shell = 6,
    Editor = 7,
    Misc = 8,
    Meta = 9,
    #[default]
    Unknown = 255,
}

impl Category {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Core,
            1 => Self::System,
            2 => Self::Network,
            3 => Self::Text,
            4 => Self::Archive,
            5 => Self::Devel,
            6 => Self::Shell,
            7 => Self::Editor,
            8 => Self::Misc,
            9 => Self::Meta,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::System => "system",
            Self::Network => "network",
            Self::Text => "text",
            Self::Archive => "archive",
            Self::Devel => "devel",
            Self::Shell => "shell",
            Self::Editor => "editor",
            Self::Misc => "misc",
            Self::Meta => "meta",
            Self::Unknown => "unknown",
        }
    }

    /// Guess category from package directory path or name
    pub fn from_name(name: &str) -> Self {
        match name {
            "musl" | "musl-dev" | "linux" | "linux-headers" | "zlib" => Self::Core,
            n if n.starts_with("llvm")
                || n.starts_with("clang")
                || n.starts_with("compiler-rt")
                || n.starts_with("lld")
                || n.starts_with("libcxx")
                || n == "gcc"
                || n == "binutils"
                || n == "make"
                || n == "cmake"
                || n == "ninja"
                || n == "rust"
                || n == "build-essential"
                || n == "pkgconf"
                || n == "bison"
                || n == "flex"
                || n == "nasm"
                || n == "perl"
                || n == "python3" =>
            {
                Self::Devel
            }
            "openssl" | "curl" => Self::Network,
            "xz" | "bzip2" | "zstd" => Self::Archive,
            n if n.starts_with("xorriso")
                || n.starts_with("syslinux")
                || n.starts_with("grub")
                || n.starts_with("mtools")
                || n.starts_with("dosfstools") =>
            {
                Self::System
            }
            "helix" => Self::Editor,
            "nushell" => Self::Shell,
            n if n.contains("coreutils") || n.contains("essential") => Self::Meta,
            _ => Self::Misc,
        }
    }
}

/// On-disk package entry (52 bytes)
///
/// ```text
/// Offset  Size  Field
///   0       6   name        (StringRef)
///   6       6   version     (StringRef)
///  12       6   description (StringRef)
///  18       6   url         (StringRef)
///  24       6   license     (StringRef)
///  30       6   arch        (StringRef)
///  36       6   depends     (StringRef, comma-separated)
///  42       6   provides    (StringRef, comma-separated)
///  48       1   category    (Category enum)
///  49       1   flags       (bitfield, reserved)
///  50       2   _reserved
/// ```
#[derive(Clone, Copy, Debug)]
pub struct PackageEntry {
    pub name: StringRef,
    pub version: StringRef,
    pub description: StringRef,
    pub url: StringRef,
    pub license: StringRef,
    pub arch: StringRef,
    pub depends: StringRef,
    pub provides: StringRef,
    pub category: u8,
    pub flags: u8,
    pub _reserved: u16,
}

// ═══════════════════════════════════════════════════════════════════
// Error type (no_std)
// ═══════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexError {
    /// Data too short for header
    TooShort,
    /// Magic bytes don't match
    BadMagic,
    /// Unsupported version
    BadVersion,
    /// Section offset/size exceeds data length
    BadOffset,
    /// String table reference out of bounds
    BadStringRef,
    /// Index not sorted (corrupt)
    NotSorted,
    /// Total size field doesn't match data length
    SizeMismatch,
}

// ═══════════════════════════════════════════════════════════════════
// Zero-copy reader (no_std, no alloc)
// ═══════════════════════════════════════════════════════════════════

/// A read-only view into a `.abpd` binary package index.
///
/// Zero-copy: all string references borrow from the underlying `&[u8]`.
/// Lookups by name are O(log n) via binary search on the sorted index.
///
/// # Lifetimes
///
/// Borrows the underlying byte slice for the entire lifetime of the
/// `PackageIndex`. All returned `&str` references share this lifetime.
pub struct PackageIndex<'a> {
    data: &'a [u8],
    header: Header,
}

impl<'a> core::fmt::Debug for PackageIndex<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PackageIndex")
            .field("version", &self.header.version)
            .field("pkg_count", &self.header.pkg_count)
            .field("total_size", &self.header.total_size)
            .finish()
    }
}

impl<'a> PackageIndex<'a> {
    /// Parse and validate a `.abpd` file from raw bytes.
    ///
    /// This only validates the header and section bounds — it does NOT
    /// copy or allocate. The returned `PackageIndex` borrows `data`.
    pub fn from_bytes(data: &'a [u8]) -> Result<Self, IndexError> {
        if data.len() < HEADER_SIZE {
            return Err(IndexError::TooShort);
        }

        let header = read_header(data);

        if header.magic != MAGIC {
            return Err(IndexError::BadMagic);
        }
        if header.version != VERSION {
            return Err(IndexError::BadVersion);
        }
        if header.total_size as usize != data.len() {
            return Err(IndexError::SizeMismatch);
        }

        // Validate section bounds
        let idx_end = header.index_offset as usize + (header.pkg_count as usize) * INDEX_ENTRY_SIZE;
        if idx_end > data.len() {
            return Err(IndexError::BadOffset);
        }

        let ent_end =
            header.entries_offset as usize + (header.pkg_count as usize) * PACKAGE_ENTRY_SIZE;
        if ent_end > data.len() {
            return Err(IndexError::BadOffset);
        }

        let str_end = header.strings_offset as usize + header.strings_size as usize;
        if str_end > data.len() {
            return Err(IndexError::BadOffset);
        }

        // Validate each name-index entry: `entry_idx` is attacker-controlled
        // and is used to index the entry table during lookup. Reject any
        // out-of-range index here so `lookup` can never panic. Also verify the
        // index is sorted by name, which binary search relies on.
        let pkg_count = header.pkg_count as usize;
        let strings_start = header.strings_offset as usize;
        let strings_size = header.strings_size as usize;
        let mut prev_name: Option<&[u8]> = None;
        for n in 0..pkg_count {
            let base = header.index_offset as usize + n * INDEX_ENTRY_SIZE;
            let entry_idx = read_u16(data, base + 6) as usize;
            if entry_idx >= pkg_count {
                return Err(IndexError::BadOffset);
            }

            // Resolve the name string for the sorted-order check.
            let name_off = read_u32(data, base) as usize;
            let name_len = read_u16(data, base + 4) as usize;
            let name: &[u8] = if name_len == 0 {
                b""
            } else {
                let start = strings_start + name_off;
                let end = start + name_len;
                // Name must lie within the declared string table.
                if name_off > strings_size || name_len > strings_size - name_off || end > data.len()
                {
                    return Err(IndexError::BadStringRef);
                }
                &data[start..end]
            };

            if let Some(prev) = prev_name {
                if name < prev {
                    return Err(IndexError::NotSorted);
                }
            }
            prev_name = Some(name);
        }

        Ok(PackageIndex { data, header })
    }

    /// Number of packages in the index
    #[inline]
    pub fn count(&self) -> usize {
        self.header.pkg_count as usize
    }

    /// Look up a package by exact name. O(log n) binary search.
    pub fn lookup(&'a self, name: &str) -> Option<PackageRef<'a>> {
        let count = self.header.pkg_count as usize;
        if count == 0 {
            return None;
        }

        let name_bytes = name.as_bytes();

        // Binary search on the sorted name index
        let mut lo: usize = 0;
        let mut hi: usize = count;

        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let idx_entry = self.read_index_entry(mid);
            let entry_name = self.resolve_str_raw(idx_entry.name_off, idx_entry.name_len);

            match entry_name.cmp(name_bytes) {
                core::cmp::Ordering::Equal => {
                    let pkg = self.read_package_entry(idx_entry.entry_idx as usize);
                    return Some(PackageRef {
                        index: self,
                        entry: pkg,
                    });
                }
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
            }
        }

        None
    }

    /// Get a package by its position in the entry table (0-based).
    pub fn get(&'a self, idx: usize) -> Option<PackageRef<'a>> {
        if idx >= self.count() {
            return None;
        }
        let entry = self.read_package_entry(idx);
        Some(PackageRef { index: self, entry })
    }

    /// Iterate all packages in entry-table order.
    pub fn iter(&'a self) -> PackageIter<'a> {
        PackageIter {
            index: self,
            pos: 0,
        }
    }

    /// Search for packages whose name contains the given substring.
    /// Returns indices into the entry table. O(n) scan.
    pub fn search<'q>(&'a self, query: &'q str) -> SearchIter<'a, 'q> {
        SearchIter {
            index: self,
            query,
            pos: 0,
        }
    }

    // ── Internal helpers ───────────────────────────────────────

    fn read_index_entry(&self, n: usize) -> IndexEntry {
        let base = self.header.index_offset as usize + n * INDEX_ENTRY_SIZE;
        let d = &self.data[base..];
        IndexEntry {
            name_off: read_u32(d, 0),
            name_len: read_u16(d, 4),
            entry_idx: read_u16(d, 6),
        }
    }

    fn read_package_entry(&self, n: usize) -> PackageEntry {
        let base = self.header.entries_offset as usize + n * PACKAGE_ENTRY_SIZE;
        let d = &self.data[base..];
        PackageEntry {
            name: read_sref(d, 0),
            version: read_sref(d, 6),
            description: read_sref(d, 12),
            url: read_sref(d, 18),
            license: read_sref(d, 24),
            arch: read_sref(d, 30),
            depends: read_sref(d, 36),
            provides: read_sref(d, 42),
            category: d[48],
            flags: d[49],
            _reserved: read_u16(d, 50),
        }
    }

    fn resolve_str_raw(&self, off: u32, len: u16) -> &'a [u8] {
        if len == 0 {
            return b"";
        }
        let start = self.header.strings_offset as usize + off as usize;
        let end = start + len as usize;
        if end <= self.data.len() {
            &self.data[start..end]
        } else {
            b""
        }
    }

    fn resolve_str(&self, sref: StringRef) -> &'a str {
        let raw = self.resolve_str_raw(sref.offset, sref.len);
        core::str::from_utf8(raw).unwrap_or("")
    }
}

/// A zero-copy reference to a single package in the index.
#[derive(Clone, Copy)]
pub struct PackageRef<'a> {
    index: &'a PackageIndex<'a>,
    entry: PackageEntry,
}

impl<'a> PackageRef<'a> {
    #[inline]
    pub fn name(&self) -> &'a str {
        self.index.resolve_str(self.entry.name)
    }
    #[inline]
    pub fn version(&self) -> &'a str {
        self.index.resolve_str(self.entry.version)
    }
    #[inline]
    pub fn description(&self) -> &'a str {
        self.index.resolve_str(self.entry.description)
    }
    #[inline]
    pub fn url(&self) -> &'a str {
        self.index.resolve_str(self.entry.url)
    }
    #[inline]
    pub fn license(&self) -> &'a str {
        self.index.resolve_str(self.entry.license)
    }
    #[inline]
    pub fn arch(&self) -> &'a str {
        self.index.resolve_str(self.entry.arch)
    }
    #[inline]
    pub fn category(&self) -> Category {
        Category::from_u8(self.entry.category)
    }
    #[inline]
    pub fn flags(&self) -> u8 {
        self.entry.flags
    }

    /// Raw comma-separated depends string
    #[inline]
    pub fn depends_raw(&self) -> &'a str {
        self.index.resolve_str(self.entry.depends)
    }

    /// Raw comma-separated provides string
    #[inline]
    pub fn provides_raw(&self) -> &'a str {
        self.index.resolve_str(self.entry.provides)
    }

    /// Iterate dependency names (splits on comma)
    pub fn depends(&self) -> FieldIter<'a> {
        FieldIter {
            remaining: self.depends_raw(),
        }
    }

    /// Iterate provides names (splits on comma)
    pub fn provides(&self) -> FieldIter<'a> {
        FieldIter {
            remaining: self.provides_raw(),
        }
    }
}

/// Iterator over comma-separated field values
pub struct FieldIter<'a> {
    remaining: &'a str,
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.remaining.is_empty() {
                return None;
            }
            // Find next comma or end
            let (item, rest) = match self.remaining.find(',') {
                Some(pos) => {
                    let item = &self.remaining[..pos];
                    let rest = if pos + 1 < self.remaining.len() {
                        &self.remaining[pos + 1..]
                    } else {
                        ""
                    };
                    (item, rest)
                }
                None => (self.remaining, ""),
            };
            self.remaining = rest;
            let trimmed = item.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
}

/// Iterator over all packages in the index
pub struct PackageIter<'a> {
    index: &'a PackageIndex<'a>,
    pos: usize,
}

impl<'a> Iterator for PackageIter<'a> {
    type Item = PackageRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.index.count() {
            return None;
        }
        let pkg = self.index.get(self.pos);
        self.pos += 1;
        pkg
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.index.count() - self.pos;
        (remaining, Some(remaining))
    }
}

/// Iterator that yields packages matching a substring query
pub struct SearchIter<'a, 'q> {
    index: &'a PackageIndex<'a>,
    query: &'q str,
    pos: usize,
}

impl<'a, 'q> Iterator for SearchIter<'a, 'q> {
    type Item = PackageRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.index.count() {
            let pkg = self.index.get(self.pos)?;
            self.pos += 1;
            if contains_ignore_ascii_case(pkg.name(), self.query)
                || contains_ignore_ascii_case(pkg.description(), self.query)
            {
                return Some(pkg);
            }
        }
        None
    }
}

/// Simple ASCII case-insensitive substring search (no_std)
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.len() > h.len() {
        return false;
    }
    'outer: for i in 0..=(h.len() - n.len()) {
        for j in 0..n.len() {
            if h[i + j].to_ascii_lowercase() != n[j].to_ascii_lowercase() {
                continue 'outer;
            }
        }
        return true;
    }
    false
}

// ═══════════════════════════════════════════════════════════════════
// Raw byte helpers (no_std, no alloc)
// ═══════════════════════════════════════════════════════════════════

#[inline]
fn read_u16(d: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([d[off], d[off + 1]])
}

#[inline]
fn read_u32(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
}

#[inline]
fn read_sref(d: &[u8], off: usize) -> StringRef {
    StringRef {
        offset: read_u32(d, off),
        len: read_u16(d, off + 4),
    }
}

fn read_header(data: &[u8]) -> Header {
    Header {
        magic: [data[0], data[1], data[2], data[3]],
        version: read_u16(data, 4),
        flags: read_u16(data, 6),
        pkg_count: read_u32(data, 8),
        index_offset: read_u32(data, 12),
        entries_offset: read_u32(data, 16),
        strings_offset: read_u32(data, 20),
        strings_size: read_u32(data, 24),
        total_size: read_u32(data, 28),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Builder (requires alloc)
// ═══════════════════════════════════════════════════════════════════

#[cfg(feature = "alloc")]
pub mod build {
    extern crate alloc;
    use super::*;
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// Input to the builder — one per package
    #[derive(Clone, Debug, Default)]
    pub struct PackageInfo {
        pub name: String,
        pub version: String,
        pub description: String,
        pub url: String,
        pub license: String,
        pub arch: String,
        pub depends: String,  // comma-separated
        pub provides: String, // comma-separated
        pub category: Category,
    }

    /// Builds a `.abpd` binary blob from a list of packages
    pub struct PackageIndexBuilder {
        packages: Vec<PackageInfo>,
    }

    impl PackageIndexBuilder {
        pub fn new() -> Self {
            PackageIndexBuilder {
                packages: Vec::new(),
            }
        }

        pub fn add(&mut self, info: PackageInfo) {
            self.packages.push(info);
        }

        pub fn count(&self) -> usize {
            self.packages.len()
        }

        /// Produce the complete `.abpd` binary
        pub fn build(&self) -> Vec<u8> {
            let pkg_count = self.packages.len();

            // ── 1. Build deduplicated string table ──────────
            let mut string_table = StringTable::new();
            for pkg in &self.packages {
                string_table.intern(&pkg.name);
                string_table.intern(&pkg.version);
                string_table.intern(&pkg.description);
                string_table.intern(&pkg.url);
                string_table.intern(&pkg.license);
                string_table.intern(&pkg.arch);
                string_table.intern(&pkg.depends);
                string_table.intern(&pkg.provides);
            }

            // ── 2. Compute section offsets ──────────────────
            let index_offset = HEADER_SIZE as u32;
            let entries_offset = index_offset + (pkg_count as u32) * (INDEX_ENTRY_SIZE as u32);
            let strings_offset = entries_offset + (pkg_count as u32) * (PACKAGE_ENTRY_SIZE as u32);
            let strings_size = string_table.len() as u32;
            let total_size = strings_offset + strings_size;

            // ── 3. Build entry table + index table ──────────
            // Sort packages by name for the index
            let mut sorted_indices: Vec<usize> = (0..pkg_count).collect();
            sorted_indices.sort_by(|&a, &b| self.packages[a].name.cmp(&self.packages[b].name));

            let mut buf: Vec<u8> = Vec::with_capacity(total_size as usize);

            // ── 4. Write header ─────────────────────────────
            buf.extend_from_slice(&MAGIC);
            buf.extend_from_slice(&VERSION.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes()); // flags
            buf.extend_from_slice(&(pkg_count as u32).to_le_bytes());
            buf.extend_from_slice(&index_offset.to_le_bytes());
            buf.extend_from_slice(&entries_offset.to_le_bytes());
            buf.extend_from_slice(&strings_offset.to_le_bytes());
            buf.extend_from_slice(&strings_size.to_le_bytes());
            buf.extend_from_slice(&total_size.to_le_bytes());

            // ── 5. Write name index (sorted) ────────────────
            for &orig_idx in &sorted_indices {
                let pkg = &self.packages[orig_idx];
                let sref = string_table.get(&pkg.name);
                buf.extend_from_slice(&sref.offset.to_le_bytes());
                buf.extend_from_slice(&sref.len.to_le_bytes());
                buf.extend_from_slice(&(orig_idx as u16).to_le_bytes());
            }

            // ── 6. Write entry table (original order) ───────
            for pkg in &self.packages {
                write_sref(&mut buf, &string_table.get(&pkg.name));
                write_sref(&mut buf, &string_table.get(&pkg.version));
                write_sref(&mut buf, &string_table.get(&pkg.description));
                write_sref(&mut buf, &string_table.get(&pkg.url));
                write_sref(&mut buf, &string_table.get(&pkg.license));
                write_sref(&mut buf, &string_table.get(&pkg.arch));
                write_sref(&mut buf, &string_table.get(&pkg.depends));
                write_sref(&mut buf, &string_table.get(&pkg.provides));
                buf.push(pkg.category as u8);
                buf.push(0); // flags
                buf.extend_from_slice(&0u16.to_le_bytes()); // reserved
            }

            // ── 7. Write string table ───────────────────────
            buf.extend_from_slice(string_table.bytes());

            debug_assert_eq!(buf.len(), total_size as usize);

            buf
        }
    }

    fn write_sref(buf: &mut Vec<u8>, sref: &StringRef) {
        buf.extend_from_slice(&sref.offset.to_le_bytes());
        buf.extend_from_slice(&sref.len.to_le_bytes());
    }

    /// Deduplicated string table builder
    struct StringTable {
        data: Vec<u8>,
        map: BTreeMap<String, StringRef>,
    }

    impl StringTable {
        fn new() -> Self {
            StringTable {
                data: Vec::new(),
                map: BTreeMap::new(),
            }
        }

        fn intern(&mut self, s: &str) -> StringRef {
            if s.is_empty() {
                return StringRef { offset: 0, len: 0 };
            }
            if let Some(&sref) = self.map.get(s) {
                return sref;
            }
            let offset = self.data.len() as u32;
            let len = s.len() as u16;
            self.data.extend_from_slice(s.as_bytes());
            let sref = StringRef { offset, len };
            self.map.insert(String::from(s), sref);
            sref
        }

        fn get(&self, s: &str) -> StringRef {
            if s.is_empty() {
                return StringRef { offset: 0, len: 0 };
            }
            self.map
                .get(s)
                .copied()
                .unwrap_or(StringRef { offset: 0, len: 0 })
        }

        fn len(&self) -> usize {
            self.data.len()
        }

        fn bytes(&self) -> &[u8] {
            &self.data
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    fn sample_db() -> Vec<u8> {
        let mut builder = PackageIndexBuilder::new();

        builder.add(PackageInfo {
            name: "ripgrep".into(),
            version: "14.1.1".into(),
            description: "Fast grep replacement".into(),
            url: "https://github.com/BurntSushi/ripgrep".into(),
            license: "MIT".into(),
            arch: "x86_64".into(),
            depends: "musl".into(),
            provides: "rg".into(),
            category: Category::Misc,
            ..Default::default()
        });

        builder.add(PackageInfo {
            name: "bat".into(),
            version: "0.24.0".into(),
            description: "Cat clone with syntax highlighting".into(),
            url: "https://github.com/sharkdp/bat".into(),
            license: "MIT".into(),
            arch: "x86_64".into(),
            depends: "musl".into(),
            provides: "".into(),
            category: Category::Misc,
            ..Default::default()
        });

        builder.add(PackageInfo {
            name: "musl".into(),
            version: "1.2.5".into(),
            description: "musl C library".into(),
            url: "https://musl.libc.org".into(),
            license: "MIT".into(),
            arch: "x86_64".into(),
            depends: "".into(),
            provides: "".into(),
            category: Category::Core,
            ..Default::default()
        });

        builder.build()
    }

    #[test]
    fn test_roundtrip() {
        let data = sample_db();
        let idx = PackageIndex::from_bytes(&data).expect("parse failed");
        assert_eq!(idx.count(), 3);
    }

    #[test]
    fn test_lookup() {
        let data = sample_db();
        let idx = PackageIndex::from_bytes(&data).expect("parse failed");

        let rg = idx.lookup("ripgrep").expect("ripgrep not found");
        assert_eq!(rg.name(), "ripgrep");
        assert_eq!(rg.version(), "14.1.1");
        assert_eq!(rg.description(), "Fast grep replacement");
        assert_eq!(rg.license(), "MIT");
        assert_eq!(rg.arch(), "x86_64");
        assert_eq!(rg.depends_raw(), "musl");
        assert_eq!(rg.provides_raw(), "rg");
        assert_eq!(rg.category(), Category::Misc);

        let bat = idx.lookup("bat").expect("bat not found");
        assert_eq!(bat.version(), "0.24.0");

        assert!(idx.lookup("nonexistent").is_none());
    }

    #[test]
    fn test_iterate() {
        let data = sample_db();
        let idx = PackageIndex::from_bytes(&data).expect("parse failed");

        let names: Vec<&str> = idx.iter().map(|p| p.name()).collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"ripgrep"));
        assert!(names.contains(&"bat"));
        assert!(names.contains(&"musl"));
    }

    #[test]
    fn test_depends_iter() {
        let mut builder = PackageIndexBuilder::new();
        builder.add(PackageInfo {
            name: "test".into(),
            depends: "musl, openssl, zlib".into(),
            ..Default::default()
        });
        let data = builder.build();
        let idx = PackageIndex::from_bytes(&data).unwrap();
        let pkg = idx.lookup("test").unwrap();
        let deps: Vec<&str> = pkg.depends().collect();
        assert_eq!(deps, vec!["musl", "openssl", "zlib"]);
    }

    #[test]
    fn test_search() {
        let data = sample_db();
        let idx = PackageIndex::from_bytes(&data).unwrap();

        let results: Vec<&str> = idx.search("grep").map(|p| p.name()).collect();
        assert_eq!(results, vec!["ripgrep"]);

        let results: Vec<&str> = idx.search("MUSL").map(|p| p.name()).collect();
        assert!(results.contains(&"musl"));
    }

    #[test]
    fn test_string_dedup() {
        let mut builder = PackageIndexBuilder::new();
        // Both packages share "musl" in depends and "MIT" in license
        builder.add(PackageInfo {
            name: "a".into(),
            license: "MIT".into(),
            depends: "musl".into(),
            ..Default::default()
        });
        builder.add(PackageInfo {
            name: "b".into(),
            license: "MIT".into(),
            depends: "musl".into(),
            ..Default::default()
        });
        let data = builder.build();
        let idx = PackageIndex::from_bytes(&data).unwrap();
        assert_eq!(idx.count(), 2);
        // "MIT" and "musl" should each appear only once in the string table
        let strings = &data[idx.header.strings_offset as usize..];
        let strings_str = core::str::from_utf8(strings).unwrap();
        assert_eq!(strings_str.matches("MIT").count(), 1);
        assert_eq!(strings_str.matches("musl").count(), 1);
    }

    #[test]
    fn test_empty_db() {
        let builder = PackageIndexBuilder::new();
        let data = builder.build();
        let idx = PackageIndex::from_bytes(&data).unwrap();
        assert_eq!(idx.count(), 0);
        assert!(idx.lookup("anything").is_none());
        assert_eq!(idx.iter().count(), 0);
    }

    #[test]
    fn test_bad_magic() {
        let mut data = sample_db();
        data[0] = b'X';
        assert_eq!(
            PackageIndex::from_bytes(&data).unwrap_err(),
            IndexError::BadMagic
        );
    }

    #[test]
    fn test_out_of_range_entry_idx_rejected() {
        // A crafted .abpd whose first name-index entry points at entry 0xFFFF
        // must be rejected by from_bytes rather than panicking in lookup.
        let mut data = sample_db();
        let idx = PackageIndex::from_bytes(&data).unwrap();
        let index_offset = idx.header.index_offset as usize;
        // entry_idx is at byte offset 6 within the first 8-byte index entry.
        let ei = index_offset + 6;
        data[ei] = 0xFF;
        data[ei + 1] = 0xFF;
        assert_eq!(
            PackageIndex::from_bytes(&data).unwrap_err(),
            IndexError::BadOffset
        );
    }

    #[test]
    fn test_unsorted_index_rejected() {
        // Swap the two 8-byte index entries at the front so the name index is
        // no longer sorted; binary search requires sorted order.
        let data = sample_db();
        let idx = PackageIndex::from_bytes(&data).unwrap();
        let io = idx.header.index_offset as usize;
        let count = idx.header.pkg_count as usize;
        assert!(count >= 2);
        let mut bad = data.clone();
        // Reverse the order of the sorted index entries.
        for n in 0..count {
            let src = io + n * INDEX_ENTRY_SIZE;
            let dst = io + (count - 1 - n) * INDEX_ENTRY_SIZE;
            bad[dst..dst + INDEX_ENTRY_SIZE].copy_from_slice(&data[src..src + INDEX_ENTRY_SIZE]);
        }
        assert_eq!(
            PackageIndex::from_bytes(&bad).unwrap_err(),
            IndexError::NotSorted
        );
    }
}
