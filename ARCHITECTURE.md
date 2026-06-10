# Architecture — Agentic Binary Language (ABL) & tool-mediated construction

This document describes the **ABL paradigm** as built and verified in the
MechGen prototype: an LLM agent constructs verified, deterministic, no-exec
binary AI artifacts by emitting **typed structured specs** instead of source
text. It is the leverage the text-token floor denies the language track (see
[IDEAL_AGENTIC_LANGUAGE.md](IDEAL_AGENTIC_LANGUAGE.md) for that analysis).

> **Scope.** Everything below is implemented and test-covered in
> `prototype/` (976 lib + 132 + 30 tests green) and scored in the sibling
> `agentic-eval` crate (80 tests). The one deliberate non-feature is
> agent/swarm *execution* — see [Honest boundaries](#honest-boundaries).

---

## 1. What ABL is

**Agentic Binary Language (ABL)** is MechGen's binary IR target — the artifact an
agent emits, ships, loads, and introspects. It is **not** text source; it is a
deterministic binary container that:

- is **byte-stable** (same spec → byte-identical bytes → content-hashable cache key),
- **loads as pure data** — decoding never executes code (no pickle-class risk),
- is **self-describing** — the symbol table is serialized, so names recover on decode.

Under the hood ABL is produced/consumed via the vendored
**RecursiveMachineIntelligence (`rmi`)** crate's codec (`rmi::lang::codec`); RMI
keeps its own identity as the framework, ABL is the IR's name at the MechGen layer.

### Container format (`prototype/src/abl.rs`)

```
magic   : "ABL1"            (4 bytes)
version : u16 LE            (currently 2)
count   : u32 LE            (item count)
items   : count × { name_len:u32, name, expr_len:u32, expr_bytes }
symbols : sym_count:u32, then per id (in order) { name_len:u32, name }
```

`decode_container` returns the items (pure data); `decode_symbols` returns the
id→name table; both are bounds-checked and never execute. Extension: **`.abl`**.

---

## 2. The tool-mediated loop

A closed, no-exec loop over the artifact. CLI: `MechGen-parse <mode>`.

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

## 8. Why this is the agentic frontier

For a token-emitting model, the cost of *naming* a computation is irreducible, so
a text language tops out around composite 0.90 (token-floored). The way past that
is paradigm, not syntax: a **typed, self-describing, tool-mediated interface over
a deterministic no-exec binary artifact**. ABL is that interface — reject-invalid
specs by construction, build byte-stable artifacts, introspect and execute them as
pure data. The leverage lives in reliability + determinism + safety +
discoverability, exactly the axes a text language can't buy with fewer tokens.
