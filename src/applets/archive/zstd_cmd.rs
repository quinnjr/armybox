//! zstd - compress files using Zstandard
//!
//! Compress or decompress files using the Zstandard algorithm.

extern crate alloc;
use super::get_arg;
use crate::io;
use alloc::vec::Vec;

fn read_file(path: &[u8]) -> Option<Vec<u8>> {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }
    let data = io::read_all(fd);
    io::close(fd);
    Some(data)
}

fn write_file(path: &[u8], data: &[u8]) -> bool {
    let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if fd < 0 {
        return false;
    }
    let written = io::write_all(fd, data);
    io::close(fd);
    written == data.len() as isize
}

fn file_exists(path: &[u8]) -> bool {
    io::access(path, libc::F_OK) == 0
}

fn remove_file(path: &[u8]) {
    io::unlink(path);
}

pub fn zstd(argc: i32, argv: *const *const u8) -> i32 {
    let mut decompress_mode = false;
    let mut keep = false;
    let mut to_stdout = false;
    let mut force = false;
    let mut level = 3u32;
    let mut files: Vec<&[u8]> = Vec::new();

    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-d" || arg == b"--decompress" {
            decompress_mode = true;
        } else if arg == b"-k" || arg == b"--keep" {
            keep = true;
        } else if arg == b"-c" || arg == b"--stdout" {
            to_stdout = true;
        } else if arg == b"-f" || arg == b"--force" {
            force = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: zstd [OPTIONS] [FILE...]\n\n");
            io::write_str(1, b"Compress or decompress files using Zstandard.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(1, b"  -d, --decompress  Decompress\n");
            io::write_str(1, b"  -k, --keep        Keep input files\n");
            io::write_str(1, b"  -c, --stdout      Write to stdout\n");
            io::write_str(1, b"  -f, --force       Force overwrite\n");
            io::write_str(1, b"  -1...-9           Compression level (default: 3)\n");
            io::write_str(1, b"  -h, --help        Show this help\n\n");
            io::write_str(1, b"With no FILE, read from stdin.\n");
            return 0;
        } else if arg.len() == 2 && arg[0] == b'-' && arg[1] >= b'1' && arg[1] <= b'9' {
            level = (arg[1] - b'0') as u32;
        } else if !arg.starts_with(b"-") {
            files.push(arg);
        }
        i += 1;
    }

    let prog_name = unsafe { get_arg(argv, 0) }.unwrap_or(b"zstd");
    if prog_name.ends_with(b"unzstd") || prog_name.ends_with(b"zstdcat") {
        decompress_mode = true;
    }
    if prog_name.ends_with(b"zstdcat") {
        to_stdout = true;
    }

    if files.is_empty() {
        let input = io::read_all(0);
        let output = if decompress_mode {
            match zstd_nostd::decompress(&input) {
                Ok(d) => d,
                Err(_) => {
                    io::write_str(2, b"zstd: decompression failed\n");
                    return 1;
                }
            }
        } else {
            zstd_nostd::compress(&input, level)
        };
        io::write_all(1, &output);
        return 0;
    }

    let mut status = 0;

    for file in files {
        let input = match read_file(file) {
            Some(d) => d,
            None => {
                io::write_str(2, b"zstd: cannot read ");
                io::write_all(2, file);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }
        };

        let (output, out_name) = if decompress_mode {
            let decompressed = match zstd_nostd::decompress(&input) {
                Ok(d) => d,
                Err(_) => {
                    io::write_str(2, b"zstd: ");
                    io::write_all(2, file);
                    io::write_str(2, b": decompression failed\n");
                    status = 1;
                    continue;
                }
            };

            let name = if file.ends_with(b".zst") {
                &file[..file.len() - 4]
            } else {
                io::write_str(2, b"zstd: ");
                io::write_all(2, file);
                io::write_str(2, b": unknown suffix -- ignored\n");
                status = 1;
                continue;
            };

            (decompressed, name.to_vec())
        } else {
            let compressed = zstd_nostd::compress(&input, level);
            let mut name = file.to_vec();
            name.extend_from_slice(b".zst");
            (compressed, name)
        };

        if to_stdout {
            io::write_all(1, &output);
        } else {
            if !force && file_exists(&out_name) {
                io::write_str(2, b"zstd: ");
                io::write_all(2, &out_name);
                io::write_str(2, b" already exists; use -f to overwrite\n");
                status = 1;
                continue;
            }

            if !write_file(&out_name, &output) {
                io::write_str(2, b"zstd: cannot write ");
                io::write_all(2, &out_name);
                io::write_str(2, b"\n");
                status = 1;
                continue;
            }

            if !keep {
                remove_file(file);
            }
        }
    }

    status
}

pub fn unzstd(argc: i32, argv: *const *const u8) -> i32 {
    zstd(argc, argv)
}

pub fn zstdcat(argc: i32, argv: *const *const u8) -> i32 {
    zstd(argc, argv)
}
