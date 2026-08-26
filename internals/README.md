# MAGE Internals Guide

Compiler architecture, pipeline design, and implementation details for
contributors to the MAGE compiler and toolchain.

> **Read this first (2026-08-25).** All eight chapters were checked against
> `prototype/src` over 2026-08-25 and corrected. Two described systems that do
> not exist and are now labelled as design — **Chapter 1**'s query-driven
> multi-crate compiler and **Chapter 6**'s MLIR→LLVM backend. Chapters 2–5, 7
> and 8 were a mix: real machinery under wrong names, enums listed in part, and
> in three places a feature that parses and is then discarded.
>
> This index was the concentrated version of all of it. What follows is
> corrected; the pipeline diagram and crate map that used to be here described
> a compiler that was designed and not built, and are preserved at the bottom
> under [The original design](#the-original-design-not-implemented).

---

## Audience

This guide is for developers who want to:

- Contribute to the MAGE compiler
- Understand how MAGE source becomes machine code
- Extend the compiler with new passes or diagnostics
- Work on the RAP language server
- Add SKB rules or effect system features
- Build tools that interface with the compiler's query engine

## Prerequisites

- Familiarity with MAGE syntax (see [quick-start/](../quick-start/); there is no `book/` directory)
- Basic understanding of compiler concepts (parsing, type checking, IR)
- Rust programming experience (the compiler is written in Rust)

## Chapters

| # | Chapter | Topic |
| --- | --- | --- |
| 1 | [Architecture Overview](01-architecture.md) | The real pipeline: one crate, eager passes, `run_check` |
| 2 | [Lexer & Parser](02-lexer-parser.md) | Tokenization, Pratt expression parsing, and why the parser does not recover |
| 3 | [AST & HIR](03-ast-hir.md) | `ast::Module`, the full `ItemKind`/`Expr`/`Ty` enums, name resolution |
| 4 | [Type System](04-type-system.md) | `TypeChecker`, unification, generics — and the trait solving that does not exist |
| 5 | [Effects & Resolution](05-effects-resolution.md) | Effect inference, the flat effect set, user-defined effects |
| 6 | [MLIR Pipeline](06-mlir-pipeline.md) | `mlir::emit` produces text; the LLVM backend is design |
| 7 | [RAP Server](07-rap-server.md) | The 37-method agent protocol over JSON-RPC/TCP |
| 8 | [SKB & ACI](08-skb-aci.md) | 255 compiled safety rules, and the ACI engines |

## The pipeline

`run_check` in `prototype/src/main.rs`, phase numbering from its own comments:

```
 Source (.mg)
     │
     ▼  legacy::translate      phase 0, only under --syntax=legacy
     ▼  lexer::lex             phase 1   Error tokens in place; lexing continues
     ▼  parser::parse          phase 2   first ParseError aborts — the one hard stop
     ▼  elision::elide         phase 2.5 safety elision, on by default
     ▼  resolve::resolve       phase 3   ┐
     ▼  types::check           phase 4   │ each returns diagnostics
     ▼  effects::infer_effects phase 5   │ rather than halting, so one run
     ▼  verify::verify_module  phase 5.5 │ reports all of them
     ▼  abl_shape::check_…     phase 5.6 ┘
     ▼  heal                   phase 6   fix candidates for what the above found
```

There is no HIR lowering step, no LLVM, and no binary: `--check` ends here.
`mlir::emit` produces MLIR *text* that nothing consumes (Chapter 6), and the
path that reaches a GPU is Agentic Binary Language via `abl_bridge` /
`abl_compute` / `cuda_backend`, which no chapter covers — see `ARCHITECTURE.md`.

## Module map

One crate, `mage-prototype`, 62 public modules in `lib.rs`. The ones the
pipeline uses:

| Module | Role | Key types |
|---|---|---|
| `lexer` | Tokenization | `Token`, `TokenKind`, `Span` |
| `parser` | Parsing | `parse`, `ParseError` (`Parser` is private) |
| `ast` | Syntax tree | `Module`, `Item`, `ItemKind`, `Expr`, `Stmt`, `Type` |
| `resolve` | Name resolution | `resolve` |
| `types` | Inference + checking | `TypeChecker`, `check`, `Ty` |
| `effects` | Effect inference | `infer_effects`, `EffectInfer`, `Effect` |
| `verify` | Contract verification | `verify_module`, `VerifyStatus` |
| `abl_shape` | Typed-composition gate | `check_module_shapes` |
| `heal` | Fix generation | — |
| `hir` | Shared diagnostics | `Diagnostic`, `Severity`, `Ty` |
| `mlir` | MLIR text emission | `emit` |
| `rap` | Agent protocol server | `serve` |
| `skb` / `aci` | Safety rules, agentic intelligence | `Rule`, `RuleDatabase`, `DynamicWarningEngine` |

## Quick Links

- [MAGE_PROPOSAL.md](../MAGE_PROPOSAL.md) — Language design proposal
- [MAGE_SPEC.md](../MAGE_SPEC.md) — Formal language specification
- [MAGE_ECOSYSTEM.md](../MAGE_ECOSYSTEM.md) — Ecosystem architecture
- [Agent Guide](../agent-guide/README.md) — AI agent coding patterns
- [prototype/src/](../prototype/src/) — Working prototype implementation

---

## The original design (not implemented)

**Not implemented.** Design sketch — no `rdx_*` crate exists; the compiler is one crate, and nothing below is in `prototype/src`.

```
 Source (.mg) → Lexer → Parser → Resolve (DefIds) → HIR → Types → Effects
              → MAGE MLIR → LLVM MLIR → LLVM IR → Machine code → Binary
```

| Crate | Role | Key types |
| --- | --- | --- |
| `rdx_lexer` | Tokenization | `Token`, `TokenKind`, `Span` |
| `rdx_parser` | LL(1) parsing | `Parser`, `ParseResult` |
| `rdx_ast` | AST definitions | `Expr`, `Stmt`, `Item`, `Pattern` |
| `rdx_resolve` | Name resolution | `DefId`, `Resolver`, `Scope` |
| `rdx_hir` | HIR definitions | `HirExpr`, `HirStmt`, `HirItem` |
| `rdx_types` | Type inference/checking | `Ty`, `TyCtxt`, `InferCtxt` |
| `rdx_effects` | Effect system | `Effect`, `EffectSet`, `Capability` |
| `rdx_mlir` | MLIR codegen | `MlirModule`, `MlirOp`, `LoweringCtxt` |
| `rdx_skb` | Safety Knowledge Base | `Rule`, `RuleEngine`, `Violation` |
| `rdx_rap` | Language server | `RapServer`, `QueryEngine`, `Cache` |
| `rdx_driver` | CLI entry point | `CompileSession`, `Config` |
| `rdx_errors` | Diagnostics | `Diagnostic`, `DiagnosticGraph`, `Fix` |
| `rdx_span` | Source locations | `Span`, `SourceMap`, `FileId` |

Of those key types, the ones that do exist — `Token`, `TokenKind`, `Span`,
`Expr`, `Stmt`, `Item`, `Ty`, `Effect`, `Rule`, `Diagnostic` — live in modules
of the single crate, not in crates of their own.
