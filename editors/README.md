# Redox Editor Support

Configuration files for using Redox with various editors.

## Editors

| Editor | Directory | Status |
|--------|-----------|--------|
| [VS Code](../redox-vscode/) | `redox-vscode/` | Full extension (TextMate + RAP) |
| [Neovim](neovim/) | `editors/neovim/` | LSP + tree-sitter + ftdetect |
| [Helix](helix/) | `editors/helix/` | Language config + queries |
| [Zed](zed/) | `editors/zed/` | Extension manifest + highlights |

## Quick Setup

### Neovim

```lua
-- Add to your init.lua or lazy.nvim config:
require('lspconfig').rap.setup({
  cmd = { 'rap' },
  filetypes = { 'redox' },
  root_dir = function(fname)
    return require('lspconfig.util').root_pattern('Forge.toml')(fname)
  end,
})
```

### Helix

```bash
# Copy language config into your Helix config directory:
cp editors/helix/languages.toml ~/.config/helix/languages.toml
cp -r editors/helix/queries ~/.config/helix/runtime/queries/redox
```

### Zed

```bash
# Install from the Zed extension directory:
cp -r editors/zed ~/.config/zed/extensions/redox
```

## RAP (Redox Agent Protocol)

All editors connect to the same RAP language server for:
- Diagnostics (errors, warnings, SKB violations)
- Completion (type-aware, effect-aware)
- Hover (type info, effect signatures, cost oracle)
- Go-to-definition, find-references
- Code actions (auto-fix, extract function, inline)
- Formatting (`rdx fmt` integration)

Start the server with:

```bash
rdx rap
```
