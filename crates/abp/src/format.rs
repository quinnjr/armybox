//! ABP Package Format
//!
//! Defines the .abp package file format and parsing.
//!
//! # Format Structure
//! ```text
//! Offset  Size    Field
//! 0       4       Magic "ABP\x00"
//! 4       2       Version (major.minor)
//! 6       2       Flags
//! 8       8       Metadata offset
//! 16      8       Metadata size (compressed)
//! 24      8       Manifest offset
//! 32      8       Manifest size
//! 40      8       Payload offset
//! 48      8       Payload size (compressed)
//! 56      8       Reserved
//! 64      var     Metadata (TOML, optionally compressed)
//! ...     var     Manifest (file checksums)
//! ...     var     Payload (tar archive, zstd compressed when COMPRESSED_PAYLOAD flag set)
//! ...     64      Ed25519 signature
//! ...     32      Key ID (SHA256 of public key)
//! ```

extern crate alloc;

use crate::crypto::{sha256, verify_signature};
use crate::io;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// ABP magic bytes
pub const ABP_MAGIC: [u8; 4] = *b"ABP\x00";

/// Current format version
pub const ABP_VERSION_MAJOR: u16 = 1;
pub const ABP_VERSION_MINOR: u16 = 0;

/// Header size
pub const ABP_HEADER_SIZE: usize = 64;

/// Signature size (Ed25519)
pub const ABP_SIGNATURE_SIZE: usize = 64;

/// Key ID size (SHA256)
pub const ABP_KEY_ID_SIZE: usize = 32;

/// Maximum decompressed payload size (256 MiB).
///
/// Bounds zstd decompression to defend against decompression bombs in
/// untrusted packages. Decompression that would exceed this cap fails
/// rather than allocating without limit.
pub const ABP_MAX_PAYLOAD: usize = 256 * 1024 * 1024;

/// Package flags
pub mod flags {
    pub const SIGNED: u16 = 0x0001;
    pub const COMPRESSED_METADATA: u16 = 0x0002;
    /// Payload is zstd-compressed (tar.zst)
    pub const COMPRESSED_PAYLOAD: u16 = 0x0004;
}

/// ABP file header
#[derive(Clone, Debug)]
pub struct AbpHeader {
    pub version_major: u16,
    pub version_minor: u16,
    pub flags: u16,
    pub metadata_offset: u64,
    pub metadata_size: u64,
    pub manifest_offset: u64,
    pub manifest_size: u64,
    pub payload_offset: u64,
    pub payload_size: u64,
}

impl AbpHeader {
    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < ABP_HEADER_SIZE {
            return None;
        }

        // Check magic
        if data[0..4] != ABP_MAGIC {
            return None;
        }

        Some(AbpHeader {
            version_major: u16::from_le_bytes([data[4], data[5]]),
            version_minor: u16::from_le_bytes([data[6], data[7]]),
            flags: u16::from_le_bytes([data[8], data[9]]),
            metadata_offset: u64::from_le_bytes([
                data[10], data[11], data[12], data[13], data[14], data[15], data[16], data[17],
            ]),
            metadata_size: u64::from_le_bytes([
                data[18], data[19], data[20], data[21], data[22], data[23], data[24], data[25],
            ]),
            manifest_offset: u64::from_le_bytes([
                data[26], data[27], data[28], data[29], data[30], data[31], data[32], data[33],
            ]),
            manifest_size: u64::from_le_bytes([
                data[34], data[35], data[36], data[37], data[38], data[39], data[40], data[41],
            ]),
            payload_offset: u64::from_le_bytes([
                data[42], data[43], data[44], data[45], data[46], data[47], data[48], data[49],
            ]),
            payload_size: u64::from_le_bytes([
                data[50], data[51], data[52], data[53], data[54], data[55], data[56], data[57],
            ]),
        })
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; ABP_HEADER_SIZE] {
        let mut buf = [0u8; ABP_HEADER_SIZE];

        buf[0..4].copy_from_slice(&ABP_MAGIC);
        buf[4..6].copy_from_slice(&self.version_major.to_le_bytes());
        buf[6..8].copy_from_slice(&self.version_minor.to_le_bytes());
        buf[8..10].copy_from_slice(&self.flags.to_le_bytes());

        buf[10..18].copy_from_slice(&self.metadata_offset.to_le_bytes());
        buf[18..26].copy_from_slice(&self.metadata_size.to_le_bytes());
        buf[26..34].copy_from_slice(&self.manifest_offset.to_le_bytes());
        buf[34..42].copy_from_slice(&self.manifest_size.to_le_bytes());
        buf[42..50].copy_from_slice(&self.payload_offset.to_le_bytes());
        buf[50..58].copy_from_slice(&self.payload_size.to_le_bytes());

        buf
    }

    /// Check if package is signed
    pub fn is_signed(&self) -> bool {
        self.flags & flags::SIGNED != 0
    }
}

/// Package metadata (parsed from TOML)
#[derive(Clone, Debug, Default)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub maintainer: String,
    pub homepage: String,
    pub license: String,
    pub arch: String,
    pub depends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub install_size: u64,
}

impl PackageMetadata {
    /// Parse metadata from TOML-like format
    pub fn from_toml(data: &[u8]) -> Option<Self> {
        let mut meta = PackageMetadata::default();
        let text = core::str::from_utf8(data).ok()?;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "name" => meta.name = String::from(value),
                    "version" => meta.version = String::from(value),
                    "description" => meta.description = String::from(value),
                    "maintainer" => meta.maintainer = String::from(value),
                    "homepage" => meta.homepage = String::from(value),
                    "license" => meta.license = String::from(value),
                    "arch" => meta.arch = String::from(value),
                    "install_size" => meta.install_size = value.parse().unwrap_or(0),
                    "depends" => meta.depends = parse_list(value),
                    "provides" => meta.provides = parse_list(value),
                    "conflicts" => meta.conflicts = parse_list(value),
                    "replaces" => meta.replaces = parse_list(value),
                    _ => {}
                }
            }
        }

        if meta.name.is_empty() || meta.version.is_empty() {
            return None;
        }

        Some(meta)
    }

    /// Serialize to TOML format
    pub fn to_toml(&self) -> Vec<u8> {
        let mut result = Vec::new();

        result.extend_from_slice(b"[package]\n");
        write_field(&mut result, b"name", self.name.as_bytes());
        write_field(&mut result, b"version", self.version.as_bytes());

        if !self.description.is_empty() {
            write_field(&mut result, b"description", self.description.as_bytes());
        }
        if !self.maintainer.is_empty() {
            write_field(&mut result, b"maintainer", self.maintainer.as_bytes());
        }
        if !self.homepage.is_empty() {
            write_field(&mut result, b"homepage", self.homepage.as_bytes());
        }
        if !self.license.is_empty() {
            write_field(&mut result, b"license", self.license.as_bytes());
        }
        if !self.arch.is_empty() {
            write_field(&mut result, b"arch", self.arch.as_bytes());
        }
        if self.install_size > 0 {
            result.extend_from_slice(b"install_size = ");
            write_number(&mut result, self.install_size);
            result.push(b'\n');
        }

        if !self.depends.is_empty() {
            write_list(&mut result, b"depends", &self.depends);
        }
        if !self.provides.is_empty() {
            write_list(&mut result, b"provides", &self.provides);
        }
        if !self.conflicts.is_empty() {
            write_list(&mut result, b"conflicts", &self.conflicts);
        }
        if !self.replaces.is_empty() {
            write_list(&mut result, b"replaces", &self.replaces);
        }

        result
    }

    /// Get package identifier (name-version)
    pub fn id(&self) -> String {
        let mut id = self.name.clone();
        id.push('-');
        id.push_str(&self.version);
        id
    }
}

fn parse_list(value: &str) -> Vec<String> {
    let value = value.trim_matches(|c| c == '[' || c == ']');
    value
        .split(',')
        .map(|s| String::from(s.trim().trim_matches('"')))
        .filter(|s| !s.is_empty())
        .collect()
}

fn write_field(buf: &mut Vec<u8>, key: &[u8], value: &[u8]) {
    buf.extend_from_slice(key);
    buf.extend_from_slice(b" = \"");
    buf.extend_from_slice(value);
    buf.extend_from_slice(b"\"\n");
}

fn write_list(buf: &mut Vec<u8>, key: &[u8], items: &[String]) {
    buf.extend_from_slice(key);
    buf.extend_from_slice(b" = [");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf.extend_from_slice(b", ");
        }
        buf.push(b'"');
        buf.extend_from_slice(item.as_bytes());
        buf.push(b'"');
    }
    buf.extend_from_slice(b"]\n");
}

fn write_number(buf: &mut Vec<u8>, mut n: u64) {
    if n == 0 {
        buf.push(b'0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        buf.push(digits[i]);
    }
}

/// File manifest entry
#[derive(Clone, Debug)]
pub struct ManifestEntry {
    pub path: String,
    pub size: u64,
    pub mode: u32,
    pub checksum: [u8; 32],
}

impl ManifestEntry {
    /// Parse manifest entry from line
    pub fn from_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 4 {
            return None;
        }

        let checksum = parse_hex_32(parts[0])?;
        let size = parts[1].parse().ok()?;
        let mode = u32::from_str_radix(parts[2], 8).ok()?;
        let path = String::from(parts[3]);

        Some(ManifestEntry {
            path,
            size,
            mode,
            checksum,
        })
    }

    /// Serialize to line
    pub fn to_line(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Checksum (hex)
        const HEX: &[u8] = b"0123456789abcdef";
        for &byte in &self.checksum {
            buf.push(HEX[(byte >> 4) as usize]);
            buf.push(HEX[(byte & 0xf) as usize]);
        }

        buf.push(b'\t');

        // Size
        write_number(&mut buf, self.size);
        buf.push(b'\t');

        // Mode (octal)
        write_octal(&mut buf, self.mode);
        buf.push(b'\t');

        // Path
        buf.extend_from_slice(self.path.as_bytes());
        buf.push(b'\n');

        buf
    }
}

fn write_octal(buf: &mut Vec<u8>, mut n: u32) {
    if n == 0 {
        buf.push(b'0');
        return;
    }
    let mut digits = [0u8; 12];
    let mut i = 0;
    while n > 0 {
        digits[i] = b'0' + (n & 7) as u8;
        n >>= 3;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        buf.push(digits[i]);
    }
}

fn parse_hex_32(s: &str) -> Option<[u8; 32]> {
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

/// Package manifest (list of files with checksums)
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// Parse manifest from text
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        let text = core::str::from_utf8(data).ok()?;
        let mut entries = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(entry) = ManifestEntry::from_line(line) {
                entries.push(entry);
            }
        }

        Some(Manifest { entries })
    }

    /// Serialize manifest to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"# ABP Manifest\n");
        buf.extend_from_slice(b"# checksum\tsize\tmode\tpath\n");
        for entry in &self.entries {
            buf.extend_from_slice(&entry.to_line());
        }
        buf
    }

    /// Find entry by path
    pub fn find(&self, path: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.path == path)
    }
}

/// Complete ABP package
pub struct AbpPackage {
    pub header: AbpHeader,
    pub metadata: PackageMetadata,
    pub manifest: Manifest,
    pub payload: Vec<u8>,
    pub signature: Option<[u8; 64]>,
    pub key_id: Option<[u8; 32]>,
    /// The exact on-disk bytes the signature covers: `data[0..payload_end]`,
    /// i.e. header ‖ metadata ‖ manifest ‖ payload as they appeared in the
    /// file, excluding the trailing signature + key-id. Populated only by
    /// [`AbpPackage::from_bytes`]; `None` for packages built in memory (which
    /// therefore cannot be signature-verified, as there is no authenticated
    /// byte representation to check).
    pub signed_region: Option<Vec<u8>>,
}

impl AbpPackage {
    /// Read package from file
    pub fn from_file(path: &[u8]) -> Option<Self> {
        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd < 0 {
            return None;
        }

        let data = io::read_all(fd);
        io::close(fd);

        Self::from_bytes(&data)
    }

    /// Parse package from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < ABP_HEADER_SIZE {
            return None;
        }

        let header = AbpHeader::from_bytes(data)?;

        // Version check
        if header.version_major > ABP_VERSION_MAJOR {
            return None;
        }

        // Extract metadata
        let meta_start = header.metadata_offset as usize;
        if meta_start < ABP_HEADER_SIZE {
            return None;
        }
        // `start + size` can overflow usize (values come straight off disk),
        // wrapping past the `end > data.len()` guard into an OOB slice panic.
        // Use checked_add and bound both endpoints against the buffer length.
        let meta_end = meta_start.checked_add(header.metadata_size as usize)?;
        if meta_start > data.len() || meta_end > data.len() {
            return None;
        }
        let meta_data = &data[meta_start..meta_end];

        // Decompress metadata if needed
        let meta_bytes = if header.flags & flags::COMPRESSED_METADATA != 0 {
            // TODO: XZ decompress
            meta_data.to_vec()
        } else {
            meta_data.to_vec()
        };

        let metadata = PackageMetadata::from_toml(&meta_bytes)?;

        // Extract manifest
        let manifest_start = header.manifest_offset as usize;
        if manifest_start < ABP_HEADER_SIZE {
            return None;
        }
        let manifest_end = manifest_start.checked_add(header.manifest_size as usize)?;
        if manifest_start > data.len() || manifest_end > data.len() {
            return None;
        }
        let manifest = Manifest::from_bytes(&data[manifest_start..manifest_end])?;

        // Extract payload
        let payload_start = header.payload_offset as usize;
        if payload_start < ABP_HEADER_SIZE {
            return None;
        }
        let payload_end = payload_start.checked_add(header.payload_size as usize)?;
        if payload_start > data.len() || payload_end > data.len() {
            return None;
        }
        let payload = data[payload_start..payload_end].to_vec();

        // Extract signature if present
        let (signature, key_id) = if header.is_signed() {
            let sig_start = payload_end;
            // Each trailer offset can overflow usize as well; bound them all.
            let key_start = sig_start.checked_add(ABP_SIGNATURE_SIZE)?;
            let key_end = key_start.checked_add(ABP_KEY_ID_SIZE)?;

            if sig_start > data.len() || key_start > data.len() || key_end > data.len() {
                return None;
            }

            let mut sig = [0u8; 64];
            let mut kid = [0u8; 32];
            sig.copy_from_slice(&data[sig_start..key_start]);
            kid.copy_from_slice(&data[key_start..key_end]);

            (Some(sig), Some(kid))
        } else {
            (None, None)
        };

        // Capture the exact bytes the signature covers (everything before the
        // signature trailer) so verification checks the received bytes rather
        // than a re-serialization of the parsed structures.
        let signed_region = Some(data[..payload_end].to_vec());

        Some(AbpPackage {
            header,
            metadata,
            manifest,
            payload,
            signature,
            key_id,
            signed_region,
        })
    }

    /// Write package to file
    pub fn write_to_file(&self, path: &[u8]) -> bool {
        let data = self.to_bytes();
        let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
        if fd < 0 {
            return false;
        }
        let written = io::write_all(fd, &data);
        io::close(fd);
        written == data.len() as isize
    }

    /// Serialize package to bytes
    ///
    /// The payload is always zstd-compressed in the output. If the payload is
    /// already compressed (COMPRESSED_PAYLOAD flag set in header), it is used
    /// as-is. Otherwise, it is compressed before writing.
    pub fn to_bytes(&self) -> Vec<u8> {
        let meta_bytes = self.metadata.to_toml();
        let manifest_bytes = self.manifest.to_bytes();

        // Compress payload if not already compressed
        let (payload_bytes, payload_compressed) =
            if self.header.flags & flags::COMPRESSED_PAYLOAD != 0 {
                // Already compressed (e.g. loaded from file), use as-is
                (self.payload.clone(), true)
            } else if !self.payload.is_empty() {
                // Compress with zstd (level 3)
                (zstd_nostd::compress(&self.payload, 3), true)
            } else {
                (self.payload.clone(), false)
            };

        let metadata_offset = ABP_HEADER_SIZE as u64;
        let manifest_offset = metadata_offset + meta_bytes.len() as u64;
        let payload_offset = manifest_offset + manifest_bytes.len() as u64;

        let mut flags = 0u16;
        if self.signature.is_some() {
            flags |= flags::SIGNED;
        }
        if payload_compressed {
            flags |= flags::COMPRESSED_PAYLOAD;
        }

        let header = AbpHeader {
            version_major: ABP_VERSION_MAJOR,
            version_minor: ABP_VERSION_MINOR,
            flags,
            metadata_offset,
            metadata_size: meta_bytes.len() as u64,
            manifest_offset,
            manifest_size: manifest_bytes.len() as u64,
            payload_offset,
            payload_size: payload_bytes.len() as u64,
        };

        let mut data = Vec::new();
        data.extend_from_slice(&header.to_bytes());
        data.extend_from_slice(&meta_bytes);
        data.extend_from_slice(&manifest_bytes);
        data.extend_from_slice(&payload_bytes);

        if let (Some(sig), Some(kid)) = (&self.signature, &self.key_id) {
            data.extend_from_slice(sig);
            data.extend_from_slice(kid);
        }

        data
    }

    /// Get the decompressed payload data.
    ///
    /// If the payload is zstd-compressed (COMPRESSED_PAYLOAD flag set), it is
    /// decompressed. Otherwise, the raw payload is returned.
    pub fn decompressed_payload(&self) -> Option<Vec<u8>> {
        if self.header.flags & flags::COMPRESSED_PAYLOAD != 0 {
            zstd_nostd::decompress_bounded(&self.payload, ABP_MAX_PAYLOAD).ok()
        } else {
            Some(self.payload.clone())
        }
    }

    /// Verify package signature
    pub fn verify_signature(&self, public_key: &[u8; 32]) -> bool {
        let signature = match &self.signature {
            Some(s) => s,
            None => return false,
        };

        // Verify key ID matches
        if let Some(kid) = &self.key_id {
            let expected_kid = sha256(public_key);
            if kid != &expected_kid {
                return false;
            }
        }

        // Verify over the exact on-disk bytes the signature covers. Only a
        // package parsed from bytes has this; an in-memory package has no
        // authenticated representation and cannot be verified.
        let signed_data = match &self.signed_region {
            Some(region) => region,
            None => return false,
        };

        verify_signature(public_key, signed_data, signature)
    }

    /// Verify file checksums in manifest
    pub fn verify_checksums(&self, root: &[u8]) -> Vec<String> {
        let mut errors = Vec::new();

        for entry in &self.manifest.entries {
            let mut path = root.to_vec();
            if !path.ends_with(b"/") {
                path.push(b'/');
            }
            path.extend_from_slice(entry.path.as_bytes());

            let fd = io::open(&path, libc::O_RDONLY, 0);
            if fd < 0 {
                errors.push(format!("missing: {}", entry.path));
                continue;
            }

            let data = io::read_all(fd);
            io::close(fd);

            let checksum = sha256(&data);
            if checksum != entry.checksum {
                errors.push(format!("checksum mismatch: {}", entry.path));
            }
        }

        errors
    }
}

/// Version comparison
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub epoch: u32,
    pub version: String,
    pub release: u32,
}

impl Version {
    /// Parse version string (epoch:version-release)
    pub fn parse(s: &str) -> Self {
        let (epoch, rest) = if let Some((e, r)) = s.split_once(':') {
            (e.parse().unwrap_or(0), r)
        } else {
            (0, s)
        };

        let (version, release) = if let Some((v, r)) = rest.rsplit_once('-') {
            (String::from(v), r.parse().unwrap_or(0))
        } else {
            (String::from(rest), 0)
        };

        Version {
            epoch,
            version,
            release,
        }
    }

    /// Compare versions
    pub fn compare(&self, other: &Version) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        match self.epoch.cmp(&other.epoch) {
            Ordering::Equal => {}
            o => return o,
        }

        match compare_version_strings(&self.version, &other.version) {
            Ordering::Equal => {}
            o => return o,
        }

        self.release.cmp(&other.release)
    }
}

fn compare_version_strings(a: &str, b: &str) -> core::cmp::Ordering {
    use core::cmp::Ordering;

    let mut a_iter = a.chars().peekable();
    let mut b_iter = b.chars().peekable();

    loop {
        // Skip non-alphanumeric
        while a_iter.peek().map(|c| !c.is_alphanumeric()).unwrap_or(false) {
            a_iter.next();
        }
        while b_iter.peek().map(|c| !c.is_alphanumeric()).unwrap_or(false) {
            b_iter.next();
        }

        let a_done = a_iter.peek().is_none();
        let b_done = b_iter.peek().is_none();

        if a_done && b_done {
            return Ordering::Equal;
        }
        if a_done {
            return Ordering::Less;
        }
        if b_done {
            return Ordering::Greater;
        }

        // Compare segment (numeric or alpha)
        let a_numeric = a_iter.peek().map(|c| c.is_ascii_digit()).unwrap_or(false);
        let b_numeric = b_iter.peek().map(|c| c.is_ascii_digit()).unwrap_or(false);

        if a_numeric && b_numeric {
            // Compare as numbers
            let a_num: String = a_iter.by_ref().take_while(|c| c.is_ascii_digit()).collect();
            let b_num: String = b_iter.by_ref().take_while(|c| c.is_ascii_digit()).collect();

            let a_val: u64 = a_num.parse().unwrap_or(0);
            let b_val: u64 = b_num.parse().unwrap_or(0);

            match a_val.cmp(&b_val) {
                Ordering::Equal => continue,
                o => return o,
            }
        } else if a_numeric {
            return Ordering::Greater; // Numbers come after letters in version comparison
        } else if b_numeric {
            return Ordering::Less;
        } else {
            // Compare as strings
            let a_str: String = a_iter.by_ref().take_while(|c| c.is_alphabetic()).collect();
            let b_str: String = b_iter.by_ref().take_while(|c| c.is_alphabetic()).collect();

            match a_str.cmp(&b_str) {
                Ordering::Equal => continue,
                o => return o,
            }
        }
    }
}

/// Dependency specification
#[derive(Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub constraint: Option<VersionConstraint>,
}

#[derive(Clone, Debug)]
pub enum VersionConstraint {
    Eq(String), // =version
    Lt(String), // <version
    Le(String), // <=version
    Gt(String), // >version
    Ge(String), // >=version
}

impl Dependency {
    /// Parse dependency string
    pub fn parse(s: &str) -> Self {
        let s = s.trim();

        if let Some(idx) = s.find(['=', '<', '>']) {
            let name = String::from(s[..idx].trim());
            let rest = &s[idx..];

            let constraint = if let Some(v) = rest.strip_prefix(">=") {
                Some(VersionConstraint::Ge(String::from(v.trim())))
            } else if let Some(v) = rest.strip_prefix("<=") {
                Some(VersionConstraint::Le(String::from(v.trim())))
            } else if let Some(v) = rest.strip_prefix('>') {
                Some(VersionConstraint::Gt(String::from(v.trim())))
            } else if let Some(v) = rest.strip_prefix('<') {
                Some(VersionConstraint::Lt(String::from(v.trim())))
            } else {
                rest.strip_prefix('=')
                    .map(|v| VersionConstraint::Eq(String::from(v.trim())))
            };

            Dependency { name, constraint }
        } else {
            Dependency {
                name: String::from(s),
                constraint: None,
            }
        }
    }

    /// Check if a version satisfies this dependency
    pub fn satisfied_by(&self, version: &str) -> bool {
        match &self.constraint {
            None => true,
            Some(VersionConstraint::Eq(v)) => {
                Version::parse(version).compare(&Version::parse(v)) == core::cmp::Ordering::Equal
            }
            Some(VersionConstraint::Lt(v)) => {
                Version::parse(version).compare(&Version::parse(v)) == core::cmp::Ordering::Less
            }
            Some(VersionConstraint::Le(v)) => {
                Version::parse(version).compare(&Version::parse(v)) != core::cmp::Ordering::Greater
            }
            Some(VersionConstraint::Gt(v)) => {
                Version::parse(version).compare(&Version::parse(v)) == core::cmp::Ordering::Greater
            }
            Some(VersionConstraint::Ge(v)) => {
                Version::parse(version).compare(&Version::parse(v)) != core::cmp::Ordering::Less
            }
        }
    }
}

// Helper for alloc::format! equivalent
fn format(args: core::fmt::Arguments<'_>) -> String {
    use core::fmt::Write;
    let mut s = String::new();
    let _ = s.write_fmt(args);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compare() {
        assert!(Version::parse("1.0").compare(&Version::parse("1.1")) == core::cmp::Ordering::Less);
        assert!(
            Version::parse("2.0").compare(&Version::parse("1.9")) == core::cmp::Ordering::Greater
        );
        assert!(
            Version::parse("1.0").compare(&Version::parse("1.0")) == core::cmp::Ordering::Equal
        );
        assert!(
            Version::parse("1:1.0").compare(&Version::parse("2.0")) == core::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_dependency_parse() {
        let dep = Dependency::parse("foo>=1.0");
        assert_eq!(dep.name, "foo");
        assert!(dep.satisfied_by("1.0"));
        assert!(dep.satisfied_by("2.0"));
        assert!(!dep.satisfied_by("0.9"));
    }

    #[test]
    fn test_metadata_roundtrip() {
        let meta = PackageMetadata {
            name: String::from("test-pkg"),
            version: String::from("1.0.0"),
            description: String::from("Test package"),
            depends: vec![String::from("dep1"), String::from("dep2")],
            ..Default::default()
        };

        let toml = meta.to_toml();
        let parsed = PackageMetadata::from_toml(&toml).unwrap();

        assert_eq!(parsed.name, meta.name);
        assert_eq!(parsed.version, meta.version);
        assert_eq!(parsed.description, meta.description);
        assert_eq!(parsed.depends, meta.depends);
    }

    #[test]
    fn test_package_zstd_payload_roundtrip() {
        // Simulate a tar payload (just some data for testing)
        let payload = b"This is simulated tar payload data with enough content \
            to trigger real compression. Repetitive patterns help: \
            ABCDEFGH ABCDEFGH ABCDEFGH ABCDEFGH ABCDEFGH ABCDEFGH";

        let meta = PackageMetadata {
            name: String::from("test-zstd"),
            version: String::from("1.0.0"),
            ..Default::default()
        };

        let manifest = Manifest {
            entries: Vec::new(),
        };

        let pkg = AbpPackage {
            header: AbpHeader {
                version_major: ABP_VERSION_MAJOR,
                version_minor: ABP_VERSION_MINOR,
                flags: 0, // No flags - payload is uncompressed
                metadata_offset: 0,
                metadata_size: 0,
                manifest_offset: 0,
                manifest_size: 0,
                payload_offset: 0,
                payload_size: 0,
            },
            metadata: meta,
            manifest,
            payload: payload.to_vec(),
            signature: None,
            key_id: None,
            signed_region: None,
        };

        // Serialize - should compress the payload
        let serialized = pkg.to_bytes();

        // Parse back
        let loaded = AbpPackage::from_bytes(&serialized).unwrap();

        // Verify the COMPRESSED_PAYLOAD flag is set
        assert!(
            loaded.header.flags & flags::COMPRESSED_PAYLOAD != 0,
            "COMPRESSED_PAYLOAD flag should be set"
        );

        // Verify the compressed payload is smaller than original
        assert!(
            loaded.payload.len() < payload.len(),
            "compressed payload ({}) should be smaller than original ({})",
            loaded.payload.len(),
            payload.len()
        );

        // Decompress and verify roundtrip
        let decompressed = loaded.decompressed_payload().unwrap();
        assert_eq!(
            &decompressed,
            &payload[..],
            "decompressed payload should match original"
        );
    }

    #[test]
    fn test_package_already_compressed_not_double_compressed() {
        // Create a package with already-compressed payload
        let original_payload = b"Some data that gets compressed once";
        let compressed = zstd_nostd::compress(original_payload, 3);

        let meta = PackageMetadata {
            name: String::from("test-precompressed"),
            version: String::from("1.0.0"),
            ..Default::default()
        };

        let manifest = Manifest {
            entries: Vec::new(),
        };

        let pkg = AbpPackage {
            header: AbpHeader {
                version_major: ABP_VERSION_MAJOR,
                version_minor: ABP_VERSION_MINOR,
                flags: flags::COMPRESSED_PAYLOAD, // Already compressed
                metadata_offset: 0,
                metadata_size: 0,
                manifest_offset: 0,
                manifest_size: 0,
                payload_offset: 0,
                payload_size: 0,
            },
            metadata: meta,
            manifest,
            payload: compressed.clone(),
            signature: None,
            key_id: None,
            signed_region: None,
        };

        // Serialize
        let serialized = pkg.to_bytes();

        // Parse back
        let loaded = AbpPackage::from_bytes(&serialized).unwrap();

        // Payload should be identical (not double-compressed)
        assert_eq!(
            loaded.payload, compressed,
            "already-compressed payload should not be re-compressed"
        );

        // Decompress should give original
        let decompressed = loaded.decompressed_payload().unwrap();
        assert_eq!(&decompressed, &original_payload[..]);
    }

    // The captured signed_region must be exactly the on-disk bytes preceding
    // the signature trailer — this is what makes signature verification
    // byte-exact rather than a re-serialization of parsed structures.
    #[test]
    fn test_signed_region_is_exact_on_disk_bytes() {
        let meta = PackageMetadata {
            name: String::from("sig-region"),
            version: String::from("1.0.0"),
            ..Default::default()
        };
        let pkg = AbpPackage {
            header: AbpHeader {
                version_major: ABP_VERSION_MAJOR,
                version_minor: ABP_VERSION_MINOR,
                flags: 0,
                metadata_offset: 0,
                metadata_size: 0,
                manifest_offset: 0,
                manifest_size: 0,
                payload_offset: 0,
                payload_size: 0,
            },
            metadata: meta,
            manifest: Manifest {
                entries: Vec::new(),
            },
            payload: b"hello payload contents".to_vec(),
            signature: None,
            key_id: None,
            signed_region: None,
        };
        let serialized = pkg.to_bytes();
        let loaded = AbpPackage::from_bytes(&serialized).unwrap();

        // payload_end = payload_offset + payload_size, from the parsed header.
        let payload_end = (loaded.header.payload_offset + loaded.header.payload_size) as usize;
        let region = loaded
            .signed_region
            .as_ref()
            .expect("parsed package has a signed_region");
        assert_eq!(
            region.as_slice(),
            &serialized[..payload_end],
            "signed_region must equal the on-disk bytes before the signature trailer"
        );
    }

    // An in-memory package (never parsed from bytes) has no authenticated
    // representation, so signature verification must refuse it rather than
    // fabricating one from re-serialized fields.
    #[test]
    fn test_verify_signature_false_for_in_memory_package() {
        let meta = PackageMetadata {
            name: String::from("no-region"),
            version: String::from("1.0.0"),
            ..Default::default()
        };
        let pkg = AbpPackage {
            header: AbpHeader {
                version_major: ABP_VERSION_MAJOR,
                version_minor: ABP_VERSION_MINOR,
                flags: flags::SIGNED,
                metadata_offset: 0,
                metadata_size: 0,
                manifest_offset: 0,
                manifest_size: 0,
                payload_offset: 0,
                payload_size: 0,
            },
            metadata: meta,
            manifest: Manifest {
                entries: Vec::new(),
            },
            payload: Vec::new(),
            signature: Some([0u8; 64]),
            key_id: Some([0u8; 32]),
            signed_region: None,
        };
        assert!(!pkg.verify_signature(&[0u8; 32]));
    }
}
