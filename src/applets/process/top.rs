//! top - display Linux processes
//!
//! Interactive real-time process viewer, ported from BusyBox 1.36.1's
//! `procps/top.c` for the configuration embedded systems use: global CPU
//! percentages, per-process %CPU, no SMP per-CPU lines and no topmem screen.
//! The layout, the sort order and the refresh loop match BusyBox byte for byte.

use super::ps::{Proc, read_file, user_name};
use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// BusyBox's `LINE_BUF_SIZE` (512 - 64).
const LINE_BUF: usize = 448;
/// BusyBox prints this column header verbatim.
const HEADER: &[u8] = b"  PID  PPID USER     STAT   VSZ %VSZ %CPU COMMAND";
const DEFAULT_INTERVAL_MS: i32 = 5000;
/// `smart_ulltoa5()`'s scale suffixes, indexed by the 1024-exponent.
const SCALE: &[u8] = b" mgtpezy";

const USAGE: &[u8] = b"usage: top [-b] [-n COUNT] [-d SECONDS]\n";

/// top - display Linux processes
///
/// # Synopsis
/// ```text
/// top [-b] [-n COUNT] [-d SECONDS]
/// top --help | --version
/// ```
///
/// # Description
/// Show a view of process activity in real time: the status of all processes
/// is re-read from /proc every SECONDS (default 5) and a screenful is shown.
/// In interactive mode `n`/`m`/`p`/`t` sort by pid/mem/cpu/time, `r` reverses
/// the sort and `q` or ^C quits.
///
/// # Exit Status
/// - 0: Success (including quitting with `q`)
/// - 1: An error occurred or bad usage
pub fn top(argc: i32, argv: *const *const u8) -> i32 {
    let mut batch = false;
    let mut iterations: i64 = 0; // 0 = forever
    let mut interval_ms: i32 = DEFAULT_INTERVAL_MS;

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
        if arg == b"--version" {
            io::write_str(1, b"top from armybox\n");
            return 0;
        }
        if arg.len() < 2 || arg[0] != b'-' {
            return usage_err(arg);
        }
        let mut j = 1;
        while j < arg.len() {
            let opt = arg[j];
            if opt == b'd' || opt == b'n' {
                // The operand may be glued on ("-d1") or the next argument.
                let mut obuf = [0u8; 64];
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
                if opt == b'd' {
                    match parse_interval_ms(val) {
                        Some(ms) => interval_ms = ms,
                        None => return usage_err(arg),
                    }
                } else {
                    match sys::parse_u64(val) {
                        Some(n) => iterations = n.min(i64::MAX as u64) as i64,
                        None => return usage_err(arg),
                    }
                }
                j = arg.len();
                continue;
            }
            match opt {
                b'b' => batch = true,
                // Accepted and ignored, exactly like BusyBox ("-m" only does
                // something with the topmem screen, "-H" without thread scan).
                b'm' | b'H' => {}
                _ => return usage_err(arg),
            }
            j += 1;
        }
        i += 1;
    }

    let mut app = Top {
        entries: Vec::new(),
        hist: Vec::new(),
        hist_next: Vec::new(),
        cur: Jif::default(),
        prev: Jif::default(),
        total_pcpu: 0,
        inverted: false,
        sort_fns: [pcpu_sort, mem_sort, time_sort],
        eof: batch,
        vin: 0,
        veof: 0,
    };
    if !batch {
        app.set_raw_mode();
        // Eat initial input, if any.
        let _ = app.handle_input(0);
    }

    let mut status = 0;
    loop {
        let (col, lines) = if batch {
            (LINE_BUF - 2, i32::MAX)
        } else {
            match screen_size() {
                Some(size) => size,
                None => {
                    sleep_ms(interval_ms);
                    continue;
                }
            }
        };

        app.read_procs();
        if app.entries.is_empty() {
            // BusyBox reports this and then exits successfully.
            io::write_str(2, b"top: no process info in /proc\n");
            break;
        }
        if app.hist.is_empty() {
            // First pass only samples the CPU counters; the real deltas are
            // computed on the next pass, hence the short sleep.
            if !app.do_stats() {
                status = 1;
                break;
            }
            sleep_ms(100);
            continue;
        }
        if !app.do_stats() {
            status = 1;
            break;
        }
        app.sort();
        app.display(lines, col, batch);

        // BusyBox counts iterations down to zero and treats 0/negative as
        // "forever", so -n 0 keeps running.
        if iterations >= 0 {
            iterations -= 1;
            if iterations == 0 {
                break;
            }
        }
        if !app.handle_input(interval_ms) {
            break;
        }
    }

    io::write_str(1, b"\n");
    if !batch {
        app.restore_terminal();
    }
    status
}

fn usage_err(arg: &[u8]) -> i32 {
    io::write_str(2, b"top: invalid option: ");
    io::write_all(2, arg);
    io::write_str(2, b"\n");
    io::write_str(2, USAGE);
    1
}

/// BusyBox's `parse_duration_str()`: seconds with an optional fraction and an
/// `s`/`m`/`h`/`d` suffix, converted to milliseconds.
fn parse_interval_ms(s: &[u8]) -> Option<i32> {
    let mut i = 0;
    let mut ms: u64 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        ms = ms.saturating_mul(10).saturating_add((s[i] - b'0') as u64);
        i += 1;
    }
    ms = ms.saturating_mul(1000);
    if i < s.len() && s[i] == b'.' {
        i += 1;
        let mut scale = 100u64;
        while i < s.len() && s[i].is_ascii_digit() {
            if scale > 0 {
                ms = ms.saturating_add((s[i] - b'0') as u64 * scale);
                scale /= 10;
            }
            i += 1;
        }
    }
    let mult = match s.get(i) {
        None | Some(b's') => 1u64,
        Some(b'm') => 60,
        Some(b'h') => 3600,
        Some(b'd') => 86400,
        _ => return None,
    };
    Some(ms.saturating_mul(mult).min(i32::MAX as u64) as i32)
}

/// CPU time accounting from /proc/stat's aggregate "cpu" line.
#[derive(Clone, Copy, Default)]
struct Jif {
    usr: u64,
    nic: u64,
    sys: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    total: u64,
    busy: u64,
}

/// One process plus the frame-to-frame data top needs for it.
struct Entry {
    proc: Proc,
    /// utime + stime, used for %CPU deltas and the time sort.
    ticks: u64,
    /// VSZ in KB, like BusyBox (stat's vsize >> 10).
    vsz: u64,
    pcpu: u32,
    /// BusyBox's 3-char STAT field (state + W + nice flag).
    state: [u8; 3],
}

type CmpFn = fn(&Entry, &Entry) -> i32;

struct Top {
    entries: Vec<Entry>,
    /// (pid, ticks) of the previous frame, for the %CPU deltas.
    hist: Vec<(u64, u64)>,
    /// Scratch twin of `hist`, swapped in every frame so that steady-state
    /// refreshes allocate nothing.
    hist_next: Vec<(u64, u64)>,
    cur: Jif,
    prev: Jif,
    total_pcpu: u32,
    sort_fns: [CmpFn; 3],
    inverted: bool,
    eof: bool,
    vin: u8,
    veof: u8,
}

impl Top {
    /// Walk /proc and fill `entries`. Processes that exit mid-scan are skipped.
    fn read_procs(&mut self) {
        self.entries.clear();
        let fd = io::open(b"/proc", libc::O_RDONLY | libc::O_DIRECTORY, 0);
        if fd < 0 {
            return;
        }
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::syscall(libc::SYS_getdents64, fd, buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
            let mut off = 0;
            while off < n as usize {
                let dirent = unsafe { &*(buf.as_ptr().add(off) as *const libc::dirent64) };
                let name = unsafe { io::cstr_to_slice(dirent.d_name.as_ptr() as *const u8) };
                off += dirent.d_reclen as usize;
                if name.is_empty() || name[0] < b'0' || name[0] > b'9' {
                    continue;
                }
                if let Some(p) = Proc::load(name) {
                    let ticks = p.utime + p.stime;
                    let vsz = p.vsize >> 10;
                    let state = stat_field(&p, vsz);
                    self.entries.push(Entry {
                        ticks,
                        vsz,
                        state,
                        pcpu: 0,
                        proc: p,
                    });
                }
            }
        }
        io::close(fd);
    }

    /// Read /proc/stat, shifting the current sample into the previous one.
    /// Returns false when the file cannot be parsed.
    fn get_jiffy_counts(&mut self) -> bool {
        self.prev = self.cur;

        let mut buf = [0u8; 512];
        let n = read_file(b"/proc/stat\0", &mut buf);
        if n <= 0 {
            io::write_str(2, b"top: can't read /proc/stat\n");
            return false;
        }
        let n = n as usize;
        let end = buf[..n].iter().position(|&c| c == b'\n').unwrap_or(n);
        let line = &buf[..end];
        if !line.starts_with(b"cpu") {
            io::write_str(2, b"top: can't read /proc/stat\n");
            return false;
        }

        let mut v = [0u64; 8];
        let mut idx = 0;
        for tok in line.split(|&c| c == b' ') {
            if tok.is_empty() || !tok[0].is_ascii_digit() {
                continue; // the "cpu" label itself
            }
            if idx >= v.len() {
                break;
            }
            v[idx] = sys::parse_u64(tok).unwrap_or(0);
            idx += 1;
        }
        if idx < 4 {
            io::write_str(2, b"top: can't read /proc/stat\n");
            return false;
        }

        let j = Jif {
            usr: v[0],
            nic: v[1],
            sys: v[2],
            idle: v[3],
            iowait: v[4],
            irq: v[5],
            softirq: v[6],
            steal: v[7],
            total: v[0] + v[1] + v[2] + v[3] + v[4] + v[5] + v[6] + v[7],
            busy: 0,
        };
        // procps 2.x does not count iowait as busy time.
        let mut j = j;
        j.busy = j.total - j.idle - j.iowait;
        self.cur = j;
        true
    }

    /// Turn this frame's tick counts into per-process %CPU deltas.
    fn do_stats(&mut self) -> bool {
        if !self.get_jiffy_counts() {
            return false;
        }
        self.total_pcpu = 0;

        let mut next = core::mem::take(&mut self.hist_next);
        next.clear();
        next.reserve(self.entries.len());
        {
            let hist = &self.hist;
            for e in self.entries.iter_mut() {
                e.pcpu = 0;
                // Linear lookup, as BusyBox's rotating scan does.
                if let Some(&(_, prev_ticks)) = hist.iter().find(|&&(pid, _)| pid == e.proc.pid) {
                    e.pcpu = e.ticks.wrapping_sub(prev_ticks) as u32;
                    self.total_pcpu = self.total_pcpu.wrapping_add(e.pcpu);
                }
                next.push((e.proc.pid, e.ticks));
            }
        }
        // This frame's samples become `hist`; the old buffer is kept for the
        // next one.
        self.hist_next = core::mem::replace(&mut self.hist, next);
        true
    }

    /// BusyBox's `mult_lvl_cmp()`: each sort key in turn, optionally reversed.
    fn sort(&mut self) {
        let fns = self.sort_fns;
        let inverted = self.inverted;
        self.entries.sort_by(|a, b| {
            let mut v = 0i32;
            for f in fns.iter() {
                v = f(a, b);
                if v != 0 {
                    break;
                }
            }
            if inverted {
                v = v.wrapping_neg();
            }
            v.cmp(&0)
        });
    }

    /// Draw a whole frame. `lines`/`col` are the screen size (or the batch
    /// mode values: unlimited lines, BusyBox's maximum row width).
    fn display(&mut self, lines: i32, col: usize, batch: bool) {
        let mut lines_rem = lines;
        let total_memory = self.display_header(col, &mut lines_rem, batch);

        // BusyBox scales with integer multiply/shift to avoid divides; the
        // exact same arithmetic is kept so the percentages match exactly.
        let mem_div = if total_memory == 0 {
            1
        } else {
            total_memory.min(u32::MAX as u64) as u32
        };
        let mut pmem_shift: i32 = 32 - 11;
        let mut pmem_scale: u32 = 100u32 * (1u32 << (32 - 11)) / mem_div;
        while pmem_scale >= 512 {
            pmem_scale /= 4;
            pmem_shift -= 2;
        }
        let pmem_half: u32 = 1u32.wrapping_shl(pmem_shift as u32) / 2;

        let busy_jifs = self.cur.busy.wrapping_sub(self.prev.busy) as u32;
        if self.total_pcpu < busy_jifs {
            self.total_pcpu = busy_jifs;
        }
        let mut pcpu_shift: i32 = 6;
        let mut pcpu_scale: u32 = 100u32
            .wrapping_mul(64)
            .wrapping_mul(busy_jifs as u16 as u32);
        if pcpu_scale == 0 {
            pcpu_scale = 1;
        }
        while pcpu_scale < (1u32 << (32 - 2)) {
            pcpu_scale = pcpu_scale.wrapping_mul(4);
            pcpu_shift += 2;
        }
        let tmp = (self.cur.total.wrapping_sub(self.prev.total) as u16 as u32)
            .wrapping_mul(self.total_pcpu);
        if let Some(q) = pcpu_scale.checked_div(tmp) {
            pcpu_scale = q;
        }
        while pcpu_scale >= 1024 {
            pcpu_scale /= 4;
            pcpu_shift -= 2;
        }
        let pcpu_half: u32 = 1u32.wrapping_shl(pcpu_shift as u32) / 2;

        // Column header, in reverse video on a terminal.
        let mut out = Buf::new();
        if !batch {
            out.s(b"\x1b[7m");
        }
        out.s(&HEADER[..HEADER.len().min(col)]);
        if !batch {
            out.s(b"\x1b[m");
        }
        io::write_all(1, out.bytes());

        // Rows: BusyBox reserves 2 extra bytes for '\n' and the NUL.
        let row_space = col + 2;
        lines_rem -= 1;
        let max_rows = if lines_rem < 0 { 0 } else { lines_rem as usize };
        let mut user_buf = [0u8; 20];
        let mut ppu = [0u8; 32];
        let mut row = [0u8; LINE_BUF];

        for e in self.entries.iter().take(max_rows) {
            let name = user_name(e.proc.uid);
            let user: &[u8] = if name == b"?" {
                sys::format_u64(e.proc.uid as u64, &mut user_buf)
            } else {
                name
            };

            let mut n = 0;
            row[n] = b'\n';
            n += 1;
            let (pstart, plen) = build_ppu(&mut ppu, e.proc.pid, e.proc.ppid, user);
            row[n..n + plen].copy_from_slice(&ppu[pstart..pstart + plen]);
            n += plen;
            row[n] = b' ';
            n += 1;
            row[n..n + 3].copy_from_slice(&e.state);
            n += 3;
            row[n] = b' ';
            row[n + 1] = b' ';
            n += 2;

            let mut vsz_str = [0u8; 5];
            smart_ulltoa5(e.vsz, &mut vsz_str, SCALE);
            row[n..n + 5].copy_from_slice(&vsz_str);
            n += 5;

            let pmem = e
                .vsz
                .wrapping_mul(pmem_scale as u64)
                .wrapping_add(pmem_half as u64)
                .wrapping_shr(pmem_shift as u32) as u32;
            n += put_pct(&mut row[n..], pmem);

            let pcpu = e
                .pcpu
                .wrapping_mul(pcpu_scale)
                .wrapping_add(pcpu_half)
                .wrapping_shr(pcpu_shift as u32);
            n += put_pct(&mut row[n..], pcpu);

            row[n] = b' ';
            n += 1;

            let space = row_space - n;
            if space > 1 {
                n += write_cmdline(&mut row[n..], space, &e.proc);
            }
            io::write_all(1, &row[..n]);
        }

        io::write_str(1, if batch { b"\n" } else { b"\r" });
    }

    /// Mem/CPU/load average lines; returns MemTotal in KB.
    fn display_header(&self, col: usize, lines_rem: &mut i32, batch: bool) -> u64 {
        let mut mi = [0u64; MI_MAX];
        parse_meminfo(&mut mi);

        let mut out = Buf::new();
        if !batch {
            out.s(b"\x1b[H\x1b[J");
        }
        out.s(b"Mem: ");
        out.u(mi[MI_TOTAL].saturating_sub(mi[MI_FREE]));
        out.s(b"K used, ");
        out.u(mi[MI_FREE]);
        out.s(b"K free, ");
        out.u(mi[MI_MEMSHARED] + mi[MI_SHMEM]);
        out.s(b"K shrd, ");
        out.u(mi[MI_BUFFERS]);
        out.s(b"K buff, ");
        out.u(mi[MI_CACHED]);
        out.s(b"K cached");
        out.truncate(col);
        out.s(b"\n");
        io::write_all(1, out.bytes());
        *lines_rem -= 1;

        self.display_cpus(col);
        *lines_rem -= 1;

        let mut out = Buf::new();
        out.s(b"Load average: ");
        let mut lbuf = [0u8; 128];
        let n = read_file(b"/proc/loadavg\0", &mut lbuf);
        if n > 0 {
            let n = n as usize;
            let end = lbuf[..n].iter().position(|&c| c == b'\n').unwrap_or(n);
            out.s(&lbuf[..end]);
        }
        // BusyBox truncates this line one char earlier than the others.
        out.truncate(col.saturating_sub(1));
        out.s(b"\n");
        io::write_all(1, out.bytes());
        *lines_rem -= 1;

        mi[MI_TOTAL]
    }

    /// The "CPU: NN% usr NN% sys ..." line.
    fn display_cpus(&self, col: usize) {
        let total_diff = self.cur.total.wrapping_sub(self.prev.total) as u32;
        let total_diff = if total_diff == 0 { 1 } else { total_diff };

        let mut out = Buf::new();
        out.s(b"CPU:");
        let fields: [(u64, u64, &[u8]); 7] = [
            (self.cur.usr, self.prev.usr, b"usr"),
            (self.cur.sys, self.prev.sys, b"sys"),
            (self.cur.nic, self.prev.nic, b"nic"),
            (self.cur.idle, self.prev.idle, b"idle"),
            (self.cur.iowait, self.prev.iowait, b"io"),
            (self.cur.irq, self.prev.irq, b"irq"),
            (self.cur.softirq, self.prev.softirq, b"sirq"),
        ];
        for (cur, prev, label) in fields {
            let val = 100u32.wrapping_mul(cur.wrapping_sub(prev) as u32) / total_diff;
            out.u_padded(val as u64, 4);
            out.s(b"% ");
            out.s(label);
        }
        out.truncate(col);
        out.s(b"\n");
        io::write_all(1, out.bytes());
    }

    /// BusyBox's `handle_input()`: wait up to `interval_ms` for one key, then
    /// keep draining pending keys without waiting. Returns false to quit.
    fn handle_input(&mut self, interval_ms: i32) -> bool {
        if self.eof {
            sleep_ms(interval_ms);
            return true;
        }
        let mut wait = interval_ms;
        loop {
            let c = match read_key(wait) {
                Key::Got(c) => c,
                Key::Timeout => return true,
                Key::Eof => {
                    self.eof = true;
                    return true;
                }
            };
            wait = 0;

            if c == self.vin || c == self.veof {
                return false;
            }
            match c | 0x20 {
                b'q' => return false,
                b'n' => self.sort_fns[0] = pid_sort,
                b'm' => self.sort_fns = [mem_sort, pcpu_sort, time_sort],
                b'p' => self.sort_fns = [pcpu_sort, mem_sort, time_sort],
                b't' => self.sort_fns = [time_sort, mem_sort, pcpu_sort],
                b'r' => self.inverted = !self.inverted,
                // Unknown key (or an escape sequence): refresh the screen.
                _ => return true,
            }
        }
    }

    /// Put stdin in raw mode like BusyBox's `set_termios_to_raw()`, minus ISIG
    /// so ^C arrives as a key instead of a signal.
    fn set_raw_mode(&mut self) {
        let mut old: libc::termios = unsafe { core::mem::zeroed() };
        if unsafe { libc::tcgetattr(0, &mut old) } != 0 {
            return;
        }
        self.vin = old.c_cc[libc::VINTR];
        self.veof = old.c_cc[libc::VEOF];

        let mut new = old;
        new.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ECHONL | libc::ISIG);
        new.c_cc[libc::VMIN] = 1;
        new.c_cc[libc::VTIME] = 0;
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &new) };

        unsafe {
            SAVED_TERMIOS = old;
            HAVE_TERMIOS = true;
        }
        // Restore the terminal if we are killed while it is raw.
        for sig in [
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGPIPE,
            libc::SIGSEGV,
            libc::SIGBUS,
            libc::SIGABRT,
            libc::SIGFPE,
            libc::SIGILL,
            libc::SIGTRAP,
        ] {
            unsafe {
                libc::signal(
                    sig,
                    restore_and_reraise as extern "C" fn(libc::c_int) as libc::sighandler_t,
                );
            }
        }
    }

    fn restore_terminal(&mut self) {
        unsafe {
            if HAVE_TERMIOS {
                libc::tcsetattr(0, libc::TCSANOW, core::ptr::addr_of!(SAVED_TERMIOS));
                HAVE_TERMIOS = false;
            }
        }
    }
}

// ── process helpers ─────────────────────────────────────────────────────────

/// BusyBox's 3-character STAT field: state, `W` for zero VSZ (not a zombie)
/// and `<`/`N` for negative/positive nice.
fn stat_field(p: &Proc, vsz: u64) -> [u8; 3] {
    let mut s = [p.state, b' ', b' '];
    let mut idx = 1;
    if vsz == 0 && p.state != b'Z' {
        s[1] = b'W';
        idx = 2;
    }
    if p.nice != 0 {
        s[idx] = if p.nice < 0 { b'<' } else { b'N' };
    }
    s
}

/// BusyBox's `smart_ulltoa5()`: five characters, scaled with `scale` when the
/// value does not fit ("  123", "97.6m", " 195m").
fn smart_ulltoa5(ul: u64, out: &mut [u8; 5], scale: &[u8]) {
    const SPACED: &[u8; 10] = b" 123456789";
    const PLAIN: &[u8; 10] = b"0123456789";

    let mut ul = ul;
    let mut idx = 0usize;
    if ul > 99999 {
        ul *= 10;
        loop {
            ul /= 1024;
            idx += 1;
            if ul < 100000 {
                break;
            }
        }
        idx = idx.min(scale.len() - 1);
    }

    let mut u = (ul / 10) as u32;
    let mut v = (ul % 10) as u32;
    let mut fmt = SPACED;

    if idx == 0 {
        let c = SPACED[(u / 1000) as usize];
        out[0] = c;
        if c != b' ' {
            fmt = PLAIN;
        }
        let c = fmt[((u / 100) % 10) as usize];
        out[1] = c;
        if c != b' ' {
            fmt = PLAIN;
        }
        let c = fmt[((u / 10) % 10) as usize];
        out[2] = c;
        if c != b' ' {
            fmt = PLAIN;
        }
        out[3] = fmt[(u % 10) as usize];
        out[4] = PLAIN[v as usize];
    } else {
        if u >= 100 {
            let c = SPACED[(u / 1000) as usize];
            out[0] = c;
            if c != b' ' {
                fmt = PLAIN;
            }
            let c = fmt[((u / 100) % 10) as usize];
            out[1] = c;
            if c != b' ' {
                fmt = PLAIN;
            }
            v = u % 10;
            u /= 10;
            out[2] = fmt[(u % 10) as usize];
        } else {
            let c = SPACED[(u / 10) as usize];
            out[0] = c;
            if c != b' ' {
                fmt = PLAIN;
            }
            out[1] = fmt[(u % 10) as usize];
            out[2] = b'.';
        }
        out[3] = PLAIN[v as usize];
        out[4] = scale[idx];
    }
}

/// BusyBox's `sprintf(ppubuf, "%5u %5u %-8.8s", pid, ppid, user)` plus its fixup
/// that shrinks the PID/PPID padding (and truncates USER) back to 20 chars.
/// Returns (start, len) of the 20-character field inside `out`.
fn build_ppu(out: &mut [u8; 32], pid: u64, ppid: u64, user: &[u8]) -> (usize, usize) {
    let mut n = 0;
    n += put_u(&mut out[n..], pid, 5);
    out[n] = b' ';
    n += 1;
    n += put_u(&mut out[n..], ppid, 5);
    out[n] = b' ';
    n += 1;
    let ul = user.len().min(8);
    out[n..n + ul].copy_from_slice(&user[..ul]);
    n += ul;
    for _ in ul..8 {
        out[n] = b' ';
        n += 1;
    }

    let mut start = 0;
    if n != 20 {
        // Shrink the PID field's leading spaces first...
        if out[start] == b' ' {
            start += 1;
            n -= 1;
            while n != 20 && out[start] == b' ' {
                start += 1;
                n -= 1;
            }
        }
        // ...then the PPID field's, moving the rest of the line left.
        let mut p = start;
        while out[p] != b' ' {
            p += 1;
        }
        p += 1;
        if out[p] == b' ' {
            let mut dst = start;
            while out[dst] != b' ' {
                dst += 1;
            }
            dst += 1;
            p += 1;
            n -= 1;
            while n != 20 && out[p] == b' ' {
                p += 1;
                n -= 1;
            }
            let tail = start + n - p;
            for i in 0..tail {
                out[dst + i] = out[p + i];
            }
        }
    }
    (start, 20)
}

/// Write the COMMAND column: the command line, prefixed with "{comm} " when the
/// basename of argv[0] differs from the kernel task name (BusyBox's
/// `read_cmdline()`, fed from the /proc data ps already read).
fn write_cmdline(row: &mut [u8], space: usize, p: &Proc) -> usize {
    let max = space - 1; // room for the text, excluding BusyBox's NUL

    if p.args_len == 0 {
        // Kernel threads: ps already formatted this as "[comm]".
        let name = p.name();
        let l = name.len().min(max);
        row[..l].copy_from_slice(&name[..l]);
        return l;
    }

    // Basename of argv[0], minus a leading '-' for login shells.
    let argv0 = p.name();
    let argv0_end = argv0.iter().position(|&c| c == b' ').unwrap_or(argv0.len());
    let slash = argv0[..argv0_end]
        .iter()
        .rposition(|&c| c == b'/')
        .map(|i| i + 1)
        .unwrap_or(0);
    let base = &argv0[slash..argv0_end];
    let base = if base.first() == Some(&b'-') {
        &base[1..]
    } else {
        base
    };
    let comm = p.comm();
    let differs = base.len() < comm.len() || base[..comm.len()] != *comm;

    let text = p.args();
    if !differs {
        let l = text.len().min(max);
        for i in 0..l {
            // BusyBox replaces control characters with spaces.
            row[i] = if text[i] < b' ' { b' ' } else { text[i] };
        }
        return l;
    }

    let need = comm.len() + 3; // "{comm} "
    if space <= need {
        // No room for the command line itself.
        let mut w = 0;
        row[w] = b'{';
        w += 1;
        let cl = comm.len().min(max.saturating_sub(2));
        row[w..w + cl].copy_from_slice(&comm[..cl]);
        w += cl;
        row[w] = b'}';
        w += 1;
        return w.min(max);
    }

    let text_len = text.len().min(space - need);
    let mut i = text_len;
    while i > 0 {
        i -= 1;
        // BusyBox replaces control characters with spaces.
        let c = text[i];
        row[need + i] = if c < b' ' { b' ' } else { c };
    }
    let mut w = 0;
    row[w] = b'{';
    w += 1;
    let cl = comm.len().min(max.saturating_sub(2));
    row[w..w + cl].copy_from_slice(&comm[..cl]);
    w += cl;
    row[w] = b'}';
    // BusyBox terminates the "{comm}" with a space at the full (untruncated)
    // offset, then NULs the last byte of the field if it was too small.
    row[need - 1] = b' ';
    (need + text_len).min(max)
}

// ── sorting ─────────────────────────────────────────────────────────────────

fn pcpu_sort(p: &Entry, q: &Entry) -> i32 {
    (q.pcpu as i32).wrapping_sub(p.pcpu as i32)
}

fn mem_sort(p: &Entry, q: &Entry) -> i32 {
    if q.vsz < p.vsz {
        -1
    } else {
        (q.vsz != p.vsz) as i32
    }
}

fn time_sort(p: &Entry, q: &Entry) -> i32 {
    if q.ticks < p.ticks {
        -1
    } else {
        (q.ticks != p.ticks) as i32
    }
}

fn pid_sort(p: &Entry, q: &Entry) -> i32 {
    (q.proc.pid as i32).wrapping_sub(p.proc.pid as i32)
}

// ── /proc parsing ───────────────────────────────────────────────────────────

const MI_TOTAL: usize = 0;
const MI_FREE: usize = 1;
const MI_MEMSHARED: usize = 2;
const MI_SHMEM: usize = 3;
const MI_BUFFERS: usize = 4;
const MI_CACHED: usize = 5;
const MI_MAX: usize = 6;

/// BusyBox's `parse_meminfo()`: the fields top shows, in KB.
fn parse_meminfo(mi: &mut [u64; MI_MAX]) {
    const NAMES: [(&[u8], usize); MI_MAX] = [
        (b"MemTotal", MI_TOTAL),
        (b"MemFree", MI_FREE),
        (b"MemShared", MI_MEMSHARED),
        (b"Shmem", MI_SHMEM),
        (b"Buffers", MI_BUFFERS),
        (b"Cached", MI_CACHED),
    ];

    let mut buf = [0u8; 4096];
    let n = read_file(b"/proc/meminfo\0", &mut buf);
    if n <= 0 {
        return;
    }
    for line in buf[..n as usize].split(|&c| c == b'\n') {
        let colon = match line.iter().position(|&c| c == b':') {
            Some(i) => i,
            None => continue,
        };
        let name = &line[..colon];
        let idx = match NAMES.iter().find(|(nm, _)| *nm == name) {
            Some((_, i)) => *i,
            None => continue,
        };
        let rest = &line[colon + 1..];
        let start = rest
            .iter()
            .position(|c| c.is_ascii_digit())
            .unwrap_or(rest.len());
        let end = rest[start..]
            .iter()
            .position(|c| !c.is_ascii_digit())
            .map(|i| start + i)
            .unwrap_or(rest.len());
        mi[idx] = sys::parse_u64(&rest[start..end]).unwrap_or(0);
    }
}

// ── terminal helpers ────────────────────────────────────────────────────────

/// A fixed output buffer for the header lines. Column headers and rows are
/// written straight to stdout to avoid buffering the whole screen.
struct Buf {
    b: [u8; LINE_BUF],
    n: usize,
}

impl Buf {
    fn new() -> Buf {
        Buf {
            b: [0; LINE_BUF],
            n: 0,
        }
    }

    fn s(&mut self, v: &[u8]) {
        let room = self.b.len() - self.n;
        let l = v.len().min(room);
        self.b[self.n..self.n + l].copy_from_slice(&v[..l]);
        self.n += l;
    }

    fn u(&mut self, v: u64) {
        let mut tmp = [0u8; 24];
        let d = sys::format_u64(v, &mut tmp);
        self.s(d);
    }

    fn u_padded(&mut self, v: u64, width: usize) {
        let mut tmp = [0u8; 24];
        let d = sys::format_u64(v, &mut tmp);
        for _ in d.len()..width {
            self.s(b" ");
        }
        self.s(d);
    }

    fn truncate(&mut self, width: usize) {
        if self.n > width {
            self.n = width;
        }
    }

    fn bytes(&self) -> &[u8] {
        &self.b[..self.n]
    }
}

/// Right-align `v` in at least `width` columns, returning the length written.
fn put_u(out: &mut [u8], v: u64, width: usize) -> usize {
    let mut tmp = [0u8; 24];
    let d = sys::format_u64(v, &mut tmp);
    let pad = width.saturating_sub(d.len());
    for c in out[..pad].iter_mut() {
        *c = b' ';
    }
    out[pad..pad + d.len()].copy_from_slice(d);
    pad + d.len()
}

/// `"%4u%%"`, returning the length written (5 unless the value overflows).
fn put_pct(out: &mut [u8], v: u32) -> usize {
    let k = put_u(out, v as u64, 4);
    out[k] = b'%';
    k + 1
}

fn env_num(name: &[u8]) -> Option<i32> {
    let val = io::getenv(name)?;
    sys::parse_u64(val).map(|v| v.min(i32::MAX as u64) as i32)
}

/// Screen size of stdout, defaulting like BusyBox's `get_terminal_width_height()`.
fn screen_size() -> Option<(usize, i32)> {
    let mut ws: libc::winsize = unsafe { core::mem::zeroed() };
    let _ = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) };

    let lines = if ws.ws_row == 0 {
        env_num(b"LINES").unwrap_or(24)
    } else {
        ws.ws_row as i32
    };
    let mut col = if ws.ws_col == 0 {
        env_num(b"COLUMNS").unwrap_or(80)
    } else {
        ws.ws_col as i32
    };
    if lines < 5 || col < 10 {
        return None;
    }
    if col > (LINE_BUF - 2) as i32 {
        col = (LINE_BUF - 2) as i32;
    }
    Some((col as usize, lines))
}

enum Key {
    Got(u8),
    Timeout,
    Eof,
}

/// Read one key, waiting at most `timeout_ms` (0 = poll only).
fn read_key(timeout_ms: i32) -> Key {
    let mut pfd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if r < 0 {
        return Key::Timeout;
    }
    if r == 0 {
        return Key::Timeout;
    }
    let mut b = [0u8; 1];
    if io::read(0, &mut b) <= 0 {
        return Key::Eof;
    }
    Key::Got(b[0])
}

fn sleep_ms(ms: i32) {
    if ms <= 0 {
        return;
    }
    let ts = libc::timespec {
        tv_sec: (ms / 1000) as libc::time_t,
        tv_nsec: ((ms % 1000) as libc::c_long) * 1_000_000,
    };
    unsafe { libc::nanosleep(&ts, core::ptr::null_mut()) };
}

static mut SAVED_TERMIOS: libc::termios = unsafe { core::mem::zeroed() };
static mut HAVE_TERMIOS: bool = false;

extern "C" fn restore_and_reraise(sig: libc::c_int) {
    unsafe {
        if HAVE_TERMIOS {
            libc::tcsetattr(0, libc::TCSANOW, core::ptr::addr_of!(SAVED_TERMIOS));
            HAVE_TERMIOS = false;
        }
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
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
    fn test_smart_ulltoa5_exact_format() {
        let mut b = [0u8; 5];
        smart_ulltoa5(0, &mut b, SCALE);
        assert_eq!(&b, b"    0");
        smart_ulltoa5(2680, &mut b, SCALE);
        assert_eq!(&b, b" 2680");
        smart_ulltoa5(14748, &mut b, SCALE);
        assert_eq!(&b, b"14748");
        smart_ulltoa5(200000, &mut b, SCALE); // 195.3 MB -> " 195m"
        assert_eq!(&b, b" 195m");
        smart_ulltoa5(100000, &mut b, SCALE); // 97.6 MB
        assert_eq!(&b, b"97.6m");
    }

    #[test]
    fn test_ppu_is_twenty_chars_wide() {
        let mut b = [0u8; 32];
        let (s, l) = build_ppu(&mut b, 475, 1, b"avahi");
        assert_eq!(l, 20);
        assert_eq!(&b[s..s + l], b"  475     1 avahi   ");
        // A 6-digit pid overflows the 20-char field: the padding is shaved off
        // (one byte here) instead of the field growing.
        let (s, l) = build_ppu(&mut b, 123456, 1, b"root");
        assert_eq!(l, 20);
        assert_eq!(&b[s..s + l], b"123456    1 root    ");
        // ...and a long user name is truncated to keep the field at 20.
        let (s, l) = build_ppu(&mut b, 123456, 123456, b"verylongname");
        assert_eq!(&b[s..s + l], b"123456 123456 verylo");
    }

    #[test]
    fn test_interval_parsing() {
        assert_eq!(parse_interval_ms(b"5"), Some(5000));
        assert_eq!(parse_interval_ms(b"0.5"), Some(500));
        assert_eq!(parse_interval_ms(b"1m"), Some(60000));
        assert_eq!(parse_interval_ms(b"2d"), Some(172800000));
        assert_eq!(parse_interval_ms(b"x"), None);
    }

    #[test]
    fn test_top_batch_frame_matches_busybox_layout() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["top", "-bn1"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();

        assert!(lines[0].starts_with("Mem: "), "line0: {}", lines[0]);
        assert!(lines[1].starts_with("CPU:"), "line1: {}", lines[1]);
        assert!(
            lines[2].starts_with("Load average: "),
            "line2: {}",
            lines[2]
        );
        assert_eq!(
            lines[3],
            "  PID  PPID USER     STAT   VSZ %VSZ %CPU COMMAND"
        );
        // Batch mode shows the whole process list, so there is a data row.
        let row = lines[4];
        assert!(row.len() > 42, "row: {row}");
        assert!(row[..5].trim().parse::<u32>().is_ok(), "pid column: {row}");
        assert!(row[31..].contains('%'), "percent columns: {row}");
    }

    #[test]
    fn test_top_row_columns_line_up_with_header() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["top", "-bn1"])
            .output()
            .unwrap();
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let mut rows = 0;
        let mut aligned = 0;
        for row in stdout.lines().skip(4) {
            let r = row.as_bytes();
            // The final newline BusyBox also prints leaves one empty line.
            if r.is_empty() {
                continue;
            }
            assert!(r.len() >= 42, "short row: {row:?}");
            rows += 1;
            // A pid wider than 5 digits shifts the rest of the line, exactly as
            // BusyBox's own shrink logic does; only check the normal layout.
            if r[0] != b' ' {
                continue;
            }
            aligned += 1;
            // PID and PPID are separated by a space.
            assert_eq!(r[5], b' ', "row: {row:?}");
            assert_eq!(r[11], b' ', "row: {row:?}");
            // STAT is 3 chars (state + W + nice flag) followed by 2 spaces.
            assert!(b"RSDTZWIXPK".contains(&r[21]), "row: {row:?}");
            assert!(b" W<>N".contains(&r[22]), "row: {row:?}");
            assert!(b" <>N".contains(&r[23]), "row: {row:?}");
            assert_eq!(&r[24..26], b"  ", "row: {row:?}");
            // %VSZ/%CPU are `%4u%%`; wider values shift the COMMAND column the
            // same way BusyBox's own printf does, so only check the normal case.
            if r[35] == b'%' && r[40] == b'%' {
                assert_eq!(r[41], b' ', "row: {row:?}");
            }
        }
        assert!(rows > 0, "no process rows");
        assert!(aligned > 0, "no normally-aligned rows");
    }

    #[test]
    fn test_top_help_and_version() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        for flag in ["--help", "--version"] {
            let output = Command::new(&armybox).args(["top", flag]).output().unwrap();
            assert_eq!(output.status.code(), Some(0), "top {flag}");
            assert!(!output.stdout.is_empty(), "top {flag}");
        }
    }

    #[test]
    fn test_top_invalid_option() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["top", "-Z"]).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }
}
