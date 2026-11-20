# Unknown method names in the corpus

`mage-parse --check` warns (E0714) when a call, `x.name(args)` or `name(args)`, names
nothing the evaluator can run: no user function or method, no binding in scope, no
published builtin (`STD_FNS`, `VOCABULARY`, `SCALAR_MATH`, `LIST_MUTATORS`), no capability
operation (`CAPABILITY_OPS`), no struct field, import, variant or effect operation. The type
checker types such a call as a fresh variable, so it passes `--check` and then stops
`--eval` with `unknown function`. This is the census of those names over every `.mg` file
in the repository (127 files) and the 99 benchmark reference solutions
(`solution.rdx_source` of `benchmarks/tasks/*.json`).

Result: **207 call sites, 43 names, all in 43 benchmark tasks** (of 99; 56 are clean). No
other `.mg` file in the repository has one. Measured 2026-11-19 over the 127 `.mg` files and
the 99 task references: the totals and every per-name count below are unchanged from the
measurement before it, and no name is new or gone.

The rule is by name, not by receiver type, so a name the module defines anywhere counts as
known even if the receiver's own type lacks it (not warned: when unsure, do not warn). Bare
calls the resolver already refuses (E0425) are not warned twice.

## Recommendation

One per name. The rule: `rename` when a published word already does it; `implement` when
the meaning is obvious, spec-consistent and used at 3 or more sites in 2 or more files;
`remove` otherwise (no clear meaning without a type or runtime the evaluator lacks, an
identity, or too rare to earn a word). MAGE_SPEC fixes the library at 31 words, so every
`implement` is a spec change first. The five `rename` names the first census found (`contains_key`, `io.write_line`, `fs.read_string`, `fs.write_string`, `async.sleep`; 11 sites) were applied to the references on 2026-11-18 and no longer appear; nothing else here has been acted on. The counts rose on other names because the merged tree type-checks and evaluates further than the tree the first census ran on (`app-008` now appears).

| Recommendation | Names | Sites |
|---|---|---|
| implement | 16 | 146 |
| remove | 27 | 61 |

## The table

| Name | Sites | Tasks | Where | Recommendation | What it would do, or the published word |
|---|---|---|---|---|---|
| `to_string` | 30 | 15 | agent-002, agent-006, agent-007, app-001, app-002, app-005, app-006, app-008, err-002, err-003, err-005, sys-008, web-001, web-006, web-009 | implement | `to_string(x)`: the display string of any value, as `println` and f-strings already render it (str) |
| `clone` | 25 | 15 | agent-004, agent-006, agent-007, agent-010, app-004, app-006, app-008, app-009, app-010, conc-005, err-004, err-005, gt-003, sys-006, sys-008 | implement | `clone(x)`: x unchanged; every value is already copied on binding, so it is the identity, kept so Rust-shaped programs run |
| `to_vec` | 14 | 12 | agent-001, agent-004, agent-005, app-001, app-003, app-005, app-008, app-010, conc-009, err-002, err-003, err-004 | remove | identity on a list, which is already an owned value; no meaning beyond `clone` |
| `unwrap_or` | 14 | 8 | agent-007, app-001, ds-007, err-005, gt-005, sys-007, web-001, web-002 | implement | `unwrap_or(o, d)`: the payload of a `?A`, else `d` |
| `insert` | 12 | 10 | agent-004, agent-007, app-001, app-004, app-005, app-010, ds-007, sys-006, sys-008, web-006 | implement | in place on the receiver like `push`: `m.insert(k, v)` sets a map key and returns the old value as `?V`; `xs.insert(i, x)` inserts into a list |
| `trim` | 11 | 6 | agent-007, app-002, app-006, app-008, err-002, err-003 | implement | `trim(s)`: s without leading and trailing whitespace |
| `iter` | 9 | 6 | agent-005, agent-008, app-003, app-008, err-004, sys-006 | remove | a list is already iterable (`for x in xs`, `map(xs, f)`); the call is the identity |
| `remove` | 9 | 4 | agent-005, app-001, gt-003, sys-006 | implement | in place on the receiver like `push`: `m.remove(k)` and `xs.remove(i)` return the removed value as `?V` |
| `starts_with` | 9 | 4 | app-002, app-005, app-006, sys-008 | implement | `starts_with(s, p)`: bool, beside `upper`/`lower`/`split` |
| `is_empty` | 7 | 6 | app-002, app-006, app-008, err-002, err-003, gt-003 | implement | `is_empty(x)`: bool for a list, map or string |
| `get` | 6 | 4 | agent-004, app-004, err-005, web-007 | implement | `get(m, k)` / `get(xs, i)`: `?V`, none when absent |
| `llm.complete` | 5 | 3 | agent-002, agent-003, agent-005 | remove | the evaluator is deterministic and has no model; `llm` has no operations, by design |
| `spawn` | 5 | 5 | conc-003, conc-004, conc-005, conc-006, conc-009 | implement | `async.spawn(f)`: spec section "async: spawn, join, select" names it; run `f` to completion at once, return a handle `join` reads |
| `header` | 4 | 2 | web-001, web-009 | remove | method of a web `Response`/request type no program defines; no meaning without the type |
| `load` | 4 | 3 | conc-002, conc-007, conc-008 | implement | atomic read `a.load()`, with `fetch_add`/`fetch_sub`: the spec names the atomic operations; single-threaded, so plain place read/update |
| `map_err` | 4 | 4 | err-003, err-004, sys-005, sys-007 | implement | `map_err(r, f)`: apply `f` to the `Err` payload of a result, pass `Ok` through |
| `fetch_add` | 3 | 3 | conc-002, conc-007, conc-008 | implement | `a.fetch_add(n)`: add in place, return the previous value |
| `splitn` | 3 | 3 | err-002, web-002, web-006 | implement | `splitn(s, sep, n)`: `split` limited to n parts |
| `unwrap` | 3 | 2 | agent-006, agent-008 | implement | `unwrap(o)`: the payload of `?A` or an `Ok`; a precise error on none or `Err` (the spec's own contract example uses it) |
| `and_then` | 2 | 2 | app-008, err-005 | remove | two sites; obvious meaning, but below the bar for a new word |
| `from_utf8` | 2 | 2 | agent-008, sys-005 | remove | no byte type to decode from |
| `new` | 2 | 2 | web-001, web-007 | remove | `Response.new(..)` names a type the program never defines |
| `parse` | 2 | 2 | err-003, err-004 | remove | needs the target type from context, which `--eval` does not infer; no clear meaning |
| `parse_u16` | 2 | 2 | sys-007, web-002 | remove | a one-off integer parser; `parse` has no meaning, so neither does its typed form |
| `position` | 2 | 2 | app-008, sys-006 | remove | two sites; `find` returns the element, not its index |
| `as_bytes` | 1 | 1 | algo-004 | remove | no byte type |
| `as_str` | 1 | 1 | app-004 | remove | identity on a string |
| `body` | 1 | 1 | web-007 | remove | method of a web response type no program defines |
| `cloned` | 1 | 1 | app-001 | remove | identity on a list (see `clone`) |
| `fetch_sub` | 1 | 1 | conc-007 | implement | `a.fetch_sub(n)`: subtract in place, return the previous value (with `fetch_add`) |
| `fs.read_bytes` | 1 | 1 | agent-008 | remove | no byte type; `fs.read` returns the text |
| `get_mut` | 1 | 1 | app-010 | remove | a mutable reference into a collection; no clear meaning without `&!` bindings |
| `is_some` | 1 | 1 | app-001 | remove | one site; `?=` pattern matching covers it |
| `json.parse` | 1 | 1 | app-001 | remove | `json` is a capability namespace with no operations; no JSON value type to parse into |
| `json.to_string` | 1 | 1 | app-001 | remove | as `json.parse` |
| `lock` | 1 | 1 | conc-005 | remove | one site (`self.inner.lock()`); a mutex guard has no meaning in a single-threaded evaluator |
| `net.get` | 1 | 1 | agent-008 | remove | the evaluator has no network |
| `ok_or` | 1 | 1 | gt-005 | remove | one site; `?=` pattern matching covers it |
| `query` | 1 | 1 | web-001 | remove | method of a web request type no program defines |
| `select` | 1 | 1 | conc-006 | remove | one site (`async.select`); needs real concurrency |
| `send` | 1 | 1 | web-007 | remove | method of a web client type no program defines |
| `tools.compile` | 1 | 1 | agent-003 | remove | `tools` is not a namespace |
| `yield` | 1 | 1 | conc-008 | remove | one site (`async.yield`); needs real concurrency |

## Reproduce

```
for f in $(git ls-files '*.mg'); do mage-parse --check "$f" 2>&1 | grep '\[E0714\]'; done
```

and the same over each task's `solution.rdx_source` written to a file. Each task's name is
its `id` (`agent-001` ... `web-010`).
