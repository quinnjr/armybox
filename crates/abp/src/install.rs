//! Package Installation
//!
//! Implements the `abp add` command for installing packages.

extern crate alloc;

use crate::database::{Database, PackageRecord, current_time};
use crate::format::AbpPackage;
use crate::io;
use crate::resolver::resolve_with_local;
use alloc::string::String;
use alloc::vec::Vec;

/// Install packages
pub fn install_packages(packages: &[&[u8]], force: bool, no_deps: bool) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let mut to_install: Vec<AbpPackage> = Vec::new();

    // Load all packages
    for &pkg_path in packages {
        // Check if it's a local file or package name
        if pkg_path.contains(&b'/') || pkg_path.ends_with(b".abp") {
            // Local file
            match AbpPackage::from_file(pkg_path) {
                Some(pkg) => {
                    io::write_str(1, b"Loading ");
                    io::write_all(1, pkg_path);
                    io::write_str(1, b"... ");
                    io::write_str(1, b"OK\n");
                    to_install.push(pkg);
                }
                None => {
                    io::write_str(2, b"abp: cannot read package ");
                    io::write_all(2, pkg_path);
                    io::write_str(2, b"\n");
                    return 1;
                }
            }
        } else {
            // Package name - look in available packages
            let name = match core::str::from_utf8(pkg_path) {
                Ok(s) => s,
                Err(_) => {
                    io::write_str(2, b"abp: invalid package name\n");
                    return 1;
                }
            };

            if let Some(avail) = db.get_available(name) {
                io::write_str(1, b"Package '");
                io::write_all(1, pkg_path);
                io::write_str(1, b"' found in repository '");
                io::write_all(1, avail.repo.as_bytes());
                io::write_str(1, b"'\n");

                // TODO: Download package from repository
                io::write_str(
                    2,
                    b"abp: downloading from repositories not yet implemented\n",
                );
                io::write_str(2, b"abp: please specify a local .abp file\n");
                return 1;
            } else {
                io::write_str(2, b"abp: package '");
                io::write_all(2, pkg_path);
                io::write_str(2, b"' not found\n");
                return 1;
            }
        }
    }

    if to_install.is_empty() {
        io::write_str(2, b"abp: no packages to install\n");
        return 1;
    }

    // Resolve dependencies
    let install_order = if no_deps {
        to_install.iter().map(|p| p.metadata.name.clone()).collect()
    } else {
        let pkg_names: Vec<String> = to_install.iter().map(|p| p.metadata.name.clone()).collect();

        // Build a map of local packages being installed (for the resolver)
        let mut local_deps: Vec<(String, Vec<String>)> = Vec::new();
        for pkg in &to_install {
            local_deps.push((pkg.metadata.name.clone(), pkg.metadata.depends.clone()));
        }

        match resolve_with_local(&db, &pkg_names, &local_deps) {
            Ok(order) => order,
            Err(e) => {
                io::write_str(2, b"abp: dependency resolution failed: ");
                io::write_all(2, e.as_bytes());
                io::write_str(2, b"\n");
                return 1;
            }
        }
    };

    // Handle `replaces`: a package that replaces an installed one takes it
    // over (dpkg/apk-style), so remove the replaced package first. Kept simple
    // and documented: the replaced package's files are unlinked and its DB
    // record deleted before the replacing package is extracted. A package that
    // is itself part of this transaction is not force-removed here.
    for pkg in &to_install {
        for rep in &pkg.metadata.replaces {
            if db.is_installed(rep) && !to_install.iter().any(|p| &p.metadata.name == rep) {
                io::write_str(1, b"Replacing '");
                io::write_all(1, rep.as_bytes());
                io::write_str(1, b"' (superseded by '");
                io::write_all(1, pkg.metadata.name.as_bytes());
                io::write_str(1, b"')\n");
                crate::remove::remove_single_package(&db, rep, false);
            }
        }
    }

    // Detect INTRA-transaction file collisions: two packages in THIS run that
    // ship the same path. `db.get_file_owner` only sees already-registered
    // files, so this catches the case both packages are new. Always fatal
    // (even with --force): two packages cannot both own one path.
    let entries_by_pkg: Vec<(String, Vec<String>)> = to_install
        .iter()
        .map(|p| {
            (
                p.metadata.name.clone(),
                p.manifest.entries.iter().map(|e| e.path.clone()).collect(),
            )
        })
        .collect();
    if let Some((path, a, b)) = intra_transaction_collision(&entries_by_pkg) {
        io::write_str(2, b"abp: file collision: ");
        io::write_all(2, path.as_bytes());
        io::write_str(2, b" is shipped by both ");
        io::write_all(2, a.as_bytes());
        io::write_str(2, b" and ");
        io::write_all(2, b.as_bytes());
        io::write_str(2, b"\n");
        return 1;
    }

    // Enforce declared `conflicts` against the installed system (name or
    // `provides` match), unless --force is given.
    if !force {
        let installed_snapshot: Vec<(String, Vec<String>)> = db
            .list_packages()
            .iter()
            .map(|p| (p.name.clone(), p.provides.clone()))
            .collect();
        for pkg in &to_install {
            if let Some((cname, _owner)) =
                find_active_conflict(&pkg.metadata.conflicts, &installed_snapshot)
            {
                io::write_str(2, b"abp: ");
                io::write_all(2, pkg.metadata.name.as_bytes());
                io::write_str(2, b" conflicts with installed ");
                io::write_all(2, cname.as_bytes());
                io::write_str(2, b"\n");
                io::write_str(2, b"abp: use --force to override\n");
                return 1;
            }
        }
    }

    // Check for conflicts
    if !force {
        for pkg in &to_install {
            // Check if already installed
            if db.is_installed(&pkg.metadata.name) {
                let installed = db.get_package(&pkg.metadata.name).unwrap();
                if installed.version == pkg.metadata.version {
                    io::write_str(1, b"Package '");
                    io::write_all(1, pkg.metadata.name.as_bytes());
                    io::write_str(1, b"' is already installed\n");
                    continue;
                }
                io::write_str(1, b"Upgrading ");
                io::write_all(1, pkg.metadata.name.as_bytes());
                io::write_str(1, b" from ");
                io::write_all(1, installed.version.as_bytes());
                io::write_str(1, b" to ");
                io::write_all(1, pkg.metadata.version.as_bytes());
                io::write_str(1, b"\n");
            }

            // Check file conflicts
            for entry in &pkg.manifest.entries {
                if let Some(owner) = db.get_file_owner(&entry.path) {
                    if owner != pkg.metadata.name {
                        io::write_str(2, b"abp: file conflict: ");
                        io::write_all(2, entry.path.as_bytes());
                        io::write_str(2, b" is owned by ");
                        io::write_all(2, owner.as_bytes());
                        io::write_str(2, b"\n");
                        io::write_str(2, b"abp: use --force to override\n");
                        return 1;
                    }
                }
            }
        }
    }

    // Show transaction summary
    io::write_str(1, b"\nPackages to install:\n");
    for pkg in &to_install {
        io::write_str(1, b"  ");
        io::write_all(1, pkg.metadata.name.as_bytes());
        io::write_str(1, b"-");
        io::write_all(1, pkg.metadata.version.as_bytes());
        if db.is_installed(&pkg.metadata.name) {
            io::write_str(1, b" [upgrade]");
        }
        io::write_str(1, b"\n");
    }
    io::write_str(1, b"\n");

    // Order the loaded packages according to the resolved dependency order
    // (dependencies first) rather than command-line order. Packages named in
    // the resolved order that are not among the locally loaded packages (e.g.
    // dependencies that would be downloaded) are skipped here.
    let mut ordered: Vec<&AbpPackage> = Vec::new();
    for name in &install_order {
        if let Some(pkg) = to_install.iter().find(|p| &p.metadata.name == name) {
            if !ordered.iter().any(|p| p.metadata.name == pkg.metadata.name) {
                ordered.push(pkg);
            }
        }
    }
    // Safety net: append any loaded package not covered by the resolved order.
    for pkg in &to_install {
        if !ordered.iter().any(|p| p.metadata.name == pkg.metadata.name) {
            ordered.push(pkg);
        }
    }

    // Load trusted keys once for signature verification.
    let trusted_keys = crate::verify::load_trusted_keys();

    // Execute installation
    let mut success = true;
    for pkg in ordered {
        // Verify the package (signature + integrity) before extracting.
        // Packages that fail verification are skipped and mark the whole
        // transaction as failed, but do not abort remaining packages.
        if let Err(e) = crate::verify::verify_package(pkg, &trusted_keys) {
            io::write_str(2, b"abp: verification failed for ");
            io::write_all(2, pkg.metadata.name.as_bytes());
            io::write_str(2, b": ");
            io::write_all(2, e.as_bytes());
            io::write_str(2, b"\n");
            io::write_str(2, b"abp: skipping unverified package\n");
            success = false;
            continue;
        }

        if !install_single_package(&db, pkg, force) {
            success = false;
            break;
        }
    }

    if success {
        io::write_str(1, b"\nInstallation complete.\n");
        0
    } else {
        io::write_str(2, b"\nInstallation failed.\n");
        1
    }
}

/// Detect a file path shipped by two DIFFERENT packages in the same install
/// run. `entries` is (package_name, file_paths) for every package to install.
/// Returns `(path, first_owner, second_owner)` for the first collision found.
pub(crate) fn intra_transaction_collision(
    entries: &[(String, Vec<String>)],
) -> Option<(String, String, String)> {
    let mut seen: alloc::collections::BTreeMap<String, String> =
        alloc::collections::BTreeMap::new();
    for (name, paths) in entries {
        for p in paths {
            match seen.get(p) {
                Some(prev) if prev != name => {
                    return Some((p.clone(), prev.clone(), name.clone()));
                }
                Some(_) => {}
                None => {
                    seen.insert(p.clone(), name.clone());
                }
            }
        }
    }
    None
}

/// Given a package's declared `conflicts` and a snapshot of installed packages
/// as `(name, provides)` pairs, return `(conflict_name, owning_installed_pkg)`
/// for the first conflict satisfied by an installed package — matched either by
/// exact name or by a `provides` entry (with any `=version` suffix ignored).
/// Returns `None` when no declared conflict is currently active.
pub(crate) fn find_active_conflict(
    conflicts: &[String],
    installed: &[(String, Vec<String>)],
) -> Option<(String, String)> {
    for c in conflicts {
        let cname = c.as_str();
        for (name, provides) in installed {
            if name == cname {
                return Some((c.clone(), name.clone()));
            }
            for prov in provides {
                let pname = prov.split('=').next().unwrap_or(prov);
                if pname == cname {
                    return Some((c.clone(), name.clone()));
                }
            }
        }
    }
    None
}

/// Install a single package
fn install_single_package(db: &Database, pkg: &AbpPackage, _force: bool) -> bool {
    io::write_str(1, b"Installing ");
    io::write_all(1, pkg.metadata.name.as_bytes());
    io::write_str(1, b"-");
    io::write_all(1, pkg.metadata.version.as_bytes());
    io::write_str(1, b"...\n");

    // If upgrading, remove old files first
    if let Some(old_pkg) = db.get_package(&pkg.metadata.name) {
        io::write_str(1, b"  Removing old version...\n");
        for file in &old_pkg.files {
            let path = file.as_bytes();
            io::unlink(path);
        }
        db.unregister_files(&pkg.metadata.name);
    }

    // Extract payload (tar archive, possibly zstd-compressed)
    io::write_str(1, b"  Extracting files...\n");

    let payload_data = match pkg.decompressed_payload() {
        Some(data) => data,
        None => {
            io::write_str(2, b"abp: failed to decompress payload\n");
            return false;
        }
    };

    // Track every path created during extraction so ANY later failure (bad
    // member, write error, DB failure) can be fully undone, leaving the system
    // as it was before this package was touched.
    let mut rb = Rollback::new();

    let files_extracted = extract_payload(&payload_data, b"/", &pkg.manifest, &mut rb);
    if !files_extracted {
        io::write_str(2, b"abp: failed to extract package\n");
        rb.undo();
        return false;
    }

    // Register files in database. A failed registration leaves the on-disk
    // state and the database out of sync, so treat it as an install failure and
    // roll back both the extracted files and any partial DB registrations.
    for entry in &pkg.manifest.entries {
        if !db.register_file(&entry.path, &pkg.metadata.name) {
            io::write_str(2, b"abp: failed to register file: ");
            io::write_all(2, entry.path.as_bytes());
            io::write_str(2, b"\n");
            db.unregister_files(&pkg.metadata.name);
            rb.undo();
            return false;
        }
    }

    // Create package record
    let record = PackageRecord::from_metadata(&pkg.metadata, &pkg.manifest, current_time());
    if !db.put_package(&record) {
        io::write_str(2, b"abp: failed to register package\n");
        db.unregister_files(&pkg.metadata.name);
        rb.undo();
        return false;
    }

    io::write_str(1, b"  Done.\n");
    true
}

/// Records paths created during an extraction so a failed install can be
/// rolled back to leave the system exactly as it was found.
///
/// Files and symlinks are unlinked; directories that this extraction actually
/// created (as opposed to pre-existing ones) are `rmdir`'d. Undo walks both
/// lists in reverse creation order so nested paths are removed before their
/// parents.
pub(crate) struct Rollback {
    /// Files and symlinks created (to be unlinked on rollback).
    files: Vec<Vec<u8>>,
    /// Directories freshly created by this extraction (to be rmdir'd).
    dirs: Vec<Vec<u8>>,
}

impl Rollback {
    pub(crate) fn new() -> Self {
        Rollback {
            files: Vec::new(),
            dirs: Vec::new(),
        }
    }

    /// Undo everything recorded, best-effort (errors are ignored: the goal is
    /// to leave behind as little as possible, and a path already gone is fine).
    pub(crate) fn undo(&self) {
        for f in self.files.iter().rev() {
            io::unlink(f);
        }
        for d in self.dirs.iter().rev() {
            io::rmdir(d);
        }
    }
}

/// Extract tar payload to root directory, recording created paths in `rb` for
/// rollback on failure.
///
/// The payload should already be decompressed (zstd decompression happens in
/// the caller). The tar stream is walked by the shared [`TarWalker`], so this
/// and the integrity walker in `verify.rs` interpret GNU long names, pax
/// extended headers and base-256 sizes identically and can never diverge.
pub(crate) fn extract_payload(
    payload: &[u8],
    root: &[u8],
    _manifest: &super::format::Manifest,
    rb: &mut Rollback,
) -> bool {
    let mut success = true;
    let mut walker = TarWalker::new(payload);

    loop {
        let entry = match walker.next() {
            None => break,
            Some(Ok(e)) => e,
            Some(Err(_)) => {
                io::write_str(2, b"abp: malformed tar payload\n");
                success = false;
                break;
            }
        };

        let name = entry.name;
        if name.is_empty() {
            continue;
        }

        let typeflag = entry.typeflag;
        // Strip the setuid/setgid/sticky bits (S_ISUID|S_ISGID|S_ISVTX =
        // 0o7000) from the archived mode before it reaches open()/mkdir().
        // Honoring them would let a package drop a root-owned setuid binary on
        // the system. Only the permission bits are trusted.
        let mode = entry.mode & !0o7000;

        // Reject path-traversal attempts: absolute member names or names that
        // contain a ".." path component would let a package write outside the
        // installation root. Skip the member and record failure.
        if is_unsafe_member_name(&name) {
            io::write_str(2, b"abp: refusing unsafe path in package: ");
            io::write_all(2, &name);
            io::write_str(2, b"\n");
            success = false;
            continue;
        }

        // Build full path
        let mut full_path = root.to_vec();
        if !full_path.ends_with(b"/") {
            full_path.push(b'/');
        }
        full_path.extend_from_slice(&name);

        let is_dir = typeflag == b'5' || (typeflag == 0 && name.ends_with(b"/"));

        if is_dir {
            if !mkdir_tracked(&full_path, mode, rb) {
                io::write_str(2, b"abp: failed to create directory: ");
                io::write_all(2, &full_path);
                io::write_str(2, b"\n");
                success = false;
            }
        } else if typeflag == b'0' || typeflag == 0 {
            // Regular file
            if !create_parent_dirs(&full_path, rb) {
                io::write_str(2, b"abp: failed to create parent directories for: ");
                io::write_all(2, &full_path);
                io::write_str(2, b"\n");
                success = false;
            }

            // O_NOFOLLOW refuses to open through an existing symlink at the
            // final path component: if a hostile earlier member planted a
            // symlink here, the open fails (ELOOP) instead of writing
            // through it and escaping the install root.
            let fd = io::open(
                &full_path,
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW,
                mode,
            );
            if fd >= 0 {
                // Record the file for rollback as soon as it exists on disk.
                rb.files.push(full_path.clone());
                let data_start = entry.data_offset;
                let data_end = data_start + entry.size as usize;
                if data_end <= payload.len() {
                    let expected = (data_end - data_start) as isize;
                    let written = io::write_all(fd, &payload[data_start..data_end]);
                    if written != expected {
                        io::write_str(2, b"abp: failed to write file: ");
                        io::write_all(2, &full_path);
                        io::write_str(2, b"\n");
                        success = false;
                    }
                } else {
                    // Payload is truncated: the header claims more data
                    // than the archive actually contains.
                    io::write_str(2, b"abp: truncated payload for: ");
                    io::write_all(2, &full_path);
                    io::write_str(2, b"\n");
                    success = false;
                }
                io::close(fd);
            } else {
                // A failure here includes ELOOP from O_NOFOLLOW (an existing
                // symlink at this path). Treat any open failure as a hard
                // error rather than silently succeeding.
                io::write_str(2, b"abp: failed to create file: ");
                io::write_all(2, &full_path);
                io::write_str(2, b"\n");
                success = false;
            }
        } else if typeflag == b'2' {
            // Symlink. Validate the TARGET (linkname) with the same rules as
            // member names: an absolute target or one containing a ".."
            // component could point the link outside the install root, so a
            // later write through it (e.g. `x -> /etc`, then `x/cron.d/...`)
            // would escape as root. Reject such symlinks entirely.
            let linkname = &entry.linkname;
            if is_unsafe_member_name(linkname) {
                io::write_str(2, b"abp: refusing unsafe symlink target in package: ");
                io::write_all(2, linkname);
                io::write_str(2, b"\n");
                success = false;
            } else if !linkname.is_empty() {
                if !create_parent_dirs(&full_path, rb) {
                    io::write_str(2, b"abp: failed to create parent directories for: ");
                    io::write_all(2, &full_path);
                    io::write_str(2, b"\n");
                    success = false;
                }
                if io::symlink(linkname, &full_path) == 0 {
                    rb.files.push(full_path.clone());
                } else {
                    io::write_str(2, b"abp: failed to create symlink: ");
                    io::write_all(2, &full_path);
                    io::write_str(2, b"\n");
                    success = false;
                }
            }
        } else {
            // Unknown/unsupported member type: nothing to create.
        }
    }

    success
}

/// A single decoded tar member, with GNU long-name / pax overrides already
/// applied and `data_offset` pointing at its payload bytes.
pub(crate) struct TarEntry {
    pub name: Vec<u8>,
    pub typeflag: u8,
    pub size: u64,
    pub mode: u32,
    pub linkname: Vec<u8>,
    /// Absolute offset into the payload where this member's data begins.
    pub data_offset: usize,
}

/// A strict, shared iterator over tar members.
///
/// It transparently consumes and applies the metadata entry types both the
/// installer and the verifier must agree on:
/// - **`L`** (GNU `LongName`): its data supplies the following entry's name.
/// - **`K`** (GNU `LongLink`): its data supplies the following entry's linkname.
/// - **`x`** (pax extended header): `path=` / `linkpath=` / `size=` records
///   override the following entry.
/// - **`g`** (pax global header): treated like `x` for the following entry
///   only (a documented simplification — global persistence across the whole
///   archive is not needed by ABP packages, which are produced by our own
///   `makepkg`).
///
/// Sizes are decoded as octal or GNU base-256 (high-bit) so members larger than
/// 8 GiB are handled. `next()` yields `None` at end-of-archive (a zero block or
/// a short trailing block) and `Some(Err(_))` on a malformed/truncated meta
/// entry.
pub(crate) struct TarWalker<'a> {
    payload: &'a [u8],
    pos: usize,
}

impl<'a> TarWalker<'a> {
    pub(crate) fn new(payload: &'a [u8]) -> Self {
        TarWalker { payload, pos: 0 }
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn next(&mut self) -> Option<Result<TarEntry, alloc::string::String>> {
        use alloc::string::String;

        // Overrides accumulated from preceding L / K / x / g meta entries.
        let mut pending_name: Option<Vec<u8>> = None;
        let mut pending_link: Option<Vec<u8>> = None;
        let mut pending_size: Option<u64> = None;

        loop {
            if self.pos + 512 > self.payload.len() {
                return None; // no room for another header => end of stream
            }
            let header = &self.payload[self.pos..self.pos + 512];

            // End of archive: an all-zero block.
            if header.iter().all(|&b| b == 0) {
                return None;
            }

            let hdr_size = parse_tar_size(&header[124..136]);
            let typeflag = header[156];
            let data_off = self.pos + 512;

            match typeflag {
                b'L' | b'K' => {
                    // GNU long name / long linkname: the member data is the
                    // NUL-padded path for the following entry.
                    let end = data_off.checked_add(hdr_size as usize)?;
                    if end > self.payload.len() {
                        return Some(Err(String::from("truncated GNU long-name entry")));
                    }
                    let raw = strip_trailing_nul(&self.payload[data_off..end]);
                    if typeflag == b'L' {
                        pending_name = Some(raw);
                    } else {
                        pending_link = Some(raw);
                    }
                    self.pos = data_off + block_round(hdr_size);
                    continue;
                }
                b'x' | b'g' => {
                    // pax extended header: parse "len key=value\n" records.
                    let end = data_off.checked_add(hdr_size as usize)?;
                    if end > self.payload.len() {
                        return Some(Err(String::from("truncated pax header")));
                    }
                    parse_pax_records(
                        &self.payload[data_off..end],
                        &mut pending_name,
                        &mut pending_link,
                        &mut pending_size,
                    );
                    self.pos = data_off + block_round(hdr_size);
                    continue;
                }
                _ => {
                    // A real member. Apply any pending overrides.
                    let name = pending_name
                        .take()
                        .unwrap_or_else(|| parse_tar_name(header));
                    let linkname = pending_link
                        .take()
                        .unwrap_or_else(|| parse_tar_name(&header[157..257]));
                    // A pax size= override replaces the header size and also
                    // determines how many data blocks the member occupies.
                    let size = pending_size.take().unwrap_or(hdr_size);
                    let mode = parse_tar_mode(&header[100..108]);

                    self.pos = data_off + block_round(size);
                    return Some(Ok(TarEntry {
                        name,
                        typeflag,
                        size,
                        mode,
                        linkname,
                        data_offset: data_off,
                    }));
                }
            }
        }
    }
}

/// Number of bytes a member of `size` data occupies in the archive, rounded up
/// to a whole 512-byte block.
fn block_round(size: u64) -> usize {
    (size.div_ceil(512) * 512) as usize
}

/// Strip trailing NUL bytes from a long-name / pax path value.
fn strip_trailing_nul(data: &[u8]) -> Vec<u8> {
    let mut end = data.len();
    while end > 0 && data[end - 1] == 0 {
        end -= 1;
    }
    data[..end].to_vec()
}

/// Parse pax extended-header records of the form `"<len> <key>=<value>\n"` and
/// apply the ones ABP honors (`path`, `linkpath`, `size`) as overrides for the
/// next real entry. Unknown keys and malformed records are ignored.
fn parse_pax_records(
    mut data: &[u8],
    pending_name: &mut Option<Vec<u8>>,
    pending_link: &mut Option<Vec<u8>>,
    pending_size: &mut Option<u64>,
) {
    while !data.is_empty() {
        // "<len> " prefix: decimal record length (including the length digits,
        // the space, the key=value, and the trailing newline).
        let sp = match data.iter().position(|&b| b == b' ') {
            Some(i) => i,
            None => break,
        };
        let mut len = 0usize;
        let mut ok = sp > 0;
        for &b in &data[..sp] {
            if b.is_ascii_digit() {
                len = len * 10 + (b - b'0') as usize;
            } else {
                ok = false;
                break;
            }
        }
        if !ok || len == 0 || len > data.len() {
            break;
        }
        // The record body is bytes [sp+1 .. len-1] (excluding trailing '\n').
        let record = &data[sp + 1..len.saturating_sub(1).max(sp + 1)];
        if let Some(eq) = record.iter().position(|&b| b == b'=') {
            let key = &record[..eq];
            let value = &record[eq + 1..];
            match key {
                b"path" => *pending_name = Some(value.to_vec()),
                b"linkpath" => *pending_link = Some(value.to_vec()),
                b"size" => {
                    let mut n = 0u64;
                    let mut good = !value.is_empty();
                    for &b in value {
                        if b.is_ascii_digit() {
                            n = n * 10 + (b - b'0') as u64;
                        } else {
                            good = false;
                            break;
                        }
                    }
                    if good {
                        *pending_size = Some(n);
                    }
                }
                _ => {}
            }
        }
        data = &data[len..];
    }
}

/// Return true if a tar member name is unsafe to extract: an absolute path
/// (leading `/`) or one containing a `..` path component. Such names could be
/// used to escape the installation root (path traversal).
fn is_unsafe_member_name(name: &[u8]) -> bool {
    if name.first() == Some(&b'/') {
        return true;
    }
    for component in name.split(|&b| b == b'/') {
        if component == b".." {
            return true;
        }
    }
    false
}

pub(crate) fn parse_tar_name(header: &[u8]) -> Vec<u8> {
    let end = header.iter().position(|&b| b == 0).unwrap_or(header.len());
    header[..end].to_vec()
}

/// Parse a tar size field. Handles both classic octal ASCII and GNU base-256
/// encoding (used for sizes that do not fit the 11 octal digits, i.e. >= 8 GiB):
/// when the field's top bit is set, the value is a big-endian integer in the
/// remaining bytes with the high bit masked off the first byte.
pub(crate) fn parse_tar_size(field: &[u8]) -> u64 {
    if field.is_empty() {
        return 0;
    }

    // GNU base-256: high bit of the first byte is the sign/format flag.
    if field[0] & 0x80 != 0 {
        let mut result = (field[0] & 0x7f) as u64;
        for &b in &field[1..] {
            result = (result << 8) | b as u64;
        }
        return result;
    }

    // Classic octal ASCII (space/NUL terminated).
    let mut result = 0u64;
    for &b in field {
        if b == 0 || b == b' ' {
            break;
        }
        if (b'0'..=b'7').contains(&b) {
            result = result * 8 + (b - b'0') as u64;
        }
    }
    result
}

fn parse_tar_mode(field: &[u8]) -> u32 {
    let mut result = 0u32;
    for &b in field {
        if b == 0 || b == b' ' {
            break;
        }
        if (b'0'..=b'7').contains(&b) {
            result = result * 8 + (b - b'0') as u32;
        }
    }
    result
}

/// Create a directory, treating an already-existing directory as success.
/// Returns `false` only for a genuine failure (a non-EEXIST error), so callers
/// can propagate real problems without tripping over pre-existing directories.
///
/// A directory that was freshly created (mkdir returned 0) is recorded in `rb`
/// so a failed install can `rmdir` it during rollback; pre-existing directories
/// (EEXIST) are left untouched.
fn mkdir_tracked(path: &[u8], mode: u32, rb: &mut Rollback) -> bool {
    if io::mkdir(path, mode) == 0 {
        rb.dirs.push(path.to_vec());
        return true;
    }
    io::errno() == libc::EEXIST
}

/// Create every parent directory of `path`, recording freshly-created ones in
/// `rb`. Returns `false` if any component could not be created (ignoring
/// already-existing directories), so the caller can mark the extraction as
/// failed instead of writing into a broken tree.
fn create_parent_dirs(path: &[u8], rb: &mut Rollback) -> bool {
    let mut ok = true;
    for i in 0..path.len() {
        if path[i] == b'/' && i > 0 {
            let parent = &path[..i];
            if !mkdir_tracked(parent, 0o755, rb) {
                ok = false;
            }
        }
    }
    ok
}

/// Upgrade all installed packages
pub fn upgrade_all(dry_run: bool) -> i32 {
    let db = match Database::open() {
        Some(db) => db,
        None => {
            io::write_str(2, b"abp: cannot open database\n");
            return 1;
        }
    };

    let installed = db.list_packages();
    let mut upgradeable = Vec::new();

    for pkg in &installed {
        if let Some(avail) = db.get_available(&pkg.name) {
            use crate::format::Version;
            let installed_ver = Version::parse(&pkg.version);
            let available_ver = Version::parse(&avail.version);

            if available_ver.compare(&installed_ver) == core::cmp::Ordering::Greater {
                upgradeable.push((pkg.clone(), avail));
            }
        }
    }

    if upgradeable.is_empty() {
        io::write_str(1, b"All packages are up to date.\n");
        return 0;
    }

    io::write_str(1, b"Packages to upgrade:\n");
    for (pkg, avail) in &upgradeable {
        io::write_str(1, b"  ");
        io::write_all(1, pkg.name.as_bytes());
        io::write_str(1, b" ");
        io::write_all(1, pkg.version.as_bytes());
        io::write_str(1, b" -> ");
        io::write_all(1, avail.version.as_bytes());
        io::write_str(1, b"\n");
    }

    if dry_run {
        io::write_str(1, b"\n(dry run - no changes made)\n");
        return 0;
    }

    io::write_str(2, b"\nabp: upgrade from repositories not yet implemented\n");
    io::write_str(
        2,
        b"abp: please use 'abp add <package.abp>' to upgrade manually\n",
    );
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    // ---- tar payload construction helpers (mirror tests/security.rs) --------

    fn octal(mut n: u64) -> Vec<u8> {
        if n == 0 {
            return vec![b'0'];
        }
        let mut d = Vec::new();
        while n > 0 {
            d.push(b'0' + (n & 7) as u8);
            n >>= 3;
        }
        d.reverse();
        d
    }

    /// Build a 512-byte header + block-padded data for a member. `name` is
    /// written truncated into the 100-byte name field; `typeflag` selects the
    /// member type. For base-256 sizes, pass `base256_size = Some(n)`.
    fn tar_block(name: &[u8], data: &[u8], typeflag: u8, base256_size: Option<u64>) -> Vec<u8> {
        let mut block = vec![0u8; 512];
        let nlen = core::cmp::min(name.len(), 99);
        block[..nlen].copy_from_slice(&name[..nlen]);
        let mode = b"0000644";
        block[100..100 + mode.len()].copy_from_slice(mode);

        match base256_size {
            Some(n) => {
                // GNU base-256: high bit set, big-endian in the low bytes.
                let mut field = [0u8; 12];
                field[0] = 0x80;
                let mut v = n;
                for i in (0..12).rev() {
                    field[i] |= (v & 0xff) as u8;
                    v >>= 8;
                    if i == 1 {
                        break;
                    }
                }
                block[124..136].copy_from_slice(&field);
            }
            None => {
                let size = octal(data.len() as u64);
                block[124..124 + size.len()].copy_from_slice(&size);
            }
        }
        block[156] = typeflag;

        let mut out = block;
        out.extend_from_slice(data);
        let pad = (512 - (data.len() % 512)) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
        out
    }

    fn regular(name: &str, data: &[u8]) -> Vec<u8> {
        tar_block(name.as_bytes(), data, b'0', None)
    }

    // ---- unique temp dir under the system temp directory -------------------

    fn temp_root(tag: &str) -> Vec<u8> {
        let base = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = base.join(std::format!("abp-test-{}-{}-{}", tag, pid, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir.into_os_string().into_encoded_bytes()
    }

    fn path_in(root: &[u8], rel: &str) -> Vec<u8> {
        let mut p = root.to_vec();
        p.push(b'/');
        p.extend_from_slice(rel.as_bytes());
        p
    }

    fn exists(path: &[u8]) -> bool {
        io::access(path, libc::F_OK) == 0
    }

    // ------------------------------------------------------------------------
    // Item 2: GNU long name, pax overrides, base-256 sizes
    // ------------------------------------------------------------------------

    #[test]
    fn test_parse_tar_size_base256() {
        // 10 GiB does not fit 11 octal digits; encode as base-256.
        let n: u64 = 10 * 1024 * 1024 * 1024;
        let mut field = [0u8; 12];
        field[0] = 0x80;
        let mut v = n;
        for i in (1..12).rev() {
            field[i] = (v & 0xff) as u8;
            v >>= 8;
        }
        assert_eq!(parse_tar_size(&field), n);
        // Octal still works.
        assert_eq!(parse_tar_size(b"0000010\0\0\0\0"), 8);
    }

    #[test]
    fn test_walker_gnu_longname() {
        // A GNU 'L' entry carries a >100-byte name for the following file.
        let long = "usr/share/very/deeply/nested/path/that/is/way/way/way/longer/than/exactly/one/hundred/and/twenty/eight/bytes/final_file.txt";
        assert!(long.len() > 100);
        let mut payload = Vec::new();
        payload.extend_from_slice(&tar_block(b"././@LongLink", long.as_bytes(), b'L', None));
        // The following regular file's own name field is truncated garbage.
        payload.extend_from_slice(&regular("SHORTNAME", b"contents"));

        let mut w = TarWalker::new(&payload);
        let e = w.next().unwrap().unwrap();
        assert_eq!(e.name, long.as_bytes());
        assert_eq!(e.typeflag, b'0');
        assert!(w.next().is_none());
    }

    #[test]
    fn test_walker_pax_path_and_size_override() {
        // pax 'x' header overrides path= and size= for the next entry.
        let pax = b"29 path=usr/bin/paxname\n" as &[u8]; // "29 " + record
        // Build the pax record with a correct length prefix.
        fn pax_record(kv: &str) -> Vec<u8> {
            // length includes the length digits, space, kv, and '\n'
            let body_len = kv.len() + 1; // kv + '\n'
            let mut len = body_len + 2; // assume 1-2 digit prefix + space
            // iterate to a fixed point on digit count
            loop {
                let digits = std::format!("{}", len).len();
                let candidate = body_len + digits + 1; // + space
                if candidate == len {
                    break;
                }
                len = candidate;
            }
            let mut out = std::format!("{} ", len).into_bytes();
            out.extend_from_slice(kv.as_bytes());
            out.push(b'\n');
            out
        }
        let _ = pax;
        let mut records = pax_record("path=usr/bin/paxname");
        records.extend_from_slice(&pax_record("size=5"));

        let mut payload = Vec::new();
        payload.extend_from_slice(&tar_block(b"PaxHeaders/x", &records, b'x', None));
        // The real entry: header name/size are placeholders overridden by pax.
        payload.extend_from_slice(&regular("ORIGINAL", b"hello"));

        let mut w = TarWalker::new(&payload);
        let e = w.next().unwrap().unwrap();
        assert_eq!(e.name, b"usr/bin/paxname");
        assert_eq!(e.size, 5);
        assert_eq!(&payload[e.data_offset..e.data_offset + 5], b"hello");
        assert!(w.next().is_none());
    }

    #[test]
    fn test_extract_honors_longname() {
        let long = "usr/share/very/deeply/nested/path/that/is/way/way/way/longer/than/exactly/one/hundred/and/twenty/eight/bytes/final_file.txt";
        let mut payload = Vec::new();
        payload.extend_from_slice(&tar_block(b"././@LongLink", long.as_bytes(), b'L', None));
        payload.extend_from_slice(&regular("SHORTNAME", b"data123"));

        let root = temp_root("longname");
        let mut rb = Rollback::new();
        let manifest = super::super::format::Manifest {
            entries: Vec::new(),
        };
        assert!(extract_payload(&payload, &root, &manifest, &mut rb));
        assert!(exists(&path_in(&root, long)));
        rb.undo();
        std::fs::remove_dir_all(std::path::Path::new(core::str::from_utf8(&root).unwrap())).ok();
    }

    // ------------------------------------------------------------------------
    // Item 3: atomic extraction / rollback
    // ------------------------------------------------------------------------

    #[test]
    fn test_rollback_removes_all_created_files_on_midway_failure() {
        // Members: a good file, then a file whose parent is a regular file
        // ("blocker"), forcing the open of "blocker/inner" to fail (ENOTDIR).
        let mut payload = Vec::new();
        payload.extend_from_slice(&regular("good/one.txt", b"first"));
        payload.extend_from_slice(&regular("blocker", b"iam a file"));
        payload.extend_from_slice(&regular("blocker/inner", b"cannot create"));
        payload.extend_from_slice(&regular("good/two.txt", b"third"));

        let root = temp_root("rollback");
        let mut rb = Rollback::new();
        let manifest = super::super::format::Manifest {
            entries: Vec::new(),
        };

        let ok = extract_payload(&payload, &root, &manifest, &mut rb);
        assert!(!ok, "extraction must report failure on the bad member");

        // Simulate install_single_package's failure path.
        rb.undo();

        // NO files created by this extraction may remain.
        assert!(!exists(&path_in(&root, "good/one.txt")));
        assert!(!exists(&path_in(&root, "blocker")));
        assert!(!exists(&path_in(&root, "good/two.txt")));
        // Directories created should be gone too.
        assert!(!exists(&path_in(&root, "good")));

        std::fs::remove_dir_all(std::path::Path::new(core::str::from_utf8(&root).unwrap())).ok();
    }

    // ------------------------------------------------------------------------
    // Item 1: conflicts / replaces (pure helpers)
    // ------------------------------------------------------------------------

    #[test]
    fn test_intra_transaction_collision_detected() {
        let entries = vec![
            (
                String::from("pkg-a"),
                vec![String::from("/usr/bin/x"), String::from("/usr/bin/a")],
            ),
            (
                String::from("pkg-b"),
                vec![String::from("/usr/bin/b"), String::from("/usr/bin/x")],
            ),
        ];
        let hit = intra_transaction_collision(&entries).expect("collision expected");
        assert_eq!(hit.0, "/usr/bin/x");
        // Same package listing a path twice is NOT a collision.
        let same = vec![(
            String::from("p"),
            vec![String::from("/a"), String::from("/a")],
        )];
        assert!(intra_transaction_collision(&same).is_none());
    }

    #[test]
    fn test_find_active_conflict_by_name_and_provides() {
        let installed = vec![
            (String::from("foo"), vec![String::from("libfoo=1.2")]),
            (String::from("bar"), Vec::new()),
        ];
        // Direct name conflict.
        let c = find_active_conflict(&[String::from("foo")], &installed).unwrap();
        assert_eq!(c.0, "foo");
        assert_eq!(c.1, "foo");
        // Provides-based conflict: conflicting with "libfoo" hits pkg "foo".
        let c = find_active_conflict(&[String::from("libfoo")], &installed).unwrap();
        assert_eq!(c.0, "libfoo");
        assert_eq!(c.1, "foo");
        // No conflict.
        assert!(find_active_conflict(&[String::from("baz")], &installed).is_none());
    }
}
