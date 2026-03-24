# MechGen Internals Guide

Compiler architecture, pipeline design, and implementation details for
contributors to the MechGen compiler and toolchain.

---

## Audience

This guide is for developers who want to:

- Contribute to the MechGen compiler
- Understand how MechGen source becomes machine code
- Extend the compiler with new passes or diagnostics
- Work on the RAP language server
- Add SKB rules or effect system features
- Build tools that interface with the compiler's query engine

## Prerequisites

- Familiarity with MechGen syntax (see the [Book](../book/README.md))
- Basic understanding of compiler concepts (parsing, type checking, IR)
- Rust programming experience (the compiler is written in Rust)

## Chapters

| #   | Chapter                                          | Topic                                                  |
| --- | ------------------------------------------------ | ------------------------------------------------------ |
| 1   | [Architecture Overview](01-architecture.md)      | Compilation pipeline, crate graph, query engine        |
| 2   | [Lexer & Parser](02-lexer-parser.md)             | LL(1) tokenization, AST construction, error recovery   |
| 3   | [AST & HIR](03-ast-hir.md)                       | Abstract syntax tree, lowering to HIR, name resolution |
| 4   | [Type System](04-type-system.md)                 | Inference, sugar desugaring, trait solving, generics   |
| 5   | [Effects & Resolution](05-effects-resolution.md) | Effect inference, checking, capability validation      |
| 6   | [MLIR Pipeline](06-mlir-pipeline.md)             | MechGen dialect, lowering passes, LLVM codegen           |
| 7   | [RAP Server](07-rap-server.md)                   | Agent protocol, JSON-RPC, IDE integration, queries     |
| 8   | [SKB & ACI](08-skb-aci.md)                       | Safety Knowledge Base, Agentic Compiler Intelligence   |

## High-Level Pipeline

```
 Source (.mg)
     │
     ▼
 ┌────────┐
 │ Lexer  │  Tokenize: Unicode → Token stream
 └───┬────┘
     ▼
 ┌────────┐
 │ Parser │  Parse: Tokens → AST  (LL(1), zero ambiguity)
 └───┬────┘
     ▼
 ┌────────┐
 │Resolve │  Name resolution: AST → AST with DefIds
 └───┬────┘
     ▼
 ┌────────┐
 │   HIR  │  Lower: AST → HIR  (desugar syntax)
 └───┬────┘
     ▼
 ┌────────┐
 │ Types  │  Type inference + checking
 └───┬────┘
     ▼
 ┌────────┐
 │Effects │  Effect inference + capability validation
 └───┬────┘
     ▼
 ┌────────┐
 │  MLIR  │  Lower: HIR → MechGen MLIR → LLVM MLIR → LLVM IR
 └───┬────┘
     ▼
 ┌────────┐
 │ LLVM   │  Codegen: LLVM IR → Machine code
 └───┬────┘
     ▼
  Binary / Library
```

## Crate Map

The compiler is organized into the following crates, mirroring the pipeline:

| Crate         | Role                    | Key Types                              |
| ------------- | ----------------------- | -------------------------------------- |
| `rdx_lexer`   | Tokenization            | `Token`, `TokenKind`, `Span`           |
| `rdx_parser`  | LL(1) parsing           | `Parser`, `ParseResult`                |
| `rdx_ast`     | AST definitions         | `Expr`, `Stmt`, `Item`, `Pattern`      |
| `rdx_resolve` | Name resolution         | `DefId`, `Resolver`, `Scope`           |
| `rdx_hir`     | HIR definitions         | `HirExpr`, `HirStmt`, `HirItem`        |
| `rdx_types`   | Type inference/checking | `Ty`, `TyCtxt`, `InferCtxt`            |
| `rdx_effects` | Effect system           | `Effect`, `EffectSet`, `Capability`    |
| `rdx_mlir`    | MLIR codegen            | `MlirModule`, `MlirOp`, `LoweringCtxt` |
| `rdx_skb`     | Safety Knowledge Base   | `Rule`, `RuleEngine`, `Violation`      |
| `rdx_rap`     | Language server         | `RapServer`, `QueryEngine`, `Cache`    |
| `rdx_driver`  | CLI entry point         | `CompileSession`, `Config`             |
| `rdx_errors`  | Diagnostics             | `Diagnostic`, `DiagnosticGraph`, `Fix` |
| `rdx_span`    | Source locations        | `Span`, `SourceMap`, `FileId`          |

## Quick Links

- [MechGen_PROPOSAL.md](../MechGen_PROPOSAL.md) — Language design proposal
- [MECHGEN_SPEC.md](../MECHGEN_SPEC.md) — Formal language specification
- [MechGen_ECOSYSTEM.md](../MechGen_ECOSYSTEM.md) — Ecosystem architecture
- [Agent Guide](../agent-guide/README.md) — AI agent coding patterns
- [prototype/src/](../prototype/src/) — Working prototype implementation
