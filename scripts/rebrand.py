#!/usr/bin/env python3
"""
Rebrand script: rename all rustc_* compiler crates to redox_*.

This script handles:
1. Renaming compiler/ crate directories (rustc_* -> redox_*, rustc -> redox)
2. Updating Cargo.toml package names and path dependencies
3. Updating Rust source use/extern-crate/crate-path references
4. Updating the workspace Cargo.toml
5. Updating bootstrap/build system references
6. Updating library/ compiler attribute names (rustc_* -> redox_*)
7. Updating src/tools/ compiler crate imports

Usage:
    python scripts/rebrand.py --dry-run    # Preview changes
    python scripts/rebrand.py              # Apply changes
    python scripts/rebrand.py --verify     # Check for remaining rustc_ refs
"""

import os
import re
import sys
import shutil
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).resolve().parent.parent
COMPILER_DIR = ROOT / "compiler"

# ─── Configuration ───────────────────────────────────────────────────────────

OLD_PREFIX = "rustc_"
NEW_PREFIX = "redox_"
OLD_DRIVER = "rustc"
NEW_DRIVER = "redox"

# External crates that happen to start with "rustc" but should NOT be renamed.
# These use dashes (rustc-xxx) or are external dependencies.
EXTERNAL_CRATES = {
    "rustc-literal-escaper",
    "rustc-demangle",
    "rustc-hash",
    "rustc-rayon",
    "rustc-rayon-core",
    "rustc-std-workspace-alloc",
    "rustc-std-workspace-core",
    "rustc-std-workspace-std",
    "rustc-main",
    "rustc-ap-rustc_lexer",
}

# Patterns that contain rustc_ but should NOT be replaced (regexes).
# These are upstream Rust concepts, not our crate names.
SKIP_PATTERNS = [
    # Redox OS target references (target_os = "redox" doesn't contain rustc_)
    # URLs to upstream rust-lang repo (we DO want to keep these as-is in comments)
    r'https?://github\.com/rust-lang/rust',
    r'https?://doc\.rust-lang\.org',
]


def get_compiler_crate_dirs():
    """Get all compiler crate directory names (the ones to rename)."""
    dirs = []
    for entry in sorted(COMPILER_DIR.iterdir()):
        if entry.is_dir():
            dirs.append(entry.name)
    return dirs


def build_rename_map():
    """Build a mapping of old crate names to new crate names."""
    rename_map = {}
    for dirname in get_compiler_crate_dirs():
        if dirname == OLD_DRIVER:
            rename_map[dirname] = NEW_DRIVER
        elif dirname.startswith(OLD_PREFIX):
            suffix = dirname[len(OLD_PREFIX):]
            rename_map[dirname] = NEW_PREFIX + suffix
        # else: skip non-rustc directories
    return rename_map


def rename_directories(rename_map, dry_run=False):
    """Rename compiler crate directories."""
    renamed = []
    for old_name, new_name in sorted(rename_map.items()):
        old_path = COMPILER_DIR / old_name
        new_path = COMPILER_DIR / new_name
        if old_path.exists():
            if dry_run:
                print(f"  RENAME DIR: {old_path.relative_to(ROOT)} -> {new_path.relative_to(ROOT)}")
            else:
                shutil.move(str(old_path), str(new_path))
            renamed.append((old_name, new_name))
    return renamed


def replace_in_file(filepath, replacements, dry_run=False):
    """
    Apply text replacements to a file.
    replacements: list of (old_text, new_text) tuples
    Returns number of replacements made.
    """
    try:
        content = filepath.read_text(encoding="utf-8")
    except (UnicodeDecodeError, PermissionError):
        return 0

    original = content
    total = 0
    for old_text, new_text in replacements:
        count = content.count(old_text)
        if count > 0:
            content = content.replace(old_text, new_text)
            total += count

    if total > 0:
        if dry_run:
            print(f"  MODIFY: {filepath.relative_to(ROOT)} ({total} replacements)")
        else:
            filepath.write_text(content, encoding="utf-8")

    return total


def build_replacements(rename_map):
    """
    Build ordered list of text replacements.
    Longer patterns first to avoid partial matches.
    """
    replacements = []

    # Sort by length descending to match longest first
    for old_name in sorted(rename_map.keys(), key=len, reverse=True):
        new_name = rename_map[old_name]
        # Crate name with underscore (most common in Rust code)
        replacements.append((old_name, new_name))

    # The compiler driver binary package is "rustc-main" (with dash)
    replacements.append(("rustc-main", "redox-main"))

    return replacements


def build_path_replacements(rename_map):
    """Build replacements specifically for Cargo.toml path references."""
    replacements = []
    for old_name in sorted(rename_map.keys(), key=len, reverse=True):
        new_name = rename_map[old_name]
        # Path references: "../rustc_xxx" -> "../redox_xxx"
        replacements.append((f"../{old_name}", f"../{new_name}"))
        # Workspace member references: "compiler/rustc_xxx" -> "compiler/redox_xxx"
        replacements.append((f"compiler/{old_name}", f"compiler/{new_name}"))
    return replacements


def process_compiler_crates(rename_map, dry_run=False):
    """Update all files within compiler/ crates."""
    text_replacements = build_replacements(rename_map)
    path_replacements = build_path_replacements(rename_map)

    stats = defaultdict(int)

    for crate_dir in sorted(COMPILER_DIR.iterdir()):
        if not crate_dir.is_dir():
            continue

        # Process Cargo.toml
        cargo_toml = crate_dir / "Cargo.toml"
        if cargo_toml.exists():
            # For Cargo.toml, apply both name replacements and path replacements
            all_replacements = path_replacements + text_replacements
            n = replace_in_file(cargo_toml, all_replacements, dry_run)
            stats["cargo_toml"] += n

        # Process all .rs files
        for rs_file in crate_dir.rglob("*.rs"):
            n = replace_in_file(rs_file, text_replacements, dry_run)
            stats["rs_files"] += n

        # Process any .toml files (besides Cargo.toml)
        for toml_file in crate_dir.rglob("*.toml"):
            if toml_file.name != "Cargo.toml":
                all_replacements = path_replacements + text_replacements
                n = replace_in_file(toml_file, all_replacements, dry_run)
                stats["other_toml"] += n

    return stats


def process_workspace_cargo_toml(rename_map, dry_run=False):
    """Update the top-level workspace Cargo.toml."""
    cargo_toml = ROOT / "Cargo.toml"
    path_replacements = build_path_replacements(rename_map)
    text_replacements = build_replacements(rename_map)
    all_replacements = path_replacements + text_replacements
    return replace_in_file(cargo_toml, all_replacements, dry_run)


def process_library(rename_map, dry_run=False):
    """
    Update library/ files. These contain compiler attributes like
    #[rustc_diagnostic_item], #[rustc_const_stable], etc.
    """
    library_dir = ROOT / "library"
    if not library_dir.exists():
        return 0

    text_replacements = build_replacements(rename_map)
    total = 0

    for rs_file in library_dir.rglob("*.rs"):
        n = replace_in_file(rs_file, text_replacements, dry_run)
        total += n

    for toml_file in library_dir.rglob("*.toml"):
        path_replacements = build_path_replacements(rename_map)
        all_replacements = path_replacements + text_replacements
        n = replace_in_file(toml_file, all_replacements, dry_run)
        total += n

    return total


def process_src(rename_map, dry_run=False):
    """
    Update src/ files (bootstrap, tools, etc.).
    Tools like clippy, miri, rustdoc import compiler crates.
    Bootstrap knows compiler crate names.
    """
    src_dir = ROOT / "src"
    if not src_dir.exists():
        return 0

    text_replacements = build_replacements(rename_map)
    path_replacements = build_path_replacements(rename_map)
    total = 0

    for ext in ("*.rs", "*.toml", "*.py", "*.yml", "*.yaml", "*.json", "*.md"):
        for filepath in src_dir.rglob(ext):
            if ext == "*.toml":
                all_replacements = path_replacements + text_replacements
            else:
                all_replacements = text_replacements
            n = replace_in_file(filepath, all_replacements, dry_run)
            total += n

    return total


def process_tests(rename_map, dry_run=False):
    """Update tests/ directory references to compiler crates."""
    tests_dir = ROOT / "tests"
    if not tests_dir.exists():
        return 0

    text_replacements = build_replacements(rename_map)
    total = 0

    for rs_file in tests_dir.rglob("*.rs"):
        n = replace_in_file(rs_file, text_replacements, dry_run)
        total += n

    return total


def process_root_files(rename_map, dry_run=False):
    """Update root-level config files that reference compiler crates."""
    text_replacements = build_replacements(rename_map)
    path_replacements = build_path_replacements(rename_map)
    all_replacements = path_replacements + text_replacements

    total = 0
    for filename in ["triagebot.toml", "rustfmt.toml", "typos.toml",
                     "REUSE.toml", "rust-bors.toml"]:
        filepath = ROOT / filename
        if filepath.exists():
            n = replace_in_file(filepath, all_replacements, dry_run)
            total += n

    return total


def verify(rename_map):
    """Check for remaining rustc_ references that should have been renamed."""
    print("\n=== Verification ===")
    old_names = set(rename_map.keys())

    remaining = defaultdict(list)

    # Check compiler/
    for filepath in (ROOT / "compiler").rglob("*"):
        if filepath.is_file() and filepath.suffix in (".rs", ".toml"):
            try:
                content = filepath.read_text(encoding="utf-8")
            except (UnicodeDecodeError, PermissionError):
                continue
            for old_name in old_names:
                if old_name in content:
                    remaining[old_name].append(str(filepath.relative_to(ROOT)))

    if remaining:
        print(f"  WARNING: {len(remaining)} old crate names still found:")
        for name, files in sorted(remaining.items()):
            print(f"    {name}: {len(files)} files")
            for f in files[:3]:
                print(f"      - {f}")
            if len(files) > 3:
                print(f"      ... and {len(files) - 3} more")
    else:
        print("  OK: No old crate names found in compiler/")


def main():
    dry_run = "--dry-run" in sys.argv
    verify_only = "--verify" in sys.argv

    rename_map = build_rename_map()

    if verify_only:
        verify(rename_map)
        return

    mode = "DRY RUN" if dry_run else "APPLYING"
    print(f"=== Redox Rebrand Script ({mode}) ===")
    print(f"Renaming {len(rename_map)} crates: {OLD_PREFIX}* -> {NEW_PREFIX}*")
    print()

    # Step 1: Rename directories
    print("Phase 1: Renaming directories...")
    if not dry_run:
        renamed = rename_directories(rename_map, dry_run=False)
        print(f"  Renamed {len(renamed)} directories")
    else:
        rename_directories(rename_map, dry_run=True)
    print()

    # Step 2: Update compiler crate files
    print("Phase 2: Updating compiler/ crate files...")
    stats = process_compiler_crates(rename_map, dry_run)
    print(f"  Cargo.toml: {stats['cargo_toml']} replacements")
    print(f"  .rs files: {stats['rs_files']} replacements")
    print(f"  Other .toml: {stats['other_toml']} replacements")
    print()

    # Step 3: Update workspace Cargo.toml
    print("Phase 3: Updating workspace Cargo.toml...")
    n = process_workspace_cargo_toml(rename_map, dry_run)
    print(f"  {n} replacements")
    print()

    # Step 4: Update library/
    print("Phase 4: Updating library/ files...")
    n = process_library(rename_map, dry_run)
    print(f"  {n} replacements")
    print()

    # Step 5: Update src/
    print("Phase 5: Updating src/ files...")
    n = process_src(rename_map, dry_run)
    print(f"  {n} replacements")
    print()

    # Step 6: Update tests/
    print("Phase 6: Updating tests/ files...")
    n = process_tests(rename_map, dry_run)
    print(f"  {n} replacements")
    print()

    # Step 7: Update root config files
    print("Phase 7: Updating root config files...")
    n = process_root_files(rename_map, dry_run)
    print(f"  {n} replacements")
    print()

    if not dry_run:
        print("=== Rebrand complete ===")
        print("Run with --verify to check for remaining references.")
    else:
        print("=== Dry run complete (no files modified) ===")
        print("Run without --dry-run to apply changes.")


if __name__ == "__main__":
    main()
