# Handoff — through 2026-08-14

What this repository is, what state it is in, and what to do next. Written for
someone picking it up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,908** — rmi 1,384 · prototype 1,195 · ribosome 164 · germline 112 · forge 53 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm — and the four *committed* lockfiles now report 0 warnings too |
| CI | 10 jobs, green on `master` |
| Reliability floors | file-oracle parse 99/100, perturbed pattern-heal 42, native-lexer ratio 0.997 |
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

**Nothing on `handoff` is pushed, and no PR is open** — count them with
`git rev-list --count origin/handoff..handoff`. That is a decision waiting,
not an oversight — open item 0.

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
| `scripts/check-vocabulary.sh` | that all 31 words of §8 check, run, and return what the ontology publishes — and that its own case list *is* the published vocabulary |
| `scripts/check-ci-floors.sh` | the three measurable published floors, measured fresh; also fails if the committed `TOKEN_REPORT.md` differs from a new run |
| `scripts/check-rmi-api-doc.sh` | every item `rmi/docs/*.md` documents exists in the crate, and every `**Module:**` path resolves — baseline **0**, so any invented name fails |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |

Plus, in the suite: every keyword introduces the token it claims, every
published type can be written in a signature, every published control sigil
parses, every published path exists, every capability namespace performs its
effect, every layer surface name compiles, every published RAP method has a
working call, every heal pattern matches an example and returns a fix, every
opcode the compute dispatcher handles is in its doc table, every sigil the
quick reference teaches is discoverable from the ontology, and a decoder
following the published container format consumes every byte of a real
container.

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** And if two documents state
the same measured figure, pin **both** rows to the one measured key — that is
what caught `ribosome`'s dependency count, where `ARCHITECTURE.md` and
`RIBOSOME.md` had agreed on 39 for as long as either had existed. The `.mg` source counts
(`101 checked, 25 listed sketches`) were wired in this way, and the pin caught
its own extraction bug on the first run — the measured value came out as
`files;` because the awk field index was off by one. The one figure that
stayed stale after the checker existed was one nobody had listed.

### Four things the instruments taught, the hard way

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

**A fixed list under a universal doc comment is the commonest failure of
all.** Six separate tests in this repository claimed a cross-boundary property
and asserted a hardcoded subset:

| The test said | What it checked |
|---|---|
| "every CLI flag the binary accepts must be in the ontology" | eight named flags were present — 12 were missing, including `--eval` |
| "every published RAP method dispatches" | each method called with `{}` and not "unknown" — the *contract* was wrong for a third of them |
| `examples_all_parse` | that the published examples parse, not that they check |
| `every_framewerx_module_path_exists` | 256 paths exist, not that the 243 names they publish are in them |
| `ci_floors` (published as "read from ci.yml") | nothing at all: the workflow had none of them |
| `heal_patterns_section_nonempty` | that a list of 34 is not empty |

Each of them passed for years. The pattern is easy to spot once named: **if
the doc comment says "every" and the body has a literal list, the test is
weaker than its own claim.** Every replacement here compares both directions
and was verified by breaking it on purpose.

**And then I wrote a seventh.** The replacement for row two —
`every_published_rap_key_is_real`, whose whole point was that the old test
checked the wrong thing — is a hand-written list of working calls under a doc
comment saying "every published parameter is read". It exercised **31 of 37**
methods, and asserted nothing about the other six. Knowing the pattern by name
is not enough: a list-driven test needs an explicit assertion that the list
covers its subject, in the same function, or the next person to add a method
gets no signal. It now iterates the published methods and fails on any without
a call. That immediately paid for itself — `abl/decode`, `skb/spec`,
`verify/contracts`, `sandbox/policy` and `nl/refactor` had never had their
contracts checked, and `sandbox/policy` turned out to publish only `agent`
while reading the program out of an unpublished `source`, so *every* call an
agent could construct from the ontology returned `agent X not found` for every agent that existed.
`manifest/generate` had the same hole. The params check is now bidirectional
too: the calls are minimal, so anything a working call sends and the ontology
omits is a parameter no caller could know to pass.

**Two more, found by sweeping for the shape.** Grepping for `#[test]` whose doc
comment says "every"/"all" over a body containing a literal list turned up 13
candidates and two real holes. `every_typed_vocabulary_name_checks_its_arity`
listed 29 of the 31 published words, and the two it omitted — `scan` and
`group` — were the only two with **no typed arm at all**, so `scan(1, 2, 3, 4,
5)` typechecked clean. It now iterates `VOCABULARY`. And
`every_effect_documented_in_the_spec_parses_and_checks` held its own copy of
§11.2's seventeen rows, which is the one boundary the test exists to guard: it
now reads the table out of `MAGE_SPEC.md`, and a row added there that the
compiler rejects fails it. Verified by adding a `quantum` row and watching it
go red. A test that names its own subjects cannot report the subject it never
names — which is the same lesson as the table above, arriving through a
different door: not "the list is short" but "the list exists".

A third, in the same file:
`the_builtin_names_attributed_on_call_are_the_documented_ones` kept two
hand-written buckets — names that attribute an effect, names that attribute
nothing — under a comment saying they were §11.2's domain column. Between them
they covered 36 of its 47 names, and the eleven left out included `read`,
`listen`, `send`, `open`, `remove`, `alloc`, `dealloc`, `now` and `sleep`, all
of which do attribute an effect and none of which was checked. It now reads the
domain column and asserts every identifier in it lands in one bucket or the
other, so a name added to a domain fails until someone decides which it is.
Only `exec` turned out to be genuinely inert — `proc`'s domain is "spawn, exec,
tool invocation" and `exec()` performs nothing, which is consistent with
§11.2's own sentence that the domain column is not a table of callable
operations, and is now stated where a reader of that table would look.

**And a new instrument, `check-vocabulary.sh`.** The 31 words of §8 are the
entire standard library — there are no modules and no `stdlib/`, so an agent
writing MAGE has these and nothing else. Three lists must agree for one to
work: the resolver's, the checker's `infer_vocab_call`, and the evaluator's.
The unit tests compare each against its neighbour; nothing ran a word end to
end. This runs the binary, both oracles, one call per word, and compares the
answer to the published signature — and asserts its own case list is exactly
the published vocabulary, so the coverage question is answered in the script
rather than by whoever reads it. All 31 pass today. Wired into CI beside the
doc instruments.

**The unimplemented-capability message named the wrong kind of thing.** All 20
capability namespaces report honestly under `--eval` — none silently returns a
wrong value, which is the failure that would matter most here. But the message
read "Implemented: io, fs, env, time", a list of *namespaces*, while `io.op`
was refused: within those four only specific operations have arms. An agent
recovering from that error was told to retry in a namespace that would refuse
it again. It now names the operations of the namespace the caller just tried,
or says plainly that nothing in it is implemented. Since the message is now a
claim that can be wrong, `every_advertised_capability_operation_has_an_arm`
runs every operation it advertises — verified by advertising a `time.deadline`
that does not exist.

**`forge`'s manifest and its dispatcher were never compared.** All four tests
over `COMMANDS` check how the table is *rendered*, so a published command the
binary does not dispatch, or a command the binary accepts and the manifest
never mentions, passes every one of them. `forge manifest` is what an agent
reads instead of `--help`. They agree today; a test now says so in both
directions, verified by adding a hidden `secret` command. `describe` was also
checked against three names by hand and is now checked against all ten, with
and without dashes.

**The guide and the ontology disagreed about what the language contains.**
`agent-guide/syntax-quick-ref.md` is what an agent reads to *learn* the
compressed surface; `MAGE_ONTOLOGY.json` is what it reads to *discover* the
language mechanically. Nothing compared them. Thirteen sigils that parse were
taught by the guide and published by neither: `D`, `xd`, `fx`, `sp`, `sw`,
`af`, `gd`, `df`, `yl`, and the `pub` forms `+S`, `+E`, `+T`, `+D`. Only `+f`
was there, so an agent reading the JSON could discover public *functions* and
not public types — and four of the nine are whole features (records, inherent
methods, effect declarations, contracts). The section went 37 → 50. Checking the guide's other column while there turned
up one more: it listed `static` under "Not in the language", which is true of
the human spelling and false of the agent-mode `Z NAME: T = expr;`, the only
declaration in the table with no human form. Its human column is otherwise
sound — every row parses once the meta-syntax placeholders (`fields`,
`methods`, `ops`) are replaced with real ones.

The test that now holds them together taught its own lesson. Written first as
"every token the guide teaches appears in some ontology section", it passed
with `xd` deleted from `sigils` — because `xd` is *also* a lexer keyword, and
the union of every section made the check nearly unfalsifiable. **A both-
directions test can still be vacuous if the right-hand side is broad enough.**
It now checks the guide's Declarations fence against `sigils` specifically,
which is the narrow claim that can fail, and keeps the union check for the
broader one. Verified by deleting `xd` again.

**The weakest test in the catalogue was the one still standing.** The table
above lists `heal_patterns_section_nonempty` — "that a list of 34 is not
empty" — and unlike the other five it had never been replaced. Replacing it
meant deciding what the published list *claims*: that each of the 34 names a
mechanical repair an agent can ask for. `ErrorPattern` now carries an
`example` message beside its matcher, the compiler requires one, and a test
runs every pattern against its own example through the same `heal` path an
agent calls. It found `type-mismatch` generating nothing for the checker's
commonest message. The design point is that the example lives *next to the
matcher*: a test file listing 34 messages would drift from the matchers within
a session, which is how the list-driven tests in the table above got their
start.

**And a sixth section pinned across its boundary.** `recovery_stages`
publishes seven names; `RecoveryStage::as_str` reports six, the difference
being `agent.refine`, which belongs to the reliability bench rather than to
`recover.rs`. Nothing compared the two, so a stage added to the enum would
have gone unpublished silently. The test now checks both directions and names
`agent.refine` as the one deliberate exception — which is what keeps it
deliberate rather than merely present.

**A floor read from a generated file is only as fresh as the file.**
`check-ci-floors.sh` enforced the published token-ratio ceiling by reading the
`**Total**` row of the *committed* `benchmarks/TOKEN_REPORT.md`. That file is
generated by `token-bench`, **nothing ran `token-bench`** — not CI, not
`test-all.sh`, not any check script — and it had gone stale in two categories.
The ceiling was being enforced against a number nobody had measured recently.
It now runs the bench and compares.

Writing that check taught the lesson again, immediately. The bench *writes*
`TOKEN_REPORT.md` as a side effect, so my first version — run the bench, then
compare the file against the bench's stdout — compared two things produced by
the same run and agreed by construction. It passed when handed a deliberately
stale report. The working version saves the committed copy, runs the bench,
compares the two files, and puts the original back, because a check that
rewrites a tracked file is not a check either. **When a tool both measures and
writes, comparing its output to its own artifact proves nothing.**

The bench also exits non-zero on a clean tree: 150 of the corpus's claimed
`token_count` fields disagree with measurement by >10 %, which is precisely
what `benchmarks/FINDINGS.md` §1 concludes. Its exit status therefore cannot
gate anything until those claims are corrected or dropped — a decision, not a
defect, now written where someone will find it.

One generated artifact deliberately has *no* freshness check:
`benchmarks/RELIABILITY_REPORT.md` embeds p50/p95/p99 latencies, so it differs
after every run on every machine. Checking it for staleness would be
permanently red, which is the failure mode a ratchet is supposed to avoid.
`TOKEN_REPORT.md` is byte-stable and therefore checkable; the distinction is
whether the artifact records a measurement or a timing.

**A harness can change meaning under a table it produced.** `MEASUREMENTS.md`
gave ABL artifact scaling as 78 / 234 / 858 / 3354 bytes at 2 / 8 / 32 / 128
layers — "linear, ~26 B/layer". Re-running `perf_report` today prints **67
bytes four times**. Neither number is wrong. The harness builds `n` *identical*
`Linear(16,16)` layers, and v3's REPEAT fold collapses them to a constant; the
documented table was measured before that fold existed, and is still exactly
right for layers that do not repeat. **The natural response to that
disagreement is to "correct" a true table into a misleading one** — the
document looks stale and the tool looks authoritative, and here the tool had
silently started measuring a different question. The harness now measures both
cases and says which is which.

This one is also the session's first *under*-claim: a 128-layer stack of
identical layers ships in 67 bytes and decompiles back to 128 layers, which
`MEASUREMENTS.md` did not mention at all. A test pins both halves, because a
fold that lost layers would also produce a small artifact and "compact at
rest" would be measuring data loss.

**Two documents wrong together, because one was copied from the other.**
`ARCHITECTURE.md` and `RIBOSOME.md` both give `ribosome`'s transitive dependency
count as evidence for the claim that the build engine privileges no language.
Both said **39**. Measured: **28** unique crate names on normal edges, or 34
counting the six that resolve at two versions — and neither document said which
it meant. Agreement between two documents is not corroboration when one is a
copy; it is the same claim counted once. Now pinned, so `--check-docs` fails if
either drifts, and the *measurement* is emitted by `test-all.sh` rather than
transcribed.

**An accepted-risk register decays like any other measured claim, and worse.**
`SECURITY_AUDIT.md` §1 named two accepted `cargo audit` warnings. Re-running the
five surfaces: one of them (`rand`, RUSTSEC-2026-0097) had **stopped firing** —
the lockfile now carries `rand 0.8.6`, which the advisory itself lists as
patched — and a different one had **started**: RUSTSEC-2026-0190, `anyhow`
unsound, dated 2026-06-25 and absent from the table. It had a patched version
available, so accepting it was never the right call; `cargo update -p anyhow`
closed it and the four committed lockfiles now report zero findings of any kind.

The register had drifted in *both* directions at once, and neither is visible
to a reader: someone checking whether the accepted set was complete would have
found it neither complete nor current. CI's `audit` job deliberately does not
fail on warnings — correctly, since a permanently red job gets ignored — which
means **nothing compares the accepted-risk table against the warnings actually
reported**. That comparison is still manual; the command to run is now in the
document.

**"0 npm" was a claim about a surface nothing checked.** `video/` has a tracked
`package-lock.json`, and `npm audit` reported one **high-severity** advisory
(`nanoid < 3.3.18`, transitive via `postcss`). Fixed. CI's `audit` job runs
`cargo audit` over five Cargo lockfiles and does not touch this one, so the
figure in the table above had no mechanism behind it — it was true when
someone last looked, which for a vulnerability count is a different thing from
being true.

**A security document's *under*-claims are defects too.** `SECURITY_AUDIT.md`
§3 said the non-loopback `--rap` refusal was "proposed, not yet enforced". It
has been enforced for some time: `rap::is_non_loopback` exits 2 with an
explanation unless `MAGE_RAP_ALLOW_REMOTE=1`, and warns even then. A reader
planning a deployment would have built a guard that already exists, or avoided
the tool over a gap it does not have.

The same row's memory-safety claim was wrong in the other direction, and this
one mattered: "rmi has 1 audited `unsafe` in lib.rs, 3 in the CUDA FFI shim —
all reviewed, FFI-boundary only". `lib.rs` has none, and the real surface is
**9 `unsafe` blocks in `runtime/memory_pool.rs`** — an arena allocator doing
`alloc_zeroed` and raw pointer arithmetic, compiled unconditionally. Not an FFI
boundary; the highest-risk category of `unsafe` there is, asserted not to exist.
(`compute/cuda_full.rs` holds 16 more but no `mod` declaration references it, so
it never compiles — counting those would overstate the surface as badly as the
old row understated it.)

Reviewing those 9 found **four** defects. Two in `Slab::new`, both reachable in
one call because `PoolConfig`'s fields are public and `with_config` guards
nothing: a zero `initial_capacity` called `alloc_zeroed` on a **zero-sized
layout** (documented UB, at construction), and `size * capacity` was unchecked.
Two more in `TensorBuffer`, and these are the serious ones because they need no
unusual configuration at all:

- **`Drop` freed a layout the allocator never handed out.** `from_vec` forgot
  the `Vec`'s capacity; `Drop` rebuilt it as `from_raw_parts(ptr, len, len)`.
  Capacity differs from length in the *ordinary* case, so this was undefined
  behaviour on the normal path — and the SAFETY comment claimed "the pointer,
  len, and capacity match what was passed to `mem::forget`".
- **`slice(offset, len)` added the two unchecked.** `slice(usize::MAX, 1)`
  wraps to 0, passes the bound, and offsets the pointer by `usize::MAX`. Two
  plain integers through a safe public method.

All four fixed, all four with tests verified by removing the guard. **The claim
"reviewed" is not free** — this document made it on the code's behalf, and the
review it stood in for found undefined behaviour on a default path. When a
security document says "audited", the useful question is who did it and what
they wrote down.

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
| 0 | **Unpushed commits on `handoff`** | Push the branch, or open a PR against `master`. Note `gh pr merge --auto` merges *immediately* here — the repo has no required status checks — which is how PR #4 landed with CI still pending. |
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

This was the shipped examples' story a third time, after `examples/` and
`prototype/examples/`, and by volume the largest instance — and the rate of
compiler bugs per document never dropped, across all three.
`scripts/check-doc-blocks.sh` and `check-doc-evals.sh` now hold the result.

### Real work, unstarted

Two items, and both are large.

| # | Item | Size |
|---|---|---|
| 10 | **Multi-shot resumption for effect handlers** | Largest, and *smaller than it was recorded as*. Single-shot resumption already works — an arm's value becomes the operation's value and the body continues — and `ret` in an arm aborts the handled block cleanly. What is missing is reifying the continuation so it can be stored or invoked twice, which is what generators and backtracking need. State, reader, logging and mocking handlers all work today. Still means reworking the tree-walking evaluator into a form that can capture continuations. |
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

- **A swarm's `dispatch`, `aggregate` and `on_failure` blocks.** They parsed,
  were stored on `SwarmDef`, and reached exactly one consumer: the MLIR text
  dump. No resolve pass walked them, no typechecker entered them, the evaluator
  had never seen them — and `--fmt` printed them back as the literal string
  `dispatch { ... }`, so a format round-trip **deleted the body**. An agent
  could write a swarm's coordination logic, get `Status: OK`, format the file,
  and be left with nothing. §14 and §15 both say these three do not exist, so
  the parser now agrees and the diagnostic says what to write instead
  (`map`/`fold` over ordinary functions, §9.2). The `ast`/`mlir`/`fmt` paths
  stay — they are reachable from the bridge, and the damage was on the surface.
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
- **The published container format omitted its symbol table.** The `.abl`
  container has carried one since v2 — the section that makes a `kb` artifact
  self-describing, because without it a decoder recovers predicate arities and
  not their names. It appeared in **none** of the four places the format is
  described: `MAGE_ONTOLOGY.json`'s `abl.format`, `AGENT_PROTOCOL.md`, and the
  module docs in `abl.rs` and `main.rs`. An agent implementing a decoder from
  any of them parsed the items and stopped, leaving **100 of `unified.mg`'s 420
  bytes** unread. `decode_container` was fine throughout, which is why nothing
  noticed: the code reads the real format, and only the description was wrong.
  A test now walks a real container using *only* the published field list and
  asserts it lands exactly on the last byte.
- **A global rename put the language's name where four magic bytes belong.**
  `ABL` → "Agentic Binary Language" rewrote every literal spelling of the
  container magic: the ontology's format line and its fallback, the
  `--manifest` entry for `--target=abl-bytes`, two module docs, and — worst —
  the decoder's own error message, so the one thing a caller sees when a file
  fails to open said it expected a 23-character string instead of `ABL1`. The
  `"magic"` *field* was right the whole time, because it is read from the
  constant, which is exactly why the section looked healthy.
- **`abl_compute`'s "Supported ops" table listed 6 of the 23 it dispatches.**
  It said `LINEAR`, `MATMUL`, `CONV2D`, `ATTN` and the normalisations "are
  reported as **unsupported** because they require parameter tensors that the
  bridge does not yet thread through" — while the parameter store described
  fifty lines below threads every one of them, and the capstone benchmark had
  been dispatching 97 ops with an empty unsupported list. Anyone deciding
  whether the CPU backend could run their net was reading a description of a
  compiler that no longer existed. The table is now the dispatcher's arms, in
  both directions, checked by scraping this file — an opcode listed but not
  dispatched promises what the backend cannot do, and an arm missing from the
  table hides what it can.
- **`type-mismatch` healed nothing for the mismatch the language reports.**
  The pattern is published as the mechanical repair for type errors, and its
  generator only fired when the message named `Option` or `Result`. The
  checker's own commonest mismatch names neither — `type mismatch: I32 vs
  Usize`, which is what `f m() -> i32 { len(xs) }` produces, the first error
  most programs hit. So the repair an agent asked for came back empty, on the
  error it was most likely to be asking about. It now offers a real cast edit
  between two numeric types and names both sides otherwise. Found by giving
  every pattern an `example` message and running it.
- **The ontology published 17 of the binary's 29 flags.** The twelve missing
  included **`--eval`** — the only way to run a program, and half of the
  two-oracle discipline this repository depends on — plus `--version`,
  `--json`, `--fix`, `--manifest`, `--token-report` and the whole `--build=`
  / `--describe=` / `--spine=` family. An agent grounding in
  `MAGE_ONTOLOGY.json` could not learn that MAGE programs can be executed.

  The test that should have caught it asserted **eight named flags were
  present**, under a doc comment reading "every CLI flag the binary actually
  accepts must be in the cli_flags ontology section". It now scrapes the flag
  literals out of `main.rs` and compares the two sets in both directions —
  verified by removing `--eval` and watching it fail. 36 flags are published.
- **The tool manifest omitted `--eval`, calling it "superseded by
  `--run=abl`".** It is not superseded: `--run=abl` reads an Agentic Binary
  Language *container* and answers `bad magic` on a `.mg` file. So
  `--manifest` — which describes itself as "read this first" and promises an
  agent will never need the prose docs — listed no way to run a MAGE program.
  Now listed, with the two-oracle point in its detail text.
- **A test kept its own copy of the thing it tested.** `positional()` in
  `main.rs`'s test module duplicated the production modifier-flag filter, so
  deleting `--token-report` from the real filter left the test green. One
  function now, called by both — verified by deleting the flag again and
  watching it fail. **A test that mirrors the implementation cannot catch the
  implementation drifting.**
- **`MAGE_SPEC.md` Appendix B — the dual-syntax mapping table — had seven
  wrong rows**, under the heading "Every Human-mode construct has an
  Agent-mode equivalent. Both parse to the same AST." `const` → `c`
  (lowercase is an identifier; the sigil is `C`), `else if` → `:?` (the space
  matters: `: ?`), `for x in y` → `@ x ~ y` (the separator is `in`), `while` →
  `loop ?` (it is `@w`), `Foo { x }` → `Foo @{ x }` (the `@` goes before the
  name), plus `const fn` → `c f`, `pub(crate)` → `~` and `crate::` → `~.`,
  none of which parse in either mode. B.8's "shared syntax" listed `f16`,
  `bf16` and `tensor!` literals, none of which exist.
- **`?:` was published as a sigil by two documents that disagreed about what
  it meant** — the ontology called it "human-mode if (sugar for ?)",
  MAGE_SPEC.md B.2 called it the KB-query sigil — and the lexer has no such
  token at all. Removed, with its absence pinned the way `^`'s is.
- **`nl/explain` answered with Rust `Debug` output.** Asked to explain
  `+f add(a: i32, b: i32) -> i32`, it said
  `(a: Path { segments: ["i32"], type_args: [] }, …)` — the AST node
  `{:?}`-formatted into a natural-language answer, from the method an agent
  reaches for to *understand* code. The formatter has rendered types correctly
  all along; `fmt::type_to_string` is now public and the explanation uses it.
- **`nl/refactor` could not refactor, defeated by its own name.** The intent
  classifier matched keywords as bare substrings, and the knowledge-base
  branch lists `fact` — which is inside "re**fact**or". Every refactor request
  classified as "generate a knowledge base" and answered `kb Generated { }`,
  with `ok: true`. The previous session had "fixed" this method by fencing its
  source, which only got it far enough to answer confidently and wrongly.

  Keyword matching is word-bounded now (`add` no longer matches
  "addressable", `run` no longer matches "runtime"), multi-word phrases still
  match as phrases, and `nl/refactor` returns refactored source.
- **The published RAP contract was wrong for a third of its methods.** RAP is
  the protocol an agent drives the compiler through, and `rap_methods`
  publishes each method's parameters and return keys. **Eight methods read
  parameters the ontology did not name, and seventeen returned keys it did
  not name.** `skb/query` publishes `{query}` and reads `by`/`value` —
  calling it as documented returns `matches: []`, because the missing keys
  default to the empty string. `sandbox/policy` publishes `name` and reads
  `agent`. `verify/contracts` publishes `source` and reads five other keys.
  `build/check` publishes `diagnostics` and returns `errors`.

  The test that should have caught it called each method with `{}` and
  asserted only that the method was not "unknown" — which a method that
  always errors still passes. The new one calls each with inputs that
  *succeed* and checks the answer's shape, and it caught a mistake in its own
  first draft.

  **`format/agent` and `format/human` did not format.** Both returned the
  AST — `format/human` said "same as parse for now" — while publishing
  `{formatted, ok}`. `fmt::format_agent` is what `--fmt-compact` has always
  used. Both now return formatted source, verified over the wire.
- **Six published "CI floors", enforced by nothing.** The ontology's
  `ci_floors` section named `MIN_PARSE >= 98`, `MIN_HEAL >= 40`, a
  `native-lexer ratio <= 1.100` and three more, under a doc comment saying
  they were "read from `.github/workflows/ci.yml`". **That file contained
  none of them** — no reliability-bench job, no heal threshold, no ratio gate.
  `UNIFICATION.md` went further and described a "new CI step" parsing
  `benchmarks/TOKEN_REPORT.md`; there was no such step. An agent reading
  either believed six regressions were gated.

  Two of the six were not even true: the file-oracle structural-heal
  contribution is **1**, not `>= 2`, and the stage-3 refine smoke is 0 without
  a wrapper. Those are now stated as observations. The three real ones are
  measured and enforced by `scripts/check-ci-floors.sh`, which CI runs —
  verified by raising `MIN_PARSE` to 100 and watching it fail. Reading the
  ratio also has a trap worth knowing: `TOKEN_REPORT.md` has four `**Total**`
  rows and the first one is *source bytes* (1.055), so the check anchors on
  the section heading rather than the row shape. It passed for the wrong
  reason on its first run.
- **`data` and `extend` were missing from the published AST kinds.** The
  ontology enumerated 18 item families; `ItemKind` has 20. The two absent
  were records/sums (`data Point(…)`) and methods (`extend Type { … }`) —
  three of the constructs the human-mode guides lead with. Nothing compared
  the two lists, and the published names were a parallel vocabulary (`Mod`,
  `EffectDef`, `SpecBlock`) that made comparing them awkward enough that
  nobody had. The names now match the variants exactly and a test scrapes
  `ast.rs` to compare both ways.
- **The ten examples the ontology publishes were only *parsed*.** They are
  what an agent grounds in when it asks what MAGE looks like, and the test
  bar was "parses" — the same weaker criterion that let 43 documentation
  blocks through earlier this session. They all typecheck (verified by
  breaking one and watching the test fail), and the test now requires it.
  Likewise `framewerx_modules` checked that its 256 paths exist but not that
  the 243 symbol names it publishes appear in the files it points at.
- **Five ontology counts quoted in the docs were stale**, in the two places
  that quote them: `cli_flags (17)` for 36, `heal_patterns (~13)` for 34,
  `keywords (12)` for 102, `layer_map (31)` for 21, `effects (15)` for 22,
  and "21 sections" for 22. All 22 section sizes are now emitted from the
  committed ontology and pinned, so a claim about the ontology cannot drift
  from the ontology.
- **An integer literal adopted a width without having to fit it.**
  `f g(n: u8)` called as `g(300)` typechecked clean, as did `i8` ← 200 and
  `i32` ← 3000000000. The literal's value is now carried alongside its type
  variable and range-checked against whatever kind it ends up unified with —
  boundaries (255, 127, `0xFFFFFFFF`) still pass, and a float context still
  accepts an integer literal.

  What this does *not* do is constant-fold: `g(0 - 1)` into a `u32` is an
  expression, not a literal, and still passes. That is the remaining half of
  old item 12, and it needs a different analysis rather than a bigger range
  table.
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

- **`benchmarks/FINDINGS.md`, the document that exists to hold the honest
  numbers.** Its §1 measurement table had five stale figures (source bytes,
  dense bytes and the native-lexer total, which is the one the CI ceiling
  gates). Worse, §1 spends four sections proving the "~50 %" claim
  unsupported and then heads a later section "Where the ~50 % claim *is*
  real", asserting the binary encodings are "1-2 orders of magnitude smaller
  than the equivalent text in either MAGE or Rust". Measured against the MAGE
  text they replace: TransformerBlock 243 B → 47 B, MLP 98 B → 17 B, FamilyKb
  116 B → 68 B. That is **2-6×**, and the section now carries the table rather
  than the adjective.
- **`benchmarks/FINDINGS.md` §2 and §6 read as present tense and were two
  years of work out of date.** §2's TL;DR: "Three quarters of the corpus's own
  reference solutions don't parse through today's MAGE prototype parser", with
  a per-category table showing seven of ten categories at 0.0 %. Measured
  today: **lex 100/100, parse 99/100, effective 100/100** — every one of those
  categories parses. §6's headline "effective 39/100 under near-correct input"
  is now **70/100**, and heal-reach on perturbed input went 13 → **42 of 73**.
  The file is a chronological log and its sections are legitimately dated
  snapshots; what made them misleading is that nothing said so, and its own
  summary table carried the old numbers as current. Each superseded section now
  carries the re-measured figure beside it, and the header table is dated.
- **`rmi/docs/api.md` documents an FFI API that exists nowhere in the crate.**
  `FfiValue` and `FfiFuncPtr` are in no source file; `register` takes an
  `FfiBinding`, not `(name, sig, ptr)`; `call` deals in `Val`; and
  `call_unchecked` is documented **`unsafe`** and is not. The real thing passes
  RMIL values to safe Rust closures (`Box<dyn Fn(&[Val]) -> Result<Val,
  String>>`), so the block painted a *more* alarming surface than exists —
  raw `*mut u8` pointers and an unsafe entry point where there are neither.
  `unsafe` in a signature is a contract, and documenting one that is not there
  is as much a defect as omitting one that is. Also every **Module:** line said
  `framewerx::…` when the crate is `rmi` — all twelve wrong, all twelve modules
  present, so a reader following them concludes the modules are missing. Found
  while checking whether the vendored crate repeated the "all reviewed" claim,
  which is the only reason it was found. Measured across the whole file:
  **27 of 228 documented items — 12% — exist nowhere in `src/`.** Correcting
  `CheckpointManager` took that to 23; every method in *its* block was wrong
  too (`save_checkpoint` for `save`, `&str` ids for `Uuid`, a constructor that
  never took those arguments), and the *type* resolves, which is exactly what
  made it hard to notice — nothing fails until you call a method.
  `scripts/check-rmi-api-doc.sh` ratcheted that to **0** — every documented name
  now exists — and fails outright on an unresolvable module path.

  Draining it turned up one mistake made over and over, in five separate
  sections: **a documented lookup keyed by *name* where the real API takes a
  `Uuid`.** `Ontology`, `AIConceptsOntology`, `AIHistoryKB`,
  `CheckpointManager` and the hash ring all had it. That shape survives review
  because the call reads correctly — `get_concept("attention")` is exactly what
  you would expect to write — and only the type checker objects. Two others
  were worse than a compile error: `KeyValueStore` documented
  `with_compression` when the real builder is `without_compression`
  (compression is on by default, so the reader enables what is already on), and
  `ArchitectureBuilder::add_skip_connection(from: usize, to: usize)` where the
  real `residual_add` takes a node `Uuid` — positions offered where identities
  are wanted. The check is deliberately name-level: it cannot
  see a wrong signature on a function that exists, which is most of what was
  wrong here, so `cargo doc` stays the authority and the file says so.

  Following that admission with a measurement: of 162 documented functions,
  **66 are defined exactly once** in `src/` and so can be compared by arity
  without ambiguity. Four disagreed — `add_fact` (2 params documented, 3 real),
  `similarity` (documented as a method, actually an associated function on
  `&[f32]`), and `Protocol::encode`/`decode`, which **belong to `Frame` in a
  different module** and have different shapes. All four fixed; the set is now
  clean. The other 96 share a name with another definition (`new`, `len`,
  `save`), so a name-keyed comparison cannot tell which one it is looking at.

  **No arity checker was added, deliberately.** It would cover 41% of the
  functions and carries a false-positive class — my own `/* … */` placeholders
  tripped it twice while writing the fixes above. A check that is right 41% of
  the time and cries wolf is the kind this session has spent its length
  cleaning up after. The measurement is worth repeating by hand after a
  refactor; it is not worth a ratchet.
- **`ontology.md` was invisible to the checker, and wrong in all three of its
  enumerations.** It contains no `pub` items, so the name check reported zero
  documented items and passed — a file the instrument cannot see reads exactly
  like a file with nothing wrong. Checked by hand: **six of eleven documented
  relations do not exist** (`related_to`, `extends`, `used_for`, `builds_on`,
  `introduced_by`, `superseded_by`) and six real ones were missing; the domain
  set is ten variants, not the five headings ML/DL/SYM/NS/MAS, of which only
  two have any counterpart; and the concept-property table named `name` as the
  unique identifier when it is `id: Uuid` — the same name-versus-id error the
  API docs made in five places.

  The taxonomies were *measured* rather than rewritten: **166 documented nodes,
  28 shipped concepts, 12 in common.** They are a survey of the field, which is
  a fine thing for the document to hold and a misleading thing to present as a
  crate's contents, so the file now says which it is. **A prose document is not
  covered by a check that reads code blocks**, and the passing result said
  nothing about it either way.
- **`rmi/docs/protocol.md` describes a wire format the implementation does not
  speak.** Checked after `api.md`, on the same suspicion that one bad doc is
  rarely alone. The header diagram is wrong in every field: the magic is
  **`FWRX`, documented `FRWX`** — the W and R transposed, with the hex spelling
  out the same wrong order so the two agreed with each other and not the code;
  `flags` and `message_type` are the wrong widths and the wrong way round;
  `MsgId` and `Timestamp` are not in the header at all (those 16 bytes are
  `payload_length` and `checksum`); there is no sender-ID section and no
  attachment count. The checksum is **XXH64 stored inside the header**, not a
  CRC32 trailer — wrong algorithm, wrong width, wrong position, and computed
  over different bytes in a different order. A client written from that page
  fails at byte 4, and every one of these is a silent interop failure rather
  than a compile error, which is what makes a wrong wire-format diagram worse
  than a missing one. The other two docs were nearly clean: one stale
  `SymbolEmbedding`, and two `pub fn`s that are deliberately illustrative.
- **`AGENT_PROTOCOL.md`, nine figures at once.** The document that tells an
  agent how to emit bytes rather than text opened with "the genuine ~50×
  efficiency win", and its own worked example three sections down measures
  **0.225** — 1866 B of text to 420 B of container, which is 4.4×. It also
  gave the container as v1 (it is v3) with 300 bytes (420), "header overhead
  ~50 bytes" (framing is 195, nearly half the file at this size), three of the
  five item hashes stale, "12 opcode families, 95 ops" (7 and 107), "31 neural
  opcodes by name" (21), and "~3 orders of magnitude" more programs per
  response (nearer 5-8× per transformer block). Every one of them is
  reproducible with a command printed in the same file. The decode block
  underneath was accurate, which is the tell: the outputs that were *pasted
  from a run* survived, and the ones written by hand around them did not.
- **The README, four figures at once.** The front page's binary-IR block was
  labelled *(measured)* and three of its numbers were not: it printed the
  container header as `ABL1 02 00` (version is **3**), sized the source at
  `327 B` (the file it names is **139 B**), and derived "**71.9 % smaller**"
  from that wrong size (the real reduction is **33.8 %**). The one measured
  number, 92 bytes, was right — which is how the rest survived. It also said
  the container "decompiles to the exact net above"; it decompiles to an
  equivalent net whose **layer names are regenerated** (`fc1` → `l_linear_1`),
  because the container stores ops, not identifiers. Everything else on that
  page does hold: form 1, form 3, the composition operators, the Quick Start
  snippet (→ `30`), the whole `forge` workflow (→ `120`), and all eight
  `mage-parse` targets it lists.

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
- **A module held at most one expression-body function.** `+f a(x) = x + 1`
  followed by another `+f` failed with `expected expression, found KwF 'f'`: an
  expression body runs to the end of the expression, and the `+` opening the
  *next item* was consumed as an addition operator whose right side was the
  keyword. So the form worked only for the last item in a file — which is why
  every example that used it happened to have one. `parse_expr_bp` now stops at
  a `+` that begins a line and is followed by a declaration keyword. Found by
  running `README.md`'s own second example, which had never been executed.
- **All four AI blocks reported the token kind instead of the word, in the
  same place.** Each spec section promises "a field the parser does not
  recognise is an error naming it", and each block has *two* rejection paths:
  an `Ident` arm that names the field, and a generic arm for everything else.
  The recognised fields are keywords, so any wrong word that happens to be a
  keyword lands in the generic arm — `agent A { brain: x }` said "unknown agent
  field `brain`" and `agent A { handle: x }` said "found KwHandle", the same
  mistake reported two ways with only one of them usable. `evolve` named
  nothing at all and `kb` named the alternatives but not the word. All four now
  name it and list the valid fields. Found by checking one block; the test that
  pins it caught the fourth, `swarm`, which I had not looked at. The `evolve`
  test also parses each of the seven fields it advertises, so the list cannot
  become a second wrong claim.
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

- **`${#arr[@]}` on a never-assigned associative array is *unbound*.** Under
  `set -u`, `declare -A ACTUAL` followed by a `read` loop that assigns nothing
  makes `[ "${#ACTUAL[@]}" -eq 0 ]` abort the line — so the empty-input guard
  in `check-doc-counts.sh` never ran, and the script went on to report
  "documentation disagrees with measurement" for all 74 claims when what had
  actually happened was that nothing was measured. Count with a plain integer
  assigned before the loop, so it is always bound. Corollary, and the reason
  this mattered more than the syntax: **a checker must distinguish "wrong" from
  "not checked"**, because the two have opposite remedies — edit the doc versus
  run the suite — and reporting the wrong one sends someone to change numbers
  that were correct.
- **`grep -c` exits 1 when the count is zero.** `cargo clippy … | grep -c
  '^warning' && git commit …` printed `0` and then did not commit: a clean
  result is a *failure* exit for `grep`, so the `&&` chain stopped. Same
  family as the `grep -q` pipefail trap below, opposite direction.
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

- Prototype tests **1,066 → 1,195**, all green — checked against the live run, so
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
