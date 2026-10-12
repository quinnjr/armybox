//! ABP Package Manager - Armybox Package Manager
//!
//! A minimal, secure package manager for armybox systems.
//!
//! # Package Format (.abp)
//! ```text
//! ┌─────────────────────────┐
//! │ Magic "ABP\x00" + ver   │  8 bytes
//! ├─────────────────────────┤
//! │ Header (offsets/sizes)  │  56 bytes
//! ├─────────────────────────┤
//! │ Metadata (TOML, XZ)     │  Variable
//! ├─────────────────────────┤
//! │ Manifest (checksums)    │  Variable
//! ├─────────────────────────┤
//! │ Payload (tar / tar.zst)  │  Variable
//! ├─────────────────────────┤
//! │ Ed25519 Signature       │  64 bytes
//! │ Key ID (SHA256 of key)  │  32 bytes
//! └─────────────────────────┘
//! ```
//!
//! # Database Layout
//! ```text
//! /var/lib/abp/packages/  # One record file per installed package
//! /var/lib/abp/files.idx  # File -> package index (append log)
//! /var/lib/abp/repos/     # Cached repository metadata
//! /var/lib/abp/available/ # Known-available package records
//! /var/cache/abp/         # Downloaded packages
//! /etc/abp/abp.conf       # Configuration
//! /etc/abp/repos.d/       # Repository definitions
//! /etc/abp/keys/          # Trusted signing keys
//! ```

#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub mod io;
pub mod util;

#[allow(dead_code)]
pub mod config;
#[allow(dead_code)]
pub mod crypto;
#[allow(dead_code)]
pub mod database;
#[allow(dead_code)]
pub mod fetch;
#[allow(dead_code)]
pub mod format;
#[allow(dead_code)]
pub mod install;
#[allow(dead_code)]
pub mod list;
pub mod makepkg;
pub mod pkgindex;
#[allow(dead_code)]
pub mod remove;
#[allow(dead_code)]
pub mod repo;
#[allow(dead_code)]
pub mod resolver;
#[allow(dead_code)]
pub mod transaction;
#[allow(dead_code)]
pub mod verify;

/// ABP command entry point
///
/// Takes a slice of argument byte slices (e.g. `[b"abp", b"add", b"pkg"]`).
///
/// # Exit Status
/// - 0: Success
/// - 1: General error
/// - 2: Usage error
#[cfg(feature = "alloc")]
pub fn run(args: &[&[u8]]) -> i32 {
    if args.len() < 2 {
        print_usage();
        return 2;
    }

    let cmd = args[1];

    match cmd {
        b"add" | b"install" => cmd_add(args),
        b"del" | b"remove" | b"rm" => cmd_del(args),
        b"update" => cmd_update(),
        b"upgrade" => cmd_upgrade(args),
        b"search" => cmd_search(args),
        b"list" | b"ls" => cmd_list(args),
        b"info" | b"show" => cmd_info(args),
        b"verify" => cmd_verify(args),
        b"repo" => cmd_repo(args),
        b"makepkg" | b"build" => makepkg::run(args),
        b"help" | b"--help" | b"-h" => {
            print_usage();
            0
        }
        b"version" | b"--version" | b"-V" => {
            print_version();
            0
        }
        _ => {
            io::write_str(2, b"abp: unknown command '");
            io::write_all(2, cmd);
            io::write_str(2, b"'\n");
            io::write_str(2, b"Run 'abp help' for usage.\n");
            2
        }
    }
}

#[cfg(feature = "alloc")]
fn cmd_add(args: &[&[u8]]) -> i32 {
    let mut packages: Vec<&[u8]> = Vec::new();
    let mut force = false;
    let mut no_deps = false;

    for &arg in &args[2..] {
        if arg == b"-f" || arg == b"--force" {
            force = true;
        } else if arg == b"--no-deps" {
            no_deps = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: abp add [options] <package>...\n\n");
            io::write_str(1, b"Install packages.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(
                1,
                b"  -f, --force   Force installation (overwrite conflicts)\n",
            );
            io::write_str(1, b"  --no-deps     Skip dependency resolution\n");
            io::write_str(1, b"  -h, --help    Show this help\n");
            return 0;
        } else if !arg.starts_with(b"-") {
            packages.push(arg);
        }
    }

    if packages.is_empty() {
        io::write_str(2, b"abp add: no packages specified\n");
        return 2;
    }

    install::install_packages(&packages, force, no_deps)
}

#[cfg(feature = "alloc")]
fn cmd_del(args: &[&[u8]]) -> i32 {
    let mut packages: Vec<&[u8]> = Vec::new();
    let mut purge = false;
    let mut force = false;

    for &arg in &args[2..] {
        if arg == b"--purge" {
            purge = true;
        } else if arg == b"-f" || arg == b"--force" {
            force = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: abp del [options] <package>...\n\n");
            io::write_str(1, b"Remove packages.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(1, b"  --purge       Also remove configuration files\n");
            io::write_str(1, b"  -f, --force   Force removal (ignore dependencies)\n");
            io::write_str(1, b"  -h, --help    Show this help\n");
            return 0;
        } else if !arg.starts_with(b"-") {
            packages.push(arg);
        }
    }

    if packages.is_empty() {
        io::write_str(2, b"abp del: no packages specified\n");
        return 2;
    }

    remove::remove_packages(&packages, purge, force)
}

fn cmd_update() -> i32 {
    repo::update_repos()
}

#[cfg(feature = "alloc")]
fn cmd_upgrade(args: &[&[u8]]) -> i32 {
    let mut dry_run = false;

    for &arg in &args[2..] {
        if arg == b"-n" || arg == b"--dry-run" {
            dry_run = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: abp upgrade [options]\n\n");
            io::write_str(1, b"Upgrade all installed packages.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(1, b"  -n, --dry-run   Show what would be upgraded\n");
            io::write_str(1, b"  -h, --help      Show this help\n");
            return 0;
        }
    }

    install::upgrade_all(dry_run)
}

fn cmd_search(args: &[&[u8]]) -> i32 {
    if args.len() < 3 {
        io::write_str(2, b"Usage: abp search <query>\n");
        return 2;
    }

    list::search_packages(args[2])
}

#[cfg(feature = "alloc")]
fn cmd_list(args: &[&[u8]]) -> i32 {
    let mut verbose = false;
    let mut available = false;

    for &arg in &args[2..] {
        if arg == b"-v" || arg == b"--verbose" {
            verbose = true;
        } else if arg == b"-a" || arg == b"--available" {
            available = true;
        } else if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: abp list [options]\n\n");
            io::write_str(1, b"List packages.\n\n");
            io::write_str(1, b"Options:\n");
            io::write_str(1, b"  -v, --verbose     Show version information\n");
            io::write_str(
                1,
                b"  -a, --available   List available (not just installed)\n",
            );
            io::write_str(1, b"  -h, --help        Show this help\n");
            return 0;
        }
    }

    if available {
        list::list_available(verbose)
    } else {
        list::list_installed(verbose)
    }
}

fn cmd_info(args: &[&[u8]]) -> i32 {
    if args.len() < 3 {
        io::write_str(2, b"Usage: abp info <package>\n");
        return 2;
    }

    list::show_info(args[2])
}

#[cfg(feature = "alloc")]
fn cmd_verify(args: &[&[u8]]) -> i32 {
    let mut package: Option<&[u8]> = None;

    for &arg in &args[2..] {
        if arg == b"-h" || arg == b"--help" {
            io::write_str(1, b"Usage: abp verify [package]\n\n");
            io::write_str(1, b"Verify package integrity.\n\n");
            io::write_str(
                1,
                b"If no package is specified, verify all installed packages.\n",
            );
            return 0;
        } else if !arg.starts_with(b"-") {
            package = Some(arg);
        }
    }

    match package {
        Some(pkg) => verify::verify_installed(&[pkg]),
        None => verify::verify_installed(&[]),
    }
}

#[cfg(feature = "alloc")]
fn cmd_repo(args: &[&[u8]]) -> i32 {
    if args.len() < 3 {
        io::write_str(2, b"Usage: abp repo <add|del|list> [args]\n");
        return 2;
    }

    let subcmd = args[2];

    match subcmd {
        b"add" => {
            if args.len() < 5 {
                io::write_str(2, b"Usage: abp repo add <url> <name>\n");
                return 2;
            }
            repo::add_repo(args[3], args[4])
        }
        b"del" | b"remove" | b"rm" => {
            if args.len() < 4 {
                io::write_str(2, b"Usage: abp repo del <name>\n");
                return 2;
            }
            repo::remove_repo(args[3])
        }
        b"list" | b"ls" => repo::list_repos(),
        _ => {
            io::write_str(2, b"abp repo: unknown subcommand '");
            io::write_all(2, subcmd);
            io::write_str(2, b"'\n");
            2
        }
    }
}

fn print_usage() {
    io::write_str(1, b"ABP Package Manager - Armybox Package Manager\n\n");
    io::write_str(1, b"Usage: abp <command> [options] [arguments]\n\n");
    io::write_str(1, b"Package Operations:\n");
    io::write_str(1, b"  add <pkg>...     Install package(s)\n");
    io::write_str(1, b"  del <pkg>...     Remove package(s)\n");
    io::write_str(1, b"  update           Refresh repository index\n");
    io::write_str(1, b"  upgrade          Upgrade all packages\n");
    io::write_str(1, b"  search <query>   Search packages\n");
    io::write_str(1, b"  list             List installed packages\n");
    io::write_str(1, b"  info <pkg>       Show package details\n");
    io::write_str(1, b"  verify           Verify checksums/signatures\n");
    io::write_str(1, b"  repo <cmd>       Manage repositories\n\n");
    io::write_str(1, b"Build:\n");
    io::write_str(1, b"  makepkg [opts]   Build package from ABUILD file\n\n");
    io::write_str(1, b"Other:\n");
    io::write_str(1, b"  help             Show this help\n");
    io::write_str(1, b"  version          Show version\n\n");
    io::write_str(
        1,
        b"Run 'abp <command> --help' for command-specific help.\n",
    );
}

fn print_version() {
    io::write_str(1, b"abp (Armybox Package Manager) 0.1.0\n");
}
