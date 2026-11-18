# Task acceptance: do the references type-check, and do they pass their own tests?

Measured 2026-11-17 by `scripts/task-acceptance.py` (wrapper:
`scripts/check-task-acceptance.sh`, baseline: `task-acceptance-baseline.txt`),
over the 99 tasks in `benchmarks/tasks/*.json`, with `mage-parse` v0.3.0.

Every other figure about this corpus ("98/99", "parse 99/100") counts
**parsing only**. That is the weakest of three questions:

1. Does `solution.rdx_source` parse?
2. Does it type-check (`mage-parse --check --json`, zero error diagnostics)?
3. Does every entry of the task's `tests` array evaluate to its `expected` value
   (`mage-parse --eval`)?

A task is **accepted** when the answer to 2 is yes and every case in 3 passes.
This document is measurement and classification. It does not fix any task or
the compiler.

## Numbers

| Category | Tasks | Parse | Check-clean | All tests pass | Some test fails | Unrunnable | Accepted |
|---|---:|---:|---:|---:|---:|---:|---:|
| agent-orchestration | 10 | 10 | 2 | 0 | 5 | 5 | 0 |
| algorithms | 15 | 15 | 15 | 7 | 8 | 0 | 7 |
| basic-io | 10 | 10 | 10 | 9 | 1 | 0 | 9 |
| concurrency | 9 | 9 | 0 | 0 | 8 | 1 | 0 |
| data-structures | 15 | 15 | 12 | 1 | 14 | 0 | 1 |
| error-handling | 5 | 5 | 1 | 0 | 5 | 0 | 0 |
| full-applications | 10 | 9 | 2 | 0 | 9 | 1 | 0 |
| generics-traits | 5 | 5 | 2 | 1 | 4 | 0 | 1 |
| systems | 10 | 10 | 6 | 0 | 7 | 3 | 0 |
| web-network | 10 | 10 | 2 | 0 | 4 | 6 | 0 |
| **Total** | **99** | **98** | **52** | **18** | **65** | **16** | **18** |

"All tests pass", "some test fails" and "unrunnable" partition the tasks by
their test cases alone, independent of the check column ("some test fails"
includes cases that could not be evaluated: see below). The 175 cases: 56 pass,
9 fail (ran, wrong value), 89 error (could not be evaluated), 21 unrunnable.
The 18 accepted tasks are the 7 algorithms (`algo-001/002/005/007/008/009/011`),
9 basic-io (all but `basic-008`), `ds-002` and `gt-001`.

The only task that does not parse is `app-008` (a closure parameter named `f`).

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
  never changes a verdict. It is true for 5 of the 9 `fail` cases (`basic-008`,
  `ds-001`, `gt-002`).

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

Three tasks are `unrunnable` outright (`app-001`, `sys-010`, `web-008`): check-clean
with every case unrunnable. The other 13 tasks whose test column is
`unrunnable` also fail the type check.

## Why tasks are not accepted, by group

Each group names the first (or only) reason; counts are tasks. "Task" means the
reference or the test is wrong as written. "Compiler" or "evaluator" means a
program that follows the spec is rejected or cannot be run. "Unclear" means the
spec text does not settle it.

### A. The reference does not type-check (47 tasks; first error shown)

| Group | Tasks | Where the defect is | Example |
|---|---:|---|---|
| `R.Ok(..)` / `R.Err(..)` / `R` used as a value | 10 | **Task.** The spec's constructors are `Ok(e)`/`Err(e)` (`MAGE_SPEC.md` line 790, `PxSumCtor`); `R[T,E]` is only the type. | `agent-003` line 15 `? result.success { ret R.Ok(code); }`: E0425 unresolved name `R` |
| Types the file never defines and the compiler does not have (`Handle`, `AtomicI64`, `Request`, `Response`, `Point`, `Any`) | 12 | **Task** as written (the file refers to a library that does not exist). Whether MAGE should ship these is a separate decision. | `conc-002` `@AtomicI64.new(0)`: E0412 `AtomicI64`; `ds-005` uses `Point` without declaring it |
| Generic parameter not in scope on an `impl` | 3 | **Task.** `MAGE_SPEC.md` line 701: `impl [generics] Type`, so generics go after `I`. | `ds-013` `I KeyValue[T] {`: E0412 `T` (also `conc-005`, `sys-006`) |
| Undeclared / unknown effect | 2 | **Task.** `tools` is not a built-in effect (the spec lists `proc` for tool invocation). | `agent-002` line 28 `+f execute(&!self) / llm, tools`: E0550 |
| `use http...` imports | 2 | **Task.** The checker says "MAGE has one flat namespace" and no modules. | `web-001` line 1 `u http.{Request, Response, Status}`: E0425 |
| Closure parameter named `f` | 1 | **Task.** `f` is a keyword. | `app-008` `.map(fn(f) => quote_field(f))`: E0001 |
| Open-ended slice `x[a..]` | 4 | **Unclear**, leaning compiler: the spec documents slice ranges only for tensors; the checker reports an unresolved name `_`. | `agent-005` `self.messages[start..].to_vec()`: E0425 `_` |
| `str`/`char`/`&str` mismatch in string methods | 4 | **Unclear**, leaning compiler: `split(':')` and `find(':')` are rejected with "expected str, found char", and `markdown.lines()` with `markdown: &s` with "expected str, found &str". | `app-005` `pattern.split('/')`: E0308; `app-002` `markdown.lines()`: E0308 |
| `s.from_utf8(..)` (type name used as a path) | 2 | **Unclear**, leaning compiler: `s` is the string type's name, and an associated function on it is not resolved. | `sys-005` `s.from_utf8(bytes)`: E0425 `s` |
| Associated types in traits (`type Item;`) | 2 | **Unclear.** The spec text found does not define them; the checker reports unresolved type `_`. | `gt-004` `T Iterator { type Item; ... }`: E0412 |
| Generic effect declaration `effect channel[T]` | 1 | **Unclear.** No spec text defines parameterised effects. | `conc-003` line 1: E0412 `T` |
| Same variant name in two enums | 1 | **Language limitation** (flat namespace), valid in Rust. | `app-004` `Num` in `Expr` and `Value`: E0428 |
| Other E0308 (generic struct literal arity, `*n` on a by-value binding, `str` vs `&str` effect operand) | 3 | **Unclear**, leaning compiler. | `ds-004` `@Pair { first: p.second, second: p.first }`: "type argument count mismatch" |

### B. The reference type-checks but the tests do not pass (31 tasks)

These tasks are check-clean, so the failure is in running them. 28 of the 31
involve **an evaluator that cannot run a program the checker accepts** (26
purely, 2 together with a test-input defect); those are evaluator limitations,
not task defects, and the task may well be correct. 3 are test-side defects only.

| Group | Tasks | Where the defect is | Example |
|---|---:|---|---|
| `.push` unknown to `--eval` | 13 | **Evaluator.** `.push` type-checks, appears in shipped `prototype/examples/agent_rpn.mg` line 28, and `--eval` of that file fails with the same message. | `algo-010` `rotate_left([1,2,3,4,5], 2)`: eval error unknown function `push` |
| Unary `&` / `&!` / `*` unsupported by `--eval` | 9 (8, plus `ds-007` which also hits `get`) | **Evaluator.** `&` is a documented borrow (`MAGE_SPEC.md`); the evaluator has no references. | `algo-003` `binary_search(&[1,3,5,7,9], 5)`: eval error unsupported unary `&` |
| Floats and structs printed without `.0`; `Some` without `?` | 2 | **Evaluator formatting vs corpus notation**; unclear which should move. Strict comparison fails these. | `basic-008` `square(3.0)`: expected `9.0`, got `9`; `ds-001` `Point { x: 0, y: 0 }` |
| Other missing method (`as_bytes`) | 1 | **Evaluator.** | `algo-004` `is_palindrome("racecar")`: unknown function `as_bytes` |
| `u64` constant above `i64::MAX` | 1 | **Evaluator** (integers are `i64`). | `sys-009` `fnv1a(b"")`: bad int `14695981039346656037` |
| Test input uses a keyword as a name | 2 | **Task (test).** `v` is `val`. | `algo-006` `m v = [3,1,4,1,5]; insertion_sort(&!v); v`: "`v` is a keyword" |
| Test input uses Rust struct literal `Name{..}` | 1 | **Task (test).** MAGE spells it `@Name{..}`; the bare form parses as a map and silently gives a wrong answer. | `ds-011` `Range{start:0,end:10}.contains(5)`: got `false`; with `@Range{..}` the same solution returns `true` |
| Test input uses `^expr` (and `&`) | 2 | **Task (test)** and evaluator. `^` is a type-only sigil; there is no expression form. | `ds-010` `... ^Tree.Node(2, ^Tree.Leaf, ^Tree.Leaf) ...`: does not parse |

### C. Test cases blocked behind a type error, or not evaluable

For the 47 check-failing tasks the tests mostly cannot succeed either: of their
cases, 3 pass, 4 fail, 39 error, 16 are unrunnable. Extra test-side defects
visible there: `agent-004`, `agent-007`, `ds-004`, `ds-005` use bare `Name{..}` literals;
`conc-007` uses `rl` (a keyword) as a name; `err-001` and `web-010`
get `Ok(<fn R>)` because `R.Ok` is read as a call on an unknown `R`
(**Task**: group A); `app-005` and `app-006` have inputs that do not parse
(nested quotes in `f"hi {p["name"]}"`, `[TokenRule{..}]`; **unclear** whether
the input or the lexer is at fault); `conc-001/003/004` fail at
`async.spawn`, and `conc-002/005/008` at `AtomicI64.new`/`Mutex.new`, because
the evaluator does not implement them (and the checker already rejects the files).

## Reading the numbers

* "Check-clean 52" is a clean compiler verdict; it is the number the 98/99 parse
  rate is most often mistaken for. Of the 47 failures, 30 are defects in the
  task and 17 are unclear or compiler-side (group A).
* "Accepted 18" is a **lower bound** on correct tasks: 28 of the 31 check-clean
  tasks that fail involve an evaluator limitation. Equally it is not an
  upper bound on wrong ones: a case passes only if the evaluator and the task agree.
* The baseline can only shrink. When a task or the compiler is fixed so a task is
  accepted, delete its line (the check fails until you do); when a task changes
  from `check` to `test`, update the word.
