# Chapter 1: Architecture Overview

> **Rewritten 2026-08-25 against the code.** Every structural claim below was
> checked by reading `prototype/src`. The chapter this replaces described a
> query-driven, incremental, Salsa-backed compiler split across nine crates.
> None of that exists, and it had been presented in the present tense as
> documentation of the shipped compiler since the file was written. The design
> is preserved in [Appendix 1.A](#appendix-1a-the-original-design-not-implemented),
> labelled.

The MAGE compiler is **one crate**, `mage-prototype`, whose library surface is
62 modules under `prototype/src`. It compiles a `.mg` source file by running a
fixed sequence of passes, eagerly, front to back. There is no query engine, no
incremental recomputation, and no per-stage crate split.

That is a smaller machine than the original design, and worth stating plainly
rather than apologising for: the passes are ordinary functions, each takes the
whole module and returns diagnostics, and the order they run in is written out
literally in one function.

---

## 1.1 The compilation pipeline

`run_check` in `prototype/src/main.rs` *is* the pipeline. The phase numbering
below is the numbering in its own comments:

| Phase | Call | What it does |
|---|---|---|
| 0 | `legacy::translate` | Legacy-syntax translation. Only under `--syntax=legacy` |
| 1 | `lexer::lex` | Source → tokens. Emits `TokenKind::Error` in place rather than aborting |
| 2 | `parser::parse` | Tokens → `ast::Module`. A parse error **exits 1** here; nothing downstream runs |
| 2.5 | `elision::elide` | Safety elision, on by default in agentic mode |
| 3 | `resolve::resolve` | Name resolution |
| 4 | `types::check` | Type checking |
| 5 | `effects::infer_effects` | Effect inference |
| 5.5 | `verify::verify_module` | Contract verification (`spec` blocks) |
| 5.6 | `abl_shape::check_module_shapes` | Typed-composition gate — rejects a shape-mismatched `net` composition |
| 6 | `heal::…` | Self-healing: generates fix candidates for the diagnostics collected above |

Phases 3 through 5.6 each return a diagnostic list rather than halting, so one
run reports resolution, type, effect, contract and shape problems together.
Parse failure is the one hard stop, because every later pass takes an
`ast::Module`.

**Eager, not demand-driven.** Each phase is called once, on the whole module,
in this order. Nothing is memoised and nothing is skipped when a file has not
changed; there is no dependency graph between phases beyond the order of these
statements.

## 1.2 Module layout

`prototype/src/lib.rs` declares **62 public modules**. The pipeline above uses:

```rust
pub mod lexer;      // tokens
pub mod parser;     // ast::Module
pub mod ast;        // the syntax tree
pub mod elision;    // safety elision
pub mod resolve;    // name resolution
pub mod types;      // type checking
pub mod effects;    // effect inference
pub mod verify;     // contract verification
pub mod abl_shape;  // typed-composition gate
pub mod heal;       // fix generation
pub mod hir;        // Diagnostic, Severity — shared by every pass
```

The rest are the surrounding system rather than the front end: `eval` (the
tree-walking evaluator), `abl_bridge` / `abl_compute` (the Agentic Binary
Language pipeline), `mlir`, `rap` (the JSON-RPC server), `ontology`,
`skb`, `aci`, and the agent/swarm modules. `rmi` — the AI framework — is a
separate crate that `prototype` path-depends on.

## 1.3 Key data structures

**`Span`** — `prototype/src/lexer.rs`. Carried by tokens and diagnostics:

```rust
pub struct Span {
    pub offset: usize,
    pub len: usize,
    pub line: usize,
    pub col: usize,
}
```

A span is an offset and a length into *the* source string, plus a
precomputed line and column. There is **no `FileId` and no `SourceMap`**: the
compiler works on one source at a time, and the filename is passed alongside
as a `&str` for diagnostics.

**`ast::Module`** — the parser's output and every later pass's input:

```rust
pub struct Module {
    pub items: Vec<Item>,
}
```

**`hir::Diagnostic`** — what phases 3 through 5.6 return:

```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    /// Unique error code (e.g. "E0502"). None for ad-hoc diagnostics.
    pub id: Option<String>,
    // … semantic category, and the fields the healer reads
}

pub enum Severity {
    Error,
    Warning,
    Info,
}
```

`run_check` counts a diagnostic toward `total_errors` only when its severity is
`Error`, so warnings and info are reported and do not fail the run.

## 1.4 Entry points

The compiler is driven by a flag as the first argument, dispatched in `main`.
The ones that exercise the pipeline above:

| Flag | Path |
|---|---|
| `--check` | the full pipeline, human-readable summary |
| `--check --json` | the same, as JSON (`--json` is a modifier, not its own flag) |
| `--eval` | pipeline, then `eval` walks the module |
| `--fmt-compact` / `--fmt-expand` | parse, then print. There is no plain `--fmt` |
| `--target=abl` / `--target=abl-bytes` | lower to Agentic Binary Language |
| `--run=abl` / `--run=abl-bytes` | lower and dispatch to a compute backend |
| `--from=abl-bytes` | decode a container back to a MAGE view |
| `--rap` | the JSON-RPC server, loopback-only by default |

Modifiers, which are matched anywhere in the argument list rather than
dispatched on: `--json`, `--no-elision`, `--token-report`,
`--syntax=legacy`.

`mage-parse --help` is the authority; `MAGE_ONTOLOGY.json`'s `cli_flags`
section is checked against the binary by the test suite.

---

## Appendix 1.A: the original design (not implemented)

Everything below described the compiler this chapter was originally written
for. It is kept because the design intent is worth having, and labelled
because presenting it as description is what made this file misleading. None
of it is in `prototype/src`.

**Not implemented.** Design sketch — the crate split below does not exist; `prototype` is a single crate.

```
rdx_driver
├── rdx_rap          (language server)
├── rdx_mlir         (backend codegen)
│   └── rdx_effects  (effect system)
│       └── rdx_types (type checking)
│           └── rdx_hir (HIR)
│               └── rdx_resolve (name resolution)
│                   └── rdx_parser (parsing)
│                       └── rdx_lexer (tokenization)
├── rdx_skb          (safety knowledge base)
├── rdx_errors       (diagnostics)
└── rdx_span         (source locations)
```

The design principles that went with it — **query-driven** (every computation
a named, memoised query), **incremental** (changing one file re-runs only the
affected queries), **parallel** (independent queries on separate threads) —
describe a Salsa-style engine. `salsa` is not a dependency of any crate in this
repository, and nothing in `prototype/src` implements a query cache or a
dependency graph.

**Not implemented.** Design sketch — no `CompileSession`, no `DefId`, no query trait, no interner.

```rust
// The query group the design was built around.
#[salsa::query_group(TypeCheckStorage)]
pub trait TypeCheck: HirDatabase {
    fn infer_expr(&self, expr: HirExprId) -> TypeResult<Ty>;
    fn check_fn_body(&self, def_id: DefId) -> TypeResult<()>;
    fn resolve_type(&self, ty_ann: &TyAnnotation) -> TypeResult<Ty>;
}
```

If incrementality is ever wanted, the thing to change first is phase ordering
in `run_check`: the passes take whole modules and return whole diagnostic
lists, which is the property a query engine would have to break.
