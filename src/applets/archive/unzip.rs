//! unzip - extract files from ZIP archives
//!
//! List and extract compressed files from ZIP archives.

use super::gzip::inflate;
use super::{create_parent_dirs, get_arg, open_read, open_write_create};
use crate::io;
use alloc::vec::Vec;

/// Returns `true` if `name` is unsafe to use as an extraction path:
/// an absolute path, or a path containing a `..` component.
///
/// Detection is component-based (splitting on `/`), not a substring
/// search, so it correctly rejects `../x`, `a/../b`, `a/..`, and `..`.
fn is_unsafe_path(name: &[u8]) -> bool {
    if name.starts_with(b"/") {
        return true;
    }
    let mut start = 0;
    for i in 0..=name.len() {
        if i == name.len() || name[i] == b'/' {
            if &name[start..i] == b".." {
                return true;
            }
            start = i + 1;
        }
    }
    false
}

/// unzip - extract files from ZIP archives
///
/// # Synopsis
/// ```text
/// unzip [-l] ARCHIVE
/// ```
///
/// # Description
/// Extract files from ZIP archives.
///
/// # Options
/// - `-l`: List archive contents
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn unzip(argc: i32, argv: *const *const u8) -> i32 {
    let mut list_only = false;
    let mut archive: Option<&[u8]> = None;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-l" {
                list_only = true;
            } else if !arg.starts_with(b"-") && archive.is_none() {
                archive = Some(arg);
            }
        }
    }

    let archive = match archive {
        Some(a) => a,
        None => {
            io::write_str(2, b"unzip: specify archive\n");
            return 1;
        }
    };

    let fd = open_read(archive);
    if fd < 0 {
        io::write_str(2, b"unzip: cannot open archive\n");
        return 1;
    }

    // Read entire archive into memory (simplified approach)
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }
        data.extend_from_slice(&buf[..n as usize]);
    }
    io::close(fd);

    // Process ZIP local file headers
    let mut offset = 0usize;
    let mut exit_code = 0;

    while offset + 30 <= data.len() {
        // Check for local file header signature
        if &data[offset..offset + 4] != &[0x50, 0x4b, 0x03, 0x04] {
            // Might be central directory, stop
            break;
        }

        let compression = u16::from_le_bytes([data[offset + 8], data[offset + 9]]);
        let compressed_size = u32::from_le_bytes([
            data[offset + 18],
            data[offset + 19],
            data[offset + 20],
            data[offset + 21],
        ]) as usize;
        let uncompressed_size = u32::from_le_bytes([
            data[offset + 22],
            data[offset + 23],
            data[offset + 24],
            data[offset + 25],
        ]) as usize;
        let filename_len = u16::from_le_bytes([data[offset + 26], data[offset + 27]]) as usize;
        let extra_len = u16::from_le_bytes([data[offset + 28], data[offset + 29]]) as usize;

        let filename_start = offset + 30;
        let filename_end = filename_start + filename_len;
        let data_start = filename_end + extra_len;
        let data_end = data_start + compressed_size;

        if data_end > data.len() {
            break;
        }

        let filename = &data[filename_start..filename_end];

        if list_only {
            io::write_num(1, uncompressed_size as u64);
            io::write_str(1, b"  ");
            io::write_all(1, filename);
            io::write_str(1, b"\n");
        } else if is_unsafe_path(filename) {
            io::write_str(2, b"unzip: skipping unsafe path\n");
            exit_code = 1;
        } else if filename.len() >= io::PATH_MAX {
            io::write_str(2, b"unzip: path too long, skipping\n");
            exit_code = 1;
        } else {
            io::write_str(1, b"  inflating: ");
            io::write_all(1, filename);
            io::write_str(1, b"\n");

            // Extract file
            if !filename.ends_with(b"/") {
                if !create_parent_dirs(filename) {
                    io::write_str(2, b"unzip: path too long, skipping\n");
                    exit_code = 1;
                } else {
                    let out_fd = open_write_create(filename, 0o644);
                    if out_fd >= 0 {
                        let compressed_data = &data[data_start..data_end];

                        match compression {
                            0 => {
                                // Stored (no compression)
                                io::write_all(out_fd, compressed_data);
                            }
                            8 => {
                                // DEFLATE
                                match inflate(compressed_data) {
                                    Ok(decompressed) => {
                                        io::write_all(out_fd, &decompressed);
                                    }
                                    Err(()) => {
                                        io::write_str(2, b"unzip: invalid compressed data\n");
                                        exit_code = 1;
                                    }
                                }
                            }
                            _ => {
                                io::write_str(2, b"unzip: unsupported compression\n");
                            }
                        }
                        io::close(out_fd);
                    } else {
                        exit_code = 1;
                    }
                }
            } else {
                // Directory
                io::mkdir(filename, 0o755);
            }
        }

        offset = data_end;
    }

    exit_code
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
    fn test_unzip_no_archive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["unzip"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("specify archive"));
    }

    #[test]
    fn test_unzip_cannot_open() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["unzip", "/nonexistent/file.zip"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cannot open"));
    }
}
