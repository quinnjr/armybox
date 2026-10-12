//! Cryptographic primitives for ABP package manager
//!
//! Provides SHA256 hashing and Ed25519 signature verification.

extern crate alloc;
use alloc::vec::Vec;

// =============================================================================
// SHA256 Implementation
// =============================================================================

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA256 hasher
pub struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
    buflen: usize,
}

impl Sha256 {
    /// Create a new SHA256 hasher
    pub fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            count: 0,
            buffer: [0; 64],
            buflen: 0,
        }
    }

    /// Update the hasher with more data
    pub fn update(&mut self, data: &[u8]) {
        self.count += data.len() as u64;
        let mut offset = 0;

        if self.buflen > 0 {
            let space = 64 - self.buflen;
            let copy = core::cmp::min(space, data.len());
            self.buffer[self.buflen..self.buflen + copy].copy_from_slice(&data[..copy]);
            self.buflen += copy;
            offset = copy;

            if self.buflen == 64 {
                self.transform(&self.buffer.clone());
                self.buflen = 0;
            }
        }

        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            self.transform(&block);
            offset += 64;
        }

        if offset < data.len() {
            self.buflen = data.len() - offset;
            self.buffer[..self.buflen].copy_from_slice(&data[offset..]);
        }
    }

    /// Finalize and return the hash
    pub fn finalize(&mut self) -> [u8; 32] {
        let bit_len = self.count * 8;
        let pad_len = if self.buflen < 56 {
            56 - self.buflen
        } else {
            120 - self.buflen
        };

        let mut padding = [0u8; 128];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);

        let mut len_bytes = [0u8; 8];
        for i in 0..8 {
            len_bytes[7 - i] = (bit_len >> (i * 8)) as u8;
        }
        self.update(&len_bytes);

        let mut result = [0u8; 32];
        for (i, &s) in self.state.iter().enumerate() {
            result[i * 4] = (s >> 24) as u8;
            result[i * 4 + 1] = (s >> 16) as u8;
            result[i * 4 + 2] = (s >> 8) as u8;
            result[i * 4 + 3] = s as u8;
        }
        result
    }

    fn transform(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Compute SHA256 hash of data
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize()
}

/// Compute SHA256 hash and return as hex string
pub fn sha256_hex(data: &[u8]) -> [u8; 64] {
    let hash = sha256(data);
    let mut hex = [0u8; 64];
    const HEX: &[u8] = b"0123456789abcdef";
    for (i, &byte) in hash.iter().enumerate() {
        hex[i * 2] = HEX[(byte >> 4) as usize];
        hex[i * 2 + 1] = HEX[(byte & 0xf) as usize];
    }
    hex
}

// =============================================================================
// SHA512 Implementation (needed for Ed25519)
// =============================================================================

const SHA512_K: [u64; 80] = [
    0x428a2f98d728ae22,
    0x7137449123ef65cd,
    0xb5c0fbcfec4d3b2f,
    0xe9b5dba58189dbbc,
    0x3956c25bf348b538,
    0x59f111f1b605d019,
    0x923f82a4af194f9b,
    0xab1c5ed5da6d8118,
    0xd807aa98a3030242,
    0x12835b0145706fbe,
    0x243185be4ee4b28c,
    0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f,
    0x80deb1fe3b1696b1,
    0x9bdc06a725c71235,
    0xc19bf174cf692694,
    0xe49b69c19ef14ad2,
    0xefbe4786384f25e3,
    0x0fc19dc68b8cd5b5,
    0x240ca1cc77ac9c65,
    0x2de92c6f592b0275,
    0x4a7484aa6ea6e483,
    0x5cb0a9dcbd41fbd4,
    0x76f988da831153b5,
    0x983e5152ee66dfab,
    0xa831c66d2db43210,
    0xb00327c898fb213f,
    0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2,
    0xd5a79147930aa725,
    0x06ca6351e003826f,
    0x142929670a0e6e70,
    0x27b70a8546d22ffc,
    0x2e1b21385c26c926,
    0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df,
    0x650a73548baf63de,
    0x766a0abb3c77b2a8,
    0x81c2c92e47edaee6,
    0x92722c851482353b,
    0xa2bfe8a14cf10364,
    0xa81a664bbc423001,
    0xc24b8b70d0f89791,
    0xc76c51a30654be30,
    0xd192e819d6ef5218,
    0xd69906245565a910,
    0xf40e35855771202a,
    0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8,
    0x1e376c085141ab53,
    0x2748774cdf8eeb99,
    0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63,
    0x4ed8aa4ae3418acb,
    0x5b9cca4f7763e373,
    0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc,
    0x78a5636f43172f60,
    0x84c87814a1f0ab72,
    0x8cc702081a6439ec,
    0x90befffa23631e28,
    0xa4506cebde82bde9,
    0xbef9a3f7b2c67915,
    0xc67178f2e372532b,
    0xca273eceea26619c,
    0xd186b8c721c0c207,
    0xeada7dd6cde0eb1e,
    0xf57d4f7fee6ed178,
    0x06f067aa72176fba,
    0x0a637dc5a2c898a6,
    0x113f9804bef90dae,
    0x1b710b35131c471b,
    0x28db77f523047d84,
    0x32caab7b40c72493,
    0x3c9ebe0a15c9bebc,
    0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6,
    0x597f299cfc657e2a,
    0x5fcb6fab3ad6faec,
    0x6c44198c4a475817,
];

/// SHA512 hasher
pub struct Sha512 {
    state: [u64; 8],
    count: u128,
    buffer: [u8; 128],
    buflen: usize,
}

impl Sha512 {
    /// Create a new SHA512 hasher
    pub fn new() -> Self {
        Sha512 {
            state: [
                0x6a09e667f3bcc908,
                0xbb67ae8584caa73b,
                0x3c6ef372fe94f82b,
                0xa54ff53a5f1d36f1,
                0x510e527fade682d1,
                0x9b05688c2b3e6c1f,
                0x1f83d9abfb41bd6b,
                0x5be0cd19137e2179,
            ],
            count: 0,
            buffer: [0; 128],
            buflen: 0,
        }
    }

    /// Update the hasher with more data
    pub fn update(&mut self, data: &[u8]) {
        self.count += data.len() as u128;
        let mut offset = 0;

        if self.buflen > 0 {
            let space = 128 - self.buflen;
            let copy = core::cmp::min(space, data.len());
            self.buffer[self.buflen..self.buflen + copy].copy_from_slice(&data[..copy]);
            self.buflen += copy;
            offset = copy;

            if self.buflen == 128 {
                self.transform(&self.buffer.clone());
                self.buflen = 0;
            }
        }

        while offset + 128 <= data.len() {
            let mut block = [0u8; 128];
            block.copy_from_slice(&data[offset..offset + 128]);
            self.transform(&block);
            offset += 128;
        }

        if offset < data.len() {
            self.buflen = data.len() - offset;
            self.buffer[..self.buflen].copy_from_slice(&data[offset..]);
        }
    }

    /// Finalize and return the hash
    pub fn finalize(&mut self) -> [u8; 64] {
        let bit_len = self.count * 8;
        let pad_len = if self.buflen < 112 {
            112 - self.buflen
        } else {
            240 - self.buflen
        };

        let mut padding = [0u8; 256];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);

        let mut len_bytes = [0u8; 16];
        for i in 0..16 {
            len_bytes[15 - i] = (bit_len >> (i * 8)) as u8;
        }
        self.update(&len_bytes);

        let mut result = [0u8; 64];
        for (i, &s) in self.state.iter().enumerate() {
            for j in 0..8 {
                result[i * 8 + j] = (s >> (56 - j * 8)) as u8;
            }
        }
        result
    }

    fn transform(&mut self, block: &[u8; 128]) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            w[i] = u64::from_be_bytes([
                block[i * 8],
                block[i * 8 + 1],
                block[i * 8 + 2],
                block[i * 8 + 3],
                block[i * 8 + 4],
                block[i * 8 + 5],
                block[i * 8 + 6],
                block[i * 8 + 7],
            ]);
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA512_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

/// Compute SHA512 hash of data
pub fn sha512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize()
}

// =============================================================================
// Ed25519 Implementation
// =============================================================================

//
// Signature verification delegates to the audited `ed25519-dalek` crate.
// (The previous hand-rolled ref10 field/group/scalar arithmetic was broken —
// it overflowed in field multiplication and never reduced scalars — so it was
// removed in favour of a reviewed implementation.)

use ed25519_dalek::{Signature, VerifyingKey};

/// Ed25519 public key
pub struct PublicKey([u8; 32]);

impl PublicKey {
    /// Create a public key from bytes, rejecting encodings that are not a
    /// valid point on the curve.
    pub fn from_bytes(bytes: &[u8; 32]) -> Option<PublicKey> {
        VerifyingKey::from_bytes(bytes)
            .ok()
            .map(|_| PublicKey(*bytes))
    }

    /// Get the key bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Verify a signature over `message`.
    ///
    /// Uses `verify_strict`, which rejects non-canonical `R`/`s` encodings and
    /// small-order public keys, giving non-malleable verification.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        let vk = match VerifyingKey::from_bytes(&self.0) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        let sig = Signature::from_bytes(signature);
        vk.verify_strict(message, &sig).is_ok()
    }
}

/// Verify an Ed25519 signature.
pub fn verify_signature(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    match PublicKey::from_bytes(public_key) {
        Some(pk) => pk.verify(message, signature),
        None => false,
    }
}

/// Compute key ID (SHA256 of public key)
pub fn key_id(public_key: &[u8; 32]) -> [u8; 32] {
    sha256(public_key)
}

// =============================================================================
// Utility functions
// =============================================================================

/// Convert bytes to hex string
pub fn to_hex(data: &[u8]) -> Vec<u8> {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut result = Vec::with_capacity(data.len() * 2);
    for &byte in data {
        result.push(HEX[(byte >> 4) as usize]);
        result.push(HEX[(byte & 0xf) as usize]);
    }
    result
}

/// Parse hex string to bytes
pub fn from_hex(hex: &[u8]) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }

    let mut result = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.chunks(2) {
        let hi = hex_digit(chunk[0])?;
        let lo = hex_digit(chunk[1])?;
        result.push((hi << 4) | lo);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_empty() {
        let hash = sha256(b"");
        let hex = sha256_hex(b"");
        assert_eq!(
            &hex[..],
            b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_hello() {
        let hash = sha256(b"hello");
        let hex = sha256_hex(b"hello");
        assert_eq!(
            &hex[..],
            b"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha512_empty() {
        let hash = sha512(b"");
        assert_eq!(hash[0], 0xcf);
        assert_eq!(hash[1], 0x83);
    }

    fn hex32(h: &[u8]) -> [u8; 32] {
        let v = from_hex(h).unwrap();
        let mut a = [0u8; 32];
        a.copy_from_slice(&v);
        a
    }

    fn hex64(h: &[u8]) -> [u8; 64] {
        let v = from_hex(h).unwrap();
        let mut a = [0u8; 64];
        a.copy_from_slice(&v);
        a
    }

    // RFC 8032 section 7.1, Test 1 (empty message).
    #[test]
    fn test_ed25519_rfc8032_test1_empty() {
        let pk = hex32(b"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let sig = hex64(b"e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b");
        assert!(
            verify_signature(&pk, b"", &sig),
            "valid RFC 8032 test-1 signature must verify"
        );
    }

    // RFC 8032 section 7.1, Test 2 (1-byte message 0x72).
    #[test]
    fn test_ed25519_rfc8032_test2_onebyte() {
        let pk = hex32(b"3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c");
        let sig = hex64(b"92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00");
        assert!(
            verify_signature(&pk, &[0x72], &sig),
            "valid RFC 8032 test-2 signature must verify"
        );
    }

    // Tampering with the message must make a previously-valid signature fail.
    #[test]
    fn test_ed25519_rejects_tampered_message() {
        let pk = hex32(b"3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c");
        let sig = hex64(b"92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00");
        assert!(
            !verify_signature(&pk, &[0x73], &sig),
            "signature must not verify for a different message"
        );
    }

    // Flipping one signature bit must fail.
    #[test]
    fn test_ed25519_rejects_tampered_signature() {
        let pk = hex32(b"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let mut sig = hex64(b"e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b");
        sig[0] ^= 0x01;
        assert!(
            !verify_signature(&pk, b"", &sig),
            "bit-flipped signature must not verify"
        );
    }
}
