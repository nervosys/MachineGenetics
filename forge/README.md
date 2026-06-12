# Forge — MechGen Toolchain & Package Registry

Forge is the MechGen toolchain. It has two parts:

1. **Project toolchain** (`forge` binary, [`src/project.rs`](src/project.rs)) —
   a manifest-driven build/run driver over `Forge.toml`.
2. **Package registry** (`forge-server` binary + client library) — the central
   repository for MechGen modules, with crates.io compatibility.

## Project toolchain

A Forge project is a directory with a `Forge.toml` manifest and a `.mg` entry
point (default `src/main.mg`). The `forge` binary locates the real MechGen
compiler/evaluator (`MechGen-parse`) and drives it:

```bash
cargo build --release --bin forge   # build the toolchain

forge new <name>     # scaffold Forge.toml + src/main.mg (checks + runs as-is)
forge check          # parse + typecheck the entry point
forge build          # check, then lower through the Agentic Binary Language IR
forge run [fn]       # execute the entry function (default: the manifest's `main`)
forge info           # print the resolved manifest
```

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

The compiler is auto-located at `prototype/target/release/MechGen-parse` (found
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

- **Dual-format packages**: Native MechGen (`.mg`) + optional Rust (`.rs`)
- **MLIR artifact caching**: Pre-lowered artifacts for fast builds (>95% hit rate)
- **SAT-based dependency resolution**: Handles MechGen + Rust deps together
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
