<div align="center">

# MechGen

**An agentic-first programming language built on Rust.**

[Specification](mechgen_SPEC.md) · [Ecosystem](mechgen_ECOSYSTEM.md) · [Proposal](mechgen_PROPOSAL.md) · [Examples](examples/) · [Contributing](CONTRIBUTING.md)

</div>

---

MechGen extends Rust's type system, performance model, and safety guarantees with
features designed for multi-agent AI development workflows. It compiles `.mg`
source files through a token-minimal, zero-ambiguity syntax to native code via
MLIR and LLVM, targeting CPU, GPU, NPU, WASM, and RISC-V.

```MechGen
use std::io;

#[derive(Debug, Clone)]
pub struct Task {
    name: String,
    priority: i32,
}

pub fn main() / io {
    let tasks = vec![
        Task { name: "parse".into(), priority: 1 },
        Task { name: "check".into(), priority: 2 },
        Task { name: "emit".into(), priority: 3 },
    ];

    for task in &tasks {
        println!("[{task.priority}] {task.name}");
    }
}
```

## Why MechGen?

- **Zero-Ambiguity Syntax** — Deterministic LL(1) grammar eliminates parsing failures for both humans and AI agents. No backtracking, no ambiguity.

- **Token Minimalism** — Canonical sigil-based forms (`+f` = pub fn, `v` = let, `?` = match, `@` = for) cut token counts ~50% vs. Rust, reducing cost and latency for agent-generated code.

- **Algebraic Effects** — A tracked effect system (`/ io`, `/ net`, `/ io + net`) makes side effects explicit in function signatures, enabling composition without monadic boilerplate.

- **Formal Contracts** — Built-in `@req`, `@ens`, and `@inv` annotations enable spec-first development. The compiler verifies contracts and uses them for synthesis.

- **Safety Knowledge Base** — 9,157 safety rules across ownership, borrowing, lifetimes, type safety, concurrency, and FFI — queryable at compile time via SKB-QL, removing surface-syntax noise (no lifetime annotations in source).

- **Cost Oracle** — Every construct exposes predicted cost (cycles, memory, latency, energy) per target architecture **before** code generation. Agents make informed optimization decisions.

- **Self-Healing Compiler** — Errors produce ranked repair candidates with confidence scores. The compiler proposes fixes, applies them, and re-checks automatically.

- **Swarm-Native** — First-class multi-agent coordination primitives: leases, consensus protocols, capability-based sandboxing, CRDT-based merging, and a message bus.

- **Hot Reload** — Function-level live patching with <1ms swap time. Rollback on regression, versioned function slots, zero-downtime iteration.

- **Hardware-Agnostic Compilation** — MLIR-native dialect with lowering passes for LLVM, SPIR-V, WASM, and RISC-V. Autotuning selects optimal strategies per target.

## Quick Start

```bash
# Create a new project
mg new my-project
cd my-project

# Build and run
mg run

# Transpile existing Rust code to MechGen
rust2mg src/main.rs --output src/main.mg

# Back-transpile to Rust
mg2rs src/main.mg --output rs/
```

## Syntax at a Glance

| MechGen   | Rust equivalent        | MechGen    | Rust equivalent       |
| ------- | ---------------------- | -------- | --------------------- |
| `f`     | `fn`                   | `v`      | `let`                 |
| `+f`    | `pub fn`               | `m`      | `let mut`             |
| `af`    | `async fn`             | `?`      | `match`               |
| `uf`    | `unsafe fn`            | `?:`     | `if`                  |
| `+S`    | `pub struct`           | `:?`     | `else if`             |
| `+E`    | `pub enum`             | `:`      | `else`                |
| `+T`    | `pub trait`            | `@`      | `for .. in`           |
| `I`     | `impl`                 | `@@`     | `loop`                |
| `u`     | `use`                  | `@w`     | `while`               |
| `.`     | `::` (path)            | `!`      | `break`               |
| `@d()`  | `#[derive()]`          | `>>`     | `continue`            |
| `p""`   | `println!()`           | `1b`     | `true`                |
| `[T]~`  | `Vec<T>`               | `0b`     | `false`               |
| `{K:V}` | `HashMap<K,V>`         | `?T`     | `Option<T>`           |
| `{K}`   | `HashSet<K>`           | `R[T,E]` | `Result<T,E>`         |
| `/ io`  | effect annotation      | `@req`   | precondition contract |
| `@ens`  | postcondition contract | `@inv`   | invariant contract    |

## Project Structure

```
compiler/           150+ mechgen_* crates — the full compiler
  mechgen_lexer/        Lexer and tokenizer
  mechgen_parser/       LL(1) parser
  mechgen_ast/          Abstract syntax tree
  mechgen_effects/      Algebraic effect system
  mechgen_contracts/    Contract checking (@req, @ens, @inv)
  mechgen_mlir/         MLIR dialect and lowering
  mechgen_codegen_*/    LLVM, GCC, Cranelift backends
  mechgen_swarm*/       Multi-agent coordination (7 crates)
  mechgen_aci_*/        Agent-Computer Interface (7 crates)
  mechgen_rap*/         MechGen Agent Protocol / IDE integration
  mechgen_skb*/         Safety Knowledge Base
  mechgen_cost_*/       Cost oracle and calibration
  mechgen_self_heal/    Auto-repair engine
  mechgen_hot_reload/   Live function patching
  mechgen_ffi/          Foreign function interface generation
  ...

library/            Standard library (core, alloc, std)
prototype/          Working compiler prototype (36 modules, 640+ tests)
examples/           12 example projects
skb/                Safety Knowledge Base (9,157 rules)
agent-guide/        AI agent integration guide
training/           Training data (100 samples, JSONL)
benchmarks/         100-task evaluation corpus
cookbook/            Practical recipes (I/O, HTTP, agents, CLI)
migration-guide/    Rust → MechGen migration guide
forge/              Package registry prototype
MechGen-vscode/       VS Code extension (syntax, effects, cost hints)
ci/                 CI/CD pipeline (lint → build → test → transpile → validate)
```

## Examples

Twelve self-contained projects in [`examples/`](examples/), each with a
`Forge.toml` manifest and `src/main.mg` entry point:

| Project                                                | Focus                            |
| ------------------------------------------------------ | -------------------------------- |
| [hello-world](examples/hello-world/)                   | Entry point, printing, variables |
| [data-structures](examples/data-structures/)           | Structs, enums, generics, traits |
| [http-client](examples/http-client/)                   | Async HTTP, effects, JSON        |
| [cli-tool](examples/cli-tool/)                         | File I/O, iterators, arguments   |
| [agent-swarm](examples/agent-swarm/)                   | Multi-agent coordination         |
| [effects-showcase](examples/effects-showcase/)         | Effect declarations and handlers |
| [autonomous-pipeline](examples/autonomous-pipeline/)   | Task decomposition and pipelines |
| [swarm-code-review](examples/swarm-code-review/)       | Scatter/gather consensus review  |
| [safe-plugin-host](examples/safe-plugin-host/)         | Capability sandbox with auditing |
| [live-compiler](examples/live-compiler/)               | Hot reload and self-healing      |
| [multilang-bindings](examples/multilang-bindings/)     | FFI bridge (C, Python, WASM)     |
| [cost-aware-optimizer](examples/cost-aware-optimizer/) | Cost-model strategy selection    |

```bash
cd examples/hello-world
mg run
```

## Documentation

| Document                                                   | Description                                        |
| ---------------------------------------------------------- | -------------------------------------------------- |
| [mechgen_SPEC.md](mechgen_SPEC.md)                             | Formal language specification                      |
| [mechgen_ECOSYSTEM.md](mechgen_ECOSYSTEM.md)                   | Ecosystem architecture (Forge, RAP, migration)     |
| [mechgen_PROPOSAL.md](mechgen_PROPOSAL.md)                     | Design philosophy and 24 design principles         |
| [prototype/docs/BOOK.md](prototype/docs/BOOK.md)           | User guide (12 chapters)                           |
| [prototype/docs/INTERNALS.md](prototype/docs/INTERNALS.md) | Compiler architecture (36 modules)                 |
| [agent-guide/](agent-guide/)                               | AI agent SDK (prompts, patterns, RAP methods)      |
| [cookbook/](cookbook/)                                     | Practical recipes (I/O, HTTP, agents, concurrency) |
| [migration-guide/](migration-guide/)                       | Rust → MechGen migration                             |
| [skb/](skb/)                                               | Safety Knowledge Base (9,157 rules, 6 categories)  |
| [training/](training/)                                     | Training data and evaluation (100 samples)         |
| [benchmarks/](benchmarks/)                                 | 100-task evaluation corpus with metrics            |

## Building from Source

See [INSTALL.md](INSTALL.md) for full instructions. Summary:

```bash
# Prerequisites: Python 3, C compiler, LLVM, cmake
git clone https://github.com/nervosys/MechGen.git
cd MechGen
cp bootstrap.example.toml bootstrap.toml  # edit as needed
./x build
./x test
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). For compiler internals, see
[prototype/docs/INTERNALS.md](prototype/docs/INTERNALS.md).

## License

MechGen is distributed under the terms of both the MIT license and the Apache
License (Version 2.0), with portions covered by various BSD-like licenses.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
