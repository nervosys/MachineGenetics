# Forge — MechGen Package Registry

Forge is the package registry for the MechGen programming language. It serves as
the central repository for MechGen modules, supporting dual-format packages
(native `.mg` and transpiled `.rs`), MLIR artifact caching, and crates.io
compatibility.

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
