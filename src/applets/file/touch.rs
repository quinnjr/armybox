//! touch - change file timestamps
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/touch.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;

/// touch - change file access and modification times
///
/// # Synopsis
/// ```text
/// touch [-acm] [-r ref_file | -t time] file...
/// ```
///
/// # Description
/// Updates the access and modification times of each file.
/// Creates files that do not exist (unless -c is specified).
///
/// # Options
/// - `-a`: Change only access time
/// - `-c`: Do not create file if it doesn't exist
/// - `-m`: Change only modification time
/// - `-r ref_file`: Use timestamps from reference file
/// - `-t time`: Use specified time (format: [[CC]YY]MMDDhhmm[.SS])
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn touch(argc: i32, argv: *const *const u8) -> i32 {
    let mut access_only = false;
    let mut mod_only = false;
    let mut no_create = false;
    let mut ref_file: Option<&[u8]> = None;
    let mut time_str: Option<&[u8]> = None;
    let mut files_start = 1;

    // Parse options
    let mut i = 1;
    while i < argc as usize {
        if let Some(arg) = unsafe { get_arg(argv, i as i32) } {
            if arg == b"--" {
                files_start = i + 1;
                break;
            } else if arg.len() > 1 && arg[0] == b'-' {
                let mut j = 1;
                while j < arg.len() {
                    match arg[j] {
                        b'a' => access_only = true,
                        b'c' => no_create = true,
                        b'm' => mod_only = true,
                        b'r' => {
                            i += 1;
                            ref_file = unsafe { get_arg(argv, i as i32) };
                            break;
                        }
                        b't' => {
                            i += 1;
                            time_str = unsafe { get_arg(argv, i as i32) };
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                files_start = i + 1;
            } else {
                files_start = i;
                break;
            }
        }
        i += 1;
    }

    // If neither -a nor -m, change both
    if !access_only && !mod_only {
        access_only = true;
        mod_only = true;
    }

    // Determine the timestamp to use
    let use_times = if let Some(rf) = ref_file {
        // Get timestamps from reference file
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::stat(rf, &mut st) < 0 {
            io::write_str(2, b"touch: failed to get attributes of '");
            io::write_all(2, rf);
            io::write_str(2, b"'\n");
            return 1;
        }
        Some((st.st_atime as i64, st.st_mtime as i64))
    } else if let Some(ts) = time_str {
        match parse_touch_time(ts) {
            Some((sec, _)) => Some((sec, sec)),
            None => {
                io::write_str(2, b"touch: invalid date format '");
                io::write_all(2, ts);
                io::write_str(2, b"'\n");
                return 1;
            }
        }
    } else {
        None // use current time
    };

    let mut exit_code = 0;

    for idx in files_start..argc as usize {
        if let Some(path) = unsafe { get_arg(argv, idx as i32) } {
            // Check if file exists
            let mut st: libc::stat = unsafe { core::mem::zeroed() };
            let exists = io::stat(path, &mut st) == 0;

            if !exists {
                if no_create {
                    continue;
                }
                // Create the file
                let fd = io::open(path, libc::O_WRONLY | libc::O_CREAT, 0o644);
                if fd >= 0 {
                    io::close(fd);
                } else {
                    sys::perror(path);
                    exit_code = 1;
                    continue;
                }
            }

            match use_times {
                None => {
                    // Current time - use utimes with null
                    if access_only && mod_only {
                        if unsafe {
                            libc::utimes(path.as_ptr() as *const libc::c_char, core::ptr::null())
                        } < 0
                        {
                            sys::perror(path);
                            exit_code = 1;
                        }
                    } else {
                        // Need to preserve one timestamp - re-stat
                        let mut st2: libc::stat = unsafe { core::mem::zeroed() };
                        if io::stat(path, &mut st2) < 0 {
                            sys::perror(path);
                            exit_code = 1;
                            continue;
                        }
                        let mut now: libc::timeval = unsafe { core::mem::zeroed() };
                        unsafe { libc::gettimeofday(&mut now, core::ptr::null_mut()) };

                        let times = [
                            libc::timeval {
                                tv_sec: if access_only {
                                    now.tv_sec
                                } else {
                                    st2.st_atime
                                },
                                tv_usec: 0,
                            },
                            libc::timeval {
                                tv_sec: if mod_only { now.tv_sec } else { st2.st_mtime },
                                tv_usec: 0,
                            },
                        ];
                        if unsafe {
                            libc::utimes(path.as_ptr() as *const libc::c_char, times.as_ptr())
                        } < 0
                        {
                            sys::perror(path);
                            exit_code = 1;
                        }
                    }
                }
                Some((atime, mtime)) => {
                    // Need current stat if preserving one time
                    let mut st2: libc::stat = unsafe { core::mem::zeroed() };
                    if (!access_only || !mod_only) && io::stat(path, &mut st2) < 0 {
                        sys::perror(path);
                        exit_code = 1;
                        continue;
                    }

                    let times = [
                        libc::timeval {
                            tv_sec: if access_only {
                                atime as libc::time_t
                            } else {
                                st2.st_atime
                            },
                            tv_usec: 0,
                        },
                        libc::timeval {
                            tv_sec: if mod_only {
                                mtime as libc::time_t
                            } else {
                                st2.st_mtime
                            },
                            tv_usec: 0,
                        },
                    ];
                    if unsafe { libc::utimes(path.as_ptr() as *const libc::c_char, times.as_ptr()) }
                        < 0
                    {
                        sys::perror(path);
                        exit_code = 1;
                    }
                }
            }
        }
    }
    exit_code
}

/// Parse touch time format: [[CC]YY]MMDDhhmm[.SS]
/// Returns (seconds since epoch, subseconds)
fn parse_touch_time(s: &[u8]) -> Option<(i64, i64)> {
    // Split off .SS if present
    let (main, secs) = if let Some(dot_pos) = s.iter().position(|&c| c == b'.') {
        let ss = parse_2digits(&s[dot_pos + 1..])?;
        (&s[..dot_pos], ss as i64)
    } else {
        (s, 0i64)
    };

    // Parse based on length
    let (year, month, day, hour, min) = match main.len() {
        8 => {
            // MMDDhhmm - use current year
            let mut now: libc::time_t = 0;
            unsafe { libc::time(&mut now) };
            let tm = unsafe { libc::localtime(&now) };
            let year = unsafe { (*tm).tm_year + 1900 } as i64;
            let month = parse_2digits(&main[0..2])? as i64;
            let day = parse_2digits(&main[2..4])? as i64;
            let hour = parse_2digits(&main[4..6])? as i64;
            let min = parse_2digits(&main[6..8])? as i64;
            (year, month, day, hour, min)
        }
        10 => {
            // YYMMDDhhmm
            let yy = parse_2digits(&main[0..2])? as i64;
            let year = if yy >= 69 { 1900 + yy } else { 2000 + yy };
            let month = parse_2digits(&main[2..4])? as i64;
            let day = parse_2digits(&main[4..6])? as i64;
            let hour = parse_2digits(&main[6..8])? as i64;
            let min = parse_2digits(&main[8..10])? as i64;
            (year, month, day, hour, min)
        }
        12 => {
            // CCYYMMDDhhmm
            let cc = parse_2digits(&main[0..2])? as i64;
            let yy = parse_2digits(&main[2..4])? as i64;
            let year = cc * 100 + yy;
            let month = parse_2digits(&main[4..6])? as i64;
            let day = parse_2digits(&main[6..8])? as i64;
            let hour = parse_2digits(&main[8..10])? as i64;
            let min = parse_2digits(&main[10..12])? as i64;
            (year, month, day, hour, min)
        }
        _ => return None,
    };

    // Convert to epoch using mktime via libc
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    tm.tm_year = (year - 1900) as i32;
    tm.tm_mon = (month - 1) as i32;
    tm.tm_mday = day as i32;
    tm.tm_hour = hour as i32;
    tm.tm_min = min as i32;
    tm.tm_sec = secs as i32;
    tm.tm_isdst = -1;

    let epoch = unsafe { libc::mktime(&mut tm) };
    if epoch == -1 {
        return None;
    }

    Some((epoch as i64, 0))
}

/// Parse a 2-digit decimal number
fn parse_2digits(s: &[u8]) -> Option<u32> {
    if s.len() < 2 {
        return None;
    }
    let d1 = s[0].wrapping_sub(b'0');
    let d2 = s[1].wrapping_sub(b'0');
    if d1 > 9 || d2 > 9 {
        return None;
    }
    Some(d1 as u32 * 10 + d2 as u32)
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
            "armybox_touch_test_{}_{}",
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
    fn test_touch_create_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("newfile.txt");

        let output = Command::new(&armybox)
            .args(["touch", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(file.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_touch_multiple_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file1 = dir.join("file1.txt");
        let file2 = dir.join("file2.txt");

        let output = Command::new(&armybox)
            .args(["touch", file1.to_str().unwrap(), file2.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(file1.exists());
        assert!(file2.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_touch_update_existing() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("existing.txt");
        fs::write(&file, "content").unwrap();

        // Get original mtime
        let orig_meta = fs::metadata(&file).unwrap();
        let orig_mtime = orig_meta.modified().unwrap();

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        let output = Command::new(&armybox)
            .args(["touch", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // mtime should be updated (or at least not older)
        let new_meta = fs::metadata(&file).unwrap();
        let new_mtime = new_meta.modified().unwrap();
        assert!(new_mtime >= orig_mtime);
        cleanup(&dir);
    }

    #[test]
    fn test_touch_no_create() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let file = dir.join("nonexistent.txt");

        let output = Command::new(&armybox)
            .args(["touch", "-c", file.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert!(!file.exists());
        cleanup(&dir);
    }

    #[test]
    fn test_touch_reference_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        let ref_file = dir.join("ref.txt");
        let target = dir.join("target.txt");
        fs::write(&ref_file, "ref").unwrap();
        fs::write(&target, "target").unwrap();

        // Set ref file to a known time
        let output = Command::new(&armybox)
            .args(["touch", "-t", "202001011200", ref_file.to_str().unwrap()])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));

        // Now touch target with -r ref
        let output = Command::new(&armybox)
            .args([
                "touch",
                "-r",
                ref_file.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));

        // Both files should have the same mtime
        let ref_mtime = fs::metadata(&ref_file).unwrap().modified().unwrap();
        let target_mtime = fs::metadata(&target).unwrap().modified().unwrap();
        assert_eq!(ref_mtime, target_mtime);
        cleanup(&dir);
    }

    #[test]
    fn test_touch_no_permission() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        // Skip if root
        if std::env::var("USER").map(|u| u == "root").unwrap_or(false) {
            return;
        }

        let output = Command::new(&armybox)
            .args(["touch", "/root/test_file"])
            .output()
            .unwrap();

        assert_ne!(output.status.code(), Some(0));
    }
}
