# Step 1: Install Redox

## Option A: Pre-Built Binary (Recommended)

### Linux / macOS

```bash
curl -sSf https://redox-lang.org/install.sh | sh
```

This installs:
- `rdx` — the Redox CLI (compiler, runner, formatter, linter)
- `rdx-rap` — the language server for editor integration

### Windows

```powershell
irm https://redox-lang.org/install.ps1 | iex
```

Or download the `.msi` installer from the
[releases page](https://github.com/nervosys/Redox/releases).

## Option B: Build from Source

```bash
git clone https://github.com/nervosys/Redox.git
cd Redox
cargo build --release -p rdx
```

The binary will be at `target/release/rdx`.

## Verify Installation

```bash
rdx --version
# rdx 0.1.0
```

```bash
rdx help
# USAGE: rdx <COMMAND>
#
# Commands:
#   new       Create a new Redox project
#   build     Compile a Redox project
#   run       Build and run a Redox program
#   check     Type-check without building
#   test      Run tests
#   fmt       Format source code
#   lint      Run linter
#   doc       Generate documentation
#   bench     Run benchmarks
#   rap       Start the RAP language server
#   skb       Manage Safety Knowledge Base rules
#   repl      Interactive REPL
#   eval      Evaluate a Redox expression
```

## Editor Setup (Optional)

### VS Code

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "Redox"
4. Install the **Redox Language** extension

This gives you:
- Syntax highlighting for `.rdx` files
- Error underlining via the RAP server
- Code completion and hover information

### Other Editors

Any editor that supports TextMate grammars can use the grammar file in
`redox-vscode/syntaxes/redox.tmLanguage.json`.

For Vim/Neovim, use the RAP server directly:

```vim
" In your LSP config
lua require('lspconfig').redox_rap.setup{}
```

---

**[Next: Hello, World! →](02-hello-world.md)**
