# Contributing to Armybox

First off, thank you for considering contributing to Armybox! It's people like you that make Armybox such a great tool.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [How Can I Contribute?](#how-can-i-contribute)
- [Development Setup](#development-setup)
- [Coding Guidelines](#coding-guidelines)
- [Submitting Changes](#submitting-changes)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Features](#suggesting-features)

## Code of Conduct

This project and everyone participating in it is governed by the [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

- Make sure you have a [GitHub account](https://github.com/signup)
- Fork the repository on GitHub
- Clone your fork locally
- Set up the development environment (see below)

## How Can I Contribute?

### Good First Issues

Look for issues labeled `good first issue` - these are great for newcomers!

### Areas That Need Help

- **New Applets**: Adding missing Unix utilities
- **POSIX Compliance**: Improving compliance with POSIX.1-2017
- **Documentation**: Improving docs, examples, and tutorials
- **Testing**: Adding tests for edge cases
- **Performance**: Optimizing applet implementations
- **Cross-Platform**: Improving support for different architectures

## Development Setup

### Prerequisites

- **Rust 2024** (1.94+) - Install via [rustup](https://rustup.rs)
- **Git** - For version control
- **UPX** (optional) - For binary compression testing

### Clone and Build

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/armybox
cd armybox

# Add upstream remote
git remote add upstream https://github.com/quinnjr/armybox

# Build
cargo build

# Run tests
cargo test

# Build release
cargo build --release
```

### Running Specific Applets

```bash
# Run directly
./target/debug/armybox ls -la

# Run via symlink
ln -sf target/debug/armybox ls
./ls -la
```

## Coding Guidelines

### General Principles

1. **`#[no_std]` First**: All code must work in `no_std` environments
2. **Direct libc Calls**: Use raw `libc::*` functions, not std wrappers
3. **Memory Safety**: Leverage Rust's ownership system
4. **Minimal Allocations**: Prefer stack allocation when possible
5. **POSIX Compliance**: Follow POSIX.1-2017 specifications

### Code Style

```rust
// Good: Direct libc usage
pub fn cat(argc: i32, argv: *const *const u8) -> i32 {
    // Implementation using libc calls
}

// Bad: Using std abstractions
pub fn cat(args: &[String]) -> Result<(), std::io::Error> {
    // This won't work in no_std!
}
```

### Applet Structure

Each applet should:

1. Follow the signature `fn(i32, *const *const u8) -> i32`
2. Return 0 on success, non-zero on error
3. Write errors to stderr (fd 2)
4. Support standard options (--help, --version)
5. Be registered in `src/applets/mod.rs`

### Example Applet

```rust
/// basename - strip directory from filenames
pub fn basename(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"usage: basename path [suffix]\n");
        return 1;
    }

    let path = unsafe { get_arg(argv, 1) }.unwrap_or(b"");

    // Find last component
    let name = match path.iter().rposition(|&c| c == b'/') {
        Some(pos) => &path[pos + 1..],
        None => path,
    };

    io::write_all(1, name);
    io::write_str(1, b"\n");
    0
}
```

### Testing

- Add unit tests in the same file
- Add integration tests in `tests/`
- Test edge cases (empty input, large files, etc.)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basename_simple() {
        // Test implementation
    }
}
```

## Submitting Changes

### Branch Naming

- `feature/applet-name` - New applets
- `fix/issue-description` - Bug fixes
- `docs/what-changed` - Documentation
- `perf/what-improved` - Performance

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`

Examples:
```
feat(applets): add screen terminal multiplexer
fix(ls): handle symlinks correctly
docs: update installation instructions
perf(grep): optimize pattern matching
```

### Pull Request Process

1. Update the CHANGELOG.md with your changes
2. Update documentation if needed
3. Ensure all tests pass: `cargo test`
4. Ensure it compiles in release: `cargo build --release`
5. Check binary size hasn't regressed significantly
6. Create a Pull Request with a clear description

### Pull Request Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature (applet)
- [ ] Documentation
- [ ] Performance improvement

## Checklist
- [ ] Code follows project style
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Compiles with no warnings
```

## Reporting Bugs

### Before Submitting

1. Check existing issues
2. Try the latest version
3. Verify it's reproducible

### Bug Report Template

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce:
1. Run command '...'
2. See error

**Expected behavior**
What you expected to happen.

**Environment**
- OS: [e.g., Ubuntu 22.04]
- Arch: [e.g., x86_64]
- Armybox version: [e.g., 0.3.0]

**Additional context**
Any other relevant information.
```

## Suggesting Features

### Feature Request Template

```markdown
**Is your feature request related to a problem?**
A clear description of what the problem is.

**Describe the solution you'd like**
What you want to happen.

**Describe alternatives you've considered**
Any alternative solutions or features.

**Which applet/component?**
[e.g., new applet, shell, vi, etc.]

**POSIX Reference** (if applicable)
Link to POSIX specification for the feature.
```

## Questions?

Feel free to open an issue with the `question` label or reach out to the maintainers.

---

Thank you for contributing! 🪖
