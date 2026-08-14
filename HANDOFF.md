# Handoff — through 2026-08-12

What changed, what state it is in, and what to look at next. Written for someone
picking this up cold.

---

## Where things stand

`master` is green and released. Everything below is verified, not asserted —
each claim has a command beside it.

| | |
|---|---|
| Tests | **2,836** — rmi 1,380 · prototype 1,128 · ribosome 164 · germline 112 · forge 52 |
| CUDA | **1,071 passing** on dual RTX 3090 Ti, driver 610.88 |
| Warnings | 0 compiler, 0 clippy in the four owned crates (`rmi` keeps 2 — vendored) |
| Vulnerabilities | 0 Rust across five lockfiles, 0 npm |
| CI | 10 jobs, green on `master` |
| Examples | 12 of 12 typecheck, run, and print their recorded answer |
| Release | `v0.3.0`, with the promo video attached as a release asset |

Reproduce all of it:

```sh
scripts/test-all.sh --check-docs          # everything + documentation check
scripts/check-examples.sh                 # the 12 shipped examples, end to end
scripts/test-all.sh --cuda --bench --check-docs   # + GPU and the benchmark harnesses
```

---

## What happened, in one paragraph

Three sessions. The first built out the Ribosome build engine and repaired a CI
pipeline red since June. The second was an accident: running documented commands
rather than reading them found **six typechecker bugs, one soundness hole, and
five stale documented figures**. The third — 2026-08-11/12 — rewrote all twelve
shipped examples, which turned out to be aspirational Rust that had never
compiled; doing so surfaced **fourteen compiler and evaluator bugs**, closed the
effect system's missing elimination rule, and corrected a section of the language
spec that described a language this compiler does not implement. The
repository's code has been in better shape than the claims about it at every
step, and the claims are what keep turning out to be wrong.

---

## The instruments now in place

These exist because the same failure kept recurring: a claim written once, never
revisited, and quietly wrong. Each fails in **both** directions — a claim that
breaks, *and* a recorded state that silently starts passing.

| Command | Checks |
|---|---|
| `scripts/test-all.sh --check-docs` | every documented test count against the run that just produced them; it prints how many it checked, and `--cuda --bench` adds the GPU and benchmark figures |
| `scripts/check-examples.sh` | that all 12 examples typecheck, evaluate, **and print the recorded answer** |
| CI `audit` job | `cargo audit` over all five lockfiles separately |
| CI ontology step | `MAGE_ONTOLOGY.json` matches a fresh `--emit-ontology` |
| CI version step | `mage-parse --version` matches the tool id Ribosome keys on |
| CI dependency guard | `ribosome` depends on no MAGE crate, no `germline`, and no TLS stack by default |
| `scripts/check-mg-sources.sh` | every `.mg` file in the repo typechecks, or is a listed sketch with a stated reason |

**If you add a measured claim to a document, add it to `CHECKS` in
`scripts/check-doc-counts.sh` in the same commit.** The one figure that stayed
stale after the checker existed was one nobody had listed.

---

## Open items

Nothing here is a surprise or a regression. They are grouped by what would
actually unblock them, because "open" has meant three different things.

### Waiting on a decision, not on work

| # | Item | The decision |
|---|---|---|
| 1 | GPU CI runner | Correctness **is** verified on the hardware here and recorded. What is missing is a self-hosted runner so `cuda-gpu` runs unattended — an account action, declined once already. |
| 2 | TLS trust posture | The transport seam and a `rustls` implementation exist behind `--features tls`. Pinned self-signed / mutual TLS / public PKI is deliberately the operator's; `acceptor`/`connector` take your config. |
| 3 | RAP error shape | An unknown method returns `{"result":{"error":…}}` — an HTTP-200-shaped success containing an error, not a JSON-RPC `error` member. Fixing it is a client-visible wire change, so it is a decision, not a cleanup. |

### Deliberate, and not defects

| # | Item | Why it stays |
|---|---|---|
| 4 | `rmi`'s 2 clippy warnings | Vendored; must stay syncable against its own upstream. |
| 5 | Ab-initio migration steps 2c & 3 | Declined as negative-sum (ROADMAP step 99). Revisit only with new measurements. |
| 6 | Single-workspace build | The crates are separate workspaces by design; `rmi` must stay independent. |
| 7 | External dependency resolution | Fetching third-party code is a distinct trust problem — provenance, pinning, revocation. Folding it into the planner is how build systems become unauditable. |

### Real work, unstarted

| # | Item | Size |
|---|---|---|
| 8 | **`resume` for effect handlers** | Largest. Handlers dispatch and return like ordinary calls; nothing captures a continuation, so there are no generators, no backtracking, and no async from the same mechanism. Needs the tree-walking evaluator reworked into a form that can capture continuations, which touches every expression form. |
| 9 | `int` literal constraint | Medium. The current fix is a post-hoc check in `default_int_literals`, not a real integer-kind constraint threaded through `unify`. Correct for the programs it rejects; the principled version is larger. |

### Small, sharp, cheap

Each is a contained fix, and each is a real trap someone will hit.

- **`guard` cannot be *referenced*, only bound.** `v guard = 2` binds fine, but
  `guard + 1` on its own line starts a guard statement and dies with
  `expected expression, found Plus '+'`. Found while fixing the sigil-letter
  diagnostic; it is the same shape one level over — a keyword-as-identifier that
  works in one position and not another — and the same class as `f` and `v`,
  which are still not usable as names anywhere. Making these letters real
  identifiers is the larger change the diagnostic fix deliberately did not make.
- **`scan` emits its seed** as the first element, so its result is one longer
  than its input. The two conventions differ by exactly one line, which is
  invisible until you count. Worth a doc comment at the definition.

---

## The example rewrite

Rewriting the examples was supposed to be a documentation task. It was not: the
examples exercised surfaces nothing else did, and **fourteen compiler and
evaluator bugs** fell out. Every one was found by running `--check` or `--eval`
on a small probe, never by reading the compiler.

The `use std::x;` line every example shared was only the *first* error in each.
A bulk `::` → `.` conversion was tried and every file then failed deeper on
constructs that had never existed (`data`, pipeline `|>`, `handle`). They were
aspirational Rust, and were rewritten rather than converted.

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
| 12 | `guard cond else { … }` **fell through** | `guard n > 0 else { 0 }` did not return `0`; it ran the body anyway with the precondition false, and `a(-5)` answered `-10`. Now a check-time error, as Rust's `let`-else requires |
| 13 | effect operations typechecked but did not evaluate | `Audit.record(x)` died with `unknown function`; an `effect` block declared operations nothing ever read |
| 14 | a **misspelled** operation was accepted | `Audit.recrod(x)` counted as performing `audit`, satisfied the annotation, checked clean, and died at run time |

Bugs 9 and 11 are the ones to learn from. Both survived because the broken parse
still produced *something*, and neither had a test that could tell the
difference: nothing had ever asserted which type an impl belonged to, and
nothing had ever applied a prefix operator to a call. In both cases **no test
noticed when the bug was fixed**. The regression tests now use two types
implementing one trait, and a `!` on a call — the shapes that expose them.

Seven of the fourteen — #5, #6, #7, #8, #9, #11, #13 — are one class: a bug that
**typechecks and then does not evaluate**. `--check` cannot find these. Only
`--eval` can, which is why the pin now runs it.

That rewrite took the prototype suite from 1,066 tests to 1,107. Counting every
session since, prototype tests **1,066 → 1,128**, all green — this figure is
checked against the live run, so it tracks forward rather than freezing at the
session that wrote it.

### The examples are pinned to their output, not to their exit status

`check-examples.sh` used to record which examples typechecked. That bar was too
low twice over. Two examples typechecked, ran, and printed the **wrong answer**:

- `cli-tool` searched with `contains`, which is element membership and not
  substring search, so its grep matched nothing and reported `0`;
- `autonomous-pipeline` filtered its worklist on readiness rather than on what
  had been placed, and called three of five tasks unplaceable on an acyclic
  graph.

Neither is visible from an exit status; both are obvious the moment the output
is read against what the example claims to demonstrate. So the script pins the
answer, and `--print` regenerates the block after an intentional change. It has
already earned this: it caught `effects-showcase` changing when the effect
handlers landed, and refused to bless it silently.

---

## The effect system got its elimination rule

Effects could be declared, annotated, inferred, and enforced, but never
*discharged* — `/ audit` propagated outward forever and the only way to satisfy
it was to keep declaring it. Three parts landed together, because none is useful
alone.

- **Introduction.** `Audit.record(x)` performs the operation, is checked against
  the signature in the `effect` block, and puts `audit` in the calling
  function's effect set.
- **Elimination.** `handle { … } with Audit { record(e) => … }` removes the
  effect from the block it wraps, so a function can be pure despite calling
  something effectful. The subtraction is **per block, not per function**: an
  unhandled call sitting beside a handled one still reports. What the arm itself
  does is attributed honestly, so handling `audit` by writing a file makes the
  handling function `/ fs`. A handler exchanges one effect for the effects of
  handling it, and says so.
- **Declaration.** An operation the effect does not declare is an error, and so
  is an effect annotation naming nothing. `/ nte` used to be accepted as a
  *different effect* from `/ net` — enforced consistently and matching nothing.

Handlers are found **dynamically** (innermost wins) and evaluated **lexically**
(an arm sees the scope the handler was written in). Both are tested, as is the
stack discipline that stops a handler outliving its block.

**Handlers do not resume.** See open item 8. This is the single largest piece of
unbuilt work in the language.

The implementation matches the spec's `[E-Handle]` rule in §11.3 exactly — body
type preserved, handled effect removed. That was convergence, not compliance:
§11.3 was read afterwards, while checking whether the change contradicted the
spec. It did contradict §11.4, which is the next section.

---

## The spec described a language this compiler does not implement

`MAGE_SPEC.md` §11.4, in full, before this session:

> Effects are inferred bottom-up. Explicit annotations are optional documentation.

The second sentence had been false for as long as the boundary check existed. A
`pub` function performing an undeclared effect is an **error**, and any function
that annotates at all is held to `inferred ⊆ declared`. "Optional documentation"
describes a language where the capability gate does not exist.

§11.4 now states the rule the compiler enforces — private infers, published
declares, over-declaration is an upper bound — and a new §11.5 gives the
concrete syntax for performing and handling. Every claim in the new text was
checked by running it; the §11.5 example is a real file that reports
`f transcribe: { audit }`, `f summarize: pure`, and evaluates to
`"recorded 11 chars"`.

**§11.2 was read with suspicion, and every row was run.** See the next section.

---

## Four documents disagreed with the compiler about effects

The task was one line of the list above: `/ agent` is a parse error, and `agent`
is a documented effect. Fixing it took two changes — `agent` lexes as the
keyword introducing an `agent` item, so the annotation never parsed, *and* it
was not a built-in kind either, so it would have failed the unknown-effect check
if it had. Two independent failures on one row of one table.

Then the rest of §11.2 was run, which is what the previous handoff had asked
for. The other sixteen names all worked. But the table's middle column,
labelled **Operations**, turned out to name 41 operations of which **22 perform
nothing** — `dispatch`, `generate`, `lifecycle`, `random`, `forward` and the
rest have no `effect` block behind them and are attributed by nothing, so a
function calling them is pure. §11.2 now says *domain*, and carries a second
table of the names really attributed on call. Both tables are pinned by tests.

Running out from there found the same disagreement in three more places:

| Document | Claimed | Actually |
|---|---|---|
| `MAGE_ONTOLOGY.json` | ten `effect_name`s, incl. `db`, `log`, `tools`, `rand` | four of ten rejected as unknown effects; ten built-in kinds absent |
| `internals/03-ast-hir.md` | an `Effect` enum with `Io`, `Unsafe`, `Db`, `Log` | none of those variants exist; eleven real ones missing |
| `internals/05-effects-resolution.md` | §5.3 hierarchy, §5.4 `Forge.toml` grants, §5.5 `/ *` | none implemented; `/ *` is a parse error |

The ontology one is the worst of the three, and it is not a documentation bug.
That file is what an agent grounds on — the handoff above says to read it
*before guessing* — and CI already pinned it against a fresh `--emit-ontology`.
So the check was working perfectly and guaranteeing that a wrong answer stayed
byte-identical. `db`, `log` and `tools` are capability **namespaces**
(`db.query(…)`, from `resolve.rs`), a different list that had been folded into
this one; `rand` was `rng` under a name nothing has ever accepted. The published
list is now the sixteen built-in kinds, and a test asserts each one both parses
and checks — which is the property the old test, asserting only that the *name
appeared in the file*, could never have caught.

The internals sections are marked rather than rewritten: `is_sub_effect`,
`[capabilities] allow-io` and effect polymorphism are somebody's design, and
choosing whether to build them is not a cleanup. What is stated now is which
parts are unbuilt and what the compiler does instead. §5.6, which describes a
feature that *does* exist, had the wrong syntax throughout (`handle / Rng { … }
with { Rng.op(…) => … }`); it is rewritten to the real form and every snippet in
it was run — the example evaluates to `5`.

The lesson is the previous handoff's, one level up. It says a claim checked in
only one direction goes stale; the ontology shows a claim can be checked in both
directions, byte for byte, against a generator that is itself wrong. **A pin
guarantees agreement, not truth.** What was missing was the check that crosses
the boundary: not "does the file match the generator" but "does every name the
file publishes actually work". Six new tests, all green.

---

## A newline did not end a statement before `(`

The next item on the small-and-sharp list said a `(` after a block parses as
calling the block, and that *any statement in between separates them*. The first
half was right. The second half was wrong, and it was hiding the real bug:

```mg
v a = 1
(2 + 3)
```

This called the literal `1`. Nothing about blocks is involved. Statements here
are newline-terminated, and the postfix loop could not see the terminator — so
**any** expression followed by a line starting with `(` was a call. The block
case was just the one anybody had noticed, because a block is the shape you most
often want to follow with a parenthesised line.

`[` had it too: an array literal opening a line was an index on the previous
statement.

The fix is the rule the parser already applied two arms up. The postfix `?`
breaks on `newline_before_current()` — with a comment explaining that `7\n ? c
{}` would otherwise parse as `(7?)…` — and `expect_stmt_end` treats a newline as
a terminator. `(` and `[` now break on the same condition. A multi-line argument
list is untouched, because its `(` hugs the callee on the callee's own line.

Two things worth carrying:

**The bug report was a partial observation stated as a rule.** "Any statement in
between separates them" is what you would conclude after trying it once with a
binding in between and seeing the error move — which it does, to a *different*
wrong parse. Reproducing the reported symptom is not the same as reproducing the
bug, and the first probe here disagreed with the note.

**The tests were checked by breaking the fix.** Both new tests were run against
a build with the two guards deleted, and both fail there. This is the check the
handoff says was missing for bugs 9 and 11 — where nothing noticed the fix — and
it is worth doing every time, because the first version of these tests passed
with the guards removed. They asserted statement counts, and the tail expression
is not a statement.

---

## The sigil-letter diagnostic blamed the binding keyword

The item said naming a variable `f` yields `unresolved name: val`. Running it
gave two *different* wrong errors, depending on the letter:

| Written | Reported | Wrong how |
|---|---|---|
| `v f = 3` | `expected expression, found KwF 'f'` | right token, wrong story — nothing there wants an expression |
| `var v = 3` | ``unresolved name: `var` `` | wrong token entirely, and it names a keyword the author wrote correctly |

The mechanism explains both. `is_let_statement` decides whether `v`/`val` starts
a binding by peeking at the next token against a list of keywords that may serve
as names — `val`, `guard`, `data`, `query` and a couple of dozen more, because
the lexer eagerly tokenises ordinary variable names as keywords. The sigil
letters are not on that list. So the statement is not recognised as a binding at
all, the binding keyword falls through to expression position, and whatever
error comes out is about the wrong token by construction. `unresolved name:
var` is not a bad message; it is a correct message about a parse that should
never have happened.

The fix keeps the list (extracted, so it has one home instead of being inlined
in the peek) and claims the failing shape anyway: a binding keyword, a keyword,
then `=` or `:` is unambiguously a binding whose name is a keyword, so it routes
into `parse_let_stmt`, which says exactly that and points at the name. The hint
naming the other spelling — `f` is `fn`, `S` is `struct`, `ret` is `return` — is
computed from `lexer::KEYWORDS`, so it cannot drift from the table that causes
the collision in the first place.

All thirteen names now report at the name's own line and column. What has *not*
changed is that they cannot be used as identifiers; that is the larger change,
and the handoff was right that the diagnostic is separable from it.

Two notes:

**The bug report was again a partial observation.** `unresolved name: val` is
what one of the two failure modes looks like, generalised into a rule covering
both. This is the second small-and-sharp item in a row where the first probe
disagreed with the note — worth expecting for the rest of the list.

**Verified by breaking the fix**, as with the `(` guard: the test fails against
a build with the new check deleted. Also worth recording is a failure that is
*not* mine — `v guard = 2` followed by `guard + 1` was already broken before
this change, confirmed by stashing the file and rebuilding rather than by
reasoning about it. It is now its own item above.

---

## The capability gate was open at its own documented entrance

This is the most serious thing found so far, and it was found by checking an
assumption rather than a claim. The effect system is what lets you hand an
autonomous agent a program and bound what it can reach — so the question worth
asking is not "does the checker work" but "does it work on the code people
actually write".

`resolve.rs` registered twenty capability namespaces, with this comment:

> MAGE is effect-oriented: I/O is performed through capability handles
> (`io.println(..)`, `fs.open(..)`, `net.connect(..)`, `llm.complete(..)`)
> whose use is tracked by the effect system.

Nothing tracked them. With a control to show the checker was live:

| `pub` function declared **pure** | Before |
|---|---|
| `println(s)` | caught — `performs undeclared effects: [IO]` |
| `io.println(s)` | **clean** |
| `fs.open(p)` · `net.connect(h)` | **clean** |
| `llm.generate(p)` · `gpu.dispatch(k)` | **clean** |

Same shape, same visibility. The bare name was caught; the namespaced form —
the one the design calls primary — passed. A generated program could open
sockets, call out to an LLM, or dispatch to the GPU while advertising itself as
pure. **The gate was open at exactly the seam the language documents as the way
through it**, which is the worst possible place: the safe-looking code was the
unchecked code.

The fix attributes the effect from the receiver, which is the rule
`Audit.record(x)` already used for declared effects — the capability is named by
the receiver, so the receiver is what gets attributed. `hir::CAPABILITY_NAMESPACES`
is now the single source of both the registered names and the attribution, so a
namespace cannot be added without deciding what it performs.

Three things fell out of doing it:

**A seventeenth built-in effect, `proc`.** `os`, `sys`, `process` and `tools`
had no kind that named them. Attributing them to `io` would understate them
badly — `process.spawn(…)` is arbitrary code execution, and for a program a
human did not write that is *the* question. `tools` is grouped with it because
invoking an external tool is the agentic spelling of the same capability.

**`agent.spawn(…)` did not parse**, and the new test is what found it. `agent`
lexes as the keyword introducing an `agent` item, so it failed in expression
position exactly as it had failed in annotation position earlier this session —
the same root cause, one seam over, and the earlier fix had not generalised.
`swarm` and `kb` were the same; `net` had been fixed long ago in isolation.

**`json`, `kb` and `db` attribute nothing, deliberately.** `json` computes over
values in hand. `kb`/`db` reach a store no built-in kind names, and inventing a
`Custom` here would be worse than nothing: the checker would infer an effect
that §11.4 then refuses in an annotation, leaving no way to declare what you
perform. `effect Db { … }` is the path that works, and `effects-showcase` uses
it.

Nothing shipped and verified broke — the only in-repo callers are in `stdlib/`,
which does not parse today and which no script or CI checks. That is its own
finding, and it is the examples' story again one directory over.

**What this says about the goal.** A capability system is only as good as its
worst-covered call shape, and coverage is not something a test suite reports —
every test here passed before and after. The gap was visible only by asking
"what does a program written the documented way actually check as", which is a
different question from any the suite was asking.

---

## The ontology audit: every section, executed

The effects section had been 40% wrong, so the obvious question was what the
other twenty-one sections would say if anyone ran them. Result:

| Section | Verdict |
|---|---|
| `project_layout`, `docs` (29 paths) | clean — every path exists |
| `vocabulary` (31) | clean — every name resolves |
| `rap_methods` (37) | clean — every method is in the dispatch table |
| `keywords` (102) | **19 wrong**, and wrong in a way that made the field unusable |
| `types` (30) | **3 unusable** |
| `sigils` (38) | **2 unimplemented** |

**`keywords.introduces` was two fields wearing one name.** For the 83 keywords
with no curated entry it was the real token kind (`KwC`). For the 19 with one it
was a hand-written name: `agent` claimed `AgentDef`, `match` claimed `Match`
(it is `QuestionEq`), and `val`/`var` claimed `Let` — a token this language
*removed* and now rejects on sight with a dedicated diagnostic. Nothing marked
which kind of answer you were reading, so the field could not be joined against
anything. It is now always the token the spelling produces, and a test joins it
back against `lexer::KEYWORDS`. The curated prose survives in `summary`, where
it was always the useful part.

**Three documented types could not be written.** `S` was published as a sigil
shorthand for `String` — but `S` is the `struct` keyword and can never be a
type. `Map[K,V]` and `Set[T]` are spelled `{K: V}` and `{T}`. An agent that
believed the ontology emitted a program that did not compile, which is the
precise failure this file exists to prevent.

**Two published sigils did not parse — and they failed for opposite reasons.**
`!` is published as Break and `^` as Return; neither worked, and `break` / `ret`
did.

`!` was a genuine gap. The spec names `!` as Break in both of its sigil tables,
and the lexer's own comment reads
`KwBreak, // break (legacy — canonical is !)` — so the spelling it calls
canonical parsed nowhere and the one it calls legacy was the only one that
worked. Prefix `!` always demanded an operand. It is now `break` when no operand
follows, which is unambiguous: logical-not requires one and break cannot take
one. A parser comment had claimed this was "handled via context in statement
parsing"; the only other `Bang` arm is the `!` *type* (Never). Fifth comment
this session stating a rule the code beside it did not implement.

**`^` was the ontology being wrong, and I got this backwards first.** I
implemented `^` as return to make the published claim true — then had to add a
newline guard when `m x = 7` followed by `^ x` parsed as `7 ^ x`, because `^` is
already an infix operator. That guard was the warning sign, and I only checked
the spec afterwards. `MAGE_SPEC.md` names `ret` in both sigil tables and
mentions `^` exactly once, as bitwise XOR — and `^` is *also* the `^T` Box type
prefix, so it was double-booked before anyone proposed a third meaning. Nothing
but that one ontology line ever claimed `^` returns. Both changes are reverted;
the ontology entry now reads `ret`.

The lesson is sharper than the fix: **the ontology describes the language, it
does not define it.** When a generated artifact and the spec disagree, the
artifact is the likelier suspect, and "make the compiler match the document" is
the wrong instinct — it is the same instinct that would have kept `Map[K,V]` and
invented a `Map` type. Reaching for a special-case parser guard to support a
one-character spelling is a signal that the spelling is wrong, not that the
parser is.

**Why the sigil section matters more than its count suggests.** These are the
*token efficiency* claims. The short forms are exactly what an agent minimising
tokens reaches for, so the sigils nobody had run were the ones most likely to be
used. Same for `S` over `String`. Optimising for token count while never
executing the short spellings is how a language ends up with its cheapest forms
broken.

Four sections now have executable pins: every keyword introduces the token it
claims, every documented type can be written in a signature, every published
control sigil parses, every published path exists.

---

## 36 of 126 `.mg` files in this repo are not MAGE

`check-examples.sh` pins the twelve in `examples/`. Nothing looked at the other
eighty-nine. Of those, **36 do not typecheck** — and the largest block is the
standard library.

`stdlib/` is 25 files, 4,402 lines, and all of it is Rust: `pub trait Read`,
`&mut self`, `let total = 0usize;`, `pub mod agent;`. Nothing read it. The
compiler never opens `stdlib/`, no script checked it, and `u std.io` resolves
*without* it because imports are nominal. So 25 files sat there claiming to be
the standard library of a language they were not written in, and an agent
reading `stdlib/std/io.mg` to learn MAGE idiom learned Rust.

`scripts/check-mg-sources.sh` is now that consumer, and runs in CI. Every `.mg`
file typechecks or is listed with a reason; the list can only shrink. Running it
the first time found eleven more beyond `stdlib/` — a second, older example set
in `prototype/examples/` that had drifted exactly as the shipped twelve did, and
four `framewerx` modules referencing `Tensor`, `Module` and `ParamStore`, type
names nothing defines and neither the ontology nor the spec publishes.

Two real compiler bugs came out of reading the failures:

**Keyword-as-identifier, fourth and fifth positions.** `M agent { }` and
`u std.agent` were parse errors. This is the same collision fixed twice earlier
today — in effect annotations, then in expression position — each time patched
only where it had been noticed. `agent` and `swarm` are now in `expect_ident`,
which covers every identifier position at once. That is the fix that should have
been made the first time; patching per-position had a 100% record of leaving
another position broken.

**The prelude reserved eighty words globally.** `M net { }` reported
`duplicate definition: net` — against a definition the author never wrote and
could not see, because the resolver registers the capability namespaces, the
vocabulary and the builtin functions into the same root scope as the program's
own items. That makes the obvious module names for a standard library — `io`,
`net`, `fs`, `agent` — unusable, which is precisely the shape `stdlib/` needs.
Source definitions now shadow prelude names; two source definitions of the same
name are still a duplicate, and there is a test for that specifically, because
a shadowing rule that quietly disabled duplicate detection would be worse than
the bug it fixed.

### A correction

Last section I wrote that nothing shipped and verified broke when the capability
gate closed. That came from a grep over `examples stdlib framework` — which
misses `prototype/examples/`. One file there, `hello.mg`, did regress: it calls
`io.println` and declared no effects, so the new gate caught it. That is the
gate working as intended, and the file now declares `/ io`; but the claim was
made from an incomplete search and stated without qualification. The two other
capability users in that directory already declared their effects.

The instrument above exists partly because of this. A repo-wide check is not a
substitute for looking, but it is a substitute for *remembering every directory*
— and that is the part I got wrong.

---

## Traps this session hit, so you do not

- **`cmd | grep -q` under `set -o pipefail`.** `grep -q` exits at the first
  match, the writer takes SIGPIPE, and the pipeline reports failure — so every
  match reads as no-match. Cost two separate bugs, in
  `purge-video-from-history.sh` and later in `check-examples.sh` *despite a
  comment about it in the first file*. Capture the output, then match.
- **`bash` from PowerShell is WSL's**, which cannot open a `C:/…` path; and a
  Windows path with backslashes gets read as escapes. Prefer Git's bash and use
  repo-relative paths.
- **Shell mode bits do not survive a Windows checkout.** `chmod +x` locally is
  not enough; use `git update-index --chmod=+x`, and invoke scripts via `bash`
  anyway.
- **MAGE is not Rust.** Struct literals are `@P { x: 1 }`, not `P { x: 1 }`
  (which parses as a *map*). Booleans are `1b`/`0b` — there is no `true`.
  `MAGE_ONTOLOGY.json` and `--build=schema` answer these authoritatively; read
  them before guessing.
- **`remove_dir_all` then `create_dir_all` on the same fixed path is unsound on
  Windows.** Deletion returns while the directory is still open somewhere (an
  indexer, a scanner, an Explorer window), leaving it *delete-pending*: it
  exists, cannot be opened, and cannot be recreated. The next `create_dir_all`
  fails with `os error 183` — "cannot create a file when that file already
  exists" — and keeps failing forever, because nothing ever closes the handle.
  Three `forge` registry tests had been red on this machine since 2026-08-05 and
  the error pointed at the creation rather than the deletion that caused it.
- **Tight timeouts are flaky under whole-suite load.** A 300 ms connect to a
  just-spawned loopback worker in `ribosome` passed alone and failed inside
  `test-all.sh` with a Windows `os error 10060`, reddening everything. Both
  flaky tests found this week were invisible to CI and visible only locally, and
  both were found by running the **whole** suite rather than one crate.
- **`gh pr merge --auto` merges immediately** when the repo has no required
  status checks. It does not queue, and it does not warn. PR #4 landed on
  `master` with CI still pending because of this. If you want merge-on-green,
  either turn on branch protection or watch the run and merge by hand.
- **A tool's output means nothing until you know what you pointed it at.**
  `cargo audit` flagged `crossbeam-epoch` in `rmi`, which turned out to be a
  stale artifact in one working copy — `rmi` git-ignores its lockfile, so there
  was no committed pin to be stale. A local finding is not a repository finding.

---

## The pattern worth carrying forward

Every bug in the table above was found by **running** something. Not one came
from reading the compiler. `--version` did not exist. `--fix` was unreachable in
the documented argument order. Nine of thirteen RAP methods named in the roadmap
did not exist. The published ABL wire version was wrong. If you want to find the
next one, pick a surface nobody has run and run it.

Three counter-lessons, each of which cost real time this week:

**A comment agreeing with your expectation is not evidence.** It is usually what
the author *meant*. Three typechecker bugs were places where a comment stated
the correct rule and the code beside it did something weaker — "No else → must
be unit" returning the branch type; "unifies with any int width" enforcing
nothing; "Return the struct type" returning a fresh variable. The `guard`
fall-through was a fourth: the evaluator's own comment admitted it.

**Reading is not verification, including when you are the one who wrote it.**
An earlier draft of the "found and left alone" list claimed that a nested `"`
inside an f-string ends it early, and three examples were written around the
workaround. It is false — `f"{join(xs, ", ")}"`, method receivers, and two-deep
nesting all work. The claim came from a parse error whose real cause was never
isolated, and it was written *into a document about not doing that*.

**Fixing a class of bug confers no immunity to committing another instance of
it.** Bug 14 — a misspelled effect operation that typechecks and dies at run
time — was written into the first version of the feature built specifically to
eliminate that class, and shipped past twelve new tests, a green CI run, and the
output pin. What caught it was typing a probe with a deliberate typo in it, an
hour after claiming the class was closed.

---

## Notes on the shape of the work

- Every typechecker fix landed without breaking an existing test — **except
  one**, where widening `collection_elem` made `sum("hi")` legal and
  `vocab_rejects_non_collection` caught it. That is the datapoint showing the
  suite has teeth rather than merely being large.
- The five prototype commits in the example-rewrite PR were each tested in
  isolation (stage one file, stash the rest, run the suite), so that history
  bisects: 1,066 → 1,071 → 1,073 → 1,073 → 1,089.
- `git log` is the real record. The commit messages carry the reasoning,
  including the false starts, the two retractions, and the occasions where a
  pre-existing failure was nearly attributed to the change in hand.
