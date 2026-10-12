//! tar - tape archiver
//!
//! Create, extract, and list tar archives.

use super::{create_parent_dirs, get_arg, mode_string, open_read, open_write_create};
use crate::io;
use alloc::vec::Vec;

/// USTAR tar header structure (512 bytes)
#[repr(C)]
pub struct TarHeader {
    pub name: [u8; 100],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub checksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; 100],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub prefix: [u8; 155],
    pub _pad: [u8; 12],
}

impl TarHeader {
    pub fn new() -> Self {
        Self {
            name: [0; 100],
            mode: [0; 8],
            uid: [0; 8],
            gid: [0; 8],
            size: [0; 12],
            mtime: [0; 12],
            checksum: [0; 8],
            typeflag: 0,
            linkname: [0; 100],
            magic: [0; 6],
            version: [0; 2],
            uname: [0; 32],
            gname: [0; 32],
            devmajor: [0; 8],
            devminor: [0; 8],
            prefix: [0; 155],
            _pad: [0; 12],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name[0] == 0
    }

    pub fn get_name(&self) -> &[u8] {
        let len = self.name.iter().position(|&c| c == 0).unwrap_or(100);
        &self.name[..len]
    }

    pub fn get_size(&self) -> u64 {
        parse_octal(&self.size)
    }

    pub fn get_mode(&self) -> u32 {
        parse_octal(&self.mode) as u32
    }

    pub fn compute_checksum(&self) -> u32 {
        let bytes = unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, 512) };
        let mut sum = 0u32;
        for (i, &b) in bytes.iter().enumerate() {
            // Treat checksum field (bytes 148-155) as spaces
            if i >= 148 && i < 156 {
                sum += b' ' as u32;
            } else {
                sum += b as u32;
            }
        }
        sum
    }

    pub fn set_checksum(&mut self) {
        // Fill with spaces first
        self.checksum = *b"        ";
        let sum = self.compute_checksum();
        // Write checksum as octal with null terminator and space
        write_octal(&mut self.checksum[..6], sum as u64);
        self.checksum[6] = 0;
        self.checksum[7] = b' ';
    }
}

fn parse_octal(bytes: &[u8]) -> u64 {
    let mut result = 0u64;
    for &b in bytes {
        if b == 0 || b == b' ' {
            break;
        }
        if b >= b'0' && b <= b'7' {
            result = result * 8 + (b - b'0') as u64;
        }
    }
    result
}

fn write_octal(buf: &mut [u8], mut val: u64) {
    let len = buf.len();
    for i in (0..len).rev() {
        buf[i] = b'0' + (val & 7) as u8;
        val >>= 3;
    }
}

/// Returns `true` if `name` is unsafe to use as an extraction path:
/// an absolute path, or a path containing a `..` component.
///
/// Detection is component-based (splitting on `/`), not a substring
/// search, so this correctly rejects `../x`, `a/../b`, `a/..`, and `..`
/// while allowing legitimate names that merely contain the bytes `..`
/// as part of a longer component (e.g. `foo..bar`).
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

/// tar - tape archiver
///
/// # Synopsis
/// ```text
/// tar [-cxtv] -f ARCHIVE [FILES...]
/// ```
///
/// # Description
/// Create, extract, or list tar archives.
///
/// # Options
/// - `-c`: Create archive
/// - `-x`: Extract archive
/// - `-t`: List archive contents
/// - `-v`: Verbose output
/// - `-f FILE`: Archive file
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn tar(argc: i32, argv: *const *const u8) -> i32 {
    let mut create = false;
    let mut extract = false;
    let mut list = false;
    let mut verbose = false;
    let mut archive_file: Option<&[u8]> = None;
    let mut files: Vec<&[u8]> = Vec::new();

    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") || (i == 1 && !arg.starts_with(b"/")) {
                // Options (possibly without leading dash for first arg)
                let opts = if arg.starts_with(b"-") {
                    &arg[1..]
                } else {
                    arg
                };
                for &c in opts {
                    match c {
                        b'c' => create = true,
                        b'x' => extract = true,
                        b't' => list = true,
                        b'v' => verbose = true,
                        b'f' => {
                            // Next arg is the archive file
                            i += 1;
                            if let Some(f) = unsafe { get_arg(argv, i) } {
                                archive_file = Some(f);
                            }
                        }
                        b'z' | b'j' | b'J' => {
                            // Compression flags - we'll handle uncompressed only for now
                        }
                        _ => {}
                    }
                }
            } else {
                files.push(arg);
            }
        }
        i += 1;
    }

    let archive = match archive_file {
        Some(f) => f,
        None => {
            io::write_str(2, b"tar: -f archive required\n");
            return 1;
        }
    };

    if create {
        tar_create(archive, &files, verbose)
    } else if extract {
        tar_extract(archive, &files, verbose)
    } else if list {
        tar_list(archive, verbose)
    } else {
        io::write_str(2, b"tar: specify -c, -x, or -t\n");
        1
    }
}

fn tar_create(archive: &[u8], files: &[&[u8]], verbose: bool) -> i32 {
    let fd = open_write_create(archive, 0o644);
    if fd < 0 {
        io::write_str(2, b"tar: cannot create archive\n");
        return 1;
    }

    for &file in files {
        if tar_add_file(fd, file, verbose) != 0 {
            io::close(fd);
            return 1;
        }
    }

    // Write two empty blocks as end marker
    let empty = [0u8; 512];
    io::write_all(fd, &empty);
    io::write_all(fd, &empty);

    io::close(fd);
    0
}

fn tar_add_file(fd: i32, path: &[u8], verbose: bool) -> i32 {
    // Get file info
    let mut stat_buf: libc::stat = unsafe { core::mem::zeroed() };

    if io::stat(path, &mut stat_buf) != 0 {
        io::write_str(2, b"tar: cannot stat ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    let mode = stat_buf.st_mode as u32;
    let is_dir = (mode & libc::S_IFMT as u32) == libc::S_IFDIR as u32;
    let is_link = (mode & libc::S_IFMT as u32) == libc::S_IFLNK as u32;

    if is_dir {
        // Add directory and recurse
        tar_add_dir_entry(fd, path, &stat_buf, verbose);
        // Recursively add directory contents
        let dir = io::opendir(path);
        if !dir.is_null() {
            loop {
                let entry = io::readdir(dir);
                if entry.is_null() {
                    break;
                }
                let name = unsafe { io::cstr_to_slice((*entry).d_name.as_ptr() as *const u8) };
                if name == b"." || name == b".." {
                    continue;
                }

                // Build full path
                let mut full_path = Vec::new();
                full_path.extend_from_slice(path);
                if !path.ends_with(b"/") {
                    full_path.push(b'/');
                }
                full_path.extend_from_slice(name);

                tar_add_file(fd, &full_path, verbose);
            }
            io::closedir(dir);
        }
    } else if is_link {
        if tar_add_link_entry(fd, path, &stat_buf, verbose) != 0 {
            return 1;
        }
    } else if tar_add_file_entry(fd, path, &stat_buf, verbose) != 0 {
        return 1;
    }

    0
}

fn tar_add_dir_entry(fd: i32, path: &[u8], stat: &libc::stat, verbose: bool) {
    let mut header = TarHeader::new();

    // Name (with trailing slash for directories)
    let len = path.len().min(99);
    header.name[..len].copy_from_slice(&path[..len]);
    if len < 99 && !path.ends_with(b"/") {
        header.name[len] = b'/';
    }

    // Mode
    write_octal(&mut header.mode[..7], (stat.st_mode & 0o7777) as u64);

    // UID/GID
    write_octal(&mut header.uid[..7], stat.st_uid as u64);
    write_octal(&mut header.gid[..7], stat.st_gid as u64);

    // Size (0 for directories)
    write_octal(&mut header.size[..11], 0);

    // Mtime
    write_octal(&mut header.mtime[..11], stat.st_mtime as u64);

    // Type flag - directory
    header.typeflag = b'5';

    // USTAR magic
    header.magic = *b"ustar ";
    header.version = *b" \0";

    header.set_checksum();

    let header_bytes =
        unsafe { core::slice::from_raw_parts(&header as *const _ as *const u8, 512) };
    io::write_all(fd, header_bytes);

    if verbose {
        io::write_all(1, path);
        io::write_str(1, b"/\n");
    }
}

fn tar_add_link_entry(fd: i32, path: &[u8], stat: &libc::stat, verbose: bool) -> i32 {
    if path.len() >= io::PATH_MAX {
        io::write_str(2, b"tar: path too long, skipping ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    let mut header = TarHeader::new();

    // Name
    let len = path.len().min(100);
    header.name[..len].copy_from_slice(&path[..len]);

    // Mode
    write_octal(&mut header.mode[..7], (stat.st_mode & 0o7777) as u64);

    // UID/GID
    write_octal(&mut header.uid[..7], stat.st_uid as u64);
    write_octal(&mut header.gid[..7], stat.st_gid as u64);

    // Size (0 for symlinks)
    write_octal(&mut header.size[..11], 0);

    // Mtime
    write_octal(&mut header.mtime[..11], stat.st_mtime as u64);

    // Read link target
    let mut linkbuf = [0u8; 100];
    let link_len = io::readlink(path, &mut linkbuf);
    if link_len > 0 {
        header.linkname[..link_len as usize].copy_from_slice(&linkbuf[..link_len as usize]);
    }

    // Type flag - symlink
    header.typeflag = b'2';

    // USTAR magic
    header.magic = *b"ustar ";
    header.version = *b" \0";

    header.set_checksum();

    let header_bytes =
        unsafe { core::slice::from_raw_parts(&header as *const _ as *const u8, 512) };
    io::write_all(fd, header_bytes);

    if verbose {
        io::write_all(1, path);
        io::write_str(1, b"\n");
    }
    0
}

fn tar_add_file_entry(fd: i32, path: &[u8], stat: &libc::stat, verbose: bool) -> i32 {
    if path.len() >= io::PATH_MAX {
        io::write_str(2, b"tar: path too long, skipping ");
        io::write_all(2, path);
        io::write_str(2, b"\n");
        return 1;
    }

    let mut header = TarHeader::new();

    // Name
    let len = path.len().min(100);
    header.name[..len].copy_from_slice(&path[..len]);

    // Mode
    write_octal(&mut header.mode[..7], (stat.st_mode & 0o7777) as u64);

    // UID/GID
    write_octal(&mut header.uid[..7], stat.st_uid as u64);
    write_octal(&mut header.gid[..7], stat.st_gid as u64);

    // Size
    let size = stat.st_size as u64;
    write_octal(&mut header.size[..11], size);

    // Mtime
    write_octal(&mut header.mtime[..11], stat.st_mtime as u64);

    // Type flag - regular file
    header.typeflag = b'0';

    // USTAR magic
    header.magic = *b"ustar ";
    header.version = *b" \0";

    header.set_checksum();

    let header_bytes =
        unsafe { core::slice::from_raw_parts(&header as *const _ as *const u8, 512) };
    io::write_all(fd, header_bytes);

    // Write file content
    let file_fd = open_read(path);
    if file_fd >= 0 {
        let mut buf = [0u8; 512];
        let mut remaining = size;
        while remaining > 0 {
            let to_read = remaining.min(512) as usize;
            let n = io::read(file_fd, &mut buf[..to_read]);
            if n <= 0 {
                break;
            }
            // Pad last block to 512 bytes
            if (n as usize) < 512 {
                for i in (n as usize)..512 {
                    buf[i] = 0;
                }
            }
            io::write_all(fd, &buf);
            remaining -= n as u64;
        }
        io::close(file_fd);
    }

    if verbose {
        io::write_all(1, path);
        io::write_str(1, b"\n");
    }
    0
}

fn tar_extract(archive: &[u8], files: &[&[u8]], verbose: bool) -> i32 {
    let fd = open_read(archive);
    if fd < 0 {
        io::write_str(2, b"tar: cannot open archive\n");
        return 1;
    }

    let mut header_buf = [0u8; 512];
    let mut exit_code = 0;

    loop {
        if io::read(fd, &mut header_buf) != 512 {
            break;
        }

        let header = unsafe { &*(header_buf.as_ptr() as *const TarHeader) };

        if header.is_empty() {
            break;
        }

        let name = header.get_name();
        let size = header.get_size();
        let mode = header.get_mode();

        // Check if this file matches the filter (if any)
        let should_extract =
            files.is_empty() || files.iter().any(|&f| name.starts_with(f) || f == name);

        // Reject absolute paths and paths with ".." components: extracting
        // them could write outside the destination directory.
        let unsafe_path = should_extract && is_unsafe_path(name);
        if unsafe_path {
            io::write_str(2, b"tar: skipping unsafe path\n");
            exit_code = 1;
        }
        let do_extract = should_extract && !unsafe_path;

        if do_extract {
            match header.typeflag {
                b'5' | b'\0' if name.ends_with(b"/") => {
                    // Directory
                    if name.len() >= io::PATH_MAX {
                        io::write_str(2, b"tar: path too long, skipping\n");
                        exit_code = 1;
                    } else {
                        io::mkdir(name, mode);
                        if verbose {
                            io::write_all(1, name);
                            io::write_str(1, b"\n");
                        }
                    }
                }
                b'2' => {
                    // Symlink
                    let target = {
                        let len = header.linkname.iter().position(|&c| c == 0).unwrap_or(100);
                        &header.linkname[..len]
                    };
                    if name.len() >= io::PATH_MAX || target.len() >= io::PATH_MAX {
                        io::write_str(2, b"tar: path too long, skipping\n");
                        exit_code = 1;
                    } else {
                        io::symlink(target, name);
                        if verbose {
                            io::write_all(1, name);
                            io::write_str(1, b"\n");
                        }
                    }
                }
                b'0' | b'\0'
                    // Regular file
                    if extract_file(name, fd, size, mode, verbose) != 0 => {
                        exit_code = 1;
                    }
                _ => {
                    // Skip unknown types
                }
            }
        }

        // Skip file content blocks (also needed when we rejected the entry
        // above but it was a regular file, so the stream stays aligned).
        if size > 0 && (header.typeflag == b'0' || header.typeflag == 0) {
            let blocks = (size + 511) / 512;
            if !do_extract {
                for _ in 0..blocks {
                    io::read(fd, &mut header_buf);
                }
            }
        }
    }

    io::close(fd);
    exit_code
}

fn extract_file(name: &[u8], archive_fd: i32, size: u64, mode: u32, verbose: bool) -> i32 {
    // Create parent directories if needed
    let parents_ok = create_parent_dirs(name);

    let fd = if parents_ok && name.len() < io::PATH_MAX {
        open_write_create(name, mode as i32)
    } else {
        -1
    };

    if fd < 0 {
        if !parents_ok || name.len() >= io::PATH_MAX {
            io::write_str(2, b"tar: path too long, skipping ");
        } else {
            io::write_str(2, b"tar: cannot create ");
        }
        io::write_all(2, name);
        io::write_str(2, b"\n");
        // Still need to skip the blocks
        let blocks = (size + 511) / 512;
        let mut skip_buf = [0u8; 512];
        for _ in 0..blocks {
            io::read(archive_fd, &mut skip_buf);
        }
        return 1;
    }

    let mut remaining = size;
    let mut buf = [0u8; 512];
    while remaining > 0 {
        if io::read(archive_fd, &mut buf) != 512 {
            break;
        }
        let to_write = remaining.min(512) as usize;
        io::write_all(fd, &buf[..to_write]);
        remaining -= to_write as u64;
    }

    io::close(fd);

    if verbose {
        io::write_all(1, name);
        io::write_str(1, b"\n");
    }
    0
}

fn tar_list(archive: &[u8], verbose: bool) -> i32 {
    let fd = open_read(archive);
    if fd < 0 {
        io::write_str(2, b"tar: cannot open archive\n");
        return 1;
    }

    let mut header_buf = [0u8; 512];

    loop {
        if io::read(fd, &mut header_buf) != 512 {
            break;
        }

        let header = unsafe { &*(header_buf.as_ptr() as *const TarHeader) };

        if header.is_empty() {
            break;
        }

        let name = header.get_name();
        let size = header.get_size();

        if verbose {
            // Show mode, size, name
            let mode = header.get_mode();
            let type_char = match header.typeflag {
                b'5' => b'd',
                b'2' => b'l',
                _ => b'-',
            };
            io::write_all(1, &[type_char]);
            io::write_all(1, &mode_string(mode));
            io::write_str(1, b" ");
            io::write_num(1, size);
            io::write_str(1, b" ");
        }
        io::write_all(1, name);
        io::write_str(1, b"\n");

        // Skip file content blocks
        if size > 0 && (header.typeflag == b'0' || header.typeflag == 0) {
            let blocks = (size + 511) / 512;
            for _ in 0..blocks {
                io::read(fd, &mut header_buf);
            }
        }
    }

    io::close(fd);
    0
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_tar_no_archive() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["tar", "-c"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("-f archive required"));
    }

    #[test]
    fn test_tar_no_mode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["tar", "-f", "test.tar"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("specify -c, -x, or -t"));
    }

    #[test]
    fn test_tar_create_and_list() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = std::env::temp_dir().join("armybox_tar_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("testfile.txt"), "hello world").unwrap();

        let tar_path = dir.join("test.tar");

        // Create
        let output = Command::new(&armybox)
            .args([
                "tar",
                "-cf",
                tar_path.to_str().unwrap(),
                dir.join("testfile.txt").to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // List
        let output = Command::new(&armybox)
            .args(["tar", "-tf", tar_path.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("testfile.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
