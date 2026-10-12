//! readelf - display ELF file information
//!
//! Display information about ELF format object files.

use super::get_arg;
use crate::io;

/// readelf - display information about ELF files
///
/// # Synopsis
/// ```text
/// readelf [-h] [-S] [-l] [-a] FILE
/// ```
///
/// # Options
/// - `-h`: Display ELF header
/// - `-S`: Display sections
/// - `-l`: Display program headers
/// - `-a`: Display all
pub fn readelf(argc: i32, argv: *const *const u8) -> i32 {
    let mut show_header = false;
    let mut show_sections = false;
    let mut show_program = false;
    let mut show_all = false;
    let mut file: Option<&[u8]> = None;

    for i in 1..argc {
        if let Some(arg) = unsafe { get_arg(argv, i) } {
            if arg.starts_with(b"-") {
                for &c in &arg[1..] {
                    match c {
                        b'h' => show_header = true,
                        b'S' => show_sections = true,
                        b'l' => show_program = true,
                        b'a' => show_all = true,
                        _ => {}
                    }
                }
            } else if file.is_none() {
                file = Some(arg);
            }
        }
    }

    let file = match file {
        Some(f) => f,
        None => {
            io::write_str(2, b"readelf: no input file\n");
            return 1;
        }
    };

    if show_all {
        show_header = true;
        show_sections = true;
        show_program = true;
    }
    if !show_header && !show_sections && !show_program {
        show_header = true;
    }

    let fd = io::open(file, libc::O_RDONLY, 0);
    if fd < 0 {
        io::write_str(2, b"readelf: cannot open file\n");
        return 1;
    }

    // Read ELF header
    let mut ehdr = [0u8; 64];
    if io::read(fd, &mut ehdr) < 52 {
        io::write_str(2, b"readelf: failed to read ELF header\n");
        io::close(fd);
        return 1;
    }

    // Check ELF magic
    if &ehdr[0..4] != b"\x7fELF" {
        io::write_str(2, b"readelf: not an ELF file\n");
        io::close(fd);
        return 1;
    }

    let is_64bit = ehdr[4] == 2;
    let is_le = ehdr[5] == 1;

    if show_header {
        display_elf_header(&ehdr, is_64bit, is_le);
    }

    if show_program {
        display_program_headers(fd, &ehdr, is_64bit, is_le);
    }

    io::close(fd);
    0
}

fn display_elf_header(ehdr: &[u8], is_64bit: bool, is_le: bool) {
    io::write_str(1, b"ELF Header:\n");
    io::write_str(1, b"  Magic:   ");
    for i in 0..16 {
        write_hex_byte(1, ehdr[i]);
        io::write_str(1, b" ");
    }
    io::write_str(1, b"\n");

    io::write_str(1, b"  Class:                             ");
    match ehdr[4] {
        1 => {
            io::write_str(1, b"ELF32\n");
        }
        2 => {
            io::write_str(1, b"ELF64\n");
        }
        _ => {
            io::write_str(1, b"Invalid\n");
        }
    }

    io::write_str(1, b"  Data:                              ");
    match ehdr[5] {
        1 => {
            io::write_str(1, b"2's complement, little endian\n");
        }
        2 => {
            io::write_str(1, b"2's complement, big endian\n");
        }
        _ => {
            io::write_str(1, b"Invalid\n");
        }
    }

    let e_type = read_u16(ehdr, 16, is_le);
    io::write_str(1, b"  Type:                              ");
    match e_type {
        1 => {
            io::write_str(1, b"REL (Relocatable file)\n");
        }
        2 => {
            io::write_str(1, b"EXEC (Executable file)\n");
        }
        3 => {
            io::write_str(1, b"DYN (Shared object file)\n");
        }
        4 => {
            io::write_str(1, b"CORE (Core file)\n");
        }
        _ => {
            io::write_num(1, e_type as u64);
            io::write_str(1, b"\n");
        }
    }

    let e_machine = read_u16(ehdr, 18, is_le);
    io::write_str(1, b"  Machine:                           ");
    match e_machine {
        3 => {
            io::write_str(1, b"Intel 80386\n");
        }
        40 => {
            io::write_str(1, b"ARM\n");
        }
        62 => {
            io::write_str(1, b"Advanced Micro Devices X86-64\n");
        }
        183 => {
            io::write_str(1, b"AArch64\n");
        }
        243 => {
            io::write_str(1, b"RISC-V\n");
        }
        _ => {
            io::write_num(1, e_machine as u64);
            io::write_str(1, b"\n");
        }
    }

    if is_64bit {
        let e_entry = read_u64(ehdr, 24, is_le);
        let e_phnum = read_u16(ehdr, 56, is_le);
        let e_shnum = read_u16(ehdr, 60, is_le);

        io::write_str(1, b"  Entry point address:               0x");
        write_hex64(1, e_entry);
        io::write_str(1, b"\n");
        io::write_str(1, b"  Number of program headers:         ");
        io::write_num(1, e_phnum as u64);
        io::write_str(1, b"\n");
        io::write_str(1, b"  Number of section headers:         ");
        io::write_num(1, e_shnum as u64);
        io::write_str(1, b"\n");
    }
}

fn display_program_headers(fd: i32, ehdr: &[u8], is_64bit: bool, is_le: bool) {
    io::write_str(1, b"\nProgram Headers:\n");
    io::write_str(
        1,
        b"  Type           Offset   VirtAddr           FileSiz  MemSiz   Flg\n",
    );

    let (e_phoff, e_phentsize, e_phnum) = if is_64bit {
        (
            read_u64(ehdr, 32, is_le),
            read_u16(ehdr, 54, is_le),
            read_u16(ehdr, 56, is_le),
        )
    } else {
        (
            read_u32(ehdr, 28, is_le) as u64,
            read_u16(ehdr, 42, is_le),
            read_u16(ehdr, 44, is_le),
        )
    };

    for i in 0..e_phnum {
        let offset = e_phoff + (i as u64 * e_phentsize as u64);
        io::lseek(fd, offset as i64, libc::SEEK_SET);

        let mut phdr = [0u8; 56];
        let phdr_size = if is_64bit { 56 } else { 32 };
        io::read(fd, &mut phdr[..phdr_size]);

        let p_type = read_u32(&phdr, 0, is_le);

        io::write_str(1, b"  ");
        match p_type {
            0 => {
                io::write_str(1, b"NULL         ");
            }
            1 => {
                io::write_str(1, b"LOAD         ");
            }
            2 => {
                io::write_str(1, b"DYNAMIC      ");
            }
            3 => {
                io::write_str(1, b"INTERP       ");
            }
            6 => {
                io::write_str(1, b"PHDR         ");
            }
            0x6474e551 => {
                io::write_str(1, b"GNU_STACK    ");
            }
            _ => {
                io::write_str(1, b"UNKNOWN      ");
            }
        }
        io::write_str(1, b"\n");
    }
}

fn read_u16(buf: &[u8], offset: usize, is_le: bool) -> u16 {
    if is_le {
        u16::from_le_bytes([buf[offset], buf[offset + 1]])
    } else {
        u16::from_be_bytes([buf[offset], buf[offset + 1]])
    }
}

fn read_u32(buf: &[u8], offset: usize, is_le: bool) -> u32 {
    if is_le {
        u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    } else {
        u32::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    }
}

fn read_u64(buf: &[u8], offset: usize, is_le: bool) -> u64 {
    if is_le {
        u64::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
            buf[offset + 5],
            buf[offset + 6],
            buf[offset + 7],
        ])
    } else {
        u64::from_be_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
            buf[offset + 5],
            buf[offset + 6],
            buf[offset + 7],
        ])
    }
}

fn write_hex_byte(fd: i32, b: u8) {
    const HEX: &[u8] = b"0123456789abcdef";
    io::write_all(fd, &[HEX[(b >> 4) as usize], HEX[(b & 0xf) as usize]]);
}

fn write_hex64(fd: i32, val: u64) {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut v = val;
    for i in (0..16).rev() {
        buf[i] = HEX[(v & 0xf) as usize];
        v >>= 4;
    }
    let start = buf.iter().position(|&c| c != b'0').unwrap_or(15);
    io::write_all(fd, &buf[start..]);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::process::Command;

    use crate::test_utils::test_helpers::get_armybox_path;

    #[test]
    fn test_readelf_no_file() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox).args(["readelf"]).output().unwrap();

        assert_eq!(output.status.code(), Some(1));
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("no input file"));
    }

    #[test]
    fn test_readelf_self() {
        let armybox = get_armybox_path();
        if !armybox.exists() {
            return;
        }

        let output = Command::new(&armybox)
            .args(["readelf", "-h", armybox.to_str().unwrap()])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        let stdout = std::string::String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("ELF Header"));
        assert!(stdout.contains("Magic"));
    }
}
