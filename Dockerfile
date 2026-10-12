# Dockerfile for armybox
# Multi-stage build for minimal image size

# =============================================================================
# Stage 1: Build armybox static binary
# =============================================================================
# Track stable like the CI workflows; a pinned minor goes stale as the
# MSRV (rust-version in Cargo.toml) moves
FROM rust:1-bookworm AS builder

# Install musl tools for static linking
RUN apt-get update && apt-get install -y \
    musl-tools \
    musl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add musl target
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

# Copy source code (crates/ holds workspace members; .cargo/ holds the
# per-target rustflags, including +crt-static and -lc link ordering)
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY crates/ crates/
COPY src/ src/
COPY benches/ benches/
COPY tests/ tests/

# Build the static binary
RUN cargo build --release --target x86_64-unknown-linux-musl \
    --features full

# Strip the binary for smaller size
RUN strip /build/target/x86_64-unknown-linux-musl/release/armybox

# =============================================================================
# Stage 2: Minimal runtime image
# =============================================================================
FROM scratch AS runtime

# Copy the static binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/armybox /bin/armybox

# Create symlinks for common utilities
# Note: We'll use the --install feature at runtime instead

# Set up basic environment
ENV PATH="/bin:/usr/bin:/sbin:/usr/sbin"
ENV HOME="/root"

# Default entrypoint is the shell
ENTRYPOINT ["/bin/armybox"]
CMD ["sh"]

# =============================================================================
# Stage 3: Full image with symlinks installed
# =============================================================================
FROM scratch AS full

# Copy the static binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/armybox /bin/armybox

# We can't run --install in a FROM scratch image, so we create a helper
# Users should run: docker run --rm armybox --install /bin
# Or use the armybox:with-symlinks tag

ENV PATH="/bin:/usr/bin:/sbin:/usr/sbin"
ENV HOME="/root"

ENTRYPOINT ["/bin/armybox"]
CMD ["sh"]

# =============================================================================
# Stage 4: Alpine-based image with symlinks (for easier use)
# =============================================================================
FROM alpine:3.19 AS alpine

# Copy armybox binary
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/armybox /bin/armybox

# Install symlinks
RUN /bin/armybox --install /usr/local/bin

# Set up environment
ENV PATH="/usr/local/bin:/bin:/usr/bin:/sbin:/usr/sbin"

# Use armybox shell as default
SHELL ["/bin/armybox", "sh", "-c"]
CMD ["/bin/armybox", "sh"]
