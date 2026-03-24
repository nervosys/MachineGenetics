# Installation

## Prerequisites

MechGen requires:
- A 64-bit OS (Linux, macOS, or Windows)
- LLVM 18+ (bundled with the MechGen installer)
- Git (for project management)

## Installing MechGen

### Using the installer (recommended)

```sh
curl -sSf https://MechGen-lang.org/install.sh | sh
```

On Windows, download and run the installer from
[MechGen-lang.org/install](https://MechGen-lang.org/install).

This installs:
- `mg` — the unified CLI (build, test, run, format, lint, etc.)
- `rdxc` — the MechGen compiler
- `rap` — the MechGen Agent Protocol language server
- `rust2mg` — the Rust-to-MechGen migration tool

### Verifying the installation

```sh
mg --version
# MechGen 0.1.0 (2025 edition)
```

### Updating

```sh
mg self update
```

## Editor Setup

### VS Code (recommended)

Install the **MechGen** extension from the VS Code marketplace:

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "MechGen"
4. Click Install

The extension provides syntax highlighting, RAP integration (diagnostics, hover,
completion), inline cost annotations, and effect visualization.

### Neovim / Helix

Add the RAP server to your LSP configuration:

```lua
-- Neovim (nvim-lspconfig)
require('lspconfig').rap.setup{}
```

RAP speaks a superset of the Language Server Protocol, so any LSP-compatible
editor works out of the box.
