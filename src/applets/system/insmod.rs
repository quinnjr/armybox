//! insmod - insert a module into the Linux kernel
//!
//! Insert a loadable module into the kernel.

extern crate alloc;
use crate::applets::get_arg;
use crate::io;
use crate::sys;
use alloc::vec;
use alloc::vec::Vec;

/// insmod - insert a module into the Linux kernel
///
/// # Synopsis
/// ```text
/// insmod module.ko [module_params...]
/// ```
///
/// # Description
/// Insert a loadable module into the kernel using the init_module syscall.
/// Module parameters can be passed as additional arguments.
///
/// # Exit Status
/// - 0: Success
/// - >0: An error occurred
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn insmod(argc: i32, argv: *const *const u8) -> i32 {
    if argc < 2 {
        io::write_str(2, b"Usage: insmod MODULE [params...]\n");
        return 1;
    }

    let path = match unsafe { get_arg(argv, 1) } {
        Some(p) => p,
        None => return 1,
    };

    // Collect module parameters
    let mut params = Vec::new();
    for i in 2..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if !params.is_empty() {
                params.push(b' ');
            }
            params.extend_from_slice(arg);
        }
    }
    params.push(0); // Null terminate

    // Open and read the module file
    let fd = io::open(path, libc::O_RDONLY, 0);
    if fd < 0 {
        sys::perror(path);
        return 1;
    }

    // Get file size
    let size = unsafe { libc::lseek(fd, 0, libc::SEEK_END) };
    if size < 0 {
        sys::perror(b"lseek");
        io::close(fd);
        return 1;
    }
    unsafe { libc::lseek(fd, 0, libc::SEEK_SET) };

    // Read the entire module into memory
    let mut module_data: Vec<u8> = vec![0; size as usize];
    let mut total_read = 0usize;

    while total_read < size as usize {
        let n = io::read(fd, &mut module_data[total_read..]);
        if n <= 0 {
            if n < 0 {
                sys::perror(path);
            } else {
                io::write_str(2, b"insmod: unexpected end of file\n");
            }
            io::close(fd);
            return 1;
        }
        total_read += n as usize;
    }
    io::close(fd);

    // Validate ELF magic
    if module_data.len() < 4 || &module_data[0..4] != b"\x7FELF" {
        io::write_str(2, b"insmod: not a valid ELF module\n");
        return 1;
    }

    // Call init_module syscall
    // int init_module(void *module_image, unsigned long len, const char *param_values);
    let ret = unsafe {
        libc::syscall(
            libc::SYS_init_module,
            module_data.as_ptr() as *const libc::c_void,
            module_data.len() as libc::c_ulong,
            params.as_ptr() as *const libc::c_char,
        )
    };

    if ret != 0 {
        let errno = crate::sys::errno();
        io::write_str(2, b"insmod: cannot insert '");
        io::write_all(2, path);
        io::write_str(2, b"': ");

        // Provide helpful error messages
        match errno {
            libc::ENOEXEC => {
                io::write_str(2, b"Invalid module format\n");
            }
            libc::ENOENT => {
                io::write_str(2, b"Unknown symbol in module\n");
            }
            libc::EEXIST => {
                io::write_str(2, b"Module already loaded\n");
            }
            libc::EPERM => {
                io::write_str(2, b"Operation not permitted\n");
            }
            libc::ENOMEM => {
                io::write_str(2, b"Out of memory\n");
            }
            libc::EBUSY => {
                io::write_str(2, b"Module is busy\n");
            }
            libc::EOVERFLOW => {
                io::write_str(2, b"Module too large\n");
            }
            _ => {
                io::write_str(2, b"Error ");
                io::write_num(2, errno as u64);
                io::write_str(2, b"\n");
            }
        }
        return 1;
    }

    0
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn insmod(_argc: i32, _argv: *const *const u8) -> i32 {
    io::write_str(2, b"insmod: only available on Linux\n");
    1
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
    fn test_insmod_no_args() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["insmod"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
    }

    #[test]
    fn test_insmod_nonexistent() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["insmod", "/nonexistent.ko"])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1));
    }
}
