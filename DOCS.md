# Documentation index

There are eighteen Markdown documents at the repository root, ~700 KB in total,
written across five months. They are **not** all current, and several describe
designs that were deliberately *not* built. This index says which is which, so
nothing here has to be read to find out whether it is still true.

**The rule:** when a design document and a measurement disagree, the measurement
wins. [`MEASUREMENTS.md`](MEASUREMENTS.md) and [`benchmarks/`](benchmarks/) are
the ground truth for every performance or efficiency claim.

---

## Normative — what actually ships

Read these to understand the system as it exists.

| Document | What it covers |
| --- | --- |
| [README.md](README.md) | Entry point: the four forms, the composition algebra, measured benchmarks |
| [ARCHITECTURE.md](ARCHITECTURE.md) | ABL and tool-mediated construction — the current architecture, including a §6 "Honest boundaries" |
| [MEASUREMENTS.md](MEASUREMENTS.md) | Every measured functionality and performance figure, with reproduction commands |
| [ROADMAP.md](ROADMAP.md) | All 122 implementation steps + the open-items list |
| [ARCHITECTURE_DSL.md](ARCHITECTURE_DSL.md) | The composition algebra (`stack`/`residual`/`branch`/`wrap`) and its measured basis |
| [RIBOSOME.md](RIBOSOME.md) | The distributed, agent-operated build engine — its own crate, `ribosome/`. **Mixed status, marked inline**: the core (graph, keys, CAS, executor seam, healing, scheduler, fitness), network distribution with authentication and signed provenance, sandboxed subprocess execution, and multi-language support are implemented and tested ✅; TLS and the evolutionary loop above the build are designed ◻. *This row said distribution was unbuilt through step 7 and was not updated when it landed — corrected 2026-08-04.* |
| [GERMLINE.md](GERMLINE.md) | Model succession, handoff, and fallback — the RSI control plane, its own crate `germline/`. **Mixed status, marked inline**: the control plane is complete and tested end to end ✅ (variation, directed search, gate, attestation, lineage, hash-chained journal, cycle, supervision); model training/inference and any unattended daemon are not ◻ |
| [AGENT_PROTOCOL.md](AGENT_PROTOCOL.md) | How an agent should target ABL bytes rather than text |
| [UNIFICATION.md](UNIFICATION.md) | MAGE ↔ RMI unification: the bridge, adapters, and the 21-section ontology |
| [SECURITY_AUDIT.md](SECURITY_AUDIT.md) | CVE/RustSec, NIST FIPS 140-3, MITRE ATT&CK, CMMC 2.0 audit |
| [SPINE_COLLABORATION.md](SPINE_COLLABORATION.md) | Multi-agent collaboration over SPINE |
| [MAGE_ONTOLOGY.md](MAGE_ONTOLOGY.md) | The ontology in prose. The machine-readable form is [`MAGE_ONTOLOGY.json`](MAGE_ONTOLOGY.json), generated from the implementation — prefer it |

## Design record — decisions and their evidence

Current thinking, but design rather than description. Useful for *why*.

| Document | Status |
| --- | --- |
| [AB_INITIO_DESIGN.md](AB_INITIO_DESIGN.md) | **Current design direction.** Explicitly supersedes `IDEAL_AGENTIC_LANGUAGE.md`. Phase K of the roadmap implements it — including the two steps measured as negative-sum and declined |
| [IDEAL_AGENTIC_LANGUAGE.md](IDEAL_AGENTIC_LANGUAGE.md) | **Superseded** by `AB_INITIO_DESIGN.md`. Its token "floor" was the floor of the *then-current* design, not the irreducible one. Kept for provenance |
| [MAGE_SPEC.md](MAGE_SPEC.md) | The language specification. Self-labelled "pre-implementation"; that label is now stale — the prototype implements a large part of it, but the spec has **not** been reconciled with the ab-initio changes (optional `;`, layout blocks, no `let`, type inference). Where they differ, the prototype is authoritative |
| [MAGE_ECOSYSTEM.md](MAGE_ECOSYSTEM.md) | Ecosystem design. The registry/toolchain part is real — see `forge/`. The IDE and training-data sections are partly aspirational |
| [AGENTIC_IR_DESIGN.md](AGENTIC_IR_DESIGN.md) | The IR co-design essay. Its realized descendant is the ABL track (`ARCHITECTURE.md`); read that first |
| [MAGE_PROPOSAL.md](MAGE_PROPOSAL.md) | The origin document (2026-03-15, "Status: Proposal", 266 KB). Historical. It predates every measurement and the entire ABL pivot |

## Not built — aspirational strategy

> **These describe a native-code compiler that does not exist.** There is no
> machine-code emitter, no LLVM backend, and no benchmark supporting their
> headline claims. The forked-rustc compiler that would have hosted this pipeline
> was deleted on 2026-06-11 (`b1b910f`, 199 files) as dormant and unused. MLIR
> appears in the prototype only as a *dialect definition and lowering surface*
> (`prototype/src/mlir.rs`), not as a code-generating backend.
>
> They are kept because the reasoning is worth preserving. They are not a
> description of MAGE, and their claims should not be quoted as results.

| Document | Claim | Reality |
| --- | --- | --- |
| [PERFORMANCE_STRATEGY.md](PERFORMANCE_STRATEGY.md) | "Faster Than C, C++, and Rust" | Unimplemented and unmeasured. The measured performance story is the *front end and the ABL hot path* (`MEASUREMENTS.md` §2): ~41 MB/s lex+parse, ~1.4 µs/layer build, 12.6 µs no-exec describe |
| [DIRECT_CODEGEN_STRATEGY.md](DIRECT_CODEGEN_STRATEGY.md) | Direct machine-code emission, bypassing MLIR and LLVM | Never built. MAGE executes via the tree-walking evaluator (`eval.rs`, 73/73 exact) and the ABL CPU/CUDA compute backend |

---

## Where to start

- **Using MAGE as an agent** → `README.md` → `AGENT_PROTOCOL.md` → `mage-parse --build=schema` (the schema is self-describing; you should not need docs at all)
- **Understanding the design** → `ARCHITECTURE.md` → `ARCHITECTURE_DSL.md` → `AB_INITIO_DESIGN.md`
- **Checking a claim** → `MEASUREMENTS.md`, then reproduce it with the command given
- **Contributing** → `ROADMAP.md` §"Open / not done", then `scripts/test-all.ps1`
