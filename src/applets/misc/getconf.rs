//! getconf - get configuration values
//!
//! Query system configuration variables.

use super::get_arg;
use crate::io;

/// getconf - get configuration values
///
/// # Synopsis
/// ```text
/// getconf NAME
/// ```
///
/// # Description
/// Get values for system configuration variables.
///
/// # Exit Status
/// - 0: Success
/// - 1: Unknown variable
fn write_signed(fd: i32, v: i64) {
    if v < 0 {
        io::write_str(fd, b"-");
        io::write_num(fd, (-v) as u64);
    } else {
        io::write_num(fd, v as u64);
    }
}

/// Map a system (sysconf) variable name to its `_SC_*` constant.
fn sysconf_var(name: &[u8]) -> Option<libc::c_int> {
    Some(match name {
        b"PAGE_SIZE" | b"PAGESIZE" | b"_SC_PAGE_SIZE" => libc::_SC_PAGESIZE,
        b"NPROCESSORS_ONLN" | b"_NPROCESSORS_ONLN" => libc::_SC_NPROCESSORS_ONLN,
        b"NPROCESSORS_CONF" | b"_NPROCESSORS_CONF" => libc::_SC_NPROCESSORS_CONF,
        b"ARG_MAX" => libc::_SC_ARG_MAX,
        b"CHILD_MAX" => libc::_SC_CHILD_MAX,
        b"CLK_TCK" => libc::_SC_CLK_TCK,
        b"NGROUPS_MAX" => libc::_SC_NGROUPS_MAX,
        b"OPEN_MAX" => libc::_SC_OPEN_MAX,
        b"STREAM_MAX" => libc::_SC_STREAM_MAX,
        b"TZNAME_MAX" => libc::_SC_TZNAME_MAX,
        b"LINE_MAX" => libc::_SC_LINE_MAX,
        b"RE_DUP_MAX" => libc::_SC_RE_DUP_MAX,
        b"_POSIX_VERSION" => libc::_SC_VERSION,
        b"_POSIX2_VERSION" => libc::_SC_2_VERSION,
        b"_POSIX_JOB_CONTROL" => libc::_SC_JOB_CONTROL,
        b"_POSIX_SAVED_IDS" => libc::_SC_SAVED_IDS,
        b"SYMLOOP_MAX" => libc::_SC_SYMLOOP_MAX,
        b"HOST_NAME_MAX" => libc::_SC_HOST_NAME_MAX,
        b"LOGIN_NAME_MAX" => libc::_SC_LOGIN_NAME_MAX,
        _ => return None,
    })
}

/// Map a pathname (pathconf) variable name to its `_PC_*` constant.
fn pathconf_var(name: &[u8]) -> Option<libc::c_int> {
    Some(match name {
        b"PATH_MAX" => libc::_PC_PATH_MAX,
        b"NAME_MAX" => libc::_PC_NAME_MAX,
        b"LINK_MAX" => libc::_PC_LINK_MAX,
        b"MAX_CANON" => libc::_PC_MAX_CANON,
        b"MAX_INPUT" => libc::_PC_MAX_INPUT,
        b"PIPE_BUF" => libc::_PC_PIPE_BUF,
        b"FILESIZEBITS" => crate::sys::_PC_FILESIZEBITS,
        b"_POSIX_CHOWN_RESTRICTED" => libc::_PC_CHOWN_RESTRICTED,
        b"_POSIX_NO_TRUNC" => libc::_PC_NO_TRUNC,
        _ => return None,
    })
}

pub fn getconf(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"usage: getconf variable [pathname]\n");
        return 1;
    }
    let name = match unsafe { get_arg(argv, 1) } {
        Some(n) => n,
        None => return 1,
    };
    let path = unsafe { get_arg(argv, 2) };

    if let Some(p) = path {
        // Pathname variable: query via pathconf.
        let pc = match pathconf_var(name) {
            Some(c) => c,
            None => {
                io::write_str(2, b"getconf: unknown variable\n");
                return 1;
            }
        };
        let mut cbuf = alloc::vec::Vec::with_capacity(p.len() + 1);
        cbuf.extend_from_slice(p);
        cbuf.push(0);
        let val = unsafe { libc::pathconf(cbuf.as_ptr() as *const libc::c_char, pc) };
        // pathconf returns -1 for "no limit"; POSIX getconf prints it.
        write_signed(1, val as i64);
        io::write_str(1, b"\n");
        0
    } else if let Some(sc) = sysconf_var(name) {
        let val = unsafe { libc::sysconf(sc) };
        write_signed(1, val as i64);
        io::write_str(1, b"\n");
        0
    } else {
        io::write_str(2, b"getconf: unknown variable\n");
        1
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_getconf_page_size() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["getconf", "PAGE_SIZE"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let val: u64 = stdout.trim().parse().unwrap_or(0);
        assert!(val >= 4096); // Page size is at least 4KB
    }

    #[test]
    fn test_getconf_nprocessors() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["getconf", "NPROCESSORS_ONLN"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        let val: u64 = stdout.trim().parse().unwrap_or(0);
        assert!(val >= 1);
    }

    #[test]
    fn test_getconf_unknown() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["getconf", "UNKNOWN_VAR"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
