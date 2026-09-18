//! Armybox - A #[no_std] BusyBox/Toybox clone in Rust
//!
//! Multi-call binary providing Unix utilities in a tiny package.

#![no_std]
#![no_main]
#![allow(clippy::not_unsafe_ptr_arg_deref)] // main() takes raw pointer argv from C

extern crate armybox;

use armybox::{io, applets, run_applet};

/// Main entry point
#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 1 || argv.is_null() {
        return 1;
    }

    // Get the program name (argv[0])
    let prog_name = unsafe { applets::get_arg(argv, 0) };

    let prog_name = match prog_name {
        Some(name) => name,
        None => return 1,
    };

    // Extract basename from program name
    let applet_name = basename(prog_name);

    // Check if invoked as "armybox" itself
    if io::bytes_eq(applet_name, b"armybox") {
        // If there's an argument, use it as the applet name
        if argc > 1 {
            if let Some(arg1) = unsafe { applets::get_arg(argv, 1) } {
                // Check for special flags
                if io::bytes_eq(arg1, b"--list") || io::bytes_eq(arg1, b"-l") {
                    applets::list_applets();
                    return 0;
                }

                if io::bytes_eq(arg1, b"--help") || io::bytes_eq(arg1, b"-h") {
                    print_help();
                    return 0;
                }

                if io::bytes_eq(arg1, b"--version") || io::bytes_eq(arg1, b"-V") {
                    print_version();
                    return 0;
                }

                if io::bytes_eq(arg1, b"--install") {
                    if argc > 2 {
                        if let Some(dir) = unsafe { applets::get_arg(argv, 2) } {
                            return install_symlinks(dir);
                        }
                    }
                    io::write_str(2, b"armybox: --install requires a directory\n");
                    return 1;
                }

                // Run the specified applet
                // Shift argv: argv[1] becomes argv[0] for the applet
                return run_applet(arg1, argc - 1, unsafe { argv.offset(1) });
            }
        }

        // No arguments - print usage
        print_usage();
        return 0;
    }

    // Invoked via symlink - run the corresponding applet
    run_applet(applet_name, argc, argv)
}

/// Get basename from path
fn basename(path: &[u8]) -> &[u8] {
    let pos = path.iter().rposition(|&c| c == b'/');
    match pos {
        Some(p) => &path[p + 1..],
        None => path,
    }
}

/// Print usage information
fn print_usage() {
    io::write_str(1, b"armybox - A tiny Unix utility collection\n\n");
    io::write_str(1, b"Usage: armybox [APPLET] [ARGS...]\n");
    io::write_str(1, b"       armybox --list\n");
    io::write_str(1, b"       armybox --install DIR\n\n");
    io::write_str(1, b"Run 'armybox --list' to see available applets.\n");
}

/// Print help
fn print_help() {
    print_usage();
    io::write_str(1, b"\nOptions:\n");
    io::write_str(1, b"  -l, --list      List all available applets\n");
    io::write_str(1, b"  -h, --help      Show this help message\n");
    io::write_str(1, b"  -V, --version   Show version information\n");
    io::write_str(1, b"  --install DIR   Create symlinks in DIR for all applets\n");
}

/// Print version
fn print_version() {
    io::write_str(1, concat!("armybox ", env!("CARGO_PKG_VERSION"), "\n").as_bytes());
    io::write_str(1, b"A #[no_std] BusyBox/Toybox clone in Rust\n");
    io::write_str(1, b"Copyright (c) 2025 Joseph R. Quinn\n");
}

/// Install symlinks for all applets
fn install_symlinks(dir: &[u8]) -> i32 {
    // Get path to ourselves
    let self_path = b"/proc/self/exe";

    let mut target = [0u8; 4096];
    let mut target_buf = [0u8; 4096];
    target_buf[..self_path.len()].copy_from_slice(self_path);
    target_buf[self_path.len()] = 0;

    let n = unsafe {
        libc::readlink(
            target_buf.as_ptr() as *const libc::c_char,
            target.as_mut_ptr() as *mut libc::c_char,
            target.len() - 1
        )
    };

    if n < 0 {
        io::write_str(2, b"armybox: cannot read /proc/self/exe\n");
        return 1;
    }

    let target_path = &target[..n as usize];

    io::write_str(1, b"Installing symlinks to ");
    io::write_all(1, dir);
    io::write_str(1, b"...\n");

    let mut count = 0;

    // Get list via list_applets - for now just create basic symlinks
    let names: &[&[u8]] = &[
        b"true", b"false", b"echo", b"cat", b"ls", b"cp", b"mv", b"rm",
        b"mkdir", b"rmdir", b"pwd", b"touch", b"ln", b"chmod", b"head", b"tail",
        b"wc", b"grep", b"sed", b"awk", b"sort", b"uniq", b"cut", b"tr",
        b"date", b"uname", b"hostname", b"whoami", b"id", b"ps", b"kill", b"sleep",
    ];
    for &name in names {
        // Build link path: dir/name
        let mut link_path = [0u8; 4096];
        let mut len = 0;

        for &c in dir {
            if len < link_path.len() - 1 {
                link_path[len] = c;
                len += 1;
            }
        }

        if len > 0 && link_path[len - 1] != b'/' {
            link_path[len] = b'/';
            len += 1;
        }

        for &c in name {
            if len < link_path.len() - 1 {
                link_path[len] = c;
                len += 1;
            }
        }

        // Remove existing link
        io::unlink(&link_path[..len]);

        // Create symlink
        if io::symlink(target_path, &link_path[..len]) == 0 {
            count += 1;
        } else {
            io::write_str(2, b"armybox: failed to create ");
            io::write_all(2, &link_path[..len]);
            io::write_str(2, b"\n");
        }
    }

    io::write_str(1, b"Installed ");
    io::write_num(1, count as u64);
    io::write_str(1, b" symlinks.\n");

    0
}
