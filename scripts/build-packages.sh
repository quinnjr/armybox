#!/bin/bash
# build-packages.sh - Build distribution packages for armybox
#
# Usage: ./scripts/build-packages.sh [deb|rpm|arch|alpine|all]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | cut -d'"' -f2)
DIST_DIR="$PROJECT_DIR/dist/packages"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }

mkdir -p "$DIST_DIR"

# Build release binary first
build_binary() {
    log_info "Building release binary..."
    cd "$PROJECT_DIR"
    cargo build --release
    log_success "Binary built: target/release/armybox"
}

# Build Debian package
build_deb() {
    log_info "Building Debian package..."

    if ! command -v dpkg-deb &> /dev/null; then
        log_error "dpkg-deb not found. Install with: apt install dpkg-dev"
        return 1
    fi

    local pkg_dir="$DIST_DIR/armybox_${VERSION}_amd64"
    rm -rf "$pkg_dir"
    mkdir -p "$pkg_dir/DEBIAN"
    mkdir -p "$pkg_dir/usr/bin"
    mkdir -p "$pkg_dir/usr/share/doc/armybox"

    # Control file
    cat > "$pkg_dir/DEBIAN/control" << EOF
Package: armybox
Version: $VERSION
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Joseph R. Quinn <quinn.josephr@protonmail.com>
Description: BusyBox/Toybox clone written in Rust
 armybox is a modern implementation of common Unix utilities
 in a single multi-call binary, similar to BusyBox and Toybox.
 .
 Features: 303 applets, ash-compatible shell, init system.
Homepage: https://github.com/quinnjr/armybox
EOF

    # Post-install script
    cat > "$pkg_dir/DEBIAN/postinst" << 'EOF'
#!/bin/sh
echo "armybox installed. Run 'armybox --install /usr/local/bin' to create symlinks."
EOF
    chmod 755 "$pkg_dir/DEBIAN/postinst"

    # Copy files
    cp "$PROJECT_DIR/target/release/armybox" "$pkg_dir/usr/bin/"
    chmod 755 "$pkg_dir/usr/bin/armybox"

    cp "$PROJECT_DIR/packaging/debian/armybox-install-symlinks" "$pkg_dir/usr/bin/"
    chmod 755 "$pkg_dir/usr/bin/armybox-install-symlinks"

    cp "$PROJECT_DIR/README.md" "$pkg_dir/usr/share/doc/armybox/"

    # Build package
    dpkg-deb --root-owner-group --build "$pkg_dir"
    rm -rf "$pkg_dir"

    log_success "Debian package: $DIST_DIR/armybox_${VERSION}_amd64.deb"
}

# Build RPM package
build_rpm() {
    log_info "Building RPM package..."

    if ! command -v rpmbuild &> /dev/null; then
        log_error "rpmbuild not found. Install with: dnf install rpm-build"
        return 1
    fi

    local rpm_dir="$DIST_DIR/rpmbuild"
    rm -rf "$rpm_dir"
    mkdir -p "$rpm_dir"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    # Create source tarball
    local src_name="armybox-$VERSION"
    local tarball="$rpm_dir/SOURCES/$src_name.tar.gz"

    cd "$PROJECT_DIR/.."
    tar --transform "s,^armybox,$src_name," -czf "$tarball" \
        armybox/target/release/armybox \
        armybox/packaging \
        armybox/README.md \
        armybox/BENCHMARK.md \
        armybox/DOCKER.md

    # Copy spec file
    cp "$PROJECT_DIR/packaging/rpm/armybox.spec" "$rpm_dir/SPECS/"

    # Build RPM
    rpmbuild --define "_topdir $rpm_dir" -bb "$rpm_dir/SPECS/armybox.spec" 2>/dev/null || {
        # Simplified approach - create binary RPM directly
        log_info "Using simplified RPM build..."

        local rpm_name="armybox-${VERSION}-1.x86_64.rpm"
        cd "$PROJECT_DIR"

        # Use fpm if available
        if command -v fpm &> /dev/null; then
            fpm -s dir -t rpm \
                -n armybox \
                -v "$VERSION" \
                --description "BusyBox/Toybox clone written in Rust" \
                --url "https://github.com/quinnjr/armybox" \
                --license "MIT AND Apache-2.0" \
                target/release/armybox=/usr/bin/armybox \
                packaging/debian/armybox-install-symlinks=/usr/bin/armybox-install-symlinks
            mv armybox*.rpm "$DIST_DIR/"
        else
            log_error "Neither rpmbuild works nor fpm is available"
            return 1
        fi
    }

    log_success "RPM package built in $DIST_DIR/"
}

# Build Arch Linux package
build_arch() {
    log_info "Building Arch Linux package..."

    if ! command -v makepkg &> /dev/null; then
        log_error "makepkg not found. This must be run on Arch Linux."
        return 1
    fi

    local arch_dir="$DIST_DIR/archlinux"
    rm -rf "$arch_dir"
    mkdir -p "$arch_dir"

    # Copy PKGBUILD
    cp "$PROJECT_DIR/packaging/archlinux/PKGBUILD" "$arch_dir/"

    # For local build, modify PKGBUILD to use local source
    sed -i "s|source=.*|source=(\"armybox-$VERSION::git+file://$PROJECT_DIR\")|" "$arch_dir/PKGBUILD"
    sed -i "s|sha256sums=.*|sha256sums=('SKIP')|" "$arch_dir/PKGBUILD"

    cd "$arch_dir"
    makepkg -f --noconfirm

    mv *.pkg.tar.* "$DIST_DIR/" 2>/dev/null || true

    log_success "Arch package built in $DIST_DIR/"
}

# Build Alpine package
build_alpine() {
    log_info "Building Alpine package..."

    if ! command -v abuild &> /dev/null; then
        log_error "abuild not found. This must be run on Alpine Linux."
        return 1
    fi

    local alpine_dir="$DIST_DIR/alpine"
    rm -rf "$alpine_dir"
    mkdir -p "$alpine_dir"

    cp "$PROJECT_DIR/packaging/alpine/APKBUILD" "$alpine_dir/"

    cd "$alpine_dir"
    abuild -r

    log_success "Alpine package built"
}

# Show usage
usage() {
    cat << EOF
Build distribution packages for armybox

Usage: $0 [COMMAND]

Commands:
    deb         Build Debian/Ubuntu package (.deb)
    rpm         Build RPM package (Fedora/RHEL/CentOS)
    arch        Build Arch Linux package
    alpine      Build Alpine Linux package
    all         Build all packages
    binary      Build release binary only

Examples:
    $0 deb          # Build .deb package
    $0 all          # Build all packages
EOF
}

# Main
main() {
    if [ $# -eq 0 ]; then
        usage
        exit 0
    fi

    case "$1" in
        binary)
            build_binary
            ;;
        deb)
            build_binary
            build_deb
            ;;
        rpm)
            build_binary
            build_rpm
            ;;
        arch)
            build_binary
            build_arch
            ;;
        alpine)
            build_binary
            build_alpine
            ;;
        all)
            build_binary
            build_deb || true
            build_rpm || true
            # arch and alpine require their respective systems
            ;;
        *)
            log_error "Unknown command: $1"
            usage
            exit 1
            ;;
    esac

    echo ""
    log_info "Packages in $DIST_DIR:"
    ls -la "$DIST_DIR/" 2>/dev/null || true
}

main "$@"
