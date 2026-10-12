//! login - begin session on the system
//!
//! Begin a session on the system.

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// login - begin session on the system
///
/// # Synopsis
/// ```text
/// login [-f] [-h HOST] [-p] [username]
/// ```
///
/// # Description
/// Begin a session on the system. Prompts for username and password,
/// validates credentials, sets up environment, and spawns user shell.
///
/// # Options
/// - `-f`: Skip authentication (requires root)
/// - `-h HOST`: Remote host name (for utmp)
/// - `-p`: Preserve environment
///
/// # Exit Status
/// - 0: Success (never returns on successful login)
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn login(argc: i32, argv: *const *const u8) -> i32 {
    let mut username: Option<&[u8]> = None;
    let mut skip_auth = false;
    let mut preserve_env = false;
    let mut _hostname: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-f" {
            skip_auth = true;
        } else if arg == b"-h" {
            i += 1;
            if let Some(h) = unsafe { get_arg(argv, i as i32) } {
                _hostname = Some(h);
            }
        } else if arg == b"-p" {
            preserve_env = true;
        } else if arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            username = Some(arg);
        }
        i += 1;
    }

    // Skip auth requires root
    if skip_auth && unsafe { libc::getuid() } != 0 {
        io::write_str(2, b"login: -f requires root\n");
        return 1;
    }

    // Get username if not provided
    let username = match username {
        Some(u) => u.to_vec(),
        None => {
            io::write_str(1, b"login: ");
            match read_line() {
                Some(u) if !u.is_empty() => u,
                _ => {
                    io::write_str(2, b"\n");
                    return 1;
                }
            }
        }
    };

    // Look up user in passwd
    let passwd_entry = match get_passwd_entry(&username) {
        Some(e) => e,
        None => {
            // Delay to prevent timing attacks
            delay(2);
            io::write_str(2, b"Login incorrect\n");
            return 1;
        }
    };

    // Authenticate unless -f
    if !skip_auth {
        // Get password hash from shadow
        let hash = get_shadow_hash(&username);

        // Prompt for password
        io::write_str(1, b"Password: ");

        // Disable echo
        let old_termios = disable_echo();

        let password = match read_line() {
            Some(p) => p,
            None => {
                restore_termios(&old_termios);
                io::write_str(1, b"\n");
                return 1;
            }
        };

        restore_termios(&old_termios);
        io::write_str(1, b"\n");

        // Verify password
        if !verify_password(&password, &hash) {
            // Delay to prevent brute force
            delay(2);
            io::write_str(2, b"Login incorrect\n");
            return 1;
        }
    }

    // Check if account is expired or shell is nologin
    if is_nologin_shell(&passwd_entry.shell) {
        io::write_str(2, b"This account is currently not available.\n");
        return 1;
    }

    // Check for /etc/nologin
    if unsafe { libc::getuid() } != 0 {
        let nologin_fd = io::open(b"/etc/nologin", libc::O_RDONLY, 0);
        if nologin_fd >= 0 {
            let content = io::read_all(nologin_fd);
            io::close(nologin_fd);
            if !content.is_empty() {
                io::write_all(2, &content);
            } else {
                io::write_str(2, b"System is unavailable\n");
            }
            return 1;
        }
    }

    // Set up session
    if unsafe { libc::setgid(passwd_entry.gid) } < 0 {
        sys::perror(b"setgid");
        return 1;
    }

    // Initialize supplementary groups
    let username_cstr = make_cstr(&username);
    if unsafe {
        libc::initgroups(
            username_cstr.as_ptr() as *const libc::c_char,
            passwd_entry.gid,
        )
    } < 0
    {
        sys::perror(b"initgroups");
        return 1;
    }

    if unsafe { libc::setuid(passwd_entry.uid) } < 0 {
        sys::perror(b"setuid");
        return 1;
    }

    // Change to home directory
    let home_cstr = make_cstr(&passwd_entry.home);
    if unsafe { libc::chdir(home_cstr.as_ptr() as *const libc::c_char) } < 0 {
        // Fall back to /
        unsafe { libc::chdir(b"/\0".as_ptr() as *const libc::c_char) };
    }

    // Set up environment
    if !preserve_env {
        unsafe { libc::clearenv() };
    }

    set_env(b"HOME", &passwd_entry.home);
    set_env(b"SHELL", &passwd_entry.shell);
    set_env(b"USER", &username);
    set_env(b"LOGNAME", &username);
    set_env(b"PATH", b"/usr/local/bin:/usr/bin:/bin");

    if let Ok(term) = get_env(b"TERM") {
        set_env(b"TERM", &term);
    } else {
        set_env(b"TERM", b"linux");
    }

    // Print motd
    let motd_fd = io::open(b"/etc/motd", libc::O_RDONLY, 0);
    if motd_fd >= 0 {
        let content = io::read_all(motd_fd);
        io::close(motd_fd);
        if !content.is_empty() {
            io::write_all(1, &content);
        }
    }

    // Execute shell
    let shell_cstr = make_cstr(&passwd_entry.shell);

    // Create login shell name (prepend -)
    let mut shell_name = Vec::new();
    shell_name.push(b'-');
    if let Some(pos) = passwd_entry.shell.iter().rposition(|&c| c == b'/') {
        shell_name.extend_from_slice(&passwd_entry.shell[pos + 1..]);
    } else {
        shell_name.extend_from_slice(&passwd_entry.shell);
    }
    shell_name.push(0);

    let argv: [*const libc::c_char; 2] = [
        shell_name.as_ptr() as *const libc::c_char,
        core::ptr::null(),
    ];

    unsafe {
        libc::execv(shell_cstr.as_ptr() as *const libc::c_char, argv.as_ptr());
    }

    sys::perror(b"exec");
    1
}

struct PasswdEntry {
    uid: u32,
    gid: u32,
    home: Vec<u8>,
    shell: Vec<u8>,
}

fn get_passwd_entry(username: &[u8]) -> Option<PasswdEntry> {
    let fd = io::open(b"/etc/passwd", libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }

    let content = io::read_all(fd);
    io::close(fd);

    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        let fields: Vec<&[u8]> = line.split(|&c| c == b':').collect();
        if fields.len() < 7 {
            continue;
        }

        if fields[0] == username {
            return Some(PasswdEntry {
                uid: sys::parse_u64(fields[2]).unwrap_or(65534) as u32,
                gid: sys::parse_u64(fields[3]).unwrap_or(65534) as u32,
                home: fields[5].to_vec(),
                shell: fields[6].to_vec(),
            });
        }
    }

    None
}

pub(crate) fn get_shadow_hash(username: &[u8]) -> Vec<u8> {
    let fd = io::open(b"/etc/shadow", libc::O_RDONLY, 0);
    if fd < 0 {
        return Vec::new();
    }

    let content = io::read_all(fd);
    io::close(fd);

    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        let fields: Vec<&[u8]> = line.split(|&c| c == b':').collect();
        if fields.len() < 2 {
            continue;
        }

        if fields[0] == username {
            return fields[1].to_vec();
        }
    }

    Vec::new()
}

pub(crate) fn verify_password(password: &[u8], hash: &[u8]) -> bool {
    // Handle locked/disabled accounts
    if hash.is_empty() || hash == b"*" || hash == b"!" || hash == b"!!" || hash == b"x" {
        return false;
    }

    // Parse hash format: $id$[rounds=N$]salt$hash
    if !hash.starts_with(b"$") {
        return false;
    }

    // Parse hash components
    let parts: Vec<&[u8]> = hash.split(|&c| c == b'$').collect();
    if parts.len() < 4 {
        return false;
    }

    let id = parts[1];
    let (rounds, salt, expected_hash_idx) = if parts[2].starts_with(b"rounds=") {
        let rounds_str = &parts[2][7..];
        let r = parse_rounds(rounds_str)
            .unwrap_or(5000)
            .max(1000)
            .min(999999999);
        (r as usize, parts[3], 4)
    } else {
        (5000usize, parts[2], 3)
    };

    // Get the expected hash portion
    let expected_hash = if expected_hash_idx < parts.len() {
        parts[expected_hash_idx]
    } else {
        return false;
    };

    // Limit salt to 16 characters
    let salt = if salt.len() > 16 { &salt[..16] } else { salt };

    // Determine hash type and verify
    let computed = if id == b"6" {
        // SHA-512
        sha512_crypt(password, salt, rounds)
    } else if id == b"5" {
        // SHA-256
        sha256_crypt(password, salt, rounds)
    } else {
        // Unsupported hash type (MD5 $1$, Blowfish $2a$, etc.)
        return false;
    };

    // Constant-time comparison of hash portions only
    if computed.len() != expected_hash.len() {
        return false;
    }

    let mut diff = 0u8;
    for (a, b) in computed.iter().zip(expected_hash.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn parse_rounds(s: &[u8]) -> Option<u32> {
    let mut val = 0u32;
    for &c in s {
        if c >= b'0' && c <= b'9' {
            val = val.checked_mul(10)?.checked_add((c - b'0') as u32)?;
        } else {
            break;
        }
    }
    Some(val)
}

// SHA-512 crypt - based on Ulrich Drepper's specification
// Reference: https://www.akkadia.org/drepper/SHA-crypt.txt
fn sha512_crypt(password: &[u8], salt: &[u8], rounds: usize) -> Vec<u8> {
    const BLOCK_SIZE: usize = 64;

    let pw_len = password.len();
    let salt_len = salt.len();

    // Step 1-3: Compute alternate digest B
    let mut hasher = Sha512::new();
    hasher.update(password);
    hasher.update(salt);
    hasher.update(password);
    let digest_b = hasher.finalize();

    // Step 4-8: Compute initial digest A
    let mut hasher = Sha512::new();
    hasher.update(password);
    hasher.update(salt);

    // Step 9-10: Add bytes from B based on password length
    for _ in 0..(pw_len / BLOCK_SIZE) {
        hasher.update(&digest_b);
    }
    hasher.update(&digest_b[..(pw_len % BLOCK_SIZE)]);

    // Step 11: For each bit in password length, add B or password
    let mut n = pw_len;
    while n > 0 {
        if (n & 1) != 0 {
            hasher.update(&digest_b);
        } else {
            hasher.update(password);
        }
        n >>= 1;
    }

    let digest_a = hasher.finalize();

    // Step 12-14: Compute P = SHA512(password repeated pw_len times)
    let mut hasher = Sha512::new();
    for _ in 0..pw_len {
        hasher.update(password);
    }
    let dp = hasher.finalize();

    // Step 15-16: Create P_bytes sequence
    let mut p_bytes = Vec::with_capacity(pw_len);
    for _ in 0..(pw_len / BLOCK_SIZE) {
        p_bytes.extend_from_slice(&dp);
    }
    p_bytes.extend_from_slice(&dp[..(pw_len % BLOCK_SIZE)]);

    // Step 17-18: Compute S = SHA512(salt repeated (16 + A[0]) times)
    let mut hasher = Sha512::new();
    for _ in 0..(16 + digest_a[0] as usize) {
        hasher.update(salt);
    }
    let ds = hasher.finalize();

    // Step 19-20: Create S_bytes sequence
    let mut s_bytes = Vec::with_capacity(salt_len);
    for _ in 0..(salt_len / BLOCK_SIZE) {
        s_bytes.extend_from_slice(&ds);
    }
    s_bytes.extend_from_slice(&ds[..(salt_len % BLOCK_SIZE)]);

    // Step 21: Main rounds loop
    let mut digest_c = digest_a;
    for i in 0..rounds {
        let mut hasher = Sha512::new();

        // Add key or last result
        if (i & 1) != 0 {
            hasher.update(&p_bytes);
        } else {
            hasher.update(&digest_c);
        }

        // Add salt for numbers not divisible by 3
        if i % 3 != 0 {
            hasher.update(&s_bytes);
        }

        // Add key for numbers not divisible by 7
        if i % 7 != 0 {
            hasher.update(&p_bytes);
        }

        // Add key or last result
        if (i & 1) != 0 {
            hasher.update(&digest_c);
        } else {
            hasher.update(&p_bytes);
        }

        digest_c = hasher.finalize();
    }

    // Encode with crypt's base64
    sha512_b64_encode(&digest_c)
}

// SHA-256 crypt
fn sha256_crypt(password: &[u8], salt: &[u8], rounds: usize) -> Vec<u8> {
    const BLOCK_SIZE: usize = 32;

    let pw_len = password.len();
    let salt_len = salt.len();

    // Compute alternate digest B
    let mut hasher = Sha256::new();
    hasher.update(password);
    hasher.update(salt);
    hasher.update(password);
    let digest_b = hasher.finalize();

    // Compute initial digest A
    let mut hasher = Sha256::new();
    hasher.update(password);
    hasher.update(salt);

    for _ in 0..(pw_len / BLOCK_SIZE) {
        hasher.update(&digest_b);
    }
    hasher.update(&digest_b[..(pw_len % BLOCK_SIZE)]);

    let mut n = pw_len;
    while n > 0 {
        if (n & 1) != 0 {
            hasher.update(&digest_b);
        } else {
            hasher.update(password);
        }
        n >>= 1;
    }

    let digest_a = hasher.finalize();

    // Compute P
    let mut hasher = Sha256::new();
    for _ in 0..pw_len {
        hasher.update(password);
    }
    let dp = hasher.finalize();

    let mut p_bytes = Vec::with_capacity(pw_len);
    for _ in 0..(pw_len / BLOCK_SIZE) {
        p_bytes.extend_from_slice(&dp);
    }
    p_bytes.extend_from_slice(&dp[..(pw_len % BLOCK_SIZE)]);

    // Compute S
    let mut hasher = Sha256::new();
    for _ in 0..(16 + digest_a[0] as usize) {
        hasher.update(salt);
    }
    let ds = hasher.finalize();

    let mut s_bytes = Vec::with_capacity(salt_len);
    for _ in 0..(salt_len / BLOCK_SIZE) {
        s_bytes.extend_from_slice(&ds);
    }
    s_bytes.extend_from_slice(&ds[..(salt_len % BLOCK_SIZE)]);

    // Main rounds loop
    let mut digest_c = digest_a;
    for i in 0..rounds {
        let mut hasher = Sha256::new();

        if (i & 1) != 0 {
            hasher.update(&p_bytes);
        } else {
            hasher.update(&digest_c);
        }

        if i % 3 != 0 {
            hasher.update(&s_bytes);
        }

        if i % 7 != 0 {
            hasher.update(&p_bytes);
        }

        if (i & 1) != 0 {
            hasher.update(&digest_c);
        } else {
            hasher.update(&p_bytes);
        }

        digest_c = hasher.finalize();
    }

    sha256_b64_encode(&digest_c)
}

// Custom base64 encoding for crypt (different alphabet than standard base64)
const B64_CHARS: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn b64_encode_3(b0: u8, b1: u8, b2: u8, n: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(n);
    let mut val = (b0 as u32) | ((b1 as u32) << 8) | ((b2 as u32) << 16);
    for _ in 0..n {
        result.push(B64_CHARS[(val & 0x3f) as usize]);
        val >>= 6;
    }
    result
}

fn sha512_b64_encode(digest: &[u8; 64]) -> Vec<u8> {
    let mut result = Vec::with_capacity(86);
    // SHA-512 crypt uses a specific byte permutation based on MAP_SHA512
    // Order: [42,21,0], [1,43,22], [23,2,44], [45,24,3], [4,46,25], [26,5,47],
    //        [48,27,6], [7,49,28], [29,8,50], [51,30,9], [10,52,31], [32,11,53],
    //        [54,33,12], [13,55,34], [35,14,56], [57,36,15], [16,58,37], [38,17,59],
    //        [60,39,18], [19,61,40], [41,20,62], [63]
    result.extend(b64_encode_3(digest[42], digest[21], digest[0], 4));
    result.extend(b64_encode_3(digest[1], digest[43], digest[22], 4));
    result.extend(b64_encode_3(digest[23], digest[2], digest[44], 4));
    result.extend(b64_encode_3(digest[45], digest[24], digest[3], 4));
    result.extend(b64_encode_3(digest[4], digest[46], digest[25], 4));
    result.extend(b64_encode_3(digest[26], digest[5], digest[47], 4));
    result.extend(b64_encode_3(digest[48], digest[27], digest[6], 4));
    result.extend(b64_encode_3(digest[7], digest[49], digest[28], 4));
    result.extend(b64_encode_3(digest[29], digest[8], digest[50], 4));
    result.extend(b64_encode_3(digest[51], digest[30], digest[9], 4));
    result.extend(b64_encode_3(digest[10], digest[52], digest[31], 4));
    result.extend(b64_encode_3(digest[32], digest[11], digest[53], 4));
    result.extend(b64_encode_3(digest[54], digest[33], digest[12], 4));
    result.extend(b64_encode_3(digest[13], digest[55], digest[34], 4));
    result.extend(b64_encode_3(digest[35], digest[14], digest[56], 4));
    result.extend(b64_encode_3(digest[57], digest[36], digest[15], 4));
    result.extend(b64_encode_3(digest[16], digest[58], digest[37], 4));
    result.extend(b64_encode_3(digest[38], digest[17], digest[59], 4));
    result.extend(b64_encode_3(digest[60], digest[39], digest[18], 4));
    result.extend(b64_encode_3(digest[19], digest[61], digest[40], 4));
    result.extend(b64_encode_3(digest[41], digest[20], digest[62], 4));
    result.extend(b64_encode_3(digest[63], 0, 0, 2));
    result
}

fn sha256_b64_encode(digest: &[u8; 32]) -> Vec<u8> {
    let mut result = Vec::with_capacity(43);
    // SHA-256 crypt uses a specific byte permutation based on MAP_SHA256
    // Order: [20,10,0], [11,1,21], [2,22,12], [23,13,3], [14,4,24], [5,25,15],
    //        [26,16,6], [17,7,27], [8,28,18], [29,19,9], [30,31]
    result.extend(b64_encode_3(digest[20], digest[10], digest[0], 4));
    result.extend(b64_encode_3(digest[11], digest[1], digest[21], 4));
    result.extend(b64_encode_3(digest[2], digest[22], digest[12], 4));
    result.extend(b64_encode_3(digest[23], digest[13], digest[3], 4));
    result.extend(b64_encode_3(digest[14], digest[4], digest[24], 4));
    result.extend(b64_encode_3(digest[5], digest[25], digest[15], 4));
    result.extend(b64_encode_3(digest[26], digest[16], digest[6], 4));
    result.extend(b64_encode_3(digest[17], digest[7], digest[27], 4));
    result.extend(b64_encode_3(digest[8], digest[28], digest[18], 4));
    result.extend(b64_encode_3(digest[29], digest[19], digest[9], 4));
    result.extend(b64_encode_3(digest[30], digest[31], 0, 3));
    result
}

// SHA-512 implementation
struct Sha512 {
    state: [u64; 8],
    count: u128,
    buffer: [u8; 128],
    buffer_len: usize,
}

impl Sha512 {
    const K: [u64; 80] = [
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

    fn new() -> Self {
        Self {
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
            buffer_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        let mut i = 0;
        self.count += data.len() as u128;

        if self.buffer_len > 0 {
            let space = 128 - self.buffer_len;
            let take = data.len().min(space);
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            i = take;

            if self.buffer_len == 128 {
                let block = self.buffer;
                self.transform(&block);
                self.buffer_len = 0;
            }
        }

        while i + 128 <= data.len() {
            let mut block = [0u8; 128];
            block.copy_from_slice(&data[i..i + 128]);
            self.transform(&block);
            i += 128;
        }

        if i < data.len() {
            self.buffer_len = data.len() - i;
            self.buffer[..self.buffer_len].copy_from_slice(&data[i..]);
        }
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
                .wrapping_add(Self::K[i])
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

    fn finalize(mut self) -> [u8; 64] {
        let bit_len = self.count * 8;

        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 112 {
            while self.buffer_len < 128 {
                self.buffer[self.buffer_len] = 0;
                self.buffer_len += 1;
            }
            let block = self.buffer;
            self.transform(&block);
            self.buffer_len = 0;
        }

        while self.buffer_len < 112 {
            self.buffer[self.buffer_len] = 0;
            self.buffer_len += 1;
        }

        let len_bytes = bit_len.to_be_bytes();
        self.buffer[112..128].copy_from_slice(&len_bytes);
        let block = self.buffer;
        self.transform(&block);

        let mut result = [0u8; 64];
        for (i, &val) in self.state.iter().enumerate() {
            result[i * 8..(i + 1) * 8].copy_from_slice(&val.to_be_bytes());
        }
        result
    }
}

// SHA-256 implementation
struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
    buffer_len: usize,
}

impl Sha256 {
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

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            count: 0,
            buffer: [0; 64],
            buffer_len: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        let mut i = 0;
        self.count += data.len() as u64;

        if self.buffer_len > 0 {
            let space = 64 - self.buffer_len;
            let take = data.len().min(space);
            self.buffer[self.buffer_len..self.buffer_len + take].copy_from_slice(&data[..take]);
            self.buffer_len += take;
            i = take;

            if self.buffer_len == 64 {
                let block = self.buffer;
                self.transform(&block);
                self.buffer_len = 0;
            }
        }

        while i + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[i..i + 64]);
            self.transform(&block);
            i += 64;
        }

        if i < data.len() {
            self.buffer_len = data.len() - i;
            self.buffer[..self.buffer_len].copy_from_slice(&data[i..]);
        }
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
                .wrapping_add(Self::K[i])
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

    fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.count * 8;

        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        if self.buffer_len > 56 {
            while self.buffer_len < 64 {
                self.buffer[self.buffer_len] = 0;
                self.buffer_len += 1;
            }
            let block = self.buffer;
            self.transform(&block);
            self.buffer_len = 0;
        }

        while self.buffer_len < 56 {
            self.buffer[self.buffer_len] = 0;
            self.buffer_len += 1;
        }

        let len_bytes = bit_len.to_be_bytes();
        self.buffer[56..64].copy_from_slice(&len_bytes);
        let block = self.buffer;
        self.transform(&block);

        let mut result = [0u8; 32];
        for (i, &val) in self.state.iter().enumerate() {
            result[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
        }
        result
    }
}

fn is_nologin_shell(shell: &[u8]) -> bool {
    shell.ends_with(b"/nologin") || shell.ends_with(b"/false")
}

pub(crate) fn read_line() -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut c = [0u8; 1];

    loop {
        let n = io::read(0, &mut c);
        if n <= 0 {
            if buf.is_empty() {
                return None;
            }
            break;
        }
        if c[0] == b'\n' {
            break;
        }
        if c[0] == b'\r' {
            continue;
        }
        buf.push(c[0]);
    }

    Some(buf)
}

pub(crate) fn disable_echo() -> libc::termios {
    let mut old: libc::termios = unsafe { core::mem::zeroed() };
    let mut new: libc::termios = unsafe { core::mem::zeroed() };

    unsafe {
        libc::tcgetattr(0, &mut old);
        new = old;
        new.c_lflag &= !(libc::ECHO);
        libc::tcsetattr(0, libc::TCSANOW, &new);
    }

    old
}

pub(crate) fn restore_termios(termios: &libc::termios) {
    unsafe {
        libc::tcsetattr(0, libc::TCSANOW, termios);
    }
}

fn delay(seconds: u32) {
    unsafe {
        libc::sleep(seconds);
    }
}

fn make_cstr(s: &[u8]) -> Vec<u8> {
    let mut cstr = Vec::from(s);
    cstr.push(0);
    cstr
}

fn set_env(name: &[u8], value: &[u8]) {
    let name_cstr = make_cstr(name);
    let value_cstr = make_cstr(value);
    unsafe {
        libc::setenv(
            name_cstr.as_ptr() as *const libc::c_char,
            value_cstr.as_ptr() as *const libc::c_char,
            1,
        );
    }
}

fn get_env(name: &[u8]) -> Result<Vec<u8>, ()> {
    let name_cstr = make_cstr(name);
    let ptr = unsafe { libc::getenv(name_cstr.as_ptr() as *const libc::c_char) };
    if ptr.is_null() {
        return Err(());
    }
    let len = unsafe { libc::strlen(ptr) };
    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    Ok(bytes.to_vec())
}

fn print_usage() {
    io::write_str(1, b"Usage: login [-f] [-h HOST] [-p] [username]\n\n");
    io::write_str(1, b"Begin a session on the system.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -f        Skip authentication (requires root)\n");
    io::write_str(1, b"  -h HOST   Remote host name\n");
    io::write_str(1, b"  -p        Preserve environment\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn login(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"login: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_login_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["login", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_sha512_basic() {
        // Verify SHA-512 produces correct output
        use super::Sha512;

        let mut hasher = Sha512::new();
        hasher.update(b"abc");
        let result = hasher.finalize();

        let hex: std::string::String = result.iter().map(|b| std::format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn test_sha256_basic() {
        // Verify SHA-256 produces correct output
        use super::Sha256;

        let mut hasher = Sha256::new();
        hasher.update(b"abc");
        let result = hasher.finalize();

        let hex: std::string::String = result.iter().map(|b| std::format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_sha512_crypt() {
        use super::verify_password;

        // Test vector: password "test" with salt "saltsalt"
        // Generated with: perl -e 'print crypt("test", "\$6\$saltsalt\$") . "\n"'
        let hash = b"$6$saltsalt$JcVDtuB6d1BHhCd5RPBh8g8xX/1CbY8EU2PN0MTaj2/Mypw4P./C6dN4j0HALhzBDTocyW1Jm.gYaTPjFGCV40";

        // verify_password should return true
        assert!(verify_password(b"test", hash));

        // Wrong password should fail
        assert!(!verify_password(b"wrong", hash));
    }

    #[test]
    fn test_sha256_crypt() {
        use super::verify_password;

        // Test vector for SHA-256 ($5$)
        // Generated with: perl -e 'print crypt("test", "\$5\$saltsalt\$") . "\n"'
        let hash = b"$5$saltsalt$q6kNPzk9GPb3dpYKO2TUS45z2kiVxsUVz7eWVJcih.0";

        // verify_password should return true
        assert!(verify_password(b"test", hash));

        // Wrong password should fail
        assert!(!verify_password(b"wrong", hash));
    }

    #[test]
    fn test_locked_accounts() {
        use super::verify_password;

        // Locked accounts should always fail
        assert!(!verify_password(b"anything", b"*"));
        assert!(!verify_password(b"anything", b"!"));
        assert!(!verify_password(b"anything", b"!!"));
        assert!(!verify_password(b"anything", b"x"));
    }
}
