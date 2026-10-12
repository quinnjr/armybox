//! uclampset - manipulate the utilization clamping attributes of a process
//!
//! Set or retrieve utilization clamping attributes.

#![allow(dead_code)]

extern crate alloc;
use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

// sched_attr structure size
const SCHED_ATTR_SIZE: u32 = 56;

// Scheduling policies
const SCHED_NORMAL: u32 = 0;
const SCHED_FIFO: u32 = 1;
const SCHED_RR: u32 = 2;
const SCHED_BATCH: u32 = 3;
const SCHED_IDLE: u32 = 5;
const SCHED_DEADLINE: u32 = 6;

// Flags for sched_getattr/sched_setattr
const SCHED_FLAG_RESET_ON_FORK: u64 = 0x01;
const SCHED_FLAG_RECLAIM: u64 = 0x02;
const SCHED_FLAG_DL_OVERRUN: u64 = 0x04;
const SCHED_FLAG_KEEP_POLICY: u64 = 0x08;
const SCHED_FLAG_KEEP_PARAMS: u64 = 0x10;
const SCHED_FLAG_UTIL_CLAMP_MIN: u64 = 0x20;
const SCHED_FLAG_UTIL_CLAMP_MAX: u64 = 0x40;
const SCHED_FLAG_UTIL_CLAMP: u64 = SCHED_FLAG_UTIL_CLAMP_MIN | SCHED_FLAG_UTIL_CLAMP_MAX;

// sched_attr structure
#[repr(C)]
#[derive(Clone, Copy)]
struct SchedAttr {
    size: u32,
    sched_policy: u32,
    sched_flags: u64,
    sched_nice: i32,
    sched_priority: u32,
    sched_runtime: u64,
    sched_deadline: u64,
    sched_period: u64,
    sched_util_min: u32,
    sched_util_max: u32,
}

impl Default for SchedAttr {
    fn default() -> Self {
        Self {
            size: SCHED_ATTR_SIZE,
            sched_policy: 0,
            sched_flags: 0,
            sched_nice: 0,
            sched_priority: 0,
            sched_runtime: 0,
            sched_deadline: 0,
            sched_period: 0,
            sched_util_min: 0,
            sched_util_max: 1024,
        }
    }
}

/// uclampset - manipulate the utilization clamping attributes
///
/// # Synopsis
/// ```text
/// uclampset [options] command [args]
/// uclampset [options] -p pid
/// ```
///
/// # Description
/// Set or retrieve the utilization clamping attributes of a process.
///
/// # Options
/// - `-m MIN`: Set minimum utilization (0-1024)
/// - `-M MAX`: Set maximum utilization (0-1024)
/// - `-a`: Reset on fork
/// - `-p PID`: Operate on existing process
/// - `-R`: Reset to system defaults
/// - `-s`: Show current settings
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn uclampset(argc: i32, argv: *const *const u8) -> i32 {
    let mut min: Option<u32> = None;
    let mut max: Option<u32> = None;
    let mut reset_on_fork = false;
    let mut pid: Option<i32> = None;
    let mut reset = false;
    let mut show = false;
    let mut command_start = 0usize;

    // Parse arguments
    let mut i = 1;
    while i < argc as usize {
        let arg = match unsafe { get_arg(argv, i as i32) } {
            Some(a) => a,
            None => break,
        };

        if arg == b"-m" {
            i += 1;
            if let Some(v) = unsafe { get_arg(argv, i as i32) } {
                min = Some(sys::parse_u64(v).unwrap_or(0) as u32);
            }
        } else if arg == b"-M" {
            i += 1;
            if let Some(v) = unsafe { get_arg(argv, i as i32) } {
                max = Some(sys::parse_u64(v).unwrap_or(1024) as u32);
            }
        } else if arg == b"-a" {
            reset_on_fork = true;
        } else if arg == b"-p" {
            i += 1;
            if let Some(p) = unsafe { get_arg(argv, i as i32) } {
                pid = Some(sys::parse_i64(p).unwrap_or(0) as i32);
            }
        } else if arg == b"-R" {
            reset = true;
        } else if arg == b"-s" {
            show = true;
        } else if arg == b"-h" || arg == b"--help" {
            print_usage();
            return 0;
        } else if !arg.starts_with(b"-") {
            command_start = i;
            break;
        }
        i += 1;
    }

    // Determine target PID
    let target_pid = match pid {
        Some(p) => p,
        None => {
            if command_start == 0 && !show {
                io::write_str(2, b"uclampset: must specify pid or command\n");
                print_usage();
                return 1;
            }
            0 // Will be set after fork if running command
        }
    };

    // Show current settings
    if show {
        let p = if target_pid == 0 {
            unsafe { libc::getpid() }
        } else {
            target_pid
        };
        return show_uclamp(p);
    }

    // Set attributes
    if pid.is_some() {
        // Operate on existing process
        return set_uclamp(target_pid, min, max, reset, reset_on_fork);
    }

    // Run command with new settings
    if command_start > 0 {
        // Apply to self first
        if set_uclamp(0, min, max, reset, reset_on_fork) != 0 {
            return 1;
        }

        // Collect command and args
        let mut cmd_args: Vec<*const u8> = Vec::new();
        for j in command_start..(argc as usize) {
            if let Some(arg) = unsafe { get_arg(argv, j as i32) } {
                // Add null terminator
                let mut arg_cstr = Vec::from(arg);
                arg_cstr.push(0);
                cmd_args.push(arg_cstr.leak().as_ptr());
            }
        }
        cmd_args.push(core::ptr::null());

        if cmd_args.len() <= 1 {
            io::write_str(2, b"uclampset: missing command\n");
            return 1;
        }

        // Execute command
        unsafe {
            libc::execvp(
                cmd_args[0] as *const libc::c_char,
                cmd_args.as_ptr() as *const *const libc::c_char,
            );
        }

        sys::perror(b"exec");
        return 1;
    }

    io::write_str(2, b"uclampset: must specify pid or command\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_uclamp(
    pid: i32,
    min: Option<u32>,
    max: Option<u32>,
    reset: bool,
    reset_on_fork: bool,
) -> i32 {
    let mut attr = SchedAttr::default();

    // Get current attributes first
    if unsafe { sched_getattr(pid, &mut attr, SCHED_ATTR_SIZE, 0) } < 0 {
        sys::perror(b"sched_getattr");
        return 1;
    }

    if reset {
        attr.sched_util_min = 0;
        attr.sched_util_max = 1024;
        attr.sched_flags &= !(SCHED_FLAG_UTIL_CLAMP);
    } else {
        if let Some(m) = min {
            attr.sched_util_min = m.min(1024);
            attr.sched_flags |= SCHED_FLAG_UTIL_CLAMP_MIN;
        }
        if let Some(m) = max {
            attr.sched_util_max = m.min(1024);
            attr.sched_flags |= SCHED_FLAG_UTIL_CLAMP_MAX;
        }
    }

    if reset_on_fork {
        attr.sched_flags |= SCHED_FLAG_RESET_ON_FORK;
    }

    // Keep existing policy and params
    attr.sched_flags |= SCHED_FLAG_KEEP_POLICY | SCHED_FLAG_KEEP_PARAMS;

    if unsafe { sched_setattr(pid, &attr, 0) } < 0 {
        sys::perror(b"sched_setattr");
        return 1;
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn show_uclamp(pid: i32) -> i32 {
    let mut attr = SchedAttr::default();

    if unsafe { sched_getattr(pid, &mut attr, SCHED_ATTR_SIZE, 0) } < 0 {
        sys::perror(b"sched_getattr");
        return 1;
    }

    io::write_str(1, b"pid ");
    let mut num_buf = [0u8; 16];
    io::write_all(1, sys::format_i64(pid as i64, &mut num_buf));
    io::write_str(1, b"'s current uclamp values:\n");

    io::write_str(1, b"    util_min: ");
    io::write_all(1, sys::format_u64(attr.sched_util_min as u64, &mut num_buf));
    io::write_str(1, b"\n");

    io::write_str(1, b"    util_max: ");
    io::write_all(1, sys::format_u64(attr.sched_util_max as u64, &mut num_buf));
    io::write_str(1, b"\n");

    0
}

// Syscall wrappers
#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn sched_getattr(pid: i32, attr: *mut SchedAttr, size: u32, flags: u32) -> i32 {
    unsafe { libc::syscall(libc::SYS_sched_getattr, pid, attr, size, flags) as i32 }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
unsafe fn sched_setattr(pid: i32, attr: *const SchedAttr, flags: u32) -> i32 {
    unsafe { libc::syscall(libc::SYS_sched_setattr, pid, attr, flags) as i32 }
}

fn print_usage() {
    io::write_str(1, b"Usage: uclampset [options] command [args]\n");
    io::write_str(1, b"       uclampset [options] -p pid\n\n");
    io::write_str(1, b"Set or show utilization clamping attributes.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -m MIN   Set minimum utilization (0-1024)\n");
    io::write_str(1, b"  -M MAX   Set maximum utilization (0-1024)\n");
    io::write_str(1, b"  -a       Reset on fork\n");
    io::write_str(1, b"  -p PID   Operate on existing process\n");
    io::write_str(1, b"  -R       Reset to system defaults\n");
    io::write_str(1, b"  -s       Show current settings\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn uclampset(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"uclampset: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_uclampset_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["uclampset", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_uclampset_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["uclampset"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
