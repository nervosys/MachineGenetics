# MAGE Editor Support

Configuration files for using MAGE with various editors.

## Editors

| Editor                      | Directory         | Status                          |
| --------------------------- | ----------------- | ------------------------------- |
| ~~VS Code~~                  | —                 | **Does not exist.** This row linked to `../MAGE-vscode/`, which is not in the repository and never has been (checked 2026-08-25). |
| [Neovim](neovim/)           | `editors/neovim/` | tree-sitter + ftdetect. **No LSP** — see below |
| [Helix](helix/)             | `editors/helix/`  | Language config + queries. **No LSP** — see below |
| [Zed](zed/)                 | `editors/zed/`    | Extension manifest + highlights |

## Quick Setup

### Neovim

```lua
-- Add to your init.lua or lazy.nvim config:
require('mage').setup()   -- tree-sitter highlighting, ftdetect
```

**There is no language server, and this block used to configure one.** It read
`require('lspconfig').rap.setup({ cmd = { 'rap' } })`, which cannot work:

1. **RAP is not LSP.** `prototype/src/rap.rs` contains zero occurrences of
   `initialize` or `textDocument`. It speaks `language/parse` and 36 other
   custom methods, so the handshake never completes.
2. **There is no `rap` binary.** The server is `mage-parse --rap`.
3. **RAP is TCP**; `cmd` spawns a process and speaks over stdio.

Copying the old block gave you a spawn error on every MAGE buffer. The same
registration has been removed from `editors/helix/languages.toml`, and
`require('mage').setup_lsp()` now says this and returns rather than registering
it — it still accepts `rap_cmd` if you have written your own LSP shim over RAP.

**What does work in every editor here is tree-sitter highlighting**, which is
what the `queries/` and `grammar.js` files are for.

**And formatting, as of 2026-09-02.** Two hours earlier this paragraph said
formatting worked nowhere, and it was true: `mage-parse --fmt-compact` took a
filename, `-` was opened as a file, and every editor's format hook pipes the
buffer through stdin. `--fmt-compact -` now reads standard input, so the one
compiler capability that maps onto an editor feature without any protocol at
all is finally reachable:

| Editor | Formatting |
| --- | --- |
| Neovim | `formatprg` is set on MAGE buffers by `require('mage').setup()`; `gq` formats. Override with `fmt_cmd`, disable with `fmt = false` |
| Helix | `formatter` + `auto-format` in `languages.toml`, already configured |
| Zed, others | run `mage-parse --fmt-compact -` as an external formatter |

It needs `mage-parse` on `PATH`, and it is byte-stable over a pipe
(`fmt(fmt(x)) == fmt(x)`), which is what makes format-on-save safe.

### Helix

```bash
# Copy language config into your Helix config directory:
cp editors/helix/languages.toml ~/.config/helix/languages.toml
cp -r editors/helix/queries ~/.config/helix/runtime/queries/MAGE
```

### Zed

```bash
# Install from the Zed extension directory:
cp -r editors/zed ~/.config/zed/extensions/MAGE
```

## RAP (MAGE Agent Protocol)

All editors connect to the same RAP language server for:
- Diagnostics (errors, warnings, SKB violations)
- Completion (type-aware, effect-aware)
- Hover (type info, effect signatures, cost oracle)
- Go-to-definition, find-references
- Code actions (auto-fix, extract function, inline)
- Formatting (`mg fmt` integration)

Start the server with:

```bash
mg rap
```
