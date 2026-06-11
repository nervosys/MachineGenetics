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
- **Layout, not delimiters.** Significant indentation replaces `{ }` and `;` —
  each is ~1 token, and a function body has many. (Python's main token edge.)
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
