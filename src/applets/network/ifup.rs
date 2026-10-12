//! ifup - bring a network interface up
//!
//! Activate a network interface based on /etc/network/interfaces.

extern crate alloc;
use super::get_arg;
use crate::io;
use alloc::vec::Vec;

/// ifup - bring a network interface up
///
/// # Synopsis
/// ```text
/// ifup [-a] [-f] [IFACE...]
/// ```
///
/// # Description
/// Configure network interfaces according to /etc/network/interfaces.
///
/// # Options
/// - `-a`: Bring up all auto interfaces
/// - `-f`: Force configuration even if interface is already up
/// - `-v`: Verbose output
///
/// # Configuration File
/// Reads /etc/network/interfaces for interface configuration.
///
/// # Exit Status
/// - 0: Success
/// - 1: Error
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn ifup(argc: i32, argv: *const *const u8) -> i32 {
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
    let config = match parse_interfaces_file() {
        Some(c) => c,
        None => {
            if verbose {
                io::write_str(2, b"ifup: cannot read /etc/network/interfaces\n");
            }
            // Fall back to direct ifconfig
            for iface in &ifaces {
                if configure_interface_direct(iface, true, verbose) != 0 {
                    return 1;
                }
            }
            return 0;
        }
    };

    if all {
        // Bring up all auto interfaces
        for iface_config in &config {
            if iface_config.auto {
                if verbose {
                    io::write_str(1, b"Configuring ");
                    io::write_all(1, &iface_config.name);
                    io::write_str(1, b"\n");
                }
                if configure_interface(iface_config, force, verbose) != 0 {
                    io::write_str(2, b"ifup: failed to configure ");
                    io::write_all(2, &iface_config.name);
                    io::write_str(2, b"\n");
                }
            }
        }
    } else {
        if ifaces.is_empty() {
            io::write_str(2, b"ifup: specify interface or use -a\n");
            return 1;
        }

        for iface in &ifaces {
            // Find config for this interface
            let iface_config = config.iter().find(|c| c.name == *iface);

            match iface_config {
                Some(cfg) => {
                    if verbose {
                        io::write_str(1, b"Configuring ");
                        io::write_all(1, iface);
                        io::write_str(1, b"\n");
                    }
                    if configure_interface(cfg, force, verbose) != 0 {
                        return 1;
                    }
                }
                None => {
                    // No config, just bring interface up
                    if configure_interface_direct(iface, true, verbose) != 0 {
                        return 1;
                    }
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
    method: Vec<u8>, // static, dhcp, loopback, manual
    address: Option<Vec<u8>>,
    netmask: Option<Vec<u8>>,
    gateway: Option<Vec<u8>>,
    broadcast: Option<Vec<u8>>,
    pre_up: Vec<Vec<u8>>,
    up: Vec<Vec<u8>>,
    post_up: Vec<Vec<u8>>,
}

impl InterfaceConfig {
    fn new(name: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            auto: false,
            method: b"manual".to_vec(),
            address: None,
            netmask: None,
            gateway: None,
            broadcast: None,
            pre_up: Vec::new(),
            up: Vec::new(),
            post_up: Vec::new(),
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
            // auto eth0 eth1 ...
            for &iface in parts.iter().skip(1) {
                auto_ifaces.push(iface.to_vec());
            }
        } else if parts[0] == b"iface" && parts.len() >= 4 {
            // iface eth0 inet static
            // Save previous config
            if let Some(cfg) = current.take() {
                configs.push(cfg);
            }

            let name = parts[1];
            let method = parts[3];
            let mut cfg = InterfaceConfig::new(name);
            cfg.method = method.to_vec();
            cfg.auto = auto_ifaces.iter().any(|a| a == name);
            current = Some(cfg);
        } else if let Some(ref mut cfg) = current {
            // Interface options
            if parts[0] == b"address" && parts.len() >= 2 {
                cfg.address = Some(parts[1].to_vec());
            } else if parts[0] == b"netmask" && parts.len() >= 2 {
                cfg.netmask = Some(parts[1].to_vec());
            } else if parts[0] == b"gateway" && parts.len() >= 2 {
                cfg.gateway = Some(parts[1].to_vec());
            } else if parts[0] == b"broadcast" && parts.len() >= 2 {
                cfg.broadcast = Some(parts[1].to_vec());
            } else if parts[0] == b"pre-up" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.pre_up.push(cmd);
            } else if parts[0] == b"up" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.up.push(cmd);
            } else if parts[0] == b"post-up" && parts.len() >= 2 {
                let cmd = join(&parts[1..], b" ");
                cfg.post_up.push(cmd);
            }
        }
    }

    // Save last config
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
fn configure_interface(config: &InterfaceConfig, _force: bool, verbose: bool) -> i32 {
    // Run pre-up commands
    for cmd in &config.pre_up {
        if verbose {
            io::write_str(1, b"  pre-up: ");
            io::write_all(1, cmd);
            io::write_str(1, b"\n");
        }
        run_command(cmd);
    }

    let result = match config.method.as_slice() {
        b"loopback" => configure_loopback(&config.name, verbose),
        b"dhcp" => configure_dhcp(&config.name, verbose),
        b"static" => configure_static(config, verbose),
        b"manual" => {
            // Just bring interface up
            configure_interface_direct(&config.name, true, verbose)
        }
        _ => {
            io::write_str(2, b"ifup: unknown method ");
            io::write_all(2, &config.method);
            io::write_str(2, b"\n");
            1
        }
    };

    if result == 0 {
        // Run up commands
        for cmd in &config.up {
            if verbose {
                io::write_str(1, b"  up: ");
                io::write_all(1, cmd);
                io::write_str(1, b"\n");
            }
            run_command(cmd);
        }

        // Run post-up commands
        for cmd in &config.post_up {
            if verbose {
                io::write_str(1, b"  post-up: ");
                io::write_all(1, cmd);
                io::write_str(1, b"\n");
            }
            run_command(cmd);
        }
    }

    result
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn configure_loopback(iface: &[u8], verbose: bool) -> i32 {
    if verbose {
        io::write_str(1, b"  Configuring loopback\n");
    }

    // ifconfig lo 127.0.0.1 netmask 255.0.0.0 up
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return 1;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    copy_iface_name(&mut ifr.ifr_name, iface);

    // Set address
    let addr_in = create_sockaddr_in(0x0100007F); // 127.0.0.1 in network byte order
    unsafe {
        core::ptr::copy_nonoverlapping(
            &addr_in as *const _ as *const u8,
            &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut u8,
            core::mem::size_of::<libc::sockaddr_in>(),
        );
    }

    if unsafe { libc::ioctl(fd, libc::SIOCSIFADDR as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    // Set netmask 255.0.0.0
    let mask_in = create_sockaddr_in(0x000000FF);
    unsafe {
        core::ptr::copy_nonoverlapping(
            &mask_in as *const _ as *const u8,
            &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut u8,
            core::mem::size_of::<libc::sockaddr_in>(),
        );
    }

    if unsafe { libc::ioctl(fd, libc::SIOCSIFNETMASK as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    // Bring up
    if unsafe { libc::ioctl(fd, libc::SIOCGIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    unsafe { ifr.ifr_ifru.ifru_flags |= (libc::IFF_UP | libc::IFF_RUNNING) as i16 };

    if unsafe { libc::ioctl(fd, libc::SIOCSIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    io::close(fd);
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn configure_dhcp(iface: &[u8], verbose: bool) -> i32 {
    if verbose {
        io::write_str(1, b"  Running DHCP client\n");
    }

    // Try common DHCP clients
    let dhcp_clients = [
        b"udhcpc -i ".as_slice(),
        b"dhclient ".as_slice(),
        b"dhcpcd ".as_slice(),
    ];

    for client in &dhcp_clients {
        let mut cmd = client.to_vec();
        cmd.extend_from_slice(iface);

        // Check if client exists by trying to run it
        let result = run_command(&cmd);
        if result == 0 {
            return 0;
        }
    }

    io::write_str(2, b"ifup: no DHCP client found\n");
    1
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn configure_static(config: &InterfaceConfig, verbose: bool) -> i32 {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return 1;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    copy_iface_name(&mut ifr.ifr_name, &config.name);

    // Set address
    if let Some(ref addr) = config.address {
        if verbose {
            io::write_str(1, b"  Address: ");
            io::write_all(1, addr);
            io::write_str(1, b"\n");
        }

        if let Some(ip) = parse_ipv4(addr) {
            let addr_in = create_sockaddr_in(ip);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &addr_in as *const _ as *const u8,
                    &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut u8,
                    core::mem::size_of::<libc::sockaddr_in>(),
                );
            }

            if unsafe { libc::ioctl(fd, libc::SIOCSIFADDR as _, &ifr) } < 0 {
                io::close(fd);
                return 1;
            }
        }
    }

    // Set netmask
    if let Some(ref mask) = config.netmask {
        if verbose {
            io::write_str(1, b"  Netmask: ");
            io::write_all(1, mask);
            io::write_str(1, b"\n");
        }

        if let Some(ip) = parse_ipv4(mask) {
            let mask_in = create_sockaddr_in(ip);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &mask_in as *const _ as *const u8,
                    &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut u8,
                    core::mem::size_of::<libc::sockaddr_in>(),
                );
            }

            if unsafe { libc::ioctl(fd, libc::SIOCSIFNETMASK as _, &ifr) } < 0 {
                io::close(fd);
                return 1;
            }
        }
    }

    // Set broadcast
    if let Some(ref bcast) = config.broadcast {
        if let Some(ip) = parse_ipv4(bcast) {
            let bcast_in = create_sockaddr_in(ip);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &bcast_in as *const _ as *const u8,
                    &mut ifr.ifr_ifru.ifru_addr as *mut _ as *mut u8,
                    core::mem::size_of::<libc::sockaddr_in>(),
                );
            }
            unsafe { libc::ioctl(fd, libc::SIOCSIFBRDADDR as _, &ifr) };
        }
    }

    // Bring up
    if unsafe { libc::ioctl(fd, libc::SIOCGIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    unsafe { ifr.ifr_ifru.ifru_flags |= (libc::IFF_UP | libc::IFF_RUNNING) as i16 };

    if unsafe { libc::ioctl(fd, libc::SIOCSIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    io::close(fd);

    // Add default gateway
    if let Some(ref gw) = config.gateway {
        if verbose {
            io::write_str(1, b"  Gateway: ");
            io::write_all(1, gw);
            io::write_str(1, b"\n");
        }
        add_default_route(gw);
    }

    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn configure_interface_direct(iface: &[u8], up: bool, verbose: bool) -> i32 {
    if verbose {
        io::write_str(1, b"  Bringing interface ");
        if up {
            io::write_str(1, b"up\n");
        } else {
            io::write_str(1, b"down\n");
        }
    }

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return 1;
    }

    let mut ifr: libc::ifreq = unsafe { core::mem::zeroed() };
    copy_iface_name(&mut ifr.ifr_name, iface);

    if unsafe { libc::ioctl(fd, libc::SIOCGIFFLAGS as _, &ifr) } < 0 {
        io::close(fd);
        return 1;
    }

    if up {
        unsafe { ifr.ifr_ifru.ifru_flags |= (libc::IFF_UP | libc::IFF_RUNNING) as i16 };
    } else {
        unsafe { ifr.ifr_ifru.ifru_flags &= !(libc::IFF_UP as i16) };
    }

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

fn create_sockaddr_in(addr: u32) -> libc::sockaddr_in {
    let mut sa: libc::sockaddr_in = unsafe { core::mem::zeroed() };
    sa.sin_family = libc::AF_INET as u16;
    sa.sin_addr.s_addr = addr;
    sa
}

fn parse_ipv4(s: &[u8]) -> Option<u32> {
    let mut octets = [0u8; 4];
    let mut octet_idx = 0;
    let mut current = 0u32;
    let mut has_digits = false;

    for &c in s {
        if c >= b'0' && c <= b'9' {
            current = current * 10 + (c - b'0') as u32;
            if current > 255 {
                return None;
            }
            has_digits = true;
        } else if c == b'.' {
            if !has_digits || octet_idx >= 3 {
                return None;
            }
            octets[octet_idx] = current as u8;
            octet_idx += 1;
            current = 0;
            has_digits = false;
        } else {
            return None;
        }
    }

    if !has_digits || octet_idx != 3 {
        return None;
    }
    octets[3] = current as u8;

    Some(u32::from_ne_bytes(octets))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn add_default_route(gateway: &[u8]) -> i32 {
    let gw_ip = match parse_ipv4(gateway) {
        Some(ip) => ip,
        None => return 1,
    };

    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return 1;
    }

    let mut rt: crate::sys::rtentry = unsafe { core::mem::zeroed() };

    // Destination: 0.0.0.0
    let dst = create_sockaddr_in(0);
    unsafe {
        core::ptr::copy_nonoverlapping(
            &dst as *const _ as *const u8,
            &mut rt.rt_dst as *mut _ as *mut u8,
            core::mem::size_of::<libc::sockaddr_in>(),
        );
    }

    // Gateway
    let gw = create_sockaddr_in(gw_ip);
    unsafe {
        core::ptr::copy_nonoverlapping(
            &gw as *const _ as *const u8,
            &mut rt.rt_gateway as *mut _ as *mut u8,
            core::mem::size_of::<libc::sockaddr_in>(),
        );
    }

    // Netmask: 0.0.0.0
    let mask = create_sockaddr_in(0);
    unsafe {
        core::ptr::copy_nonoverlapping(
            &mask as *const _ as *const u8,
            &mut rt.rt_genmask as *mut _ as *mut u8,
            core::mem::size_of::<libc::sockaddr_in>(),
        );
    }

    rt.rt_flags = crate::sys::RTF_UP as u16 | crate::sys::RTF_GATEWAY as u16;

    let result = unsafe { libc::ioctl(fd, libc::SIOCADDRT as _, &rt) };
    io::close(fd);

    if result < 0 { 1 } else { 0 }
}

fn run_command(cmd: &[u8]) -> i32 {
    // Simple command execution via /bin/sh
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return 1;
    }

    if pid == 0 {
        // Child
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

    // Parent - wait for child
    let mut status: i32 = 0;
    unsafe { libc::waitpid(pid, &mut status, 0) };

    if status == 0 { 0 } else { 1 }
}

fn print_usage() {
    io::write_str(1, b"Usage: ifup [-a] [-f] [IFACE...]\n\n");
    io::write_str(1, b"Configure network interface.\n\n");
    io::write_str(1, b"Options:\n");
    io::write_str(1, b"  -a        Configure all auto interfaces\n");
    io::write_str(1, b"  -f        Force reconfiguration\n");
    io::write_str(1, b"  -v        Verbose output\n");
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn ifup(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"ifup: only available on Linux\n");
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
    fn test_ifup_help() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["ifup", "-h"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Usage"));
    }

    #[test]
    fn test_parse_ipv4() {
        use super::parse_ipv4;

        assert_eq!(parse_ipv4(b"192.168.1.1"), Some(0x0101A8C0));
        assert_eq!(parse_ipv4(b"127.0.0.1"), Some(0x0100007F));
        assert_eq!(parse_ipv4(b"invalid"), None);
    }
}
