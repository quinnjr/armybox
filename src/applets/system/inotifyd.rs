//! inotifyd - inotify event daemon
//!
//! Run a program on file system events.

extern crate alloc;

use super::get_arg;
use crate::io;
use crate::sys;
use alloc::vec::Vec;

/// Watch entry
struct Watch {
    wd: i32,
    path: Vec<u8>,
}

/// inotifyd - inotify event daemon
///
/// # Synopsis
/// ```text
/// inotifyd PROG FILE:MASK...
/// ```
///
/// # Description
/// Run PROG when file system events occur on FILE.
/// PROG receives: event_type filename [newfilename]
///
/// # Masks
/// - a: Access
/// - c: Create
/// - d: Delete
/// - m: Modify
/// - M: Move (moved from or to)
/// - n: Name change (rename - both from and to)
/// - w: Close write
/// - 0: Close nowrite
/// - r: Open
/// - D: Delete self
/// - u: Unmount
///
/// # Exit Status
/// - 0: Success (or terminated)
/// - 1: Error
pub fn inotifyd(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 3 {
        io::write_str(2, b"inotifyd: usage: inotifyd PROG FILE:MASK...\n");
        io::write_str(2, b"\nMasks:\n");
        io::write_str(2, b"  a: Access\n");
        io::write_str(2, b"  c: Create\n");
        io::write_str(2, b"  d: Delete\n");
        io::write_str(2, b"  m: Modify\n");
        io::write_str(2, b"  M: Move\n");
        io::write_str(2, b"  n: Rename\n");
        io::write_str(2, b"  w: Close write\n");
        io::write_str(2, b"  0: Close nowrite\n");
        io::write_str(2, b"  r: Open\n");
        io::write_str(2, b"  D: Delete self\n");
        io::write_str(2, b"  u: Unmount\n");
        return 1;
    }

    let prog = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => return 1,
    };

    // Initialize inotify
    let inotify_fd = unsafe { libc::inotify_init() };
    if inotify_fd < 0 {
        sys::perror(b"inotify_init");
        return 1;
    }

    // Track watches
    let mut watches: Vec<Watch> = Vec::new();

    // Add watches for each FILE:MASK
    for i in 2..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            // Find colon separator
            let colon_pos = arg.iter().position(|&c| c == b':');

            let (file, mask_str) = if let Some(pos) = colon_pos {
                (&arg[..pos], &arg[pos + 1..])
            } else {
                (arg, b"" as &[u8])
            };

            let mask = parse_mask(mask_str);
            if mask == 0 {
                io::write_str(2, b"inotifyd: empty mask for ");
                io::write_all(2, file);
                io::write_str(2, b"\n");
                continue;
            }

            // Need null-terminated path
            let mut path = [0u8; 4096];
            let len = file.len().min(path.len() - 1);
            path[..len].copy_from_slice(&file[..len]);

            let wd = unsafe {
                libc::inotify_add_watch(inotify_fd, path.as_ptr() as *const libc::c_char, mask)
            };
            if wd < 0 {
                sys::perror(file);
            } else {
                watches.push(Watch {
                    wd,
                    path: file.to_vec(),
                });
            }
        }
    }

    if watches.is_empty() {
        io::write_str(2, b"inotifyd: no watches added\n");
        unsafe { libc::close(inotify_fd) };
        return 1;
    }

    // Event buffer - inotify_event struct is variable length
    // struct inotify_event { int wd; uint32_t mask; uint32_t cookie; uint32_t len; char name[]; }
    let mut event_buf = [0u8; 4096];

    // Event loop
    loop {
        let n = unsafe {
            libc::read(
                inotify_fd,
                event_buf.as_mut_ptr() as *mut libc::c_void,
                event_buf.len(),
            )
        };

        if n < 0 {
            let err = sys::errno();
            if err == libc::EINTR {
                continue;
            }
            sys::perror(b"read");
            break;
        }

        if n == 0 {
            break;
        }

        // Process events
        let mut offset = 0;
        while offset < n as usize {
            // inotify_event layout:
            // i32 wd (4 bytes)
            // u32 mask (4 bytes)
            // u32 cookie (4 bytes)
            // u32 len (4 bytes)
            // char name[len]
            if offset + 16 > n as usize {
                break;
            }

            let wd = i32::from_ne_bytes([
                event_buf[offset],
                event_buf[offset + 1],
                event_buf[offset + 2],
                event_buf[offset + 3],
            ]);
            let mask = u32::from_ne_bytes([
                event_buf[offset + 4],
                event_buf[offset + 5],
                event_buf[offset + 6],
                event_buf[offset + 7],
            ]);
            let _cookie = u32::from_ne_bytes([
                event_buf[offset + 8],
                event_buf[offset + 9],
                event_buf[offset + 10],
                event_buf[offset + 11],
            ]);
            let name_len = u32::from_ne_bytes([
                event_buf[offset + 12],
                event_buf[offset + 13],
                event_buf[offset + 14],
                event_buf[offset + 15],
            ]) as usize;

            // Get filename if present
            let name = if name_len > 0 && offset + 16 + name_len <= n as usize {
                let name_bytes = &event_buf[offset + 16..offset + 16 + name_len];
                // Find null terminator
                let end = name_bytes.iter().position(|&c| c == 0).unwrap_or(name_len);
                &name_bytes[..end]
            } else {
                &[][..]
            };

            // Find the watch path
            let watch_path = watches
                .iter()
                .find(|w| w.wd == wd)
                .map(|w| &w.path[..])
                .unwrap_or(b"?");

            // Convert mask to event character
            let event_char = mask_to_char(mask);

            // Build the full path
            let mut full_path: Vec<u8> = watch_path.to_vec();
            if !name.is_empty() {
                if !full_path.ends_with(b"/") {
                    full_path.push(b'/');
                }
                full_path.extend_from_slice(name);
            }

            // Execute the program: PROG event_char path
            execute_handler(prog, event_char, &full_path);

            // Check if watch was removed
            if mask & libc::IN_IGNORED != 0 {
                watches.retain(|w| w.wd != wd);
                if watches.is_empty() {
                    break;
                }
            }

            offset += 16 + name_len;
            // Align to next 4-byte boundary
            while offset % 4 != 0 && offset < n as usize {
                offset += 1;
            }
        }

        if watches.is_empty() {
            break;
        }
    }

    unsafe { libc::close(inotify_fd) };
    0
}

fn parse_mask(s: &[u8]) -> u32 {
    let mut mask: u32 = 0;
    for &c in s {
        mask |= match c {
            b'a' => libc::IN_ACCESS,
            b'c' => libc::IN_CREATE,
            b'd' => libc::IN_DELETE | libc::IN_DELETE_SELF,
            b'm' => libc::IN_MODIFY,
            b'M' => libc::IN_MOVE_SELF | libc::IN_MOVED_FROM | libc::IN_MOVED_TO,
            b'n' => libc::IN_MOVED_FROM | libc::IN_MOVED_TO,
            b'w' => libc::IN_CLOSE_WRITE,
            b'0' => libc::IN_CLOSE_NOWRITE,
            b'r' => libc::IN_OPEN,
            b'D' => libc::IN_DELETE_SELF,
            b'u' => libc::IN_UNMOUNT,
            _ => 0,
        };
    }
    mask
}

/// Convert inotify mask to single-character event type
fn mask_to_char(mask: u32) -> u8 {
    if mask & libc::IN_ACCESS != 0 {
        b'a'
    } else if mask & libc::IN_CREATE != 0 {
        b'c'
    } else if mask & libc::IN_DELETE != 0 {
        b'd'
    } else if mask & libc::IN_DELETE_SELF != 0 {
        b'D'
    } else if mask & libc::IN_MODIFY != 0 {
        b'm'
    } else if mask & libc::IN_MOVED_FROM != 0 {
        b'M'
    } else if mask & libc::IN_MOVED_TO != 0 {
        b'M'
    } else if mask & libc::IN_MOVE_SELF != 0 {
        b'M'
    } else if mask & libc::IN_CLOSE_WRITE != 0 {
        b'w'
    } else if mask & libc::IN_CLOSE_NOWRITE != 0 {
        b'0'
    } else if mask & libc::IN_OPEN != 0 {
        b'r'
    } else if mask & libc::IN_UNMOUNT != 0 {
        b'u'
    } else {
        b'?'
    }
}

/// Execute the handler program
fn execute_handler(prog: &[u8], event_char: u8, path: &[u8]) {
    let pid = unsafe { libc::fork() };

    if pid == 0 {
        // Child process
        let mut prog_buf = [0u8; 4096];
        let prog_len = prog.len().min(prog_buf.len() - 1);
        prog_buf[..prog_len].copy_from_slice(&prog[..prog_len]);

        let event_buf = [event_char, 0];

        let mut path_buf = [0u8; 4096];
        let path_len = path.len().min(path_buf.len() - 1);
        path_buf[..path_len].copy_from_slice(&path[..path_len]);

        let argv_ptrs = [
            prog_buf.as_ptr() as *const libc::c_char,
            event_buf.as_ptr() as *const libc::c_char,
            path_buf.as_ptr() as *const libc::c_char,
            core::ptr::null(),
        ];

        unsafe {
            libc::execvp(prog_buf.as_ptr() as *const libc::c_char, argv_ptrs.as_ptr());
            libc::_exit(127);
        }
    } else if pid > 0 {
        // Parent - wait for child (non-blocking could be an option)
        let mut status: i32 = 0;
        unsafe { libc::waitpid(pid, &mut status, 0) };
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::path::PathBuf;
    use std::process::Command;

    fn get_armybox_path() -> PathBuf {
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

    #[test]
    fn test_inotifyd_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["inotifyd"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("usage"));
    }
}
