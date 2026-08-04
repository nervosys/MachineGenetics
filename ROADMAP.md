# MAGE Implementation Roadmap

> Tracking progress from prototype toward production. Steps 1–22 completed prior.
> Each step is a concrete, testable increment.
>
> **Last verified: 2026-08-03** — all three crates built and tested from a clean
> cache: prototype **1,209** (1038 + 141 + 30), rmi **1,380**, forge **52** —
> **2,641 tests, 0 failures, 0 warnings**. Steps 1–78 below are the *language and
> compiler* phases (through 2026-03). Phases I–N are the 2026-06 work — the ABL
> binary track, the ab-initio redesign, the evaluator, Forge, and the
> architecture DSL — which is what the README and MEASUREMENTS.md describe.
> [Open items](#open--not-done) at the end are the honest remainder.

## Legend

- ✅ Complete
- 🔧 In Progress
- ⬚ Not Started

---

## Phase A: Compiler Foundation (Steps 23–28)

| Step | Title                              | Status | Description                                                                                                                       |
| ---- | ---------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| 23   | Complete the lexer                 | ✅      | Cover all 60+ keyword/attribute/type mappings from §5.5 (ep"...", `@@`, `?=`, `~>`, `af`, `uf`, `Y`, `Z`, `R`, `Ok`, `Err`, etc.) |
| 24   | Complete the parser                | ✅      | Proper LL(1) with all MAGE syntax forms: contracts, specs, effects decl, capability blocks, swarm patterns, perf annotations   |
| 25   | Structured Diagnostic Graph        | ✅      | Replace flat error strings with DiagnosticGraph (§6.2): fix candidates, confidence, causal chains, related errors                 |
| 26   | Safety elision pass                | ✅      | Strip lifetimes, `unsafe`, `&mut`, `move`, `ref`, `Pin`, `PhantomData`, `Send`/`Sync` from AST in agentic mode                    |
| 27   | Dual-syntax transpiler integration | ✅      | `--syntax=legacy` flag: accept Rust syntax via rust2mg, feed canonical form to compiler                                           |
| 28   | Token budget reporting             | ✅      | `--token-report` per-function/module token counts, compact vs expanded metrics                                                    |

## Phase B: Agentic Core Deepening (Steps 29–35)

| Step | Title                        | Status | Description                                                                                                                |
| ---- | ---------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| 29   | Deepen the SKB               | ✅      | Expand from 16 to 200+ rules across 6 databases (ownership, borrow, lifetime, type, concurrency, FFI)                      |
| 30   | Contract syntax & checking   | ✅      | `@req`/`@ens`/`@inv` in parser + AST + verification oracle integration                                                     |
| 31   | Formal specification syntax  | ✅      | `spec` blocks with `@req`/`@ens`/`@perf`/`@fx`, parsed and stored in AST                                                   |
| 32   | Refinement types             | ✅      | Value-level type constraints (`NonZero[u32]`, `Range[0..100]`) in type checker                                             |
| 33   | Capability system            | ✅      | `agent` keyword + `AgentDef` AST, capability declarations, bracket-list parser, verification oracle, known-cap taxonomy    |
| 34   | Deepen self-healing          | ✅      | 17 error patterns (was 6): borrow/move, unused-var, missing-field, contract @req/@ens/@inv, capability-denied, perf-budget |
| 35   | Attribute compression system | ✅      | 24-entry `@shorthand` → Rust attr bidirectional map, `expand_attribute`/`compress_attribute_name`, full roundtrip tests    |

## Phase C: Agent Protocol & Services (Steps 36–41)

| Step | Title                      | Status | Description                                                                                                                                                                               |
| ---- | -------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 36   | Expand RAP server          | ✅      | From 9 to 25+ methods: format/compact, format/expand, lint/check, doc/query, token/report, grammar/extensions, grammar/expand, aci/*, sandbox/*, ffi/*, hotpatch/*, memory/*, synthesis/* |
| 37   | magefmt service           | ✅      | `--agent` (min tokens) and `--human` (human-readable), bidirectional lossless AST conversion                                                                                           |
| 38   | Agent discovery attributes | ✅      | `@as("...")`, `@ac("...")`, `@ax("...")`, `@ao("...")`, `@ae("...")` in lexer/parser/AST                                                                                                  |
| 39   | Grammar extension system   | ✅      | `grammar_extension!` macro, MAGE.toml registration, namespace-scoped discovery, frequency promotion                                                                                    |
| 40   | Capability manifests       | ✅      | JSON manifest generation per crate, capability-indexed search in Forge                                                                                                                    |
| 41   | MLIR dialect definition    | ✅      | First-class MLIR dialect ops: `MAGE.contract.*`, `MAGE.perf`, `MAGE.agent`, `MAGE.spec`, `MAGE.ownership.*`; 7 new tests (313 total)                                       |

## Phase D: Swarm Runtime (Steps 42–48)

| Step | Title                     | Status | Description                                                                                                                       |
| ---- | ------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| 42   | Semantic lease manager    | ✅      | SharedRead/ExclusiveWrite/Restructuring leases, region overlap, timeout expiry, wait-for deadlock detection; 24 tests (337 total) |
| 43   | CRDT merge engine         | ✅      | Semantic CRDTs: InsertItem/RemoveItem/ModifyBody/ModifySignature/AddImpl/Rename, Lamport LWW, batch merge; 17 tests (354 total)   |
| 44   | Consensus protocol        | ✅      | 5-phase Propose→ImpactAnalysis→Vote→Resolve→Integrate, quorum majority, phase enforcement; 13 tests (367 total)                   |
| 45   | Task decomposition engine | ✅      | Task DAG, topological sort, parallel waves, critical path, capability-based agent assignment; 15 tests (382 total)                |
| 46   | Swarm message bus         | ✅      | Typed SwarmMessage, per-agent mailboxes, topic pub/sub, priority, depth limits, bus stats; 18 tests (400 total)                   |
| 47   | Swarm SDK                 | ✅      | `mage_swarm` crate: derive macros, role taxonomy, SwarmAgent trait, example orchestrator                                       |
| 48   | Semantic VCS              | ✅      | Operation-log-based version control, semantic branching/merging, intent-based history queries                                     |

## Phase E: Advanced Subsystems (Steps 49–55)

| Step | Title                     | Status | Description                                                                                                                 |
| ---- | ------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| 49   | Synthesis oracle          | ✅      | Spec→candidate generation, candidate ranking by cost, verification of candidates against specs                              |
| 50   | ACI subsystem             | ✅      | Dynamic Warning Engine, Intelligent Debugging Engine, Performance Advisor, Swarm Coordination Intelligence, 8 RAP endpoints |
| 51   | Verification certificates | ✅      | Machine-checkable proofs: memory safety, data-race freedom, contract satisfaction, effect containment                       |
| 52   | FFI binding generator     | ✅      | Auto-bind from C headers (parse .h), Python stubs (.pyi), WASM (.wit); safe wrapper generation                              |
| 53   | Hot-reload runtime        | ✅      | Function-level live patching, MLIR single-function re-lowering stubs, rollback management                                   |
| 54   | Capability-based sandbox  | ✅      | Per-agent isolation, resource limits (mem/CPU/syscalls), capability attenuation, audit logging                              |
| 55   | Performance annotations   | ✅      | `@pi!`, `@pnb`, `@pv(N)`, `@pt(target)`, `@pa(N)`, `@pp`, `#[repr(target_optimal)]` processing                              |

## Phase F: Stdlib & Ecosystem (Steps 56–60)

| Step | Title                        | Status | Description                                                                                |
| ---- | ---------------------------- | ------ | ------------------------------------------------------------------------------------------ |
| 56   | Deepen stdlib                | ✅      | Batch APIs, streaming I/O, SwarmVec, ArenaVec, SwarmChannel, per-agent arena allocators    |
| 57   | Deepen Forge registry        | ✅      | Capability-indexed search, semantic search by capability query, contract-based composition |
| 58   | Agentic benchmarking suite   | ✅      | Token throughput, parse error rate, synthesis success rate, swarm latency metrics          |
| 59   | Cost model calibration       | ✅      | Standardized benchmarks for cost oracle accuracy across targets                            |
| 60   | Language specification draft | ✅      | Formal MAGE language specification document                                             |

## Phase G: Documentation & Training (Steps 61–63)

| Step | Title                | Status | Description                                                                         |
| ---- | -------------------- | ------ | ----------------------------------------------------------------------------------- |
| 61   | Update documentation | ✅      | Book, cookbook, agent-guide, internals for all new features                         |
| 62   | Update training data | ✅      | JSONL samples for contracts, specs, swarm patterns, ACI, synthesis, FFI             |
| 63   | Example projects     | ✅      | End-to-end examples: swarm audit, capability-sandboxed agent, spec-driven synthesis |

---

## Prior Steps (1–22): ✅ All Complete

| Step | Title                                                                                                                   |
| ---- | ----------------------------------------------------------------------------------------------------------------------- |
| 1    | Prototype compiler (lexer, parser, AST, HIR, types, effects, MLIR, resolver)                                            |
| 2    | rust2mg transpiler                                                                                                      |
| 3    | VS Code extension                                                                                                       |
| 4    | Safety Knowledge Base (SKB)                                                                                             |
| 5    | Benchmarks                                                                                                              |
| 6    | End-to-end demo                                                                                                         |
| 7    | mg CLI                                                                                                                  |
| 8    | Standard library stubs                                                                                                  |
| 9    | MAGE Book                                                                                                            |
| 10   | Cookbook                                                                                                                |
| 11   | Agent Guide                                                                                                             |
| 12   | Migration Guide                                                                                                         |
| 13   | Internals Guide                                                                                                         |
| 14   | Quick Start Guide                                                                                                       |
| 15   | mg2rs back-transpiler                                                                                                   |
| 16   | Example projects                                                                                                        |
| 17   | CI/CD pipeline                                                                                                          |
| 18   | Editor configs                                                                                                          |
| 19   | Agent training data corpus                                                                                              |
| 20   | Community infrastructure                                                                                                |
| 21   | Forge package registry                                                                                                  |
| 22   | Agentic AI integration (self-healing, cost oracle, SKB query engine, verification oracle, agent memory, swarm patterns) |

---

## Phase H: AI-Native Language Primitives (Steps 64–78)

> Implement the MAGE_SPEC.md AI constructs in the prototype compiler.
> Strategy: prototype first (nimble, testable), then migrate to compiler crates.

| Step | Title                           | Status | Description                                                                                                                                                                                                                                                |
| ---- | ------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 64   | AI keywords in lexer            | ✅      | Add 22 AI keywords to prototype lexer: `net`, `layer`, `tensor`, `param`, `train`, `grad`, `kb`, `fact`, `rule`, `query`, `evolve`, `genome`, `mutate`, `fitness`, `select`, `crossover`, `rl`, `policy`, `reward`, `forward`, `population`, `generations` |
| 65   | Agent-mode Greek symbols       | ✅      | Lex AI-specific Greek/math symbols: `Ψ`→net, `λ`→layer, `Φ`→tensor, `Π`→param, `Θ`→train, `∇`→grad, `α`→agent, `κ`→kb, `ρ`→rule, `Ω`→evolve, `Γ`→genome, `φ`→fitness, `Ξ`→policy, `μ`→mutate, `χ`→crossover                                                |
| 66   | AST: AI item kinds & types      | ✅      | `ItemKind::Net`, `::Kb`, `::Evolve`, `::Train` with child structures. `Type::Tensor`, `::Param`, `::Genome`, `::Policy`, `::KnowledgeBase`, `::LLM`                                                                                                        |
| 67   | Parser: `net` blocks            | ✅      | Parse `net Name { layer dense(...); layer dropout(...); fn forward(...) }` per §5.1 BNF, produce `NetDef` AST                                                                                                                                              |
| 68   | Parser: `kb` blocks             | ✅      | Parse `kb Name { fact ...; rule ... :- ...; query ... :- ...; }` per §7.1 BNF, produce `KbDef` AST                                                                                                                                                         |
| 69   | Parser: `evolve` blocks         | ✅      | Parse `evolve Name { genome: T, population: N, generations: N, fitness fn, select/crossover/mutate strategies }` per §8.1 BNF                                                                                                                              |
| 70   | Parser: `train` blocks          | ✅      | Parse `train Name { model, data, optimizer, loss, epochs, batch_size, on_epoch hook }` per §5.4 BNF                                                                                                                                                        |
| 71   | Tensor operator parsing         | ✅      | Parse `⊗` (matmul), `⊙` (Hadamard), `⊤` (transpose), `⊥` (flatten), `▸` (pipeline) with correct operator precedence                                                                                                                                        |
| 72   | HIR: tensor/neural types        | ✅      | `Ty::Tensor(Box<Ty>, Vec<TensorDimHir>)`, `Ty::Param`, `Ty::Genome`, `Ty::Policy`, `Ty::KnowledgeBase`, `Ty::LlmType`                                                                                                                                      |
| 73   | Type checker: shape unification | ✅      | Shape dimension unification: `Lit(n)=Lit(n)`, `Var(v)` unifies with anything; rank mismatch diagnostics                                                                                                                                                    |
| 74   | Type checker: grad typing       | ✅      | Built-in `grad(expr)` type rule: argument must be `Tensor`/`Param` type, returns same type; differentiability check                                                                                                                                        |
| 75   | Effect system: AI effects       | ✅      | Register `Gpu`, `Npu`, `Llm`, `Evolve`, `Learn`, `Rng` as built-in effects; `Effect::from_name()` recognition                                                                                                                                              |
| 76   | Name resolution: AI items       | ✅      | `SymbolKind::Net`, `::Kb`, `::Evolve`, `::Train`; scoped resolution for layers in nets, rules in KBs                                                                                                                                                       |
| 77   | MLIR lowering: AI ops           | ✅      | `MAGE.tensor.*`, `MAGE.neural.*`, `MAGE.evolve.*`, `MAGE.kb.*` dialect operations                                                                                                                                                              |
| 78   | Agent-mode AI operators        | ✅      | Tensor op symbols `⊗`→matmul, `⊙`→hadamard, `⊤`→transpose, `⊥`→flatten, `▸`→pipeline; Pratt parser binding powers; type inference rules                                                                                                                    |

---

# Part II — the 2026-06 work

> Phases A–H built *a language*. Phases I–N are the consequence of a measured
> result that changed the plan: **source-token efficiency is a floor, not a win**
> (MAGE source is ≈ Rust, `MEASUREMENTS.md` §2). The leverage is elsewhere — in a
> binary artifact an agent constructs through a tool, in constructs that subsume
> boilerplate, and in reject-by-construction. Every phase below is anchored to
> commits and to a measurement.

## Phase I: ABL — the tool-mediated construction track (Steps 79–88)

> *The pivot.* An agent stops emitting source and instead calls a tool that
> **constructs** the artifact, so invalid states are unrepresentable rather than
> diagnosed. Named "Machine Language" on 06-09, renamed **Agentic Binary
> Language (ABL)** the same day.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 79 | Tool-mediated construction paradigm | ✅ | `--build=abl spec.json out.abl` — agent emits a *spec*, the compiler constructs the artifact; ARCHITECTURE.md documents the paradigm |
| 80 | Self-describing schema | ✅ | `--build=schema` emits the full JSON contract: `ops` catalogue, `spec_format` per kind, and the complete error taxonomy with `fix` strings — an agent needs no docs |
| 81 | Reject-by-construction errors | ✅ | Machine-readable `{code, message, fix}`: **B0000–B0006** (net), **K0001–K0007** (kb), **A0001–A0003** (agent), **S0001–S0006** (swarm), **U0001–U0003** (unified) |
| 82 | kb construction + execution | ✅ | Horn-clause knowledge bases build to ABL and **execute** as forward-chaining Datalog; auto-fix repair (`--build=abl --fix`) |
| 83 | Container v2 — serialized symbol table | ✅ | kb artifacts fully self-describing; predicate/term/param names round-trip |
| 84 | Unified multi-item containers | ✅ | One ABL container holds net + kb (+ agent + swarm) — a whole neurosymbolic application in a single artifact |
| 85 | agent + swarm construction | ✅ | Capability/approval policy and swarm topology/consensus/transport build, describe, and execute |
| 86 | Round-trip fidelity | ✅ | Every item kind decompiles back to exact source; content hashes match across build→describe |
| 87 | No-exec introspection | ✅ | `--describe=abl` reports structure as pure bounds-checked data — **12.6 µs** for an 858 B artifact, `exec:false` |
| 88 | SPINE collaboration bridge | ✅ | ABL agents communicate over SPINE (Hyperlight); gap analysis + `spine-mage` bridge, 5 tests |

## Phase J: Measurement infrastructure (Steps 89–92)

> *"Ensure that all measurements are quantitative rather than qualitative, with
> zero assumptions."* This phase exists because several earlier claims did not
> survive measurement.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 89 | `MEASUREMENTS.md` + perf harness | ✅ | `perf_measure.rs`; every figure reproducible via `cargo test --release perf_report -- --ignored` |
| 90 | Cross-language executability harness | ✅ | `benchmarks/cross_lang` — compile+run 5 tasks across languages with a **real BPE** tokenizer, repo-relative paths (no local-path leakage) |
| 91 | Datalog evaluator optimization | ✅ | Naive → **indexed semi-naive** (interning + `(pred,arg0)` index + delta): O(N²)→O(N) join, O(N³)→O(output) fixpoint, up to **~1430×** |
| 92 | Token + reliability benches | ✅ | `token-bench` (100-task corpus vs Rust), `reliability-bench` (lex 100/100, parse 99/100, effective 100/100 with recovery) |

## Phase K: Ab-initio language redesign (Steps 93–99)

> `AB_INITIO_DESIGN.md` — measured with real cl100k/o200k tokenizers, ceremony is
> ~half the tokens of a program and is designable away. Each step below was
> migrated by TDD; **two were evaluated and declined as negative-sum**, which is
> recorded rather than hidden.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 93 | Optional `;` | ✅ | Newline-terminated statements |
| 94 | Brace-optional layout blocks | ✅ | Offside rule |
| 95 | Return-type inference | ✅ | Drop `-> T` from value-returning functions |
| 96 | Parameter-type inference | ✅ | Drop param annotations |
| 97 | Eliminate `let` | ✅ | `let` statements removed from the language as superfluous |
| 98 | Effect inference at trust boundaries | ✅ | Sound inference (§3e) |
| 99 | Migration steps 2c & 3 | ⬚ | **Declined — measured negative-sum.** Evaluated, found to cost more than they save, and deliberately not implemented |

## Phase L: The vocabulary frontier + evaluator (Steps 100–106)

> A standard vocabulary where each name is a single BPE token, plus the runtime
> that makes MAGE genuinely *executable* rather than only checkable. Step 103 is
> ~30 commits of evaluator completion on 06-11.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 100 | Register the standard vocabulary | ✅ | `map`/`filter`/`fold`/`sum`/`freq`/`scan` + string/text ops — measured 60–65 % reduction vs explicit loops |
| 101 | Precise total typing for the vocabulary | ✅ | Every vocabulary entry totally typed |
| 102 | Publish vocabulary in the self-ontology | ✅ | Drift-proof — the ontology is generated from the implementation |
| 103 | Complete the evaluator (`eval.rs`, `--eval`) | ✅ | Full expression/statement coverage: match, structs (`@Name{…}`), tuple/slice/struct patterns, f-strings, `?`/Option, compound assignment, bitwise/shift, slice indexing, guard/defer, nested + mutually-recursive functions, `.await` |
| 104 | `eval_bench` correctness suite | ✅ | **73/73** programs compute exact expected results |
| 105 | Digital rain representation | ✅ | Matrix-inspired dense-UTF-8 form (`rain.rs`) + the Remotion render in `video/` |
| 106 | Cross-language executability result | ✅ | MAGE executes **5/5** tasks and is the tersest runnable language (173 cl100k tokens vs Rust 275, Java 297) |

## Phase M: Forge — the agentic-first toolchain (Steps 107–111)

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 107 | Project toolchain | ✅ | `forge new/check/build/run/info` |
| 108 | Agentic-first surface | ✅ | `manifest`/`describe` + `--json` on every command — machine-readable by default |
| 109 | `forge fmt` | ✅ | Completes the toolchain lifecycle |
| 110 | Content-addressed block registry | ✅ | `forge publish` + cross-project resolve; block bodies live **off-context**, referenced by name |
| 111 | Examples run via forge | ✅ | `hello-world`, `data-structures` check + run through the toolchain |

## Phase N: The architecture DSL — a composition algebra (Steps 112–118)

> `ARCHITECTURE_DSL.md`. The measured premise: "higher level → fewer tokens" holds
> **only where the construct subsumes boilerplate**. So build operators that
> subsume *depth*, and let a shared block library carry the rest off-context.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 112 | `stack N { … }` repeat combinator | ✅ | O(1) surface cost in depth |
| 113 | Named `block` macros | ✅ | The leaf-library tier, registry-ready |
| 114 | `residual` / `branch` / `wrap` | ✅ | Dataflow composition operators; blocks can hold combinators |
| 115 | REPEAT-folded binary | ✅ | Stacked nets are O(1) in depth **in bytes too** |
| 116 | CPU execution of RES_ADD / PAR | ✅ | The operators lower to RMIL primitives and actually run |
| 117 | Typed-composition gate | ✅ | `--check` rejects shape-mismatched net compositions |
| 118 | Capstone benchmark | ✅ | A real residual GPT (Embedding → batched 3-D attention → …) runs end to end |

## Phase O: Release (Steps 119–122)

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 119 | Rebrand + vendor rmi | ✅ | REDOX → MechGen → **MAGE (Machine Genetics)**; `RecursiveMachineIntelligence` vendored into the monorepo |
| 120 | Security audit | ✅ | CVE/RustSec, NIST FIPS 140-3, MITRE ATT&CK, CMMC 2.0 — one High CVE fixed (`lz4_flex` ≥ 0.11.6), rest triaged in `SECURITY_AUDIT.md` |
| 121 | Dead-subsystem removal | ✅ | Forked-rustc compiler, REDOX-named tools and CI removed |
| 122 | v0.2.0 tag + announcement | ✅ | prototype + forge at 0.2.0, tagged, blog post + X thread |

## Phase P: Ribosome — the distributed build engine (Steps 123–130)

> `forge/src/ribosome/`, design in [RIBOSOME.md](RIBOSOME.md). The evaluation
> harness for the autonomous-SWE / RSI track: reproducible builds, comparable
> measurement, clean revert. Composes existing subsystems rather than duplicating
> them — the DAG mirrors `decompose.rs`, the accelerator catalogue is
> `backends.rs`, coordination is meant to be driven by `lease.rs`/`consensus.rs`.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 123 | Action graph with derived edges | ✅ | Dependencies computed from declared inputs, not hand-written; duplicate outputs and cycles rejected. Waves + critical path |
| 124 | Deterministic action keys | ✅ | SHA-256 over a canonical **length-prefixed** encoding — no field-boundary collisions; inputs sorted, args not; accelerator partitions the cache only when declared |
| 125 | CAS + action cache | ✅ | Separate immutable blob store and invalidatable claim store; `get` rehashes on every read so rot is caught at the source |
| 126 | Executor seam | ✅ | `Executor` trait (`Send + Sync`, inputs passed not found); `LocalExecutor` (in-process tools), `PoolExecutor` (capability-routed fleet) |
| 127 | Self-healing | ✅ | Failure classified into transient / corruption / missing-capability / deterministic, each with its own remedy; platform fallback is opt-in and **re-keys** |
| 128 | Scheduler + no-exec plan | ✅ | Cache → dispatch → heal → record; failures contained with a named cause; `plan()` predicts a build without running it |
| 129 | Fitness signal | ✅ | Four normalized axes + a composite where **correctness is a gate, not a weight** — a weighted sum lets a broken-but-cached build outrank a working one |
| 130 | Network distribution + evolutionary loop | ◻ | **Not built.** Transport, shared cache service, worker registration, sandboxed subprocesses, signed provenance; then population/selection/mutation and self-overwrite. Substrates exist (`rmi` QUIC/TCP/gRPC, `sandbox.rs`, `certs.rs`, `evolve_gen.rs`, `semantic_vcs.rs`) |

## Phase Q: Germline — model succession and fallback (Steps 131–137)

> `forge/src/germline/`, design in [GERMLINE.md](GERMLINE.md). The operating mode
> where a model proposes a higher-fitness successor by directed evolution, hands
> RSI work to it, and falls back on malfunction or decline. This is the **control
> plane** — it decides whether a successor takes over, not how it is produced.

| Step | Title | Status | Description |
| ---- | ----- | ------ | ----------- |
| 131 | Lineage + append-only succession log | ✅ | Generations, champion pointer, promotions/demotions/rollbacks as events. History is never edited — a self-modifying system that can rewrite its own record cannot be debugged after an incident |
| 132 | Promotion gate | ✅ | Five `AND`-ed checks: evaluator independence, held-out evidence, comparable axes, primary improvement above the noise floor, guard ratchet. `AND` because a gate where capability can buy a guard regression prices safety |
| 133 | The Weismann barrier | ✅ | Gate and suite pinned by digest before an episode opens; changing either **voids** it. Removes the cheapest path to a higher score — editing the judge — rather than hoping it is not taken |
| 134 | Drift ratchet | ✅ | Guard axes compared to the lineage **high-water mark**, not the incumbent. Uniform sub-tolerance steps pass a per-step check forever; the ratchet stops them |
| 135 | Directed search + calibration | ✅ | Predict → rank → spend budget on the top-k. Trust is **earned by measurement**, 0 for an untested predictor; miscalibration widens the search back toward undirected, because the answer to a broken world-model is to explore more |
| 136 | Supervisor + fallback | ✅ | Four failure modes (malfunction, fitness decline, metric divergence, stall); fallback verified materialized *before* authority moves; quarantined generations skipped. Deliberately **not a model**, so the loop cannot route around it |
| 137 | Variation — candidate production | ✅ | Deterministic seeded operators mirroring the compiler's `evolve` vocabulary. A candidate's id encodes its seed, so its origin is a checkable claim rather than an unfalsifiable one |
| 138 | Attestation | ✅ | HMAC-SHA256 over a canonical `(verdict, evaluator)` encoding — RFC 4231 vectors, constant-time compare, evaluator name inside the signed material. Symmetric, so adequate in one trust domain and not across a fleet; stated as such |
| 139 | Durable hash-chained journal | ✅ | Append-only JSONL where each record carries its predecessor's digest. A targeted edit or deletion breaks the chain at a reported index; `head()` anchors the whole history in 32 bytes. Tamper-**evident**, not tamper-proof, and documented as such |
| 140 | Cycle state machine + Ribosome seam | ✅ | Proposed → Evaluated → Shadowing → Adjudicated → Promoted/Refused, phases enforced so no path to authority skips the gate. Promotion needs an attested verdict **and** an `Authority`; `fitness_from_build` bridges build measurement into succession fitness |
| 141 | Model workload + daemon | ◻ | **Not built.** Training/inference themselves, and any scheduler that runs cycles without a person. `Authority::Unattended` is the seam it would attach to — deliberately not the default |

---

## Open / not done

> The honest remainder as of **2026-08-03**. Nothing here is a regression; these
> are known gaps, not surprises.

| # | Item | Status | Notes |
| - | ---- | ------ | ----- |
| 1 | CUDA **runtime** correctness not CI-verified | 🔧 | *Compile* coverage landed 2026-08-03: the `cuda` job in CI runs `cargo check --features cuda --all-targets`, which works on a driverless runner because IronAccelerator dispatches via `libloading`. What CI still cannot do is *run* the kernels — GPU correctness (the P101–P139 precision/quantization stack) is verified only on the dev machine: **1,269 tests green** on dual 3090 Ti via `cargo test --features cuda`. Closing this needs a GPU runner. |
| 2 | IronAccelerator version drift | ✅ | **Fixed 2026-08-03.** Was a path dep on a sibling checkout, so the lock silently re-resolved whenever that checkout moved (it drifted 1.2.0 → 2.2.0 exactly this way) and `--features cuda` could not build from a clean clone. Now pinned to the published tag `v2.2.0` (rev `46ceb09d`); `prototype/Cargo.toml` carries a commented `[patch]` block for local IronAccelerator development. |
| 3 | Steps 2c & 3 of the ab-initio migration | ⬚ | Declined as negative-sum (step 99). Revisit only with new measurement. |
| 4 | Single-workspace build | ⬚ | The three crates are separate workspaces by design (rmi is vendored and must stay independently buildable). `scripts/test-all.{ps1,sh}` is the supported way to build/test everything at once — see the note in `ARCHITECTURE.md`. |
| 5 | `video/out/agentic-rain.mp4` in git | 🔧 | A 38 MB binary is tracked, and the `.git` directory is ~158 MB largely because of it plus the 12 MB of banner PNGs. Fine for now; if the repo is ever cloned often, move it to a release asset or LFS. |
| 8 | The RSI loop runs, but not by itself | 🔧 | The full cycle is wired and tested end to end (`forge/tests/rsi_loop.rs`): variation → directed search → Ribosome build → gate → attestation → journal → supervision → fallback. What is missing is the *workload* (training/inference behind a genome) and any daemon that drives cycles without a person. `Authority::Unattended` is the seam; it is deliberately not the default. Step 141. |
| 7 | Ribosome is not yet distributed | 🔧 | The `Executor` seam, capability routing, and hermetic input passing are built and tested, but every executor today is in-process. Until a network transport and shared cache service exist, "distributed" describes the design, not the deployment — step 130. |
| 6 | Reference surface has no call sites | 🔧 | ~85 items exist for the ontology / `--build=schema` / RAP surface but are unused in-tree, so `dead_code` is silenced crate-wide in `prototype` (documented at the top of `main.rs`). The real fix is splitting the reference surface into a library crate where `pub` carries the meaning. |
