# Handoff — through 2026-08-14

What this repository is, what state it is in, and what to do next. Written for
someone picking it up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,854** — rmi 1,380 · prototype 1,146 · ribosome 164 · germline 112 · forge 52 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm |
| CI | 10 jobs, green on `master` |
| Examples | 12 of 12 typecheck, run, and print their recorded answer |
| `.mg` sources | 94 checked, 32 listed sketches |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/check-examples.sh                 # the 12 shipped examples, end to end
scripts/check-mg-sources.sh               # every .mg file in the repo
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

**There are 14 unpushed commits on `handoff`.** Nothing is pushed and no PR is
open. That is a decision waiting, not an oversight — open item 0.

---

## The one thing to understand

Four sessions have now found roughly forty bugs in this repository, and **not
one was found by reading code**. Every one came from running something and
comparing the result against what a document, a comment, or a test claimed.

The code has consistently been in better shape than the claims about it. The
claims are what keep turning out to be wrong — and because this is a language
built to be *generated and consumed by agents*, a wrong claim is not a
documentation defect. It is a supply of confidently incorrect programs.

[Failure taxonomy](#failure-taxonomy) is the useful part of this document. It
groups what has gone wrong by *shape*, because the shapes repeat, and knowing
them tells you where to look next.

---

## The instruments now in place

These exist because the same failure kept recurring: a claim written once, never
revisited, and quietly wrong. Each fails in **both** directions — a claim that
breaks, *and* a recorded state that silently starts passing.

| Command | Checks |
|---|---|
| `scripts/test-all.sh --check-docs` | every documented test count against the run that just produced them; `--cuda --bench` adds the GPU and benchmark figures |
| `scripts/check-examples.sh` | that all 12 examples typecheck, evaluate, **and print the recorded answer** |
| `scripts/check-mg-sources.sh` | every `.mg` file in the repo typechecks, or is a listed sketch with a stated reason |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

Plus, in the suite: every keyword introduces the token it claims, every
documented type can be written in a signature, every published control sigil
parses, every published path exists, every capability namespace performs its
effect, every layer surface name compiles.

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** The one figure that stayed
stale after the checker existed was one nobody had listed.

### Two things the instruments taught, the hard way

**A pin guarantees agreement, not truth.** CI checked `MAGE_ONTOLOGY.json`
byte-for-byte against a fresh `--emit-ontology` — and thereby guaranteed that a
*wrong* answer stayed identical. Four of the ten effect names it published were
rejected by the compiler. The check was working perfectly and proving nothing.
What was missing crosses the boundary: not "does the file match its generator"
but "does every name the file publishes actually work".

**A pin over a generated list tends toward tautology.** The follow-up test for
layer names iterated `layer_map`, which is *filtered* by the same function the
validation uses — so it could not fail by construction. The escape is inputs the
generator has never seen. Catching `Lienar` requires a name no list contains.

---

## Open items

Grouped by what would actually unblock them, because "open" has meant three
different things.

### Waiting on a decision, not on work

| # | Item | The decision |
|---|---|---|
| 0 | **14 unpushed commits on `handoff`** | Push the branch, or open a PR against `master`. Note `gh pr merge --auto` merges *immediately* here — the repo has no required status checks — which is how PR #4 landed with CI still pending. |
| 1 | **Module system, or a decision that there is none** | The blocker under `stdlib/`. `resolve_use` parses a path and discards it, so nothing can be imported; the library surface is global instead. That may well be *right* for a token-efficient agentic language — an import costs tokens and buys nothing when the library is small and fixed — in which case say so in the spec and delete `stdlib/`. Everything about a standard library is downstream of this. See `stdlib/README.md`. |
| 2 | GPU CI runner | Correctness **is** verified on the hardware here and recorded. What is missing is a self-hosted runner so `cuda-gpu` runs unattended — an account action, declined once already. |
| 3 | TLS trust posture | The transport seam and a `rustls` implementation exist behind `--features tls`. Pinned self-signed / mutual TLS / public PKI is deliberately the operator's; `acceptor`/`connector` take your config. |
| 4 | RAP error shape | An unknown method returns `{"result":{"error":…}}` — an HTTP-200-shaped success containing an error, not a JSON-RPC `error` member. Fixing it is a client-visible wire change. |

### Deliberate, and not defects

| # | Item | Why it stays |
|---|---|---|
| 5 | `rmi`'s 2 clippy warnings | Vendored; must stay syncable against its own upstream. |
| 6 | Ab-initio migration steps 2c & 3 | Declined as negative-sum (ROADMAP step 99). Revisit only with new measurements. |
| 7 | Single-workspace build | The crates are separate workspaces by design; `rmi` must stay independent. |
| 8 | External dependency resolution | Fetching third-party code is a distinct trust problem — provenance, pinning, revocation. Folding it into the planner is how build systems become unauditable. |
| 9 | `json`, `kb`, `db` attribute no effect | No built-in kind names a store, and inventing a `Custom` would infer an effect that §11.4 then refuses in an annotation — leaving no way to declare what you perform. Declare `effect Db { … }`; `examples/effects-showcase` does. |

### Real work, unstarted

| # | Item | Size |
|---|---|---|
| 10 | **`resume` for effect handlers** | Largest. Handlers dispatch and return like ordinary calls; nothing captures a continuation, so there are no generators, no backtracking, and no async from the same mechanism. Needs the tree-walking evaluator reworked into a form that can capture continuations, which touches every expression form. |
| 11 | **Generic calls do not typecheck** | `f identity[T](v: T) -> T` declares fine and `identity(1)` *evaluates* to `1`, but the checker reports `type mismatch: I32 vs sym1`: the type variable is never instantiated at the call site. `prototype/examples/analysis.mg` keeps a declared-but-uncalled generic with a comment pointing here. |
| 12 | `int` literal constraint | Medium. The current fix is a post-hoc check in `default_int_literals`, not a real integer-kind constraint threaded through `unify`. Correct for the programs it rejects; the principled version is larger. |
| 13 | One sketch example file | `prototype/examples/spec_synthesis.mg`. Five of six have been rewritten and produced **eleven** compiler bugs between them; the rate has not dropped. |

### Small, sharp, cheap

- **`guard` cannot be *referenced*, only bound.** `v guard = 2` binds fine, but
  `guard + 1` on its own line starts a guard statement and dies with
  `expected expression, found Plus '+'`. Same family as the keyword-collision
  class below.
- **`unknown operator: `+=`` is a lie.** The operator is fine; the *operand
  type* is unknown, which happens with untyped parameters (`f sumto(n){ … }`).
  The diagnostic names the wrong thing, exactly as the sigil-letter one used to.
  See `benchmarks/cross_lang/tasks.mg`.
- **`scan` emits its seed** as the first element, so its result is one longer
  than its input. The two conventions differ by exactly one line, which is
  invisible until you count. Worth a doc comment at the definition.
- **`pub` on a `data` field is a parse error** (`data Point(pub x: f64)`).
  Undocumented either way; decide whether fields have visibility at all.

---

## Failure taxonomy

Every bug found so far falls into one of six shapes, ordered worst first. The
worst are the ones that produce a clean `--check`.

### 1. Silently wrong answer

Compiles, runs, returns the wrong number. No error anywhere.

- **A bare unit-variant pattern bound instead of testing.**
  `?= s { Circle => 0, Square => 4, Triangle => 3 }` bound `s` to a fresh
  variable named `Circle` and took the first arm — every time, for every input.
  `sides(Square)` returned `0`.
- **An unknown net layer lowered to `Op::IDENTITY`.** `layer b: Lienar(128, 64)`
  — a typo of `Linear` — became a pass-through, and the net compiled, lowered
  and ran with that layer doing nothing. The translator had recorded it in
  `unknown_layers` all along; the only readers were the ABL-lowering path and a
  `train` warning. **Detected, recorded, and never surfaced is
  indistinguishable from not detected.**
- **An enum variant pattern matched nothing**, so a `match` quietly took the
  wildcard arm.
- **`guard cond else { … }` fell through**, running the body with the
  precondition false: `a(-5)` answered `-10`.

*How to find more:* run programs and check the **answer**, not the exit status.
That is why `check-examples.sh` pins printed output.

### 2. Typechecks, then does not evaluate

`--check` clean; `--eval` dies. `--check` cannot find these.

- `data` sum-type variants — resolution registered the type name and no variants
- bare variant constructors (`Rect(3.0, 4.0)` vs `Shape.Rect(3.0, 4.0)`)
- **`println`, `print`, `eprint`, `eprintln`** — registered as builtins, typed,
  attributed `IO`, and the evaluator had no arm for any of them. The most common
  function in the language. It survived because *no shipped example calls it*:
  the pin records each example's returned value, and all twelve return theirs
  rather than printing. **A pin covers only what it exercises.**
- `Ok`/`Err`, effect operations, and `impl`/`extend`/trait methods

### 2b. Evaluates, then does not typecheck

The inverse, and rarer — the checker rejects a working program.

- **`x |> f(a)` did not typecheck.** The pipeline operator is in the spec four
  times: prose, a token definition (`PIPE = '|>'`), a grammar rule
  (`pipe_expr = expression '|>' expression`), and a worked example. The lexer
  emits the token, the parser builds an `Expr::Pipeline`, and the *evaluator*
  desugars it correctly to `f(x, a)` — but the checker inferred the two sides
  independently, so `10 |> add(5)` was checked as the standalone call `add(5)`
  and reported `expected 2 argument(s), found 1` while evaluating to `15`.
  Every program using the documented operator ran correctly and failed
  `--check`.

The lesson is the same as its mirror image, from the other side: **`--check`
and `--eval` are two different oracles, and agreeing with one says nothing
about the other.** Run both.

### 3. Accepted and silently discarded

The parser takes it; nothing downstream honours it. The failure surfaces
elsewhere, pointing at the wrong line.

- **`use` brings nothing into scope.** `u totally.made.up.path` was accepted;
  `u std.io.read_to_string` then `read_to_string("x")` gave `unresolved name` at
  the *call site* — the one line the author wrote correctly. Now warns.
- **Default arguments were parsed and discarded.** `Param::default` was in the
  AST, with a parser test asserting it was *stored*, and read by nothing. Now
  honoured.
- **Capability handles performed no effect.** `resolve.rs` registered twenty
  namespaces and said their "use is tracked by the effect system". It was not: a
  `pub` function declared pure could call `net.connect(…)` or `llm.generate(…)`
  and check clean, while the bare `println(…)` beside it was caught. **The gate
  was open at exactly the seam the language documents as the way through it** —
  the safe-looking code was the unchecked code.

### 4. Documented but unimplemented

- **`|x| expr` closures** — in the spec's formal grammar *and* its feature list,
  and a parse error. The vocabulary is built on higher-order functions, so the
  documented way to pass one to `map`/`filter`/`fold` was the one that failed.
- **`!` as break** — the spec names it, and the lexer's own comment reads
  `KwBreak, // break (legacy — canonical is !)`. The canonical spelling parsed
  nowhere; the "legacy" one was the only one that worked.
- **`/ agent`** — a documented effect that was a parse error *and* not a
  built-in kind: two independent failures on one row of one table.
- **§11.2's operations column** — of 41 operations named, 22 perform nothing.
- **`internals/05`** — `is_sub_effect`, `Forge.toml` capability grants, effect
  polymorphism (`/ *`), `E0401`/`W0410` diagnostics: none exist. Marked rather
  than rewritten, because whether to build them is a design decision.
- **`internals/03` §3.2** — four `use`-resolution steps and six import styles,
  none of which happen.

### 5. The document is wrong, not the compiler

The mirror image, and easy to get backwards.

- **`^` as Return.** One ontology line claimed it; the spec names `ret` in both
  sigil tables and mentions `^` exactly once, as bitwise XOR — and `^` is already
  the `^T` Box prefix. **I implemented `^`-as-return first**, to make the
  published claim true, then had to add a special-case newline guard when
  `m x = 7` / `^ x` parsed as `7 ^ x`. That guard was the signal. Reverted.
- **`keywords.introduces`** was two fields wearing one name: the real token kind
  for 83 keywords, a hand-written label for 19 — `agent` claimed `AgentDef`,
  `val`/`var` claimed `Let`, a token this language *removed*.
- **`types`**: `S` published as shorthand for `String` (it is the `struct`
  keyword and can never be a type); `Map[K,V]` and `Set[T]` are `{K: V}`, `{T}`.

**The rule:** the ontology *describes* the language; it does not define it. When
a generated artifact and the spec disagree, the artifact is the likelier
suspect. Reaching for a special-case parser guard to support a one-character
spelling means the spelling is wrong, not the parser.

### 6. Parse ambiguity, and diagnostics naming the wrong token

- **A newline did not end a statement before `(` or `[`.** `v a = 1` followed by
  `(2 + 3)` called the literal `1`. Reported as "a `(` after a block", which was
  one instance of a general rule.
- **Keyword-as-identifier, five positions.** `agent`, `swarm`, `kb` collided in
  effect annotations, expression position, `expect_ident`, module declarations
  and use paths. **Patched per-position three times before generalising** — each
  patch fixed only where someone had hit it and left the rest broken.
- **The prelude reserved eighty words globally.** `M net { }` reported
  `duplicate definition: net` against a definition the author never wrote and
  could not see. Source definitions now shadow prelude names.
- **The sigil-letter diagnostic blamed the binding keyword.** `var v = 3` gave
  `unresolved name: `var`` — the statement was never recognised as a binding, so
  the keyword fell through to expression position and the error landed on the
  one token written correctly.

---

## What the compiler actually is

Corrected facts, each verified by running it. Several documents said otherwise.

**The effect system is the security boundary.** There are 17 built-in kinds
(§11.2). A function acquires one three ways: by annotation, through a capability
handle (`io.println(…)` — the receiver names the capability), or by calling a
recognised builtin by bare name. A `pub` function must declare what it performs;
a private one infers silently and its effects reach its public callers.
Annotation is an **upper bound** — over-declaring is deliberately allowed and
never warned about. `handle … with` discharges an effect **per block, not per
function**, and a handler's own effects are attributed honestly. **Handlers do
not resume** (item 10).

What it does *not* have: no effect hierarchy (`/ io` grants nothing else), no
per-file or per-module capability grants, no effect polymorphism.

**There is no module system.** `use` parses and is discarded; it now warns. The
library surface is **global** — 31 vocabulary combinators
(`resolve::VOCABULARY`), 20 capability namespaces (`hir::CAPABILITY_NAMESPACES`)
and the builtin functions are in scope everywhere, with no import and no tokens
spent on one. For a language optimising for token efficiency that is plausibly
right rather than a gap, but it was written down nowhere — and it is what makes
a hand-written `stdlib/` unreachable by construction.

**36 of 126 `.mg` files were not MAGE.** `stdlib/` is 25 files, 4,402 lines, and
all of it Rust — read by nothing, checked by nothing, and resolvable-around
because imports are nominal. It now carries a `README.md` saying what it is and
the ordered prerequisites for making it real. `check-mg-sources.sh` is the
consumer it never had; its sketch list only shrinks, and is down from 36 to 32.

---

## Traps, so you do not repeat them

- **`cmd | grep -q` under `set -o pipefail`.** `grep -q` exits at the first
  match, the writer takes SIGPIPE, and the pipeline reports failure — so every
  match reads as no-match. Cost two separate bugs, the second *despite a comment
  about it in the first file*. Capture the output, then match.
- **A tool's output means nothing until you know what you pointed it at.** A
  probe loop reported `ok` for every case because the shell was already inside
  `prototype/`, so `./prototype/target/release/mage-parse` did not exist and the
  missing-binary message matched no error pattern.
- **`bash` from PowerShell is WSL's**, which cannot open a `C:/…` path; a Windows
  path with backslashes gets read as escapes. Prefer Git's bash and repo-relative
  paths.
- **Shell mode bits do not survive a Windows checkout.** `chmod +x` locally is
  not enough; use `git update-index --chmod=+x`, and invoke via `bash` anyway.
- **MAGE is not Rust.** Struct literals are `@P { x: 1 }`, not `P { x: 1 }`
  (which parses as a *map*). Booleans are `1b`/`0b`. `[T]~` is a type suffix, not
  a value one. Receivers are `self`, not `&self`.
- **`remove_dir_all` then `create_dir_all` on a fixed path is unsound on
  Windows.** Deletion returns while the directory is still open somewhere,
  leaving it delete-pending: it exists, cannot be opened, cannot be recreated,
  and `os error 183` points at the creation rather than the deletion that caused
  it.
- **Tight timeouts are flaky under whole-suite load.** Both flaky tests found so
  far were invisible to CI and visible only locally, and both were found by
  running the **whole** suite rather than one crate.
- **`gh pr merge --auto` merges immediately** when the repo has no required
  status checks. It does not queue and does not warn.
- **Regenerate the ontology from a *rebuilt* binary.** One commit shipped a stale
  `MAGE_ONTOLOGY.json` because `--emit-ontology` ran from a binary built before
  the change. `cargo test` builds the test harness, not `mage-parse`.

---

## The pattern worth carrying forward

Every bug was found by **running** something. Not one came from reading the
compiler. To find the next one, pick a surface nobody has run, and run it.

Four counter-lessons, each of which cost real time:

**A comment agreeing with your expectation is not evidence.** It is usually what
the author *meant*. Six times now a comment stated the correct rule while the
code beside it did something weaker — "No else → must be unit"; "unifies with any
int width"; "Return the struct type"; the `guard` evaluator admitting its own
fall-through; `resolve.rs` claiming capability handles were "tracked by the
effect system"; and a parser comment saying `!`-as-break was "handled via context
in statement parsing" when the only other `Bang` arm was the `!` *type*.

**Reading is not verification, including when you wrote it.** An earlier draft
claimed a nested `"` inside an f-string ends it early, and three examples were
written around the workaround. It is false. The claim came from a parse error
whose real cause was never isolated, and it was written *into a document about
not doing that*.

**Fixing a class of bug confers no immunity to committing another instance.** The
misspelled-operation bug was written into the first version of the feature built
specifically to eliminate that class, and shipped past twelve new tests, a green
CI run, and the output pin.

**Verify a test by breaking the fix.** Two tests this session passed against a
build with the fix deleted. One asserted statement counts when the thing that
changes is the *tail expression*; the other called `types::check` when the
validation lives in `abl_shape::check_module_shapes` — a different pass.
Verifying against the wrong entry point is a way to be green about nothing, and
it is invisible unless you break the thing on purpose and watch.

---

## Notes on the shape of the work

- Prototype tests **1,066 → 1,146**, all green — checked against the live run, so
  it tracks forward rather than freezing at the session that wrote it.
- Every typechecker fix has landed without breaking an existing test **except
  one**, where widening `collection_elem` made `sum("hi")` legal and
  `vocab_rejects_non_collection` caught it. That is the datapoint showing the
  suite has teeth rather than merely being large.
- **A bug report is an observation, not a specification.** Three of this
  session's starting items described the symptom correctly and the *rule*
  wrongly — "a `(` after a block" was really any newline; "`unresolved name:
  val`" was one of two different wrong errors. Reproduce before believing.
- `git log` is the real record. The commit messages carry the reasoning,
  including the false starts, the retractions, and the one case where a fix was
  implemented in the wrong direction and reverted.
