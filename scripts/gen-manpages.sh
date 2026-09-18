#!/bin/sh
# gen-manpages.sh — Generate individual man pages for every armybox applet
#
# Creates a man page per applet that includes a brief description, synopsis,
# and pointers to armybox(1) and the POSIX specification where applicable.
#
# Usage:
#   ./scripts/gen-manpages.sh [output-dir]
#
# Output: man/man1/*.1 and man/man8/*.8
#
# Copyright (c) 2026 Joseph R. Quinn

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="${1:-$ROOT_DIR/man}"

mkdir -p "$OUT/man1" "$OUT/man8"

VERSION="0.3.0"
DATE="2026-02-08"

# ── Applet database ──────────────────────────────────────────────────────────
# Format: name|section|posix|description
# section: 1=user command, 8=admin command
# posix: P=POSIX, L=Linux-specific, (empty)=other

APPLETS='
basename|1|P|Strip directory and suffix from filenames
cat|1|P|Concatenate files and print on standard output
cd|1|P|Change the working directory
chattr|1|L|Change file attributes on a Linux filesystem
chgrp|1|P|Change group ownership of files
chmod|1|P|Change file mode bits
chown|1|P|Change file owner and group
cp|1|P|Copy files and directories
dd|1|P|Convert and copy a file
dirname|1|P|Strip last component from filenames
file|1||Determine file type
find|1|P|Search for files in a directory hierarchy
fstype|1|L|Print the filesystem type of a block device
install|1||Copy files and set attributes
link|1|P|Create a hard link to a file
ln|1|P|Make links between files
ls|1|P|List directory contents
lsattr|1|L|List file attributes on a Linux filesystem
makedevs|8|L|Create a range of special files as specified in a device table
mkdir|1|P|Make directories
mkfifo|1|P|Make FIFOs (named pipes)
mknod|1|P|Make block or character special files
mktemp|1||Create a temporary file or directory
mv|1|P|Move (rename) files
patch|1|P|Apply a diff file to an original
pwd|1|P|Print name of current working directory
readlink|1||Print resolved symbolic links or canonical filenames
realpath|1||Print the resolved absolute pathname
rm|1|P|Remove files or directories
rmdir|1|P|Remove empty directories
setfattr|1|L|Set extended attributes of filesystem objects
shred|1||Overwrite a file to hide its contents and optionally delete it
split|1|P|Split a file into pieces
stat|1||Display file or filesystem status
sync|1|P|Synchronize cached writes to persistent storage
touch|1|P|Change file timestamps
truncate|1||Shrink or extend the size of a file
unlink|1|P|Remove a directory entry
xargs|1|P|Build and execute command lines from standard input
awk|1|P|Pattern scanning and text processing language
comm|1|P|Compare two sorted files line by line
cut|1|P|Remove sections from each line of files
dos2unix|1||Convert DOS/Mac line endings to Unix
echo|1|P|Display a line of text
expand|1|P|Convert tabs to spaces
fmt|1||Simple optimal text formatter
fold|1|P|Wrap each input line to fit in specified width
grep|1|P|Print lines matching a pattern
egrep|1|P|Print lines matching an extended regular expression
fgrep|1|P|Print lines matching a fixed string
head|1|P|Output the first part of files
nl|1|P|Number lines of files
paste|1|P|Merge lines of files
printf|1|P|Format and print data
rev|1||Reverse lines characterwise
sed|1|P|Stream editor for filtering and transforming text
seq|1||Print a sequence of numbers
sort|1|P|Sort lines of text files
strings|1||Print the sequences of printable characters in files
tac|1||Concatenate and print files in reverse
tail|1|P|Output the last part of files
tee|1|P|Read from standard input and write to standard output and files
tr|1|P|Translate or delete characters
unexpand|1|P|Convert spaces to tabs
uniq|1|P|Report or omit repeated lines
unix2dos|1||Convert Unix line endings to DOS/Mac
wc|1|P|Print newline, word, and byte counts for each file
yes|1||Output a string repeatedly until killed
acpi|1|L|Show battery status and other ACPI information
arch|1||Print machine hardware name
blkdiscard|8|L|Discard the content of sectors on a block device
blkid|8|L|Locate and print block device attributes
blockdev|8|L|Call block device ioctls from the command line
cal|1|P|Display a calendar
chroot|8||Run command or shell with special root directory
chvt|1|L|Change foreground virtual terminal
date|1|P|Print or set the system date and time
deallocvt|1|L|Deallocate unused virtual terminals
devmem|1|L|Read or write from physical address
df|1|P|Report filesystem disk space usage
dmesg|8|L|Print or control the kernel ring buffer
dnsdomainname|1||Show the system DNS domain name
du|1|P|Estimate file space usage
eject|1|L|Eject removable media
env|1|P|Run a program in a modified environment
fallocate|1|L|Preallocate or deallocate space to a file
fgconsole|1|L|Print the number of the active VT
flock|1||Manage file locks from shell scripts
free|1|L|Display amount of free and used memory in the system
freeramdisk|8|L|Free all memory used by the specified ramdisk
fsfreeze|8|L|Suspend access to a filesystem
fsync|1||Synchronize a file in-core state with storage device
groups|1|P|Print the groups a user is in
halt|8||Halt the machine
reboot|8||Reboot the machine
poweroff|8||Power off the machine
hostid|1|P|Print the numeric identifier for the current host
hostname|1|P|Show or set the system hostname
hwclock|8|L|Query or set the hardware clock (RTC)
id|1|P|Print real and effective user and group IDs
insmod|8|L|Insert a module into the Linux kernel
rmmod|8|L|Remove a module from the Linux kernel
modprobe|8|L|Add or remove modules from the Linux kernel
logger|1|P|Make entries in the system log
login|1||Begin a session on the system
logname|1|P|Print user login name
losetup|8|L|Set up and control loop devices
lsmod|8|L|Show the status of kernel modules
lspci|8|L|List all PCI devices
lsusb|8|L|List USB devices
mkswap|8|L|Set up a Linux swap area
modinfo|8|L|Show information about a Linux kernel module
mount|8||Mount a filesystem
umount|8||Unmount a filesystem
mountpoint|1|L|Test whether a directory is a mountpoint
nologin|8||Politely refuse a login
nproc|1|P|Print the number of processing units available
openvt|1|L|Start a program on a new virtual terminal
partprobe|8|L|Inform the OS of partition table changes
pivot_root|8|L|Change the root filesystem
printenv|1|P|Print all or part of environment
readahead|8|L|Initiate readahead on files
rfkill|8|L|Enable and disable wireless devices
rtcwake|8|L|Enter a system sleep state until specified wakeup time
shuf|1||Generate random permutations
sleep|1|P|Delay for a specified amount of time
usleep|1||Sleep for the specified number of microseconds
su|1||Run a command with substitute user and group ID
swapon|8|L|Enable devices for paging and swapping
swapoff|8|L|Disable devices for paging and swapping
sysctl|8|L|Configure kernel parameters at runtime
tty|1|P|Print the terminal file name
ulimit|1|P|Get and set user limits
uname|1|P|Print system information
uptime|1||Tell how long the system has been running
vmstat|8|L|Report virtual memory statistics
w|1||Show who is logged on and what they are doing
watch|1||Execute a program periodically showing output fullscreen
who|1|P|Show who is logged on
whoami|1|P|Print effective userid
users|1|P|Print the user names of users currently logged in
chrt|1|L|Manipulate the real-time scheduling attributes of a process
ionice|1|L|Set or get process I/O scheduling class and priority
iorenice|1|L|Change I/O scheduling class and priority of a running process
iotop|8|L|Simple top-like I/O monitor
kill|1|P|Send a signal to a process
killall|1||Kill processes by name
killall5|8|L|Send a signal to all processes
nice|1|P|Run a program with modified scheduling priority
nohup|1|P|Run a command immune to hangups
nsenter|1|L|Run a program in existing Linux namespaces
pgrep|1|P|Look up processes based on name and other attributes
pkill|1|P|Signal processes based on name and other attributes
pidof|1|L|Find the process ID of a running program
pmap|1|L|Report memory map of a process
prlimit|1|L|Get and set a process resource limits
ps|1|P|Report a snapshot of the current processes
pwdx|1|L|Report current working directory of a process
renice|1|P|Alter priority of running processes
setsid|1||Run a program in a new session
taskset|1|L|Set or retrieve a process CPU affinity
timeout|1||Start a command and kill it if still running after a duration
top|1|L|Display Linux processes
uclampset|1|L|Set or get utilization clamping attributes of a process
unshare|1|L|Run a program in new Linux namespaces
arp|8|L|Manipulate the system ARP cache
arping|8|L|Send ARP requests to a neighbour host
brctl|8|L|Ethernet bridge administration
ether-wake|8|L|Send a Wake-on-LAN magic packet
ftpget|1||Retrieve a file via FTP
ftpput|1||Upload a file via FTP
host|1||DNS lookup utility
httpd|8||A small HTTP server
ifconfig|8|L|Configure a network interface
ifup|8|L|Bring a network interface up
ifdown|8|L|Bring a network interface down
ip|8|L|Show and manipulate routing, devices, policy routing, and tunnels
ipaddr|8|L|Protocol address management
iplink|8|L|Network device configuration
ipneigh|8|L|Neighbour/ARP table management
iproute|8|L|Routing table management
iprule|8|L|Routing policy database management
ipcalc|1||Calculate IP network settings from an address and netmask
microcom|1||Minimalistic terminal emulator for serial communication
nameif|8|L|Name network interfaces based on MAC addresses
nbd-client|8|L|Connect to a network block device server
nbd-server|8|L|Serve a file as a network block device
nc|1||Arbitrary TCP and UDP connections and listens
netcat|1||Arbitrary TCP and UDP connections and listens
netstat|1|P|Print network connections, routing tables, interface statistics
nslookup|1||Query Internet name servers interactively
ping|1||Send ICMP ECHO_REQUEST to network hosts
ping6|1||Send ICMPv6 ECHO_REQUEST to network hosts
route|8|L|Show and manipulate the IP routing table
slattach|8|L|Attach a network interface to a serial line
sntp|1||Simple Network Time Protocol client
ss|8|L|Socket statistics
telnet|1||User interface to the TELNET protocol
tftp|1||Trivial file transfer protocol client
traceroute|1||Trace the route to a network host
traceroute6|1||Trace the IPv6 route to a network host
tunctl|8|L|Create or delete TUN/TAP interfaces
vconfig|8|L|VLAN (802.1Q) configuration
wget|1||Non-interactive network downloader
bzip2|1|P|Block-sorting file compressor
bunzip2|1|P|Decompress bzip2 compressed files
bzcat|1|P|Decompress bzip2 files to stdout
compress|1|P|Compress data using adaptive Lempel-Ziv coding
uncompress|1|P|Expand compressed data
cpio|1|P|Copy files to and from archives
gzip|1|P|Compress or expand files
gunzip|1|P|Decompress gzip compressed files
zcat|1|P|Decompress gzip files to stdout
lzma|1||LZMA compression utility
unlzma|1||Decompress LZMA compressed files
lzcat|1||Decompress LZMA files to stdout
tar|1|P|Manipulate tape archives
unzip|1||Extract files from ZIP archives
xz|1||XZ compression utility
unxz|1||Decompress XZ compressed files
xzcat|1||Decompress XZ files to stdout
zstd|1||Zstandard compression utility
unzstd|1||Decompress Zstandard compressed files
zstdcat|1||Decompress Zstandard files to stdout
sh|1|P|POSIX-compliant command interpreter
ash|1|P|POSIX-compliant command interpreter (alias for sh)
dash|1|P|POSIX-compliant command interpreter (alias for sh)
expr|1|P|Evaluate expressions
getopt|1||Parse command options
test|1|P|Evaluate conditional expression
true|1|P|Return true (exit status 0)
false|1|P|Return false (exit status 1)
time|1|P|Time a simple command
vi|1||Screen-oriented (visual) text editor
view|1||Read-only text editor (vi in read-only mode)
hexedit|1||View and edit binary files in hexadecimal
init|8||System initialization process (PID 1)
linuxrc|8||Initramfs initialization process
getty|8||Open a terminal and set its mode
sulogin|8||Single-user login
telinit|8||Change SysV runlevel
runlevel|8|L|Print previous and current SysV runlevel
switch_root|8|L|Switch to another filesystem as the root
oneit|8||Simple init that runs a single process
watchdog|8|L|Periodically write to a watchdog device
ascii|1||Print the ASCII character table
base32|1||Base32 encode or decode data
base64|1||Base64 encode or decode data
cksum|1|P|Checksum and count the bytes in a file
crc32|1||Compute CRC-32 checksum
clear|1||Clear the terminal screen
reset|1||Reset the terminal
cmp|1|P|Compare two files byte by byte
count|1||Count lines, words, and bytes from stdin
diff|1|P|Compare files line by line
factor|1||Print prime factors of numbers
getconf|1|P|Get system configuration values
hd|1||Hexadecimal dump (alias for hexdump)
hexdump|1||Display files in hexadecimal, octal, decimal, or ASCII
od|1|P|Dump files in various formats
xxd|1||Make a hexdump or do the reverse
help|1||Display armybox help information
iconv|1|P|Convert text between character encodings
mcookie|1||Generate random 128-bit hexadecimal numbers
md5sum|1||Compute and check MD5 message digest
sha1sum|1||Compute and check SHA-1 message digest
sha224sum|1||Compute and check SHA-224 message digest
sha256sum|1||Compute and check SHA-256 message digest
sha384sum|1||Compute and check SHA-384 message digest
sha512sum|1||Compute and check SHA-512 message digest
sha3sum|1||Compute and check SHA-3 message digest
mesg|1|P|Display or set permission to receive messages
mkpasswd|1||Generate a password hash
pwgen|1||Generate pronounceable passwords
readelf|1||Display information about ELF files
screen|1||Terminal multiplexer
tmux|1||Terminal multiplexer (alias for screen)
toybox|1||BusyBox/Toybox compatibility alias
ts|1||Timestamp standard input
tsort|1|P|Topological sort
unicode|1||Look up Unicode character names and codepoints
uuencode|1|P|Encode a binary file for UUCP transmission
uudecode|1|P|Decode a uuencoded file
uuidgen|1||Create a new UUID
which|1|P|Locate a command
abp|1||ArmyBox Package manager
apk|8||Alpine Package Keeper compatibility layer
gpiodetect|1|L|List all gpiochips present on the system
gpiofind|1|L|Find a GPIO line by name
gpioget|1|L|Read values from a GPIO line
gpioinfo|1|L|List all lines of specified gpiochips
gpioset|1|L|Set values of a GPIO line
i2cdetect|8|L|Detect I2C chips
i2cdump|8|L|Examine I2C registers
i2cget|8|L|Read from I2C/SMBus chip registers
i2cset|8|L|Set I2C registers
i2ctransfer|8|L|Send user-defined I2C messages
'

count=0
total=$(echo "$APPLETS" | grep -c '|')

echo "==> Generating per-applet man pages ($total applets)"

echo "$APPLETS" | while IFS='|' read -r name section posix desc; do
    # Skip blank lines
    [ -z "$name" ] && continue

    count=$((count + 1))
    outdir="$OUT/man${section}"
    outfile="$outdir/${name}.${section}"

    # Section title
    case "$section" in
        1) section_title="User Commands" ;;
        5) section_title="File Formats" ;;
        7) section_title="Miscellaneous" ;;
        8) section_title="System Administration" ;;
        *) section_title="ArmyBox Manual" ;;
    esac

    # POSIX note
    posix_note=""
    if [ "$posix" = "P" ]; then
        posix_note=".PP
This is a POSIX.1-2017 utility. The armybox implementation aims for full
compliance with IEEE\\ Std\\ 1003.1-2017.
.PP
See
.UR https://pubs.opengroup.org/onlinepubs/9699919799/utilities/${name}.html
POSIX specification for ${name}
.UE
for the authoritative reference."
    elif [ "$posix" = "L" ]; then
        posix_note=".PP
This is a Linux-specific utility."
    fi

    # Build the man page
    cat > "$outfile" << MANPAGE
.\\" ${name}(${section}) man page — generated by gen-manpages.sh
.\\" Copyright (c) 2026 Joseph R. Quinn
.\\"
.TH ${name} ${section} "${DATE}" "armybox ${VERSION}" "${section_title}"
.\\"
.SH NAME
${name} \\- ${desc}
.\\"
.SH SYNOPSIS
.B ${name}
.RI [ OPTIONS ]
.RI [ ARGUMENTS .\\|.\\|.]
.\\"
.SH DESCRIPTION
.B ${name}
is provided by
.BR armybox (1),
a multi-call binary.
It is typically invoked through a symbolic link.
.PP
${desc}.
${posix_note}
.\\"
.SH OPTIONS
Run
.B "${name} --help"
for a summary of supported options, or see
.BR armybox-applets (7)
for the full applet list.
.\\"
.SH EXIT STATUS
.TP
.B 0
Success.
.TP
.B 1
General error.
.TP
.B 2
Incorrect usage.
.\\"
.SH SEE ALSO
.BR armybox (1),
.BR armybox-applets (7)
.\\"
.SH AUTHORS
Joseph R. Quinn.
MANPAGE

done

# Count generated files
n1=$(ls "$OUT/man1"/*.1 2>/dev/null | wc -l | tr -d ' ')
n8=$(ls "$OUT/man8"/*.8 2>/dev/null | wc -l | tr -d ' ')
ntotal=$((n1 + n8))

echo "==> Generated $n1 section-1 + $n8 section-8 = $ntotal man pages"
echo "    Output: $OUT/man1/ and $OUT/man8/"
echo ""
echo "Preview with:  man -l $OUT/man1/ls.1"
echo "Install with:  cp -r $OUT/man* /usr/share/man/"
