# Chapter 2: Project Setup & Tooling

Set up a MechGen project structure, configure Forge.toml, and establish a
dual-build workflow so Rust and MechGen can coexist during migration.

---

## 2.1 Creating the MechGen Project

```bash
# Option A: New project alongside existing Rust project
mg new my-project-mg
cd my-project-mg

# Option B: Initialize MechGen in an existing directory
cd my-existing-project
mg init
```

This creates:

```
my-project-mg/
├── Forge.toml          # Project manifest (like Cargo.toml)
├── src/
│   └── main.mg        # Entry point
└── .MechGen/
    └── config.toml     # Local MechGen configuration
```

## 2.2 Forge.toml Configuration

The Forge.toml is MechGen's equivalent of Cargo.toml:

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2025"
description = "Migrated from Rust"

[dependencies]
# MechGen packages from the Forge registry
# MechGen-http = "0.3"

[rust-dependencies]
# Existing Rust crates — compiled by Cargo, linked into the MechGen build
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1.0"

[build]
target = "native"         # or "wasm32", "aarch64-linux"
opt-level = 2
lto = true

[effects]
allowed = ["io", "net", "async", "env", "time", "process"]
# denied = ["rng"]        # Uncomment to forbid randomness

[capabilities]
# Grants at build time — runtime can further restrict
grants = [
    "fs.read",
    "fs.write",
    "net.http.get",
    "net.http.post",
    "env.read",
]
```

### Key Differences from Cargo.toml

| Feature      | Cargo.toml          | Forge.toml                                   |
| ------------ | ------------------- | -------------------------------------------- |
| Dependencies | `[dependencies]`    | `[dependencies]` + `[rust-dependencies]`     |
| Features     | `[features]`        | `[features]` (same)                          |
| Build config | `[profile.release]` | `[build]` section                            |
| Effects      | N/A                 | `[effects]` — allowed/denied effect sets     |
| Capabilities | N/A                 | `[capabilities]` — runtime permission grants |
| Workspace    | `[workspace]`       | `[workspace]` (same)                         |

## 2.3 Dual-Build Setup

During migration, maintain both build systems:

```
my-project/
├── Cargo.toml          # Existing Rust build
├── Forge.toml          # New MechGen build
├── src/
│   ├── main.rs         # Rust entry point (keep working)
│   └── main.mg        # MechGen entry point (migrate into)
├── src/models/
│   ├── user.rs         # Not yet migrated
│   └── user.mg        # Migrated version
└── .github/workflows/
    └── ci.yml          # Run both cargo test AND mg test
```

### CI Configuration (GitHub Actions)

```yaml
name: CI
on: [push, pull_request]

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
      - run: cargo clippy -- -D warnings

  MechGen:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: MechGen-lang/setup-mg@v1
      - run: mg check
      - run: mg test
      - run: mg lint
```

## 2.4 Directory Structure Conventions

MechGen follows similar conventions to Rust:

```
src/
├── main.mg            # Binary entry point (+f main())
├── lib.mg             # Library root (replaces lib.rs)
├── models/
│   ├── mod.mg         # Module declaration (replaces mod.rs)
│   ├── user.mg        # User model
│   └── order.mg       # Order model
├── handlers/
│   ├── mod.mg
│   ├── auth.mg
│   └── api.mg
└── utils/
    ├── mod.mg
    ├── config.mg
    └── helpers.mg
```

### Mapping Rust Files to MechGen

| Rust            | MechGen            | Notes                        |
| --------------- | ---------------- | ---------------------------- |
| `main.rs`       | `main.mg`       | Same role                    |
| `lib.rs`        | `lib.mg`        | Same role                    |
| `mod.rs`        | `mod.mg`        | Same role                    |
| `foo.rs`        | `foo.mg`        | Same role                    |
| `foo/mod.rs`    | `foo/mod.mg`    | Same role                    |
| `build.rs`      | `build.mg`      | Build script (rarely needed) |
| `tests/*.rs`    | `tests/*.mg`    | Integration tests            |
| `benches/*.rs`  | `benches/*.mg`  | Benchmarks                   |
| `examples/*.rs` | `examples/*.mg` | Example programs             |

## 2.5 Editor Setup

### VS Code

Install the MechGen VS Code extension:

```bash
code --install-extension nervosys.MechGen-lang
```

The extension provides:
- Syntax highlighting (`.mg` files)
- RAP language server (diagnostics, completion, go-to-definition)
- Effect annotation hints
- Inline SKB rule feedback

### Settings (`.vscode/settings.json`)

```json
{
    "MechGen.rapPath": "mg",
    "MechGen.checkOnSave": true,
    "MechGen.effectHints": true,
    "files.associations": {
        "*.mg": "MechGen"
    }
}
```

## 2.6 CLI Quick Reference

Commands you'll use during migration:

```bash
mg build              # Compile the project
mg check              # Type-check without building (fast)
mg test               # Run all tests
mg fmt                # Format all .mg files
mg lint               # Run linter
mg run                # Build and run
mg migrate <path>     # Auto-translate .rs files to .mg
mg migrate --dry-run  # Preview migration without writing
mg doc                # Generate documentation
mg bench              # Run benchmarks
```

## 2.7 Incremental Migration Strategy

You don't have to migrate everything at once. Recommended order:

```
1. Leaf modules first     (models, types, utilities — no dependencies)
2. Internal libraries     (business logic, data transformations)
3. I/O boundaries         (file handling, network — add effects here)
4. Async code             (spawn sites, runtime setup)
5. Entry point            (main.rs → main.mg)
6. Tests                  (port test suite last)
```

### Module-by-Module Checklist

For each module being migrated:

- [ ] Create `module.mg` alongside `module.rs`
- [ ] Run `mg migrate src/module.rs --output src/module.mg`
- [ ] Review and fix automated translation
- [ ] Add effect annotations to impure functions
- [ ] Update `mod.mg` to include the new module
- [ ] Run `mg check` — fix any errors
- [ ] Run `mg test` — verify behavior matches
- [ ] Remove `module.rs` once confident
- [ ] Update `mod.rs` / Cargo.toml if needed
