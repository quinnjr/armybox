//! blockdev - call block device ioctls from the command line
//!
//! Get and set block device parameters.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

// Block device ioctls
const BLKGETSIZE64: crate::io::IoctlReq = 0x80081272u32 as crate::io::IoctlReq; // Get size in bytes
const BLKGETSIZE: crate::io::IoctlReq = 0x1260u32 as crate::io::IoctlReq; // Get size in 512-byte sectors
const BLKFLSBUF: crate::io::IoctlReq = 0x1261u32 as crate::io::IoctlReq; // Flush buffers
const BLKRRPART: crate::io::IoctlReq = 0x125fu32 as crate::io::IoctlReq; // Reread partition table
const BLKROSET: crate::io::IoctlReq = 0x125du32 as crate::io::IoctlReq; // Set read-only
const BLKROGET: crate::io::IoctlReq = 0x125eu32 as crate::io::IoctlReq; // Get read-only
const BLKSSZGET: crate::io::IoctlReq = 0x1268u32 as crate::io::IoctlReq; // Get logical sector size
const BLKBSZGET: crate::io::IoctlReq = 0x80081270u32 as crate::io::IoctlReq; // Get block size
const BLKBSZSET: crate::io::IoctlReq = 0x40081271u32 as crate::io::IoctlReq; // Set block size
const BLKPBSZGET: crate::io::IoctlReq = 0x127bu32 as crate::io::IoctlReq; // Get physical sector size
const BLKIOMIN: crate::io::IoctlReq = 0x1278u32 as crate::io::IoctlReq; // Get minimum I/O size
const BLKIOOPT: crate::io::IoctlReq = 0x1279u32 as crate::io::IoctlReq; // Get optimal I/O size
const BLKALIGNOFF: crate::io::IoctlReq = 0x127au32 as crate::io::IoctlReq; // Get alignment offset
const BLKDISCARDZEROES: crate::io::IoctlReq = 0x127cu32 as crate::io::IoctlReq; // Get discard zeroes data

/// blockdev - call block device ioctls from the command line
///
/// # Synopsis
/// ```text
/// blockdev [options] device
/// ```
///
/// # Description
/// Call block device ioctls from the command line.
///
/// # Options
/// - `--getsize64`: Print device size in bytes
/// - `--getsize`: Print device size in 512-byte sectors
/// - `--getsz`: Print device size in 512-byte sectors
/// - `--getss`: Print logical sector size
/// - `--getbsz`: Print block size
/// - `--setbsz N`: Set block size
/// - `--getpbsz`: Print physical sector size
/// - `--getiomin`: Print minimum I/O size
/// - `--getioopt`: Print optimal I/O size
/// - `--getalignoff`: Print alignment offset
/// - `--getro`: Print read-only status (1=read-only, 0=read-write)
/// - `--setro`: Set read-only
/// - `--setrw`: Set read-write
/// - `--flushbufs`: Flush buffers
/// - `--rereadpt`: Reread partition table
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn blockdev(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        print_usage();
        return 1;
    }

    let mut i = 1;
    let mut errors = 0;

    while i < argc {
        let option = match unsafe { get_arg(argv, i) } {
            Some(o) => o,
            None => break,
        };

        // Check if this is a device (doesn't start with --)
        if !option.starts_with(b"--") {
            // It's a device, we need an option before it
            io::write_str(2, b"blockdev: missing option before device\n");
            return 1;
        }

        // Find the device (next non-option argument, or the argument after a value option)
        let (device_idx, value) = if needs_value(option) {
            i += 1;
            let val = unsafe { get_arg(argv, i) };
            i += 1;
            (i, val)
        } else {
            i += 1;
            (i, None)
        };

        let device = match unsafe { get_arg(argv, device_idx) } {
            Some(d) => d,
            None => {
                io::write_str(2, b"blockdev: missing device\n");
                return 1;
            }
        };
        i = device_idx + 1;

        // Open device (read-write for set operations, read-only for get operations)
        let flags = if option.starts_with(b"--set")
            || option == b"--flushbufs"
            || option == b"--rereadpt"
        {
            libc::O_RDWR
        } else {
            libc::O_RDONLY
        };

        let fd = io::open(device, flags, 0);
        if fd < 0 {
            sys::perror(device);
            errors += 1;
            continue;
        }

        let result = execute_option(fd, option, value);
        io::close(fd);

        if result != 0 {
            errors += 1;
        }
    }

    if errors > 0 { 1 } else { 0 }
}

fn needs_value(option: &[u8]) -> bool {
    option == b"--setbsz"
}

fn execute_option(fd: i32, option: &[u8], value: Option<&[u8]>) -> i32 {
    if option == b"--getsize64" {
        let mut size: u64 = 0;
        if unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size) } < 0 {
            sys::perror(b"BLKGETSIZE64");
            return 1;
        }
        io::write_num(1, size);
        io::write_str(1, b"\n");
    } else if option == b"--getsize" || option == b"--getsz" {
        let mut size: u64 = 0;
        if unsafe { libc::ioctl(fd, BLKGETSIZE, &mut size) } < 0 {
            // Fall back to BLKGETSIZE64
            if unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size) } < 0 {
                sys::perror(b"BLKGETSIZE");
                return 1;
            }
            size /= 512;
        }
        io::write_num(1, size);
        io::write_str(1, b"\n");
    } else if option == b"--getss" {
        let mut ss: i32 = 0;
        if unsafe { libc::ioctl(fd, BLKSSZGET, &mut ss) } < 0 {
            sys::perror(b"BLKSSZGET");
            return 1;
        }
        io::write_num(1, ss as u64);
        io::write_str(1, b"\n");
    } else if option == b"--getbsz" {
        let mut bsz: i32 = 0;
        if unsafe { libc::ioctl(fd, BLKBSZGET, &mut bsz) } < 0 {
            sys::perror(b"BLKBSZGET");
            return 1;
        }
        io::write_num(1, bsz as u64);
        io::write_str(1, b"\n");
    } else if option == b"--setbsz" {
        let bsz = match value.and_then(|v| sys::parse_u64(v)) {
            Some(v) => v as i32,
            None => {
                io::write_str(2, b"blockdev: --setbsz requires a value\n");
                return 1;
            }
        };
        if unsafe { libc::ioctl(fd, BLKBSZSET, &bsz) } < 0 {
            sys::perror(b"BLKBSZSET");
            return 1;
        }
    } else if option == b"--getpbsz" {
        let mut pbsz: u32 = 0;
        if unsafe { libc::ioctl(fd, BLKPBSZGET, &mut pbsz) } < 0 {
            sys::perror(b"BLKPBSZGET");
            return 1;
        }
        io::write_num(1, pbsz as u64);
        io::write_str(1, b"\n");
    } else if option == b"--getiomin" {
        let mut iomin: u32 = 0;
        if unsafe { libc::ioctl(fd, BLKIOMIN, &mut iomin) } < 0 {
            sys::perror(b"BLKIOMIN");
            return 1;
        }
        io::write_num(1, iomin as u64);
        io::write_str(1, b"\n");
    } else if option == b"--getioopt" {
        let mut ioopt: u32 = 0;
        if unsafe { libc::ioctl(fd, BLKIOOPT, &mut ioopt) } < 0 {
            sys::perror(b"BLKIOOPT");
            return 1;
        }
        io::write_num(1, ioopt as u64);
        io::write_str(1, b"\n");
    } else if option == b"--getalignoff" {
        let mut alignoff: u32 = 0;
        if unsafe { libc::ioctl(fd, BLKALIGNOFF, &mut alignoff) } < 0 {
            sys::perror(b"BLKALIGNOFF");
            return 1;
        }
        io::write_num(1, alignoff as u64);
        io::write_str(1, b"\n");
    } else if option == b"--getro" {
        let mut ro: i32 = 0;
        if unsafe { libc::ioctl(fd, BLKROGET, &mut ro) } < 0 {
            sys::perror(b"BLKROGET");
            return 1;
        }
        io::write_num(1, ro as u64);
        io::write_str(1, b"\n");
    } else if option == b"--setro" {
        let one: i32 = 1;
        if unsafe { libc::ioctl(fd, BLKROSET, &one) } < 0 {
            sys::perror(b"BLKROSET");
            return 1;
        }
    } else if option == b"--setrw" {
        let zero: i32 = 0;
        if unsafe { libc::ioctl(fd, BLKROSET, &zero) } < 0 {
            sys::perror(b"BLKROSET");
            return 1;
        }
    } else if option == b"--flushbufs" {
        if unsafe { libc::ioctl(fd, BLKFLSBUF) } < 0 {
            sys::perror(b"BLKFLSBUF");
            return 1;
        }
    } else if option == b"--rereadpt" {
        if unsafe { libc::ioctl(fd, BLKRRPART) } < 0 {
            sys::perror(b"BLKRRPART");
            return 1;
        }
    } else {
        io::write_str(2, b"blockdev: unknown option: ");
        io::write_all(2, option);
        io::write_str(2, b"\n");
        return 1;
    }

    0
}

fn print_usage() {
    io::write_str(1, b"Usage: blockdev [options] device\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  --getsize64    Print device size in bytes\n");
    io::write_str(
        1,
        b"  --getsz        Print device size in 512-byte sectors\n",
    );
    io::write_str(1, b"  --getss        Print logical sector size\n");
    io::write_str(1, b"  --getbsz       Print block size\n");
    io::write_str(1, b"  --setbsz N     Set block size\n");
    io::write_str(1, b"  --getpbsz      Print physical sector size\n");
    io::write_str(1, b"  --getiomin     Print minimum I/O size\n");
    io::write_str(1, b"  --getioopt     Print optimal I/O size\n");
    io::write_str(1, b"  --getalignoff  Print alignment offset\n");
    io::write_str(1, b"  --getro        Print read-only status\n");
    io::write_str(1, b"  --setro        Set read-only\n");
    io::write_str(1, b"  --setrw        Set read-write\n");
    io::write_str(1, b"  --flushbufs    Flush buffers\n");
    io::write_str(1, b"  --rereadpt     Reread partition table\n");
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
    fn test_blockdev_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["blockdev"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_blockdev_unknown_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["blockdev", "--unknown", "/dev/null"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
