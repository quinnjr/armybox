//! # Armybox - A `#[no_std]` BusyBox Clone
//!
//! Armybox provides Unix utilities in a tiny, `#[no_std]` compatible binary.
//! It uses only `libc` for system calls and `alloc` for heap allocation.
//!
//! ## Features
//!
//! - **Truly `#[no_std]`**: No standard library dependency
//! - **Tiny binary**: ~108KB stripped, ~54KB with UPX compression
//! - **Embedded-ready**: Works on systems without full std support

// Use no_std except during tests (which require std for test harness)
#![cfg_attr(not(test), no_std)]
// Clippy configuration - allow stylistic lints that add noise in a no_std codebase
#![allow(clippy::len_zero)] // .len() == 0 vs .is_empty() - personal preference
#![allow(clippy::manual_range_contains)] // x >= a && x <= b is clearer than (a..=b).contains(&x)
#![allow(clippy::needless_range_loop)] // index loops often needed for multiple arrays
#![allow(clippy::too_many_arguments)] // libc wrappers often need many args
#![allow(clippy::collapsible_if)] // nested ifs can be clearer
#![allow(clippy::redundant_closure)] // closures can be clearer than fn refs
#![allow(clippy::single_match)] // single-arm match can be clearer than if-let
#![allow(clippy::match_single_binding)] // intentional pattern matching
#![allow(clippy::unnecessary_cast)] // explicit casts document intent
#![allow(clippy::manual_c_str_literals)] // b"...\0" syntax is clearer than c"..."
#![allow(clippy::missing_safety_doc)] // many unsafe fns are thin wrappers
#![allow(clippy::if_same_then_else)] // sometimes clearer to be explicit
#![allow(clippy::manual_strip)] // strip_prefix pattern matching preferred
#![allow(clippy::manual_div_ceil)] // explicit div/mod clearer than div_ceil
#![allow(clippy::needless_return)] // explicit returns can be clearer
#![allow(clippy::wrong_self_convention)] // naming conventions differ for FFI
#![allow(clippy::same_item_push)] // intentional repeated pushes
#![allow(clippy::not_unsafe_ptr_arg_deref)] // thin libc wrappers are safe in practice
#![allow(clippy::explicit_auto_deref)] // explicit derefs document intent
#![allow(clippy::manual_clamp)] // explicit min/max clearer
#![allow(clippy::let_and_return)] // named returns can document intent
#![allow(clippy::needless_borrow)] // explicit borrows can be clearer
#![allow(clippy::op_ref)] // reference ops for clarity
#![allow(clippy::needless_pub_self)] // explicit pub(self) can document intent
#![allow(clippy::manual_pattern_char_comparison)] // explicit char comparisons clearer
#![allow(clippy::implicit_saturating_sub)] // explicit checked_sub clearer
#![allow(clippy::identity_op)] // x + 0 or x * 1 for documentation
#![allow(clippy::while_let_loop)] // explicit loop control preferred
#![allow(clippy::slow_vector_initialization)] // vec![0; n] is fine
#![allow(clippy::match_overlapping_arm)] // intentional fallthrough patterns
#![allow(clippy::manual_rotate)] // explicit shifts clearer
#![allow(clippy::manual_map)] // explicit match clearer than map
#![allow(clippy::manual_contains)] // explicit iteration clearer
#![allow(clippy::iter_out_of_bounds)] // false positives with slices
#![allow(clippy::doc_overindented_list_items)]
// documentation formatting

// Allow unused variables/fields for protocol constants and optional features
#![allow(dead_code)] // many protocol constants defined for completeness
#![allow(unused_assignments)] // some assignments used in conditional paths
#![allow(unreachable_patterns)] // explicit exhaustive matching
#![allow(unused_variables)] // some vars used conditionally or for documentation

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod applets;
pub mod io;
pub mod sys;

#[cfg(test)]
pub mod test_utils;

/// Applet function type
pub type AppletFn = fn(i32, *const *const u8) -> i32;

/// Check if an applet exists
pub fn is_applet(name: &[u8]) -> bool {
    applets::find_applet(name).is_some()
}

/// Run an applet by name
pub fn run_applet(name: &[u8], argc: i32, argv: *const *const u8) -> i32 {
    match applets::find_applet(name) {
        Some(f) => f(argc, argv),
        None => {
            io::write_str(2, b"armybox: applet not found: ");
            io::write_all(2, name);
            io::write_str(2, b"\n");
            127
        }
    }
}

/// Get applet count
pub const fn applet_count() -> usize {
    applets::APPLET_COUNT
}

// ============================================================================
// Panic handler for no_std (not used in test builds)
// ============================================================================

#[cfg(all(not(test), not(feature = "std")))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    io::write_str(2, b"armybox: panic!\n");
    unsafe {
        libc::_exit(1);
    }
}

/// The precompiled core/compiler_builtins rlibs carry unwind-table data that
/// references this symbol even though the binary is panic=abort (seen on the
/// aarch64 musl and Android release targets). It is never called; defining it
/// only satisfies the linker.
#[cfg(all(not(test), not(feature = "std")))]
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() {}

// ============================================================================
// Global allocator using libc malloc (not used in test builds)
// ============================================================================

#[cfg(all(not(test), not(feature = "std"), feature = "alloc"))]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    pub struct LibcAllocator;

    unsafe impl GlobalAlloc for LibcAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe { libc::malloc(layout.size()) as *mut u8 }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            unsafe { libc::free(ptr as *mut libc::c_void) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
            unsafe { libc::realloc(ptr as *mut libc::c_void, new_size) as *mut u8 }
        }
    }

    #[global_allocator]
    static ALLOCATOR: LibcAllocator = LibcAllocator;
}
