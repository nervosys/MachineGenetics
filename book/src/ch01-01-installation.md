# Installation

## Prerequisites

Redox requires:
- A 64-bit OS (Linux, macOS, or Windows)
- LLVM 18+ (bundled with the Redox installer)
- Git (for project management)

## Installing Redox

### Using the installer (recommended)

```sh
curl -sSf https://redox-lang.org/install.sh | sh
```

On Windows, download and run the installer from
[redox-lang.org/install](https://redox-lang.org/install).

This installs:
- `rdx` — the unified CLI (build, test, run, format, lint, etc.)
- `rdxc` — the Redox compiler
- `rap` — the Redox Agent Protocol language server
- `rust2rdx` — the Rust-to-Redox migration tool

### Verifying the installation

```sh
rdx --version
# redox 0.1.0 (2025 edition)
```

### Updating

```sh
rdx self update
```

## Editor Setup

### VS Code (recommended)

Install the **Redox** extension from the VS Code marketplace:

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "Redox"
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
