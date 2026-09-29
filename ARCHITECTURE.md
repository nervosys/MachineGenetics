# Architecture — Agentic Binary Language (ABL) & tool-mediated construction

This document describes the **ABL paradigm** as built and verified in the
MAGE prototype: an LLM agent constructs verified, deterministic, no-exec
binary AI artifacts by emitting **typed structured specs** instead of source
text. It is the leverage the text-token floor denies the language track (see
[IDEAL_AGENTIC_LANGUAGE.md](IDEAL_AGENTIC_LANGUAGE.md) for that analysis).

> **Scope.** Everything below is implemented and test-covered in `prototype/`
> (**1,322 tests** green) and scored in the sibling `agentic-eval` crate (80
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

> **This block was right when four others were wrong.** On 2026-08-18 the
> symbol-table line was found missing from `MAGE_ONTOLOGY.json`'s `abl.format`,
> from `AGENT_PROTOCOL.md`, and from the module docs in both `abl.rs` and
> `main.rs` — so a decoder written from any of those stopped 100 bytes early on
> a 420-byte container. It was here, correct, the whole time. Nobody had
> compared the five descriptions of one format against each other.

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
are **six independent Cargo workspaces**:

| Path | Crate | Tests | Notes |
|---|---|--:|---|
| `RecursiveMachineIntelligence/` | `rmi` | 1,384 | The low-level neurosymbolic framework. Feature-gated (`cpu` / `gpu` / `cuda`); build with `--no-default-features --features cpu` for the portable set |
| `prototype/` | `mage-prototype` | 1,322 | Compiler, evaluator, ABL, RAP server. Path-depends on `rmi` |
| `ribosome/` | `ribosome` | 168 | The distributed build engine. Depends on nothing in this repository — see below |
| `germline/` | `germline` | 147 | Model succession, handoff, fallback — the RSI control plane. Path-depends on `ribosome` |
| `forge/` | `forge` | 63 | The package registry, and the content-addressed block and definition stores |
| `arena/` | `arena` | 56 | The self-driven loop: agents write MAGE, learners predict it, joules are counted. Path-depends on `prototype`, `germline` and `forge` |

The dependency graph is a forest, not a web:

```
rmi ←── prototype ←── arena ──→ germline ──→ ribosome          forge
```

`forge`'s count is not a regression. `ribosome` and `germline` were developed
inside it and moved out on 2026-08-04; **52** is what the registry alone was
before they arrived, and this table said exactly that until they did. It reads
60 now for three unrelated reasons. 2026-08-18 added a test comparing `forge
manifest` against the binary's dispatcher, which nothing had ever done, and
removed one that checked three command names by hand. 2026-09-15 added
`a_block_that_does_not_hash_to_its_name_is_refused`, after `get_by_sha` was
found serving a block whose bytes no longer matched the content address naming
its file. Later the same day, six tests arrived with `Effects` and
`EffectOracle` — and the count moved by six rather than seven, because that
same commit fixed a duplicated `#[test]` attribute. The duplicate had been
generating a phantom second copy of the tampering test while swallowing the
attribute on `identical_block_is_deduplicated`, so the registry's dedup
property had never once been exercised and the total had been inflated by one
to hide it.

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
  is `serde`, `serde_json`, `sha2`, `ed25519-dalek` and nothing else:
  **28 crates transitively** — 34 counting the six that resolve at two versions
  (`sha2`, `digest`, `block-buffer`, `crypto-common`, `cpufeatures`, `syn`).
  This said 39, and so did `RIBOSOME.md`, because one was copied from the other
  and neither was measured. Now pinned: `scripts/test-all.sh --check-docs`
  measures it with `cargo tree -e normal` and fails if either document drifts.
  Say which of the two counts you mean — they differ by a fifth. Encryption (`rustls`) is behind the optional `tls` feature and
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
scripts/test-all.sh              # all six crates, debug
scripts/test-all.sh --release    # optimized
scripts/test-all.sh --bench      # + eval_bench (73/73) and perf_report
scripts/test-all.sh --cuda       # + prototype --features cuda (1,229 tests)
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
verified on hardware — 1,229 tests on dual 3090 Ti.

---

## Two regimes: where failure is free, and where it is fatal

Evolution needs somewhere most candidates die cheaply. Governance needs
somewhere nothing takes authority without being adjudicated. Those are opposite
requirements, and this repository is built almost entirely to the second —
every checker fails closed, the promotion gate promotes only on an empty reason
list, provenance refuses five ways before it accepts. That instinct is correct
for authority and wrong for exploration: a search in which every candidate must
survive a canary phase and a held-out suite runs at perhaps 10³ a day, and
biology runs 10⁶.

**The split is not a new mechanism.** It is a statement about which existing
mechanism belongs on which side, because one used on the wrong side is either a
bottleneck or a hole.

| | sandbox — failure is free | authority — failure is fatal |
|---|---|---|
| capabilities | none; nothing here may act | `requires_approval`, `sandbox/policy` |
| persistence | nothing survives the candidate | the journal, written as it happens |
| fitness | proxy only — `directed::Predictor`, modelled size, static checks | the held-out suite, `min_shadow_successes` |
| decision | `variation::propose` and its refusals | `gate::Episode::adjudicate` |
| identity | none | evaluator ≠ challenger, Ed25519 across a trust boundary |
| refusals | expected steady state, returned as data | an incident, journaled |
| what matters | throughput | attribution |

**`propose` is the boundary.** Everything before it is search, and a candidate
refused there costs a set difference. Everything after is succession, and a
candidate refused there has already cost a build, an evaluation and a canary.
`Proposal::refused` exists so the two are distinguishable: a search that
produces eight candidates and refuses all eight is pushing against a capability
boundary, which is a different fact from finding nothing.

### What the boundary can and cannot read

`propose` refuses a child that declares an effect no parent did. That is only a
check if the effects it compares are *known*, and for most of this session they
were not: the registry recorded no effects at all, genes arrived carrying an
empty list, and an empty list read as a purity claim. Every candidate passed —
not because nothing escalated, but because there was nothing to compare. **A
check that cannot fail is not a check**, and this one could not, which is the
same shape as the absence-claim problem `check-crypto-inventory.sh` names.

The fix is a type rather than a rule. `forge::models::Effects` and
`germline::variation::Effects` both distinguish `Checked { declared }` from
`Unchecked`, so "found to have no effects" and "nobody looked" stop being the
same value. The error then runs the safe way:

| gene state | `legality` | why |
|---|---|---|
| checked, effects ⊆ parents' | `Inherited` | proposable |
| checked, effects ⊄ parents' | `Acquired` | the escalation signal the boundary exists to produce |
| unchecked | `Unknowable` | reading it as pure would admit exactly what is being checked for |

Refusing the unchecked case costs one candidate and names the block that needs
checking; admitting it costs the property. An unchecked gene is refused even
when a parent carries the same one — inheritance legitimately launders a
*declared* effect, but two unknowns do not make a known.

`forge` cannot populate `Checked` itself: it hashes and stores bytes, and
deciding what bytes do is a front-end pass it does not contain. `EffectOracle`
is that seam, mirroring `GeneResolver` on the germline side and separate for
the same reason. The compiler already computes per-function effects
(`mage-parse --check` reports them); the oracle is how that answer reaches the
registry without the registry depending on a compiler. Checking a block later
**upgrades** its index entry, so the remedy for a refusal is checking the block
rather than weakening the check.

A consequence worth stating plainly: **a candidate that cannot be evaluated
cheaply will not be tried.** If proxy fitness requires a full build, the search
is authority-regime whatever it is called, and its throughput will be the
gate's. `Architecture::decode` is total over `&[f64]` precisely so that
rejection is about fitness rather than parsing — that totality is what makes
the sandbox regime affordable, and it is worth protecting when the decoder
grows to read genes.

## The improvable surface

For *recursive* to mean anything, the improver must be improvable. Today it is
not: the compiler is Rust, agents write MAGE, so agents improve artifacts and
not the substrate that judges them.

Self-hosting is the obvious answer and the wrong first move. Biology does not
encode its ribosome in the mRNA it translates — expression machinery sits
outside the genome, which is why the build engine here is called `ribosome` and
the heritable material `germline`. What makes a loop recursive is not that the
compiler is written in the language. It is that **the policy the compiler
applies is data the loop can change.**

Much of that policy is already data, and one surface is now loadable:

| surface | size | governs | today |
|---|---|---|---|
| SKB safety rules | 255, 8 databases | what agents and codegen are *told* is unsafe | **loaded overlay over a Rust floor** |
| heal patterns | 34 | what a broken candidate recovers to | Rust |
| elision rules | — | the agent-mode surface itself | Rust |
| cost model | `cost.rs` | which constructs search prefers | Rust |
| CI floors | 6 | what counts as a regression | shell, hand-edited |
| the 21 checkers | — | what "green" means | shell |
| the pins | 94 | which claims must match measurement | shell + docs |

**The harness is the part that moves under a fixed model.** With the weights
held constant, improvement comes from scaffolding: a sharper legality check, a
cheaper proxy fitness, a checker that catches a class of error one stage
earlier, a heal pattern that recovers a mutation that would otherwise be
discarded. Those are all harness changes, and none of them is reachable by a
loop that may only emit `.mg` files. A system whose agents can rewrite their
own evaluation criteria but not their own tools has the recursion pointed at
the least useful half.

Making a surface evolvable means four things, and the repository already has
machinery for each:

1. **Content-addressed** — `forge` stores blocks by hash and `ribosome`'s CAS
   rehashes on read, so an artifact's identity is its content.
2. **Constructible and rejectable** — `--build=abl <spec>` builds from a typed
   spec and refuses invalid ones, which is closure under variation implemented
   as a gate rather than an encoding.
3. **Gated** — changes take effect only through `Episode::adjudicate`, so a
   policy change is a succession like any other.
4. **Attributable** — the journal records the operators and seed, so
   "generation 47 was produced from 46 under seed 0x…" stays checkable.

What is missing is not mechanism but *representation*: these surfaces are Rust
literals rather than artifacts. The smallest real step was to move one of them,
and the SKB was the obvious candidate — `skb/` is already a generated tree and
`check-skb-tree.sh` already compares it against the compiler. That step is
taken.

### What the SKB is, and is not

**No SKB rule is executed.** The 255 rules are knowledge, not a compiler pass:
`codegen_bridge` reads `query_rules_by_tag("safety")` and keeps the
*descriptions* as strings, and `rmi_ontology_adapter` substring-matches over
category, description, rationale and tags so agents can find them. Nothing
matches a rule id against an AST, and the effect checker, the type checker and
the contract verifier are separate machinery that does not consult the SKB at
all.

This row of the table said the SKB governs "what variation may legally
produce". It does not, and did not — that is `propose`'s legality check and the
front-end passes. What the SKB governs is what an agent is *told*, which is a
real thing to protect and a smaller one than enforcement.

**And a large part of it describes conditions no pass can detect.** The
pipeline is lex, parse, resolve, typecheck, effect inference, MLIR lowering,
heal — there is no ownership or borrow-checking phase. So of the 255 rules,
the 40 ownership and 40 borrow rules are knowledge about a language rule the
checker does not implement, and the 35 lifetime rules sit behind
`AEL-0003`'s claim that lifetimes are inferred rather than checked. That is not
a defect to fix by deleting rules — the knowledge is what agents search — but a
reader who assumed `mage-parse --check` enforces the ownership database would
be wrong, and this paragraph exists because I assumed it.

`DiagnosticCategory` divides the same way, and `check-diagnostic-codes.sh`
now records the split. Seven variants are constructed by a pass on finding the
condition — `TypeMismatch` (types.rs), `UnresolvedName`, `UnresolvedType`,
`DuplicateDefinition` (resolve.rs), `UndeclaredEffect` (effects.rs),
`SyntaxError`, `Other`. Three — `BorrowConflict`, `UseAfterMove`,
`SpecViolation` — have exactly one non-test mention each, all inside
`heal::infer_category`, the function that guesses a category from a message
someone else already wrote. Those three say the compiler can *relay* such an
error, not that it can *find* one, and for the first two that is precisely
because no ownership or borrow-checking phase exists. `SpecViolation` is the
third for a different reason: the contract verifier runs and prints its own
summary rather than emitting diagnostics, so a refuted contract produces no
coded diagnostic at all.

`DiagnosticCategory` tells the same story from the other side. Ten variants,
each with a stable code that hir.rs calls "machine-matchable" and part of the
agent contract; measured against the crate, **`UseAfterMove` had no producer at
all** and `BorrowConflict`'s only one is the healer classifying a message it was
handed. `E0382` was unreachable: `heal::infer_category` folded "move" into the
borrow branch, so *use of moved value `x`* came back coded `E0502` while the
code that names it could not be emitted — and the fix table three hundred lines
above recognised the same message as a move and offered move fixes for it. Two
predicates, disagreeing. They are one function now.

It matters here because it sets what the floor below is worth. An overlay that
could lower severities would change the advice a synthesising agent receives,
silently and with no diagnostic anywhere; it would not turn off a check,
because there is no check to turn off. Making these rules executable is a
separate step that nothing in this section has taken.

### The builtin rules are a floor, not a default

`$MAGE_SKB_OVERLAY` names a directory of rule arrays merged onto
`builtin_rules()` at startup. The merge **starts from** the builtins and an
overlay may only introduce a new id or raise a severity, so removing a rule is
not something the format can express. The guarantee is structural rather than
checked, which is the difference between a property and a test of one.

That is what makes the fail-open/fail-closed question dissolve rather than get
decided. The obvious framing is a dilemma — halting on an unreadable policy
store stops the loop, while continuing evaporates the safety check exactly when
something is wrong — and it is a dilemma only because it assumes the fallback
might be weaker than what was lost. Under a floor it cannot be, so the two
cases separate cleanly:

| state | result | why |
|---|---|---|
| no overlay directory | the builtins | nobody wrote a policy, and no expressible policy is weaker |
| overlay adds ids / raises severities | applied, reported on stderr | the loop improving how it judges |
| overlay would lower a severity | **halt** | the floor is the point |
| overlay unreadable, malformed, or self-contradictory | **halt** | something *was* written and cannot be honoured |

The last row is the one worth stating separately. Reading a corrupt overlay as
an absent one would mean truncating a policy file silently restores different
rules — an absence claim that cannot fail loudly, the same shape as `get_by_sha`
serving a block whose bytes no longer matched its name, and as an unchecked gene
reading as a pure one. Three occurrences in one subsystem is a house style, not
a coincidence.

`--emit-skb` deliberately still emits the builtins rather than the installed
set. The committed `skb/` tree is a projection of the floor and its manifest
says so; if it tracked the active policy, `check-skb-tree.sh` would be testing
whether `$MAGE_SKB_OVERLAY` happened to be set in CI's shell.

### A policy has a name

`RuleSet::digest()` is SHA-256 over the rules in force, and the startup line
reports it. It hashes the **merged set**, not the overlay files: what a run
needs to be able to state afterwards is which policy it enforced, and that is
the rules, not the spelling of the directory that produced them. Two overlays
that differ in filenames, formatting or how rules are split across files are the
same policy and hash the same; moving one severity does not. The builtins have a
digest too, so "which rules ran" has an answer on every run rather than only on
configured ones.

That is the fourth of the four properties above — attributable — and it is what
makes the third worth having. A gate can only adjudicate a policy change if the
policy before and after have names.

What is *not* yet done: no CLI flag surfaces the rule set, and changes to it do
not pass through `Episode::adjudicate`, so this is policy as named data rather
than policy as a gated succession. The remaining six surfaces in the table above
are untouched.

## 8. Why this is the agentic frontier

For a token-emitting model, the cost of *naming* a computation is irreducible, so
a text language tops out around composite 0.90 (token-floored). The way past that
is paradigm, not syntax: a **typed, self-describing, tool-mediated interface over
a deterministic no-exec binary artifact**. ABL is that interface — reject-invalid
specs by construction, build byte-stable artifacts, introspect and execute them as
pure data. The leverage lives in reliability + determinism + safety +
discoverability, exactly the axes a text language can't buy with fewer tokens.
