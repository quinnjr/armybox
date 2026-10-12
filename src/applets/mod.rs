//! Native no_std applet implementations
//!
//! All applets implemented using libc directly, no std required.

#[cfg(any(
    feature = "cat",
    feature = "cp",
    feature = "mv",
    feature = "rm",
    feature = "mkdir",
    feature = "rmdir",
    feature = "touch",
    feature = "ln",
    feature = "ls",
    feature = "pwd",
    feature = "chmod",
    feature = "chown",
    feature = "chgrp",
    feature = "stat",
    feature = "readlink",
    feature = "realpath",
    feature = "basename",
    feature = "dirname",
    feature = "sync",
    feature = "link",
    feature = "unlink",
    feature = "dd",
    feature = "mktemp",
    feature = "mkfifo",
    feature = "mknod",
    feature = "split",
    feature = "install",
    feature = "truncate",
    feature = "shred",
    feature = "file",
    feature = "xargs",
    feature = "patch",
    feature = "find",
    feature = "cd",
    feature = "chattr",
    feature = "lsattr",
    feature = "fstype",
    feature = "makedevs",
    feature = "setfattr"
))]
mod file;

#[cfg(any(
    feature = "echo",
    feature = "printf",
    feature = "head",
    feature = "tail",
    feature = "wc",
    feature = "tee",
    feature = "tac",
    feature = "rev",
    feature = "yes",
    feature = "seq",
    feature = "nl",
    feature = "tr",
    feature = "cut",
    feature = "paste",
    feature = "sort",
    feature = "uniq",
    feature = "grep",
    feature = "sed",
    feature = "awk",
    feature = "comm",
    feature = "expand",
    feature = "unexpand",
    feature = "fold",
    feature = "fmt",
    feature = "strings",
    feature = "dos2unix",
    feature = "unix2dos"
))]
mod text;

// Shared BRE/ERE regex engine used by grep, sed, awk (text/) and expr (misc/).
#[cfg(any(feature = "grep", feature = "sed", feature = "awk", feature = "expr"))]
pub(crate) mod regex;

#[cfg(any(
    feature = "uname",
    feature = "hostname",
    feature = "whoami",
    feature = "id",
    feature = "groups",
    feature = "who",
    feature = "w",
    feature = "users",
    feature = "arch",
    feature = "date",
    feature = "env",
    feature = "printenv",
    feature = "tty",
    feature = "sleep",
    feature = "usleep",
    feature = "uptime",
    feature = "free",
    feature = "df",
    feature = "du",
    feature = "mount",
    feature = "umount",
    feature = "mountpoint",
    feature = "dmesg",
    feature = "halt",
    feature = "reboot",
    feature = "poweroff",
    feature = "chroot",
    feature = "logname",
    feature = "logger",
    feature = "dnsdomainname",
    feature = "hostid",
    feature = "nproc",
    feature = "fgconsole",
    feature = "chvt",
    feature = "flock",
    feature = "fsync",
    feature = "sysctl",
    feature = "swapoff",
    feature = "swapon",
    feature = "blkid",
    feature = "losetup",
    feature = "insmod",
    feature = "rmmod",
    feature = "modprobe",
    feature = "lsmod",
    feature = "pivot_root",
    feature = "readahead",
    feature = "rfkill",
    feature = "acpi",
    feature = "cal",
    feature = "vmstat",
    feature = "watch",
    feature = "hwclock",
    feature = "fallocate",
    feature = "shuf",
    feature = "mkswap",
    feature = "nologin",
    feature = "su",
    feature = "login",
    feature = "eject",
    feature = "blockdev",
    feature = "rtcwake",
    feature = "ulimit",
    feature = "blkdiscard",
    feature = "deallocvt",
    feature = "devmem",
    feature = "freeramdisk",
    feature = "fsfreeze",
    feature = "gpiodetect",
    feature = "gpiofind",
    feature = "gpioget",
    feature = "gpioinfo",
    feature = "gpioset",
    feature = "i2cdetect",
    feature = "i2cdump",
    feature = "i2cget",
    feature = "i2cset",
    feature = "i2ctransfer",
    feature = "inotifyd",
    feature = "linux32",
    feature = "lspci",
    feature = "lsusb",
    feature = "modinfo",
    feature = "openvt",
    feature = "partprobe"
))]
mod system;

#[cfg(any(
    feature = "kill",
    feature = "killall",
    feature = "killall5",
    feature = "ps",
    feature = "pgrep",
    feature = "pkill",
    feature = "pidof",
    feature = "pwdx",
    feature = "nice",
    feature = "renice",
    feature = "nohup",
    feature = "setsid",
    feature = "timeout",
    feature = "taskset",
    feature = "ionice",
    feature = "chrt",
    feature = "top",
    feature = "nsenter",
    feature = "unshare",
    feature = "pmap",
    feature = "prlimit",
    feature = "uclampset",
    feature = "iorenice",
    feature = "iotop"
))]
mod process;

#[cfg(any(
    feature = "boolean",
    feature = "test",
    feature = "which",
    feature = "expr",
    feature = "time",
    feature = "mesg",
    feature = "getconf",
    feature = "factor",
    feature = "base64",
    feature = "cmp",
    feature = "diff",
    feature = "hexdump",
    feature = "hash",
    feature = "ascii",
    feature = "iconv",
    feature = "tsort",
    feature = "getopt",
    feature = "count",
    feature = "unicode",
    feature = "ts",
    feature = "uuidgen",
    feature = "mcookie",
    feature = "pwgen",
    feature = "uuencode",
    feature = "uudecode",
    feature = "help",
    feature = "memeater",
    feature = "mix",
    feature = "mkpasswd",
    feature = "toybox",
    feature = "readelf",
    feature = "screen",
    feature = "terminal"
))]
mod misc;

#[cfg(any(
    feature = "wget",
    feature = "nc",
    feature = "ping",
    feature = "traceroute",
    feature = "host",
    feature = "nslookup",
    feature = "ifconfig",
    feature = "netstat",
    feature = "route",
    feature = "tftp",
    feature = "ftpget",
    feature = "ftpput",
    feature = "ipcalc",
    feature = "brctl",
    feature = "tunctl",
    feature = "ether_wake",
    feature = "ifup",
    feature = "ifdown",
    feature = "ss",
    feature = "arp",
    feature = "arping",
    feature = "ip",
    feature = "ipaddr",
    feature = "iplink",
    feature = "ipneigh",
    feature = "iproute",
    feature = "iprule",
    feature = "nameif",
    feature = "slattach",
    feature = "vconfig",
    feature = "telnet",
    feature = "httpd",
    feature = "sntp",
    feature = "microcom",
    feature = "nbd_client",
    feature = "nbd_server"
))]
mod network;

#[cfg(any(
    feature = "tar",
    feature = "gzip",
    feature = "bzip2",
    feature = "xz",
    feature = "cpio",
    feature = "unzip",
    feature = "compress",
    feature = "zstd"
))]
mod archive;

#[cfg(any(feature = "vi", feature = "hexedit"))]
mod editors;

#[cfg(any(
    feature = "init",
    feature = "telinit",
    feature = "runlevel",
    feature = "getty",
    feature = "sulogin",
    feature = "oneit",
    feature = "switch_root",
    feature = "watchdog"
))]
mod init;

#[cfg(feature = "sh")]
mod shell;

#[cfg(any(feature = "apk", feature = "abp"))]
mod package;

use crate::io;

/// Total number of applets
pub const APPLET_COUNT: usize = 303;

/// Get argument as byte slice
#[inline]
pub unsafe fn get_arg(argv: *const *const u8, idx: i32) -> Option<&'static [u8]> {
    if argv.is_null() {
        return None;
    }
    // SAFETY: Caller guarantees argv is valid and contains at least idx+1 elements
    let ptr = unsafe { *argv.add(idx as usize) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: ptr is a valid C string from argv
    Some(unsafe { io::cstr_to_slice(ptr) })
}

/// Check if argument has option char
#[inline]
pub fn has_opt(arg: &[u8], opt: u8) -> bool {
    if arg.len() < 2 || arg[0] != b'-' {
        return false;
    }
    arg[1..].contains(&opt)
}

/// Check if argument is exactly this option
#[inline]
pub fn is_opt(arg: &[u8], opt: u8) -> bool {
    arg.len() == 2 && arg[0] == b'-' && arg[1] == opt
}

/// Check if argument looks like an option
#[inline]
pub fn is_option(arg: &[u8]) -> bool {
    arg.len() >= 2 && arg[0] == b'-'
}

/// Find an applet function by name
pub fn find_applet(name: &[u8]) -> Option<fn(i32, *const *const u8) -> i32> {
    // File utilities
    #[cfg(feature = "cat")]
    if name == b"cat" {
        return Some(file::cat);
    }
    #[cfg(feature = "cp")]
    if name == b"cp" {
        return Some(file::cp);
    }
    #[cfg(feature = "mv")]
    if name == b"mv" {
        return Some(file::mv);
    }
    #[cfg(feature = "rm")]
    if name == b"rm" {
        return Some(file::rm);
    }
    #[cfg(feature = "mkdir")]
    if name == b"mkdir" {
        return Some(file::mkdir);
    }
    #[cfg(feature = "rmdir")]
    if name == b"rmdir" {
        return Some(file::rmdir);
    }
    #[cfg(feature = "touch")]
    if name == b"touch" {
        return Some(file::touch);
    }
    #[cfg(feature = "ln")]
    if name == b"ln" {
        return Some(file::ln);
    }
    #[cfg(feature = "ls")]
    if name == b"ls" {
        return Some(file::ls);
    }
    #[cfg(feature = "pwd")]
    if name == b"pwd" {
        return Some(file::pwd);
    }
    #[cfg(feature = "chmod")]
    if name == b"chmod" {
        return Some(file::chmod);
    }
    #[cfg(feature = "chown")]
    if name == b"chown" {
        return Some(file::chown);
    }
    #[cfg(feature = "chgrp")]
    if name == b"chgrp" {
        return Some(file::chgrp);
    }
    #[cfg(feature = "stat")]
    if name == b"stat" {
        return Some(file::stat);
    }
    #[cfg(feature = "readlink")]
    if name == b"readlink" {
        return Some(file::readlink);
    }
    #[cfg(feature = "realpath")]
    if name == b"realpath" {
        return Some(file::realpath);
    }
    #[cfg(feature = "basename")]
    if name == b"basename" {
        return Some(file::basename);
    }
    #[cfg(feature = "dirname")]
    if name == b"dirname" {
        return Some(file::dirname);
    }
    #[cfg(feature = "sync")]
    if name == b"sync" {
        return Some(file::sync_cmd);
    }
    #[cfg(feature = "link")]
    if name == b"link" {
        return Some(file::link);
    }
    #[cfg(feature = "unlink")]
    if name == b"unlink" {
        return Some(file::unlink);
    }
    #[cfg(feature = "dd")]
    if name == b"dd" {
        return Some(file::dd);
    }
    #[cfg(feature = "mktemp")]
    if name == b"mktemp" {
        return Some(file::mktemp);
    }
    #[cfg(feature = "mkfifo")]
    if name == b"mkfifo" {
        return Some(file::mkfifo);
    }
    #[cfg(feature = "mknod")]
    if name == b"mknod" {
        return Some(file::mknod);
    }
    #[cfg(feature = "split")]
    if name == b"split" {
        return Some(file::split);
    }
    #[cfg(feature = "install")]
    if name == b"install" {
        return Some(file::install);
    }
    #[cfg(feature = "truncate")]
    if name == b"truncate" {
        return Some(file::truncate);
    }
    #[cfg(feature = "shred")]
    if name == b"shred" {
        return Some(file::shred);
    }
    #[cfg(feature = "file")]
    if name == b"file" {
        return Some(file::file);
    }
    #[cfg(feature = "xargs")]
    if name == b"xargs" {
        return Some(file::xargs);
    }
    #[cfg(feature = "patch")]
    if name == b"patch" {
        return Some(file::patch);
    }
    #[cfg(feature = "find")]
    if name == b"find" {
        return Some(file::find);
    }
    #[cfg(feature = "cd")]
    if name == b"cd" {
        return Some(file::cd);
    }
    #[cfg(feature = "chattr")]
    if name == b"chattr" {
        return Some(file::chattr);
    }
    #[cfg(feature = "lsattr")]
    if name == b"lsattr" {
        return Some(file::lsattr);
    }
    #[cfg(feature = "fstype")]
    if name == b"fstype" {
        return Some(file::fstype);
    }
    #[cfg(feature = "makedevs")]
    if name == b"makedevs" {
        return Some(file::makedevs);
    }
    #[cfg(feature = "setfattr")]
    if name == b"setfattr" {
        return Some(file::setfattr);
    }

    // Text utilities
    #[cfg(feature = "echo")]
    if name == b"echo" {
        return Some(text::echo);
    }
    #[cfg(feature = "printf")]
    if name == b"printf" {
        return Some(text::printf);
    }
    #[cfg(feature = "head")]
    if name == b"head" {
        return Some(text::head);
    }
    #[cfg(feature = "tail")]
    if name == b"tail" {
        return Some(text::tail);
    }
    #[cfg(feature = "wc")]
    if name == b"wc" {
        return Some(text::wc);
    }
    #[cfg(feature = "tee")]
    if name == b"tee" {
        return Some(text::tee);
    }
    #[cfg(feature = "tac")]
    if name == b"tac" {
        return Some(text::tac);
    }
    #[cfg(feature = "rev")]
    if name == b"rev" {
        return Some(text::rev);
    }
    #[cfg(feature = "yes")]
    if name == b"yes" {
        return Some(text::yes);
    }
    #[cfg(feature = "seq")]
    if name == b"seq" {
        return Some(text::seq);
    }
    #[cfg(feature = "nl")]
    if name == b"nl" {
        return Some(text::nl);
    }
    #[cfg(feature = "tr")]
    if name == b"tr" {
        return Some(text::tr);
    }
    #[cfg(feature = "cut")]
    if name == b"cut" {
        return Some(text::cut);
    }
    #[cfg(feature = "paste")]
    if name == b"paste" {
        return Some(text::paste);
    }
    #[cfg(feature = "sort")]
    if name == b"sort" {
        return Some(text::sort);
    }
    #[cfg(feature = "uniq")]
    if name == b"uniq" {
        return Some(text::uniq);
    }
    #[cfg(feature = "grep")]
    if name == b"grep" {
        return Some(text::grep);
    }
    #[cfg(feature = "grep")]
    if name == b"egrep" {
        return Some(text::egrep);
    }
    #[cfg(feature = "grep")]
    if name == b"fgrep" {
        return Some(text::fgrep);
    }
    #[cfg(feature = "sed")]
    if name == b"sed" {
        return Some(text::sed);
    }
    #[cfg(feature = "awk")]
    if name == b"awk" {
        return Some(text::awk);
    }
    #[cfg(feature = "comm")]
    if name == b"comm" {
        return Some(text::comm);
    }
    #[cfg(feature = "expand")]
    if name == b"expand" {
        return Some(text::expand);
    }
    #[cfg(feature = "unexpand")]
    if name == b"unexpand" {
        return Some(text::unexpand);
    }
    #[cfg(feature = "fold")]
    if name == b"fold" {
        return Some(text::fold);
    }
    #[cfg(feature = "fmt")]
    if name == b"fmt" {
        return Some(text::fmt);
    }
    #[cfg(feature = "strings")]
    if name == b"strings" {
        return Some(text::strings);
    }
    #[cfg(feature = "dos2unix")]
    if name == b"dos2unix" {
        return Some(text::dos2unix);
    }
    #[cfg(feature = "unix2dos")]
    if name == b"unix2dos" {
        return Some(text::unix2dos);
    }

    // System utilities
    #[cfg(feature = "uname")]
    if name == b"uname" {
        return Some(system::uname);
    }
    #[cfg(feature = "hostname")]
    if name == b"hostname" {
        return Some(system::hostname);
    }
    #[cfg(feature = "whoami")]
    if name == b"whoami" {
        return Some(system::whoami);
    }
    #[cfg(feature = "id")]
    if name == b"id" {
        return Some(system::id);
    }
    #[cfg(feature = "groups")]
    if name == b"groups" {
        return Some(system::groups);
    }
    #[cfg(feature = "who")]
    if name == b"who" {
        return Some(system::who);
    }
    #[cfg(feature = "w")]
    if name == b"w" {
        return Some(system::w);
    }
    #[cfg(feature = "users")]
    if name == b"users" {
        return Some(system::users);
    }
    #[cfg(feature = "arch")]
    if name == b"arch" {
        return Some(system::arch);
    }
    #[cfg(feature = "date")]
    if name == b"date" {
        return Some(system::date);
    }
    #[cfg(feature = "env")]
    if name == b"env" {
        return Some(system::env);
    }
    #[cfg(feature = "printenv")]
    if name == b"printenv" {
        return Some(system::printenv);
    }
    #[cfg(feature = "tty")]
    if name == b"tty" {
        return Some(system::tty);
    }
    #[cfg(feature = "sleep")]
    if name == b"sleep" {
        return Some(system::sleep);
    }
    #[cfg(feature = "usleep")]
    if name == b"usleep" {
        return Some(system::usleep);
    }
    #[cfg(feature = "uptime")]
    if name == b"uptime" {
        return Some(system::uptime);
    }
    #[cfg(feature = "free")]
    if name == b"free" {
        return Some(system::free);
    }
    #[cfg(feature = "df")]
    if name == b"df" {
        return Some(system::df);
    }
    #[cfg(feature = "du")]
    if name == b"du" {
        return Some(system::du);
    }
    #[cfg(feature = "mount")]
    if name == b"mount" {
        return Some(system::mount);
    }
    #[cfg(feature = "umount")]
    if name == b"umount" {
        return Some(system::umount);
    }
    #[cfg(feature = "mountpoint")]
    if name == b"mountpoint" {
        return Some(system::mountpoint);
    }
    #[cfg(feature = "dmesg")]
    if name == b"dmesg" {
        return Some(system::dmesg);
    }
    #[cfg(feature = "halt")]
    if name == b"halt" {
        return Some(system::halt);
    }
    #[cfg(feature = "reboot")]
    if name == b"reboot" {
        return Some(system::reboot);
    }
    #[cfg(feature = "poweroff")]
    if name == b"poweroff" {
        return Some(system::poweroff);
    }
    #[cfg(feature = "chroot")]
    if name == b"chroot" {
        return Some(system::chroot);
    }
    #[cfg(feature = "logname")]
    if name == b"logname" {
        return Some(system::logname);
    }
    #[cfg(feature = "logger")]
    if name == b"logger" {
        return Some(system::logger);
    }
    #[cfg(feature = "dnsdomainname")]
    if name == b"dnsdomainname" {
        return Some(system::dnsdomainname);
    }
    #[cfg(feature = "hostid")]
    if name == b"hostid" {
        return Some(system::hostid);
    }
    #[cfg(feature = "nproc")]
    if name == b"nproc" {
        return Some(system::nproc);
    }
    #[cfg(feature = "fgconsole")]
    if name == b"fgconsole" {
        return Some(system::fgconsole);
    }
    #[cfg(feature = "chvt")]
    if name == b"chvt" {
        return Some(system::chvt);
    }
    #[cfg(feature = "flock")]
    if name == b"flock" {
        return Some(system::flock);
    }
    #[cfg(feature = "fsync")]
    if name == b"fsync" {
        return Some(system::fsync_cmd);
    }
    #[cfg(feature = "sysctl")]
    if name == b"sysctl" {
        return Some(system::sysctl);
    }
    #[cfg(feature = "swapoff")]
    if name == b"swapoff" {
        return Some(system::swapoff);
    }
    #[cfg(feature = "swapon")]
    if name == b"swapon" {
        return Some(system::swapon);
    }
    #[cfg(feature = "blkid")]
    if name == b"blkid" {
        return Some(system::blkid);
    }
    #[cfg(feature = "losetup")]
    if name == b"losetup" {
        return Some(system::losetup);
    }
    #[cfg(feature = "insmod")]
    if name == b"insmod" {
        return Some(system::insmod);
    }
    #[cfg(feature = "rmmod")]
    if name == b"rmmod" {
        return Some(system::rmmod);
    }
    #[cfg(feature = "modprobe")]
    if name == b"modprobe" {
        return Some(system::modprobe);
    }
    #[cfg(feature = "lsmod")]
    if name == b"lsmod" {
        return Some(system::lsmod);
    }
    #[cfg(feature = "pivot_root")]
    if name == b"pivot_root" {
        return Some(system::pivot_root);
    }
    #[cfg(feature = "readahead")]
    if name == b"readahead" {
        return Some(system::readahead_cmd);
    }
    #[cfg(feature = "rfkill")]
    if name == b"rfkill" {
        return Some(system::rfkill);
    }
    #[cfg(feature = "acpi")]
    if name == b"acpi" {
        return Some(system::acpi);
    }
    #[cfg(feature = "cal")]
    if name == b"cal" {
        return Some(system::cal);
    }
    #[cfg(feature = "vmstat")]
    if name == b"vmstat" {
        return Some(system::vmstat);
    }
    #[cfg(feature = "watch")]
    if name == b"watch" {
        return Some(system::watch);
    }
    #[cfg(feature = "hwclock")]
    if name == b"hwclock" {
        return Some(system::hwclock);
    }
    #[cfg(feature = "fallocate")]
    if name == b"fallocate" {
        return Some(system::fallocate);
    }
    #[cfg(feature = "shuf")]
    if name == b"shuf" {
        return Some(system::shuf);
    }
    #[cfg(feature = "mkswap")]
    if name == b"mkswap" {
        return Some(system::mkswap);
    }
    #[cfg(feature = "nologin")]
    if name == b"nologin" {
        return Some(system::nologin);
    }
    #[cfg(feature = "su")]
    if name == b"su" {
        return Some(system::su);
    }
    #[cfg(feature = "login")]
    if name == b"login" {
        return Some(system::login);
    }
    #[cfg(feature = "eject")]
    if name == b"eject" {
        return Some(system::eject);
    }
    #[cfg(feature = "blockdev")]
    if name == b"blockdev" {
        return Some(system::blockdev);
    }
    #[cfg(feature = "rtcwake")]
    if name == b"rtcwake" {
        return Some(system::rtcwake);
    }
    #[cfg(feature = "ulimit")]
    if name == b"ulimit" {
        return Some(system::ulimit);
    }
    #[cfg(feature = "blkdiscard")]
    if name == b"blkdiscard" {
        return Some(system::blkdiscard);
    }
    #[cfg(feature = "deallocvt")]
    if name == b"deallocvt" {
        return Some(system::deallocvt);
    }
    #[cfg(feature = "devmem")]
    if name == b"devmem" {
        return Some(system::devmem);
    }
    #[cfg(feature = "freeramdisk")]
    if name == b"freeramdisk" {
        return Some(system::freeramdisk);
    }
    #[cfg(feature = "fsfreeze")]
    if name == b"fsfreeze" {
        return Some(system::fsfreeze);
    }
    #[cfg(feature = "gpiodetect")]
    if name == b"gpiodetect" {
        return Some(system::gpiodetect);
    }
    #[cfg(feature = "gpiofind")]
    if name == b"gpiofind" {
        return Some(system::gpiofind);
    }
    #[cfg(feature = "gpioget")]
    if name == b"gpioget" {
        return Some(system::gpioget);
    }
    #[cfg(feature = "gpioinfo")]
    if name == b"gpioinfo" {
        return Some(system::gpioinfo);
    }
    #[cfg(feature = "gpioset")]
    if name == b"gpioset" {
        return Some(system::gpioset);
    }
    #[cfg(feature = "i2cdetect")]
    if name == b"i2cdetect" {
        return Some(system::i2cdetect);
    }
    #[cfg(feature = "i2cdump")]
    if name == b"i2cdump" {
        return Some(system::i2cdump);
    }
    #[cfg(feature = "i2cget")]
    if name == b"i2cget" {
        return Some(system::i2cget);
    }
    #[cfg(feature = "i2cset")]
    if name == b"i2cset" {
        return Some(system::i2cset);
    }
    #[cfg(feature = "i2ctransfer")]
    if name == b"i2ctransfer" {
        return Some(system::i2ctransfer);
    }
    #[cfg(feature = "inotifyd")]
    if name == b"inotifyd" {
        return Some(system::inotifyd);
    }
    #[cfg(feature = "linux32")]
    if name == b"linux32" {
        return Some(system::linux32);
    }
    #[cfg(feature = "lspci")]
    if name == b"lspci" {
        return Some(system::lspci);
    }
    #[cfg(feature = "lsusb")]
    if name == b"lsusb" {
        return Some(system::lsusb);
    }
    #[cfg(feature = "modinfo")]
    if name == b"modinfo" {
        return Some(system::modinfo);
    }
    #[cfg(feature = "openvt")]
    if name == b"openvt" {
        return Some(system::openvt);
    }
    #[cfg(feature = "partprobe")]
    if name == b"partprobe" {
        return Some(system::partprobe);
    }

    // Process utilities
    #[cfg(feature = "kill")]
    if name == b"kill" {
        return Some(process::kill);
    }
    #[cfg(feature = "killall")]
    if name == b"killall" {
        return Some(process::killall);
    }
    #[cfg(feature = "killall5")]
    if name == b"killall5" {
        return Some(process::killall5);
    }
    #[cfg(feature = "ps")]
    if name == b"ps" {
        return Some(process::ps);
    }
    #[cfg(feature = "pgrep")]
    if name == b"pgrep" {
        return Some(process::pgrep);
    }
    #[cfg(feature = "pkill")]
    if name == b"pkill" {
        return Some(process::pkill);
    }
    #[cfg(feature = "pidof")]
    if name == b"pidof" {
        return Some(process::pidof);
    }
    #[cfg(feature = "pwdx")]
    if name == b"pwdx" {
        return Some(process::pwdx);
    }
    #[cfg(feature = "nice")]
    if name == b"nice" {
        return Some(process::nice);
    }
    #[cfg(feature = "renice")]
    if name == b"renice" {
        return Some(process::renice);
    }
    #[cfg(feature = "nohup")]
    if name == b"nohup" {
        return Some(process::nohup);
    }
    #[cfg(feature = "setsid")]
    if name == b"setsid" {
        return Some(process::setsid);
    }
    #[cfg(feature = "timeout")]
    if name == b"timeout" {
        return Some(process::timeout);
    }
    #[cfg(feature = "taskset")]
    if name == b"taskset" {
        return Some(process::taskset);
    }
    #[cfg(feature = "ionice")]
    if name == b"ionice" {
        return Some(process::ionice);
    }
    #[cfg(feature = "chrt")]
    if name == b"chrt" {
        return Some(process::chrt);
    }
    #[cfg(feature = "top")]
    if name == b"top" {
        return Some(process::top);
    }
    #[cfg(feature = "nsenter")]
    if name == b"nsenter" {
        return Some(process::nsenter);
    }
    #[cfg(feature = "unshare")]
    if name == b"unshare" {
        return Some(process::unshare);
    }
    #[cfg(feature = "pmap")]
    if name == b"pmap" {
        return Some(process::pmap);
    }
    #[cfg(feature = "prlimit")]
    if name == b"prlimit" {
        return Some(process::prlimit);
    }
    #[cfg(feature = "uclampset")]
    if name == b"uclampset" {
        return Some(process::uclampset);
    }
    #[cfg(feature = "iorenice")]
    if name == b"iorenice" {
        return Some(process::iorenice);
    }
    #[cfg(feature = "iotop")]
    if name == b"iotop" {
        return Some(process::iotop);
    }

    // Misc utilities
    #[cfg(feature = "boolean")]
    if name == b"true" {
        return Some(misc::r#true);
    }
    #[cfg(feature = "boolean")]
    if name == b"false" {
        return Some(misc::r#false);
    }
    #[cfg(feature = "boolean")]
    if name == b":" {
        return Some(misc::colon);
    }
    #[cfg(feature = "test")]
    if name == b"test" {
        return Some(misc::test);
    }
    #[cfg(feature = "test")]
    if name == b"[" {
        return Some(misc::bracket);
    }
    #[cfg(feature = "terminal")]
    if name == b"clear" {
        return Some(misc::clear);
    }
    #[cfg(feature = "terminal")]
    if name == b"reset" {
        return Some(misc::reset);
    }
    #[cfg(feature = "which")]
    if name == b"which" {
        return Some(misc::which);
    }
    #[cfg(feature = "expr")]
    if name == b"expr" {
        return Some(misc::expr);
    }
    #[cfg(feature = "time")]
    if name == b"time" {
        return Some(misc::time);
    }
    #[cfg(feature = "mesg")]
    if name == b"mesg" {
        return Some(misc::mesg);
    }
    #[cfg(feature = "getconf")]
    if name == b"getconf" {
        return Some(misc::getconf);
    }
    #[cfg(feature = "factor")]
    if name == b"factor" {
        return Some(misc::factor);
    }
    #[cfg(feature = "base64")]
    if name == b"base64" {
        return Some(misc::base64);
    }
    #[cfg(feature = "base64")]
    if name == b"base32" {
        return Some(misc::base32);
    }
    #[cfg(feature = "cmp")]
    if name == b"cmp" {
        return Some(misc::cmp);
    }
    #[cfg(feature = "diff")]
    if name == b"diff" {
        return Some(misc::diff);
    }
    #[cfg(feature = "hexdump")]
    if name == b"od" {
        return Some(misc::od);
    }
    #[cfg(feature = "hexdump")]
    if name == b"hexdump" {
        return Some(misc::hexdump);
    }
    #[cfg(feature = "hexdump")]
    if name == b"hd" {
        return Some(misc::hexdump);
    }
    #[cfg(feature = "hexdump")]
    if name == b"xxd" {
        return Some(misc::hexdump);
    }
    #[cfg(feature = "hash")]
    if name == b"md5sum" {
        return Some(misc::md5sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha1sum" {
        return Some(misc::sha1sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha224sum" {
        return Some(misc::sha224sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha256sum" {
        return Some(misc::sha256sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha384sum" {
        return Some(misc::sha384sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha512sum" {
        return Some(misc::sha512sum);
    }
    #[cfg(feature = "hash")]
    if name == b"sha3sum" {
        return Some(misc::sha3sum);
    }
    #[cfg(feature = "hash")]
    if name == b"cksum" {
        return Some(misc::cksum);
    }
    #[cfg(feature = "hash")]
    if name == b"crc32" {
        return Some(misc::crc32);
    }
    #[cfg(feature = "ascii")]
    if name == b"ascii" {
        return Some(misc::ascii);
    }
    #[cfg(feature = "iconv")]
    if name == b"iconv" {
        return Some(misc::iconv);
    }
    #[cfg(feature = "tsort")]
    if name == b"tsort" {
        return Some(misc::tsort);
    }
    #[cfg(feature = "getopt")]
    if name == b"getopt" {
        return Some(misc::getopt);
    }
    #[cfg(feature = "count")]
    if name == b"count" {
        return Some(misc::count);
    }
    #[cfg(feature = "unicode")]
    if name == b"unicode" {
        return Some(misc::unicode);
    }
    #[cfg(feature = "ts")]
    if name == b"ts" {
        return Some(misc::ts);
    }
    #[cfg(feature = "uuidgen")]
    if name == b"uuidgen" {
        return Some(misc::uuidgen);
    }
    #[cfg(feature = "mcookie")]
    if name == b"mcookie" {
        return Some(misc::mcookie);
    }
    #[cfg(feature = "pwgen")]
    if name == b"pwgen" {
        return Some(misc::pwgen);
    }
    #[cfg(feature = "uuencode")]
    if name == b"uuencode" {
        return Some(misc::uuencode);
    }
    #[cfg(feature = "uudecode")]
    if name == b"uudecode" {
        return Some(misc::uudecode);
    }
    #[cfg(feature = "help")]
    if name == b"help" {
        return Some(misc::help);
    }
    #[cfg(feature = "memeater")]
    if name == b"memeater" {
        return Some(misc::memeater);
    }
    #[cfg(feature = "mix")]
    if name == b"mix" {
        return Some(misc::mix);
    }
    #[cfg(feature = "mkpasswd")]
    if name == b"mkpasswd" {
        return Some(misc::mkpasswd);
    }
    #[cfg(feature = "toybox")]
    if name == b"toybox" {
        return Some(misc::toybox);
    }
    #[cfg(feature = "readelf")]
    if name == b"readelf" {
        return Some(misc::readelf);
    }
    #[cfg(feature = "screen")]
    if name == b"screen" {
        return Some(misc::screen);
    }
    #[cfg(feature = "screen")]
    if name == b"tmux" {
        return Some(misc::screen);
    }

    // Network utilities
    #[cfg(feature = "wget")]
    if name == b"wget" {
        return Some(network::wget);
    }
    #[cfg(feature = "nc")]
    if name == b"nc" {
        return Some(network::nc);
    }
    #[cfg(feature = "nc")]
    if name == b"netcat" {
        return Some(network::netcat);
    }
    #[cfg(feature = "ping")]
    if name == b"ping" {
        return Some(network::ping);
    }
    #[cfg(feature = "ping")]
    if name == b"ping6" {
        return Some(network::ping6);
    }
    #[cfg(feature = "traceroute")]
    if name == b"traceroute" {
        return Some(network::traceroute);
    }
    #[cfg(feature = "traceroute")]
    if name == b"traceroute6" {
        return Some(network::traceroute6);
    }
    #[cfg(feature = "host")]
    if name == b"host" {
        return Some(network::host);
    }
    #[cfg(feature = "nslookup")]
    if name == b"nslookup" {
        return Some(network::nslookup);
    }
    #[cfg(feature = "ifconfig")]
    if name == b"ifconfig" {
        return Some(network::ifconfig);
    }
    #[cfg(feature = "netstat")]
    if name == b"netstat" {
        return Some(network::netstat);
    }
    #[cfg(feature = "route")]
    if name == b"route" {
        return Some(network::route);
    }
    #[cfg(feature = "tftp")]
    if name == b"tftp" {
        return Some(network::tftp);
    }
    #[cfg(feature = "ftpget")]
    if name == b"ftpget" {
        return Some(network::ftpget);
    }
    #[cfg(feature = "ftpput")]
    if name == b"ftpput" {
        return Some(network::ftpput);
    }
    #[cfg(feature = "ipcalc")]
    if name == b"ipcalc" {
        return Some(network::ipcalc);
    }
    #[cfg(feature = "brctl")]
    if name == b"brctl" {
        return Some(network::brctl);
    }
    #[cfg(feature = "tunctl")]
    if name == b"tunctl" {
        return Some(network::tunctl);
    }
    #[cfg(feature = "ether_wake")]
    if name == b"ether-wake" {
        return Some(network::ether_wake);
    }
    #[cfg(feature = "ifup")]
    if name == b"ifup" {
        return Some(network::ifup);
    }
    #[cfg(feature = "ifdown")]
    if name == b"ifdown" {
        return Some(network::ifdown);
    }
    #[cfg(feature = "ss")]
    if name == b"ss" {
        return Some(network::ss);
    }
    #[cfg(feature = "arp")]
    if name == b"arp" {
        return Some(network::arp);
    }
    #[cfg(feature = "arping")]
    if name == b"arping" {
        return Some(network::arping);
    }
    #[cfg(feature = "ip")]
    if name == b"ip" {
        return Some(network::ip);
    }
    #[cfg(feature = "ipaddr")]
    if name == b"ipaddr" {
        return Some(network::ipaddr);
    }
    #[cfg(feature = "iplink")]
    if name == b"iplink" {
        return Some(network::iplink);
    }
    #[cfg(feature = "ipneigh")]
    if name == b"ipneigh" {
        return Some(network::ipneigh);
    }
    #[cfg(feature = "iproute")]
    if name == b"iproute" {
        return Some(network::iproute);
    }
    #[cfg(feature = "iprule")]
    if name == b"iprule" {
        return Some(network::iprule);
    }
    #[cfg(feature = "nameif")]
    if name == b"nameif" {
        return Some(network::nameif);
    }
    #[cfg(feature = "slattach")]
    if name == b"slattach" {
        return Some(network::slattach);
    }
    #[cfg(feature = "vconfig")]
    if name == b"vconfig" {
        return Some(network::vconfig);
    }
    #[cfg(feature = "telnet")]
    if name == b"telnet" {
        return Some(network::telnet);
    }
    #[cfg(feature = "httpd")]
    if name == b"httpd" {
        return Some(network::httpd);
    }
    #[cfg(feature = "sntp")]
    if name == b"sntp" {
        return Some(network::sntp);
    }
    #[cfg(feature = "microcom")]
    if name == b"microcom" {
        return Some(network::microcom);
    }
    #[cfg(feature = "nbd_client")]
    if name == b"nbd-client" {
        return Some(network::nbd_client);
    }
    #[cfg(feature = "nbd_server")]
    if name == b"nbd-server" {
        return Some(network::nbd_server);
    }

    // Archive utilities
    #[cfg(feature = "tar")]
    if name == b"tar" {
        return Some(archive::tar);
    }
    #[cfg(feature = "gzip")]
    if name == b"gzip" {
        return Some(archive::gzip);
    }
    #[cfg(feature = "gzip")]
    if name == b"gunzip" {
        return Some(archive::gunzip);
    }
    #[cfg(feature = "gzip")]
    if name == b"zcat" {
        return Some(archive::zcat);
    }
    #[cfg(feature = "bzip2")]
    if name == b"bzip2" {
        return Some(archive::bzip2);
    }
    #[cfg(feature = "bzip2")]
    if name == b"bunzip2" {
        return Some(archive::bunzip2);
    }
    #[cfg(feature = "bzip2")]
    if name == b"bzcat" {
        return Some(archive::bzcat);
    }
    #[cfg(feature = "xz")]
    if name == b"xz" {
        return Some(archive::xz);
    }
    #[cfg(feature = "xz")]
    if name == b"unxz" {
        return Some(archive::unxz);
    }
    #[cfg(feature = "xz")]
    if name == b"xzcat" {
        return Some(archive::xzcat);
    }
    #[cfg(feature = "cpio")]
    if name == b"cpio" {
        return Some(archive::cpio);
    }
    #[cfg(feature = "unzip")]
    if name == b"unzip" {
        return Some(archive::unzip);
    }
    #[cfg(feature = "compress")]
    if name == b"compress" {
        return Some(archive::compress);
    }
    #[cfg(feature = "compress")]
    if name == b"uncompress" {
        return Some(archive::uncompress);
    }
    #[cfg(feature = "xz")]
    if name == b"lzma" {
        return Some(archive::lzma);
    }
    #[cfg(feature = "xz")]
    if name == b"unlzma" {
        return Some(archive::unlzma);
    }
    #[cfg(feature = "xz")]
    if name == b"lzcat" {
        return Some(archive::lzcat);
    }
    #[cfg(feature = "zstd")]
    if name == b"zstd" {
        return Some(archive::zstd);
    }
    #[cfg(feature = "zstd")]
    if name == b"unzstd" {
        return Some(archive::unzstd);
    }
    #[cfg(feature = "zstd")]
    if name == b"zstdcat" {
        return Some(archive::zstdcat);
    }

    // Editors
    #[cfg(feature = "vi")]
    if name == b"vi" {
        return Some(editors::vi);
    }
    #[cfg(feature = "vi")]
    if name == b"view" {
        return Some(editors::view);
    }
    #[cfg(feature = "hexedit")]
    if name == b"hexedit" {
        return Some(editors::hexedit);
    }

    // Init system
    #[cfg(feature = "init")]
    if name == b"init" {
        return Some(init::init);
    }
    #[cfg(feature = "telinit")]
    if name == b"telinit" {
        return Some(init::telinit);
    }
    #[cfg(feature = "runlevel")]
    if name == b"runlevel" {
        return Some(init::runlevel);
    }
    #[cfg(feature = "getty")]
    if name == b"getty" {
        return Some(init::getty);
    }
    #[cfg(feature = "sulogin")]
    if name == b"sulogin" {
        return Some(init::sulogin);
    }
    #[cfg(feature = "init")]
    if name == b"linuxrc" {
        return Some(init::init);
    }
    #[cfg(feature = "oneit")]
    if name == b"oneit" {
        return Some(init::oneit);
    }
    #[cfg(feature = "switch_root")]
    if name == b"switch_root" {
        return Some(init::switch_root);
    }
    #[cfg(feature = "watchdog")]
    if name == b"watchdog" {
        return Some(init::watchdog);
    }

    // Shell
    #[cfg(feature = "sh")]
    if name == b"sh" {
        return Some(shell::sh);
    }
    #[cfg(feature = "sh")]
    if name == b"ash" {
        return Some(shell::ash);
    }
    #[cfg(feature = "sh")]
    if name == b"dash" {
        return Some(shell::dash);
    }

    // APK package manager (feature gated)
    #[cfg(feature = "apk")]
    if name == b"apk" {
        return Some(package::apk);
    }

    // ABP package manager (feature gated)
    #[cfg(feature = "abp")]
    if name == b"abp" {
        return Some(package::abp);
    }

    None
}

/// List all applet names
pub fn list_applets() {
    io::write_str(1, b"Currently defined applets:\n");

    #[cfg(feature = "abp")]
    io::write_str(1, b"abp\n");
    #[cfg(feature = "acpi")]
    io::write_str(1, b"acpi\n");
    #[cfg(feature = "apk")]
    io::write_str(1, b"apk\n");
    #[cfg(feature = "arch")]
    io::write_str(1, b"arch\n");
    #[cfg(feature = "arp")]
    io::write_str(1, b"arp\n");
    #[cfg(feature = "arping")]
    io::write_str(1, b"arping\n");
    #[cfg(feature = "ascii")]
    io::write_str(1, b"ascii\n");
    #[cfg(feature = "sh")]
    io::write_str(1, b"ash\n");
    #[cfg(feature = "awk")]
    io::write_str(1, b"awk\n");
    #[cfg(feature = "base64")]
    io::write_str(1, b"base32\n");
    #[cfg(feature = "base64")]
    io::write_str(1, b"base64\n");
    #[cfg(feature = "basename")]
    io::write_str(1, b"basename\n");
    #[cfg(feature = "blkdiscard")]
    io::write_str(1, b"blkdiscard\n");
    #[cfg(feature = "blkid")]
    io::write_str(1, b"blkid\n");
    #[cfg(feature = "blockdev")]
    io::write_str(1, b"blockdev\n");
    #[cfg(feature = "brctl")]
    io::write_str(1, b"brctl\n");
    #[cfg(feature = "bzip2")]
    io::write_str(1, b"bunzip2\n");
    #[cfg(feature = "bzip2")]
    io::write_str(1, b"bzcat\n");
    #[cfg(feature = "bzip2")]
    io::write_str(1, b"bzip2\n");
    #[cfg(feature = "cal")]
    io::write_str(1, b"cal\n");
    #[cfg(feature = "cat")]
    io::write_str(1, b"cat\n");
    #[cfg(feature = "cd")]
    io::write_str(1, b"cd\n");
    #[cfg(feature = "chattr")]
    io::write_str(1, b"chattr\n");
    #[cfg(feature = "chgrp")]
    io::write_str(1, b"chgrp\n");
    #[cfg(feature = "chmod")]
    io::write_str(1, b"chmod\n");
    #[cfg(feature = "chown")]
    io::write_str(1, b"chown\n");
    #[cfg(feature = "chroot")]
    io::write_str(1, b"chroot\n");
    #[cfg(feature = "chrt")]
    io::write_str(1, b"chrt\n");
    #[cfg(feature = "chvt")]
    io::write_str(1, b"chvt\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"cksum\n");
    #[cfg(feature = "terminal")]
    io::write_str(1, b"clear\n");
    #[cfg(feature = "cmp")]
    io::write_str(1, b"cmp\n");
    #[cfg(feature = "comm")]
    io::write_str(1, b"comm\n");
    #[cfg(feature = "compress")]
    io::write_str(1, b"compress\n");
    #[cfg(feature = "count")]
    io::write_str(1, b"count\n");
    #[cfg(feature = "cp")]
    io::write_str(1, b"cp\n");
    #[cfg(feature = "cpio")]
    io::write_str(1, b"cpio\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"crc32\n");
    #[cfg(feature = "cut")]
    io::write_str(1, b"cut\n");
    #[cfg(feature = "sh")]
    io::write_str(1, b"dash\n");
    #[cfg(feature = "date")]
    io::write_str(1, b"date\n");
    #[cfg(feature = "dd")]
    io::write_str(1, b"dd\n");
    #[cfg(feature = "deallocvt")]
    io::write_str(1, b"deallocvt\n");
    #[cfg(feature = "devmem")]
    io::write_str(1, b"devmem\n");
    #[cfg(feature = "df")]
    io::write_str(1, b"df\n");
    #[cfg(feature = "diff")]
    io::write_str(1, b"diff\n");
    #[cfg(feature = "dirname")]
    io::write_str(1, b"dirname\n");
    #[cfg(feature = "dmesg")]
    io::write_str(1, b"dmesg\n");
    #[cfg(feature = "dnsdomainname")]
    io::write_str(1, b"dnsdomainname\n");
    #[cfg(feature = "dos2unix")]
    io::write_str(1, b"dos2unix\n");
    #[cfg(feature = "du")]
    io::write_str(1, b"du\n");
    #[cfg(feature = "echo")]
    io::write_str(1, b"echo\n");
    #[cfg(feature = "grep")]
    io::write_str(1, b"egrep\n");
    #[cfg(feature = "eject")]
    io::write_str(1, b"eject\n");
    #[cfg(feature = "env")]
    io::write_str(1, b"env\n");
    #[cfg(feature = "ether_wake")]
    io::write_str(1, b"ether-wake\n");
    #[cfg(feature = "expand")]
    io::write_str(1, b"expand\n");
    #[cfg(feature = "expr")]
    io::write_str(1, b"expr\n");
    #[cfg(feature = "factor")]
    io::write_str(1, b"factor\n");
    #[cfg(feature = "fallocate")]
    io::write_str(1, b"fallocate\n");
    #[cfg(feature = "boolean")]
    io::write_str(1, b"false\n");
    #[cfg(feature = "fgconsole")]
    io::write_str(1, b"fgconsole\n");
    #[cfg(feature = "grep")]
    io::write_str(1, b"fgrep\n");
    #[cfg(feature = "file")]
    io::write_str(1, b"file\n");
    #[cfg(feature = "find")]
    io::write_str(1, b"find\n");
    #[cfg(feature = "flock")]
    io::write_str(1, b"flock\n");
    #[cfg(feature = "fmt")]
    io::write_str(1, b"fmt\n");
    #[cfg(feature = "fold")]
    io::write_str(1, b"fold\n");
    #[cfg(feature = "free")]
    io::write_str(1, b"free\n");
    #[cfg(feature = "freeramdisk")]
    io::write_str(1, b"freeramdisk\n");
    #[cfg(feature = "fsfreeze")]
    io::write_str(1, b"fsfreeze\n");
    #[cfg(feature = "fstype")]
    io::write_str(1, b"fstype\n");
    #[cfg(feature = "fsync")]
    io::write_str(1, b"fsync\n");
    #[cfg(feature = "ftpget")]
    io::write_str(1, b"ftpget\n");
    #[cfg(feature = "ftpput")]
    io::write_str(1, b"ftpput\n");
    #[cfg(feature = "getconf")]
    io::write_str(1, b"getconf\n");
    #[cfg(feature = "getopt")]
    io::write_str(1, b"getopt\n");
    #[cfg(feature = "getty")]
    io::write_str(1, b"getty\n");
    #[cfg(feature = "gpiodetect")]
    io::write_str(1, b"gpiodetect\n");
    #[cfg(feature = "gpiofind")]
    io::write_str(1, b"gpiofind\n");
    #[cfg(feature = "gpioget")]
    io::write_str(1, b"gpioget\n");
    #[cfg(feature = "gpioinfo")]
    io::write_str(1, b"gpioinfo\n");
    #[cfg(feature = "gpioset")]
    io::write_str(1, b"gpioset\n");
    #[cfg(feature = "grep")]
    io::write_str(1, b"grep\n");
    #[cfg(feature = "groups")]
    io::write_str(1, b"groups\n");
    #[cfg(feature = "gzip")]
    io::write_str(1, b"gunzip\n");
    #[cfg(feature = "gzip")]
    io::write_str(1, b"gzip\n");
    #[cfg(feature = "halt")]
    io::write_str(1, b"halt\n");
    #[cfg(feature = "hexdump")]
    io::write_str(1, b"hd\n");
    #[cfg(feature = "head")]
    io::write_str(1, b"head\n");
    #[cfg(feature = "help")]
    io::write_str(1, b"help\n");
    #[cfg(feature = "hexdump")]
    io::write_str(1, b"hexdump\n");
    #[cfg(feature = "hexedit")]
    io::write_str(1, b"hexedit\n");
    #[cfg(feature = "host")]
    io::write_str(1, b"host\n");
    #[cfg(feature = "hostid")]
    io::write_str(1, b"hostid\n");
    #[cfg(feature = "hostname")]
    io::write_str(1, b"hostname\n");
    #[cfg(feature = "httpd")]
    io::write_str(1, b"httpd\n");
    #[cfg(feature = "hwclock")]
    io::write_str(1, b"hwclock\n");
    #[cfg(feature = "i2cdetect")]
    io::write_str(1, b"i2cdetect\n");
    #[cfg(feature = "i2cdump")]
    io::write_str(1, b"i2cdump\n");
    #[cfg(feature = "i2cget")]
    io::write_str(1, b"i2cget\n");
    #[cfg(feature = "i2cset")]
    io::write_str(1, b"i2cset\n");
    #[cfg(feature = "i2ctransfer")]
    io::write_str(1, b"i2ctransfer\n");
    #[cfg(feature = "iconv")]
    io::write_str(1, b"iconv\n");
    #[cfg(feature = "id")]
    io::write_str(1, b"id\n");
    #[cfg(feature = "ifconfig")]
    io::write_str(1, b"ifconfig\n");
    #[cfg(feature = "ifdown")]
    io::write_str(1, b"ifdown\n");
    #[cfg(feature = "ifup")]
    io::write_str(1, b"ifup\n");
    #[cfg(feature = "init")]
    io::write_str(1, b"init\n");
    #[cfg(feature = "inotifyd")]
    io::write_str(1, b"inotifyd\n");
    #[cfg(feature = "insmod")]
    io::write_str(1, b"insmod\n");
    #[cfg(feature = "install")]
    io::write_str(1, b"install\n");
    #[cfg(feature = "ionice")]
    io::write_str(1, b"ionice\n");
    #[cfg(feature = "iorenice")]
    io::write_str(1, b"iorenice\n");
    #[cfg(feature = "iotop")]
    io::write_str(1, b"iotop\n");
    #[cfg(feature = "ip")]
    io::write_str(1, b"ip\n");
    #[cfg(feature = "ipaddr")]
    io::write_str(1, b"ipaddr\n");
    #[cfg(feature = "ipcalc")]
    io::write_str(1, b"ipcalc\n");
    #[cfg(feature = "iplink")]
    io::write_str(1, b"iplink\n");
    #[cfg(feature = "ipneigh")]
    io::write_str(1, b"ipneigh\n");
    #[cfg(feature = "iproute")]
    io::write_str(1, b"iproute\n");
    #[cfg(feature = "iprule")]
    io::write_str(1, b"iprule\n");
    #[cfg(feature = "kill")]
    io::write_str(1, b"kill\n");
    #[cfg(feature = "killall")]
    io::write_str(1, b"killall\n");
    #[cfg(feature = "killall5")]
    io::write_str(1, b"killall5\n");
    #[cfg(feature = "link")]
    io::write_str(1, b"link\n");
    #[cfg(feature = "linux32")]
    io::write_str(1, b"linux32\n");
    #[cfg(feature = "init")]
    io::write_str(1, b"linuxrc\n");
    #[cfg(feature = "ln")]
    io::write_str(1, b"ln\n");
    #[cfg(feature = "logger")]
    io::write_str(1, b"logger\n");
    #[cfg(feature = "login")]
    io::write_str(1, b"login\n");
    #[cfg(feature = "logname")]
    io::write_str(1, b"logname\n");
    #[cfg(feature = "losetup")]
    io::write_str(1, b"losetup\n");
    #[cfg(feature = "ls")]
    io::write_str(1, b"ls\n");
    #[cfg(feature = "lsattr")]
    io::write_str(1, b"lsattr\n");
    #[cfg(feature = "lsmod")]
    io::write_str(1, b"lsmod\n");
    #[cfg(feature = "lspci")]
    io::write_str(1, b"lspci\n");
    #[cfg(feature = "lsusb")]
    io::write_str(1, b"lsusb\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"lzcat\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"lzma\n");
    #[cfg(feature = "makedevs")]
    io::write_str(1, b"makedevs\n");
    #[cfg(feature = "mcookie")]
    io::write_str(1, b"mcookie\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"md5sum\n");
    #[cfg(feature = "memeater")]
    io::write_str(1, b"memeater\n");
    #[cfg(feature = "mesg")]
    io::write_str(1, b"mesg\n");
    #[cfg(feature = "microcom")]
    io::write_str(1, b"microcom\n");
    #[cfg(feature = "mix")]
    io::write_str(1, b"mix\n");
    #[cfg(feature = "mkdir")]
    io::write_str(1, b"mkdir\n");
    #[cfg(feature = "mkfifo")]
    io::write_str(1, b"mkfifo\n");
    #[cfg(feature = "mknod")]
    io::write_str(1, b"mknod\n");
    #[cfg(feature = "mkpasswd")]
    io::write_str(1, b"mkpasswd\n");
    #[cfg(feature = "mkswap")]
    io::write_str(1, b"mkswap\n");
    #[cfg(feature = "mktemp")]
    io::write_str(1, b"mktemp\n");
    #[cfg(feature = "modinfo")]
    io::write_str(1, b"modinfo\n");
    #[cfg(feature = "modprobe")]
    io::write_str(1, b"modprobe\n");
    #[cfg(feature = "mount")]
    io::write_str(1, b"mount\n");
    #[cfg(feature = "mountpoint")]
    io::write_str(1, b"mountpoint\n");
    #[cfg(feature = "mv")]
    io::write_str(1, b"mv\n");
    #[cfg(feature = "nameif")]
    io::write_str(1, b"nameif\n");
    #[cfg(feature = "nbd_client")]
    io::write_str(1, b"nbd-client\n");
    #[cfg(feature = "nbd_server")]
    io::write_str(1, b"nbd-server\n");
    #[cfg(feature = "nc")]
    io::write_str(1, b"nc\n");
    #[cfg(feature = "nc")]
    io::write_str(1, b"netcat\n");
    #[cfg(feature = "netstat")]
    io::write_str(1, b"netstat\n");
    #[cfg(feature = "nice")]
    io::write_str(1, b"nice\n");
    #[cfg(feature = "nl")]
    io::write_str(1, b"nl\n");
    #[cfg(feature = "nohup")]
    io::write_str(1, b"nohup\n");
    #[cfg(feature = "nologin")]
    io::write_str(1, b"nologin\n");
    #[cfg(feature = "nproc")]
    io::write_str(1, b"nproc\n");
    #[cfg(feature = "nsenter")]
    io::write_str(1, b"nsenter\n");
    #[cfg(feature = "nslookup")]
    io::write_str(1, b"nslookup\n");
    #[cfg(feature = "hexdump")]
    io::write_str(1, b"od\n");
    #[cfg(feature = "oneit")]
    io::write_str(1, b"oneit\n");
    #[cfg(feature = "openvt")]
    io::write_str(1, b"openvt\n");
    #[cfg(feature = "partprobe")]
    io::write_str(1, b"partprobe\n");
    #[cfg(feature = "paste")]
    io::write_str(1, b"paste\n");
    #[cfg(feature = "patch")]
    io::write_str(1, b"patch\n");
    #[cfg(feature = "pgrep")]
    io::write_str(1, b"pgrep\n");
    #[cfg(feature = "pidof")]
    io::write_str(1, b"pidof\n");
    #[cfg(feature = "ping")]
    io::write_str(1, b"ping\n");
    #[cfg(feature = "ping")]
    io::write_str(1, b"ping6\n");
    #[cfg(feature = "pivot_root")]
    io::write_str(1, b"pivot_root\n");
    #[cfg(feature = "pkill")]
    io::write_str(1, b"pkill\n");
    #[cfg(feature = "pmap")]
    io::write_str(1, b"pmap\n");
    #[cfg(feature = "poweroff")]
    io::write_str(1, b"poweroff\n");
    #[cfg(feature = "printenv")]
    io::write_str(1, b"printenv\n");
    #[cfg(feature = "printf")]
    io::write_str(1, b"printf\n");
    #[cfg(feature = "prlimit")]
    io::write_str(1, b"prlimit\n");
    #[cfg(feature = "ps")]
    io::write_str(1, b"ps\n");
    #[cfg(feature = "pwd")]
    io::write_str(1, b"pwd\n");
    #[cfg(feature = "pwdx")]
    io::write_str(1, b"pwdx\n");
    #[cfg(feature = "pwgen")]
    io::write_str(1, b"pwgen\n");
    #[cfg(feature = "readahead")]
    io::write_str(1, b"readahead\n");
    #[cfg(feature = "readelf")]
    io::write_str(1, b"readelf\n");
    #[cfg(feature = "readlink")]
    io::write_str(1, b"readlink\n");
    #[cfg(feature = "realpath")]
    io::write_str(1, b"realpath\n");
    #[cfg(feature = "reboot")]
    io::write_str(1, b"reboot\n");
    #[cfg(feature = "renice")]
    io::write_str(1, b"renice\n");
    #[cfg(feature = "terminal")]
    io::write_str(1, b"reset\n");
    #[cfg(feature = "rev")]
    io::write_str(1, b"rev\n");
    #[cfg(feature = "rfkill")]
    io::write_str(1, b"rfkill\n");
    #[cfg(feature = "rm")]
    io::write_str(1, b"rm\n");
    #[cfg(feature = "rmdir")]
    io::write_str(1, b"rmdir\n");
    #[cfg(feature = "rmmod")]
    io::write_str(1, b"rmmod\n");
    #[cfg(feature = "route")]
    io::write_str(1, b"route\n");
    #[cfg(feature = "rtcwake")]
    io::write_str(1, b"rtcwake\n");
    #[cfg(feature = "runlevel")]
    io::write_str(1, b"runlevel\n");
    #[cfg(feature = "screen")]
    io::write_str(1, b"screen\n");
    #[cfg(feature = "sed")]
    io::write_str(1, b"sed\n");
    #[cfg(feature = "seq")]
    io::write_str(1, b"seq\n");
    #[cfg(feature = "setfattr")]
    io::write_str(1, b"setfattr\n");
    #[cfg(feature = "setsid")]
    io::write_str(1, b"setsid\n");
    #[cfg(feature = "sh")]
    io::write_str(1, b"sh\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha1sum\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha224sum\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha256sum\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha384sum\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha3sum\n");
    #[cfg(feature = "hash")]
    io::write_str(1, b"sha512sum\n");
    #[cfg(feature = "shred")]
    io::write_str(1, b"shred\n");
    #[cfg(feature = "shuf")]
    io::write_str(1, b"shuf\n");
    #[cfg(feature = "slattach")]
    io::write_str(1, b"slattach\n");
    #[cfg(feature = "sleep")]
    io::write_str(1, b"sleep\n");
    #[cfg(feature = "sntp")]
    io::write_str(1, b"sntp\n");
    #[cfg(feature = "sort")]
    io::write_str(1, b"sort\n");
    #[cfg(feature = "split")]
    io::write_str(1, b"split\n");
    #[cfg(feature = "ss")]
    io::write_str(1, b"ss\n");
    #[cfg(feature = "stat")]
    io::write_str(1, b"stat\n");
    #[cfg(feature = "strings")]
    io::write_str(1, b"strings\n");
    #[cfg(feature = "su")]
    io::write_str(1, b"su\n");
    #[cfg(feature = "sulogin")]
    io::write_str(1, b"sulogin\n");
    #[cfg(feature = "swapoff")]
    io::write_str(1, b"swapoff\n");
    #[cfg(feature = "swapon")]
    io::write_str(1, b"swapon\n");
    #[cfg(feature = "switch_root")]
    io::write_str(1, b"switch_root\n");
    #[cfg(feature = "sync")]
    io::write_str(1, b"sync\n");
    #[cfg(feature = "sysctl")]
    io::write_str(1, b"sysctl\n");
    #[cfg(feature = "tac")]
    io::write_str(1, b"tac\n");
    #[cfg(feature = "tail")]
    io::write_str(1, b"tail\n");
    #[cfg(feature = "tar")]
    io::write_str(1, b"tar\n");
    #[cfg(feature = "taskset")]
    io::write_str(1, b"taskset\n");
    #[cfg(feature = "tee")]
    io::write_str(1, b"tee\n");
    #[cfg(feature = "telinit")]
    io::write_str(1, b"telinit\n");
    #[cfg(feature = "telnet")]
    io::write_str(1, b"telnet\n");
    #[cfg(feature = "test")]
    io::write_str(1, b"test\n");
    #[cfg(feature = "tftp")]
    io::write_str(1, b"tftp\n");
    #[cfg(feature = "time")]
    io::write_str(1, b"time\n");
    #[cfg(feature = "timeout")]
    io::write_str(1, b"timeout\n");
    #[cfg(feature = "screen")]
    io::write_str(1, b"tmux\n");
    #[cfg(feature = "top")]
    io::write_str(1, b"top\n");
    #[cfg(feature = "touch")]
    io::write_str(1, b"touch\n");
    #[cfg(feature = "toybox")]
    io::write_str(1, b"toybox\n");
    #[cfg(feature = "tr")]
    io::write_str(1, b"tr\n");
    #[cfg(feature = "traceroute")]
    io::write_str(1, b"traceroute\n");
    #[cfg(feature = "traceroute")]
    io::write_str(1, b"traceroute6\n");
    #[cfg(feature = "boolean")]
    io::write_str(1, b"true\n");
    #[cfg(feature = "truncate")]
    io::write_str(1, b"truncate\n");
    #[cfg(feature = "ts")]
    io::write_str(1, b"ts\n");
    #[cfg(feature = "tsort")]
    io::write_str(1, b"tsort\n");
    #[cfg(feature = "tty")]
    io::write_str(1, b"tty\n");
    #[cfg(feature = "tunctl")]
    io::write_str(1, b"tunctl\n");
    #[cfg(feature = "uclampset")]
    io::write_str(1, b"uclampset\n");
    #[cfg(feature = "ulimit")]
    io::write_str(1, b"ulimit\n");
    #[cfg(feature = "umount")]
    io::write_str(1, b"umount\n");
    #[cfg(feature = "uname")]
    io::write_str(1, b"uname\n");
    #[cfg(feature = "compress")]
    io::write_str(1, b"uncompress\n");
    #[cfg(feature = "unexpand")]
    io::write_str(1, b"unexpand\n");
    #[cfg(feature = "unicode")]
    io::write_str(1, b"unicode\n");
    #[cfg(feature = "uniq")]
    io::write_str(1, b"uniq\n");
    #[cfg(feature = "unix2dos")]
    io::write_str(1, b"unix2dos\n");
    #[cfg(feature = "unlink")]
    io::write_str(1, b"unlink\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"unlzma\n");
    #[cfg(feature = "unshare")]
    io::write_str(1, b"unshare\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"unxz\n");
    #[cfg(feature = "zstd")]
    io::write_str(1, b"unzstd\n");
    #[cfg(feature = "unzip")]
    io::write_str(1, b"unzip\n");
    #[cfg(feature = "uptime")]
    io::write_str(1, b"uptime\n");
    #[cfg(feature = "users")]
    io::write_str(1, b"users\n");
    #[cfg(feature = "usleep")]
    io::write_str(1, b"usleep\n");
    #[cfg(feature = "uudecode")]
    io::write_str(1, b"uudecode\n");
    #[cfg(feature = "uuencode")]
    io::write_str(1, b"uuencode\n");
    #[cfg(feature = "uuidgen")]
    io::write_str(1, b"uuidgen\n");
    #[cfg(feature = "vconfig")]
    io::write_str(1, b"vconfig\n");
    #[cfg(feature = "vi")]
    io::write_str(1, b"vi\n");
    #[cfg(feature = "vi")]
    io::write_str(1, b"view\n");
    #[cfg(feature = "vmstat")]
    io::write_str(1, b"vmstat\n");
    #[cfg(feature = "w")]
    io::write_str(1, b"w\n");
    #[cfg(feature = "watch")]
    io::write_str(1, b"watch\n");
    #[cfg(feature = "watchdog")]
    io::write_str(1, b"watchdog\n");
    #[cfg(feature = "wc")]
    io::write_str(1, b"wc\n");
    #[cfg(feature = "wget")]
    io::write_str(1, b"wget\n");
    #[cfg(feature = "which")]
    io::write_str(1, b"which\n");
    #[cfg(feature = "who")]
    io::write_str(1, b"who\n");
    #[cfg(feature = "whoami")]
    io::write_str(1, b"whoami\n");
    #[cfg(feature = "xargs")]
    io::write_str(1, b"xargs\n");
    #[cfg(feature = "hexdump")]
    io::write_str(1, b"xxd\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"xz\n");
    #[cfg(feature = "xz")]
    io::write_str(1, b"xzcat\n");
    #[cfg(feature = "yes")]
    io::write_str(1, b"yes\n");
    #[cfg(feature = "gzip")]
    io::write_str(1, b"zcat\n");
    #[cfg(feature = "zstd")]
    io::write_str(1, b"zstd\n");
    #[cfg(feature = "zstd")]
    io::write_str(1, b"zstdcat\n");
    #[cfg(feature = "test")]
    io::write_str(1, b"[\n");
}
