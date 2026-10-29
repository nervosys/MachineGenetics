# MAGE — measured functionality & performance

Every number below was **measured** (not estimated): test suites run, benchmarks
executed, perf harness timed. Reproduce with the commands shown. Absolute perf
numbers are machine-dependent; the shapes (throughput, scaling) are not.

Date: 2026-06-10. Build: `release` for perf, `cargo test` for functionality.

> **Re-verified 2026-09-25** — all six crates tested: prototype **1,408**, rmi
> **1,384**, ribosome **168**, germline **175**, forge **63**, arena **153** = **3,351 passing,
> 0 failing, 0 warnings**.
>
> *One figure below had not regressed — it had been superseded.* The ABL
> artifact-scaling table (78 / 234 / 858 / 3354 B) was measured before v3's
> REPEAT fold existed, and `perf_measure.rs` builds *identical* layers, which
> the fold now collapses to a constant 67 B at every depth. Re-running the
> harness produced four identical numbers, and the tempting reading — that the
> table had rotted — was the wrong one: the table is exact for layers that do
> not repeat, and the harness had silently changed which question it asked.
> Both cases are now measured and labelled. Also re-checked today and holding:
> reliability 99/100 and 100/100 effective, the kb Datalog speedups, and CLI
> per-invocation latency.
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
> *Correction to the prototype count.* This document said 1158, and an earlier
> re-verification said 1209. Both over-counted: `lexer`, `parser`, `ast`, `hir`,
> `heal`, and `recover` were re-included by `#[path]` into two auxiliary binaries
> that had no tests of their own, so 171 test functions were compiled three times
> and executed three times. The library split (roadmap step 142) collapsed the
> duplication. **1,038 is the number of distinct tests and always was** — nothing
> was removed, and the same assertions still run.
> The CUDA path is re-verified on hardware: **1,229 passing, 0 failed**
> (`cargo test --features cuda`, dual RTX 3090 Ti, driver 610.88), built against
> the pinned IronAccelerator tag `v2.2.0`. Reproduce with `scripts/test-all.ps1
> -Bench -Cuda` (or `test-all.sh --bench --cuda`).
>
> *This said 1,269, and that figure is retired.* It was written **before** the
> library split (roadmap step 142), which collapsed 171 duplicate test
> executions — so it counted the same over-count corrected elsewhere in this
> document. `1,269 − 171 = 1,098` against 1,229 measured, leaving **27
> unexplained**, and I could not reconstruct where those went; the figure is
> replaced by the measurement rather than reverse-engineered into agreement.
> What is now solid: the CUDA feature adds **33** tests over the 1,038 CPU suite,
> and all of them pass on real hardware.

---

## 1. Functionality

### Test suites (all green)
| Suite | Tests | Cmd |
|---|---|---|
| MAGE prototype | **1408 pass** (+2 ignored perf harnesses) | `cargo test` |
| rmi (`cpu`) | **1384 pass** | `cargo test --no-default-features --features cpu` |
| ribosome (build engine) | **168 pass** | `cargo test --manifest-path ribosome/Cargo.toml` |
| germline (RSI control plane) | **175 pass** | `cargo test --manifest-path germline/Cargo.toml` |
| arena (self-play arena) | **153 pass** | `cargo test --manifest-path arena/Cargo.toml` |
| forge (registry) | **63 pass** | `cargo test --manifest-path forge/Cargo.toml` |
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
Machine-dependent, and the harness says so where it prints them
(`cargo test --release --lib perf_report -- --ignored --nocapture`). A slower
box measured 54.1 µs / 29.9 MB/s for the same input on 2026-08-18; the shape
is what this row is for, not the constant. Byte counts elsewhere in this
document are machine-independent and should match exactly.

### ABL build (spec → source → byte-stable IR)

Two cases, because v3's REPEAT fold makes them different questions. Latencies
are machine-dependent; the byte columns are not.

**Layers that repeat — folded, O(1) in depth:**

| Net layers | Artifact bytes | B/layer |
|--:|--:|--:|
| 2 | 67 | 33.5 |
| 8 | 67 | 8.4 |
| 32 | 67 | 2.1 |
| 128 | 67 | 0.5 |

A 128-layer stack of identical `Linear(16,16)` ships in **67 bytes** and
decompiles back to 128 layers — verified, not inferred. The fold is
run-length over the sequence, so it catches any repeating period, not just
adjacent duplicates: a 128-layer net cycling through 8 distinct kinds is
155 bytes, the same as the 32-layer version of it.

**Layers that do not repeat — linear, ~26 B/layer:**

| Net layers | Artifact bytes | B/layer |
|--:|--:|--:|
| 2 | 78 | 39.0 |
| 8 | 234 | 29.2 |
| 32 | 858 | 26.8 |
| 128 | 3354 | 26.2 |

This second table is what this section used to hold, with no note that it
described the non-folding case. It is still exactly right, and the harness
that produced it had meanwhile started measuring *identical* layers — so
re-running it gave 67 B four times, and the natural conclusion would have
been that the document was wrong rather than that the input had become
foldable. `perf_measure.rs` now measures both.

### No-exec decode + describe (the introspection path)
A 67 B (32-layer, folded) artifact: decode_container + decode_symbols +
decompile → **7.4 µs/op**. Loading is pure bounds-checked data — no code
executes.

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
derives each fact ~once). Correctness unchanged (the suite was 984 tests green when this
was measured, and 1,408 today; the evaluator still terminates at the least
fixpoint). This was the one perf gap the prior report flagged — now fixed.

### CUDA backend — CPU vs GPU (measured 2026-08-05)

This document is the ground truth for performance claims and had **no GPU
numbers at all**, despite the CUDA backend being a headline feature with a
working harness (`cuda-bench`) that had never been run outside a CI job gated
off for want of a runner. These are that harness's output.

Hardware: **RTX 3090 Ti**, driver 610.88, IronAccelerator `v2.2.0`.
Reproduce: `cargo run --release --features cuda --bin cuda-bench`.

**P-storage chain** (`matmul → add → relu → scale`), the case that matters most:

| dim | iters | cpu ms | gpu ms | speedup |
|--:|--:|--:|--:|--:|
| 64 | 500 | 109.8 | 143.2 | 0.77× |
| 128 | 400 | 435.0 | 130.7 | 3.3× |
| 256 | 300 | 2226.8 | 92.6 | 24× |
| 512 | 200 | 11385.8 | 310.6 | 37× |
| 1024 | 100 | 48151.6 | 312.1 | **154×** |

`bounce=0` across the whole sweep — the chain stayed GPU-resident, with no
round-trip to host memory between ops. That counter is the point of the table:
a fast kernel that bounces to the host between every operation gives most of the
win back, and 0 says this one does not. At dim 64 the GPU is *slower* (0.77×),
which is the honest shape — launch overhead dominates until the work is big
enough, and a table that started at 128 would hide the crossover.

**INT8 GEMM** (cuBLASLt IMMA tensor cores) vs F32, GEMM only:

| dim | f32 ms | imma ms | speedup | calibrated e2e |
|--:|--:|--:|--:|--:|
| 512 | 5.84 | 6.34 | 0.92× | 0.64× |
| 1024 | 25.24 | 12.44 | 2.03× | 1.88× |
| 2048 | 70.50 | 19.45 | **3.62×** | 3.14× |

IMMA confirmed by `quant_imma_count=1006`, so the path is real rather than
silently falling back. Calibrated end-to-end is reported alongside GEMM-only
because quantization is not free and quoting only the kernel would flatter it.

**conv2d** — CPU naive vs GPU im2col+GEMM (NCHW, stride 1, pad 1):

| N | Cin | Cout | HW | cpu ms | gpu ms | speedup |
|--:|--:|--:|--:|--:|--:|--:|
| 1 | 3 | 16 | 32 | 59.7 | 1.8 | 33× |
| 1 | 16 | 32 | 32 | 651.7 | 7.0 | 93× |
| 8 | 16 | 32 | 28 | 4514.5 | 168.8 | 27× |
| 1 | 64 | 64 | 56 | 5352.3 | 16.8 | **319×** |

The CPU side is a naive reference implementation, not a tuned one, so these
ratios measure "GPU vs the obvious CPU code" rather than "GPU vs BLAS". Every
row carried its ✓ correctness marker: GPU output matched CPU within tolerance,
which is what makes the speedups meaningful rather than merely fast.

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
  with identical results (984 tests green at the time; 1,408 now).
- **Per-invocation latency is startup-bound (~30 ms)**, not compute-bound — use
  the RAP server for high-frequency agent loops.
- **Tokens are at the irreducible text floor**; the leverage is the binary IR +
  reject-by-construction + determinism, exactly as the language is designed.

Reproduce perf: `cargo test --release perf_report -- --ignored --nocapture`
(`prototype/src/perf_measure.rs`).

## Physics baseline (plan P4)

Question: can the arena's learners learn physics at CPU scale? Each learner is
trained directly on `physics::corpus(system, Train, seed, …)`, in contiguous
chunks, outside the agent loop, and measured at every tenth of a 192 KiB byte
budget. The held-out corpus is `corpus(system, HeldOut, seed, …)`. Bits per byte
is measured on the first 64 KiB of each corpus. Uniform is 8.0.

Command (release build, from the `arena` directory):

```sh
CARGO_TARGET_DIR=<scratchpad>/tgt-g cargo run --release --bin arena -- physics-baseline
```

Learner config. The default is `Config::default().learners`, which is
`ng:3:12:0.02` (3 orders, 2¹² rows per order, lr 0.02; 3.1 M parameters). The
larger n-gram is `ng:4:14:0.02` (4 orders, 2¹⁴ rows; 16.8 M parameters, about
335 MB with its Adam state). Both use one optimiser step per train call, with
train steps of 4 chunks of 1024 bytes each (48 train steps for 192 KiB). Each
trajectory is 1,100 steps, with seed 1. The text row is the same learner trained
on the README (28,672 bytes, whole 1 KiB steps of 4 chunks of 256 bytes), with
`germline/src/gate.rs` as the held-out text.

| system | learner | train bpb | held-out bpb | gap (held-out − train) | train order-0 bpb | held-out order-0 bpb | held-out < 6? | held-out < train order-0? |
|---|---|---|---|---|---|---|---|---|
| pendulum | ng 3×2¹² | 4.912 | 6.257 | +1.345 | 6.027 | 6.368 | no | no |
| spring | ng 3×2¹² | 4.535 | 3.794 | −0.741 | 5.267 | 3.506 | yes | yes |
| double pendulum | ng 3×2¹² | 4.729 | 5.556 | +0.828 | 5.339 | 5.469 | yes | no |
| n-body | ng 3×2¹² | 4.761 | 5.560 | +0.798 | 5.220 | 5.239 | yes | no |
| bounce | ng 3×2¹² | 4.604 | 6.218 | +1.614 | 6.053 | 6.537 | no | no |
| pendulum | ng 4×2¹⁴ | 4.837 | 6.288 | +1.451 | 6.027 | 6.368 | no | no |
| spring | ng 4×2¹⁴ | 4.458 | 3.469 | −0.988 | 5.267 | 3.506 | yes | yes |
| double pendulum | ng 4×2¹⁴ | 4.639 | 5.753 | +1.114 | 5.339 | 5.469 | yes | no |
| n-body | ng 4×2¹⁴ | 4.659 | 5.742 | +1.083 | 5.220 | 5.239 | yes | no |
| bounce | ng 4×2¹⁴ | 4.451 | 6.194 | +1.743 | 6.053 | 6.537 | no | no |
| text (README → gate.rs) | ng 3×2¹² | 5.102 | 5.498 | +0.397 | 5.117 | 4.666 | yes | no |
| text (README → gate.rs) | ng 4×2¹⁴ | 4.930 | 5.400 | +0.470 | 5.117 | 4.666 | yes | no |

Learning curves (held-out bpb at 0%, 10%, … 100% of the budget), for the
larger learner:

```text
pendulum        8.000 7.806 7.582 7.359 7.144 6.950 6.806 6.637 6.492 6.378 6.288
spring          8.000 7.493 6.855 6.254 5.686 5.113 4.694 4.221 3.888 3.647 3.469
double pendulum 8.000 7.793 7.537 7.264 6.996 6.742 6.555 6.329 6.111 5.928 5.753
n-body          8.000 7.785 7.508 7.254 6.997 6.761 6.574 6.342 6.127 5.921 5.742
bounce          8.000 7.792 7.548 7.307 7.087 6.875 6.726 6.543 6.399 6.285 6.194
text            8.000 7.837 7.489 7.171 6.886 6.623 6.456 6.178 5.902 5.639 5.400
```

Every curve is still falling at 100% of the budget, so none of these is a
converged figure. The 3×2¹² learner's curves are in the command's output: its
held-out figure at 100% is the 3.794 (spring) to 6.257 (pendulum) of the table.

The order-0 columns are the byte-frequency entropy of the same evaluation
windows. Order-0 is what a memoryless model achieves. The held-out order-0 is an
oracle, the held-out window's own entropy. The train order-0 is the fair
reference, because a byte-frequency model fit on the train side is what a
learner could reach without seeing held-out data.

**Verdict: no, physics is not shown to be learnable beyond byte statistics at
this scale.** The sub-8 figures in the "held-out < 6?" column are real, but they
are mostly the byte statistics of the encoding. The encoding is little-endian
`i32` with small magnitudes, so the high bytes are nearly constant. Against the
fair reference, the learner beats the train-side order-0 on spring only, and
spring is the case where the held-out order-0 (3.51) is far below the train
order-0 (5.27). A likely reason, not checked here, is that the spring held-out
range is heavily damped (c ∈ [0.5, 0.9]), so held-out trajectories decay toward
zero, which lowers their byte entropy. That is a distribution shift, not learned
dynamics. Against the held-out oracle, the learners are between 0.34 bits below
order-0 (bounce) and 0.50 bits above it (n-body, larger learner). Pendulum and
bounce sit below it, and n-body and double pendulum above it. Text is in the
same regime. Its learner is 0.73 to 0.83 bits above its own held-out order-0,
and no better than physics on the order-0 test. So in this comparison physics is no weaker than text, and neither system
is shown to be learned beyond byte frequencies. The plan's premise fails at this
budget, so per plan P4 the next step is to re-plan. A longer budget might change
this, because the curves had not flattened, but that is a larger run than the
10-minute baseline allows.

## Entropy-chunked learner (plan L1)

Question: does an entropy-chunked learner (`ent:`, `arena::entropy`) code text
and code better than the hashed n-gram it is built on, at the same orders and
buckets? The learner marks a chunk boundary before byte `t` when the n-gram's
next-byte entropy there exceeds a threshold, set so the mean chunk length is
`L = 4` on the training data. It mixes two experts linearly, with weights learned
online: the n-gram, and a count table keyed by (previous chunk, bytes so far in
this chunk) that backs off to the n-gram. `bits_per_byte` is the honest coding
cost, `−log2 P(byte)` under that mixture. Design and choices are in the module
docs of `arena/src/entropy.rs`.

Commands, from the `arena` directory with a release build. Both use the same
learners, and both train for one budget of whole 1 KiB steps (4 chunks of 256
bytes each, the baseline's setting for text) and score the first 64 KiB of the
held-out file:

```sh
cargo run --release --bin arena -- physics-baseline --system none \
    --learners ng:3:12:0.02,ng:4:14:0.02,ent:3:12:4:0.02,ent:4:14:4:0.02
cargo run --release --bin arena -- physics-baseline --system none \
    --learners ng:3:12:0.02,ng:4:14:0.02,ent:3:12:4:0.02,ent:4:14:4:0.02 \
    --text-train ../germline/src/gate.rs --text-heldout src/transformer.rs
```

`--system none` skips the physics rows, which wait for P4b. The first command is
text: `README.md` trained, `germline/src/gate.rs` held out, as in the P4 text
row. The second is code: `germline/src/gate.rs` trained, `arena/src/transformer.rs`
held out. The table's last column is the entropy learner's held-out bits per byte
minus the n-gram's at the same orders and buckets (negative is better).

| corpus | learner | bytes trained | train bpb | held-out bpb | ent − ng, held-out |
|---|---|---|---|---|---|
| text, README → gate.rs | ngram 3×2¹² | 28,672 | 5.102 | 5.498 | |
| text, README → gate.rs | ngram 4×2¹⁴ | 28,672 | 4.930 | 5.400 | |
| text, README → gate.rs | entropy 3×2¹² L4 | 28,672 | 4.092 | 6.294 | +0.796 |
| text, README → gate.rs | entropy 4×2¹⁴ L4 | 28,672 | 3.258 | 5.151 | −0.249 |
| code, gate.rs → transformer.rs | ngram 3×2¹² | 27,648 | 4.682 | 5.400 | |
| code, gate.rs → transformer.rs | ngram 4×2¹⁴ | 27,648 | 4.332 | 5.223 | |
| code, gate.rs → transformer.rs | entropy 3×2¹² L4 | 27,648 | 3.364 | 5.558 | +0.158 |
| code, gate.rs → transformer.rs | entropy 4×2¹⁴ L4 | 27,648 | 2.886 | 4.605 | −0.618 |

Held-out bits per byte at 0%, 10%, … 100% of the budget, for the same runs:

```
text, README → gate.rs
  ngram 3×2^12:     8.000 7.839 7.512 7.202 6.920 6.660 6.495 6.227 5.970 5.723 5.498
  ngram 4×2^14:     8.000 7.837 7.489 7.171 6.886 6.623 6.456 6.178 5.902 5.639 5.400
  entropy 3×2^12 L4: 8.000 7.643 6.518 6.512 6.646 6.334 6.014 5.822 5.792 5.629 6.294
  entropy 4×2^14 L4: 8.000 7.665 6.245 6.075 5.859 5.634 5.621 5.320 5.742 5.048 5.151
code, gate.rs → transformer.rs
  ngram 3×2^12:     8.000 7.808 7.499 7.203 7.003 6.706 6.409 6.212 5.926 5.653 5.400
  ngram 4×2^14:     8.000 7.805 7.477 7.162 6.948 6.629 6.307 6.094 5.784 5.491 5.223
  entropy 3×2^12 L4: 8.000 7.473 6.139 6.266 6.132 5.816 5.464 5.235 5.813 5.426 5.558
  entropy 4×2^14 L4: 8.000 7.597 6.024 5.971 5.750 5.431 5.406 5.113 5.356 4.835 4.605
```

What the table says, and what it does not:

- **Matched on bytes, the entropy learner wins at 4×2¹⁴ on both corpora** (−0.25
  on text, −0.62 on code). It loses at 3×2¹², by 0.80 on text and by 0.16 on
  code. The sign depends on the size, so one configuration does not settle it.
- **The curves are noisy, and the last point is one sample.** The 3×2¹² text
  curve falls from 5.63 at 90% to 6.29 at 100%. The figures here are single
  runs with one seed, so differences under about 0.3 bits are not established.
- **Train bits per byte are in-sample, and the entropy learner's gap is large.**
  Its train figure is 1.7 to 2.2 bits below its held-out figure, against 0.4 to
  0.9 for the n-gram. The chunk table memorises the chunks it has seen, which is
  the cost of a count table on its own training data.
- **Unit-test figures** (debug build, `cargo test`): on the first 512 bytes of
  the README repeated eight times, trained four epochs at 3×2¹⁰, the entropy
  learner scores 0.303 bits per byte against the n-gram's 0.280. The test asserts
  it is within 0.05 of the n-gram. On the README's first 4 KiB, after training,
  the mean chunk length is 4.66 against the target of 4.
- **Out of distribution it can be confidently wrong.** A three-round arena run
  on the self-play pool, scored on `README.md`, with
  `arena --heldout ../README.md --rounds 3 --programs 16 --learners ent:3:12:4:0.02,ng:3:12:0.02 --cost fuel`,
  gave the entropy learner 9.52 bits per byte against the n-gram's 7.96. The
  mixture is proper, and the tests check it sums to one, so this is not a
  normalisation fault. The chunk table had memorised the pool's repeated
  structure, and the mixture weight learned on that data stayed on it. Above 8
  bits per byte is worse than a uniform model. This is open.

Not measured: physics rows, which wait for P4b, and the joule and latency axes.
