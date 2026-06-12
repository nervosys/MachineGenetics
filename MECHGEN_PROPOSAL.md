# MechGen: An Agentic-First Programming Language for the 21st Century

> Transforming Rust into a Language for Humans and AI Agents Alike

**Version:** 0.1.0-draft  
**Date:** 2026-03-15  
**Status:** Proposal  

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Design Principles](#2-design-principles)
3. [Transformation Methodology](#3-transformation-methodology)
4. [Ontology of the MechGen System](#4-ontology-of-the-mechgen-system)
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

**MechGen** reimagines Rust as an **agentic-first** language — one where AI agents are first-class participants in the development lifecycle. The language is redesigned around twelve pillars: **(1) zero-ambiguity syntax** that eliminates agent parsing errors, **(2) communication-first primitives** that maximize inter-agent bandwidth, **(3) hardware-agnostic high performance** built on **MLIR and LLVM** compiler infrastructure that compiles to any target without sacrificing speed, **(4) token-minimal syntax** that minimizes the tokens agents must emit, because every token costs time, money, and memory, **(5) safety-free syntax simplification** that eliminates lifetimes, borrow annotations, ownership markers, and all other compile-time safety syntax — since agents consult the SKB directly, the syntax need not carry safety information at all, **(6) an agentic compiler** — the compiler itself is an AI-powered system that provides dynamic warnings, intelligent debugging, performance suggestions, and learns from the codebase and the swarm's history, **(7) cost model transparency** — every construct has a queryable cost (cycles, memory, energy, tokens) per target, so agents choose before emitting rather than profiling after, **(8) synthesis-first design** — formal specifications (`@req`/`@ens`/`@perf`/`@fx`) enable spec-to-code synthesis with compiler verification, closing the guess-compile-fix cycle, **(9) persistent agent memory** — a four-tier memory model (ephemeral, session, project, global) that lets agents learn across sessions and share knowledge across the ecosystem, **(10) self-healing compilation** — the compiler auto-repairs errors with ranked fix candidates, collapsing the emit→error→fix→re-emit loop into a single step, **(11) live iteration** — function-level hot-reload patches running processes in sub-millisecond time without restart, and **(12) zero-friction interop** — automatic FFI binding generation from C/C++ headers, Python stubs, WASM interfaces, and GPU kernels with capability-based sandbox security. Safety knowledge moves from compile-time enforcement to a **queryable Safety Knowledge Base (SKB)** — a structured database of rules, invariants, and constraints that agents reference directly, eliminating the compile-time overhead that slows iteration.

By building on MLIR (Multi-Level Intermediate Representation) and LLVM, MechGen inherits the broadest hardware backend ecosystem in existence — 20+ CPU architectures, GPU compute (AMDGPU, NVPTX), WASM, SPIR-V — while gaining MLIR's extensible dialect system for defining custom optimization passes for agent-specific workloads, ML accelerators (NPU/TPU), FPGA synthesis, and domain-specific hardware. MLIR's multi-level abstraction preserves high-level semantic information (parallelism intent, memory layout preferences, effect annotations) deep into the optimization pipeline, where LLVM alone would have discarded it.

Critically, following the architectural insight pioneered by Modular AI's Mojo language, the **MechGen MLIR Dialect is not a translation target — it is the language's semantic backbone**. Ownership, effects, contracts, and performance annotations are first-class MLIR operations and attributes, not metadata bolted onto a generic IR. This means the compiler's semantic understanding is preserved through the entire optimization pipeline, enabling **MLIR-native autotuning** (the compiler generates multiple lowering variants per kernel and benchmarks them per-target), **language-level SIMD types** backed directly by MLIR's vector dialect, and **automatic device placement** where the MLIR pipeline — not the programmer — decides whether a kernel runs on CPU, GPU, or NPU based on cost modeling. These are not Mojo features copied for humans; they are Mojo's architectural insights **re-derived for agent swarms**, where autotuning is a swarm-parallelizable search, device placement is an agent-queryable decision, and SIMD types are token-efficient first-class citizens.

### Core Thesis

> A programming language designed for agent swarms must have **zero syntactic ambiguity**, **maximum inter-agent communication throughput**, **hardware-agnostic performance**, and **minimum token footprint**. The compiler is an **optimizing translator**, not a safety gatekeeper — safety rules live in a queryable knowledge base that agents consult directly, not in slow compile-time passes that duplicate what agents already know. The fundamental bottleneck in agentic software engineering is not correctness (agents can be trained on rules) but **parsing speed**, **communication bandwidth**, **target-portable performance**, and **token efficiency** — because every token an agent emits costs time, money, and memory.

> The fundamental unit of agentic work is not the single agent but the **swarm** — a coordinated ensemble of specialized agents that decompose, parallelize, verify, and integrate changes across a codebase simultaneously. The language, compiler, and toolchain must be designed for this concurrent, collaborative reality from the ground up. Every syntax decision, every compiler pass, every protocol message must be evaluated against: *does this minimize parse errors? does this maximize communication? does this run fast everywhere? does this minimize the tokens an agent must spend?*

### What Changes

| Dimension                 | Rust Today                                          | MechGen                                                                        |
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
The compiler's primary role is transforming MechGen source into the fastest possible target code across all hardware platforms. Its secondary role is serving as a queryable oracle for semantic information. Safety enforcement is delegated to the Safety Knowledge Base and agent-side validation.

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
The system is designed for *many agents operating simultaneously* on the same codebase. Just as Rust's ownership model prevents data races in programs, MechGen's swarm model prevents semantic conflicts between concurrent agent modifications. The compiler acts as an arbiter — agents acquire *semantic locks* on code regions (functions, modules, trait impls) rather than file-level locks, and the system automatically detects and resolves conflicts at the semantic level.

### P9: Compositional Decomposition
Large tasks are automatically decomposable into independent subtasks that can be distributed across a swarm. The compiler's dependency graph, module boundaries, and trait interfaces define natural *seams* along which work can be parallelized. An agent working on module A's implementation cannot break module B's contract if B depends only on A's interface.

### P10: Consensus Before Commit
No change to shared state (public APIs, trait definitions, type signatures) is applied without swarm consensus. The compiler enforces a *propose-verify-accept* protocol where changes to shared interfaces require validation by all dependent agents before integration. This mirrors Rust's `&mut` exclusivity — at the swarm coordination level.

### P11: Zero-Ambiguity Syntax
The language grammar must be **deterministic LL(1)** — every token uniquely determines the parse path with zero backtracking. No context-sensitive parsing, no ambiguous constructs, no lookahead beyond one token. Rust's turbofish (`::<>`), trailing closure ambiguity, type ascription vs. struct literals — all eliminated. Agents parse MechGen with a simple state machine, not a backtracking parser. **The single biggest source of agent coding errors is parsing ambiguity — MechGen eliminates it by design.**

### P12: Communication-First Design
Inter-agent message passing is the highest-priority bottleneck to optimize. Every language construct, every compiler data structure, every protocol message is designed for **zero-copy serialization** and **sub-microsecond latency**. The swarm message bus is not an add-on — it is the foundational primitive around which the entire toolchain is built. Agent-to-agent bandwidth determines swarm performance more than any other factor.

### P13: Hardware-Agnostic Performance via MLIR + LLVM
MechGen code compiles through **MLIR** (Multi-Level Intermediate Representation) and **LLVM** to target any hardware: x86, ARM, RISC-V, WASM, GPU (AMDGPU, NVPTX, SPIR-V), NPU/TPU, FPGA. The **MechGen MLIR Dialect encodes the language's semantics directly** — ownership, effects, contracts, and performance annotations are first-class MLIR operations, not metadata translated from a separate IR. This dialect-as-semantics architecture (pioneered by Modular AI, adapted here for agent swarms) means high-level intent survives through the entire progressive lowering pipeline. LLVM provides battle-tested codegen for 20+ CPU architectures. Language-level SIMD types (`Simd[T, N]`) map directly to MLIR vector dialect ops. **MLIR-native autotuning** generates multiple lowering variants and benchmarks per-target — a search that agent swarms can parallelize across hardware configurations. **Automatic device placement** (`@pt(auto)`) lets the MLIR cost model decide CPU vs GPU vs NPU per kernel, queryable by agents via RAP. Write once, run fast everywhere — on the broadest compiler infrastructure in existence.

### P14: Database-Driven Safety
Safety rules (ownership patterns, borrow violations, lifetime errors, type mismatches) are stored in a **Safety Knowledge Base (SKB)** — a structured, versioned, queryable database. Agents consult the SKB directly instead of waiting for compile-time error messages. The compiler can *optionally* enforce SKB rules at compile time (for human developers or CI pipelines), but this is a policy choice, not a language requirement. This eliminates the compile-time overhead tax that slows agentic iteration cycles from milliseconds to seconds.

### P15: Token Economy
Every language construct must minimize the number of tokens an agent must emit to express intent. Tokens cost time, money, and memory — they are the fundamental unit of agent resource consumption. Every keyword, attribute, delimiter, and syntactic form is designed for **minimum token footprint**: short keywords, compact operators, abbreviated attributes, and structural compression. Where Rust uses `pub fn`, MechGen offers `+f`; where Rust uses `#[derive(Clone, Debug)]`, MechGen offers `@d(Cl,Db)`. The **token-optimal syntax** is not a separate mode — it is the canonical form, designed so that agents express maximal semantic intent per token spent. Human-readable aliases remain available in legacy mode. The goal: any program expressible in N Rust tokens should be expressible in ≤ N/2 MechGen tokens, with no loss of semantics.

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
Agent swarms iterate continuously — stopping a running system to recompile defeats the purpose. MechGen supports **function-level hot-reload**: individual functions can be recompiled and patched into a running process without restart. The MLIR pipeline emits position-independent code with stable ABIs at function boundaries. The swarm can patch a function, observe the result, and roll back within milliseconds. This transforms the development loop from batch compilation to **continuous, incremental, live evolution** — the natural mode for agent swarms that never sleep.

### P24: Zero-Friction Foreign Function Interface
Agents work across language boundaries — calling C libraries, Python ML frameworks, WASM modules, and GPU kernels. MechGen provides a **zero-ceremony FFI** that requires no `unsafe`, no manual struct layout matching, and no binding generators. The compiler reads C headers, Python type stubs, and WASM component model interfaces directly and generates safe MechGen bindings automatically. Cross-language calls are as cheap as intra-language calls when the MLIR pipeline can inline across boundaries.

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
   - **Deliverable:** `mechgen_diagnostics` crate with `DiagnosticGraph` type.

3. **Query API Externalization**
   - The `rustc_query_impl` system uses `QueryVTable`, on-disk caching, and dependency tracking.
   - **Action:** Expose a subset of queries as a stable external API:
     - Type queries: `type_of(DefId)`, `predicates_of(DefId)`, `adt_def(DefId)`
     - Safety queries: `is_freeze(Ty)`, `is_send(Ty)`, `is_sync(Ty)`, `needs_drop(Ty)`
     - MIR queries: `optimized_mir(DefId)`, `mir_borrowck(DefId)`
     - Diagnostic queries: `lint_levels(HirId)`, `check_match(DefId)`
   - **Deliverable:** `mechgen_query` crate, versioned independently of compiler internals.

### Phase 1: Semantic Index — The Knowledge Graph (Months 4–12)

**Goal:** Build a persistent, queryable knowledge graph of all code semantics.

**Work Streams:**

4. **Semantic Code Index**
   - Merge the capabilities of `rustdoc-json-types` (documentation), `rust-analyzer`'s `Analysis` (IDE queries), and `rustc_public` (compiler semantics) into a unified index.
   - **Action:** Create `mechgen_index`, a persistent database that stores:
     - All items with full type signatures, trait bounds, and lifetime parameters
     - Cross-reference graph (callers, callees, implementors, dependents)
     - Capability manifests (what effects each function has: I/O, allocation, panic, unsafe)
     - Natural-language documentation linked to semantic entities
   - **Deliverable:** `mechgen_index` crate with both in-memory and on-disk backends.

5. **Capability Manifests**
   - Extend the type system with *effect annotations* (inspired by Rust's existing `const`, `async`, `unsafe` qualifiers).
   - **Action:** Introduce capability tags on functions:
     ```
     #[capabilities(io::read, alloc::heap, panic::unwind)]
     fn process_file(path: &Path) -> Result<Data, Error> { ... }
     ```
   - These are inferred by the compiler for any function body, or declared explicitly on trait methods and FFI boundaries.
   - **Deliverable:** `mechgen_capabilities` analysis pass integrated into MIR transform pipeline.

### Phase 2: Agent Protocol — The Interface Contract (Months 8–18)

**Goal:** Define how agents interact with the compiler and each other.

**Work Streams:**

6. **MechGen Agent Protocol (RAP)**
   - A structured protocol (analogous to LSP but for *compilation semantics*, not just IDE features) that agents use to:
     - Submit code for analysis (incremental)
     - Query types, traits, lifetimes, borrow constraints
     - Request transformations (refactors with pre/post-condition checking)
     - Receive verification results (safety proofs, capability audits)
   - **Action:** Define RAP as a typed RPC protocol with request/response schemas derived from `mechgen_query` types.
   - **Deliverable:** `mechgen_protocol` crate + reference server implementation.

7. **Agent Capability System**
   - Agents operating on MechGen code are themselves subject to capability bounds:
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
   - **Deliverable:** `mechgen_agent` crate with capability checking.

8. **Verification Oracle**
   - Extend the compiler's verification beyond type-checking into a continuous verification service:
     - Pre-commit: "Will this change compile? Will it break dependents?"
     - Post-synthesis: "Does this generated code satisfy the specification?"
     - Invariant monitoring: "Does this crate maintain its safety contracts across versions?"
   - Built on `rustc_borrowck`, `rustc_const_eval`, `rustc_pattern_analysis`, and `rustc_transmute`.
   - **Deliverable:** `mechgen_verify` crate exposing verification as a composable service.

### Phase 3: Language Evolution — MechGen Syntax and Semantics (Months 12–24)

**Goal:** Introduce language features that make MechGen natively agent-friendly while remaining human-ergonomic.

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
   - **Deliverable:** Contract syntax + `mechgen_contracts` analysis pass.

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
      #[agent::deprecation_path("use mechgen_crypto::aead instead")]
      pub fn encrypt(key: &Key, plaintext: &[u8]) -> Vec<u8> { ... }
      ```
    - **Deliverable:** `mechgen_attrs` attribute namespace + processing in `rustc_attr_parsing`.

---

## 4. Ontology of the MechGen System

### 4.1 Top-Level Ontology

```
MechGen System
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
│   ├── Build System (MechGen Build — multi-target orchestration)
│   ├── Package Manager (capability-indexed registry)
│   ├── Formatter (mechgenfmt — canonical form enforcement)
│   ├── Linter (mechgen-lint — opt-in)
│   ├── Documentation (mechgen-doc)
│   ├── Interpreter (MechGen Interpret — opt-in UB detection)
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

Each existing `rustc_*` crate maps to a MechGen subsystem with its agent-facing interface:

#### Frontend Pipeline

| Rust Crate           | MechGen Subsystem | Agent Interface                                                                         |
| -------------------- | --------------- | --------------------------------------------------------------------------------------- |
| `rustc_lexer`        | `mechgen_lexer`   | Token stream API: agents can tokenize arbitrary source fragments                        |
| `rustc_parse`        | `mechgen_parse`   | Parse API: agents submit source, receive AST with full span info                        |
| `rustc_ast`          | `mechgen_ast`     | AST query: agents traverse, pattern-match, and transform AST nodes                      |
| `rustc_expand`       | `mechgen_expand`  | Macro expansion API: agents can expand macros incrementally and observe transformations |
| `rustc_ast_lowering` | `mechgen_lower`   | Lowering API: agents observe AST→HIR transformation with semantic annotations           |
| `rustc_resolve`      | `mechgen_resolve` | Name resolution API: agents query what any name resolves to in any scope                |

#### Middle (Semantic Analysis)

| Rust Crate               | MechGen Subsystem    | Agent Interface                                                              |
| ------------------------ | ------------------ | ---------------------------------------------------------------------------- |
| `rustc_hir`              | `mechgen_hir`        | HIR query: agents access desugared, resolved program structure               |
| `rustc_hir_analysis`     | `mechgen_typecheck`  | Type query: agents ask "what is the type of X in context Y?"                 |
| `rustc_hir_typeck`       | `mechgen_infer`      | Inference query: agents observe type inference decisions and constraints     |
| `rustc_trait_selection`  | `mechgen_traits`     | Trait query: "does T implement Trait? which impl? what are the bounds?"      |
| `rustc_borrowck`         | `mechgen_borrow`     | Borrow query: "is this borrow valid? what conflicts? what are the regions?"  |
| `rustc_infer`            | `mechgen_unify`      | Unification query: agents observe and query type unification state           |
| `rustc_middle`           | `mechgen_middle`     | Central type registry: all `Ty`, `TyKind`, `Predicate`, `Region` definitions |
| `rustc_const_eval`       | `mechgen_consteval`  | Const evaluation query: "what does this const expression evaluate to?"       |
| `rustc_pattern_analysis` | `mechgen_patterns`   | Pattern query: "is this match exhaustive? what cases are missing?"           |
| `rustc_privacy`          | `mechgen_visibility` | Visibility query: "is this item accessible from this module/crate?"          |
| `rustc_transmute`        | `mechgen_transmute`  | Transmute query: "is this transmutation safe? what assumptions are needed?"  |

#### Backend Pipeline

| Rust Crate            | MechGen Subsystem   | Agent Interface                                                               |
| --------------------- | ----------------- | ----------------------------------------------------------------------------- |
| `rustc_mir_build`     | `mechgen_mir_build` | MIR construction: agents observe HIR→MIR lowering                             |
| `rustc_mir_transform` | `mechgen_mir_opt`   | MIR optimization: agents query which passes ran and their effects             |
| `rustc_mir_dataflow`  | `mechgen_dataflow`  | Dataflow query: agents access liveness, reachability, initialization analysis |
| `rustc_codegen_ssa`   | `mechgen_codegen`   | Codegen query: agents observe MIR→target code translation                     |
| `rustc_codegen_llvm`  | `mechgen_llvm`      | LLVM backend: agents can inspect generated LLVM IR                            |
| `rustc_monomorphize`  | `mechgen_mono`      | Monomorphization query: agents see concrete instantiations                    |

#### Infrastructure

| Rust Crate          | MechGen Subsystem     | Agent Interface                                      |
| ------------------- | ------------------- | ---------------------------------------------------- |
| `rustc_errors`      | `mechgen_diagnostics` | Structured diagnostic API with fix graphs            |
| `rustc_lint`        | `mechgen_lint`        | Lint registration and query API                      |
| `rustc_session`     | `mechgen_session`     | Session configuration and state                      |
| `rustc_span`        | `mechgen_span`        | Source location management                           |
| `rustc_query_impl`  | `mechgen_query`       | Query system: the backbone of all agent interactions |
| `rustc_interface`   | `mechgen_interface`   | Top-level compiler invocation API                    |
| `rustc_feature`     | `mechgen_features`    | Feature gate query and management                    |
| `rustc_metadata`    | `mechgen_metadata`    | Crate metadata serialization and loading             |
| `rustc_incremental` | `mechgen_incremental` | Incremental compilation infrastructure               |

### 4.3 Tooling Ontology

| Rust Tool       | MechGen Tool        | Agent Interface                                                        |
| --------------- | ----------------- | ---------------------------------------------------------------------- |
| `cargo`         | `mechgen build`     | Build orchestration API: dependency resolution, compilation scheduling |
| `rustfmt`       | `mechgenfmt`        | Format API: agents request formatting with configurable style          |
| `clippy`        | `mechgen lint`      | Extended lint API: agents register custom lints, query lint results    |
| `rustdoc`       | `mechgen doc`       | Documentation generation with semantic linking                         |
| `miri`          | `mechgen interpret` | Interpretation API: agents run code in sandbox with full UB detection  |
| `rust-analyzer` | `mechgen analyze`   | Merged into RAP server: all IDE features available programmatically    |
| `compiletest`   | `mechgen test`      | Test infrastructure with property-based verification                   |
| `rustc-perf`    | `mechgen perf`      | Performance query: agents benchmark and profile code changes           |

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
├── Compile-Time (Opt-In — controlled by MechGen.toml safety profiles)
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
│   ├── Effect System [NEW IN MECHGEN]
│   │   ├── io — filesystem, network, system calls
│   │   ├── alloc — heap allocation
│   │   ├── panic — unwinding, abort
│   │   ├── unsafe — raw pointer operations, FFI
│   │   ├── async — asynchronous suspension points
│   │   └── custom — user-defined effects
│   │
│   ├── Contract System [NEW IN MECHGEN] (opt-in verification)
│   │   ├── Preconditions (#[requires])
│   │   ├── Postconditions (#[ensures])
│   │   ├── Invariants (#[invariant])
│   │   └── Refinement types (bounded integers, non-empty collections)
│   │
│   └── Capability System [NEW IN MECHGEN]
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
├── Performance Infrastructure [NEW IN MECHGEN — MLIR + LLVM]
│   ├── MLIR-based multi-level IR (MechGen Dialect → Linalg/Affine → LLVM Dialect)
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
├── Self-Evolving Grammar [NEW IN MECHGEN]
│   ├── Agent-registerable domain-specific abbreviations
│   ├── Namespace-scoped syntax extensions (version-controlled)
│   ├── Frequency-driven promotion (ACI suggests new abbreviations)
│   └── Grammar extension discovery API
│
├── Synthesis Infrastructure [NEW IN MECHGEN]
│   ├── Formal specification syntax (spec blocks with @req/@ens/@perf/@fx)
│   ├── Synthesis oracle (spec → candidate implementations)
│   ├── Verification oracle (candidate → spec satisfaction proof)
│   ├── Pipeline composition from specs
│   └── Cost-constrained synthesis (agents specify budget)
│
├── Agent Memory Model [NEW IN MECHGEN]
│   ├── Ephemeral memory (per-task scratchpad)
│   ├── Session memory (per-swarm-session patterns and caches)
│   ├── Project memory (conventions, bug patterns, perf profiles)
│   └── Global memory (cross-project ecosystem patterns)
│
├── Self-Healing Compiler [NEW IN MECHGEN]
│   ├── Auto-repair pipeline (error → infer intent → generate fix candidates)
│   ├── Confidence-ranked fixes with token cost accounting
│   └── Accept/reject/refine feedback loop (agent always in control)
│
├── Hot-Reload Runtime [NEW IN MECHGEN]
│   ├── Function-level live patching (sub-ms injection)
│   ├── ABI stability enforcement at MLIR level
│   ├── Rollback with retention window
│   └── Active call draining (no forced interruption)
│
├── Zero-Friction FFI [NEW IN MECHGEN]
│   ├── Auto-binding from C/C++ headers, Python stubs, WASM .wit, CUDA kernels
│   ├── Safe wrappers with null checks and length validation
│   ├── Cost oracle integration (cross-language overhead visible)
│   └── Zero-copy data passing where possible (buffer protocol)
│
├── Runtime Security [NEW IN MECHGEN]
│   ├── Capability-based sandboxing (memory, CPU, syscall, FFI bounds)
│   ├── Capability attenuation (child ≤ parent capabilities)
│   ├── Cryptographic audit trail (every sandbox execution logged)
│   └── Deterministic replay for audit and debugging
│
├── Swarm Orchestration Patterns [NEW IN MECHGEN]
│   ├── Map-reduce (parallel map, single-agent reduce)
│   ├── Pipeline (staged with backpressure)
│   ├── Scatter-gather (broadcast + quorum-based collection)
│   ├── Saga (distributed transaction with compensation)
│   └── Compile-time verification (effect purity, contract chaining, deadlock freedom)
│
├── Agentic Compiler Intelligence (ACI) [NEW IN MECHGEN]
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

MechGen supports **dual syntax modes**. The **canonical syntax** (default) is a zero-ambiguity LL(1) grammar optimized for agent parsing. The **legacy syntax mode** accepts standard Rust and transpiles to canonical form. All valid Rust programs can be compiled in legacy mode. The `mechgen fmt --canonicalize` command converts Rust source to canonical MechGen. New features (effects, contracts, performance annotations, SKB integration) are only available in canonical syntax.

### 5.2 New Syntax and Semantics

#### 5.2.1 Safety-Free Function Signatures

Since agents consult the SKB for safety rules, the syntax carries no safety annotations:

```rust
// Rust: lifetimes, borrows, ownership markers everywhere
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { ... }
fn process(data: &mut Vec<u8>, config: &Config) -> Result<(), Error> { ... }
unsafe fn raw_read(ptr: *const u8, len: usize) -> &[u8] { ... }

// MechGen: no lifetimes, no borrow annotations, no unsafe keyword
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

Rust's syntax, while ergonomic for humans, causes systematic agent parsing failures due to context-sensitive constructs and ambiguous token sequences. MechGen's **canonical syntax** eliminates every known source of agent parse errors:

#### 5.3.1 Ambiguity Eliminations

| Rust Ambiguity                                     | Agent Failure Mode                         | MechGen Solution                                               |
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
- **Canonical form**: `mechgenfmt` produces one unique canonical representation per AST — agents never face formatting-induced parse variations

#### 5.3.4 Dual Syntax Mode

For human developers transitioning from Rust, MechGen supports a **legacy syntax mode** that accepts standard Rust syntax and transpiles to canonical form:

```bash
mechgen build --syntax=legacy    # accepts Rust syntax, transpiles
mechgen build --syntax=canonical # default: zero-ambiguity syntax only
mechgen fmt --canonicalize       # convert legacy Rust syntax to canonical MechGen
```

### 5.4 Hardware-Agnostic Performance Model (MLIR + LLVM)

MechGen compiles through **MLIR** (Multi-Level Intermediate Representation) and **LLVM** — the broadest and most mature compiler infrastructure in existence. Following the key architectural insight from Modular AI's Mojo (but adapted for agent swarms, not human ML engineers): **the MLIR dialect is not a translation target — it is the language's semantic backbone**. Ownership, effects, contracts, performance annotations, and agent capability declarations are encoded as first-class MLIR operations and attributes in the MechGen Dialect. This means the compiler's full semantic understanding is preserved through the entire optimization pipeline — unlike a traditional approach where MIR→LLVM IR lowering discards high-level intent.

MLIR provides extensible multi-level abstractions that preserve performance intent (parallelism, memory layout, vectorization) through progressive lowering, while LLVM provides battle-tested optimization and native code generation for 20+ architectures.

#### 5.4.1 MLIR-Based Compilation Pipeline

```
Source → AST → HIR → MIR → MLIR (MechGen Dialect) → MLIR (Lowered) → Target Code
                              │                       │
                              │ Progressive Lowering:  │
                              │ ┌─────────────────┐   │
                              ├─┤ MechGen Dialect    │   │  (effects, contracts, perf annotations)
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

| Criterion                    | LLVM Alone                       | MLIR + LLVM (MechGen)                                       |
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
    v sum = Simd[f32, 8].zero;       // language-level type, not mechgen::simd::Vector
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

Every token an agent emits costs **time** (inference latency), **money** (API billing), and **memory** (context window consumption). MechGen's canonical syntax is designed to express maximum semantic intent per token. The goal: any program expressible in N Rust tokens should be expressible in ≤ N/2 MechGen tokens with identical semantics.

#### 5.5.1 Keyword Compression Table

| Rust Keyword / Construct        | Tokens | MechGen Compact Form | Tokens |      Savings      |
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

MechGen attributes use single-character prefixes and abbreviated names:

```
Rust                                    MechGen Compact
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

**MechGen Compact (19 tokens):**
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

**MechGen Compact (30 tokens):**
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

| Pattern                                              | Rust Tokens | MechGen Compact        | MechGen Tokens |
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

The MechGen compiler enforces these token economy properties:

1. **No construct requires more tokens than its Rust equivalent** — every MechGen form is ≤ the token count of the corresponding Rust form
2. **High-frequency constructs get the shortest forms** — token length is inversely proportional to usage frequency across all known Rust codebases
3. **`mechgenfmt --agent`** produces the minimum-token canonical form; **`mechgenfmt --human`** produces the human-readable expanded form
4. **Token budget reporting**: `mechgen build --token-report` emits per-function and per-module token counts, enabling agents to track and optimize their token expenditure
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

| Rust Syntax / Concept                   | Purpose (for humans)                    | MechGen (for agents)                               | Rationale                                                           |
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

With safety syntax eliminated, MechGen function signatures become radically simpler:

```
// Rust (21 tokens):
pub fn merge<'a, 'b, T: Send + Sync + Clone + 'a>(
    left: &'a [T],
    right: &'b [T],
) -> Vec<T>
where
    'b: 'a,

// MechGen (8 tokens):
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

// MechGen (8 tokens):
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

// MechGen (8 tokens):
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

| Metric                                       | Rust     | MechGen (safety-free) | Savings        |
| -------------------------------------------- | -------- | ------------------- | -------------- |
| Average tokens per function signature        | 12–25    | 4–8                 | 60–70%         |
| Lifetime annotations per 1000 LOC            | 15–50    | 0                   | 100%           |
| `unsafe` blocks per 1000 LOC (systems code)  | 5–20     | 0                   | 100%           |
| Trait bounds per generic function            | 3–6      | 0–2 (semantic only) | 50–100%        |
| Where clauses per 1000 LOC                   | 10–30    | 0–5 (semantic only) | 80–100%        |
| Total token reduction (cumulative with §5.5) | N tokens | ≤N/3 tokens         | **67%+ fewer** |

### 5.7 Cost Model Transparency: Query Before You Emit

Agents are economic actors — every choice they make (which type, which algorithm, which allocation strategy) has a measurable cost. MechGen exposes a **cost oracle** that agents query *before* emitting code, turning compilation from a feedback loop into a feed-forward pipeline.

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
# MechGen.toml — project-level grammar extensions
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

MechGen treats formal specifications as first-class inputs to the compiler. Agents don't write code from scratch — they compose specifications, and the compiler verifies that candidate implementations satisfy them.

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

When an agent emits code that doesn't compile, the traditional response is an error message. The agent reads the error, reasons about the fix, emits corrected code, and recompiles. This loop wastes tokens, latency, and agent compute. MechGen's ACI collapses this loop.

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

| Error Class              | Auto-Repair Strategy                            | Confidence | Token Savings |
| ------------------------ | ----------------------------------------------- | ---------- | ------------- |
| Type mismatch (widening) | Insert cast                                     | 0.95+      | 5-15 tokens   |
| Missing import           | Add import from project memory / crate registry | 0.99       | 3-8 tokens    |
| Unused variable          | Prefix with `_` or remove                       | 0.99       | 1-3 tokens    |
| Missing struct field     | Insert field with default value from spec       | 0.85       | 5-20 tokens   |
| Wrong argument order     | Reorder based on type matching                  | 0.90       | 0 tokens      |
| Missing return           | Infer return expression from contract `@ens`    | 0.80       | 3-10 tokens   |
| Off-by-one in loop       | Correct bound from spec `@req`/`@ens`           | 0.75       | 1-2 tokens    |
| Missing match arm        | Generate from exhaustiveness analysis           | 0.95       | 5-30 tokens   |

### 5.11 Hot-Reload: Live Function Patching

Agent swarms iterate continuously. Stopping a running system to recompile is incompatible with the agentic paradigm. MechGen supports **function-level hot-reload** — individual functions recompiled and injected into a running process without restart.

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

Agents routinely cross language boundaries — calling C libraries, Python ML frameworks, WASM modules, GPU kernels. MechGen's FFI requires **zero ceremony**: no `unsafe`, no manual layout, no binding generators.

#### 5.12.1 Automatic Binding Generation

```rust
// Import a C library — compiler reads the header directly
@ffi("c", header: "openssl/evp.h", link: "ssl")
mod openssl;

// Use it like native MechGen code — no unsafe, no manual types
v ctx = openssl.EVP_CIPHER_CTX_new();
openssl.EVP_EncryptInit_ex(ctx, openssl.EVP_aes_256_gcm(), ...);
// Compiler auto-generates safe wrappers with null checks, length validation

// Import Python — compiler reads type stubs (.pyi)
@ffi("python", module: "torch", stubs: "torch.pyi")
mod torch;

v tensor = torch.randn([3, 224, 224]);
v result = torch.nn.functional.relu(tensor);
// Data crosses Python↔MechGen boundary via zero-copy buffer protocol

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

| Language   | Mechanism                      | Zero-Copy      | Auto-Bind   | Overhead      |
| ---------- | ------------------------------ | -------------- | ----------- | ------------- |
| C          | Direct ABI call via LLVM       | ✓              | ✓ (headers) | ~5ns          |
| C++        | C-compatible subset + mangling | ✓              | ✓ (headers) | ~10ns         |
| Python     | Buffer protocol + type stubs   | ✓ (numpy)      | ✓ (.pyi)    | ~200ns        |
| WASM       | Component Model                | ✓ (shared mem) | ✓ (.wit)    | ~50ns         |
| CUDA/HIP   | MLIR GPU dialect lowering      | ✓              | ✓ (kernels) | ~1µs (launch) |
| JavaScript | WASM interop + typed arrays    | ✓ (buffers)    | Partial     | ~100ns        |

---

## 6. Compiler Architecture for Agents

### 6.1 The Query Oracle

The MechGen compiler exposes its entire semantic model through a query interface. Every piece of information the compiler computes is available as a named, typed, cached query.

```
┌─────────────────────────────────────────────────┐
│                 Agent / IDE / CLI                │
├─────────────────────────────────────────────────┤
│              MechGen Agent Protocol (RAP)         │
├───────┬───────┬───────┬───────┬────────┬────────┤
│ Parse │ Types │Borrow │ MIR   │ Diag   │ Verify │
│Queries│Queries│Queries│Queries│Queries │Queries │
├───────┴───────┴───────┴───────┴────────┴────────┤
│              mechgen_query (Stable API)            │
├─────────────────────────────────────────────────┤
│         Incremental Query Engine (Salsa)         │
├─────────────────────────────────────────────────┤
│         MLIR (MechGen Dialect → LLVM Dialect)       │
├─────────────────────────────────────────────────┤
│    LLVM Backend (20+ CPU, GPU, WASM targets)     │
├─────────────────────────────────────────────────┤
│    Compiler Internals (rustc_* crate graph)      │
└─────────────────────────────────────────────────┘
```

### 6.2 Structured Diagnostic Graph

Instead of flat error messages, MechGen emits **diagnostic graphs**:

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
    documentation_url: "https://doc.mechgen-lang.org/error/E0502",
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
    compiler_version: "mechgen 1.0.0",
    hash: "sha256:abc123...",
}
```

### 6.4 Agentic Compiler Intelligence

The MechGen compiler is not a static analyzer — it is an **agentic AI system** that learns, adapts, and collaborates with the agent swarm. This is the second major departure from traditional compiler design: the compiler itself embeds AI capabilities that go far beyond fixed lint passes and static analysis.

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

Just as Rust's type system enforces memory safety through ownership, MechGen enforces *codebase safety* through a **semantic ownership model for agent swarms**. The core insight: Rust already solved concurrent access to shared mutable state — we apply the same discipline at the agent coordination level.

```
Rust Memory Model              MechGen Swarm Model
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

A MechGen swarm is a **directed acyclic graph of specialized agents** that mirrors the compiler's own pass structure:

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

MechGen uses **semantic CRDTs** (Conflict-free Replicated Data Types) for concurrent codebase modification. Unlike text-level CRDTs (which merge character-by-character), semantic CRDTs operate on the AST/HIR:

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

MechGen replaces file-based version control (git) with **semantic version control** built into the compiler:

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

| Pattern        | Verified Property                       | Mechanism                        |
| -------------- | --------------------------------------- | -------------------------------- |
| Map-Reduce     | Map function is pure (`@fx none`)       | Effect system                    |
| Pipeline       | Stage contracts chain (`@ens` → `@req`) | Contract verification            |
| Scatter-Gather | Gather handles partial responses        | Exhaustiveness check on quorum   |
| Saga           | Every action has a compensating action  | Structural completeness check    |
| All patterns   | No swarm deadlocks possible             | Dependency graph cycle detection |
| All patterns   | Timeout guarantees progress             | Bounded liveness analysis        |

### 7.11 Agent Memory Model: Persistent Learning Across Sessions

Swarm agents are not stateless — they accumulate knowledge across sessions. MechGen provides a structured **Agent Memory Model** that persists patterns, decisions, and project-specific knowledge.

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

### 8.1 MechGen Build (Evolution of Cargo)

```toml
# MechGen.toml (evolution of Cargo.toml)
[package]
name = "flight-controller"
version = "2.1.0"
edition = "mechgen-2026"
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

The MechGen Agent Protocol server replaces separate tools with a unified service:

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
│   ├── `--agent` mode: minimum-token canonical form for agents
│   ├── `--human` mode: human-readable verbose form
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
│   ├── MIR → MLIR (MechGen Dialect) translation — dialect encodes full language semantics
│   ├── Progressive lowering: MechGen → Linalg/Affine → Vector → LLVM Dialect
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
use mechgen_swarm::{Swarm, SwarmAgent, Role, SemanticLease, SwarmBus, Consensus};
use mechgen_agent::{Agent, Capability, Session};

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

The MechGen standard library is redesigned for agent consumption patterns. Where Rust's stdlib is optimized for human ergonomics (readable names, discoverable method chains), MechGen's is optimized for **minimum-token, maximum-throughput agent interaction**.

#### 8.4.1 Design Principles

1. **Batch-first APIs**: Every collection operation has a batch variant that processes multiple items in a single call, reducing round-trip overhead
2. **Streaming by default**: I/O operations return streaming iterators, not fully-buffered results — agents process data incrementally, matching their token-streaming nature
3. **Serializable state**: Every stdlib type implements zero-copy serialization for swarm bus transport — moving data between agents costs zero allocation
4. **Cost-annotated**: Every method has a queryable cost profile per target hardware
5. **Spec-documented**: Every function has a formal specification (`@req`/`@ens`/`@fx`) alongside its implementation — agents synthesize calling code from specs, not docs

#### 8.4.2 Key Differences from Rust's stdlib

| Aspect                | Rust stdlib                         | MechGen stdlib                                                                |
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

// MechGen: streams data lazily, matching agent token-streaming nature
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

MechGen introduces the **Safety Knowledge Base (SKB)** — a structured, versioned, queryable database of all safety rules, patterns, invariants, and constraints. Agents query the SKB *before* writing code, not after. The compiler becomes an *optimizing translator* that trusts well-formed input, not a safety gatekeeper that blocks every submission.

```
Rust Model (Compiler-Enforced):
  Agent writes code → Compiler rejects → Agent reads error → Agent rewrites → Compiler accepts
  Latency: seconds per iteration (compile + parse errors + resubmit)

MechGen Model (SKB-Driven):
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
use mechgen_skb::{SafetyKB, Context, Pattern};

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
# MechGen.toml
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

| Metric                                 | Rust (Compiler-Enforced)    | MechGen (SKB-Driven)                           | Improvement          |
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

Agent swarms execute code — and code execution requires security guarantees that go beyond compile-time safety. MechGen provides a **capability-based runtime security model** that sandboxes agent-generated code.

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

| Property                    | Mechanism                                        | Enforcement |
| --------------------------- | ------------------------------------------------ | ----------- |
| **Memory isolation**        | Per-agent address space (WASM-style)             | Runtime     |
| **Resource limits**         | CPU time, memory, file handles bounded           | Runtime     |
| **Capability attenuation**  | Child agents inherit ≤ parent capabilities       | Static + RT |
| **Code integrity**          | Agent-generated code content-addressed (SHA-256) | Runtime     |
| **Audit trail**             | Every sandbox execution logged with agent ID     | Runtime     |
| **Deterministic execution** | Same input + same sandbox = same output          | By design   |
| **Rollback on violation**   | Capability violation → sandbox terminated + undo | Runtime     |
| **Cross-agent isolation**   | No shared mutable state between sandboxes        | Runtime     |

#### 9.7.3 Trust Levels

```toml
# MechGen.toml — security configuration
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

### 10.2 The MechGen Solution: Structured Capability Manifests

Every crate publishes a **capability manifest** alongside its code:

```json
{
  "crate": "mechgen_crypto",
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
// Returns: [mechgen_crypto::aead, ring::aead, ...]
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
- [ ] Fork and rebrand compiler crates (`rustc_*` → `mechgen_*`)
- [ ] Implement zero-ambiguity LL(1) canonical grammar and parser
- [ ] Implement token-compressed keyword set and lexer (single-char keywords, sigil prefixes)
- [ ] Implement safety elision pass (P33): strip all lifetime, borrow, ownership syntax in agentic mode
- [ ] Implement safety-free type inference: compiler infers `&`/`&mut`, `move`/`ref`, `dyn`/`impl` from usage
- [ ] Build dual-syntax transpiler (legacy Rust → canonical MechGen compact form)
- [ ] Implement `mechgenfmt --agent` (minimum-token canonical form) and `mechgenfmt --human` (human-readable form)
- [ ] Stabilize `mechgen_public` API to cover all MIR, HIR, and type system constructs
- [ ] Define MechGen MLIR dialect: ownership, effects, contracts, perf annotations as first-class MLIR ops
- [ ] Implement MIR → MLIR (MechGen Dialect) translation layer (thin boundary, dialect-as-semantics)
- [ ] Implement language-level SIMD types (`Simd[T, N]`) backed by MLIR vector dialect ops
- [ ] Implement MLIR progressive lowering pipeline: MechGen Dialect → Linalg/Affine → Vector → LLVM Dialect
- [ ] Wire LLVM backend codegen through MLIR LLVM Dialect (replacing direct MIR→LLVM IR path)
- [ ] Implement compile-time metaprogramming (`@pp` / `@parameter`) via MLIR constant folding
- [ ] Implement Structured Diagnostics Protocol (JSON diagnostic graphs)
- [ ] Externalize core queries as stable API (`mechgen_query`)
- [ ] Establish CI/CD pipeline for the MechGen compiler
- [ ] Implement semantic region decomposition in compiler query system
- [ ] Define standard abbreviation registry v1 (core types, traits, derives)

### Phase 1: SKB + Swarm Primitives + Multi-Target + Cost Oracle (Months 4–12)
- [ ] Build Safety Knowledge Base (SKB) with initial rule corpus (ownership, borrowing, lifetimes, types)
- [ ] Implement SKB query API (`mechgen_skb` crate)
- [ ] Make all safety compiler passes opt-in via `MechGen.toml` safety profiles
- [ ] Build `mechgen_index` (persistent semantic knowledge graph)
- [ ] Implement capability inference pass in MIR pipeline
- [ ] Extend `mechgen_metadata` with capability manifest serialization
- [ ] Build prototype RAP server (merging rust-analyzer + compiler queries)
- [ ] Implement agent discovery attributes in compact form (`@as`, `@ac`, `@ax`, `@ao`, `@ae`)
- [ ] Implement attribute compression system (full `#[...]` → compact `@...` mapping)
- [ ] Implement token budget reporting (`mechgen build --token-report`)
- [ ] Implement Cost Oracle (P38): per-target cost queries for expressions, types, and operations
- [ ] Implement multi-target cost comparison API (`cost.compare` endpoint)
- [ ] Implement semantic lease manager (shared read / exclusive write on code regions)
- [ ] Build CRDT-based semantic merge engine for concurrent AST/HIR modifications
- [ ] Implement swarm message bus with zero-copy serialization (sub-µs latency)
- [ ] Validate MLIR→LLVM backend targets: x86-64, AArch64, WASM

### Phase 2: Agent Protocol + Swarm Coordination + GPU/NPU Targets + ACI (Months 8–18)
- [ ] Define and implement MechGen Agent Protocol (RAP) specification
- [ ] Build agent capability system and enforcement layer
- [ ] Implement verification oracle (contracts, effects, capabilities) as opt-in service
- [ ] Build swarm SDK (`mechgen_swarm` crate with orchestrator, synthesizer, verifier roles)
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
- [ ] Implement effect type system in `mechgen_hir_analysis`
- [ ] Implement contract syntax and checking in `mechgen_contracts`
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
- [ ] Define `mechgen-2026` edition with all new features including token-compact canonical form
- [ ] Build verification certificate emission pipeline (opt-in for safety-critical)
- [ ] Implement swarm-of-swarms hierarchical orchestration for million-LOC+ codebases
- [ ] Implement MLIR→CIRCT pipeline for FPGA targets (Verilog/SystemVerilog synthesis)
- [ ] Implement MLIR StableHLO/TOSA dialect pipelines for NPU/TPU targets

### Phase 4: Ecosystem (Months 18–30)
- [ ] Build capability-indexed package registry
- [ ] Migrate core ecosystem crates with capability manifests
- [ ] Build agent swarm marketplace and pre-composed swarm templates
- [ ] Develop certification pipeline for safety-critical industries (opt-in full safety mode)
- [ ] Publish MechGen language specification
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

| Category                | Feature                                | Agent Queryable | Agent Discoverable |    Safety Relevant    |
| ----------------------- | -------------------------------------- | :-------------: | :----------------: | :-------------------: |
| **Types**               | Primitives (bool, i32, f64, ...)       |        ✓        |         ✓          |           —           |
|                         | Structs                                |        ✓        |         ✓          |      ✓ (layout)       |
|                         | Enums                                  |        ✓        |         ✓          |  ✓ (exhaustiveness)   |
|                         | Unions                                 |        ✓        |         ✓          |   ✓ (unsafe access)   |
|                         | Tuples                                 |        ✓        |         ✓          |           —           |
|                         | Arrays / Slices                        |        ✓        |         ✓          |      ✓ (bounds)       |
|                         | References (&T, &mut T)                |        ✓        |         ✓          |     ✓ (borrowing)     |
|                         | Raw Pointers (*const T, *mut T)        |        ✓        |         ✓          |      ✓ (unsafe)       |
|                         | Function Pointers                      |        ✓        |         ✓          |      ✓ (effects)      |
|                         | Trait Objects (dyn Trait)              |        ✓        |         ✓          |   ✓ (vtable safety)   |
|                         | impl Trait                             |        ✓        |         ✓          |           —           |
|                         | Never type (!)                         |        ✓        |         ✓          |    ✓ (unreachable)    |
|                         | Refinement types [NEW]                 |        ✓        |         ✓          |   ✓ (value bounds)    |
| **Traits**              | Auto traits (Send, Sync, Unpin)        |        ✓        |         ✓          |   ✓ (thread safety)   |
|                         | Marker traits (Copy, Sized)            |        ✓        |         ✓          |  ✓ (move semantics)   |
|                         | Operator traits (Add, Deref, ...)      |        ✓        |         ✓          |           —           |
|                         | Fn traits (Fn, FnMut, FnOnce)          |        ✓        |         ✓          |  ✓ (closure capture)  |
|                         | Custom traits                          |        ✓        |         ✓          |     ✓ (contracts)     |
| **Lifetimes**           | Named lifetimes ('a)                   |        ✓        |         ✓          |  ✓ (use-after-free)   |
|                         | Elided lifetimes                       |        ✓        |         —          |           ✓           |
|                         | 'static                                |        ✓        |         ✓          |           ✓           |
|                         | Higher-ranked (for<'a>)                |        ✓        |         ✓          |           ✓           |
| **Generics**            | Type parameters                        |        ✓        |         ✓          |           —           |
|                         | Const generics                         |        ✓        |         ✓          |           —           |
|                         | Where clauses                          |        ✓        |         ✓          |      ✓ (bounds)       |
|                         | GATs                                   |        ✓        |         ✓          |           —           |
| **Effects** [NEW]       | const                                  |        ✓        |         ✓          |           ✓           |
|                         | async                                  |        ✓        |         ✓          |           ✓           |
|                         | unsafe                                 |        ✓        |         ✓          |           ✓           |
|                         | io                                     |        ✓        |         ✓          |           ✓           |
|                         | alloc                                  |        ✓        |         ✓          |           ✓           |
|                         | panic                                  |        ✓        |         ✓          |           ✓           |
|                         | custom effects                         |        ✓        |         ✓          |           ✓           |
| **Contracts** [NEW]     | Preconditions                          |        ✓        |         ✓          |           ✓           |
|                         | Postconditions                         |        ✓        |         ✓          |           ✓           |
|                         | Invariants                             |        ✓        |         ✓          |           ✓           |
| **Control Flow**        | if/else, loop, while, for              |        ✓        |         —          |           —           |
|                         | match (exhaustive)                     |        ✓        |         ✓          |           ✓           |
|                         | ? operator                             |        ✓        |         ✓          | ✓ (error propagation) |
|                         | return, break, continue                |        ✓        |         —          |           —           |
|                         | async/await                            |        ✓        |         ✓          |           ✓           |
| **Modules**             | mod, use, pub                          |        ✓        |         ✓          |    ✓ (visibility)     |
|                         | Crate-level visibility                 |        ✓        |         ✓          |           ✓           |
| **Swarm** [NEW]         | Semantic regions                       |        ✓        |         ✓          | ✓ (write exclusivity) |
|                         | Semantic leases                        |        ✓        |         ✓          | ✓ (concurrent safety) |
|                         | Consensus points                       |        ✓        |         ✓          | ✓ (atomic interfaces) |
|                         | Agent roles                            |        ✓        |         ✓          | ✓ (capability bound)  |
|                         | Swarm messages (typed bus)             |        ✓        |         ✓          |     ✓ (isolation)     |
| **Syntax** [NEW]        | Zero-ambiguity LL(1) grammar           |        ✓        |         ✓          |           —           |
|                         | Canonical form enforcement             |        ✓        |         ✓          |           —           |
|                         | Streaming partial parse                |        ✓        |         ✓          |           —           |
| **Performance** [NEW]   | Dialect-as-semantics MLIR IR           |        ✓        |         ✓          |           —           |
|                         | Language-level SIMD (Simd[T, N])       |        ✓        |         ✓          |           —           |
|                         | MLIR-native autotuning (@pa)           |        ✓        |         ✓          |           —           |
|                         | Automatic device placement (@pt(auto)) |        ✓        |         ✓          |           —           |
|                         | Compile-time metaprogramming (@pp)     |        ✓        |         ✓          |           —           |
|                         | Multi-target compilation               |        ✓        |         ✓          |           —           |
|                         | Performance annotations                |        ✓        |         ✓          |           —           |
| **Safety-Free** [NEW]   | Lifetime elision (no `'a` syntax)      |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                         | Borrow elision (no `&mut` syntax)      |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                         | Unsafe elision (no `unsafe` keyword)   |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                         | Auto-derived safety traits             |        ✓        |         ✓          |    ✓ (SKB-handled)    |
|                         | Unified dispatch (no `dyn`/`impl`)     |        ✓        |         ✓          |           —           |
| **ACI** [NEW]           | Dynamic warnings (ML-learned)          |        ✓        |         ✓          |   ✓ (probabilistic)   |
|                         | Intelligent debugging                  |        ✓        |         ✓          |    ✓ (root-cause)     |
|                         | Performance advisor                    |        ✓        |         ✓          |           —           |
|                         | Swarm coordination intelligence        |        ✓        |         ✓          | ✓ (conflict predict)  |
|                         | Codebase learning model                |        ✓        |         ✓          |           —           |
| **Cost Model** [NEW]    | Per-target cost oracle                 |        ✓        |         ✓          |           —           |
|                         | Multi-target cost comparison           |        ✓        |         ✓          |           —           |
|                         | Cost-constrained synthesis             |        ✓        |         ✓          |           —           |
| **Grammar** [NEW]       | Domain-specific syntax extensions      |        ✓        |         ✓          |           —           |
|                         | Frequency-driven abbreviation promote  |        ✓        |         ✓          |           —           |
|                         | Extension discovery/registry           |        ✓        |         ✓          |           —           |
| **Synthesis** [NEW]     | Formal specification syntax            |        ✓        |         ✓          |     ✓ (contracts)     |
|                         | Spec-to-code synthesis oracle          |        ✓        |         ✓          |           ✓           |
|                         | Pipeline composition from specs        |        ✓        |         ✓          |           ✓           |
| **Memory** [NEW]        | Ephemeral/session/project/global tiers |        ✓        |         ✓          |           —           |
|                         | Pattern recall and learning            |        ✓        |         ✓          |           —           |
|                         | Memory-driven ACI improvement          |        ✓        |         ✓          |           —           |
| **Self-Healing** [NEW]  | Auto-repair on compile error           |        ✓        |         ✓          |           —           |
|                         | Confidence-ranked fix candidates       |        ✓        |         ✓          |           —           |
| **Hot-Reload** [NEW]    | Function-level live patching           |        ✓        |         ✓          |     ✓ (ABI check)     |
|                         | Rollback with retention window         |        ✓        |         ✓          |           —           |
| **FFI** [NEW]           | Auto-binding from foreign headers      |        ✓        |         ✓          |    ✓ (layout safe)    |
|                         | Zero-copy cross-language calls         |        ✓        |         ✓          |           —           |
| **Security** [NEW]      | Capability-based sandboxing            |        ✓        |         ✓          |    ✓ (cap. check)     |
|                         | Deterministic execution in sandbox     |        ✓        |         ✓          |           —           |
| **Orchestration** [NEW] | Map-reduce / pipeline / saga patterns  |        ✓        |         ✓          |  ✓ (pattern verify)   |
|                         | Compile-time pattern verification      |        ✓        |         ✓          |           ✓           |
| **SKB** [NEW]           | Safety Knowledge Base                  |        ✓        |         ✓          |  ✓ (queryable rules)  |
|                         | Opt-in compile-time checks             |        ✓        |         ✓          |   ✓ (configurable)    |
| **Token** [NEW]         | Compressed keywords (`+f`, `m`, `S`)   |        ✓        |         ✓          |           —           |
|                         | Attribute abbreviations (`@d`, `@r`)   |        ✓        |         ✓          |           —           |
|                         | Type abbreviations (`?T`, `R[T,E]`)    |        ✓        |         ✓          |           —           |
|                         | Standard abbreviation registry         |        ✓        |         ✓          |           —           |
|                         | Compact ↔ expanded conversion          |        ✓        |         ✓          |           —           |
|                         | Token budget reporting                 |        ✓        |         ✓          |           —           |

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
| P24     | MLIR Lowering [NEW]        | Optimized MIR    | MLIR (MechGen Dialect)  |           —            | `mlir_of(func)`            |
| P25     | MLIR→LLVM Lowering [NEW]   | MLIR MechGen       | LLVM IR / Target code |           —            | `target_code_of(func)`     |
| P26     | SKB Validation [NEW]       | Source + SKB     | Rule violations       |   Opt-in enforcement   | `skb_check(func)`          |
| P30     | Autotuning [NEW]           | MLIR MechGen       | N lowering variants   |           —            | `autotune_of(func)`        |
| P31     | Device Placement [NEW]     | MLIR MechGen       | Target assignment     |           —            | `placement_of(func)`       |
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
| P44     | Hot-Patch [NEW]            | New func source  | Patched binary        |     ABI stability      | `hotpatch(func, source)`   |
| P45     | FFI Binding Gen [NEW]      | Foreign headers  | Safe MechGen bindings   |     Layout safety      | `ffi_bindings(header)`     |
| P46     | Sandbox Exec [NEW]         | Code + Policy    | Sandboxed result      |    Capability check    | `sandbox_exec(code, pol)`  |

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

## 13. Deep Dive: Formal Type Inference Algorithm

Section 5.6 declares that lifetimes, borrow modes, ownership transfer, dispatch strategy, and allocation strategy are **all inferred by the compiler** — eliminating them from the syntax entirely. This section specifies *how* that inference works: the algorithms, judgment rules, and complexity guarantees that replace Rust's explicit annotations with compiler-driven decisions.

### 13.1 Inference Architecture

The type inference engine is a **five-phase pipeline** that runs after MIR construction and before MLIR lowering. Each phase feeds its results into the next:

```
Phase 1          Phase 2          Phase 3          Phase 4          Phase 5
Ownership    →   Borrow Mode  →  Lifetime     →   Dispatch     →  Allocation
Inference        Inference        Inference        Strategy         Strategy
                                                   Selection        Selection
     │                │                │                │                │
     ▼                ▼                ▼                ▼                ▼
 OwnershipMap     BorrowMap       LifetimeMap     DispatchMap     AllocMap
 (var → own)      (ref → mode)   (ref → region)  (call → vtbl?)  (val → loc)
```

All five phases share a **constraint generation + constraint solving** architecture:

1. **Walk the MIR** to generate constraints.
2. **Solve the constraint system** using a fixed-point algorithm.
3. **Emit the solution** as an annotation map consumed by the next phase.

### 13.2 Phase 1: Ownership Inference

Determines whether each value is **moved**, **copied**, or **borrowed** at each use site.

#### 13.2.1 Constraint Language

```
OwnershipConstraint ::=
    | Own(var)              -- var owns its value (will be dropped at scope exit)
    | Moved(var, site)      -- var's value is moved at use site
    | Copied(var, site)     -- var's value is copied at use site
    | Borrowed(var, site)   -- var's value is borrowed at use site
    | Conflict(site, site)  -- two use sites have incompatible ownership demands
```

#### 13.2.2 Inference Rules (Judgment Form)

```
                   var declared, not used after site s
───────────────────────────────────────────────────────── [MOVE]
            Γ ⊢ use(var, s) : Moved(var, s)


            var used after site s   ∧   T: Copy
───────────────────────────────────────────────────────── [COPY]
            Γ ⊢ use(var, s) : Copied(var, s)


            var used after site s   ∧   ¬(T: Copy)
───────────────────────────────────────────────────────── [BORROW-NEEDED]
            Γ ⊢ use(var, s) : Borrowed(var, s)


            Moved(var, s1) ∧ use(var, s2) ∧ s2 dominates s1
───────────────────────────────────────────────────────── [USE-AFTER-MOVE]
            Γ ⊢ Conflict(s1, s2)
```

#### 13.2.3 Algorithm

```
fn infer_ownership(mir: &MIR) -> OwnershipMap {
    // 1. Build def-use chains for every local variable
    v def_use = mir.build_def_use_chains();

    // 2. For each variable, compute last-use site via post-dominator tree
    v last_use = mir.post_dominator_last_use(&def_use);

    // 3. Generate constraints
    v constraints = Vec.new;
    @ (var, uses) : &def_use {
        @ (site, kind) : uses {
            ? {
                site == last_use[var] && kind.is_value =>
                    constraints.push(Moved(var, site)),
                var.ty.is_copy =>
                    constraints.push(Copied(var, site)),
                _ =>
                    constraints.push(Borrowed(var, site)),
            }
        }
    }

    // 4. Detect conflicts (use after move)
    @ (var, uses) : &def_use {
        v move_sites = constraints.moves_of(var);
        @ site : move_sites {
            @ later_use : uses.after(site) {
                constraints.push(Conflict(site, later_use));
            }
        }
    }

    // 5. Resolve: conflicts trigger auto-clone insertion or borrow promotion
    v solution = ConstraintSolver.solve(constraints, Strategy.PreferBorrow);
    solution.to_ownership_map()
}
```

**Complexity**: $O(V \cdot U)$ where $V$ = number of local variables, $U$ = average uses per variable. In practice $O(n)$ in function size since use counts are bounded by function length.

### 13.3 Phase 2: Borrow Mode Inference

Determines whether each reference is **shared (`&T`)** or **exclusive (`&mut T`)** based on how the referenced value is used through the reference.

#### 13.3.1 Inference Rules

```
            ref r used only in read contexts (field access, method &self, comparison)
───────────────────────────────────────────────────────── [SHARED]
            Γ ⊢ r : Shared


            ref r used in at least one write context (assignment, method &mut self)
───────────────────────────────────────────────────────── [EXCLUSIVE]
            Γ ⊢ r : Exclusive


            Shared(r1) ∧ Exclusive(r2) ∧ alias(r1, r2)
───────────────────────────────────────────────────────── [ALIAS-CONFLICT]
            Γ ⊢ Error: overlapping shared and exclusive borrows


            Exclusive(r1) ∧ Exclusive(r2) ∧ alias(r1, r2)
───────────────────────────────────────────────────────── [DOUBLE-MUT]
            Γ ⊢ Error: two exclusive borrows of same value
```

#### 13.3.2 Algorithm

```
fn infer_borrow_modes(mir: &MIR, ownership: &OwnershipMap) -> BorrowMap {
    // 1. For each borrow site, collect all transitive uses of the reference
    v borrow_uses = mir.transitive_ref_uses();

    // 2. Classify: if ANY use writes, the borrow is exclusive
    v modes = HashMap.new;
    @ (ref_id, uses) : &borrow_uses {
        v has_write = uses.any(|u| u.is_write || u.calls_mut_method);
        modes.insert(ref_id, ? { has_write => Exclusive, _ => Shared });
    }

    // 3. Check alias pairs — O(R²) but R is bounded per function
    v aliases = mir.compute_alias_sets();
    @ (r1, r2) : aliases.pairs() {
        ? {
            modes[r1] == Exclusive && modes[r2] == Exclusive =>
                emit_error("double exclusive borrow", r1, r2),
            modes[r1] != modes[r2] && aliases.overlap(r1, r2) =>
                emit_error("shared/exclusive overlap", r1, r2),
            _ => {},
        }
    }

    modes
}
```

**Key insight**: In Rust, the programmer writes `&` or `&mut` and the borrow checker validates. In MechGen, the programmer writes nothing — the compiler observes how the reference is used and *derives* the mode. The SKB provides the aliasing rules; the compiler applies them automatically.

### 13.4 Phase 3: Lifetime Inference

Determines the **live range** of every reference and ensures no reference outlives its referent.

#### 13.4.1 Region Variables and Constraints

```
LifetimeConstraint ::=
    | Live(ref, region)           -- ref is live in region
    | Outlives(region, region)    -- region₁ must outlive region₂
    | Subset(region, region)      -- region₁ ⊆ region₂ (NLL-style)
    | FnBound(fn, param_region, return_region)  -- return lifetime ≤ param lifetime
```

#### 13.4.2 Algorithm (NLL-Based)

```
fn infer_lifetimes(mir: &MIR, borrows: &BorrowMap) -> LifetimeMap {
    // 1. Assign a fresh region variable to every reference
    v regions = mir.refs().map(|r| (r, RegionVar.fresh()));

    // 2. Generate subset constraints from control flow
    //    (identical to Rust's NLL, but without user annotations)
    v constraints = Vec.new;
    @ bb : mir.basic_blocks() {
        @ stmt : bb.stmts() {
            ? stmt {
                Borrow(ref_id, place) => {
                    // ref must not outlive place's scope
                    constraints.push(Subset(regions[ref_id], scope_of(place)));
                },
                Assign(dest, src) ? is_ref(src) => {
                    // dest's region must be subset of src's region
                    constraints.push(Subset(regions[dest], regions[src]));
                },
                Return(val) ? is_ref(val) => {
                    // return ref's region must be subset of some param region
                    v param_regions = mir.param_refs().map(|p| regions[p]);
                    constraints.push(FnBound(mir.fn_id, regions[val], param_regions));
                },
                _ => {},
            }
        }
    }

    // 3. Solve via fixed-point iteration
    //    Uses Polonius-style subset propagation
    v solution = RegionSolver.solve(constraints);

    // 4. Verify: no dangling references
    @ (ref_id, region) : &solution {
        v referent_scope = scope_of(mir.borrow_place(ref_id));
        ? !region.contained_in(referent_scope) {
            emit_error("reference outlives referent", ref_id);
        }
    }

    solution
}
```

**Complexity**: $O(R^2 \cdot B)$ worst case where $R$ = region variables, $B$ = basic blocks. In practice, region constraint graphs are sparse and convergence is fast (2–4 iterations).

#### 13.4.3 Cross-Function Lifetime Inference

For function signatures, MechGen applies **lifetime elision on steroids** — not just the three Rust elision rules, but full inter-procedural inference:

```
// Rule 1 (Rust-compatible): Single input ref → output gets same lifetime
f first(items: &[T]) -> &T          // inferred: output ⊆ items

// Rule 2 (Rust-compatible): &self → output gets self's lifetime
m get(&self) -> &T                  // inferred: output ⊆ self

// Rule 3 (NEW): Multiple input refs → output gets intersection
f merge(a: &[T], b: &[T]) -> &[T]  // inferred: output ⊆ min(a, b)

// Rule 4 (NEW): No input refs → output must be 'static or owned
f create() -> &T                    // ERROR: no referent to borrow from

// Rule 5 (NEW): Struct fields → lifetime flows from constructor args
S View { data: &[u8] }             // inferred: View's lifetime ⊆ data's referent

// Rule 6 (NEW): Closures → capture lifetimes inferred from body
v f = |x| data[x];                  // inferred: f borrows data, lives within data's scope
```

### 13.5 Phase 4: Dispatch Strategy Selection

Determines whether each trait-object call uses **static dispatch** (monomorphization) or **dynamic dispatch** (vtable), and whether to use **devirtualization** for hot paths.

#### 13.5.1 Decision Algorithm

```
fn select_dispatch(mir: &MIR, call_site: CallSite) -> DispatchStrategy {
    v concrete_types = mir.possible_concrete_types(call_site.receiver);

    ? {
        // Case 1: Only one concrete type → always static
        concrete_types.len() == 1 =>
            DispatchStrategy.Static(concrete_types[0]),

        // Case 2: Few types + hot loop → static with enum dispatch
        concrete_types.len() <= 4 && call_site.in_hot_loop() =>
            DispatchStrategy.EnumDispatch(concrete_types),

        // Case 3: Many types or cold path → dynamic (vtable)
        concrete_types.len() > 4 =>
            DispatchStrategy.Dynamic,

        // Case 4: Unknown types (e.g., plugin interface) → dynamic
        concrete_types.is_open_set() =>
            DispatchStrategy.Dynamic,

        // Case 5: Few types, not hot → static with PGO hint
        _ =>
            DispatchStrategy.StaticWithFallback(concrete_types),
    }
}
```

#### 13.5.2 Agent Override

Agents can override any inference decision via annotations when they have domain knowledge:

```
@dispatch(static)    // force monomorphization
@dispatch(dynamic)   // force vtable
@dispatch(enum, 4)   // force enum dispatch with max 4 variants
```

### 13.6 Phase 5: Allocation Strategy Selection

Determines whether each value is allocated on the **stack**, **heap**, or in an **arena** — replacing the Rust programmer's choice between `T`, `Box<T>`, `Rc<T>`, `Arc<T>`.

#### 13.6.1 Escape Analysis

```
AllocConstraint ::=
    | Escapes(val, scope)       -- val escapes scope (returned, stored in longer-lived struct)
    | NoEscape(val, scope)      -- val does not escape scope
    | SharedOwner(val, N)       -- val has N owners (reference count candidate)
    | CrossThread(val)          -- val accessed from multiple threads
    | ArenaCandidate(val, grp)  -- val belongs to group grp (same lifetime, same dealloc point)
```

#### 13.6.2 Decision Algorithm

```
fn select_allocation(mir: &MIR, val: Value,
                     ownership: &OwnershipMap,
                     lifetimes: &LifetimeMap) -> AllocStrategy {
    v escape = analyze_escape(mir, val);
    v owners = count_owners(mir, val, ownership);
    v cross_thread = is_cross_thread(mir, val);
    v arena_group = find_arena_group(mir, val, lifetimes);

    ? {
        // Case 1: No escape, single owner → stack
        !escape && owners == 1 =>
            AllocStrategy.Stack,

        // Case 2: Escapes, single owner → heap (Box equivalent)
        escape && owners == 1 && !cross_thread =>
            AllocStrategy.Heap,

        // Case 3: Multiple owners, single thread → Rc equivalent
        owners > 1 && !cross_thread =>
            AllocStrategy.RefCounted,

        // Case 4: Multiple owners, cross-thread → Arc equivalent
        owners > 1 && cross_thread =>
            AllocStrategy.AtomicRefCounted,

        // Case 5: Belongs to arena group → arena allocation
        arena_group.is_some =>
            AllocStrategy.Arena(arena_group.unwrap),

        // Case 6: Large value, no escape → stack with spill hint
        !escape && val.size > STACK_THRESHOLD =>
            AllocStrategy.StackWithSpillHint,

        _ => AllocStrategy.Heap,  // conservative default
    }
}
```

#### 13.6.3 Agent Override

```
v x = ^MyStruct { ... };    // force heap (Box equivalent)
v y = @MyStruct { ... };    // force Rc
v z = $MyStruct { ... };    // force Arc
v w = MyStruct { ... };     // compiler decides (default)
```

### 13.7 Inference Guarantees

| Property                  | Guarantee                                                                 |
| ------------------------- | ------------------------------------------------------------------------- |
| **Determinism**           | Same source always produces same inference result                         |
| **Soundness**             | Inferred modes are at least as restrictive as manual annotations would be |
| **Completeness**          | Every well-typed Rust program has a valid MechGen inference                 |
| **Monotonicity**          | Adding code never invalidates previously inferred ownership               |
| **Performance**           | Inference is $O(n \log n)$ in function size for 95% of functions          |
| **Worst-case complexity** | $O(n^2 \cdot R)$ for pathological alias sets ($R$ = region variables)     |
| **SKB compatibility**     | All inferred decisions are verifiable against the SKB                     |
| **Override transparency** | Agent annotations are always respected; conflicts emit structured errors  |

---

## 14. Deep Dive: MLIR Dialect Operation Definitions

Section 5.4 describes the MechGen MLIR dialect at a high level. This section provides **formal operation definitions** for every operation in the dialect, following MLIR's ODS (Operation Definition Specification) conventions.

### 14.1 Dialect Registration

```tablegen
def MechGen_Dialect : Dialect {
  let name = "mechgen";
  let summary = "MechGen agentic language dialect for MLIR";
  let description = [{
    The MechGen dialect encodes the full semantics of the MechGen language —
    ownership, effects, contracts, performance annotations, agent capabilities,
    and safety knowledge base queries — as first-class MLIR operations and
    attributes. This enables the compiler's semantic understanding to be
    preserved through the entire optimization pipeline, unlike traditional
    approaches where MIR→LLVM IR lowering discards high-level intent.
  }];
  let cppNamespace = "::mechgen";
  let useDefaultTypePrinterParser = 1;
  let useDefaultAttributePrinterParser = 1;
}
```

### 14.2 Type Definitions

```tablegen
// Owned value type — compiler manages ownership transfer
def MechGen_OwnedType : MechGen_Type<"Owned", "owned"> {
  let summary = "An owned value with compiler-managed ownership semantics";
  let parameters = (ins "Type":$elementType);
  let assemblyFormat = "`<` $elementType `>`";
}

// Reference type — borrow mode (shared/exclusive) inferred
def MechGen_RefType : MechGen_Type<"Ref", "ref"> {
  let summary = "A reference with inferred borrow mode";
  let parameters = (ins
    "Type":$elementType,
    "BorrowModeAttr":$mode  // Shared | Exclusive | Inferred
  );
  let assemblyFormat = "`<` $elementType `,` $mode `>`";
}

// Region type — lifetime region variable
def MechGen_RegionType : MechGen_Type<"Region", "region"> {
  let summary = "A lifetime region variable";
  let parameters = (ins "StringAttr":$name);
}

// Effect type — algebraic effect annotation
def MechGen_EffectType : MechGen_Type<"Effect", "effect"> {
  let summary = "An algebraic effect (IO, Async, Alloc, etc.)";
  let parameters = (ins "StringAttr":$effectName);
}

// Capability type — agent capability token
def MechGen_CapabilityType : MechGen_Type<"Capability", "cap"> {
  let summary = "An agent capability token for discovery";
  let parameters = (ins "ArrayAttr":$capabilities);
}
```

### 14.3 Ownership Operations

```tablegen
// Move a value — transfers ownership from source to destination
def MechGen_MoveOp : MechGen_Op<"move", [Pure]> {
  let summary = "Transfer ownership of a value";
  let description = [{
    Transfers ownership from the source SSA value to the result. After this
    operation, the source value is consumed and cannot be used. The compiler
    inserts this automatically based on Phase 1 inference.
  }];
  let arguments = (ins AnyType:$source);
  let results = (outs AnyType:$result);
  let assemblyFormat = "$source attr-dict `:` type($source) `->` type($result)";

  // Verification: source must not be used after this op
  let verifier = [{ return verifyMoveOp(*this); }];
}

// --- Example MLIR ---
// %1 = mechgen.move %0 : !mechgen.owned<tensor<4xf32>> -> !mechgen.owned<tensor<4xf32>>

// Copy a value — duplicates for Copy types
def MechGen_CopyOp : MechGen_Op<"copy", [Pure]> {
  let summary = "Copy a value (only valid for Copy types)";
  let arguments = (ins AnyType:$source);
  let results = (outs AnyType:$result);
  let assemblyFormat = "$source attr-dict `:` type($source)";

  let verifier = [{ return verifyCopyType(getSource().getType()); }];
}

// Borrow a value — creates a reference
def MechGen_BorrowOp : MechGen_Op<"borrow", []> {
  let summary = "Create a reference to a value";
  let arguments = (ins
    AnyType:$source,
    BorrowModeAttr:$mode,     // shared | exclusive | inferred
    MechGen_RegionType:$region  // lifetime region
  );
  let results = (outs MechGen_RefType:$ref);
  let assemblyFormat = "$mode $source `in` $region attr-dict `:` type($source)";
}

// --- Example MLIR ---
// %ref = mechgen.borrow shared %val in %rgn : !mechgen.owned<i64>
//   → !mechgen.ref<i64, shared>

// Drop a value — runs destructor and releases resources
def MechGen_DropOp : MechGen_Op<"drop", []> {
  let summary = "Drop a value, releasing owned resources";
  let arguments = (ins AnyType:$value);
  let assemblyFormat = "$value attr-dict `:` type($value)";
}

// --- Example MLIR ---
// mechgen.drop %vec : !mechgen.owned<!mechgen.vec<f32>>
```

### 14.4 Effect Operations

```tablegen
// Declare effects on a function
def MechGen_EffectDeclOp : MechGen_Op<"effect.decl", [IsolatedFromAbove]> {
  let summary = "Declare algebraic effects for a function region";
  let arguments = (ins
    ArrayAttr:$effects,       // ["IO", "Async", "Alloc"]
    OptionalAttr<ArrayAttr>:$handlers  // effect handlers if present
  );
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$effects (`handled_by` $handlers^)? $body attr-dict";
}

// --- Example MLIR ---
// mechgen.effect.decl ["IO", "Async"] {
//   // function body with IO and Async effects
// }

// Perform an effect — runtime effect invocation
def MechGen_EffectPerformOp : MechGen_Op<"effect.perform", []> {
  let summary = "Perform an algebraic effect";
  let arguments = (ins
    MechGen_EffectType:$effect,
    Variadic<AnyType>:$args
  );
  let results = (outs Optional<AnyType>:$result);
  let assemblyFormat = "$effect `(` $args `)` attr-dict `:` functional-type($args, $result)";
}

// Effect handler — catches and handles effects from a child region
def MechGen_EffectHandleOp : MechGen_Op<"effect.handle", []> {
  let summary = "Install an effect handler for a region";
  let arguments = (ins MechGen_EffectType:$effect);
  let regions = (region SizedRegion<1>:$body, SizedRegion<1>:$handler);
  let assemblyFormat = "$effect $body `with` $handler attr-dict";
}
```

### 14.5 Contract Operations

```tablegen
// Precondition — must hold before function execution
def MechGen_RequireOp : MechGen_Op<"contract.require", [Pure]> {
  let summary = "Assert a precondition (contract)";
  let arguments = (ins
    I1:$condition,
    StrAttr:$message
  );
  let assemblyFormat = "$condition `,` $message attr-dict";
}

// Postcondition — must hold after function execution
def MechGen_EnsureOp : MechGen_Op<"contract.ensure", [Pure]> {
  let summary = "Assert a postcondition (contract)";
  let arguments = (ins
    I1:$condition,
    StrAttr:$message,
    Optional<AnyType>:$returnValue
  );
  let assemblyFormat = "$condition `,` $message (`,` $returnValue^)? attr-dict";
}

// Invariant — must hold at specific program points
def MechGen_InvariantOp : MechGen_Op<"contract.invariant", []> {
  let summary = "Assert a loop or type invariant";
  let arguments = (ins
    I1:$condition,
    StrAttr:$message,
    DefaultValuedAttr<StrAttr, "\"loop\"">:$kind  // "loop" | "type" | "module"
  );
}

// --- Example MLIR ---
// mechgen.contract.require %cond, "index must be in bounds"
// ... function body ...
// mechgen.contract.ensure %post, "result is sorted", %ret_val
```

### 14.6 Performance Annotation Operations

```tablegen
// Target placement hint — compiled by MLIR cost model
def MechGen_PlaceOp : MechGen_Op<"perf.place", []> {
  let summary = "Hint target device for a computation region";
  let arguments = (ins
    StrAttr:$target,          // "cpu" | "gpu" | "npu" | "auto"
    OptionalAttr<I64Attr>:$priority
  );
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$target (`priority` $priority^)? $body attr-dict";
}

// --- Example MLIR ---
// mechgen.perf.place "auto" {
//   // compiler evaluates cost model for each available target
//   // and selects optimal dispatch
// }

// Vectorization hint
def MechGen_VectorizeOp : MechGen_Op<"perf.vectorize", []> {
  let summary = "Hint vectorization width for a loop region";
  let arguments = (ins I64Attr:$width);  // SIMD width: 4, 8, 16, etc.
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$width $body attr-dict";
}

// No-bounds-check annotation
def MechGen_NoBoundsCheckOp : MechGen_Op<"perf.no_bounds_check", []> {
  let summary = "Disable bounds checking in a region (agent-trusted)";
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$body attr-dict";
}

// Autotune — generate N variants and benchmark
def MechGen_AutotuneOp : MechGen_Op<"perf.autotune", []> {
  let summary = "Generate N optimization variants for autotuning";
  let arguments = (ins
    I64Attr:$variants,                     // number of variants to generate
    OptionalAttr<StrAttr>:$metric          // "latency" | "throughput" | "energy"
  );
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$variants (`metric` $metric^)? $body attr-dict";
}

// Cost query — compile-time cost model evaluation
def MechGen_CostQueryOp : MechGen_Op<"perf.cost_query", [Pure]> {
  let summary = "Query the cost model for an expression";
  let arguments = (ins
    StrAttr:$target_hw,       // "x86_64" | "aarch64" | "nvptx" | ...
    StrAttr:$metric           // "latency_ns" | "memory_bytes" | "allocs" | "energy_pj"
  );
  let regions = (region SizedRegion<1>:$body);
  let results = (outs F64:$cost);
  let assemblyFormat = "$target_hw $metric $body attr-dict";
}
```

### 14.7 Capability Operations

```tablegen
// Declare agent capabilities for a module
def MechGen_CapabilityDeclOp : MechGen_Op<"capability.decl", [IsolatedFromAbove]> {
  let summary = "Declare capabilities provided by a module";
  let arguments = (ins
    StrAttr:$name,
    ArrayAttr:$provides,     // ["http_client", "json_parse", "file_read"]
    ArrayAttr:$requires,     // capabilities required from dependencies
    OptionalAttr<StrAttr>:$version
  );
}

// Capability check — verify agent has required capability at compile time
def MechGen_CapabilityCheckOp : MechGen_Op<"capability.check", [Pure]> {
  let summary = "Verify a capability is available";
  let arguments = (ins StrAttr:$capability);
  let results = (outs I1:$available);
}

// Capability-gated region — code only executes if capability is held
def MechGen_CapabilityGateOp : MechGen_Op<"capability.gate", []> {
  let summary = "Gate a region on a capability token";
  let arguments = (ins MechGen_CapabilityType:$token);
  let regions = (region SizedRegion<1>:$body);
  let assemblyFormat = "$token $body attr-dict";
}
```

### 14.8 SKB Query Operations (Compile-Time)

```tablegen
// Query the Safety Knowledge Base during compilation
def MechGen_SKBQueryOp : MechGen_Op<"skb.query", [Pure]> {
  let summary = "Query the Safety Knowledge Base for applicable rules";
  let arguments = (ins
    StrAttr:$pattern,         // e.g., "MutableBorrow", "TypeConversion"
    DictionaryAttr:$context   // key-value context for the query
  );
  let results = (outs MechGen_RuleSetType:$rules);
  let assemblyFormat = "$pattern $context attr-dict";
}

// SKB validation — verify code against SKB rules
def MechGen_SKBValidateOp : MechGen_Op<"skb.validate", []> {
  let summary = "Validate a region against SKB rules";
  let arguments = (ins
    StrAttr:$rule_set,        // "ownership" | "borrow" | "lifetime" | "concurrency"
    DefaultValuedAttr<StrAttr, "\"warn\"">:$on_violation  // "error" | "warn" | "ignore"
  );
  let regions = (region SizedRegion<1>:$body);
}
```

### 14.9 Lowering Rules

The MechGen dialect lowers progressively through MLIR's dialect hierarchy:

| MechGen Operation          | Lowers To                            | Phase          |
| ------------------------ | ------------------------------------ | -------------- |
| `mechgen.move`             | SSA value copy + source invalidation | MechGen → Std    |
| `mechgen.copy`             | `memref.copy` or SSA value copy      | MechGen → MemRef |
| `mechgen.borrow`           | `memref.view` or SSA alias           | MechGen → MemRef |
| `mechgen.drop`             | Destructor call sequence             | MechGen → Func   |
| `mechgen.effect.decl`      | No-op (metadata preserved)           | MechGen → MechGen  |
| `mechgen.effect.perform`   | `func.call` to effect handler        | MechGen → Func   |
| `mechgen.contract.require` | `cf.assert` (debug) or removed (opt) | MechGen → CF     |
| `mechgen.contract.ensure`  | `cf.assert` (debug) or removed (opt) | MechGen → CF     |
| `mechgen.perf.place "gpu"` | `gpu.launch_func`                    | MechGen → GPU    |
| `mechgen.perf.vectorize`   | `vector.transfer_read/write` + ops   | MechGen → Vector |
| `mechgen.perf.autotune`    | N clones of body with different opts | MechGen → MechGen  |
| `mechgen.perf.cost_query`  | Compile-time eval → constant         | MechGen → Arith  |
| `mechgen.capability.gate`  | `scf.if` on runtime capability check | MechGen → SCF    |
| `mechgen.skb.query`        | Compile-time eval → diagnostics      | Erased         |
| `mechgen.skb.validate`     | Compile-time eval → diagnostics      | Erased         |

Full lowering sequence:

```
MechGen Dialect
  ↓  (ownership/borrow/drop → memory operations)
MemRef + Func + SCF
  ↓  (perf annotations → target-specific dialects)
Linalg + Vector + GPU + Affine
  ↓  (progressive lowering)
LLVM Dialect
  ↓  (translation)
LLVM IR → Machine Code
```

---

## 15. Deep Dive: SKB Schema and Query Language Specification

Section 9.2 presents the SKB architecture diagram. This section formalizes the **database schema**, **query language (SKB-QL)**, **indexing strategy**, **rule lifecycle**, and **versioning protocol**.

### 15.1 Rule Schema

Every rule in the SKB conforms to a common schema. Rules are stored as structured records, not free-form text:

```
Rule {
    // Identity
    id:          RuleId,           // e.g., "OWN-0042", "BR-1203"
    database:    Database,         // Ownership | Borrow | Lifetime | TypeSafety | Concurrency | FFI
    version:     SemanticVersion,  // e.g., 1.4.2

    // Pattern matching
    pattern:     Pattern,          // structural pattern to match against code
    context:     ContextSpec,      // additional context constraints
    scope:       Scope,            // Function | Module | Crate | Global

    // Semantics
    severity:    Severity,         // Error | Warning | Info | Hint
    category:    Category,         // e.g., "use-after-move", "double-borrow", "data-race"
    description: String,           // human/agent-readable explanation
    rationale:   String,           // WHY this rule exists

    // Resolution
    fix_template: Option<FixTemplate>,   // auto-fix template if applicable
    fix_confidence: f64,                  // 0.0–1.0 confidence in the auto-fix
    alternatives: Vec<Alternative>,       // alternative approaches

    // Metadata
    source:      RuleSource,       // BuiltIn | ProjectCustom | CommunityContributed
    frequency:   u64,              // how often this rule matches across the ecosystem
    false_positive_rate: f64,      // measured false positive rate
    tags:        Vec<String>,      // searchable tags
    created:     Timestamp,
    updated:     Timestamp,
    deprecated:  Option<Timestamp>,
}
```

### 15.2 Pattern Language

Patterns are structural templates that match against MechGen MIR nodes:

```
Pattern ::=
    // Ownership patterns
    | UseAfterMove { var: VarPattern, move_site: SitePattern, use_site: SitePattern }
    | DoubleMove { var: VarPattern, sites: (SitePattern, SitePattern) }
    | MoveInLoop { var: VarPattern, loop_kind: LoopKind }

    // Borrow patterns
    | MutableBorrow { target_type: TypePattern, context: ContextSpec }
    | AliasingBorrow { refs: Vec<RefPattern>, overlap: OverlapKind }
    | BorrowEscapes { ref: RefPattern, escapes_to: ScopePattern }

    // Lifetime patterns
    | DanglingRef { ref: RefPattern, referent_scope: ScopePattern }
    | LifetimeMismatch { expected: RegionPattern, actual: RegionPattern }
    | SelfReferential { struct_type: TypePattern }

    // Type safety patterns
    | TypeConversion { from: TypePattern, to: TypePattern, target_arch: ArchPattern }
    | NarrowingCast { from: TypePattern, to: TypePattern }
    | UnsoundTransmute { from: TypePattern, to: TypePattern }
    | UninitRead { var: VarPattern, site: SitePattern }

    // Concurrency patterns
    | DataRace { var: VarPattern, threads: Vec<ThreadPattern> }
    | DeadlockRisk { locks: Vec<LockPattern>, order: OrderPattern }
    | SendViolation { type: TypePattern, context: ContextSpec }

    // FFI patterns
    | NullPointerDeref { source: FFISource, site: SitePattern }
    | LayoutMismatch { mechgen_type: TypePattern, foreign_type: TypePattern }
    | MissingFree { alloc_site: SitePattern, foreign_allocator: String }

// VarPattern and TypePattern support wildcards:
VarPattern  ::= Exact(Name) | AnyVar | Typed(TypePattern)
TypePattern ::= Exact(Type) | AnyType | Generic(Name, Vec<TypePattern>)
                | Wildcard   | OneOf(Vec<TypePattern>)
SitePattern ::= Line(u32)   | AnySite | InScope(ScopePattern)
```

### 15.3 SKB Query Language (SKB-QL)

SKB-QL is a domain-specific query language optimized for agent consumption — designed for minimal tokens and maximum precision:

#### 15.3.1 Query Syntax

```
Query      ::= SELECT fields FROM database WHERE conditions
             | MATCH pattern IN scope
             | VALIDATE code_fragment AGAINST rule_set
             | COUNT pattern IN scope

fields     ::= '*' | field (',' field)*
field      ::= 'id' | 'severity' | 'fix_template' | 'description' | 'confidence'
database   ::= 'ownership' | 'borrow' | 'lifetime' | 'type_safety'
             | 'concurrency' | 'ffi' | 'custom' | 'all'
conditions ::= condition ('AND' condition)*
condition  ::= field_cond | pattern_cond | context_cond
field_cond ::= field op value
op         ::= '=' | '!=' | '<' | '>' | '<=' | '>=' | 'LIKE' | 'IN'
pattern_cond ::= 'PATTERN' '=' pattern_expr
context_cond ::= 'CONTEXT' '=' context_expr

pattern_expr ::= pattern_name '(' args ')'
context_expr ::= '{' key ':' value (',' key ':' value)* '}'
scope      ::= 'function' '(' name ')' | 'module' '(' path ')' | 'crate' | 'global'
```

#### 15.3.2 Query Examples

```sql
-- Find all rules about mutable borrows in loop contexts
SELECT id, severity, fix_template
FROM borrow
WHERE PATTERN = MutableBorrow(target_type: "Vec<*>")
  AND CONTEXT = { location: "inside_loop" }

-- Match ownership patterns for a specific function
MATCH UseAfterMove(var: AnyVar, move_site: AnySite, use_site: AnySite)
IN function("process_data")

-- Validate a code fragment against concurrency rules
VALIDATE {
    v shared = Arc.new(Mutex.new(data));
    v handle = spawn(|| shared.lock.process);
    shared.lock.process;  -- potential data race?
} AGAINST concurrency

-- Count how many lifetime rules apply to self-referential structs
COUNT SelfReferential(struct_type: AnyType)
IN crate

-- Find all rules with auto-fix confidence > 0.9
SELECT id, fix_template, fix_confidence
FROM all
WHERE fix_confidence > 0.9
  AND severity = 'Error'

-- Compact agent-optimized query (abbreviated form)
?borrow MutBorrow(Vec<*>) @loop  -- equivalent to first example above
```

#### 15.3.3 Query Response Format

```
QueryResult {
    query_id:    QueryId,
    timestamp:   Timestamp,
    rules:       Vec<MatchedRule>,
    total_count: u64,
    truncated:   bool,           // true if result set was capped
    eval_time:   Duration,       // query evaluation time
}

MatchedRule {
    rule:        Rule,           // full rule record
    match_info:  MatchInfo,      // where/how the pattern matched
    relevance:   f64,            // 0.0–1.0 relevance score
}

MatchInfo {
    matched_nodes: Vec<MIRNodeId>,  // which MIR nodes matched
    bindings:      HashMap<String, Value>,  // pattern variable bindings
    context:       ContextSnapshot, // captured context at match site
}
```

### 15.4 Indexing Strategy

The SKB uses a **dual-index architecture** for sub-millisecond queries:

```
┌─────────────────────────────────────────────────────────┐
│                    SKB Index Layer                        │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────────────┐  ┌──────────────────────────┐ │
│  │ Pattern Trie Index   │  │ Context Bloom Filter     │ │
│  │                      │  │                          │ │
│  │ Root                 │  │ Per-database bloom filter │ │
│  │ ├─ UseAfterMove      │  │ with context keys:       │ │
│  │ │  ├─ var:Vec        │  │                          │ │
│  │ │  ├─ var:String     │  │ inside_loop: bloom[0]    │ │
│  │ │  └─ var:*          │  │ cross_thread: bloom[1]   │ │
│  │ ├─ MutableBorrow     │  │ async_context: bloom[2]  │ │
│  │ │  ├─ Vec<T>         │  │ ffi_boundary: bloom[3]   │ │
│  │ │  ├─ HashMap<K,V>   │  │ unsafe_block: bloom[4]   │ │
│  │ │  └─ *              │  │ generic_fn: bloom[5]     │ │
│  │ ├─ DataRace          │  │ closure: bloom[6]        │ │
│  │ │  └─ ...            │  │ trait_impl: bloom[7]     │ │
│  │ └─ ...               │  │                          │ │
│  └──────────────────────┘  └──────────────────────────┘ │
│                                                          │
│  ┌──────────────────────┐  ┌──────────────────────────┐ │
│  │ Severity Index       │  │ Tag Inverted Index       │ │
│  │                      │  │                          │ │
│  │ Error:   [rule_ids]  │  │ "iterator":  [rule_ids]  │ │
│  │ Warning: [rule_ids]  │  │ "allocator": [rule_ids]  │ │
│  │ Info:    [rule_ids]  │  │ "closure":   [rule_ids]  │ │
│  │ Hint:    [rule_ids]  │  │ "async":     [rule_ids]  │ │
│  └──────────────────────┘  └──────────────────────────┘ │
│                                                          │
│  Query path: Pattern Trie → Bloom Filter → Full Scan    │
│  Average: O(log R) where R = rules in matched category  │
│  Worst case: O(R) with full scan fallback                │
└─────────────────────────────────────────────────────────┘
```

### 15.5 Rule Lifecycle

```
┌──────────┐     ┌──────────┐     ┌──────────┐     ┌──────────┐
│ Proposed │ ──→ │ Staged   │ ──→ │ Active   │ ──→ │Deprecated│
└──────────┘     └──────────┘     └──────────┘     └──────────┘
     │                │                │                │
     │ Validation:    │ Testing:       │ Monitoring:    │ Retirement:
     │ - Schema check │ - False pos    │ - Usage freq   │ - Grace period
     │ - No conflicts │   rate < 5%    │ - False pos    │ - Migration
     │ - Pattern OK   │ - Perf impact  │   tracking     │   to successor
     │                │   < 1ms/query  │ - Fix accuracy │ - Tombstone
     ▼                ▼                ▼                ▼
   Rejected        Reverted         Updated          Removed
   (with reason)   (with metrics)   (new version)    (after grace)
```

**Rule versioning** follows semantic versioning:
- **Patch** (1.0.x): Fix description, update false positive rate
- **Minor** (1.x.0): Add alternative fixes, expand pattern coverage
- **Major** (x.0.0): Change pattern matching behavior (breaking)

Major version bumps require all dependent rules to be re-validated.

### 15.6 Database Statistics and Performance Targets

| Database    | Rules     | Avg Query Time | P99 Query Time | Index Size  | Update Freq |
| ----------- | --------- | -------------- | -------------- | ----------- | ----------- |
| Ownership   | 2,847     | 0.02 ms        | 0.15 ms        | 4.2 MB      | Monthly     |
| Borrow      | 1,203     | 0.01 ms        | 0.08 ms        | 1.8 MB      | Monthly     |
| Lifetime    | 894       | 0.01 ms        | 0.06 ms        | 1.3 MB      | Quarterly   |
| Type Safety | 3,412     | 0.03 ms        | 0.20 ms        | 5.1 MB      | Monthly     |
| Concurrency | 567       | 0.008 ms       | 0.05 ms        | 0.9 MB      | Quarterly   |
| FFI         | 234       | 0.005 ms       | 0.03 ms        | 0.4 MB      | As needed   |
| **Total**   | **9,157** | **0.02 ms**    | **0.20 ms**    | **13.7 MB** |             |

### 15.7 SKB Distribution Protocol

The SKB is distributed to agents as a **versioned, content-addressed artifact**:

```
skb-v1.4.2.mgdb          -- binary database file
skb-v1.4.2.mgdb.sig      -- Ed25519 signature
skb-v1.4.2.mgdb.manifest -- rule inventory (JSON)

Manifest:
{
  "version": "1.4.2",
  "databases": {
    "ownership": { "rules": 2847, "hash": "sha256:abc123..." },
    "borrow":    { "rules": 1203, "hash": "sha256:def456..." },
    ...
  },
  "total_rules": 9157,
  "min_compiler_version": "0.3.0",
  "signature_key": "ed25519:pub_key_here"
}

// Delta updates — only download changed rules
skb-delta-v1.4.1-to-v1.4.2.mgpatch
```

---

## 16. Deep Dive: Swarm Bus Wire Protocol Specification

Section 7.6 defines the `SwarmMessage` enum at the semantic level. This section specifies the **binary wire format**, **frame layout**, **serialization protocol**, **flow control**, and **connection lifecycle** for the swarm communication bus.

### 16.1 Wire Format Overview

The swarm bus uses a **length-prefixed, zero-copy binary protocol** optimized for sub-microsecond latency on shared-memory transports and low overhead on network transports.

```
┌──────────────────────────────────────────────────────────┐
│                    Swarm Bus Frame                        │
├────────┬────────┬────────┬───────────────────────────────┤
│ Header │ Routing│ Payload│ Checksum                      │
│ 16 B   │ 24 B   │ var    │ 4 B                           │
└────────┴────────┴────────┴───────────────────────────────┘
```

### 16.2 Frame Header (16 bytes)

```
Offset  Size  Field            Description
──────  ────  ──────────────── ─────────────────────────────────
0       4     magic            0x52445853 ("RDXS" — MechGen Swarm)
4       1     version          Protocol version (current: 0x01)
5       1     flags            Bit flags (see below)
6       2     message_type     SwarmMessage discriminant (u16)
8       4     payload_length   Payload size in bytes (u32, max 4 GiB)
12      4     sequence_number  Monotonic sequence per sender (u32)

Flags (bit field):
  bit 0: compressed       (1 = LZ4-compressed payload)
  bit 1: encrypted        (1 = ChaCha20-Poly1305 encrypted)
  bit 2: requires_ack     (1 = sender expects acknowledgment)
  bit 3: priority         (1 = high priority, skip normal queue)
  bit 4: fragmented       (1 = this is a fragment of a larger message)
  bit 5: last_fragment    (1 = this is the last fragment)
  bit 6-7: reserved
```

### 16.3 Routing Header (24 bytes)

```
Offset  Size  Field            Description
──────  ────  ──────────────── ─────────────────────────────────
0       8     sender_id        Agent ID (u64, globally unique)
8       8     receiver_id      Target agent ID (0 = broadcast)
16      4     channel_id       Logical channel (u32)
20      2     hop_count        TTL for multi-hop routing (u16)
22      2     correlation_id   Links request/response pairs (u16)
```

### 16.4 Message Type Registry

```
Type ID   SwarmMessage Variant      Category        Ack Required
───────   ─────────────────────     ──────────────  ────────────
0x0001    TaskAssignment            Coordination    Yes
0x0002    TaskCompleted             Coordination    Yes
0x0003    TaskFailed                Coordination    Yes
0x0010    ProposeChange             Consensus       Yes
0x0011    VoteOnChange              Consensus       Yes
0x0012    ChangeAccepted            Consensus       No (broadcast)
0x0020    QueryRequest              Query           Yes
0x0021    QueryResponse             Query           No
0x0030    ConflictDetected          Conflict        Yes
0x0031    ConflictResolution        Conflict        Yes
0x0040    DiscoveredPattern         Knowledge       No
0x0041    SharedInsight             Knowledge       No
0x0050    Heartbeat                 Health          No
0x0051    LeaseRequest              Lease           Yes
0x0052    LeaseGranted              Lease           No
0x0053    LeaseRevoked              Lease           Yes
0x00F0    Ack                       Control         No
0x00F1    Nack                      Control         No
0x00F2    FlowControl               Control         No
0x00FF    Ping                      Control         Yes (Pong)
0xFE00-   Custom (agent-defined)    Extension       Configurable
0xFEFF
```

### 16.5 Payload Serialization

Payloads use a **FlatBuffers-inspired zero-copy format** with a MechGen-specific schema:

```
Payload Layout:
┌─────────┬──────────────────────────────────────────┐
│ VTable  │ Data                                      │
│ (var)   │ (var, aligned to 8 bytes)                 │
└─────────┴──────────────────────────────────────────┘

VTable:
┌───────┬───────┬────────┬────────────────────────────┐
│ nflds │ dsize │ off[0] │ off[1] ... off[nflds-1]    │
│ u16   │ u16   │ u16    │ u16 each                   │
└───────┴───────┴────────┴────────────────────────────┘

  nflds:  number of fields
  dsize:  total data section size
  off[i]: byte offset of field i within the data section
          (0xFFFF = field not present / null)

Data Section:
  - Scalars: stored inline, naturally aligned
  - Strings: u32 length prefix + UTF-8 bytes + null terminator
  - Vectors: u32 count + elements (each aligned)
  - Nested: recursive VTable + Data (offset stored as u32 pointer)
  - Enums:  u16 discriminant + variant payload
```

#### 16.5.1 Example: TaskAssignment Serialization

```
TaskAssignment {
    region: SemanticRegion,        // field 0: nested
    constraints: Vec<Constraint>,  // field 1: vector of nested
}

Binary (hex, little-endian):
  VTable:  02 00  28 00  04 00  1C 00
           │       │      │      │
           nflds=2 dsize  off[0] off[1]

  Data:
  [off 0x04] SemanticRegion VTable + Data (nested)
  [off 0x1C] 03 00 00 00   -- Vec length = 3
             Constraint[0] VTable + Data
             Constraint[1] VTable + Data
             Constraint[2] VTable + Data
  [pad to 8-byte alignment]
```

### 16.6 Transport Layers

The wire protocol is transport-agnostic. Three transports are specified:

#### 16.6.1 Shared Memory (Local Swarm)

```
Shared Memory Ring Buffer:
┌──────────────────────────────────────────────────────┐
│ Control Block (cache-line aligned, 64 bytes)          │
│ ┌──────────┬──────────┬──────────┬─────────────────┐ │
│ │ write_idx│ read_idx │ capacity │ flags            │ │
│ │ AtomicU64│ AtomicU64│ u64      │ AtomicU64        │ │
│ └──────────┴──────────┴──────────┴─────────────────┘ │
├──────────────────────────────────────────────────────┤
│ Ring Buffer (2^N slots, each slot = max_frame_size)   │
│ ┌──────┬──────┬──────┬──────┬─────────────────────┐  │
│ │Slot 0│Slot 1│Slot 2│ ...  │ Slot 2^N - 1        │  │
│ └──────┴──────┴──────┴──────┴─────────────────────┘  │
└──────────────────────────────────────────────────────┘

Write path (lock-free):
  1. Atomically increment write_idx (CAS)
  2. Copy frame into slot[write_idx % capacity]
  3. Memory fence (release)
  4. Mark slot as ready (store-release on slot header)

Read path (lock-free):
  1. Spin on slot[read_idx % capacity] ready flag
  2. Read frame from slot (zero-copy: return pointer)
  3. Memory fence (acquire)
  4. Atomically increment read_idx

Latency: < 100 ns per message (single producer, single consumer)
Throughput: > 10 million messages/second
```

#### 16.6.2 Unix Domain Socket (Local Network)

```
Connection: SOCK_SEQPACKET (message boundaries preserved)
Framing: Length-prefixed (4-byte big-endian length + frame bytes)
Latency: ~1 μs per message
Throughput: > 1 million messages/second
```

#### 16.6.3 TCP/TLS (Remote Swarm)

```
Connection: TCP with optional TLS 1.3 (ChaCha20-Poly1305)
Framing: Same length-prefixed format
Keepalive: Ping/Pong every 5 seconds
Reconnect: Exponential backoff (100ms, 200ms, 400ms, ..., 30s cap)
Latency: Network RTT + ~10 μs protocol overhead
Throughput: Bounded by network bandwidth
```

### 16.7 Flow Control

Credit-based flow control prevents fast producers from overwhelming slow consumers:

```
Flow Control Protocol:
  1. On connection, receiver grants initial credits (e.g., 256)
  2. Each message sent consumes 1 credit
  3. When credits reach 0, sender blocks (or buffers, per config)
  4. Receiver periodically sends FlowControl messages to replenish:

     FlowControl {
         credits_granted: u32,  // new credits to add
         backpressure: f32,     // 0.0 = idle, 1.0 = overloaded
         queue_depth: u32,      // receiver's current queue depth
     }

  5. Sender adapts rate based on backpressure signal:
     - backpressure < 0.5: send at full rate
     - backpressure 0.5–0.8: reduce rate by 50%
     - backpressure > 0.8: send only high-priority messages

High-priority messages (flag bit 3) bypass flow control entirely.
```

### 16.8 Connection Lifecycle

```
┌───────────┐                              ┌───────────┐
│  Agent A  │                              │  Agent B  │
└─────┬─────┘                              └─────┬─────┘
      │                                          │
      │── Connect(agent_id, capabilities) ──────→│
      │                                          │
      │←── Accepted(initial_credits, config) ────│
      │                                          │
      │══ Data Phase (full-duplex) ══════════════│
      │── Frame(seq=1, TaskAssignment) ─────────→│
      │←── Ack(seq=1) ──────────────────────────│
      │←── Frame(seq=1, TaskCompleted) ─────────│
      │── Ack(seq=1) ──────────────────────────→│
      │                                          │
      │── Ping ─────────────────────────────────→│
      │←── Pong ────────────────────────────────│
      │                                          │
      │←── FlowControl(credits=128) ───────────│
      │                                          │
      │═════════════════════════════════════════│
      │                                          │
      │── Disconnect(reason) ───────────────────→│
      │←── DisconnectAck ──────────────────────│
      │                                          │
```

### 16.9 Error Handling

| Error Code | Name                | Recovery                           |
| ---------- | ------------------- | ---------------------------------- |
| 0x01       | InvalidMagic        | Close connection                   |
| 0x02       | VersionMismatch     | Negotiate down or close            |
| 0x03       | PayloadTooLarge     | Reject with Nack, suggest fragment |
| 0x04       | ChecksumMismatch    | Request retransmit                 |
| 0x05       | UnknownMessageType  | Skip with Nack(unsupported)        |
| 0x06       | SequenceGap         | Request retransmit of missing seq  |
| 0x07       | DecryptionFailed    | Close connection (security)        |
| 0x08       | DecompressionFailed | Request retransmit uncompressed    |
| 0x09       | CapabilityDenied    | Nack with required capabilities    |
| 0x0A       | CreditExhausted     | Wait for FlowControl replenish     |

### 16.10 Checksum

The frame checksum is **CRC-32C** (Castagnoli) computed over the entire frame excluding the checksum field itself. CRC-32C is chosen for its hardware acceleration support on x86 (`crc32` instruction) and ARM (`crc32c` instruction), enabling zero-overhead integrity verification.

```
Checksum computation:
  crc = CRC32C(header[0..16] || routing[0..24] || payload[0..payload_length])
  frame[total_length - 4 .. total_length] = crc.to_le_bytes()

Verification:
  expected = CRC32C(frame[0 .. total_length - 4])
  actual   = u32::from_le_bytes(frame[total_length - 4 .. total_length])
  valid    = expected == actual
```

---

## Summary

MechGen transforms Rust from a language *for human developers with CLI tools* into a language *for swarms of AI agents with maximum parsing speed, communication throughput, hardware-agnostic performance, and minimum token cost*. The transformation **shifts safety from compile-time enforcement to a queryable Safety Knowledge Base**, eliminates every source of agent parsing ambiguity, introduces a portable performance IR that targets any hardware, and compresses every language construct to its **minimum token footprint**:

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
18. **Token budget reporting** — `mechgen build --token-report` tracks per-function token expenditure for agent optimization
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

The compiler becomes an **agentic AI system, synthesis engine, and swarm arbiter**, built on the **MLIR + LLVM** compiler infrastructure — the broadest and most mature in existence. Its MLIR dialect encodes the full language semantics (ownership, effects, contracts) as first-class operations — not metadata on a generic IR — enabling MLIR-native autotuning, automatic device placement, and compile-time metaprogramming that survives through the entire optimization pipeline. Its **Agentic Compiler Intelligence (ACI)** learns from the project's bug history, swarm session outcomes, and codebase patterns to provide dynamic warnings, intelligent debugging, and performance suggestions that static analyzers cannot match. Its **Cost Oracle** transforms agent decision-making from guess-and-profile to query-and-choose. Its **Synthesis Oracle** closes the loop from formal spec to verified implementation. Its **Agent Memory Model** ensures that every lesson learned persists across sessions, projects, and the ecosystem. Its primary job is *making code run fast on any hardware with the fewest tokens possible while actively helping agents write better code*, not blocking submissions with safety errors that agents already know how to avoid. Safety knowledge lives in a database. Performance lives in MLIR's multi-level optimization pipeline and LLVM's battle-tested backends. Communication lives in the swarm bus. Parsing lives in a zero-ambiguity grammar. Compiler intelligence lives in a learned model that improves with every build. Synthesis lives in a formal specification system. Memory lives in a four-tier persistent store. And every construct lives in its **most compressed form** — because tokens are the currency of agentic intelligence, and MechGen is designed to spend them wisely.

Every error the compiler detects is an error it can **fix** — auto-repair candidates ranked by confidence eliminate the agent round-trip tax. Every function can be **hot-patched** into a running process without restart — because agent swarms never stop iterating. Every foreign library is accessible through **zero-ceremony FFI** — C headers, Python stubs, WASM components read directly by the compiler. Every agent runs in a **capability-bounded sandbox** — memory-limited, CPU-bounded, audit-trailed, deterministically replayable. And common multi-agent workflows — map-reduce, pipeline, scatter-gather, saga — are **first-class language constructs** verified by the compiler for deadlock freedom and contract satisfaction.

This is not Rust made safe. This is Rust made *fast*, *parseable*, *communicative*, *intelligent*, *self-evolving*, *self-healing*, and *token-efficient* — built on MLIR and LLVM, with an AI-powered compiler, for the age of agent swarms.
