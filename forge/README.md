# Forge — MAGE Toolchain & Package Registry

Forge is the MAGE toolchain. It has two parts:

1. **Project toolchain** (`forge` binary, [`src/project.rs`](src/project.rs)) —
   a manifest-driven build/run driver over `Forge.toml`.
2. **Package registry** (`forge-server` binary + client library) — the central
   repository for MAGE modules, with crates.io compatibility.

## Project toolchain

A Forge project is a directory with a `Forge.toml` manifest and a `.mg` entry
point (default `src/main.mg`). The `forge` binary locates the real MAGE
compiler/evaluator (`mage-parse`) and drives it:

```bash
cargo build --release --bin forge   # build the toolchain

forge new <name>     # scaffold Forge.toml + src/main.mg (checks + runs as-is)
forge check          # parse + typecheck the entry point
forge build          # check, then lower through the Agentic Binary Language IR
forge run [fn]       # execute the entry function (default: the manifest's `main`)
forge fmt [--human]  # reformat the entry in place (agent or human surface)
forge info           # print the resolved manifest
```

### Agentic-first interface

Forge self-describes, so an agent never needs these prose docs (the same
progressive-disclosure pattern the mage-parse CLI and `rmi` ship):

```bash
forge manifest          # token-compact, effect-classed command index (read first)
forge manifest --json   # the same, machine-readable
forge describe run      # expand one command
```

Every command takes `--json` for deterministic, machine-readable output an agent
can parse, cache, and gate on — instead of scraping prose:

```bash
$ forge check --json
{"command": "check", "ok": true, "project": "demo", "version": "0.1.0", "entry": "src/main.mg"}
$ forge run --json
{"command": "run", "ok": true, "project": "demo", "fn": "main", "result": "120"}
```

Each command carries an **effect class** (`pure` / `read_local` / `write_local`)
in the manifest, so an agent policy can gate invocations without trial-running
them. `new` is the only `write_local` command outside an explicit target.

`Forge.toml`:

```toml
[module]
name = "my-project"
version = "0.1.0"
edition = "2025"
license = "Apache-2.0"

[build]            # optional
entry = "src/main.mg"   # default
main  = "main"          # entry function for `forge run`
```

The compiler is auto-located at `prototype/target/release/mage-parse` (found
by walking up from the project), or taken from the `FORGE_MG` environment
variable.

## Package registry

The registry serves dual-format packages (native `.mg` and transpiled `.rs`)
and crates.io compatibility.

## Directory Structure

```
forge/
├── README.md                    # This file
├── Cargo.toml                   # Rust project manifest
├── src/
│   ├── lib.rs                   # Library root
│   ├── models/
│   │   ├── mod.rs               # Data model module
│   │   ├── module.rs            # Module (package) data model
│   │   ├── version.rs           # SemVer version handling
│   │   ├── metadata.rs          # Module metadata
│   │   ├── dependency.rs        # Dependency specification
│   │   ├── skb_rule.rs          # Package-level SKB rules
│   │   ├── spec.rs              # API contract (spec) blocks
│   │   └── effect.rs            # Effect declarations
│   ├── api/
│   │   ├── mod.rs               # API route module
│   │   ├── routes.rs            # Route definitions
│   │   ├── handlers.rs          # Request handlers
│   │   └── errors.rs            # API error types
│   ├── registry/
│   │   ├── mod.rs               # Registry operations module
│   │   ├── publish.rs           # Publish workflow
│   │   ├── resolve.rs           # Dependency resolution
│   │   └── cache.rs             # MLIR artifact cache
│   ├── compat/
│   │   ├── mod.rs               # Compatibility layer module
│   │   └── crates_io.rs         # crates.io bridge
│   └── cli/
│       ├── mod.rs               # CLI module
│       └── commands.rs          # forge CLI commands
├── config/
│   ├── forge-config.toml        # Registry server configuration
│   └── module-schema.json       # JSON Schema for module metadata
└── tests/
    └── integration.rs           # Integration tests
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Forge Registry                  │
│                                                  │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │
│  │ REST API │  │ Registry  │  │ MLIR Cache   │  │
│  │ /api/v1  │──│ Core      │──│ Artifacts    │  │
│  └──────────┘  └───────────┘  └──────────────┘  │
│       │              │              │            │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │
│  │ Auth &   │  │ SAT-based │  │ crates.io    │  │
│  │ Owners   │  │ Resolver  │  │ Compat Layer │  │
│  └──────────┘  └───────────┘  └──────────────┘  │
│                      │                           │
│               ┌──────┴──────┐                    │
│               │  Storage    │                    │
│               │  Backend    │                    │
│               └─────────────┘                    │
└─────────────────────────────────────────────────┘
```

## API Endpoints

| Endpoint                                    | Method | Description               |
| ------------------------------------------- | :----: | ------------------------- |
| `/api/v1/modules`                           |  GET   | Search/list modules       |
| `/api/v1/modules/{name}`                    |  GET   | Module metadata           |
| `/api/v1/modules/{name}/{version}`          |  GET   | Specific version          |
| `/api/v1/modules/{name}/{version}/download` |  GET   | Download tarball          |
| `/api/v1/modules/{name}/{version}/mlir`     |  GET   | Pre-cached MLIR artifacts |
| `/api/v1/modules/{name}/{version}/skb`      |  GET   | SKB rules for this module |
| `/api/v1/modules/{name}/{version}/specs`    |  GET   | Published API contracts   |
| `/api/v1/modules/new`                       |  PUT   | Publish new module        |
| `/api/v1/modules/{name}/owners`             |  GET   | List owners               |
| `/api/v1/modules/{name}/owners`             |  PUT   | Update owners             |
| `/api/v1/audit/{name}/{version}`            |  GET   | Security audit report     |

## Key Features

- **Dual-format packages**: Native MAGE (`.mg`) + optional Rust (`.rs`)
- **MLIR artifact caching**: Pre-lowered artifacts for fast builds (>95% hit rate)
- **SAT-based dependency resolution**: Handles MAGE + Rust deps together
- **Effect compatibility checking**: Rejects deps with incompatible effects
- **SKB rule merging**: Combines package-level safety rules into project SKB
- **Spec verification**: Verifies API contracts at resolution time
- **crates.io bridge**: Import Rust crates, publish to both registries

## Usage

```bash
# Publish a module to Forge
mg publish

# Search for modules
mg search http

# Install a binary
mg install my-tool

# Publish to both Forge and crates.io
mg publish --also-crates-io
```

## Configuration

See [config/forge-config.toml](config/forge-config.toml) for server configuration
and [config/module-schema.json](config/module-schema.json) for the module
metadata schema.
