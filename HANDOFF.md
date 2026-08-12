# Handoff — 2026-08-04/05, updated 2026-08-11

What changed, what state it is in, and what to look at next. Written for someone
picking this up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

The 2026-08-11 session closed open item #1: all twelve examples now typecheck,
run, and are pinned to their output. See "The example rewrite" below.

| | |
|---|---|
| Tests | **2,814** — rmi 1,380 · prototype 1,106 · ribosome 164 · germline 112 · forge 52 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm |
| CI | 10 jobs, green on `master` |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

---

## What happened, in one paragraph

The session began as a cleanup audit and turned into two distinct pieces of
work. The first built out the Ribosome build engine — multi-language support,
extraction into its own crate along with Germline, a CLI, optional TLS — and
repaired a CI pipeline that had been red since June. The second was accidental:
running documented commands rather than reading them found **six typechecker
bugs, one soundness hole, and five stale documented figures**. The repository's
code was in better shape than the claims about it.

---

## The instruments now in place

These exist because the same failure kept recurring: a claim written once,
never revisited, and quietly wrong. Each fails in **both** directions — a claim
that breaks *and* a known-bad entry that starts passing without being pruned.

| Command | Checks |
|---|---|
| `scripts/test-all.sh --check-docs` | every documented test count against the run that just produced them; it prints how many it checked, and `--cuda --bench` adds the GPU and benchmark figures |
| `scripts/check-examples.sh` | that all 12 examples typecheck, evaluate, **and print the recorded answer** |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** The one figure that stayed
stale after the checker existed was one nobody had listed.

---

## Open items

| # | Item | State |
|---|---|---|
| ~~1~~ | ~~**10 of 12 examples do not typecheck**~~ | **Closed 2026-08-11.** All twelve typecheck, run, and are pinned to their printed output by `check-examples.sh`. See "The example rewrite" below — it cost eleven compiler and evaluator fixes. |
| 2 | GPU CI runner | Correctness is verified on the hardware here and recorded. What is missing is a self-hosted runner so `cuda-gpu` runs unattended — an account action, declined once already. |
| 3 | TLS trust posture | The transport seam and a `rustls` implementation exist behind `--features tls`. The posture (pinned self-signed / mutual TLS / public PKI) is deliberately the operator's; `acceptor`/`connector` take your config. |
| 4 | `rmi`'s 2 clippy warnings | Left alone on purpose: vendored, must stay syncable against its own upstream. |
| 5 | RAP error shape | An unknown method returns `{"result":{"error":…}}` — an HTTP-200-shaped success containing an error, not a JSON-RPC `error` member. Fixing it is a client-visible wire change, so it is a decision, not a cleanup. |
| 6 | `int` literal constraint | The fix is a post-hoc check in `default_int_literals`, not a real integer-kind constraint threaded through `unify`. Correct for the programs it rejects; the principled version is larger. |

---

## The example rewrite

Rewriting the examples was supposed to be a documentation task. It was not: the
examples exercised surfaces nothing else did, and **eleven compiler and evaluator
bugs** fell out. Every one was found by running `--check` or `--eval` on a small
probe, never by reading the compiler.

| # | Bug | What it cost |
|---|---|---|
| 1 | `effects.rs` matched builtin names *ahead of* `resolve::VOCABULARY` | the vocabulary's pure `join` was read as a thread join and typed `Async`; every caller of a documented pure function failed unless it declared `/ async` |
| 2 | `types.rs` unified array *lengths* in list literals | `[[1, 2], [3]]` rejected as ill-typed; `flatten` could flatten nothing but a rectangle |
| 3 | `Self` never bound in `impl`/`trait` bodies | no method with a receiver and no `-> Self` constructor could be written at all |
| 4 | `Self` still unbound in `extend` bodies | fix #3 covered two of the three body forms |
| 5 | enums had no runtime representation | `Mode.A` errored; **and a variant pattern matched nothing, so a `match` quietly took the wildcard arm** — a wrong answer, not an error |
| 6 | enum variants keyed by variant name alone | `Left { X }` and `Right { X }` evicted each other from the table |
| 7 | `Ok`/`Err` typechecked but did not evaluate | a program using `Result` was accepted in full, then died on `unknown function \`Ok\`` |
| 8 | the evaluator collected only free functions | every `impl`/`extend`/trait method typechecked and then failed with `unknown function` |
| 9 | **`impl Trait for Type` parsed the implementing type and discarded it** (`let _actual_type = …`) | `self_type` named the *trait*, so impls were filed under the trait name and no receiver could dispatch to them. Two types implementing one trait collided. |
| 10 | array lengths unified in `if`/`match` **branches** too | `? found { [x] } : { [] }` rejected as `array size mismatch: 1 vs 0` — return-a-result-or-nothing, the most ordinary shape in the language, was unwritable. Fix #2 had solved this for list literals only |
| 11 | **prefix operators bound tighter than the call postfix** | `!f(x)` parsed as `(!f)(x)`: it negated the *function* and called the result. Checked clean, died at run time with `value is not callable`. Same for `-f()`, `*p.field`, `&x[i]` |

Bugs 9 and 11 are the ones to learn from. Both survived because the broken
parse still produced *something*, and neither had a test that could tell the
difference: nothing had ever asserted which type an impl belonged to, and
nothing had ever applied a prefix operator to a call. In both cases **no test
noticed when the bug was fixed**. The regression tests now use two types
implementing one trait, and a `!` on a call — the shapes that expose them.

Six of the eleven — #5, #6, #7, #8, #9, #11 — are the same class: a bug that
**typechecks and then does not evaluate**. `--check` cannot find these. Only
`--eval` can, which is why the pin now runs it.

Prototype tests **1,066 → 1,106**, all green.

### And a twelfth, from this list itself

`guard cond else { … }` **fell through** unless the else block explicitly
returned. `guard n > 0 else { 0 }` did not produce `0` — it ran the body anyway
with the precondition false, and `a(-5)` answered `-10`. The evaluator's own
comment admitted it ("A non-diverging else falls through"), which is the third
time in two sessions that a comment stated the rule while the code beside it did
something weaker.

A non-diverging else is now a check-time error, as Rust's `let`-else requires.
Surveyed first rather than assumed: every `guard` in the repository — two
examples, four `prototype/examples/*.mg`, two parser tests — already returns
explicitly, so nothing had come to depend on the fall-through.

### And the effect system got its elimination rule

`handle { … } with E { … }`. The gap this closes was the one left in the
"found and left alone" list: effects could be declared, annotated, inferred,
and enforced, but never *discharged*, so `/ audit` propagated outward forever.

Three things landed together, because none of them is useful alone:

- **Introduction.** `Audit.record(x)` performs the operation and puts `audit`
  in the calling function's effect set. Before this an `effect` block declared
  operations that no analysis attributed to anyone and that the evaluator
  rejected with `unknown function` — a thirteenth bug of the familiar
  typechecks-then-does-not-evaluate kind.
- **Elimination.** `handle` removes the effect from the block it wraps, so a
  function can be pure despite calling something effectful. The subtraction is
  **per block, not per function**: an unhandled call sitting beside a handled
  one still reports. Whatever the arm itself does is attributed honestly, so
  handling `audit` by writing a file makes the handling function `/ fs`.
- **Declaration.** An effect annotation naming nothing is now an error.
  `/ nte` used to be accepted as a *different effect* from `/ net`, enforced
  consistently and matching nothing — a typo invented an effect instead of
  failing.

Handlers do not resume. An operation call dispatches to its arm and returns
like an ordinary call, which is what a tree-walking evaluator can do without
capturing continuations. Handlers are found **dynamically** (innermost wins)
and evaluated **lexically** (the arm sees the scope the handler was written
in), and both are tested.

### The examples are now pinned to their output, not to their exit status

`check-examples.sh` used to record which examples typechecked. That bar was too
low twice over. Two examples typechecked, ran, and printed the **wrong answer**:

- `cli-tool` searched with `contains`, which is element membership and not
  substring search, so its grep matched nothing and reported `0`;
- `autonomous-pipeline` filtered its worklist on readiness rather than on what
  had been placed, and called three of five tasks unplaceable on an acyclic
  graph.

Neither is visible from an exit status; both are obvious the moment the output
is read against what the example claims to demonstrate. So the script now pins
the answer. `--print` regenerates the block after an intentional change.

### Found and left alone

- **`agent` and `unsafe` cannot be written as effect names**, because both lex
  as keywords: `/ agent` is a parse error. `rand` is not built in either — the
  built-in kind is `rng`, and anything else silently becomes `Effect::Custom`.
- **Effect annotations are required only on `pub` functions.** Private ones
  infer. Over-declaration is always accepted, so an annotation is an upper
  bound, not a description.
- **Sigil letters cannot be identifiers**: `f v m C S E T I M u Y Z` are
  keywords, as is `ret`. Naming a variable `f` yields `unresolved name: val`,
  which points at the wrong token entirely.
- **A `(` after a block is parsed as calling that block.** A `while … { … }`
  followed on the next line by `(a, b)` becomes `while(…)(a, b)`, reported as
  `call: type mismatch: () vs f(…)`. Any statement in between separates them.
- **`scan` emits its seed** as the first element, so its result is one longer
  than its input. The two conventions differ by exactly one line, which is
  invisible until you count.

An earlier draft of this list claimed that **a nested `"` inside an f-string
ends it early**, and three examples were written around that. It is false —
`f"{join(xs, ", ")}"`, method receivers, and two-deep nesting all work. The
claim was written from a parse error whose real cause was never isolated, which
is the same mistake this document keeps describing, committed while describing
it. The workarounds have been removed.

---

## Traps this session hit, so you do not

- **`cmd | grep -q` under `set -o pipefail`.** `grep -q` exits at the first
  match, the writer takes SIGPIPE, and the pipeline reports failure — so every
  match reads as no-match. Cost two separate bugs, in `purge-video-from-history.sh`
  and later in `check-examples.sh` *despite a comment about it in the first
  file*. Capture the output, then match.
- **`bash` from PowerShell is WSL's**, which cannot open a `C:/…` path; and a
  Windows path with backslashes gets read as escapes. Prefer Git's bash and use
  repo-relative paths.
- **Shell mode bits do not survive a Windows checkout.** `chmod +x` locally is
  not enough; use `git update-index --chmod=+x`, and invoke scripts via `bash`
  anyway.
- **MAGE is not Rust.** Struct literals are `@P { x: 1 }`, not `P { x: 1 }`
  (which parses as a *map*). Booleans are `1b`/`0b` — there is no `true`. I lost
  time to both. `MAGE_ONTOLOGY.json` and `--build=schema` answer these
  authoritatively; read them before guessing.
- **`remove_dir_all` then `create_dir_all` on the same fixed path is unsound on
  Windows.** Deletion returns while the directory is still open somewhere (an
  indexer, a scanner, an Explorer window), leaving it *delete-pending*: it
  exists, cannot be opened, and cannot be recreated. The next `create_dir_all`
  fails with `os error 183` — "cannot create a file when that file already
  exists" — and keeps failing forever, because nothing ever closes the handle.
  Three `forge` registry tests had been red on this machine since 2026-08-05
  and the error pointed at the creation rather than the deletion that caused
  it. Fixed by giving each store a name no other run uses; the "all green"
  claim was true in CI and false here the whole time.
- **A tool's output means nothing until you know what you pointed it at.**
  `cargo audit` flagged `crossbeam-epoch` in `rmi`, which turned out to be a
  stale artifact in one working copy — `rmi` git-ignores its lockfile, so there
  was no committed pin to be stale. A local finding is not a repository finding.

---

## The pattern worth carrying forward

Six typechecker bugs, and **three of them were places where a comment stated the
correct rule and the code beside it did something weaker** — "No else → must be
unit" returning the branch type; "unifies with any int width" enforcing nothing;
"Return the struct type" returning a fresh variable. A comment agreeing with
your expectation is not evidence. It is usually what the author *meant*.

Equally: every one of the six was found by **running** a documented command,
never by reading documentation. `--version` did not exist. `--fix` was
unreachable in the documented argument order. Nine of thirteen RAP methods named
in the roadmap did not exist. The published ABL wire version was wrong. The
agent-facing capability index omitted the entire core loop.

If you want to find the next one, pick a surface nobody has run and run it.

---

## Notes on the shape of the work

- Every typechecker fix landed without breaking an existing test — **except one**,
  where widening `collection_elem` made `sum("hi")` legal and
  `vocab_rejects_non_collection` caught it. That is the datapoint showing the
  suite has teeth rather than merely being large.
- `git log` is the real record. The commit messages carry the reasoning,
  including the false starts and the two occasions I nearly attributed a
  pre-existing failure to my own change and checked first.
