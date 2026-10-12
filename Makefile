# Makefile for armybox
#
# Usage:
#   make                  # Build native release
#   make static           # Build static musl binary
#   make all-targets      # Build for all architectures
#   make install          # Install to /usr/local/bin
#   make install-links    # Install with symlinks for all applets

CARGO := cargo
CROSS := cross
PREFIX := /usr/local
BINDIR := $(PREFIX)/bin
DESTDIR :=

# Targets
NATIVE_TARGET := $(shell rustc -vV | grep host | cut -d' ' -f2)
MUSL_TARGET := x86_64-unknown-linux-musl
ARM64_TARGET := aarch64-unknown-linux-musl
ARM32_TARGET := armv7-unknown-linux-musleabihf
MIPS_TARGET := mips-unknown-linux-musl
RISCV_TARGET := riscv64gc-unknown-linux-gnu

# Output directories
RELEASE_DIR := target/release
MUSL_RELEASE_DIR := target/$(MUSL_TARGET)/release
DIST_DIR := dist

# Binary name
BINARY := armybox

# Build flags for size optimization
RUSTFLAGS_SIZE := -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--as-needed
RUSTFLAGS_STATIC := -C target-feature=+crt-static \
                    -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--as-needed -C link-arg=-Wl,-s \
                    -C link-arg=-lc

.PHONY: all build release debug static small clean install uninstall \
        install-links uninstall-links test bench check lint \
        all-targets arm64 arm32 mips riscv dist help size-report \
        man install-man

# Default target
all: release

help:
	@echo "armybox Makefile"
	@echo ""
	@echo "Build targets:"
	@echo "  make              Build optimized release (~5.8 MB)"
	@echo "  make debug        Build native debug"
	@echo "  make static       Build static musl binary (~6.0 MB)"
	@echo "  make small        Build smallest binary with UPX (~1.7 MB)"
	@echo "  make arm64        Build for ARM64 (aarch64)"
	@echo "  make arm32        Build for ARM32 (armv7)"
	@echo "  make mips         Build for MIPS"
	@echo "  make riscv        Build for RISC-V 64"
	@echo "  make all-targets  Build for all architectures"
	@echo "  make size-report  Show binary sizes"
	@echo ""
	@echo "Install targets:"
	@echo "  make install         Install binary to $(BINDIR)"
	@echo "  make install-links   Install with symlinks for all applets"
	@echo "  make uninstall       Remove installed binary"
	@echo "  make uninstall-links Remove all symlinks"
	@echo ""
	@echo "Documentation:"
	@echo "  make man            Generate all man pages (hand-written + per-applet)"
	@echo "  make install-man    Install man pages to $(PREFIX)/share/man"
	@echo ""
	@echo "Development:"
	@echo "  make test       Run tests"
	@echo "  make bench      Run benchmarks"
	@echo "  make check      Run cargo check"
	@echo "  make lint       Run clippy"
	@echo "  make clean      Clean build artifacts"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX=$(PREFIX)"
	@echo "  DESTDIR=$(DESTDIR)"

# Build targets
build: release

release:
	@echo "Building optimized release binary..."
	RUSTFLAGS="$(RUSTFLAGS_SIZE)" $(CARGO) build --release
	@echo ""
	@echo "Binary size: $$(ls -lh $(RELEASE_DIR)/$(BINARY) | awk '{print $$5}')"

debug:
	$(CARGO) build

# Small build with UPX compression (smallest possible)
small: static
	@echo ""
	@echo "Compressing with UPX..."
	@if command -v upx >/dev/null 2>&1; then \
		upx --best --lzma $(DIST_DIR)/$(BINARY)-static -o $(DIST_DIR)/$(BINARY)-small 2>&1 | tail -5; \
		echo ""; \
		echo "Compressed binary: $(DIST_DIR)/$(BINARY)-small"; \
		ls -lh $(DIST_DIR)/$(BINARY)-small; \
	else \
		echo "Error: UPX not found. Install with: apt install upx-ucl"; \
		exit 1; \
	fi

size-report:
	@echo "=== Armybox Binary Size Report ==="
	@echo ""
	@echo "| Build Type | Size |"
	@echo "|------------|------|"
	@if [ -f $(RELEASE_DIR)/$(BINARY) ]; then \
		echo "| Release (glibc) | $$(ls -lh $(RELEASE_DIR)/$(BINARY) | awk '{print $$5}') |"; \
	fi
	@if [ -f $(DIST_DIR)/$(BINARY)-static ]; then \
		echo "| Static (musl) | $$(ls -lh $(DIST_DIR)/$(BINARY)-static | awk '{print $$5}') |"; \
	fi
	@if [ -f $(DIST_DIR)/$(BINARY)-small ]; then \
		echo "| Small (UPX) | $$(ls -lh $(DIST_DIR)/$(BINARY)-small | awk '{print $$5}') |"; \
	fi
	@echo ""
	@if [ -f $(RELEASE_DIR)/$(BINARY) ]; then \
		echo "Applets: $$($$([ -f $(DIST_DIR)/$(BINARY)-small ] && echo $(DIST_DIR)/$(BINARY)-small || echo $(RELEASE_DIR)/$(BINARY)) --list | wc -l)"; \
	fi

static:
	@echo "Building static binary with musl..."
	@rustup target add $(MUSL_TARGET) 2>/dev/null || true
	@if command -v musl-gcc >/dev/null 2>&1; then \
		echo "Using native musl-gcc..."; \
		CC=musl-gcc RUSTFLAGS="$(RUSTFLAGS_STATIC)" $(CARGO) build --release --target $(MUSL_TARGET); \
	elif command -v $(CROSS) >/dev/null 2>&1; then \
		echo "Using cross (Docker)..."; \
		$(CROSS) build --release --target $(MUSL_TARGET); \
	else \
		echo "Error: Neither musl-gcc nor cross found."; \
		echo "Install musl-tools: sudo apt install musl-tools"; \
		echo "Or install cross: cargo install cross"; \
		exit 1; \
	fi
	@mkdir -p $(DIST_DIR)
	@cp target/$(MUSL_TARGET)/release/$(BINARY) $(DIST_DIR)/$(BINARY)-static
	@echo "Static binary: $(DIST_DIR)/$(BINARY)-static"
	@file $(DIST_DIR)/$(BINARY)-static

# Cross-compilation targets
arm64:
	@echo "Building for ARM64 (aarch64)..."
	$(CROSS) build --release --target $(ARM64_TARGET)
	@mkdir -p $(DIST_DIR)
	@cp target/$(ARM64_TARGET)/release/$(BINARY) $(DIST_DIR)/$(BINARY)-arm64

arm32:
	@echo "Building for ARM32 (armv7)..."
	$(CROSS) build --release --target $(ARM32_TARGET)
	@mkdir -p $(DIST_DIR)
	@cp target/$(ARM32_TARGET)/release/$(BINARY) $(DIST_DIR)/$(BINARY)-arm32

mips:
	@echo "Building for MIPS..."
	$(CROSS) build --release --target $(MIPS_TARGET)
	@mkdir -p $(DIST_DIR)
	@cp target/$(MIPS_TARGET)/release/$(BINARY) $(DIST_DIR)/$(BINARY)-mips

riscv:
	@echo "Building for RISC-V 64..."
	$(CROSS) build --release --target $(RISCV_TARGET)
	@mkdir -p $(DIST_DIR)
	@cp target/$(RISCV_TARGET)/release/$(BINARY) $(DIST_DIR)/$(BINARY)-riscv64

all-targets: release static arm64 arm32 mips riscv
	@echo ""
	@echo "All targets built. Binaries in $(DIST_DIR)/"
	@ls -lh $(DIST_DIR)/

dist: all-targets
	@echo "Creating distribution archives..."
	@mkdir -p $(DIST_DIR)/archives
	@for bin in $(DIST_DIR)/$(BINARY)-*; do \
		name=$$(basename $$bin); \
		tar -czf $(DIST_DIR)/archives/$$name.tar.gz -C $(DIST_DIR) $$name; \
	done
	@ls -lh $(DIST_DIR)/archives/

# Installation
install: release
	@echo "Installing $(BINARY) to $(DESTDIR)$(BINDIR)/"
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 $(RELEASE_DIR)/$(BINARY) $(DESTDIR)$(BINDIR)/$(BINARY)
	@echo "Installed. Run '$(BINARY) --install $(DESTDIR)$(BINDIR)' to create symlinks."

install-links: install
	@echo "Creating symlinks for all applets..."
	$(DESTDIR)$(BINDIR)/$(BINARY) --install $(DESTDIR)$(BINDIR)

uninstall:
	@echo "Removing $(DESTDIR)$(BINDIR)/$(BINARY)"
	rm -f $(DESTDIR)$(BINDIR)/$(BINARY)

uninstall-links: uninstall
	@echo "Removing all applet symlinks..."
	@for applet in $$($(RELEASE_DIR)/$(BINARY) --list 2>/dev/null); do \
		rm -f $(DESTDIR)$(BINDIR)/$$applet; \
	done

# ── Man Pages ────────────────────────────────────────────────────────────────

MANDIR := man

# Generate all man pages (hand-written core pages are in man/; per-applet stubs
# are generated by gen-manpages.sh)
man:
	@echo "Generating per-applet man pages..."
	./scripts/gen-manpages.sh $(MANDIR)
	@echo ""
	@echo "Man pages ready in $(MANDIR)/"
	@n1=$$(ls $(MANDIR)/man1/*.1 2>/dev/null | wc -l); \
	 n5=$$(ls $(MANDIR)/man5/*.5 2>/dev/null | wc -l); \
	 n7=$$(ls $(MANDIR)/man7/*.7 2>/dev/null | wc -l); \
	 n8=$$(ls $(MANDIR)/man8/*.8 2>/dev/null | wc -l); \
	 echo "  Section 1 (commands):  $$n1 pages"; \
	 echo "  Section 5 (formats):   $$n5 pages"; \
	 echo "  Section 7 (reference): $$n7 pages"; \
	 echo "  Section 8 (admin):     $$n8 pages"
	@echo ""
	@echo "Preview:  man -l $(MANDIR)/man1/armybox.1"
	@echo "Install:  make install-man"

# Install man pages system-wide
install-man: man
	@echo "Installing man pages to $(DESTDIR)$(PREFIX)/share/man/"
	@for section in 1 5 7 8; do \
		dir="$(DESTDIR)$(PREFIX)/share/man/man$$section"; \
		install -d "$$dir"; \
		for page in $(MANDIR)/man$$section/*.$$section; do \
			[ -f "$$page" ] && install -m 644 "$$page" "$$dir/" || true; \
		done; \
	done
	@echo "Man pages installed. Run 'man armybox' to read."

# Development
test:
	$(CARGO) test

bench:
	$(CARGO) bench

check:
	$(CARGO) check

lint:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all --check

# Clean
clean:
	$(CARGO) clean
	rm -rf $(DIST_DIR)

# Size analysis
size: release
	@echo "Binary size analysis:"
	@ls -lh $(RELEASE_DIR)/$(BINARY)
	@echo ""
	@echo "Section sizes:"
	@size $(RELEASE_DIR)/$(BINARY) 2>/dev/null || true
	@echo ""
	@echo "Symbols by size (top 20):"
	@nm --size-sort --print-size $(RELEASE_DIR)/$(BINARY) 2>/dev/null | tail -20 || true

# Size comparison with BusyBox
compare: release
	@echo "Size comparison:"
	@echo ""
	@printf "%-20s %10s\n" "Binary" "Size"
	@printf "%-20s %10s\n" "------" "----"
	@printf "%-20s %10s\n" "armybox" "$$(du -h $(RELEASE_DIR)/$(BINARY) | cut -f1)"
	@if command -v busybox >/dev/null 2>&1; then \
		printf "%-20s %10s\n" "busybox" "$$(du -h $$(which busybox) | cut -f1)"; \
	fi
	@if command -v toybox >/dev/null 2>&1; then \
		printf "%-20s %10s\n" "toybox" "$$(du -h $$(which toybox) | cut -f1)"; \
	fi
