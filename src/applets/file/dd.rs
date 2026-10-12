//! dd - convert and copy a file
//!
//! GNU coreutils compatible implementation.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// Reject block sizes above this ceiling (1 GiB) so a value like `bs=1T`
/// cannot trigger a multi-terabyte up-front allocation.
const MAX_BLOCK_SIZE: u64 = 1 << 30;

/// dd - convert and copy a file
///
/// # Synopsis
/// ```text
/// dd [operand]...
/// ```
///
/// # Description
/// Copy a file, converting and formatting according to the operands.
///
/// # Operands
/// - `if=FILE`: Read from FILE instead of stdin
/// - `of=FILE`: Write to FILE instead of stdout
/// - `bs=BYTES`: Read and write up to BYTES bytes at a time (sets both ibs and obs)
/// - `ibs=BYTES`: Read up to BYTES bytes at a time
/// - `obs=BYTES`: Write up to BYTES bytes at a time
/// - `count=N`: Copy only N input blocks
/// - `skip=N`: Skip N input blocks before copying
/// - `seek=N`: Skip N output blocks before copying
/// - `conv=CONVS`: Comma separated list of conversions: notrunc, noerror, sync, lcase, ucase
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn dd(argc: i32, argv: *const *const u8) -> i32 {
    let mut if_path: Option<&[u8]> = None;
    let mut of_path: Option<&[u8]> = None;
    let mut bs: Option<usize> = None;
    let mut ibs: usize = 512;
    let mut obs: usize = 512;
    let mut count: Option<u64> = None;
    let mut skip: u64 = 0;
    let mut seek: u64 = 0;

    let mut conv_notrunc = false;
    let mut conv_noerror = false;
    let mut conv_sync = false;
    let mut conv_lcase = false;
    let mut conv_ucase = false;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"if=") {
                if_path = Some(&arg[3..]);
            } else if arg.starts_with(b"of=") {
                of_path = Some(&arg[3..]);
            } else if arg.starts_with(b"bs=") {
                match sys::parse_size(&arg[3..]) {
                    Some(v) if v <= MAX_BLOCK_SIZE => bs = Some(v as usize),
                    _ => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"ibs=") {
                match sys::parse_size(&arg[4..]) {
                    Some(v) if v <= MAX_BLOCK_SIZE => ibs = v as usize,
                    _ => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"obs=") {
                match sys::parse_size(&arg[4..]) {
                    Some(v) if v <= MAX_BLOCK_SIZE => obs = v as usize,
                    _ => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"count=") {
                match sys::parse_size(&arg[6..]) {
                    Some(v) => count = Some(v),
                    None => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"skip=") {
                match sys::parse_size(&arg[5..]) {
                    Some(v) => skip = v,
                    None => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"seek=") {
                match sys::parse_size(&arg[5..]) {
                    Some(v) => seek = v,
                    None => return dd_bad_operand(arg),
                }
            } else if arg.starts_with(b"conv=") {
                for part in arg[5..].split(|&b| b == b',') {
                    match part {
                        b"notrunc" => conv_notrunc = true,
                        b"noerror" => conv_noerror = true,
                        b"sync" => conv_sync = true,
                        b"lcase" => conv_lcase = true,
                        b"ucase" => conv_ucase = true,
                        b"" => {}
                        _ => return dd_bad_operand(arg),
                    }
                }
            } else {
                return dd_bad_operand(arg);
            }
        }
    }

    if let Some(b) = bs {
        ibs = b;
        obs = b;
    }
    if ibs == 0 {
        ibs = 512;
    }
    if obs == 0 {
        obs = 512;
    }

    let in_fd = match if_path {
        Some(p) => {
            let fd = io::open(p, libc::O_RDONLY, 0);
            if fd < 0 {
                sys::perror(p);
                return 1;
            }
            fd
        }
        None => 0,
    };

    let mut out_flags = libc::O_WRONLY | libc::O_CREAT;
    if !conv_notrunc {
        out_flags |= libc::O_TRUNC;
    }

    let out_fd = match of_path {
        Some(p) => {
            let fd = io::open(p, out_flags, 0o644);
            if fd < 0 {
                sys::perror(p);
                if in_fd != 0 {
                    io::close(in_fd);
                }
                return 1;
            }
            fd
        }
        None => 1,
    };

    // Skip input blocks: try lseek first, fall back to reading and discarding.
    if skip > 0 {
        let byte_offset = skip.saturating_mul(ibs as u64) as i64;
        let pos = io::lseek(in_fd, byte_offset, libc::SEEK_CUR);
        if pos < 0 {
            let mut discard = [0u8; 4096];
            let mut remaining = byte_offset as u64;
            while remaining > 0 {
                let want = if remaining < discard.len() as u64 {
                    remaining as usize
                } else {
                    discard.len()
                };
                let n = io::read(in_fd, &mut discard[..want]);
                if n <= 0 {
                    break;
                }
                remaining -= n as u64;
            }
        }
    }

    // Seek output blocks (create a hole / advance past existing content).
    if seek > 0 {
        let byte_offset = seek.saturating_mul(obs as u64) as i64;
        io::lseek(out_fd, byte_offset, libc::SEEK_CUR);
    }

    let mut in_full: u64 = 0;
    let mut in_partial: u64 = 0;
    let mut out_full: u64 = 0;
    let mut out_partial: u64 = 0;
    let mut had_error = false;

    #[cfg(feature = "alloc")]
    {
        use alloc::vec;
        let mut buf = vec![0u8; ibs];

        'copy: loop {
            if let Some(c) = count {
                if in_full + in_partial >= c {
                    break;
                }
            }

            let n = io::read(in_fd, &mut buf);

            if n < 0 {
                // Read error.
                had_error = true;
                if !conv_noerror {
                    break;
                }
                if conv_sync {
                    // Replace the unreadable block with a zero-filled block
                    // of ibs size and write it out, then keep going.
                    for b in buf.iter_mut() {
                        *b = 0;
                    }
                    let data_len = ibs;
                    let w = io::write_all(out_fd, &buf[..data_len]);
                    if w < 0 || (w as usize) < data_len {
                        had_error = true;
                        break;
                    }
                    in_partial += 1;
                    if data_len == obs {
                        out_full += 1;
                    } else {
                        out_partial += 1;
                    }
                }
                // Advance past the unreadable block before retrying (GNU
                // dd behavior). Without this the read offset never moves
                // and a persistent error spins forever.
                io::lseek(in_fd, ibs as i64, libc::SEEK_CUR);
                continue 'copy;
            }

            if n == 0 {
                break;
            }

            let read_len = n as usize;
            if read_len == ibs {
                in_full += 1;
            } else {
                in_partial += 1;
            }

            let mut data_len = read_len;
            if conv_sync && read_len < ibs {
                for b in buf[read_len..ibs].iter_mut() {
                    *b = 0;
                }
                data_len = ibs;
            }

            if conv_lcase {
                for b in buf[..data_len].iter_mut() {
                    if b.is_ascii_uppercase() {
                        *b += 32;
                    }
                }
            } else if conv_ucase {
                for b in buf[..data_len].iter_mut() {
                    if b.is_ascii_lowercase() {
                        *b -= 32;
                    }
                }
            }

            let w = io::write_all(out_fd, &buf[..data_len]);
            if w < 0 || (w as usize) < data_len {
                had_error = true;
                break;
            }

            if data_len == obs {
                out_full += 1;
            } else {
                out_partial += 1;
            }
        }
    }

    #[cfg(not(feature = "alloc"))]
    {
        let mut buf = [0u8; 512];
        let ibs = 512usize;
        let obs = 512usize;

        loop {
            if let Some(c) = count {
                if in_full + in_partial >= c {
                    break;
                }
            }

            let n = io::read(in_fd, &mut buf);
            if n < 0 {
                had_error = true;
                break;
            }
            if n == 0 {
                break;
            }
            let read_len = n as usize;
            if read_len == ibs {
                in_full += 1;
            } else {
                in_partial += 1;
            }

            let w = io::write_all(out_fd, &buf[..read_len]);
            if w < 0 || (w as usize) < read_len {
                had_error = true;
                break;
            }
            if read_len == obs {
                out_full += 1;
            } else {
                out_partial += 1;
            }
        }
    }

    io::write_num(2, in_full);
    io::write_str(2, b"+");
    io::write_num(2, in_partial);
    io::write_str(2, b" records in\n");
    io::write_num(2, out_full);
    io::write_str(2, b"+");
    io::write_num(2, out_partial);
    io::write_str(2, b" records out\n");

    if in_fd != 0 {
        io::close(in_fd);
    }
    if out_fd != 1 {
        io::close(out_fd);
    }

    if had_error { 1 } else { 0 }
}

fn dd_bad_operand(arg: &[u8]) -> i32 {
    io::write_str(2, b"dd: unrecognized operand '");
    io::write_all(2, arg);
    io::write_str(2, b"'\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    fn setup() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "armybox_dd_test_{}_{}",
            std::process::id(),
            counter
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &std::path::Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_dd_copy_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let input = dir.join("input.txt");
        let output = dir.join("output.txt");
        fs::write(&input, "hello world").unwrap();

        let cmd_output = Command::new(&armybox)
            .args([
                "dd",
                &format!("if={}", input.to_str().unwrap()),
                &format!("of={}", output.to_str().unwrap()),
            ])
            .output()
            .unwrap();

        assert_eq!(cmd_output.status.code(), Some(0));
        assert!(output.exists());
        assert_eq!(fs::read_to_string(&output).unwrap(), "hello world");
        cleanup(&dir);
    }

    #[test]
    fn test_dd_with_count() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let input = dir.join("input.txt");
        let output = dir.join("output.txt");
        // Write more than 512 bytes
        fs::write(&input, "x".repeat(1024)).unwrap();

        let cmd_output = Command::new(&armybox)
            .args([
                "dd",
                &format!("if={}", input.to_str().unwrap()),
                &format!("of={}", output.to_str().unwrap()),
                "count=1",
            ])
            .output()
            .unwrap();

        assert_eq!(cmd_output.status.code(), Some(0));
        assert!(output.exists());
        // With default bs=512 and count=1, should copy at most 512 bytes
        assert!(fs::read(&output).unwrap().len() <= 512);
        cleanup(&dir);
    }

    #[test]
    fn test_dd_with_bs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let input = dir.join("input.txt");
        let output = dir.join("output.txt");
        fs::write(&input, "hello").unwrap();

        let cmd_output = Command::new(&armybox)
            .args([
                "dd",
                &format!("if={}", input.to_str().unwrap()),
                &format!("of={}", output.to_str().unwrap()),
                "bs=1024",
            ])
            .output()
            .unwrap();

        assert_eq!(cmd_output.status.code(), Some(0));
        assert_eq!(fs::read_to_string(&output).unwrap(), "hello");
        cleanup(&dir);
    }

    #[test]
    fn test_dd_nonexistent_input() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let output = dir.join("output.txt");

        let cmd_output = Command::new(&armybox)
            .args([
                "dd",
                "if=/nonexistent/file",
                &format!("of={}", output.to_str().unwrap()),
            ])
            .output()
            .unwrap();

        assert_ne!(cmd_output.status.code(), Some(0));
        cleanup(&dir);
    }
}
