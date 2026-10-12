//! Configuration Management
//!
//! Parses and manages ABP configuration.

extern crate alloc;

use crate::io;
use alloc::string::String;
use alloc::vec::Vec;

/// ABP configuration
#[derive(Clone)]
pub struct Config {
    /// Root directory for package installation (default: /)
    pub root: String,
    /// Database directory (default: /var/lib/abp)
    pub db_dir: String,
    /// Cache directory (default: /var/cache/abp)
    pub cache_dir: String,
    /// Keys directory (default: /etc/abp/keys)
    pub keys_dir: String,
    /// Whether to verify signatures (default: true)
    pub verify_signatures: bool,
    /// Whether to verify checksums (default: true)
    pub verify_checksums: bool,
    /// HTTP proxy (optional)
    pub http_proxy: Option<String>,
    /// HTTPS proxy (optional)
    pub https_proxy: Option<String>,
    /// Connection timeout in seconds (default: 30)
    pub timeout: u32,
    /// Maximum parallel downloads (default: 4)
    pub parallel_downloads: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            root: String::from("/"),
            db_dir: String::from("/var/lib/abp"),
            cache_dir: String::from("/var/cache/abp"),
            keys_dir: String::from("/etc/abp/keys"),
            verify_signatures: true,
            verify_checksums: true,
            http_proxy: None,
            https_proxy: None,
            timeout: 30,
            parallel_downloads: 4,
        }
    }
}

impl Config {
    /// Load configuration from /etc/abp/abp.conf
    pub fn load() -> Self {
        let mut config = Config::default();

        let path = b"/etc/abp/abp.conf";
        let fd = io::open(path, libc::O_RDONLY, 0);
        if fd < 0 {
            return config;
        }

        let data = io::read_all(fd);
        io::close(fd);

        if let Ok(text) = core::str::from_utf8(&data) {
            config.parse(text);
        }

        config
    }

    /// Parse configuration text
    fn parse(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key = value
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                // Remove quotes if present
                let value = value.trim_matches('"').trim_matches('\'');

                self.set(key, value);
            }
        }
    }

    /// Set a configuration value
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "root" => self.root = String::from(value),
            "db_dir" => self.db_dir = String::from(value),
            "cache_dir" => self.cache_dir = String::from(value),
            "keys_dir" => self.keys_dir = String::from(value),
            "verify_signatures" => self.verify_signatures = parse_bool(value),
            "verify_checksums" => self.verify_checksums = parse_bool(value),
            "http_proxy" => {
                if !value.is_empty() {
                    self.http_proxy = Some(String::from(value));
                }
            }
            "https_proxy" => {
                if !value.is_empty() {
                    self.https_proxy = Some(String::from(value));
                }
            }
            "timeout" => self.timeout = value.parse().unwrap_or(30),
            "parallel_downloads" => self.parallel_downloads = value.parse().unwrap_or(4),
            _ => {}
        }
    }

    /// Get full path with root prefix
    pub fn root_path(&self, path: &str) -> String {
        if self.root == "/" {
            String::from(path)
        } else {
            let mut full = self.root.clone();
            if !path.starts_with('/') {
                full.push('/');
            }
            full.push_str(path);
            full
        }
    }
}

/// Parse a boolean value
fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

/// Repository configuration
#[derive(Clone)]
pub struct RepoConfig {
    /// Repository name
    pub name: String,
    /// Repository URL
    pub url: String,
    /// Whether repository is enabled
    pub enabled: bool,
    /// Priority (lower = higher priority)
    pub priority: u32,
}

impl RepoConfig {
    /// Load repository configurations from /etc/abp/repos.d/
    pub fn load_all() -> Vec<RepoConfig> {
        let repos = Vec::new();

        // In a full implementation, we'd enumerate /etc/abp/repos.d/
        // and parse each .conf file
        // For now, return empty list

        repos
    }

    /// Parse a repository configuration file
    pub fn parse(text: &str) -> Option<RepoConfig> {
        let mut config = RepoConfig {
            name: String::new(),
            url: String::new(),
            enabled: true,
            priority: 100,
        };

        for line in text.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');

                match key {
                    "name" => config.name = String::from(value),
                    "url" => config.url = String::from(value),
                    "enabled" => config.enabled = parse_bool(value),
                    "priority" => config.priority = value.parse().unwrap_or(100),
                    _ => {}
                }
            }
        }

        if config.name.is_empty() || config.url.is_empty() {
            return None;
        }

        Some(config)
    }
}

/// Initialize ABP directories and configuration
pub fn init_abp() -> Result<(), String> {
    let dirs = [
        b"/var/lib/abp".as_slice(),
        b"/var/lib/abp/packages",
        b"/var/lib/abp/repos",
        b"/var/lib/abp/available",
        b"/var/cache/abp",
        b"/etc/abp",
        b"/etc/abp/repos.d",
        b"/etc/abp/keys",
    ];

    for dir in &dirs {
        if io::access(dir, libc::F_OK) != 0 && io::mkdir(dir, 0o755) != 0 {
            let mut msg = String::from("cannot create directory: ");
            if let Ok(s) = core::str::from_utf8(dir) {
                msg.push_str(s);
            }
            return Err(msg);
        }
    }

    // Create default config if it doesn't exist
    let config_path = b"/etc/abp/abp.conf";
    if io::access(config_path, libc::F_OK) != 0 {
        let default_config = b"# ABP Package Manager Configuration
#
# Root directory for package installation
# root = /
#
# Database directory
# db_dir = /var/lib/abp
#
# Cache directory for downloaded packages
# cache_dir = /var/cache/abp
#
# Directory for trusted signing keys
# keys_dir = /etc/abp/keys
#
# Verify package signatures (recommended)
verify_signatures = true
#
# Verify file checksums
verify_checksums = true
#
# HTTP proxy (optional)
# http_proxy = http://proxy:8080
#
# HTTPS proxy (optional)
# https_proxy = http://proxy:8080
#
# Connection timeout in seconds
timeout = 30
#
# Maximum parallel downloads
parallel_downloads = 4
";

        let fd = io::open(
            config_path,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            0o644,
        );
        if fd >= 0 {
            io::write_all(fd, default_config);
            io::close(fd);
        }
    }

    Ok(())
}
