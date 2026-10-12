# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `top` is now an interactive real-time process viewer instead of a one-shot
  `ps` wrapper. It is a port of BusyBox 1.36.1's `procps/top.c` for the
  configuration embedded systems ship (global CPU percentages, per-process
  %CPU, no SMP/topmem/threads screens): the `Mem:`/`CPU:`/`Load average:`
  header, the `PID PPID USER STAT VSZ %VSZ %CPU COMMAND` table, `-b`/`-n`/`-d`
  options and the `n`/`m`/`p`/`t`/`r`/`q` keys. Rendering matches BusyBox byte
  for byte, including `smart_ulltoa5()` sizes (`1225m`, `1448g`) and the STAT
  field's `W`/`<`/`N` flags.
- `top` accepts `--version` (and already handled `--help`), as CONTRIBUTING.md
  asks of applets.
- `ps`: exposes its `/proc/<pid>` parser (`Proc`) to `top`, and gained
  `Proc::args()`/`Proc::comm()` accessors. The `top` feature now selects
  `alloc` and `ps` (`top = ["alloc", "ps"]`), so `--no-default-features
  --features top` builds; it previously failed to compile.

### Fixed
- `ps`: `state` (the `S` column, `-o state`) read the first character of the
  process name instead of the state letter after `<comm>) `.
- `ps`: `args`/`COMMAND` no longer grow trailing spaces for processes that
  rewrite their `argv`: the NUL padding after the last argument is not part of
  the command line and is not converted to spaces.
- `ps`: the task name (`comm`) is truncated to 15 characters, as BusyBox's
  `COMM_LEN - 1` and procps do; some vendor kernels report longer names in
  `/proc/<pid>/stat` (e.g. a 17-character `kworker/u3:0-hci0`).

## [0.5.0] - 2026-08-18

### Added
- Android/Bionic support with full applet parity: `aarch64-linux-android` is
  now a release target shipping prebuilt `armybox` and `abpd-gen` binaries.
  New compat layer in `sys` (reboot via direct syscall, `gethostid` with
  `/etc/hostid` fallback, `rtentry`/`RTF_*`, `SCHED_OTHER`,
  `_PC_FILESIZEBITS`); 27 applets previously stubbed as "only available on
  Linux" are enabled on Android.

### Fixed
- Cross-target portability: `*const i8` casts replaced with `libc::c_char`
  (Bionic's `c_char` is `u8`), width-safe `st_mode & S_IFMT` comparisons,
  32-bit `time_t`/`c_long` fixes across 13 applets, and the ioctl request
  type on Bionic. The `aarch64-unknown-linux-musl` and
  `armv7-unknown-linux-musleabihf` release targets compile again
  (previously 173 errors).
- Android errno: use Bionic's `__errno()` instead of `__errno_location()`
  in `sys::errno()`/`clear_errno()`; five applets that read errno through
  raw libc calls now go through `sys::errno()`.

### abp package manager (0.2.1)
- Android cross-compilation fixed (`c_char`, `st_mode` widths, Bionic
  `__errno()`), closing the reported cargo-ndk build failure.
- Path helpers set `ENAMETOOLONG` on the too-long-path short circuit, so
  `mkdir_tracked` can no longer misread a stale `EEXIST` as success for a
  directory that was never created.
- New shared helpers: `io::errno`, `io::set_errno`, `io::is_dir`,
  `io::is_reg`.

## [0.4.0] - 2026-08-12

### Changed
- Reimplemented ~33 applets to POSIX conformance (date, env, id, getconf, cut,
  tr, sort, uniq, head, tail, wc, nl, paste, fold, printf, grep, sed, awk, find,
  ls, ln, cp, dd, mkfifo, xargs, expr, test/`[`, cmp, nice, nohup, renice, sleep,
  base64) — many previously ignored their arguments or were stubs.
- Consolidated the grep/sed/awk/expr regex engines into one shared engine
  (`applets::regex`), now an iterative Pike VM: linear-time matching with no
  recursion/stack-depth caps and no catastrophic backtracking.

### Fixed
- Security & data loss: `su` now authenticates against `/etc/shadow`;
  `cp`/`mv`/`truncate`/`gunzip` no longer destroy data on error; `kill` signal
  handling corrected; `tar`/`unzip`/`cpio`/`httpd` path traversal closed.
- Engine hardening: regex `{n,m}` caps, `printf` width cap; checked output
  writes and `--help` handling across the text applets.

### abp package manager (0.2.0)
- Replaced the broken hand-rolled Ed25519 with `ed25519-dalek` (RFC 8032 KATs);
  byte-exact signature verification; extraction escapes closed (symlink targets,
  `O_NOFOLLOW`, setuid mask, integrity over all member types); parse-overflow
  and OOB panics fixed; atomic install with rollback; GNU longname/pax + base-256
  tar handling; per-file content verification; 256 MiB decompression cap.

### zstd-nostd (0.2.0)
- Added `decompress_bounded` for a caller-supplied output cap.

## [0.3.0] - 2026-01-03

### Added

#### Terminal Multiplexer
- `screen` - GNU screen-compatible terminal multiplexer
- `tmux` - Alias for screen functionality
- Session management (create, list, attach, detach)
- PTY-based terminal emulation
- Ctrl+A prefix for in-session commands
- Window creation and navigation

#### Documentation & SEO
- Comprehensive documentation website at quinnjr.github.io/armybox
- Schema.org structured data for SEO/AEO
- FAQ schema with common questions
- Sitemap and robots.txt
- Open Graph and Twitter Card meta tags
- GitHub Pages deployment workflow

### Changed
- Updated applet count to **293** (from 291)
- Updated all documentation with latest statistics
- Improved comparison tables with terminal multiplexer support

### Fixed
- Minor code cleanup and organization

## [0.2.0] - 2026-01-02

### Added

#### 100% Toybox Compatibility
- All 238 Toybox commands now supported
- 53 additional utilities beyond Toybox
- Drop-in replacement for Toybox deployments

#### POSIX Compliance Audit
- Complete POSIX.1-2017 compliance for core utilities
- `sed` - Added d, p, a, i, c commands and address ranges
- `awk` - Full pattern processing implementation
- `sort` - Added -k (key field) and -t (field separator)
- `cut` - Added -b (bytes) and -c (characters)
- `cp` - Added -i (interactive) and -p (preserve)
- `rm` - Added -i (interactive)

#### Networking Stack (35+ commands)
- `wget`, `nc`/`netcat`, `ping`, `ping6`, `traceroute`, `traceroute6`
- `host`, `nslookup`, `ifconfig`, `netstat`, `route`
- `ss`, `arp`, `arping`, `ip`, `ipaddr`, `iplink`, `ipneigh`, `iproute`, `iprule`
- `nameif`, `slattach`, `vconfig`, `telnet`, `brctl`, `tunctl`, `ether-wake`
- `ifup`, `ifdown`, `tftp`, `ftpget`, `ftpput`, `ipcalc`
- `nbd-client`, `nbd-server`, `httpd`, `sntp`

#### Init System
- `init` - PID 1 system initialization
- `telinit` - Change runlevel
- `runlevel` - Print current runlevel
- `getty` - Open terminal for login
- `sulogin` - Single-user login
- `linuxrc` - Init for initramfs
- `/etc/inittab` parsing
- Runlevel management (0-6, S)

#### Vi Editor
- Full modal editing (normal, insert, command-line modes)
- Movement commands (h, j, k, l, w, b, e, 0, $, gg, G)
- Editing commands (i, a, o, O, x, dd, yy, p, P, u)
- Search and replace (/, ?, n, N, :s)
- File operations (:w, :q, :wq, :q!, :e)
- Visual selection (v, V)

#### Archive Utilities
- `bunzip2`, `bzcat`, `bzip2`
- `cpio` - Copy files to/from archives
- `gunzip`, `gzip`, `zcat`
- `tar` - Full create, extract, list support
- `unxz`, `xz`, `xzcat`
- `unzip`
- `compress`, `uncompress` - LZW compression

#### System Utilities
- `acpi`, `cal`, `top`, `blkdiscard`, `blockdev`
- `chattr`, `cksum`, `crc32`, `deallocvt`, `devmem`
- `egrep`, `eject`, `fallocate`, `fgrep`, `freeramdisk`
- `fsfreeze`, `fstype`, `getopt`
- `gpiodetect`, `gpiofind`, `gpioget`, `gpioinfo`, `gpioset`
- `hd`, `help`, `hexedit`, `httpd`, `hwclock`
- `i2cdetect`, `i2cdump`, `i2cget`, `i2cset`, `i2ctransfer`
- `iconv`, `inotifyd`, `iorenice`, `iotop`
- `killall5`, `linux32`, `login`, `lsattr`, `lspci`, `lsusb`
- `makedevs`, `mcookie`, `memeater`, `microcom`, `mix`
- `mkpasswd`, `mkswap`, `modinfo`
- `nbd-client`, `nbd-server`, `nologin`, `nsenter`
- `oneit`, `openvt`, `partprobe`, `pmap`, `prlimit`
- `pwgen`, `readelf`, `reset`, `rtcwake`, `setfattr`
- `sha1sum`, `sha224sum`, `sha256sum`, `sha384sum`, `sha3sum`, `sha512sum`
- `shuf`, `sntp`, `su`, `switch_root`, `ts`, `tsort`
- `uclampset`, `ulimit`, `unicode`, `unshare`
- `uudecode`, `uuencode`, `uuidgen`, `vmstat`
- `watch`, `watchdog`

#### APK Package Manager (Optional)
- Alpine Linux package management support
- Feature-gated with `apk` feature flag

### Changed
- Binary size reduced to **108 KB** (from 220 KB)
- Per-applet efficiency improved to **~380 bytes** (from ~1.3 KB)
- Updated GitHub repository to pegasusheavy/armybox

### Performance
- Criterion-based benchmarking suite
- Dispatch overhead benchmarks
- Throughput benchmarks for I/O-bound commands
- Text processing utility benchmarks

## [0.1.0] - 2026-01-01

### Added

#### Core Features
- Multi-call binary architecture with 163+ applets
- Applet dispatch system with O(1) static lookup
- High-performance native implementations for hot-path applets
- Cross-compilation support for musl, ARM, MIPS, RISC-V

#### Shell
- ash-compatible shell with POSIX compliance
- Pipes, redirections (`>`, `>>`, `<`, `2>`, `&>`)
- Variables (`$var`, `${var}`, `${var:-default}`)
- Control structures (`if`, `while`, `for`, `case`)
- Arithmetic expansion (`$((expr))`)
- Command substitution (`$(cmd)`, backticks)
- Background jobs (`&`) and subshells (`()`)
- 25+ shell builtins

#### Coreutils
- File operations: `cat`, `cp`, `mv`, `rm`, `mkdir`, `rmdir`, `touch`, `chmod`, `chown`, `ln`, `ls`, etc.
- Text processing: `head`, `tail`, `wc`, `sort`, `uniq`, `cut`, `paste`, `tr`, `tee`, etc.
- System info: `uname`, `whoami`, `id`, `groups`, `hostname`, `env`, `date`, etc.
- Checksums: `md5sum`, `sha1sum`, `sha256sum`, `sha512sum`, `sha3sum`

#### Text Processing
- `grep`/`egrep`/`fgrep` with regex support
- `find` with multiple predicates
- `sed` with basic commands
- `awk` interpreter

#### System Utilities
- `ps`, `pgrep`, `pkill`
- `free`, `watch`
- `mount`, `umount`
- `dmesg`, `lsmod`
- `halt`, `reboot`, `poweroff`

#### Extra Utilities
- `dd`, `hexdump`
- `diff`, `cmp`
- `chroot`, `install`
- `dos2unix`, `unix2dos`
- `rev`, `tac`
- `which`, `usleep`

### Packaging
- Static musl builds for all architectures
- Docker images (scratch and Alpine-based)
- Debian/Ubuntu .deb packages
- RPM packages for Fedora/RHEL
- Arch Linux PKGBUILD
- Alpine Linux APKBUILD
- Universal install script

### Documentation
- Comprehensive README
- Benchmark documentation
- Docker usage guide
- Packaging instructions

[Unreleased]: https://github.com/quinnjr/armybox/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/quinnjr/armybox/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/quinnjr/armybox/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/quinnjr/armybox/releases/tag/v0.1.0
