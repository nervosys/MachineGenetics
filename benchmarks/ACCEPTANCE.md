# Task acceptance: do the references type-check, and do they pass their own tests?

Measured 2026-11-17 by `scripts/task-acceptance.py`, then re-measured after the
task defects below were fixed (wrapper:
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
fixed in `benchmarks/tasks/*.json`, which is why the tables below read 34 and
20. The compiler and evaluator were not touched. Nothing here is a claim that
the remaining 79 tasks are wrong: most fail on a compiler or evaluator limit.

## Numbers

| Category | Tasks | Parse | Check-clean | All tests pass | Some test fails | Unrunnable | Accepted |
|---|---:|---:|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 10 | 6 | 0 | 5 | 5 | 0 |
| algorithms | 15 | 15 | 15 | 7 | 8 | 0 | 7 |
| basic-io | 10 | 10 | 10 | 9 | 1 | 0 | 9 |
| concurrency | 9 | 9 | 0 | 0 | 8 | 1 | 0 |
| data-structures | 15 | 15 | 14 | 4 | 11 | 0 | 3 |
| error-handling | 5 | 5 | 4 | 0 | 5 | 0 | 0 |
| full-applications | 10 | 10 | 4 | 0 | 9 | 1 | 0 |
| generics-traits | 5 | 5 | 3 | 1 | 4 | 0 | 1 |
| systems | 10 | 10 | 7 | 0 | 7 | 3 | 0 |
| web-network | 10 | 10 | 2 | 0 | 4 | 6 | 0 |
| **Total** | **99** | **99** | **65** | **21** | **62** | **16** | **20** |

Before the task fixes the same table read 98 parse, 52 check-clean, 18 all
pass, 65 some fail, 16 unrunnable, 18 accepted.

"All tests pass", "some test fails" and "unrunnable" partition the tasks by
their test cases alone, independent of the check column ("some test fails"
includes cases that could not be evaluated: see below). The 175 cases: 59 pass,
9 fail (ran, wrong value), 86 error (could not be evaluated), 21 unrunnable
(before: 56 / 9 / 89 / 21).
The 20 accepted tasks are the 7 algorithms (`algo-001/002/005/007/008/009/011`),
9 basic-io (all but `basic-008`), `ds-002`, `ds-011`, `ds-013` and `gt-001`.
`ds-004` passes its test but its reference does not type-check (a compiler
question: group A), so it is not accepted.

Every task now parses (`app-008`'s closure parameter was renamed).

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
  never changes a verdict. It is true for all 9 `fail` cases (`basic-008`,
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
* `web-010`: `async.sleep` does not resolve, and `base_delay_ms * (1 << attempt)`
  is typed `u64 * u32`, where Rust infers a `u64` shift. The `R.Ok` fix landed;
  the rest is a checker and library question.

### A. The reference does not type-check (34 tasks; first error shown)

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

### B. The reference type-checks but the tests do not pass (39 tasks)

These tasks are check-clean, so the failure is in running them. 34 of the 39
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

* "Check-clean 65" is a clean compiler verdict; it is the number the parse rate
  is most often mistaken for. Of the 34 failures, none is a typo in the task
  that survives: 13 depend on a library MAGE does not ship (see above) and 21
  are unclear or compiler-side (group A).
* "Accepted 20" is a **lower bound** on correct tasks: 34 of the 45 check-clean
  tasks that fail on tests involve an evaluator limitation. Equally it is not an
  upper bound on wrong ones: a case passes only if the evaluator and the task
  agree.
* The baseline can only shrink. When a task or the compiler is fixed so a task is
  accepted, delete its line (the check fails until you do); when a task changes
  from `check` to `test`, update the word.
