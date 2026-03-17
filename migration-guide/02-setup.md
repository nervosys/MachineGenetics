# Chapter 2: Project Setup & Tooling

Set up a Redox project structure, configure Forge.toml, and establish a
dual-build workflow so Rust and Redox can coexist during migration.

---

## 2.1 Creating the Redox Project

```bash
# Option A: New project alongside existing Rust project
rdx new my-project-rdx
cd my-project-rdx

# Option B: Initialize Redox in an existing directory
cd my-existing-project
rdx init
```

This creates:

```
my-project-rdx/
├── Forge.toml          # Project manifest (like Cargo.toml)
├── src/
│   └── main.rdx        # Entry point
└── .redox/
    └── config.toml     # Local Redox configuration
```

## 2.2 Forge.toml Configuration

The Forge.toml is Redox's equivalent of Cargo.toml:

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2025"
description = "Migrated from Rust"

[dependencies]
# Redox packages from the Forge registry
# redox-http = "0.3"

[rust-dependencies]
# Existing Rust crates — compiled by Cargo, linked into the Redox build
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
├── Forge.toml          # New Redox build
├── src/
│   ├── main.rs         # Rust entry point (keep working)
│   └── main.rdx        # Redox entry point (migrate into)
├── src/models/
│   ├── user.rs         # Not yet migrated
│   └── user.rdx        # Migrated version
└── .github/workflows/
    └── ci.yml          # Run both cargo test AND rdx test
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

  redox:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: redox-lang/setup-rdx@v1
      - run: rdx check
      - run: rdx test
      - run: rdx lint
```

## 2.4 Directory Structure Conventions

Redox follows similar conventions to Rust:

```
src/
├── main.rdx            # Binary entry point (+f main())
├── lib.rdx             # Library root (replaces lib.rs)
├── models/
│   ├── mod.rdx         # Module declaration (replaces mod.rs)
│   ├── user.rdx        # User model
│   └── order.rdx       # Order model
├── handlers/
│   ├── mod.rdx
│   ├── auth.rdx
│   └── api.rdx
└── utils/
    ├── mod.rdx
    ├── config.rdx
    └── helpers.rdx
```

### Mapping Rust Files to Redox

| Rust            | Redox            | Notes                        |
| --------------- | ---------------- | ---------------------------- |
| `main.rs`       | `main.rdx`       | Same role                    |
| `lib.rs`        | `lib.rdx`        | Same role                    |
| `mod.rs`        | `mod.rdx`        | Same role                    |
| `foo.rs`        | `foo.rdx`        | Same role                    |
| `foo/mod.rs`    | `foo/mod.rdx`    | Same role                    |
| `build.rs`      | `build.rdx`      | Build script (rarely needed) |
| `tests/*.rs`    | `tests/*.rdx`    | Integration tests            |
| `benches/*.rs`  | `benches/*.rdx`  | Benchmarks                   |
| `examples/*.rs` | `examples/*.rdx` | Example programs             |

## 2.5 Editor Setup

### VS Code

Install the Redox VS Code extension:

```bash
code --install-extension nervosys.redox-lang
```

The extension provides:
- Syntax highlighting (`.rdx` files)
- RAP language server (diagnostics, completion, go-to-definition)
- Effect annotation hints
- Inline SKB rule feedback

### Settings (`.vscode/settings.json`)

```json
{
    "redox.rapPath": "rdx",
    "redox.checkOnSave": true,
    "redox.effectHints": true,
    "files.associations": {
        "*.rdx": "redox"
    }
}
```

## 2.6 CLI Quick Reference

Commands you'll use during migration:

```bash
rdx build              # Compile the project
rdx check              # Type-check without building (fast)
rdx test               # Run all tests
rdx fmt                # Format all .rdx files
rdx lint               # Run linter
rdx run                # Build and run
rdx migrate <path>     # Auto-translate .rs files to .rdx
rdx migrate --dry-run  # Preview migration without writing
rdx doc                # Generate documentation
rdx bench              # Run benchmarks
```

## 2.7 Incremental Migration Strategy

You don't have to migrate everything at once. Recommended order:

```
1. Leaf modules first     (models, types, utilities — no dependencies)
2. Internal libraries     (business logic, data transformations)
3. I/O boundaries         (file handling, network — add effects here)
4. Async code             (spawn sites, runtime setup)
5. Entry point            (main.rs → main.rdx)
6. Tests                  (port test suite last)
```

### Module-by-Module Checklist

For each module being migrated:

- [ ] Create `module.rdx` alongside `module.rs`
- [ ] Run `rdx migrate src/module.rs --output src/module.rdx`
- [ ] Review and fix automated translation
- [ ] Add effect annotations to impure functions
- [ ] Update `mod.rdx` to include the new module
- [ ] Run `rdx check` — fix any errors
- [ ] Run `rdx test` — verify behavior matches
- [ ] Remove `module.rs` once confident
- [ ] Update `mod.rs` / Cargo.toml if needed
