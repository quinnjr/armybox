//! cpio - copy files to and from archives
//!
//! Copy-in-copy-out archive format.

use alloc::vec::Vec;
use alloc::vec;
use crate::io;
use super::{get_arg, open_read, open_write_create, create_parent_dirs, mode_string};

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

/// cpio - copy files to and from archives
///
/// # Synopsis
/// ```text
/// cpio [-iotv]
/// ```
///
/// # Description
/// Copy files to/from cpio archives.
///
/// # Options
/// - `-i`: Extract (copy-in)
/// - `-o`: Create (copy-out)
/// - `-t`: List contents
/// - `-v`: Verbose
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn cpio(argc: i32, argv: *const *const u8) -> i32 {
    let mut mode = 0u8; // 'i' = extract, 'o' = create, 't' = list
    let mut verbose = false;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") {
                for &c in &arg[1..] {
                    match c {
                        b'i' => mode = b'i',
                        b'o' => mode = b'o',
                        b't' => mode = b't',
                        b'v' => verbose = true,
                        _ => {}
                    }
                }
            }
        }
    }

    match mode {
        b'i' => cpio_extract(verbose),
        b'o' => cpio_create(verbose),
        b't' => cpio_list(verbose),
        _ => {
            io::write_str(2, b"cpio: specify -i, -o, or -t\n");
            1
        }
    }
}

fn parse_hex_8(bytes: &[u8]) -> u32 {
    let mut result = 0u32;
    for &b in bytes {
        result <<= 4;
        if b >= b'0' && b <= b'9' {
            result |= (b - b'0') as u32;
        } else if b >= b'a' && b <= b'f' {
            result |= (b - b'a' + 10) as u32;
        } else if b >= b'A' && b <= b'F' {
            result |= (b - b'A' + 10) as u32;
        }
    }
    result
}

fn write_hex_8(buf: &mut [u8], val: u32) {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut v = val;
    for i in (0..8).rev() {
        buf[i] = HEX[(v & 0xf) as usize];
        v >>= 4;
    }
}

fn cpio_extract(verbose: bool) -> i32 {
    let mut header_buf = [0u8; 110];
    let mut exit_code = 0;

    loop {
        if io::read(0, &mut header_buf) != 110 {
            break;
        }

        // Check magic
        if &header_buf[0..6] != b"070701" && &header_buf[0..6] != b"070702" {
            io::write_str(2, b"cpio: bad magic\n");
            return 1;
        }

        let mode = parse_hex_8(&header_buf[14..22]);
        let filesize = parse_hex_8(&header_buf[54..62]) as usize;
        let namesize = parse_hex_8(&header_buf[94..102]) as usize;

        // Read filename
        let mut name = vec![0u8; namesize];
        io::read(0, &mut name);

        // Skip padding to 4-byte boundary
        let header_total = 110 + namesize;
        let padding = (4 - (header_total % 4)) % 4;
        let mut skip = [0u8; 4];
        if padding > 0 {
            io::read(0, &mut skip[..padding]);
        }

        // Remove null terminator from name
        let name_str = if namesize > 0 && name[namesize - 1] == 0 {
            &name[..namesize - 1]
        } else {
            &name[..]
        };

        // Check for trailer
        if name_str == b"TRAILER!!!" {
            break;
        }

        if verbose {
            io::write_all(1, name_str);
            io::write_str(1, b"\n");
        }

        let is_dir = (mode & 0o170000) == 0o040000;
        let is_file = (mode & 0o170000) == 0o100000;

        if is_unsafe_path(name_str) {
            io::write_str(2, b"cpio: skipping unsafe path\n");
            exit_code = 1;
            // Skip content (if any) to stay aligned with the stream
            let mut remaining = filesize;
            let mut buf = [0u8; 4096];
            while remaining > 0 {
                let to_read = remaining.min(4096);
                io::read(0, &mut buf[..to_read]);
                remaining -= to_read;
            }
        } else if name_str.len() >= io::PATH_MAX {
            io::write_str(2, b"cpio: path too long, skipping\n");
            exit_code = 1;
            let mut remaining = filesize;
            let mut buf = [0u8; 4096];
            while remaining > 0 {
                let to_read = remaining.min(4096);
                io::read(0, &mut buf[..to_read]);
                remaining -= to_read;
            }
        } else if is_dir {
            // Create directory
            io::mkdir(name_str, (mode & 0o7777) as u32);
        } else if is_file {
            // Create file
            if !create_parent_dirs(name_str) {
                io::write_str(2, b"cpio: path too long, skipping\n");
                exit_code = 1;
            }

            let fd = open_write_create(name_str, (mode & 0o7777) as i32);
            if fd >= 0 {
                let mut remaining = filesize;
                let mut buf = [0u8; 4096];
                while remaining > 0 {
                    let to_read = remaining.min(4096);
                    let n = io::read(0, &mut buf[..to_read]);
                    if n <= 0 { break; }
                    io::write_all(fd, &buf[..n as usize]);
                    remaining -= n as usize;
                }
                io::close(fd);
            } else {
                exit_code = 1;
                // Skip file content
                let mut remaining = filesize;
                let mut buf = [0u8; 4096];
                while remaining > 0 {
                    let to_read = remaining.min(4096);
                    io::read(0, &mut buf[..to_read]);
                    remaining -= to_read;
                }
            }
        } else {
            // Skip unknown types
            let mut remaining = filesize;
            let mut buf = [0u8; 4096];
            while remaining > 0 {
                let to_read = remaining.min(4096);
                io::read(0, &mut buf[..to_read]);
                remaining -= to_read;
            }
        }

        // Skip data padding
        let data_padding = (4 - (filesize % 4)) % 4;
        if data_padding > 0 {
            io::read(0, &mut skip[..data_padding]);
        }
    }

    exit_code
}

fn cpio_create(verbose: bool) -> i32 {
    // Read filenames from stdin, write cpio to stdout
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    let mut ino = 0u32;

    loop {
        line.clear();
        loop {
            if io::read(0, &mut byte) != 1 {
                if line.is_empty() {
                    // Write trailer
                    cpio_write_entry(b"TRAILER!!!", 0, 0, 0, &[], verbose);
                    return 0;
                }
                break;
            }
            if byte[0] == b'\n' { break; }
            line.push(byte[0]);
        }

        if line.is_empty() { continue; }

        // Stat the file
        let mut stat_buf: libc::stat = unsafe { core::mem::zeroed() };
        if io::lstat(&line, &mut stat_buf) != 0 {
            io::write_str(2, b"cpio: cannot stat ");
            io::write_all(2, &line);
            io::write_str(2, b"\n");
            continue;
        }

        ino += 1;
        let mode = stat_buf.st_mode as u32;
        let is_file = (mode & libc::S_IFMT as u32) == libc::S_IFREG as u32;

        if is_file {
            // Read file content
            let fd = open_read(&line);
            if fd >= 0 {
                let mut content = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = io::read(fd, &mut buf);
                    if n <= 0 { break; }
                    content.extend_from_slice(&buf[..n as usize]);
                }
                io::close(fd);
                cpio_write_entry(&line, ino, mode as u32, stat_buf.st_mtime as u32, &content, verbose);
            }
        } else {
            cpio_write_entry(&line, ino, mode as u32, stat_buf.st_mtime as u32, &[], verbose);
        }
    }
}

fn cpio_write_entry(name: &[u8], ino: u32, mode: u32, mtime: u32, data: &[u8], verbose: bool) {
    let mut header = [0u8; 110];

    // Magic
    header[0..6].copy_from_slice(b"070701");

    // ino
    write_hex_8(&mut header[6..14], ino);

    // mode
    write_hex_8(&mut header[14..22], mode);

    // uid, gid
    write_hex_8(&mut header[22..30], 0);
    write_hex_8(&mut header[30..38], 0);

    // nlink
    write_hex_8(&mut header[38..46], 1);

    // mtime
    write_hex_8(&mut header[46..54], mtime);

    // filesize
    write_hex_8(&mut header[54..62], data.len() as u32);

    // devmajor, devminor, rdevmajor, rdevminor
    write_hex_8(&mut header[62..70], 0);
    write_hex_8(&mut header[70..78], 0);
    write_hex_8(&mut header[78..86], 0);
    write_hex_8(&mut header[86..94], 0);

    // namesize (including null terminator)
    write_hex_8(&mut header[94..102], (name.len() + 1) as u32);

    // check
    write_hex_8(&mut header[102..110], 0);

    // Write header
    io::write_all(1, &header);

    // Write name with null terminator
    io::write_all(1, name);
    io::write_all(1, &[0]);

    // Padding to 4-byte boundary
    let header_total = 110 + name.len() + 1;
    let padding = (4 - (header_total % 4)) % 4;
    for _ in 0..padding {
        io::write_all(1, &[0]);
    }

    // Write data
    if !data.is_empty() {
        io::write_all(1, data);

        // Data padding
        let data_padding = (4 - (data.len() % 4)) % 4;
        for _ in 0..data_padding {
            io::write_all(1, &[0]);
        }
    }

    if verbose {
        io::write_all(2, name);
        io::write_str(2, b"\n");
    }
}

fn cpio_list(verbose: bool) -> i32 {
    let mut header_buf = [0u8; 110];

    loop {
        if io::read(0, &mut header_buf) != 110 {
            break;
        }

        if &header_buf[0..6] != b"070701" && &header_buf[0..6] != b"070702" {
            io::write_str(2, b"cpio: bad magic\n");
            return 1;
        }

        let mode = parse_hex_8(&header_buf[14..22]);
        let filesize = parse_hex_8(&header_buf[54..62]) as usize;
        let namesize = parse_hex_8(&header_buf[94..102]) as usize;

        let mut name = vec![0u8; namesize];
        io::read(0, &mut name);

        let header_total = 110 + namesize;
        let padding = (4 - (header_total % 4)) % 4;
        let mut skip = [0u8; 4];
        if padding > 0 {
            io::read(0, &mut skip[..padding]);
        }

        let name_str = if namesize > 0 && name[namesize - 1] == 0 {
            &name[..namesize - 1]
        } else {
            &name[..]
        };

        if name_str == b"TRAILER!!!" {
            break;
        }

        if verbose {
            // Print mode in ls -l format
            let type_char = match mode & 0o170000 {
                0o040000 => b'd',
                0o120000 => b'l',
                _ => b'-',
            };
            io::write_all(1, &[type_char]);
            io::write_all(1, &mode_string((mode & 0o7777) as u32));
            io::write_str(1, b" ");
            io::write_num(1, filesize as u64);
            io::write_str(1, b" ");
        }
        io::write_all(1, name_str);
        io::write_str(1, b"\n");

        // Skip data
        let mut remaining = filesize;
        let mut buf = [0u8; 4096];
        while remaining > 0 {
            let to_read = remaining.min(4096);
            io::read(0, &mut buf[..to_read]);
            remaining -= to_read;
        }

        let data_padding = (4 - (filesize % 4)) % 4;
        if data_padding > 0 {
            io::read(0, &mut skip[..data_padding]);
        }
    }

    0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::process::Command;
    use std::path::PathBuf;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_cpio_no_mode() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["cpio"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("specify -i, -o, or -t"));
    }

    #[test]
    fn test_cpio_bad_magic() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new(&armybox)
            .args(["cpio", "-i"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        {
            let stdin = child.stdin.as_mut().unwrap();
            // Send invalid cpio header
            stdin.write_all(&[0u8; 110]).unwrap();
        }

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("bad magic"));
    }
}
