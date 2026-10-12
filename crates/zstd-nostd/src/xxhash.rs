//! xxHash64 implementation for zstd content checksums

const PRIME64_1: u64 = 0x9E3779B185EBCA87;
const PRIME64_2: u64 = 0xC2B2AE3D27D4EB4F;
const PRIME64_3: u64 = 0x165667B19E3779F9;
const PRIME64_4: u64 = 0x85EBCA77C2B2AE63;
const PRIME64_5: u64 = 0x27D4EB2F165667C5;

#[inline]
fn read64(data: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([
        data[i],
        data[i + 1],
        data[i + 2],
        data[i + 3],
        data[i + 4],
        data[i + 5],
        data[i + 6],
        data[i + 7],
    ])
}

#[inline]
fn read32(data: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as u64
}

#[inline]
fn round(acc: u64, input: u64) -> u64 {
    acc.wrapping_add(input.wrapping_mul(PRIME64_2))
        .rotate_left(31)
        .wrapping_mul(PRIME64_1)
}

#[inline]
fn merge_round(acc: u64, val: u64) -> u64 {
    let val = round(0, val);
    (acc ^ val).wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4)
}

/// Compute xxHash64 of the given data with seed 0
pub fn xxhash64(data: &[u8]) -> u64 {
    xxhash64_seed(data, 0)
}

/// Compute xxHash64 of the given data with the specified seed
pub fn xxhash64_seed(data: &[u8], seed: u64) -> u64 {
    let len = data.len();
    let mut h: u64;
    let mut i = 0;

    if len >= 32 {
        let mut v1 = seed.wrapping_add(PRIME64_1).wrapping_add(PRIME64_2);
        let mut v2 = seed.wrapping_add(PRIME64_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME64_1);

        while i + 32 <= len {
            v1 = round(v1, read64(data, i));
            v2 = round(v2, read64(data, i + 8));
            v3 = round(v3, read64(data, i + 16));
            v4 = round(v4, read64(data, i + 24));
            i += 32;
        }

        h = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));

        h = merge_round(h, v1);
        h = merge_round(h, v2);
        h = merge_round(h, v3);
        h = merge_round(h, v4);
    } else {
        h = seed.wrapping_add(PRIME64_5);
    }

    h = h.wrapping_add(len as u64);

    // Process remaining 8-byte chunks
    while i + 8 <= len {
        let k = read64(data, i).wrapping_mul(PRIME64_2);
        let k = k.rotate_left(31).wrapping_mul(PRIME64_1);
        h ^= k;
        h = h
            .rotate_left(27)
            .wrapping_mul(PRIME64_1)
            .wrapping_add(PRIME64_4);
        i += 8;
    }

    // Process remaining 4-byte chunk
    if i + 4 <= len {
        h ^= read32(data, i).wrapping_mul(PRIME64_1);
        h = h
            .rotate_left(23)
            .wrapping_mul(PRIME64_2)
            .wrapping_add(PRIME64_3);
        i += 4;
    }

    // Process remaining bytes
    while i < len {
        h ^= (data[i] as u64).wrapping_mul(PRIME64_5);
        h = h.rotate_left(11).wrapping_mul(PRIME64_1);
        i += 1;
    }

    // Final mix
    h ^= h >> 33;
    h = h.wrapping_mul(PRIME64_2);
    h ^= h >> 29;
    h = h.wrapping_mul(PRIME64_3);
    h ^= h >> 32;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xxhash64_empty() {
        assert_eq!(xxhash64(b""), 0xEF46DB3751D8E999);
    }

    #[test]
    fn test_xxhash64_short() {
        // Known test vector
        assert_eq!(xxhash64(b"a"), 0xD24EC4F1A98C6E5B);
    }

    #[test]
    fn test_xxhash64_medium() {
        // "Hello, World!" with seed 0
        assert_eq!(xxhash64(b"Hello, World!"), 0xC49AACF8080FE47F);
    }

    #[test]
    fn test_xxhash64_32plus_bytes() {
        let data = b"This is a test string that is longer than 32 bytes for xxhash64";
        assert_eq!(xxhash64(data), 0xC667E9C86491AB3A);
    }
}
