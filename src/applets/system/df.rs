//! df - report file system disk space usage
//!
//! Display information about file system disk space usage.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// statfs structure
#[repr(C)]
struct StatFs {
    f_type: libc::c_ulong,
    f_bsize: libc::c_ulong,
    f_blocks: u64,
    f_bfree: u64,
    f_bavail: u64,
    f_files: u64,
    f_ffree: u64,
    f_fsid: [i32; 2],
    f_namelen: libc::c_ulong,
    f_frsize: libc::c_ulong,
    f_flags: libc::c_ulong,
    f_spare: [libc::c_ulong; 4],
}

unsafe extern "C" {
    fn statfs(path: *const libc::c_char, buf: *mut StatFs) -> libc::c_int;
}

/// df - report file system disk space usage
///
/// # Synopsis
/// ```text
/// df [-h] [-H] [-i] [-T] [-a] [FILE...]
/// ```
///
/// # Description
/// Display information about file system disk space usage for all mounted
/// file systems, or for the file systems containing the specified FILEs.
///
/// # Options
/// - `-h`: Human-readable output (powers of 1024)
/// - `-H`: Human-readable output (powers of 1000)
/// - `-i`: Show inode information instead of blocks
/// - `-T`: Show filesystem type
/// - `-a`: Include pseudo filesystems
/// - `-P`: Use POSIX output format
/// - `-k`: Use 1K blocks (default)
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn df(argc: i32, argv: *const *const u8) -> i32 {
    let mut human_readable = false;
    let mut si_units = false;
    let mut show_inodes = false;
    let mut show_type = false;
    let mut show_all = false;
    let mut posix_format = false;
    let mut paths: Vec<&[u8]> = Vec::new();

    // Parse arguments
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-h" || arg == b"--human-readable" {
                human_readable = true;
            } else if arg == b"-H" || arg == b"--si" {
                si_units = true;
            } else if arg == b"-i" || arg == b"--inodes" {
                show_inodes = true;
            } else if arg == b"-T" || arg == b"--print-type" {
                show_type = true;
            } else if arg == b"-a" || arg == b"--all" {
                show_all = true;
            } else if arg == b"-P" || arg == b"--portability" {
                posix_format = true;
            } else if arg == b"-k" {
                // Default, 1K blocks
            } else if arg == b"--help" {
                print_help();
                return 0;
            } else if !arg.starts_with(b"-") {
                paths.push(arg);
            }
        }
    }

    // Print header
    if show_inodes {
        if show_type {
            io::write_str(1, b"Filesystem           Type        Inodes       IUsed       IFree IUse% Mounted on\n");
        } else {
            io::write_str(
                1,
                b"Filesystem                Inodes       IUsed       IFree IUse% Mounted on\n",
            );
        }
    } else {
        if show_type {
            io::write_str(1, b"Filesystem           Type      1K-blocks         Used    Available Use% Mounted on\n");
        } else {
            io::write_str(
                1,
                b"Filesystem              1K-blocks         Used    Available Use% Mounted on\n",
            );
        }
    }

    let base = if si_units { 1000u64 } else { 1024u64 };

    if paths.is_empty() {
        // Show all mounted filesystems
        show_all_mounts(
            human_readable,
            si_units,
            show_inodes,
            show_type,
            show_all,
            posix_format,
            base,
        );
    } else {
        // Show specific paths
        for path in &paths {
            show_filesystem(path, human_readable, si_units, show_inodes, show_type, base);
        }
    }

    0
}

/// Show all mounted filesystems
fn show_all_mounts(
    human: bool,
    si: bool,
    inodes: bool,
    show_type: bool,
    show_all: bool,
    _posix: bool,
    base: u64,
) {
    let fd = io::open(b"/proc/mounts", libc::O_RDONLY, 0);
    if fd < 0 {
        // Fallback to /etc/mtab
        let fd = io::open(b"/etc/mtab", libc::O_RDONLY, 0);
        if fd < 0 {
            io::write_str(2, b"df: cannot read mount information\n");
            return;
        }
    }

    let content = io::read_all(fd);
    io::close(fd);

    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() {
            continue;
        }

        // Format: device mountpoint fstype options dump pass
        let fields: Vec<&[u8]> = line.split(|&c| c == b' ').collect();
        if fields.len() < 3 {
            continue;
        }

        let device = fields[0];
        let mountpoint = fields[1];
        let fstype = fields[2];

        // Skip pseudo filesystems unless -a
        if !show_all {
            let skip_types = [
                b"proc" as &[u8],
                b"sysfs",
                b"devtmpfs",
                b"devpts",
                b"tmpfs",
                b"cgroup",
                b"cgroup2",
                b"securityfs",
                b"debugfs",
                b"pstore",
                b"bpf",
                b"tracefs",
                b"hugetlbfs",
                b"mqueue",
                b"fusectl",
                b"configfs",
                b"ramfs",
                b"efivarfs",
                b"binfmt_misc",
                b"autofs",
                b"rpc_pipefs",
                b"nfsd",
                b"overlay",
            ];
            if skip_types.iter().any(|&t| t == fstype) {
                continue;
            }
            // Skip if device doesn't start with /
            if !device.starts_with(b"/") {
                continue;
            }
        }

        show_mount_entry(
            device, mountpoint, fstype, human, si, inodes, show_type, base,
        );
    }
}

/// Show a single mount entry
fn show_mount_entry(
    device: &[u8],
    mountpoint: &[u8],
    fstype: &[u8],
    human: bool,
    si: bool,
    inodes: bool,
    show_type: bool,
    base: u64,
) {
    // Create null-terminated path
    let mut path_buf = [0u8; 512];
    let mp_len = core::cmp::min(mountpoint.len(), path_buf.len() - 1);
    path_buf[..mp_len].copy_from_slice(&mountpoint[..mp_len]);

    // Get filesystem stats
    let mut stat_buf: StatFs = unsafe { core::mem::zeroed() };
    let ret = unsafe { statfs(path_buf.as_ptr() as *const libc::c_char, &mut stat_buf) };

    if ret < 0 {
        return;
    }

    let block_size = if stat_buf.f_frsize > 0 {
        stat_buf.f_frsize
    } else {
        stat_buf.f_bsize
    };

    // Print device name (left-aligned, 20 chars)
    io::write_all(1, device);
    let dev_len = device.len();
    for _ in dev_len..21 {
        io::write_str(1, b" ");
    }

    // Print type if requested
    if show_type {
        io::write_all(1, fstype);
        let type_len = fstype.len();
        for _ in type_len..10 {
            io::write_str(1, b" ");
        }
    }

    let mut buf = [0u8; 16];

    if inodes {
        // Inode information
        let total_inodes = stat_buf.f_files;
        let free_inodes = stat_buf.f_ffree;
        let used_inodes = total_inodes.saturating_sub(free_inodes);
        let use_percent = if total_inodes > 0 {
            ((used_inodes * 100) / total_inodes) as u32
        } else {
            0
        };

        // Total inodes (12 chars)
        print_size_padded(total_inodes, human, si, base, 12);

        // Used inodes (12 chars)
        print_size_padded(used_inodes, human, si, base, 12);

        // Free inodes (12 chars)
        print_size_padded(free_inodes, human, si, base, 12);

        // Use%
        let pct = sys::format_u64(use_percent as u64, &mut buf);
        for _ in pct.len()..4 {
            io::write_str(1, b" ");
        }
        io::write_all(1, pct);
        io::write_str(1, b"% ");
    } else {
        // Block information - convert to 1K blocks
        let total_kb = (stat_buf.f_blocks * block_size as u64) / 1024;
        let avail_kb = (stat_buf.f_bavail * block_size as u64) / 1024;
        let free_kb = (stat_buf.f_bfree * block_size as u64) / 1024;
        let used_kb = total_kb.saturating_sub(free_kb);

        // Use percentage based on total - (free - avail) for reserved blocks
        let usable = total_kb.saturating_sub(free_kb.saturating_sub(avail_kb));
        let use_percent = if usable > 0 {
            ((used_kb * 100) / usable).min(100) as u32
        } else {
            0
        };

        // Total (14 chars to handle TB-size disks)
        print_size_padded(total_kb, human, si, base, 14);

        // Used (13 chars)
        print_size_padded(used_kb, human, si, base, 13);

        // Available (13 chars)
        print_size_padded(avail_kb, human, si, base, 13);

        // Use%
        let pct = sys::format_u64(use_percent as u64, &mut buf);
        for _ in pct.len()..4 {
            io::write_str(1, b" ");
        }
        io::write_all(1, pct);
        io::write_str(1, b"% ");
    }

    // Mountpoint
    io::write_all(1, mountpoint);
    io::write_str(1, b"\n");
}

/// Show filesystem for a specific path
fn show_filesystem(path: &[u8], human: bool, si: bool, inodes: bool, show_type: bool, base: u64) {
    // Create null-terminated path
    let mut path_buf = [0u8; 512];
    let path_len = core::cmp::min(path.len(), path_buf.len() - 1);
    path_buf[..path_len].copy_from_slice(&path[..path_len]);

    // Get filesystem stats
    let mut stat_buf: StatFs = unsafe { core::mem::zeroed() };
    let ret = unsafe { statfs(path_buf.as_ptr() as *const libc::c_char, &mut stat_buf) };

    if ret < 0 {
        io::write_str(2, b"df: ");
        io::write_all(2, path);
        io::write_str(2, b": cannot stat\n");
        return;
    }

    // Find the mount entry for this path
    let (device, mountpoint, fstype) = find_mount_for_path(path);
    show_mount_entry(
        &device,
        &mountpoint,
        &fstype,
        human,
        si,
        inodes,
        show_type,
        base,
    );
}

/// Find mount entry for a given path
fn find_mount_for_path(target: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let fd = io::open(b"/proc/mounts", libc::O_RDONLY, 0);
    if fd < 0 {
        return (b"-".to_vec(), target.to_vec(), b"unknown".to_vec());
    }

    let content = io::read_all(fd);
    io::close(fd);

    let mut best_match: Option<(Vec<u8>, Vec<u8>, Vec<u8>)> = None;
    let mut best_len = 0;

    for line in content.split(|&c| c == b'\n') {
        if line.is_empty() {
            continue;
        }

        let fields: Vec<&[u8]> = line.split(|&c| c == b' ').collect();
        if fields.len() < 3 {
            continue;
        }

        let device = fields[0];
        let mountpoint = fields[1];
        let fstype = fields[2];

        // Check if target starts with this mountpoint
        if target.starts_with(mountpoint) && mountpoint.len() > best_len {
            // Make sure it's a proper path match
            if mountpoint.len() == target.len()
                || (target.len() > mountpoint.len() && target[mountpoint.len()] == b'/')
                || mountpoint == b"/"
            {
                best_len = mountpoint.len();
                best_match = Some((device.to_vec(), mountpoint.to_vec(), fstype.to_vec()));
            }
        }
    }

    best_match.unwrap_or_else(|| (b"-".to_vec(), target.to_vec(), b"unknown".to_vec()))
}

/// Print size with optional human-readable format
fn print_size_padded(size_kb: u64, human: bool, _si: bool, base: u64, width: usize) {
    let mut buf = [0u8; 16];

    if human {
        let (val, suffix) = humanize(size_kb * 1024, base);
        let s = sys::format_u64(val, &mut buf);
        for _ in (s.len() + 1)..width {
            io::write_str(1, b" ");
        }
        io::write_all(1, s);
        io::write_str(1, suffix);
    } else {
        let s = sys::format_u64(size_kb, &mut buf);
        for _ in s.len()..width {
            io::write_str(1, b" ");
        }
        io::write_all(1, s);
    }
}

/// Convert bytes to human-readable with suffix
fn humanize(bytes: u64, base: u64) -> (u64, &'static [u8]) {
    if bytes >= base * base * base * base {
        (
            bytes / (base * base * base * base),
            if base == 1000 { b"T" } else { b"T" },
        )
    } else if bytes >= base * base * base {
        (
            bytes / (base * base * base),
            if base == 1000 { b"G" } else { b"G" },
        )
    } else if bytes >= base * base {
        (
            bytes / (base * base),
            if base == 1000 { b"M" } else { b"M" },
        )
    } else if bytes >= base {
        (bytes / base, if base == 1000 { b"K" } else { b"K" })
    } else {
        (bytes, b"B")
    }
}

fn print_help() {
    io::write_str(1, b"Usage: df [OPTIONS] [FILE...]\n\n");
    io::write_str(1, b"Show disk space usage for file systems.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(
        1,
        b"  -h, --human-readable  Human-readable sizes (1K=1024)\n",
    );
    io::write_str(
        1,
        b"  -H, --si              Human-readable sizes (1K=1000)\n",
    );
    io::write_str(1, b"  -i, --inodes          Show inode information\n");
    io::write_str(1, b"  -T, --print-type      Show filesystem type\n");
    io::write_str(1, b"  -a, --all             Include pseudo filesystems\n");
    io::write_str(1, b"  -P, --portability     Use POSIX output format\n");
    io::write_str(1, b"  -k                    Use 1K blocks (default)\n");
    io::write_str(1, b"      --help            Show this help\n");
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
    fn test_df_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_df_has_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Filesystem"));
        assert!(stdout.contains("1K-blocks") || stdout.contains("Used"));
    }

    #[test]
    fn test_df_human_readable() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df", "-h"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain human-readable suffixes
        assert!(stdout.contains("G") || stdout.contains("M") || stdout.contains("K"));
    }

    #[test]
    fn test_df_show_type() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df", "-T"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Type"));
    }

    #[test]
    fn test_df_inodes() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df", "-i"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Inodes"));
    }

    #[test]
    fn test_df_specific_path() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["df", "/"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should show at least the root filesystem
        assert!(stdout.lines().count() >= 2); // header + at least one entry
    }

    #[test]
    fn test_df_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["df", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
