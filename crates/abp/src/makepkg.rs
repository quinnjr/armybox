//! abp makepkg — Build packages from ABUILD files.
//!
//! This module provides the `abp makepkg` subcommand, which is a thin wrapper
//! that locates and exec's the `abp-makepkg` shell script with all arguments
//! forwarded. This follows the same model as Arch Linux, where `pacman` (C
//! binary) and `makepkg` (bash script) are separate tools that share a
//! package format.
//!
//! The shell script handles the full build workflow:
//!   1. Parse ABUILD file (package descriptor)
//!   2. Download & verify sources
//!   3. Extract, prepare, build, check, package
//!   4. Create .abp binary packages
//!   5. Optionally install with `abp add`
//!
//! Search paths for the script (in order):
//!   1. /usr/bin/abp-makepkg
//!   2. /usr/lib/abp/makepkg
//!   3. $PATH lookup

extern crate alloc;

use crate::io;
use alloc::vec::Vec;

/// Known locations for the abp-makepkg script
static MAKEPKG_PATHS: &[&[u8]] = &[
    b"/usr/bin/abp-makepkg",
    b"/usr/lib/abp/makepkg",
    b"/usr/local/bin/abp-makepkg",
];

/// Entry point for `abp makepkg [args...]`
///
/// Parses any flags that the Rust wrapper cares about (currently only --help
/// and --version), then exec's the shell script with all remaining arguments.
pub fn run(args: &[&[u8]]) -> i32 {
    // Fast path: just --help from `abp makepkg`
    if args.len() == 2 {
        print_help();
        return 0;
    }

    // Check for --help in subcommand args
    for &arg in &args[2..] {
        if arg == b"-h" || arg == b"--help" {
            // Let the shell script handle its own help
            break;
        }
    }

    // Build argv for exec: ["/bin/sh", "abp-makepkg", ...args]
    let script = find_makepkg();
    if script.is_empty() {
        io::write_str(2, b"abp makepkg: cannot find abp-makepkg script\n");
        io::write_str(2, b"Searched:\n");
        for path in MAKEPKG_PATHS {
            io::write_str(2, b"  ");
            io::write_all(2, path);
            io::write_str(2, b"\n");
        }
        io::write_str(
            2,
            b"\nInstall abp-makepkg to one of these locations, or ensure it is on $PATH.\n",
        );
        return 1;
    }

    // Forward all args after "abp makepkg" to the script
    exec_makepkg(&script, &args[2..])
}

/// Find the makepkg script on the filesystem
fn find_makepkg() -> Vec<u8> {
    // Check known paths first
    for &path in MAKEPKG_PATHS {
        if io::access(path, libc::X_OK) == 0 {
            return path.to_vec();
        }
    }

    // Try PATH lookup: check common bin directories
    // (In a full implementation we'd parse $PATH, but for no_std we check
    // the most common locations)
    let extra_paths: &[&[u8]] = &[
        b"/usr/bin/abp-makepkg",
        b"/bin/abp-makepkg",
        b"/sbin/abp-makepkg",
        b"/usr/sbin/abp-makepkg",
        b"/usr/local/bin/abp-makepkg",
    ];

    for &path in extra_paths {
        if io::access(path, libc::X_OK) == 0 {
            return path.to_vec();
        }
    }

    Vec::new()
}

/// Execute the makepkg script via /bin/sh, replacing this process
fn exec_makepkg(script: &[u8], extra_args: &[&[u8]]) -> i32 {
    // Build null-terminated argument strings for execve
    // argv = ["/bin/sh", "abp-makepkg", arg1, arg2, ..., NULL]

    let mut argv_bufs: Vec<Vec<u8>> = Vec::new();

    // argv[0]: shell
    argv_bufs.push(b"/bin/sh\0".to_vec());

    // argv[1]: script path
    let mut script_buf = script.to_vec();
    script_buf.push(0);
    argv_bufs.push(script_buf);

    // argv[2..]: forwarded arguments
    for &arg in extra_args {
        let mut buf = arg.to_vec();
        buf.push(0);
        argv_bufs.push(buf);
    }

    // Build raw pointer array
    let mut argv_ptrs: Vec<*const u8> = Vec::new();
    for buf in &argv_bufs {
        argv_ptrs.push(buf.as_ptr());
    }
    argv_ptrs.push(core::ptr::null());

    // exec (fork + exec pattern for safety)
    let pid = unsafe { libc::fork() };

    if pid < 0 {
        io::write_str(2, b"abp makepkg: fork failed\n");
        return 1;
    }

    if pid == 0 {
        // Child: exec the shell script
        unsafe {
            libc::execv(
                argv_ptrs[0] as *const libc::c_char,
                argv_ptrs.as_ptr() as *const *const libc::c_char,
            );
            // If execv returns, it failed
            libc::_exit(127);
        }
    }

    // Parent: wait for child
    let mut status: libc::c_int = 0;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }

    // Extract exit code
    if status & 0x7f == 0 {
        // Normal exit
        (status >> 8) & 0xff
    } else {
        // Killed by signal
        128 + (status & 0x7f)
    }
}

fn print_help() {
    io::write_str(1, b"abp makepkg - Build packages from ABUILD files\n\n");
    io::write_str(1, b"Usage: abp makepkg [options]\n\n");
    io::write_str(
        1,
        b"This command builds .abp packages from ABUILD descriptors,\n",
    );
    io::write_str(1, b"similar to Arch Linux's makepkg/PKGBUILD system.\n\n");
    io::write_str(1, b"Build Options:\n");
    io::write_str(
        1,
        b"  -s, --syncdeps    Install missing deps with 'abp add'\n",
    );
    io::write_str(1, b"  -e, --noextract   Do not extract source files\n");
    io::write_str(1, b"  -o, --nobuild     Download & extract only\n");
    io::write_str(
        1,
        b"  -R, --repackage   Re-run package() without rebuilding\n",
    );
    io::write_str(1, b"  -f, --force       Overwrite existing package\n");
    io::write_str(1, b"  -c, --clean       Clean up work files after build\n");
    io::write_str(1, b"  -C, --cleanbuild  Remove srcdir before building\n");
    io::write_str(
        1,
        b"      --nocheck     Do not run the check() function\n\n",
    );
    io::write_str(1, b"Install Options:\n");
    io::write_str(1, b"  -i, --install     Install package after building\n");
    io::write_str(1, b"  -r, --rmdeps      Remove makedeps after build\n\n");
    io::write_str(1, b"Info Options:\n");
    io::write_str(1, b"  -g, --geninteg    Generate integrity checksums\n");
    io::write_str(1, b"      --printsrcinfo Print SRCINFO and exit\n");
    io::write_str(
        1,
        b"      --packagelist  List packages that would be created\n\n",
    );
    io::write_str(1, b"Other:\n");
    io::write_str(1, b"  -p <file>         Use alternate ABUILD file\n");
    io::write_str(1, b"  -S, --source      Build source-only tarball\n");
    io::write_str(1, b"      --config <f>  Use alternate makepkg.conf\n");
    io::write_str(1, b"  -n, --dry-run     Show what would be done\n");
    io::write_str(1, b"  -h, --help        Show full help (from script)\n\n");
    io::write_str(1, b"Configuration:\n");
    io::write_str(1, b"  /etc/abp/makepkg.conf         System configuration\n");
    io::write_str(1, b"  ~/.config/abp/makepkg.conf    User overrides\n\n");
    io::write_str(
        1,
        b"ABUILD files define packages with variables and functions:\n",
    );
    io::write_str(
        1,
        b"  Variables: pkgname, pkgver, pkgrel, pkgdesc, depends, ...\n",
    );
    io::write_str(
        1,
        b"  Functions: prepare(), build(), check(), package()\n\n",
    );
    io::write_str(
        1,
        b"See 'abp-makepkg --help' for the full option reference.\n",
    );
}
