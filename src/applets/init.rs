//! Init system utilities
//!
//! Implements init (PID 1), getty, and related utilities for system startup.

use super::get_arg;
use crate::io;

#[cfg(all(feature = "init", feature = "alloc"))]
use alloc::vec::Vec;

// Inittab action types
#[cfg(feature = "init")]
const ACTION_SYSINIT: u8 = 1;
#[cfg(feature = "init")]
const ACTION_RESPAWN: u8 = 2;
#[cfg(feature = "init")]
const ACTION_ONCE: u8 = 3;
#[cfg(feature = "init")]
const ACTION_WAIT: u8 = 4;
#[cfg(feature = "init")]
const ACTION_SHUTDOWN: u8 = 5;
#[cfg(feature = "init")]
const ACTION_CTRLALTDEL: u8 = 6;
#[cfg(feature = "init")]
const ACTION_RESTART: u8 = 7;

/// Entry in inittab
#[cfg(all(feature = "init", feature = "alloc"))]
struct InittabEntry {
    id: Vec<u8>,
    action: u8,
    process: Vec<u8>,
    pid: i32, // Current PID if running, 0 if not
}

/// Parse action string to action type
#[cfg(feature = "init")]
fn parse_action(action: &[u8]) -> u8 {
    if action == b"sysinit" {
        ACTION_SYSINIT
    } else if action == b"respawn" {
        ACTION_RESPAWN
    } else if action == b"once" {
        ACTION_ONCE
    } else if action == b"wait" {
        ACTION_WAIT
    } else if action == b"shutdown" {
        ACTION_SHUTDOWN
    } else if action == b"ctrlaltdel" {
        ACTION_CTRLALTDEL
    } else if action == b"restart" {
        ACTION_RESTART
    } else {
        0
    }
}

/// Run a command by forking and execing /bin/sh -c "command"
#[cfg(feature = "init")]
fn run_command(cmd: &[u8], wait_for: bool) -> i32 {
    let pid = io::fork();
    if pid < 0 {
        io::write_str(2, b"init: fork failed\n");
        return -1;
    }

    if pid == 0 {
        // Child process
        // Build argv for /bin/sh -c "command"
        let sh_path = b"/bin/sh\0";
        let dash_c = b"-c\0";

        // Create null-terminated command
        let mut cmd_buf = [0u8; 1024];
        let cmd_len = core::cmp::min(cmd.len(), cmd_buf.len() - 1);
        cmd_buf[..cmd_len].copy_from_slice(&cmd[..cmd_len]);
        cmd_buf[cmd_len] = 0;

        let argv: [*const libc::c_char; 4] = [
            sh_path.as_ptr() as *const libc::c_char,
            dash_c.as_ptr() as *const libc::c_char,
            cmd_buf.as_ptr() as *const libc::c_char,
            core::ptr::null(),
        ];

        unsafe {
            libc::execv(sh_path.as_ptr() as *const libc::c_char, argv.as_ptr());
        }

        // If exec fails, try running command directly
        io::write_str(2, b"init: exec failed for: ");
        io::write_all(2, cmd);
        io::write_str(2, b"\n");
        io::exit(127);
    }

    // Parent
    if wait_for {
        let mut status: i32 = 0;
        io::waitpid(pid, &mut status, 0);
        return status;
    }

    pid
}

/// Mount a filesystem if not already mounted
#[cfg(feature = "init")]
fn mount_if_needed(source: &[u8], target: &[u8], fstype: &[u8]) {
    // Check if already mounted by trying to stat the target
    let _stat_buf: libc::stat = unsafe { core::mem::zeroed() };

    // Create null-terminated strings
    let mut target_buf = [0u8; 256];
    let tlen = core::cmp::min(target.len(), target_buf.len() - 1);
    target_buf[..tlen].copy_from_slice(&target[..tlen]);

    let mut source_buf = [0u8; 256];
    let slen = core::cmp::min(source.len(), source_buf.len() - 1);
    source_buf[..slen].copy_from_slice(&source[..slen]);

    let mut fstype_buf = [0u8; 64];
    let flen = core::cmp::min(fstype.len(), fstype_buf.len() - 1);
    fstype_buf[..flen].copy_from_slice(&fstype[..flen]);

    // Try to mount
    let ret = unsafe {
        libc::mount(
            source_buf.as_ptr() as *const libc::c_char,
            target_buf.as_ptr() as *const libc::c_char,
            fstype_buf.as_ptr() as *const libc::c_char,
            0,
            core::ptr::null(),
        )
    };

    if ret == 0 {
        io::write_str(1, b"init: mounted ");
        io::write_all(1, target);
        io::write_str(1, b"\n");
    }
}

/// Basic init for systems without alloc feature
#[cfg(all(feature = "init", not(feature = "alloc")))]
pub fn init(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"ArmyBox init starting...\n");

    // Mount essential filesystems
    mount_if_needed(b"proc", b"/proc", b"proc");
    mount_if_needed(b"sysfs", b"/sys", b"sysfs");
    mount_if_needed(b"devtmpfs", b"/dev", b"devtmpfs");

    // Run /etc/init.d/rcS if it exists
    let rcs = b"/etc/init.d/rcS";
    let fd = io::open(rcs, libc::O_RDONLY, 0);
    if fd >= 0 {
        io::close(fd);
        io::write_str(1, b"init: running /etc/init.d/rcS\n");
        run_command(rcs, true);
    }

    // Spawn a shell on console
    io::write_str(1, b"init: spawning shell on console\n");
    loop {
        let pid = io::fork();
        if pid == 0 {
            // Child - exec shell
            let sh = b"/bin/sh\0";
            let argv: [*const libc::c_char; 2] =
                [sh.as_ptr() as *const libc::c_char, core::ptr::null()];
            unsafe {
                libc::setsid();
                libc::execv(sh.as_ptr() as *const libc::c_char, argv.as_ptr());
            }
            io::exit(1);
        }

        // Parent - wait for shell to exit, then respawn
        let mut status: i32 = 0;
        io::waitpid(pid, &mut status, 0);
        io::write_str(1, b"init: shell exited, respawning...\n");
    }
}

/// Full init with inittab parsing (requires alloc)
#[cfg(all(feature = "init", feature = "alloc"))]
pub fn init(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(1, b"ArmyBox init starting...\n");

    // Set up signal handlers
    unsafe {
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_IGN);
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }

    // Mount essential filesystems
    mount_if_needed(b"proc", b"/proc", b"proc");
    mount_if_needed(b"sysfs", b"/sys", b"sysfs");
    mount_if_needed(b"devtmpfs", b"/dev", b"devtmpfs");

    // Parse inittab
    let mut entries = parse_inittab();

    if entries.is_empty() {
        // No inittab - run /etc/init.d/rcS and spawn shell
        let rcs = b"/etc/init.d/rcS";
        let fd = io::open(rcs, libc::O_RDONLY, 0);
        if fd >= 0 {
            io::close(fd);
            io::write_str(1, b"init: running /etc/init.d/rcS\n");
            run_command(rcs, true);
        }

        // Spawn shell
        io::write_str(1, b"init: no inittab, spawning shell\n");
        loop {
            let pid = io::fork();
            if pid == 0 {
                let sh = b"/bin/sh\0";
                let argv: [*const libc::c_char; 2] =
                    [sh.as_ptr() as *const libc::c_char, core::ptr::null()];
                unsafe {
                    libc::setsid();
                    libc::execv(sh.as_ptr() as *const libc::c_char, argv.as_ptr());
                }
                io::exit(1);
            }
            let mut status: i32 = 0;
            io::waitpid(pid, &mut status, 0);
        }
    }

    // Run sysinit entries first (wait for each)
    for entry in entries.iter() {
        if entry.action == ACTION_SYSINIT {
            io::write_str(1, b"init: sysinit: ");
            io::write_all(1, &entry.process);
            io::write_str(1, b"\n");
            run_command(&entry.process, true);
        }
    }

    // Run wait entries
    for entry in entries.iter() {
        if entry.action == ACTION_WAIT {
            io::write_str(1, b"init: wait: ");
            io::write_all(1, &entry.process);
            io::write_str(1, b"\n");
            run_command(&entry.process, true);
        }
    }

    // Run once entries (don't wait)
    for entry in entries.iter() {
        if entry.action == ACTION_ONCE {
            io::write_str(1, b"init: once: ");
            io::write_all(1, &entry.process);
            io::write_str(1, b"\n");
            run_command(&entry.process, false);
        }
    }

    // Start respawn entries
    for entry in entries.iter_mut() {
        if entry.action == ACTION_RESPAWN {
            io::write_str(1, b"init: respawn: ");
            io::write_all(1, &entry.process);
            io::write_str(1, b"\n");
            entry.pid = run_command(&entry.process, false);
        }
    }

    // Main loop - reap children and respawn as needed
    loop {
        let mut status: i32 = 0;
        let pid = io::wait(&mut status);

        if pid > 0 {
            // Check if this was a respawn process
            for entry in entries.iter_mut() {
                if entry.action == ACTION_RESPAWN && entry.pid == pid {
                    // Respawn it
                    io::write_str(1, b"init: respawning: ");
                    io::write_all(1, &entry.process);
                    io::write_str(1, b"\n");

                    // Small delay to prevent tight respawn loops
                    unsafe {
                        libc::sleep(1);
                    }

                    entry.pid = run_command(&entry.process, false);
                    break;
                }
            }
        }
    }
}

/// Parse /etc/inittab
#[cfg(all(feature = "init", feature = "alloc"))]
fn parse_inittab() -> Vec<InittabEntry> {
    let mut entries = Vec::new();

    let fd = io::open(b"/etc/inittab", libc::O_RDONLY, 0);
    if fd < 0 {
        return entries;
    }

    let content = io::read_all(fd);
    io::close(fd);

    // Parse line by line
    // Format: id:runlevels:action:process
    // Or BusyBox format: ::action:process
    for line in content.split(|&c| c == b'\n') {
        let line = trim(line);

        // Skip empty lines and comments
        if line.is_empty() || line[0] == b'#' {
            continue;
        }

        // Split by colons
        let parts: Vec<&[u8]> = line.split(|&c| c == b':').collect();

        if parts.len() >= 4 {
            // Standard format: id:runlevels:action:process
            let id = parts[0].to_vec();
            let action = parse_action(parts[2]);
            let process = join_bytes(&parts[3..], b':');

            if action != 0 {
                entries.push(InittabEntry {
                    id,
                    action,
                    process,
                    pid: 0,
                });
            }
        } else if parts.len() >= 3 {
            // BusyBox format: ::action:process (id and runlevels can be empty)
            let action = parse_action(parts[1]);
            let process = join_bytes(&parts[2..], b':');

            if action != 0 {
                entries.push(InittabEntry {
                    id: Vec::new(),
                    action,
                    process,
                    pid: 0,
                });
            }
        }
    }

    entries
}

/// Trim whitespace from slice
#[cfg(all(feature = "init", feature = "alloc"))]
fn trim(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|&c| c != b' ' && c != b'\t' && c != b'\r')
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&c| c != b' ' && c != b'\t' && c != b'\r')
        .map(|i| i + 1)
        .unwrap_or(start);
    &s[start..end]
}

/// Join byte slices with a separator byte.
#[cfg(all(feature = "init", feature = "alloc"))]
fn join_bytes(parts: &[&[u8]], sep: u8) -> Vec<u8> {
    let mut result = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            result.push(sep);
        }
        result.extend_from_slice(part);
    }
    result
}

#[cfg(feature = "telinit")]
pub fn telinit(argc: i32, argv: *const *const u8) -> i32 {
    // Send signal to init
    if argc < 2 {
        io::write_str(2, b"Usage: telinit <runlevel>\n");
        return 1;
    }

    // For now, just print what we would do
    let arg = unsafe { get_arg(argv, 1) };
    if let Some(level) = arg {
        io::write_str(1, b"telinit: would switch to runlevel ");
        io::write_all(1, level);
        io::write_str(1, b"\n");
    }
    0
}

#[cfg(feature = "runlevel")]
pub fn runlevel(_argc: i32, _argv: *const *const u8) -> i32 {
    // Return current runlevel
    // Format: "previous current"
    io::write_str(1, b"N 3\n");
    0
}

#[cfg(feature = "getty")]
pub fn getty(argc: i32, argv: *const *const u8) -> i32 {
    // getty [OPTIONS] BAUD_RATE TTY
    // Minimal implementation: open tty, spawn login or shell

    let mut tty: Option<&[u8]> = None;

    // Parse arguments - look for tty device
    for i in 1..argc {
        let arg = match unsafe { get_arg(argv, i) } {
            Some(a) => a,
            None => continue,
        };

        if arg[0] != b'-' {
            // Not an option - could be baud rate or tty
            if arg.iter().all(|&c| c >= b'0' && c <= b'9') {
                // Numeric - baud rate, skip
                continue;
            }
            tty = Some(arg);
        }
    }

    // Build tty path
    let mut tty_buf = [0u8; 256];
    let tty_len;

    if let Some(t) = tty {
        if t.starts_with(b"/dev/") {
            // Already has /dev/ prefix
            tty_len = core::cmp::min(t.len(), tty_buf.len() - 1);
            tty_buf[..tty_len].copy_from_slice(&t[..tty_len]);
        } else {
            // Prepend /dev/
            let prefix = b"/dev/";
            let prefix_len = prefix.len();
            let name_len = core::cmp::min(t.len(), tty_buf.len() - prefix_len - 1);
            tty_buf[..prefix_len].copy_from_slice(prefix);
            tty_buf[prefix_len..prefix_len + name_len].copy_from_slice(&t[..name_len]);
            tty_len = prefix_len + name_len;
        }
    } else {
        // Default to console
        let default = b"/dev/console";
        tty_len = default.len();
        tty_buf[..tty_len].copy_from_slice(default);
    }

    let fd = io::open(&tty_buf[..tty_len], libc::O_RDWR, 0);
    if fd < 0 {
        io::write_str(2, b"getty: cannot open ");
        io::write_all(2, &tty_buf[..tty_len]);
        io::write_str(2, b"\n");
        return 1;
    }

    // Make this the controlling terminal
    unsafe {
        libc::setsid();
        libc::ioctl(fd, libc::TIOCSCTTY as crate::io::IoctlReq, 0);
    }

    // Redirect stdin/stdout/stderr to tty
    io::dup2(fd, 0);
    io::dup2(fd, 1);
    io::dup2(fd, 2);
    if fd > 2 {
        io::close(fd);
    }

    // Print banner. The username/password prompts are handled by /bin/login,
    // which we exec below so that authentication actually takes place. (The
    // login program reads stdin, which we have already pointed at the tty.)
    io::write_str(1, b"\nArmyBox Linux\n");

    // Exec /bin/login to authenticate the user rather than dropping straight
    // into an unauthenticated shell.
    let login_path = b"/bin/login\0";
    let login_name = b"login\0";
    let argv_login: [*const libc::c_char; 2] = [
        login_name.as_ptr() as *const libc::c_char,
        core::ptr::null(),
    ];

    unsafe {
        libc::execv(
            login_path.as_ptr() as *const libc::c_char,
            argv_login.as_ptr(),
        );
    }

    io::write_str(2, b"getty: exec /bin/login failed\n");
    1
}

#[cfg(feature = "sulogin")]
pub fn sulogin(_argc: i32, _argv: *const *const u8) -> i32 {
    use crate::applets::system::login;

    io::write_str(1, b"Entering single-user mode...\n");

    // Require the root password before granting the maintenance shell.
    // Matching util-linux: if root has no password or a locked hash
    // (`!`/`*`), grant the shell directly; otherwise prompt and verify.
    let hash = login::get_shadow_hash(b"root");
    let locked = hash.is_empty() || hash[0] == b'!' || hash[0] == b'*';
    if !locked {
        io::write_str(2, b"Give root password for maintenance\n");
        io::write_str(2, b"(or press Control-D to continue): ");
        let old_termios = login::disable_echo();
        let entered = login::read_line();
        login::restore_termios(&old_termios);
        io::write_str(2, b"\n");
        match entered {
            Some(password) => {
                if !login::verify_password(&password, &hash) {
                    io::write_str(2, b"sulogin: Authentication failure\n");
                    return 1;
                }
            }
            None => {
                // EOF (Control-D): reboot rather than granting a shell.
                io::write_str(2, b"sulogin: no password entered\n");
                return 1;
            }
        }
    }

    // Spawn shell
    let sh = b"/bin/sh\0";
    let argv: [*const libc::c_char; 2] = [sh.as_ptr() as *const libc::c_char, core::ptr::null()];

    unsafe {
        libc::execv(sh.as_ptr() as *const libc::c_char, argv.as_ptr());
    }

    1
}

#[cfg(feature = "oneit")]
pub fn oneit(argc: i32, argv: *const *const u8) -> i32 {
    // Run a single command as init
    if argc < 2 {
        io::write_str(2, b"Usage: oneit COMMAND [ARGS...]\n");
        return 1;
    }

    // Build argv for the command
    #[cfg(all(feature = "oneit", feature = "alloc"))]
    {
        let mut args: Vec<*const libc::c_char> = Vec::new();
        for i in 1..argc {
            if let Some(arg) = unsafe { get_arg(argv, i) } {
                // Need null-terminated string
                let mut buf = alloc::vec![0u8; arg.len() + 1];
                buf[..arg.len()].copy_from_slice(arg);
                args.push(buf.leak().as_ptr() as *const libc::c_char);
            }
        }
        args.push(core::ptr::null());

        if !args.is_empty() {
            let cmd = args[0];
            unsafe {
                libc::execvp(cmd, args.as_ptr());
            }
        }
    }

    #[cfg(all(feature = "oneit", not(feature = "alloc")))]
    {
        io::write_str(2, b"oneit: requires alloc feature\n");
    }

    1
}

#[cfg(feature = "switch_root")]
pub fn switch_root(argc: i32, argv: *const *const u8) -> i32 {
    // switch_root NEW_ROOT NEW_INIT [ARGS]
    if argc < 3 {
        io::write_str(2, b"Usage: switch_root NEW_ROOT NEW_INIT [ARGS...]\n");
        return 1;
    }

    let new_root = match unsafe { get_arg(argv, 1) } {
        Some(r) => r,
        None => return 1,
    };

    let new_init = match unsafe { get_arg(argv, 2) } {
        Some(i) => i,
        None => return 1,
    };

    // Change to new root
    let mut root_buf = [0u8; 256];
    let rlen = core::cmp::min(new_root.len(), root_buf.len() - 1);
    root_buf[..rlen].copy_from_slice(&new_root[..rlen]);

    unsafe {
        if libc::chdir(root_buf.as_ptr() as *const libc::c_char) != 0 {
            io::write_str(2, b"switch_root: chdir failed\n");
            return 1;
        }

        if libc::mount(
            root_buf.as_ptr() as *const libc::c_char,
            b"/\0".as_ptr() as *const libc::c_char,
            core::ptr::null(),
            libc::MS_MOVE,
            core::ptr::null(),
        ) != 0
        {
            io::write_str(2, b"switch_root: mount --move failed\n");
            return 1;
        }

        if libc::chroot(b".\0".as_ptr() as *const libc::c_char) != 0 {
            io::write_str(2, b"switch_root: chroot failed\n");
            return 1;
        }

        if libc::chdir(b"/\0".as_ptr() as *const libc::c_char) != 0 {
            io::write_str(2, b"switch_root: chdir / failed\n");
            return 1;
        }
    }

    // Exec new init
    let mut init_buf = [0u8; 256];
    let ilen = core::cmp::min(new_init.len(), init_buf.len() - 1);
    init_buf[..ilen].copy_from_slice(&new_init[..ilen]);

    let argv_init: [*const libc::c_char; 2] =
        [init_buf.as_ptr() as *const libc::c_char, core::ptr::null()];

    unsafe {
        libc::execv(init_buf.as_ptr() as *const libc::c_char, argv_init.as_ptr());
    }

    io::write_str(2, b"switch_root: exec failed\n");
    1
}

#[cfg(feature = "watchdog")]
pub fn watchdog(argc: i32, argv: *const *const u8) -> i32 {
    // watchdog [-t TIMEOUT] [-T SHUTDOWN_TIMEOUT] DEV
    let device = if argc >= 2 {
        match unsafe { get_arg(argv, argc - 1) } {
            Some(d) => d,
            None => b"/dev/watchdog" as &[u8],
        }
    } else {
        b"/dev/watchdog"
    };

    // Open watchdog device
    let mut dev_buf = [0u8; 256];
    let dlen = core::cmp::min(device.len(), dev_buf.len() - 1);
    dev_buf[..dlen].copy_from_slice(&device[..dlen]);

    let fd = io::open(&dev_buf[..dlen], libc::O_WRONLY, 0);
    if fd < 0 {
        io::write_str(2, b"watchdog: cannot open ");
        io::write_all(2, device);
        io::write_str(2, b"\n");
        return 1;
    }

    io::write_str(1, b"watchdog: started\n");

    // Keep writing to prevent reboot
    loop {
        io::write_all(fd, b"\0");
        unsafe {
            libc::sleep(10);
        }
    }
}
