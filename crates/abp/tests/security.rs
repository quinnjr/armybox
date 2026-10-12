//! End-to-end SECURITY tests for the ABP verify/install pipeline.
//!
//! These integration tests exercise the *public* verification entry point
//! [`abp::verify::verify_package`] against realistically-shaped, ed25519-SIGNED
//! in-memory packages. The signatures are produced with the audited
//! `ed25519-dalek` crate (a fixed seed, so the tests are deterministic and need
//! no RNG), and the packages are round-tripped through
//! [`AbpPackage::to_bytes`] / [`AbpPackage::from_bytes`] so that the exact
//! on-disk byte layout the signature covers (`signed_region`) is what gets
//! verified — not a re-serialization of parsed fields.
//!
//! ## What is covered
//! - **T1** valid signed package verifies (`Ok`).
//! - **T2** a byte flipped inside the payload breaks verification (`Err`).
//! - **T3** a manifest/payload checksum mismatch is rejected (`Err`).
//! - **T4** a package signed by an untrusted key is rejected (`Err`).
//! - **T5** FAIL-CLOSED: with an empty trusted-key set, verification is refused
//!   (`Err`). This is the most important regression guard — an empty keyring
//!   must never be treated as "trust everything".
//! - **T7** an extra payload member not listed in the manifest is rejected
//!   (`Err`), plus a manifest entry missing from the payload (`Err`).
//!
//! ## What is intentionally omitted
//! - **T6 (path traversal)**: the traversal guard lives in
//!   `install::extract_payload` / `install::is_unsafe_member_name`, both of
//!   which are private `fn` (not `pub`/`pub(crate)`) and therefore unreachable
//!   from an integration test (which is a separate crate). `parse_tar_name` /
//!   `parse_tar_size` are `pub(crate)` and likewise unreachable. Per the task,
//!   the integrity path (T3/T7) is tested instead. Traversal rejection is
//!   already unit-tested in-crate.
//!
//! Note: `verify_package` checks the signature *before* internal integrity, so
//! the integrity tests (T3/T7) deliberately carry a **valid** signature over
//! their (deliberately-inconsistent) bytes in order to reach the integrity
//! checks at all.

use abp::crypto::{PublicKey, key_id, sha256};
use abp::format::{
    ABP_VERSION_MAJOR, ABP_VERSION_MINOR, AbpHeader, AbpPackage, Manifest, ManifestEntry,
    PackageMetadata,
};
use abp::verify::verify_package;
use ed25519_dalek::{Signer, SigningKey};

// ---------------------------------------------------------------------------
// tar payload construction
// ---------------------------------------------------------------------------

/// Format `n` as ASCII octal digits (no NUL/padding — the caller copies this
/// into a zero-initialised field, and the parser stops at the trailing NUL).
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

/// Build a single ustar-style member (512-byte header + data padded to a
/// 512-byte boundary) as the ABP tar walker expects: name at offset 0
/// (NUL-terminated), mode at 100..108, octal size at 124..136, regular-file
/// typeflag `'0'` at 156.
fn tar_member(name: &str, data: &[u8]) -> Vec<u8> {
    let mut block = vec![0u8; 512];

    let nb = name.as_bytes();
    assert!(
        nb.len() < 100,
        "test member name must fit the tar name field"
    );
    block[..nb.len()].copy_from_slice(nb);

    // mode 0644 (only the parser in extract_payload reads this; harmless here).
    let mode = b"0000644";
    block[100..100 + mode.len()].copy_from_slice(mode);

    // size (octal); the field is 124..136, zero-initialised so it is NUL-ended.
    let size = octal(data.len() as u64);
    block[124..124 + size.len()].copy_from_slice(&size);

    // regular file
    block[156] = b'0';

    let mut out = block;
    out.extend_from_slice(data);
    let pad = (512 - (data.len() % 512)) % 512;
    out.extend(std::iter::repeat_n(0u8, pad));
    out
}

/// Concatenate members into a tar archive (a trailing zero block is not
/// required: the walker terminates when fewer than 512 bytes remain).
fn tar_archive(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in members {
        out.extend_from_slice(&tar_member(name, data));
    }
    out
}

/// A manifest entry whose checksum matches `data` (the honest case).
fn entry(path: &str, data: &[u8]) -> ManifestEntry {
    ManifestEntry {
        path: String::from(path),
        size: data.len() as u64,
        mode: 0o644,
        checksum: sha256(data),
    }
}

// ---------------------------------------------------------------------------
// signed-package assembly
// ---------------------------------------------------------------------------

/// Deterministic signing key from a fixed 32-byte seed.
fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

/// Assemble a fully SIGNED package as on-disk bytes.
///
/// Follows the required recipe: build an in-memory [`AbpPackage`] with a
/// placeholder signature (so `to_bytes` sets the SIGNED flag and reserves the
/// trailer), serialise, then overwrite the 64-byte signature slot with a real
/// ed25519 signature over `bytes[..payload_end]` — exactly the region
/// `from_bytes` will later capture as `signed_region`.
fn assemble_signed(
    sk: &SigningKey,
    manifest_entries: Vec<ManifestEntry>,
    payload_tar: Vec<u8>,
) -> Vec<u8> {
    let pk = sk.verifying_key().to_bytes();
    let kid = key_id(&pk);

    let meta = PackageMetadata {
        name: String::from("secpkg"),
        version: String::from("1.0.0"),
        ..Default::default()
    };

    let pkg = AbpPackage {
        header: AbpHeader {
            version_major: ABP_VERSION_MAJOR,
            version_minor: ABP_VERSION_MINOR,
            flags: 0, // to_bytes recomputes flags (SIGNED + COMPRESSED_PAYLOAD)
            metadata_offset: 0,
            metadata_size: 0,
            manifest_offset: 0,
            manifest_size: 0,
            payload_offset: 0,
            payload_size: 0,
        },
        metadata: meta,
        manifest: Manifest {
            entries: manifest_entries,
        },
        payload: payload_tar,
        signature: Some([0u8; 64]), // placeholder → to_bytes sets SIGNED
        key_id: Some(kid),
        signed_region: None,
    };

    let mut bytes = pkg.to_bytes();

    // payload_end = payload_offset + payload_size, read back from the header.
    let header = AbpHeader::from_bytes(&bytes).expect("serialized header must parse");
    let payload_end = (header.payload_offset + header.payload_size) as usize;

    let sig = sk.sign(&bytes[..payload_end]).to_bytes();
    bytes[payload_end..payload_end + 64].copy_from_slice(&sig);

    bytes
}

/// The `[payload_offset, payload_end)` byte range of an assembled package.
fn payload_region(bytes: &[u8]) -> (usize, usize) {
    let header = AbpHeader::from_bytes(bytes).expect("header must parse");
    let start = header.payload_offset as usize;
    let end = start + header.payload_size as usize;
    (start, end)
}

/// A canonical, self-consistent set of members used by the "happy path" tests.
fn good_members() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("usr/bin/hello", b"hello world binary contents\n" as &[u8]),
        ("etc/secpkg/config", b"key = value\n" as &[u8]),
    ]
}

/// Manifest matching [`good_members`].
fn good_manifest() -> Vec<ManifestEntry> {
    good_members().iter().map(|(p, d)| entry(p, d)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// T1: a correctly-signed, internally-consistent package verifies against its
/// own public key.
#[test]
fn t1_valid_signed_package_verifies() {
    let sk = signing_key(7);
    let pk = sk.verifying_key().to_bytes();

    let payload = tar_archive(&good_members());
    let bytes = assemble_signed(&sk, good_manifest(), payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");
    let trusted = [PublicKey::from_bytes(&pk).expect("valid public key")];

    assert_eq!(
        verify_package(&loaded, &trusted),
        Ok(()),
        "a well-formed, correctly-signed package must verify"
    );
}

/// T2: flipping a single byte inside the payload region invalidates the
/// signature (the tampered bytes no longer match `signed_region`).
#[test]
fn t2_tampered_payload_is_rejected() {
    let sk = signing_key(7);
    let pk = sk.verifying_key().to_bytes();

    let payload = tar_archive(&good_members());
    let mut bytes = assemble_signed(&sk, good_manifest(), payload);

    // Flip one byte inside the (compressed) payload region.
    let (start, end) = payload_region(&bytes);
    assert!(end > start, "payload region must be non-empty");
    bytes[start] ^= 0x01;

    let loaded = AbpPackage::from_bytes(&bytes).expect("tampered package still parses");
    let trusted = [PublicKey::from_bytes(&pk).expect("valid public key")];

    assert!(
        verify_package(&loaded, &trusted).is_err(),
        "a package with a tampered payload must not verify"
    );
}

/// T3: a valid signature but a manifest checksum that does not match the payload
/// member must be rejected by the integrity check.
#[test]
fn t3_manifest_checksum_mismatch_is_rejected() {
    let sk = signing_key(7);
    let pk = sk.verifying_key().to_bytes();

    let members = good_members();
    let payload = tar_archive(&members);

    // Build a manifest whose FIRST entry has a deliberately wrong checksum
    // (checksum of different data than the member actually contains).
    let mut manifest = good_manifest();
    manifest[0].checksum = sha256(b"this is not what the member contains");

    // Sign over these (inconsistent) bytes so the signature itself is valid and
    // the integrity check is actually reached.
    let bytes = assemble_signed(&sk, manifest, payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");
    let trusted = [PublicKey::from_bytes(&pk).expect("valid public key")];

    assert!(
        verify_package(&loaded, &trusted).is_err(),
        "a manifest/payload checksum mismatch must be rejected"
    );
}

/// T4: a package signed by a key that is not in the trusted set (different key
/// id) must be rejected as untrusted.
#[test]
fn t4_untrusted_signing_key_is_rejected() {
    let signer = signing_key(7);
    let payload = tar_archive(&good_members());
    let bytes = assemble_signed(&signer, good_manifest(), payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");

    // A DIFFERENT key (different seed → different key id) is the only trusted one.
    let other = signing_key(42);
    let other_pk = other.verifying_key().to_bytes();
    assert_ne!(
        key_id(&signer.verifying_key().to_bytes()),
        key_id(&other_pk),
        "test keys must have distinct key ids"
    );
    let trusted = [PublicKey::from_bytes(&other_pk).expect("valid public key")];

    assert!(
        verify_package(&loaded, &trusted).is_err(),
        "a package signed by an untrusted key must be rejected"
    );
}

/// T5: FAIL-CLOSED. With no trusted keys at all, a perfectly valid signed
/// package must still be refused. An empty keyring must never mean "trust all".
#[test]
fn t5_empty_trusted_keys_fails_closed() {
    let sk = signing_key(7);
    let payload = tar_archive(&good_members());
    let bytes = assemble_signed(&sk, good_manifest(), payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");

    assert!(
        verify_package(&loaded, &[]).is_err(),
        "verification with an empty trusted-key set must fail closed"
    );
}

/// T7a: a payload member that is not present in the manifest must be rejected
/// (a package cannot smuggle in an unlisted file).
#[test]
fn t7_extra_payload_member_is_rejected() {
    let sk = signing_key(7);
    let pk = sk.verifying_key().to_bytes();

    // Payload has two members; the manifest lists only one.
    let payload = tar_archive(&[
        ("usr/bin/hello", b"hello world binary contents\n"),
        ("usr/bin/EXTRA", b"an unlisted, unmanifested payload\n"),
    ]);
    let manifest = vec![entry("usr/bin/hello", b"hello world binary contents\n")];

    let bytes = assemble_signed(&sk, manifest, payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");
    let trusted = [PublicKey::from_bytes(&pk).expect("valid public key")];

    assert!(
        verify_package(&loaded, &trusted).is_err(),
        "a payload member missing from the manifest must be rejected"
    );
}

/// T7b: a manifest entry with no corresponding payload member must be rejected
/// (the package does not deliver a file it claims to).
#[test]
fn t7_manifest_entry_missing_from_payload_is_rejected() {
    let sk = signing_key(7);
    let pk = sk.verifying_key().to_bytes();

    // Payload has one member; the manifest lists two.
    let payload = tar_archive(&[("usr/bin/hello", b"hello world binary contents\n")]);
    let manifest = vec![
        entry("usr/bin/hello", b"hello world binary contents\n"),
        entry("usr/bin/PHANTOM", b"a file the payload never provides\n"),
    ];

    let bytes = assemble_signed(&sk, manifest, payload);

    let loaded = AbpPackage::from_bytes(&bytes).expect("package must parse");
    let trusted = [PublicKey::from_bytes(&pk).expect("valid public key")];

    assert!(
        verify_package(&loaded, &trusted).is_err(),
        "a manifest entry with no payload member must be rejected"
    );
}
