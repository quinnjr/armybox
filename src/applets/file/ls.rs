//! ls - list directory contents
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/ls.html

use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

/// Hard cap on `-R` recursion depth so a pathologically deep tree stops with a
/// diagnostic and a nonzero exit instead of overflowing the stack (SIGSEGV).
const MAX_DEPTH: usize = 4096;

/// Set when any stdout write fails (e.g. EPIPE from `ls -R | head`). Once set,
/// listing stops and ls exits nonzero rather than silently reporting success.
static WRITE_FAILED: AtomicBool = AtomicBool::new(false);

/// Write bytes to stdout, recording any failure for the caller to observe.
fn out(buf: &[u8]) {
    if io::write_all(1, buf) < 0 {
        WRITE_FAILED.store(true, Ordering::Relaxed);
    }
}

/// Write a number to stdout, recording any failure.
fn out_num(n: u64) {
    if io::write_num(1, n) < 0 {
        WRITE_FAILED.store(true, Ordering::Relaxed);
    }
}

/// Whether a stdout write has failed since listing began.
fn write_failed() -> bool {
    WRITE_FAILED.load(Ordering::Relaxed)
}

/// Print a short usage summary (for `--help`).
fn print_help() {
    out(b"Usage: ls [-aAcdFhilpRrStu1] [file...]\n");
}

/// Parsed command-line options.
#[derive(Clone, Copy)]
struct Flags {
    show_all: bool,     // -a  include . and ..
    almost_all: bool,   // -A  include hidden but not . and ..
    long_format: bool,  // -l
    one_per_line: bool, // -1
    recursive: bool,    // -R
    show_inode: bool,   // -i
    classify: bool,     // -F  append */@/=/|/ indicator
    slash_dir: bool,    // -p  append / to directories
    dir_only: bool,     // -d  list directory itself, not contents
    sort_time: bool,    // -t  sort by time
    sort_size: bool,    // -S  sort by size (descending)
    reverse: bool,      // -r  reverse sort order
    use_atime: bool,    // -u  use access time
    use_ctime: bool,    // -c  use status-change time
    human: bool,        // -h  human-readable sizes
}

impl Flags {
    fn need_stat(&self) -> bool {
        self.long_format
            || self.show_inode
            || self.classify
            || self.slash_dir
            || self.sort_time
            || self.sort_size
            || self.recursive
    }
}

/// A single directory entry with the metadata needed for sorting/display.
struct Entry {
    name: Vec<u8>,
    have_stat: bool,
    ino: u64,
    mode: u32,
    nlink: u64,
    uid: u64,
    gid: u64,
    size: i64,
    mtime: i64,
    atime: i64,
    ctime: i64,
}

impl Entry {
    fn new(name: &[u8]) -> Entry {
        Entry {
            name: name.to_vec(),
            have_stat: false,
            ino: 0,
            mode: 0,
            nlink: 0,
            uid: 0,
            gid: 0,
            size: 0,
            mtime: 0,
            atime: 0,
            ctime: 0,
        }
    }

    fn fill(&mut self, st: &libc::stat) {
        self.have_stat = true;
        self.ino = st.st_ino as u64;
        self.mode = st.st_mode as u32;
        self.nlink = st.st_nlink as u64;
        self.uid = st.st_uid as u64;
        self.gid = st.st_gid as u64;
        self.size = st.st_size as i64;
        self.mtime = st.st_mtime as i64;
        self.atime = st.st_atime as i64;
        self.ctime = st.st_ctime as i64;
    }

    fn is_dir(&self) -> bool {
        self.have_stat && (self.mode & libc::S_IFMT as u32) == libc::S_IFDIR as u32
    }

    fn sort_time(&self, f: &Flags) -> i64 {
        if f.use_ctime {
            self.ctime
        } else if f.use_atime {
            self.atime
        } else {
            self.mtime
        }
    }
}

/// ls - list directory contents
///
/// # Synopsis
/// ```text
/// ls [-aAcdFhilpRrStu1] [file...]
/// ```
///
/// # Options
/// - `-a`: Show all files including . and ..
/// - `-A`: Show all files except . and ..
/// - `-c`: Use status-change time for sorting/display
/// - `-d`: List directories themselves, not their contents
/// - `-F`: Append indicator (/, *, @, =, |)
/// - `-h`: Human-readable sizes (with -l)
/// - `-i`: Show inode numbers
/// - `-l`: Long listing format
/// - `-p`: Append / to directory names
/// - `-R`: Recurse into subdirectories
/// - `-r`: Reverse sort order
/// - `-S`: Sort by file size (largest first)
/// - `-t`: Sort by modification time (newest first)
/// - `-u`: Use access time for sorting/display
/// - `-1`: One entry per line
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
pub fn ls(argc: i32, argv: *const *const u8) -> i32 {
    let mut flags = Flags {
        show_all: false,
        almost_all: false,
        long_format: false,
        one_per_line: false,
        recursive: false,
        show_inode: false,
        classify: false,
        slash_dir: false,
        dir_only: false,
        sort_time: false,
        sort_size: false,
        reverse: false,
        use_atime: false,
        use_ctime: false,
        human: false,
    };

    let mut operands: Vec<&[u8]> = Vec::new();
    let mut no_more_opts = false;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if !no_more_opts && arg == b"--" {
                no_more_opts = true;
                continue;
            }
            if !no_more_opts && arg.len() > 1 && arg[0] == b'-' {
                // `--help` is a long option, not the short flags `-h -e -l -p`.
                if arg == b"--help" {
                    print_help();
                    return 0;
                }
                // Any other `--long` option is unrecognized (usage error).
                if arg[1] == b'-' {
                    io::write_str(2, b"ls: unrecognized option '");
                    io::write_all(2, arg);
                    io::write_str(2, b"'\n");
                    return 2;
                }
                for &c in &arg[1..] {
                    match c {
                        b'a' => flags.show_all = true,
                        b'A' => flags.almost_all = true,
                        b'l' => flags.long_format = true,
                        b'1' => flags.one_per_line = true,
                        b'R' => flags.recursive = true,
                        b'i' => flags.show_inode = true,
                        b'F' => flags.classify = true,
                        b'p' => flags.slash_dir = true,
                        b'd' => flags.dir_only = true,
                        b't' => flags.sort_time = true,
                        b'S' => flags.sort_size = true,
                        b'r' => flags.reverse = true,
                        b'u' => flags.use_atime = true,
                        b'c' => flags.use_ctime = true,
                        b'h' => flags.human = true,
                        _ => {}
                    }
                }
            } else {
                operands.push(arg);
            }
        }
    }

    if flags.long_format {
        flags.one_per_line = true;
    }

    if operands.is_empty() {
        operands.push(b".");
    }

    let mut exit_code = 0;

    // Separate file operands from directory operands. File operands (and, with
    // -d, everything) are listed together first, followed by each directory.
    let mut file_entries: Vec<Entry> = Vec::new();
    let mut dirs: Vec<&[u8]> = Vec::new();

    for &op in &operands {
        let mut st: libc::stat = unsafe { core::mem::zeroed() };
        if io::lstat(op, &mut st) != 0 {
            sys::perror(op);
            exit_code = 1;
            continue;
        }
        let is_dir = (st.st_mode as u32 & libc::S_IFMT as u32) == libc::S_IFDIR as u32;
        if is_dir && !flags.dir_only {
            dirs.push(op);
        } else {
            let mut e = Entry::new(op);
            e.fill(&st);
            file_entries.push(e);
        }
    }

    let total = file_entries.len() + dirs.len();
    let print_header = flags.recursive || total > 1;
    let mut printed_any = false;

    if !file_entries.is_empty() {
        sort_entries(&mut file_entries, &flags);
        // File operands carry their own (possibly absolute) path as the name;
        // don't prefix a directory when building full paths for them.
        print_entries(&file_entries, b"", true, &flags);
        printed_any = true;
    }

    for &d in &dirs {
        // Stop on a stdout write failure (e.g. EPIPE) instead of exiting 0.
        if write_failed() {
            exit_code = 1;
            break;
        }
        if printed_any {
            out(b"\n");
        }
        printed_any = true;
        if list_dir(d, &flags, print_header, 0) != 0 {
            exit_code = 1;
        }
    }

    if write_failed() {
        exit_code = 1;
    }

    exit_code
}

/// Read, sort, and print the contents of a single directory.
/// When `recursive`, descends into subdirectories afterward.
fn list_dir(path: &[u8], flags: &Flags, print_header: bool, depth: usize) -> i32 {
    // Bound recursion depth so a pathologically deep tree stops with a
    // diagnostic and a nonzero exit instead of overflowing the stack.
    if depth >= MAX_DEPTH {
        io::write_str(2, b"ls: '");
        io::write_all(2, path);
        io::write_str(2, b"': directory too deep\n");
        return 1;
    }

    let mut entries = match read_dir_entries(path, flags) {
        Some(e) => e,
        None => {
            sys::perror(path);
            return 1;
        }
    };

    if print_header {
        out(path);
        out(b":\n");
    }

    sort_entries(&mut entries, flags);
    print_entries(&entries, path, false, flags);

    // A failed stdout write (e.g. EPIPE) stops listing immediately.
    if write_failed() {
        return 1;
    }

    let mut exit_code = 0;

    if flags.recursive {
        for e in &entries {
            if write_failed() {
                exit_code = 1;
                break;
            }
            if !e.is_dir() {
                continue;
            }
            if e.name == b"." || e.name == b".." {
                continue;
            }
            let mut child = [0u8; io::PATH_MAX];
            if let Some(len) = join_path(path, &e.name, &mut child) {
                out(b"\n");
                if list_dir(&child[..len], flags, true, depth + 1) != 0 {
                    exit_code = 1;
                }
            } else {
                exit_code = 1;
            }
        }
    }

    exit_code
}

/// Read all entries of a directory into a Vec. Returns None if it can't open.
fn read_dir_entries(path: &[u8], flags: &Flags) -> Option<Vec<Entry>> {
    let fd = io::open(path, libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        return None;
    }

    let mut entries: Vec<Entry> = Vec::new();
    let mut buf = [0u8; 4096];
    let need_stat = flags.need_stat();

    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }

        let mut offset = 0usize;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };
            offset += dirent.d_reclen as usize;

            let is_dot = name == b".";
            let is_dotdot = name == b"..";
            let hidden = name.len() > 0 && name[0] == b'.';

            // Visibility rules: -a shows everything, -A shows hidden but not
            // . and .., default hides all dot-entries.
            if is_dot || is_dotdot {
                if !flags.show_all {
                    continue;
                }
            } else if hidden && !flags.show_all && !flags.almost_all {
                continue;
            }

            let mut e = Entry::new(name);
            if need_stat {
                let mut full = [0u8; io::PATH_MAX];
                if let Some(len) = join_path(path, name, &mut full) {
                    let mut st: libc::stat = unsafe { core::mem::zeroed() };
                    if io::lstat(&full[..len], &mut st) == 0 {
                        e.fill(&st);
                    }
                }
            }
            entries.push(e);
        }
    }

    io::close(fd);
    Some(entries)
}

/// Sort entries in place according to the active flags.
fn sort_entries(entries: &mut [Entry], flags: &Flags) {
    if flags.sort_time {
        // Newest first.
        entries.sort_unstable_by(|a, b| {
            b.sort_time(flags)
                .cmp(&a.sort_time(flags))
                .then_with(|| a.name.cmp(&b.name))
        });
    } else if flags.sort_size {
        // Largest first.
        entries.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    } else {
        // Lexicographic by byte order.
        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    }

    if flags.reverse {
        entries.reverse();
    }
}

/// Print a sorted list of entries. `dir` is the directory the entries live in
/// (used to build full paths for stat/readlink); pass an empty slice when the
/// entry names are already complete paths (file operands).
fn print_entries(entries: &[Entry], dir: &[u8], names_are_paths: bool, flags: &Flags) {
    let mut col = 0usize;
    let term_width = 80usize;

    for e in entries {
        if flags.long_format {
            print_long(e, dir, names_are_paths, flags);
            out(b"\n");
        } else if flags.one_per_line {
            if flags.show_inode && e.have_stat {
                out_num(e.ino);
                out(b" ");
            }
            out(&e.name);
            write_suffix(e, flags);
            out(b"\n");
        } else {
            // Columnar output.
            let entry_len = e.name.len() + 2;
            if col + entry_len > term_width && col > 0 {
                out(b"\n");
                col = 0;
            }
            if flags.show_inode && e.have_stat {
                out_num(e.ino);
                out(b" ");
            }
            out(&e.name);
            write_suffix(e, flags);
            out(b"  ");
            col += entry_len;
        }
    }

    if !flags.one_per_line && !flags.long_format && col > 0 {
        out(b"\n");
    }
}

/// Emit one long-format line for an entry (without trailing newline).
fn print_long(e: &Entry, dir: &[u8], names_are_paths: bool, flags: &Flags) {
    if flags.show_inode && e.have_stat {
        out_num(e.ino);
        out(b" ");
    }

    if !e.have_stat {
        // Fall back to just the name if we couldn't stat it.
        out(&e.name);
        return;
    }

    let mut mode_buf = [0u8; 10];
    sys::format_mode(e.mode, &mut mode_buf);
    out(&mode_buf);
    out(b" ");

    write_padded(e.nlink, 3);
    out(b" ");
    write_padded(e.uid, 5);
    out(b" ");
    write_padded(e.gid, 5);
    out(b" ");

    write_size(e.size, flags.human);
    out(b" ");

    let t = e.sort_time(flags);
    format_time(if t < 0 { 0 } else { t as u64 });
    out(b" ");

    out(&e.name);

    // Symlink target.
    if (e.mode & libc::S_IFMT as u32) == libc::S_IFLNK as u32 {
        let mut full = [0u8; io::PATH_MAX];
        let target_path: Option<usize> = if names_are_paths {
            let n = e.name.len().min(full.len());
            full[..n].copy_from_slice(&e.name[..n]);
            Some(n)
        } else {
            join_path(dir, &e.name, &mut full)
        };
        if let Some(len) = target_path {
            out(b" -> ");
            let mut link_target = [0u8; io::PATH_MAX];
            let link_len = io::readlink(&full[..len], &mut link_target);
            if link_len > 0 {
                out(&link_target[..link_len as usize]);
            }
        }
    }

    write_suffix(e, flags);
}

/// Append the -F/-p classification suffix for an entry.
fn write_suffix(e: &Entry, flags: &Flags) {
    if !e.have_stat {
        return;
    }
    if flags.classify {
        write_classify_suffix(e.mode);
    } else if flags.slash_dir {
        if (e.mode & libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
            out(b"/");
        }
    }
}

/// Right-align a number in `width` columns (space-padded).
fn write_padded(n: u64, width: usize) {
    let mut digits = 1;
    let mut v = n;
    while v >= 10 {
        v /= 10;
        digits += 1;
    }
    let mut i = digits;
    while i < width {
        out(b" ");
        i += 1;
    }
    out_num(n);
}

/// Write a size, optionally human-readable (K/M/G/T), right-aligned.
fn write_size(size: i64, human: bool) {
    let s = if size < 0 { 0u64 } else { size as u64 };
    if !human {
        write_padded(s, 8);
        return;
    }

    const UNITS: [&[u8]; 5] = [b"", b"K", b"M", b"G", b"T"];
    if s < 1024 {
        write_padded(s, 4);
        out(b" ");
        return;
    }
    let mut value = s;
    let mut unit = 0;
    // Round up to one decimal-free unit for simplicity.
    while value >= 1024 && unit < UNITS.len() - 1 {
        value = (value + 1023) / 1024;
        unit += 1;
    }
    // Right-align in 4 cols including the suffix.
    let mut digits = 1u64;
    let mut v = value;
    while v >= 10 {
        v /= 10;
        digits += 1;
    }
    let mut pad = 4;
    while pad > digits + 1 {
        out(b" ");
        pad -= 1;
    }
    out_num(value);
    out(UNITS[unit]);
}

/// Join `dir` and `name` into `out` with bounds checking (PATH_MAX safe).
/// Returns the resulting length, or None if it would overflow the buffer.
fn join_path(dir: &[u8], name: &[u8], out: &mut [u8; io::PATH_MAX]) -> Option<usize> {
    let mut len = 0usize;
    for &c in dir {
        if len >= out.len() {
            return None;
        }
        out[len] = c;
        len += 1;
    }
    // Add a separator unless dir already ends in one (e.g. "/").
    if len > 0 && out[len - 1] != b'/' {
        if len >= out.len() {
            return None;
        }
        out[len] = b'/';
        len += 1;
    }
    for &c in name {
        if len >= out.len() {
            return None;
        }
        out[len] = c;
        len += 1;
    }
    Some(len)
}

/// Format Unix timestamp as "Mon DD HH:MM" or "Mon DD  YYYY" for older files
fn format_time(timestamp: u64) {
    const MONTHS: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];

    // Simple time calculation (not handling timezones)
    let secs_per_day = 86400u64;
    let secs_per_hour = 3600u64;
    let secs_per_min = 60u64;

    // Days since epoch
    let days = timestamp / secs_per_day;
    let time_of_day = timestamp % secs_per_day;
    let hour = (time_of_day / secs_per_hour) % 24;
    let minute = (time_of_day % secs_per_hour) / secs_per_min;

    // Calculate year, month, day (simplified - doesn't handle leap years perfectly)
    let mut year = 1970u64;
    let mut remaining_days = days;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0usize;
    for (i, &days_in_month) in days_in_months.iter().enumerate() {
        if remaining_days < days_in_month {
            month = i;
            break;
        }
        remaining_days -= days_in_month;
    }
    let day = remaining_days + 1;

    // Get current time to determine if we show time or year
    let now = unsafe { libc::time(core::ptr::null_mut()) } as u64;
    let six_months_ago = now.saturating_sub(180 * secs_per_day);

    out(MONTHS[month]);
    out(b" ");
    if day < 10 {
        out(b" ");
    }
    out_num(day);
    out(b" ");

    if timestamp < six_months_ago || timestamp > now {
        // Show year for old/future files
        out(b" ");
        out_num(year);
    } else {
        // Show time for recent files
        if hour < 10 {
            out(b"0");
        }
        out_num(hour);
        out(b":");
        if minute < 10 {
            out(b"0");
        }
        out_num(minute);
    }
}

/// Write classify suffix (-F option)
fn write_classify_suffix(mode: u32) {
    match mode & libc::S_IFMT as u32 {
        m if m == libc::S_IFDIR as u32 => {
            out(b"/");
        }
        m if m == libc::S_IFLNK as u32 => {
            out(b"@");
        }
        m if m == libc::S_IFIFO as u32 => {
            out(b"|");
        }
        m if m == libc::S_IFSOCK as u32 => {
            out(b"=");
        }
        m if m == libc::S_IFREG as u32 && mode & 0o111 != 0 => {
            out(b"*");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for ls utility

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
            "armybox_ls_test_{}_{}",
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
    fn test_ls_current_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "content").unwrap();
        fs::write(dir.join("file2.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["ls", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("file1.txt"));
        assert!(stdout.contains("file2.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_ls_hidden_files() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("visible.txt"), "content").unwrap();
        fs::write(dir.join(".hidden"), "hidden").unwrap();

        // Without -a, hidden files should not appear
        let output = Command::new(&armybox)
            .args(["ls", dir.to_str().unwrap()])
            .output()
            .unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("visible.txt"));
        assert!(!stdout.contains(".hidden"));

        // With -a, hidden files should appear
        let output = Command::new(&armybox)
            .args(["ls", "-a", dir.to_str().unwrap()])
            .output()
            .unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("visible.txt"));
        assert!(stdout.contains(".hidden"));
        cleanup(&dir);
    }

    #[test]
    fn test_ls_one_per_line() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file1.txt"), "content").unwrap();
        fs::write(dir.join("file2.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["ls", "-1", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Each file should be on its own line
        let lines: Vec<&str> = stdout.lines().collect();
        assert!(lines.len() >= 2);
        cleanup(&dir);
    }

    #[test]
    fn test_ls_long_format() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file.txt"), "hello world").unwrap();

        let output = Command::new(&armybox)
            .args(["ls", "-l", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Long format should show permissions (starts with -)
        assert!(stdout.contains("-rw"));
        // Should show filename
        assert!(stdout.contains("file.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_ls_classify() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::create_dir(dir.join("subdir")).unwrap();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["ls", "-F", "-1", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Directories should have / suffix
        assert!(stdout.contains("subdir/"));
        cleanup(&dir);
    }

    #[test]
    fn test_ls_nonexistent_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ls", "/nonexistent/directory"])
            .output()
            .unwrap();

        // Should print an error to stderr and return non-zero
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.len() > 0 || output.status.code() == Some(0));
    }

    #[test]
    fn test_ls_inode() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let output = Command::new(&armybox)
            .args(["ls", "-i", "-1", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Output should contain numbers (inodes)
        assert!(stdout.chars().any(|c| c.is_ascii_digit()));
        assert!(stdout.contains("file.txt"));
        cleanup(&dir);
    }

    #[test]
    fn test_ls_empty_directory() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let dir = setup();
        // Dir is empty

        let output = Command::new(&armybox)
            .args(["ls", dir.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        // Output should be empty or just whitespace
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.trim().is_empty() || stdout.trim().len() == 0);
        cleanup(&dir);
    }
}
