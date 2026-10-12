//! vmstat - report virtual memory statistics
//!
//! Report information about processes, memory, paging, etc.

use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// vmstat - report virtual memory statistics
///
/// # Synopsis
/// ```text
/// vmstat [delay [count]]
/// vmstat [-n] [delay [count]]
/// ```
///
/// # Description
/// Report information about processes, memory, paging, block IO,
/// traps, and CPU activity.
///
/// # Options
/// - `-n`: Display header only once
///
/// # Exit Status
/// - 0: Success
pub fn vmstat(argc: i32, argv: *const *const u8) -> i32 {
    let mut delay: u32 = 0;
    let mut count: Option<u32> = None;
    let mut show_header_once = false;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { super::get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-n" {
            show_header_once = true;
        } else if delay == 0 {
            delay = sys::parse_u64(arg).unwrap_or(0) as u32;
        } else if count.is_none() {
            count = Some(sys::parse_u64(arg).unwrap_or(1) as u32);
        }
        i += 1;
    }

    // Print header
    print_header();

    let mut iteration = 0u32;
    loop {
        // Read memory stats
        let mem = read_meminfo();
        let cpu = read_stat();
        let proc_stats = read_proc_stats();

        print_stats(&mem, &cpu, &proc_stats);

        iteration += 1;

        // Check if we should continue
        if delay == 0 {
            break;
        }

        if let Some(max) = count {
            if iteration >= max {
                break;
            }
        }

        // Sleep
        unsafe { libc::sleep(delay) };

        // Print header again if not -n
        if !show_header_once && iteration % 25 == 0 {
            print_header();
        }
    }

    0
}

fn print_header() {
    io::write_str(
        1,
        b"procs -----------memory---------- ---swap-- -----io---- -system-- ------cpu-----\n",
    );
    io::write_str(
        1,
        b" r  b   swpd   free   buff  cache   si   so    bi    bo   in   cs us sy id wa st\n",
    );
}

/// Memory statistics from /proc/meminfo
struct MemInfo {
    free: u64,
    buffers: u64,
    cached: u64,
    swap_total: u64,
    swap_free: u64,
    swap_in: u64,
    swap_out: u64,
}

/// CPU statistics from /proc/stat
struct CpuStats {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    interrupts: u64,
    context_switches: u64,
}

/// Process statistics
struct ProcStats {
    running: u32,
    blocked: u32,
}

fn read_meminfo() -> MemInfo {
    let mut mem = MemInfo {
        free: 0,
        buffers: 0,
        cached: 0,
        swap_total: 0,
        swap_free: 0,
        swap_in: 0,
        swap_out: 0,
    };

    let fd = io::open(b"/proc/meminfo", libc::O_RDONLY, 0);
    if fd < 0 {
        return mem;
    }

    let mut buf = [0u8; 4096];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return mem;
    }

    let content = &buf[..n as usize];
    for line in content.split(|&c| c == b'\n') {
        if line.starts_with(b"MemFree:") {
            mem.free = parse_meminfo_value(line);
        } else if line.starts_with(b"Buffers:") {
            mem.buffers = parse_meminfo_value(line);
        } else if line.starts_with(b"Cached:") {
            mem.cached = parse_meminfo_value(line);
        } else if line.starts_with(b"SwapTotal:") {
            mem.swap_total = parse_meminfo_value(line);
        } else if line.starts_with(b"SwapFree:") {
            mem.swap_free = parse_meminfo_value(line);
        }
    }

    // Read swap activity from /proc/vmstat
    let fd = io::open(b"/proc/vmstat", libc::O_RDONLY, 0);
    if fd >= 0 {
        let n = io::read(fd, &mut buf);
        io::close(fd);

        if n > 0 {
            let content = &buf[..n as usize];
            for line in content.split(|&c| c == b'\n') {
                if line.starts_with(b"pswpin ") {
                    mem.swap_in = parse_stat_value(line);
                } else if line.starts_with(b"pswpout ") {
                    mem.swap_out = parse_stat_value(line);
                }
            }
        }
    }

    mem
}

fn read_stat() -> CpuStats {
    let mut cpu = CpuStats {
        user: 0,
        nice: 0,
        system: 0,
        idle: 0,
        iowait: 0,
        irq: 0,
        softirq: 0,
        steal: 0,
        interrupts: 0,
        context_switches: 0,
    };

    let fd = io::open(b"/proc/stat", libc::O_RDONLY, 0);
    if fd < 0 {
        return cpu;
    }

    let mut buf = [0u8; 4096];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return cpu;
    }

    let content = &buf[..n as usize];
    for line in content.split(|&c| c == b'\n') {
        if line.starts_with(b"cpu ") {
            // cpu user nice system idle iowait irq softirq steal guest guest_nice
            let parts: Vec<&[u8]> = line
                .split(|&c| c == b' ')
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 9 {
                cpu.user = sys::parse_u64(parts[1]).unwrap_or(0);
                cpu.nice = sys::parse_u64(parts[2]).unwrap_or(0);
                cpu.system = sys::parse_u64(parts[3]).unwrap_or(0);
                cpu.idle = sys::parse_u64(parts[4]).unwrap_or(0);
                cpu.iowait = sys::parse_u64(parts[5]).unwrap_or(0);
                cpu.irq = sys::parse_u64(parts[6]).unwrap_or(0);
                cpu.softirq = sys::parse_u64(parts[7]).unwrap_or(0);
                if parts.len() >= 9 {
                    cpu.steal = sys::parse_u64(parts[8]).unwrap_or(0);
                }
            }
        } else if line.starts_with(b"intr ") {
            // First number after "intr " is total interrupts
            let parts: Vec<&[u8]> = line
                .split(|&c| c == b' ')
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 2 {
                cpu.interrupts = sys::parse_u64(parts[1]).unwrap_or(0);
            }
        } else if line.starts_with(b"ctxt ") {
            cpu.context_switches = parse_stat_value(line);
        }
    }

    cpu
}

fn read_proc_stats() -> ProcStats {
    let mut stats = ProcStats {
        running: 0,
        blocked: 0,
    };

    let fd = io::open(b"/proc/stat", libc::O_RDONLY, 0);
    if fd < 0 {
        return stats;
    }

    let mut buf = [0u8; 4096];
    let n = io::read(fd, &mut buf);
    io::close(fd);

    if n <= 0 {
        return stats;
    }

    let content = &buf[..n as usize];
    for line in content.split(|&c| c == b'\n') {
        if line.starts_with(b"procs_running ") {
            stats.running = parse_stat_value(line) as u32;
        } else if line.starts_with(b"procs_blocked ") {
            stats.blocked = parse_stat_value(line) as u32;
        }
    }

    stats
}

/// Parse value from meminfo line like "MemFree:      1234 kB"
fn parse_meminfo_value(line: &[u8]) -> u64 {
    let parts: Vec<&[u8]> = line
        .split(|&c| c == b' ')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() >= 2 {
        sys::parse_u64(parts[1]).unwrap_or(0)
    } else {
        0
    }
}

/// Parse value from stat line like "ctxt 12345"
fn parse_stat_value(line: &[u8]) -> u64 {
    let parts: Vec<&[u8]> = line
        .split(|&c| c == b' ')
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() >= 2 {
        sys::parse_u64(parts[1]).unwrap_or(0)
    } else {
        0
    }
}

fn print_stats(mem: &MemInfo, cpu: &CpuStats, proc_stats: &ProcStats) {
    let mut buf = [0u8; 16];

    // Calculate CPU percentages
    let total_cpu = cpu.user
        + cpu.nice
        + cpu.system
        + cpu.idle
        + cpu.iowait
        + cpu.irq
        + cpu.softirq
        + cpu.steal;

    // One guard covers all four divisions, and an idle CPU reads as 100% id
    #[allow(clippy::manual_checked_ops)]
    let (us, sy, id, wa, st) = if total_cpu > 0 {
        let us = ((cpu.user + cpu.nice) * 100 / total_cpu) as u32;
        let sy = ((cpu.system + cpu.irq + cpu.softirq) * 100 / total_cpu) as u32;
        let wa = (cpu.iowait * 100 / total_cpu) as u32;
        let st = (cpu.steal * 100 / total_cpu) as u32;
        let id = 100u32.saturating_sub(us + sy + wa + st);
        (us, sy, id, wa, st)
    } else {
        (0, 0, 100, 0, 0)
    };

    // Calculate swap used
    let swap_used = mem.swap_total.saturating_sub(mem.swap_free);

    // Print: r b swpd free buff cache si so bi bo in cs us sy id wa st

    // r (running processes)
    print_field(proc_stats.running as u64, 2, &mut buf);

    // b (blocked processes)
    print_field(proc_stats.blocked as u64, 2, &mut buf);

    // swpd (swap used, kB)
    print_field(swap_used, 7, &mut buf);

    // free (free memory, kB)
    print_field(mem.free, 7, &mut buf);

    // buff (buffer memory, kB)
    print_field(mem.buffers, 6, &mut buf);

    // cache (cached memory, kB)
    print_field(mem.cached, 6, &mut buf);

    // si (swap in, kB/s - simplified as total)
    print_field(mem.swap_in % 10000, 5, &mut buf);

    // so (swap out, kB/s - simplified as total)
    print_field(mem.swap_out % 10000, 5, &mut buf);

    // bi (blocks in - placeholder)
    print_field(0, 6, &mut buf);

    // bo (blocks out - placeholder)
    print_field(0, 6, &mut buf);

    // in (interrupts per second - simplified)
    print_field((cpu.interrupts / 100) % 10000, 5, &mut buf);

    // cs (context switches per second - simplified)
    print_field((cpu.context_switches / 100) % 10000, 5, &mut buf);

    // us (user CPU %)
    print_field(us as u64, 3, &mut buf);

    // sy (system CPU %)
    print_field(sy as u64, 3, &mut buf);

    // id (idle CPU %)
    print_field(id as u64, 3, &mut buf);

    // wa (IO wait CPU %)
    print_field(wa as u64, 3, &mut buf);

    // st (steal CPU %)
    print_field(st as u64, 3, &mut buf);

    io::write_str(1, b"\n");
}

fn print_field(value: u64, width: usize, buf: &mut [u8; 16]) {
    let s = sys::format_u64(value, buf);
    for _ in 0..(width.saturating_sub(s.len())) {
        io::write_str(1, b" ");
    }
    io::write_all(1, s);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_vmstat_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["vmstat"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("procs"));
        assert!(stdout.contains("memory"));
    }

    #[test]
    fn test_vmstat_with_header_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["vmstat", "-n"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }
}
