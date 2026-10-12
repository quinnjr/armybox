//! Pure Rust no_std Zstandard (zstd) compression and decompression
//!
//! Implements RFC 8878 with no external dependencies beyond alloc.

#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bitstream;
pub mod common;
pub mod compress;
pub mod decompress;
pub mod fse;
pub mod huff;
pub mod xxhash;

pub use common::ZstdError;
pub use compress::compress;
pub use decompress::{decompress, decompress_bounded};
