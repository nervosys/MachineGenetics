# Redox Quick Start

Get from zero to running Redox code in 10 minutes.

---

## What is Redox?

Redox is the **agentic-first programming language** — designed for AI
agents to read, write, and reason about code with minimal token cost.
It compiles to native code via MLIR/LLVM with the safety of Rust and
the conciseness of a purpose-built syntax.

## Guide Structure

| Step | Page                                     | Time  |
| ---- | ---------------------------------------- | ----- |
| 1    | [Install Redox](01-install.md)           | 2 min |
| 2    | [Hello, World!](02-hello-world.md)       | 2 min |
| 3    | [Syntax in 5 Minutes](03-syntax-tour.md) | 5 min |
| 4    | [Build, Run, Test](04-build-run-test.md) | 2 min |
| 5    | [What's Next?](05-whats-next.md)         | 1 min |

## Prerequisites

- A terminal (any OS)
- A text editor (VS Code recommended — install the
  [Redox extension](https://marketplace.visualstudio.com/items?itemName=nervosys.redox-vscode)
  for syntax highlighting)

## Quick Overview

```redox
// hello.rdx — your first Redox program
+f main() {
    p"Hello, Redox!"
}
```

```bash
rdx run hello.rdx
# Hello, Redox!
```

That's it. No boilerplate, no imports, no ceremony. Let's get started.

**[Start: Install Redox →](01-install.md)**
