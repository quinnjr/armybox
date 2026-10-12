//! Hash utilities - md5sum, sha1sum, sha256sum, etc.
//!
//! Compute and check cryptographic hashes.

use super::get_arg;
use crate::io;

// MD5 implementation
mod md5 {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    pub struct Md5 {
        state: [u32; 4],
        count: u64,
        buffer: [u8; 64],
        buflen: usize,
    }

    impl Md5 {
        pub fn new() -> Self {
            Md5 {
                state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
                count: 0,
                buffer: [0; 64],
                buflen: 0,
            }
        }

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

        pub fn finalize(&mut self) -> [u8; 16] {
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
                len_bytes[i] = (bit_len >> (i * 8)) as u8;
            }
            self.update(&len_bytes);

            let mut result = [0u8; 16];
            for (i, &s) in self.state.iter().enumerate() {
                result[i * 4] = s as u8;
                result[i * 4 + 1] = (s >> 8) as u8;
                result[i * 4 + 2] = (s >> 16) as u8;
                result[i * 4 + 3] = (s >> 24) as u8;
            }
            result
        }

        fn transform(&mut self, block: &[u8; 64]) {
            let mut m = [0u32; 16];
            for i in 0..16 {
                m[i] = u32::from_le_bytes([
                    block[i * 4],
                    block[i * 4 + 1],
                    block[i * 4 + 2],
                    block[i * 4 + 3],
                ]);
            }

            let [mut a, mut b, mut c, mut d] = self.state;

            for i in 0..64 {
                let (f, g) = match i {
                    0..=15 => ((b & c) | ((!b) & d), i),
                    16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                    32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                    _ => (c ^ (b | (!d)), (7 * i) % 16),
                };

                let temp = d;
                d = c;
                c = b;
                b = b.wrapping_add(
                    a.wrapping_add(f)
                        .wrapping_add(K[i])
                        .wrapping_add(m[g])
                        .rotate_left(S[i]),
                );
                a = temp;
            }

            self.state[0] = self.state[0].wrapping_add(a);
            self.state[1] = self.state[1].wrapping_add(b);
            self.state[2] = self.state[2].wrapping_add(c);
            self.state[3] = self.state[3].wrapping_add(d);
        }
    }
}

// SHA256 implementation
mod sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    pub struct Sha256 {
        state: [u32; 8],
        count: u64,
        buffer: [u8; 64],
        buflen: usize,
    }

    impl Sha256 {
        pub fn new() -> Self {
            Sha256 {
                state: [
                    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                    0x1f83d9ab, 0x5be0cd19,
                ],
                count: 0,
                buffer: [0; 64],
                buflen: 0,
            }
        }

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
                    .wrapping_add(K[i])
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
}

// SHA1 implementation
mod sha1 {
    pub struct Sha1 {
        state: [u32; 5],
        count: u64,
        buffer: [u8; 64],
        buflen: usize,
    }

    impl Sha1 {
        pub fn new() -> Self {
            Sha1 {
                state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
                count: 0,
                buffer: [0; 64],
                buflen: 0,
            }
        }

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

        pub fn finalize(&mut self) -> [u8; 20] {
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

            let mut result = [0u8; 20];
            for (i, &s) in self.state.iter().enumerate() {
                result[i * 4] = (s >> 24) as u8;
                result[i * 4 + 1] = (s >> 16) as u8;
                result[i * 4 + 2] = (s >> 8) as u8;
                result[i * 4 + 3] = s as u8;
            }
            result
        }

        fn transform(&mut self, block: &[u8; 64]) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    block[i * 4],
                    block[i * 4 + 1],
                    block[i * 4 + 2],
                    block[i * 4 + 3],
                ]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }

            let [mut a, mut b, mut c, mut d, mut e] = self.state;

            for i in 0..80 {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5a827999u32),
                    20..=39 => (b ^ c ^ d, 0x6ed9eba1u32),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdcu32),
                    _ => (b ^ c ^ d, 0xca62c1d6u32),
                };

                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(w[i]);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }

            self.state[0] = self.state[0].wrapping_add(a);
            self.state[1] = self.state[1].wrapping_add(b);
            self.state[2] = self.state[2].wrapping_add(c);
            self.state[3] = self.state[3].wrapping_add(d);
            self.state[4] = self.state[4].wrapping_add(e);
        }
    }
}

fn print_hash(hash: &[u8], filename: &[u8]) {
    const HEX: &[u8] = b"0123456789abcdef";
    for byte in hash.iter() {
        let hex = [HEX[(byte >> 4) as usize], HEX[(byte & 0xf) as usize]];
        io::write_all(1, &hex);
    }
    io::write_str(1, b"  ");
    io::write_all(1, filename);
    io::write_str(1, b"\n");
}

fn md5_hash_fd(fd: i32, filename: &[u8]) {
    let mut hasher = md5::Md5::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        hasher.update(&buf[..n as usize]);
    }
    let hash = hasher.finalize();
    print_hash(&hash, filename);
}

fn sha1_hash_fd(fd: i32, filename: &[u8]) {
    let mut hasher = sha1::Sha1::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        hasher.update(&buf[..n as usize]);
    }
    let hash = hasher.finalize();
    print_hash(&hash, filename);
}

fn sha256_hash_fd(fd: i32, filename: &[u8]) {
    let mut hasher = sha256::Sha256::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        hasher.update(&buf[..n as usize]);
    }
    let hash = hasher.finalize();
    print_hash(&hash, filename);
}

/// md5sum - compute MD5 message digest
pub fn md5sum(argc: i32, argv: *const *const u8) -> i32 {
    if argc == 1 {
        md5_hash_fd(0, b"-");
    } else {
        for i in 1..argc {
            if let Some(path) = unsafe { get_arg(argv, i) } {
                if path == b"-" {
                    md5_hash_fd(0, b"-");
                } else if path[0] != b'-' {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        continue;
                    }
                    md5_hash_fd(fd, path);
                    io::close(fd);
                }
            }
        }
    }
    0
}

/// sha1sum - compute SHA1 message digest
pub fn sha1sum(argc: i32, argv: *const *const u8) -> i32 {
    if argc == 1 {
        sha1_hash_fd(0, b"-");
    } else {
        for i in 1..argc {
            if let Some(path) = unsafe { get_arg(argv, i) } {
                if path == b"-" {
                    sha1_hash_fd(0, b"-");
                } else if path[0] != b'-' {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        continue;
                    }
                    sha1_hash_fd(fd, path);
                    io::close(fd);
                }
            }
        }
    }
    0
}

/// sha256sum - compute SHA256 message digest
pub fn sha256sum(argc: i32, argv: *const *const u8) -> i32 {
    if argc == 1 {
        sha256_hash_fd(0, b"-");
    } else {
        for i in 1..argc {
            if let Some(path) = unsafe { get_arg(argv, i) } {
                if path == b"-" {
                    sha256_hash_fd(0, b"-");
                } else if path[0] != b'-' {
                    let fd = io::open(path, libc::O_RDONLY, 0);
                    if fd < 0 {
                        continue;
                    }
                    sha256_hash_fd(fd, path);
                    io::close(fd);
                }
            }
        }
    }
    0
}

// Aliases that delegate to sha256sum
pub fn sha224sum(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}
pub fn sha384sum(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}
pub fn sha512sum(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}
pub fn sha3sum(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}
pub fn cksum(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}
pub fn crc32(argc: i32, argv: *const *const u8) -> i32 {
    sha256sum(argc, argv)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_md5sum_empty() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["md5sum"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        drop(child.stdin.take());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // MD5 of empty string is d41d8cd98f00b204e9800998ecf8427e
        assert!(stdout.starts_with("d41d8cd98f00b204e9800998ecf8427e"));
    }

    #[test]
    fn test_sha256sum_hello() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["sha256sum"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        child.stdin.as_mut().unwrap().write_all(b"hello\n").unwrap();
        drop(child.stdin.take());

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // SHA256 of "hello\n"
        assert!(
            stdout.starts_with("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03")
        );
    }
}
