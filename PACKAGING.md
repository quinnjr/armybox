# Packaging armybox

This document describes how to build and install armybox packages for various Linux distributions.

## Quick Install

### Universal Installer Script

```bash
# Install latest version
curl -fsSL https://raw.githubusercontent.com/quinnjr/armybox/main/scripts/install.sh | bash

# Install with symlinks
curl -fsSL ... | bash -s -- --symlinks

# Install to custom prefix
curl -fsSL ... | bash -s -- --prefix=/opt/armybox

# Install from source
curl -fsSL ... | bash -s -- --source
```

### Manual Binary Installation

```bash
# Download static binary
wget https://github.com/quinnjr/armybox/releases/download/v0.1.0/armybox-x86_64-unknown-linux-musl

# Install
sudo install -m 755 armybox-* /usr/local/bin/armybox

# Create symlinks
sudo armybox --install /usr/local/bin
```

## Distribution Packages

### Debian / Ubuntu

#### Install from .deb

```bash
# Download
wget https://github.com/quinnjr/armybox/releases/download/v0.1.0/armybox_0.1.0_amd64.deb

# Install
sudo dpkg -i armybox_0.1.0_amd64.deb

# Create symlinks (optional)
sudo armybox-install-symlinks
```

#### Build from Source

```bash
# Install build dependencies
sudo apt install cargo rustc debhelper

# Build package
cd armybox
dpkg-buildpackage -us -uc -b

# Install
sudo dpkg -i ../armybox_*.deb
```

### Fedora / RHEL / CentOS

#### Install from .rpm

```bash
# Download
wget https://github.com/quinnjr/armybox/releases/download/v0.1.0/armybox-0.1.0-1.x86_64.rpm

# Install
sudo rpm -i armybox-0.1.0-1.x86_64.rpm

# Or with dnf
sudo dnf install ./armybox-0.1.0-1.x86_64.rpm
```

#### Build from Source

```bash
# Install build dependencies
sudo dnf install cargo rust rpm-build

# Build
cd armybox
./scripts/build-packages.sh rpm
```

### Arch Linux

#### Install from AUR

```bash
# Using yay
yay -S armybox

# Using paru
paru -S armybox

# Manual AUR build
git clone https://aur.archlinux.org/armybox.git
cd armybox
makepkg -si
```

#### Build from Source

```bash
cd armybox/packaging/archlinux
makepkg -si
```

### Alpine Linux

#### Install from Binary

```bash
# Download static musl binary
wget https://github.com/quinnjr/armybox/releases/download/v0.1.0/armybox-x86_64-unknown-linux-musl -O armybox

# Install
sudo install -m 755 armybox /usr/local/bin/

# Create symlinks
sudo armybox --install /usr/local/bin
```

#### Build from Source

```bash
# Install build dependencies
apk add cargo rust

# Build
cd armybox
cargo build --release

# Install
sudo install -m 755 target/release/armybox /usr/bin/
```

## Building Packages

### Prerequisites

| Distribution | Packages |
|-------------|----------|
| Debian/Ubuntu | `cargo rustc debhelper dpkg-dev` |
| Fedora/RHEL | `cargo rust rpm-build` |
| Arch Linux | `cargo rust` |
| Alpine | `cargo rust` |

### Build All Packages

```bash
# Build .deb and .rpm packages
./scripts/build-packages.sh all

# Build specific package
./scripts/build-packages.sh deb
./scripts/build-packages.sh rpm
```

### Output

Packages are created in `dist/packages/`:

```
dist/packages/
├── armybox_0.1.0_amd64.deb     # Debian/Ubuntu
├── armybox-0.1.0-1.x86_64.rpm  # Fedora/RHEL
└── armybox-0.1.0-1-x86_64.pkg.tar.zst  # Arch (if built on Arch)
```

## Symlink Installation

After installing armybox, you can create symlinks for all applets:

```bash
# Install to /usr/local/bin (requires root)
sudo armybox --install /usr/local/bin

# Or use the helper script
sudo armybox-install-symlinks

# Install to user directory (no root needed)
armybox --install ~/.local/bin

# Remove symlinks
armybox-install-symlinks --remove /usr/local/bin
```

## Docker

For container deployments, see [DOCKER.md](DOCKER.md).

```bash
# Use pre-built image
docker pull ghcr.io/quinnjr/armybox:alpine

# Build locally
docker build --target alpine -t armybox:alpine .
```

## Android

See [packaging/android/README.md](packaging/android/README.md) for detailed instructions.

### Termux (No Root)

```bash
# Quick install
curl -fsSL https://raw.githubusercontent.com/quinnjr/armybox/main/packaging/android/install-termux.sh | bash
```

### Magisk Module (Root)

```bash
# Build the module
./packaging/android/build-magisk-module.sh

# Install via Magisk Manager
# Output: dist/android/armybox-magisk-v0.1.0.zip
```

### ADB Sideload (Root)

```bash
# Build for Android
./scripts/build-android.sh

# Push to device
adb push dist/android/armybox-aarch64-linux-android /data/local/tmp/armybox
adb shell chmod +x /data/local/tmp/armybox
adb shell /data/local/tmp/armybox --list
```

## iOS (Jailbroken)

See [packaging/ios/README.md](packaging/ios/README.md) for detailed instructions.

### Cydia/Sileo Repository

1. Add repo: `https://quinnjr.github.io/repo/`
2. Search and install "armybox"

### Manual .deb Install

```bash
# Build (requires macOS)
./scripts/build-ios.sh package

# Transfer and install
scp dist/ios/armybox_0.1.0_iphoneos-arm64.deb root@<device-ip>:/var/mobile/
ssh root@<device-ip> dpkg -i /var/mobile/armybox_*.deb
```

### Supported Jailbreaks

| Jailbreak | iOS Version |
|-----------|-------------|
| unc0ver | 11.0 - 14.8 |
| checkra1n | 12.0 - 14.8.1 |
| Taurine | 14.0 - 14.8.1 |
| palera1n | 15.0 - 17.x |
| Dopamine | 15.0 - 16.6.1 |

## Verification

After installation, verify:

```bash
# Check version
armybox --version

# List applets
armybox --list

# Test some commands
armybox ls -la
armybox uname -a
armybox sh -c "echo Hello from armybox!"
```

## Uninstallation

### Debian/Ubuntu

```bash
sudo apt remove armybox
# Or
sudo dpkg -r armybox
```

### Fedora/RHEL

```bash
sudo dnf remove armybox
# Or
sudo rpm -e armybox
```

### Arch Linux

```bash
sudo pacman -R armybox
```

### Manual

```bash
# Remove symlinks first
armybox-install-symlinks --remove /usr/local/bin

# Remove binary
sudo rm /usr/local/bin/armybox
sudo rm /usr/local/bin/armybox-install-symlinks
```

## Package Contents

Each package includes:

| File | Description |
|------|-------------|
| `/usr/bin/armybox` | Main multi-call binary |
| `/usr/bin/armybox-install-symlinks` | Helper to manage symlinks |
| `/usr/share/doc/armybox/` | Documentation |

## License

armybox is dual-licensed under MIT and Apache-2.0.
