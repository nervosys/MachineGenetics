<div align="center">

# MachineGenetics (MechGen)

**Agentic-first Programming Language and Compiler Infra for Recursive Self-Improvement**

[Specification](MECHGEN_SPEC.md) · [Architecture](ARCHITECTURE.md) · [Benchmarks](#benchmarks-measured) · [Agent Protocol](AGENT_PROTOCOL.md) · [Roadmap](ROADMAP.md) · [Examples](examples/)

</div>

---

<div align="center">

### Agentic-SWE scorecard — MechGen vs traditional languages

*`agentic-eval`'s four agentic axes (0–1) + composite, across the profiled
languages. MechGen ranks **#1 of implemented languages** — only the unreachable
`ideal` design-target ceiling scores higher.*

| Language | token | determ | reliab | safety | **fitness** | SWE-wt¹ |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| **MechGen** | 0.80 | **0.97** | 0.94 | **0.95** | **0.915** | **0.930** |
| Rust | 0.55 | 0.90 | **0.95** | 0.80 | 0.800 | 0.845 |
| Go | 0.60 | 0.85 | 0.70 | 0.55 | 0.675 | 0.700 |
| Java | 0.40 | 0.75 | 0.70 | 0.60 | 0.612 | 0.650 |
| TypeScript | 0.65 | 0.55 | 0.70 | 0.40 | 0.575 | 0.588 |
| Python | **0.85** | 0.45 | 0.45 | 0.35 | 0.525 | 0.490 |
| *`ideal` (ceiling)* | 0.85 | 0.97 | 0.95 | 0.96 | *0.932* | *0.943* |

<sub>¹ SWE-weighted = reliability·.35 + determinism·.30 + safety·.20 + token·.15. The four axes are `agentic-eval`'s **curated, bias-audited** judgments (`--example swe_languages`).</sub>

**Measured anchor** (compile+run, real BPE — *not* curated): MechGen **executes
5/5** of the cross-language task suite and is the **tersest** runnable language
(**173** cl100k tokens vs Rust 275, Java 297); `eval_bench` computes **73/73**
programs to exact results. Full measured tables in [Benchmarks](#benchmarks-measured).

</div>

MechGen is an **agentic-first** language: the *same* logic spans human-readable
prose, agent-dense sigils, a declarative neural-net DSL, and the byte-level
binary IR they lower to — each a view of the one artifact, each measured. The
prototype lexes, type-checks, and
**executes** general `.mg` programs, and lowers neural networks to a compact
binary IR (Agentic Binary Language) run on a CPU/CUDA backend.

### One language, four forms

**1 · Human-first** — a typed surface that reads like a modern language *(verified `--check`; 61 cl100k tokens):*

```rust
pub fn sum_even_squares(xs: [i32]~) -> i32 {
    var total: i32 = 0;
    for x in xs {
        if x % 2 == 0 {
            total = total + x * x;
        }
    }
    total
}
```

**2 · Agentic-first** — the *same program* in sigils (`+f` = `pub fn`) + the standard vocabulary (`map`/`filter`/`fold`, each a single BPE token). **−25 % real tokens, identical result** *(verified: `--check` + executes → `56`; 46 cl100k tokens):*

```rust
+f sum_even_squares(xs) = fold(map(filter(xs, fn(x) => x % 2 == 0), fn(x) => x * x), 0, fn(a, b) => a + b)
```

**3 · High-level declarative form** — neural networks declared, not hand-wired (still source — it lowers to the binary IR in form 4):

```rust
net MLP {
    layer fc1: Linear(8, 16);
    layer act1: ReLU;
    layer fc2: Linear(16, 4);
    layer act2: Sigmoid;
    forward { fc1 }
}
```

**4 · Binary IR** — what an agent actually *ships*: the net above lowered to the
Agentic Binary Language container. **92 bytes — 71.9 % smaller than its text — and it round-trips back to source** *(measured):*

```
ABL1 02 00 01 00 …  4d 4c 50 3f …      ← "ABL1" magic + the MLP module
327 B  .mg text   →   92 B  binary   →   decompiles to the exact net above
```

> An agent writes intent in form 2 (fewest tokens), the compiler verifies it
> against form 1's types, and ships form 4 (fewest bytes) — none of which a
> human-first language gives you in one artifact. Every figure above is
> reproduced by the commands in [Benchmarks](#benchmarks-measured).

### Composing architectures — a small algebra over a shared block library

The `net` DSL is more than a flat layer list. A handful of orthogonal **composition
operators** combine **reusable blocks**, so an agent expresses a deep architecture
in a few tokens instead of hand-wiring every layer:

```rust
// A reusable block — published once to a shared, content-addressed registry
// (forge publish), then referenced by name; its body lives off-context.
block TransformerBlock(d, h, ff) {
    wrap LayerNorm {                                         // pre/post sandwich
        residual { layer attn: MultiHeadAttention(d, h); }  // x + f(x)
        residual { layer ff1: Linear(d, ff); layer act: GELU; layer ff2: Linear(ff, d); }
    }
}

net GPT {
    layer embed: Embedding(50000, 256);
    stack 12 { TransformerBlock(256, 8, 1024) }             // repeat — O(1) in depth
    forward { embed }
}
```

- **Operators** — `stack N { … }` (repeat), `residual { … }` (`x + f(x)`),
  `branch { … } { … }` (parallel paths), `wrap Op { … }` (`Op >> body >> Op`) —
  lower to the RMIL primitives `REPEAT`/`RES_ADD`/`PAR` and **execute** on the CPU
  backend.
- **Registry handles** — `forge publish` stores a block under the SHA-256 of its
  source (deduplicated); any project references it **by name** while `forge
  check`/`build` resolve the definition off-context.
- **Typed-composition gate** — a shape-mismatched composition (e.g. a `residual`
  whose body changes dimension) is **rejected at `--check`** with an actionable
  diagnostic, before any compute runs.
- **O(1) artifact** — `stack 12` ships as one block + a count (a `REPEAT` fold), so
  the binary is flat in depth.

One reproducible command threads the whole story —
[`benchmarks/capstone/run.sh`](benchmarks/capstone/run.sh): `forge publish` →
~41-token GPT → `forge check` (resolve + gate) → `forge build` (REPEAT-folded
binary, ~1.1× for 12 blocks) → the full GPT **runs** (`dispatched=97,
unsupported=[]`).

## Benchmarks (measured)

Every number below is produced by **actually compiling and running** code and
comparing real output — or by counting **real cl100k BPE tokens** of the exact
executed files — not by curated judgment. Reproduce with
[`benchmarks/cross_lang/run.sh`](benchmarks/cross_lang/run.sh) and the
`agentic-eval` `tokens_of` / `swe_executability` examples.

**Cross-language executability + terseness** — the same 5 tasks
(`fact·sumto·fib·distinct·collatz`, known integer outputs) written idiomatically
in each language, compiled+run on the host toolchain, stdout compared to the
expected value (measured 2026-06-11):

| Language | Executes (5 tasks) | Real cl100k tokens | Source bytes |
|---|:--:|:--:|:--:|
| **MechGen** | **5 / 5** | **173** | **401** |
| JavaScript | 5 / 5 | 199 | 513 |
| TypeScript | 5 / 5 | 220 | 593 |
| Go | 5 / 5 | 271 | 727 |
| Rust | 5 / 5 | 275 | 769 |
| Java | 5 / 5 | 297 | 1033 |
| Python | *runtime absent on host — excluded, not estimated* | — | — |

- **Executability** is a **gate** the agentic edit→build→test→debug loop depends
  on (`test` must run the program and check output). Every runnable language
  clears it — including MechGen, via its tree-walking evaluator (`MechGen-parse
  --eval`). This records a threshold *crossed*, not a lead on a graded axis.
- On this identical task set MechGen is the **tersest** by real tokens (173,
  1.00×) and by bytes (401). A second real-BPE set (`swe_token_benchmark`, 6
  languages × 3 different tasks) agrees: MechGen 85 cl100k vs Python 89, Go 93,
  Java 98, TS 102, Rust 113.
- **MechGen surface coverage:** its `eval_bench` correctness harness asserts
  **73 / 73** general-purpose programs each compute an *exact* result, exercising
  every reachable expression/statement form, all pattern kinds
  (tuple/slice/struct/option), and the standard vocabulary over lists/strings/maps.
  Reproduce: `cargo test --release eval_bench -- --ignored` (in `prototype/`).

**Agentic-first toolchain — measured improvement.** The same lens applied to the
**Forge** project toolchain (`forge`): a human-text baseline vs. the agentic-first
surface (self-describing `manifest`, `--json`, effect classes). Every figure
measured (`forge` runs + real BPE + `node JSON.parse` + 5× sha256; reproduce:
`agentic-eval --example swe_forge_agentic`):

| Axis (same toolchain, 8 commands) | text-only baseline | agentic-first |
|---|:--:|:--:|
| Result machine-parseable (reliability) | 0.00 | **1.00** (8/8 structured JSON) |
| Effect-gated before exec (safety) | 0.00 | **1.00** (8/8 effect-classed) |
| Output reproducible (determinism) | — | **1.00** (5/5 byte-identical) |
| Discovery cost (real cl100k tokens) | 547 (prose) | **232** (`forge manifest`) |

Self-describing + machine-readable lifts reliability and safety 0 → 1.00, keeps
output deterministic, and makes discovery **2.36× cheaper in real tokens *and*
parseable**. The one measured cost is +3 tokens (12%) per structured result —
reported, not hidden.

**Neural architecture DSL vs. PyTorch.** The same architecture declared in
MechGen's `net` DSL vs. an equivalent PyTorch `nn.Module` (MechGen declares the
layers; PyTorch must also spell out the imperative `forward`). Token counts real
cl100k BPE; binary sizes measured live (reproduce:
[`benchmarks/constructs/run.sh`](benchmarks/constructs/run.sh)):

| Architecture | MechGen | PyTorch | fewer tokens | MechGen text → binary IR |
|---|:--:|:--:|:--:|:--:|
| MLP | 50 | 78 | **36 %** | 139 B → 92 B (−34 %) |
| Transformer | 73 | 142 | **49 %** | 235 B → 137 B (−42 %) |

The saving grows with complexity (the more forward-wiring the DSL subsumes, the
bigger the win), and the declaration then lowers to a binary IR a further ~34–42 %
under its own text. The full pipeline — registry block → `--check` shape-gate →
`REPEAT`-folded binary → execution — runs in
[`benchmarks/capstone/run.sh`](benchmarks/capstone/run.sh) (a 12-deep GPT in ~41
tokens, binary 1.09× for 12 blocks vs. 1, `dispatched=97 unsupported=[]`).

The **full agentic-SWE scorecard** (`agentic-eval`'s four 0–1 axes + composite
across all profiled languages) is the table at the [top of this
README](#machinegenetics-mechgen); falsifiable guards hold there: token (0.80) ≤
Python (0.85), reliability (0.94) ≤ Rust (0.95), no axis ≥ 0.98 (it is a
prototype). Reproduce with `agentic-eval --example swe_languages`.

> **Honesty.** Two kinds of number appear in this README, kept distinct.
> **Measured** (compile+run, real BPE, sha256, JSON-parse): the executability/
> terseness, eval-bench, and agentic-toolchain tables. **Curated** (`agentic-eval`'s
> 0–1 language axes): the four-axis scorecard at the top — encoded judgments,
> bias-audited (scores were corrected *down* on evidence; this is the project's
> own language). Executability is a gate, not a parity claim — the runtime is a
> young tree-walker (no JIT; `await` is run-to-completion) on curated tasks, not
> an application corpus.

## Why MechGen?

> **Honest framing:** MechGen's value for agents lives in two places:
> (1) a structurally reliable, executable text surface — LL(1) grammar, tracked
> effects, machine-readable diagnostics, a self-describing ontology, and an
> evaluator that runs it — and (2) the **Agentic Binary Language binary IR**,
> where a full neural-network module fits in **~300 bytes**
> (~83 % smaller than the equivalent text).
>
> The text surface itself is roughly **byte-tied** with idiomatic Rust
> on the 100-task benchmark corpus — not the "~50 % reduction" earlier
> versions of this README claimed. See
> [`benchmarks/FINDINGS.md`](benchmarks/FINDINGS.md) for measurement
> and [`AGENT_PROTOCOL.md`](AGENT_PROTOCOL.md) for how agents should
> target the IR directly.
>
> The capabilities below are marked **✅ working in the prototype today** or
> **🎯 design goal** (specified/partially built, not yet in the prototype). See
> [ROADMAP.md](ROADMAP.md) for status.

- ✅ **Executes End to End** — A tree-walking evaluator (`MechGen-parse --eval`) runs general-purpose programs across the full surface — every expression/statement form, all pattern kinds, and the standard vocabulary over lists/strings/maps. The `eval_bench` suite computes **73 / 73** programs to exact results.

- ✅ **Zero-Ambiguity Syntax** — Deterministic LL(1) grammar eliminates parsing failures for both humans and AI agents. No backtracking, no ambiguity.

- ✅ **Binary IR for Agents (Agentic Binary Language)** — A transformer block encodes to **47 bytes** of Agentic Binary Language, a 5-item module to ~300 bytes (vs ~1.8 KB of text). Agents target the IR directly via `--target=abl-bytes`; the text surface is a human-readable view via the round-trip decompiler.

- ✅ **Neural architecture DSL + composition algebra** — declarative `net`s composed from a few orthogonal operators (`stack`/`residual`/`branch`/`wrap`) over reusable `block`s, shared across projects via a content-addressed registry (`forge publish` + name handles). Shape-mismatched compositions are rejected at `--check`; repeated depth folds to an `O(1)` binary (`REPEAT`); and the operators **execute** on the CPU backend. See [Composing architectures](#composing-architectures--a-small-algebra-over-a-shared-block-library).

- ✅ **Sigil-Based Text Surface** — Canonical forms (`+f` = pub fn, `v`/`val` = immutable binding, `m`/`var` = mutable binding, `?` = match, `@` = for) keep the human view compact. (`let` is *not* a keyword — bindings are always `val`/`var`; the compiler rejects a stray `let` with a fix hint.) On the benchmark corpus the text is ~tied with idiomatic Rust on raw bytes (declaration-heavy code wins 4–14 %, expression-heavy code loses 8–15 %). The structural reliability matters more than the byte delta.

- ✅ **Algebraic Effects** — A tracked effect system (`/ io`, `/ net`, `/ io + net`) makes side effects explicit in function signatures, enabling composition without monadic boilerplate.

- 🎯 **Formal Contracts** — Built-in `@req`, `@ens`, and `@inv` annotations enable spec-first development. The compiler verifies contracts and uses them for synthesis.

- 🎯 **Safety Knowledge Base** — 9,157 safety rules across ownership, borrowing, lifetimes, type safety, concurrency, and FFI — queryable at compile time via SKB-QL, removing surface-syntax noise (no lifetime annotations in source).

- 🎯 **Cost Oracle** — Every construct exposes predicted cost (cycles, memory, latency, energy) per target architecture **before** code generation. Agents make informed optimization decisions.

- 🎯 **Self-Healing Compiler** — Errors produce ranked repair candidates with confidence scores. The compiler proposes fixes, applies them, and re-checks automatically.

- 🎯 **Swarm-Native** — First-class multi-agent coordination primitives: leases, consensus protocols, capability-based sandboxing, CRDT-based merging, and a message bus.

- 🎯 **Hot Reload** — Function-level live patching with <1ms swap time. Rollback on regression, versioned function slots, zero-downtime iteration.

- 🎯 **Hardware-Agnostic Compilation** — MLIR-native dialect with lowering passes for LLVM, SPIR-V, WASM, and RISC-V. Autotuning selects optimal strategies per target.

- ✅ **Built-in AI Framework (RecursiveMachineIntelligence)** — The [`RecursiveMachineIntelligence/`](RecursiveMachineIntelligence/) `rmi` crate ships inside the project: Agentic Binary Language binary neurosymbolic IR, compute backends (CPU + CUDA via IronAccelerator — tensor-core F16/BF16, calibrated INT8/INT4 quantization), a self-describing ontology with a token-compact `manifest()`/`describe()` front door, machine-parseable error diagnostics, and effect-mapped safety. The compiler's `--target=abl-*` modes lower straight onto it.

- ✅ **Complete Ontologies, End to End** — Every layer self-describes for agents: the language/compiler (`MechGen-parse --emit-ontology`, `--manifest`), the framework (`rmi::core::manifest`, `FrameworkOntology`), and the CLI (effect-classed mode index). Deterministic output everywhere — agents can cache, diff, and gate without prose docs.

## Quick Start

MechGen is a **prototype**. The working entry point today is the
`MechGen-parse` binary; build it from `prototype/` and run a program:

```bash
# Build the prototype compiler/evaluator
cargo build --release --manifest-path prototype/Cargo.toml

# Write and run a program
echo 'f main(){ sum(map(range(5), fn(x) => x * x)) }' > squares.mg
./prototype/target/release/MechGen-parse --eval squares.mg main   # → 30

# Parse + typecheck + report on any .mg file
./prototype/target/release/MechGen-parse squares.mg
```

Or use the **Forge** project toolchain (`forge/`) for a manifest-driven project:

```bash
cargo build --release --manifest-path forge/Cargo.toml   # builds the `forge` binary

forge new my-project        # scaffold Forge.toml + src/main.mg
cd my-project
forge check                 # parse + typecheck the entry point (+ shape gate)
forge build                 # check, then lower through the binary IR
forge run                   # execute `main` → 120
forge publish block.mg      # publish a block to the shared registry (SHA-256)
forge block                 # list referenceable blocks (local + registry)
forge manifest              # token-compact, effect-classed command index (--json)
```

`forge` drives the same `MechGen-parse` compiler (auto-located, or set
`FORGE_MG`). It is agentic-first — `forge manifest`/`forge describe <cmd>`
self-describe the toolchain and every command takes `--json`. The lower-level
targets below are the compiler's own interface.

> **Still planned.** A `mg` short alias for `forge` and the Rust transpilers are
> on the [roadmap](ROADMAP.md) and not yet built.

### Working prototype CLI (`MechGen-parse`)

The prototype compiler in `prototype/` ships a single binary with the
following targets. Most of them came from the MechGen ↔ RMI unification
work; see [`UNIFICATION.md`](UNIFICATION.md) for the full phase log.

```sh
MechGen-parse <file.mg>                  # parse + check + report
MechGen-parse --target=abl <file>       # lowering summary (sizes, hashes)
MechGen-parse --target=abl-bytes <file> [out.abl]
                                         # emit binary Agentic Binary Language container
MechGen-parse --from=abl-bytes <file.abl>
                                         # decode bytes → human-readable .mg
MechGen-parse --target=abl-compute <file>   # dispatch nets to CpuBackend
MechGen-parse --target=abl-train    <file>   # SGD/Adam training loop
MechGen-parse --target=abl-infer    <file>   # load checkpoint, predict
MechGen-parse --target=abl-generate <file>   # autoregressive decode
MechGen-parse --eval <file.mg> <fn> [int args]
                                         # execute a function in the tree-walking
                                         # evaluator and print its result
```

The `--eval` evaluator runs general-purpose `.mg` programs end to end
(lex → parse → evaluate). Its correctness harness (`eval_bench`) executes
**73 programs to exact results**, covering every expression/statement form, all
pattern kinds (tuple/slice/struct/option), and the standard vocabulary over
lists, strings, and maps — see [Benchmarks](#benchmarks-measured).

```sh
# e.g. given `f fib(n){ if n < 2 { n } else { fib(n-1) + fib(n-2) } }` in fib.mg
MechGen-parse --eval fib.mg fib 25       # → 75025
```

```sh
# Run the token-efficiency benchmark across all 100 corpus tasks
cargo run --bin token-bench --manifest-path prototype/Cargo.toml
# → writes benchmarks/TOKEN_REPORT.md, exits non-zero on regressions
```

## Syntax at a Glance

| MechGen | Rust equivalent        | MechGen  | Rust equivalent       |
| ------- | ---------------------- | -------- | --------------------- |
| `f`     | `fn`                   | `v`/`val` | `let` (immutable)    |
| `+f`    | `pub fn`               | `m`/`var` | `let mut` (mutable)  |
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
prototype/          The working compiler + evaluator (this is MechGen today):
                    lexer, LL(1) parser, type inference, tree-walking evaluator,
                    Agentic Binary Language lowering, and the RAP agent server
                    — 1,184 tests
RecursiveMachineIntelligence/   Built-in agentic-first AI framework (`rmi` crate):
                    Agentic Binary Language binary IR, compute backends
                    (CPU + CUDA via IronAccelerator, F32→F16/BF16→INT8/4),
                    self-describing ontology + token-compact manifest
framework/          Framewerx — neurosymbolic layer over `rmi`
forge/              Project toolchain + content-addressed block registry
                    (`forge new/check/build/run/publish/block`, `Forge.toml`)
stdlib/             Standard library (`.mg` source)
skb/                Safety Knowledge Base (9,157 rules, 6 categories)
benchmarks/         Evaluation corpus + cross-language executability harness
examples/           Self-contained example projects (`Forge.toml` + `src/main.mg`)
editors/            Editor support: tree-sitter grammar, Helix, Neovim
agent-guide/        AI-agent integration guide (prompts, RAP methods)
cookbook/           Practical recipes (I/O, HTTP, agents, CLI)
quick-start/        Install → hello-world → syntax → build/run/test tutorials
internals/          Compiler-internals documentation
migration-guide/    Rust → MechGen migration guide
community/          Contributing, governance, issue templates
training/           Training data (100 samples, JSONL)
```

> An earlier branch carried a forked-rustc native compiler (`compiler/`, MLIR +
> LLVM backends). It was a separate, dormant experiment and has been removed;
> code generation today runs through the Agentic Binary Language IR onto the
> `rmi` backends. Native text-language codegen is a roadmap item, not a current
> claim.

## Examples

Twelve self-contained projects in [`examples/`](examples/), each with a
`Forge.toml` manifest and `src/main.mg` entry point:

| Project                                                | Focus                            |
| ------------------------------------------------------ | -------------------------------- |
| ✅ [hello-world](examples/hello-world/)                | Bindings, f-strings, vocabulary — `forge run`-able |
| ✅ [data-structures](examples/data-structures/)        | Structs, pattern matching, map/sum/min — `forge run`-able |
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

> The ✅ examples **`forge check` and `forge run` today** (`cd
> examples/hello-world && forge run`). The rest exercise the full *intended*
> surface — async HTTP, FFI, swarm coordination, hot reload — which the prototype
> evaluator does not execute yet; they are scaffolds for the
> [roadmap](ROADMAP.md), not runnable demos. `forge new` also scaffolds a project
> that checks and runs out of the box. More `.mg` programs that run today live in
> [`prototype/examples/`](prototype/examples/) (e.g. `agent_rpn.mg`, the `net`
> examples).

## Documentation

| Document                                                   | Description                                        |
| ---------------------------------------------------------- | -------------------------------------------------- |
| [MECHGEN_SPEC.md](MECHGEN_SPEC.md)                         | Formal language specification                      |
| [ARCHITECTURE.md](ARCHITECTURE.md)                         | Compiler and system architecture                   |
| [AB_INITIO_DESIGN.md](AB_INITIO_DESIGN.md)                 | Ab-initio language design (standard vocabulary §8) |
| [AGENT_PROTOCOL.md](AGENT_PROTOCOL.md)                     | How agents target the binary IR directly           |
| [MEASUREMENTS.md](MEASUREMENTS.md)                         | Measured results and methodology                   |
| [ROADMAP.md](ROADMAP.md)                                   | Roadmap and status                                 |
| [MECHGEN_ECOSYSTEM.md](MECHGEN_ECOSYSTEM.md)               | Ecosystem architecture (Forge, RAP, migration)     |
| [MECHGEN_PROPOSAL.md](MECHGEN_PROPOSAL.md)                 | Design philosophy and design principles            |
| [prototype/docs/BOOK.md](prototype/docs/BOOK.md)           | User guide (12 chapters)                           |
| [prototype/docs/INTERNALS.md](prototype/docs/INTERNALS.md) | Compiler architecture (36 modules)                 |
| [agent-guide/](agent-guide/)                               | AI agent SDK (prompts, patterns, RAP methods)      |
| [cookbook/](cookbook/)                                     | Practical recipes (I/O, HTTP, agents, concurrency) |
| [migration-guide/](migration-guide/)                       | Rust → MechGen migration                           |
| [skb/](skb/)                                               | Safety Knowledge Base (9,157 rules, 6 categories)  |
| [training/](training/)                                     | Training data and evaluation (100 samples)         |
| [benchmarks/](benchmarks/)                                 | 100-task evaluation corpus with metrics            |

## Building from Source

The prototype builds with a stable Rust toolchain — no LLVM or external build
system required for the core compiler/evaluator:

```bash
# Prerequisites: Rust (stable, edition 2024)
git clone https://github.com/nervosys/MachineGenetics.git
cd MachineGenetics

# Build and test the prototype
cargo build --release --manifest-path prototype/Cargo.toml
cargo test  --release --manifest-path prototype/Cargo.toml
```

(The optional CUDA backend is feature-gated and loads the driver at runtime via
`dlopen`; the build succeeds with or without a GPU present.)

## Contributing

Issues and pull requests are welcome — see
[community/CONTRIBUTING.md](community/CONTRIBUTING.md) and
[community/GOVERNANCE.md](community/GOVERNANCE.md). For compiler internals and
module layout, see [prototype/docs/INTERNALS.md](prototype/docs/INTERNALS.md);
for how agents consume the language, see [AGENT_PROTOCOL.md](AGENT_PROTOCOL.md).

## License

MechGen is licensed under the **Apache License, Version 2.0**. See
[LICENSE](LICENSE) for the full text.
