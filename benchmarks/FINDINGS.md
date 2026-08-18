# MAGE Mission Findings (Phases 27-38, 2026-05-22 to 2026-05-26)

Six measurement passes against the 100-task benchmark corpus, each
exposing a real gap between the project's stated mission ("maximally
token-efficient and reliable for agents") and delivery at the time.

**This file is a chronological log, and the sections below are dated
snapshots.** Read them as history; the table here is the current
measurement, re-run 2026-08-18.

| Mission axis | Tool | Where it stands now |
|---|---|---|
| Token-efficient | `cargo run --bin token-bench` | Text surface tied with Rust (native-lexer ratio **0.997**); binary IR **2-6×** smaller per declaration, 0.225 for a whole container. See §1. |
| Reliable for agents | `cargo run --bin reliability-bench` | file-oracle parse **99 / 100**, effective **100 / 100**. Perturbed input parses 27 / 100 and heals to **70 / 100**. See §2 and §6 for how it got there — both record much lower numbers, because both are snapshots of when they were written. |

---

# § 1 — Token-Efficiency Finding (Phase 27, 2026-05-22)

## TL;DR

The MAGE surface as written in the **100-task benchmark corpus** does
**not** deliver the ~30 % reduction the corpus claims, nor the ~50 %
the `README.md` / `MAGE_SPEC.md` headline advertises. Across four
honest measurements, the actual delivered reduction ranges from
**+6.7 % (best case) to −5.5 % (worst case)**.

The only place the ~50 % claim is genuinely realised is in the binary
**Agentic Binary Language IR** (a transformer block in ~47 bytes — see Phase 1 demo). The
text surface, as currently written and used by the corpus, is a wash.

## Measurement table (100 tasks, all categories)

| Metric | MAGE | Rust | Ratio | Reduction |
|---|---:|---:|---:|---:|
| **Raw source bytes** *(what BPE actually sees)* | 55 066 | 52 220 | 1.055 | **−5.5 %** |
| Whitespace-stripped bytes | 36 048 | 38 629 | 0.933 | +6.7 % |
| Native syntactic tokens *(MAGE lexer + proc-macro2)* | 15 268 | 15 310 | 0.997 | +0.3 % |
| Shared-rule tokens *(same naive tokeniser for both)* | 15 831 | 15 310 | 1.034 | −3.4 % |
| **Corpus's claimed totals** | 11 210 | 16 095 | 0.696 | 30.4 % |

The four real numbers all sit within ±7 % of parity. The 30 % claim is
**not supported by any measurement methodology** I could find.

## Why the raw-bytes number actually goes the wrong way

MAGE sources in the corpus have **more whitespace** per byte:
- MG: 55 066 raw, 36 048 dense → 34.5 % whitespace
- RS: 52 220 raw, 38 629 dense → 26.0 % whitespace

The sigil compression in MG's *non-whitespace* content (6.7 % win) is
swamped by 8.5 pp of extra whitespace formatting. An LLM consuming
the raw source pays for both.

## Why the lexer-token number is tied

Both languages reserve the keyword set in their lexers, so `loop` and
`@@` both produce one `KwLoop` token — there's no token-count
advantage from preferring the sigil at the lexer level, only a
character-count advantage. The corpus's MG solutions already use
sigils where it matters; the few cases of `loop` / `break` / `return`
in long form are tokens-neutral.

Where MG does win on tokens: declaration syntax. `+f` is one token
(`KwF`'s text is `+f`), `pub fn` is two (`Plus` + `KwF`). But there
are far fewer declarations than expressions per program, so the win
disappears in the aggregate.

## Per-category split (native tokens)

| Wins for MAGE | Losses |
|---|---|
| web-network −13.8 % | algorithms +15.3 % |
| basic-io −6.2 % | concurrency +10.7 % |
| data-structures −5.2 % | systems +8.1 % |
| error-handling −4.1 % | generics-traits +2.3 % |
| agent-orchestration −3.6 % | |
| full-applications −2.4 % | |

The pattern: **declaration-heavy code wins**, **expression-heavy code
loses**. Rust's `if` / `else` / `mut` / `let` are short enough that
MAGE's `?` / `:` / `m` / `v` don't save much, while MG's longer
constructs (`R[T,E]`, `?T`) cost.

## Where a real reduction *does* live: the Agentic Binary Language binary IR

Measured on `prototype/examples/unified.mg`, comparing each declaration's
MAGE text against its encoded expr:

| Item | MAGE text | Agentic Binary Language | Ratio |
|---|---:|---:|---:|
| TransformerBlock | 243 B | **47 B** | 0.19 |
| MLP | 98 B | **17 B** | 0.17 |
| FamilyKb | 116 B | **68 B** | 0.59 |

So **roughly 2-6× on the declaration itself**, and 0.225 for the whole
container once names, length fields and the symbol table are counted. This
section was headed "where the ~50 % claim *is* real" and said the encodings
were "1-2 orders of magnitude smaller than the equivalent text in either MAGE
or Rust"; against MAGE text they are not, and the heading promised a figure
§1 had just finished disproving. Against *Rust* the gap is larger, but no
measurement of that pairing exists here, so it is not claimed.

**If the goal is "maximally token-efficient for agents," the answer is still:
have agents emit Agentic Binary Language directly, not text** — the reduction
is simply 2-6× rather than 50×.

The text surface is best understood as a human-readable view of Agentic Binary Language,
not as the agent's primary output medium.

## What this means for the actual mission

Three honest paths forward:

### Path A: Accept the result and re-frame
Update `README.md` and `MAGE_SPEC.md` to describe MAGE's actual
delivered value:
- A **zero-ambiguity LL(1) grammar** with rich agent-affordances
  (effects, contracts, SKB, cost oracle, self-healing).
- A **modestly more compact surface** for declaration-heavy code.
- A **binary IR (Agentic Binary Language) that is genuinely smaller** than
  text — measured at **0.225** on `prototype/examples/unified.mg` (1866 B →
  420 B) and 47 bytes for a transformer block — intended as the primary agent
  output target. This line read "~50× smaller"; no module in the repository
  gets near that, and the same figure had propagated into `AGENT_PROTOCOL.md`,
  the README and the blog post. Reproduce with `mage-parse --target=abl-bytes`,
  which prints the ratio.

The "~50 % text reduction" claim is dropped or qualified.

### Path B: Re-engineer the text surface to actually deliver
This is real work — not just rewriting the corpus, but **expanding the
sigil set across expression syntax** so the gains apply uniformly.
Concrete moves:
- Single-byte sigils for `return`, `match`, `if`/`else`, `let`/`mut`
  that the lexer treats as 1 token AND that are shorter on the wire.
- A whitespace policy stricter than Rust's (no blank lines between
  declarations by default; minimal indentation).
- Token-count budgets enforced at compile time via the existing
  `--token-report` flag — fail builds that exceed a budget.

After surface changes, re-run `token-bench`; target is real 25-30 %.

### Path C: Make Agentic Binary Language the canonical agent target
Stop pretending the text surface is what agents emit. Add:
- `--target=abl-bytes` flag where agents emit binary Agentic Binary Language directly,
  text `.mg` only generated for human review.
- The decompiler (already exists from Phase 4) provides the
  human-readable view on demand.
- All cost / token / efficiency claims shift to Agentic Binary Language byte counts,
  which **already deliver** the ~50× reduction.

Of the three, Path C is the cleanest match to the project's stated
mission. It also leverages the unification work directly.

## How the numbers were produced

`prototype/src/bin/token_bench.rs` — single binary, runs in ~2 s:

```sh
cargo run --manifest-path prototype/Cargo.toml --bin token-bench
```

For every task in `benchmarks/tasks/*.json`:
1. Count raw source bytes for both languages.
2. Count whitespace-stripped bytes.
3. Tokenise MAGE via `prototype::lexer` (atomic sigils).
4. Tokenise Rust via `proc_macro2::TokenStream::from_str`
   (group delimiters count as 2).
5. Tokenise both with a shared naive rule for cross-check.
6. Compare every measurement to the corpus's claimed `token_count`.
7. Emit [`benchmarks/TOKEN_REPORT.md`](TOKEN_REPORT.md) +
   exit non-zero if any claim is off by > 10 %.

Output is reproducible. Two things to know before treating it as a gate:

- **It exits non-zero on a clean tree.** Step 6 compares each task's *claimed*
  `token_count` against the measurement, and 150 claims across the 100 tasks
  are off by more than 10 % — which is the subject of §1 above. The corpus's
  claims being wrong is the finding, not a regression, so the exit status
  cannot be used as-is.
- **Nothing ran it.** It was not in CI, `scripts/test-all.sh`, or any check
  script, and `benchmarks/TOKEN_REPORT.md` had gone stale in two categories
  while `scripts/check-ci-floors.sh` enforced the published token-ratio
  ceiling by *reading that stale file*. The floor check now runs the bench and
  fails if the committed report differs from a fresh one.

## Files added by Phase 27

- `prototype/src/bin/token_bench.rs` (~500 lines)
- `prototype/Cargo.toml` — `[[bin]] token-bench` + `proc-macro2` dep
- `benchmarks/TOKEN_REPORT.md` — auto-generated full report

---

# § 2 — Reliability Finding (Phase 30, 2026-05-22)

## TL;DR

`reliability-bench` walks every task in `benchmarks/tasks/*.json`,
asks a **candidate agent** for MAGE source, and runs it through
the prototype lexer + LL(1) parser. The default backend is a **file
oracle** that returns the corpus's own `solution.rdx_source` —
i.e. simulates a perfectly capable agent.

The numbers it produced are the **upper bound** on reliability of
any agent driving today's prototype:

| Stage | Pass | Total | Rate |
|---|---:|---:|---:|
| Lex (no error tokens) | 99 | 100 | 99.0 % |
| **Parse (LL(1) accepts)** | **25** | **100** | **25.0 %** |

**Three quarters of the corpus's own reference solutions don't
parse through today's MAGE prototype parser.** No real LLM can
exceed this ceiling — the corpus is the upper bound.

> **Superseded.** That was Phase 30. §3 and the phases after it closed the
> gap: the same command now reports **lex 100/100, parse 99/100, effective
> 100/100**. The per-category table below, with seven categories at 0.0 %,
> is history — every one of them parses today. `scripts/check-ci-floors.sh`
> enforces a floor of 98 on that number so it cannot silently regress.

## Per-category breakdown

| Category | Tasks | Parse OK | Rate |
|---|---:|---:|---:|
| basic-io | 10 | 10 | **100.0 %** |
| algorithms | 15 | 12 | 80.0 % |
| data-structures | 15 | 2 | 13.3 % |
| web-network | 10 | 1 | 10.0 % |
| agent-orchestration | 10 | 0 | 0.0 % |
| concurrency | 10 | 0 | 0.0 % |
| error-handling | 5 | 0 | 0.0 % |
| full-applications | 10 | 0 | 0.0 % |
| generics-traits | 5 | 0 | 0.0 % |
| systems | 10 | 0 | 0.0 % |

Simple imperative code parses. Anything touching effects,
capabilities, sum types, casts, or pointer-like types fails.

## Root causes (parser gaps the corpus exposes)

Inspecting the 75 failures by error message clusters:

| Pattern (count varies) | Example error | Fix |
|---|---|---|
| Effect annotations in signatures | `agent-001`: `expected LBrace, found Slash '/'` | Parser must accept `/ io`, `/ net + io` between return type and body |
| Sum-type constructor sugar | `algo-003`: `expected expression, found KwNone 'None'` | Allow bare `Some(x)` / `None` in expression position, not just `?Some` / `?None` |
| `as` casts | `algo-006`: `expected Semi, found Ident 'as'` | Add `as` as a binary cast operator in expression parser |
| Local `let mut` keyword | `algo-008`: `expected expression, found KwM 'm'` | Allow `m` in statement position inside blocks (already in agent mode?) |
| `async fn` declarations | `conc-001`: `expected identifier, found KwAf 'async'` | Accept `async` keyword equivalently to `af` sigil |
| `@T` (Arc) type prefix | `conc-002`: `expected type, found At '@'` | Add `@T` to type parser as Arc prefix per README |
| `+S`/`+E`/`+T` in nested positions | `agent-004`: `expected type, found KwF` | Some prefixes still aren't accepted everywhere they should be |
| Capability block syntax | `agent-002`: `expected Colon, found LBrace '{'` | Parser doesn't yet accept `caps { ... }` blocks inside `agent Foo { ... }` |

These are **prioritized**: a parser PR addressing the top 3 (effects,
sum-type sugar, `as` casts) would likely push the pass rate from
25 % to ~60-70 %.

## What this means for the mission

The previously claimed "maximally reliable for agents" is **not
supported by measurement** in two ways:

1. The reference solutions written by the corpus authors don't
   compile through the parser they were targeting.
2. Therefore any LLM-driven agent will inherit at least that same
   75 % failure rate, plus whatever the LLM adds on top.

## What's needed to actually deliver

1. **Implement the parser gaps listed above.** Concrete, well-scoped
   PRs targeting effect annotations, sum-type sugar, `as` casts,
   `async`, `@T`. Each one raises the bench number by a measurable
   amount.
2. **Re-run `reliability-bench` after every parser change.** It's
   fast (~ms / task), runs in CI, exits non-zero on any failure ⇒
   regression guard for free.
3. **Plug a real LLM into the `CandidateAgent` trait** once the
   reference solutions parse. Then the bench measures actual
   agent-quality, not corpus-parser alignment.
4. **Use the self-healing pipeline (Phase B / step 34) to close
   the gap** for any remaining failures. The compiler has 17 error
   patterns it can repair; measure how many of the parser-rejected
   tasks heal-and-retry into acceptance.

## Files added by Phase 30

- `prototype/src/bin/reliability_bench.rs` (~280 lines)
- `prototype/Cargo.toml` — `[[bin]] reliability-bench` entry
- `benchmarks/RELIABILITY_REPORT.md` — auto-generated, regression-guarded

---

# § 3 — Parser Patches Driven by the Bench (Phase 31, 2026-05-22)

Four targeted parser patches moved the corpus parse rate
**25 → 29**, each one verified end-to-end by re-running the bench:

| # | Patch | Bench Δ |
|---|---|---:|
| 1 | Effect annotations in signatures (`/ io`, `/ llm, tools, io`) | +1 |
| 2 | `Ok` / `Err` / `Some` / `None` accepted as identifiers in path / pattern contexts | +2 |
| 3 | `expr as Type` cast operator | +1 |
| 4 | `@T` Arc prefix in type position | 0* |

*Patch 4 didn't unblock any task by itself but advanced multiple
tasks to deeper line numbers, where they then hit the next blocker.
The bench shows incremental progress even when the count is flat.

## What's still blocking the corpus

The remaining 71 failures cluster around three shapes (in priority
order by frequency):

1. **`expected Colon, found LBrace '{'`** — struct-literal syntax
   variations. Notably `@Foo { field: value }` is meant as
   `Arc::new(Foo {…})` in expression position but conflicts with the
   new `@T` type arm. Needs an LL(1) re-design or a lookahead hack.
2. **`expected expression, found KwV 'v'`** / **`KwM 'm'`** — local
   `let` / `let mut` in statement position inside blocks. The
   declaration grammar accepts them at top-level but the statement
   grammar inside `{ … }` rejects them.
3. **`expected type, found KwF`** — `f` used in function-type
   position (e.g. `Box<dyn f(i32) -> i32>`). The lexer reserves `f`
   as a keyword and the type grammar has no arm for it as a
   fn-type opener.

Each is a real grammar gap; each is fixable in a small patch; each
will visibly move the bench number when fixed.

## Files modified by Phase 31

- `prototype/src/parser.rs` — four targeted patches (effect parsing,
  sum-type ident fallback, `as` cast, `@T` Arc type)
- `benchmarks/RELIABILITY_REPORT.md` — regenerated

## Phase 32 — four more patches (29 → 32)

| # | Patch | Bench Δ |
|---|---|---:|
| 1 | `f(T)->R` / `af(T)->R` / `uf(T)->R` function types | +1 |
| 2 | `?` disambiguates match-vs-if via `is_match_arm_body` lookahead | 0 (deeper failures) |
| 3 | Multi-segment path patterns (`Color.Red`, `R.Ok(x)`) | +2 |
| 4 | `async` / `unsafe` as identifiers in non-fn positions | 0 (deeper failures) |

The lookahead helper `is_match_arm_body` (in `parser.rs`) scans the
next `{ … }` for a `=>` at brace-depth 1 before any `;`. Cheap
because match-arm bodies are usually shallow.

## Phase 33 — four more patches (32 → 43, **+11 tasks**)

The same technique cleared the struct-literal cluster:

| # | Patch | Bench Δ |
|---|---|---:|
| 1 | `@TypeName { … }` Arc-struct-literal vs `@` for-loop disambig | **+5** |
| 2 | `&self` / `&!self` / `self` method-receiver desugar | **+6** |
| 3 | `val` / `var` as identifiers (parameter names, args) | 0 (deeper) |
| 4 | `Some` / `None` / `Ok` / `Err` in expression-prefix | 0 (deeper) |

**12 patches across Phases 31-33, +18 tasks, +72 % improvement.**
The bench-driven loop produces consistent results: ~3-minute
iterations, each verified, all 831 prototype tests still pass.

Remaining clusters (~57 failures) shift toward grammar variations
specific to declaration shapes (trait bodies, where-clauses,
ranges in indexing). Each is its own scoped patch.

## Phase 34 — three more patches (43 → 51, **crossed 50 %**)

| # | Patch | Bench Δ |
|---|---|---:|
| 1 | KwVal/KwVar/KwAf/KwUf/KwData accepted in Ident expr arm | **+6** |
| 2 | `arr[a..b]` / `arr[a..=b]` range slicing inside index brackets | 0 (deeper) |
| 3 | `parse_closure_param_list` accepts untyped lambda params | **+2** |

**Cumulative across P31–P34: 25 → 51, +104 % over 15 patches.**
Parse rate more than doubled. The bench-driven loop continues to
produce verified deltas per ~3-minute iteration.

## Phase 35 — three patches (51 → 69, **+18 in one phase**)

| # | Patch | Bench Δ |
|---|---|---:|
| 1 | Open-ended range slicing `arr[a..]` / `arr[..b]` | +2 |
| 2 | `?Some(x)` / `?None` / `?Ok(v)` / `?Err(e)` sugar (expr + pattern) | **+12** |
| 3 | KwYield / KwRule / KwQuery / KwSelect as identifiers | **+4** |

**Cumulative across P31–P35: 25 → 69, +176 % over 18 patches.**
Parse rate nearly tripled. The single `?Sum-type` patch was the
biggest win since Phase 33's `@`-disambig — corpus uses this sugar
pervasively in both expression and pattern positions. All 831
prototype tests still pass.

# § 4 — Self-Heal Coverage (Phase 36, 2026-05-22)

Wired the existing self-healing pipeline (`prototype/src/heal.rs`,
17 error patterns from Phase B step 34) into `reliability-bench` so
every parse failure now also runs through `heal::heal_one`, gets the
top fix's `TextEdit`s applied, and is re-parsed.

```
self-heal:  proposed 2/31 failures (6.5%)
            succeeded 0/31 (0.0% of failures)
```

**The self-healer's recognisers target downstream errors — borrow,
type, contract, capability, perf-budget — not parse-grammar gaps.**
Parse failures rarely match its pattern library, and when they do
the proposed fix is wrong for the actual grammar issue.

This is a real, useful, honest finding. The project advertised
self-healing as the agent-reliability story; the bench shows it
doesn't apply to the most common failure mode today.

## Three paths forward (Phase 37+)

1. **Keep the parser loop**: 31 remaining failures, ~2-3 tasks per
   phase. Steady but slow.
2. **Extend heal with parse-error recognisers**: add patterns for
   missing `;`, unbalanced `{ }`, malformed effect clauses, struct-
   literal vs unit declaration shapes. Closes the heal-coverage gap
   the bench just exposed.
3. **Plug a real LLM into `CandidateAgent`**: the harness is ready;
   only the backend wrapper is missing. Measures real-world
   reliability on top of corpus-perfect input.

`effective_pass = parse_ok + heal_succeeded` is now recorded so
future heal improvements are measured in the same units.

# § 5 — Heal Coverage Extension (Phase 37, 2026-05-22)

Six new patterns added to `heal::builtin_patterns` targeting the
bench's actual parse-error message shapes:

```
parse-missing-semi          expected Semi, found …      → insert `;`
parse-missing-rbrace        expected RBrace, found …    → insert `}`
parse-missing-rparen        expected RParen, found …    → insert `)`
parse-missing-rbrack        expected RBrack, found …    → insert `]`
parse-missing-type-colon    expected Colon, found RParen → insert `: _`
parse-empty-block           expected LBrace, found Semi  → replace `;` with `{ }`
```

### Bench delta

```
Phase 36 (heal wired, no patterns):
   self-heal: proposed 2/31 (6.5%)  succeeded 0/31 (0.0%)

Phase 37 (+6 patterns):
   self-heal: proposed 5/31 (16.1%) succeeded 1/31 (3.2%)
   Effective pass rate: 70 / 100
```

Coverage 2.5× and the first successful heal-recovery is on the
board. The proposed-vs-succeeded ratio (5 → 1) tells us what's
next: most remaining failures need multi-token structural repair,
not single-token insertion. Future patterns can be structural
without changing the harness — the `TextEdit` slice already
supports arbitrary text and ranges.

## Where the project stands now (all three findings together)

| Mission claim (original) | Measured reality | Delivered fix |
|---|---|---|
| "~50 % text token reduction vs Rust" | ~tied | Re-framed README; Agentic Binary Language bytes are the actual win |
| "Maximally reliable for agents (compiler accepts all corpus)" | 25 / 100 | 18 parser patches → 69 / 100; +176 % |
| "Self-healing repairs agent errors" | 0 / 31 parse failures healed | 6 parse-error patterns → 1 / 31 + foundation for more |

Every overclaim has a number; every number has follow-up work
backing it. The bench is the discipline that keeps the project
honest.

# § 6 — Robustness Measurement (Phase 38, 2026-05-26)

The Phase-37 heal patterns only recovered 1/31 failures on the
file-oracle because the corpus's reference solutions either parse
cleanly or fail in shapes the new patterns don't match. **Real
LLMs don't write corpus-perfect output** — they write near-correct
output with small mistakes. So the heal-reach number from Phase 37
was honest but under-measured.

Phase 38 adds three pluggable backends to `reliability-bench` so
the same harness can measure heal-reach against realistic input:

| `--agent` | Behaviour |
|---|---|
| `file-oracle` *(default)* | echo corpus reference solution verbatim |
| `perturbed` | corpus + 1 of 5 deterministic mutations per task |
| `subprocess:<cmd>` | spawn external command per task — stdin → stdout |

The mutations are intentionally small: drop the last `;` / `}` /
`)`, swap `let`↔`mut`, insert a stray `,` after the first `{`.
Each task gets one, deterministically chosen from the task id.

## The numbers

```
file-oracle:  parse 69/100  heal 1/31 (3.2%)   effective 70%
perturbed:    parse 26/100  heal 13/74 (17.6%) effective 39%
```

> **Superseded.** Phase-38 numbers. Re-run 2026-08-18:
> `file-oracle: parse 99/100, effective 100/100` and
> `perturbed: parse 27/100, pattern-heal 42/73 (57.5%), effective 70/100`.
> The heal-reach on perturbed input went 13 → 42, and the gap between
> perfect and perturbed input closed from 31 points to 30 — the *parse*
> side improved, not the tolerance side. Perturbed input is still where
> the remaining reliability work is.

**13× more heal-recoveries on perturbed input (1 → 13).** The
patterns that looked like they didn't do anything on corpus
input are doing real work when the input matches their target —
near-correct agent-shaped mistakes.

The 31-percentage-point gap between perfect (70 %) and
perturbed (39 %) is the size of the LLM-error-tolerance gap.
Heal closes ~13 pp of it. That's the first honest number for
"self-healing improves agent reliability."

## The subprocess backend = ready for any LLM

The harness now has a clean integration point:

```sh
cargo run --bin reliability-bench --manifest-path prototype/Cargo.toml \
    -- --agent subprocess:./your_llm_wrapper.sh
```

The wrapper script reads the task description on stdin and prints
MAGE source on stdout. Non-zero exit = "agent refused: <stderr>".
No credentials or network code in the bench itself.

## All six findings, one table

Two columns: what the phase measured, and what the same command reports
today (2026-08-18). Where they differ, the later number is the one that
holds — the earlier one is what the finding was written about.

| Finding | Tool | At the time | Now |
|---|---|---|---|
| Text token reduction vs Rust | `token-bench` | ~tied (claim was 50 %) | ~tied — native-lexer ratio **0.997** |
| Binary IR efficiency | `token-bench` + manual | "83 % smaller than text" | **2-6×** per declaration (243→47, 98→17, 116→68 B); 0.225 for a whole container |
| Corpus parse-rate | `reliability-bench` | 25 → 69 / 100 | **99 / 100**, floor-gated at 98 |
| Self-heal coverage on corpus | `reliability-bench` (P36) | 0 % healed | 0/1 — only one task fails, and structural-heal takes it |
| Self-heal coverage on perturbed | `reliability-bench` (P38) | **17.6 % healed** | **57.5 %** (42 of 73) |
| Effective pass under near-correct input | `reliability-bench` (P38) | **39 / 100** | **70 / 100** — still the honest agent-reliability headline, and still the gap worth closing |

The "83 %" in row two was never measured against anything: the container
ratio on `prototype/examples/unified.mg` is 0.225, and per-declaration the
range is 0.17-0.59. It had propagated to `AGENT_PROTOCOL.md` (as "~50×"),
the README and the blog post before anyone ran the numbers.
