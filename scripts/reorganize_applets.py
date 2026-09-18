#!/usr/bin/env python3
"""
Reorganize applets: move each category into a subdirectory.
The original file becomes common.rs, and mod.rs re-exports everything.
"""

import re
from pathlib import Path

def process_category(base_dir, category):
    """Move category file into subdirectory structure."""
    source_file = base_dir / f"{category}.rs"
    output_dir = base_dir / category

    if not source_file.exists():
        print(f"Skipping {category} - file not found")
        return None

    print(f"\n=== Processing {category}.rs ===")

    with open(source_file, 'r') as f:
        content = f.read()

    # Find all applet functions
    applet_pattern = r'pub fn (r#)?(\w+)\(_?argc: i32, _?argv: \*const \*const u8\) -> i32'
    matches = list(re.finditer(applet_pattern, content))

    if not matches:
        print(f"  No applets found")
        return None

    applet_names = []
    for m in matches:
        raw_prefix = m.group(1) or ""
        name = raw_prefix + m.group(2)
        applet_names.append(name)

    print(f"  Found {len(applet_names)} applets")

    # Create output directory
    output_dir.mkdir(exist_ok=True)

    # Move original file to common.rs with updated imports
    common_content = content.replace(
        "use super::{get_arg, has_opt};",
        "use super::super::{get_arg, has_opt, is_opt, is_option};"
    ).replace(
        "use super::{get_arg, has_opt, is_opt};",
        "use super::super::{get_arg, has_opt, is_opt, is_option};"
    ).replace(
        "use super::{get_arg, has_opt, is_opt, is_option};",
        "use super::super::{get_arg, has_opt, is_opt, is_option};"
    )

    with open(output_dir / "common.rs", 'w') as f:
        f.write(common_content)
    print(f"  Wrote common.rs ({len(content)} bytes)")

    # Create mod.rs that re-exports everything from common
    mod_content = f"""//! {category} applets

mod common;

// Re-export all applet functions
"""

    for name in sorted(applet_names):
        mod_content += f"pub use common::{name};\n"

    with open(output_dir / "mod.rs", 'w') as f:
        f.write(mod_content)
    print(f"  Wrote mod.rs")

    # Delete original file
    source_file.unlink()
    print(f"  Deleted {category}.rs")

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
