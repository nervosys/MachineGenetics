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

**Evaluated and DECLINED — the remaining steps are negative-sum:**

The migration's purpose was token efficiency *without* costing reliability or
safety (§2). Probing the last two steps shows both fail that test — they'd buy a
token or two by spending the other axes — so completing the migration means
*stopping here*, not forcing them.

- *Step 2c — keyword-less bindings* (`x = 5` with no `val`): **declined.** The
  saving is exactly **1 token/binding** (`val`/`v` are already single BPE tokens —
  keyword audit), and the probe shows assign-to-unbound is currently a **typo
  catch** (`error: unresolved name`). Trading a reliability guard for one token is
  backwards for a language scored on reliability. Net-negative.

- *Step 3 — effect-inference by default* (drop the `/ effect` annotation):
  **declined as a default.** MechGen already *infers* effects (it reports `[IO]`),
  but requiring the declaration is a **deliberate, tested safety invariant** —
  `effects.rs` asserts an unannotated effectful fn *must* be flagged, with the
  comment *"the capability gate non-bypassable."* Inferring-and-not-requiring would
  weaken the safety axis to save one annotation per effectful function. The §3e
  vision ("inferred, declared only at trust boundaries") is *sound* but needs a
  real trust-boundary construct first, and changing the effect-safety default is a
  decision the project owner should make explicitly — not a quiet token tweak.
  (Pure functions already need no annotation, so pure code pays zero today.)

**Conclusion.** The positive-sum levers — return inference, parameter inference,
`;`-removal, the keyword surface, and the (token-neutral but readable) layout —
are landed and measured (~29% real token reduction, 1187 tests green). The two
remaining levers cost reliability or safety for ~1 token each, which violates the
alignment principle this whole design rests on. The migration is therefore
*complete at its positive-sum frontier.*

Migration is driven by the probes + the real-BPE harness, landed safely in
test-passing increments — never a blind rewrite.
