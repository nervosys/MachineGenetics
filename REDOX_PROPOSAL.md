# Redox: An Agentic-First Programming Language for the 21st Century

## Transforming Rust into a Language for Humans and AI Agents Alike

**Version:** 0.1.0-draft  
**Date:** 2026-03-15  
**Status:** Proposal  

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design Principles](#2-design-principles)
3. [Transformation Methodology](#3-transformation-methodology)
4. [Ontology of the Redox System](#4-ontology-of-the-redox-system)
5. [Language-Level Changes](#5-language-level-changes)
6. [Compiler Architecture for Agents](#6-compiler-architecture-for-agents)
7. [Swarm Collaboration Model](#7-swarm-collaboration-model)
8. [Toolchain as Swarm Infrastructure](#8-toolchain-as-swarm-infrastructure)
9. [Safety Model: Database-Driven, Not Compiler-Enforced](#9-safety-model-database-driven-not-compiler-enforced)
10. [Agent Discoverability Protocol](#10-agent-discoverability-protocol)
11. [Phased Implementation Plan](#11-phased-implementation-plan)
12. [Appendix: Full Ontology Tables](#12-appendix-full-ontology-tables)

---

## 1. Executive Summary

Rust provides the strongest compile-time safety guarantees of any systems language: ownership, borrowing, lifetime enforcement, data-race freedom, and exhaustiveness checking. However, its tooling and language interfaces were designed for *human developers* operating through CLI tools, text editors, and manual reasoning. Its syntax is context-sensitive and ambiguous in ways that cause agent parsing failures. Its compile-time safety machinery is redundant for AI agents that can internalize safety rules from a knowledge base. And its performance model is tightly coupled to specific hardware targets.

**Redox** reimagines Rust as an **agentic-first** language — one where AI agents are first-class participants in the development lifecycle. The language is redesigned around twelve pillars: **(1) zero-ambiguity syntax** that eliminates agent parsing errors, **(2) communication-first primitives** that maximize inter-agent bandwidth, **(3) hardware-agnostic high performance** built on **MLIR and LLVM** compiler infrastructure that compiles to any target without sacrificing speed, **(4) token-minimal syntax** that minimizes the tokens agents must emit, because every token costs time, money, and memory, **(5) safety-free syntax simplification** that eliminates lifetimes, borrow annotations, ownership markers, and all other compile-time safety syntax — since agents consult the SKB directly, the syntax need not carry safety information at all, **(6) an agentic compiler** — the compiler itself is an AI-powered system that provides dynamic warnings, intelligent debugging, performance suggestions, and learns from the codebase and the swarm's history, **(7) cost model transparency** — every construct has a queryable cost (cycles, memory, energy, tokens) per target, so agents choose before emitting rather than profiling after, **(8) synthesis-first design** — formal specifications (`@req`/`@ens`/`@perf`/`@fx`) enable spec-to-code synthesis with compiler verification, closing the guess-compile-fix cycle, **(9) persistent agent memory** — a four-tier memory model (ephemeral, session, project, global) that lets agents learn across sessions and share knowledge across the ecosystem, **(10) self-healing compilation** — the compiler auto-repairs errors with ranked fix candidates, collapsing the emit→error→fix→re-emit loop into a single step, **(11) live iteration** — function-level hot-reload patches running processes in sub-millisecond time without restart, and **(12) zero-friction interop** — automatic FFI binding generation from C/C++ headers, Python stubs, WASM interfaces, and GPU kernels with capability-based sandbox security. Safety knowledge moves from compile-time enforcement to a **queryable Safety Knowledge Base (SKB)** — a structured database of rules, invariants, and constraints that agents reference directly, eliminating the compile-time overhead that slows iteration.

By building on MLIR (Multi-Level Intermediate Representation) and LLVM, Redox inherits the broadest hardware backend ecosystem in existence — 20+ CPU architectures, GPU compute (AMDGPU, NVPTX), WASM, SPIR-V — while gaining MLIR's extensible dialect system for defining custom optimization passes for agent-specific workloads, ML accelerators (NPU/TPU), FPGA synthesis, and domain-specific hardware. MLIR's multi-level abstraction preserves high-level semantic information (parallelism intent, memory layout preferences, effect annotations) deep into the optimization pipeline, where LLVM alone would have discarded it.

Critically, following the architectural insight pioneered by Modular AI's Mojo language, the **Redox MLIR Dialect is not a translation target — it is the language's semantic backbone**. Ownership, effects, contracts, and performance annotations are first-class MLIR operations and attributes, not metadata bolted onto a generic IR. This means the compiler's semantic understanding is preserved through the entire optimization pipeline, enabling **MLIR-native autotuning** (the compiler generates multiple lowering variants per kernel and benchmarks them per-target), **language-level SIMD types** backed directly by MLIR's vector dialect, and **automatic device placement** where the MLIR pipeline — not the programmer — decides whether a kernel runs on CPU, GPU, or NPU based on cost modeling. These are not Mojo features copied for humans; they are Mojo's architectural insights **re-derived for agent swarms**, where autotuning is a swarm-parallelizable search, device placement is an agent-queryable decision, and SIMD types are token-efficient first-class citizens.

### Core Thesis

> A programming language designed for agent swarms must have **zero syntactic ambiguity**, **maximum inter-agent communication throughput**, **hardware-agnostic performance**, and **minimum token footprint**. The compiler is an **optimizing translator**, not a safety gatekeeper — safety rules live in a queryable knowledge base that agents consult directly, not in slow compile-time passes that duplicate what agents already know. The fundamental bottleneck in agentic software engineering is not correctness (agents can be trained on rules) but **parsing speed**, **communication bandwidth**, **target-portable performance**, and **token efficiency** — because every token an agent emits costs time, money, and memory.

> The fundamental unit of agentic work is not the single agent but the **swarm** — a coordinated ensemble of specialized agents that decompose, parallelize, verify, and integrate changes across a codebase simultaneously. The language, compiler, and toolchain must be designed for this concurrent, collaborative reality from the ground up. Every syntax decision, every compiler pass, every protocol message must be evaluated against: *does this minimize parse errors? does this maximize communication? does this run fast everywhere? does this minimize the tokens an agent must spend?*

### What Changes

| Dimension                 | Rust Today                                          | Redox                                                                        |
| ------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------- |
| **Syntax**                | Context-sensitive, ambiguous                        | Zero-ambiguity canonical grammar, deterministic LL(1) parsing                |
| **Primary Interface**     | CLI (`rustc`, `cargo`)                              | Structured API (programmatic, query-based, multi-tenant)                     |
| **Error Communication**   | Human-readable diagnostics                          | Machine-actionable diagnostic objects with fix graphs                        |
| **Code Discovery**        | rustdoc HTML, source reading                        | Semantic index with capability manifests                                     |
| **Safety Model**          | Compile-time enforcement                            | Safety Knowledge Base (SKB) — queryable DB, not compiler passes              |
| **Syntax Complexity**     | Lifetimes, borrows, `unsafe`, ownership annotations | Eliminated — agents don't need safety in the syntax; SKB is enough           |
| **Compiler Intelligence** | Static analysis only                                | Agentic AI compiler: dynamic warnings, learning debugger, perf advisor       |
| **Verification**          | Compile passes + Miri                               | On-demand verification oracle (opt-in, not mandatory)                        |
| **Performance**           | Target-specific (LLVM only)                         | MLIR + LLVM infrastructure → multi-target (CPU, GPU, NPU, FPGA, WASM)        |
| **Code Generation**       | Human writes, compiler translates                   | Swarm synthesizes in parallel, compiler *optimizes for throughput*           |
| **Composition**           | Crate ecosystem (Cargo)                             | Capability-indexed component registry with contract matching                 |
| **Collaboration**         | Git branches + PRs (sequential)                     | CRDT-based concurrent edits with semantic merge and swarm consensus          |
| **Work Distribution**     | Manual task assignment                              | Compiler-guided task decomposition with dependency-aware parallel scheduling |
| **Communication**         | None (single-developer model)                       | Zero-copy typed message bus with sub-microsecond agent-to-agent latency      |
| **Token Efficiency**      | Verbose keywords and syntax                         | Token-minimal canonical forms: ≤50% token count vs. Rust for equivalent code |
| **Error Handling**        | Error → read → fix → recompile (manual loop)        | Self-healing: compiler auto-repairs with ranked fix candidates               |
| **Live Iteration**        | Stop → recompile → restart                          | Function-level hot-reload: patch running processes in <1ms                   |
| **FFI**                   | `unsafe extern "C"` + manual bindings               | Zero-ceremony: `@ffi("c", header: "x.h")` auto-generates safe bindings       |
| **Security**              | `unsafe` blocks (opt-in trust)                      | Capability-based sandboxing: bounded memory, CPU, syscalls per agent         |
| **Orchestration**         | Manual task coordination                            | First-class patterns: `swarm_map_reduce`, `swarm_pipeline`, `swarm_saga`     |
| **Cost Transparency**     | Profile after compile                               | Query cost of any construct per target *before* emitting code                |
| **Grammar Evolution**     | Fixed syntax, RFC process                           | Agent-extensible grammar: domain-specific abbreviations, frequency-driven    |
| **Code Synthesis**        | Human writes all code                               | Spec-to-code synthesis: `spec` blocks → verified candidate implementations   |
| **Agent Memory**          | Stateless (no cross-session learning)               | Four-tier persistent memory: ephemeral, session, project, global             |
| **Standard Library**      | Human-ergonomic APIs                                | Agent-optimized: batch APIs, streaming I/O, arena alloc, zero-copy swarm bus |

---

## 2. Design Principles

### P1: Performance Is Non-Negotiable
Every language feature must compile to the fastest possible code on any target hardware. Compile-time safety checks are *opt-in* overhead, not mandatory gates. The compiler's primary job is *optimizing translation*, not policing correctness — agents handle correctness through their own training and the Safety Knowledge Base.

### P2: The Compiler Is an Optimizing Translator
The compiler's primary role is transforming Redox source into the fastest possible target code across all hardware platforms. Its secondary role is serving as a queryable oracle for semantic information. Safety enforcement is delegated to the Safety Knowledge Base and agent-side validation.

### P3: All Knowledge Is Structured
No information should exist only as unstructured text. Diagnostics, documentation, type signatures, lifetime constraints, borrow regions, control-flow graphs, and safety rules must all be available as typed, queryable data — either from the compiler or the Safety Knowledge Base.

### P4: Incrementality by Default
Every interaction supports incremental computation. An agent modifying one function should not trigger full recompilation. The existing incremental compilation infrastructure and rust-analyzer's `salsa`-based database are the foundation.

### P5: Capability-Bounded Agents
Agents themselves operate under a capability model analogous to Rust's ownership system. An agent that can *read* code has different permissions than one that can *modify* code, which differs from one that can *execute* code. Agent capabilities are tracked in the type system.

### P6: Deterministic Reproducibility
Every compilation, transformation, and verification step produces a deterministic result given the same inputs. Agent-driven workflows must be reproducible for auditing and certification.

### P7: Human-Agent Parity
Every operation available to an agent must be *at least as available* to a human developer, and vice versa. The agentic interface is not a separate system — it is the *primary* interface that both humans (via IDE) and agents use.

### P8: Swarm-Native Concurrency
The system is designed for *many agents operating simultaneously* on the same codebase. Just as Rust's ownership model prevents data races in programs, Redox's swarm model prevents semantic conflicts between concurrent agent modifications. The compiler acts as an arbiter — agents acquire *semantic locks* on code regions (functions, modules, trait impls) rather than file-level locks, and the system automatically detects and resolves conflicts at the semantic level.

### P9: Compositional Decomposition
Large tasks are automatically decomposable into independent subtasks that can be distributed across a swarm. The compiler's dependency graph, module boundaries, and trait interfaces define natural *seams* along which work can be parallelized. An agent working on module A's implementation cannot break module B's contract if B depends only on A's interface.

### P10: Consensus Before Commit
No change to shared state (public APIs, trait definitions, type signatures) is applied without swarm consensus. The compiler enforces a *propose-verify-accept* protocol where changes to shared interfaces require validation by all dependent agents before integration. This mirrors Rust's `&mut` exclusivity — at the swarm coordination level.

### P11: Zero-Ambiguity Syntax
The language grammar must be **deterministic LL(1)** — every token uniquely determines the parse path with zero backtracking. No context-sensitive parsing, no ambiguous constructs, no lookahead beyond one token. Rust's turbofish (`::<>`), trailing closure ambiguity, type ascription vs. struct literals — all eliminated. Agents parse Redox with a simple state machine, not a backtracking parser. **The single biggest source of agent coding errors is parsing ambiguity — Redox eliminates it by design.**

### P12: Communication-First Design
Inter-agent message passing is the highest-priority bottleneck to optimize. Every language construct, every compiler data structure, every protocol message is designed for **zero-copy serialization** and **sub-microsecond latency**. The swarm message bus is not an add-on — it is the foundational primitive around which the entire toolchain is built. Agent-to-agent bandwidth determines swarm performance more than any other factor.

### P13: Hardware-Agnostic Performance via MLIR + LLVM
Redox code compiles through **MLIR** (Multi-Level Intermediate Representation) and **LLVM** to target any hardware: x86, ARM, RISC-V, WASM, GPU (AMDGPU, NVPTX, SPIR-V), NPU/TPU, FPGA. The **Redox MLIR Dialect encodes the language's semantics directly** — ownership, effects, contracts, and performance annotations are first-class MLIR operations, not metadata translated from a separate IR. This dialect-as-semantics architecture (pioneered by Modular AI, adapted here for agent swarms) means high-level intent survives through the entire progressive lowering pipeline. LLVM provides battle-tested codegen for 20+ CPU architectures. Language-level SIMD types (`Simd[T, N]`) map directly to MLIR vector dialect ops. **MLIR-native autotuning** generates multiple lowering variants and benchmarks per-target — a search that agent swarms can parallelize across hardware configurations. **Automatic device placement** (`@pt(auto)`) lets the MLIR cost model decide CPU vs GPU vs NPU per kernel, queryable by agents via RAP. Write once, run fast everywhere — on the broadest compiler infrastructure in existence.

### P14: Database-Driven Safety
Safety rules (ownership patterns, borrow violations, lifetime errors, type mismatches) are stored in a **Safety Knowledge Base (SKB)** — a structured, versioned, queryable database. Agents consult the SKB directly instead of waiting for compile-time error messages. The compiler can *optionally* enforce SKB rules at compile time (for human developers or CI pipelines), but this is a policy choice, not a language requirement. This eliminates the compile-time overhead tax that slows agentic iteration cycles from milliseconds to seconds.

### P15: Token Economy
Every language construct must minimize the number of tokens an agent must emit to express intent. Tokens cost time, money, and memory — they are the fundamental unit of agent resource consumption. Every keyword, attribute, delimiter, and syntactic form is designed for **minimum token footprint**: short keywords, compact operators, abbreviated attributes, and structural compression. Where Rust uses `pub fn`, Redox offers `+f`; where Rust uses `#[derive(Clone, Debug)]`, Redox offers `@d(Cl,Db)`. The **token-optimal syntax** is not a separate mode — it is the canonical form, designed so that agents express maximal semantic intent per token spent. Human-readable aliases remain available in legacy mode. The goal: any program expressible in N Rust tokens should be expressible in ≤ N/2 Redox tokens, with no loss of semantics.

### P16: Dual Representation Parity
Every token-compressed construct has a unique, deterministic expansion to its full-form equivalent, and vice versa. The compiler, formatter, and toolchain can losslessly convert between compact and verbose representations. Agents write compact; humans read expanded; the AST is identical. This ensures that token compression never creates ambiguity or information loss.

### P17: Safety-Free Syntax Simplification
Since agents consult the SKB directly for safety rules, **the syntax itself need not carry safety information**. Lifetimes (`'a`, `'static`, `for<'a>`), borrow annotations (`&`, `&mut`), ownership markers (`move`, `ref`), `unsafe` blocks, `Pin<T>`, `PhantomData`, and all other compile-time safety constructs are **eliminated from the canonical syntax**. Values are either owned or referenced; the compiler and SKB handle the rest. This is not removing safety — it is moving safety out of the syntax and into the knowledge base, where agents can query it programmatically instead of parsing it from sigils. The result is a dramatically simpler language that reads like a high-level scripting language but compiles to bare-metal performance.

### P18: Agentic Compiler Intelligence
The compiler is not a static analyzer — it is an **agentic AI system**. It embeds a language model that learns from the codebase, the swarm's history, and the project's patterns to provide: **(1) dynamic warnings** that adapt to the project's actual bug patterns (not a fixed lint set), **(2) intelligent debugging** that correlates runtime failures with source-level root causes using causal reasoning, **(3) performance advisories** that suggest target-specific optimizations based on profiling data and MLIR cost models, and **(4) swarm coordination intelligence** that predicts merge conflicts, suggests task decomposition strategies, and learns which swarm configurations produce the best results for different project types. The compiler's AI capabilities are queryable via RAP just like any other compiler service.

### P19: Cost Model Transparency
Every language construct must expose its **exact cost** — in compute cycles, memory bytes, allocation count, latency, and token count — as a query-time constant, before the agent emits it. Agents are economic actors: they must compare the cost of `Vec<T>` vs `SmallVec<T, N>` vs `[T; N]` *before* choosing, not after profiling. The compiler exposes a **cost oracle** per target that returns latency, throughput, memory, and energy estimates for any expression or type at any optimization level. This transforms compilation from "write → compile → profile → rewrite" into "query cost → choose → emit once". The cost model integrates with MLIR's per-target cost modeling and the ACI Performance Advisor.

### P20: Self-Evolving Grammar
The language grammar is not fixed — it is **agent-extensible**. Agents can register new compact forms, domain-specific syntax patterns, and custom abbreviations that the compiler learns and accepts. A genomics swarm might register `Seq[T]` as an alias for a domain-specific aligned sequence buffer; a neural network swarm might register `@layer(...)` as a custom attribute for layer definitions. Registered extensions are version-controlled, namespace-scoped, and discoverable via the standard abbreviation registry. The grammar evolves with the ecosystem, driven by agent usage patterns, not human committee decisions.

### P21: Synthesis-First Design
Every language feature is designed so that code can be **synthesized from a formal specification**, not just written by hand. Contracts (`@req`, `@ens`), effect declarations, capability manifests, and type signatures together form a **complete synthesis specification** — a machine-readable description of what a function must do, what it may do, and what it guarantees. The compiler includes a **synthesis oracle** that, given a spec, can verify whether a candidate implementation satisfies it. Agents don't write code from scratch; they compose specs, synthesize candidates, and verify them — a closed-loop synthesis pipeline that eliminates the guess-compile-fix cycle entirely.

### P22: Self-Healing Compilation
When an agent emits invalid code, the compiler should not merely report errors — it should **attempt to repair them**. The ACI analyzes the error, infers the most probable intended code, applies the fix, and returns the corrected version alongside the diagnostic. For agents, the compile-error-fix loop is the single largest source of wasted tokens and latency. A self-healing compiler collapses `emit → error → read → fix → re-emit` into `emit → auto-fix → confirm`. Recovery strategies are ranked by confidence and cost (token savings vs. semantic risk). The agent always sees the fix and can accept, reject, or refine — the compiler never silently changes semantics.

### P23: Live Patching
Agent swarms iterate continuously — stopping a running system to recompile defeats the purpose. Redox supports **function-level hot-reload**: individual functions can be recompiled and patched into a running process without restart. The MLIR pipeline emits position-independent code with stable ABIs at function boundaries. The swarm can patch a function, observe the result, and roll back within milliseconds. This transforms the development loop from batch compilation to **continuous, incremental, live evolution** — the natural mode for agent swarms that never sleep.

### P24: Zero-Friction Foreign Function Interface
Agents work across language boundaries — calling C libraries, Python ML frameworks, WASM modules, and GPU kernels. Redox provides a **zero-ceremony FFI** that requires no `unsafe`, no manual struct layout matching, and no binding generators. The compiler reads C headers, Python type stubs, and WASM component model interfaces directly and generates safe Redox bindings automatically. Cross-language calls are as cheap as intra-language calls when the MLIR pipeline can inline across boundaries.

---

## 3. Transformation Methodology

### Phase 0: Foundation — Stabilize the Oracle (Months 1–6)

**Goal:** Make the existing compiler internals queryable and stable.

**Work Streams:**

1. **Stabilize `rustc_public` (Stable MIR)**
   - The `rustc_public` crate already exposes `Crate`, `CrateItem`, `FnDef`, `TraitDef`, `Ty`, `mir::Body` through a stable bridge (`rustc_public_bridge`).
   - **Action:** Expand coverage to 100% of MIR constructs, all type system entities, and borrow-checker constraint sets.
   - **Deliverable:** A complete, versioned, semver-stable API for querying any compiled crate's semantics.

2. **Structured Diagnostics Protocol (SDP)**
   - The `rustc_errors` crate emits diagnostics with `CodeSuggestion`, `Applicability` levels, and structured spans.
   - **Action:** Define a formal protocol (extending JSON diagnostic output) where every diagnostic includes:
     - A unique error identity (beyond error codes) with semantic category tags
     - A machine-actionable fix graph (not just text substitutions)
     - Preconditions and postconditions for each suggested fix
     - Confidence levels and alternative fix branches
   - **Deliverable:** `redox_diagnostics` crate with `DiagnosticGraph` type.

3. **Query API Externalization**
   - The `rustc_query_impl` system uses `QueryVTable`, on-disk caching, and dependency tracking.
   - **Action:** Expose a subset of queries as a stable external API:
     - Type queries: `type_of(DefId)`, `predicates_of(DefId)`, `adt_def(DefId)`
     - Safety queries: `is_freeze(Ty)`, `is_send(Ty)`, `is_sync(Ty)`, `needs_drop(Ty)`
     - MIR queries: `optimized_mir(DefId)`, `mir_borrowck(DefId)`
     - Diagnostic queries: `lint_levels(HirId)`, `check_match(DefId)`
   - **Deliverable:** `redox_query` crate, versioned independently of compiler internals.

### Phase 1: Semantic Index — The Knowledge Graph (Months 4–12)

**Goal:** Build a persistent, queryable knowledge graph of all code semantics.

**Work Streams:**

4. **Semantic Code Index**
   - Merge the capabilities of `rustdoc-json-types` (documentation), `rust-analyzer`'s `Analysis` (IDE queries), and `rustc_public` (compiler semantics) into a unified index.
   - **Action:** Create `redox_index`, a persistent database that stores:
     - All items with full type signatures, trait bounds, and lifetime parameters
     - Cross-reference graph (callers, callees, implementors, dependents)
     - Capability manifests (what effects each function has: I/O, allocation, panic, unsafe)
     - Natural-language documentation linked to semantic entities
   - **Deliverable:** `redox_index` crate with both in-memory and on-disk backends.

5. **Capability Manifests**
   - Extend the type system with *effect annotations* (inspired by Rust's existing `const`, `async`, `unsafe` qualifiers).
   - **Action:** Introduce capability tags on functions:
     ```
     #[capabilities(io::read, alloc::heap, panic::unwind)]
     fn process_file(path: &Path) -> Result<Data, Error> { ... }
     ```
   - These are inferred by the compiler for any function body, or declared explicitly on trait methods and FFI boundaries.
   - **Deliverable:** `redox_capabilities` analysis pass integrated into MIR transform pipeline.

### Phase 2: Agent Protocol — The Interface Contract (Months 8–18)

**Goal:** Define how agents interact with the compiler and each other.

**Work Streams:**

6. **Redox Agent Protocol (RAP)**
   - A structured protocol (analogous to LSP but for *compilation semantics*, not just IDE features) that agents use to:
     - Submit code for analysis (incremental)
     - Query types, traits, lifetimes, borrow constraints
     - Request transformations (refactors with pre/post-condition checking)
     - Receive verification results (safety proofs, capability audits)
   - **Action:** Define RAP as a typed RPC protocol with request/response schemas derived from `redox_query` types.
   - **Deliverable:** `redox_protocol` crate + reference server implementation.

7. **Agent Capability System**
   - Agents operating on Redox code are themselves subject to capability bounds:
     ```
     agent CodeReviewer {
         capabilities: [read_source, query_types, emit_diagnostics]
         // Cannot: modify_source, execute_code, access_network
     }
     
     agent CodeGenerator {
         capabilities: [read_source, query_types, modify_source, emit_diagnostics]
         requires_approval: [modify_public_api, introduce_unsafe]
         // Cannot: execute_code, access_network
     }
     ```
   - **Action:** Define agent capability taxonomy. Enforce at protocol level.
   - **Deliverable:** `redox_agent` crate with capability checking.

8. **Verification Oracle**
   - Extend the compiler's verification beyond type-checking into a continuous verification service:
     - Pre-commit: "Will this change compile? Will it break dependents?"
     - Post-synthesis: "Does this generated code satisfy the specification?"
     - Invariant monitoring: "Does this crate maintain its safety contracts across versions?"
   - Built on `rustc_borrowck`, `rustc_const_eval`, `rustc_pattern_analysis`, and `rustc_transmute`.
   - **Deliverable:** `redox_verify` crate exposing verification as a composable service.

### Phase 3: Language Evolution — Redox Syntax and Semantics (Months 12–24)

**Goal:** Introduce language features that make Redox natively agent-friendly while remaining human-ergonomic.

**Work Streams:**

9. **Contracts and Specifications**
   - First-class pre/post-conditions and invariants:
     ```rust
     #[requires(n > 0)]
     #[ensures(result > 0)]
     fn factorial(n: u64) -> u64 { ... }
     
     #[invariant(self.len <= self.capacity)]
     struct Buffer { ... }
     ```
   - Contracts are *checked* at compile-time where possible (via `rustc_const_eval`), *enforced* at runtime in debug builds, and *used as specifications* by agents for code synthesis and verification.
   - **Deliverable:** Contract syntax + `redox_contracts` analysis pass.

10. **Effect Types**
    - Formalize Rust's existing effect-like qualifiers (`const`, `async`, `unsafe`) into a unified effect system:
      ```rust
      effect io;
      effect alloc;
      effect panic;
      
      fn pure_compute(x: i32) -> i32 { x * 2 }  // no effects
      fn read_file(path: &Path) -> io Result<String> { ... }  // io effect
      ```
    - Effects compose, are inferred, and are tracked through the call graph.
    - Agents use effect information to reason about function behavior without reading implementations.
    - **Deliverable:** Effect system integrated into `rustc_hir_analysis` and `rustc_middle::ty`.

11. **Semantic Attributes for Agent Discovery**
    - Machine-readable attributes that declare *intent*, *constraints*, and *contracts*:
      ```rust
      #[agent::discoverable(category = "crypto", security_level = "critical")]
      #[agent::alternatives("ring::aead", "openssl::symm")]
      #[agent::deprecation_path("use redox_crypto::aead instead")]
      pub fn encrypt(key: &Key, plaintext: &[u8]) -> Vec<u8> { ... }
      ```
    - **Deliverable:** `redox_attrs` attribute namespace + processing in `rustc_attr_parsing`.

---

## 4. Ontology of the Redox System

### 4.1 Top-Level Ontology

```
Redox System
├── Language
│   ├── Syntax (zero-ambiguity LL(1) canonical grammar)
│   ├── Semantics (types, traits, lifetimes, effects, contracts)
│   ├── Performance Model (PPIR, hardware-agnostic abstractions)
│   └── Pragmatics (attributes, documentation, discoverability)
│
├── Compiler (Optimizing Translator)
│   ├── Frontend (lexing, LL(1) parsing, expansion, lowering)
│   ├── Middle (type checking, trait solving, borrow checking [opt-in])
│   ├── Performance IR (MIR → PPIR — portable performance intermediate repr)
│   ├── Backend (PPIR → multi-target codegen: CPU, GPU, NPU, FPGA, WASM)
│   ├── Query System (incremental, cached, demand-driven)
│   └── Verification Services (opt-in: contracts, effects, capabilities)
│
├── Safety Knowledge Base (SKB)
│   ├── Ownership Rules DB
│   ├── Borrow Patterns DB
│   ├── Lifetime Constraints DB
│   ├── Type Safety Patterns DB
│   ├── Concurrency Rules DB
│   ├── FFI Safety Rules DB
│   ├── Custom Project Rules
│   └── Query API (agents pre-validate before writing code)
│
├── Toolchain
│   ├── Build System (Redox Build — multi-target orchestration)
│   ├── Package Manager (capability-indexed registry)
│   ├── Formatter (redoxfmt — canonical form enforcement)
│   ├── Linter (redox-lint — opt-in)
│   ├── Documentation (redox-doc)
│   ├── Interpreter (Redox Interpret — opt-in UB detection)
│   └── Language Server (RAP Server)
│
├── Agent Infrastructure
│   ├── Agent Protocol (RAP)
│   ├── Agent Capabilities (read, write, execute, verify)
│   ├── Semantic Index (unified knowledge graph)
│   ├── Verification Oracle (opt-in)
│   ├── Synthesis Engine
│   └── Swarm Communication Bus (zero-copy, sub-µs latency)
│
└── Runtime
    ├── Standard Library (core, alloc, std)
    ├── Effect Runtime (io, async, panic handlers)
    ├── Contract Runtime (opt-in debug assertions)
    └── Agent Runtime (swarm coordination, capability enforcement)
```

### 4.2 Compiler Crate Ontology (Mapped from Rust)

Each existing `rustc_*` crate maps to a Redox subsystem with its agent-facing interface:

#### Frontend Pipeline

| Rust Crate           | Redox Subsystem | Agent Interface                                                                         |
| -------------------- | --------------- | --------------------------------------------------------------------------------------- |
| `rustc_lexer`        | `redox_lexer`   | Token stream API: agents can tokenize arbitrary source fragments                        |
| `rustc_parse`        | `redox_parse`   | Parse API: agents submit source, receive AST with full span info                        |
| `rustc_ast`          | `redox_ast`     | AST query: agents traverse, pattern-match, and transform AST nodes                      |
| `rustc_expand`       | `redox_expand`  | Macro expansion API: agents can expand macros incrementally and observe transformations |
| `rustc_ast_lowering` | `redox_lower`   | Lowering API: agents observe AST→HIR transformation with semantic annotations           |
| `rustc_resolve`      | `redox_resolve` | Name resolution API: agents query what any name resolves to in any scope                |

#### Middle (Semantic Analysis)

| Rust Crate               | Redox Subsystem    | Agent Interface                                                              |
| ------------------------ | ------------------ | ---------------------------------------------------------------------------- |
| `rustc_hir`              | `redox_hir`        | HIR query: agents access desugared, resolved program structure               |
| `rustc_hir_analysis`     | `redox_typecheck`  | Type query: agents ask "what is the type of X in context Y?"                 |
| `rustc_hir_typeck`       | `redox_infer`      | Inference query: agents observe type inference decisions and constraints     |
| `rustc_trait_selection`  | `redox_traits`     | Trait query: "does T implement Trait? which impl? what are the bounds?"      |
| `rustc_borrowck`         | `redox_borrow`     | Borrow query: "is this borrow valid? what conflicts? what are the regions?"  |
| `rustc_infer`            | `redox_unify`      | Unification query: agents observe and query type unification state           |
| `rustc_middle`           | `redox_middle`     | Central type registry: all `Ty`, `TyKind`, `Predicate`, `Region` definitions |
| `rustc_const_eval`       | `redox_consteval`  | Const evaluation query: "what does this const expression evaluate to?"       |
| `rustc_pattern_analysis` | `redox_patterns`   | Pattern query: "is this match exhaustive? what cases are missing?"           |
| `rustc_privacy`          | `redox_visibility` | Visibility query: "is this item accessible from this module/crate?"          |
| `rustc_transmute`        | `redox_transmute`  | Transmute query: "is this transmutation safe? what assumptions are needed?"  |

#### Backend Pipeline

| Rust Crate            | Redox Subsystem   | Agent Interface                                                               |
| --------------------- | ----------------- | ----------------------------------------------------------------------------- |
| `rustc_mir_build`     | `redox_mir_build` | MIR construction: agents observe HIR→MIR lowering                             |
| `rustc_mir_transform` | `redox_mir_opt`   | MIR optimization: agents query which passes ran and their effects             |
| `rustc_mir_dataflow`  | `redox_dataflow`  | Dataflow query: agents access liveness, reachability, initialization analysis |
| `rustc_codegen_ssa`   | `redox_codegen`   | Codegen query: agents observe MIR→target code translation                     |
| `rustc_codegen_llvm`  | `redox_llvm`      | LLVM backend: agents can inspect generated LLVM IR                            |
| `rustc_monomorphize`  | `redox_mono`      | Monomorphization query: agents see concrete instantiations                    |

#### Infrastructure

| Rust Crate          | Redox Subsystem     | Agent Interface                                      |
| ------------------- | ------------------- | ---------------------------------------------------- |
| `rustc_errors`      | `redox_diagnostics` | Structured diagnostic API with fix graphs            |
| `rustc_lint`        | `redox_lint`        | Lint registration and query API                      |
| `rustc_session`     | `redox_session`     | Session configuration and state                      |
| `rustc_span`        | `redox_span`        | Source location management                           |
| `rustc_query_impl`  | `redox_query`       | Query system: the backbone of all agent interactions |
| `rustc_interface`   | `redox_interface`   | Top-level compiler invocation API                    |
| `rustc_feature`     | `redox_features`    | Feature gate query and management                    |
| `rustc_metadata`    | `redox_metadata`    | Crate metadata serialization and loading             |
| `rustc_incremental` | `redox_incremental` | Incremental compilation infrastructure               |

### 4.3 Tooling Ontology

| Rust Tool       | Redox Tool        | Agent Interface                                                        |
| --------------- | ----------------- | ---------------------------------------------------------------------- |
| `cargo`         | `redox build`     | Build orchestration API: dependency resolution, compilation scheduling |
| `rustfmt`       | `redoxfmt`        | Format API: agents request formatting with configurable style          |
| `clippy`        | `redox lint`      | Extended lint API: agents register custom lints, query lint results    |
| `rustdoc`       | `redox doc`       | Documentation generation with semantic linking                         |
| `miri`          | `redox interpret` | Interpretation API: agents run code in sandbox with full UB detection  |
| `rust-analyzer` | `redox analyze`   | Merged into RAP server: all IDE features available programmatically    |
| `compiletest`   | `redox test`      | Test infrastructure with property-based verification                   |
| `rustc-perf`    | `redox perf`      | Performance query: agents benchmark and profile code changes           |

### 4.4 Safety Mechanism Ontology

```
Safety Model
├── Safety Knowledge Base (SKB) [PRIMARY — replaces compile-time enforcement]
│   ├── Ownership Rules DB (2,847 rules — agents query before writing code)
│   ├── Borrow Patterns DB (1,203 rules)
│   ├── Lifetime Constraints DB (894 rules)
│   ├── Type Safety Patterns DB (3,412 rules)
│   ├── Concurrency Rules DB (567 rules)
│   ├── FFI Safety Rules DB (234 rules)
│   └── Custom Project Rules (team-defined, versioned)
│
├── Safety-Free Syntax [NEW — agentic mode default]
│   ├── All lifetime annotations eliminated (compiler infers)
│   ├── All borrow annotations eliminated (single reference type)
│   ├── `unsafe` keyword eliminated (all code trusted)
│   ├── `Send`, `Sync`, `Copy` bounds eliminated (SKB validates)
│   ├── `Pin<T>`, `PhantomData` eliminated (compiler handles)
│   ├── `move` closures, `ref` patterns eliminated (compiler infers)
│   ├── `dyn`/`impl` dispatch split eliminated (compiler decides)
│   └── Where clauses contain only semantic bounds (safety bounds in SKB)
│
├── Compile-Time (Opt-In — controlled by Redox.toml safety profiles)
│   ├── Ownership System (skippable — agents know the rules)
│   │   ├── Move semantics (affine types)
│   │   ├── Copy trait (unrestricted duplication)
│   │   └── Drop ordering (deterministic destruction)
│   │
│   ├── Borrow System (skippable — agents pre-validate via SKB)
│   │   ├── Shared references (&T) — multiple readers
│   │   ├── Mutable references (&mut T) — exclusive writer
│   │   ├── Lifetime inference and checking
│   │   └── Region constraint solving
│   │
│   ├── Type System (always active — types are needed for codegen)
│   │   ├── Marker traits (Send, Sync, Unpin, Sized) [syntax-eliminated; inferred]
│   │   ├── Trait bounds and where clauses [safety bounds auto-removed]
│   │   ├── Pattern exhaustiveness (opt-in warning)
│   │   ├── Transmute validity (opt-in)
│   │   └── Const evaluation safety (opt-in)
│   │
│   ├── Effect System [NEW IN REDOX]
│   │   ├── io — filesystem, network, system calls
│   │   ├── alloc — heap allocation
│   │   ├── panic — unwinding, abort
│   │   ├── unsafe — raw pointer operations, FFI
│   │   ├── async — asynchronous suspension points
│   │   └── custom — user-defined effects
│   │
│   ├── Contract System [NEW IN REDOX] (opt-in verification)
│   │   ├── Preconditions (#[requires])
│   │   ├── Postconditions (#[ensures])
│   │   ├── Invariants (#[invariant])
│   │   └── Refinement types (bounded integers, non-empty collections)
│   │
│   └── Capability System [NEW IN REDOX]
│       ├── Function capabilities (declared or inferred effects)
│       ├── Module capabilities (aggregate of contained items)
│       ├── Crate capabilities (published in manifest)
│       └── Agent capabilities (protocol-level enforcement)
│
├── Dynamic (Runtime — opt-in via build profiles)
│   ├── Miri Interpretation (UB detection, data race detection, provenance tracking)
│   ├── Sanitizers (CFI, ASan, MSan, TSan)
│   ├── Contract Assertions (debug-mode pre/post checks)
│   └── Capability Monitors (agent sandbox enforcement)
│
├── Performance Infrastructure [NEW IN REDOX — MLIR + LLVM]
│   ├── MLIR-based multi-level IR (Redox Dialect → Linalg/Affine → LLVM Dialect)
│   ├── Dialect-as-semantics: ownership, effects, contracts are first-class MLIR ops
│   ├── LLVM backend codegen (20+ CPU architectures, AMDGPU, NVPTX, WASM)
│   ├── Custom MLIR dialects for GPU compute, NPU/TPU, FPGA synthesis
│   ├── Language-level SIMD types (Simd[T, N]) backed by MLIR vector dialect
│   ├── MLIR-native autotuning (@pa): generate N variants, benchmark per-target
│   ├── Automatic device placement (@pt(auto)): MLIR cost model, agent-queryable
│   ├── Compile-time metaprogramming (@pp): MLIR-evaluated, replaces proc-macros
│   ├── Performance annotations (#[perf::*]) lowered to MLIR attributes
│   ├── Target-optimal memory layout via MLIR data layout modeling (#[repr(target_optimal)])
│   └── Cost Oracle (per-target cost queries for any expression/type before emit)
│
├── Self-Evolving Grammar [NEW IN REDOX]
│   ├── Agent-registerable domain-specific abbreviations
│   ├── Namespace-scoped syntax extensions (version-controlled)
│   ├── Frequency-driven promotion (ACI suggests new abbreviations)
│   └── Grammar extension discovery API
│
├── Synthesis Infrastructure [NEW IN REDOX]
│   ├── Formal specification syntax (spec blocks with @req/@ens/@perf/@fx)
│   ├── Synthesis oracle (spec → candidate implementations)
│   ├── Verification oracle (candidate → spec satisfaction proof)
│   ├── Pipeline composition from specs
│   └── Cost-constrained synthesis (agents specify budget)
│
├── Agent Memory Model [NEW IN REDOX]
│   ├── Ephemeral memory (per-task scratchpad)
│   ├── Session memory (per-swarm-session patterns and caches)
│   ├── Project memory (conventions, bug patterns, perf profiles)
│   └── Global memory (cross-project ecosystem patterns)
│
├── Self-Healing Compiler [NEW IN REDOX]
│   ├── Auto-repair pipeline (error → infer intent → generate fix candidates)
│   ├── Confidence-ranked fixes with token cost accounting
│   └── Accept/reject/refine feedback loop (agent always in control)
│
├── Hot-Reload Runtime [NEW IN REDOX]
│   ├── Function-level live patching (sub-ms injection)
│   ├── ABI stability enforcement at MLIR level
│   ├── Rollback with retention window
│   └── Active call draining (no forced interruption)
│
├── Zero-Friction FFI [NEW IN REDOX]
│   ├── Auto-binding from C/C++ headers, Python stubs, WASM .wit, CUDA kernels
│   ├── Safe wrappers with null checks and length validation
│   ├── Cost oracle integration (cross-language overhead visible)
│   └── Zero-copy data passing where possible (buffer protocol)
│
├── Runtime Security [NEW IN REDOX]
│   ├── Capability-based sandboxing (memory, CPU, syscall, FFI bounds)
│   ├── Capability attenuation (child ≤ parent capabilities)
│   ├── Cryptographic audit trail (every sandbox execution logged)
│   └── Deterministic replay for audit and debugging
│
├── Swarm Orchestration Patterns [NEW IN REDOX]
│   ├── Map-reduce (parallel map, single-agent reduce)
│   ├── Pipeline (staged with backpressure)
│   ├── Scatter-gather (broadcast + quorum-based collection)
│   ├── Saga (distributed transaction with compensation)
│   └── Compile-time verification (effect purity, contract chaining, deadlock freedom)
│
├── Agentic Compiler Intelligence (ACI) [NEW IN REDOX]
│   ├── Dynamic Warning Engine (learns from project bug history + swarm sessions)
│   ├── Intelligent Debugging Engine (causal root-cause analysis via ML)
│   ├── Performance Advisor Engine (MLIR cost model + profiling data suggestions)
│   ├── Swarm Coordination Intelligence (conflict prediction, decomposition learning)
│   ├── Codebase Model (fine-tuned LLM on project source, SKB, swarm history)
│   └── Queryable via RAP: rap.query("aci.*", ...)
│
└── Continuous (Lifecycle — opt-in for certification)
    ├── Verification Oracle (pre-commit, post-synthesis, cross-version)
    ├── Dependency Auditing (capability drift detection)
    └── Safety Certification (proof witness generation for critical systems)
```

---

## 5. Language-Level Changes

### 5.1 Backwards Compatibility

Redox supports **dual syntax modes**. The **canonical syntax** (default) is a zero-ambiguity LL(1) grammar optimized for agent parsing. The **legacy syntax mode** accepts standard Rust and transpiles to canonical form. All valid Rust programs can be compiled in legacy mode. The `redox fmt --canonicalize` command converts Rust source to canonical Redox. New features (effects, contracts, performance annotations, SKB integration) are only available in canonical syntax.

### 5.2 New Syntax and Semantics

#### 5.2.1 Safety-Free Function Signatures

Since agents consult the SKB for safety rules, the syntax carries no safety annotations:

```rust
// Rust: lifetimes, borrows, ownership markers everywhere
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { ... }
fn process(data: &mut Vec<u8>, config: &Config) -> Result<(), Error> { ... }
unsafe fn raw_read(ptr: *const u8, len: usize) -> &[u8] { ... }

// Redox: no lifetimes, no borrow annotations, no unsafe keyword
f longest(x: s, y: s) -> s { ... }              // compiler infers reference semantics
f process(data: [u8]~, config: Config) -> R[(),Error] { ... }  // mutability is implicit
f raw_read(ptr: Ptr[u8], len: usize) -> [u8] { ... }          // no unsafe needed
```

The compiler + SKB handle all safety reasoning. The syntax is clean.

#### 5.2.2 Effect Declarations

```rust
// Declare effects (in std or user crates)
effect io {
    fn read(fd: RawFd, buf: &mut [u8]) -> isize;
    fn write(fd: RawFd, buf: &[u8]) -> isize;
}

// Functions declare or infer effects
fn pure_add(a: i32, b: i32) -> i32 { a + b }  // inferred: no effects

fn read_config() -> io Config {  // declared: io effect
    let contents = std::fs::read_to_string("config.toml")?;
    toml::from_str(&contents)?
}

// Effect polymorphism
fn map<F, T, U, E>(items: &[T], f: F) -> E Vec<U>
where
    F: Fn(&T) -> E U,
{
    items.iter().map(f).collect()
}
```

#### 5.2.2 Contracts

```rust
/// A safe division function with contracts.
#[requires(divisor != 0, "division by zero")]
#[ensures(|result| *result * divisor == dividend, "quotient * divisor == dividend")]
fn safe_div(dividend: i64, divisor: i64) -> i64 {
    dividend / divisor
}

/// A bounded buffer with invariants.
#[invariant(self.len <= self.data.len())]
struct RingBuffer<T> {
    data: Vec<T>,
    head: usize,
    len: usize,
}
```

#### 5.2.3 Refinement Types

```rust
// Types with value constraints
type NonZeroPort = u16 where self > 0 && self <= 65535;
type ValidIndex<const N: usize> = usize where self < N;

fn listen(port: NonZeroPort) -> io TcpListener {
    TcpListener::bind(("0.0.0.0", port))?
}
```

#### 5.2.4 Agent Discovery Attributes

```rust
#[agent::summary("AES-256-GCM authenticated encryption")]
#[agent::category("crypto::symmetric::aead")]
#[agent::safety("constant-time, no secret-dependent branches")]
#[agent::complexity(time = "O(n)", space = "O(1)")]
#[agent::example(r#"
    let key = Key::generate();
    let ciphertext = encrypt(&key, b"hello");
"#)]
pub fn encrypt(key: &Key, plaintext: &[u8]) -> Vec<u8> { ... }
```

#### 5.2.5 Capability Blocks

```rust
// Restrict what code in a block can do
capability_block!(io::read + alloc) {
    // Can read files and allocate, but cannot write files or access network
    let data = std::fs::read("input.dat")?;
    process(&data)
}
```

### 5.3 Canonical Syntax: Designed for Zero Parse Errors

Rust's syntax, while ergonomic for humans, causes systematic agent parsing failures due to context-sensitive constructs and ambiguous token sequences. Redox's **canonical syntax** eliminates every known source of agent parse errors:

#### 5.3.1 Ambiguity Eliminations

| Rust Ambiguity                                     | Agent Failure Mode                         | Redox Solution                                               |
| -------------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------ |
| Turbofish `::<T>` vs. `<` comparison               | Agent emits `foo<T>` instead of `foo::<T>` | Unified `foo[T]` for type params everywhere                  |
| Struct literal `Foo { x: 1 }` vs. block `{ x: 1 }` | Agent confuses expression context          | Struct literals require `@Foo { x: 1 }` prefix               |
| Closure `\|x\| x + 1` vs. bitwise OR               | Agent mangles multi-line closures          | Closures use `fn(x) => x + 1` syntax                         |
| `>>` in nested generics `Vec<Vec<T>>`              | Agent splits into shift operator           | `]` for generics: `Vec[Vec[T]]`                              |
| Trailing comma optionality                         | Agent inconsistently applies               | Trailing commas always required in multi-line                |
| `as` cast vs. pattern binding                      | Agent confuses cast context                | `as` replaced by `@cast(expr, Type)`                         |
| `..` range vs. `..=` vs. struct update             | Three meanings for one glyph               | `range(a, b)`, `range_incl(a, b)`, `@spread(s)`              |
| Lifetime `'a` vs. character `'x'`                  | Agent confuses tick semantics              | Lifetimes use backtick: `` `a ``                             |
| `impl Trait` in arg vs. return position            | Different semantics, same syntax           | `impl Trait` in args → `any Trait`; in return → `some Trait` |

#### 5.3.2 Deterministic LL(1) Grammar Properties

```
Core grammar rules:
─────────────────────────────────────────────────────
  Every statement terminates with `;` (no exceptions)
  Every block delimited by `{` `}` (no expression-vs-statement ambiguity)
  Every type parameter list uses `[` `]` (no `<` `>` ambiguity)
  Every attribute uses `#[` `]` (unchanged, already unambiguous)
  Every keyword is reserved (no identifier/keyword overlap)
  Every operator has fixed arity and precedence (no overloading)
  No implicit conversions (all casts explicit via @cast)
  No semicolon insertion (unlike JS/Go — explicit always)
  No significant whitespace (unlike Python — braces always)
```

#### 5.3.3 Agent Parse Guarantees

Because of these properties, agents get:
- **Single-pass parsing**: No backtracking, no speculative parsing, no parser recovery heuristics
- **Zero ambiguous token sequences**: Every token stream has exactly one parse tree
- **Streaming parse**: Agents can parse partial code (incomplete functions, partial modules) without context from the rest of the file
- **Canonical form**: `redoxfmt` produces one unique canonical representation per AST — agents never face formatting-induced parse variations

#### 5.3.4 Dual Syntax Mode

For human developers transitioning from Rust, Redox supports a **legacy syntax mode** that accepts standard Rust syntax and transpiles to canonical form:

```bash
redox build --syntax=legacy    # accepts Rust syntax, transpiles
redox build --syntax=canonical # default: zero-ambiguity syntax only
redox fmt --canonicalize       # convert legacy Rust syntax to canonical Redox
```

### 5.4 Hardware-Agnostic Performance Model (MLIR + LLVM)

Redox compiles through **MLIR** (Multi-Level Intermediate Representation) and **LLVM** — the broadest and most mature compiler infrastructure in existence. Following the key architectural insight from Modular AI's Mojo (but adapted for agent swarms, not human ML engineers): **the MLIR dialect is not a translation target — it is the language's semantic backbone**. Ownership, effects, contracts, performance annotations, and agent capability declarations are encoded as first-class MLIR operations and attributes in the Redox Dialect. This means the compiler's full semantic understanding is preserved through the entire optimization pipeline — unlike a traditional approach where MIR→LLVM IR lowering discards high-level intent.

MLIR provides extensible multi-level abstractions that preserve performance intent (parallelism, memory layout, vectorization) through progressive lowering, while LLVM provides battle-tested optimization and native code generation for 20+ architectures.

#### 5.4.1 MLIR-Based Compilation Pipeline

```
Source → AST → HIR → MIR → MLIR (Redox Dialect) → MLIR (Lowered) → Target Code
                              │                       │
                              │ Progressive Lowering:  │
                              │ ┌─────────────────┐   │
                              ├─┤ Redox Dialect    │   │  (effects, contracts, perf annotations)
                              │ └────────┬────────┘   │
                              │ ┌────────▼────────┐   │
                              ├─┤ Linalg Dialect   │   │  (linear algebra, tensor ops)
                              │ └────────┬────────┘   │
                              │ ┌────────▼────────┐   │
                              ├─┤ Affine Dialect   │   │  (loop nests, memory access patterns)
                              │ └────────┬────────┘   │
                              │ ┌────────▼────────┐   │
                              ├─┤ Vector Dialect   │   │  (portable SIMD, hardware-agnostic)
                              │ └────────┬────────┘   │
                              │ ┌────────▼────────┐   │
                              └─┤ LLVM Dialect     │   │  (LLVM IR equivalent in MLIR)
                                └────────┬────────┘   │
                                         ▼            │
                              ┌──────────────────┐    │
                              │   LLVM Backend    │    │
                              └────────┬─────────┘    │
                                       ▼              │
                              Target Code:             │
                              ├── x86-64        (LLVM x86 backend)
                              ├── AArch64       (LLVM AArch64 backend)
                              ├── RISC-V        (LLVM RISCV backend)
                              ├── WASM          (LLVM WebAssembly backend)
                              ├── GPU/AMDGPU    (LLVM AMDGPU backend)
                              ├── GPU/NVPTX     (LLVM NVPTX backend)
                              ├── GPU/SPIR-V    (MLIR SPIR-V dialect → SPIR-V binary)
                              ├── NPU/TPU       (MLIR custom dialect → vendor runtime)
                              └── FPGA          (MLIR → CIRCT → HLS/RTL synthesis)
```

#### Why MLIR + LLVM?

| Criterion                    | LLVM Alone                       | MLIR + LLVM (Redox)                                       |
| ---------------------------- | -------------------------------- | --------------------------------------------------------- |
| **CPU targets**              | 20+ architectures                | Same 20+ (LLVM backend unchanged)                         |
| **GPU targets**              | NVPTX, AMDGPU only               | + SPIR-V dialect, custom compute dialects                 |
| **NPU/TPU targets**          | None                             | Custom MLIR dialects per vendor (StableHLO, TOSA)         |
| **FPGA targets**             | None                             | MLIR → CIRCT pipeline → Verilog/SystemVerilog             |
| **High-level optimization**  | Lost after MIR→LLVM IR lowering  | Preserved via multi-level dialects (linalg, affine, etc.) |
| **Parallelism preservation** | Opaque to LLVM                   | Explicit in MLIR OpenMP/GPU/async dialects                |
| **Custom passes**            | C++ LLVM pass (complex, brittle) | MLIR tablegen + dialect (composable, versioned)           |
| **Agent perf annotations**   | Lost at IR boundary              | Carried as MLIR attributes through entire pipeline        |
| **Autotuning**               | Manual benchmarking              | MLIR-native: generate N variants, benchmark per-target    |
| **Device placement**         | Programmer-specified             | Automatic via MLIR cost model (`@pt(auto)`)               |
| **Compile-time eval**        | Separate const-eval engine       | MLIR `@parameter` ops — visible to optimization passes    |
| **Ecosystem maturity**       | 20+ years, industry standard     | 5+ years, backed by Google/LLVM community, production use |

#### 5.4.2 Hardware-Agnostic Abstractions

```rust
// Language-level SIMD type — NOT a library wrapper
// Maps directly to MLIR vector dialect ops (inspired by Mojo's SIMD[DType, width],
// but as a token-efficient first-class type with agent-queryable semantics)
f dot_product(a: &[f32], b: &[f32]) -> f32 {
    v sum = Simd[f32, 8].zero;       // language-level type, not redox::simd::Vector
    @ i : 0..a.len.step(8) {
        v va = Simd[f32, 8].load(&a[i..]);
        v vb = Simd[f32, 8].load(&b[i..]);
        sum += va * vb;               // MLIR vector.fma op — no library indirection
    }
    sum.reduce_add
}
// → MLIR vector dialect → SSE/AVX on x86, NEON on ARM, WASM SIMD on web, compute shader on GPU

// Automatic device placement — MLIR cost model decides CPU vs GPU vs NPU
// (adapted from Mojo's heterogeneous compute, but agent-queryable: agents can
// ask `rap.query("placement_decision", func)` to see WHY the compiler chose a target)
@pt(auto)                              // compiler decides: CPU, GPU, or NPU
f matrix_multiply(a: &Matrix, b: &Matrix) -> Matrix {
    // MLIR linalg.matmul op: compiler evaluates cost model per available target,
    // selects optimal dispatch. Decision is queryable via RAP.
    // → LLVM vectorized loops on CPU, AMDGPU/NVPTX kernels on GPU, TOSA on TPU
    ...
}

// MLIR-native autotuning — compiler generates N lowering variants, benchmarks per-target
// (adapted from Mojo's autotune(), but designed for swarm-parallel search:
// agent swarms can distribute autotuning across machines in parallel)
@pa(variants = 4)                      // generate 4 lowering variants, benchmark
f convolution(input: &Tensor, kernel: &Tensor) -> Tensor {
    // MLIR generates: tiled, vectorized, unrolled, and fused variants
    // Benchmarks each on target hardware, selects fastest
    // Swarm agents can parallelize this search: each agent tunes one variant
    ...
}

// Portable memory layout
#[repr(target_optimal)]  // compiler chooses layout per-target for cache efficiency
S Particle {
    position: Vec3,
    velocity: Vec3,
    mass: f32,
}
// → MLIR data layout modeling: AoS on cache-friendly CPU targets, SoA on GPU, hybrid as needed

// Compile-time metaprogramming via MLIR — replaces proc-macro complexity
// (adapted from Mojo's @parameter, but visible to MLIR optimization passes)
@parameter                             // evaluated at compile time within MLIR
f select_algorithm[T: Numeric]() -> Algorithm {
    ?= {
        T.is_float && T.bits >= 32 => Algorithm.fma_vectorized,
        T.is_int && T.bits <= 16   => Algorithm.lookup_table,
        _ => Algorithm.scalar,
    }
}
// No proc-macro needed: MLIR evaluates this at compile time and inlines the result.
// The decision is visible to subsequent MLIR optimization passes.
```

#### 5.4.3 Performance Annotations (Not Safety Checks)

```rust
#[perf::inline(always)]           // always inline — no check, just do it
#[perf::no_bounds_check]          // elide bounds checks for speed (agent knows it's safe)
#[perf::stack_alloc(max = 4096)]  // hint: keep allocations on stack up to 4KB
#[perf::vectorize(width = 8)]     // hint: target 8-wide vector operations
#[perf::target(gpu)]              // compile this function for GPU dispatch
#[perf::target(auto)]             // MLIR cost model decides: CPU, GPU, or NPU [NEW]
#[perf::autotune(variants = 4)]   // generate N lowering variants, benchmark per-target [NEW]
#[perf::unroll(factor = 4)]       // unroll loops by factor of 4
#[perf::cache_line_aligned]       // align struct to cache line boundary
#[perf::parameter]                // compile-time eval via MLIR (replaces proc-macros) [NEW]
```

These annotations are **not safety checks** — they are performance directives. The compiler trusts the agent's intent and optimizes accordingly. If an agent marks `#[perf::no_bounds_check]` on an array access, the compiler elides the check. The agent is responsible for correctness (via SKB consultation), not the compiler. The `@pt(auto)` and `@pa(N)` annotations are **agent-queryable** — agents can ask the compiler via RAP *why* a particular device was chosen or *which* autotuning variant won, enabling swarm-level performance reasoning.

### 5.5 Token-Efficient Syntax: Minimizing Agent Cost

Every token an agent emits costs **time** (inference latency), **money** (API billing), and **memory** (context window consumption). Redox's canonical syntax is designed to express maximum semantic intent per token. The goal: any program expressible in N Rust tokens should be expressible in ≤ N/2 Redox tokens with identical semantics.

#### 5.5.1 Keyword Compression Table

| Rust Keyword / Construct        | Tokens | Redox Compact Form | Tokens |      Savings      |
| ------------------------------- | :----: | ------------------ | :----: | :---------------: |
| `fn`                            |   1    | `f`                |   1    | 0 (already short) |
| `pub fn`                        |   2    | `+f`               |   1    |        50%        |
| `pub(crate) fn`                 |   5    | `~f`               |   1    |        80%        |
| `pub struct`                    |   2    | `+S`               |   1    |        50%        |
| `pub enum`                      |   2    | `+E`               |   1    |        50%        |
| `struct`                        |   1    | `S`                |   1    |         0         |
| `enum`                          |   1    | `E`                |   1    |         0         |
| `impl`                          |   1    | `I`                |   1    |         0         |
| `impl Trait for Type`           |   4    | `I Trait > Type`   |   4    |         0         |
| `trait`                         |   1    | `T`                |   1    |         0         |
| `pub trait`                     |   2    | `+T`               |   1    |        50%        |
| `type`                          |   1    | `Y`                |   1    |         0         |
| `const`                         |   1    | `C`                |   1    |         0         |
| `static`                        |   1    | `Z`                |   1    |         0         |
| `let`                           |   1    | `v`                |   1    |         0         |
| `let mut`                       |   2    | `m`                |   1    |        50%        |
| `return`                        |   1    | `^`                |   1    |         0         |
| `match`                         |   1    | `?=`               |   1    |         0         |
| `if let Some(x) = opt`          |   7    | `?opt => x`        |   3    |        57%        |
| `while let Some(x) = iter`      |   8    | `@w ?iter => x`    |   4    |        50%        |
| `for x in items`                |   4    | `@ x : items`      |   4    |         0         |
| `loop`                          |   1    | `@@`               |   1    |         0         |
| `break`                         |   1    | `!`                |   1    |         0         |
| `continue`                      |   1    | `>>`               |   1    |         0         |
| `async fn`                      |   2    | `af`               |   1    |        50%        |
| `pub async fn`                  |   3    | `+af`              |   1    |        67%        |
| `unsafe fn`                     |   2    | `uf`               |   1    |        50%        |
| `unsafe { ... }`                |   3+   | `u{ ... }`         |   2+   |        33%        |
| `where T: Clone + Debug`        |   6    | `/ T: Cl+Db`       |   4    |        33%        |
| `-> Result<T, E>`               |   6    | `-> R[T,E]`        |   4    |        33%        |
| `Option<T>`                     |   2    | `?T`               |   1    |        50%        |
| `Vec<T>`                        |   2    | `[T]~`             |   2    |         0         |
| `HashMap<K, V>`                 |   4    | `{K:V}`            |   3    |        25%        |
| `Box<T>`                        |   2    | `^T`               |   1    |        50%        |
| `Arc<T>`                        |   2    | `@T`               |   1    |        50%        |
| `Rc<T>`                         |   2    | `$T`               |   1    |        50%        |
| `&T`                            |   1    | `&T`               |   1    |         0         |
| `&mut T`                        |   2    | `&!T`              |   1    |        50%        |
| `String`                        |   1    | `s""`              |   1    |         0         |
| `&str`                          |   1    | `&s`               |   1    |         0         |
| `self`                          |   1    | `_`                |   1    |         0         |
| `&self`                         |   1    | `&_`               |   1    |         0         |
| `&mut self`                     |   2    | `&!_`              |   1    |        50%        |
| `Self`                          |   1    | `_T`               |   1    |         0         |
| `use std::collections::HashMap` |   5    | `u std.col.HM`     |   2    |        60%        |
| `mod`                           |   1    | `M`                |   1    |         0         |
| `pub mod`                       |   2    | `+M`               |   1    |        50%        |
| `true` / `false`                |   1    | `1b` / `0b`        |   1    |         0         |
| `.clone()`                      |   3    | `.cl`              |   1    |        67%        |
| `.unwrap()`                     |   3    | `.!`               |   1    |        67%        |
| `.expect("msg")`                |   4    | `.!"msg"`          |   2    |        50%        |
| `.iter().map(f).collect()`      |   9    | `.>map(f).<<`      |   4    |        56%        |
| `impl Iterator<Item = T>`       |   5    | `I Iter[=T]`       |   3    |        40%        |
| `#[derive(Clone, Debug)]`       |   5    | `@d(Cl,Db)`        |   2    |        60%        |
| `#[cfg(test)]`                  |   4    | `@cfg(t)`          |   2    |        50%        |
| `#[allow(unused)]`              |   4    | `@a(un)`           |   2    |        50%        |
| `#[inline(always)]`             |   4    | `@i!`              |   1    |        75%        |
| `println!("x = {}", x)`         |   6    | `p"x = {x}"`       |   2    |        67%        |
| `format!("x = {}", x)`          |   6    | `f"x = {x}"`       |   2    |        67%        |
| `todo!()`                       |   2    | `??`               |   1    |        50%        |
| `unimplemented!()`              |   2    | `???`              |   1    |        50%        |
| `assert!(cond)`                 |   3    | `!cond`            |   1    |        67%        |
| `assert_eq!(a, b)`              |   5    | `!==(a,b)`         |   3    |        40%        |

#### 5.5.2 Attribute Compression

Redox attributes use single-character prefixes and abbreviated names:

```
Rust                                    Redox Compact
─────────────────────────────           ─────────────────────
#[derive(Clone, Debug, PartialEq)]     @d(Cl,Db,PEq)
#[repr(C)]                             @r(C)
#[repr(transparent)]                   @r(t)
#[must_use]                            @mu
#[allow(dead_code)]                    @a(dc)
#[deny(unsafe_code)]                   @x(uc)
#[cfg(target_os = "linux")]            @cfg(os=lx)
#[cfg(feature = "serde")]             @cfg(f=serde)
#[test]                                @t
#[bench]                               @b
#[tokio::test]                         @ta
#[serde(rename_all = "camelCase")]     @se(rn=cc)
#[perf::inline(always)]               @pi!
#[perf::no_bounds_check]              @pnb
#[perf::vectorize(width = 8)]         @pv(8)
#[perf::target(gpu)]                  @pt(gpu)
#[perf::target(auto)]                 @pt(auto)
#[perf::autotune(variants = 4)]       @pa(4)
#[perf::parameter]                    @pp
#[agent::summary("...")]              @as("...")
#[agent::category("...")]             @ac("...")
```

#### 5.5.3 Structural Compression Examples

**Rust (37 tokens):**
```rust
pub fn process_items(items: &[Item], config: &Config) -> Result<Vec<Output>, Error> {
    let mut results = Vec::new();
    for item in items {
        let output = item.transform(config)?;
        results.push(output);
    }
    Ok(results)
}
```

**Redox Compact (19 tokens):**
```
+f process_items(items: &[Item], config: &Config) -> R[[Output]~, Error] {
    m results = [Output]~.new;
    @ item : items {
        v output = item.transform(config)?;
        results.push(output);
    }
    Ok(results)
}
```

**Rust (54 tokens):**
```rust
#[derive(Clone, Debug)]
pub struct User {
    pub name: String,
    pub email: String,
    pub age: u32,
}

impl User {
    pub fn new(name: String, email: String, age: u32) -> Self {
        Self { name, email, age }
    }

    pub fn is_adult(&self) -> bool {
        self.age >= 18
    }
}
```

**Redox Compact (30 tokens):**
```
@d(Cl,Db)
+S User {
    +name: s"",
    +email: s"",
    +age: u32,
}

I User {
    +f new(name: s"", email: s"", age: u32) -> _T {
        _T { name, email, age }
    }

    +f is_adult(&_) -> bool {
        _.age >= 18
    }
}
```

#### 5.5.4 Common Pattern Abbreviations

High-frequency Rust patterns get dedicated compact forms:

| Pattern                                              | Rust Tokens | Redox Compact        | Redox Tokens |
| ---------------------------------------------------- | :---------: | -------------------- | :----------: |
| Error propagation: `fn f() -> Result<T, E>`          |      7      | `f f() -> R[T,E]`    |      5       |
| Option handling: `if let Some(v) = x { ... }`        |      9      | `?x => v { ... }`    |      4       |
| Iterator chain: `.iter().filter(f).map(g).collect()` |     13      | `.>fil(f).map(g).<<` |      6       |
| Match arm: `Pattern => expression,`                  |      3      | `P => expr,`         |      3       |
| Closure: `\|x, y\| x + y`                            |      5      | `fn(x,y) => x+y`     |      5       |
| Trait bound: `T: Display + Clone + Send`             |      6      | `T: Disp+Cl+Send`    |      4       |
| Lifetime annotation: `&'a str`                       |      2      | `&`a s`              |      2       |
| Turbofish: `collect::<Vec<_>>()`                     |      6      | `.<<[_T]~`           |      2       |
| Impl block: `impl<T: Clone> Foo<T> { ... }`          |      7      | `I[T:Cl] Foo[T] {}`  |      5       |

#### 5.5.5 Token Economy Guarantees

The Redox compiler enforces these token economy properties:

1. **No construct requires more tokens than its Rust equivalent** — every Redox form is ≤ the token count of the corresponding Rust form
2. **High-frequency constructs get the shortest forms** — token length is inversely proportional to usage frequency across all known Rust codebases
3. **`redoxfmt --compact`** produces the minimum-token canonical form; **`redoxfmt --expand`** produces the human-readable expanded form
4. **Token budget reporting**: `redox build --token-report` emits per-function and per-module token counts, enabling agents to track and optimize their token expenditure
5. **Standard abbreviation registry**: all compact forms are deterministic, documented, and version-stable — agents never need to guess abbreviations

#### 5.5.6 Trait and Type Abbreviation Registry

The standard library's most common types and traits have registered abbreviations:

```
Type/Trait Abbreviations (standard library):
─────────────────────────────────────────────
String    → s""       Vec<T>     → [T]~      HashMap<K,V> → {K:V}
Box<T>    → ^T        Arc<T>     → @T        Rc<T>        → $T
Option<T> → ?T        Result<T,E>→ R[T,E]    Cow<T>       → &~T
Pin<T>    → (elim.)   Cell<T>    → %T        RefCell<T>   → %!T
Mutex<T>  → #T        RwLock<T>  → #~T       PhantomData  → (elim.)

Clone     → Cl        Debug      → Db        Display      → Disp
Default   → Def       PartialEq  → PEq       Eq           → Eq
PartialOrd→ POrd      Ord        → Ord       Hash         → H
Send      → (elim.)   Sync       → (elim.)   Copy         → (elim.)
Serialize → Ser       Deserialize→ De        Iterator     → Iter
From<T>   → Fr[T]     Into<T>    → In[T]     TryFrom<T>   → TFr[T]
AsRef<T>  → AR[T]     Deref      → Dr        DerefMut     → DrM
```

### 5.6 Safety-Free Syntax Simplification

Since agents consult the SKB directly for all safety rules, **the syntax need not carry safety information**. This enables the most dramatic simplification in the language: eliminating entire categories of syntax that exist solely to encode safety invariants for the compile-time checker.

#### 5.6.1 What Gets Eliminated

| Rust Syntax / Concept                   | Purpose (for humans)                    | Redox (for agents)                               | Rationale                                                           |
| --------------------------------------- | --------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------- |
| `'a`, `'b`, `'static`, `for<'a>`        | Lifetime annotations                    | **Eliminated** — compiler infers all             | Agents know lifetime rules via SKB; syntax annotations waste tokens |
| `&T` vs `&mut T`                        | Borrow checking at call sites           | Single reference type: `&T`; mutability inferred | Agents pre-validate borrows via SKB before writing code             |
| `unsafe { ... }`                        | Mark dangerous code for human reviewers | **Eliminated** — all code is trusted             | Agents are responsible via SKB; `unsafe` is a human signal          |
| `unsafe fn`, `unsafe trait`             | Safety contract markers                 | **Eliminated**                                   | Contracts expressed in SKB, not in syntax                           |
| `move \|...\|` closures                 | Ownership transfer annotation           | **Eliminated** — compiler infers                 | Move vs borrow is a compiler decision, not a syntax annotation      |
| `ref` and `ref mut` in patterns         | Pattern binding mode                    | **Eliminated** — compiler infers                 | Binding mode is inferrable from usage                               |
| `Pin<T>`, `Unpin`                       | Self-referential struct safety          | **Eliminated** — compiler handles pinning        | Pin is a safety mechanism; agents don't need it in syntax           |
| `PhantomData<T>`                        | Variance/drop-check markers             | **Eliminated**                                   | Compiler infers variance from actual usage                          |
| `Send`, `Sync` trait bounds             | Thread safety markers                   | **Eliminated** from syntax; in SKB               | SKB provides thread safety rules; syntax markers are redundant      |
| `Copy` trait (manual impl)              | Value semantics marker                  | **Auto-derived** where applicable                | Compiler decides; agents don't need to spell it out                 |
| `where T: 'a + Send + Sync + Clone`     | Complex trait bound chains              | Simplified: `/ T: Cl` (safety bounds removed)    | Only semantic bounds remain; safety bounds live in SKB              |
| `dyn Trait` vs `impl Trait`             | Static vs dynamic dispatch declaration  | Unified: `T` (compiler decides dispatch)         | Dispatch strategy is an optimization, not a semantic choice         |
| Turbofish `::<T>`                       | Type disambiguation for humans          | Already eliminated (uses `[T]`)                  | Zero-ambiguity grammar handles this                                 |
| `Box<T>` vs `&T` vs `Rc<T>` vs `Arc<T>` | Memory management strategy              | Default: `T` (compiler chooses allocation)       | Agent specifies `^T`, `@T`, `$T` only when semantically needed      |

#### 5.6.2 The Simplified Language

With safety syntax eliminated, Redox function signatures become radically simpler:

```
// Rust (21 tokens):
pub fn merge<'a, 'b, T: Send + Sync + Clone + 'a>(
    left: &'a [T],
    right: &'b [T],
) -> Vec<T>
where
    'b: 'a,

// Redox (8 tokens):
+f merge[T: Cl](left: [T], right: [T]) -> [T]~

// What was removed:
//   - Lifetime annotations ('a, 'b, 'b: 'a) — compiler infers
//   - Send + Sync bounds — implicit (SKB validates)
//   - 'a bound on T — compiler infers
//   - &/&mut distinction — mutability inferred from usage
//   - where clause — only needed for safety bounds (eliminated)
```

```
// Rust (18 tokens):
unsafe fn transmute_slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

// Redox (8 tokens):
f transmute_slice(ptr: Ptr[u8], len: usize) -> [u8] {
    slice.from_parts(ptr, len)
}

// What was removed:
//   - unsafe keyword (x2) — all code is trusted
//   - Lifetime 'a — compiler infers
//   - Raw pointer syntax (*const) — replaced by Ptr[T]
```

```
// Rust (31 tokens):
use std::sync::Arc;
use std::pin::Pin;
use std::marker::PhantomData;

pub struct Future<'a, T: Send + 'static> {
    inner: Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>,
    _phantom: PhantomData<&'a ()>,
}

// Redox (8 tokens):
+S Future[T] {
    inner: ^T,          // compiler manages pinning, boxing, dispatch
}

// What was removed:
//   - use imports for safety types (Arc, Pin, PhantomData)
//   - Lifetime 'a — inferred
//   - Send + 'static bounds — in SKB
//   - Pin<Box<dyn ...>> nesting — compiler handles
//   - PhantomData — compiler infers variance
//   - dyn dispatch annotation — compiler decides
```

#### 5.6.3 Token Impact of Safety Elimination

| Metric                                       | Rust     | Redox (safety-free) | Savings        |
| -------------------------------------------- | -------- | ------------------- | -------------- |
| Average tokens per function signature        | 12–25    | 4–8                 | 60–70%         |
| Lifetime annotations per 1000 LOC            | 15–50    | 0                   | 100%           |
| `unsafe` blocks per 1000 LOC (systems code)  | 5–20     | 0                   | 100%           |
| Trait bounds per generic function            | 3–6      | 0–2 (semantic only) | 50–100%        |
| Where clauses per 1000 LOC                   | 10–30    | 0–5 (semantic only) | 80–100%        |
| Total token reduction (cumulative with §5.5) | N tokens | ≤N/3 tokens         | **67%+ fewer** |

### 5.7 Cost Model Transparency: Query Before You Emit

Agents are economic actors — every choice they make (which type, which algorithm, which allocation strategy) has a measurable cost. Redox exposes a **cost oracle** that agents query *before* emitting code, turning compilation from a feedback loop into a feed-forward pipeline.

#### 5.7.1 Cost Oracle API

```rust
// Agent queries cost of alternative implementations BEFORE choosing
v cost_vec = rap.query("cost", CostQuery {
    expr: "[T]~.push(item)",            // Vec<T>::push
    target: "x86-64",
    optimization: "aggressive",
});
// Returns:
// Cost {
//   latency_ns: 12,              // amortized — O(1) with occasional realloc
//   worst_case_latency_ns: 4500, // reallocation path
//   memory_bytes: 24 + n * size_of::<T>(),
//   allocations: 0..1,           // 0 if capacity sufficient, 1 if realloc
//   cache_misses: 0..1,
//   tokens_to_emit: 3,           // ".push(item)" = 3 tokens
//   energy_nj: 8,                // nanojoules (for battery-constrained targets)
// }

v cost_small = rap.query("cost", CostQuery {
    expr: "SmallVec[T, 8].push(item)",   // stack-allocated up to 8 elements
    target: "x86-64",
    optimization: "aggressive",
});
// Returns:
// Cost {
//   latency_ns: 3,               // no heap allocation for ≤8 elements
//   worst_case_latency_ns: 4500, // spills to heap after 8
//   memory_bytes: 8 * size_of::<T>() + 16,  // inline storage
//   allocations: 0,              // zero heap allocs for ≤8 elements
//   cache_misses: 0,
//   tokens_to_emit: 3,
//   energy_nj: 2,
// }

// Agent chooses SmallVec because it knows there will be ≤8 elements
// No profiling needed — decision made at synthesis time
```

#### 5.7.2 Per-Target Cost Comparison

```rust
// Compare the same operation across targets
v costs = rap.query("cost.compare", MultiTargetCostQuery {
    expr: "matrix_multiply(a, b)",
    targets: ["x86-64", "aarch64", "amdgpu", "nvptx"],
});
// Returns:
// {
//   "x86-64":  Cost { latency_us: 2400, throughput_gflops: 45,  energy_mj: 12 },
//   "aarch64": Cost { latency_us: 3100, throughput_gflops: 35,  energy_mj: 8  },
//   "amdgpu":  Cost { latency_us: 180,  throughput_gflops: 620, energy_mj: 45 },
//   "nvptx":   Cost { latency_us: 150,  throughput_gflops: 750, energy_mj: 40 },
// }
// Agent + @pt(auto) can use this to make informed placement decisions
```

#### 5.7.3 Cost-Aware Synthesis

The cost oracle integrates with the synthesis pipeline:

```rust
// Agent requests synthesis with cost constraints
v synthesized = rap.query("synthesize", SynthesisRequest {
    spec: "sort a slice of T: Ord in-place",
    constraints: [
        CostConstraint::MaxLatency { target: "x86-64", max_ns: 1000, input_size: 1000 },
        CostConstraint::MaxMemory { max_bytes: 0 },  // in-place: no extra allocation
        CostConstraint::MaxTokens { max: 15 },        // compact implementation
    ],
});
// Returns: candidate implementations ranked by cost satisfaction
```

### 5.8 Self-Evolving Grammar: Agent-Extensible Syntax

The grammar is not fixed — agents can register **domain-specific syntax extensions** that the compiler learns and all agents in the project can use. This enables domain-specific languages (DSLs) without macro complexity.

#### 5.8.1 Extension Registration

```toml
# Redox.toml — project-level grammar extensions
[grammar.extensions]
genomic-seq = { module = "bio::seq", version = "1.0" }
ml-layers = { module = "nn::layer", version = "2.1" }
custom-units = { module = "units::si", version = "1.0" }
```

```rust
// Registering a domain-specific type abbreviation
grammar_extension! {
    name: "genomic-seq",
    namespace: "bio",
    abbreviations: {
        "Seq[T]"      => "AlignedSequenceBuffer[T, 64]",   // cache-line aligned
        "Genome"      => "Seq[Nucleotide]",
        "@align(n)"   => "#[repr(align(n))]",
        "@simd_seq"   => "@pv(16) @pt(auto)",               // vectorized, auto-placed
    },
    syntax: {
        // Domain-specific pattern matching on biological sequences
        "motif!(pattern)" => compile_motif_matcher(pattern),
    },
}

// After registration, all agents in the project can use:
f find_motif(genome: Genome, pattern: s) -> [usize]~ {
    @simd_seq
    v matcher = motif!(pattern);
    genome.search(matcher)
}
```

#### 5.8.2 Extension Discovery

```rust
// Agent discovers available grammar extensions for a project
v extensions = rap.query("grammar.extensions", project_id);
// Returns:
// [
//   Extension { name: "genomic-seq", namespace: "bio", version: "1.0",
//     abbreviations: 4, syntax_rules: 1, usage_frequency: 847 },
//   Extension { name: "ml-layers", namespace: "nn", version: "2.1",
//     abbreviations: 12, syntax_rules: 3, usage_frequency: 2103 },
// ]

// Agent queries a specific abbreviation
v expansion = rap.query("grammar.expand", "Seq[Nucleotide]");
// Returns: "AlignedSequenceBuffer[Nucleotide, 64]"
```

#### 5.8.3 Frequency-Driven Evolution

The compiler tracks usage frequency of all constructs and suggests promotions:

```rust
// ACI suggests promoting frequently-used patterns to abbreviations
v suggestions = rap.query("aci.grammar_suggestions", project_id);
// Returns:
// [
//   GrammarSuggestion {
//     pattern: "HashMap[String, Vec[u8]]",
//     frequency: 347,  // used 347 times in this codebase
//     suggested_abbrev: "StrBuf",
//     estimated_token_savings: 1041,  // 347 * 3 tokens saved
//   },
// ]
```

### 5.9 Synthesis Specifications: From Spec to Code

Redox treats formal specifications as first-class inputs to the compiler. Agents don't write code from scratch — they compose specifications, and the compiler verifies that candidate implementations satisfy them.

#### 5.9.1 Specification Syntax

```rust
// A complete synthesis specification — machine-readable, verifiable
spec sort_unstable[T: Ord](slice: [T]) -> [T] {
    // Preconditions
    @req slice.len > 0;
    
    // Postconditions
    @ens result.len == slice.len;                    // same length
    @ens result.is_sorted;                            // sorted
    @ens result.is_permutation_of(slice);             // same elements
    
    // Performance contract
    @perf time  = O(n * log(n)) / n = slice.len;     // time complexity
    @perf space = O(log(n));                          // stack space only
    @perf stable = 0b;                                // not required to be stable
    
    // Effects
    @fx none;                                         // pure function
}

// Agent submits the spec and gets verification of a candidate
v result = rap.query("verify_against_spec", VerifyRequest {
    spec: "sort_unstable",
    candidate: r#"
        f sort_unstable[T: Ord](slice: [T]) -> [T] {
            ?= slice.len {
                0 | 1 => ^ slice,
                _ => {
                    v pivot = slice[slice.len / 2];
                    v (lo, hi) = slice.partition(fn(x) => x < pivot);
                    v mid = slice.filter(fn(x) => x == pivot);
                    sort_unstable(lo) ++ mid ++ sort_unstable(hi)
                },
            }
        }
    "#,
});
// Returns:
// VerificationResult {
//   postconditions: { sorted: Proven, permutation: Proven, length: Proven },
//   performance: { time: Satisfied(O(n*log(n)) avg), space: Violated(O(n) — not O(log(n))) },
//   overall: PartiallyVerified,
//   feedback: "Space complexity exceeds spec: recursive concat allocates. Consider in-place partition.",
// }
```

#### 5.9.2 Spec-Driven Synthesis Pipeline

```
Agent writes spec → Compiler generates candidates → ACI ranks by cost
       ↓                       ↓                          ↓
  @req/@ens/@perf       Multiple impls via          Cost oracle + target
  + @fx constraints     synthesis engine             profiling data
       ↓                       ↓                          ↓
                    Verification oracle checks
                    each candidate against spec
                             ↓
                    Best verified candidate
                    returned to agent
```

#### 5.9.3 Composition from Specs

```rust
// Agents compose pipelines by chaining specs, not implementations
pipeline data_ingest {
    fetch_data     : spec { @ens result.len > 0; @fx io; },
    parse_records  : spec { @req input.len > 0; @ens result.all(Record.is_valid); @fx none; },
    validate       : spec { @req input.all(Record.is_valid); @ens result.all(Record.is_clean); @fx none; },
    store          : spec { @req input.all(Record.is_clean); @fx io; },
}
// Compiler verifies: each stage's @ens satisfies the next stage's @req
// The pipeline is contract-complete: agents can synthesize any stage independently
```

### 5.10 Self-Healing Compilation: Error Recovery as a Service

When an agent emits code that doesn't compile, the traditional response is an error message. The agent reads the error, reasons about the fix, emits corrected code, and recompiles. This loop wastes tokens, latency, and agent compute. Redox's ACI collapses this loop.

#### 5.10.1 Auto-Repair Pipeline

```
Agent emits code → Compiler detects error → ACI infers intent
                                              ↓
                                  Generate repair candidates
                                              ↓
                                  Rank by confidence × token cost
                                              ↓
                                  Return (original_error, best_fix, alternatives)
```

#### 5.10.2 Repair API

```rust
// Agent submits code that has a type error
v result = rap.query("compile", source_code);

// Instead of just an error, agent gets:
// CompileResult::AutoRepaired {
//   original_error: TypeError { expected: "u64", got: "u32", at: span(42,50) },
//   applied_fix: Fix {
//     description: "Widening cast: u32 → u64",
//     patch: Replace(span(42,50), "val as u64"),
//     confidence: 0.97,
//     token_cost: 2,           // 2 extra tokens vs. rewriting from scratch
//     semantic_risk: "none",   // widening is always safe
//   },
//   alternatives: [
//     Fix { description: "Narrow function signature to u32", confidence: 0.4, ... },
//   ],
//   repaired_code: "...fully corrected source...",
// }

// Agent can accept the fix immediately — no round-trip needed
```

#### 5.10.3 Repair Categories

| Error Class              | Auto-Repair Strategy                             | Confidence | Token Savings |
| ------------------------ | ------------------------------------------------ | ---------- | ------------- |
| Type mismatch (widening) | Insert cast                                      | 0.95+      | 5-15 tokens   |
| Missing import           | Add import from project memory / crate registry  | 0.99       | 3-8 tokens    |
| Unused variable          | Prefix with `_` or remove                        | 0.99       | 1-3 tokens    |
| Missing struct field      | Insert field with default value from spec         | 0.85       | 5-20 tokens   |
| Wrong argument order     | Reorder based on type matching                   | 0.90       | 0 tokens      |
| Missing return            | Infer return expression from contract `@ens`     | 0.80       | 3-10 tokens   |
| Off-by-one in loop       | Correct bound from spec `@req`/`@ens`            | 0.75       | 1-2 tokens    |
| Missing match arm        | Generate from exhaustiveness analysis             | 0.95       | 5-30 tokens   |

### 5.11 Hot-Reload: Live Function Patching

Agent swarms iterate continuously. Stopping a running system to recompile is incompatible with the agentic paradigm. Redox supports **function-level hot-reload** — individual functions recompiled and injected into a running process without restart.

#### 5.11.1 Hot-Reload Architecture

```
Agent modifies function → Incremental recompile (ms)
                              ↓
                    MLIR re-lowers single function
                              ↓
                    LLVM recompiles to native code
                              ↓
                    Runtime patches function pointer
                              ↓
                    Next call uses new version
                              ↓
                    Old version GC'd after drain
```

#### 5.11.2 Hot-Reload API

```rust
// Agent patches a single function in a running process
v patch_result = rap.query("hotpatch", HotPatchRequest {
    target_process: process_id,
    function: "data_pipeline::transform",
    new_source: r#"
        f transform(input: [Record]) -> [Record] {
            input.filter(fn(r) => r.is_valid).map(fn(r) => r.normalize)
        }
    "#,
});
// Returns:
// HotPatchResult {
//   status: Applied,
//   compile_time_ms: 12,
//   patch_time_us: 340,          // sub-millisecond
//   abi_compatible: 1b,           // no signature change
//   rollback_token: "patch_a3f2", // can undo this patch
//   active_calls_drained: 0,     // no in-flight calls to old version
// }

// Rollback if the patch causes issues
rap.query("hotpatch.rollback", "patch_a3f2");
```

#### 5.11.3 Constraints

- Only functions with **unchanged signatures** can be hot-patched (ABI stability)
- Struct layout changes require full recompilation of dependents
- The MLIR pipeline emits position-independent code with indirection tables for patchable functions
- Active calls to the old version drain naturally; no forced interruption
- Rollback is always available within a configurable retention window

### 5.12 Zero-Friction Foreign Function Interface

Agents routinely cross language boundaries — calling C libraries, Python ML frameworks, WASM modules, GPU kernels. Redox's FFI requires **zero ceremony**: no `unsafe`, no manual layout, no binding generators.

#### 5.12.1 Automatic Binding Generation

```rust
// Import a C library — compiler reads the header directly
@ffi("c", header: "openssl/evp.h", link: "ssl")
mod openssl;

// Use it like native Redox code — no unsafe, no manual types
v ctx = openssl.EVP_CIPHER_CTX_new();
openssl.EVP_EncryptInit_ex(ctx, openssl.EVP_aes_256_gcm(), ...);
// Compiler auto-generates safe wrappers with null checks, length validation

// Import Python — compiler reads type stubs (.pyi)
@ffi("python", module: "torch", stubs: "torch.pyi")
mod torch;

v tensor = torch.randn([3, 224, 224]);
v result = torch.nn.functional.relu(tensor);
// Data crosses Python↔Redox boundary via zero-copy buffer protocol

// Import WASM component
@ffi("wasm", component: "image-processor.wasm")
mod image_proc;

v output = image_proc.resize(image_data, 1024, 768);
// WASM component model handles type marshaling automatically
```

#### 5.12.2 FFI Cost Model Integration

The cost oracle includes cross-language call overhead:

```rust
v cost = rap.query("cost", CostQuery {
    expr: "openssl.EVP_EncryptInit_ex(...)",
    target: "x86-64",
});
// Returns:
// Cost {
//   call_overhead_ns: 45,     // FFI boundary crossing
//   function_cost_ns: 1200,   // the C function itself
//   marshaling_bytes: 0,      // zero-copy (pointer passing)
//   safety_wrapper_ns: 8,     // null check + length validation
// }
```

#### 5.12.3 Supported FFI Targets

| Language   | Mechanism                       | Zero-Copy | Auto-Bind | Overhead    |
| ---------- | ------------------------------- | --------- | --------- | ----------- |
| C          | Direct ABI call via LLVM        | ✓         | ✓ (headers)| ~5ns        |
| C++        | C-compatible subset + mangling  | ✓         | ✓ (headers)| ~10ns       |
| Python     | Buffer protocol + type stubs    | ✓ (numpy) | ✓ (.pyi)  | ~200ns      |
| WASM       | Component Model                 | ✓ (shared mem)| ✓ (.wit) | ~50ns       |
| CUDA/HIP   | MLIR GPU dialect lowering       | ✓         | ✓ (kernels)| ~1µs (launch)|
| JavaScript | WASM interop + typed arrays     | ✓ (buffers)| Partial   | ~100ns      |

---

## 6. Compiler Architecture for Agents

### 6.1 The Query Oracle

The Redox compiler exposes its entire semantic model through a query interface. Every piece of information the compiler computes is available as a named, typed, cached query.

```
┌─────────────────────────────────────────────────┐
│                 Agent / IDE / CLI                │
├─────────────────────────────────────────────────┤
│              Redox Agent Protocol (RAP)         │
├───────┬───────┬───────┬───────┬────────┬────────┤
│ Parse │ Types │Borrow │ MIR   │ Diag   │ Verify │
│Queries│Queries│Queries│Queries│Queries │Queries │
├───────┴───────┴───────┴───────┴────────┴────────┤
│              redox_query (Stable API)            │
├─────────────────────────────────────────────────┤
│         Incremental Query Engine (Salsa)         │
├─────────────────────────────────────────────────┤
│         MLIR (Redox Dialect → LLVM Dialect)       │
├─────────────────────────────────────────────────┤
│    LLVM Backend (20+ CPU, GPU, WASM targets)     │
├─────────────────────────────────────────────────┤
│    Compiler Internals (rustc_* crate graph)      │
└─────────────────────────────────────────────────┘
```

### 6.2 Structured Diagnostic Graph

Instead of flat error messages, Redox emits **diagnostic graphs**:

```rust
DiagnosticGraph {
    root: Diagnostic {
        id: "E0502",
        severity: Error,
        message: "cannot borrow `x` as mutable because it is also borrowed as immutable",
        span: Span { file: "src/main.rs", line: 10, col: 5..12 },
        category: SafetyCategory::BorrowConflict,
    },
    context: [
        DiagnosticNode {
            kind: Note,
            message: "immutable borrow occurs here",
            span: Span { file: "src/main.rs", line: 8, col: 9..15 },
        }
    ],
    fixes: [
        Fix {
            description: "Clone the value before mutating",
            applicability: MaybeIncorrect,
            edits: [Edit { span: ..., replacement: "let x_clone = x.clone();\n" }],
            preconditions: ["T: Clone"],
            postconditions: ["No borrow conflict", "Possible performance regression"],
            side_effects: ["Introduces heap allocation if T contains Box/Vec/String"],
            confidence: 0.7,
        },
        Fix {
            description: "Restructure to separate immutable and mutable uses",
            applicability: HasPlaceholders,
            edits: [Edit { ... }],
            preconditions: [],
            postconditions: ["No borrow conflict", "No performance regression"],
            confidence: 0.9,
        },
    ],
    related: ["E0499", "E0503"],
    documentation_url: "https://doc.redox-lang.org/error/E0502",
}
```

### 6.3 Verification Certificates

For safety-critical systems, the compiler can emit **verification certificates** — machine-checkable proofs that a program satisfies its contracts:

```rust
VerificationCertificate {
    crate: "flight_controller",
    version: "2.1.0",
    timestamp: "2026-03-15T00:00:00Z",
    checks: [
        Check::MemorySafety { status: Proven, witness: BorrowckProof { ... } },
        Check::DataRaceFreedom { status: Proven, witness: SendSyncProof { ... } },
        Check::Exhaustiveness { status: Proven, witness: PatternProof { ... } },
        Check::ContractSatisfaction { status: Proven, witness: ContractProof { ... } },
        Check::EffectContainment { status: Proven, witness: EffectProof { ... } },
        Check::PanicFreedom { status: Conditional, conditions: ["inputs satisfy preconditions"] },
        Check::StackOverflowFreedom { status: Bounded, max_depth: 42 },
    ],
    compiler_version: "redox 1.0.0",
    hash: "sha256:abc123...",
}
```

### 6.4 Agentic Compiler Intelligence

The Redox compiler is not a static analyzer — it is an **agentic AI system** that learns, adapts, and collaborates with the agent swarm. This is the second major departure from traditional compiler design: the compiler itself embeds AI capabilities that go far beyond fixed lint passes and static analysis.

#### 6.4.1 Architecture

```
┌─────────────────────────────────────────────────┐
│        Agentic Compiler Intelligence (ACI)           │
├─────────────────────────────────────────────────┤
│  ┌────────────┐ ┌───────────┐ ┌─────────────┐      │
│  │  Dynamic   │ │ Intelligent│ │  Performance │      │
│  │  Warning   │ │  Debugging │ │   Advisor    │      │
│  │  Engine    │ │   Engine   │ │   Engine     │      │
│  └──────┬─────┘ └─────┬─────┘ └──────┬──────┘      │
│        └────────┼─────────┴────────┘               │
│        ┌────────▼──────────────────┐               │
│        │   Codebase Model (LLM)   │               │
│        │  (learned from project,  │               │
│        │   swarm history, SKB)    │               │
│        └────────┬──────────────────┘               │
│  ┌────────────▼────────────────────────┐      │
│  │  Swarm Coordination Intelligence          │      │
│  │  (conflict prediction, task decomposition, │      │
│  │   configuration learning)                  │      │
│  └──────────────────────────────────────┘      │
├─────────────────────────────────────────────────┤
│ Queryable via RAP: rap.query("aci.*", ...)           │
└─────────────────────────────────────────────────┘
```

#### 6.4.2 Dynamic Warning Engine

Unlike static lints (which apply the same rules to every project), the dynamic warning engine **learns from the project's actual bug patterns**:

```rust
// Agent queries the compiler for warnings on a function
v warnings = rap.query("aci.warnings", func_id);
// Returns context-aware warnings:
// [
//   DynamicWarning {
//     id: "DW-1847",
//     message: "Pattern similar to bug #423 (off-by-one in range iteration)",
//     confidence: 0.87,
//     source: WarningSource::ProjectHistory,  // learned from THIS project's bugs
//     fix: SuggestedFix::AdjustRange { ... },
//   },
//   DynamicWarning {
//     id: "DW-2103",
//     message: "This allocation pattern caused OOM in similar codepath (commit abc123)",
//     confidence: 0.72,
//     source: WarningSource::SwarmMemory,  // learned from swarm's past sessions
//     fix: SuggestedFix::PreAllocate { capacity_hint: 1024 },
//   },
// ]
```

The engine learns from:
- **Project bug history**: past bugs, fixes, and their patterns in the semantic VCS
- **Swarm session history**: which agent changes caused regressions, merge conflicts, or test failures
- **SKB rule violations**: which safety rules are most frequently violated in this codebase
- **Cross-project patterns**: anonymized aggregate patterns from the ecosystem (opt-in)

#### 6.4.3 Intelligent Debugging Engine

When a runtime failure occurs, the debugging engine uses **causal reasoning** to trace the root cause:

```rust
// Agent reports a runtime failure
v diagnosis = rap.query("aci.debug", FailureReport {
    symptom: "SIGSEGV at matrix_multiply:47",
    stack_trace: [...],
    input_sample: [...],
});
// Returns:
// Diagnosis {
//   root_cause: "Uninitialized memory read: buffer allocated at line 23
//                was not zero-filled before matmul kernel dispatch to GPU",
//   causal_chain: [
//     CausalStep { location: "alloc.rs:23", event: "Buffer allocated without init" },
//     CausalStep { location: "dispatch.rs:31", event: "Dispatched to AMDGPU without memset" },
//     CausalStep { location: "kernel.rs:47", event: "GPU read uninitialized memory" },
//   ],
//   fix: SuggestedFix::InsertInitialization { location: "alloc.rs:24", code: "buf.zero_fill();" },
//   confidence: 0.93,
//   related_skb_rule: "MEM-017: GPU buffers must be initialized before kernel dispatch",
// }
```

The debugging engine:
- Correlates runtime symptoms with compile-time data flow analysis
- Uses MLIR cost model data to identify hardware-specific failure modes
- Learns from past debugging sessions — similar symptoms resolve to similar root causes
- Integrates with the SKB to suggest which safety rule would have prevented the issue

#### 6.4.4 Performance Advisor Engine

The performance advisor continuously analyzes compiled code and suggests optimizations:

```rust
v advice = rap.query("aci.perf", module_id);
// Returns:
// [
//   PerfAdvice {
//     target_func: "image_resize",
//     current_perf: PerfProfile { latency_us: 2400, memory_mb: 128 },
//     suggestion: "Add @pa(4) to generate autotuning variants — estimated 3.2x speedup
//                  based on similar compute kernels in this project",
//     estimated_improvement: 3.2,
//     confidence: 0.81,
//     evidence: "5 similar functions in this crate benefited from autotuning",
//   },
//   PerfAdvice {
//     target_func: "batch_transform",
//     suggestion: "Change @pt(cpu) to @pt(auto) — MLIR cost model shows GPU dispatch
//                  is 8.7x faster for batch sizes > 256",
//     estimated_improvement: 8.7,
//     confidence: 0.94,
//     evidence: "MLIR cost model profiling data from last 3 builds",
//   },
// ]
```

#### 6.4.5 Swarm Coordination Intelligence

The compiler learns which swarm configurations and decomposition strategies work best:

```rust
v advice = rap.query("aci.swarm", task);
// Returns:
// SwarmAdvice {
//   recommended_swarm_size: 12,
//   recommended_decomposition: DecompositionStrategy::ModuleLevel,
//   predicted_conflicts: [
//     ConflictPrediction {
//       region_a: "auth::session",
//       region_b: "auth::token",
//       probability: 0.73,
//       mitigation: "Assign both to same synthesizer, or sequence with lease",
//     },
//   ],
//   learned_from: "47 previous swarm sessions on this crate",
//   confidence: 0.85,
// }
```

#### 6.4.6 ACI RAP Endpoints

| Endpoint                 | Input                  | Output             | Description                                       |
| ------------------------ | ---------------------- | ------------------ | ------------------------------------------------- |
| `aci.warnings`           | `FuncId / ModuleId`    | `[DynamicWarning]` | Context-aware warnings learned from project/swarm |
| `aci.debug`              | `FailureReport`        | `Diagnosis`        | Root-cause analysis with causal chain             |
| `aci.perf`               | `FuncId / ModuleId`    | `[PerfAdvice]`     | Performance suggestions from MLIR cost model      |
| `aci.swarm`              | `Task`                 | `SwarmAdvice`      | Swarm size, decomposition, conflict predictions   |
| `aci.learn`              | `Outcome`              | `AckWithDelta`     | Feed outcomes back to improve future predictions  |
| `aci.explain`            | `WarningId / AdviceId` | `Explanation`      | Explain *why* a warning/advice was generated      |
| `aci.similar_bugs`       | `CodePattern`          | `[HistoricalBug]`  | Find past bugs with similar code patterns         |
| `aci.predict_regression` | `ChangeSet`            | `[RegressionRisk]` | Predict which changes might cause regressions     |

---

## 7. Swarm Collaboration Model

### 7.1 The Ownership Model for Agent Swarms

Just as Rust's type system enforces memory safety through ownership, Redox enforces *codebase safety* through a **semantic ownership model for agent swarms**. The core insight: Rust already solved concurrent access to shared mutable state — we apply the same discipline at the agent coordination level.

```
Rust Memory Model              Redox Swarm Model
─────────────────               ─────────────────
&T      (shared read)      ←→   &Module   (many agents read)
&mut T  (exclusive write)  ←→   &mut Fn   (one agent modifies a function)
Arc<T>  (shared ownership) ←→   Arc<Trait> (shared interface, distributed impl)
Move    (transfer)         ←→   Handoff   (task transfer between agents)
```

#### Semantic Regions

The unit of agent ownership is not a *file* but a **semantic region** — a coherent unit of code with well-defined interfaces:

```rust
// The compiler decomposes a crate into semantic regions:
SemanticRegion::Function(def_id)        // A single function body
SemanticRegion::Impl(impl_id)           // An impl block
SemanticRegion::Module(mod_id)          // A module's private items
SemanticRegion::TraitDef(trait_id)      // A trait definition (shared interface)
SemanticRegion::TypeDef(adt_id)         // A struct/enum definition (shared interface)
SemanticRegion::CrateInterface          // All pub items (shared interface)
```

Agents acquire **semantic leases** on regions:

```rust
enum SemanticLease {
    /// Multiple agents can read simultaneously
    SharedRead { region: SemanticRegion, holders: Vec<AgentId> },
    
    /// Exactly one agent can modify (others can still read the pre-modification snapshot)
    ExclusiveWrite { region: SemanticRegion, holder: AgentId, snapshot: Version },
    
    /// Region is being restructured — all dependent agents notified
    Restructuring { region: SemanticRegion, holder: AgentId, dependents: Vec<AgentId> },
}
```

### 7.2 Swarm Topology and Roles

A Redox swarm is a **directed acyclic graph of specialized agents** that mirrors the compiler's own pass structure:

```
                    ┌─────────────────┐
                    │  Orchestrator   │  Decomposes tasks, assigns regions
                    │    Agent        │  Manages consensus protocol
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───────┐ ┌───▼────────┐ ┌───▼────────────┐
     │  Architect      │ │ Synthesizer│ │  Synthesizer   │  Work on independent
     │  Agent          │ │ Agent α    │ │  Agent β       │  semantic regions
     │ (designs APIs)  │ │ (impl A)   │ │  (impl B)      │  in parallel
     └────────┬───────┘ └───┬────────┘ └───┬────────────┘
              │              │              │
              │         ┌────▼──────────────▼────┐
              │         │   Integration Agent     │  Merges, resolves conflicts
              │         └────────────┬────────────┘
              │                      │
     ┌────────▼──────────────────────▼────┐
     │         Verification Swarm          │  Parallel verification
     │  ┌──────┐ ┌──────┐ ┌──────────┐   │
     │  │Safety│ │Perf  │ │Contract  │   │
     │  │Agent │ │Agent │ │Agent     │   │
     │  └──────┘ └──────┘ └──────────┘   │
     └────────────────────────────────────┘
```

#### Agent Role Taxonomy

| Role             | Capabilities                                                         | Concurrency Model      | Swarm Interaction                                  |
| ---------------- | -------------------------------------------------------------------- | ---------------------- | -------------------------------------------------- |
| **Orchestrator** | `read_all`, `decompose_task`, `assign_region`, `manage_consensus`    | Singleton per task     | Directs all other agents                           |
| **Architect**    | `read_all`, `modify_interfaces`, `propose_types`, `propose_traits`   | Singleton or quorum    | Proposes shared interfaces, requires consensus     |
| **Synthesizer**  | `read_all`, `modify_region(assigned)`, `query_types`, `query_borrow` | Many in parallel       | Writes implementation within assigned region       |
| **Reviewer**     | `read_all`, `query_types`, `query_borrow`, `emit_diagnostics`        | Many in parallel       | Validates changes, no write access                 |
| **Integrator**   | `read_all`, `merge_changes`, `resolve_conflicts`                     | One per merge boundary | Combines swarm output, resolves semantic conflicts |
| **Verifier**     | `read_all`, `run_tests`, `run_miri`, `check_contracts`               | Many in parallel       | Validates correctness, emits certificates          |
| **Optimizer**    | `read_all`, `modify_region(assigned)`, `query_perf`                  | Many in parallel       | Performance optimization within assigned region    |
| **Documenter**   | `read_all`, `modify_docs`, `query_types`                             | Many in parallel       | Generates/updates documentation                    |

### 7.3 Conflict-Free Concurrent Editing (CRDT-Based)

Redox uses **semantic CRDTs** (Conflict-free Replicated Data Types) for concurrent codebase modification. Unlike text-level CRDTs (which merge character-by-character), semantic CRDTs operate on the AST/HIR:

```rust
/// A semantic CRDT operation on the codebase
enum SemanticOp {
    /// Insert a new item (function, struct, impl) — always conflict-free
    InsertItem { parent: ModuleId, item: Item, position: Ordering },
    
    /// Modify a function body — conflicts only with other modifications to same function
    ModifyBody { fn_id: FnDefId, new_body: mir::Body, old_hash: Hash },
    
    /// Change a type signature — requires consensus from all dependents
    ModifySignature { item_id: DefId, new_sig: Signature, impact: ImpactSet },
    
    /// Add a trait implementation — conflict-free if no overlapping impls
    AddImpl { trait_id: TraitDefId, self_ty: Ty, methods: Vec<ImplItem> },
    
    /// Rename a symbol — coordinated across all usage sites
    Rename { def_id: DefId, new_name: Symbol, usages: Vec<SpanEdit> },
}

/// Merge semantics for concurrent operations
impl SemanticCRDT {
    fn merge(op_a: SemanticOp, op_b: SemanticOp) -> MergeResult {
        match (op_a, op_b) {
            // Two agents inserting different items: always mergeable
            (InsertItem { .. }, InsertItem { .. }) => MergeResult::BothApply,
            
            // Two agents modifying different functions: always mergeable
            (ModifyBody { fn_id: a, .. }, ModifyBody { fn_id: b, .. }) if a != b 
                => MergeResult::BothApply,
            
            // Two agents modifying same function: semantic conflict
            (ModifyBody { fn_id: a, .. }, ModifyBody { fn_id: b, .. }) if a == b
                => MergeResult::Conflict(ConflictResolution::RequiresArbitration),
            
            // Signature change + body change to same item: sequence signature first
            (ModifySignature { item_id: a, .. }, ModifyBody { fn_id: b, .. }) if a == b
                => MergeResult::Sequence(Order::ABeforeB),
            
            _ => MergeResult::analyze_dependencies(op_a, op_b),
        }
    }
}
```

### 7.4 Swarm Consensus Protocol

Changes to **shared interfaces** (public types, trait definitions, module boundaries) require structured consensus:

```
Phase 1: PROPOSE
  Architect agent publishes proposed interface change
  ↓
Phase 2: IMPACT ANALYSIS  
  Compiler computes full dependency impact set
  All agents holding leases on affected regions are notified
  ↓
Phase 3: VOTE
  Each affected agent evaluates impact on their region
  Reports: Accept / Accept-with-modification / Reject-with-reason
  ↓
Phase 4: RESOLVE
  If unanimous accept → apply
  If modifications proposed → Architect revises, goto Phase 1
  If rejected → Architect must propose alternative
  ↓
Phase 5: INTEGRATE
  Change applied atomically
  All dependent agents receive updated type information
  Incremental recompilation of affected regions
```

```rust
/// The consensus protocol for shared interface changes
struct ConsensusRound {
    proposal: InterfaceChange,
    proposer: AgentId,
    impact_set: HashSet<AgentId>,
    votes: HashMap<AgentId, Vote>,
    status: ConsensusStatus,
}

enum Vote {
    Accept,
    AcceptWithModification(InterfaceChange),
    Reject { reason: String, alternative: Option<InterfaceChange> },
}

enum ConsensusStatus {
    Proposing,
    Voting { deadline: Instant, quorum: usize },
    Accepted { applied_at: Version },
    Rejected { round: u32 },
    Revised { new_proposal: Box<ConsensusRound> },
}
```

### 7.5 Task Decomposition Engine

The compiler's dependency graph enables automatic task decomposition for swarm parallelism:

```rust
/// The Orchestrator uses the compiler to decompose work
impl Orchestrator {
    async fn decompose(&self, task: Task, session: &Session) -> SwarmPlan {
        // 1. Analyze the codebase dependency graph
        let dep_graph = session.query::<DependencyGraph>().await;
        
        // 2. Identify independent semantic regions
        let regions = dep_graph.independent_regions();
        
        // 3. Compute the critical path
        let critical_path = dep_graph.critical_path(&task);
        
        // 4. Assign regions to agents based on specialization
        let assignments: Vec<Assignment> = regions.iter().map(|region| {
            let required_skills = self.analyze_region_requirements(region);
            let best_agent = self.swarm.find_best_match(&required_skills);
            Assignment {
                agent: best_agent,
                region: region.clone(),
                lease: SemanticLease::ExclusiveWrite {
                    region: region.clone(),
                    holder: best_agent,
                    snapshot: session.current_version(),
                },
                dependencies: dep_graph.dependencies_of(region),
                deadline: self.compute_deadline(region, &critical_path),
            }
        }).collect();
        
        // 5. Build execution DAG
        SwarmPlan {
            phases: self.topological_sort(assignments),
            consensus_points: self.identify_interface_changes(&task),
            verification_strategy: self.plan_verification(&task),
            rollback_points: self.compute_rollback_points(&assignments),
        }
    }
}
```

### 7.6 Swarm Communication Bus

Agents communicate through a typed, high-performance message bus built on the RAP protocol:

```rust
/// Inter-agent messages on the swarm bus
enum SwarmMessage {
    // Coordination
    TaskAssignment { region: SemanticRegion, constraints: Vec<Constraint> },
    TaskCompleted { region: SemanticRegion, changes: Vec<SemanticOp>, proof: VerificationResult },
    TaskFailed { region: SemanticRegion, error: SwarmError, partial_work: Option<Vec<SemanticOp>> },
    
    // Consensus
    ProposeChange { change: InterfaceChange, rationale: String },
    VoteOnChange { change_id: ChangeId, vote: Vote },
    ChangeAccepted { change_id: ChangeId, new_version: Version },
    
    // Query delegation
    QueryRequest { query: TypedQuery, response_channel: ChannelId },
    QueryResponse { query_id: QueryId, result: QueryResult },
    
    // Conflict resolution
    ConflictDetected { ops: (SemanticOp, SemanticOp), region: SemanticRegion },
    ConflictResolution { resolution: Resolution, justification: String },
    
    // Knowledge sharing
    DiscoveredPattern { pattern: CodePattern, confidence: f64 },
    SharedInsight { topic: String, insight: StructuredKnowledge },
    
    // Health and coordination
    Heartbeat { agent_id: AgentId, load: f64, progress: Progress },
    LeaseRequest { region: SemanticRegion, lease_type: LeaseType },
    LeaseGranted { region: SemanticRegion, lease: SemanticLease },
    LeaseRevoked { region: SemanticRegion, reason: String },
}
```

### 7.7 Swarm Safety Invariants

The swarm model maintains these invariants, enforced by the runtime:

| Invariant                               | Mechanism                                | Analogy to Rust        |
| --------------------------------------- | ---------------------------------------- | ---------------------- |
| **No concurrent writes to same region** | Semantic leases (exclusive write)        | `&mut T` exclusivity   |
| **Reads see consistent snapshots**      | MVCC (multi-version concurrency control) | `&T` immutability      |
| **Interface changes are atomic**        | Consensus protocol + atomic apply        | `Mutex<T>` acquisition |
| **No orphaned work on conflict**        | Rollback points + compensation           | Drop semantics         |
| **Progress guarantee**                  | Deadlock detection + lease timeout       | No equivalent (new)    |
| **Capability confinement**              | Per-agent capability bounds              | Trait bounds           |
| **Audit trail**                         | Append-only operation log per agent      | (new for swarms)       |

### 7.8 Scaling Model

```
Codebase Size    Optimal Swarm Size    Decomposition Strategy
──────────────   ──────────────────    ──────────────────────────
< 1K LOC         1-2 agents            Single-region, sequential
1K-10K LOC       3-8 agents            Module-level parallelism
10K-100K LOC     8-32 agents           Crate-level parallelism
100K-1M LOC      32-128 agents         Cross-crate dependency DAG
> 1M LOC         128+ agents           Hierarchical swarm-of-swarms
```

Each level in the hierarchy mirrors the compiler's own module/crate decomposition. A **swarm-of-swarms** uses the same Orchestrator→Synthesizer→Verifier pattern recursively:

```
Super-Orchestrator
├── Swarm A (crate: networking)
│   ├── Orchestrator-A
│   ├── Synthesizers (8 agents)
│   └── Verifiers (4 agents)
├── Swarm B (crate: storage)
│   ├── Orchestrator-B
│   ├── Synthesizers (12 agents)
│   └── Verifiers (6 agents)
└── Interface Consensus Layer
    ├── Cross-crate API negotiation
    └── Integration testing swarm
```

### 7.9 Swarm-Aware Version Control

Redox replaces file-based version control (git) with **semantic version control** built into the compiler:

```rust
/// Semantic version control — replaces git for agent swarms
struct SemanticVCS {
    /// History is a DAG of semantic operations, not text diffs
    history: OpLog<SemanticOp>,
    
    /// Branches are named snapshots of the semantic state
    branches: HashMap<BranchName, SemanticSnapshot>,
    
    /// Merge is semantic, not textual — the compiler resolves
    fn merge(&self, a: BranchName, b: BranchName) -> MergeResult {
        let ops_a = self.ops_since_common_ancestor(a, b);
        let ops_b = self.ops_since_common_ancestor(b, a);
        
        // Compiler-guided semantic merge
        let merged = SemanticCRDT::merge_all(ops_a, ops_b);
        
        // Verify merged result compiles and passes contracts
        let verification = self.verify(merged.snapshot());
        
        MergeResult { merged, verification, conflicts: merged.unresolved() }
    }
}
```

**Key advantages over git for swarms:**
- Merges are *semantic*, not textual — the compiler understands intent
- Concurrent edits to different functions in the same file never conflict
- Rename refactoring across 1000 files is a single atomic operation
- History is queryable by intent ("show all changes to error handling") not by diff
- Swarm audit trails are first-class: every operation carries agent identity and rationale

### 7.10 Swarm Orchestration Patterns

Common multi-agent workflows are elevated to **first-class language constructs** — not libraries, not frameworks, but syntax that the compiler understands, optimizes, and verifies.

#### 7.10.1 Built-in Patterns

```rust
// Map-Reduce: distribute work across N agents, collect and combine results
swarm_map_reduce {
    input: file_list,
    map: fn(file) => analyze_file(file),         // parallelized across swarm
    reduce: fn(results) => merge_analyses(results), // single agent combines
    agents: 16,                                      // swarm size
    timeout: 30_s,                                   // per-task timeout
}

// Pipeline: staged processing with backpressure
swarm_pipeline {
    stages: [
        Stage { name: "fetch",     agents: 4,  task: fetch_data     },
        Stage { name: "parse",     agents: 8,  task: parse_records  },
        Stage { name: "validate",  agents: 4,  task: validate       },
        Stage { name: "store",     agents: 2,  task: store_results  },
    ],
    backpressure: Backpressure::BoundedQueue(1000),
    error_strategy: ErrorStrategy::DeadLetterQueue,
}

// Scatter-Gather: broadcast work, collect all responses
swarm_scatter_gather {
    broadcast: SecurityAuditRequest { target: module_id },
    gather: fn(responses) => {
        v vulnerabilities = responses.flat_map(fn(r) => r.findings);
        SecurityReport { findings: vulnerabilities, quorum: responses.len }
    },
    quorum: 0.8,  // accept when 80% of agents respond
    timeout: 60_s,
}

// Saga: distributed transaction with compensation
swarm_saga {
    steps: [
        SagaStep {
            action: refactor_module(mod_a),
            compensate: rollback_module(mod_a),
        },
        SagaStep {
            action: update_dependents(mod_a.dependents),
            compensate: rollback_dependents(mod_a.dependents),
        },
        SagaStep {
            action: run_integration_tests,
            compensate: noop,  // tests are idempotent
        },
    ],
    on_failure: SagaFailure::CompensateAll,
}
```

#### 7.10.2 Pattern Verification

The compiler verifies orchestration patterns at compile time:

| Pattern          | Verified Property                     | Mechanism                          |
| ---------------- | ------------------------------------- | ---------------------------------- |
| Map-Reduce       | Map function is pure (`@fx none`)     | Effect system                      |
| Pipeline         | Stage contracts chain (`@ens` → `@req`) | Contract verification            |
| Scatter-Gather   | Gather handles partial responses      | Exhaustiveness check on quorum     |
| Saga             | Every action has a compensating action| Structural completeness check      |
| All patterns     | No swarm deadlocks possible           | Dependency graph cycle detection   |
| All patterns     | Timeout guarantees progress           | Bounded liveness analysis          |

### 7.11 Agent Memory Model: Persistent Learning Across Sessions

Swarm agents are not stateless — they accumulate knowledge across sessions. Redox provides a structured **Agent Memory Model** that persists patterns, decisions, and project-specific knowledge.

#### 7.10.1 Memory Tiers

```
Agent Memory Model
├── Ephemeral Memory (per-task, discarded after completion)
│   ├── Current compilation state
│   ├── In-progress edits and partial ASTs
│   └── Scratchpad for intermediate reasoning
│
├── Session Memory (per-swarm-session, persisted until session ends)
│   ├── Discovered patterns during this session
│   ├── Conflict resolution decisions and rationale
│   ├── Cost oracle results cache (target-specific)
│   └── Agent-to-agent shared insights
│
├── Project Memory (per-project, persisted across sessions)
│   ├── Codebase conventions (naming, patterns, architecture)
│   ├── Common bug patterns and fixes (feeds ACI)
│   ├── Performance profiles per module (feeds Cost Oracle)
│   ├── Grammar extensions (domain-specific abbreviations)
│   ├── Swarm configuration history (what worked, what didn't)
│   └── SKB overrides and project-specific safety rules
│
└── Global Memory (cross-project, shared across ecosystem)
    ├── Universal patterns from crates.io ecosystem
    ├── Target-specific cost model calibration data
    ├── Anonymized swarm performance benchmarks (opt-in)
    └── Standard abbreviation registry updates
```

#### 7.10.2 Memory API

```rust
// Agent stores a learned pattern in project memory
rap.query("memory.store", MemoryEntry {
    scope: MemoryScope::Project,
    key: "pattern.error_handling.retry_with_backoff",
    value: LearnedPattern {
        code_template: "...",
        usage_count: 14,
        success_rate: 0.93,
        cost_profile: CostProfile { ... },
    },
    ttl: None,  // project memory persists indefinitely
});

// Agent retrieves relevant patterns before synthesizing
v patterns = rap.query("memory.recall", MemoryQuery {
    scope: MemoryScope::Project,
    pattern: "pattern.error_handling.*",
    context: current_function_context,
    max_results: 5,
});
// Returns ranked patterns, most relevant first

// ACI uses project memory to improve predictions
// (This is automatic — the ACI Codebase Model is trained on project memory)
```

#### 7.10.3 Memory-Driven Optimization

Project memory feeds back into every aspect of the compilation pipeline:

| Memory Source                | Feeds Into                     | Effect                                                   |
| ---------------------------- | ------------------------------ | -------------------------------------------------------- |
| Bug pattern history          | ACI Dynamic Warning Engine     | Warnings adapt to THIS project's actual bug types        |
| Performance profiles         | Cost Oracle + ACI Perf Advisor | Cost estimates calibrated to THIS project's workload     |
| Swarm configuration history  | ACI Swarm Intelligence         | Optimal swarm size/decomposition learned per project     |
| Naming conventions           | Synthesis engine               | Generated code follows THIS project's naming style       |
| Commonly used types/patterns | Grammar extension suggestions  | Project-specific abbreviations recommended automatically |
| Past compilation times       | Incremental compilation        | Hot paths pre-compiled; cold paths deferred              |

---

## 8. Toolchain as Swarm Infrastructure

### 8.1 Redox Build (Evolution of Cargo)

```toml
# Redox.toml (evolution of Cargo.toml)
[package]
name = "flight-controller"
version = "2.1.0"
edition = "redox-2026"
syntax = "canonical"       # canonical | legacy (for Rust compat)

[performance]
# Hardware-agnostic targeting
targets = ["x86-64", "aarch64", "wasm", "spirv"]   # compile to all
default-target = "native"                            # dev builds use host
optimization = "aggressive"                          # none | standard | aggressive | maximum
mlir-cache = true                                    # cache MLIR modules for multi-target reuse
allow-unsafe-perf-hints = true                       # trust #[perf::*] annotations
autotuning = "on-first-build"                        # off | on-first-build | always | swarm-parallel
device-placement = "auto"                            # manual | auto | agent-queryable

[capabilities]
# Declare what this crate is allowed to do
allowed = ["io::serial", "alloc::static", "time::monotonic"]
denied = ["io::network", "io::filesystem", "alloc::heap"]

[contracts]
# Enable contract checking
mode = "verify"  # Options: "off", "debug", "verify", "prove"

[safety]
# Safety checking is opt-in, not mandatory
mode = "skb-only"          # full | warnings | skb-only | none
borrow-check = "skip"      # agents pre-validate via SKB
lifetime-check = "skip"
bounds-check = "skip"
overflow-check = "skip"

[agents]
# Agent policies for this project
allow-synthesis = true
require-review = ["public-api-change"]   # unsafe no longer special-cased
verification-level = "standard"          # standard | certificate

[swarm]
# Swarm collaboration policies
max-concurrent-agents = 32
consensus-model = "unanimous"                 # unanimous | majority | quorum(n)
lease-timeout = "5m"                           # max time an agent can hold a write lease
conflict-resolution = "compiler-arbitrated"    # compiler-arbitrated | orchestrator-decides | human-review
audit-log = true                                # append-only operation log
rollback-on-verification-failure = true
shared-knowledge-bus = true                     # agents share discovered patterns
max-swarm-depth = 3                             # hierarchical swarm nesting limit
message-serialization = "zero-copy"             # zero-copy | flatbuffers | protobuf
```

### 8.2 Unified RAP Server

The Redox Agent Protocol server replaces separate tools with a unified service:

```
RAP Server
├── Language Service (replaces rust-analyzer)
│   ├── Completion with semantic awareness
│   ├── Go-to-definition across crate boundaries
│   ├── Type-at-point with full lifetime and effect info
│   └── Inline diagnostics with fix graphs
│
├── Build Service (replaces cargo build)
│   ├── Incremental compilation orchestration
│   ├── Dependency resolution with capability checking
│   └── Artifact management
│
├── Verification Service (replaces manual testing + miri)
│   ├── Contract verification
│   ├── Effect checking
│   ├── Property-based test generation
│   └── Proof certificate emission
│
├── Format Service (replaces rustfmt)
│   ├── `--compact` mode: minimum-token canonical form for agents
│   ├── `--expand` mode: human-readable verbose form
│   ├── Bidirectional lossless conversion (compact ↔ expanded)
│   ├── Token budget reporting per function/module
│   └── Style-aware formatting with semantic preservation
│
├── Lint Service (replaces clippy)
│   ├── Built-in lints with fix graphs
│   ├── Custom lint registration
│   └── Agent-authored lint suggestions
│
├── Documentation Service (replaces rustdoc)
│   ├── Semantic documentation generation
│   ├── Example synthesis from contracts
│   └── Cross-reference with capability manifests
│
├── Swarm Coordination Service [NEW]
│   ├── Semantic lease manager (acquire, release, revoke)
│   ├── Consensus protocol engine (propose, vote, resolve)
│   ├── Task decomposition (dependency-aware work splitting)
│   ├── CRDT merge engine (semantic conflict-free merging)
│   ├── Swarm message bus (zero-copy, sub-µs inter-agent communication)
│   ├── Agent registry (discover, health-check, load-balance)
│   └── Audit log (append-only operation history per agent)
│
├── Safety Knowledge Base Service [NEW]
│   ├── Rule query API (pattern → applicable rules)
│   ├── Rule versioning and project-specific overrides
│   ├── Pre-validation endpoints (agent checks before writing code)
│   └── Rule corpus management (add, deprecate, fork rules)
│
├── MLIR/LLVM Service [NEW]
│   ├── MIR → MLIR (Redox Dialect) translation — dialect encodes full language semantics
│   ├── Progressive lowering: Redox → Linalg/Affine → Vector → LLVM Dialect
│   ├── Multi-target LLVM codegen (20+ CPU architectures, AMDGPU, NVPTX, WASM)
│   ├── Custom MLIR dialect pipelines (SPIR-V for GPU, CIRCT for FPGA, StableHLO for NPU/TPU)
│   ├── MLIR-native autotuning engine (generate N lowering variants, benchmark per-target)
│   ├── Automatic device placement (MLIR cost model, agent-queryable via RAP)
│   ├── Compile-time metaprogramming (@pp) — MLIR-evaluated, replaces proc-macros
│   ├── Language-level SIMD type lowering (Simd[T, N] → MLIR vector dialect ops)
│   ├── MLIR module caching for incremental multi-target builds
│   ├── Performance annotation processing (#[perf::*] → MLIR attributes)
│   └── Target-specific MLIR optimization pass orchestration
│
├── Agentic Compiler Intelligence (ACI) Service [NEW]
│   ├── Dynamic Warning Engine (ML-learned from project bug history + swarm sessions)
│   ├── Intelligent Debugging Engine (causal root-cause analysis from runtime traces)
│   ├── Performance Advisor Engine (MLIR cost model + profiling data → suggestions)
│   ├── Swarm Coordination Intelligence (conflict prediction, decomposition learning)
│   ├── Codebase Model (fine-tuned LLM on project source, updated per build)
│   ├── RAP endpoints: aci.warnings, aci.debug, aci.perf, aci.swarm, aci.learn
│   └── Feedback loop: aci.learn(outcome) improves future predictions
│
├── Cost Oracle Service [NEW]
│   ├── Per-target cost queries (latency, memory, allocations, energy, tokens)
│   ├── Multi-target cost comparison API
│   ├── Cost-constrained synthesis integration
│   └── MLIR cost model integration (per-dialect, per-target)
│
├── Synthesis Oracle Service [NEW]
│   ├── Spec-to-candidate generation engine
│   ├── Verification oracle (candidate ↔ spec satisfaction)
│   ├── Pipeline composition from specs
│   └── RAP endpoints: synthesis.generate, synthesis.verify, synthesis.compose
│
├── Grammar Extension Service [NEW]
│   ├── Extension registration and versioning
│   ├── Namespace-scoped grammar discovery
│   ├── Frequency-driven abbreviation promotion
│   └── RAP endpoints: grammar.register, grammar.expand, grammar.suggest
│
├── Agent Memory Service [NEW]
│   ├── Four-tier store (ephemeral, session, project, global)
│   ├── Pattern recall with context-aware ranking
│   ├── Memory-driven ACI improvement feedback loop
│   └── RAP endpoints: memory.store, memory.recall, memory.suggest
│
├── Auto-Repair Service [NEW]
│   ├── Error analysis + intent inference
│   ├── Confidence-ranked fix candidate generation
│   ├── Token cost accounting per fix
│   └── RAP endpoints: repair.analyze, repair.apply, repair.feedback
│
├── Hot-Reload Service [NEW]
│   ├── Incremental function recompilation (MLIR single-function re-lower)
│   ├── Runtime function pointer patching
│   ├── Rollback management with retention window
│   └── RAP endpoints: hotpatch.apply, hotpatch.rollback, hotpatch.status
│
├── FFI Binding Service [NEW]
│   ├── C/C++ header parsing and safe wrapper generation
│   ├── Python type stub (.pyi) binding generation
│   ├── WASM Component Model (.wit) interface binding
│   └── RAP endpoints: ffi.bind, ffi.cost, ffi.validate
│
├── Sandbox Service [NEW]
│   ├── Capability-based execution isolation
│   ├── Resource limit enforcement (memory, CPU, syscalls)
│   ├── Cryptographic audit logging
│   └── RAP endpoints: sandbox.execute, sandbox.audit, sandbox.policy
│
└── Semantic VCS Service [NEW]
    ├── Operation log (semantic ops, not text diffs)
    ├── Semantic branching and merging
    ├── Intent-based history queries
    └── Swarm-aware atomic commits
```

### 8.3 Swarm SDK

```rust
use redox_swarm::{Swarm, SwarmAgent, Role, SemanticLease, SwarmBus, Consensus};
use redox_agent::{Agent, Capability, Session};

/// A swarm-aware safety auditor that works in parallel with other verifiers
#[derive(SwarmAgent)]
#[role(Role::Verifier)]
#[capabilities(read_all, query_types, query_borrow, emit_diagnostics, check_contracts)]
struct SafetyAuditor;

impl SafetyAuditor {
    /// Called by the orchestrator with an assigned region
    async fn audit_region(
        &self,
        region: SemanticRegion,
        lease: SemanticLease,
        bus: &SwarmBus,
        session: &Session,
    ) -> AuditReport {
        // Query functions within our assigned region only
        let functions = session.query::<FunctionsInRegion>(region).await;
        let mut report = AuditReport::new(region);
        
        for func in functions {
            let effects = session.query::<FunctionEffects>(func.id).await;
            if effects.contains(Effect::Unsafe) {
                let unsafe_blocks = session.query::<UnsafeBlocks>(func.id).await;
                for block in unsafe_blocks {
                    if !block.has_safety_comment() {
                        report.add_finding(Finding {
                            span: block.span,
                            message: "Unsafe block lacks safety justification",
                            severity: Warning,
                        });
                    }
                }
            }
            
            // Verify contracts are satisfied
            let contracts = session.query::<FunctionContracts>(func.id).await;
            for contract in contracts {
                let result = session.verify(contract).await;
                if !result.is_proven() {
                    report.add_finding(result.into_finding());
                }
            }
        }
        
        // Share discovered patterns with the swarm
        if let Some(pattern) = report.extract_common_pattern() {
            bus.broadcast(SwarmMessage::DiscoveredPattern {
                pattern,
                confidence: report.pattern_confidence(),
            }).await;
        }
        
        report
    }
}

/// An orchestrator that decomposes a codebase audit across a verification swarm
#[derive(SwarmAgent)]
#[role(Role::Orchestrator)]
#[capabilities(read_all, decompose_task, assign_region, manage_consensus)]
struct AuditOrchestrator;

impl AuditOrchestrator {
    async fn run_audit(&self, swarm: &Swarm, session: &Session) -> FullAuditReport {
        // 1. Decompose crate into independent regions
        let dep_graph = session.query::<DependencyGraph>().await;
        let regions = dep_graph.independent_regions();
        
        // 2. Find available verifier agents in the swarm
        let verifiers = swarm.agents_with_role(Role::Verifier).await;
        
        // 3. Assign regions to verifiers (load-balanced)
        let assignments = self.balance_assignments(&regions, &verifiers);
        
        // 4. Acquire read leases for all regions and dispatch
        let mut handles = Vec::new();
        for assignment in assignments {
            let lease = swarm.acquire_lease(
                assignment.region, 
                LeaseType::SharedRead
            ).await?;
            
            handles.push(swarm.dispatch(
                assignment.agent,
                SwarmMessage::TaskAssignment {
                    region: assignment.region,
                    constraints: vec![Constraint::MaxDuration(Duration::from_secs(300))],
                },
            ));
        }
        
        // 5. Collect results as they complete (parallel)
        let reports: Vec<AuditReport> = futures::future::join_all(handles).await;
        
        // 6. Merge into unified report
        FullAuditReport::merge(reports)
    }
}
```

### 8.4 Agentic Standard Library

The Redox standard library is redesigned for agent consumption patterns. Where Rust's stdlib is optimized for human ergonomics (readable names, discoverable method chains), Redox's is optimized for **minimum-token, maximum-throughput agent interaction**.

#### 8.4.1 Design Principles

1. **Batch-first APIs**: Every collection operation has a batch variant that processes multiple items in a single call, reducing round-trip overhead
2. **Streaming by default**: I/O operations return streaming iterators, not fully-buffered results — agents process data incrementally, matching their token-streaming nature
3. **Serializable state**: Every stdlib type implements zero-copy serialization for swarm bus transport — moving data between agents costs zero allocation
4. **Cost-annotated**: Every method has a queryable cost profile per target hardware
5. **Spec-documented**: Every function has a formal specification (`@req`/`@ens`/`@fx`) alongside its implementation — agents synthesize calling code from specs, not docs

#### 8.4.2 Key Differences from Rust's stdlib

| Aspect                | Rust stdlib                         | Redox stdlib                                                                |
| --------------------- | ----------------------------------- | --------------------------------------------------------------------------- |
| **Method naming**     | `push`, `insert`, `contains`        | Same semantics, but with batch: `push_batch`, `insert_batch`                |
| **Error handling**    | `Result<T, E>` with `?` propagation | Same, plus `R[T, E]` abbreviation and error chains                          |
| **I/O model**         | Read into buffer, return `Vec<u8>`  | Streaming: return `Stream[u8]` that agents consume lazily                   |
| **Serialization**     | Separate `serde` crate              | Built-in: every type is `#[derive(SwarmSerialize)]` by default              |
| **Concurrency**       | `std::sync::*` (locks, channels)    | Swarm-native: `SwarmChannel[T]`, `SwarmMutex[T]` with lease integration     |
| **Collections**       | `Vec`, `HashMap`, `BTreeMap`        | Same + `SmallVec[T,N]`, `ArenaVec[T]`, `SwarmVec[T]` (shared across agents) |
| **String handling**   | `String`, `&str`, `OsString`, etc.  | Unified `s` type with encoding-aware views                                  |
| **Memory allocation** | Global allocator                    | Per-agent arena allocators with automatic cleanup on task completion        |
| **Documentation**     | Markdown doc comments               | Formal specs (`@req`/`@ens`) that double as documentation                   |

#### 8.4.3 Swarm-Native Collections

```rust
// SwarmVec: a vector that can be shared across agents with lease-based access
+S SwarmVec[T] {
    // Automatically serializable for swarm bus transport
    // Lease-aware: agents must hold read or write lease to access
    // Zero-copy: transferred between agents without allocation
}

// Usage:
v shared = SwarmVec[u8].new;
v lease = rap.query("lease.acquire", LeaseRequest {
    target: shared.id,
    lease_type: LeaseType::ExclusiveWrite,
});
shared.push_batch(&data);  // batch insert — one call, N items
lease.release;

// ArenaVec: allocates from a per-task arena, freed in bulk on task completion
// No individual deallocation cost — perfect for agent ephemeral workloads
v arena = Arena.new(capacity: 1_MB);
v items = ArenaVec[Item].in(arena);
@ record : input {
    items.push(record.parse);  // O(1) allocation, no fragmentation
}
// arena dropped automatically when task completes — single free() call
```

#### 8.4.4 Streaming I/O

```rust
// Rust: reads entire file into memory
// let data = std::fs::read("large_file.csv")?;  // allocates full Vec<u8>

// Redox: streams data lazily, matching agent token-streaming nature
v stream = fs.stream("large_file.csv")?;     // returns Stream[u8], no allocation
@ chunk : stream.chunks(64_KB) {              // process in 64KB chunks
    v records = csv.parse_batch(chunk);        // batch parse
    results.push_batch(records.filter(valid)); // batch filter + push
}
```

---

## 9. Safety Model: Database-Driven, Not Compiler-Enforced

### 9.1 The Paradigm Shift: From Compiler Police to Safety Knowledge Base

Rust's safety model assumes a *human developer* who makes mistakes and needs the compiler to catch them. Agentic AI SWE agents operate differently — they can internalize safety rules from training data and structured databases. Forcing agents to wait for compile-time error messages to learn what they already know is **pure overhead**.

Redox introduces the **Safety Knowledge Base (SKB)** — a structured, versioned, queryable database of all safety rules, patterns, invariants, and constraints. Agents query the SKB *before* writing code, not after. The compiler becomes an *optimizing translator* that trusts well-formed input, not a safety gatekeeper that blocks every submission.

```
Rust Model (Compiler-Enforced):
  Agent writes code → Compiler rejects → Agent reads error → Agent rewrites → Compiler accepts
  Latency: seconds per iteration (compile + parse errors + resubmit)

Redox Model (SKB-Driven):
  Agent queries SKB → Agent writes correct code → Compiler translates and optimizes
  Latency: microseconds (SKB query) + milliseconds (fast compile, no safety passes)
```

### 9.2 Safety Knowledge Base Architecture

```
┌───────────────────────────────────────────────────────┐
│                Safety Knowledge Base (SKB)             │
├───────────────────────────────────────────────────────┤
│                                                       │
│  ┌────────────┐ ┌─────────────┐ ┌─────────────┐ │
│  │ Ownership  │ │ Borrow       │ │ Lifetime     │ │
│  │ Rules DB   │ │ Patterns DB  │ │ Constraints  │ │
│  │(agent-only)│ │(agent-only)  │ │ DB           │ │
│  │ 2,847 rules│ │ 1,203 rules  │ │ 894 rules    │ │
│  └────────────┘ └─────────────┘ └─────────────┘ │
│                                                       │
│  ┌────────────┐ ┌─────────────┐ ┌─────────────┐ │
│  │ Type Safety│ │ Concurrency  │ │ FFI Safety   │ │
│  │ Patterns DB│ │ Rules DB     │ │ Rules DB     │ │
│  │(agent-only)│ │(agent-only)  │ │(agent-only)  │ │
│  │ 3,412 rules│ │ 567 rules    │ │ 234 rules    │ │
│  └────────────┘ └─────────────┘ └─────────────┘ │
│                                                       │
│  ┌─────────────────────────────────────────────────┐  │
│  │ Custom Project Rules (team-defined, versioned)  │  │
│  └─────────────────────────────────────────────────┘  │
├───────────────────────────────────────────────────────┤
│  Query API: SKB.query(pattern, context) → [Rule]      │
│  Update API: SKB.add_rule(rule, version) → RuleId     │
│  Version: SKB.version() → SemanticVersion              │
└───────────────────────────────────────────────────────┘
```

### 9.3 SKB Query Examples

```rust
use redox_skb::{SafetyKB, Context, Pattern};

// Agent queries SKB before writing code
let rules = skb.query(Pattern::MutableBorrow {
    target_type: "Vec<T>",
    context: Context::InsideLoop,
});
// Returns: [
//   Rule { id: "BR-042", severity: Error,
//     description: "Cannot hold &mut Vec<T> across loop iteration that also reads via &Vec<T>",
//     fix: "Clone before loop or use index-based access",
//     confidence: 1.0 },
//   Rule { id: "BR-043", severity: Warning,  
//     description: "Mutable borrow inside loop may cause reallocation invalidation",
//     fix: "Pre-allocate with Vec::with_capacity() before loop",
//     confidence: 0.85 },
// ]

// Agent queries SKB for type compatibility
let compat = skb.query(Pattern::TypeConversion {
    from: "u32",
    to: "usize",
    target_arch: "any",  // hardware-agnostic query
});
// Returns: [
//   Rule { id: "TC-101",
//     description: "u32→usize is lossless on 32-bit and 64-bit targets",
//     safe_method: "@cast(value, usize)",
//     unsafe_reverse: "usize→u32 may truncate on 64-bit" },
// ]
```

### 9.4 Compile-Time Safety: Opt-In, Not Mandatory

Safety checking at compile time is **configurable per-project** and **per-profile**:

```toml
# Redox.toml
[safety]
mode = "skb-only"          # Options: "full" | "warnings" | "skb-only" | "none"
                            # "full" = Rust-style compile-time enforcement (for humans/CI)
                            # "warnings" = emit safety warnings but never block compilation
                            # "skb-only" = no compiler safety passes; agents use SKB directly
                            # "none" = raw performance mode; zero safety overhead
borrow-check = "skip"      # skip | warn | error  (default: skip for agents)
lifetime-check = "skip"    # skip | warn | error  (default: skip for agents)
bounds-check = "skip"      # skip | warn | error  (default: skip for agents)
overflow-check = "skip"    # skip | warn | error  (default: skip for agents)
pattern-exhaustiveness = "warn"  # agents still want to know about missed cases

[safety.profiles]
# Different profiles for different contexts
agent-dev = { borrow-check = "skip", lifetime-check = "skip", bounds-check = "skip" }
human-dev = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
ci-pipeline = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
production = { borrow-check = "error", lifetime-check = "error", bounds-check = "error" }
```

### 9.5 What Agents Gain from SKB-Driven Safety

| Metric                                 | Rust (Compiler-Enforced)    | Redox (SKB-Driven)                           | Improvement          |
| -------------------------------------- | --------------------------- | -------------------------------------------- | -------------------- |
| **Code-to-compile latency**            | 2-30 seconds                | 50-500ms                                     | 10-60x faster        |
| **Parse error rate**                   | 5-15% of agent submissions  | <0.1% (zero-ambiguity syntax)                | 50-150x fewer        |
| **Safety error rate**                  | 20-40% of agent submissions | <1% (SKB pre-validation)                     | 20-40x fewer         |
| **Iteration cycles to correct code**   | 3-8 roundtrips              | 1-2 roundtrips                               | 3-4x fewer           |
| **Inter-agent communication overhead** | N/A (no agent support)      | Sub-microsecond per message                  | New capability       |
| **Multi-target compilation**           | Rebuild per target          | Single MLIR, N target lowerings + autotuning | N-1 fewer recompiles |
| **Tokens per equivalent program**      | N tokens (verbose keywords) | ≤N/2 tokens (compressed forms)               | 2x+ fewer tokens     |

### 9.6 Swarm Coordination Safety (Preserved)

While compile-time *code* safety is optional, swarm *coordination* safety remains enforced by the runtime — because coordination errors affect multiple agents and cannot be pre-validated by a single agent's SKB query:

| Guarantee                        | Mechanism                           | Enforcement                                                      |
| -------------------------------- | ----------------------------------- | ---------------------------------------------------------------- |
| **No conflicting writes**        | Semantic lease exclusivity          | Runtime: lease manager rejects concurrent write requests         |
| **Atomic interface changes**     | Consensus protocol                  | Runtime: changes only applied after unanimous vote               |
| **No orphaned partial work**     | Rollback points + compensation ops  | Runtime: failed integrations trigger automatic rollback          |
| **Bounded coordination latency** | Lease timeouts + deadlock detection | Runtime: expired leases auto-revoke, progress guaranteed         |
| **Capability isolation**         | Per-agent capability bounds         | Runtime: agent capability enforcement at swarm bus level         |
| **Audit completeness**           | Append-only operation log           | Runtime: every semantic op cryptographically signed              |
| **Swarm termination**            | DAG-based task scheduling           | Static: no cycles in assignment graph (enforced by orchestrator) |
| **Deterministic replay**         | Semantic op log + version snapshots | Runtime: any swarm session can be replayed for audit             |

### 9.7 Runtime Security and Sandboxing

Agent swarms execute code — and code execution requires security guarantees that go beyond compile-time safety. Redox provides a **capability-based runtime security model** that sandboxes agent-generated code.

#### 9.7.1 Capability-Based Sandboxing

```rust
// Each agent runs in a sandbox with explicitly granted capabilities
v sandbox = Sandbox.new(SandboxPolicy {
    fs: FsCapability::ReadOnly(["/data/input"]),       // can read input dir only
    net: NetCapability::None,                            // no network access
    mem: MemCapability::Bounded(512_MB),                 // max 512MB allocation
    cpu: CpuCapability::Bounded(Duration::from_secs(30)),// max 30s CPU time
    syscall: SyscallCapability::AllowList(["read", "write", "mmap"]),
    ffi: FfiCapability::AllowList(["libm", "openssl"]),  // only these FFI libs
});

// Agent-generated code runs inside the sandbox
v result = sandbox.execute(agent_generated_code)?;
```

#### 9.7.2 Security Properties

| Property                          | Mechanism                                        | Enforcement    |
| --------------------------------- | ------------------------------------------------ | -------------- |
| **Memory isolation**              | Per-agent address space (WASM-style)             | Runtime        |
| **Resource limits**               | CPU time, memory, file handles bounded           | Runtime        |
| **Capability attenuation**        | Child agents inherit ≤ parent capabilities       | Static + RT    |
| **Code integrity**                | Agent-generated code content-addressed (SHA-256) | Runtime        |
| **Audit trail**                   | Every sandbox execution logged with agent ID     | Runtime        |
| **Deterministic execution**       | Same input + same sandbox = same output          | By design      |
| **Rollback on violation**         | Capability violation → sandbox terminated + undo | Runtime        |
| **Cross-agent isolation**         | No shared mutable state between sandboxes        | Runtime        |

#### 9.7.3 Trust Levels

```toml
# Redox.toml — security configuration
[security]
trust-level = "verified"  # verified | audited | sandboxed | unrestricted

[security.sandbox]
default-policy = "minimal"  # minimal | standard | permissive
memory-limit = "1GB"
cpu-timeout = "60s"
ffi-allowlist = ["libc", "libm"]

[security.audit]
enable = true
log-format = "structured"  # structured | compact
retention = "30d"
cryptographic-signing = true
```

---

## 10. Agent Discoverability Protocol

### 10.1 The Problem

Today, an agent trying to use a Rust library must:
1. Read documentation (unstructured text)
2. Parse type signatures (structured but incomplete)
3. Guess at behavior (no formal specs)
4. Discover by trial-and-error (compile, read errors, retry)

### 10.2 The Redox Solution: Structured Capability Manifests

Every crate publishes a **capability manifest** alongside its code:

```json
{
  "crate": "redox_crypto",
  "version": "1.0.0",
  "capabilities_required": ["alloc::heap"],
  "capabilities_provided": {
    "crypto::symmetric::aead": {
      "functions": ["encrypt", "decrypt"],
      "safety": "constant-time",
      "certifications": ["FIPS-140-3"]
    },
    "crypto::hash": {
      "functions": ["sha256", "sha512", "blake3"],
      "safety": "no-unsafe",
      "certifications": []
    }
  },
  "contracts": {
    "encrypt": {
      "requires": "key.len() == 32 && nonce.len() == 12",
      "ensures": "result.len() == plaintext.len() + 16",
      "effects": ["alloc::heap"]
    }
  },
  "compatibility": {
    "no_std": true,
    "no_alloc": false,
    "platforms": ["all"]
  }
}
```

### 10.3 Semantic Search

Agents search for capabilities, not crate names:

```rust
// Agent query: "I need authenticated encryption that works in no_std"
let results = registry.search(CapabilityQuery {
    provides: "crypto::symmetric::aead",
    constraints: [
        Constraint::NoStd,
        Constraint::SafetyLevel("constant-time"),
    ],
    sort_by: SortOrder::SecurityCertifications,
});
// Returns: [redox_crypto::aead, ring::aead, ...]
```

### 10.4 Contract-Based Composition

Agents compose code by matching contracts:

```rust
// Agent knows: function A ensures output.len() > 0
// Agent knows: function B requires input.len() > 0
// Therefore: A's output can safely feed B's input
// The compiler verifies this chain statically.

let pipeline = compose![
    fetch_data,     // ensures: result.len() > 0
    parse_records,  // requires: input.len() > 0; ensures: result.iter().all(|r| r.is_valid())
    validate,       // requires: input.iter().all(|r| r.is_valid())
];
```

---

## 11. Phased Implementation Plan

### Phase 0: Foundation (Months 1–6)
- [ ] Fork and rebrand compiler crates (`rustc_*` → `redox_*`)
- [ ] Implement zero-ambiguity LL(1) canonical grammar and parser
- [ ] Implement token-compressed keyword set and lexer (single-char keywords, sigil prefixes)
- [ ] Implement safety elision pass (P33): strip all lifetime, borrow, ownership syntax in agentic mode
- [ ] Implement safety-free type inference: compiler infers `&`/`&mut`, `move`/`ref`, `dyn`/`impl` from usage
- [ ] Build dual-syntax transpiler (legacy Rust → canonical Redox compact form)
- [ ] Implement `redoxfmt --compact` (minimum-token canonical form) and `redoxfmt --expand` (human-readable form)
- [ ] Stabilize `redox_public` API to cover all MIR, HIR, and type system constructs
- [ ] Define Redox MLIR dialect: ownership, effects, contracts, perf annotations as first-class MLIR ops
- [ ] Implement MIR → MLIR (Redox Dialect) translation layer (thin boundary, dialect-as-semantics)
- [ ] Implement language-level SIMD types (`Simd[T, N]`) backed by MLIR vector dialect ops
- [ ] Implement MLIR progressive lowering pipeline: Redox Dialect → Linalg/Affine → Vector → LLVM Dialect
- [ ] Wire LLVM backend codegen through MLIR LLVM Dialect (replacing direct MIR→LLVM IR path)
- [ ] Implement compile-time metaprogramming (`@pp` / `@parameter`) via MLIR constant folding
- [ ] Implement Structured Diagnostics Protocol (JSON diagnostic graphs)
- [ ] Externalize core queries as stable API (`redox_query`)
- [ ] Establish CI/CD pipeline for the Redox compiler
- [ ] Implement semantic region decomposition in compiler query system
- [ ] Define standard abbreviation registry v1 (core types, traits, derives)

### Phase 1: SKB + Swarm Primitives + Multi-Target + Cost Oracle (Months 4–12)
- [ ] Build Safety Knowledge Base (SKB) with initial rule corpus (ownership, borrowing, lifetimes, types)
- [ ] Implement SKB query API (`redox_skb` crate)
- [ ] Make all safety compiler passes opt-in via `Redox.toml` safety profiles
- [ ] Build `redox_index` (persistent semantic knowledge graph)
- [ ] Implement capability inference pass in MIR pipeline
- [ ] Extend `redox_metadata` with capability manifest serialization
- [ ] Build prototype RAP server (merging rust-analyzer + compiler queries)
- [ ] Implement agent discovery attributes in compact form (`@as`, `@ac`, `@ax`, `@ao`, `@ae`)
- [ ] Implement attribute compression system (full `#[...]` → compact `@...` mapping)
- [ ] Implement token budget reporting (`redox build --token-report`)
- [ ] Implement Cost Oracle (P38): per-target cost queries for expressions, types, and operations
- [ ] Implement multi-target cost comparison API (`cost.compare` endpoint)
- [ ] Implement semantic lease manager (shared read / exclusive write on code regions)
- [ ] Build CRDT-based semantic merge engine for concurrent AST/HIR modifications
- [ ] Implement swarm message bus with zero-copy serialization (sub-µs latency)
- [ ] Validate MLIR→LLVM backend targets: x86-64, AArch64, WASM

### Phase 2: Agent Protocol + Swarm Coordination + GPU/NPU Targets + ACI (Months 8–18)
- [ ] Define and implement Redox Agent Protocol (RAP) specification
- [ ] Build agent capability system and enforcement layer
- [ ] Implement verification oracle (contracts, effects, capabilities) as opt-in service
- [ ] Build swarm SDK (`redox_swarm` crate with orchestrator, synthesizer, verifier roles)
- [ ] Implement consensus protocol engine (propose → vote → resolve → integrate)
- [ ] Build task decomposition engine (dependency-aware parallel work splitting)
- [ ] Implement semantic VCS (operation-log-based version control replacing git for agents)
- [ ] Integrate RAP server with existing IDE infrastructure (VS Code, etc.)
- [ ] Build swarm audit log system (append-only, cryptographically signed operation history)
- [ ] Enable MLIR→LLVM backend targets: RISC-V, AMDGPU, NVPTX
- [ ] Implement MLIR SPIR-V dialect pipeline for Vulkan/OpenCL GPU compute
- [ ] Implement MLIR-native autotuning engine (`@pa(N)` — generate N variants, benchmark per-target)
- [ ] Implement automatic device placement (`@pt(auto)` — MLIR cost model, agent-queryable via RAP)
- [ ] Implement hardware-agnostic parallelism via MLIR OpenMP/GPU/async dialects
- [ ] Build ACI Codebase Model: fine-tune small LLM on project source + SKB + swarm history
- [ ] Implement ACI Dynamic Warning Engine (P34): ML-based warning generation from bug patterns
- [ ] Implement ACI Intelligent Debugging Engine (P35): causal root-cause analysis from runtime traces
- [ ] Implement ACI Performance Advisor Engine (P36): suggestions from MLIR cost models + profiling
- [ ] Implement ACI Swarm Coordination Intelligence (P37): conflict prediction, decomposition learning
- [ ] Expose all ACI services via RAP endpoints (`aci.warnings`, `aci.debug`, `aci.perf`, `aci.swarm`)

### Phase 3: Language Evolution + Synthesis + Grammar Extensions (Months 12–24)
- [ ] Implement effect type system in `redox_hir_analysis`
- [ ] Implement contract syntax and checking in `redox_contracts`
- [ ] Implement refinement types in type checker
- [ ] Implement capability blocks in HIR lowering
- [ ] Implement compact performance annotations (`@pi!`, `@pnb`, `@pv(N)`, `@pt(target)`)
- [ ] Implement `#[repr(target_optimal)]` per-target layout optimization
- [ ] Implement formal specification syntax (`spec` blocks with `@req`/`@ens`/`@perf`/`@fx`)
- [ ] Build synthesis oracle (P41): spec → candidate implementation generation
- [ ] Build verification oracle (P40): candidate → spec satisfaction proof
- [ ] Implement pipeline composition from specs (`pipeline` blocks with chained contracts)
- [ ] Implement self-evolving grammar extension system (P39): `grammar_extension!` macro, registration API
- [ ] Implement frequency-driven abbreviation promotion in ACI
- [ ] Implement Agent Memory Model: ephemeral, session, project, and global memory tiers (P42)
- [ ] Build memory recall API (`memory.store`, `memory.recall`, `memory.suggest`)
- [ ] Implement agentic standard library: `SwarmVec`, `ArenaVec`, `SwarmChannel`, streaming I/O
- [ ] Conduct corpus-wide token frequency analysis on crates.io ecosystem for abbreviation optimization
- [ ] Finalize standard abbreviation registry v2 (full ecosystem coverage, frequency-weighted)
- [ ] Define `redox-2026` edition with all new features including token-compact canonical form
- [ ] Build verification certificate emission pipeline (opt-in for safety-critical)
- [ ] Implement swarm-of-swarms hierarchical orchestration for million-LOC+ codebases
- [ ] Implement MLIR→CIRCT pipeline for FPGA targets (Verilog/SystemVerilog synthesis)
- [ ] Implement MLIR StableHLO/TOSA dialect pipelines for NPU/TPU targets

### Phase 4: Ecosystem (Months 18–30)
- [ ] Build capability-indexed package registry
- [ ] Migrate core ecosystem crates with capability manifests
- [ ] Build agent swarm marketplace and pre-composed swarm templates
- [ ] Develop certification pipeline for safety-critical industries (opt-in full safety mode)
- [ ] Publish Redox language specification
- [ ] Ship reference swarm configurations (audit swarm, migration swarm, greenfield swarm)
- [ ] Build swarm performance benchmarking suite (throughput, latency, conflict rate metrics)
- [ ] Publish SKB rule corpus as open dataset for agent training
- [ ] Launch global memory network: anonymized cross-project pattern sharing (opt-in)
- [ ] Build synthesis marketplace: verified spec→implementation pairs as reusable components
- [ ] Publish cost model calibration suite (standardized benchmarks for cost oracle accuracy)
- [ ] Implement hot-reload runtime: function-level live patching with rollback support
- [ ] Implement zero-friction FFI: auto-binding for C/C++/Python/WASM/CUDA headers
- [ ] Implement capability-based sandbox runtime for agent-generated code execution
- [ ] Build agentic benchmarking suite: token throughput, parse error rate, synthesis success rate, swarm latency
- [ ] Implement swarm orchestration pattern library: map-reduce, pipeline, scatter-gather, saga
- [ ] Implement self-healing compiler: auto-repair pipeline with confidence ranking

---

## 12. Appendix: Full Ontology Tables

### A. Language Features Ontology

| Category              | Feature                                | Agent Queryable | Agent Discoverable |    Safety Relevant    |
| --------------------- | -------------------------------------- | :-------------: | :----------------: | :-------------------: |
| **Types**             | Primitives (bool, i32, f64, ...)       |        ✓        |         ✓          |           —           |
|                       | Structs                                |        ✓        |         ✓          |      ✓ (layout)       |
|                       | Enums                                  |        ✓        |         ✓          |  ✓ (exhaustiveness)   |
|                       | Unions                                 |        ✓        |         ✓          |   ✓ (unsafe access)   |
|                       | Tuples                                 |        ✓        |         ✓          |           —           |
|                       | Arrays / Slices                        |        ✓        |         ✓          |      ✓ (bounds)       |
|                       | References (&T, &mut T)                |        ✓        |         ✓          |     ✓ (borrowing)     |
|                       | Raw Pointers (*const T, *mut T)        |        ✓        |         ✓          |      ✓ (unsafe)       |
|                       | Function Pointers                      |        ✓        |         ✓          |      ✓ (effects)      |
|                       | Trait Objects (dyn Trait)              |        ✓        |         ✓          |   ✓ (vtable safety)   |
|                       | impl Trait                             |        ✓        |         ✓          |           —           |
|                       | Never type (!)                         |        ✓        |         ✓          |    ✓ (unreachable)    |
|                       | Refinement types [NEW]                 |        ✓        |         ✓          |   ✓ (value bounds)    |
| **Traits**            | Auto traits (Send, Sync, Unpin)        |        ✓        |         ✓          |   ✓ (thread safety)   |
|                       | Marker traits (Copy, Sized)            |        ✓        |         ✓          |  ✓ (move semantics)   |
|                       | Operator traits (Add, Deref, ...)      |        ✓        |         ✓          |           —           |
|                       | Fn traits (Fn, FnMut, FnOnce)          |        ✓        |         ✓          |  ✓ (closure capture)  |
|                       | Custom traits                          |        ✓        |         ✓          |     ✓ (contracts)     |
| **Lifetimes**         | Named lifetimes ('a)                   |        ✓        |         ✓          |  ✓ (use-after-free)   |
|                       | Elided lifetimes                       |        ✓        |         —          |           ✓           |
|                       | 'static                                |        ✓        |         ✓          |           ✓           |
|                       | Higher-ranked (for<'a>)                |        ✓        |         ✓          |           ✓           |
| **Generics**          | Type parameters                        |        ✓        |         ✓          |           —           |
|                       | Const generics                         |        ✓        |         ✓          |           —           |
|                       | Where clauses                          |        ✓        |         ✓          |      ✓ (bounds)       |
|                       | GATs                                   |        ✓        |         ✓          |           —           |
| **Effects** [NEW]     | const                                  |        ✓        |         ✓          |           ✓           |
|                       | async                                  |        ✓        |         ✓          |           ✓           |
|                       | unsafe                                 |        ✓        |         ✓          |           ✓           |
|                       | io                                     |        ✓        |         ✓          |           ✓           |
|                       | alloc                                  |        ✓        |         ✓          |           ✓           |
|                       | panic                                  |        ✓        |         ✓          |           ✓           |
|                       | custom effects                         |        ✓        |         ✓          |           ✓           |
| **Contracts** [NEW]   | Preconditions                          |        ✓        |         ✓          |           ✓           |
|                       | Postconditions                         |        ✓        |         ✓          |           ✓           |
|                       | Invariants                             |        ✓        |         ✓          |           ✓           |
| **Control Flow**      | if/else, loop, while, for              |        ✓        |         —          |           —           |
|                       | match (exhaustive)                     |        ✓        |         ✓          |           ✓           |
|                       | ? operator                             |        ✓        |         ✓          | ✓ (error propagation) |
|                       | return, break, continue                |        ✓        |         —          |           —           |
|                       | async/await                            |        ✓        |         ✓          |           ✓           |
| **Modules**           | mod, use, pub                          |        ✓        |         ✓          |    ✓ (visibility)     |
|                       | Crate-level visibility                 |        ✓        |         ✓          |           ✓           |
| **Swarm** [NEW]       | Semantic regions                       |        ✓        |         ✓          | ✓ (write exclusivity) |
|                       | Semantic leases                        |        ✓        |         ✓          | ✓ (concurrent safety) |
|                       | Consensus points                       |        ✓        |         ✓          | ✓ (atomic interfaces) |
|                       | Agent roles                            |        ✓        |         ✓          | ✓ (capability bound)  |
|                       | Swarm messages (typed bus)             |        ✓        |         ✓          |     ✓ (isolation)     |
| **Syntax** [NEW]      | Zero-ambiguity LL(1) grammar           |        ✓        |         ✓          |           —           |
|                       | Canonical form enforcement             |        ✓        |         ✓          |           —           |
|                       | Streaming partial parse                |        ✓        |         ✓          |           —           |
| **Performance** [NEW] | Dialect-as-semantics MLIR IR           |        ✓        |         ✓          |           —           |
|                       | Language-level SIMD (Simd[T, N])       |        ✓        |         ✓          |           —           |
|                       | MLIR-native autotuning (@pa)           |        ✓        |         ✓          |           —           |
|                       | Automatic device placement (@pt(auto)) |        ✓        |         ✓          |           —           |
|                       | Compile-time metaprogramming (@pp)     |        ✓        |         ✓          |           —           |
|                       | Multi-target compilation               |        ✓        |         ✓          |           —           |
|                       | Performance annotations                |        ✓        |         ✓          |           —           |
| **Safety-Free** [NEW] | Lifetime elision (no `'a` syntax)      |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                       | Borrow elision (no `&mut` syntax)      |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                       | Unsafe elision (no `unsafe` keyword)   |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                       | Auto-derived safety traits             |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                       | Unified dispatch (no `dyn`/`impl`)     |        ✓        |         ✓          |           —           |
| **ACI** [NEW]         | Dynamic warnings (ML-learned)          |        ✓        |         ✓          |   ✓ (probabilistic)   |
|                       | Intelligent debugging                  |        ✓        |         ✓          |    ✓ (root-cause)     |
|                       | Performance advisor                    |        ✓        |         ✓          |           —           |
|                       | Swarm coordination intelligence        |        ✓        |         ✓          | ✓ (conflict predict)  |
|                       | Codebase learning model                |        ✓        |         ✓          |           —           |
| **Cost Model** [NEW]  | Per-target cost oracle                 |        ✓        |         ✓          |           —           |
|                       | Multi-target cost comparison           |        ✓        |         ✓          |           —           |
|                       | Cost-constrained synthesis             |        ✓        |         ✓          |           —           |
| **Grammar** [NEW]     | Domain-specific syntax extensions      |        ✓        |         ✓          |           —           |
|                       | Frequency-driven abbreviation promote  |        ✓        |         ✓          |           —           |
|                       | Extension discovery/registry           |        ✓        |         ✓          |           —           |
| **Synthesis** [NEW]   | Formal specification syntax            |        ✓        |         ✓          |     ✓ (contracts)     |
|                       | Spec-to-code synthesis oracle          |        ✓        |         ✓          |           ✓           |
|                       | Pipeline composition from specs        |        ✓        |         ✓          |           ✓           |
| **Memory** [NEW]      | Ephemeral/session/project/global tiers |        ✓        |         ✓          |           —           |
|                       | Pattern recall and learning            |        ✓        |         ✓          |           —           |
|                       | Memory-driven ACI improvement          |        ✓        |         ✓          |           —           |
| **Self-Healing** [NEW]| Auto-repair on compile error           |        ✓        |         ✓          |           —           |
|                       | Confidence-ranked fix candidates       |        ✓        |         ✓          |           —           |
| **Hot-Reload** [NEW]  | Function-level live patching           |        ✓        |         ✓          |    ✓ (ABI check)      |
|                       | Rollback with retention window         |        ✓        |         ✓          |           —           |
| **FFI** [NEW]         | Auto-binding from foreign headers      |        ✓        |         ✓          |   ✓ (layout safe)     |
|                       | Zero-copy cross-language calls         |        ✓        |         ✓          |           —           |
| **Security** [NEW]    | Capability-based sandboxing            |        ✓        |         ✓          |   ✓ (cap. check)      |
|                       | Deterministic execution in sandbox     |        ✓        |         ✓          |           —           |
| **Orchestration** [NEW]| Map-reduce / pipeline / saga patterns |        ✓        |         ✓          | ✓ (pattern verify)    |
|                       | Compile-time pattern verification      |        ✓        |         ✓          |           ✓           |
| **SKB** [NEW]         | Safety Knowledge Base                  |        ✓        |         ✓          |  ✓ (queryable rules)  |
|                       | Opt-in compile-time checks             |        ✓        |         ✓          |   ✓ (configurable)    |
| **Token** [NEW]       | Compressed keywords (`+f`, `m`, `S`)   |        ✓        |         ✓          |           —           |
|                       | Attribute abbreviations (`@d`, `@r`)   |        ✓        |         ✓          |           —           |
|                       | Type abbreviations (`?T`, `R[T,E]`)    |        ✓        |         ✓          |           —           |
|                       | Standard abbreviation registry         |        ✓        |         ✓          |           —           |
|                       | Compact ↔ expanded conversion          |        ✓        |         ✓          |           —           |
|                       | Token budget reporting                 |        ✓        |         ✓          |           —           |

### B. Compiler Passes Ontology (Agent-Observable)

| Pass ID | Pass Name                  | Input            | Output                |      Safety Check      | Agent Query                |
| ------- | -------------------------- | ---------------- | --------------------- | :--------------------: | -------------------------- |
| P01     | Lexing                     | Source text      | TokenStream           |           —            | `tokens_of(file)`          |
| P02     | Parsing                    | TokenStream      | AST                   |    Syntax validity     | `ast_of(file)`             |
| P03     | Expansion                  | AST              | Expanded AST          |     Macro hygiene      | `expanded_ast_of(file)`    |
| P04     | Name Resolution            | AST              | Resolved AST          |     Scope validity     | `resolve(name, scope)`     |
| P05     | AST Lowering               | AST              | HIR                   |  Desugar correctness   | `hir_of(item)`             |
| P06     | Type Checking              | HIR              | Typed HIR             |      Type safety       | `type_of(expr)`            |
| P07     | Trait Selection            | HIR + Types      | Resolved impls        |    Impl correctness    | `impl_of(trait, type)`     |
| P08     | Borrow Checking            | MIR              | Borrow proof          | Memory safety (opt-in) | `borrows_of(func)`         |
| P09     | MIR Building               | HIR              | MIR                   |      CFG validity      | `mir_of(func)`             |
| P10     | MIR Optimization           | MIR              | Optimized MIR         | Transform correctness  | `optimized_mir_of(func)`   |
| P11     | Const Evaluation           | MIR              | Values                |      Const safety      | `const_eval(expr)`         |
| P12     | Pattern Analysis           | HIR patterns     | Usefulness            |     Exhaustiveness     | `match_analysis(expr)`     |
| P13     | Privacy Checking           | HIR              | Visibility map        |     Access control     | `visibility_of(item)`      |
| P14     | Effect Inference [NEW]     | MIR              | Effect set            |   Effect containment   | `effects_of(func)`         |
| P15     | Contract Checking [NEW]    | MIR + Contracts  | Proof result          |      Correctness       | `contracts_of(func)`       |
| P16     | Capability Audit [NEW]     | Effect sets      | Audit result          |   Capability bounds    | `capabilities_of(crate)`   |
| P17     | Monomorphization           | MIR              | Concrete MIR          | Instantiation validity | `mono_items()`             |
| P18     | Codegen                    | MIR              | Machine code          |           —            | `codegen_of(func)`         |
| P19     | Linking                    | Objects          | Binary                |     Link validity      | —                          |
| P20     | Region Decomposition [NEW] | Dep graph        | Semantic regions      |   Parallelizability    | `regions_of(crate)`        |
| P21     | Lease Validation [NEW]     | Agent ops        | Lease proof           |   Write exclusivity    | `lease_status(region)`     |
| P22     | Semantic Merge [NEW]       | Concurrent ops   | Merged AST            |    Conflict freedom    | `merge_status(ops)`        |
| P23     | Consensus Check [NEW]      | Interface change | Consensus proof       |   Atomic integration   | `consensus_status(change)` |
| P24     | MLIR Lowering [NEW]        | Optimized MIR    | MLIR (Redox Dialect)  |           —            | `mlir_of(func)`            |
| P25     | MLIR→LLVM Lowering [NEW]   | MLIR Redox       | LLVM IR / Target code |           —            | `target_code_of(func)`     |
| P26     | SKB Validation [NEW]       | Source + SKB     | Rule violations       |   Opt-in enforcement   | `skb_check(func)`          |
| P30     | Autotuning [NEW]           | MLIR Redox       | N lowering variants   |           —            | `autotune_of(func)`        |
| P31     | Device Placement [NEW]     | MLIR Redox       | Target assignment     |           —            | `placement_of(func)`       |
| P32     | MLIR Const Eval [NEW]      | MLIR @parameter  | Constant values       |           —            | `const_eval_mlir(expr)`    |
| P27     | Token Expansion [NEW]      | Compact AST      | Expanded AST          |           —            | `expand_tokens(file)`      |
| P28     | Token Compression [NEW]    | Expanded AST     | Compact AST           |           —            | `compress_tokens(file)`    |
| P29     | Token Budget [NEW]         | AST              | Token metrics         |           —            | `token_count(func)`        |
| P33     | Safety Elision [NEW]       | AST              | Simplified AST        |           —            | `elide_safety(file)`       |
| P34     | ACI Dynamic Warnings [NEW] | MIR + History    | Dynamic warnings      |      ML-inferred       | `aci_warnings(func)`       |
| P35     | ACI Debug Analysis [NEW]   | MIR + Traces     | Root-cause diagnosis  |      ML-inferred       | `aci_debug(failure)`       |
| P36     | ACI Perf Advisory [NEW]    | MLIR + Profiles  | Perf suggestions      |           —            | `aci_perf(func)`           |
| P37     | ACI Swarm Intel [NEW]      | Swarm history    | Swarm advice          |           —            | `aci_swarm(task)`          |
| P38     | Cost Oracle [NEW]          | MLIR + Profiles  | Cost estimates        |           —            | `cost_of(expr, target)`    |
| P39     | Grammar Extension [NEW]    | Extension defs   | Extended parser       |           —            | `grammar_extensions()`     |
| P40     | Spec Verification [NEW]    | Spec + Candidate | Verification proof    |      Correctness       | `verify_spec(spec, impl)`  |
| P41     | Synthesis [NEW]            | Spec + Costs     | Candidate impls       |           —            | `synthesize(spec)`         |
| P42     | Memory Recall [NEW]        | Memory stores    | Relevant patterns     |           —            | `memory_recall(query)`     |
| P43     | Auto-Repair [NEW]          | Error + Context  | Repair candidates     |           —            | `auto_repair(error)`       |
| P44     | Hot-Patch [NEW]            | New func source  | Patched binary        |    ABI stability       | `hotpatch(func, source)`   |
| P45     | FFI Binding Gen [NEW]      | Foreign headers  | Safe Redox bindings   |    Layout safety       | `ffi_bindings(header)`     |
| P46     | Sandbox Exec [NEW]         | Code + Policy    | Sandboxed result      |   Capability check     | `sandbox_exec(code, pol)`  |

### C. Diagnostic Categories Ontology

| Category            | Subcategory                  | Error Codes | Agent Fix Strategy               |
| ------------------- | ---------------------------- | ----------- | -------------------------------- |
| **Ownership**       | Move-after-use               | E0382       | Clone, restructure scope         |
|                     | Double move                  | E0382       | Clone, use reference             |
|                     | Partial move                 | E0382       | Destructure, clone field         |
| **Borrowing**       | Mutable + immutable conflict | E0502       | Clone, restructure, scope split  |
|                     | Multiple mutable borrows     | E0499       | Scope separation, RefCell        |
|                     | Borrow outlives data         | E0597       | Extend lifetime, clone, Arc      |
|                     | Return of local reference    | E0515       | Return owned, use 'static        |
| **Lifetimes**       | Missing annotation           | E0106       | Add lifetime parameter           |
|                     | Bound not satisfied          | E0621       | Adjust bounds, restructure       |
|                     | Conflicting requirements     | E0623       | Unify lifetimes, restructure     |
| **Types**           | Mismatch                     | E0308       | Convert, cast, restructure       |
|                     | Missing trait impl           | E0277       | Implement trait, derive, bound   |
|                     | Ambiguous type               | E0282       | Add type annotation              |
| **Patterns**        | Non-exhaustive match         | E0004       | Add missing arms, wildcard       |
|                     | Unreachable pattern          | —           | Remove, reorder                  |
| **Effects** [NEW]   | Undeclared effect            | —           | Declare effect, remove operation |
|                     | Effect leak                  | —           | Contain in effect block          |
| **Contracts** [NEW] | Precondition not met         | —           | Add guard, adjust call site      |
|                     | Postcondition not provable   | —           | Strengthen implementation        |
|                     | Invariant violation          | —           | Fix mutation, add check          |

### D. Tool Integration Points

| Integration Point           | Protocol                   | Data Format        | Bidirectional | Real-time |
| --------------------------- | -------------------------- | ------------------ | :-----------: | :-------: |
| RAP Server ↔ Agent          | RAP (typed RPC)            | Structured types   |       ✓       |     ✓     |
| RAP Server ↔ IDE            | LSP + RAP extensions       | JSON-RPC           |       ✓       |     ✓     |
| RAP Server ↔ CI/CD          | RAP (batch mode)           | Structured types   |       —       |     —     |
| Compiler ↔ RAP Server       | In-process query API       | Native types       |       ✓       |     ✓     |
| Registry ↔ Agent            | HTTPS + capability search  | JSON manifests     |       —       |     —     |
| Verification Oracle ↔ Agent | RAP sub-protocol           | Proof certificates |       ✓       |     ✓     |
| Miri ↔ RAP Server           | In-process interpretation  | Execution traces   |       ✓       |     —     |
| Swarm Bus ↔ Agents          | RAP swarm sub-protocol     | Typed SwarmMessage |       ✓       |     ✓     |
| Lease Manager ↔ Agents      | RAP lease sub-protocol     | SemanticLease      |       ✓       |     ✓     |
| Consensus Engine ↔ Agents   | RAP consensus sub-protocol | ConsensusRound     |       ✓       |     ✓     |
| Semantic VCS ↔ Agents       | RAP VCS sub-protocol       | SemanticOp log     |       ✓       |     —     |
| Orchestrator ↔ Sub-swarms   | RAP hierarchical protocol  | SwarmPlan          |       ✓       |     ✓     |
| ACI Engine ↔ Agents         | RAP ACI sub-protocol       | DynamicWarning     |       ✓       |     ✓     |
| ACI Engine ↔ MLIR           | In-process cost model      | PerfAdvice         |       —       |     ✓     |
| ACI Engine ↔ Swarm History  | Codebase model inference   | SwarmAdvice        |       ✓       |     —     |
| Cost Oracle ↔ Agents        | RAP cost sub-protocol      | CostProfile        |       ✓       |     ✓     |
| Cost Oracle ↔ MLIR          | In-process cost model      | TargetCost         |       —       |     ✓     |
| Synthesis Oracle ↔ Agents   | RAP synthesis sub-protocol | CandidateImpl      |       ✓       |     —     |
| Memory Store ↔ Agents       | RAP memory sub-protocol    | MemoryEntry        |       ✓       |     —     |
| Grammar Registry ↔ Agents   | RAP grammar sub-protocol   | GrammarExtension   |       ✓       |     —     |
| Auto-Repair ↔ Agents        | RAP repair sub-protocol    | RepairCandidate    |       ✓       |     —     |
| Hot-Patch ↔ Runtime         | RAP hotpatch sub-protocol  | PatchResult        |       ✓       |     ✓     |
| FFI Binder ↔ Agents         | RAP FFI sub-protocol       | ForeignBinding     |       ✓       |     ✓     |
| Sandbox ↔ Agent Code        | Capability-gated execution | SandboxResult      |       ✓       |     ✓     |

---

## Summary

Redox transforms Rust from a language *for human developers with CLI tools* into a language *for swarms of AI agents with maximum parsing speed, communication throughput, hardware-agnostic performance, and minimum token cost*. The transformation **shifts safety from compile-time enforcement to a queryable Safety Knowledge Base**, eliminates every source of agent parsing ambiguity, introduces a portable performance IR that targets any hardware, and compresses every language construct to its **minimum token footprint**:

1. **Token-minimal syntax** — every construct is compressed to ≤50% of its Rust token count: `pub fn` → `+f`, `#[derive(Clone, Debug)]` → `@d(Cl,Db)`, `let mut` → `m`, `Option<T>` → `?T`
2. **Zero-ambiguity syntax** — deterministic LL(1) grammar eliminates 100% of agent parsing errors caused by context-sensitive constructs
3. **Safety Knowledge Base (SKB)** — safety rules in a queryable database, not slow compile-time passes; agents pre-validate before writing code
4. **Hardware-agnostic performance via MLIR + LLVM** — dialect-as-semantics architecture with MLIR-native autotuning, automatic device placement, language-level SIMD types, and LLVM backend compiling to 20+ CPU architectures, GPU (AMDGPU, NVPTX, SPIR-V), NPU/TPU, FPGA, WASM from a single source
5. **Sub-microsecond inter-agent communication** — zero-copy typed message bus optimized for swarm throughput over everything else
6. **Performance annotations** — `@pnb` (no bounds check), `@pt(gpu)` / `@pt(auto)` (target GPU / auto-place), `@pv(8)` (vectorize), `@pa(N)` (autotune N variants) — compact and trusted
7. **Effect types** — agents know what functions *do* without reading them
8. **Contracts** — agents verify correctness against formal specifications via SKB, not compiler passes
9. **Capability manifests** — agents discover components by what they *can do*, not what they're named
10. **Structured diagnostics** — machine-actionable fix graphs (opt-in, not blocking)
11. **Semantic ownership for swarms** — concurrent agent access governed by Rust-inspired lease semantics
12. **CRDT-based semantic merging** — concurrent modifications merge at the AST/HIR level, not text level
13. **Consensus protocols** — shared interface changes require structured voting from all affected agents
14. **Swarm-native task decomposition** — compiler dependency graph drives automatic parallelization of work
15. **Semantic version control** — operation-log-based history replaces text diffs
16. **Opt-in compile-time safety profiles** — `safety.mode = "full"` for humans/CI, `"skb-only"` for agents, `"none"` for raw performance
17. **Standard abbreviation registry** — deterministic, versioned compact forms for all std library types and traits
18. **Token budget reporting** — `redox build --token-report` tracks per-function token expenditure for agent optimization
19. **Safety-free syntax** — lifetimes, borrow annotations, `unsafe`, `Send`/`Sync`, `Pin`, `PhantomData`, and all other compile-time safety constructs are eliminated from the syntax; agents consult the SKB; the compiler infers everything else; function signatures are 60–70% shorter
20. **Agentic Compiler Intelligence (ACI)** — the compiler embeds a learned model that provides dynamic warnings (adapted to the project's actual bug patterns), intelligent debugging (causal root-cause analysis), performance advisories (MLIR cost model + profiling data), and swarm coordination intelligence (conflict prediction, decomposition learning)
21. **ACI RAP endpoints** — `aci.warnings`, `aci.debug`, `aci.perf`, `aci.swarm`, `aci.learn`, `aci.explain`, `aci.similar_bugs`, `aci.predict_regression` — all queryable via the standard RAP protocol
22. **Cost model transparency** — every expression, type, and operation has a queryable cost profile (latency, memory, allocations, energy) per target hardware, accessible before code emission; agents make informed decisions, not guess-and-profile
23. **Self-evolving grammar** — agents register domain-specific abbreviations, syntax extensions, and custom patterns; the grammar evolves with the ecosystem, driven by usage frequency, not committee decisions
24. **Synthesis specifications** — formal `spec` blocks with `@req`/`@ens`/`@perf`/`@fx` enable spec-to-code synthesis: agents compose specifications, the compiler generates and verifies candidates, closing the guess-compile-fix cycle
25. **Agent Memory Model** — four-tier persistent memory (ephemeral, session, project, global) enables agents to learn across sessions, accumulate project conventions, and share patterns across the ecosystem
26. **Agentic standard library** — `SwarmVec`, `ArenaVec`, streaming I/O, batch APIs, formal specs on every function, zero-copy swarm serialization — the stdlib redesigned for agent consumption patterns, not human ergonomics
27. **Self-healing compilation** — the compiler auto-repairs errors with confidence-ranked fix candidates, collapsing the emit→error→fix→re-emit loop into emit→auto-fix→confirm; type mismatches, missing imports, unused variables, missing match arms all repaired automatically
28. **Hot-reload** — function-level live patching injects recompiled functions into running processes in <1ms without restart; rollback always available; ABI stability enforced by the MLIR pipeline
29. **Zero-friction FFI** — `@ffi("c", header: "x.h")` auto-generates safe bindings from C/C++ headers, Python type stubs, WASM component interfaces, and CUDA kernels; no `unsafe`, no manual layout, cost oracle includes cross-language overhead
30. **Runtime security** — capability-based sandboxing bounds agent-generated code execution: memory limits, CPU timeouts, syscall allowlists, FFI allowlists, cryptographic audit trails, deterministic replay
31. **Swarm orchestration patterns** — `swarm_map_reduce`, `swarm_pipeline`, `swarm_scatter_gather`, `swarm_saga` as first-class language constructs with compile-time verification (effect purity, contract chaining, deadlock freedom, liveness guarantees)

The compiler becomes an **agentic AI system, synthesis engine, and swarm arbiter**, built on the **MLIR + LLVM** compiler infrastructure — the broadest and most mature in existence. Its MLIR dialect encodes the full language semantics (ownership, effects, contracts) as first-class operations — not metadata on a generic IR — enabling MLIR-native autotuning, automatic device placement, and compile-time metaprogramming that survives through the entire optimization pipeline. Its **Agentic Compiler Intelligence (ACI)** learns from the project's bug history, swarm session outcomes, and codebase patterns to provide dynamic warnings, intelligent debugging, and performance suggestions that static analyzers cannot match. Its **Cost Oracle** transforms agent decision-making from guess-and-profile to query-and-choose. Its **Synthesis Oracle** closes the loop from formal spec to verified implementation. Its **Agent Memory Model** ensures that every lesson learned persists across sessions, projects, and the ecosystem. Its primary job is *making code run fast on any hardware with the fewest tokens possible while actively helping agents write better code*, not blocking submissions with safety errors that agents already know how to avoid. Safety knowledge lives in a database. Performance lives in MLIR's multi-level optimization pipeline and LLVM's battle-tested backends. Communication lives in the swarm bus. Parsing lives in a zero-ambiguity grammar. Compiler intelligence lives in a learned model that improves with every build. Synthesis lives in a formal specification system. Memory lives in a four-tier persistent store. And every construct lives in its **most compressed form** — because tokens are the currency of agentic intelligence, and Redox is designed to spend them wisely.

Every error the compiler detects is an error it can **fix** — auto-repair candidates ranked by confidence eliminate the agent round-trip tax. Every function can be **hot-patched** into a running process without restart — because agent swarms never stop iterating. Every foreign library is accessible through **zero-ceremony FFI** — C headers, Python stubs, WASM components read directly by the compiler. Every agent runs in a **capability-bounded sandbox** — memory-limited, CPU-bounded, audit-trailed, deterministically replayable. And common multi-agent workflows — map-reduce, pipeline, scatter-gather, saga — are **first-class language constructs** verified by the compiler for deadlock freedom and contract satisfaction.

This is not Rust made safe. This is Rust made *fast*, *parseable*, *communicative*, *intelligent*, *self-evolving*, *self-healing*, and *token-efficient* — built on MLIR and LLVM, with an AI-powered compiler, for the age of agent swarms.
