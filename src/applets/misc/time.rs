//! time - time command execution
//!
//! Run a command and report how long it took including user/sys CPU time.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::ffi::CString;
use alloc::vec::Vec;

/// rusage structure for getrusage()
#[repr(C)]
struct Rusage {
    ru_utime: Timeval,         // user CPU time
    ru_stime: Timeval,         // system CPU time
    ru_maxrss: libc::c_long,   // max resident set size
    ru_ixrss: libc::c_long,    // integral shared memory size
    ru_idrss: libc::c_long,    // integral unshared data size
    ru_isrss: libc::c_long,    // integral unshared stack size
    ru_minflt: libc::c_long,   // page reclaims (soft page faults)
    ru_majflt: libc::c_long,   // page faults (hard page faults)
    ru_nswap: libc::c_long,    // swaps
    ru_inblock: libc::c_long,  // block input operations
    ru_oublock: libc::c_long,  // block output operations
    ru_msgsnd: libc::c_long,   // IPC messages sent
    ru_msgrcv: libc::c_long,   // IPC messages received
    ru_nsignals: libc::c_long, // signals received
    ru_nvcsw: libc::c_long,    // voluntary context switches
    ru_nivcsw: libc::c_long,   // involuntary context switches
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Timeval {
    tv_sec: i64,
    tv_usec: libc::suseconds_t,
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: libc::c_long,
}

const CLOCK_MONOTONIC: libc::clockid_t = 1;

unsafe extern "C" {
    fn wait4(
        pid: libc::pid_t,
        status: *mut libc::c_int,
        options: libc::c_int,
        rusage: *mut Rusage,
    ) -> libc::pid_t;
    fn clock_gettime(clockid: libc::clockid_t, tp: *mut Timespec) -> libc::c_int;
}

/// time - time command execution
///
/// # Synopsis
/// ```text
/// time [-p] [-v] COMMAND [ARGS...]
/// ```
///
/// # Description
/// Run COMMAND and report timing information including real (wall-clock),
/// user (CPU time in user mode), and system (CPU time in kernel mode) times.
///
/// # Options
/// - `-p`: Use POSIX portable output format
/// - `-v`: Verbose output with memory and I/O statistics
///
/// # Exit Status
/// - Exit status of COMMAND, or 127 if command cannot be found
pub fn time(argc: i32, argv: *const *const u8) -> i32 {
    let mut portable = false;
    let mut verbose = false;
    let mut cmd_start = 1;

    // Parse options
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-p" || arg == b"--portability" {
                portable = true;
                cmd_start = i + 1;
            } else if arg == b"-v" || arg == b"--verbose" {
                verbose = true;
                cmd_start = i + 1;
            } else if arg == b"-h" || arg == b"--help" {
                print_help();
                return 0;
            } else if arg == b"--" {
                cmd_start = i + 1;
                break;
            } else if !arg.starts_with(b"-") {
                cmd_start = i;
                break;
            }
        }
    }

    if cmd_start >= argc {
        io::write_str(2, b"time: missing command\n");
        io::write_str(2, b"Usage: time [-p] [-v] COMMAND [ARGS...]\n");
        return 1;
    }

    // Get start time with high precision
    let mut start_time = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut start_time);
    }

    // Fork and exec
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        io::write_str(2, b"time: fork failed\n");
        return 1;
    }

    if pid == 0 {
        // Child process - exec the command
        let mut args: Vec<CString> = Vec::new();

        for i in cmd_start..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                let mut v = Vec::with_capacity(arg.len() + 1);
                v.extend_from_slice(arg);
                v.push(0);
                if let Ok(cs) = CString::from_vec_with_nul(v) {
                    args.push(cs);
                }
            }
        }

        let ptrs: Vec<*const libc::c_char> = args
            .iter()
            .map(|s| s.as_ptr())
            .chain(core::iter::once(core::ptr::null()))
            .collect();

        unsafe {
            libc::execvp(ptrs[0], ptrs.as_ptr());
        }

        // If exec failed
        io::write_str(2, b"time: ");
        if let Some(cmd) = unsafe { get_arg(argv, cmd_start) } {
            io::write_all(2, cmd);
        }
        io::write_str(2, b": command not found\n");
        unsafe {
            libc::_exit(127);
        }
    }

    // Parent - wait for child and get resource usage
    let mut status: libc::c_int = 0;
    let mut rusage: Rusage = unsafe { core::mem::zeroed() };

    let ret = unsafe { wait4(pid, &mut status, 0, &mut rusage) };

    // Get end time
    let mut end_time = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_gettime(CLOCK_MONOTONIC, &mut end_time);
    }

    if ret < 0 {
        io::write_str(2, b"time: wait failed\n");
        return 1;
    }

    // Calculate real time
    let real_sec = end_time.tv_sec - start_time.tv_sec;
    let mut real_nsec = end_time.tv_nsec - start_time.tv_nsec;
    let real_sec = if real_nsec < 0 {
        real_nsec += 1_000_000_000;
        real_sec - 1
    } else {
        real_sec
    };

    // Convert rusage times to seconds and microseconds
    let user_sec = rusage.ru_utime.tv_sec as u64;
    let user_usec = rusage.ru_utime.tv_usec as u64;
    let sys_sec = rusage.ru_stime.tv_sec as u64;
    let sys_usec = rusage.ru_stime.tv_usec as u64;

    io::write_str(2, b"\n");

    if portable {
        // POSIX portable format
        print_portable_time(b"real", real_sec as u64, (real_nsec / 1000) as u64);
        print_portable_time(b"user", user_sec, user_usec);
        print_portable_time(b"sys", sys_sec, sys_usec);
    } else if verbose {
        // Verbose format with all stats
        print_verbose_stats(
            real_sec as u64,
            (real_nsec / 1000) as u64,
            user_sec,
            user_usec,
            sys_sec,
            sys_usec,
            &rusage,
        );
    } else {
        // Default bash-like format
        print_default_time(b"real", real_sec as u64, (real_nsec / 1_000_000) as u64);
        print_default_time(b"user", user_sec, user_usec / 1000);
        print_default_time(b"sys", sys_sec, sys_usec / 1000);
    }

    // Return child's exit status
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    }
}

/// Print time in POSIX portable format: name Xm Y.YYs
fn print_portable_time(name: &[u8], secs: u64, usecs: u64) {
    io::write_all(2, name);
    io::write_str(2, b" ");

    let minutes = secs / 60;
    let seconds = secs % 60;
    let centisecs = usecs / 10000; // Convert to hundredths

    io::write_num(2, minutes);
    io::write_str(2, b"m");
    io::write_num(2, seconds);
    io::write_str(2, b".");
    if centisecs < 10 {
        io::write_str(2, b"0");
    }
    io::write_num(2, centisecs);
    io::write_str(2, b"s\n");
}

/// Print time in default bash-like format with tab: name\tXmY.YYYs
fn print_default_time(name: &[u8], secs: u64, msecs: u64) {
    io::write_all(2, name);
    io::write_str(2, b"\t");

    let minutes = secs / 60;
    let seconds = secs % 60;

    io::write_num(2, minutes);
    io::write_str(2, b"m");
    io::write_num(2, seconds);
    io::write_str(2, b".");

    // Print milliseconds with leading zeros
    if msecs < 100 {
        io::write_str(2, b"0");
    }
    if msecs < 10 {
        io::write_str(2, b"0");
    }
    io::write_num(2, msecs);
    io::write_str(2, b"s\n");
}

/// Print verbose statistics (like GNU time -v)
fn print_verbose_stats(
    real_sec: u64,
    real_usec: u64,
    user_sec: u64,
    user_usec: u64,
    sys_sec: u64,
    sys_usec: u64,
    rusage: &Rusage,
) {
    io::write_str(2, b"\tCommand being timed\n");

    // Time stats
    io::write_str(2, b"\tUser time (seconds): ");
    print_seconds(user_sec, user_usec);
    io::write_str(2, b"\n");

    io::write_str(2, b"\tSystem time (seconds): ");
    print_seconds(sys_sec, sys_usec);
    io::write_str(2, b"\n");

    // Calculate CPU percentage
    let total_cpu_usec = (user_sec * 1_000_000 + user_usec) + (sys_sec * 1_000_000 + sys_usec);
    let real_usec_total = real_sec * 1_000_000 + real_usec;
    let cpu_percent = (total_cpu_usec * 100)
        .checked_div(real_usec_total)
        .unwrap_or(0);

    io::write_str(2, b"\tPercent of CPU this job got: ");
    io::write_num(2, cpu_percent);
    io::write_str(2, b"%\n");

    io::write_str(2, b"\tElapsed (wall clock) time: ");
    let minutes = real_sec / 60;
    let seconds = real_sec % 60;
    let hours = minutes / 60;
    let minutes = minutes % 60;
    if hours > 0 {
        io::write_num(2, hours);
        io::write_str(2, b":");
    }
    if minutes < 10 && hours > 0 {
        io::write_str(2, b"0");
    }
    io::write_num(2, minutes);
    io::write_str(2, b":");
    if seconds < 10 {
        io::write_str(2, b"0");
    }
    io::write_num(2, seconds);
    io::write_str(2, b".");
    let centisecs = real_usec / 10000;
    if centisecs < 10 {
        io::write_str(2, b"0");
    }
    io::write_num(2, centisecs);
    io::write_str(2, b"\n");

    // Memory stats
    io::write_str(2, b"\tMaximum resident set size (kbytes): ");
    io::write_num(2, rusage.ru_maxrss as u64);
    io::write_str(2, b"\n");

    // Page fault stats
    io::write_str(2, b"\tMajor (requiring I/O) page faults: ");
    io::write_num(2, rusage.ru_majflt as u64);
    io::write_str(2, b"\n");

    io::write_str(2, b"\tMinor (reclaiming a frame) page faults: ");
    io::write_num(2, rusage.ru_minflt as u64);
    io::write_str(2, b"\n");

    // I/O stats
    io::write_str(2, b"\tVoluntary context switches: ");
    io::write_num(2, rusage.ru_nvcsw as u64);
    io::write_str(2, b"\n");

    io::write_str(2, b"\tInvoluntary context switches: ");
    io::write_num(2, rusage.ru_nivcsw as u64);
    io::write_str(2, b"\n");

    io::write_str(2, b"\tFile system inputs: ");
    io::write_num(2, rusage.ru_inblock as u64);
    io::write_str(2, b"\n");

    io::write_str(2, b"\tFile system outputs: ");
    io::write_num(2, rusage.ru_oublock as u64);
    io::write_str(2, b"\n");
}

/// Print seconds with microsecond precision
fn print_seconds(secs: u64, usecs: u64) {
    io::write_num(2, secs);
    io::write_str(2, b".");

    // Print microseconds with leading zeros
    let mut num_buf = [0u8; 16];
    let usec_str = sys::format_u64(usecs, &mut num_buf);
    // Pad to 6 digits
    for _ in 0..(6 - usec_str.len()) {
        io::write_str(2, b"0");
    }
    io::write_all(2, usec_str);
}

fn print_help() {
    io::write_str(1, b"Usage: time [-p] [-v] COMMAND [ARGS...]\n\n");
    io::write_str(1, b"Run COMMAND and report time statistics.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(
        1,
        b"  -p, --portability   Use POSIX portable output format\n",
    );
    io::write_str(
        1,
        b"  -v, --verbose       Print all resource usage statistics\n",
    );
    io::write_str(1, b"  -h, --help          Display this help\n\n");
    io::write_str(1, b"Output:\n");
    io::write_str(1, b"  real    Wall clock time (elapsed time)\n");
    io::write_str(1, b"  user    CPU time in user mode\n");
    io::write_str(1, b"  sys     CPU time in kernel mode\n");
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_time_true() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("real"));
        assert!(stderr.contains("user"));
        assert!(stderr.contains("sys"));
    }

    #[test]
    fn test_time_no_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["time"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("missing command"));
    }

    #[test]
    fn test_time_portable() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "-p", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        // Portable format uses "real Xm Y.YYs"
        assert!(stderr.contains("real "));
        assert!(stderr.contains("m"));
    }

    #[test]
    fn test_time_verbose() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "-v", "true"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Maximum resident set size"));
        assert!(stderr.contains("User time"));
    }

    #[test]
    fn test_time_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "--help"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_time_exit_status() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "false"])
            .output()
            .unwrap();

        // 'false' returns exit code 1
        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_time_nonexistent_command() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "nonexistent_command_12345"])
            .output()
            .unwrap();

        // Should return 127 for command not found
        assert_eq!(output.status.code(), Some(127));
    }

    #[test]
    fn test_time_with_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["time", "echo", "hello", "world"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("hello world"));
    }
}
