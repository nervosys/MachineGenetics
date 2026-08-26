# Chapter 1: Architecture Overview

> **Status, measured 2026-08-25: this chapter describes an intended
> architecture, not the shipped one.** It is written in the present tense and
> `README.md` lists `internals/` as "Compiler-internals documentation", so it
> reads as description. Three of its opening claims were checked and none
> holds:
>
> | It says | Measured |
> |---|---|
> | "Each stage is a separate crate" | `prototype` is **one** crate — 64 files under a single `[package]` |
> | "The query engine (based on Salsa)…" | **`salsa` is not a dependency of any crate here**, and nothing in `prototype/src` implements a query cache |
> | "The `CompileSession` holds all configuration" | there is no `CompileSession` |
>
> Across `internals/`, **15 of 36 documented `pub` items did not exist** —
> `CompileSession`, `DefId`, `InferCtxt`, `TraitObligation`, `EffectChecker`
> and others are rustc/salsa-shaped names for a compiler that was designed and
> not built. Each was checked individually against `prototype/src`: **none is a
> rename.** The real compiler has `Effect`, `EffectAnalysis`, `Type` and
> `TypeAlias`; there is no `TypeError`, `InferCtxt` or `DefId` under any name.
>
> The **12 blocks** holding them now carry a `**Not implemented.**` label,
> following `MAGE_SPEC.md`'s precedent for the five constructs it documents and
> does not implement — *the design intent is worth keeping, the false
> impression is not*. `scripts/check-internals-doc.sh` skips a labelled block
> and holds the rest at **0**, so a newly invented item fails and a label
> removed from a block that still describes nothing fails too. What it reports
> is "17 documented items, 12 blocks skipped", which keeps the size of the gap
> visible rather than hiding it behind a green tick.
>
> The prose has no such mechanism. Neither do the ASCII diagrams below, which
> show a `rdx_driver` / `rdx_lexer` / `rdx_parser` crate graph that does not
> exist either.
>
> The design is worth keeping. Reading it as a map of the current code is not:
> the real pipeline is a single crate, eager rather than demand-driven. Start
> from `ARCHITECTURE.md` for what exists.

The MAGE compiler transforms `.mg` source files into optimized machine code
through a pipeline of well-defined stages. Each stage is a separate crate with
a clean query-based interface.

---

## 1.1 Design Principles

1. **Query-driven**: Every compiler computation is a named, memoized query.
   Nothing is computed eagerly — results are demanded by downstream stages
   and cached for reuse.

2. **Incremental**: Changing one file re-runs only the queries whose inputs
   changed. The query engine (based on Salsa) tracks dependencies automatically.

3. **Parallel**: Independent queries run on separate threads. The query engine
   manages the task graph and ensures deterministic results.

4. **Agent-friendly**: Every query result is serializable to JSON. The RAP
   protocol exposes the full query namespace to external tools and agents.

5. **Layered IR**: Source passes through four intermediate representations
   (Token → AST → HIR → MLIR → LLVM IR), each with a clear role.

## 1.2 Compilation Pipeline

```
┌──────────────────────────────────────────────────────────┐
│                    rdx_driver                            │
│  CompileSession orchestrates the full pipeline           │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Phase 1: Frontend                                  │  │
│  │                                                    │  │
│  │  .mg files                                        │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_lexer ──→ Token Stream                        │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_parser ──→ Unresolved AST                     │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_resolve ──→ Resolved AST (with DefIds)        │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_hir::lower ──→ HIR                            │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Phase 2: Analysis                                  │  │
│  │                                                    │  │
│  │  rdx_types ──→ Typed HIR                           │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_effects ──→ Effect-checked HIR                │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  rdx_skb ──→ Safety-validated HIR                  │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Phase 3: Backend                                   │  │
│  │                                                    │  │
│  │  rdx_mlir ──→ MAGE MLIR Dialect                   │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  MLIR Passes ──→ LLVM MLIR Dialect                 │  │
│  │      │                                             │  │
│  │      ▼                                             │  │
│  │  LLVM ──→ Object Files ──→ Linker ──→ Binary       │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## 1.3 Query Engine

The compiler is built on an incremental query engine inspired by Salsa
(used in rust-analyzer). Every piece of information flows through queries.

### Query Definition

**Not implemented.** Design sketch — no such item exists in `prototype/src`. See Chapter 1's status note for what the compiler actually is.

```rust
// In rdx_types/src/queries.rs
#[salsa::query_group(TypeCheckStorage)]
pub trait TypeCheck: HirDatabase {
    /// Infer the type of an expression.
    fn infer_expr(&self, expr: HirExprId) -> TypeResult<Ty>;

    /// Check that a function body matches its declared signature.
    fn check_fn_body(&self, def_id: DefId) -> TypeResult<()>;

    /// Resolve a type annotation to a concrete Ty.
    fn resolve_type(&self, ty_ann: &TyAnnotation) -> TypeResult<Ty>;
}
```

### Query Dependencies

```
parse_file(FileId)
    └→ resolve_names(FileId)
        └→ lower_to_hir(FileId)
            ├→ infer_expr(HirExprId)
            ├→ check_fn_body(DefId)
            └→ infer_effects(DefId)
                └→ check_capabilities(DefId)
                    └→ lower_to_mlir(DefId)
```

### Incrementality

When a file changes:

1. The driver invalidates `parse_file(file_id)`
2. The query engine propagates: which `resolve_names` results changed?
3. Only affected subtrees are recomputed
4. Unchanged queries return cached results instantly

This means editing a single function re-checks only that function and its
callers — not the entire crate.

## 1.4 Crate Dependency Graph

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

Leaf crates (`rdx_lexer`, `rdx_span`, `rdx_errors`) have no compiler
dependencies and can be used standalone.

## 1.5 Key Data Structures

### Span

Every token, AST node, and diagnostic carries a `Span` — a range within a
source file:

```rust
pub struct Span {
    pub file: FileId,
    pub start: ByteOffset,
    pub end: ByteOffset,
}
```

`FileId` is an interned identifier. The `SourceMap` (in `rdx_span`) maps
`FileId` to filenames and source text.

### DefId

Every named item gets a `DefId` — a unique identifier across the crate graph:

**Not implemented.** Design sketch — no such item exists in `prototype/src`. See Chapter 1's status note for what the compiler actually is.

```rust
pub struct DefId {
    pub crate_id: CrateId,
    pub local_id: LocalDefId,
}
```

Functions, types, traits, modules, constants — all are identified by `DefId`.
The resolver assigns these during name resolution.

### Interning

Strings (identifiers, paths) are interned for O(1) comparison:

**Not implemented.** Design sketch — no such item exists in `prototype/src`. See Chapter 1's status note for what the compiler actually is.

```rust
pub struct Symbol(u32);  // index into global string interner

impl Symbol {
    pub fn intern(s: &str) -> Symbol { ... }
    pub fn as_str(&self) -> &str { ... }
}
```

## 1.6 Error Handling

The compiler never panics on invalid input. Every stage produces structured
diagnostics:

```rust
pub struct Diagnostic {
    pub severity: Severity,     // Error, Warning, Note, Help
    pub message: String,
    pub span: Span,
    pub code: Option<DiagCode>, // E0001, W0042, etc.
    pub children: Vec<SubDiagnostic>,
    pub fixes: Vec<SuggestedFix>,
}
```

Diagnostics are collected in a `DiagnosticSink` and serialized as JSON for
agent consumers or rendered as terminal output for humans.

## 1.7 Session Configuration

The `CompileSession` holds all configuration for a compilation:

**Not implemented.** Design sketch — no such item exists in `prototype/src`. See Chapter 1's status note for what the compiler actually is.

```rust
pub struct CompileSession {
    pub config: Config,           // CLI flags, Forge.toml settings
    pub source_map: SourceMap,    // file → source text mapping
    pub diags: DiagnosticSink,    // accumulated diagnostics
    pub query_db: Database,       // Salsa database
    pub target: TargetSpec,       // compilation target
    pub edition: Edition,         // 2025, 2026, etc.
    pub capabilities: CapabilitySet, // granted capabilities
}
```

The driver creates one `CompileSession` per invocation and threads it through
all pipeline stages.
