#!/usr/bin/env python3
"""
Split applet source files into individual files with shared helpers.
Creates:
  - Individual .rs files for each applet
  - common.rs for shared helper functions
  - mod.rs to tie everything together
"""

import re
import os
from pathlib import Path
from collections import defaultdict

def find_matching_brace(content, start_pos):
    """Find the position of the closing brace that matches the opening brace."""
    depth = 0
    in_string = False
    in_char = False
    in_raw_string = False
    escape_next = False
    i = start_pos

    while i < len(content):
        c = content[i]

        if escape_next:
            escape_next = False
            i += 1
            continue

        # Check for raw strings r#"..."#
        if not in_string and not in_char and i + 2 < len(content):
            if content[i:i+2] == 'r#' and content[i+2] == '"':
                in_raw_string = True
                i += 3
                continue

        if in_raw_string:
            if c == '"' and i + 1 < len(content) and content[i+1] == '#':
                in_raw_string = False
                i += 2
                continue
            i += 1
            continue

        if c == '\\' and (in_string or in_char):
            escape_next = True
            i += 1
            continue

        if c == '"' and not in_char:
            in_string = not in_string
        elif c == "'" and not in_string and i + 2 < len(content):
            # Check for char literals or lifetime annotations
            next_char = content[i+1] if i+1 < len(content) else ''
            if next_char != 'a':  # Skip lifetime 'a
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

def extract_functions(content):
    """Extract all functions from the content."""
    # Pattern for applet functions (with argc, argv signature)
    applet_pattern = r'((?:///[^\n]*\n)*)(pub fn (r#)?(\w+)\(_?argc: i32, _?argv: \*const \*const u8\) -> i32 \{)'

    # Pattern for helper functions (any other fn)
    helper_pattern = r'((?:///[^\n]*\n)*)((?:pub )?fn (\w+)\([^)]*\)[^{]*\{)'

    applets = []
    helpers = []

    # Find all applet functions
    for match in re.finditer(applet_pattern, content):
        doc = match.group(1)
        raw_prefix = match.group(3) or ""
        name = raw_prefix + match.group(4)
        start = match.start()
        brace_pos = match.end() - 1
        end = find_matching_brace(content, brace_pos)
        func_content = content[start:end + 1]
        applets.append((name, func_content, start, end))

    # Sort applets by position to mark ranges
    applets.sort(key=lambda x: x[2])
    applet_ranges = [(a[2], a[3]) for a in applets]

    # Find all helper functions (that aren't applets)
    for match in re.finditer(helper_pattern, content):
        start = match.start()
        # Check if this overlaps with any applet
        is_applet = False
        for (a_start, a_end) in applet_ranges:
            if a_start <= start <= a_end:
                is_applet = True
                break

        if not is_applet:
            name = match.group(3)
            brace_pos = match.end() - 1
            end = find_matching_brace(content, brace_pos)
            func_content = content[start:end + 1]
            helpers.append((name, func_content, start, end))

    return applets, helpers

def extract_constants_and_types(content, applet_ranges, helper_ranges):
    """Extract constants, statics, type definitions, and use statements."""
    items = []

    # Patterns for various items
    patterns = [
        (r'((?:///[^\n]*\n)*)((pub )?const \w+[^;]+;)', 'const'),
        (r'((?:///[^\n]*\n)*)((pub )?static \w+[^;]+;)', 'static'),
        (r'((?:///[^\n]*\n)*)((pub )?type \w+[^;]+;)', 'type'),
        (r'((?:///[^\n]*\n)*)(#\[derive[^\]]+\]\n)?(pub )?struct \w+[^}]+\}', 'struct'),
        (r'((?:///[^\n]*\n)*)(#\[derive[^\]]+\]\n)?(pub )?enum \w+[^}]+\}', 'enum'),
    ]

    all_ranges = applet_ranges + helper_ranges

    for pattern, item_type in patterns:
        for match in re.finditer(pattern, content):
            start = match.start()
            # Check if inside a function
            inside_func = False
            for (r_start, r_end) in all_ranges:
                if r_start < start < r_end:
                    inside_func = True
                    break

            if not inside_func:
                items.append((item_type, match.group(0).strip(), start))

    return items

def write_applet_file(output_dir, applet_name, applet_content, use_common=True):
    """Write an individual applet file."""
    file_name = applet_name.replace("r#", "r_")
    output_path = output_dir / f"{file_name}.rs"

    header = f"""//! {applet_name.replace('r#', '')} applet

use crate::io;
use crate::sys;
use super::super::{{get_arg, has_opt}};
"""

    if use_common:
        header += "use super::common::*;\n"

    # Check for alloc features
    if 'Vec<' in applet_content or 'vec![' in applet_content:
        header += """#[cfg(feature = "alloc")]
use alloc::vec::Vec;
"""
    if 'String' in applet_content and 'String::' in applet_content:
        header += """#[cfg(feature = "alloc")]
use alloc::string::String;
"""

    header += "\n"

    with open(output_path, 'w') as f:
        f.write(header)
        f.write(applet_content)
        f.write("\n")

    return file_name

def write_common_file(output_dir, helpers, constants):
    """Write the common.rs file with helpers and constants."""
    output_path = output_dir / "common.rs"

    content = f"""//! Shared helper functions and constants

use crate::io;
use crate::sys;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use alloc::string::String;

"""

    # Add constants first
    for item_type, item_content, _ in sorted(constants, key=lambda x: x[2]):
        # Make all items public for sharing
        if not item_content.startswith('pub '):
            item_content = 'pub ' + item_content
        content += item_content + "\n\n"

    # Add helper functions
    for name, func_content, _, _ in helpers:
        # Make helper functions public
        if not func_content.lstrip().startswith('pub '):
            func_content = 'pub ' + func_content.lstrip()
        content += func_content + "\n\n"

    with open(output_path, 'w') as f:
        f.write(content)

def write_mod_file(output_dir, applet_names, has_common=True):
    """Create the mod.rs file."""
    content = f"//! {output_dir.name} applets\n\n"

    if has_common:
        content += "pub mod common;\n\n"

    for name in sorted(applet_names):
        mod_name = name.replace("r#", "r_")
        content += f"mod {mod_name};\n"

    content += "\n"

    for name in sorted(applet_names):
        mod_name = name.replace("r#", "r_")
        content += f"pub use {mod_name}::{name};\n"

    with open(output_dir / "mod.rs", 'w') as f:
        f.write(content)

def process_category(base_dir, category):
    """Process a single category file into a directory."""
    source_file = base_dir / f"{category}.rs"
    output_dir = base_dir / category

    if not source_file.exists():
        print(f"Skipping {category} - file not found")
        return None

    print(f"\n=== Processing {category}.rs ===")

    with open(source_file, 'r') as f:
        content = f.read()

    # Extract functions
    applets, helpers = extract_functions(content)
    print(f"  Found {len(applets)} applets, {len(helpers)} helpers")

    # Get ranges for constant extraction
    applet_ranges = [(a[2], a[3]) for a in applets]
    helper_ranges = [(h[2], h[3]) for h in helpers]

    # Extract constants
    constants = extract_constants_and_types(content, applet_ranges, helper_ranges)
    print(f"  Found {len(constants)} constants/types")

    # Create output directory
    output_dir.mkdir(exist_ok=True)

    # Write common.rs if there are helpers or constants
    has_common = len(helpers) > 0 or len(constants) > 0
    if has_common:
        write_common_file(output_dir, helpers, constants)
        print(f"  Wrote common.rs")

    # Write individual applet files
    applet_names = []
    for name, func_content, _, _ in applets:
        file_name = write_applet_file(output_dir, name, func_content, has_common)
        applet_names.append(name)
        print(f"  Wrote {file_name}.rs")

    # Write mod.rs
    write_mod_file(output_dir, applet_names, has_common)
    print(f"  Wrote mod.rs")

    return applet_names

def main():
    base_dir = Path("/home/joseph/Projects/armybox/src/applets")

    categories = ['text', 'file', 'system', 'network', 'misc', 'archive', 'init', 'editors', 'shell', 'package']

    results = {}
    for cat in categories:
        names = process_category(base_dir, cat)
        if names:
            results[cat] = names

    print("\n=== Summary ===")
    total = 0
    for cat, names in results.items():
        print(f"  {cat}: {len(names)} applets")
        total += len(names)
    print(f"  Total: {total} applets")

if __name__ == "__main__":
    main()
