//! mkpasswd - generate password hash
//!
//! Generates a password hash in various formats.

use super::get_arg;
use crate::io;

/// mkpasswd - generate password hash
///
/// # Synopsis
/// ```text
/// mkpasswd [-m method] [-S salt] [password]
/// ```
///
/// # Description
/// Generates a password hash. Currently outputs a simple hash format.
/// For production use, use the system's mkpasswd or openssl.
///
/// # Options
/// - `-m, --method`: Hash method (md5, sha256, sha512)
/// - `-S, --salt`: Salt to use
///
/// # Exit Status
/// - 0: Success
pub fn mkpasswd(argc: i32, argv: *const *const u8) -> i32 {
    let mut method = b"sha512" as &[u8];
    let mut salt: Option<&[u8]> = None;
    let mut password: Option<&[u8]> = None;
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-m" || arg == b"--method" {
            i += 1;
            if let Some(m) = unsafe { get_arg(argv, i as i32) } {
                method = m;
            }
        } else if arg == b"-S" || arg == b"--salt" {
            i += 1;
            salt = unsafe { get_arg(argv, i as i32) };
        } else if !arg.starts_with(b"-") {
            password = Some(arg);
        }
        i += 1;
    }

    let salt_str = salt.unwrap_or(b"randomsalt");
    let pwd = password.unwrap_or(b"password");

    // Generate a simple hash (not cryptographically secure - for demo purposes)
    // In real usage, users should use system mkpasswd or openssl passwd
    let prefix: &[u8] = match method {
        b"md5" | b"1" => b"$1$",
        b"sha256" | b"5" => b"$5$",
        b"sha512" | b"6" => b"$6$",
        _ => b"$6$",
    };

    // Simple hash: XOR password bytes with salt, then format as hex
    let mut hash: u64 = 0x5381; // djb2 seed
    for &b in pwd {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    for &b in salt_str {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }

    io::write_all(1, prefix);
    io::write_all(1, salt_str);
    io::write_str(1, b"$");

    // Output hash as base64-ish characters (like crypt output)
    const B64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut hash_chars = [0u8; 86];
    let mut h = hash;
    for i in 0..86 {
        hash_chars[i] = B64[(h & 0x3F) as usize];
        h = h.wrapping_mul(0x5DEECE66D).wrapping_add(11);
    }
    io::write_all(1, &hash_chars);
    io::write_str(1, b"\n");

    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_mkpasswd() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["mkpasswd", "mypassword"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.starts_with("$6$")); // Default sha512
    }
}
