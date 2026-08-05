# MAGE — measured functionality & performance

Every number below was **measured** (not estimated): test suites run, benchmarks
executed, perf harness timed. Reproduce with the commands shown. Absolute perf
numbers are machine-dependent; the shapes (throughput, scaling) are not.

Date: 2026-06-10. Build: `release` for perf, `cargo test` for functionality.

> **Re-verified 2026-08-04** — all five crates tested: prototype **1,040**, rmi
> **1,380**, ribosome **164**, germline **112**, forge **52** = **2,748 passing,
> 0 failing, 0 warnings**. No figure below has regressed.
>
> *A measurement that was wrong.* `BuildReport::cache_hit_ratio` was
> `1 - work_done/work_total`. A failed action increments neither term, so a build
> that failed **every** action reported a cache hit ratio of **1.0** — and
> `Fitness::reuse`, a selection signal for the RSI loop, paid it the maximum
> score for breaking the build. Found by running the new CLI against a real
> compiler and reading the report; no test caught it because every test used
> graphs that succeeded. Now tracked as `work_cached` directly, with a regression
> test. Any cache-reuse figure recorded here for a *successful* build is
> unaffected — the two formulas agree exactly when nothing fails.
>
> *On the crate count, and a stale figure.* The 2026-08-03 line said "three
> crates, forge 235". Two things changed since. The build engine was extracted
> from `forge` into its own `ribosome` crate (roadmap step 148), so what was one
> column is now two — no code moved in or out of the test suite. And **235 was
> already stale when it was written**: counting `#[test]` per commit shows 235
> immediately before step 144, 253 after it, 271 after steps 145–146, and 303
> after step 147. So the rise from 2,653 to 2,721 is 36 tests that steps 144–146
> added and the summary line did not pick up, plus the 32 added by step 147.
> Verified with `git grep -c '#\[test\]'` at each commit rather than inferred.
>
> *A second stale figure, unresolved.* `GERMLINE.md` claimed 141 tests. Counting
> `#[test]` in germline's own files gives **112** today and **100** at the commit
> where 141 was written, so 141 never matched those files and I could not
> reconstruct what it counted. It is corrected to the measured 112 rather than
> explained away. The extraction itself removed nothing: 112 both before and
> after, checked across four commits.
>
> *Correction to the prototype count.* This document said 1146, and an earlier
> re-verification said 1209. Both over-counted: `lexer`, `parser`, `ast`, `hir`,
> `heal`, and `recover` were re-included by `#[path]` into two auxiliary binaries
> that had no tests of their own, so 171 test functions were compiled three times
> and executed three times. The library split (roadmap step 142) collapsed the
> duplication. **1,038 is the number of distinct tests and always was** — nothing
> was removed, and the same assertions still run.
> The CUDA path is re-verified on hardware: **1,071 passing, 0 failed**
> (`cargo test --features cuda`, dual RTX 3090 Ti, driver 610.88), built against
> the pinned IronAccelerator tag `v2.2.0`. Reproduce with `scripts/test-all.ps1
> -Bench -Cuda` (or `test-all.sh --bench --cuda`).
>
> *This said 1,269, and that figure is retired.* It was written **before** the
> library split (roadmap step 142), which collapsed 171 duplicate test
> executions — so it counted the same over-count corrected elsewhere in this
> document. `1,269 − 171 = 1,098` against 1,071 measured, leaving **27
> unexplained**, and I could not reconstruct where those went; the figure is
> replaced by the measurement rather than reverse-engineered into agreement.
> What is now solid: the CUDA feature adds **33** tests over the 1,038 CPU suite,
> and all of them pass on real hardware.

---

## 1. Functionality

### Test suites (all green)
| Suite | Tests | Cmd |
|---|---|---|
| MAGE prototype | **1040 pass** (+2 ignored perf harnesses) | `cargo test` |
| rmi (`cpu`) | **1380 pass** | `cargo test --no-default-features --features cpu` |
| ribosome (build engine) | **164 pass** | `cargo test --manifest-path ribosome/Cargo.toml` |
| germline (RSI control plane) | **112 pass** | `cargo test --manifest-path germline/Cargo.toml` |
| forge (registry) | **52 pass** | `cargo test --manifest-path forge/Cargo.toml` |
| agentic-eval (AetherShell) | **80 pass** | `cargo test -p agentic-eval` |
| SPINE `spine-agentic` | **285 pass** | `cargo test -p spine-agentic` |
| SPINE `spine-mage` (ABL bridge) | **5 pass** | `cargo test -p spine-mage` |

### ABL tool-mediated construction — full functional matrix
Every item kind builds → describes (no-exec) correctly (`--build=abl` / `--describe=abl`):

| Kind | build | describe | run (`--run=abl`) |
|---|---|---|---|
| net | ✓ | `kind:net` (layers/dims) | forward pass (`--run=abl-bytes`) |
| kb | ✓ | `kind:kb` (facts/rules) | Datalog fixpoint → derived facts |
| agent | ✓ | `kind:agent` (caps/approvals) | capability-policy decisions |
| swarm | ✓ | `kind:swarm` (size/topology/consensus) | consensus over proposals |
| unified | ✓ | per-item kinds | per-item |

Reject-by-construction error coverage: **B0000–B0006** (net), **K0001–K0007** (kb),
**A0001–A0003** (agent), **S0001–S0006** (swarm), **U0001–U0003** (unified) — all
machine-readable `{code, message, fix}`.

### Front-end reliability (reliability-bench, 100-task corpus)
`lex 100/100 (100%) · parse 99/100 (99%) · effective 100/100 (100%)` (the 1 hard
parse recovers via pattern-heal / structural-heal / refine). `cargo run --bin reliability-bench`.

### agentic-eval quality scores (curated four-axis, bias-audited)
- **Language composite 0.865** — #1 among *implemented* languages (Rust 0.80, Go
  0.675, Python 0.525); only the `ideal` design-target (0.90) ranks above.
- **Single-agent SWE benchmark 0.94**, **collaborative multi-agent SWE 0.98**
  (grounded in real runs). `cargo run -p agentic-eval --example swe_{abl_session,multiagent,languages}`.

---

## 2. Performance

### Front-end (lex + parse) — `release`, in-process median
A realistic 50-layer net (1620 B / 509 tokens):
```
39.3 µs/parse  →  41.2 MB/s,  12.95 M tokens/s
```

### ABL build (spec → source → byte-stable IR) — linear, compact
| Net layers | Build latency | Artifact bytes | B/layer |
|--:|--:|--:|--:|
| 2 | 4.3 µs | 78 | 39.0 |
| 8 | 11.8 µs | 234 | 29.2 |
| 32 | 41.5 µs | 858 | 26.8 |
| 128 | 180.0 µs | 3354 | 26.2 |

≈ **1.4 µs/layer**, **~26 B/layer** — linear in size, very compact at rest.

### No-exec decode + describe (the introspection path)
An 858 B (32-layer) artifact: decode_container + decode_symbols + decompile →
**12.6 µs/op**. Loading is pure bounds-checked data — no code executes.

### kb Datalog evaluation — now **indexed semi-naive** (was naive; optimized)
The evaluator was rewritten with term/predicate **interning** (u32, no string
compares in the hot loop), a **`(pred, arg0)` join index**, and **semi-naive**
evaluation (join only against the previous round's delta). Same results, ~linear/
quadratic instead of quadratic/cubic. Measured before → after:

2-hop join over an N-edge chain (now ≈ linear):
| N edges | Derived | Naive | **Indexed semi-naive** | Speedup |
|--:|--:|--:|--:|--:|
| 100 | 99 | 3.0 ms | **0.22 ms** | 13× |
| 500 | 499 | 62 ms | **0.66 ms** | 95× |
| 1000 | 999 | 250 ms | **1.36 ms** | 184× |
| 2000 | 1999 | 1344 ms | **3.10 ms** | **433×** |

Recursive transitive closure / fixpoint (now ≈ output-size, was cubic):
| Chain | Closure facts | Naive | **Indexed semi-naive** | Speedup |
|--:|--:|--:|--:|--:|
| 20 | 210 | 16 ms | **0.31 ms** | 52× |
| 40 | 820 | 173 ms | **0.60 ms** | 288× |
| 80 | 3240 | 3298 ms | **2.31 ms** | **~1430×** |

**Complexity:** join went from ~O(N²) → ~O(N) (the `arg0` index makes a chain
join an O(matches) lookup); the fixpoint went from ~O(N³) → ~O(output) (semi-naive
derives each fact ~once). Correctness unchanged (984 tests green, terminates at
the least fixpoint). This was the one perf gap the prior report flagged — now fixed.

### CLI per-invocation latency (what an agent experiences)
25-run mean, release binary:
```
--build=schema  28.8 ms   --build=abl  28.7 ms   --describe=abl  31.2 ms   --run=abl  30.1 ms
```
This is **dominated by process startup (~28 ms on Windows)** — the actual work is
µs-scale (see above). An agent doing many ops should drive the **long-running RAP
server** (`--rap`) to amortize startup to ~0.

### Token efficiency (token-bench, 100-task corpus vs Rust)
```
source bytes  1.055 (MAGE 5.5% MORE)   dense  0.933 (6.7% fewer)   native lexers  0.997 (~tie)
```
Confirms the measured thesis: **text token efficiency is a floor** (≈ Rust), not a
win. The compaction lives in the binary IR at rest (≈26 B/layer), not in source.

### Determinism (verified)
ABL artifacts are **byte-stable**: same spec → byte-identical `.abl` across builds;
`build→describe` content hashes match; the collaborative multi-agent run is
run-to-run identical. → content-hashable cache keys, meaningful diffs.

---

## 3. Bottom line

- **Front-end is fast** (~41 MB/s, ~13 M tok/s) and **reliable** (100% effective
  on the corpus with recovery).
- **ABL build/decode are µs-scale, linear, compact, deterministic, no-exec** —
  the agent-facing hot path is cheap.
- **The kb Datalog evaluator was the one perf gap — now FIXED:** rewritten as
  indexed semi-naive (interning + `(pred,arg0)` index + delta evaluation), giving
  up to **~1430×** on transitive closure and ~O(N)/~O(N²) instead of ~O(N²)/~O(N³),
  with identical results (984 tests green).
- **Per-invocation latency is startup-bound (~30 ms)**, not compute-bound — use
  the RAP server for high-frequency agent loops.
- **Tokens are at the irreducible text floor**; the leverage is the binary IR +
  reject-by-construction + determinism, exactly as the language is designed.

Reproduce perf: `cargo test --release perf_report -- --ignored --nocapture`
(`prototype/src/perf_measure.rs`).
