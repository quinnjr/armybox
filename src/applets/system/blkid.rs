//! blkid - locate/print block device attributes
//!
//! Print block device attributes like UUID, TYPE, and LABEL by reading
//! filesystem superblocks.

extern crate alloc;

use crate::applets::get_arg;
use crate::io;
use alloc::vec::Vec;

/// Filesystem detection result
struct FsInfo {
    fs_type: &'static [u8],
    uuid: Option<[u8; 16]>,
    label: Option<Vec<u8>>,
}

/// blkid - locate/print block device attributes
///
/// # Synopsis
/// ```text
/// blkid [-o format] [-s tag] [device...]
/// ```
///
/// # Description
/// Print block device attributes by reading filesystem superblocks.
/// Detects UUID, TYPE (filesystem type), and LABEL.
///
/// # Options
/// - `-o format`: Output format (full, value, device, export)
/// - `-s tag`: Show only specified tag (UUID, TYPE, LABEL)
/// - `-c /dev/null`: Don't use cache (for compatibility, ignored)
///
/// # Exit Status
/// - 0: Success
/// - 2: No devices found or specified device not found
pub fn blkid(argc: i32, argv: *const *const u8) -> i32 {
    let mut output_format = OutputFormat::Full;
    let mut show_tag: Option<&[u8]> = None;
    let mut devices: Vec<&[u8]> = Vec::new();

    // Parse arguments
    let mut i = 1i32;
    while i < argc {
        let Some(arg) = (unsafe { get_arg(argv, i) }) else {
            i += 1;
            continue;
        };
        if arg.starts_with(b"-") {
            if arg == b"-o" {
                i += 1;
                if let Some(fmt) = unsafe { get_arg(argv, i) } {
                    output_format = match fmt {
                        b"full" => OutputFormat::Full,
                        b"value" => OutputFormat::Value,
                        b"device" => OutputFormat::Device,
                        b"export" => OutputFormat::Export,
                        _ => {
                            io::write_all(2, b"blkid: unknown output format\n");
                            return 1;
                        }
                    };
                }
            } else if arg == b"-s" {
                i += 1;
                if let Some(tag) = unsafe { get_arg(argv, i) } {
                    show_tag = Some(tag);
                }
            } else if arg == b"-c" {
                // Skip cache file argument (compatibility)
                i += 1;
            } else if arg == b"-h" || arg == b"--help" {
                io::write_all(1, b"Usage: blkid [-o format] [-s tag] [device...]\n");
                io::write_all(
                    1,
                    b"  -o format  Output format: full, value, device, export\n",
                );
                io::write_all(
                    1,
                    b"  -s tag     Show only specified tag (UUID, TYPE, LABEL)\n",
                );
                return 0;
            }
        } else {
            devices.push(arg);
        }
        i += 1;
    }

    // If no devices specified, scan all block devices
    if devices.is_empty() {
        return scan_all_devices(output_format, show_tag);
    }

    // Process specified devices
    let mut found_any = false;
    for device in devices {
        if let Some(info) = probe_device(device) {
            print_device_info(device, &info, output_format, show_tag);
            found_any = true;
        }
    }

    if found_any { 0 } else { 2 }
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Full,   // /dev/sda1: UUID="..." TYPE="..."
    Value,  // Just the value
    Device, // Just device name
    Export, // DEVNAME=/dev/sda1\nUUID=...\n
}

/// Scan all block devices
fn scan_all_devices(format: OutputFormat, show_tag: Option<&[u8]>) -> i32 {
    let mut found_any = false;

    // Read /sys/block to find block devices
    let dir_fd = unsafe {
        libc::open(
            b"/sys/block\0".as_ptr() as *const libc::c_char,
            libc::O_RDONLY | libc::O_DIRECTORY,
        )
    };
    if dir_fd < 0 {
        return 2;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, dir_fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }

        let mut offset = 0usize;
        while offset < n as usize {
            let d_reclen = u16::from_ne_bytes([buf[offset + 16], buf[offset + 17]]) as usize;
            let d_type = buf[offset + 18];

            // Find name (null-terminated after d_type)
            let name_start = offset + 19;
            let mut name_end = name_start;
            while name_end < offset + d_reclen && buf[name_end] != 0 {
                name_end += 1;
            }
            let name = &buf[name_start..name_end];

            // Skip . and .. and non-directories
            if name != b"." && name != b".." && (d_type == 4 || d_type == 10) {
                // Check this device and its partitions
                scan_block_device(name, format, show_tag, &mut found_any);
            }

            offset += d_reclen;
        }
    }

    unsafe { libc::close(dir_fd) };

    if found_any { 0 } else { 2 }
}

/// Scan a block device and its partitions
fn scan_block_device(
    name: &[u8],
    format: OutputFormat,
    show_tag: Option<&[u8]>,
    found_any: &mut bool,
) {
    // Skip loop, ram, and dm devices for cleaner output (like real blkid)
    if name.starts_with(b"loop") || name.starts_with(b"ram") {
        return;
    }

    // Build device path /dev/NAME
    let mut dev_path = Vec::with_capacity(name.len() + 6);
    dev_path.extend_from_slice(b"/dev/");
    dev_path.extend_from_slice(name);

    // Try the main device
    if let Some(info) = probe_device(&dev_path) {
        print_device_info(&dev_path, &info, format, show_tag);
        *found_any = true;
    }

    // Look for partitions in /sys/block/NAME/
    let mut sys_path = Vec::with_capacity(name.len() + 16);
    sys_path.extend_from_slice(b"/sys/block/");
    sys_path.extend_from_slice(name);
    sys_path.push(0);

    let dir_fd = unsafe {
        libc::open(
            sys_path.as_ptr() as *const libc::c_char,
            libc::O_RDONLY | libc::O_DIRECTORY,
        )
    };
    if dir_fd < 0 {
        return;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, dir_fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }

        let mut offset = 0usize;
        while offset < n as usize {
            let d_reclen = u16::from_ne_bytes([buf[offset + 16], buf[offset + 17]]) as usize;

            let name_start = offset + 19;
            let mut name_end = name_start;
            while name_end < offset + d_reclen && buf[name_end] != 0 {
                name_end += 1;
            }
            let part_name = &buf[name_start..name_end];

            // Partitions are named like sda1, nvme0n1p1 - they start with the device name
            if part_name.starts_with(name) && part_name.len() > name.len() {
                let mut part_dev_path = Vec::with_capacity(part_name.len() + 6);
                part_dev_path.extend_from_slice(b"/dev/");
                part_dev_path.extend_from_slice(part_name);

                if let Some(info) = probe_device(&part_dev_path) {
                    print_device_info(&part_dev_path, &info, format, show_tag);
                    *found_any = true;
                }
            }

            offset += d_reclen;
        }
    }

    unsafe { libc::close(dir_fd) };
}

/// Probe a device and detect filesystem
fn probe_device(path: &[u8]) -> Option<FsInfo> {
    // Need null-terminated path
    let mut path_z = Vec::with_capacity(path.len() + 1);
    path_z.extend_from_slice(path);
    path_z.push(0);

    let fd = unsafe { libc::open(path_z.as_ptr() as *const libc::c_char, libc::O_RDONLY) };
    if fd < 0 {
        return None;
    }

    // Read first 128KB to catch all filesystem superblocks
    let mut buf = [0u8; 131072];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
    unsafe { libc::close(fd) };

    if n < 512 {
        return None;
    }
    let buf = &buf[..n as usize];

    // Try each filesystem detector
    if let Some(info) = detect_ext234(buf) {
        return Some(info);
    }
    if let Some(info) = detect_xfs(buf) {
        return Some(info);
    }
    if let Some(info) = detect_btrfs(buf) {
        return Some(info);
    }
    if let Some(info) = detect_swap(buf) {
        return Some(info);
    }
    if let Some(info) = detect_ntfs(buf) {
        return Some(info);
    }
    if let Some(info) = detect_vfat(buf) {
        return Some(info);
    }
    if let Some(info) = detect_iso9660(buf) {
        return Some(info);
    }
    if let Some(info) = detect_squashfs(buf) {
        return Some(info);
    }
    if let Some(info) = detect_f2fs(buf) {
        return Some(info);
    }

    None
}

/// Detect ext2/3/4 filesystem
fn detect_ext234(buf: &[u8]) -> Option<FsInfo> {
    // Superblock at offset 1024, magic at offset 0x38 (56) within superblock = 0x438
    if buf.len() < 0x480 {
        return None;
    }

    let magic = u16::from_le_bytes([buf[0x438], buf[0x439]]);
    if magic != 0xEF53 {
        return None;
    }

    // Determine ext2/3/4 by feature flags
    let compat_features = u32::from_le_bytes([buf[0x45C], buf[0x45D], buf[0x45E], buf[0x45F]]);
    let incompat_features = u32::from_le_bytes([buf[0x460], buf[0x461], buf[0x462], buf[0x463]]);
    let _ro_compat_features = u32::from_le_bytes([buf[0x464], buf[0x465], buf[0x466], buf[0x467]]);

    // ext4 features
    let has_extents = (incompat_features & 0x40) != 0;
    let has_64bit = (incompat_features & 0x80) != 0;
    let has_flex_bg = (incompat_features & 0x200) != 0;

    // ext3 feature: has_journal
    let has_journal = (compat_features & 0x4) != 0;

    let fs_type: &'static [u8] = if has_extents || has_64bit || has_flex_bg {
        b"ext4"
    } else if has_journal {
        b"ext3"
    } else {
        b"ext2"
    };

    // UUID at offset 0x468 (16 bytes)
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&buf[0x468..0x478]);

    // Label at offset 0x478 (16 bytes, null-terminated)
    let label = extract_label(&buf[0x478..0x488]);

    Some(FsInfo {
        fs_type,
        uuid: if uuid != [0u8; 16] { Some(uuid) } else { None },
        label,
    })
}

/// Detect XFS filesystem
fn detect_xfs(buf: &[u8]) -> Option<FsInfo> {
    if buf.len() < 0x100 {
        return None;
    }

    // Magic "XFSB" at offset 0
    if &buf[0..4] != b"XFSB" {
        return None;
    }

    // UUID at offset 0x20 (32)
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&buf[0x20..0x30]);

    // Label at offset 0x6C (108), 12 bytes
    let label = extract_label(&buf[0x6C..0x78]);

    Some(FsInfo {
        fs_type: b"xfs",
        uuid: if uuid != [0u8; 16] { Some(uuid) } else { None },
        label,
    })
}

/// Detect Btrfs filesystem
fn detect_btrfs(buf: &[u8]) -> Option<FsInfo> {
    // Btrfs superblock at 0x10000 (64KB), magic at offset 0x40 = "_BHRfS_M"
    if buf.len() < 0x10100 {
        return None;
    }

    let sb_offset = 0x10000;
    if &buf[sb_offset + 0x40..sb_offset + 0x48] != b"_BHRfS_M" {
        return None;
    }

    // UUID at offset 0x20 in superblock
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&buf[sb_offset + 0x20..sb_offset + 0x30]);

    // Label at offset 0x12B in superblock (256 bytes)
    let label =
        extract_label(&buf[sb_offset + 0x12B..sb_offset + 0x22B.min(buf.len() - sb_offset)]);

    Some(FsInfo {
        fs_type: b"btrfs",
        uuid: if uuid != [0u8; 16] { Some(uuid) } else { None },
        label,
    })
}

/// Detect Linux swap
fn detect_swap(buf: &[u8]) -> Option<FsInfo> {
    // Swap magic at end of first page (typically 4096, but could be other page sizes)
    // Magic is "SWAPSPACE2" at offset pagesize - 10

    for &pagesize in &[4096usize, 8192, 16384, 65536] {
        if buf.len() >= pagesize {
            let magic_offset = pagesize - 10;
            if &buf[magic_offset..magic_offset + 10] == b"SWAPSPACE2"
                || &buf[magic_offset..magic_offset + 10] == b"SWAP-SPACE"
            {
                // UUID at offset 0x40C in swap header (swap_header_v1_2)
                let mut uuid = [0u8; 16];
                if buf.len() >= 0x41C {
                    uuid.copy_from_slice(&buf[0x40C..0x41C]);
                }

                // Label at offset 0x41C (16 bytes)
                let label = if buf.len() >= 0x42C {
                    extract_label(&buf[0x41C..0x42C])
                } else {
                    None
                };

                return Some(FsInfo {
                    fs_type: b"swap",
                    uuid: if uuid != [0u8; 16] { Some(uuid) } else { None },
                    label,
                });
            }
        }
    }

    None
}

/// Detect NTFS filesystem
fn detect_ntfs(buf: &[u8]) -> Option<FsInfo> {
    if buf.len() < 512 {
        return None;
    }

    // "NTFS    " at offset 3
    if &buf[3..11] != b"NTFS    " {
        return None;
    }

    // NTFS volume serial number at offset 0x48 (8 bytes)
    // This is used as a pseudo-UUID
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&buf[0x48..0x50]);

    // NTFS doesn't have a label in the boot sector - it's in the $Volume file
    // We can't easily read that without parsing NTFS structures

    Some(FsInfo {
        fs_type: b"ntfs",
        uuid: None, // NTFS uses serial number, not UUID
        label: None,
    })
}

/// Detect FAT12/16/32 and exFAT
fn detect_vfat(buf: &[u8]) -> Option<FsInfo> {
    if buf.len() < 512 {
        return None;
    }

    // Check boot sector signature
    if buf[510] != 0x55 || buf[511] != 0xAA {
        return None;
    }

    // Check for exFAT first (has "EXFAT   " at offset 3)
    if &buf[3..11] == b"EXFAT   " {
        // exFAT volume serial at offset 100-103
        let label = extract_label(&buf[0x47..0x52]);
        return Some(FsInfo {
            fs_type: b"exfat",
            uuid: None,
            label,
        });
    }

    // FAT32 has "FAT32   " at offset 0x52
    if &buf[0x52..0x5A] == b"FAT32   " {
        // Volume serial at 0x43 (4 bytes)
        let serial = &buf[0x43..0x47];
        // Label at 0x47 (11 bytes)
        let label = extract_label(&buf[0x47..0x52]);

        // Create pseudo-UUID from serial
        let mut uuid = [0u8; 16];
        uuid[0..4].copy_from_slice(serial);

        return Some(FsInfo {
            fs_type: b"vfat",
            uuid: Some(uuid),
            label,
        });
    }

    // FAT12/16 has "FAT     " or "FAT12   " or "FAT16   " at offset 0x36
    if buf[0x36..0x3B].starts_with(b"FAT") {
        // Volume serial at 0x27 (4 bytes)
        let serial = &buf[0x27..0x2B];
        // Label at 0x2B (11 bytes)
        let label = extract_label(&buf[0x2B..0x36]);

        let mut uuid = [0u8; 16];
        uuid[0..4].copy_from_slice(serial);

        return Some(FsInfo {
            fs_type: b"vfat",
            uuid: Some(uuid),
            label,
        });
    }

    None
}

/// Detect ISO9660 (CD/DVD filesystem)
fn detect_iso9660(buf: &[u8]) -> Option<FsInfo> {
    // Primary volume descriptor at sector 16 (offset 0x8000)
    // Magic "CD001" at offset 1 in the descriptor
    if buf.len() < 0x8800 {
        return None;
    }

    let pvd_offset = 0x8000;
    if &buf[pvd_offset + 1..pvd_offset + 6] != b"CD001" {
        return None;
    }

    // Volume identifier at offset 40 (32 bytes)
    let label = extract_label(&buf[pvd_offset + 40..pvd_offset + 72]);

    Some(FsInfo {
        fs_type: b"iso9660",
        uuid: None,
        label,
    })
}

/// Detect squashfs
fn detect_squashfs(buf: &[u8]) -> Option<FsInfo> {
    if buf.len() < 96 {
        return None;
    }

    // Magic at offset 0: "hsqs" (little-endian) or "sqsh" (big-endian)
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0x73717368 {
        // "hsqs"
        // Try big-endian
        let magic_be = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic_be != 0x73717368 {
            return None;
        }
    }

    Some(FsInfo {
        fs_type: b"squashfs",
        uuid: None,
        label: None,
    })
}

/// Detect F2FS (Flash-Friendly File System)
fn detect_f2fs(buf: &[u8]) -> Option<FsInfo> {
    // F2FS superblock at offset 0x400 (1024)
    // Magic at offset 0 in superblock = 0xF2F52010
    if buf.len() < 0x500 {
        return None;
    }

    let sb_offset = 0x400;
    let magic = u32::from_le_bytes([
        buf[sb_offset],
        buf[sb_offset + 1],
        buf[sb_offset + 2],
        buf[sb_offset + 3],
    ]);

    if magic != 0xF2F52010 {
        return None;
    }

    // UUID at offset 0x6C in superblock (16 bytes)
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&buf[sb_offset + 0x6C..sb_offset + 0x7C]);

    // Volume name at offset 0x7C (512 bytes, UTF-16LE)
    // For simplicity, try to extract ASCII portion
    let label =
        extract_utf16_label(&buf[sb_offset + 0x7C..sb_offset + 0x27C.min(buf.len() - sb_offset)]);

    Some(FsInfo {
        fs_type: b"f2fs",
        uuid: if uuid != [0u8; 16] { Some(uuid) } else { None },
        label,
    })
}

/// Extract a null-terminated or space-padded label
fn extract_label(data: &[u8]) -> Option<Vec<u8>> {
    // Find actual end (trim nulls and spaces from end)
    let mut end = data.len();
    while end > 0 && (data[end - 1] == 0 || data[end - 1] == b' ') {
        end -= 1;
    }

    if end == 0 {
        return None;
    }

    Some(data[..end].to_vec())
}

/// Extract label from UTF-16LE data (like F2FS volume names)
fn extract_utf16_label(data: &[u8]) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    let mut i = 0;

    while i + 1 < data.len() {
        let c = u16::from_le_bytes([data[i], data[i + 1]]);
        if c == 0 {
            break;
        }
        // Only keep ASCII characters
        if c < 128 {
            result.push(c as u8);
        }
        i += 2;
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Print device info in the requested format
fn print_device_info(path: &[u8], info: &FsInfo, format: OutputFormat, show_tag: Option<&[u8]>) {
    match format {
        OutputFormat::Full => {
            // /dev/sda1: UUID="xxxx" TYPE="ext4" LABEL="..."
            io::write_all(1, path);
            io::write_all(1, b":");

            if let Some(ref tag) = show_tag {
                // Show only specific tag
                if *tag == b"UUID" {
                    if let Some(uuid) = &info.uuid {
                        io::write_all(1, b" UUID=\"");
                        print_uuid(uuid);
                        io::write_all(1, b"\"");
                    }
                } else if *tag == b"TYPE" {
                    io::write_all(1, b" TYPE=\"");
                    io::write_all(1, info.fs_type);
                    io::write_all(1, b"\"");
                } else if *tag == b"LABEL" {
                    if let Some(label) = &info.label {
                        io::write_all(1, b" LABEL=\"");
                        io::write_all(1, label);
                        io::write_all(1, b"\"");
                    }
                }
            } else {
                // Show all tags
                if let Some(uuid) = &info.uuid {
                    io::write_all(1, b" UUID=\"");
                    print_uuid(uuid);
                    io::write_all(1, b"\"");
                }
                io::write_all(1, b" TYPE=\"");
                io::write_all(1, info.fs_type);
                io::write_all(1, b"\"");
                if let Some(label) = &info.label {
                    io::write_all(1, b" LABEL=\"");
                    io::write_all(1, label);
                    io::write_all(1, b"\"");
                }
            }
            io::write_all(1, b"\n");
        }
        OutputFormat::Value => {
            // Just print the value(s)
            if let Some(ref tag) = show_tag {
                if *tag == b"UUID" {
                    if let Some(uuid) = &info.uuid {
                        print_uuid(uuid);
                        io::write_all(1, b"\n");
                    }
                } else if *tag == b"TYPE" {
                    io::write_all(1, info.fs_type);
                    io::write_all(1, b"\n");
                } else if *tag == b"LABEL" {
                    if let Some(label) = &info.label {
                        io::write_all(1, label);
                        io::write_all(1, b"\n");
                    }
                }
            } else {
                // Print all values on separate lines
                if let Some(uuid) = &info.uuid {
                    print_uuid(uuid);
                    io::write_all(1, b"\n");
                }
                io::write_all(1, info.fs_type);
                io::write_all(1, b"\n");
                if let Some(label) = &info.label {
                    io::write_all(1, label);
                    io::write_all(1, b"\n");
                }
            }
        }
        OutputFormat::Device => {
            io::write_all(1, path);
            io::write_all(1, b"\n");
        }
        OutputFormat::Export => {
            // DEVNAME=/dev/sda1
            // UUID=xxxx
            // TYPE=ext4
            io::write_all(1, b"DEVNAME=");
            io::write_all(1, path);
            io::write_all(1, b"\n");
            if let Some(uuid) = &info.uuid {
                io::write_all(1, b"UUID=");
                print_uuid(uuid);
                io::write_all(1, b"\n");
            }
            io::write_all(1, b"TYPE=");
            io::write_all(1, info.fs_type);
            io::write_all(1, b"\n");
            if let Some(label) = &info.label {
                io::write_all(1, b"LABEL=");
                io::write_all(1, label);
                io::write_all(1, b"\n");
            }
            io::write_all(1, b"\n");
        }
    }
}

/// Print UUID in canonical format (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
fn print_uuid(uuid: &[u8; 16]) {
    let hex = b"0123456789abcdef";
    let mut out = [0u8; 36];

    // Format: 8-4-4-4-12
    let mut j = 0;
    for (i, &b) in uuid.iter().enumerate() {
        out[j] = hex[(b >> 4) as usize];
        out[j + 1] = hex[(b & 0xf) as usize];
        j += 2;

        if i == 3 || i == 5 || i == 7 || i == 9 {
            out[j] = b'-';
            j += 1;
        }
    }

    io::write_all(1, &out);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_blkid_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["blkid"]).output().unwrap();

        // Exit code 0 if found devices, 2 if no devices (both acceptable in tests)
        assert!(output.status.code() == Some(0) || output.status.code() == Some(2));
    }

    #[test]
    fn test_blkid_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["blkid", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.starts_with(b"Usage:"));
    }
}
