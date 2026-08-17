# Handoff — through 2026-08-14

What this repository is, what state it is in, and what to do next. Written for
someone picking it up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,884** — rmi 1,380 · prototype 1,176 · ribosome 164 · germline 112 · forge 52 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm |
| CI | 10 jobs, green on `master` |
| Examples | 12 of 12 typecheck, run, and print their recorded answer |
| `.mg` sources | 101 checked, 25 listed sketches (all of them `stdlib/`) |
| Documentation | 206 MAGE blocks typecheck; 57 documentation entry points run |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/check-examples.sh                 # the 12 shipped examples, end to end
scripts/check-mg-sources.sh               # every .mg file in the repo
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

**There are 34 unpushed commits on `handoff`.** Nothing is pushed and no PR is
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
| `scripts/check-doc-blocks.sh` | that **every** MAGE block in every markdown file passes `--check` — the baseline is empty, so any new failing block fails the check |
| `scripts/check-doc-evals.sh` | that every documentation block defining `main` or a `@test` **runs** — 57 entry points, the second oracle |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

Plus, in the suite: every keyword introduces the token it claims, every
documented type can be written in a signature, every published control sigil
parses, every published path exists, every capability namespace performs its
effect, every layer surface name compiles.

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** The `.mg` source counts
(`101 checked, 25 listed sketches`) were wired in this way, and the pin caught
its own extraction bug on the first run — the measured value came out as
`files;` because the awk field index was off by one. The one figure that
stayed stale after the checker existed was one nobody had listed.

### Two things the instruments taught, the hard way

**A pin guarantees agreement, not truth.** CI checked `MAGE_ONTOLOGY.json`
byte-for-byte against a fresh `--emit-ontology` — and thereby guaranteed that a
*wrong* answer stayed identical. Four of the ten effect names it published were
rejected by the compiler. The check was working perfectly and proving nothing.
What was missing crosses the boundary: not "does the file match its generator"
but "does every name the file publishes actually work".

**A weaker criterion reports success early.** `check-doc-blocks.sh` counted
parse errors, because that was the failure that prompted it. Blocks that
parsed and then failed the *checker* scored as passes — 43 of them, including
some in files the script had been used to certify as fixed. The criterion a
ratchet enforces is the definition of done it hands to whoever comes next.

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
| 0 | **34 unpushed commits on `handoff`** | Push the branch, or open a PR against `master`. Note `gh pr merge --auto` merges *immediately* here — the repo has no required status checks — which is how PR #4 landed with CI still pending. |
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

### The documentation, finished

**Every MAGE code block in every markdown file in this repository now parses
and typechecks.** `scripts/check-doc-blocks.sh` reads `0 (baseline 0)`, and
its baseline file is empty. It started at 177 of 258 failing.

Two kinds of block are skipped, both visible in the source: fragments
(containing `...`), and blocks whose label says they are broken or invalid.
`MAGE_SPEC.md` uses the second deliberately, to record five designs that do
not compile — `grad(…)`, `rl` blocks, the compile-time SKB query API, SIMD
types and the module system — each labelled as invalid where it appears.
That is the honest state: the spec still describes more language than exists,
but it now says which parts.

**Midway, the count went *up* because the criterion did.**
`check-doc-blocks.sh` counted *parse* errors only, and 43 blocks parsed and
then failed the checker — unresolved names, undeclared effects, type
mismatches — several of them in files the script had already been used to
certify as fixed. It now requires `Errors: 0`, which turned 82 remaining into
114. **An instrument that measures the weaker property reports success early.**

**The spec was the hardest, and the most valuable.** A disagreement between
`MAGE_SPEC.md` and the compiler has to be *resolved* rather than patched,
because either side may be the one that is wrong, and the answer differs per
construct. What the pass found, in the sections nobody had run:

| §  | The spec said | The compiler says |
|---|---|---|
| 5.1 | `layer dense(784, 256, relu)` | `layer name: Linear(784, 256)` — every layer is named, the kind comes from the layer map, and activations are layers |
| 5.4 | `model:`, `data:`, `fn on_epoch(…)` callbacks | `net:`, `dataset:`, 25 fields, no callbacks |
| 6.1 | `Tensor<f32, [3, 224, 224]>` | `tensor[f32, 3, 224, 224]` — lowercase, one bracket |
| 7.1 | `rule integer(T) :- numeric(T), !floating(T);` | `rule integer(t: str) { numeric(t) }` — no Prolog, no `query` item |
| 8.1 | `select tournament(k: 8), target fitness > 0.98` | `select { 8 }` — strategies are blocks, no `target`, no callbacks |
| 9.1 | `agent` with `brain:`, `memory:` and methods | two fields, both lists of identifiers, and no code at all |
| 9.2 | `dispatch` / `aggregate` / `on_failure` blocks | four fields; fan-out is `map`, fan-in is `fold` |
| 12.1 | `@req(…)` above the signature | a `spec` block sharing the function's name |

And five constructs the spec documented that do not exist at all: `grad(…)`,
`rl` blocks, `Capability`/`Region`, SIMD types, and the module system. Each is
now labelled **Invalid MAGE today** with the parse error it produces, rather
than deleted — the design intent is worth keeping, the false impression is
not.

Seven of those are worth knowing about, because the defect was not just syntax:

- **`effects.md` taught agents to under-declare.** Four places asserted an
  effect hierarchy — "CORRECT — net implies io", "CORRECT — agent implies
  async", "apply the hierarchy rule — don't list implied effects" — and marked
  the *under-declared* version correct. There is no hierarchy: `/ net` does not
  cover an inferred `io`, verified by making one function call another and
  watching the checker object. For a capability system, advice to omit an
  annotation is the worst direction to be wrong in.
- **Every "CORRECT:" block in `anti-patterns.md` was also wrong.** The file
  pairs a WRONG block with a CORRECT one, ten times; the WRONG halves are
  supposed to fail, and all ten corrections failed too. Its Anti-Pattern 10,
  "Mixing Rust Crate Paths with MAGE Stdlib", offered `use std::fs;` as the fix
  for `use tokio::fs;` — both Rust, and moot besides, since `use` brings
  nothing into scope.
- **Both system prompts stated false rules in prose.** The agent-guide one
  claimed the same effect hierarchy and listed `process` as a built-in effect
  (the kind is `proc`). **Nothing checks prose**, and in a system prompt a
  false rule outranks a broken example — a model follows the rule when
  generating code the examples do not cover.

- **`migration-guide/` documented a capability system that does not exist.**
  `[capabilities] grants = […]` in `Forge.toml`, a `Capability` type,
  `cap.require("fs.read")?`, `Capability.request(…, Lease.new(Duration…))` and
  a table of 14 capability strings (`net.http.get`, `mem.deref`, `ffi.call`).
  None of it exists — and it is not missing, because the real design puts the
  grant somewhere stronger: reaching the namespace *is* the request, the
  annotation is the grant, and the check happens before the program starts.
  A document describing a weaker runtime mechanism teaches an agent to look
  for a permission call that is not there, and to assume the annotation is
  documentation.
- **The whole `cookbook/` was written against a standard library that does
  not exist.** Eight files, 60 blocks: `u std.io.{File, BufReader, Read}`,
  `s.new()`, `[T]~.new()`, `TempFile`, `fs.watch`, `Command.new("git")`,
  `Mutex`, `channel`, `join_all`, `I Agent ~ Greeter`, `AgentRuntime`, `Bus`,
  `Capability.request(…, Lease.new(…))`. It is the most detailed
  documentation in the repository and none of it ran. Rewriting it against
  the capability namespaces found the `contains` divergence, the `async`
  namespace that never existed, and the fact that **no capability call could
  be evaluated at all**.
- **`quick-start/03-syntax-tour.md` taught the opposite of the central rule.**
  "The compiler tracks effects automatically — you don't need to annotate them
  unless you want to document intent." It requires them, at every public
  boundary, and that requirement *is* the capability gate. The same page taught
  `I Area ~ Shape`, `Point @{ x, y }`, `@ item ~ items`, `[1, 2, 3]~`,
  `{1, 2, 3}`, `+v PI` and `c f` — none of which parse. It is page three of the
  quick start.
- **`patterns.md` was twelve Rust patterns.** Ten did not parse; the two that
  did were still Rust. Two patterns had no MAGE referent at all —
  "Effect-Bounded Generics" (there is no effect polymorphism; `fn(str) -> T / io`
  does not parse) and "Module Organization" (there is no module system). They
  are now default arguments and one flat namespace. Rewriting it found the
  capability hole below.

- **`syntax-quick-ref.md` and `migration.md` were Rust cheat sheets.** These
  two are the densest documents an agent reads, and almost every row was
  false: `let`, `async fn`, `mod`, `use path::to::Item`, `foo::<i32>()`,
  `#[derive(Debug)]`, `println!`, `true`/`false`, `Foo { x: 1 }` as a struct
  literal, four `std::` module tables, and the effect hierarchy again.
  `migration.md` answered six of its eight worked migrations with "no changes
  needed — identical Rust and MAGE". **Both are now measured**: every row was
  run through `--check` before it was written down, and the ones that fail are
  listed as things that do not carry over.
- **`agent-guide/examples/` answered 26 prompts in Rust.** These are
  prompt → response pairs — training data, where the response *is* the answer a
  model learns to give. Only 19 of the 26 failed to parse; the rest were Rust
  the lenient parser happened to accept, which is the worse half. The advanced
  file taught an entire fictional agent API: `impl Agent for X`,
  `self.cap.request("net.http.get", …)`, `Swarm::new()`. Rewriting the three
  files found four compiler defects — the escaped-quote bug, `range`'s missing
  arity check, `len` committing an open type, and the method effect hole — all
  four by running the examples and checking the answers.

This is the shipped examples' story a third time, after `examples/` and
`prototype/examples/`, and by volume the largest instance. Both of those
rewrites produced compiler bugs at a steady rate.
`scripts/check-doc-blocks.sh` ratchets the remainder.


| # | Item | Size |
|---|---|---|
| 10 | **Multi-shot resumption for effect handlers** | Largest, and *smaller than it was recorded as*. Single-shot resumption already works — an arm's value becomes the operation's value and the body continues — and `ret` in an arm aborts the handled block cleanly. What is missing is reifying the continuation so it can be stored or invoked twice, which is what generators and backtracking need. State, reader, logging and mocking handlers all work today. Still means reworking the tree-walking evaluator into a form that can capture continuations. |
| 12 | `int` literal constraint | Medium. The current fix is a post-hoc check in `default_int_literals`, not a real integer-kind constraint threaded through `unify`. Correct for the programs it rejects; the principled version is larger. |
| 13 | `stdlib/` | The remaining 25 sketches, and **the only aspirational `.mg` left in the repository** — 4,402 lines of Rust. Blocked on item 1: what a standard library *is* here depends on whether there are modules. `prototype/examples/` and `framework/framewerx/` are both done. |

### Small, sharp, cheap

This section is empty. The five items that were here — `guard` as a
reference, the `+=` diagnostic, `scan`'s seed, `pub` on a `data` field, and
the array-literal/slice mismatch — are all done.

Three were worse than recorded. `+=` failed `--check` *always*, not only with
untyped parameters. The array-literal item was filed as "a design question,
not obviously a bug", and turned out to be `lower_type` discarding the
declared length — which made **every** fixed-size array parameter uncallable
with a literal. And `pub` on a `data` field was already in the spec's grammar
(§4.3, `visibility? IDENT ':' type`), so it was a straight parser omission
rather than a decision anyone needed to make.

**A "design question" filed against a defect stops anyone from running the
check that would settle it.** Both of the last two sat here for a session
because the note framed them as needing judgment.

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
- **No capability call could be evaluated.** `fs.read_to_string(p)`,
  `env.get_env(k)`, `time.now()` and every other namespace call typechecked
  and then died with `unknown function` — the receiver is not a value, so the
  call fell through to the ordinary builtin table. **Every documented way for
  an agent to reach a resource was checkable and unrunnable**, which is why
  no cookbook recipe had ever been run. `io`, `fs`, `env` and `time` are now
  implemented; the rest report that the checker tracks the capability and the
  interpreter cannot perform it, rather than answering with a unit value.
- **`p"…"` printed nothing and interpolated nothing.** The print-string and
  eprint-string forms — the ones `cookbook/` uses throughout, and the shortest
  print in the language — were folded into a plain string literal by the
  parser. The statement was a no-op whose value was the raw text, braces
  included. They now desugar to `println` / `eprintln` over a format string.
- **A format-string hole is invisible to `--check`.** `p"value {nope}"` and
  `f"value {nope}"` typecheck clean and print `value <fn nope>` — the holes
  are parsed and evaluated only at run time, so an unresolved name inside one
  is neither a resolve error nor a type error, and renders as a function
  value. Not fixed: making holes visible to the checker means desugaring the
  interpolation at parse time rather than in the evaluator.
- **`println!("hi")` printed nothing and returned a bool.** MAGE has no
  macros, and this was the worst way not to have them: `!` parsed as logical
  *not* applied to the call, so `format!("hi {x}")` evaluated to `true` and
  `println!(…)` did nothing at all. No error at any stage. It is the single
  most likely line for a model carrying Rust habits to write. A `name!(` with
  no space between is now a parse error that names the macro and the
  replacement.
- **A string ending in an escaped quote lost it.** Literal decoding used
  `trim_matches('"')`, which strips *every* delimiter at each end, so `"x\""`
  came out as `x\` — the escaped quote became a backslash. Nothing errored:
  `contains(html, "\"")` was simply false and `split(html, "href=\"")` never
  split, so a link extractor returned `[]` and read as a logic bug in the
  program. Found by running a documentation example and checking its answer.
- **`range(1, 101)` ran as `0..1`.** `range` was the one typed vocabulary arm
  with no arity check, and the evaluator read argument 0 and discarded the
  rest. A FizzBuzz over `range(1, 101)` printed one line and exited 0. Both
  oracles now reject it and point at `a..b`.
- **`contains` on a string failed the checker.** The evaluator has always
  done three things — substring for a string, key membership for a map,
  element membership for a list — and the checker knew only the third. So
  `contains(email, "@")` evaluated correctly and reported `expected a
  collection, found str`.
- **The vocabulary enforced nothing.** All 15 argument unifications in
  `infer_vocab_call` were `let _ = unify(…)`, discarding the failure, so
  `join(xs, 7)` and `upper(5)` checked clean. The one thing a fixed, closed
  set of 31 combinators is *for* is catching misuse. They now report, and the
  stricter pass immediately caught three blocks written earlier in this
  session.
- **`len` decided an open type.** `len` accepts a string or a collection, and
  on an unresolved type variable it committed to a collection — so
  `v body = net.connect(url)` / `len(body)` / `body: str` was a type error,
  while the same three statements in the other order checked clean. **Statement
  order decided whether the program compiled.**
- **The suggested fix for an undeclared effect named the function.**
  `heal.rs` built the annotation from the first backticked word in the
  diagnostic, which in that message is the function — so the repair for
  `P.leak` reads "Add `/ P.leak` effect annotation". Applied, it writes an
  annotation that fails the unknown-effect check on the next pass. The
  diagnostic states the effects in a bracketed list; the fix now reads that.
  **A repair loop is an agent-facing answer too** — wrong advice there is the
  same defect class as a wrong number.

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

- **Generic functions were declarable and uncallable.** `f id[T](v: T) -> T`
  lowered `T` to a nominal `Ty::Named` — a distinct type unifying with nothing —
  because signature collection ran `lower_type` with no generic binding.
  `check_function` bound the generics as fresh variables, but only for the
  *body*. So `id(1)` evaluated to `1` and reported `type mismatch: I32 vs
  sym1`. Each call site now instantiates its own copy of the quantified
  variables, which is what lets `id(1)` and `id("ab")` coexist.

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
- **`--backend=<name>` selected nothing.** Documented as "Select hardware
  accelerator for dispatch", it reaches exactly one of six dispatch paths
  (`--run=abl-bytes`); every `--target=abl-*` builds a `CpuBackend` directly.
  So `--target=abl-compute --backend=cuda` printed "CpuBackend dispatch" and
  ran on the CPU without a word. For a repository whose headline numbers
  include GPU results, the failure mode is someone believing they measured a
  GPU. It now says when it is inert; wiring the other paths means threading
  `SelectedBackend` through their dispatch loops, which is item 14.
- **`nl/explain` and `nl/refactor` could never succeed.** Both take a `source`
  parameter, interpolate it bare into a prompt, and hand it to an engine whose
  `extract_code_block` reads source *only* from a ``` fence. So `intent.source`
  was always `None` and both answered "No source code provided" for every
  input, including well-formed ones. They are the natural-language surface —
  the methods an agent reaches for first — and the only 4 of 37 RAP methods
  with no test.
- **Capability handles performed no effect.** `resolve.rs` registered twenty
  namespaces and said their "use is tracked by the effect system". It was not: a
  `pub` function declared pure could call `net.connect(…)` or `llm.generate(…)`
  and check clean, while the bare `println(…)` beside it was caught. **The gate
  was open at exactly the seam the language documents as the way through it** —
  the safe-looking code was the unchecked code.
- **Method bodies were never effect-checked.** `infer_module` walked
  `ItemKind::Function` only, so nothing inside an `impl` or `extend` block was
  collected: `--check` on a module of methods printed **"Functions analyzed:
  0"** and returned OK, and a `pub fn` inside one could `fs.read_to_string(…)`
  while declaring nothing. Calling a method propagated nothing either, so a
  pure-looking `main` could reach any capability through one `.`. This is the
  same shape as the row above it, one call form over: **the two ways to reach a
  capability were `namespace.op(…)` and `receiver.method(…)`, and neither was
  checked.** Methods are now keyed `Type.method` and checked like any other
  body; a call is charged to a method when the name is unambiguous in the
  module. The count in the summary line is the tell — it was the one number
  that said the checker had done nothing.

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
- **The five swarm orchestration patterns.** `swarm_map_reduce`,
  `swarm_pipeline`, `swarm_saga`, `swarm_fan_out`, `swarm_race` and
  `grammar_extension` are reserved in the lexer and consumed by **nothing** —
  no parser arm, no evaluator, no mention in MAGE_SPEC.md — while
  `agent-guide/rap-agentic.md` documented three of them with worked calls. A
  reservation costs the name twice: the call is a parse error, *and* no user
  function can fill the gap. They now say so when written. Implementing them,
  or un-reserving them, is a design decision.
- **`--backend` reached one dispatch path out of six.** `--run=abl-bytes`
  honoured it; every `--target=abl-*` path built a `CpuBackend` directly, so
  `--target=abl-compute --backend=cuda` printed "CpuBackend dispatch" and ran
  on the CPU. The selection is now resolved once in `main` and threaded
  through all five via `SelectedBackend::as_dyn`, each path reports the
  backend it is using, and a subprocess backend — which has no in-process
  `Backend` — says what it fell back to instead of quietly using the CPU. The
  duplicate resolution inside `run_dispatch_abl_bytes` is gone too; a bad
  `--backend` name used to print its error twice. **The GPU path itself is
  still unverified here** — that needs `--features cuda` and hardware.
- **Every fixed-size array parameter was uncallable with a literal.**
  `lower_type` dropped the declared length and used 0 while an array literal
  types with its real length, so `f take(xs: [i32; 3])` called as
  `take([1, 2, 3])` failed with "array size mismatch: 3 vs 0". Now lowered; a
  non-literal length still lowers to 0 and unifies with anything, and a
  two-element literal is still rejected.
- **`pub` on a `data` record field was a parse error**, while the same field
  in a `struct` accepted it — and MAGE_SPEC.md §4.3 spells the grammar
  `visibility? IDENT ':' type`.
- **Thirteen of the seventeen registered builtins had no arm in the
  evaluator.** `resolve` registers `assert`, `assert_eq`, `panic`, `todo`,
  `dbg`, `vec`, `format` and the rest so ordinary code resolves; only `min`,
  `max`, `abs` and the print family were implemented. **`assert` is the only
  assertion the language has** — every `@test` in the documentation reaches
  for it, and it could only ever fail with `unknown function`. Found by
  running the documentation instead of checking it, which is now
  `scripts/check-doc-evals.sh`.
- **`GlobalAvgPool` had no shape rule.** It fell into the unknown-op arm and
  was treated as shape-preserving, so a `Linear` after it was checked against
  the *width* instead of the channel count.
  `framework/framewerx/examples/resnet_classifier.mg` — a textbook ResNet head
  — reported "expects last dim 256, but the preceding layer produced
  [1, 256, 20, 2]", and **two sessions wrote the example off as an
  unadjudicable sketch**: "either the example or the shape rule is wrong, and
  telling which needs someone who knows the intended architecture". The rule
  was wrong. Adjudicating it needed one look at the arm list, not an expert.
- **The Greek agent-mode surface for AI constructs.** `MAGE_SPEC.md`
  Appendix D publishes 33 symbol rows — `Ψ` for `net`, `λ` for `layer`, `Ω`
  for `evolve`, `κ` for `kb`, `Σ` for `swarm`, `⊗` for matrix multiply — and
  **the parser consumes none of them.** Fifteen are real tokens in the lexer
  (`KwPsi`, `KwSigma`, …) and no arm matches any; `Ψ Classifier { … }` is
  `expected item, found KwPsi`. The agent mode that works is the ASCII half:
  `+f`, `v`, `m`, `S`, `E`, `I`, `T`, the control compressions, and the short
  aliases (`sw`, `topo`, `cons`, `fx`, `hx`, `gd`, `df`, `xd`) — all verified.
  The AI blocks use the same keyword in both modes. The tables are now headed
  with that status rather than deleted: the compression argument is why the
  language has two surfaces, and it is worth keeping as design.
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

- **And once, understating.** The spec said "**Handlers do not resume.** An
  operation call dispatches to its arm and returns like an ordinary call." The
  second sentence is mechanically exact — and *returning like an ordinary call
  is* single-shot tail resumption; the body carries on. The first sentence
  reads as "the body stops", which is not what happens. Abort via `ret` in an
  arm worked too, scoped correctly to the handled block. None of it was tested.
  A reader would have concluded handlers were unusable and either avoided them
  or started a large refactor.

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
- **A type name blocked a function of the same name.** `define_type` mirrors
  into the value namespace so enum constructors resolve, and duplicate
  detection could not tell that copy from a definition — so `S Point { … }`
  beside `f Point(…) -> Point`, the ordinary constructor pattern, reported
  `duplicate definition: Point`. Worst for `sp`, where a spec block *names the
  function it constrains*: the contract feature could not be used as designed
  at all.
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
function**, and a handler's own effects are attributed honestly.

**Handlers resume, single-shot and implicitly** — the arm's value becomes the
operation's value and the body continues — and an arm may `ret` instead, which
aborts the handled block only and leaves the enclosing function running. Only
*multi-shot* resumption is missing (item 10).

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
consumer it never had; its sketch list only shrinks, and is down from 36 to 30.
Only `stdlib/` and four `framewerx` files remain.

---

## Traps, so you do not repeat them

- **Editing a shell script while it runs.** `bash` reads a script
  incrementally, so an edit that changes byte offsets can make the running
  copy execute a *fragment* of a line. The symptom is a nonsense error naming
  a line that is a comment — `test-all.sh: line 160: a: command not found`.
  Nothing was wrong with the script; the run was reading a file that moved
  under it. Wait for the run, or copy the script first.
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

- Prototype tests **1,066 → 1,176**, all green — checked against the live run, so
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
