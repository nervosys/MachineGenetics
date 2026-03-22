# Redox Production Roadmap

> Transition from prototype simulation to actual compiler implementation.
> Each step modifies the real compiler crates (`compiler/`, `library/`, `src/`).
> Derived from REDOX_PROPOSAL.md Section 11: Phased Implementation Plan.

**Status Legend**: ⬜ Not started | 🔄 In progress | ✅ Complete

---

## Phase 0: Foundation (Steps 1–30)

### Rebrand & Build Infrastructure

- [x] **Step 1**: Create automated rename script (`scripts/rebrand.py`) that renames all `rustc_*` crate directories under `compiler/` to `redox_*`, updates every `Cargo.toml` package name, and rewrites all `use rustc_*` / `extern crate rustc_*` imports across the entire tree. Validate with `grep -r "rustc_" compiler/` showing zero hits post-rename. Reference: REDOX_PROPOSAL.md §4.2 Compiler Crate Ontology.
- [x] **Step 2**: Update the build system (`src/bootstrap/`, `x.py`, `configure`, `bootstrap.example.toml`) to use `redox_*` crate names. Update `Cargo.toml` workspace members list. Ensure `python x.py check` succeeds with the rebranded crates.
- [x] **Step 3**: Rename the top-level compiler driver from `compiler/rustc/` to `compiler/redox/` and update all references. The produced binary should be `redoxc` (or `redox` as a compiler command). Update `src/tools/` references.
- [x] **Step 4**: Establish CI/CD pipeline: add `.github/workflows/redox-ci.yml` with build, test, and lint jobs for the rebranded compiler. Verify the compiler bootstraps successfully (stage 0 → stage 1 at minimum).

### Lexer: Token-Compressed Keywords & Redox Tokens

- [x] **Step 5**: Extend `redox_lexer` (`compiler/redox_lexer/src/lib.rs`) — add new `TokenKind` variants for Redox-specific tokens: `@@` (discovery), `?=` (refinement), `~>` (pipeline), `@` prefix for compact attributes. Add unit tests for each new token.
- [x] **Step 6**: Implement token-compressed keyword recognition in the lexer. Add recognition for single-character keywords: `v` (let), `f` (fn), `t` (type), `s` (struct), `e` (enum), `m` (mod), `p` (pub), `i` (impl), `S` (Self), `E` (Effect), `T` (Trait), `I` (Interface). These should be recognized in canonical syntax mode only (controlled by a session flag). Add comprehensive lexer tests.
- [x] **Step 7**: Implement sigil prefix token recognition in the lexer: `+f` (async fn), `-f` (const fn), `!f` (unsafe fn), `*f` (extern fn). Add type abbreviation tokens: `?T` (Option<T>), `R[T,E]` (Result<T,E>), `V[T]` (Vec<T>), `[T]~` (mutable slice). Add lexer tests.
- [x] **Step 8**: Implement compact attribute lexing: `@d` (#[derive(...)]), `@r` (#[repr(...)]), `@t` (#[test]), `@i` (#[inline]), `@as` (agent_spec), `@ac` (agent_contract), `@ax` (agent_effect), `@ao` (agent_capability), `@ae` (agent_entry). The lexer should emit `CompactAttribute(kind)` tokens. Add tests.

### Parser: LL(1) Canonical Grammar

- [x] **Step 9**: Create `compiler/redox_grammar/` crate containing the formal LL(1) grammar specification for Redox canonical syntax as a structured data file (grammar rules, FIRST/FOLLOW sets). This serves as the single source of truth for the parser. Include grammar validation tests (no LL(1) conflicts).
- [x] **Step 10**: Modify `redox_parse` to support dual-mode parsing: add a `SyntaxMode` enum (`Canonical`, `Legacy`) to the parser state, controlled by the session configuration. In `Legacy` mode, the parser behaves identically to upstream `rustc_parse`. In `Canonical` mode, it accepts the new LL(1) grammar. Add plumbing tests.
- [x] **Step 11**: Implement `spec` block parsing in `redox_parse` (canonical mode). A `spec` block contains `@req` (precondition), `@ens` (postcondition), `@perf` (cost bound), and `@fx` (effect declaration) clauses. Parse into new AST nodes. Add parser tests with valid and invalid spec blocks.
- [x] **Step 12**: Implement contract syntax parsing in `redox_parse`: `@req(expr)`, `@ens(expr)`, `@inv(expr)` as function/method attributes in canonical mode. Parse into `ContractAttr` AST nodes. Add parser tests.
- [x] **Step 13**: Implement effect declaration parsing: `effect name { fn ...; }` blocks and effect annotations on functions (`fn foo() -> io T`). Parse into `EffectDecl` and `EffectAnnotation` AST nodes. Add parser tests.
- [x] **Step 14**: Implement capability block parsing: `capability name { ... }` declarations and `#[capability(name)]` / `@ao(name)` annotations. Parse into `CapabilityDecl` AST nodes. Add parser tests.
- [x] **Step 15**: Implement compact keyword parsing in canonical mode: recognize `v` as `let`, `f` as `fn`, `s` as `struct`, `e` as `enum`, etc. The parser should accept both compact and full keywords. Add comprehensive round-trip parser tests.

### AST: Redox Node Types

- [x] **Step 16**: Extend `redox_ast` with new AST node types: `SpecBlock`, `ContractAttr`, `EffectDecl`, `EffectAnnotation`, `CapabilityDecl`, `CapabilityBlock`, `RefinementType`, `PerfAnnotation`. Add visitor trait implementations for all new nodes. Add serialization support.
- [x] **Step 17**: Extend `redox_ast_pretty` to pretty-print all new Redox AST nodes in both canonical (compact) and expanded (human-readable) forms. Add round-trip pretty-print tests: parse → print → re-parse should be identity.

### Safety Elision Pass

- [x] **Step 18**: Add a `Redox.toml` configuration file format (parsed via a new `redox_config` crate or extension of `redox_session`). Support `[safety]` section with `mode = "agent" | "human" | "ci"`, `borrow-check`, `lifetime-check`, `bounds-check`, `overflow-check` settings per the proposal §9.4. Add config parsing tests.
- [x] **Step 19**: Implement the safety elision pass in `redox_ast_lowering` (or as a MIR pass). When `safety.mode = "agent"`, strip all explicit lifetime annotations, borrow annotations, and `unsafe` blocks during lowering. The compiler should infer these from usage context. Add tests showing identical compilation output with and without explicit annotations.
- [x] **Step 20**: Implement safety-free type inference extensions in `redox_hir_typeck` / `redox_infer`. When in agent mode, the compiler should infer: `&` vs `&mut` from usage, `move` vs `ref` from context, `dyn` vs `impl` from call sites. Add tests with the safety-free function signatures from the proposal §5.2.1.

### Dual-Syntax Transpiler & Formatter

- [x] **Step 21**: Build the `rust2redox` transpiler (can live in `src/tools/rust2redox/`). Input: valid Rust source. Output: canonical Redox compact form. Must handle: keyword compression, attribute compression, lifetime/borrow elision, type abbreviation. Add tests with the examples from REDOX_PROPOSAL.md §5.4 and §5.7.
- [x] **Step 22**: Implement `redoxfmt` tool (in `src/tools/redoxfmt/`). Two modes: `--compact` (minimum-token canonical form, single-char keywords, all abbreviations applied) and `--expand` (fully-expanded human-readable form with full keywords and all safety annotations). Add round-trip test: `compact(expand(src)) == compact(src)`.

### Diagnostics & Query API

- [x] **Step 23**: Implement the Structured Diagnostics Protocol in `redox_diagnostics` (extending `redox_errors`). Emit diagnostics as JSON diagnostic graphs with: error node, cause chain, fix candidates (each with token cost and confidence), related locations. Reference: REDOX_PROPOSAL.md §6.2. Add tests for structured diagnostic output.
- [x] **Step 24**: Create `redox_query` crate (or extend `redox_public`) that externalizes core compiler queries as a stable API. Agents should be able to query: `tokens_of(file)`, `ast_of(file)`, `type_of(expr)`, `impl_of(trait, type)`, `diagnostics_of(file)`. Add integration tests.

### Abbreviation Registry

- [x] **Step 25**: Create the standard abbreviation registry v1 as a data file in `compiler/redox_lexer/` (or `compiler/redox_config/`). Map all core types (`V[T]`→`Vec<T>`, `S[T]`→`String`, `?T`→`Option<T>`, `R[T,E]`→`Result<T,E>`, etc.), traits (`D`→`Debug`, `Cl`→`Clone`, `Cp`→`Copy`, `Df`→`Default`, etc.), and derives. The lexer and parser should use this registry. Add registry lookup tests.

### Semantic Region Decomposition

- [x] **Step 26**: Implement semantic region decomposition in the compiler query system. Each source file is divided into semantic regions (function, impl block, module, type definition) that can be independently queried, parsed, and compiled. Region boundaries are exposed via the query API. Add tests showing independent region compilation.

### MLIR Integration Foundation

- [x] **Step 27**: Set up MLIR build infrastructure. Add MLIR as a dependency alongside the existing LLVM dependency in `src/llvm-project/`. Create `compiler/redox_mlir/` crate with Rust FFI bindings to the MLIR C API. Verify that the MLIR libraries build and link correctly. Add a smoke test that creates and destroys an MLIR context.
- [x] **Step 28**: Define the Redox MLIR dialect using TableGen ODS (in `compiler/redox_mlir/dialect/`). Define types: `OwnedType`, `RefType`, `RegionType`, `EffectType`, `CapabilityType`. Define operations: `redox.move`, `redox.copy`, `redox.borrow`, `redox.drop` (ownership); `redox.effect.decl`, `redox.effect.perform`, `redox.effect.handle` (effects); `redox.contract.require`, `redox.contract.ensure`, `redox.contract.invariant` (contracts); performance annotation ops. Reference: REDOX_PROPOSAL.md §14. Add dialect registration and verification tests.
- [x] **Step 29**: Implement MIR → Redox MLIR dialect translation layer in `compiler/redox_mlir/`. Translate MIR basic blocks, terminators, places, rvalues, and operands into Redox MLIR operations. This is a thin boundary layer where MIR semantics map to dialect operations. Add translation tests for basic MIR patterns (assignment, function call, branch, drop).
- [x] **Step 30**: Implement MLIR progressive lowering pipeline: Redox Dialect → standard MLIR dialects (Linalg, Affine, Vector, SCF) → LLVM Dialect. Wire the LLVM backend codegen through MLIR LLVM Dialect, replacing the direct MIR→LLVM IR path. Verify that a simple Redox program compiles end-to-end through the MLIR pipeline. Add integration test.

---

## Phase 1: SKB + Swarm Primitives + Multi-Target + Cost Oracle (Steps 31–46)

### Safety Knowledge Base

- [x] **Step 31**: Build the Safety Knowledge Base (SKB) as `compiler/redox_skb/`. Implement the rule schema (category, subcategory, severity, condition, remedy, references). Seed with an initial corpus: ownership rules (2,847), borrowing rules (1,203), lifetime rules (894), type safety rules (3,412), concurrency rules (567), FFI rules (234) — totaling ~9,157 rules. Reference: REDOX_PROPOSAL.md §15.
- [x] **Step 32**: Implement the SKB query API and SKB-QL query language parser. Support queries like `QUERY rules WHERE category = "ownership" AND severity >= "error"`. Implement dual-index architecture (B-tree on category + hash on pattern signature). Performance target: avg 0.02ms, P99 0.20ms query time. Add query benchmarks.
- [x] **Step 33**: Make all safety compiler passes opt-in via `Redox.toml` safety profiles. Implement profile presets: `agent-dev` (all checks skipped), `human-dev` (all checks enforced), `ci-pipeline` (all enforced), `production` (all enforced). Wire profile selection into the compiler session. Add tests.

### Agent Infrastructure

- [x] **Step 34**: Build `redox_index` — a persistent semantic knowledge graph that indexes all symbols, types, traits, impls, and their relationships across crates. Store as a queryable database (embedded, e.g., SQLite or custom B-tree). Expose via the query API.
- [x] **Step 35**: Implement capability inference pass in the MIR pipeline. Analyze function bodies to determine required capabilities (alloc, io, unsafe, panic, etc.). Store inferred capabilities in function metadata. Add tests.
- [x] **Step 36**: Extend `redox_metadata` with capability manifest serialization. Each compiled crate produces a capability manifest JSON alongside its rlib/dylib. Implement the schema from REDOX_PROPOSAL.md §10.2.
- [x] **Step 37**: Build prototype RAP (Redox Agent Protocol) server, merging rust-analyzer-style IDE support with direct compiler query access. The server should expose: `query.*`, `tokens.*`, `ast.*`, `type.*`, `diagnostic.*` endpoints. Reference: REDOX_PROPOSAL.md §8.2.

### Token Economy & Cost Oracle

- [x] **Step 38**: Implement agent discovery attributes in the compiler: `@as` (agent_spec), `@ac` (agent_contract), `@ax` (agent_effect), `@ao` (agent_capability), `@ae` (agent_entry). Wire through parsing, AST, lowering, and metadata emission.
- [x] **Step 39**: Implement the attribute compression system — map `#[derive(...)]` ↔ `@d(...)`, `#[repr(...)]` ↔ `@r(...)`, `#[test]` ↔ `@t`, `#[inline]` ↔ `@i`, etc. Store mappings in the abbreviation registry. Add round-trip tests.
- [x] **Step 40**: Implement token budget reporting: `redox build --token-report` outputs per-file and per-function token counts, with compact vs expanded savings metrics.
- [x] **Step 41**: Implement the Cost Oracle (REDOX_PROPOSAL.md P38): per-target cost queries for expressions, types, and operations. Expose via `cost.query(expr, target)` API. Implement multi-target cost comparison (`cost.compare`). Seed with initial cost models for x86-64, AArch64, WASM.

### Swarm Primitives

- [x] **Step 42**: Implement the semantic lease manager. Semantic leases grant shared-read or exclusive-write access to code regions. Implement lease acquisition, release, timeout, and deadlock detection. Add concurrency tests.
- [x] **Step 43**: Build the CRDT-based semantic merge engine for concurrent AST/HIR modifications. Implement the CRDT types from REDOX_PROPOSAL.md §7.4: `SemanticCRDT` with `InsertNode`, `DeleteNode`, `MoveNode`, `RenameSymbol`, `ChangeType`, `AddImport` operations. Add merge conflict resolution tests.
- [x] **Step 44**: Implement the swarm message bus with zero-copy FlatBuffers-inspired serialization. Support three transport layers: shared memory (<100ns), Unix domain sockets (~1μs), TCP/TLS (network). Implement the wire protocol from REDOX_PROPOSAL.md §16 (frame header, routing header, CRC-32C). Add latency benchmarks.
- [x] **Step 45**: Validate MLIR→LLVM backend targets: x86-64, AArch64, WASM. Run the Redox compiler test suite producing binaries for each target. Verify correctness with target-specific test harnesses.
- [x] **Step 46**: Phase 1 integration testing: end-to-end test that an agent (simulated) can query the SKB, acquire a semantic lease, compile code with safety elision, receive structured diagnostics, and query the cost oracle.

---

## Phase 2: Agent Protocol + Swarm Coordination + GPU/NPU Targets + ACI (Steps 47–67)

- [x] **Step 47**: Define and implement the full Redox Agent Protocol (RAP) specification: JSON-RPC request/response format, capability negotiation, session management.
- [x] **Step 48**: Build the agent capability system and enforcement layer: per-agent capability bounds, attenuation (child ≤ parent), runtime enforcement at the swarm bus level.
- [x] **Step 49**: Implement the verification oracle as an opt-in service: verify contracts, effects, and capabilities at compile time. Emit verification certificates.
- [x] **Step 50**: Build the swarm SDK (`compiler/redox_swarm/` or `library/redox_swarm/`) with orchestrator, synthesizer, and verifier agent roles.
- [x] **Step 51**: Implement the consensus protocol engine: propose → vote → resolve → integrate cycle. Support configurable quorum rules.
- [x] **Step 52**: Build the task decomposition engine: dependency-aware parallel work splitting with DAG scheduling. No cycles in assignment graph (enforced by orchestrator).
- [x] **Step 53**: Implement semantic VCS: operation-log-based version control for agent swarms, replacing git-level merges with semantic-operation-level merges.
- [x] **Step 54**: Integrate the RAP server with IDE infrastructure (VS Code extension, LSP integration).
- [x] **Step 55**: Build swarm audit log system: append-only, cryptographically signed (SHA-256) operation history with agent ID attribution.
- [x] **Step 56**: Enable MLIR→LLVM backend targets: RISC-V, AMDGPU, NVPTX. Add target-specific lowering passes and validation tests.
- [x] **Step 57**: Implement MLIR SPIR-V dialect pipeline for Vulkan/OpenCL GPU compute.
- [x] **Step 58**: Implement MLIR-native autotuning engine: `@pa(N)` generates N variants, benchmarks per-target, selects optimal.
- [x] **Step 59**: Implement automatic device placement: `@pt(auto)` uses MLIR cost model to select device. Agent-queryable via RAP.
- [ ] **Step 60**: Implement hardware-agnostic parallelism via MLIR OpenMP/GPU/async dialects.
- [ ] **Step 61**: Build ACI Codebase Model: fine-tune small LLM on project source + SKB + swarm history for project-specific intelligence.
- [ ] **Step 62**: Implement ACI Dynamic Warning Engine: ML-based warning generation from bug patterns and swarm session history.
- [ ] **Step 63**: Implement ACI Intelligent Debugging Engine: causal root-cause analysis from runtime traces.
- [ ] **Step 64**: Implement ACI Performance Advisor Engine: suggestions from MLIR cost models + profiling data.
- [ ] **Step 65**: Implement ACI Swarm Coordination Intelligence: conflict prediction, decomposition learning from swarm history.
- [ ] **Step 66**: Expose all ACI services via RAP endpoints: `aci.warnings`, `aci.debug`, `aci.perf`, `aci.swarm`.
- [ ] **Step 67**: Phase 2 integration testing: full agent swarm performing coordinated compilation with ACI assistance.

---

## Phase 3: Language Evolution + Synthesis + Grammar Extensions (Steps 68–88)

- [ ] **Step 68**: Implement the effect type system in `redox_hir_analysis`: effect declarations, effect inference, effect polymorphism, effect handling.
- [ ] **Step 69**: Implement contract syntax and checking in `redox_contracts`: preconditions, postconditions, invariants with compile-time and runtime verification modes.
- [ ] **Step 70**: Implement refinement types in the type checker: `{x: i32 | x > 0}` style value constraints with SMT solver integration for static verification.
- [ ] **Step 71**: Implement capability blocks in HIR lowering: scoped capability grants that limit what code within the block can do.
- [ ] **Step 72**: Implement compact performance annotations: `@pi!` (inline), `@pnb` (no bounds check), `@pv(N)` (vectorize with width N), `@pt(target)` (target placement).
- [ ] **Step 73**: Implement `#[repr(target_optimal)]`: per-target struct layout optimization using MLIR cost model.
- [ ] **Step 74**: Implement formal specification syntax: `spec` blocks with `@req`/`@ens`/`@perf`/`@fx` clauses, machine-verifiable.
- [ ] **Step 75**: Build the synthesis oracle: spec → candidate implementation generation using constraint solving and template synthesis.
- [ ] **Step 76**: Build the verification oracle: candidate → spec satisfaction proof using symbolic execution and SMT solving.
- [ ] **Step 77**: Implement pipeline composition from specs: `pipeline` blocks that chain function contracts, with compile-time verification of contract compatibility.
- [ ] **Step 78**: Implement the self-evolving grammar extension system: `grammar_extension!` macro, registration API, extension discovery.
- [ ] **Step 79**: Implement frequency-driven abbreviation promotion in ACI: analyze token frequency across corpus, suggest new abbreviations.
- [ ] **Step 80**: Implement the Agent Memory Model: ephemeral (per-task), session (per-swarm-session), project (conventions), global (cross-project) memory tiers.
- [ ] **Step 81**: Build the memory recall API: `memory.store`, `memory.recall`, `memory.suggest` endpoints in RAP.
- [ ] **Step 82**: Implement the agentic standard library: `SwarmVec`, `ArenaVec`, `SwarmChannel`, streaming I/O primitives in `library/`.
- [ ] **Step 83**: Conduct corpus-wide token frequency analysis on crates.io ecosystem for abbreviation optimization.
- [ ] **Step 84**: Finalize standard abbreviation registry v2 with full ecosystem coverage, frequency-weighted.
- [ ] **Step 85**: Define `redox-2026` edition with all new features including token-compact canonical form.
- [ ] **Step 86**: Build verification certificate emission pipeline: opt-in for safety-critical code, emits machine-checkable proofs.
- [ ] **Step 87**: Implement swarm-of-swarms hierarchical orchestration for million-LOC+ codebases.
- [ ] **Step 88**: Implement MLIR→CIRCT pipeline for FPGA targets and MLIR StableHLO/TOSA dialect pipelines for NPU/TPU targets.

---

## Phase 4: Ecosystem (Steps 89–105)

- [ ] **Step 89**: Build capability-indexed package registry.
- [ ] **Step 90**: Migrate core ecosystem crates with capability manifests.
- [ ] **Step 91**: Build agent swarm marketplace and pre-composed swarm templates.
- [ ] **Step 92**: Develop certification pipeline for safety-critical industries (opt-in full safety mode).
- [ ] **Step 93**: Publish Redox language specification.
- [ ] **Step 94**: Ship reference swarm configurations (audit swarm, migration swarm, greenfield swarm).
- [ ] **Step 95**: Build swarm performance benchmarking suite (throughput, latency, conflict rate metrics).
- [ ] **Step 96**: Publish SKB rule corpus as open dataset for agent training.
- [ ] **Step 97**: Launch global memory network: anonymized cross-project pattern sharing (opt-in).
- [ ] **Step 98**: Build synthesis marketplace: verified spec→implementation pairs as reusable components.
- [ ] **Step 99**: Publish cost model calibration suite (standardized benchmarks for cost oracle accuracy).
- [ ] **Step 100**: Implement hot-reload runtime: function-level live patching with rollback support.
- [ ] **Step 101**: Implement zero-friction FFI: auto-binding for C/C++/Python/WASM/CUDA headers.
- [ ] **Step 102**: Implement capability-based sandbox runtime for agent-generated code execution.
- [ ] **Step 103**: Build agentic benchmarking suite: token throughput, parse error rate, synthesis success rate, swarm latency.
- [ ] **Step 104**: Implement swarm orchestration pattern library: map-reduce, pipeline, scatter-gather, saga.
- [ ] **Step 105**: Implement self-healing compiler: auto-repair pipeline with confidence ranking.

---

## Notes

- Steps within each phase may overlap or be parallelized where dependencies allow.
- The prototype in `prototype/` serves as reference implementation and test oracle for all production steps.
- Each step should include tests that verify the feature works end-to-end.
- The compiler must remain self-hosting throughout: every step must preserve the ability to bootstrap.
