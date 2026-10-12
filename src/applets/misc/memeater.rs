//! memeater - memory stress test
//!
//! Allocates and touches memory to stress test the system.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// memeater - memory stress test
///
/// # Synopsis
/// ```text
/// memeater [size_in_MB]
/// ```
///
/// # Description
/// Allocates and touches memory to stress test the system.
/// Default allocation is 100MB.
///
/// # Exit Status
/// - 0: Success (or interrupted)
pub fn memeater(argc: i32, argv: *const *const u8) -> i32 {
    let size_mb = if argc > 1 {
        match unsafe { get_arg(argv, 1) }.and_then(|s| sys::parse_u64(s)) {
            Some(n) => n as usize,
            None => 100,
        }
    } else {
        100 // Default 100MB
    };

    io::write_str(1, b"memeater: allocating ");
    let mut buf = [0u8; 16];
    let size_str = sys::format_u64(size_mb as u64, &mut buf);
    io::write_all(1, size_str);
    io::write_str(1, b" MB...\n");

    let total_bytes = size_mb * 1024 * 1024;
    let mut memory: Vec<u8> = Vec::with_capacity(total_bytes);

    // Fill memory to ensure it's actually allocated
    for i in 0..total_bytes {
        memory.push((i & 0xFF) as u8);
    }

    io::write_str(1, b"memeater: allocated. Press Ctrl+C to exit.\n");

    // Keep the memory allocated until interrupted
    loop {
        // Touch memory periodically to prevent it from being swapped out
        for i in (0..memory.len()).step_by(4096) {
            let _ = memory[i];
        }
        unsafe {
            libc::sleep(1);
        }
    }
}

#[cfg(test)]
mod tests {
    // memeater is designed to run indefinitely, so no automated tests
}
