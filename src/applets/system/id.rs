//! id - print user and group IDs
//!
//! POSIX.1-2017 compliant implementation.
//! Reference: https://pubs.opengroup.org/onlinepubs/9699919799/utilities/id.html

use crate::io;

/// id - print user and group IDs
///
/// # Synopsis
/// ```text
/// id [user]
/// ```
///
/// # Description
/// Print user and group information for the specified user,
/// or the current user if no user is specified.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
fn uname_of(uid: libc::uid_t) -> Option<&'static [u8]> {
    let pw = unsafe { libc::getpwuid(uid) };
    if pw.is_null() {
        return None;
    }
    let name = unsafe { (*pw).pw_name };
    if name.is_null() {
        return None;
    }
    Some(unsafe { io::cstr_to_slice(name as *const u8) })
}

fn gname_of(gid: libc::gid_t) -> Option<&'static [u8]> {
    let gr = unsafe { libc::getgrgid(gid) };
    if gr.is_null() {
        return None;
    }
    let name = unsafe { (*gr).gr_name };
    if name.is_null() {
        return None;
    }
    Some(unsafe { io::cstr_to_slice(name as *const u8) })
}

/// Write `id(name)` or just `id` when the name is unknown.
fn write_id_name(fd: i32, id: u64, name: Option<&[u8]>) {
    io::write_num(fd, id);
    if let Some(n) = name {
        io::write_str(fd, b"(");
        io::write_all(fd, n);
        io::write_str(fd, b")");
    }
}

pub fn id(argc: i32, argv: *const *const u8) -> i32 {
    let mut want_u = false;
    let mut want_g = false;
    let mut want_groups = false;
    let mut want_name = false;
    let mut want_real = false;
    let mut user: Option<&[u8]> = None;

    let mut i = 1;
    while i < argc {
        let arg = match unsafe { super::get_arg(argv, i) } {
            Some(a) => a,
            None => break,
        };
        if arg.len() >= 2 && arg[0] == b'-' && arg != b"--" {
            for &c in &arg[1..] {
                match c {
                    b'u' => want_u = true,
                    b'g' => want_g = true,
                    b'G' => want_groups = true,
                    b'n' => want_name = true,
                    b'r' => want_real = true,
                    _ => {
                        io::write_str(2, b"id: invalid option\n");
                        return 2;
                    }
                }
            }
        } else if arg == b"--" {
            // no-op
        } else {
            user = Some(arg);
        }
        i += 1;
    }

    // Selector modes are mutually exclusive; -n/-r only modify them.
    let modes = (want_u as u8) + (want_g as u8) + (want_groups as u8);
    if modes > 1 {
        io::write_str(2, b"id: cannot print \"only\" of more than one choice\n");
        return 1;
    }
    if (want_name || want_real) && modes == 0 {
        io::write_str(
            2,
            b"id: cannot print only names or real IDs in default format\n",
        );
        return 1;
    }

    // Resolve the subject: either a named user, or the current process.
    // (uid/gid = real, euid/egid = effective; for a named user they coincide.)
    let (uid, gid, euid, egid, pw_name): (u64, u64, u64, u64, Option<&[u8]>) = if let Some(u) = user
    {
        // Named user: look up the passwd entry.
        let mut buf = alloc::vec::Vec::with_capacity(u.len() + 1);
        buf.extend_from_slice(u);
        buf.push(0);
        let pw = unsafe { libc::getpwnam(buf.as_ptr() as *const libc::c_char) };
        if pw.is_null() {
            io::write_str(2, b"id: '");
            io::write_all(2, u);
            io::write_str(2, b"': no such user\n");
            return 1;
        }
        let p = unsafe { &*pw };
        let name = if p.pw_name.is_null() {
            None
        } else {
            Some(unsafe { io::cstr_to_slice(p.pw_name as *const u8) })
        };
        (
            p.pw_uid as u64,
            p.pw_gid as u64,
            p.pw_uid as u64,
            p.pw_gid as u64,
            name,
        )
    } else {
        let ru = unsafe { libc::getuid() } as u64;
        let rg = unsafe { libc::getgid() } as u64;
        let eu = unsafe { libc::geteuid() } as u64;
        let eg = unsafe { libc::getegid() } as u64;
        (ru, rg, eu, eg, None)
    };

    // For selector modes, pick real vs effective.
    let sel_uid = if want_real { uid } else { euid };
    let sel_gid = if want_real { gid } else { egid };

    // Collect the supplementary/group-list GIDs.
    let group_ids: alloc::vec::Vec<libc::gid_t> = if let Some(u) = user {
        // getgrouplist for the named user.
        let mut buf = alloc::vec::Vec::with_capacity(u.len() + 1);
        buf.extend_from_slice(u);
        buf.push(0);
        let mut ngroups: libc::c_int = 16;
        let mut groups: alloc::vec::Vec<libc::gid_t> = alloc::vec![0; ngroups as usize];
        loop {
            let rc = unsafe {
                libc::getgrouplist(
                    buf.as_ptr() as *const libc::c_char,
                    gid as libc::gid_t,
                    groups.as_mut_ptr(),
                    &mut ngroups,
                )
            };
            if rc >= 0 {
                groups.truncate(ngroups as usize);
                break groups;
            }
            // Buffer too small: retry with the returned size.
            groups = alloc::vec![0; ngroups as usize];
        }
    } else {
        let n = unsafe { libc::getgroups(0, core::ptr::null_mut()) };
        if n <= 0 {
            alloc::vec![egid as libc::gid_t]
        } else {
            let mut groups: alloc::vec::Vec<libc::gid_t> = alloc::vec![0; n as usize];
            let got = unsafe { libc::getgroups(n, groups.as_mut_ptr()) };
            if got < 0 {
                alloc::vec![egid as libc::gid_t]
            } else {
                groups.truncate(got as usize);
                groups
            }
        }
    };

    // --- Selector modes ---
    if want_u {
        if want_name {
            match uname_of(sel_uid as libc::uid_t) {
                Some(n) => io::write_all(1, n),
                None => io::write_num(1, sel_uid),
            };
        } else {
            io::write_num(1, sel_uid);
        }
        io::write_str(1, b"\n");
        return 0;
    }
    if want_g {
        if want_name {
            match gname_of(sel_gid as libc::gid_t) {
                Some(n) => io::write_all(1, n),
                None => io::write_num(1, sel_gid),
            };
        } else {
            io::write_num(1, sel_gid);
        }
        io::write_str(1, b"\n");
        return 0;
    }
    if want_groups {
        for (idx, &g) in group_ids.iter().enumerate() {
            if idx > 0 {
                io::write_str(1, b" ");
            }
            if want_name {
                match gname_of(g) {
                    Some(n) => io::write_all(1, n),
                    None => io::write_num(1, g as u64),
                };
            } else {
                io::write_num(1, g as u64);
            }
        }
        io::write_str(1, b"\n");
        return 0;
    }

    // --- Default format: uid=U(name) gid=G(name) [euid=..] [egid=..] groups=... ---
    let uname = pw_name.or_else(|| uname_of(uid as libc::uid_t));
    io::write_str(1, b"uid=");
    write_id_name(1, uid, uname);
    io::write_str(1, b" gid=");
    write_id_name(1, gid, gname_of(gid as libc::gid_t));
    if user.is_none() && euid != uid {
        io::write_str(1, b" euid=");
        write_id_name(1, euid, uname_of(euid as libc::uid_t));
    }
    if user.is_none() && egid != gid {
        io::write_str(1, b" egid=");
        write_id_name(1, egid, gname_of(egid as libc::gid_t));
    }
    io::write_str(1, b" groups=");
    for (idx, &g) in group_ids.iter().enumerate() {
        if idx > 0 {
            io::write_str(1, b",");
        }
        write_id_name(1, g as u64, gname_of(g));
    }
    io::write_str(1, b"\n");
    0
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
    fn test_id_output() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["id"]).output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("uid="));
        assert!(stdout.contains("gid="));
    }

    #[test]
    fn test_id_contains_current_uid() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["id"]).output().unwrap();

        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        // Should contain uid= followed by a number
        assert!(stdout.starts_with("uid="));
    }
}
