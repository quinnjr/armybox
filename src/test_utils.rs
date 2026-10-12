//! Test utilities for armybox integration tests
//!
//! Provides unique temp directory creation to avoid conflicts in parallel tests.

#[cfg(test)]
pub mod test_helpers {
    extern crate std;

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Create a unique temp directory for this test.
    /// Uses atomic counter + process ID to ensure uniqueness across parallel tests.
    pub fn create_test_dir(prefix: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("{}_{}_{}", prefix, pid, id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("Failed to create test directory");
        dir
    }

    /// Clean up a test directory
    pub fn cleanup_test_dir(dir: &std::path::Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Get path to armybox binary
    pub fn get_armybox_path() -> PathBuf {
        if let Ok(path) = std::env::var("ARMYBOX_PATH") {
            return PathBuf::from(path);
        }
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap());
        let release = manifest_dir.join("target/release/armybox");
        if release.exists() {
            return release;
        }
        manifest_dir.join("target/debug/armybox")
    }
}
