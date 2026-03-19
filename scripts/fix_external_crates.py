#!/usr/bin/env python3
"""
Fix incorrectly renamed external crate names.

The rebrand script's bare "rustc" -> "redox" replacement incorrectly renamed
external crates published on crates.io that start with "rustc-".
This script reverts those specific replacements.
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# External crates that were incorrectly renamed: redox-xxx -> rustc-xxx
FIXUPS = [
    ("rustc-literal-escaper", "rustc-literal-escaper"),
    ("rustc-demangle", "rustc-demangle"),
    ("rustc-hash", "rustc-hash"),
    ("rustc-rayon-core", "rustc-rayon-core"),  # must come before rustc-rayon
    ("rustc-rayon", "rustc-rayon"),
    ("rustc-perf", "rustc-perf"),  # URLs in comments
    # The workspace shim crate names should stay as rustc-std-workspace-*
    ("rustc-std-workspace-alloc", "rustc-std-workspace-alloc"),
    ("rustc-std-workspace-core", "rustc-std-workspace-core"),
    ("rustc-std-workspace-std", "rustc-std-workspace-std"),
    ("rustc-std-workspace", "rustc-std-workspace"),
]

# Also fix incorrectly renamed publish path
FIXUPS_AP = [
    ("rustc-ap-rustc_lexer", "rustc-ap-rustc_lexer"),
]


def fix_file(filepath, fixups, dry_run=False):
    try:
        content = filepath.read_text(encoding="utf-8")
    except (UnicodeDecodeError, PermissionError):
        return 0

    original = content
    total = 0
    for wrong, correct in fixups:
        count = content.count(wrong)
        if count > 0:
            content = content.replace(wrong, correct)
            total += count

    if total > 0:
        if dry_run:
            print(f"  FIX: {filepath.relative_to(ROOT)} ({total} fixups)")
        else:
            filepath.write_text(content, encoding="utf-8")
    return total


def main():
    dry_run = "--dry-run" in sys.argv
    mode = "DRY RUN" if dry_run else "APPLYING"
    print(f"=== Fix External Crate Names ({mode}) ===")

    all_fixups = FIXUPS_AP + FIXUPS
    total = 0

    # Fix all Cargo.toml files
    for toml_file in ROOT.rglob("Cargo.toml"):
        n = fix_file(toml_file, all_fixups, dry_run)
        total += n

    # Fix .rs files in compiler/ that might reference these in comments or use statements
    for rs_file in (ROOT / "compiler").rglob("*.rs"):
        n = fix_file(rs_file, all_fixups, dry_run)
        total += n

    # Fix .rs files in src/ 
    for rs_file in (ROOT / "src").rglob("*.rs"):
        n = fix_file(rs_file, all_fixups, dry_run)
        total += n

    # Fix .rs files in library/
    for rs_file in (ROOT / "library").rglob("*.rs"):
        n = fix_file(rs_file, all_fixups, dry_run)
        total += n

    # Fix .toml, .py, .yml files in src/
    for ext in ("*.toml", "*.py", "*.yml", "*.yaml"):
        for filepath in ROOT.rglob(ext):
            n = fix_file(filepath, all_fixups, dry_run)
            total += n

    # Fix root config files
    for filename in ["triagebot.toml", "REUSE.toml"]:
        fp = ROOT / filename
        if fp.exists():
            n = fix_file(fp, all_fixups, dry_run)
            total += n

    print(f"\nTotal: {total} fixups applied")
    if dry_run:
        print("Run without --dry-run to apply.")


if __name__ == "__main__":
    main()
