# Redox Implementation Roadmap

> Tracking progress from prototype toward production. Steps 1–22 completed prior.
> Each step is a concrete, testable increment.

## Legend

- ✅ Complete
- 🔧 In Progress
- ⬚ Not Started

---

## Phase A: Compiler Foundation (Steps 23–28)

| Step | Title                              | Status | Description                                                                                                                       |
| ---- | ---------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| 23   | Complete the lexer                 | ✅      | Cover all 60+ keyword/attribute/type mappings from §5.5 (ep"...", `@@`, `?=`, `~>`, `af`, `uf`, `Y`, `Z`, `R`, `Ok`, `Err`, etc.) |
| 24   | Complete the parser                | ✅      | Proper LL(1) with all Redox syntax forms: contracts, specs, effects decl, capability blocks, swarm patterns, perf annotations     |
| 25   | Structured Diagnostic Graph        | ✅      | Replace flat error strings with DiagnosticGraph (§6.2): fix candidates, confidence, causal chains, related errors                 |
| 26   | Safety elision pass                | ✅      | Strip lifetimes, `unsafe`, `&mut`, `move`, `ref`, `Pin`, `PhantomData`, `Send`/`Sync` from AST in agentic mode                    |
| 27   | Dual-syntax transpiler integration | ✅      | `--syntax=legacy` flag: accept Rust syntax via rust2rdx, feed canonical form to compiler                                          |
| 28   | Token budget reporting             | ✅      | `--token-report` per-function/module token counts, compact vs expanded metrics                                                    |

## Phase B: Agentic Core Deepening (Steps 29–35)

| Step | Title                        | Status | Description                                                                                                                                        |
| ---- | ---------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| 29   | Deepen the SKB               | ✅      | Expand from 16 to 200+ rules across 6 databases (ownership, borrow, lifetime, type, concurrency, FFI)                                              |
| 30   | Contract syntax & checking   | ✅      | `@req`/`@ens`/`@inv` in parser + AST + verification oracle integration                                                                             |
| 31   | Formal specification syntax  | ✅      | `spec` blocks with `@req`/`@ens`/`@perf`/`@fx`, parsed and stored in AST                                                                           |
| 32   | Refinement types             | ✅      | Value-level type constraints (`NonZero[u32]`, `Range[0..100]`) in type checker                                                                     |
| 33   | Capability system            | ✅      | `agent` keyword + `AgentDef` AST, capability declarations, bracket-list parser, verification oracle, known-cap taxonomy |
| 34   | Deepen self-healing          | ✅      | 17 error patterns (was 6): borrow/move, unused-var, missing-field, contract @req/@ens/@inv, capability-denied, perf-budget |
| 35   | Attribute compression system | ⬚      | Full `#[...]` → `@...` mapping: `@d`, `@r`, `@mu`, `@a`, `@x`, `@cfg`, `@t`, `@b`, `@se`, `@pi!`, `@pnb`, `@pv`, `@pt`, `@pa`, `@pp`, `@as`, `@ac` |

## Phase C: Agent Protocol & Services (Steps 36–41)

| Step | Title                      | Status | Description                                                                                                                                                                               |
| ---- | -------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 36   | Expand RAP server          | ⬚      | From 9 to 25+ methods: format/compact, format/expand, lint/check, doc/query, token/report, grammar/extensions, grammar/expand, aci/*, sandbox/*, ffi/*, hotpatch/*, memory/*, synthesis/* |
| 37   | redoxfmt service           | ⬚      | `--compact` (min tokens) and `--expand` (human-readable), bidirectional lossless AST conversion                                                                                           |
| 38   | Agent discovery attributes | ⬚      | `@as("...")`, `@ac("...")`, `@ax("...")`, `@ao("...")`, `@ae("...")` in lexer/parser/AST                                                                                                  |
| 39   | Grammar extension system   | ⬚      | `grammar_extension!` macro, Redox.toml registration, namespace-scoped discovery, frequency promotion                                                                                      |
| 40   | Capability manifests       | ⬚      | JSON manifest generation per crate, capability-indexed search in Forge                                                                                                                    |
| 41   | MLIR dialect definition    | ⬚      | Define Redox MLIR dialect ops: ownership, effects, contracts, perf annotations as first-class MLIR operations (textual spec)                                                              |

## Phase D: Swarm Runtime (Steps 42–48)

| Step | Title                     | Status | Description                                                                                               |
| ---- | ------------------------- | ------ | --------------------------------------------------------------------------------------------------------- |
| 42   | Semantic lease manager    | ⬚      | SharedRead / ExclusiveWrite / Restructuring leases on semantic regions, lease timeout, deadlock detection |
| 43   | CRDT merge engine         | ⬚      | Semantic CRDTs on AST/HIR: InsertItem, ModifyBody, ModifySignature, AddImpl, Rename with merge semantics  |
| 44   | Consensus protocol        | ⬚      | 5-phase: Propose → Impact Analysis → Vote → Resolve → Integrate for shared interface changes              |
| 45   | Task decomposition engine | ⬚      | Dependency-aware parallel work splitting, critical path computation, agent assignment                     |
| 46   | Swarm message bus         | ⬚      | Typed SwarmMessage enum, zero-copy serialization, sub-µs latency target                                   |
| 47   | Swarm SDK                 | ⬚      | `redox_swarm` crate: derive macros, role taxonomy, SwarmAgent trait, example orchestrator                 |
| 48   | Semantic VCS              | ⬚      | Operation-log-based version control, semantic branching/merging, intent-based history queries             |

## Phase E: Advanced Subsystems (Steps 49–55)

| Step | Title                     | Status | Description                                                                                                                 |
| ---- | ------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| 49   | Synthesis oracle          | ⬚      | Spec→candidate generation, candidate ranking by cost, verification of candidates against specs                              |
| 50   | ACI subsystem             | ⬚      | Dynamic Warning Engine, Intelligent Debugging Engine, Performance Advisor, Swarm Coordination Intelligence, 8 RAP endpoints |
| 51   | Verification certificates | ⬚      | Machine-checkable proofs: memory safety, data-race freedom, contract satisfaction, effect containment                       |
| 52   | FFI binding generator     | ⬚      | Auto-bind from C headers (parse .h), Python stubs (.pyi), WASM (.wit); safe wrapper generation                              |
| 53   | Hot-reload runtime        | ⬚      | Function-level live patching, MLIR single-function re-lowering stubs, rollback management                                   |
| 54   | Capability-based sandbox  | ⬚      | Per-agent isolation, resource limits (mem/CPU/syscalls), capability attenuation, audit logging                              |
| 55   | Performance annotations   | ⬚      | `@pi!`, `@pnb`, `@pv(N)`, `@pt(target)`, `@pa(N)`, `@pp`, `#[repr(target_optimal)]` processing                              |

## Phase F: Stdlib & Ecosystem (Steps 56–60)

| Step | Title                        | Status | Description                                                                                |
| ---- | ---------------------------- | ------ | ------------------------------------------------------------------------------------------ |
| 56   | Deepen stdlib                | ⬚      | Batch APIs, streaming I/O, SwarmVec, ArenaVec, SwarmChannel, per-agent arena allocators    |
| 57   | Deepen Forge registry        | ⬚      | Capability-indexed search, semantic search by capability query, contract-based composition |
| 58   | Agentic benchmarking suite   | ⬚      | Token throughput, parse error rate, synthesis success rate, swarm latency metrics          |
| 59   | Cost model calibration       | ⬚      | Standardized benchmarks for cost oracle accuracy across targets                            |
| 60   | Language specification draft | ⬚      | Formal Redox language specification document                                               |

## Phase G: Documentation & Training (Steps 61–63)

| Step | Title                | Status | Description                                                                         |
| ---- | -------------------- | ------ | ----------------------------------------------------------------------------------- |
| 61   | Update documentation | ⬚      | Book, cookbook, agent-guide, internals for all new features                         |
| 62   | Update training data | ⬚      | JSONL samples for contracts, specs, swarm patterns, ACI, synthesis, FFI             |
| 63   | Example projects     | ⬚      | End-to-end examples: swarm audit, capability-sandboxed agent, spec-driven synthesis |

---

## Prior Steps (1–22): ✅ All Complete

| Step | Title                                                                                                                   |
| ---- | ----------------------------------------------------------------------------------------------------------------------- |
| 1    | Prototype compiler (lexer, parser, AST, HIR, types, effects, MLIR, resolver)                                            |
| 2    | rust2rdx transpiler                                                                                                     |
| 3    | VS Code extension                                                                                                       |
| 4    | Safety Knowledge Base (SKB)                                                                                             |
| 5    | Benchmarks                                                                                                              |
| 6    | End-to-end demo                                                                                                         |
| 7    | rdx CLI                                                                                                                 |
| 8    | Standard library stubs                                                                                                  |
| 9    | Redox Book                                                                                                              |
| 10   | Cookbook                                                                                                                |
| 11   | Agent Guide                                                                                                             |
| 12   | Migration Guide                                                                                                         |
| 13   | Internals Guide                                                                                                         |
| 14   | Quick Start Guide                                                                                                       |
| 15   | rdx2rs back-transpiler                                                                                                  |
| 16   | Example projects                                                                                                        |
| 17   | CI/CD pipeline                                                                                                          |
| 18   | Editor configs                                                                                                          |
| 19   | Agent training data corpus                                                                                              |
| 20   | Community infrastructure                                                                                                |
| 21   | Forge package registry                                                                                                  |
| 22   | Agentic AI integration (self-healing, cost oracle, SKB query engine, verification oracle, agent memory, swarm patterns) |
