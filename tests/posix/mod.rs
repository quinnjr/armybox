//! POSIX.1-2017 compliance tests
//!
//! Tests verify behavior matches POSIX specifications.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/
//!
//! **Important:** These tests require a pre-built armybox binary.
//! Run: `RUSTFLAGS="-C linker=gcc -C link-arg=-lc" cargo build --release`
//! before running these tests.

pub mod helpers;

// Re-export helpers for backward compatibility
pub use helpers::{get_armybox_path, setup_test_env};

mod file;
mod misc;
mod process;
mod system;
mod text;
