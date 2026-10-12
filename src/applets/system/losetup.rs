//! losetup - set up and control loop devices
//!
//! Associate loop devices with regular files.

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

// Loop device ioctl commands
#[cfg(any(target_os = "linux", target_os = "android"))]
const LOOP_SET_FD: crate::io::IoctlReq = 0x4C00u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LOOP_CLR_FD: crate::io::IoctlReq = 0x4C01u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LOOP_SET_STATUS64: crate::io::IoctlReq = 0x4C04u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LOOP_GET_STATUS64: crate::io::IoctlReq = 0x4C05u32 as crate::io::IoctlReq;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LOOP_CTL_GET_FREE: crate::io::IoctlReq = 0x4C82u32 as crate::io::IoctlReq;

// Loop flags
#[cfg(any(target_os = "linux", target_os = "android"))]
const LO_FLAGS_READ_ONLY: u32 = 1;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LO_FLAGS_AUTOCLEAR: u32 = 4;
#[cfg(any(target_os = "linux", target_os = "android"))]
const LO_FLAGS_PARTSCAN: u32 = 8;

/// Loop device info structure
#[cfg(any(target_os = "linux", target_os = "android"))]
#[repr(C)]
struct LoopInfo64 {
    lo_device: u64,
    lo_inode: u64,
    lo_rdevice: u64,
    lo_offset: u64,
    lo_sizelimit: u64,
    lo_number: u32,
    lo_encrypt_type: u32,
    lo_encrypt_key_size: u32,
    lo_flags: u32,
    lo_file_name: [u8; 64],
    lo_crypt_name: [u8; 64],
    lo_encrypt_key: [u8; 32],
    lo_init: [u64; 2],
}

/// losetup - set up and control loop devices
///
/// # Synopsis
/// ```text
/// losetup [-a|-d|-f] [-o OFFSET] [-r] [LOOPDEV] [FILE]
/// ```
///
/// # Description
/// Associate loop devices with regular files, or show/detach loop devices.
///
/// # Options
/// - `-a`: Show all loop devices
/// - `-d LOOPDEV`: Detach loop device
/// - `-f`: Find and show next free loop device
/// - `-o OFFSET`: Start at offset into file
/// - `-r`: Read-only
/// - `-P`: Force kernel to scan for partitions
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn losetup(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_all = false;
    let mut detach: Option<&[u8]> = None;
    let mut find_free = false;
    let mut offset: u64 = 0;
    let mut read_only = false;
    let mut partscan = false;
    let mut loopdev: Option<&[u8]> = None;
    let mut file: Option<&[u8]> = None;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-a" || arg == b"--all" {
            show_all = true;
        } else if arg == b"-d" || arg == b"--detach" {
            i += 1;
            if i < argc as usize {
                detach = unsafe { get_arg(argv, i as i32) };
            }
        } else if arg == b"-f" || arg == b"--find" {
            find_free = true;
        } else if arg == b"-o" || arg == b"--offset" {
            i += 1;
            if i < argc as usize {
                if let Some(o) = unsafe { get_arg(argv, i as i32) } {
                    offset = sys::parse_u64(o).unwrap_or(0);
                }
            }
        } else if arg == b"-r" || arg == b"--read-only" {
            read_only = true;
        } else if arg == b"-P" || arg == b"--partscan" {
            partscan = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            if loopdev.is_none() {
                loopdev = Some(arg);
            } else if file.is_none() {
                file = Some(arg);
            }
        }
        i += 1;
    }

    // Handle detach
    if let Some(dev) = detach {
        return detach_loop(dev);
    }

    // Handle find free
    if find_free && file.is_none() && loopdev.is_none() {
        return find_free_loop();
    }

    // Handle show all
    if show_all || (loopdev.is_none() && file.is_none() && !find_free) {
        return show_loops();
    }

    // Handle setup: either losetup -f file or losetup loopdev file
    if let Some(f) = file {
        let dev = if find_free { None } else { loopdev };
        return setup_loop(dev, f, offset, read_only, partscan);
    } else if let Some(dev) = loopdev {
        // Just show info for this device
        return show_loop_info(dev);
    }

    print_usage();
    1
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn losetup(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"losetup: only available on Linux\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show_loops() -> i32 {
    // Iterate through /dev/loop* and show configured ones
    for n in 0..256 {
        let mut path = Vec::new();
        path.extend_from_slice(b"/dev/loop");
        let mut buf = [0u8; 16];
        path.extend_from_slice(sys::format_u64(n, &mut buf));
        path.push(0);

        let fd = io::open(&path, libc::O_RDONLY, 0);
        if fd >= 0 {
            let mut info: LoopInfo64 = unsafe { core::mem::zeroed() };
            let ret =
                unsafe { libc::ioctl(fd, LOOP_GET_STATUS64 as crate::io::IoctlReq, &mut info) };
            io::close(fd);

            if ret >= 0 {
                // Print loop info
                path.pop(); // remove null
                io::write_all(1, &path);
                io::write_str(1, b": [");

                // Device and inode
                io::write_all(1, sys::format_u64(info.lo_device, &mut buf));
                io::write_str(1, b"]:");
                io::write_all(1, sys::format_u64(info.lo_inode, &mut buf));
                io::write_str(1, b" (");

                // Filename
                let name_len = info.lo_file_name.iter().position(|&c| c == 0).unwrap_or(64);
                io::write_all(1, &info.lo_file_name[..name_len]);
                io::write_str(1, b")");

                // Offset if non-zero
                if info.lo_offset > 0 {
                    io::write_str(1, b", offset ");
                    io::write_all(1, sys::format_u64(info.lo_offset, &mut buf));
                }

                // Size limit if set
                if info.lo_sizelimit > 0 {
                    io::write_str(1, b", sizelimit ");
                    io::write_all(1, sys::format_u64(info.lo_sizelimit, &mut buf));
                }

                io::write_str(1, b"\n");
            }
        }
    }
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show_loop_info(device: &[u8]) -> i32 {
    let mut path = Vec::new();
    path.extend_from_slice(device);
    path.push(0);

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"losetup: ");
        io::write_all(2, device);
        io::write_str(2, b": cannot open\n");
        return 1;
    }

    let mut info: LoopInfo64 = unsafe { core::mem::zeroed() };
    let ret = unsafe { libc::ioctl(fd, LOOP_GET_STATUS64 as crate::io::IoctlReq, &mut info) };
    io::close(fd);

    if ret < 0 {
        io::write_str(2, b"losetup: ");
        io::write_all(2, device);
        io::write_str(2, b": not configured\n");
        return 1;
    }

    let mut buf = [0u8; 16];
    io::write_all(1, device);
    io::write_str(1, b": [");
    io::write_all(1, sys::format_u64(info.lo_device, &mut buf));
    io::write_str(1, b"]:");
    io::write_all(1, sys::format_u64(info.lo_inode, &mut buf));
    io::write_str(1, b" (");
    let name_len = info.lo_file_name.iter().position(|&c| c == 0).unwrap_or(64);
    io::write_all(1, &info.lo_file_name[..name_len]);
    io::write_str(1, b")\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn find_free_loop() -> i32 {
    let fd = io::open(b"/dev/loop-control", libc::O_RDWR, 0);
    if fd < 0 {
        io::write_str(2, b"losetup: cannot open /dev/loop-control\n");
        return 1;
    }

    let num = unsafe { libc::ioctl(fd, LOOP_CTL_GET_FREE as crate::io::IoctlReq) };
    io::close(fd);

    if num < 0 {
        io::write_str(2, b"losetup: cannot find free loop device\n");
        return 1;
    }

    io::write_str(1, b"/dev/loop");
    let mut buf = [0u8; 16];
    io::write_all(1, sys::format_u64(num as u64, &mut buf));
    io::write_str(1, b"\n");

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn setup_loop(
    device: Option<&[u8]>,
    file: &[u8],
    offset: u64,
    read_only: bool,
    partscan: bool,
) -> i32 {
    // Get device path
    let dev_path = if let Some(d) = device {
        let mut p = Vec::new();
        p.extend_from_slice(d);
        p.push(0);
        p
    } else {
        // Find free device
        let ctl_fd = io::open(b"/dev/loop-control", libc::O_RDWR, 0);
        if ctl_fd < 0 {
            io::write_str(2, b"losetup: cannot open /dev/loop-control\n");
            return 1;
        }

        let num = unsafe { libc::ioctl(ctl_fd, LOOP_CTL_GET_FREE as crate::io::IoctlReq) };
        io::close(ctl_fd);

        if num < 0 {
            io::write_str(2, b"losetup: cannot find free loop device\n");
            return 1;
        }

        let mut p = Vec::new();
        p.extend_from_slice(b"/dev/loop");
        let mut buf = [0u8; 16];
        p.extend_from_slice(sys::format_u64(num as u64, &mut buf));
        p.push(0);
        p
    };

    // Open the backing file
    let file_flags = if read_only {
        libc::O_RDONLY
    } else {
        libc::O_RDWR
    };
    let mut file_path = Vec::new();
    file_path.extend_from_slice(file);
    file_path.push(0);

    let file_fd = io::open(&file_path, file_flags, 0);
    if file_fd < 0 {
        io::write_str(2, b"losetup: cannot open ");
        io::write_all(2, file);
        io::write_str(2, b"\n");
        return 1;
    }

    // Open the loop device
    let loop_flags = if read_only {
        libc::O_RDONLY
    } else {
        libc::O_RDWR
    };
    let loop_fd = io::open(&dev_path, loop_flags, 0);
    if loop_fd < 0 {
        io::write_str(2, b"losetup: cannot open loop device\n");
        io::close(file_fd);
        return 1;
    }

    // Associate the loop device with the file
    let ret = unsafe { libc::ioctl(loop_fd, LOOP_SET_FD as crate::io::IoctlReq, file_fd) };
    if ret < 0 {
        io::write_str(2, b"losetup: failed to set up loop device\n");
        io::close(file_fd);
        io::close(loop_fd);
        return 1;
    }

    // Set up loop info if needed
    if offset > 0 || read_only || partscan {
        let mut info: LoopInfo64 = unsafe { core::mem::zeroed() };

        // Get current status first
        unsafe { libc::ioctl(loop_fd, LOOP_GET_STATUS64 as crate::io::IoctlReq, &mut info) };

        info.lo_offset = offset;
        if read_only {
            info.lo_flags |= LO_FLAGS_READ_ONLY;
        }
        if partscan {
            info.lo_flags |= LO_FLAGS_PARTSCAN;
        }

        // Copy filename
        let len = core::cmp::min(file.len(), 63);
        info.lo_file_name[..len].copy_from_slice(&file[..len]);

        let ret = unsafe { libc::ioctl(loop_fd, LOOP_SET_STATUS64 as crate::io::IoctlReq, &info) };
        if ret < 0 {
            io::write_str(2, b"losetup: warning: failed to set loop status\n");
        }
    }

    io::close(file_fd);
    io::close(loop_fd);

    // Print the device name if we auto-selected it
    if device.is_none() {
        dev_path[..dev_path.len() - 1].iter().for_each(|&c| {
            io::write_all(1, &[c]);
        });
        io::write_str(1, b"\n");
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn detach_loop(device: &[u8]) -> i32 {
    let mut path = Vec::new();
    path.extend_from_slice(device);
    path.push(0);

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"losetup: ");
        io::write_all(2, device);
        io::write_str(2, b": cannot open\n");
        return 1;
    }

    let ret = unsafe { libc::ioctl(fd, LOOP_CLR_FD as crate::io::IoctlReq) };
    io::close(fd);

    if ret < 0 {
        io::write_str(2, b"losetup: ");
        io::write_all(2, device);
        io::write_str(2, b": detach failed\n");
        return 1;
    }

    0
}

fn print_usage() {
    io::write_str(1, b"Usage: losetup [OPTIONS] [[LOOPDEV] FILE]\n\n");
    io::write_str(1, b"Set up and control loop devices.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -a, --all           Show all loop devices\n");
    io::write_str(1, b"  -d, --detach DEV    Detach loop device\n");
    io::write_str(1, b"  -f, --find          Find and show next free device\n");
    io::write_str(1, b"  -o, --offset N      Start at offset N into file\n");
    io::write_str(1, b"  -r, --read-only     Set up read-only loop\n");
    io::write_str(1, b"  -P, --partscan      Scan for partitions\n");
    io::write_str(1, b"  -h, --help          Show this help\n");
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
    fn test_losetup_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["losetup", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_losetup_show() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["losetup", "-a"])
            .output()
            .unwrap();

        // Should succeed even if no loops configured
        assert_eq!(output.status.code(), Some(0));
    }
}
