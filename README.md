# 🪖 Armybox

[![Crates.io](https://img.shields.io/crates/v/armybox)](https://crates.io/crates/armybox)
[![docs.rs](https://img.shields.io/docsrs/armybox)](https://docs.rs/armybox)
[![License](https://img.shields.io/crates/l/armybox)](LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/quinnjr/armybox/release.yml)](https://github.com/quinnjr/armybox/actions)

A memory-safe `#[no_std]` BusyBox/Toybox clone written in Rust.

**[📖 Documentation](https://quinnjr.github.io/armybox)** · **[📦 Crates.io](https://crates.io/crates/armybox)** · **[🔧 API Docs](https://docs.rs/armybox)**

## ✨ Features

- **303 applets** — 100% Toybox compatible + 65 additional utilities
- **Multi-call binary** — single executable providing all utilities
- **Pure Rust 2024** — memory-safe implementation using the latest Rust edition
- **Incredibly tiny** — **108 KB** stripped, **~54 KB** with UPX compression
- **True `#[no_std]`** — no standard library dependency, only `libc` and `alloc`
- **Android-native** — first-class Android/Bionic support, works on Android 5.0+
- **Embedded-ready** — works on systems without full std support
- **Cross-platform** — builds for Linux (glibc/musl), Android, x86_64, ARM64, ARM32
- **POSIX.1-2017 compliant** — core utilities follow the POSIX standard

## 📊 Binary Size Comparison

| Binary | Size | UPX Size | Applets | Size/Applet |
|--------|------|----------|---------|-------------|
| **Armybox** | 108 KB | ~54 KB | 303 | **~380 bytes** |
| Toybox | ~500 KB | ~200 KB | 238 | ~2.1 KB |
| BusyBox | 2.4 MB | ~1 MB | 274 | ~9 KB |

**Armybox is 24x more efficient per applet than BusyBox and 5.5x more efficient than Toybox!**

## 🚀 Quick Start

```bash
# Clone and build
git clone https://github.com/quinnjr/armybox
cd armybox
cargo build --release

# Install symlinks
sudo ./target/release/armybox --install /usr/local/bin

# Compress with UPX (optional)
upx --best target/release/armybox
```

## 📚 Applet Categories (303 total)

### File Operations (45+)
`basename`, `cat`, `cd`, `chattr`, `chgrp`, `chmod`, `chown`, `cp`, `dd`, `dirname`, `fallocate`, `file`, `find`, `fstype`, `install`, `link`, `ln`, `ls`, `lsattr`, `makedevs`, `mkdir`, `mkfifo`, `mknod`, `mktemp`, `mv`, `patch`, `pwd`, `readlink`, `realpath`, `rm`, `rmdir`, `setfattr`, `shred`, `split`, `stat`, `sync`, `touch`, `truncate`, `unlink`, `xargs`

### Text Processing (35+)
`awk`, `base32`, `base64`, `comm`, `cut`, `dos2unix`, `echo`, `egrep`, `expand`, `fgrep`, `fmt`, `fold`, `grep`, `head`, `iconv`, `nl`, `paste`, `printf`, `rev`, `sed`, `seq`, `sort`, `strings`, `tac`, `tail`, `tee`, `tr`, `tsort`, `unexpand`, `uniq`, `unix2dos`, `wc`, `yes`

### System Utilities (60+)
`acpi`, `arch`, `blkdiscard`, `blkid`, `blockdev`, `cal`, `chroot`, `chrt`, `chvt`, `date`, `deallocvt`, `devmem`, `df`, `dmesg`, `dnsdomainname`, `du`, `env`, `fgconsole`, `flock`, `free`, `freeramdisk`, `fsfreeze`, `fsync`, `getconf`, `getopt`, `groups`, `halt`, `hostid`, `hostname`, `hwclock`, `id`, `insmod`, `ionice`, `iorenice`, `iotop`, `linux32`, `logger`, `logname`, `losetup`, `lsmod`, `lspci`, `lsusb`, `modinfo`, `modprobe`, `mount`, `mountpoint`, `nice`, `nohup`, `nproc`, `openvt`, `partprobe`, `pivot_root`, `poweroff`, `printenv`, `reboot`, `readahead`, `renice`, `rfkill`, `rmmod`, `rtcwake`, `swapoff`, `swapon`, `sysctl`, `taskset`, `timeout`, `top`, `tty`, `umount`, `uname`, `uptime`, `users`, `vmstat`, `w`, `watch`, `who`, `whoami`

### Process Management (15+)
`kill`, `killall`, `killall5`, `pgrep`, `pidof`, `pkill`, `pmap`, `prlimit`, `ps`, `pwdx`, `renice`, `setsid`, `time`

### Shell (3)
`sh`, `ash`, `dash` — full POSIX-compliant shell

### Vi Editor (2)
`vi`, `view` — modal editor with normal, insert, and command-line modes

### Terminal Multiplexer (2)
`screen`, `tmux` — terminal session management

### Init System (6)
`init`, `getty`, `linuxrc`, `runlevel`, `sulogin`, `telinit`

### Networking (35+)
`arp`, `arping`, `brctl`, `ftpget`, `ftpput`, `host`, `httpd`, `ifconfig`, `ifdown`, `ifup`, `ip`, `ipaddr`, `ipcalc`, `iplink`, `ipneigh`, `iproute`, `iprule`, `nameif`, `nbd-client`, `nbd-server`, `nc`, `netcat`, `netstat`, `nslookup`, `ping`, `ping6`, `route`, `slattach`, `sntp`, `ss`, `telnet`, `tftp`, `traceroute`, `traceroute6`, `tunctl`, `wget`

### Archive & Compression (15)
`bunzip2`, `bzcat`, `bzip2`, `compress`, `cpio`, `gunzip`, `gzip`, `tar`, `uncompress`, `unxz`, `unzip`, `xz`, `xzcat`, `zcat`

### Checksums & Crypto (10)
`cksum`, `crc32`, `md5sum`, `sha1sum`, `sha224sum`, `sha256sum`, `sha384sum`, `sha3sum`, `sha512sum`

### Hardware & GPIO (15+)
`devmem`, `gpiodetect`, `gpiofind`, `gpioget`, `gpioinfo`, `gpioset`, `i2cdetect`, `i2cdump`, `i2cget`, `i2cset`, `i2ctransfer`, `lspci`, `lsusb`

### Miscellaneous (25+)
`[`, `ascii`, `clear`, `cmp`, `count`, `diff`, `expr`, `factor`, `false`, `help`, `hexdump`, `hexedit`, `mcookie`, `memeater`, `mesg`, `microcom`, `mix`, `mkpasswd`, `mkswap`, `nologin`, `nsenter`, `oneit`, `pwgen`, `readelf`, `reset`, `shuf`, `sleep`, `switch_root`, `test`, `toybox`, `true`, `ts`, `uclampset`, `ulimit`, `unicode`, `unshare`, `usleep`, `uudecode`, `uuencode`, `uuidgen`, `watchdog`

## 📱 Android Support

Armybox has native Android support with Bionic libc compatibility.

```bash
# Install Android targets
rustup target add aarch64-linux-android    # ARM64 (most modern devices)
rustup target add armv7-linux-androideabi  # ARMv7 (older 32-bit)
rustup target add x86_64-linux-android     # x86_64 (emulator/Chromebook)

# Set up Android NDK (required)
export ANDROID_NDK_HOME=/path/to/android-ndk
export PATH=$PATH:$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin

# Build for Android ARM64
cargo build --release --target aarch64-linux-android

# Deploy via ADB
adb push target/aarch64-linux-android/release/armybox /data/local/tmp/
adb shell chmod +x /data/local/tmp/armybox
adb shell /data/local/tmp/armybox --list
```

## 📦 Usage

### Direct Invocation
```bash
./armybox ls -la
./armybox cat file.txt
./armybox echo "Hello, World!"
./armybox sh  # Start POSIX shell
./armybox vi file.txt  # Edit with vi
./armybox screen  # Terminal multiplexer
```

### List Available Applets
```bash
./armybox --list
```

### Via Symlinks
After installing symlinks:
```bash
ls -la
cat file.txt
echo "Hello!"
```

## 📚 Library Usage

Armybox is a `#[no_std]` library that can be used in embedded environments.

### Cargo.toml

```toml
[dependencies]
armybox = { version = "0.3", default-features = false, features = ["alloc"] }
```

### Features

| Feature | Description | Default |
|---------|-------------|---------|
| `alloc` | Heap allocation (Vec, String) | ✅ |
| `std` | Standard library support | ❌ |
| `apk` | APK package manager support | ❌ |

### Example

```rust
#![no_std]
extern crate alloc;

use armybox::applets;

// Find and run an applet
if let Some(func) = applets::find_applet(b"echo") {
    let args = [b"echo\0".as_ptr(), b"hello\0".as_ptr()];
    func(2, args.as_ptr());
}
```

## 🏗️ Architecture

```
src/
├── lib.rs          # Library entry (no_std compatible)
├── main.rs         # Binary entry (no_std, no_main)
├── io.rs           # Raw I/O via libc
├── sys.rs          # System utilities
└── applets/
    ├── mod.rs      # Applet registry (303 applets)
    ├── file.rs     # File operations
    ├── text.rs     # Text processing
    ├── system.rs   # System utilities
    ├── misc.rs     # Miscellaneous
    ├── network.rs  # Networking
    ├── archive.rs  # Archive/compression
    ├── init.rs     # Init system
    ├── shell.rs    # POSIX shell
    ├── vi.rs       # Vi editor
    └── screen.rs   # Terminal multiplexer
```

## 🐳 Docker

Use armybox as a minimal base for containers. Perfect for FROM scratch images.

```dockerfile
FROM scratch
COPY target/release/armybox /bin/armybox
RUN ["/bin/armybox", "--install", "/bin"]
ENTRYPOINT ["/bin/sh"]
```

Build: `docker build -t myapp .` → ~100KB image!

## 🔍 How It Works

Armybox is built entirely with `#[no_std]`:

1. **No Rust Standard Library** — Only uses `core` and `alloc`
2. **Direct libc Calls** — All I/O goes through raw `libc::*` functions
3. **Custom Allocator** — Uses `libc::malloc/free` for heap allocation
4. **Custom Panic Handler** — Minimal panic handling without unwinding
5. **No Main Runtime** — Uses `#[no_main]` with raw C entry point

This results in an incredibly small binary that's perfect for:
- Embedded systems
- Containers (FROM scratch)
- Rescue environments
- Space-constrained systems
- Android devices

## 📋 Comparison

| Feature | Armybox | Toybox | BusyBox |
|---------|---------|--------|---------|
| Language | Rust | C | C |
| Memory Safety | ✅ Compile-time | ❌ Manual | ❌ Manual |
| Binary Size | **108 KB** | ~500 KB | 2.4 MB |
| Per-Applet Size | **~380 bytes** | ~2.1 KB | ~9 KB |
| `#[no_std]` | ✅ | N/A | N/A |
| Applet Count | **303** | 238 | 274 |
| Toybox Compatible | ✅ 100% | N/A | Partial |
| POSIX Shell | ✅ | ✅ | ✅ |
| Vi Editor | ✅ | ✅ | ✅ |
| Init System | ✅ | Partial | ✅ |
| Terminal Multiplexer | ✅ | ❌ | ❌ |
| License | MIT/Apache-2.0 | 0BSD | GPL v2 |

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🙏 Acknowledgments

- [Toybox](https://landley.net/toybox/) — BSD-licensed Unix utilities
- [BusyBox](https://busybox.net/) — The original multi-call binary inspiration
- [libc](https://github.com/rust-lang/libc) — Rust FFI bindings to platform libraries

---

**Made with ❤️ by [Joseph R. Quinn](https://github.com/quinnjr)**
