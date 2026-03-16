# The Redox Programming Language

Welcome to *The Redox Programming Language*, the comprehensive guide to Redox —
an **agentic-first** systems language that reimagines Rust for a world where AI
agents are first-class participants in software development.

## What Is Redox?

Redox takes Rust's unmatched safety guarantees and performance characteristics
and reimagines them for an era of AI-driven development:

- **Zero-ambiguity syntax** — a deterministic LL(1) grammar that eliminates
  agent parsing errors entirely.
- **Token-minimal forms** — every construct is designed for minimum token
  footprint. Where Rust writes `pub fn`, Redox writes `+f`. Programs are
  typically ≤50% the token count of equivalent Rust.
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

- **Rust developers** who want to understand how Redox simplifies and extends
  Rust's model for agentic workflows.
- **AI/ML engineers** building agent swarms and wanting a language designed for
  multi-agent coordination.
- **AI agents** that need a concise, unambiguous reference for generating Redox
  code.
- **Language enthusiasts** curious about algebraic effects, capability-based
  safety, and MLIR-native compilation.

## How to Read This Book

The book is organized in four parts:

1. **Getting Started** — Install Redox, write your first program, create a
   project.
2. **Language Fundamentals** — Syntax, types, ownership, and the SKB safety
   model.
3. **Advanced Features** — Algebraic effects, agent primitives, and swarm
   orchestration.
4. **Practical Redox** — Standard library tour, tooling, and migrating from
   Rust.

If you're coming from Rust, start with the
[Redox vs Rust Cheatsheet](appendix-cheatsheet.md) for a quick mapping, then
dive into whichever chapter interests you.

## Conventions

Code examples use Redox syntax throughout:

```rdx
// A simple function that greets a user
+f greet(name: &s) -> s {
    f"Hello, {name}!"
}
```

Rust equivalents are shown in separate blocks when comparing:

```rust
// The Rust equivalent
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

> **Note** blocks provide additional context or caveats.

> **Warning** blocks highlight common pitfalls.
