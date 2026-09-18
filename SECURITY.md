# Security Policy

## Supported Versions

We actively support the following versions with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :white_check_mark: |
| 0.1.x   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in Armybox, please report it responsibly.

### How to Report

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email us at: **quinn.josephr@protonmail.com**

Include the following information:

1. **Description**: A clear description of the vulnerability
2. **Impact**: What an attacker could do with this vulnerability
3. **Reproduction**: Steps to reproduce the issue
4. **Version**: Armybox version(s) affected
5. **Environment**: OS, architecture, and other relevant details
6. **Suggested Fix**: If you have one (optional but appreciated)

### What to Expect

- **Acknowledgment**: We'll acknowledge receipt within 48 hours
- **Initial Assessment**: Within 7 days, we'll provide an initial assessment
- **Updates**: We'll keep you informed of our progress
- **Credit**: We'll credit you in the security advisory (unless you prefer anonymity)

### Security Considerations

#### Memory Safety

Armybox is written in Rust, which provides strong memory safety guarantees:

- No buffer overflows
- No use-after-free
- No data races
- No null pointer dereferences

However, we use `unsafe` blocks for:
- FFI calls to libc
- Raw pointer operations in applet dispatch
- Custom allocator implementation

These areas receive extra scrutiny during code review.

#### Input Handling

All applets that process user input must:

1. Validate input sizes before allocation
2. Handle malformed input gracefully
3. Avoid unbounded recursion
4. Properly sanitize paths and filenames

#### Privilege Escalation

Applets that interact with system resources (mount, init, etc.) must:

1. Drop privileges when appropriate
2. Validate permissions before operations
3. Avoid TOCTOU vulnerabilities
4. Handle signals safely

### Known Security Considerations

1. **Shell Command Injection**: The shell (`sh`/`ash`/`dash`) executes user-provided commands. Users should be aware of standard shell security practices.

2. **Network Applets**: Networking utilities like `wget`, `nc`, and `ping` make network connections. Use appropriate firewall rules.

3. **Init System**: The init system runs as PID 1 with root privileges. Ensure `/etc/inittab` is properly secured.

4. **Vi Editor**: The vi editor can execute shell commands via `:!`. This is expected behavior.

### Security Best Practices

When using Armybox in production:

1. **Use Read-Only Filesystem**: Mount the armybox binary read-only
2. **Restrict Capabilities**: Use Linux capabilities to limit privileges
3. **Container Isolation**: In containers, use appropriate security contexts
4. **Regular Updates**: Keep armybox updated to the latest version

### Disclosure Policy

We follow a 90-day disclosure policy:

1. We'll work to fix the vulnerability within 90 days
2. After 90 days, or when a fix is released (whichever is first), we'll publish a security advisory
3. We'll coordinate with you on the disclosure timeline

### Security Advisories

Security advisories are published at:
- GitHub Security Advisories: https://github.com/quinnjr/armybox/security/advisories
- Our documentation: https://quinnjr.github.io/armybox/security

---

Thank you for helping keep Armybox and its users safe! 🛡️
