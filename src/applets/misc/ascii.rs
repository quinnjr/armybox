//! ascii - display ASCII character table
//!
//! Shows the ASCII table with character codes and names.

use crate::io;
use crate::sys;

/// ascii - display ASCII table
///
/// # Synopsis
/// ```text
/// ascii
/// ```
///
/// # Description
/// Displays an ASCII table showing decimal, hexadecimal values and
/// character representations for all 128 ASCII characters.
///
/// # Exit Status
/// - 0: Success
pub fn ascii(_argc: i32, _argv: *const *const u8) -> i32 {
    // Control character names
    const CTRL_NAMES: [&[u8]; 32] = [
        b"NUL", b"SOH", b"STX", b"ETX", b"EOT", b"ENQ", b"ACK", b"BEL", b"BS ", b"HT ", b"LF ",
        b"VT ", b"FF ", b"CR ", b"SO ", b"SI ", b"DLE", b"DC1", b"DC2", b"DC3", b"DC4", b"NAK",
        b"SYN", b"ETB", b"CAN", b"EM ", b"SUB", b"ESC", b"FS ", b"GS ", b"RS ", b"US ",
    ];

    io::write_str(
        1,
        b"Dec Hex    Dec Hex    Dec Hex  Dec Hex  Dec Hex  Dec Hex   Dec Hex   Dec Hex\n",
    );

    for row in 0..16 {
        for col in 0..8 {
            let val = row + col * 16;
            let mut buf = [0u8; 16];

            // Format decimal (right-aligned in 3 chars)
            if val < 10 {
                io::write_str(1, b"  ");
            } else if val < 100 {
                io::write_str(1, b" ");
            }
            let dec = sys::format_u64(val as u64, &mut buf);
            io::write_all(1, dec);
            io::write_str(1, b" ");

            // Format hex
            let hex = sys::format_hex(val as u64, &mut buf);
            if hex.len() < 2 {
                io::write_str(1, b"0");
            }
            io::write_all(1, hex);
            io::write_str(1, b" ");

            // Format character
            if val < 32 {
                io::write_all(1, CTRL_NAMES[val]);
            } else if val == 32 {
                io::write_str(1, b"   ");
            } else if val == 127 {
                io::write_str(1, b"DEL");
            } else {
                io::write_all(1, &[val as u8, b' ', b' ']);
            }

            if col < 7 {
                io::write_str(1, b" ");
            }
        }
        io::write_str(1, b"\n");
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
    fn test_ascii() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["ascii"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("NUL"));
        assert!(stdout.contains("DEL"));
    }
}
