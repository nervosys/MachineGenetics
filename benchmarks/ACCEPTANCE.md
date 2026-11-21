# Task acceptance: do the references type-check, and do they pass their own tests?

Measured 2026-11-17 by `scripts/task-acceptance.py`, re-measured after the
task defects below were fixed, and re-measured again on 2026-11-18 on a tree
that also carries the evaluator fixes (`push`/`pop` on a receiver, `&`/`&!`
references) and five mechanical word renames (see "Re-measurement" below;
wrapper:
`scripts/check-task-acceptance.sh`, baseline: `task-acceptance-baseline.txt`),
over the 99 tasks in `benchmarks/tasks/*.json`, with `mage-parse` v0.3.0.

Every other figure about this corpus ("98/99", "parse 99/100") counts
**parsing only**. That is the weakest of three questions:

1. Does `solution.rdx_source` parse?
2. Does it type-check (`mage-parse --check --json`, zero error diagnostics)?
3. Does every entry of the task's `tests` array evaluate to its `expected` value
   (`mage-parse --eval`)?

A task is **accepted** when the answer to 2 is yes and every case in 3 passes.
This document is measurement and classification. The first measurement found 47
references that did not type-check and 18 accepted tasks; the task-side defects
among them (the reference or a test input is wrong as MAGE) have since been
fixed in `benchmarks/tasks/*.json`; with the evaluator fixes the tables below
read 34 not type-checking and 41 accepted. Nothing here is a claim that the
remaining 58 tasks are wrong: most fail on a compiler or evaluator limit.

## Numbers

| Category | Tasks | Parse | Check-clean | All tests pass | Some test fails | Unrunnable | Accepted |
|---|---:|---:|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 10 | 9 | 3 | 2 | 5 | 3 |
| algorithms | 15 | 15 | 15 | 15 | 0 | 0 | 15 |
| basic-io | 10 | 10 | 10 | 10 | 0 | 0 | 10 |
| concurrency | 9 | 9 | 0 | 0 | 8 | 1 | 0 |
| data-structures | 15 | 15 | 15 | 14 | 1 | 0 | 14 |
| error-handling | 5 | 5 | 5 | 5 | 0 | 0 | 5 |
| full-applications | 10 | 10 | 9 | 4 | 5 | 1 | 4 |
| generics-traits | 5 | 5 | 3 | 5 | 0 | 0 | 3 |
| systems | 10 | 10 | 9 | 6 | 1 | 3 | 6 |
| web-network | 10 | 10 | 5 | 3 | 1 | 6 | 3 |
| **Total** | **99** | **99** | **80** | **65** | **18** | **16** | **63** |

The table is the 2026-11-20 measurement, after twelve library words were
implemented (`clone`, `to_string`, `unwrap`, `unwrap_or`, `map_err`, `get`,
`is_empty`, `trim`, `starts_with`, `splitn`, `insert`, `remove`; see
`UNKNOWN_METHODS.md`). Six more tasks are accepted (`agent-004`, `app-009`,
`ds-007`, `gt-003`, `sys-008`, `web-006`): 45 to **51**, cases 114 pass / 1 fail / 39 error
/ 21 unrunnable, baseline 54 to 48 tasks. `remove` on a list returns the
element, as Rust's `Vec::remove` does and as `gt-003` uses it; a first version
returned `?A` and that task then ran to a wrong value. Three references had a char literal
where the word takes a string (`splitn(2, '=')` in `err-002` and `web-006`,
`starts_with(':')` in `app-005`), fixed as `split(':')` was on 2026-11-18. Then three
questions the specification left open were settled as Rust settles them: `!` on
an integer is the bitwise complement, a string literal is accepted for a `&s`
parameter, and a shift count may have another integer type. `sys-002` and
`sys-003` (`x & !(n)`) are accepted: 51 to **53**, cases 117 pass / 1 fail / 36
error / 21 unrunnable, baseline 48 to 46 tasks. The evaluator's integers carry
no width, so `!x` printed alone on an unsigned value shows the 64-bit
complement; masking with it is exact. Then a
second batch of words: `to_vec` and `iter` (the identity on a list), `ok_or`,
`as_bytes`, and the string forms of `find`, `contains`, `replace` and
`push`/`push_str`; and the string words that take a pattern accept a char as
well as a string, as Rust's do. Six more tasks are accepted (`agent-006`,
`agent-010`, `algo-004`, `app-002`, `err-002`, `gt-005`): 53 to **59**; type-check
76 to **80** (`agent-007`, `app-002`, `app-008`, `web-002`); cases 129 pass / 1
fail / 24 error / 21 unrunnable; baseline 46 to 40 tasks. Next blockers seen:
`parse` (`err-003`, `err-004`), `position` (`sys-006`, `app-008`), `parse_u16`
(`web-002`). Then a
third batch: `position`, `and_then`, `parse` (`s.parse::<T>()` with Rust's
error wording), and `map` on an option or a result. Four more tasks are
accepted (`app-008`, `err-003`, `err-004`, `sys-006`): 59 to **63**; cases 136
pass / 1 fail / 19 error / 19 unrunnable; baseline 40 to 36 tasks. Three
`expected` strings in `err-003` and `err-004` were placeholders (`...`) and
could not be compared; they now hold the value the reference computes, with
Rust's parse message ("invalid digit found in string"). The
paragraphs below describe the 2026-11-18 state (45 accepted) and are kept as
the record of it.

Before the task fixes the table read 98 parse, 52 check-clean, 18 all pass,
65 some fail, 16 unrunnable, 18 accepted; after them (evaluator as it was) 99,
65, 21, 62, 16, 20. The table above is the 2026-11-18 measurement after the
type-check pass (see "Re-measurement, 2026-11-18 (type-check pass and uniform
notation)"): before it, 65 type-checked, 44 / 39 / 16 and 41 accepted.

"All tests pass", "some test fails" and "unrunnable" partition the tasks by
their test cases alone, independent of the check column ("some test fails"
includes cases that could not be evaluated: see below). The 175 cases: 105 pass,
1 fail (ran, wrong value: `agent-009`), 48 error (could not be evaluated), 21
unrunnable (before the type-check pass: 102 / 3 / 49 / 21; on 2026-11-18 before
the notation and receiver fixes: 79 / 22 / 53 / 21; after
the task fixes, before the evaluator fixes: 59 / 9 / 86 / 21; before
either: 56 / 9 / 89 / 21).
The 45 accepted tasks are the 14 algorithms (all but `algo-004`), all 10
basic-io, 13 data-structures (all but `ds-006/007`), `err-001`, `err-005`,
`gt-001`, `sys-001`, `sys-004`, `app-007`, `web-004` and `web-010`. `gt-004` passes its
tests but its reference does not type-check (associated types: a language
decision), so it is not accepted.

Every task now parses (`app-008`'s closure parameter was renamed).

## Re-measurement, 2026-11-18

Tree: the task fixes above, plus the evaluator fixes (`push`/`pop` on a
receiver, `&` and `&!` references) and the E0714 warning. Counts, with the
2026-11-17 figures in brackets:

| | Task-fix tree | + evaluator fixes | + five renames |
|---|---:|---:|---:|
| parse | 99 | 99 | 99 |
| type-check | 65 | 65 | 65 |
| accepted | 20 | **28** | **28** |
| cases pass / fail / error / unrunnable | 59 / 9 / 86 / 21 | 79 / 22 / 53 / 21 | 79 / 22 / 53 / 21 |
| baseline lines | 79 | 71 | 71 |

The eight newly accepted tasks all come from the evaluator fixes (they were
`test` failures on `push`/`&`): `algo-006`, `algo-010`, `algo-012`, `algo-014`,
`algo-015`, `ds-009`, `ds-010`, `ds-014`. The five renames moved no task: the
sites they touched (`agent-006`, `app-001`, `app-010`, `gt-003`, `sys-006`,
`sys-010`, `web-005`, `web-010`) still fail on something else (`clone`, a type
error, a host effect, or output that is not a value). The 11 sites renamed in
`solution.rdx_source` were `contains_key` to `contains` (5), `io.write_line` to
`io.writeln` (3), `fs.read_string` to `fs.read_to_string`, `fs.write_string` to
`fs.write` and `async.sleep` to `time.sleep` (one each); the published word was
checked with `--ontology-query` and against `eval.rs` (`contains` tests map
keys). No `expected` value and no Rust twin was changed. `token_count` claims
are unchanged: each rename replaces one identifier by one identifier, and the
native-lexer ratio stays 0.980.

The measurement was run twice (and the baseline regenerated by a third run): the
status of every task is identical.

Remaining non-accepted tasks by first reason (71): type errors 34
(E0412 unresolved type 14: `Handle`/`AtomicI64`/`Mutex`, `Request`/`Response`,
associated types; E0308 11; E0425 8; E0428 1); evaluator or notation on a
check-clean task 31 (float / `?Some` / `R.Ok` / struct notation 12; a missing
method 9: `clone` 4, `as_bytes`, `get`, `splitn`, `iter`, `ok_or`; mutation
through a reference or a receiver the evaluator does not model 5; binary `&` 2;
`^expr` input 2; a `u64` above `i64::MAX` 1); unrunnable (host effects or
output that is not a value) 6. The prose in the sections below was written for
the 2026-11-17 counts: where it gives a count of the evaluator groups in B
(`.push` 13, unary `&` 15), those two groups are now gone and the groups above
replace them.

## Re-measurement, 2026-11-18 (value notation and list receivers)

Tree: the one above, plus three evaluator changes and fifteen `expected`
rewrites (below). Accepted 28 to **41**; type-check 65 (unchanged); cases pass /
fail / error / unrunnable 79 / 22 / 53 / 21 to 102 / 3 / 49 / 21; baseline
lines 71 to 58. The measurement was run twice: the status of every task is
identical. No existing unit test changed.

Thirteen tasks moved, by cause:

* **Float printing, evaluator only (5):** `algo-013`, `basic-008`, `ds-001`,
  `ds-005`, `ds-008`. A whole float printed as `32`; it now prints as the float
  literal is written, `32.0` (MAGE_SPEC section 3.5: a decimal number is a float
  only with a fractional part or an exponent, so `32` reads back as an
  integer). Every other float, `NaN` and the infinities print as before. The
  specification has no section on how a value is printed; this follows the
  literal grammar and the existing struct form, and is recorded as a decision
  to review (see "Questions").
* **Float printing and `expected` notation (1):** `err-001`.
* **`expected` notation only, same value (4):** `algo-003`, `err-005`, `sys-001`,
  `web-004`. The specification's constructors are `Some(e)`, `None`, `Ok(e)`
  and `Err(e)` (section 4.6); `?Some`/`?None` is a parse-time spelling of the
  same constructor, and `R.Ok` a Rust-style path, neither part of the printed
  value. `web-004` wrote a string value without its quotes.
* **`push`/`pop` on a list inside a type that declares its own `push`/`pop`,
  plus `expected` notation (2):** `ds-003`, `ds-015`. A user method named
  `push` made `self.items.push(v)` look like a call of that method, so the
  builtin never ran and the case failed with "`push` changes its receiver".
  The receiver now decides: a plain path (names, fields, indices that are names
  or literals) that currently holds a list takes the builtin. A computed
  receiver (`f().push(1)`) is still refused.
* **`xs.swap(i, j)` on a list place (1):** `ds-012` (`reverse(&!xs)` swapping
  through an exclusive reference). It is in place on the variable, field or
  index path, and the copy-out of `&!` returns it to the caller. An index out
  of range, or a receiver that is not a list, is an error. The two-place free
  function `swap(a, b)` is still refused: it needs two references at once,
  which copy-in/copy-out does not give.

Fifteen `expected` strings in nine tasks changed; the value is the same in
every one:

| task | before | after |
|---|---|---|
| `algo-003` | `?Some(2)` / `?None` | `Some(2)` / `None` |
| `err-001` | `R.Ok(5.0)` / `R.Err("division by zero")` | `Ok(5.0)` / `Err("division by zero")` |
| `err-005` | `?Some(3)` / `?None` | `Some(3)` / `None` |
| `sys-001` | `?Some(0)` / `?None` | `Some(0)` / `None` |
| `sys-003` | `?Some(0)` / `?None` | `Some(0)` / `None` (still fails: see below) |
| `sys-004` | `?Some(42)` | `Some(42)` (still fails: see below) |
| `ds-003` | `?Some(2)` | `Some(2)` |
| `ds-015` | `?Some(1)` / `?Some(2)` | `Some(1)` / `Some(2)` |
| `web-004` | `{"name":"app","version":"1.0","features":["fast"]}` | `"{\"name\":\"app\",\"version\":\"1.0\",\"features\":[\"fast\"]}"` |

The `test` group was 31 tasks (12 notation, 9 missing method, 5 mutation, 2
binary `&`, 2 `^expr`, 1 `u64`); the 18 now left are:

* **missing method, pending a language decision (9):** `agent-004`, `app-009`,
  `gt-003`, `sys-006` (`clone`), `algo-004` (`as_bytes`), `ds-007` (`get`),
  `err-002` (`splitn`), `err-004` (`iter`), `gt-005` (`ok_or`).
* **a reference that cannot be modelled (2):** `app-003` returns
  `&!self.tasks[i]` inside `?Some(...)`, an exclusive reference stored in a
  value; copy-in/copy-out has nowhere to write it back, so the precise error
  stays. `agent-010` calls `str.replace(a, b)`, which the evaluator reads as the
  two-place `replace(place, value)` and refuses; it would next stop on `clone`.
* **`!` on an integer (2):** `sys-002`, `sys-003` write `x & !(n)`. `!` is
  logical not here, and the specification gives no bitwise complement, so `&`
  meets a boolean. The error names `&`, but binary `&` on integers works; the
  cause is `!`.
* **`^expr` in a test input (2):** `app-010`, `ds-006`.
* **`u64` above `i64::MAX` (1):** `sys-009`.
* **the stated value is not the computed one (2):** `agent-009` computes
  `0.6 + 0.3 + 0.1` in IEEE doubles, `0.9999999999999999`, where `expected`
  says `1.0`; `sys-004`'s second case does `256 as u8`, which traps by section
  4.10 (the reference is at fault).

Questions the specification does not answer, left alone: how `--eval` prints
floats and strings generally (decided above for the whole-float case only);
whether `!` on an integer is a bitwise complement; what a reference solution
that casts `256 as u8` should mean.

## Re-measurement, 2026-11-18 (type-check pass and uniform notation)

The 34 references that did not type-check were classified from their exact
`mage-parse --check --json` diagnostics. Type-check 65 to **76**; accepted 41 to
**45** (`ds-004`, `app-007`, `web-010`, `sys-004`); cases pass / fail / error / unrunnable
102 / 3 / 49 / 21 to 105 / 1 / 48 / 21; baseline 58 to 54 tasks (23 `check`).
Measured twice, identical.

**Fixed in the checker (4 tasks; each is valid MAGE the checker rejected):**

* Open-ended slices `xs[a..]`, `xs[..b]` (`agent-005`, `agent-006`, `sys-008`;
  MAGE_SPEC 4.6 `e[a..b]`, and the parser's own comment says the checker
  "recognises `_` as unbounded"; it did not): the resolver reported the
  parser's `_` sentinel as an unresolved name. A range index now also types as a
  slice of the receiver (`[T]~`, or `str` for a string), where it typed as one
  element.
* A generic struct literal (`@Pair { .. }` for `S Pair[T]`, `ds-004`) had no type
  arguments and so never unified with `Pair[T]`; it now carries one fresh
  argument per parameter.
* `&str` passed to `split`/`chars`/`words`/`lines`/`upper`/`lower` (shared with
  `app-005`, `web-006`): `&str` is the string slice of section B.4, and
  `s.len()` already took it; the string words now auto-deref likewise.

**Fixed in the task (7 tasks, none changes `expected` or the Rust twin):**
`app-005`, `err-003`, `web-006` (a char literal where `split` takes a string:
`split(':')` to `split(":")`); `app-007` (`*n as i64` is `*(n as i64)` by the
precedence table of Appendix C, written `(*n) as i64`); `sys-007` (a string
literal passed to a `&s` parameter is written `&"PORT"`); `web-010` (`time.sleep`
needs `/ time`, the effect list was `io, async`; and `1 << attempt` is typed
`u32`, so the shift count is written `attempt as u64`); `app-006` (`R.Ok`/`R.Err`
to `Ok`/`Err`, as also in `agent-007`, `agent-008`, `app-004`, `sys-007`,
`web-007`, which still fail for another reason). One test input (`agent-004`,
`R.Ok(s)` in a closure) got the same constructor fix.

**Left, 23 `check` tasks:**

* **Unimplemented but specified (9): `conc-001` to `conc-009`.** `Handle[T]`,
  `@AtomicI64`, `@Mutex`, `async.spawn/join/select/yield` (sections 11.2 and
  14.2, "Atomic ordering"). `conc-003` also needs a generic effect
  (`effect channel[T]`) and every effect operation with its own parameters
  (`f spawn[T](..)`), which `kb/grammar.mg` defines (`EffectDef: Name Generics?`,
  `EfEffectOp: Name Generics?`) and the AST drops; `conc-005` also needs
  `I[T] Locked[T]`. Size: effect generics about 120 lines (AST, parser,
  resolver, effect-operation typing); `Handle`/atomics/`Mutex` as checker
  types about 150 lines, and single-threaded evaluator semantics another
  200-300. None is under the 80-line bar.
* **Needs a language decision (14):**
  string `find` with a string or a char (`agent-007`, `app-002`, `web-002`;
  `find` is `([A], pred)` in the 31 words); `s.from_utf8` and a byte type
  (`agent-008`, `sys-005`); string `push` and `contains` (`app-008`); associated
  types `type Item;` (`gt-002`, `gt-004`); the same variant name in two enums
  (`app-004`, flat namespace against `Expr.Num` and `Value.Num`); the HTTP types
  `Request`, `Response`, `Status`, `http.Client` and `use http` (`web-001`,
  `web-003`, `web-005`, `web-007`, `web-009`: the specification lists
  `std::net` HTTP only inside the section it marks invalid MAGE).
* Questions raised, not acted on: whether a string literal is accepted for a
  `&s` parameter (the checker says no; the specification does not say), and
  whether a shift count may differ in type from its operand (the checker
  unifies them; the specification is silent).

**Newly check-clean, still failing a test (not a checker or task cause):**
`agent-006` (`clone`), `app-005`, `app-006` (test input does not parse: nested
quotes in an f-string, a `...` placeholder), `err-003` (`trim`), `sys-008`
(`starts_with`), `web-006` (`splitn`); `agent-005` and `sys-007` are
`unrunnable` (host effects).

**Uniform `expected` notation.** The same value-preserving rewrite as above
(`?Some(`/`?None` to `Some(`/`None`; `R.Ok(`/`R.Err(` to `Ok(`/`Err(`, outside
quoted strings) was applied to every remaining `expected`: 29 strings, 31
tokens: `agent_orchestration.json` 3, `concurrency.json` 1,
`error_handling.json` 7, `full_applications.json` 5, `generics_traits.json` 3,
`systems.json` 4, `web_network.json` 6 (8 tokens); the other three files had
none. No `expected` string contains `?Some`, `?None` or `R.` any more.

**Two suspect references.** `agent-009` (not changed): `expected` says
`total:1.0`; `0.6 + 0.3 + 0.1` in IEEE doubles is `0.9999999999999999`, which
the Rust twin returns too (the same expression and operand order), so the
stated value is the error, not the evaluator. `sys-004` (fixed in the
reference, `expected` untouched): `write_u32_be(256)` did `256 as u8`; Rust's `as`
truncates to `0`, MAGE traps (section 4.10), so each byte is now masked,
`((val >> 8) & 255) as u8`, the faithful form of the Rust twin; accepted.

## How one case is judged

For each case the solution is written to a scratch file and one function is
appended:

    +f __ta_case() { <input> }

`input` is spliced verbatim, so multi-statement inputs such as
`v s = Stack.new(); s.push(1); s.pop()` work and the last expression is the
value. The file is run as `mage-parse --eval FILE __ta_case --fuel 5000000`
from an empty scratch directory. The last stdout line is the "got" string.

* **Fuel** bounds the run and makes it deterministic; running out is an error,
  never a pass. The fuel-bounded evaluator refuses struct/enum results ("takes
  and returns plain data"); only then is the case re-run without `--fuel`,
  under a 30 s wall-clock limit. A timeout is retried once, then is an error.
* **Comparison** is strict: `canon(got) == canon(expected)`, where `canon`
  deletes whitespace that is outside double-quoted strings and changes nothing
  else. `[1, 2]` equals `[1,2]`. `32` does **not** equal `32.0`, `Some(2)`
  does not equal `?Some(2)`, `"a"` does not equal `a`. No float, quote or
  constructor-name normalisation exists, and none is to be added to make a case
  pass.
* Statuses: `pass`; `fail` (a value was produced and differs); `error` (no value:
  evaluator error, the input does not parse, fuel exhausted, timeout). `fail`
  and `error` both count as "some test fails". The JSON keeps both strings.
* For information only, each `fail` carries `notation_only`: whether it would
  match after mapping `?Some(`/`R.Ok(` to `Some(`/`Ok(` and `3.0` to `3`. This
  never changes a verdict. On 2026-11-17 it was true for all 9 `fail` cases (3 remain: `agent-009`, `app-007`, `web-010`) (`basic-008`,
  `ds-001`, `ds-005`, `err-001`, `gt-002`, `web-010`).

### Unrunnable

A case is `unrunnable`, not passed and not failed, when a mechanical verdict
would be meaningless or unsafe. 21 cases, in 17 tasks:

* 18 cases: the solution declares a host-reaching effect (`fs`, `net`, `env`,
  `llm`, `tools`, `timer`) and `--eval` "really performs" such effects
  (`--describe eval`), e.g. `web-007` fetching `https://httpbin.org/get`. The
  rule is by declared effect, so it is conservative: `agent-005`'s input
  (`m.add(...).len()`) is pure, but the solution declares `llm`.
* 3 cases: `expected` is not a value literal: `R.Err(AppError.InvalidInput(...))`
  (`err-003`, placeholder `...`), and `[app] [INFO] started` (`sys-010`, printed
  output, not a value). A placeholder such as `<contents>` or prose such as
  "formatted report string" is also in this class; those cases are additionally
  masked by the host-effect rule.

Six tasks are `unrunnable` outright (`agent-001`, `agent-002`, `agent-003`,
`app-001`, `sys-010`, `web-008`): check-clean with every case unrunnable
(the first three became check-clean when their references were fixed). The other
10 tasks whose test column is `unrunnable` still fail the type check.

## Why tasks are not accepted, by group

Each group names the first (or only) reason; counts are tasks. "Task" means the
reference or the test is wrong as written. "Compiler" or "evaluator" means a
program that follows the spec is rejected or cannot be run. "Unclear" means the
spec text does not settle it.

### Task defects that were fixed

The first measurement attributed 30 reference failures and 3 test-input
failures to the task. They were fixed with the smallest change that keeps the
meaning of the Rust twin; no `expected` value was touched.

| Defect | Tasks fixed | What changed |
|---|---|---|
| `R.Ok(..)` / `R.Err(..)` where MAGE writes `Ok(..)` / `Err(..)` | 13: `agent-001/003/004`, `app-008/009/010`, `err-001/002/003/004`, `gt-005`, `web-002`, `web-010` (and the test input of `web-010`) | constructors only; `R[T, E]` stays as the type |
| Type the file never defines | 2: `ds-005` (`S Point { x: f64, y: f64 }`, as `ds-001` defines it), `app-010` (`trait Any {}`, standing for `std::any::Any`) | a definition added |
| Generic parameter on the wrong side of `I` | 2: `ds-013`, `sys-006` | `I[T] KeyValue[T]`, `I[V] LruCache[V]` |
| `tools` used as an effect | 2: `agent-002` (the effect was unused: the Rust twin's `execute` only calls the LLM), `agent-003` (`tools.compile` is a process call: `/ llm, proc`) | effect list, and the task's `effects` metadata |
| Closure parameter named with a keyword | 1: `app-008` (`fn(f)` to `fn(fld)`) | rename |
| Test input uses a keyword as a binding name | 2: `algo-006`, `ds-012` (`v` to `xs`) | input |
| Test input uses a bare `Name{..}` literal | `ds-011`, `ds-013` (now accepted), `ds-005`, `ds-004`, `agent-004`, `agent-009`, `agent-010`, `ds-008`, `web-004`, `web-009` (`@Name{..}`) | input |
| Test input uses `^expr` | 1: `ds-010` (`^Tree.Node(..)` to `Tree.Node(..)`) | input |

Newly accepted by this: `ds-011` and `ds-013`. Newly check-clean: 13 tasks.
`agent-004`, `agent-009`, `algo-006`, `ds-010`, `ds-012` and others now get past
the type check or their input and fail further on, on an evaluator limit.

Left alone, with the reason:

* `app-010`'s test input `^42`: `^` has no expression form; the evaluator has no
  boxed values to build one with. The input is left as written.
* `conc-*` (8 tasks): the references need `Handle[T]`, `@AtomicI64`, `@Mutex`
  and `async.spawn`/`async.join`. The spec names them (`std::sync`, line 2690
  and 2712) but the compiler has no such types, so a definition inside the file
  cannot give them atomic or thread meaning. That is a library gap, not a typo
  in the task. `conc-005` also needed `I[T]`, which is a one-token fix that was
  not applied while the file still fails on `Mutex` and `async`.
* `web-001/003/005/007/009`: the references use `http.{Request, Response,
  Status}` or `http.Client`, and a JSON module. The Rust twins do not define
  these either (they return tuples or use `reqwest`/`http`), so there is no
  twin definition to copy, and inventing an HTTP library per task would change
  what the task measures.
* `web-010`: `async.sleep` did not resolve (renamed `time.sleep` on 2026-11-18), and `base_delay_ms * (1 << attempt)`
  is typed `u64 * u32`, where Rust infers a `u64` shift. The `R.Ok` fix landed;
  the rest is a checker and library question.

### A. The reference does not type-check (34 tasks on 2026-11-17; 23 remain, see the type-check pass above; first error shown)

| Group | Tasks | Where the defect is | Example |
|---|---:|---|---|
| `Handle`, `AtomicI64`, `Mutex`, `async.spawn` (the file refers to a library the compiler lacks) | 8 (`conc-001/002/004/005/006/007/008/009`) | **Library gap**, see above. | `conc-002` `@AtomicI64.new(0)`: E0412 `AtomicI64` |
| `use http...` / `Request` / `Response` | 5 (`web-001/003/005/007/009`) | **Library gap**, see above. | `web-001` line 1 `u http.{Request, Response, Status}`: E0425 |
| Open-ended slice `x[a..]` | 4 (`agent-005/006`, `app-006`, `sys-008`) | **Unclear**, leaning compiler: the spec documents slice ranges only for tensors; the checker reports an unresolved name `_`. | `agent-005` `self.messages[start..].to_vec()`: E0425 `_` |
| `str`/`char`/`&str` mismatch in string methods | 7 (`agent-007`, `app-002`, `app-005`, `app-008`, `err-003`, `web-002`, `web-006`) | **Unclear**, leaning compiler: `split(':')` and `find(':')` are rejected with "expected str, found char", and `chars()` / `lines()` on a `&str` with "expected str, found &str". Three of these (`app-008`, `err-003`, `web-002`) were hidden behind a task defect until it was fixed. | `err-003` `trimmed.split(':')`: E0308 |
| `s.from_utf8(..)` (type name used as a path) | 2 (`agent-008`, `sys-005`) | **Unclear**, leaning compiler: `s` is the string type's name, and an associated function on it is not resolved. | `sys-005` `s.from_utf8(bytes)`: E0425 `s` |
| Associated types in traits (`type Item;`) | 2 (`gt-002`, `gt-004`) | **Unclear.** The spec text found does not define them; the checker reports unresolved type `_`. | `gt-004` `T Iterator { type Item; ... }`: E0412 |
| Generic effect declaration `effect channel[T]` | 1 (`conc-003`) | **Unclear.** No spec text defines parameterised effects. | `conc-003` line 1: E0412 `T` |
| Same variant name in two enums | 1 (`app-004`) | **Language limitation** (flat namespace), valid in Rust. | `Num` in `Expr` and `Value`: E0428 |
| Other E0308 (generic struct literal arity, `*n` on a by-value binding, `str` vs `&str` effect operand, `u64 * u32` shift) | 4 (`ds-004`, `app-007`, `sys-007`, `web-010`) | **Unclear**, leaning compiler. | `ds-004` `@Pair { first: p.second, second: p.first }`: "type argument count mismatch" |

Of the 34, none is a plain typo any more. The 8 + 5 library-gap tasks are the
ones where the task as written depends on a library MAGE does not ship;
whether MAGE should ship it is a separate decision.

### B. The reference type-checks but the tests do not pass (31 tasks; 39 before the evaluator fixes)

These tasks are check-clean, so the failure is in running them. (The counts in
this table are those of 2026-11-17; see "Re-measurement" for the current groups.)
34 of the 39
involve **an evaluator that cannot run a program the checker accepts**; those
are evaluator limitations, not task defects, and the task may well be correct.
4 are a formatting-versus-notation question and 1 is a test-input defect.

| Group | Tasks | Where the defect is | Example |
|---|---:|---|---|
| `.push` unknown to `--eval` | 13 | **Evaluator.** `.push` type-checks, appears in shipped `prototype/examples/agent_rpn.mg` line 28, and `--eval` of that file fails with the same message. | `algo-010` `rotate_left([1,2,3,4,5], 2)`: eval error unknown function `push` |
| Unary `&` / `&!` / `*` unsupported by `--eval` | 15 (13 for `&`, 2 for `&!`) | **Evaluator.** `&` is a documented borrow (`MAGE_SPEC.md`); the evaluator has no references. | `algo-003` `binary_search(&[1,3,5,7,9], 5)`: eval error unsupported unary `&` |
| Floats and structs printed without `.0`; `Some` without `?`; `Ok` without `R.` | 4 (`basic-008`, `ds-001`, `ds-005`, `err-001`) | **Evaluator formatting vs corpus notation**; unclear which should move. Strict comparison fails these. | `basic-008` `square(3.0)`: expected `9.0`, got `9` |
| Other missing method (`as_bytes`, `clone`, `splitn`, `ok_or`) | 5 (`algo-004`, `agent-004`, `app-009`, `err-002`, `gt-005`) | **Evaluator.** | `algo-004` `is_palindrome("racecar")`: unknown function `as_bytes` |
| `u64` constant above `i64::MAX` | 1 (`sys-009`) | **Evaluator** (integers are `i64`). | `sys-009` `fnv1a(b"")`: bad int `14695981039346656037` |
| Test input uses `^expr` | 1 (`app-010`) | **Task (test)** and evaluator. `^` is a type-only sigil; there is no expression form. | `^42` does not parse |

The test-input defects found by the first measurement (`v` as a name, bare
`Name{..}` literals, `^expr` in `ds-010`) are fixed; `algo-006`, `ds-012` and
`ds-010` now stop at the evaluator's lack of `&`/`&!`.

### C. Test cases blocked behind a type error, or not evaluable

For the 34 check-failing tasks the tests mostly cannot succeed either: of their
cases, 4 pass, 2 fail, 25 error, 13 are unrunnable. `app-005` and `app-006`
have inputs that do not parse (nested quotes in `f"hi {p["name"]}"`,
`[TokenRule{..}]`; **unclear** whether the input or the lexer is at fault);
`conc-001/003/004` fail at `async.spawn`, and `conc-002/005/008` at
`AtomicI64.new`/`Mutex.new`, because the evaluator does not implement them (and
the checker already rejects the files).

## Reading the numbers

* "Check-clean 76" is a clean compiler verdict; it is the number the parse rate
  is most often mistaken for. Of the 23 failures, none is a typo in the task
  that survives: 9 need concurrency types the compiler lacks and 14 need a
  language decision (see "Re-measurement, 2026-11-18 (type-check pass)").
* "Accepted 45" is a **lower bound** on correct tasks: most of the 32 check-clean
  tasks that are not accepted involve an evaluator limitation. Equally it is not an
  upper bound on wrong ones: a case passes only if the evaluator and the task
  agree.
* The baseline can only shrink. When a task or the compiler is fixed so a task is
  accepted, delete its line (the check fails until you do); when a task changes
  from `check` to `test`, update the word.
