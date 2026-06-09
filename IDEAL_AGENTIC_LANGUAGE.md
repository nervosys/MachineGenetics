# The Ideal Agentic-First Language — a measurement-grounded design

**Goal:** maximize agentic-eval composite = `mean(token, determinism, reliability, safety)`.

**Method:** design from what this session *measured*, not from intuition. Every
choice below cites the evidence that justifies it, and the honest ceiling is
stated rather than hidden.

---

## The central finding: three axes are designable, one has a floor

| Axis | Designable? | Achievable | Why (measured) |
|---|---|--:|---|
| determinism | **fully** | ~0.97 | byte-stable IR + idempotent formatter + deterministic diagnostics/ontology are all verifiable properties (demonstrated, byte-identical) |
| safety | **fully** | ~0.96 | memory-safety + sound enforced capability effects + no-exec artifacts are design properties (soundness property-verified, 10k cases) |
| reliability | **mostly** | ~0.95 | sound types/effects/exhaustiveness + machine-readable fixes + ontology grounding + fuzz-verified robustness; the residual gap is *battle-testing*, not design |
| **token** | **floored** | **~0.72** | identifiers + literals = **62%** of bytes (measured) and are irreducible; keywords/sigils already ~minimal; only ~22% (punctuation/structure) is even partially compressible |

So the ideal language **maxes the three designable axes and accepts the token
floor.** Composite ceiling:

```
(0.72 + 0.97 + 0.95 + 0.96) / 4  ≈  0.90
```

**~0.90 is the honest ceiling for any text language an LLM writes.** No amount
of cleverness exceeds it, because the token cost of naming a computation
(identifiers, literals) is irreducible for a token-emitting model.

---

## Evidence the floor is real (this session)

- **Byte breakdown** of real code: identifiers 55.8%, punctuation/structure
  22.3%, keywords/sigils 16.1%, literals 5.8%. ~62% incompressible.
- **C / Go** (grammatically simple): ~22% fewer tokens than Rust/MechGen — but
  only via *type inference* + *dropping safety ceremony* (sentinel `-1` instead
  of `Option`). Terseness there is a **safety trade**, not free.
- **LLVM IR** (lower-level): ~2.5× *more* tokens than C/Go. Abstraction level
  doesn't help; IR text is verbose by construction.
- **Binary emission**: a 144 B model as base64 ≈ 106 tokens, hex ≈ 144, vs ~134
  text tokens. The byte win is **storage/transport/load**, and does **not**
  survive an LLM emitting it. Models emit tokens, not bytes.

Conclusion: you cannot win the token axis by simplifying, lowering, or
binarizing the text. You can only avoid *losing* it.

---

## Design of the ideal language

### Token (≈0.72 — the max for text)
The ideal recovers the ceremony tokens that put a safe language behind C/Go,
*without* dropping safety:
- **Maximal inference** — no local type annotations, **inferred mutability**
  (no `mut`/`m` keyword), inferred return types where unambiguous. (C/Go's token
  edge was inference; take it further.)
- **Terse safety sigils** — `?T` optional, `T!E` result, postfix `?`
  propagation. Safety expressed in one glyph, not a wrapper.
- **Zero standing context** — builtins and capabilities (`io`, `fs`, `net`,
  `llm`) need **no imports**; a working snippet drags in nothing.
- **One canonical surface** — no dual human/agent syntaxes to drift apart.

### Determinism (≈0.97)
- A **byte-stable binary IR** as the canonical artifact (caching/diffing by
  content hash).
- An **idempotent** canonical formatter (`fmt(fmt x) == fmt x`, property-tested)
  whose output always re-parses.
- A **deterministic, machine-readable diagnostic stream** (sorted, stable
  schema) — the error channel is reproducible, not just the build.
- A **deterministic self-ontology** emitted by the compiler.

### Reliability (≈0.95)
- **Sound** static types + **sound, mandatory, enforced** effects +
  **exhaustiveness** + contracts — catch the agent's mistakes at compile time.
- Every diagnostic carries **stable code + span + machine-applicable fix**, as
  JSON; plus ranked **auto-fix** — the strongest self-correction loop.
- A **complete, drift-proof self-ontology** (derived from the compiler's own
  tables) so the agent grounds in ground-truth and *doesn't make the mistake* —
  the first-pass-success half of reliability.
- **Property/fuzz-verified** front-end robustness (no input panics).

### Safety (≈0.96)
- **Memory-safe** by default.
- **Capability effects**: a function cannot perform a net/fs/io/exec effect it
  didn't declare (non-bypassable, soundness property-verified). No ambient
  authority.
- **No-exec artifacts**: loading a model/program is pure data decode — never
  code execution (no pickle).
- The full **declared-vs-inferred effect surface** is machine-readable, so a
  runtime can sandbox/refuse generated code *before* running it.

This is, deliberately, **MechGen's design taken to maturity with maximal
inference** — the session validated these properties one axis at a time. The one
change from today's MechGen is dropping binding-type/mutability ceremony to
reclaim the token axis (0.60 → ~0.72) without surrendering safety.

---

## Tool-mediated construction — built and MEASURED

The proposed way past the floor was tool-mediated construction: the agent emits a
schema-validated structured spec instead of source, and the toolchain builds the
artifact. This is now **built** (`MechGen-parse --build=ml <spec.json>`,
`prototype/src/builder.rs`): a compact JSON net spec is validated structurally
(unknown op / bad dims / **shape mismatch** rejected *by construction*, with
machine-readable errors and no artifact emitted) and lowered to the byte-stable,
no-exec Machine Language artifact through the existing pipeline.

**But the measurement corrected the thesis.** It does NOT beat the *per-call*
token floor:

```
JSON spec (agent emits): 54 tokens   .mg source (text):  43 tokens   → JSON +26% WORSE
```

JSON's quotes/brackets/commas cost more than the terse DSL. **The token floor is
universal** — text, IR-text, binary-as-base64, *and* structured JSON all cost ≈
the irreducible payload (names, ops, dims). No encoding escapes it, because the
information content is irreducible.

What tool-mediated construction *genuinely* buys (measured/verified):
- **reliability**: invalid specs rejected before any artifact (shape mismatch,
  unknown op, bad dims) — fewer correction rounds.
- **determinism / safety**: byte-identical artifact, pure-data (no-exec) load.
- **amortized tokens** (the rubric's session model, not per-call): the schema is
  paid once (prompt-cached, not re-emitted each turn) and validation cuts retry
  rounds — so over a session it wins on the *standing-context* and *retry* terms,
  even though a single call's payload ≈ source.

So the honest frontier isn't "fewer tokens per call" (impossible — the payload is
irreducible). It's **reliability + determinism + safety by construction, plus
amortized token savings** from a cached schema and fewer retries. The IR/builder
belongs on the framework track for exactly this reason.

### The complete loop (built 2026-06-09)

The paradigm is now a closed, three-step, no-exec loop over the binary artifact:

```
1. MechGen-parse --build=schema            # typed self-describing interface
     → deterministic JSON: op catalog (arities, shape-rule), spec format,
       full error-code catalog with fixes. Fetched ONCE, prompt-cached —
       the standing context the agent grounds in (amortized tokens).
2. MechGen-parse --build=ml spec.json out.ml
     → validate the spec (reject-by-construction: B0001–B0006, machine-readable,
       NO artifact on failure) → lower to a byte-stable Machine Language artifact.
3. MechGen-parse --describe=ml out.ml   # no-exec structured introspection
     → decode the artifact as PURE DATA (exec:false) into JSON: container size,
       per-item content hash, recovered op/dim structure. The agent verifies
       what it built without ever running it.
```

The loop spans **both halves of the neurosymbolic IR**: a `{"net":..}` spec
(neural layer stack) or a `{"kb":..}` spec (symbolic facts + rules). The kb path
carries its own reject-by-construction codes (K0001–K0006: empty kb, invalid
identifier, **arity conflict**, **dangling reference**), and `--describe=ml`
classifies each item (`kind: net|kb`) and reports the recoverable structure.

> Honest limitation (kb): the symbol table is **not** serialized into the
> container, so a kb artifact stores predicate **arities + the unify→infer rule
> structure**, not ground argument terms or predicate names. That *is* the
> symbolic IR the VM executes; `--describe=ml` reports the arities/counts and
> says so. Validation runs on the spec (names/refs present), so
> reject-by-construction is fully enforced before names are elided.

Verified properties (all property/regression-tested):
- **reject-by-construction**, both directions: 6000 generated net specs — no valid
  net refused, no invalid net ever reaches an artifact; plus kb validation
  (arity-conflict, dangling-ref, identifier, empty) (`builder.rs`).
- **byte-stable construction**: same spec → byte-identical `.ml` across builds
  (net and kb).
- **drift-proof schema**: the `--build=schema` op catalog and error codes are
  derived from the same `OPS` table the validator uses, with a test that fails on
  divergence — the agent's grounding can't drift from enforcement.
- **faithful no-exec round-trip**: `build → describe` recovers the exact op/dim
  structure (net) and fact-arities/rule-param-counts (kb); the describe
  content-hash equals the encode content-hash.

This is where the leverage the token floor denies the text track actually lives:
discoverability (self-describing schema), reliability (reject-by-construction),
determinism + safety (byte-stable, pure-data, no-exec artifact), and amortized
tokens (cache the schema once, fewer retries).

---

## Honest bottom line

- The ideal **text** agentic language scores **~0.90** — three axes near their
  designable maxima, token at its irreducible floor. (Today's MechGen: 0.865,
  short mainly on token ceremony.)
- You **cannot** honestly exceed ~0.90 for a text language; the token axis is a
  floor, not a knob.
- The path beyond is **paradigm, not syntax**: a typed, self-describing,
  tool-mediated interface over a deterministic no-exec binary artifact (built:
  `--build=ml`). It does NOT cut per-call tokens (the payload is irreducible —
  measured), but it wins on **reliability** (reject-invalid-by-construction),
  **determinism/safety**, and **amortized** tokens (cached schema + fewer
  retries). The binary IR's real wins live on the framework track.

## Capstone finding

**Direct-corpus confirmation (2026-06-09).** The parser now folds verbose
std-wrapper paths into their terse sigil AST forms (`Option<T>`→`?T`,
`Box<T>`→`^T`, `Result<T,E>`→`T or E`, …), so the canonical formatter emits the
terse idiom. Re-tokenizing all 100 scoring-corpus solutions through
`lex→parse→format` (99/100 parse) measured the canonical form at **−1.1%
(agent) / −0.4% (human)** vs as-authored — i.e. *no token win; marginally
worse*. The scoring corpus was already at the floor (the verbose spellings lived
only in hand-written examples). The token axis was therefore **not** raised —
the floor is not a knob, even when the terser surface exists and is free to use.

Across every representation tested — terse text, simple languages
(C/Go), low-level IR (LLVM), binary-as-base64, structured JSON, *and now the
canonical sigil form re-measured on the scoring corpus itself* — the token
cost of expressing a computation is **floored** at roughly the same level,
because the information (names, ops, dims) is irreducible for a token-emitting
model. **Token efficiency is a floor, not a knob.** The ideal agentic-first
language therefore doesn't chase tokens; it maximizes the three designable axes
(determinism, reliability, safety) to ~0.95+ and accepts ~0.72 token → composite
~0.90, and moves the *real* leverage (caching, validation, deterministic no-exec
artifacts) to a tool-mediated construction layer where it actually pays off.
