# MAGE Implementation Roadmap

> Tracking progress from prototype toward production. Steps 1–22 completed prior.
> Each step is a concrete, testable increment.
>
> **Last verified: 2026-08-04** — all five crates built and tested: prototype
> **1,038**, rmi **1,380**, ribosome **164**, germline **112**, forge **52** —
> **2,746 tests, 0 failures, 0 warnings**. The crate count went from three to
> five when the build engine (step 148) and the RSI control plane (step 149)
> were extracted from `forge`; the total is unchanged by those moves, and
> `forge`'s 52 is what the registry alone measured before they were parked in
> it. Forge's old "235" was stale as well as merged: accurate immediately before
> step 144, never updated through steps 144–146, which added 36 tests. See
> `MEASUREMENTS.md` for the per-commit counts.
>
> *Correction:* earlier runs reported prototype at 1,209. The library split
> (step 142) showed 171 of those were the **same test functions compiled into
> three binaries** and executed three times — `lexer`, `parser`, `ast`, `hir`,
> `heal`, and `recover` were re-included by `#[path]` into `reliability-bench`
> and `token-bench`, which had no tests of their own. 1,038 is the number of
> distinct tests, and always was. Steps 1–78 below are the *language and
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

> `ribosome/` (its own crate since step 148; built inside `forge` up to step
> 147), design in [RIBOSOME.md](RIBOSOME.md). The evaluation
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

> `germline/` (its own crate since step 149; built inside `forge` up to step
> 148), design in [GERMLINE.md](GERMLINE.md). The operating mode
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
| 141 | Bounded unattended runner | ✅ | `Runner` drives cycles under a **pinned policy**, bounded by an explicit budget — no daemon mode and no way to request unlimited cycles. Halts on demotion (a gate that approved something production rejected needs revising, not another candidate), on a stalled search, and on workload failure. A `Workload` trait is the seam to real training |
| 142 | Library split | ✅ | The reference surface is now `pub` API of a `mage_prototype` library with thin binaries over it, so visibility carries the meaning. The crate-wide `allow(dead_code)` is gone: ~85 warnings became **3 real findings**, and the split revealed 171 duplicated test executions |
| 143 | A real workload | ✅ | `germline::workload::BuildWorkload` — architecture search evaluated by **actual Ribosome builds**, not a stub. A genome decodes to a network architecture, `materialize` builds it, fitness comes from the real `BuildReport` plus artifact properties. Axes deliberately pull against each other so the search cannot win by growing without limit. Neural-net training is a different implementation of the same trait; nothing in the control plane changes for it |
| 145 | Subprocess sandbox | ✅ | Fresh workdir, cleared environment, executable allowlist, traversal rejection, wall-clock timeout. Containment rather than isolation, and documented as such — it removes accidental non-hermeticity, not a hostile tool |
| 146 | Asymmetric provenance | ✅ | Per-worker Ed25519 with a `TrustStore`: a compromise becomes attributable, containable and **revocable**, which a shared secret cannot be. Verifiers hold only public keys |
| 154 | Clippy clean across the owned crates | ✅ | `prototype` went 194 → **0** clippy warnings, `ribosome`/`germline`/`forge` were brought to 0 earlier. 163 were machine-applicable; the rest were hand-reviewed, and three were real defects rather than style: a `--target=abl-train` doc block stranded on `const ABL_MAGIC` (documenting a training command as a magic-bytes constant, because the command is handled inline and has no function to attach to); `&label[5..]` after a `starts_with`, the same byte-indexing shape as the `hex_decode` panic; and three branches in `heal.rs` asserting a `DiagnosticCategory` distinction the enum cannot express. `ItemKind::Train` is now boxed — one variant made *every* `ItemKind` cost 2.3 KB against 392 B for the next largest. Six signatures keep documented per-item `#[allow]`s rather than a blanket one. **`rmi` keeps its 2**: it is vendored and must stay syncable against its own upstream, so cosmetic edits there buy nothing and cost merge conflicts |
| 156 | Dependency alerts cleared | ✅ | **2026-08-05: 16 open Dependabot alerts → 0.** All 16 were npm, all in `video/`, the standalone Remotion promo-video project; **no Rust crate had any**, which is worth stating because "16 vulnerabilities on the default branch" reads as a compiler problem and was not one. `remotion`/`@remotion/cli` 4.0.293 → 4.0.506 — patch releases within 4.0.x — cleared every one including the two criticals, and the transitive `fast-uri`/`postcss`/`ws`/`webpack` advisories with them. Done as an explicit pinned install rather than `npm audit fix --force`, which wanted to disable protections to reach the same version. Verified functionally, not just by a clean `npm audit`: `tsc --noEmit` passes and Remotion still resolves the `AgenticRain` composition. The video was **not** re-rendered |
| 155 | All seven builtins verified against real toolchains | ✅ | `c` (clang 22.1.8), `cpp` (clang++), `python` (CPython 3.12.11) now join `rust`, `go`, `typescript` and `mage`: every builtin builds a real program end to end through the CLI, artifacts execute, and incremental rebuild is minimal (editing `helper.c` recompiles it and relinks, leaving `main.o` cached). **Three of the seven were broken** — `rust`, `python`, `typescript` — each with a template that looked right and came from the tool's documented interface. I had recorded these as blocked on missing tooling **three separate times**; every time it was an incomplete search, not a missing tool — LLVM was in `C:\Program Files\LLVM`, Go and Node were installed, CPython was in `~/.local/bin`. The C build also demonstrated hermeticity working by failing first: the sandbox strips the env clang uses to find the Windows SDK, so it failed with `'stdio.h' file not found` until those variables were declared in the manifest — and therefore keyed |
| 153 | Builtin languages verified against real toolchains | ✅ | I had recorded `go` and `typescript` as unverifiable "needs tooling this machine lacks" — but I had only ever searched for C compilers, never for `go` or `node`. Both were installed the whole time. `go` 1.26.5 builds a real multi-file program through the CLI, the artifact runs, and editing a non-root source rebuilds it. `typescript` 5.9.3 exposed a **third broken builtin**: `--outFile` is rejected outright for any source with an import (`TS6131`), so it could not compile a module; now `--outDir . --rootDir .`, with `--rootDir` load-bearing because without it `tsc` flattens `src/a.ts` to `a.js` and the declared output never appears. Three of the five builtins that have now met a compiler were wrong. `c`/`cpp` (no C compiler) and `python` (Store stub only) remain genuinely unrun |
| 152 | Documented counts are checked, not asserted | ✅ | Four documented test figures were found stale on 2026-08-04, each by accident. The failure is structural, not careless: a summary line is written once while its subject is fresh and nothing forces a second look, so docs asserting measured facts decay silently — which is exactly what `DOCS.md`'s "the measurement wins" rule forbids while giving no way to notice a violation. `scripts/check-doc-counts.sh` re-derives all 37 documented counts from the run that just happened and fails on any mismatch; `--check-docs` on both test-all scripts feeds it, and a CI job runs it. A claim reworded past its pattern is a **failure, not a skip**. Verified in both directions — wrong numbers are caught in all 8 places a count appears, and a reworded claim fails loudly |
| 151 | Transport seam for encryption | ✅ | The blocker for TLS was not cryptography, it was that a length-prefixed-JSON protocol had been *written* against `TcpStream` concretely. Both ends are now generic over `Read + Write`, with one wrapper point each — `WorkerServer::serve_with` and `RemoteExecutor::connect_over`. Proven end to end with a byte-transforming wrapper **and its negative case**, since a wrapper that did nothing would pass the positive test alone. Dividend: the protocol now runs over a scripted in-memory stream, so "auth gates every frame including `Describe`" is deterministic rather than socket-and-sleep. TLS itself stays ◻ — not for want of plumbing, but because the trust posture (pinned self-signed / mutual TLS / public PKI) is the operator's decision and picking one silently would make a security choice on their behalf |
| 150 | A command line, and what it found | ✅ | `ribosome plan` / `build` / `languages`, over a JSON manifest that is deliberately data — no conditionals, includes, or globs, since a manifest that can compute is a second door for the ambient state the action key exists to shut. Hand-rolled arg parsing to keep the dependency list a specification. **Running it against real `rustc` found three defects the tests could not**: the `rust` builtin passed every source to a compiler that accepts one input filename; the `python` builtin declared an output `-m py_compile` never writes; and `cache_hit_ratio` reported **1.0 on a build that failed everything**, which paid a candidate maximum `Fitness::reuse` for breaking the build. All three fixed, the last with a regression test. Verified end to end: cold → cache hit → edit a non-root source → rebuild, plus the shared-store refusal under a real toolchain |
| 149 | Germline as its own crate | ✅ | Extracted from `forge` to `germline/`, a fifth workspace, for the same reason as step 148: a succession control plane is not something anyone would look for inside a package registry, and nobody could adopt it without adopting one. Depends on `ribosome` and nothing else in the repository, and that direction is enforced — the Weismann barrier is one-way, so a build engine able to call into succession would be a somatic path into the germline. The CI `cargo tree` guard now covers that edge too. `forge` drops both subsystems and returns to 52 tests, exactly what the registry measured before they were parked in it |
| 148 | Ribosome as its own crate | ✅ | Extracted from `forge` to `ribosome/`, a fourth workspace. `forge` is a package registry; a build system living inside one can only ever be that registry's build system, and step 147's claim that no language is privileged is not credible from a crate that depends on one language's compiler. Dependencies are now `serde`, `serde_json`, `sha2`, `ed25519-dalek` — CI enforces the absence of any MAGE crate with `cargo tree` rather than trusting the doc. `mac` moved with it; `forge` re-exports the crate and `germline` drives it through the public API, so that API now has a real consumer rather than only its own tests |
| 147 | Arbitrary languages | ✅ | `ribosome::lang` — a language is data: extensions, a toolchain, a declared granularity (`PerSource` + link for C, `WholeTarget` for Rust/Go, because pretending otherwise produces a graph that lies about both parallelism and rebuilds), and argument templates. The real content is the honesty layer: `Hermeticity::{Structural, Pinned, Declared}`, the toolchain digest inside the action key, and a shared store that **refuses to publish** unverified claims while still building and caching them locally. Cross-language edges are derived, not declared. Not a package manager |
| 144 | Transport authentication | ✅ | HMAC challenge-response before any frame is served — a worker that answers `Describe` to an unauthenticated peer has already disclosed its capabilities. Server-generated per-connection nonce, so a captured proof cannot be replayed. **Authenticates but does not encrypt**; TLS remains ◻ |

---

## Open / not done

> The honest remainder as of **2026-08-03**. Nothing here is a regression; these
> are known gaps, not surprises.

| # | Item | Status | Notes |
| - | ---- | ------ | ----- |
| 1 | CUDA runtime correctness needs a GPU runner | 🔧 | **Correctness itself is verified**, 2026-08-05: `cargo test --features cuda` → **1,071 passing, 0 failed** on dual RTX 3090 Ti, driver 610.88, against pinned IronAccelerator `v2.2.0`. That retired the documented **1,269**, which was written *before* the library split collapsed 171 duplicate executions; `1,269 − 171 = 1,098` still leaves **27 unexplained**, so the figure was replaced by the measurement rather than reverse-engineered into agreement. `scripts/*.sh --cuda --check-docs` now verifies the CUDA number in all 6 places it is claimed, so it cannot drift again. **Remaining is genuinely infrastructure:** the `cuda-gpu` job is gated on `vars.HAS_GPU_RUNNER == 'true'` and needs a self-hosted runner registered — an *unattended CI* check, which a developer box running the suite by hand is not. |
| 2 | IronAccelerator version drift | ✅ | **Fixed 2026-08-03.** Was a path dep on a sibling checkout, so the lock silently re-resolved whenever that checkout moved (it drifted 1.2.0 → 2.2.0 exactly this way) and `--features cuda` could not build from a clean clone. Now pinned to the published tag `v2.2.0` (rev `46ceb09d`); `prototype/Cargo.toml` carries a commented `[patch]` block for local IronAccelerator development. |
| 3 | Steps 2c & 3 of the ab-initio migration | ⬚ | Declined as negative-sum (step 99). Revisit only with new measurement. |
| 4 | Single-workspace build | ⬚ | The four crates are separate workspaces by design: rmi is vendored and must stay independently buildable, and `ribosome` must be able to leave without taking MAGE with it. `scripts/test-all.{ps1,sh}` is the supported way to build/test everything at once — see the note in `ARCHITECTURE.md`. |
| 5 | `video/out/agentic-rain.mp4` in git | ✅ | **Closed 2026-08-04.** Purged with `git-filter-repo`, then force-pushed on explicit human authorization. Evidence gathered before rewriting: **0 forks, 0 stars, 0 watchers**, 4 clone uniques/14d (CI checkouts); mirror backup at `../MachineGenetics.backup-20260803-194230.git`. Pre-flight check: `git diff origin/master master` showed the *only* content difference was the video itself, so the rewrite changed history and nothing else. Verified by **fresh clone of the public repo**, not by local state — the mistake made on 2026-08-03, when a `git fetch` re-imported the old history and briefly made a corrected claim look wrong: `.git` **158 → 121 MB**, blob absent from every remote ref, `v0.2.0` intact at `84731af`, 327 commits preserved. Local `.git` is 112 MB after `reflog expire --expire-unreachable=now` + `gc --prune=now`; plain `gc` alone left it at 156 MB because reflog entries still held the blob reachable. **Known cost:** the 38 MB render is no longer obtainable from git history. The file survives untracked on disk and can be attached to the release with `gh release upload v0.2.0 video/out/agentic-rain.mp4` if it should stay available. |
| 8 | The RSI loop | ✅ | **Closed 2026-08-03.** `Runner` drives bounded, policy-pinned cycles (step 141). What remains is the *workload* — training/inference behind a genome — which is step 143, not a gap in the loop. |
| 7 | Ribosome distribution | ✅ | **Built 2026-08-03.** Real TCP transport (`remote.rs`), `RemoteExecutor` behind the `Executor` seam, worker advertisement, registry with heartbeat/eviction/recovery, and signed provenance (`provenance.rs`). Tested against live loopback workers. Remaining and stated in `RIBOSOME.md`: TLS, connection auth, and a sandboxed subprocess executor. |
| 6 | Reference surface has no call sites | ✅ | **Fixed 2026-08-03** by the library split (step 142). `pub` now carries the meaning and the crate-wide suppression is gone. |
| 9 | Ribosome builds only MAGE | ✅ | **Built 2026-08-04** (step 147). `ribosome::lang`: languages, toolchains, and declared granularity, with builtins for MAGE, C, C++, Rust, Go, Python, and TypeScript. The substance is not the planner but the `Hermeticity` tier — `Structural` / `Pinned` / `Declared`. A pinned toolchain's executable digest enters the action key, so two machines' differently-patched `gcc-13.2.0` cannot collide; an unpinned one is marked `+unpinned` and `Store::open_shared` refuses to publish its claims, per action. 32 tests. |
| 10 | External dependency resolution | ⬚ | `lang` deliberately stops at targets in one graph. Fetching third-party code is a distinct trust problem — provenance, pinning, and revocation for code nobody in this repo wrote — and folding it into the planner is how build systems become unauditable. Not started, and not an oversight. |
