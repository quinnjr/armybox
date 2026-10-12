//! File attribute and filesystem utilities
//!
//! Implements chattr/lsattr for ext2/ext3/ext4 file attributes,
//! setfattr for extended attributes, fstype for filesystem detection,
//! and makedevs for device node creation.

use super::get_arg;
use crate::io;

// Linux ext2/ext3/ext4 attribute flags
const EXT2_SECRM_FL: u32 = 0x00000001; // Secure deletion
const EXT2_UNRM_FL: u32 = 0x00000002; // Undelete
const EXT2_COMPR_FL: u32 = 0x00000004; // Compress file
const EXT2_SYNC_FL: u32 = 0x00000008; // Synchronous updates
const EXT2_IMMUTABLE_FL: u32 = 0x00000010; // Immutable file
const EXT2_APPEND_FL: u32 = 0x00000020; // Append only
const EXT2_NODUMP_FL: u32 = 0x00000040; // No dump
const EXT2_NOATIME_FL: u32 = 0x00000080; // No atime updates
const EXT4_DIRSYNC_FL: u32 = 0x00010000; // Dirsync
const EXT4_TOPDIR_FL: u32 = 0x00020000; // Top of directory hierarchies
const EXT4_EXTENTS_FL: u32 = 0x00080000; // Inode uses extents

// ioctl numbers for ext2 attributes
const FS_IOC_GETFLAGS: crate::io::IoctlReq = 0x80086601u32 as crate::io::IoctlReq;
const FS_IOC_SETFLAGS: crate::io::IoctlReq = 0x40086602u32 as crate::io::IoctlReq;

/// chattr - change file attributes on a Linux file system
///
/// Usage: chattr [-R] [-v version] [-p project] mode files...
/// Mode: [+-=][aAcCdDeijsStTu]
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn chattr(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"Usage: chattr [-R] [-v version] [mode] files...\n");
        return 1;
    }

    let mut recursive = false;
    let mut add_flags: u32 = 0;
    let mut remove_flags: u32 = 0;
    let mut set_flags: Option<u32> = None;
    let mut files_start = 1;

    // Parse options and mode
    let mut i = 1;
    while i < argc as usize {
        // SAFETY: argv is valid for argc elements
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-R" {
            recursive = true;
        } else if arg == b"-v" || arg == b"-p" {
            i += 1; // Skip version/project argument
        } else if !arg.is_empty() && (arg[0] == b'+' || arg[0] == b'-' || arg[0] == b'=') {
            // Parse mode
            let mode = arg[0];
            let mut flags: u32 = 0;

            for &c in &arg[1..] {
                flags |= match c {
                    b'a' => EXT2_APPEND_FL,
                    b'A' => EXT2_NOATIME_FL,
                    b'c' => EXT2_COMPR_FL,
                    b'd' => EXT2_NODUMP_FL,
                    b'D' => EXT4_DIRSYNC_FL,
                    b'e' => EXT4_EXTENTS_FL,
                    b'i' => EXT2_IMMUTABLE_FL,
                    b's' => EXT2_SECRM_FL,
                    b'S' => EXT2_SYNC_FL,
                    b'T' => EXT4_TOPDIR_FL,
                    b'u' => EXT2_UNRM_FL,
                    _ => 0,
                };
            }

            match mode {
                b'+' => add_flags |= flags,
                b'-' => remove_flags |= flags,
                b'=' => set_flags = Some(flags),
                _ => {}
            }
        } else {
            files_start = i;
            break;
        }
        i += 1;
    }

    let _ = recursive; // TODO: implement recursive

    let mut status = 0;

    // Process files
    for j in files_start..argc as usize {
        let path = match unsafe { get_arg(argv, j as i32) } {
            Some(p) => p,
            None => continue,
        };

        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd < 0 {
            io::write_str(2, b"chattr: ");
            io::write_all(2, path);
            io::write_str(2, b": No such file or directory\n");
            status = 1;
            continue;
        }

        // Get current flags
        let mut flags: u32 = 0;
        let ret = unsafe {
            libc::ioctl(
                fd,
                FS_IOC_GETFLAGS as crate::io::IoctlReq,
                &mut flags as *mut u32,
            )
        };

        if ret < 0 {
            io::write_str(2, b"chattr: ");
            io::write_all(2, path);
            io::write_str(2, b": Inappropriate ioctl for device\n");
            io::close(fd);
            status = 1;
            continue;
        }

        // Modify flags
        if let Some(new_flags) = set_flags {
            flags = new_flags;
        } else {
            flags |= add_flags;
            flags &= !remove_flags;
        }

        // Set new flags
        let ret = unsafe {
            libc::ioctl(
                fd,
                FS_IOC_SETFLAGS as crate::io::IoctlReq,
                &flags as *const u32,
            )
        };

        if ret < 0 {
            io::write_str(2, b"chattr: ");
            io::write_all(2, path);
            io::write_str(2, b": Operation not permitted\n");
            status = 1;
        }

        io::close(fd);
    }

    status
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn chattr(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"chattr: only available on Linux\n");
    1
}

/// lsattr - list file attributes on a Linux file system
///
/// Usage: lsattr [-R] [-a] [-d] [-l] [-v] files...
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn lsattr(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_all = false;
    let mut dir_only = false;
    let mut long_format = false;
    let mut files_start = 1;

    // Parse options
    let mut i = 1;
    while i < argc as usize {
        // SAFETY: argv is valid for argc elements
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg.starts_with(b"-") && arg.len() > 1 {
            for &c in &arg[1..] {
                match c {
                    b'a' => show_all = true,
                    b'd' => dir_only = true,
                    b'l' => long_format = true,
                    b'R' | b'v' | b'V' => {} // Ignored for now
                    _ => {}
                }
            }
        } else {
            files_start = i;
            break;
        }
        i += 1;
    }

    let _ = show_all;
    let _ = dir_only;

    // If no files specified, use current directory
    let process_current_dir = files_start >= argc as usize;

    if process_current_dir {
        list_attrs(b".", long_format);
    } else {
        for j in files_start..argc as usize {
            if let Some(path) = unsafe { get_arg(argv, j as i32) } {
                list_attrs(path, long_format);
            }
        }
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn list_attrs(path: &[u8], long_format: bool) {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"lsattr: ");
        io::write_all(2, path);
        io::write_str(2, b": No such file or directory\n");
        return;
    }

    let mut flags: u32 = 0;
    let ret = unsafe {
        libc::ioctl(
            fd,
            FS_IOC_GETFLAGS as crate::io::IoctlReq,
            &mut flags as *mut u32,
        )
    };
    io::close(fd);

    if ret < 0 {
        // Not a supported filesystem, show dashes
        if long_format {
            io::write_str(1, b"------- ");
        } else {
            io::write_str(1, b"------------- ");
        }
        io::write_all(1, path);
        io::write_str(1, b"\n");
        return;
    }

    if long_format {
        // Long format: show attribute names
        let attrs = [
            (EXT2_SECRM_FL, b"Secure_Deletion" as &[u8]),
            (EXT2_UNRM_FL, b"Undelete"),
            (EXT2_COMPR_FL, b"Compression"),
            (EXT2_SYNC_FL, b"Synchronous_Updates"),
            (EXT2_IMMUTABLE_FL, b"Immutable"),
            (EXT2_APPEND_FL, b"Append_Only"),
            (EXT2_NODUMP_FL, b"No_Dump"),
            (EXT2_NOATIME_FL, b"No_Atime"),
        ];

        let mut first = true;
        for (flag, name) in &attrs {
            if flags & flag != 0 {
                if !first {
                    io::write_str(1, b", ");
                }
                io::write_all(1, name);
                first = false;
            }
        }
        if first {
            io::write_str(1, b"---");
        }
    } else {
        // Short format: show attribute letters
        let mut attr_str = [b'-'; 13];

        if flags & EXT2_SECRM_FL != 0 {
            attr_str[0] = b's';
        }
        if flags & EXT2_UNRM_FL != 0 {
            attr_str[1] = b'u';
        }
        if flags & EXT2_SYNC_FL != 0 {
            attr_str[2] = b'S';
        }
        if flags & EXT4_DIRSYNC_FL != 0 {
            attr_str[3] = b'D';
        }
        if flags & EXT2_IMMUTABLE_FL != 0 {
            attr_str[4] = b'i';
        }
        if flags & EXT2_APPEND_FL != 0 {
            attr_str[5] = b'a';
        }
        if flags & EXT2_NODUMP_FL != 0 {
            attr_str[6] = b'd';
        }
        if flags & EXT2_NOATIME_FL != 0 {
            attr_str[7] = b'A';
        }
        if flags & EXT2_COMPR_FL != 0 {
            attr_str[8] = b'c';
        }
        if flags & EXT4_EXTENTS_FL != 0 {
            attr_str[9] = b'e';
        }
        // Positions 10-12 for other flags

        io::write_all(1, &attr_str);
    }

    io::write_str(1, b" ");
    io::write_all(1, path);
    io::write_str(1, b"\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn lsattr(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"lsattr: only available on Linux\n");
    1
}

/// fstype - print type of filesystem
///
/// Detects filesystem type by reading superblock magic numbers.
pub fn fstype(argc: i32, argv: *const *const u8) -> i32 {
    let path = if argc > 1 {
        match unsafe { get_arg(argv, 1) } {
            Some(p) => p,
            None => return 1,
        }
    } else {
        io::write_str(2, b"Usage: fstype <device>\n");
        return 1;
    };

    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"fstype: ");
        io::write_all(2, path);
        io::write_str(2, b": Cannot open\n");
        return 1;
    }

    // Read first 64KB to check magic numbers
    let mut buf = [0u8; 65536];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n < 2 {
        io::write_str(1, b"unknown\n");
        return 1;
    }

    // Check various filesystem magic numbers
    let fstype = detect_filesystem(&buf[..n as usize]);
    io::write_all(1, fstype);
    io::write_str(1, b"\n");

    0
}

fn detect_filesystem(data: &[u8]) -> &'static [u8] {
    // ext2/3/4: magic at offset 0x438
    if data.len() > 0x439 && data[0x438] == 0x53 && data[0x439] == 0xEF {
        // Check for ext4 features
        if data.len() > 0x464 {
            let feature_incompat =
                u32::from_le_bytes([data[0x460], data[0x461], data[0x462], data[0x463]]);
            if feature_incompat & 0x40 != 0 {
                // EXT4_FEATURE_INCOMPAT_EXTENTS
                return b"ext4";
            }
            if feature_incompat & 0x04 != 0 {
                // EXT3_FEATURE_INCOMPAT_JOURNAL_DEV
                return b"ext3";
            }
        }
        return b"ext2";
    }

    // FAT: check for boot signature and FAT string
    if data.len() > 512 && data[510] == 0x55 && data[511] == 0xAA {
        if data.len() > 0x36 + 5 && &data[0x36..0x3B] == b"FAT32" {
            return b"vfat";
        }
        if data.len() > 0x36 + 3 && &data[0x36..0x39] == b"FAT" {
            return b"vfat";
        }
    }

    // NTFS: "NTFS    " at offset 3
    if data.len() > 11 && &data[3..11] == b"NTFS    " {
        return b"ntfs";
    }

    // XFS: "XFSB" at offset 0
    if data.len() > 4 && &data[0..4] == b"XFSB" {
        return b"xfs";
    }

    // Btrfs: "_BHRfS_M" at offset 0x10040
    if data.len() > 0x10048 && &data[0x10040..0x10048] == b"_BHRfS_M" {
        return b"btrfs";
    }

    // ReiserFS: "ReIsEr" at offset 0x10034
    if data.len() > 0x1003A && &data[0x10034..0x1003A] == b"ReIsEr" {
        return b"reiserfs";
    }

    // ISO9660: "CD001" at offset 0x8001
    if data.len() > 0x8006 && &data[0x8001..0x8006] == b"CD001" {
        return b"iso9660";
    }

    // Swap: "SWAPSPACE2" at offset 4086 (4K page - 10)
    if data.len() > 4096 && &data[4086..4096] == b"SWAPSPACE2" {
        return b"swap";
    }

    // squashfs: magic at offset 0
    if data.len() > 4 {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic == 0x73717368 {
            // "hsqs"
            return b"squashfs";
        }
    }

    b"unknown"
}

/// makedevs - create a range of device files
///
/// Usage: makedevs [-d device_table] rootdir
/// Or: makedevs NAME TYPE MAJOR MINOR FIRST LAST [s]
///
/// Creates device nodes based on a device table or command line arguments.
#[cfg(feature = "alloc")]
pub fn makedevs(argc: i32, argv: *const *const u8) -> i32 {
    use crate::sys;
    use alloc::vec::Vec;

    if argc < 2 {
        io::write_str(2, b"Usage: makedevs [-d device_table] rootdir\n");
        io::write_str(2, b"  Or:  makedevs NAME TYPE MAJOR MINOR FIRST LAST [s]\n");
        return 1;
    }

    let mut device_table: Option<&[u8]> = None;
    let mut rootdir: Option<&[u8]> = None;
    let mut i = 1;

    // Parse arguments
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-d" {
            i += 1;
            device_table = unsafe { get_arg(argv, i as i32) };
        } else if rootdir.is_none() {
            rootdir = Some(arg);
        }
        i += 1;
    }

    // If device table specified, parse it
    if let Some(table_path) = device_table {
        let root = rootdir.unwrap_or(b".");
        return process_device_table(table_path, root);
    }

    // Otherwise, expect command-line device specification
    // NAME TYPE MAJOR MINOR FIRST LAST [s]
    if argc < 6 {
        io::write_str(2, b"makedevs: not enough arguments\n");
        return 1;
    }

    let name = match unsafe { get_arg(argv, 1) } {
        Some(n) => n,
        None => return 1,
    };
    let dtype = match unsafe { get_arg(argv, 2) } {
        Some(t) => t,
        None => return 1,
    };
    let major = match unsafe { get_arg(argv, 3) }.and_then(|s| sys::parse_u64(s)) {
        Some(m) => m as u32,
        None => return 1,
    };
    let minor = match unsafe { get_arg(argv, 4) }.and_then(|s| sys::parse_u64(s)) {
        Some(m) => m as u32,
        None => return 1,
    };
    let first = match unsafe { get_arg(argv, 5) }.and_then(|s| sys::parse_u64(s)) {
        Some(f) => f as u32,
        None => 0,
    };
    let last = if argc > 6 {
        match unsafe { get_arg(argv, 6) }.and_then(|s| sys::parse_u64(s)) {
            Some(l) => l as u32,
            None => first,
        }
    } else {
        first
    };

    let mode = match dtype {
        b"c" | b"u" => libc::S_IFCHR | 0o660,
        b"b" => libc::S_IFBLK | 0o660,
        b"p" => libc::S_IFIFO | 0o660,
        _ => {
            io::write_str(2, b"makedevs: unknown device type\n");
            return 1;
        }
    };

    // Create devices
    for n in first..=last {
        let dev = sys::makedev(major, minor + n - first);
        let mut path: Vec<u8> = name.to_vec();

        if first != last {
            let mut num_buf = [0u8; 12];
            let num_str = sys::format_u64(n as u64, &mut num_buf);
            path.extend_from_slice(num_str);
        }
        path.push(0);

        let ret = unsafe { libc::mknod(path.as_ptr() as *const libc::c_char, mode, dev) };

        if ret < 0 {
            io::write_str(2, b"makedevs: failed to create ");
            io::write_all(2, &path[..path.len() - 1]);
            io::write_str(2, b"\n");
        }
    }

    0
}

#[cfg(not(feature = "alloc"))]
pub fn makedevs(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"makedevs: requires alloc feature\n");
    1
}

/// Process device table file
#[cfg(feature = "alloc")]
fn process_device_table(table_path: &[u8], rootdir: &[u8]) -> i32 {
    use crate::sys;
    use alloc::vec::Vec;

    let fd = io::open(table_path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"makedevs: cannot open device table\n");
        return 1;
    }

    let content = io::read_all(fd);
    io::close(fd);

    let mut status = 0;

    // Parse each line
    for line in content.split(|&c| c == b'\n') {
        let line = line.trim_ascii();
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        // Parse: <name> <type> <mode> <uid> <gid> <major> <minor> <start> <inc> <count>
        let parts: Vec<&[u8]> = line
            .split(|&c| c == b' ' || c == b'\t')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() < 5 {
            continue;
        }

        let name = parts[0];
        let dtype = parts[1];
        let mode = sys::parse_octal(parts[2]).unwrap_or(0o644);
        let uid = sys::parse_u64(parts[3]).unwrap_or(0) as libc::uid_t;
        let gid = sys::parse_u64(parts[4]).unwrap_or(0) as libc::gid_t;

        // Build full path
        let mut path: Vec<u8> = rootdir.to_vec();
        if !path.ends_with(b"/") {
            path.push(b'/');
        }
        // Skip leading / in name
        if name.starts_with(b"/") {
            path.extend_from_slice(&name[1..]);
        } else {
            path.extend_from_slice(name);
        }

        match dtype {
            b"d" => {
                // Directory
                path.push(0);
                let ret = unsafe {
                    libc::mkdir(path.as_ptr() as *const libc::c_char, mode as libc::mode_t)
                };
                if ret < 0 && crate::sys::errno() != libc::EEXIST {
                    status = 1;
                }
                unsafe {
                    libc::chown(path.as_ptr() as *const libc::c_char, uid, gid);
                }
            }
            b"f" => {
                // File - just set permissions
                path.push(0);
                unsafe {
                    libc::chmod(path.as_ptr() as *const libc::c_char, mode as libc::mode_t);
                    libc::chown(path.as_ptr() as *const libc::c_char, uid, gid);
                }
            }
            b"c" | b"b" => {
                // Character or block device
                if parts.len() >= 7 {
                    let major = sys::parse_u64(parts[5]).unwrap_or(0) as u32;
                    let minor = sys::parse_u64(parts[6]).unwrap_or(0) as u32;
                    let dev_mode = mode
                        | if dtype == b"c" {
                            libc::S_IFCHR as u32
                        } else {
                            libc::S_IFBLK as u32
                        };
                    let dev = sys::makedev(major, minor);

                    path.push(0);
                    unsafe {
                        libc::mknod(
                            path.as_ptr() as *const libc::c_char,
                            dev_mode as libc::mode_t,
                            dev,
                        );
                        libc::chown(path.as_ptr() as *const libc::c_char, uid, gid);
                    }
                }
            }
            _ => {}
        }
    }

    status
}

/// setfattr - set extended attributes of filesystem objects
///
/// Usage: setfattr [-n name] [-v value] [-x name] file...
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn setfattr(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(
            2,
            b"Usage: setfattr [-n name] [-v value] [-x name] file...\n",
        );
        return 1;
    }

    let mut attr_name: Option<&[u8]> = None;
    let mut attr_value: Option<&[u8]> = None;
    let mut remove_attr: Option<&[u8]> = None;
    let mut files_start = argc as usize;
    let mut i = 1;

    // Parse options
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-n" || arg == b"--name" {
            i += 1;
            attr_name = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-v" || arg == b"--value" {
            i += 1;
            attr_value = unsafe { get_arg(argv, i as i32) };
        } else if arg == b"-x" || arg == b"--remove" {
            i += 1;
            remove_attr = unsafe { get_arg(argv, i as i32) };
        } else if !arg.starts_with(b"-") {
            files_start = i;
            break;
        }
        i += 1;
    }

    let mut status = 0;

    // Process files
    for j in files_start..argc as usize {
        let path = match unsafe { get_arg(argv, j as i32) } {
            Some(p) => p,
            None => continue,
        };

        // Null-terminate path
        let mut path_buf = [0u8; 4096];
        let len = core::cmp::min(path.len(), path_buf.len() - 1);
        path_buf[..len].copy_from_slice(&path[..len]);

        if let Some(name) = remove_attr {
            // Remove attribute
            let mut name_buf = [0u8; 256];
            let nlen = core::cmp::min(name.len(), name_buf.len() - 1);
            name_buf[..nlen].copy_from_slice(&name[..nlen]);

            let ret = unsafe {
                libc::removexattr(
                    path_buf.as_ptr() as *const libc::c_char,
                    name_buf.as_ptr() as *const libc::c_char,
                )
            };
            if ret < 0 {
                io::write_str(2, b"setfattr: ");
                io::write_all(2, path);
                io::write_str(2, b": failed to remove attribute\n");
                status = 1;
            }
        } else if let Some(name) = attr_name {
            // Set attribute
            let value = attr_value.unwrap_or(b"");
            let mut name_buf = [0u8; 256];
            let nlen = core::cmp::min(name.len(), name_buf.len() - 1);
            name_buf[..nlen].copy_from_slice(&name[..nlen]);

            let ret = unsafe {
                libc::setxattr(
                    path_buf.as_ptr() as *const libc::c_char,
                    name_buf.as_ptr() as *const libc::c_char,
                    value.as_ptr() as *const libc::c_void,
                    value.len(),
                    0,
                )
            };
            if ret < 0 {
                io::write_str(2, b"setfattr: ");
                io::write_all(2, path);
                io::write_str(2, b": failed to set attribute\n");
                status = 1;
            }
        }
    }

    status
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn setfattr(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"setfattr: only available on Linux\n");
    1
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
    fn test_chattr_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["chattr"]).output().unwrap();

        // Returns 1 for usage error (no arguments)
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_lsattr_basic() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["lsattr"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_fstype_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["fstype"]).output().unwrap();

        // Returns 1 for usage error (no device specified)
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_makedevs_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["makedevs"]).output().unwrap();

        // Returns 1 for usage error (no arguments)
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }

    #[test]
    fn test_setfattr_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["setfattr"]).output().unwrap();

        // Returns 1 for usage error (no arguments)
        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Usage"));
    }
}
