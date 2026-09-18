#!/usr/bin/env python3
"""
Split applet source files into individual files per applet.
"""

import re
import os
from pathlib import Path

def find_matching_brace(content, start_pos):
    """Find the position of the closing brace that matches the opening brace at start_pos."""
    depth = 0
    in_string = False
    in_char = False
    escape_next = False
    i = start_pos

    while i < len(content):
        c = content[i]

        if escape_next:
            escape_next = False
            i += 1
            continue

        if c == '\\':
            escape_next = True
            i += 1
            continue

        if c == '"' and not in_char:
            in_string = not in_string
        elif c == "'" and not in_string:
            in_char = not in_char
        elif not in_string and not in_char:
            if c == '{':
                depth += 1
            elif c == '}':
                depth -= 1
                if depth == 0:
                    return i

        i += 1

    return len(content) - 1

def extract_applets_improved(source_file, output_dir):
    """Extract applets from a source file into individual files."""

    with open(source_file, 'r') as f:
        content = f.read()

    # Pattern to find applet function declarations
    # Matches: /// doc comments\npub fn name(argc: i32, argv: ...) -> i32 {
    # Also matches _argc and _argv variants, and r#keyword syntax
    pattern = r'((?:///[^\n]*\n)*)(pub fn (r#)?(\w+)\(_?argc: i32, _?argv: \*const \*const u8\) -> i32 \{)'

    applets = []

    for match in re.finditer(pattern, content):
        doc_comment = match.group(1)
        func_decl = match.group(2)
        raw_prefix = match.group(3) or ""  # "r#" or empty
        func_name = raw_prefix + match.group(4)  # e.g., "r#true" or "echo"

        # Find the opening brace position
        brace_pos = match.end() - 1

        # Find matching closing brace
        close_brace = find_matching_brace(content, brace_pos)

        # Extract full function
        full_func = doc_comment + content[match.start() + len(doc_comment):close_brace + 1]

        applets.append((func_name, full_func.strip()))
        print(f"  Found: {func_name}")

    return applets

def write_applet_file(output_dir, applet_name, applet_content):
    """Write an individual applet file."""

    # Handle raw identifier names like r#true, r#false
    file_name = applet_name.replace("r#", "r_")
    output_path = output_dir / f"{file_name}.rs"

    # Build header
    header = f"""//! {applet_name} applet

use crate::io;
use crate::sys;
use super::super::{{get_arg, has_opt}};
"""

    # Check if alloc features are used
    if 'Vec<' in applet_content or 'vec![' in applet_content:
        header += """#[cfg(feature = "alloc")]
use alloc::vec::Vec;
"""
    if 'String::' in applet_content or ': String' in applet_content or '-> String' in applet_content:
        header += """#[cfg(feature = "alloc")]
use alloc::string::String;
"""
    if 'CString' in applet_content:
        header += """#[cfg(feature = "alloc")]
use alloc::ffi::CString;
"""

    header += "\n"

    with open(output_path, 'w') as f:
        f.write(header)
        f.write(applet_content)
        f.write("\n")

    print(f"  Wrote: {output_path}")

def create_mod_file(output_dir, applet_names):
    """Create the mod.rs file for the category."""

    mod_content = f"//! {output_dir.name} applets\n\n"

    for name in sorted(applet_names):
        # Convert r#keyword to r_keyword for module name
        mod_name = name.replace("r#", "r_")
        mod_content += f"mod {mod_name};\n"

    mod_content += "\n"

    for name in sorted(applet_names):
        mod_name = name.replace("r#", "r_")
        # For pub use, keep the original function name
        mod_content += f"pub use {mod_name}::{name};\n"

    mod_path = output_dir / "mod.rs"
    with open(mod_path, 'w') as f:
        f.write(mod_content)

    print(f"  Wrote: {mod_path}")

def process_file(base_dir, source_name):
    """Process a single source file."""
    source_file = base_dir / f"{source_name}.rs"
    output_dir = base_dir / source_name

    if not source_file.exists():
        print(f"Source file not found: {source_file}")
        return

    print(f"\nProcessing {source_name}.rs...")
    output_dir.mkdir(exist_ok=True)

    applets = extract_applets_improved(source_file, output_dir)
    applet_names = []

    for name, content in applets:
        write_applet_file(output_dir, name, content)
        applet_names.append(name)

    create_mod_file(output_dir, applet_names)

    print(f"\nExtracted {len(applets)} applets from {source_name}.rs")
    return applet_names

def main():
    base_dir = Path("/home/joseph/Projects/armybox/src/applets")

    # Process all category files
    categories = ['text', 'file', 'system', 'network', 'misc', 'archive', 'init', 'editors', 'shell', 'package']

    all_applets = {}

    for cat in categories:
        applet_names = process_file(base_dir, cat)
        if applet_names:
            all_applets[cat] = applet_names

    print("\n\n=== Summary ===")
    total = 0
    for cat, names in all_applets.items():
        print(f"{cat}: {len(names)} applets")
        total += len(names)
    print(f"Total: {total} applets")

if __name__ == "__main__":
    main()
