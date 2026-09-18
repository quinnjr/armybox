#!/usr/bin/env python3
"""
Simple split: each applet file contains the applet AND its helper functions.
"""

import re
from pathlib import Path

def find_brace_end(content, start):
    """Find matching closing brace."""
    depth = 0
    i = start
    in_string = False
    in_char = False

    while i < len(content):
        c = content[i]

        # Handle escape sequences
        if i > 0 and content[i-1] == '\\':
            i += 1
            continue

        # String handling
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

def process_category(base_dir, category):
    """Process a category file into a directory with individual applet files."""
    source_file = base_dir / f"{category}.rs"
    output_dir = base_dir / category

    if not source_file.exists():
        print(f"Skipping {category}")
        return

    print(f"\n=== Processing {category}.rs ===")

    with open(source_file, 'r') as f:
        content = f.read()

    # Find applet function pattern
    applet_pattern = r'((?:///[^\n]*\n)*)pub fn (r#)?(\w+)\(_?argc: i32, _?argv: \*const \*const u8\) -> i32 \{'

    # Find all applet positions
    applet_matches = list(re.finditer(applet_pattern, content))

    if not applet_matches:
        print(f"  No applets found")
        return

    print(f"  Found {len(applet_matches)} applets")

    # Create output directory
    output_dir.mkdir(exist_ok=True)

    # Extract header (use statements, constants before first function)
    first_func_match = re.search(r'((?:///[^\n]*\n)*)?(?:pub )?fn \w+', content)
    header_end = first_func_match.start() if first_func_match else 0
    header = content[:header_end].strip()

    # Extract module-level constants and structs
    module_items = []
    for match in re.finditer(r'^(pub )?(const|static|struct|enum|type) [^;{}]+[;{}]', content[:header_end], re.MULTILINE):
        module_items.append(match.group(0))

    # Write common.rs if there are module-level items
    has_common = len(module_items) > 0
    if has_common:
        common_content = """//! Shared constants and types

use crate::io;
use crate::sys;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
#[cfg(feature = "alloc")]
use alloc::string::String;

"""
        for item in module_items:
            if not item.startswith('pub '):
                item = 'pub ' + item
            common_content += item + "\n\n"

        with open(output_dir / "common.rs", 'w') as f:
            f.write(common_content)
        print(f"  Wrote common.rs")

    applet_names = []

    # Process each applet - include content until the next applet
    for i, match in enumerate(applet_matches):
        raw_prefix = match.group(2) or ""
        name = raw_prefix + match.group(3)
        file_name = name.replace('r#', 'r_')

        start = match.start()

        # End is either start of next applet or end of file
        if i + 1 < len(applet_matches):
            end = applet_matches[i + 1].start()
        else:
            end = len(content)

        # Extract this applet and its helpers
        applet_content = content[start:end].rstrip()

        # Write applet file
        file_content = f"""//! {name.replace('r#', '')} applet

use crate::io;
use crate::sys;
use super::super::{{get_arg, has_opt, is_opt, is_option}};
"""
        if has_common:
            file_content += "use super::common::*;\n"

        # Add alloc imports if needed
        if 'Vec<' in applet_content or 'vec![' in applet_content:
            file_content += """#[cfg(feature = "alloc")]
use alloc::vec::Vec;
"""
        if 'String' in applet_content:
            file_content += """#[cfg(feature = "alloc")]
use alloc::string::String;
"""
        if 'CString' in applet_content:
            file_content += """#[cfg(feature = "alloc")]
use alloc::ffi::CString;
"""

        file_content += "\n" + applet_content + "\n"

        with open(output_dir / f"{file_name}.rs", 'w') as f:
            f.write(file_content)

        applet_names.append(name)
        print(f"  Wrote {file_name}.rs")

    # Write mod.rs
    mod_content = f"//! {category} applets\n\n"

    if has_common:
        mod_content += "pub mod common;\n\n"

    for name in sorted(applet_names):
        mod_name = name.replace('r#', 'r_')
        mod_content += f"mod {mod_name};\n"

    mod_content += "\n"

    for name in sorted(applet_names):
        mod_name = name.replace('r#', 'r_')
        mod_content += f"pub use {mod_name}::{name};\n"

    with open(output_dir / "mod.rs", 'w') as f:
        f.write(mod_content)

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
