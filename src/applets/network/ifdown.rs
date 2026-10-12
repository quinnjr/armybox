//! ifdown - bring a network interface down
//!
//! Deactivate a network interface.

extern crate alloc;
use super::get_arg;
use crate::io;
use alloc::vec::Vec;

/// ifdown - bring a network interface down
///
/// # Synopsis
/// ```text
/// ifdown [-a] [-f] [IFACE...]
/// ```
///
/// # Description
/// Deconfigure network interfaces according to /etc/network/interfaces.
///
/// # Options
/// - `-a`: Bring down all auto interfaces
/// - `-f`: Force deconfiguration even if interface is already down
/// - `-v`: Verbose output
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn ifdown(argc: i32, argv: *const *const u8) -> i32 {
    let mut all = false;
    let mut force = false;
    let mut verbose = false;
    let mut ifaces: Vec<&[u8]> = Vec::new();

    // Parse arguments
    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg == b"-a" {
                all = true;
            } else if arg == b"-f" || arg == b"--force" {
                force = true;
            } else if arg == b"-v" || arg == b"--verbose" {
                verbose = true;
            } else if arg == b"-h" || arg == b"--help" {
                print_usage();
                return 0;
            } else if !arg.starts_with(b"-") {
                ifaces.push(arg);
            }
        }
    }

    // Parse interfaces file
    let config = parse_interfaces_file();

    if all {
        // Bring down all auto interfaces
        if let Some(ref configs) = config {
            for iface_config in configs.iter().rev() {
                if iface_config.auto {
                    if verbose {
                        io::write_str(1, b"Deconfiguring ");
                        io::write_all(1, &iface_config.name);
                        io::write_str(1, b"\n");
                    }
                    if deconfigure_interface(iface_config, force, verbose) != 0 {
                        io::write_str(2, b"ifdown: failed to deconfigure ");
                        io::write_all(2, &iface_config.name);
                        io::write_str(2, b"\n");
                    }
                }
            }
        }
        return 0;
    }

    if ifaces.is_empty() {
        io::write_str(2, b"ifdown: specify interface or use -a\n");
        return 1;
    }

    for iface in &ifaces {
        // Find config for this interface
        let iface_config = config
            .as_ref()
            .and_then(|c| c.iter().find(|cfg| cfg.name == *iface));

        match iface_config {
            Some(cfg) => {
                if verbose {
                    io::write_str(1, b"Deconfiguring ");
                    io::write_all(1, iface);
                    io::write_str(1, b"\n");
                }
                if deconfigure_interface(cfg, force, verbose) != 0 {
                    return 1;
                }
            }
            None => {
                // No config, just bring interface down
                if deconfigure_interface_direct(iface, verbose) != 0 {
                    return 1;
                }
            }
        }
    }

    0
}

#[derive(Clone)]
struct InterfaceConfig {
    name: Vec<u8>,
    auto: bool,
    pre_down: Vec<Vec<u8>>,
    down: Vec<Vec<u8>>,
    post_down: Vec<Vec<u8>>,
}

impl InterfaceConfig {
    fn new(name: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            auto: false,
            pre_down: Vec::new(),
            down: Vec::new(),
            post_down: Vec::new(),
        }
    }
}

fn parse_interfaces_file() -> Option<Vec<InterfaceConfig>> {
    let fd = io::open(b"/etc/network/interfaces", libc::O_RDONLY, 0);
    if fd < 0 {
        return None;
    }

    let content = io::read_all(fd);
    io::close(fd);

    let mut configs: Vec<InterfaceConfig> = Vec::new();
    let mut auto_ifaces: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<InterfaceConfig> = None;

    for line in content.split(|&c| c == b'\n') {
        let line = trim(line);
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }

        let parts: Vec<&[u8]> = line
            .split(|&c| c == b' ' || c == b'\t')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.is_empty() {
            continue;
        }

        if parts[0] == b"auto" {
            for &iface in parts.iter().skip(1) {
                auto_ifaces.push(iface.to_vec());
            }
        } else if parts[0] == b"iface" && parts.len() >= 4 {
            if let Some(cfg) = current.take() {
                configs.push(cfg);
            }

            let name = parts[1];
            let mut cfg = InterfaceConfig::new(name);
            cfg.auto = auto_ifaces.iter().any(|a| a == name);
            current = Some(cfg);
        } else if let Some(ref mut cfg) = current {
            if parts[0] == b"pre-down" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.pre_down.push(cmd);
            } else if parts[0] == b"down" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.down.push(cmd);
            } else if parts[0] == b"post-down" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.post_down.push(cmd);
            }
        }
    }

    if let Some(cfg) = current {
        configs.push(cfg);
    }

    Some(configs)
}

fn trim(s: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = s.len();
    while start < end && (s[start] == b' ' || s[start] == b'\t') {
        start += 1;
    }
    while end > start && (s[end - 1] == b' ' || s[end - 1] == b'\t' || s[end - 1] == b'\r') {
        end -= 1;
    }
    &s[start..end]
}

fn join(parts: &[&[u8]], sep: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            result.extend_from_slice(sep);
        }
        result.extend_from_slice(part);
    }
    result
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn deconfigure_interface(config: &InterfaceConfig, _force: bool, verbose: bool) -> i32 {
    // Run pre-down commands
    for cmd in &config.pre_down {
        if verbose {
            io::write_str(1, b"  pre-down: ");
            io::write_all(1, cmd);
            io::write_str(1, b"\n");
        }
        run_command(cmd);
    }

    // Bring interface down
    let result = deconfigure_interface_direct(&config.name, verbose);

    // Run down commands
    for cmd in &config.down {
        if verbose {
            io::write_str(1, b"  down: ");
            io::write_all(1, cmd);
            io::write_str(1, b"\n");
        }
        run_command(cmd);
    }

    // Run post-down commands
    for cmd in &config.post_down {
        if verbose {
            io::write_str(1, b"  post-down: ");
            io::write_all(1, cmd);
            io::write_str(1, b"\n");
        }
        run_command(cmd);
    }

    result
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn deconfigure_interface_direct(iface: &[u8], verbose: bool) -> i32 {
    if verbose {
        io::write_str(1, b"  Bringing interface down\n");
    }

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return 1;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    copy_iface_name(&mut ifr.ifr_name, iface);

    // Get current flags
    if unsafe { libc::ioctl(fd, libc::SIOCGIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    // Clear UP flag
    unsafe { ifr.ifr_ifru.ifru_flags &= !(libc::IFF_UP as i16) };

    let result = if unsafe { libc::ioctl(fd, libc::SIOCSIFFLAGS as _, &ifr) } < 0 {
        1
    } else {
        0
    };

    io::close(fd);
    result
}

fn copy_iface_name(dest: &mut [libc::c_char; 16], src: &[u8]) {
    let len = src.len().min(15);
    for i in 0..len {
        dest[i] = src[i] as libc::c_char;
    }
    dest[len] = 0;
}

fn run_command(cmd: &[u8]) -> i32 {
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return 1;
    }

    if pid == 0 {
        let mut cmd_cstr = [0u8; 1024];
        let len = cmd.len().min(1023);
        cmd_cstr[..len].copy_from_slice(&cmd[..len]);
        cmd_cstr[len] = 0;

        let sh = b"/bin/sh\0";
        let c_flag = b"-c\0";

        let argv: [*const libc::c_char; 4] = [
            sh.as_ptr() as *const libc::c_char,
            c_flag.as_ptr() as *const libc::c_char,
            cmd_cstr.as_ptr() as *const libc::c_char,
            core::ptr::null(),
        ];

        unsafe {
            libc::execv(sh.as_ptr() as *const libc::c_char, argv.as_ptr());
            libc::_exit(127);
        }
    }

    let mut status: i32 = 0;
    unsafe { libc::waitpid(pid, &mut status, 0) };

    if status == 0 { 0 } else { 1 }
}

fn print_usage() {
    io::write_str(1, b"Usage: ifdown [-a] [-f] [IFACE...]\n\n");
    io::write_str(1, b"Deconfigure network interface.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -a        Deconfigure all auto interfaces\n");
    io::write_str(1, b"  -f        Force deconfiguration\n");
    io::write_str(1, b"  -v        Verbose output\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ifdown(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"ifdown: only available on Linux\n");
    1
}

#[cfg(test)]
mod tests {
    extern crate std;
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
    fn test_ifdown_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ifdown", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }
}
