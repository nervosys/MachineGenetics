# MechGen — measured functionality & performance

Every number below was **measured** (not estimated): test suites run, benchmarks
executed, perf harness timed. Reproduce with the commands shown. Absolute perf
numbers are machine-dependent; the shapes (throughput, scaling) are not.

Date: 2026-06-10. Build: `release` for perf, `cargo test` for functionality.

---

## 1. Functionality

### Test suites (all green)
| Suite | Tests | Cmd |
|---|---|---|
| MechGen prototype | **1146 pass** (+1 ignored perf harness) | `cargo test` |
| agentic-eval (AetherShell) | **80 pass** | `cargo test -p agentic-eval` |
| SPINE `spine-agentic` | **285 pass** | `cargo test -p spine-agentic` |
| SPINE `spine-mechgen` (ABL bridge) | **5 pass** | `cargo test -p spine-mechgen` |

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

### kb Datalog evaluation — correct, but a NAIVE evaluator (honest limitation)
2-hop join over an N-edge chain:
| N edges | Derived | Eval time |
|--:|--:|--:|
| 100 | 99 | 3.0 ms |
| 500 | 499 | 62 ms |
| 1000 | 999 | 250 ms |
| 2000 | 1999 | 1344 ms |

Recursive transitive closure (fixpoint):
| Chain | Closure facts | Time |
|--:|--:|--:|
| 20 | 210 | 16 ms |
| 40 | 820 | 173 ms |
| 80 | 3240 | 3298 ms |

**Finding:** the join is **~O(N²)** (nested loop, no hash-index on join vars) and
the fixpoint **~O(N³)** (naive, re-derives each round — no semi-naive evaluation).
This is **fine for the tool-mediated use case** (specs are small — tens of
facts/rules), but it would **not** scale to large fact bases. The fix is standard
(hash-join index + semi-naive evaluation) and is the clear next perf item if kb
size grows. Correctness is unaffected (terminates at the least fixpoint).

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
source bytes  1.055 (MechGen 5.5% MORE)   dense  0.933 (6.7% fewer)   native lexers  0.997 (~tie)
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
- **One real perf gap:** the kb Datalog evaluator is naive (O(N²) join / O(N³)
  fixpoint) — correct and fine for small specs, but needs semi-naive + indexing
  before large knowledge bases. Reported, not hidden.
- **Per-invocation latency is startup-bound (~30 ms)**, not compute-bound — use
  the RAP server for high-frequency agent loops.
- **Tokens are at the irreducible text floor**; the leverage is the binary IR +
  reject-by-construction + determinism, exactly as the language is designed.

Reproduce perf: `cargo test --release perf_report -- --ignored --nocapture`
(`prototype/src/perf_measure.rs`).
