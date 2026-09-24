//! ps - report process status
//!
//! Display information about running processes.

use crate::applets::get_arg;
use crate::io;
use crate::sys;

const USAGE: &[u8] = b"usage: ps [-Aade] [-fl] [-o format] [-p pidlist] [-u user]\n";

/// Field metadata: keyword, header, default width, right-aligned. Shared by the
/// fixed formats, by -o, and by the value renderer.
const KEYWORDS: &[(&[u8], &[u8], usize, bool)] = &[
    (b"addr", b"ADDR", 4, true),
    (b"args", b"COMMAND", 0, false),
    (b"c", b"C", 1, true),
    (b"comm", b"CMD", 0, false),
    (b"etime", b"ELAPSED", 8, true),
    (b"f", b"F", 8, true),
    (b"gid", b"GID", 5, true),
    (b"group", b"GROUP", 8, false),
    (b"nice", b"NI", 3, true),
    (b"pcpu", b"%CPU", 5, true),
    (b"pgid", b"PGID", 5, true),
    (b"pid", b"PID", 5, true),
    (b"ppid", b"PPID", 5, true),
    (b"pri", b"PRI", 3, true),
    (b"rgroup", b"RGROUP", 8, false),
    (b"rss", b"RSS", 5, true),
    (b"ruser", b"RUSER", 8, false),
    (b"state", b"S", 1, false),
    (b"stime", b"STIME", 5, true),
    (b"sz", b"SZ", 6, true),
    (b"thcount", b"NLWP", 5, true),
    (b"time", b"TIME", 8, true),
    (b"tty", b"TTY", 8, false),
    (b"uid", b"UID", 5, true),
    (b"user", b"USER", 8, false),
    (b"vsz", b"VSZ", 6, true),
    (b"wchan", b"WCHAN", 6, false),
];

/// POSIX default: `PID TTY TIME CMD`.
const FMT_DEFAULT: &[&[u8]] = &[b"pid", b"tty", b"time", b"comm"];
/// `-f`: `USER PID PPID C STIME TTY TIME CMD`.
const FMT_FULL: &[&[u8]] = &[
    b"user", b"pid", b"ppid", b"c", b"stime", b"tty", b"time", b"comm",
];
/// `-l`: `F S UID PID PPID C PRI NI ADDR SZ WCHAN TTY TIME CMD`.
const FMT_LONG: &[&[u8]] = &[
    b"f", b"state", b"uid", b"pid", b"ppid", b"c", b"pri", b"nice", b"addr", b"sz", b"wchan",
    b"tty", b"time", b"comm",
];

/// ps - report process status
///
/// # Synopsis
/// ```text
/// ps [-Aade] [-fl] [-o format] [-p pidlist] [-u user]
/// ```
///
/// # Description
/// Report process status from /proc. The default output is the POSIX
/// `PID TTY TIME CMD` table, restricted to processes sharing the invoker's
/// effective user ID and controlling terminal; `-A`/`-a`/`-d`/`-e` select all
/// processes while `-p`, `-u` and `-o` select processes and fields.
///
/// # Exit Status
/// - 0: Success
/// - 1: An error occurred
/// - 2: Incorrect usage
pub fn ps(argc: i32, argv: *const *const u8) -> i32 {
    let mut all = false;
    let mut full = false;
    let mut long = false;
    let mut pids = [0u64; 64];
    let mut n_pids = 0;
    let mut uid_filter: Option<u32> = None;
    let mut fmt_buf = [0u8; 256];
    let mut fmt_len = 0;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if arg == b"--help" {
            io::write_str(1, USAGE);
            return 0;
        }
        if arg.len() < 2 || arg[0] != b'-' {
            return usage_err(arg);
        }
        let mut j = 1;
        while j < arg.len() {
            let opt = arg[j];
            let mut obuf = [0u8; 256];
            // -o/-p/-u take an operand: the rest of this argument, else the
            // next one (POSIX allows both "-o format" and "-oformat").
            if matches!(opt, b'o' | b'p' | b'u') {
                let olen = if j + 1 < arg.len() {
                    let rest = &arg[j + 1..];
                    if rest.len() > obuf.len() {
                        return usage_err(arg);
                    }
                    obuf[..rest.len()].copy_from_slice(rest);
                    rest.len()
                } else {
                    i += 1;
                    match unsafe { get_arg(argv, i) } {
                        Some(next) if next.len() <= obuf.len() => {
                            obuf[..next.len()].copy_from_slice(next);
                            next.len()
                        }
                        _ => return usage_err(arg),
                    }
                };
                let val = &obuf[..olen];
                match opt {
                    b'o' => {
                        fmt_buf[..olen].copy_from_slice(val);
                        fmt_len = olen;
                    }
                    b'p' => {
                        if !parse_pidlist(val, &mut pids, &mut n_pids) {
                            return usage_err(arg);
                        }
                    }
                    _ => match user_uid(val) {
                        Some(uid) => uid_filter = Some(uid),
                        None => {
                            io::write_str(2, b"ps: unknown user: ");
                            io::write_all(2, val);
                            io::write_str(2, b"\n");
                            return 1;
                        }
                    },
                }
                j = arg.len();
                continue;
            }
            match opt {
                b'A' | b'a' | b'd' | b'e' => all = true,
                b'f' => full = true,
                b'l' => long = true,
                _ => return usage_err(arg),
            }
            j += 1;
        }
        i += 1;
    }

    // Resolve -o keywords up front so a typo cannot print a half-formed table.
    let mut sel: [usize; 32] = [0; 32];
    let mut n_sel = 0;
    if fmt_len > 0 {
        for key in fmt_buf[..fmt_len].split(|&c| c == b',') {
            match keyword(key) {
                Some(idx) if n_sel < sel.len() => {
                    sel[n_sel] = idx;
                    n_sel += 1;
                }
                _ => {
                    io::write_str(2, b"ps: unknown format keyword: ");
                    io::write_all(2, key);
                    io::write_str(2, b"\n");
                    io::write_str(2, USAGE);
                    return 2;
                }
            }
        }
        if n_sel == 0 {
            return usage_err(b"-o");
        }
    }

    // Clock ticks per second, for the TIME column.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let hz = if ticks > 0 { ticks as u64 } else { 100 };

    let ctx = Ctx {
        all,
        my_uid: io::geteuid(),
        my_tty: self_tty(),
        hz,
        boot: btime(),
        now: unsafe { libc::time(core::ptr::null_mut()) } as u64,
        pids,
        n_pids,
        uid_filter,
    };

    // Columns to print: -o keywords, or the selected fixed format.
    let fixed: &[&[u8]] = if long {
        FMT_LONG
    } else if full {
        FMT_FULL
    } else {
        FMT_DEFAULT
    };
    let mut cells = [0usize; 32];
    let mut n_cells = 0;
    if n_sel > 0 {
        cells[..n_sel].copy_from_slice(&sel[..n_sel]);
        n_cells = n_sel;
    } else {
        for key in fixed {
            cells[n_cells] = keyword(key).unwrap_or(0);
            n_cells += 1;
        }
    }

    for k in 0..n_cells {
        let kw = KEYWORDS[cells[k]];
        write_field(1, kw.1, kw.2, kw.3);
        if k + 1 < n_cells {
            io::write_str(1, b" ");
        }
    }
    io::write_str(1, b"\n");

    let fd = io::open(b"/proc", libc::O_RDONLY | libc::O_DIRECTORY, 0);
    if fd < 0 {
        return 1;
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            break;
        }
        let mut offset = 0;
        while offset < n as usize {
            let dirent = unsafe { &*(buf.as_ptr().add(offset) as *const libc::dirent64) };
            let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };
            offset += dirent.d_reclen as usize;
            if name.is_empty() || name[0] < b'0' || name[0] > b'9' {
                continue;
            }
            let p = match Proc::load(name) {
                Some(p) => p,
                None => continue,
            };
            if !ctx.selected(&p) {
                continue;
            }
            let mut value = [0u8; 512];
            for k in 0..n_cells {
                let kw = KEYWORDS[cells[k]];
                let v = render(cells[k], &p, &ctx, &mut value);
                write_field(1, v, kw.2, kw.3);
                if k + 1 < n_cells {
                    io::write_str(1, b" ");
                }
            }
            io::write_str(1, b"\n");
        }
    }
    io::close(fd);
    0
}

struct Ctx {
    all: bool,
    my_uid: u32,
    my_tty: u64,
    hz: u64,
    boot: u64,
    now: u64,
    pids: [u64; 64],
    n_pids: usize,
    uid_filter: Option<u32>,
}

impl Ctx {
    fn selected(&self, p: &Proc) -> bool {
        if self.n_pids > 0 {
            return self.pids[..self.n_pids].contains(&p.pid);
        }
        if let Some(uid) = self.uid_filter {
            return p.uid == uid;
        }
        if self.all {
            return true;
        }
        // POSIX default: same effective user ID and controlling terminal.
        p.uid == self.my_uid && p.tty == self.my_tty
    }
}

fn keyword(key: &[u8]) -> Option<usize> {
    KEYWORDS.iter().position(|k| k.0 == key)
}

fn parse_pidlist(list: &[u8], out: &mut [u64; 64], n: &mut usize) -> bool {
    let mut any = false;
    for item in list.split(|&c| c == b',' || c == b' ') {
        if item.is_empty() {
            continue;
        }
        match sys::parse_u64(item) {
            Some(pid) if *n < out.len() => {
                out[*n] = pid;
                *n += 1;
                any = true;
            }
            _ => return false,
        }
    }
    any
}

fn user_uid(user: &[u8]) -> Option<u32> {
    let mut buf = [0u8; 64];
    if user.len() < buf.len() {
        buf[..user.len()].copy_from_slice(user);
        let pwd = unsafe { libc::getpwnam(buf.as_ptr() as *const libc::c_char) };
        if !pwd.is_null() {
            return Some(unsafe { (*pwd).pw_uid });
        }
    }
    sys::parse_u64(user).map(|u| u as u32)
}

fn usage_err(arg: &[u8]) -> i32 {
    io::write_str(2, b"ps: invalid option: ");
    io::write_all(2, arg);
    io::write_str(2, b"\n");
    io::write_str(2, USAGE);
    2
}

/// Write `s` padded to `width`, right-aligned when `right`, on fd 1.
fn write_field(fd: i32, s: &[u8], width: usize, right: bool) {
    const SPACES: [u8; 32] = [b' '; 32];
    let pad = width.saturating_sub(s.len());
    let mut spaces = pad;
    if right {
        while spaces > 0 {
            let chunk = spaces.min(SPACES.len());
            io::write_all(fd, &SPACES[..chunk]);
            spaces -= chunk;
        }
    }
    io::write_all(fd, s);
    if !right {
        while spaces > 0 {
            let chunk = spaces.min(SPACES.len());
            io::write_all(fd, &SPACES[..chunk]);
            spaces -= chunk;
        }
    }
}

/// Build `/proc/<pid><suffix>` (NUL terminated) into `path`, returning its length.
pub(super) fn proc_path(pid: &[u8], suffix: &[u8], path: &mut [u8; 64]) -> Option<usize> {
    let mut n = 0;
    for &c in b"/proc/".iter().chain(pid).chain(suffix) {
        if n + 1 >= path.len() {
            return None;
        }
        path[n] = c;
        n += 1;
    }
    path[n] = 0;
    Some(n + 1)
}

pub(super) fn read_file(path: &[u8], buf: &mut [u8]) -> isize {
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        return -1;
    }
    let n = io::read(fd, buf);
    io::close(fd);
    n
}

/// BusyBox's `COMM_LEN - 1`: the kernel task name is at most 15 characters.
const COMM_LEN: usize = 15;

/// Everything ps prints about one process, read from /proc.
pub(super) struct Proc {
    pub(super) pid: u64,
    pub(super) ppid: u64,
    pgrp: u64,
    pub(super) uid: u32,
    gid: u32,
    pub(super) state: u8,
    tty: u64,
    flags: u64,
    pub(super) utime: u64,
    pub(super) stime: u64,
    pub(super) nice: i64,
    prio: u64,
    threads: u64,
    start: u64,
    pub(super) vsize: u64,
    rss: u64,
    /// argv[0], or "[comm]" for a kernel thread.
    name: [u8; 64],
    name_len: usize,
    /// Full command line, arguments joined by spaces.
    args: [u8; 512],
    pub(super) args_len: usize,
    /// Kernel task name (comm), kept whole for top's "{comm} " annotation.
    /// Truncated to BusyBox's COMM_LEN-1, like procps does.
    comm: [u8; 32],
    comm_len: usize,
}

impl Proc {
    pub(super) fn load(pid_s: &[u8]) -> Option<Proc> {
        let mut path = [0u8; 64];
        let mut buf = [0u8; 1024];
        let len = proc_path(pid_s, b"/stat", &mut path)?;
        let n = read_file(&path[..len], &mut buf);
        if n <= 0 {
            return None;
        }
        let stat = &buf[..n as usize];

        // comm is parenthesised and may itself contain spaces or parentheses,
        // so split on the first '(' and the last ')'.
        let lp = stat.iter().position(|&c| c == b'(')?;
        let rp = stat.iter().rposition(|&c| c == b')')?;
        if rp <= lp {
            return None;
        }

        let mut p = Proc {
            pid: sys::parse_u64(pid_s).unwrap_or(0),
            ppid: 0,
            pgrp: 0,
            uid: u32::MAX,
            gid: u32::MAX,
            // State is the field after the "<comm>) " suffix, not the
            // first char of comm itself.
            state: stat.get(rp + 2).copied().unwrap_or(b'?'),
            tty: 0,
            flags: 0,
            utime: 0,
            stime: 0,
            nice: 0,
            prio: 0,
            threads: 0,
            start: 0,
            vsize: 0,
            rss: 0,
            name: [0; 64],
            name_len: 0,
            args: [0; 512],
            args_len: 0,
            comm: [0; 32],
            comm_len: 0,
        };
        let comm = &stat[lp + 1..rp];
        p.comm_len = comm.len().min(COMM_LEN);
        p.comm[..p.comm_len].copy_from_slice(&comm[..p.comm_len]);
        let comm = &p.comm[..p.comm_len];

        // Fields after the ')': f[1]=ppid, f[2]=pgrp, f[4]=tty_nr, f[6]=flags,
        // f[11]=utime, f[12]=stime, f[15]=priority, f[16]=nice,
        // f[17]=num_threads, f[19]=starttime, f[20]=vsize, f[21]=rss.
        let mut f = [0i64; 24];
        let mut idx = 0;
        for tok in stat[rp + 1..].split(|&c| c == b' ') {
            if tok.is_empty() {
                continue;
            }
            if idx >= f.len() {
                break;
            }
            // f[0] is the state letter; the rest are numeric (nice may be < 0).
            if idx > 0 {
                f[idx] = sys::parse_i64(tok).unwrap_or(0);
            }
            idx += 1;
        }
        p.ppid = f[1] as u64;
        p.pgrp = f[2] as u64;
        p.tty = f[4] as u64;
        p.flags = f[6] as u64;
        p.utime = f[11] as u64;
        p.stime = f[12] as u64;
        p.prio = f[15] as u64;
        p.nice = f[16];
        p.threads = f[17] as u64;
        p.start = f[19] as u64;
        p.vsize = f[20] as u64;
        p.rss = f[21] as u64;

        // The owner of /proc/<pid> carries the real UID/GID.
        let mut sb = io::stat_zeroed();
        if let Some(len) = proc_path(pid_s, b"", &mut path)
            && io::stat(&path[..len], &mut sb) == 0
        {
            p.uid = sb.st_uid;
            p.gid = sb.st_gid;
        }

        // argv[0] for CMD, full argument vector for the args keyword.
        let mut cmd = [0u8; 512];
        let mut argv0 = [0u8; 64];
        let mut argv0_len = 0;
        if let Some(len) = proc_path(pid_s, b"/cmdline", &mut path) {
            let cn = read_file(&path[..len], &mut cmd);
            if cn > 0 {
                let cn = (cn as usize).min(cmd.len());
                argv0_len = cmd[..cn].iter().position(|&c| c == 0).unwrap_or(cn).min(argv0.len());
                argv0[..argv0_len].copy_from_slice(&cmd[..argv0_len]);

                // Bytes past the last non-NUL one (a rewritten argv leaves
                // NUL padding) are not part of the command line, so they must
                // not become trailing spaces.
                let mut end = cn;
                while end > 0 && cmd[end - 1] == 0 {
                    end -= 1;
                }
                let mut w = 0;
                for &c in cmd[..end].iter() {
                    if w >= p.args.len() {
                        break;
                    }
                    p.args[w] = if c == 0 { b' ' } else { c };
                    w += 1;
                }
                p.args_len = w;
            }
        }

        if argv0_len > 0 {
            p.name[..argv0_len].copy_from_slice(&argv0[..argv0_len]);
            p.name_len = argv0_len;
        } else {
            // Kernel threads have no argv; print the comm in brackets.
            let cl = comm.len().min(p.name.len() - 2);
            p.name[0] = b'[';
            p.name[1..1 + cl].copy_from_slice(&comm[..cl]);
            p.name[1 + cl] = b']';
            p.name_len = 2 + cl;
        }
        // comm is still needed for nothing else; the name carries the fallback.
        Some(p)
    }

    pub(super) fn name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    pub(super) fn comm(&self) -> &[u8] {
        &self.comm[..self.comm_len]
    }

    /// Full command line, arguments joined by spaces.
    pub(super) fn args(&self) -> &[u8] {
        &self.args[..self.args_len]
    }

    fn started_at(&self, ctx: &Ctx) -> u64 {
        if ctx.boot == 0 {
            return ctx.now;
        }
        ctx.boot + self.start / ctx.hz
    }
}

/// Render one field of `p` into `out`, returning the written slice.
fn render<'a>(idx: usize, p: &Proc, ctx: &Ctx, out: &'a mut [u8; 512]) -> &'a [u8] {
    fn num(out: &mut [u8], n: u64) -> usize {
        let mut tmp = [0u8; 24];
        let d = sys::format_u64(n, &mut tmp);
        out[..d.len()].copy_from_slice(d);
        d.len()
    }
    fn snum(out: &mut [u8], n: i64) -> usize {
        let mut tmp = [0u8; 24];
        let d = sys::format_i64(n, &mut tmp);
        out[..d.len()].copy_from_slice(d);
        d.len()
    }
    fn text(out: &mut [u8], s: &[u8]) -> usize {
        let n = s.len().min(out.len());
        out[..n].copy_from_slice(&s[..n]);
        n
    }

    let len = match KEYWORDS[idx].0 {
        b"pid" => num(out, p.pid),
        b"ppid" => num(out, p.ppid),
        b"pgid" => num(out, p.pgrp),
        b"uid" => num(out, p.uid as u64),
        b"gid" => num(out, p.gid as u64),
        b"user" | b"ruser" => text(out, user_name(p.uid)),
        b"group" | b"rgroup" => text(out, group_name(p.gid)),
        b"comm" => text(out, p.name()),
        b"args" => {
            if p.args_len > 0 {
                text(out, &p.args[..p.args_len])
            } else {
                text(out, p.name())
            }
        }
        b"state" => {
            out[0] = p.state;
            1
        }
        b"f" => {
            let mut tmp = [0u8; 24];
            let d = sys::format_octal(p.flags as u32, &mut tmp);
            text(out, d)
        }
        // C is scheduler utilisation; without sampling it is always 0, which is
        // what BusyBox prints too.
        b"c" => {
            out[0] = b'0';
            1
        }
        // Modern /proc exposes no kernel address for a process.
        b"addr" => {
            out[0] = b'-';
            1
        }
        b"nice" => snum(out, p.nice),
        b"pri" => num(out, p.prio),
        b"thcount" => num(out, p.threads),
        b"vsz" | b"sz" => num(out, p.vsize / 1024),
        b"rss" => num(out, p.rss),
        b"time" => cpu_time(p.utime + p.stime, ctx.hz, out),
        b"etime" => cpu_time(ctx.now.saturating_sub(p.started_at(ctx)), 1, out),
        b"stime" => start_time(p.started_at(ctx), ctx.now, out),
        b"pcpu" => pcpu(p, ctx, out),
        b"tty" => {
            let mut tmp = [0u8; 24];
            let t = tty_name(p.tty, &mut tmp);
            text(out, t)
        }
        b"wchan" => wchan(p, out),
        _ => {
            out[0] = b'?';
            1
        }
    };
    &out[..len]
}

/// Average CPU use over the process lifetime, as a percentage with one decimal.
fn pcpu(p: &Proc, ctx: &Ctx, out: &mut [u8]) -> usize {
    let elapsed = ctx.now.saturating_sub(p.started_at(ctx));
    let cputime = (p.utime + p.stime) / ctx.hz;
    // Tenths of a percent, capped so the column cannot grow without bound.
    let tenths = if elapsed == 0 {
        0
    } else {
        (1000 * cputime / elapsed).min(9999)
    };
    let whole = sys::format_u64(tenths / 10, out);
    let n = whole.len();
    out[n] = b'.';
    out[n + 1] = b'0' + (tenths % 10) as u8;
    n + 2
}

fn wchan(p: &Proc, out: &mut [u8]) -> usize {
    let mut path = [0u8; 64];
    let mut pid_s = [0u8; 20];
    let pid = sys::format_u64(p.pid, &mut pid_s);
    let len = match proc_path(pid, b"/wchan", &mut path) {
        Some(l) => l,
        None => {
            out[0] = b'-';
            return 1;
        }
    };
    let n = read_file(&path[..len], out);
    if n <= 0 {
        out[0] = b'-';
        return 1;
    }
    let n = (n as usize).min(out.len());
    // /proc/<pid>/wchan is empty when the process is running.
    let n = match out[..n].iter().position(|&c| c == b'\n') {
        Some(0) => 0,
        Some(i) => i,
        None => n,
    };
    // Empty (running) or "0" (no symbol) means there is no wait channel.
    if n == 0 || (n == 1 && out[0] == b'0') {
        out[0] = b'-';
        return 1;
    }
    n
}

/// tty_nr of the calling process, for the POSIX default selection.
fn self_tty() -> u64 {
    let mut buf = [0u8; 1024];
    let n = read_file(b"/proc/self/stat\0", &mut buf);
    if n <= 0 {
        return 0;
    }
    let stat = &buf[..n as usize];
    let rp = match stat.iter().rposition(|&c| c == b')') {
        Some(i) => i,
        None => return 0,
    };
    // f[4] is tty_nr.
    let mut f = [0u64; 5];
    let mut idx = 0;
    for tok in stat[rp + 1..].split(|&c| c == b' ') {
        if tok.is_empty() {
            continue;
        }
        if idx >= f.len() {
            break;
        }
        if idx > 0 {
            f[idx] = sys::parse_u64(tok).unwrap_or(0);
        }
        idx += 1;
    }
    f[4]
}

/// System boot time (epoch seconds) from /proc/stat.
fn btime() -> u64 {
    let mut buf = [0u8; 4096];
    let n = read_file(b"/proc/stat\0", &mut buf);
    if n <= 0 {
        return 0;
    }
    for line in buf[..n as usize].split(|&c| c == b'\n') {
        if let Some(rest) = line.strip_prefix(b"btime ") {
            return sys::parse_u64(rest).unwrap_or(0);
        }
    }
    0
}

/// Map a tty_nr dev_t to the short name ps prints (`pts/0`, `ttyS0`, ...).
fn tty_name(nr: u64, buf: &mut [u8]) -> &[u8] {
    if nr == 0 {
        return b"?";
    }
    // Linux dev_t: 12 bits major, 20 bits minor.
    let major = ((nr >> 8) & 0xfff) as u32;
    let minor = ((nr & 0xff) | ((nr >> 12) & 0xfff00)) as u32;

    let (prefix, num): (&[u8], u32) = match major {
        4 if minor < 64 => (b"tty", minor),
        4 => (b"ttyS", minor - 64),
        136..=143 => (b"pts/", minor),
        204 if minor >= 64 => (b"ttyAMA", minor - 64),
        229 => (b"hvc", minor),
        _ => return b"?",
    };

    let mut n = 0;
    for &c in prefix {
        if n + 2 >= buf.len() {
            return b"?";
        }
        buf[n] = c;
        n += 1;
    }
    let mut digits = [0u8; 20];
    let d = sys::format_u64(num as u64, &mut digits);
    for &c in d {
        if n >= buf.len() {
            return b"?";
        }
        buf[n] = c;
        n += 1;
    }
    &buf[..n]
}

fn pad2(out: &mut [u8], v: u64) -> usize {
    // format_u64() right-aligns inside its buffer, so format into a scratch
    // buffer and copy the digits to the front.
    let mut tmp = [0u8; 24];
    let d = sys::format_u64(v, &mut tmp);
    out[..d.len()].copy_from_slice(d);
    d.len()
}

/// TIME column: [[dd-]hh:]mm:ss. Returns the length written.
fn cpu_time(ticks: u64, hz: u64, out: &mut [u8]) -> usize {
    let secs = ticks / hz;
    let (dd, hh, mm, ss) = (
        secs / 86400,
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
    );
    let mut n = 0;
    if dd > 0 {
        n += pad2(&mut out[n..], dd);
        out[n] = b'-';
        n += 1;
    }
    if dd > 0 || hh > 0 {
        n += pad2(&mut out[n..], hh);
        out[n] = b':';
        n += 1;
    }
    n += pad2(&mut out[n..], mm);
    out[n] = b':';
    n += 1;
    n += pad2(&mut out[n..], ss);
    n
}

/// STIME column: HH:MM for processes started today, MonDD for older ones.
fn start_time(start: u64, now: u64, out: &mut [u8]) -> usize {
    const MONTHS: [&[u8]; 12] = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    let mut tm: libc::tm = unsafe { core::mem::zeroed() };
    let t = start as libc::time_t;
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        out[..5].copy_from_slice(b"?????");
        return 5;
    }
    let mon = tm.tm_mon.clamp(0, 11) as usize;
    if now.saturating_sub(start) > 86400 {
        out[..3].copy_from_slice(MONTHS[mon]);
        let mut num = [0u8; 20];
        let d = sys::format_u64(tm.tm_mday.max(1) as u64, &mut num);
        let d = &d[..d.len().min(out.len() - 3)];
        out[3..3 + d.len()].copy_from_slice(d);
        3 + d.len()
    } else {
        let n = pad2(out, tm.tm_hour.max(0) as u64);
        out[n] = b':';
        n + 1 + pad2(&mut out[n + 1..], tm.tm_min.max(0) as u64)
    }
}

pub(super) fn user_name(uid: u32) -> &'static [u8] {
    static UNKNOWN: [u8; 1] = [b'?'];
    let pwd = unsafe { libc::getpwuid(uid) };
    if !pwd.is_null() {
        let name = unsafe { io::cstr_to_slice((*pwd).pw_name as *const u8) };
        if !name.is_empty() {
            return name;
        }
    }
    &UNKNOWN
}

fn group_name(gid: u32) -> &'static [u8] {
    static UNKNOWN: [u8; 1] = [b'?'];
    let grp = unsafe { libc::getgrgid(gid) };
    if !grp.is_null() {
        let name = unsafe { io::cstr_to_slice((*grp).gr_name as *const u8) };
        if !name.is_empty() {
            return name;
        }
    }
    &UNKNOWN
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::process::Command;
    use std::path::PathBuf;

    fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() { return release; }
        manifest_dir.join("target/debug/armybox")
    }

    #[test]
    fn test_ps_runs() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["ps"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
    }

    #[test]
    fn test_ps_has_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["ps"])
            .output()
            .unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("PID"));
        assert!(stdout.contains("TTY"));
    }

    #[test]
    fn test_ps_lists_processes() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["ps", "-A"])
            .output()
            .unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should have at least the header plus one process
        let lines: Vec<&str> = stdout.split('\n').collect();
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_ps_invalid_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["ps", "-Z"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn test_ps_unknown_keyword() {
        let armybox = get_armybox_path();
        if !armybox.exists() { return; }

        let output = Command::new(&armybox)
            .args(["ps", "-o", "nosuchfield"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2));
    }
}
