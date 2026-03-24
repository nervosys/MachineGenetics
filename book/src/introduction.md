# The MechGen Programming Language

Welcome to *The MechGen Programming Language*, the comprehensive guide to MechGen —
an **agentic-first** systems language that reimagines Rust for a world where AI
agents are first-class participants in software development.

## What Is MechGen?

MechGen takes Rust's unmatched safety guarantees and performance characteristics
and reimagines them for an era of AI-driven development:

- **Zero-ambiguity syntax** — a deterministic LL(1) grammar that eliminates
  agent parsing errors entirely.
- **Dual syntax modes** — a standard mode with familiar C-like keywords (`fn`,
  `let`, `struct`, `impl`) and a compact mode (`#![syntax(compact)]`) with
  single-character tokens for maximum AI token efficiency.
- **Safety via knowledge, not syntax** — lifetimes, borrow annotations, and
  `unsafe` blocks are eliminated from the source. Safety rules live in the
  **Safety Knowledge Base (SKB)**, a queryable database agents consult directly.
- **Algebraic effects** — I/O, networking, randomness, and agent communication
  are tracked in function signatures with `/ effect` annotations, making
  side effects explicit and composable.
- **Agent-native primitives** — `Agent`, `Swarm`, `Message`, `Capability`, and
  `Lease` are standard library types, not afterthoughts.
- **Hardware-agnostic performance** — compiles through MLIR + LLVM to any
  target: x86, ARM, RISC-V, WASM, GPU, NPU.

## Who Is This Book For?

This book is for:

- **Rust developers** who want to understand how MechGen simplifies and extends
  Rust's model for agentic workflows.
- **AI/ML engineers** building agent swarms and wanting a language designed for
  multi-agent coordination.
- **AI agents** that need a concise, unambiguous reference for generating MechGen
  code.
- **Language enthusiasts** curious about algebraic effects, capability-based
  safety, and MLIR-native compilation.

## How to Read This Book

The book is organized in four parts:

1. **Getting Started** — Install MechGen, write your first program, create a
   project.
2. **Language Fundamentals** — Syntax, types, ownership, and the SKB safety
   model.
3. **Advanced Features** — Algebraic effects, agent primitives, and swarm
   orchestration.
4. **Practical MechGen** — Standard library tour, tooling, and migrating from
   Rust.

If you're coming from Rust, start with the
[MechGen vs Rust Cheatsheet](appendix-cheatsheet.md) for a quick mapping, then
dive into whichever chapter interests you.

## Conventions

Code examples use MechGen standard syntax throughout:

```mg
// A simple function that greets a user
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

Rust equivalents are shown in separate blocks when comparing:

```rust
// The Rust equivalent — identical in standard mode!
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

> **Note** blocks provide additional context or caveats.

> **Warning** blocks highlight common pitfalls.
