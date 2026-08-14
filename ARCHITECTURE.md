# Architecture — Agentic Binary Language (ABL) & tool-mediated construction

This document describes the **ABL paradigm** as built and verified in the
MAGE prototype: an LLM agent constructs verified, deterministic, no-exec
binary AI artifacts by emitting **typed structured specs** instead of source
text. It is the leverage the text-token floor denies the language track (see
[IDEAL_AGENTIC_LANGUAGE.md](IDEAL_AGENTIC_LANGUAGE.md) for that analysis).

> **Scope.** Everything below is implemented and test-covered in `prototype/`
> (**1,148 tests** green) and scored in the sibling `agentic-eval` crate (80
> tests, in the AetherShell repository and not verifiable from here). The one
> deliberate non-feature is agent/swarm *execution* — see
> [Honest boundaries](#honest-boundaries).
>
> *This read "976 lib + 132 + 30 tests green" — 1,138, against 1,038 measured.
> It is the fifth stale count found this session, and the first one
> `scripts/check-doc-counts.sh` did not catch: the checker read the
> repository-layout table below and never this banner. Now covered.*

---

## 1. What ABL is

**Agentic Binary Language (ABL)** is MAGE's binary IR target — the artifact an
agent emits, ships, loads, and introspects. It is **not** text source; it is a
deterministic binary container that:

- is **byte-stable** (same spec → byte-identical bytes → content-hashable cache key),
- **loads as pure data** — decoding never executes code (no pickle-class risk),
- is **self-describing** — the symbol table is serialized, so names recover on decode.

Under the hood ABL is produced/consumed via the vendored
**RecursiveMachineIntelligence (`rmi`)** crate's codec (`rmi::lang::codec`); RMI
keeps its own identity as the framework, ABL is the IR's name at the MAGE layer.

### Container format (`prototype/src/abl.rs`)

```
magic   : "ABL1"            (4 bytes)
version : u16 LE            (currently 3)
count   : u32 LE            (item count)
items   : count × { name_len:u32, name, expr_len:u32, expr_bytes }
symbols : sym_count:u32, then per id (in order) { name_len:u32, name }
```

`decode_container` returns the items (pure data); `decode_symbols` returns the
id→name table; both are bounds-checked and never execute. Extension: **`.abl`**.

---

## 2. The tool-mediated loop

A closed, no-exec loop over the artifact. CLI: `mage-parse <mode>`.

```
1. --build=schema                       typed, self-describing interface
     → deterministic JSON: per-kind spec format, op catalog (arities, shape
       rule), and the full error-code catalog with fixes. Fetched once,
       prompt-cached — the standing context the agent grounds in.
2. --build=abl spec.json out.abl        construct (reject-by-construction)
     → validate the spec; on failure emit machine-readable {code,message,fix}
       and write NO artifact; on success lower to a byte-stable .abl.
       (--fix attempts deterministic auto-repair first; see §5.)
3. --describe=abl out.abl               no-exec structured introspection
     → decode as pure data (exec:false) → JSON: per-item kind + recovered
       structure + content hash. Verify what you built without running it.
4. --run=abl out.abl                    execute (where semantics exist)
     → forward-chain each kb item to its fixpoint; report derived facts.
```

The schema is **drift-proof**: the op catalog and error codes are derived from
the same tables the validator enforces, with a test that fails on divergence.

---

## 3. The four item kinds

A spec is detected by its discriminating key. Each kind round-trips its full
structure through the serialized symbol table.

| Kind | Spec (positional) | Validates (reject-by-construction) | Lowers to |
|---|---|---|---|
| **net** | `{"net":N,"layers":[[name,op,[dims]]]}` | B0001–B0006 (unknown op, arity, non-positive dim, **shape-chain mismatch**) | layer-op chain |
| **kb** | `{"kb":N,"facts":[[pred,[args]]],"rules":[[name,[params],[body]]]}` | K0001–K0007 (ident, arity conflict, dangling body pred, **range safety**) | `RESOLVE` facts + `UNIFY…MATCH*…INFER` rules |
| **agent** | `{"agent":N,"capabilities":[…],"requires_approval":[…]}` | A0001–A0003 (identifiers) | `SPAWN(agent, caps…) [>> DELEGATE(approvals…)]` |
| **swarm** | `{"swarm":N,"agent":T,"size":k,"topology":…,"consensus":…,"transport":…}` | S0001–S0006 (idents, size>0, known topology/consensus, `rmi_*` transport) | `SPAWN(agent,size,topology) >> comm[transport] >> REDUCE(consensus)` |
| **unified** | `{"items":[ <any mix> ]}` | U0001–U0003 (empty, unknown kind, duplicate name); per-item errors index-prefixed | one multi-item container |

Why lowering carries names as extra op args: the `rmi` VM treats the
symbolic/agentic ops (`RESOLVE/UNIFY/INFER/MATCH/SPAWN/SEND/RECV/REDUCE/DELEGATE`)
as **arg-agnostic stubs**, so encoding names/terms as additional `Ref` args is
execution-safe and recovers losslessly via the symbol table.

---

## 4. Execution semantics (`--run=abl`)

- **kb** — a Horn-clause logic program. `rule h(x,z) where p(x,y), p(y,z)`
  lowers to `UNIFY(h,x,z) >> MATCH(p,x,y) >> MATCH(p,y,z) >> INFER`, reconstructed
  by a flat-`Seq` state machine and forward-chained to the **least fixpoint**.
  It is a **safe, terminating, pure-data interpreter** (no function symbols →
  finite Herbrand base; no arbitrary code), so the no-exec property holds. Rules
  are **range-safe by construction** (K0007: every head variable is bound by the
  body). Example: `edge(a,b), edge(b,c) ⊢ path(a,c)`.
- **net** — defer to `--run=abl-bytes`, which dispatches the decoded graph to the
  CPU backend (`abl_compute.rs`) for a real forward pass.
- **agent** — a **capability-policy evaluator**. Given requested ops via
  `--input {"ops":[..]}`, each op is decided **allowed** (in `capabilities`, not
  gated) / **requires-approval** (in both) / **denied** (not a capability).
  Without input it reports the policy surface.
- **swarm** — a **consensus evaluator**. Reports propagation rounds for the
  topology (graph diameter: mesh/star/broadcast = 1, ring = n−1, tree = ⌈log₂n⌉)
  and, given `--input {"proposals":[..]}`, the decided value under the strategy
  (`majority`/`weighted` = plurality, `unanimous`, `quorum` = strict majority;
  deterministic smallest-on-tie). Example: ring/quorum over `[7,7,7,3,7]` → **7**
  (4/5 quorum, 4 rounds).

All four are **pure-data interpreters** — they read the artifact and compute; no
arbitrary code runs.

---

## 5. Self-correction: auto-fix (`--build=abl --fix`)

On a rejected spec the toolchain applies **deterministic, conservative** repairs,
re-validates, and builds — turning reject-by-construction into one-shot correction:

- **net**: unknown op → nearest known op by edit distance; non-positive dim → 1;
  `Linear` input dim → previous layer's output (shape chain).
- **swarm**: topology/consensus → nearest valid; non-`rmi_` transport → `rmi_quic`.

Everything not auto-fixable is still surfaced as a machine-readable error + fix hint.

---

## 6. Honest boundaries

These are deliberate, documented scope lines — *not* gaps papered over:

- **agent/swarm execution is a *reference policy/protocol* model, not arbitrary
  agent behavior.** `--run=abl` evaluates the *declared* policy (capability
  gating) and protocol (consensus over proposals + topology rounds) — the natural
  meaning of the fields the spec stores. It does **not** run application logic (an
  agent has no code body in ABL); that would be a general agent runtime, which is
  out of scope by design. The model is deterministic and pure.
- **kb ground terms vs. arg order semantics.** Facts store predicate + ground
  term names verbatim; there is no separate constant/variable type system beyond
  "rule args are variables, fact args are constants."
- **Text token floor.** ABL does **not** reduce per-call tokens vs. source (the
  payload is irreducible — measured). Its wins are reliability, determinism,
  safety, discoverability, and amortized tokens (cached schema + fewer retries).

---

## 7. Source map

| File | Role |
|---|---|
| `prototype/src/builder.rs` | spec types, validation, schema, auto-fix repair |
| `prototype/src/abl.rs` | ABL container codec (encode/decode, symbol table) |
| `prototype/src/abl_bridge.rs` | lowering (AST → IR), decompile, `evaluate_kb` |
| `prototype/src/abl_compute.rs` | CPU backend (net forward pass) |
| `prototype/src/abl_shape.rs` | shape inference for the compute path |
| `prototype/src/main.rs` | CLI dispatch (`--build`/`--describe`/`--run`/`--fix`) |
| `prototype/src/ontology.rs` | drift-proof self-ontology (incl. the `abl` section) |
| `prototype/src/rap.rs` | RAP server (`abl/encode`/`decode`/`run`, `abl_hex`) |

---

## Repository layout — five workspaces, on purpose

`cargo test` at the repository root does nothing, and that is deliberate. There
are **five independent Cargo workspaces**:

| Path | Crate | Tests | Notes |
|---|---|--:|---|
| `RecursiveMachineIntelligence/` | `rmi` | 1,380 | The low-level neurosymbolic framework. Feature-gated (`cpu` / `gpu` / `cuda`); build with `--no-default-features --features cpu` for the portable set |
| `prototype/` | `mage-prototype` | 1,148 | Compiler, evaluator, ABL, RAP server. Path-depends on `rmi` |
| `ribosome/` | `ribosome` | 164 | The distributed build engine. Depends on nothing in this repository — see below |
| `germline/` | `germline` | 112 | Model succession, handoff, fallback — the RSI control plane. Path-depends on `ribosome` |
| `forge/` | `forge` | 52 | The package registry, and only that |

The dependency graph is a forest, not a web:

```
rmi ←── prototype          ribosome ←── germline          forge
```

`forge`'s 52 is not a regression. `ribosome` and `germline` were developed
inside it and moved out on 2026-08-04; 52 is what the registry alone was before
they arrived, and this table said exactly that until they did.

A root workspace *did* exist, but it listed only `compiler/*` — the forked-rustc
compiler — and was removed with it on 2026-06-11 (`b1b910f`). The surviving
crates were always built standalone via `--manifest-path`.

Keeping them separate is a trade, not an oversight:

- **`rmi` is vendored, not a submodule** (`UNIFICATION.md`). It must stay
  independently buildable and testable so it can be synced against its own
  upstream without inheriting this repo's lockfile. Merging it into a shared
  workspace would collapse the `Cargo.lock` files into one — including the
  pinned `lz4_flex >= 0.11.6` CVE fix recorded in `SECURITY_AUDIT.md` §1.
- **`ribosome` must not depend on MAGE.** Its central claim — that no language
  is privileged below the planner (`RIBOSOME.md` §2.1) — is not credible from a
  crate that depends on one language's compiler, so its default dependency list
  is `serde`, `serde_json`, `sha2`, `ed25519-dalek` and nothing else: 39 crates
  transitively. Encryption (`rustls`) is behind the optional `tls` feature and
  CI checks it has not leaked into the default build, because "optional" is a
  property that decays the moment something in the default path uses it.
- **`ribosome` must not depend on `germline`.** The Weismann barrier is
  one-way by design (`GERMLINE.md`): a build engine able to call into the
  succession layer is a somatic path into the germline, which is the failure
  that document exists to prevent. The crate boundary makes the one-wayness
  structural instead of a convention.

  Both of these are checked in CI with `cargo tree` rather than trusted to this
  document, because a dependency boundary is exactly the kind of property that
  erodes by one convenient `use`. The check was verified in both directions —
  it passes on `ribosome` and trips when pointed at a crate that does have an
  in-repo dependency.
- The cost is that no single `cargo` invocation covers everything.

So the supported entry points are:

```sh
scripts/test-all.sh              # all five crates, debug
scripts/test-all.sh --release    # optimized
scripts/test-all.sh --bench      # + eval_bench (73/73) and perf_report
scripts/test-all.sh --cuda       # + prototype --features cuda (1,071 tests)
```

```powershell
./scripts/test-all.ps1           # same, on Windows
./scripts/test-all.ps1 -Bench -Cuda
```

CI (`.github/workflows/ci.yml`) runs one job per crate over the same set. Because
`prototype` path-depends on `rmi`, an `rmi` change triggers every job.

### The CUDA feature

`--features cuda` pulls in `ironaccelerator-cuda`, **pinned to the published tag
`v2.2.0`** rather than a sibling path — a path dep meant the lockfile re-resolved
whenever a neighbouring checkout moved, and the feature could not be built from a
clean clone. `prototype/Cargo.toml` ends with a commented `[patch]` block for
developing against a local IronAccelerator.

Because IronAccelerator dispatches through `libloading`, the backend **compiles
with no CUDA toolkit and no GPU**, so CI can compile-check it (`cargo check
--features cuda --all-targets`). CI cannot *run* the kernels; GPU correctness is
verified on hardware — 1,071 tests on dual 3090 Ti.

---

## 8. Why this is the agentic frontier

For a token-emitting model, the cost of *naming* a computation is irreducible, so
a text language tops out around composite 0.90 (token-floored). The way past that
is paradigm, not syntax: a **typed, self-describing, tool-mediated interface over
a deterministic no-exec binary artifact**. ABL is that interface — reject-invalid
specs by construction, build byte-stable artifacts, introspect and execute them as
pure data. The leverage lives in reliability + determinism + safety +
discoverability, exactly the axes a text language can't buy with fewer tokens.
