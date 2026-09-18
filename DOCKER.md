# Docker Support for armybox

armybox provides Docker images for easy deployment and as a minimal container base.

## Quick Start

### Pull from GitHub Container Registry

```bash
# Minimal scratch-based image (~6MB)
docker pull ghcr.io/quinnjr/armybox:latest

# Alpine-based with symlinks installed
docker pull ghcr.io/quinnjr/armybox:alpine
```

### Run Interactive Shell

```bash
# Start armybox shell
docker run -it ghcr.io/quinnjr/armybox:alpine

# Run a specific command
docker run ghcr.io/quinnjr/armybox:alpine ls -la /
```

## Building Locally

### Build All Targets

```bash
# Build the minimal image
docker build --target runtime -t armybox:latest .

# Build Alpine-based image with symlinks
docker build --target alpine -t armybox:alpine .
```

### Using Docker Compose

```bash
# Build and run interactive shell
docker compose run --rm armybox-alpine

# Run development build
docker compose run --rm dev

# Run tests
docker compose run --rm test

# Run benchmarks
docker compose run --rm bench
```

## Image Variants

### `armybox:latest` (scratch-based)

- **Size**: ~6MB
- **Base**: `FROM scratch`
- **Contents**: Single `/bin/armybox` binary
- **Use case**: Minimal container base, security-focused deployments

```dockerfile
FROM ghcr.io/quinnjr/armybox:latest
# Binary is at /bin/armybox
ENTRYPOINT ["/bin/armybox", "sh"]
```

### `armybox:alpine`

- **Size**: ~12MB
- **Base**: Alpine Linux 3.19
- **Contents**: armybox + symlinks in `/usr/local/bin`
- **Use case**: Drop-in BusyBox replacement, easier debugging

```dockerfile
FROM ghcr.io/quinnjr/armybox:alpine
# All applets available as commands
RUN ls -la && cat /etc/os-release
```

## Using as Container Base

### Minimal Application Container

```dockerfile
FROM ghcr.io/quinnjr/armybox:latest AS base

# Copy your application
COPY --from=builder /app/myapp /app/myapp

# armybox provides shell for debugging
ENTRYPOINT ["/app/myapp"]
```

### Init System Container

```dockerfile
FROM ghcr.io/quinnjr/armybox:alpine

# armybox can run as PID 1
COPY inittab /etc/inittab
ENTRYPOINT ["/bin/armybox", "init"]
```

### Multi-stage Build Example

```dockerfile
# Build stage
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Runtime stage with armybox
FROM ghcr.io/quinnjr/armybox:latest
COPY --from=builder /app/target/release/myapp /app/myapp
CMD ["/app/myapp"]
```

## Available Commands

In the Alpine variant, all armybox applets are available:

```bash
# File operations
ls, cat, cp, mv, rm, mkdir, chmod, chown, ln, find

# Text processing
grep, sed, awk, head, tail, wc, sort, uniq, cut, tr

# Compression
gzip, gunzip, bzip2, tar, unzip

# Networking
ping, wget, nc, ifconfig, netstat

# System
ps, free, uname, date, kill, top

# Shell
sh, ash (armybox shell)

# And 150+ more...
```

Run `armybox --list` to see all available applets.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PATH` | `/bin:/usr/bin:/sbin:/usr/sbin` | Command search path |
| `HOME` | `/root` | Home directory |

## Security Considerations

1. **Scratch-based images** have minimal attack surface
2. **No shell history** is persisted by default
3. **Read-only filesystem** is supported
4. **Non-root user** can be configured

### Running as Non-root

```dockerfile
FROM ghcr.io/quinnjr/armybox:alpine
RUN adduser -D -u 1000 appuser
USER appuser
```

## Troubleshooting

### "exec format error"

The default images are built for `linux/amd64`. For ARM:

```bash
# Build for ARM64
docker build --platform linux/arm64 -t armybox:arm64 .
```

### "command not found"

In scratch-based images, only `/bin/armybox` exists. Use:

```bash
/bin/armybox ls -la
# or
/bin/armybox sh -c "ls -la"
```

### Debugging

```bash
# Get a shell in a running container
docker exec -it <container> /bin/armybox sh

# Check available commands
docker run armybox:alpine armybox --list
```

## Building for Multiple Architectures

```bash
# Create buildx builder
docker buildx create --use

# Build and push multi-arch
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --target alpine \
  -t ghcr.io/quinnjr/armybox:alpine \
  --push .
```

## License

MIT / Apache-2.0
