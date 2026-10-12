//! abpd-gen — Generate .abpd binary package index from TSV input
//!
//! Reads tab-separated records from stdin, one package per line:
//!   name\tversion\tdescription\turl\tlicense\tarch\tdepends\tprovides\tcategory
//!
//! Writes the binary .abpd file to the path given as the first argument
//! (or stdout if "-" or no argument given).
//!
//! Lines starting with '#' or empty lines are ignored.
//!
//! Usage:
//!   ./scripts/gen-package-db | abpd-gen output/db/packages.abpd

use std::io::{self, BufRead, Write};

// Use the library's pkgindex module
use abp::pkgindex::Category;
use abp::pkgindex::build::{PackageIndexBuilder, PackageInfo};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output_path = args.get(1).map(|s| s.as_str());

    let stdin = io::stdin();
    let mut builder = PackageIndexBuilder::new();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("abpd-gen: read error: {}", e);
                std::process::exit(1);
            }
        };

        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 9 {
            eprintln!(
                "abpd-gen: warning: skipping malformed line ({} fields, need 9): {}",
                fields.len(),
                truncate(line, 60)
            );
            continue;
        }

        let category = match fields[8] {
            "core" | "foundation" => Category::Core,
            "system" | "iso-tool" => Category::System,
            "network" | "rust-netutil" => Category::Network,
            "text" => Category::Text,
            "archive" => Category::Archive,
            "devel" | "build-tool" | "bootstrap" | "rust-toolchain" | "llvm-toolchain"
            | "runtime" => Category::Devel,
            "shell" | "rust-shell" => Category::Shell,
            "editor" | "rust-devtool" => Category::Editor,
            "meta" => Category::Meta,
            "core-lib" => Category::Core,
            "rust-coreutil" | "rust-sysutil" => Category::Misc,
            _ => Category::Unknown,
        };

        // depends and provides are space-separated in ABUILDs;
        // convert to comma-separated for the binary format
        let depends = fields[6].split_whitespace().collect::<Vec<_>>().join(",");
        let provides = fields[7].split_whitespace().collect::<Vec<_>>().join(",");

        builder.add(PackageInfo {
            name: fields[0].to_string(),
            version: fields[1].to_string(),
            description: fields[2].to_string(),
            url: fields[3].to_string(),
            license: fields[4].to_string(),
            arch: fields[5].to_string(),
            depends,
            provides,
            category,
        });
    }

    let data = builder.build();
    let count = builder.count();

    match output_path {
        Some("-") | None => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            if let Err(e) = out.write_all(&data) {
                eprintln!("abpd-gen: write error: {}", e);
                std::process::exit(1);
            }
        }
        Some(path) => {
            if let Err(e) = std::fs::write(path, &data) {
                eprintln!("abpd-gen: cannot write '{}': {}", path, e);
                std::process::exit(1);
            }
            eprintln!(
                "abpd-gen: wrote {} packages ({} bytes) -> {}",
                count,
                data.len(),
                path
            );
        }
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
