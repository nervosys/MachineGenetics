# MechGen, ab initio — maximally token-efficient, reliable, and safe

A first-principles design for a language an LLM writes, derived from this
project's measurements (not intuition). It supersedes the conservative token
analysis in [IDEAL_AGENTIC_LANGUAGE.md](IDEAL_AGENTIC_LANGUAGE.md): a real-BPE
measurement shows the token "floor" was the floor *of the current design*, not
the irreducible one — and there is measured headroom a clean design captures.

---

## 1. The reframe (measured, not asserted)

Three semantically-identical programs at three ceremony levels, counted with the
**real cl100k + o200k tokenizers** (`agentic-eval --example design_tokens
--features real-tokens`):

| Form | cl100k tokens | vs heavy |
|---|--:|--:|
| **A — ceremony-heavy** (explicit types, braces, `;`, imports, `Option<T>`) | 145 | 100% |
| **B — current-MechGen-ish** (sigils, `val/var`, partial inference) | 122 | 84% |
| **C — ab-initio** (full inference, layout, ambient builtins, terse sigils) | **69** | **48%** |

**Ceremony is ~half the tokens, and it is designable away.** What remains at C —
~48% — is the **payload**: the identifiers, operators, and literals that *denote
the agent's intent*. That residue is the true, irreducible floor (no language can
remove the agent's chosen names and values). Earlier this project reported
"token efficiency is floored ≈ Rust (0.60)" — true for **current** MechGen (form
B), which still carries ceremony, but *not* the irreducible floor. The floor is
the payload (form C); current MechGen leaves ~half its tokens on the table.

> Honest caveat: a small part of A→C is shorter names (a payload choice available
> in any language); the **dominant, designable** part is structural ceremony
> (inference + layout + ambient builtins + sigils). The factorial case isolates
> it — same algorithm, ceremony alone accounts for most of the cut.

---

## 2. The central principle: verification is the compiler's cost, not the token budget

Token-efficiency, reliability, and safety look like they trade off — reliability
and safety usually mean *more* explicit text. The resolution, and the spine of
this design:

> **Every guarantee that requires the agent to *write* something is a token tax.
> Design so guarantees are default-on and *inferred*, checked by the compiler, and
> surfaced only as machine-readable diagnostics.**

The agent spends tokens on **intent** (the payload). The **compiler** spends on
verification (types, effects, exhaustiveness, safety) and answers on its *output*
channel (diagnostics), which is not the agent's input budget. This makes the three
goals *aligned*, not opposed:

- **Token** ← strip everything inferable; guarantees you don't type are free.
- **Reliability** ← sound *inferred* types/effects/exhaustiveness + contracts
  catch the agent's mistakes; the agent paid zero tokens to get that checking.
- **Safety** ← memory-safety, capability effects, and no-exec are *defaults*
  (zero tokens), declared only where they cross a trust boundary.

---

## 3. The design

### 3a. Surface — minimize the multiplier (tokens per unit of meaning)
- **Maximal inference.** No local type annotations, no mutability markers, no
  return types, no imports. The compiler reconstructs all of it. (This is C/Go's
  measured edge, taken to the limit.)
- **Drop `;`; layout for readability.** Omitting `;` is a real per-statement token
  win. Replacing `{ }` with indentation is **token-neutral** (measured: BPE charges
  for the indentation whitespace ≈ what the braces cost — see §7), so layout is a
  readability choice, not a token lever. (This corrects the original assumption
  that significant indentation is itself a multiplier-reducer.)
- **Terse safety sigils.** `?T` optional, `T!E` result, postfix `?` propagation —
  safety in *one* token, not a four-token `Option<T>` wrapper.
- **Ambient builtins + capabilities.** `io`, `fs`, `net`, `llm`, collections,
  common ops need *no import*; a working snippet drags in nothing (kills the
  standing-context token term — the rubric's biggest amortized cost).
- **One canonical surface.** No dual human/agent syntaxes to drift or double-count.

### 3b. Tokenizer alignment — the lever the rain measurement exposed
The token cost of a surface is `information × tokens-per-symbol`, and
`tokens-per-symbol` is a property of **how the surface aligns with the BPE merge
table**, not of information. (Proven negatively: dense UTF-8 "digital rain" cuts
*characters* ~3× but costs ~2× more *tokens*, because BPE splits rare glyphs.)
So, by construction:
- **Keywords and builtins are single BPE tokens** in the dominant tokenizers
  (common English words: `if`, `for`, `match`, `fn`/`fun`, `use`…). Verified, not
  assumed — every reserved word is checked against cl100k/o200k.
- **Punctuation is minimal and common** (a lone ASCII sigil is 1 token; avoid
  multi-char operator soup like `::<>` that fragments).
- **Names tokenize well** — encourage `snake_case`/short conventional names that
  map to whole-word tokens, rather than forcing `CamelCase` that BPE splits.

This is the one genuinely *new* token lever: the floor is partly a surface-vs-
vocabulary property, and a language can be co-designed with the tokenizer.

### 3c. Reliability — sound, inferred, machine-actionable
- **Sound inferred static types** + **inferred capability effects** +
  **exhaustiveness** + **contracts** — full verification, zero annotation tax.
- **Diagnostics are the only error surface**: every error carries a stable
  `code + span + machine-applicable fix`, emitted as structured data, plus ranked
  auto-fix — the strongest self-correction loop, all on the compiler's output.
- **A complete, drift-proof self-ontology** (derived from the compiler's own
  tables) the agent grounds in, so it *doesn't make the mistake* — the
  first-pass-success half of reliability.
- **Property/fuzz-verified** front-end (no input panics).

### 3d. Determinism
- A **byte-stable binary IR** as the canonical artifact (content-hash cache/diff).
- An **idempotent** canonical formatter whose output always re-parses (the
  layout-significant surface makes formatting *the* normalizer).
- A **deterministic, sorted, machine-readable** diagnostic stream and self-ontology.

### 3e. Safety
- **Memory-safe by default** (no annotations).
- **Sound capability effects, inferred** — a function can't do net/fs/io/exec it
  doesn't (transitively) use; the *declaration* is required only where it crosses
  a trust boundary, so the common case is free.
- **No-exec artifacts** — loading a model/program is pure bounds-checked data
  decode, never code execution; the declared-vs-inferred effect surface is
  machine-readable for pre-run sandboxing.

---

## 4. The honest revised ceiling

With ceremony removed, the **token axis reaches the payload floor** — materially
above current MechGen (0.60) and above ceremony-heavy languages; co-designing with
the tokenizer pushes it further. The three designable axes stay near their maxima.
The composite ceiling therefore rises from the previously-stated ~0.90:

```
token ~0.85   determinism ~0.97   reliability ~0.95   safety ~0.96   →  composite ~0.93
```

This is an *upward* revision justified by measurement: the earlier 0.90 used a
conservative token 0.72 that under-counted the ceremony headroom the BPE numbers
now make explicit. (The exact token score needs the multi-language corpus
re-measured in the ab-initio surface — the lever, not the final digit, is proven.)

**~0.93 is the honest ceiling for a text language an LLM writes.** Below the
payload you cannot go: an LLM emits tokens for the *meaning*, and the meaning is
irreducible.

---

## 5. Beyond the floor (already built)

Even at the payload floor, a single call's tokens are bounded by intent. The only
lever past it is **paradigm, not syntax**: the agent emits a typed structured
spec against a cached, self-describing schema and the toolchain constructs a
deterministic no-exec binary artifact — the **tool-mediated construction** path
(`--build=abl`, shipped). It does **not** cut per-call tokens (the payload is
irreducible — measured), but it wins on the *amortized* terms (schema cached once,
reject-by-construction cuts retries) and on reliability/determinism/safety. That
is where token leverage actually lives once the surface is at its floor.

---

## 6. Migration (current MechGen → ab initio)

This is not a rewrite from zero — it is current MechGen minus ceremony:
1. **Significant layout** (offside rule) replacing mandatory `{ }` / `;`.
2. **Full local inference** — drop `val/var`/type/return annotations entirely.
3. **Ambient builtins + capabilities** — zero-import working snippets.
4. **Tokenizer-audited keywords** — a test that every reserved word is a single
   BPE token (the analogue of the existing ontology drift-guard).
5. Keep what already measured well: terse safety sigils, sound effects, byte-stable
   IR, machine-readable diagnostics, drift-proof ontology, the ABL tool-mediated
   track.

Each step is independently measurable on the token-bench + the real-BPE
`design_tokens` harness — so the redesign is driven by numbers, the same way this
analysis was.

---

## 7. Migration status (performed 2026-06-10)

Empirical probes of the *current* surface (`MechGen-parse --check`) mapped exactly
what is and isn't there, and two steps were landed safely:

- **Step 4 — tokenizer-audited keywords: DONE (measured).**
  `agentic-eval --example keyword_audit --features real-tokens` audits all 94
  reserved words against cl100k + o200k: **87/94 are already a single BPE token**
  (the agent-mode `f`/`m`/`v`/`u`… forms and common words `if`/`for`/`match`…).
  The 7 offenders are rare compound words (`swarm_fan_out` = 4 tokens,
  `swarm_map_reduce` = 3, `grammar_extension` = 2, …). They are the documented
  work-list; aliasing them is deferred as low-value (rarely emitted, cryptic
  aliases would cost more clarity than the tokens they save). The keyword surface
  is essentially already token-optimal.

- **Step 2a — return-type inference: DONE (landed in `types.rs`).**
  An un-annotated function now *infers* its return type from the body instead of
  defaulting to `()`. Implemented soundly via the two-pass checker: the signature
  registers a fresh type var (pass 1) that the body resolves (pass 2), so it is
  **recursion-correct** (`fn fact(n) { if n<=1 {1} else { n*fact(n-1) } }` checks)
  and callers see the inferred type. It only turns previous *errors* into
  successes. Every value-returning function can now drop `-> T`.

- **Step 2b — parameter-type inference: DONE (landed in `parser.rs` + `types.rs`).**
  The param type annotation is now optional (`f add(a, b) { a + b }`); an omitted
  type becomes `Type::Inferred` and binds the *same* signature fresh var the body
  and callers constrain — sound, and **generics + annotated params keep the exact
  existing path**, so it is provably non-regressing. With 2a this makes the full
  inferred signature `f sq(n) { n * n }` valid.

  **Measured (real cl100k/o200k, `inference_tokens`):** the now-valid inferred
  forms cost **~32% fewer tokens** than the annotated equivalents (square −40%,
  add −45%, factorial −19%). Real, not hypothetical — the compiler accepts them.
  **1149 tests green across 2a+2b, zero regressions.**

- **Step 1a — `;` optional (newline-terminated statements): DONE (`parser.rs`).**
  A binding statement now ends at a newline, a closing `}`, or EOF — `;` is only
  consumed when present (`expect_stmt_end` + `newline_before_current`). Expression
  statements already allowed this; only `val/var` required `;`. A same-line
  statement with no separator is still an error, so it only turns previous errors
  into successes. Combined with 2a/2b, a full multi-statement function now reads
  with no `fn`, no types, no return type, and no `;`:
  ```
  f area3(w, h) {
    val a = w * h
    val b = a + a
    b
  }
  ```
  **Measured (real cl100k/o200k, `inference_tokens`):** inference + `;`-optional
  together cut **~30%** of tokens vs the fully-annotated/semicolon form across four
  functions (area3 −28%). **1149 tests green, zero regressions.**

- **Step 1b — brace-optional layout blocks: DONE (`parser.rs`, TDD).**
  An indented body on a new line needs no braces. Built test-first: a layout test
  suite (single-expr, multi-statement, nested if/else, mixed brace+layout, dedent
  boundary between two fns, same-line-still-errors) was written and made to fail,
  then `parse_block` was refactored so an `{` keeps the exact original path and
  only a newline-introduced body enters a column-tracked `parse_block_body`. Works
  end-to-end (parses *and* type-checks, incl. nested if/else). **1187 tests green
  (8 new layout tests), zero regressions.** The full form-C surface now compiles:
  ```
  f area3(w, h)
    val a = w * h
    val b = a + a
    b
  ```

  **Honest measured sub-finding (real cl100k, `inference_tokens`):** dropping
  braces is **token-neutral — often slightly worse** (area3: braced-no-`;` 24 →
  layout 25). BPE charges for the indentation whitespace about what the two brace
  tokens saved — the same "whitespace tokenizes too" lesson as the digital-rain
  experiment. **So braces→layout is a readability/aesthetic feature, NOT a token
  lever.** The real token wins were `;`-removal (1a) and inference (2a/2b); §3a
  overstated layout as a multiplier-reducer and is corrected below.

- **Step 3 — effect inference at trust boundaries: DONE (`effects.rs`).**
  This is the §3e model, implemented soundly rather than as a blunt "drop the
  annotation" (which *would* have weakened safety). Now:
  - A **private** function infers its effects and needs **no** `/ effect`
    annotation.
  - A **public** function (and `main`) is a **trust boundary** — it must declare
    its effects (directly *or transitively* performed), exactly as before.
  - An **explicit annotation is still a contract** enforced everywhere
    (`inferred ⊆ declared`), pub or private — declaring `/ io` and doing `net` is
    still flagged.

  **Soundness is preserved, not weakened — it is *relocated* to the module
  surface.** Effects propagate transitively (Pass 2), so any effect a private
  function performs surfaces in the inferred set of every public caller that
  reaches it, and that boundary must declare it. Nothing escapes undeclared.
  This is verified empirically: the two effect fuzz properties were rewritten to
  the boundary model and pass over **6000 + 4000 generated programs** (a public
  caller is flagged for a private callee's undeclared effect; a declared bound is
  never exceeded). **1166 tests green, zero regressions.**

  **Measured (real cl100k, `inference_tokens`):** a private effectful function
  drops `/ io` for **−2 tokens** (more with multiple effects) — a real,
  **safety-preserving** saving, since enforcement moved to the boundary, not away.

**Evaluated and DECLINED — one step is genuinely negative-sum:**

- *Step 2c — keyword-less bindings* (`x = 5` with no `val`): **declined.** The
  saving is exactly **1 token/binding** (`val`/`v` are already single BPE tokens —
  keyword audit), and the probe shows assign-to-unbound is currently a **typo
  catch** (`error: unresolved name`). Trading a reliability guard for one token is
  backwards for a language scored on reliability. Net-negative — the one lever the
  design's own alignment principle says *not* to pull.

**Conclusion.** Every positive-sum lever is landed and measured: return inference,
parameter inference, `;`-removal, the keyword surface, the (token-neutral but
readable) layout, and boundary-scoped effect inference — together a real token
reduction with **reliability and safety intact** (effects still bounded at the
module surface, declared contracts still enforced, 1166 tests green). Only
keyword-less bindings is declined, because it alone would spend reliability for a
single token. The migration is complete at its positive-sum frontier.

Migration is driven by the probes + the real-BPE harness, landed safely in
test-passing increments — never a blind rewrite.

---

## 8. Second pass — the vocabulary frontier (2026-06-10)

The first pass was a *design*; it has now been *implemented and measured*, and a
second pass should be driven by what that taught — not a re-derivation. Two
results reframe the problem:

1. **The surface is now AT the payload floor.** With inference + `;`-removal
   landed, MechGen is **#1 of six** on the real-BPE `swe_token_benchmark`
   (85 cl100k vs Python 89, Rust 113) — but only ~5% ahead of Python. There is
   **no more text-surface headroom**: you are spending tokens on the payload
   (names/ops/literals), which is the floor.
2. **Whitespace is a cost, not a lever** (measured). Layout/indentation is
   token-neutral-to-worse (braces 24 → layout 25); BPE charges for indentation.
   Encoding tricks (binary, dense UTF-8 "rain") are token-neutral-to-worse too.
   **No representation beats the payload.** §3a's "layout reduces the multiplier"
   was wrong and is corrected.

So the first pass exhausted the *surface*. The second pass asks: with the surface
at the floor, **what is the next per-call token lever?** The answer is the one
thing that reduces the payload *itself* — **abstraction**.

### 8a. The lever: a standard vocabulary (measured 65%)

The payload is irreducible only *at a fixed abstraction level*. A high-level
primitive expresses an intent in far fewer payload tokens than hand-rolling it.
Measured (`--example abstraction_tokens`, real cl100k), hand-rolled vs a primitive:

| Intent | hand-rolled | via vocabulary | saved |
|---|--:|--:|--:|
| sum a list | 26 | `fold(xs, 0, +)` 13 | 50% |
| word frequencies | 29 | `freq(ws)` 8 | 73% |
| evens, doubled | 35 | `xs \| filter even \| map double` 12 | 66% |
| max of a list | 31 | `reduce(xs, max)` 10 | 68% |
| **total** | **121** | **43** | **65%** |

**65% — more than double the surface levers' ~30%.** And uniquely, it is
**positive-sum across all three axes** — the only lever that is:
- **Token** ↓ — the intent is named, not spelled out.
- **Reliability** ↑ — a *total, verified* primitive has no hand-rolled off-by-one,
  empty-list, or accumulator bug. Less agent-authored code is less to get wrong;
  this is the *first-pass-success* half of reliability, raised directly.
- **Safety** = — the primitive is *capability-typed*: its effect rides its type to
  the boundary (§7 step 3), so `read_file` carries FS automatically.

### 8b. The design discipline for the vocabulary

A standard library is not automatically a *token* lever — it is one only if
designed for it:
- **Single-BPE-token names.** `sum` not `summation`, `map` not `transform`, `freq`
  not `frequencies`. Audited against cl100k/o200k exactly like keywords
  (`keyword_audit` extended to the stdlib). A two-token primitive name leaks the
  saving back.
- **Total / well-defined.** No panics — `max` of an empty list returns `?T`, etc.
  Totality is what lets the agent *not* write the guard, and what makes the axis
  win real instead of moving the bug into the library.
- **Capability-typed.** Effects in the primitive's signature, inferred-and-bounded
  at the boundary — safety stays free.
- **Frequency-matched.** Chosen by the empirical distribution of agentic-SWE
  intents (map/filter/fold/reduce/freq/sort/zip/group/scan…), not by completeness.
  The long tail of rare operations is *not* worth a reserved name.
- **Composable in one operator.** A pipeline `xs | filter even | map double` chains
  primitives at ~1 token per stage — the combinator glue must itself be near-free.

### 8c. The coupled requirement: discoverability is amortized, not paid

The payload moves from "spell out the loop" to "name the operation" — but only if
the agent *knows the operation exists*. That knowledge must cost ~0 per call, so
the vocabulary lives in the **cached, drift-proof self-ontology** (the schema-cache
lever already built): fetched once, prompt-cached, never re-emitted. The token
math is then: novel logic pays the payload floor; vocabulary-covered intents pay
~1 token (the name) + a one-time cached cost to know the name. This is why
abstraction beats every encoding trick — it doesn't re-encode the payload, it
*removes* it for the common case.

### 8d. The universal law (generalized from the effects work)

The session proved one pattern soundly for effects (§7 step 3) — **infer inside,
contract at the boundary**: private functions infer; public boundaries declare;
the compiler enforces `inferred ⊆ declared`; transitivity makes it sound. The
second pass elevates this to the **design law for all verification** — types,
effects, contracts, and the vocabulary's capability typing alike. It is the
concrete mechanism behind §2's thesis ("verification is the compiler's cost, not
the token budget"): internal code pays *zero* annotation tokens; only the module
surface declares; nothing escapes. Reliability and safety become free *inside*,
enforced *at the edge*.

### 8e. Honest end state

- The **text surface is at the payload floor** — confirmed, not asserted. Further
  syntax work is readability, not tokens.
- The **vocabulary lever cuts the payload ~65% on covered intents** — the dominant
  remaining lever, and the only positive-sum one. Novel logic stays at the floor.
- Below that lies only **paradigm**: tool-mediated construction (emit intent, the
  toolchain builds the artifact) — and even there the intent is written *in the
  vocabulary*, so the vocabulary is upstream of everything.

**Actionable second-pass design for MechGen:** freeze the surface (it is at the
floor; treat whitespace as cost); curate a **frequency-matched, single-token,
total, capability-typed standard vocabulary** with a near-free pipeline operator;
audit its names as single BPE tokens; publish it in the cached ontology; and apply
**infer-inside / contract-at-boundary** uniformly. That is where the next real
token efficiency lives — and it buys reliability rather than spending it.

### 8f. Implementation status (landed 2026-06-10)

The first slice is shipped:
- **24 vocabulary combinators registered** (`resolve.rs`): `map filter fold reduce
  sum len sort reverse zip freq first last count any all find take range keys
  values flatten group scan contains`. They resolve and type (inferred from use,
  generic-safe per call, like `max`), so an agent can write `freq(ws)`,
  `sum(map(xs, sq))`, and `xs |> filter(even) |> map(double)` — all `--check`ed.
  **997 prototype tests green, zero regressions.**
- **Pipeline operator** `|>` already existed and composes the vocabulary.
- **Names audited single-token** (`vocabulary_audit`, real cl100k+o200k):
  **27/27** (the 24 + min/max/abs) are a single BPE token — §8b holds.

- **Precise total signatures — now LANDED (`types.rs`).** The combinators are
  typed precisely with *fresh-per-call generics* (`infer_vocab_call` +
  `collection_elem`), so misuse is caught and the **reliability** win is real, not
  just the token win:
  - scalar/element: `len/count → usize`, `sum → A`, `first/last → ?A`;
  - collection: `sort/reverse/take/flatten → [A]`, `zip → [(A,B)]`,
    `freq → {A: usize}`, `keys/values → [K]/[V]`, `range → [usize]`;
  - higher-order: `map(xs, f) → [B]` (f: `A→B`), `filter/any/all/find` (pred:
    `A→bool`), `fold(xs, init, f) → B`, `reduce → ?A`.
  - **Totality enforced:** `first/last/find/reduce` return `?A`, so `first(xs)`
    used as a bare value is a *type error* — the agent must handle the empty case.
  - **Misuse caught:** `sum("hi")` and `sum(5)` (concrete *and* integer-literal
    non-collections), wrong arity, and a predicate of the wrong shape.
  - **User functions shadow** the vocabulary; `min/max/abs/group/scan` fall through
    to generic inference. 6 new typing tests; **1003 prototype tests green, zero
    regressions.**

  So both halves of §8a are now real: the **token** win (name the intent, ~65%)
  *and* the **reliability** win (total, precisely-typed primitives that catch the
  mistakes hand-rolled code makes).

- **Discoverability — now LANDED (§8c).** A single-source `resolve::VOCABULARY`
  table `(name, signature, summary)` drives name resolution, typing, *and* a new
  **`vocabulary` section in the self-ontology** (`--emit-ontology`): the emitted
  JSON carries all 24 combinators with their signatures (e.g.
  `map: ([A], A->B) -> [B]`). So an agent grounding in the **cached** ontology
  discovers the full vocabulary without spending tokens to rediscover it — the
  coupled requirement that makes the lever pay. Drift-proof: a test asserts the
  ontology section covers every `VOCABULARY` entry, and the same table registers
  the names, so resolution / typing / discovery cannot diverge. **1004 tests green.**

- **Runtime — now LANDED (`eval.rs`, `--eval`).** A focused tree-walking
  evaluator executes the vocabulary and the arithmetic/control-flow around it, so
  the combinators compute real results (not just type). `Value` covers
  int/float/bool/str/list/map/tuple/option/closure; `map/filter/fold/reduce/sum/
  freq/sort/zip/first/…` are implemented over it, with named-function and closure
  (`fn(x) => …`) arguments, recursion, `for`/`while`, and the `|>` pipeline.
  Verified end-to-end (`--eval`): `fold(map(filter([1..6], even), dbl), 0, add) =
  24`, `freq([7,7,8,7]) = {7: 3, 8: 1}`, `sum(range(100)) = 4950`, `fact(6) = 720`.
  8 evaluator tests; **1012 tests green, zero regressions.** (Pure subset —
  IO/structs/traits report an honest "unsupported" rather than miscomputing.)

  **§8 is fully implemented:** registered → precisely typed/total → discoverable →
  **executable**, all from one authoritative table.
