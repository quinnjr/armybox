//! iotop - display I/O usage by processes
//!
//! Monitor I/O usage in real-time.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// Process I/O information
struct ProcessIO {
    pid: u32,
    read_bytes: u64,
    write_bytes: u64,
    read_bytes_prev: u64,
    write_bytes_prev: u64,
    comm: [u8; 32],
}

/// iotop - display I/O usage by processes
///
/// # Synopsis
/// ```text
/// iotop [-o] [-b] [-n COUNT] [-d DELAY] [-p PID]
/// ```
///
/// # Description
/// Display I/O usage by processes, similar to top but for I/O.
///
/// # Options
/// - `-o, --only`: Only show processes doing I/O
/// - `-b, --batch`: Batch mode (non-interactive)
/// - `-n COUNT`: Number of iterations (implies -b)
/// - `-d DELAY`: Delay between updates in seconds (default: 1)
/// - `-p PID`: Monitor only this PID
/// - `-q, --quiet`: Suppress header
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
pub fn iotop(argc: i32, argv: *const *const u8) -> i32 {
    let mut only_active = false;
    let mut batch_mode = false;
    let mut iterations: Option<u32> = None;
    let mut delay_secs: u32 = 1;
    let mut specific_pid: Option<u32> = None;
    let mut quiet = false;

    // Parse options
    let mut i = 1;
    while i < argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-o" || arg == b"--only" {
                only_active = true;
            } else if arg == b"-b" || arg == b"--batch" {
                batch_mode = true;
            } else if arg == b"-n" {
                i += 1;
                if let Some(n) = unsafe { get_arg(argv, i) } {
                    iterations = sys::parse_u64(n).map(|v| v as u32);
                    batch_mode = true;
                }
            } else if arg == b"-d" {
                i += 1;
                if let Some(d) = unsafe { get_arg(argv, i) } {
                    delay_secs = sys::parse_u64(d).unwrap_or(1) as u32;
                }
            } else if arg == b"-p" {
                i += 1;
                if let Some(p) = unsafe { get_arg(argv, i) } {
                    specific_pid = sys::parse_u64(p).map(|v| v as u32);
                }
            } else if arg == b"-q" || arg == b"--quiet" {
                quiet = true;
            } else if arg == b"-h" || arg == b"--help" {
                print_help();
                return 0;
            }
        }
        i += 1;
    }

    // Check if we can access /proc
    let proc_dir = io::opendir(b"/proc");
    if proc_dir.is_null() {
        io::write_str(2, b"iotop: cannot access /proc\n");
        return 1;
    }
    io::closedir(proc_dir);

    // Collect initial I/O stats
    let mut processes = collect_io_stats(specific_pid);

    // If batch mode with iterations, run that many times
    let mut iter_count = 0u32;
    loop {
        // Sleep
        if iter_count > 0 {
            unsafe {
                libc::sleep(delay_secs);
            }
        }

        // Collect new stats
        let new_processes = collect_io_stats(specific_pid);

        // Calculate rates
        let mut entries: Vec<IOEntry> = Vec::new();
        for new in &new_processes {
            // Find previous entry
            let prev = processes.iter().find(|p| p.pid == new.pid);
            let (read_rate, write_rate) = if let Some(p) = prev {
                let read_diff = new.read_bytes.saturating_sub(p.read_bytes);
                let write_diff = new.write_bytes.saturating_sub(p.write_bytes);
                (
                    read_diff / delay_secs as u64,
                    write_diff / delay_secs as u64,
                )
            } else {
                (0, 0)
            };

            if only_active && read_rate == 0 && write_rate == 0 {
                continue;
            }

            entries.push(IOEntry {
                pid: new.pid,
                read_rate,
                write_rate,
                read_total: new.read_bytes,
                write_total: new.write_bytes,
                comm: new.comm,
            });
        }

        // Sort by total I/O rate (descending)
        entries.sort_by(|a, b| {
            let a_total = a.read_rate + a.write_rate;
            let b_total = b.read_rate + b.write_rate;
            b_total.cmp(&a_total)
        });

        // Calculate totals
        let total_read: u64 = entries.iter().map(|e| e.read_rate).sum();
        let total_write: u64 = entries.iter().map(|e| e.write_rate).sum();

        // Print header
        if !quiet {
            io::write_str(1, b"Total DISK READ: ");
            print_rate(total_read);
            io::write_str(1, b" | Total DISK WRITE: ");
            print_rate(total_write);
            io::write_str(1, b"\n");

            io::write_str(
                1,
                b"    PID  PRIO  USER     DISK READ  DISK WRITE  COMMAND\n",
            );
        }

        // Print entries (limit to 20 in non-batch mode)
        let limit = if batch_mode {
            entries.len()
        } else {
            20.min(entries.len())
        };
        for entry in entries.iter().take(limit) {
            // PID
            let mut buf = [0u8; 16];
            let pid_str = sys::format_u64(entry.pid as u64, &mut buf);
            for _ in pid_str.len()..7 {
                io::write_str(1, b" ");
            }
            io::write_all(1, pid_str);
            io::write_str(1, b"  ");

            // Priority (placeholder)
            io::write_str(1, b"be/4  ");

            // User (get from /proc/pid/status)
            let uid = get_process_uid(entry.pid);
            let user = get_username(uid);
            io::write_all(1, &user);
            for _ in user.len()..9 {
                io::write_str(1, b" ");
            }

            // Read rate
            print_rate_padded(entry.read_rate, 11);
            io::write_str(1, b" ");

            // Write rate
            print_rate_padded(entry.write_rate, 12);
            io::write_str(1, b" ");

            // Command
            let comm_len = entry
                .comm
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.comm.len());
            io::write_all(1, &entry.comm[..comm_len]);
            io::write_str(1, b"\n");
        }

        // Store for next iteration
        processes = new_processes;

        iter_count += 1;
        if let Some(max) = iterations {
            if iter_count >= max {
                break;
            }
        } else if batch_mode {
            // In batch mode without -n, run once
            break;
        } else {
            // Interactive mode - just run once for now
            // (full interactive mode would need ncurses)
            break;
        }
    }

    0
}

struct IOEntry {
    pid: u32,
    read_rate: u64,
    write_rate: u64,
    read_total: u64,
    write_total: u64,
    comm: [u8; 32],
}

/// Collect I/O statistics for all processes
fn collect_io_stats(specific_pid: Option<u32>) -> Vec<ProcessIO> {
    let mut processes = Vec::new();

    if let Some(pid) = specific_pid {
        if let Some(proc) = read_process_io(pid) {
            processes.push(proc);
        }
        return processes;
    }

    let proc_dir = io::opendir(b"/proc");
    if proc_dir.is_null() {
        return processes;
    }

    loop {
        let entry = io::readdir(proc_dir);
        if entry.is_null() {
            break;
        }

        let d_name = unsafe { &(*entry).d_name };
        let name_len = d_name.iter().position(|&c| c == 0).unwrap_or(d_name.len());
        let name: &[u8] = unsafe { core::mem::transmute(&d_name[..name_len]) };

        // Check if numeric (PID)
        if name.iter().all(|&c| c >= b'0' && c <= b'9') {
            if let Some(pid) = sys::parse_u64(name) {
                if let Some(proc) = read_process_io(pid as u32) {
                    processes.push(proc);
                }
            }
        }
    }

    io::closedir(proc_dir);
    processes
}

/// Read I/O stats for a single process
fn read_process_io(pid: u32) -> Option<ProcessIO> {
    // Read /proc/pid/io
    let mut path = Vec::new();
    path.extend_from_slice(b"/proc/");
    let mut num_buf = [0u8; 16];
    let pid_str = sys::format_u64(pid as u64, &mut num_buf);
    path.extend_from_slice(pid_str);
    path.extend_from_slice(b"/io");

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }

    let mut buf = [0u8; 512];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return None;
    }

    let content = &buf[..n as usize];
    let mut read_bytes: u64 = 0;
    let mut write_bytes: u64 = 0;

    for line in content.split(|&b| b == b'\n') {
        if line.starts_with(b"read_bytes:") {
            if let Some(val) = parse_io_value(line) {
                read_bytes = val;
            }
        } else if line.starts_with(b"write_bytes:") {
            if let Some(val) = parse_io_value(line) {
                write_bytes = val;
            }
        }
    }

    // Read command name
    path.truncate(path.len() - 3); // Remove "/io"
    path.extend_from_slice(b"/comm");

    let mut comm = [0u8; 32];
    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd >= 0 {
        let n = io::read(fd, &mut comm);
        io::close(fd);
        // Remove trailing newline
        if n > 0 && comm[n as usize - 1] == b'\n' {
            comm[n as usize - 1] = 0;
        }
    }

    Some(ProcessIO {
        pid,
        read_bytes,
        write_bytes,
        read_bytes_prev: 0,
        write_bytes_prev: 0,
        comm,
    })
}

fn parse_io_value(line: &[u8]) -> Option<u64> {
    let mut i = 0;
    // Skip to colon
    while i < line.len() && line[i] != b':' {
        i += 1;
    }
    i += 1; // Skip colon

    // Skip whitespace
    while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
        i += 1;
    }

    // Parse number
    sys::parse_u64(&line[i..])
}

/// Get UID for a process
fn get_process_uid(pid: u32) -> u32 {
    let mut path = Vec::new();
    path.extend_from_slice(b"/proc/");
    let mut num_buf = [0u8; 16];
    let pid_str = sys::format_u64(pid as u64, &mut num_buf);
    path.extend_from_slice(pid_str);
    path.extend_from_slice(b"/status");

    let fd = io::open(&path, libc::O_RDONLY, 0);
    if fd < 0 {
        return 0;
    }

    let mut buf = [0u8; 2048];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return 0;
    }

    let content = &buf[..n as usize];
    for line in content.split(|&b| b == b'\n') {
        if line.starts_with(b"Uid:") {
            // Format: Uid:\treal\teffective\tsaved\tfsuid
            let mut i = 4;
            while i < line.len() && (line[i] == b' ' || line[i] == b'\t') {
                i += 1;
            }
            return sys::parse_u64(&line[i..]).unwrap_or(0) as u32;
        }
    }

    0
}

/// Get username from UID (simplified - just returns UID as string if not found)
fn get_username(uid: u32) -> Vec<u8> {
    // Try to read /etc/passwd
    let fd = io::open(b"/etc/passwd", libc::O_RDONLY, 0);
    if fd < 0 {
        let mut buf = [0u8; 16];
        let s = sys::format_u64(uid as u64, &mut buf);
        return s.to_vec();
    }

    let content = io::read_all(fd);
    io::close(fd);

    let uid_str = {
        let mut buf = [0u8; 16];
        sys::format_u64(uid as u64, &mut buf).to_vec()
    };

    for line in content.split(|&b| b == b'\n') {
        // Format: name:x:uid:gid:...
        let fields: Vec<&[u8]> = line.split(|&b| b == b':').collect();
        if fields.len() >= 3 && fields[2] == uid_str.as_slice() {
            return fields[0].to_vec();
        }
    }

    uid_str
}

/// Print I/O rate in human-readable format
fn print_rate(rate: u64) {
    let mut buf = [0u8; 16];

    if rate >= 1024 * 1024 * 1024 {
        let gb = rate / (1024 * 1024 * 1024);
        let frac = (rate % (1024 * 1024 * 1024)) / (1024 * 1024 * 10);
        let s = sys::format_u64(gb, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b".");
        let s = sys::format_u64(frac, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b" G/s");
    } else if rate >= 1024 * 1024 {
        let mb = rate / (1024 * 1024);
        let frac = (rate % (1024 * 1024)) / (1024 * 10);
        let s = sys::format_u64(mb, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b".");
        let s = sys::format_u64(frac, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b" M/s");
    } else if rate >= 1024 {
        let kb = rate / 1024;
        let frac = (rate % 1024) / 10;
        let s = sys::format_u64(kb, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b".");
        let s = sys::format_u64(frac, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b" K/s");
    } else {
        let s = sys::format_u64(rate, &mut buf);
        io::write_all(1, s);
        io::write_str(1, b" B/s");
    }
}

/// Print rate with padding
fn print_rate_padded(rate: u64, width: usize) {
    // Calculate what we'll print
    let len = if rate >= 1024 * 1024 * 1024 {
        // X.XX G/s = ~8-10 chars
        9
    } else if rate >= 1024 * 1024 {
        // X.XX M/s = ~8-10 chars
        9
    } else if rate >= 1024 {
        // X.XX K/s = ~8-10 chars
        9
    } else {
        // X B/s = varies
        let mut buf = [0u8; 16];
        let s = sys::format_u64(rate, &mut buf);
        s.len() + 4 // " B/s"
    };

    print_rate(rate);
    for _ in len..width {
        io::write_str(1, b" ");
    }
}

fn print_help() {
    io::write_str(1, b"Usage: iotop [OPTIONS]\n\n");
    io::write_str(1, b"Display I/O usage by processes.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -o, --only     Only show processes doing I/O\n");
    io::write_str(1, b"  -b, --batch    Batch mode (non-interactive)\n");
    io::write_str(1, b"  -n COUNT       Number of iterations\n");
    io::write_str(1, b"  -d DELAY       Delay between updates (default: 1s)\n");
    io::write_str(1, b"  -p PID         Monitor only this PID\n");
    io::write_str(1, b"  -q, --quiet    Suppress header\n");
    io::write_str(1, b"  -h, --help     Show this help\n\n");
    io::write_str(
        1,
        b"Note: Reading other processes' I/O stats requires root.\n",
    );
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
    fn test_iotop_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["iotop", "-b", "-n1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("DISK READ") || stdout.contains("PID"));
    }

    #[test]
    fn test_iotop_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["iotop", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_iotop_only_active() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["iotop", "-o", "-b", "-n1"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
