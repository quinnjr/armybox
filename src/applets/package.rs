//! ABP Package Manager - thin wrapper around the `abp` crate.

extern crate alloc;

#[cfg(feature = "abp")]
use super::get_arg;
#[cfg(feature = "apk")]
use crate::io;
#[cfg(feature = "abp")]
use alloc::vec::Vec;

/// ABP command entry point - delegates to the abp crate
#[cfg(feature = "abp")]
pub fn abp(argc: i32, argv: *const *const u8) -> i32 {
    let mut args: Vec<&[u8]> = Vec::new();
    for i in 0..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            args.push(arg);
        }
    }
    ::abp::run(&args)
}

/// APK compatibility stub
#[cfg(feature = "apk")]
pub fn apk(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"apk: stub - use 'abp' for package management\n");
    0
}
