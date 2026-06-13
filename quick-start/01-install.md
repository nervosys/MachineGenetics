# Step 1: Install MAGE

## Option A: Pre-Built Binary (Recommended)

### Linux / macOS

```bash
curl -sSf https://MAGE-lang.org/install.sh | sh
```

This installs:
- `mg` — the MAGE CLI (compiler, runner, formatter, linter)
- `mg-rap` — the language server for editor integration

### Windows

```powershell
irm https://MAGE-lang.org/install.ps1 | iex
```

Or download the `.msi` installer from the
[releases page](https://github.com/nervosys/MAGE/releases).

## Option B: Build from Source

```bash
git clone https://github.com/nervosys/MAGE.git
cd MAGE
cargo build --release -p mg
```

The binary will be at `target/release/mg`.

## Verify Installation

```bash
mg --version
# mg 0.1.0
```

```bash
mg help
# USAGE: mg <COMMAND>
#
# Commands:
#   new       Create a new MAGE project
#   build     Compile a MAGE project
#   run       Build and run a MAGE program
#   check     Type-check without building
#   test      Run tests
#   fmt       Format source code
#   lint      Run linter
#   doc       Generate documentation
#   bench     Run benchmarks
#   rap       Start the RAP language server
#   skb       Manage Safety Knowledge Base rules
#   repl      Interactive REPL
#   eval      Evaluate a MAGE expression
```

## Editor Setup (Optional)

### VS Code

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "MAGE"
4. Install the **MAGE Language** extension

This gives you:
- Syntax highlighting for `.mg` files
- Error underlining via the RAP server
- Code completion and hover information

### Other Editors

Any editor that supports TextMate grammars can use the grammar file in
`MAGE-vscode/syntaxes/MAGE.tmLanguage.json`.

For Vim/Neovim, use the RAP server directly:

```vim
" In your LSP config
lua require('lspconfig').mage_rap.setup{}
```

---

**[Next: Hello, World! →](02-hello-world.md)**
