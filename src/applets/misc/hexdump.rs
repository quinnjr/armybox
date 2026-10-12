//! hexdump - display file in hexadecimal
//!
//! Display file contents in hex format.

use super::get_arg;
use crate::io;
use crate::sys;

/// hexdump - display file in hex
///
/// # Synopsis
/// ```text
/// hexdump [FILE]
/// ```
///
/// # Description
/// Display file contents in hexadecimal format.
///
/// # Exit Status
/// - 0: Success
pub fn hexdump(argc: i32, argv: *const *const u8) -> i32 {
    let fd = if argc > 1 {
        if let Some(path) = unsafe { get_arg(argv, argc - 1) } {
            if path[0] != b'-' {
                io::open(path, libc::O_RDONLY, 0)
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let mut buf = [0u8; 16];
    let mut offset = 0u64;

    loop {
        let n = io::read(fd, &mut buf);
        if n <= 0 {
            break;
        }

        // Print offset
        let mut hex = [0u8; 16];
        let s = sys::format_hex(offset, &mut hex);
        for _ in 0..(8 - s.len()) {
            io::write_str(1, b"0");
        }
        io::write_all(1, s);
        io::write_str(1, b"  ");

        // Print hex
        for i in 0..n as usize {
            let h = sys::format_hex(buf[i] as u64, &mut hex);
            if h.len() == 1 {
                io::write_str(1, b"0");
            }
            io::write_all(1, h);
            io::write_str(1, b" ");
        }
        io::write_str(1, b"\n");

        offset += n as u64;
    }

    if fd != 0 {
        io::close(fd);
    }
    0
}

/// od - octal dump (POSIX compliant)
///
/// Usage: od [-A radix] [-t type_string] [-j skip] [-N count] [file...]
/// Dumps files in octal (default) or other formats.
#[cfg(feature = "alloc")]
pub fn od(argc: i32, argv: *const *const u8) -> i32 {
    use alloc::vec::Vec;

    // Output format types
    #[derive(Clone, Copy)]
    enum Format {
        Octal1,    // -t o1 - octal bytes
        Octal2,    // -t o2 - octal 2-byte words
        Hex1,      // -t x1 - hex bytes
        Hex2,      // -t x2 - hex 2-byte words
        Decimal1,  // -t d1 - signed decimal bytes
        Decimal2,  // -t d2 - signed decimal 2-byte words
        Unsigned1, // -t u1 - unsigned decimal bytes
        Char,      // -t c - characters
    }

    #[derive(Clone, Copy)]
    enum AddressRadix {
        Octal,
        Hex,
        Decimal,
        None,
    }

    let mut format = Format::Octal2; // Default: octal 2-byte words
    let mut address_radix = AddressRadix::Octal;
    let mut skip: usize = 0;
    let mut count: Option<usize> = None;
    let mut files: Vec<&[u8]> = Vec::new();
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { super::get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-A" {
            i += 1;
            if let Some(r) = unsafe { super::get_arg(argv, i as i32) } {
                address_radix = match r {
                    b"o" => AddressRadix::Octal,
                    b"x" => AddressRadix::Hex,
                    b"d" => AddressRadix::Decimal,
                    b"n" => AddressRadix::None,
                    _ => AddressRadix::Octal,
                };
            }
        } else if arg == b"-t" {
            i += 1;
            if let Some(t) = unsafe { super::get_arg(argv, i as i32) } {
                format = match t {
                    b"o1" | b"o" => Format::Octal1,
                    b"o2" => Format::Octal2,
                    b"x1" | b"x" => Format::Hex1,
                    b"x2" => Format::Hex2,
                    b"d1" | b"d" => Format::Decimal1,
                    b"d2" => Format::Decimal2,
                    b"u1" | b"u" => Format::Unsigned1,
                    b"c" => Format::Char,
                    _ => Format::Octal2,
                };
            }
        } else if arg == b"-j" {
            i += 1;
            if let Some(s) = unsafe { super::get_arg(argv, i as i32) } {
                skip = crate::sys::parse_u64(s).unwrap_or(0) as usize;
            }
        } else if arg == b"-N" {
            i += 1;
            if let Some(n) = unsafe { super::get_arg(argv, i as i32) } {
                count = Some(crate::sys::parse_u64(n).unwrap_or(0) as usize);
            }
        } else if arg == b"-b" {
            format = Format::Octal1;
        } else if arg == b"-c" {
            format = Format::Char;
        } else if arg == b"-d" {
            format = Format::Unsigned1;
        } else if arg == b"-o" {
            format = Format::Octal2;
        } else if arg == b"-x" {
            format = Format::Hex2;
        } else if !arg.starts_with(b"-") {
            files.push(arg);
        }
        i += 1;
    }

    // Read input
    let content = if files.is_empty() {
        io::read_all(0)
    } else {
        let mut data: Vec<u8> = Vec::new();
        for file in &files {
            let fd = io::open(file, libc::O_RDONLY, 0);
            if fd < 0 {
                io::write_str(2, b"od: ");
                io::write_all(2, file);
                io::write_str(2, b": No such file\n");
                return 1;
            }
            let file_data = io::read_all(fd);
            io::close(fd);
            data.extend_from_slice(&file_data);
        }
        data
    };

    // Apply skip and count
    let start = core::cmp::min(skip, content.len());
    let end = match count {
        Some(n) => core::cmp::min(start + n, content.len()),
        None => content.len(),
    };
    let data = &content[start..end];

    // Output
    let bytes_per_line = 16;
    let mut offset = skip;
    let mut pos = 0;

    while pos < data.len() {
        // Print address
        let mut buf = [0u8; 16];
        match address_radix {
            AddressRadix::Octal => {
                let oct = crate::sys::format_octal(offset as u32, &mut buf);
                for _ in 0..(7usize.saturating_sub(oct.len())) {
                    io::write_str(1, b"0");
                }
                io::write_all(1, oct);
            }
            AddressRadix::Hex => {
                let hex = crate::sys::format_hex(offset as u64, &mut buf);
                for _ in 0..(6usize.saturating_sub(hex.len())) {
                    io::write_str(1, b"0");
                }
                io::write_all(1, hex);
            }
            AddressRadix::Decimal => {
                let dec = crate::sys::format_u64(offset as u64, &mut buf);
                for _ in 0..(7usize.saturating_sub(dec.len())) {
                    io::write_str(1, b" ");
                }
                io::write_all(1, dec);
            }
            AddressRadix::None => {}
        }

        // Print data
        let line_end = core::cmp::min(pos + bytes_per_line, data.len());
        let line = &data[pos..line_end];

        match format {
            Format::Octal1 => {
                for &b in line {
                    io::write_str(1, b" ");
                    let oct = crate::sys::format_octal(b as u32, &mut buf);
                    for _ in 0..(3usize.saturating_sub(oct.len())) {
                        io::write_str(1, b"0");
                    }
                    io::write_all(1, oct);
                }
            }
            Format::Octal2 => {
                for chunk in line.chunks(2) {
                    io::write_str(1, b" ");
                    let val = if chunk.len() == 2 {
                        (chunk[1] as u32) << 8 | (chunk[0] as u32)
                    } else {
                        chunk[0] as u32
                    };
                    let oct = crate::sys::format_octal(val, &mut buf);
                    for _ in 0..(6usize.saturating_sub(oct.len())) {
                        io::write_str(1, b"0");
                    }
                    io::write_all(1, oct);
                }
            }
            Format::Hex1 => {
                for &b in line {
                    io::write_str(1, b" ");
                    let hex = crate::sys::format_hex(b as u64, &mut buf);
                    if hex.len() < 2 {
                        io::write_str(1, b"0");
                    }
                    io::write_all(1, hex);
                }
            }
            Format::Hex2 => {
                for chunk in line.chunks(2) {
                    io::write_str(1, b" ");
                    let val = if chunk.len() == 2 {
                        (chunk[1] as u64) << 8 | (chunk[0] as u64)
                    } else {
                        chunk[0] as u64
                    };
                    let hex = crate::sys::format_hex(val, &mut buf);
                    for _ in 0..(4usize.saturating_sub(hex.len())) {
                        io::write_str(1, b"0");
                    }
                    io::write_all(1, hex);
                }
            }
            Format::Decimal1 => {
                for &b in line {
                    io::write_str(1, b" ");
                    let val = b as i8 as i64;
                    let dec = crate::sys::format_i64(val, &mut buf);
                    for _ in 0..(4usize.saturating_sub(dec.len())) {
                        io::write_str(1, b" ");
                    }
                    io::write_all(1, dec);
                }
            }
            Format::Decimal2 => {
                for chunk in line.chunks(2) {
                    io::write_str(1, b" ");
                    let val = if chunk.len() == 2 {
                        ((chunk[1] as u16) << 8 | (chunk[0] as u16)) as i16 as i64
                    } else {
                        chunk[0] as i8 as i64
                    };
                    let dec = crate::sys::format_i64(val, &mut buf);
                    for _ in 0..(6usize.saturating_sub(dec.len())) {
                        io::write_str(1, b" ");
                    }
                    io::write_all(1, dec);
                }
            }
            Format::Unsigned1 => {
                for &b in line {
                    io::write_str(1, b" ");
                    let dec = crate::sys::format_u64(b as u64, &mut buf);
                    for _ in 0..(3usize.saturating_sub(dec.len())) {
                        io::write_str(1, b" ");
                    }
                    io::write_all(1, dec);
                }
            }
            Format::Char => {
                for &b in line {
                    io::write_str(1, b" ");
                    match b {
                        0 => {
                            io::write_str(1, b" \\0");
                        }
                        b'\t' => {
                            io::write_str(1, b" \\t");
                        }
                        b'\n' => {
                            io::write_str(1, b" \\n");
                        }
                        b'\r' => {
                            io::write_str(1, b" \\r");
                        }
                        b'\\' => {
                            io::write_str(1, b" \\\\");
                        }
                        0x20..=0x7E => {
                            io::write_all(1, &[b' ', b' ', b]);
                        }
                        _ => {
                            let oct = crate::sys::format_octal(b as u32, &mut buf);
                            io::write_str(1, b"\\");
                            for _ in 0..(3usize.saturating_sub(oct.len())) {
                                io::write_str(1, b"0");
                            }
                            io::write_all(1, oct);
                        }
                    }
                }
            }
        }

        io::write_str(1, b"\n");
        offset += line_end - pos;
        pos = line_end;
    }

    // Print final address
    if !matches!(address_radix, AddressRadix::None) {
        let mut buf = [0u8; 16];
        match address_radix {
            AddressRadix::Octal => {
                let oct = crate::sys::format_octal(offset as u32, &mut buf);
                for _ in 0..(7usize.saturating_sub(oct.len())) {
                    io::write_str(1, b"0");
                }
                io::write_all(1, oct);
            }
            AddressRadix::Hex => {
                let hex = crate::sys::format_hex(offset as u64, &mut buf);
                for _ in 0..(6usize.saturating_sub(hex.len())) {
                    io::write_str(1, b"0");
                }
                io::write_all(1, hex);
            }
            AddressRadix::Decimal => {
                let dec = crate::sys::format_u64(offset as u64, &mut buf);
                for _ in 0..(7usize.saturating_sub(dec.len())) {
                    io::write_str(1, b" ");
                }
                io::write_all(1, dec);
            }
            AddressRadix::None => {}
        }
        io::write_str(1, b"\n");
    }

    0
}

#[cfg(not(feature = "alloc"))]
pub fn od(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"od: requires alloc feature\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::fs;
    use std::io::Write;

    use std::process::{Command, Stdio};

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_hexdump_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = std::env::temp_dir().join("armybox_test_hexdump");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("test.bin"), b"ABCD").unwrap();

        let output = Command::new(&armybox)
            .args(["hexdump", dir.join("test.bin").to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("41")); // 'A' = 0x41
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_hexdump_stdin() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let mut child = Command::new(&armybox)
            .args(["hexdump"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();

        child.stdin.as_mut().unwrap().write_all(b"test").unwrap();
        drop(child.stdin.take());

        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("74")); // 't' = 0x74
    }
}
