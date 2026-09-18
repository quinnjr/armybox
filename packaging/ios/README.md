# iOS (Jailbroken) Installation

armybox can be installed on jailbroken iOS devices via Cydia, Sileo, or Zebra.

## Supported Jailbreaks

| Jailbreak | iOS Version | Devices |
|-----------|-------------|---------|
| unc0ver | 11.0 - 14.8 | A9-A13 |
| checkra1n | 12.0 - 14.8.1 | A7-A11 |
| Taurine | 14.0 - 14.8.1 | A9-A14 |
| palera1n | 15.0 - 17.x | A9-A11 |
| Dopamine | 15.0 - 16.6.1 | A12+ |

## Method 1: Add Repository (Recommended)

### For Cydia/Sileo/Zebra

1. Open your package manager
2. Go to Sources/Repos → Add
3. Enter: `https://quinnjr.github.io/repo/`
4. Search for "armybox" and install

## Method 2: Download .deb Manually

### Download

```bash
# From a computer
wget https://github.com/quinnjr/armybox/releases/download/v0.1.0/armybox_0.1.0_iphoneos-arm64.deb

# Transfer to device
scp armybox_*.deb root@<device-ip>:/var/mobile/
```

### Install via Terminal

```bash
# SSH into device
ssh root@<device-ip>

# Install
dpkg -i /var/mobile/armybox_*.deb

# Create symlinks
armybox --install /usr/local/bin
```

### Install via Filza

1. Copy the .deb to your device
2. Open Filza File Manager
3. Navigate to the .deb file
4. Tap → Install

## Method 3: Build from Source

### Prerequisites

- macOS with Xcode
- Theos installed: https://theos.dev/docs/installation
- iOS SDK

### Setup

```bash
# Install Theos
bash -c "$(curl -fsSL https://raw.githubusercontent.com/theos/theos/master/bin/install-theos)"

# Add iOS Rust target
rustup target add aarch64-apple-ios

# For older devices (32-bit)
rustup target add armv7-apple-ios
```

### Build

```bash
cd armybox/packaging/ios
make package
```

The .deb will be in `packages/`.

## Supported Architectures

| Architecture | Target | Devices |
|-------------|--------|---------|
| ARM64 | `aarch64-apple-ios` | iPhone 5s+ |
| ARM64e | `aarch64-apple-ios` | iPhone XS+ (A12+) |

Note: ARM64e devices run ARM64 binaries via Rosetta-like compatibility.

## File Locations

After installation:

| Path | Description |
|------|-------------|
| `/usr/bin/armybox` | Main binary |
| `/usr/local/bin/*` | Symlinks to applets |

## Usage

```bash
# List all applets
armybox --list

# Use directly
armybox ls -la

# Or via symlinks
ls -la
cat /etc/hosts
```

## Troubleshooting

### "Killed: 9" or Immediate Crash

This usually means the binary isn't properly signed. Re-sign with:

```bash
ldid -S /usr/bin/armybox
```

Or with entitlements:

```bash
ldid -S/path/to/entitlements.plist /usr/bin/armybox
```

### "Operation not permitted"

Some operations require root. Use:

```bash
sudo armybox <command>
```

Or run as root:

```bash
su
armybox <command>
```

### Binary won't run on A12+ devices

Ensure you're using the ARM64 build. ARM64e is backwards compatible.

## Uninstallation

### Via Package Manager

Search for "armybox" → Uninstall

### Manual

```bash
# Remove symlinks
armybox --install --remove /usr/local/bin

# Remove binary
rm /usr/bin/armybox

# Remove package metadata
rm -rf /var/lib/dpkg/info/dev.quinnjr.armybox.*
```
