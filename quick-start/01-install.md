# Step 1: Install MechGen

## Option A: Pre-Built Binary (Recommended)

### Linux / macOS

```bash
curl -sSf https://MechGen-lang.org/install.sh | sh
```

This installs:
- `mg` — the MechGen CLI (compiler, runner, formatter, linter)
- `mg-rap` — the language server for editor integration

### Windows

```powershell
irm https://MechGen-lang.org/install.ps1 | iex
```

Or download the `.msi` installer from the
[releases page](https://github.com/nervosys/MechGen/releases).

## Option B: Build from Source

```bash
git clone https://github.com/nervosys/MechGen.git
cd MechGen
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
#   new       Create a new MechGen project
#   build     Compile a MechGen project
#   run       Build and run a MechGen program
#   check     Type-check without building
#   test      Run tests
#   fmt       Format source code
#   lint      Run linter
#   doc       Generate documentation
#   bench     Run benchmarks
#   rap       Start the RAP language server
#   skb       Manage Safety Knowledge Base rules
#   repl      Interactive REPL
#   eval      Evaluate a MechGen expression
```

## Editor Setup (Optional)

### VS Code

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "MechGen"
4. Install the **MechGen Language** extension

This gives you:
- Syntax highlighting for `.mg` files
- Error underlining via the RAP server
- Code completion and hover information

### Other Editors

Any editor that supports TextMate grammars can use the grammar file in
`MechGen-vscode/syntaxes/MechGen.tmLanguage.json`.

For Vim/Neovim, use the RAP server directly:

```vim
" In your LSP config
lua require('lspconfig').mechgen_rap.setup{}
```

---

**[Next: Hello, World! →](02-hello-world.md)**
