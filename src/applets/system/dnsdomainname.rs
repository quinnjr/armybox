//! dnsdomainname - show the system's DNS domain name
//!
//! Display the DNS domain name of the system.

use crate::io;

/// dnsdomainname - show the system's DNS domain name
///
/// # Synopsis
/// ```text
/// dnsdomainname
/// ```
///
/// # Description
/// Display the DNS domain name of the system by parsing the hostname.
///
/// # Exit Status
/// - 0: Success
pub fn dnsdomainname(_argc: i32, _argv: *const *const u8) -> i32 {
    let mut buf = [0u8; 256];

    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } != 0 {
        return 1;
    }

    // Find the null terminator
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());

    // Find the first dot - domain name starts after it
    if let Some(dot_pos) = buf[..len].iter().position(|&c| c == b'.') {
        if dot_pos + 1 < len {
            io::write_all(1, &buf[dot_pos + 1..len]);
            io::write_str(1, b"\n");
        }
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

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
    fn test_dnsdomainname_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["dnsdomainname"])
            .output()
            .unwrap();

        // Should succeed even if no domain is set
        assert_eq!(output.status.code(), Some(0));
    }
}
