# MAGE — measured functionality & performance

Every number below was **measured** (not estimated): test suites run, benchmarks
executed, perf harness timed. Reproduce with the commands shown. Absolute perf
numbers are machine-dependent; the shapes (throughput, scaling) are not.

Date: 2026-06-10. Build: `release` for perf, `cargo test` for functionality.

> **Re-verified 2026-09-25** — all six crates tested: prototype **1,516**, rmi
> **1,384**, ribosome **168**, germline **175**, forge **63**, arena **222** = **3,528 passing,
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
| MAGE prototype | **1516 pass** (+2 ignored perf harnesses) | `cargo test` |
| rmi (`cpu`) | **1384 pass** | `cargo test --no-default-features --features cpu` |
| ribosome (build engine) | **168 pass** | `cargo test --manifest-path ribosome/Cargo.toml` |
| germline (RSI control plane) | **175 pass** | `cargo test --manifest-path germline/Cargo.toml` |
| arena (self-play arena) | **222 pass** | `cargo test --manifest-path arena/Cargo.toml` |
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

Reject-by-construction error coverage: **B0000–B0006** (net), **K0001–K0008** (kb),
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
was measured, and 1,516 today; the evaluator still terminates at the least
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
  with identical results (984 tests green at the time; 1,516 now).
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
online, and they keep moving while the learner scores: the n-gram, and a count table keyed by (previous chunk, bytes so far in
this chunk) that backs off to the n-gram. `bits_per_byte` is the honest coding
cost, `−log2 P(byte)` under that mixture. Design and choices are in the module
docs of `arena/src/entropy.rs`.

**The figures in this section are after the adaptive-weight fix** (see the out-of-distribution bullet). Before the fix the mixture weights were learned in training and then frozen during scoring. Pre-fix figures are kept, labelled, only where a conclusion depended on them.

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
| text, README → gate.rs | entropy 3×2¹² L4 | 28,672 | 3.756 | 5.410 | −0.088 |
| text, README → gate.rs | entropy 4×2¹⁴ L4 | 28,672 | 3.169 | 4.805 | −0.595 |
| code, gate.rs → transformer.rs | ngram 3×2¹² | 27,648 | 4.682 | 5.400 | |
| code, gate.rs → transformer.rs | ngram 4×2¹⁴ | 27,648 | 4.332 | 5.223 | |
| code, gate.rs → transformer.rs | entropy 3×2¹² L4 | 27,648 | 3.062 | 4.898 | −0.502 |
| code, gate.rs → transformer.rs | entropy 4×2¹⁴ L4 | 27,648 | 2.655 | 4.674 | −0.549 |

Held-out bits per byte at 0%, 10%, … 100% of the budget, for the same runs:

```
text, README → gate.rs
  ngram 3×2^12:     8.000 7.839 7.512 7.202 6.920 6.660 6.495 6.227 5.970 5.723 5.498
  ngram 4×2^14:     8.000 7.837 7.489 7.171 6.886 6.623 6.456 6.178 5.902 5.639 5.400
  entropy 3×2^12 L4: 8.000 7.715 6.304 6.113 5.870 5.653 5.585 5.258 5.078 4.947 5.410
  entropy 3×2^12 L4, before the fix: 8.000 7.643 6.518 6.512 6.646 6.334 6.014 5.822 5.792 5.629 6.294
  entropy 4×2^14 L4: 8.000 7.626 6.203 5.965 5.719 5.511 5.431 5.111 4.962 4.848 4.805
  entropy 4×2^14 L4, before the fix: 8.000 7.665 6.245 6.075 5.859 5.634 5.621 5.320 5.742 5.048 5.151
code, gate.rs → transformer.rs
  ngram 3×2^12:     8.000 7.808 7.499 7.203 7.003 6.706 6.409 6.212 5.926 5.653 5.400
  ngram 4×2^14:     8.000 7.805 7.477 7.162 6.948 6.629 6.307 6.094 5.784 5.491 5.223
  entropy 3×2^12 L4: 8.000 7.631 6.143 5.926 5.758 5.449 5.214 5.062 5.261 5.089 4.898
  entropy 3×2^12 L4, before the fix: 8.000 7.473 6.139 6.266 6.132 5.816 5.464 5.235 5.813 5.426 5.558
  entropy 4×2^14 L4: 8.000 7.532 6.028 5.833 5.608 5.264 5.224 5.060 5.117 4.856 4.674
  entropy 4×2^14 L4, before the fix: 8.000 7.597 6.024 5.971 5.750 5.431 5.406 5.113 5.356 4.835 4.605
```

What the table says, and what it does not:

- **Matched on bytes, the entropy learner now wins at both sizes on both corpora.** After the fix: at 4×2¹⁴ it is 0.60 below the n-gram on text and 0.55 below on code; at 3×2¹² it is 0.09 below on text and 0.50 below on code. Before the fix it lost at 3×2¹² by 0.80 on text and 0.16 on code, and the sign depended on the size. That reading came from frozen mixture weights, so it is not kept as a finding about size.
- **The curves are noisy, and the last point is one sample.** The 3×2¹² text curve now falls to 4.95 at 90% and ends at 5.41, a rise of 0.46 at the last checkpoint (before the fix it rose from 5.63 to 6.29). The figures are single runs with one seed, so differences under about 0.3 bits are not established.
- **Train bits per byte are in-sample, and the entropy learner's gap is large.** After the fix its train figure is 1.6 to 2.0 bits below its held-out figure (before the fix, 1.7 to 2.2), against 0.4 to 0.9 for the n-gram. The chunk table still memorises the chunks it has seen, which is the cost of a count table on its own training data.
- **Unit-test figures** (debug build, `cargo test`, recorded before the fix and not re-measured): on the first 512 bytes of the README repeated eight times, trained four epochs at 3×2¹⁰, the entropy learner scores 0.303 bits per byte against the n-gram's 0.280. The test asserts it is within 0.05 of the n-gram. On the README's first 4 KiB, after training, the mean chunk length is 4.66 against the target of 4.
- **Out of distribution, the mixture was confidently wrong, and the fix removed it.** Before the fix, a three-round arena run on the self-play pool, scored on `README.md`, with `arena --heldout ../README.md --rounds 3 --programs 16 --learners ent:3:12:4:0.02,ng:3:12:0.02 --cost fuel`, gave the entropy learner 9.52 bits per byte against the n-gram's 7.96. The mixture is proper, and the tests check it sums to one, so this was not a normalisation fault. Cause: 99.2% of the README's bytes were unseen in their chunk key. For an unseen key the chunk table backs off to the n-gram with a Dirichlet prior (β = 1), so its probability for a byte is the n-gram's divided by N+1, which costs about log₂(N+1) bits. The mixture weight that put 0.99 on the chunk table in training was then frozen during scoring, so the mixture kept paying that cost. The fix updates the weights per byte while scoring, as a Bayes update with a fixed share of 0.02, starting from the trained weights. The same run now gives 8.00 bits per byte against the n-gram's 7.99. The n-gram figure moved from 7.96 because the self-play pool changes with learner credit. A test checks this, and with the weights frozen again the test fails (8.92 against 8.55).

Not measured: physics rows (P4b, below, now chooses an encoding for them), and the joule and latency axes.

## Physics encodings (plan P4b)

Question: is the P4 negative verdict the encoding? P4 wrote each sample as `i32`
little-endian at the system's quantum, so the low byte moves by hundreds of
quanta per step. P4b re-runs the same grid under four encodings
(`arena physics-baseline --encoding`, see `ARENA.md`):

- `i32`: the P2 layout, unchanged. Its output reproduces the P4 table exactly.
- `coarse`: 100 times the default quantum, stored as `i16` little-endian, saturating.
- `planes`: the same `i32` values, byte-plane-major within each trajectory (byte 0
  of every value in time order, then byte 1, and so on, as Blosc's shuffle).
- `diff2`: per channel, the `i32` second difference `q[t] − 2q[t−1] + q[t−2]`
  (wrapping) for `t ≥ 2`, with the first two samples raw.

Command, one run per encoding (release build, from the `arena` directory):

```sh
CARGO_TARGET_DIR=<scratchpad>/tgt-p4b cargo run --release --bin arena -- physics-baseline --encoding i32
CARGO_TARGET_DIR=<scratchpad>/tgt-p4b cargo run --release --bin arena -- physics-baseline --encoding coarse
CARGO_TARGET_DIR=<scratchpad>/tgt-p4b cargo run --release --bin arena -- physics-baseline --encoding planes
CARGO_TARGET_DIR=<scratchpad>/tgt-p4b cargo run --release --bin arena -- physics-baseline --encoding diff2
```

The learners, budget (192 KiB), trajectory length (1,100 steps) and seed (1) are
those of P4. Only the encoding changes. The text row is encoding-independent, and
is repeated in each run. Each run took 3 to 9 minutes.

Gate (from `IMPLEMENTATION_PLAN.md`): an encoding passes if, on at least three of
the five systems, the larger learner's (`ng 4×2¹⁴`) held-out bpb is at least 0.3
bits below that corpus's held-out order-0 entropy, in the same encoding. Gap
below is the larger learner's held-out bpb minus held-out order-0 bpb.

| encoding | system | learner | held-out bpb | held-out order-0 bpb | gap (held-out − order-0) | gap ≤ −0.3? | train order-0 bpb | held-out − train order-0 |
|---|---|---|---|---|---|---|---|---|
| i32 | pendulum | ng 3×2¹² | 6.257 | 6.368 | −0.111 | no | 6.027 | +0.230 |
| i32 | pendulum | ng 4×2¹⁴ | 6.288 | 6.368 | −0.080 | no | 6.027 | +0.261 |
| i32 | spring | ng 3×2¹² | 3.794 | 3.506 | +0.288 | no | 5.267 | −1.473 |
| i32 | spring | ng 4×2¹⁴ | 3.469 | 3.506 | −0.037 | no | 5.267 | −1.798 |
| i32 | double pendulum | ng 3×2¹² | 5.556 | 5.469 | +0.087 | no | 5.339 | +0.217 |
| i32 | double pendulum | ng 4×2¹⁴ | 5.753 | 5.469 | +0.284 | no | 5.339 | +0.414 |
| i32 | n-body | ng 3×2¹² | 5.560 | 5.239 | +0.321 | no | 5.220 | +0.340 |
| i32 | n-body | ng 4×2¹⁴ | 5.742 | 5.239 | +0.503 | no | 5.220 | +0.522 |
| i32 | bounce | ng 3×2¹² | 6.218 | 6.537 | −0.319 | yes | 6.053 | +0.165 |
| i32 | bounce | ng 4×2¹⁴ | 6.194 | 6.537 | −0.343 | yes | 6.053 | +0.141 |
| coarse | pendulum | ng 3×2¹² | 7.430 | 7.219 | +0.211 | no | 6.859 | +0.571 |
| coarse | pendulum | ng 4×2¹⁴ | 7.309 | 7.219 | +0.090 | no | 6.859 | +0.450 |
| coarse | spring | ng 3×2¹² | 3.462 | 2.915 | +0.547 | no | 5.328 | −1.866 |
| coarse | spring | ng 4×2¹⁴ | 2.784 | 2.915 | −0.131 | no | 5.328 | −2.544 |
| coarse | double pendulum | ng 3×2¹² | 5.943 | 5.878 | +0.065 | no | 5.490 | +0.453 |
| coarse | double pendulum | ng 4×2¹⁴ | 5.988 | 5.878 | +0.110 | no | 5.490 | +0.498 |
| coarse | n-body | ng 3×2¹² | 6.627 | 5.343 | +1.284 | no | 5.293 | +1.334 |
| coarse | n-body | ng 4×2¹⁴ | 6.642 | 5.343 | +1.299 | no | 5.293 | +1.349 |
| coarse | bounce | ng 3×2¹² | 7.823 | 7.498 | +0.325 | no | 6.838 | +0.985 |
| coarse | bounce | ng 4×2¹⁴ | 7.690 | 7.498 | +0.192 | no | 6.838 | +0.852 |
| planes | pendulum | ng 3×2¹² | 5.976 | 6.498 | −0.522 | yes | 6.182 | −0.206 |
| planes | pendulum | ng 4×2¹⁴ | 5.822 | 6.498 | −0.676 | yes | 6.182 | −0.360 |
| planes | spring | ng 3×2¹² | 2.720 | 3.505 | −0.785 | yes | 5.437 | −2.717 |
| planes | spring | ng 4×2¹⁴ | 2.386 | 3.505 | −1.119 | yes | 5.437 | −3.051 |
| planes | double pendulum | ng 3×2¹² | 5.321 | 5.739 | −0.418 | yes | 5.591 | −0.270 |
| planes | double pendulum | ng 4×2¹⁴ | 4.959 | 5.739 | −0.780 | yes | 5.591 | −0.632 |
| planes | n-body | ng 3×2¹² | 5.664 | 5.972 | −0.308 | yes | 5.951 | −0.287 |
| planes | n-body | ng 4×2¹⁴ | 5.388 | 5.972 | −0.584 | yes | 5.951 | −0.563 |
| planes | bounce | ng 3×2¹² | 6.214 | 6.655 | −0.441 | yes | 6.204 | +0.010 |
| planes | bounce | ng 4×2¹⁴ | 6.095 | 6.655 | −0.560 | yes | 6.204 | −0.109 |
| diff2 | pendulum | ng 3×2¹² | 2.776 | 3.398 | −0.622 | yes | 3.734 | −0.958 |
| diff2 | pendulum | ng 4×2¹⁴ | 2.513 | 3.398 | −0.885 | yes | 3.734 | −1.221 |
| diff2 | spring | ng 3×2¹² | 2.427 | 2.423 | +0.004 | no | 3.429 | −1.002 |
| diff2 | spring | ng 4×2¹⁴ | 2.066 | 2.423 | −0.357 | yes | 3.429 | −1.363 |
| diff2 | double pendulum | ng 3×2¹² | 1.345 | 1.467 | −0.122 | no | 1.297 | +0.048 |
| diff2 | double pendulum | ng 4×2¹⁴ | 1.124 | 1.467 | −0.343 | yes | 1.297 | −0.173 |
| diff2 | n-body | ng 3×2¹² | 0.986 | 1.171 | −0.185 | no | 1.181 | −0.195 |
| diff2 | n-body | ng 4×2¹⁴ | 0.740 | 1.171 | −0.431 | yes | 1.181 | −0.441 |
| diff2 | bounce | ng 3×2¹² | 0.939 | 1.182 | −0.243 | no | 1.187 | −0.248 |
| diff2 | bounce | ng 4×2¹⁴ | 0.595 | 1.182 | −0.587 | yes | 1.187 | −0.592 |

The text row is unchanged across encodings (larger learner: held-out 5.400,
order-0 4.666, gap +0.734), and is not part of the gate.

Gate counts, larger learner, held-out order-0 (the plan's rule):

| encoding | systems passing (of 5) | verdict |
|---|---|---|
| i32 | 1 (bounce) | fails |
| coarse | 0 | fails |
| planes | 5 | passes |
| diff2 | 5 | passes |

Against the train-side order-0 instead (the fair reference that P4 used for its
headline), the larger learner's count is planes 4 of 5 (all but bounce) and
diff2 4 of 5 (all but double pendulum). Both still pass the 3-of-5 gate.

**Decision: the gate passes for `planes` and for `diff2`.** Both are recorded
here, and neither is made the default. `--heldout-physics` is unchanged, and the
choice of default is left to the next step. The P4b plan says "if one passes,
make it the default", and two pass, so the choice between them needs a decision
the numbers alone do not settle.

Cautions, which the gate does not capture:

- Held-out order-0 is an oracle, computed on the same window the learner is
  scored on, and each encoding has its own order-0 (the planes and diff2 streams
  are different byte sequences from i32). So the comparisons are within an
  encoding. Across encodings, bpb is not comparable, since each measures a
  different stream.
- Spring's held-out range is heavily damped (c in [0.5, 0.9], against [0, 0.3] for
  training). A rough envelope estimate, which ignores the oscillation, puts about a
  third of held-out spring samples at zero after quantization (`exp(−ct/2)` at the
  1e-5 quantum, dt 0.05, 1,100 steps). Those quiet tails are cheap for any learner,
  and they lower the held-out order-0 more than the train order-0. The spring
  passes are therefore partly a distribution shift, the same caveat as P4. The
  estimate is not a measurement; the exact count was not taken.
- `diff2` passes on the residual stream, where most bytes are near zero for smooth
  motion. Its order-0 is low for the same reason, so the gap is measured against a
  stream that is already close to zero. The pass says the learner beats byte
  frequencies of the residuals. It does not say physics was learned beyond that.
  Its train-side pass count is 4 of 5, which is the stronger check.
- `coarse` is worse than `i32` on every system except spring. Its saturation was
  not measured, so the cause is not isolated: a 1e-3 quantum on pendulum is
  coarse for the byte statistics, and the i16 low byte is still noise. The table
  gives no reason to prefer it.
- Every curve is still falling at 100% of the budget, as in P4. No run here is
  converged.

## State-space learner (plan L2)

Question: does a small diagonal state-space learner (`ssm:`, `arena::ssm`) code
text, code and physics better than the hashed n-gram and the entropy-chunked
learner, at what cost? Design, the gradient check and the choices are in the
module docs of `arena/src/ssm.rs`.

The configuration is `ssm:16:0.01`: 16 channels, Adam at lr 0.01, and truncated
backpropagation through the recurrence every 64 bytes (the default). It has
**9,248 parameters**. The n-gram `ng:4:14` has 16.8 M and the entropy learner
`ent:4:14` has 21.0 M (its n-gram plus a 4.2 M-entry chunk table). The SSM was
fixed before any measurement and was not tuned: one probe run on the default
text row, with this configuration, was made to check run time. No other setting
was tried, so these figures are for one untuned configuration and one seed.

Commands, from the `arena` directory with a release build. Each learner is run
in its own invocation, so the wall time below is per learner. The text rows are
the default README → `gate.rs` and the code rows are `gate.rs` → `transformer.rs`.
The physics rows use `--encoding planes`, the encoding that passed the P4b gate,
and cover all five systems in one invocation, with the text row included in it.
The default budget of 192 KiB was used for every learner, since none was too slow
for it, so no smaller budget was needed. The text rows train on the README-size
budget regardless of `--budget`.

```sh
cargo run --release --bin arena -- physics-baseline --system none --learners ng:4:14:0.02
cargo run --release --bin arena -- physics-baseline --system none --learners ent:4:14:4:0.02
cargo run --release --bin arena -- physics-baseline --system none --learners ssm:16:0.01
# code: the same three, with --text-train ../germline/src/gate.rs --text-heldout src/transformer.rs
cargo run --release --bin arena -- physics-baseline --encoding planes --learners ng:4:14:0.02
cargo run --release --bin arena -- physics-baseline --encoding planes --learners ent:4:14:4:0.02
cargo run --release --bin arena -- physics-baseline --encoding planes --learners ssm:16:0.01
```

Held-out bits per byte is measured on the first 64 KiB of the held-out window
(the text rows use the whole held-out file, which is smaller). Held-out order-0
is the byte-frequency entropy of that window, an oracle. The last two columns
are the gate's comparison (the plan's rule: the larger learner, held-out bpb at
least 0.3 below held-out order-0) and the train-side comparison (held-out bpb
below the order-0 of the training window).

| corpus | system | learner | params | held-out bpb | held-out order-0 | held-out − order-0 | ≤ −0.3? | held-out < train order-0? | wall (s) |
|---|---|---|---|---|---|---|---|---|---|
| text, README → gate.rs | — | ng 4×2¹⁴ | 16.8 M | 5.400 | 4.666 | +0.734 | no | no | 5.1 |
| text, README → gate.rs | — | ent 4×2¹⁴ L4 | 21.0 M | 4.805 (was 5.151) | 4.666 | +0.139 (was +0.485) | no | yes (5.117; was no) | 12.3 (pre-fix) |
| text, README → gate.rs | — | ssm d16 | 9,248 | **5.075** | 4.666 | +0.409 | no | yes (5.117) | 4.8 |
| code, gate.rs → transformer.rs | — | ng 4×2¹⁴ | 16.8 M | 5.223 | 4.907 | +0.316 | no | no (4.666) | 7.2 |
| code, gate.rs → transformer.rs | — | ent 4×2¹⁴ L4 | 21.0 M | **4.674** (was 4.605) | 4.907 | −0.233 (was −0.302) | no | no (4.666) | 9.7 (pre-fix) |
| code, gate.rs → transformer.rs | — | ssm d16 | 9,248 | 5.043 | 4.907 | +0.136 | no | no | 2.3 |
| physics, planes | pendulum | ng 4×2¹⁴ | 16.8 M | 5.822 | 6.498 | −0.676 | yes | yes | |
| physics, planes | pendulum | ent 4×2¹⁴ L4 | 21.0 M | 5.828 (was 5.822) | 6.498 | −0.670 (was −0.676) | yes | yes | |
| physics, planes | pendulum | ssm d16 | 9,248 | 5.679 | 6.498 | −0.819 | yes | yes | |
| physics, planes | spring | ng 4×2¹⁴ | 16.8 M | 2.386 | 3.505 | −1.119 | yes | yes | |
| physics, planes | spring | ent 4×2¹⁴ L4 | 21.0 M | 2.400 (was 2.390) | 3.505 | −1.105 (was −1.115) | yes | yes | |
| physics, planes | spring | ssm d16 | 9,248 | 2.500 | 3.505 | −1.005 | yes | yes | |
| physics, planes | double pendulum | ng 4×2¹⁴ | 16.8 M | 4.959 | 5.739 | −0.780 | yes | yes | |
| physics, planes | double pendulum | ent 4×2¹⁴ L4 | 21.0 M | 4.966 (was 4.968) | 5.739 | −0.773 (was −0.771) | yes | yes | |
| physics, planes | double pendulum | ssm d16 | 9,248 | 4.904 | 5.739 | −0.835 | yes | yes | |
| physics, planes | n-body | ng 4×2¹⁴ | 16.8 M | 5.388 | 5.972 | −0.584 | yes | yes | |
| physics, planes | n-body | ent 4×2¹⁴ L4 | 21.0 M | **5.397** (was 7.836) | 5.972 | −0.575 (was +1.864) | yes | yes | |
| physics, planes | n-body | ssm d16 | 9,248 | 5.160 | 5.972 | −0.812 | yes | yes | |
| physics, planes | bounce | ng 4×2¹⁴ | 16.8 M | 6.095 | 6.655 | −0.560 | yes | yes | |
| physics, planes | bounce | ent 4×2¹⁴ L4 | 21.0 M | 6.084 (was 6.098) | 6.655 | −0.571 (was −0.557) | yes | yes | |
| physics, planes | bounce | ssm d16 | 9,248 | 6.153 | 6.655 | −0.502 | yes | yes | |

Wall time per learner. The physics wall time covers all five systems and the text row. ng: text 5.1 s, code 7.2 s, physics 101.8 s. ent: text 12.3 s, code 9.7 s, physics 260.3 s (all pre-fix). ssm: text 4.8 s, code 2.3 s, physics 59.5 s. One run each, sequential, on this machine, release build. The after-fix entropy runs were made 12 jobs at once, so their seconds are not reported. The fix adds a per-byte posterior update to the entropy learner, so its pre-fix seconds may understate its cost now.

Gate counts, the plan's rule, on the five planes systems: **ng 5 of 5** (the P4b result again, which this run reproduces to the third decimal on all five), **ent 5 of 5** (it was 4 of 5 before the fix, failing n-body), **ssm 5 of 5**. Against the train-side order-0, ssm, ng and ent are all below it on all five. Before the fix the entropy learner was above it on n-body (7.836 against 5.951); after the fix it is below (5.397).

Held-out bits per byte at 0%, 10%, … 100% of the budget, for the rows whose
curves change the reading:

```
n-body
  ng:   8.000 7.977 7.730 7.288 7.114 6.639 6.350 6.177 5.734 5.582 5.388
  ent:  8.000 7.989 7.713 7.041 7.038 4.763 6.230 6.082 4.821 5.596 5.397
  ent, before the fix: 8.000 8.653 9.798 7.257 8.615 7.103 6.344 6.172 7.117 7.590 7.836
  ssm:  8.000 7.994 7.940 7.692 7.144 5.362 5.172 5.167 5.202 6.116 5.160
bounce
  ng:   8.000 7.825 7.520 7.311 7.020 6.771 6.572 6.371 6.248 6.155 6.095
  ent:  8.000 7.814 7.472 6.638 6.741 6.647 6.536 6.327 6.228 6.163 6.084
  ent, before the fix: 8.000 9.430 7.495 7.269 8.441 6.769 6.585 6.368 6.245 6.162 6.098
  ssm:  8.000 7.963 7.801 7.327 6.361 6.422 6.443 6.369 6.329 6.249 6.153
text (README → gate.rs)
  ng:   8.000 7.837 7.489 7.171 6.886 6.623 6.456 6.178 5.902 5.639 5.400
  ent:  8.000 7.626 6.203 5.965 5.719 5.511 5.431 5.111 4.962 4.848 4.805
  ent, before the fix: 8.000 7.665 6.245 6.075 5.859 5.634 5.621 5.320 5.742 5.048 5.151
  ssm:  8.000 7.950 7.816 7.460 6.589 5.522 5.524 5.749 5.487 5.113 5.075
```

What the table says, and what it does not:

- **Text: the entropy learner is lowest, and the SSM beats the n-gram by 0.33 on one seed.** After the fix the entropy learner reaches 4.805 (was 5.151). The SSM is at 5.075, 0.27 above it, and the n-gram at 5.400, 0.60 above it. The SSM's 0.33 margin over the n-gram is above 0.3, but it is one seed, so it is suggestive rather than established. Its training figure is 5.22, above its held-out figure, so it underfits the README at 28 steps. Its curve is still falling at the end. Before the fix the SSM was lowest on text; that no longer holds.
- **Code: the entropy learner is lowest.** Its 4.674 is 0.37 below the SSM and 0.55 below the n-gram (before the fix: 4.605, 0.44 and 0.62). The SSM beats the n-gram by 0.18, which is under 0.3 and not established. Here the entropy learner's train figure (2.655) is 2.0 bits under its held-out figure, the memorisation gap from L1, and the SSM's gap is small.
- **Physics: on the gate, all three learners pass all five.** The entropy learner's n-body result, the failure before the fix, is now 5.397 at the end, against 5.388 for the n-gram. Its n-body curve is still non-monotone: it is 7.99 and 7.71 at 10% and 20% of the budget, falls to 4.76 at 50%, rises to 6.23 at 60%, and ends at 5.40. Before the fix it was above uniform at 10% and 20% (8.65 and 9.80) and ended at 7.84. That was the frozen mixture weight (L1), and the fix removed it. Bounce's 10% point is 7.81 now (was 9.43 before the fix), and it recovers as before.
- **Physics: the SSM is level with the n-gram, and the differences are small.**
  Against the n-gram it is lower on pendulum (0.14), double pendulum (0.06) and
  n-body (0.23), and higher on spring (0.11) and bounce (0.06). None of these is
  over 0.3, so none is established. The n-gram and the entropy learner agree on
  pendulum to within 0.01 (5.822 and 5.828 after the fix; 5.822 both before), so the chunk table adds nothing there.
- **Size.** The SSM has about 0.05% of the n-gram's parameters, and its state is
  16 numbers per position. That it matches or beats the n-gram on text and on
  four of five physics systems says that the recurrence is a useful bias for
  these streams. It does not say the SSM is the better learner at equal size, and
  no size-matched run was made.
- **Cost.** The SSM was the cheapest of the three on every corpus here: 59.5 s for the physics invocation, against 101.8 s for the n-gram and 260.3 s for the entropy learner, all pre-fix sequential runs. The cause of the n-gram's and the entropy learner's extra time was not isolated. The fix adds a per-byte posterior to the entropy learner, so its cost may have changed, and it was not re-timed cleanly.
- **Optimisation is not smooth.** The SSM's n-body curve goes from 5.17 at 60%
  of the budget up to 6.12 at 90%, then back to 5.16. The learning rate was not
  tuned; a smaller rate or a schedule may remove the spike, and this was not tested.
- **Cross-check.** The n-gram's planes rows reproduce the P4b table exactly
  (pendulum 5.822, spring 2.386, double pendulum 4.959, n-body 5.388, bounce
  6.095), so the harness and the seed are the same as in P4b.
- **What is not measured.** Seeds beyond one, learning-rate and width sweeps,
  size-matched comparisons, and the joule and latency axes. Every difference
  under about 0.3 bits in this table is one seed's difference, and should be read
  as a tie.

## Learner comparison (plan L3)

Question: on each corpus, how much more or less data (bytes, and wall-clock seconds) does each learner need to reach the same held-out bits per byte as the n-gram `ng:4:14:0.02`? Learners: `ng:4:14:0.02`, `ent:4:14:4:0.02` and `ssm:16:0.01`, the same configurations as the L1 and L2 sections, untuned, one configuration each. Corpora: text (README → `gate.rs`), code (`gate.rs` → `transformer.rs`), and the five physics systems under `--encoding planes`. Seeds 1 to 6 for physics. The text and code rows are one run each, because `--seed` does not reach them (see below).

Commands, from the `arena` directory, release build:

```sh
# physics: one invocation per learner and seed, all five systems plus the README text row
arena physics-baseline --encoding planes --seed S --learners ng:4:14:0.02 --json curves/seedS-ng.json
arena physics-baseline --encoding planes --seed S --learners ent:4:14:4:0.02 --json curves/seedS-ent.json
arena physics-baseline --encoding planes --seed S --learners ssm:16:0.01 --json curves/seedS-ssm.json
# code: text only, one invocation per learner
arena physics-baseline --system none --learners LEARNER --text-train ../germline/src/gate.rs --text-heldout src/transformer.rs --json code-TAG.json
# measurement
arena measure-curves curves/ code-ng.json code-ent.json code-ssm.json --json measurement.json
```

The pre-fix grid ran sequentially in one background process, so its wall-clock figures are not contaminated by parallel runs. It took 1,774 s in total (29.6 min): ng 84–121 s per seed, ent 130–172 s, ssm 38–43 s; the code runs took 2–3 s each. The default budget of 192 KiB per learner was used, so no budget reduction was needed. **After the fix the grid was re-run with 12 jobs at once** to save time. Its bytes multipliers are deterministic and are reported below; its wall-clock seconds are contaminated by the parallel run and are not reported. The seconds columns are the pre-fix sequential figures.

**What `--seed` varies.** It seeds the physics trajectories only: `physics_corpus` draws the train and held-out trajectories from `Rng(seed)`. The text and code curves do not use it, the n-gram and entropy learners are deterministic, and the SSM's initialisation uses a fixed internal seed. The text and code rows are therefore **single-seed**, and no spread is reported for them. The duplicate text rows that the six physics sweeps carry were checked to agree bit for bit on every bits-per-byte figure before one was kept.

Target: the worst final held-out bits per byte among the three learners on that corpus, so every frontier is asked for a loss its own curve reaches. Per corpus, after the fix: pendulum 5.871, spring 2.682, double pendulum 4.9825 (was 5.016), n-body 5.591 (was 7.984), bounce 6.153, text 5.400, code 5.223.

The multiplier is cost(learner) / cost(ng) to reach the target. Above 1 means the learner needs more. Pooled is over the six seeds' frontiers together; the per-seed columns are each seed's own frontiers, as mean ± sample sd over the seeds that reach the target.

| corpus | learner | bytes: pooled | bytes: per seed | seconds: pooled | seconds: per seed (pre-fix) |
|---|---|---|---|---|---|
| pendulum | ent | 0.982 (was 1.005) | 0.982 ± 0.016 (6) (was 0.999 ± 0.004) | 1.033 | 1.360 ± 0.288 (6) |
| pendulum | ssm | 0.442 | 0.508 ± 0.139 (6) | 0.031 | 0.039 ± 0.010 (6) |
| spring | ent | 0.969 (was 1.002) | 1.001 ± 0.006 (6) (was 1.008 ± 0.007) | 1.224 | 1.312 ± 0.278 (6) |
| spring | ssm | 0.929 | 1.024 ± 0.101 (6) | 0.121 | 0.108 ± 0.021 (6) |
| double pendulum | ent | n/a (was 1.010) | 0.994 ± 0.009 (5; seed 4 misses the target) (was 1.016 ± 0.013 (6)) | 1.149 | 1.225 ± 0.294 (6) |
| double pendulum | ssm | 0.421 (was 0.429) | 0.455 ± 0.065 (6) (was 0.439 ± 0.030) | 0.059 | 0.055 ± 0.014 (6) |
| n-body | ent | 0.252 (was n/a) | 0.594 ± 0.285 (6) (was n/a) | not re-timed | not re-timed |
| n-body | ssm | 0.527 (was n/a) | 0.504 ± 0.025 (6) (was n/a) | not re-timed | not re-timed |
| bounce | ent | 1.002 (was 1.008) | 1.007 ± 0.005 (6) (was 1.010 ± 0.002) | 1.095 | 1.096 ± 0.241 (6) |
| bounce | ssm | 0.454 | 0.886 ± 0.251 (6) | 0.048 | 0.074 ± 0.020 (6) |
| text (README → gate.rs) | ent | 0.581 (was 0.648) | single run | 1.125 | single run |
| text (README → gate.rs) | ssm | 0.809 | single run | 0.085 | single run |
| code (gate.rs → transformer.rs) | ent | 0.593 (was 0.638) | single run | 0.778 | single run |
| code (gate.rs → transformer.rs) | ssm | 0.885 | single run | 0.113 | single run |

Notes on the table:

- **n-body was `n/a` before the fix, because of the default target, and that was a grid artefact.** The default target was 7.984, the entropy learner's final loss on seed 4. It lay above the n-gram's first checkpoint on every seed, so the n-gram's frontier could not say when it crossed. After the fix the entropy learner's final loss is 5.397, the default target is 5.591, and every frontier reaches it. So n-body is defined at the default: entropy 0.252 pooled (0.594 ± 0.285 per seed; five of six seeds below 1), and SSM 0.527 (0.504 ± 0.025). The entropy learner's pooled figure rests on single-checkpoint dips: the seed-1 curve (L2) falls to 4.76 at 50% of the budget and is back at 6.23 at 60%. Read the per-seed mean and its spread, not the pooled figure alone. At `--target 7.5`, which every frontier reaches, the entropy learner is now 0.432 pooled (0.751 ± 0.250 per seed; before the fix 1.021, 1.016 ± 0.070), and the SSM is unchanged at 1.289 (1.307 ± 0.047). The SSM is consistently more expensive than the n-gram at 7.5, because its curve stays near 8 bits until about 40% of the budget, so its default-target advantage on the other systems is not uniform across physics. The pre-fix raw output is in `grid/measure-nbody-t75.txt`, alongside the full `measure.txt`; the `grid/` files were not regenerated, and the after-fix outputs are not committed.
- **Seconds** are wall-clock, cumulative over train steps, evaluation excluded. They include the learners' per-byte cost, so the SSM's seconds figure reflects its size (9,248 parameters) as much as its learning.
- **Bytes** is the same byte grid for all three learners (every checkpoint at 10% steps of the budget), so the bytes multiplier only says at which checkpoint each curve crosses the target. It is coarse, and it is not a compute measure.

### M1 loss-to-loss fits (train bpb as proxy, held-out bpb as target)

Fitted by `fit_loss_to_loss` over all trained checkpoints pooled over seeds (n = 60 per physics learner, n = 10 per text or code learner).

| corpus | ng: c, rmse | ent: c, rmse | ssm: c, rmse |
|---|---|---|---|
| pendulum | 1.25, 0.039 | 1.30, 0.072 (was 0.25, 0.378) | 0.40, 0.061 |
| spring | 1.20, 0.047 | 1.10, 0.226 (was 0.45, 0.256) | 0.65, 0.101 |
| double pendulum | 1.05, 0.023 | 0.85, 0.278 (was 0.25, 0.462) | 1.05, 0.054 |
| n-body | 1.05, 0.028 | 0.60, 0.211 (was 0.95, 0.572) | 1.30, 0.073 |
| bounce | 1.60, 0.020 | 2.50, 0.129 (was 0.75, 0.580) | 0.45, 0.065 |
| text | 1.00, 0.018 | 2.85, 0.134 (was 3.00, 0.156) | 1.15, 0.056 |
| code | 1.05, 0.005 | 1.00, 0.080 (was 0.40, 0.176) | 0.65, 0.084 |

The fits are good for the n-gram and reasonable for the SSM. For the entropy learner they are now reasonable on every system: rmse 0.07 to 0.28 on physics (before the fix, 0.26 to 0.58), 0.13 on text (was 0.16) and 0.08 on code (was 0.18). The fitted exponent still varies from 0.60 (n-body) to 2.50 (bounce), so the loss-to-loss map is not stable across systems. Its held-out curve is also still not a smooth function of its train curve: on n-body it falls to 4.76 at 50% of the budget and rises to 6.23 at 60% (seed 1), and before the fix it spiked above uniform at early checkpoints. The M1 fit should not be used to extrapolate the entropy learner.

### Plain conclusions

- **The entropy learner is no cheaper than the n-gram on four physics systems, and cheaper on n-body in five of six seeds.** Its bytes multiplier after the fix is 0.98 on the pendulum, 1.00 on the spring, 0.99 on the double pendulum (five seeds) and 1.01 on the bounce, with per-seed sd at most 0.016 (before the fix, 1.00 to 1.01). On n-body it is 0.25 pooled and 0.59 ± 0.29 per seed (before the fix: undefined at the default target, and 1.02 at 7.5, now 0.43 pooled and 0.75 ± 0.25 per seed). The per-seed mean is 0.41 below 1, less than two seed sd (0.57), so the n-body advantage is not established, though five of six seeds are below 1.
- **The entropy learner's text and code advantage now carries to n-body, but not to the other four physics systems.** It needs 0.58 and 0.59 of the n-gram's bytes on text and code (single runs; before the fix 0.65 and 0.64), and 0.98 to 1.01 on the pendulum, spring, double pendulum and bounce. On n-body the sign matches text and code. The earlier reading that the physics result was the reverse of text and code held only for the pre-fix learner.
- **The SSM needs fewer bytes than the n-gram on text (0.81) and on code (0.89).** On the physics systems its pooled figure is below 1 on all five (0.42 to 0.93), and its per-seed mean is below 1 on four: 0.51 ± 0.14 (pendulum), 0.46 ± 0.07 (double pendulum; was 0.44 ± 0.03), 0.50 ± 0.03 (n-body; undefined before the fix), and 0.89 ± 0.25 (bounce). Spring is 1.02 ± 0.10. At `--target 7.5` the n-body figure is 1.29 ± 0.05, worse than the n-gram, so its n-body advantage depends on the target. Its advantage is therefore not specific to physics, and it is not uniform across the physics systems. It is the largest in magnitude on the double pendulum (pooled 0.42), and most consistent across seeds on n-body.
- **Seconds favour the SSM strongly on every corpus,** because it is roughly 8 to 30 times cheaper per byte at this size. That is a statement about the model's cost, not its learning. Under this measure it is not a claim that the SSM learns physics better.
- **Nothing is established at the level of two seed standard deviations.** Each physics difference between the SSM and the n-gram is judged against the physics seed spread. The text and code figures are single runs with no spread, so there is no sd against which to set them. The SSM's text figure (0.81) lies outside the physics per-seed ranges for the pendulum (0.42 to 0.78), the double pendulum (0.39 to 0.58) and the n-body (0.47 to 0.55), and inside the bounce's (0.45 to 1.11). A single text run is not a sample from the same distribution, so these comparisons are suggestive, not a test.

### Critical-path question

*Does any learner show a consistent advantage, or a measurable signal, on physics that it does not on text?*

No, not in the sense of an advantage present on physics and absent on text.

- The entropy learner has an advantage on text and code (0.58 and 0.59), and on n-body in five of six seeds (0.59 per seed). It has none on the other four physics systems. Its n-body sign matches its text and code sign, so the direction is not reversed for that system.
- The SSM has an advantage on text (0.81 bytes, single run) and on physics (0.42 to 0.93 pooled, and 0.46 to 0.89 per-seed mean on four of the five systems, spring excepted). Its physics advantage is larger in magnitude on the pendulum, the double pendulum and the bounce, and it is most consistent across seeds on n-body (0.50 ± 0.03) and the double pendulum (0.46 ± 0.07). That is a signal that the SSM is cheaper to fit physics data than the n-gram. It is not a signal that the physics data is special for the SSM: the same ordering holds on text. Whether the physics advantage is larger than the text advantage cannot be settled, because the text figure has no seed spread.
- On the seconds axis the SSM is cheaper on every corpus where it is defined, for the cost reason above; the entropy learner is no cheaper on any physics system measured in the sequential run (its n-body seconds were not re-timed).

So on this corpus set, the signal the critical path asks for is still not there. The entropy learner's n-body advantage (five of six seeds below 1) matches its text and code advantage in sign, so it does not meet the question's test either. The SSM's physics advantage is real on the pendulum, the double pendulum and, less consistently, the bounce, but it is not confined to physics, and it reverses on n-body at a target both frontiers reach. The self-improvement loop would get the same answer from the SSM on text, with the same sign. The result is for untuned learners, one configuration each, and a 10% checkpoint grid.

### Limits of this measurement

- One configuration per learner, untuned, and no size-matched comparison. The SSM has about 0.05% of the n-gram's parameters, so the bytes advantage is not an equal-size comparison (see the L2 section).
- The frontier is the lower envelope of the checkpoints, so a curve that dips below the target at one checkpoint earns that dip (the M2 definition). The entropy learner's n-body curve has such dips: the seed-1 curve falls to 4.76 at 50% of the budget (against 5.40 at the end), and its n-body multiplier rests on them. Read the per-seed spread (0.25 to 1.01) rather than the pooled 0.25 alone.
- Checkpoints are at 10% steps of the budget. Bytes multipliers are therefore resolved only to about one tenth of the budget.
- The default target is the worst learner's final held-out loss, so that learner's own frontier ends exactly at the target. Before the fix the default target for n-body lay above the reference's first checkpoint, which made it `n/a`. After the fix every frontier reaches the default (5.591). A `--target 7.5` re-run is reported above for n-body only. The other corpora were not re-run at other targets, so their figures hold at the default target only.
- Pre-fix wall-clock seconds were recorded on a shared machine, one learner at a time. The after-fix grid ran 12 jobs at once, so its seconds are not reported. They are noisy: the per-seed sd of the entropy learner's seconds is about 20–25% of its mean.
- Joules were not measured on this axis; only bytes and seconds.

## Rollout metrics (plan P6)

Question: can a learner trained on physics *predict* a trajectory, not just code it? The learner is given the header and the first 10 timesteps of a held-out trajectory, generates 100 more timesteps one byte at a time, and the bytes are decoded and compared with the true states. Bits per byte says how well a learner codes trajectories; this measures whether its predictions track them. The code is `arena/src/rollout.rs`, and the generator is `Predictor`, a per-learner next-byte accessor in `arena/src/learner.rs`.

Design, and what each figure means:

- **The encoding is `i32`, and only `i32`.** Planes lay out the byte-planes across the whole trajectory, so a planes prefix holds no whole timestep and cannot be generated a step at a time. (`coarse` and `diff2` could be rolled out, but the plan specifies `i32`, and they are not measured here.) Training is on the `i32` training corpus, as `physics-baseline` does; the held-out trajectories are `i32` too.
- **Generation is greedy.** Each byte is the most probable byte under the learner's distribution, so the rollout is deterministic. Predictions are decoded with `physics::decode_prefix`.
- **State error** at horizons 1, 10 and 100 is the RMS over 8 held-out trajectories of the RMS over channels, in the system's units. Angles are compared on the circle. Channels mix units (angles, rates, positions, velocities), so the figure is a scale, not a single quantity.
- **Energy drift** is the largest `|E_n − E_0| / |E_0|` over the generated steps, with `E` from `physics::energy` (the same function the conservation tests use) and `E_0` the true initial energy, averaged over trajectories. It is reported only for the pendulum, the double pendulum and the n-body system, which conserve energy. The spring (damped) and the bounce (restitution below 1) lose energy by design, so they have no drift figure.
- **Two trivial baselines** on the same contexts. *Hold* repeats the last given state. *Constant velocity* extrapolates the last two given states. A learner **beats the trivial baseline** at a horizon when its error is below the smaller of the two.
- Holding a state conserves energy exactly, so *hold* has near-zero drift on a conservative system. The drift column is read with the error columns, not on its own.

Command, from the `arena` directory with a release build (about three to four minutes of run time on this machine, one run, before the fix; the after-fix greedy and sampled runs took about 900 to 1,265 s each, 12 jobs at once):

```sh
CARGO_TARGET_DIR=<scratchpad>/tgt-p6 cargo run --release --bin arena -- physics-rollout
```

The defaults are the learners of the L1 and L2 sections (`ng:4:14:0.02`, `ent:4:14:4:0.02`, `ssm:16:0.01`), a training budget of 192 KiB on trajectories of 1,100 steps, seed 1, and 8 held-out trajectories per system of 110 steps (10 given, 100 generated). The table is after the adaptive-weight fix: only the entropy rows changed, on the double pendulum and the n-body system.

| system | learner | err @1 | err @10 | err @100 | energy drift | beats the better trivial baseline at |
|---|---|---|---|---|---|---|
| pendulum | ng 4×2¹⁴ | 1.7528 | 1.7708 | 3.0992 | 1.00e0 | none |
| spring | ng 4×2¹⁴ | 3.2946 | 1.8535 | 0.5227 | n/a | 10, 100 |
| double pendulum | ng 4×2¹⁴ | 1.3882 | 1.3889 | 1.4225 | 5.02e0 | none |
| n-body | ng 4×2¹⁴ | 1.4010 | 1.4010 | 1.4012 | 2.67e2 | none |
| bounce | ng 4×2¹⁴ | 2.9430 | 2.9442 | 2.9984 | n/a | none |
| pendulum | ent 4×2¹⁴ L4 | 1.7528 | 1.7708 | 3.0992 | 1.00e0 | none |
| spring | ent 4×2¹⁴ L4 | 3.2946 | 1.8535 | 0.5227 | n/a | 10, 100 |
| double pendulum | ent 4×2¹⁴ L4 | 1.8028 (was 1.3882) | 1.3889 | 1.4225 | 1.24e5 (was 5.02e0) | none |
| n-body | ent 4×2¹⁴ L4 | 7770.1242 (was 7756.8774) | 1026.6792 (was 8532.9468) | 7334.8471 (was 2212.8203) | 4.18e9 (was 6.61e9) | none |
| bounce | ent 4×2¹⁴ L4 | 2.9430 | 2.9442 | 2.9984 | n/a | none |
| pendulum | ssm d16 | 1.7528 | 1.7708 | 3.0992 | 1.00e0 | none |
| spring | ssm d16 | 3.2946 | 1.8535 | 0.5227 | n/a | 10, 100 |
| double pendulum | ssm d16 | 1.3881 | 1.3889 | 1.4225 | 5.02e0 | none |
| n-body | ssm d16 | 1.4010 | 1.4010 | 1.4012 | 2.67e2 | none |
| bounce | ssm d16 | 2.9430 | 2.9442 | 2.9984 | n/a | none |

The trivial baselines, which do not depend on the learner:

| system | baseline | err @1 | err @10 | err @100 | energy drift |
|---|---|---|---|---|---|
| pendulum | hold | 0.0257 | 0.2641 | 3.3710 | 1.71e-6 |
| pendulum | constant velocity | 0.0003 | 0.0159 | 1.6694 | 6.41e-1 |
| spring | hold | 0.1804 | 2.0872 | 2.8130 | n/a |
| spring | constant velocity | 0.0613 | 2.9078 | 22.3656 | n/a |
| double pendulum | hold | 0.0035 | 0.0353 | 0.3514 | 5.92e-5 |
| double pendulum | constant velocity | 0.0001 | 0.0006 | 0.0227 | 9.36e-2 |
| n-body | hold | 0.0004 | 0.0036 | 0.0357 | 1.29e-4 |
| n-body | constant velocity | 0.0001 | 0.0004 | 0.0040 | 9.52e-3 |
| bounce | hold | 0.0069 | 0.0694 | 0.6949 | n/a |
| bounce | constant velocity | 0.0000 | 0.0004 | 0.0351 | n/a |

### What the greedy rollouts actually emit

Before the fix, the three learners' rows were the same on the pendulum, the spring and the bounce, and the double pendulum differed only in the fourth decimal. After the fix the entropy row on the double pendulum differs at horizon 1 (1.8028 against 1.3881) and has energy drift 1.24e5. That row was not inspected, so it is not read as the zero state. A check on the n-gram (one held-out trajectory per system, the same training as above, not committed) found that greedy decoding emits **one constant byte in every position** on all five systems: `0xFF` on the pendulum, the double pendulum and the bounce, which is the `i32` value −1, one quantum and so effectively zero; and `0x00` on the spring and the n-body system, which is zero. The n-gram's predicted state is therefore the zero state. Its error matches the zero state's to about four significant figures (pendulum at horizon 1: 3.3755 for the learner, 3.3755 for zero; spring and n-body exactly). The SSM and entropy rows match the n-gram's to four decimals on those systems, so they are presumed to emit the same constant, but their bytes were not inspected. Before the fix, the entropy row on the n-body system was not the same: its predicted values reached about 2 × 10⁵ in magnitude, which is why its error was in the thousands. After the fix its error is still in the thousands (7770, 1027, 7335), and its predicted values were not re-inspected.

The mode of a byte distribution, read greedily at every position, is therefore the zero state. For the n-gram at this budget the rollout is a constant predictor, not a model of the dynamics, and before the fix the other rows matched it on those systems.

### Sampled rollouts (`--decode sample:0.5 --samples 8`)

Run once, on the same seed and contexts, for the entropy learner before and after the fix. The n-gram row did not change between the runs and is shown for reference. Each cell is state error at horizons 1, 10 and 100, and the drift column is the after-fix value. One run, eight samples per context.

| system | ent before the fix @1 / @10 / @100 | ent after the fix @1 / @10 / @100 | ent drift (before / after) | ng @1 / @10 / @100 |
|---|---|---|---|---|
| pendulum | 1914.1896 / 2346.3253 / 2850.3016 | 2585.7961 / 2881.6145 / 2553.2120 | 1.75e7 / 2.18e7 | 2245.9078 / 1287.0536 / 1889.6567 |
| spring | 165.0552 / 545.3825 / 869.9983 | 415.6688 / 385.5141 / 0.5227 (beats hold at 100) | n/a | 258.6407 / 1162.6779 / 1112.1973 |
| double pendulum | 10149.7208 / 9728.2253 / 15185.6360 | 18614.4971 / 38.1583 / 112.4447 | 6.17e9 / 2.02e9 | 13493.1053 / 7484.4881 / 15708.1455 |
| n-body | 44848.8985 / 40579.0713 / 43059.0608 | 43354.0521 / 41445.0898 / 42161.1482 | 8.80e10 / 9.59e10 | 40214.0540 / 41805.3651 / 33110.0606 |
| bounce | 3232.5732 / 921.3660 / 1661.6357 | 2732.2335 / 5363.7079 / 5530.1295 | n/a | 4240.3555 / 1214.5225 / 2564.6738 |

What it says:

- For four of five systems the sampled errors are in the thousands for every learner, the same scale as the n-gram. Sampling does not make the learners track the trajectories.
- The double pendulum's error falls from 18614 at horizon 1 to 38 at horizon 10 and is 112 at horizon 100. A tracking rollout's error grows with horizon, and this one is largest at horizon 1, so it is not a tracking signal. It was not inspected further.
- The spring's 0.5227 at horizon 100 is the greedy zero-state error (the same figure as the n-gram's greedy row), so the sampled entropy learner's win over hold at horizon 100 is the zero-state artefact again, not learned dynamics. The sampled-rollouts commit message says the spring wins disappear under sampling. That held for the entropy learner before the fix, and it does not hold after it.
- The n-body divergence does not go away under sampling. The entropy learner's error stays near 4 × 10⁴ at all three horizons, as the n-gram's does, so the divergence is not a greedy-decoding artefact.
- The sampled runs took 904 s (before) and 1,230 s (after), run 12 jobs at once, so their times are not comparable and are not reported as cost.

### Plain conclusions

- **No learner beats the trivial baselines on the pendulum, the double pendulum, the n-body system or the bounce, at any horizon.** The constant-velocity baseline has error 0.0003 at horizon 1 on the pendulum, against 1.7528 for every learner. The learners are a long way behind even the hold baseline on every conservative system.
- **On the spring, the learners "beat" the trivial baseline at horizons 10 and 100, and that win is not learned dynamics.** At horizon 10 the learner's 1.85 is below hold's 2.09, and at horizon 100 its 0.52 is below hold's 2.81. The learners emit the zero state, and the damped spring decays toward zero, so zero is closer to the truth than a held state. The learners are not better than the zero state anywhere. The zero state is not one of the baselines in this table, and it should be the next one added. At horizon 1 the spring learners are at 3.29 against 0.06 to 0.18 for the baselines, which is the constant-output error showing through.
- **Energy drift confirms the zero state.** A learner whose predicted state is zero has energy zero, so its relative drift is exactly `|0 − E_0| / |E_0|`, which is 1 for the pendulum (the reported 1.00). The hold baseline's drift on the pendulum is 1.7 × 10⁻⁶ because holding a state conserves energy; that is why drift is never read alone. The entropy row on the double pendulum after the fix has drift 1.24e5 and is not the zero state.
- **Bits per byte and rollouts disagree in an instructive way.** The learners code held-out physics below uniform (2.4 to 6.1 bits per byte across the five systems after the fix, the L2 rows), yet their greedy generations are constant. A distribution that is well calibrated over many bytes can still have its mode at one byte, and greedy decoding reads only the mode. The rollout measures what the learner predicts, and bits per byte measures how much it codes; the two are not the same question.
- **So the answer to the P6 question is no, in this configuration.** At 192 KiB of training and greedy `i32` decoding, none of the three learners generates a trajectory that tracks the true one. Sampling at temperature 0.5 was run once and did not change it (above). A learned value read-out and more training are not measured here.

### Limits of this measurement

- One seed, one set of 8 held-out trajectories per system, one budget (192 KiB), one configuration per learner. The learners are untuned.
- Greedy decoding was the main run. Sampling at temperature 0.5 with eight samples was run once, on the same seed, and is reported above.
- The context is 10 timesteps. A longer context is not tested.
- Trajectory length 110 steps, so horizon 100 is the longest measured.
- The zero-state baseline is described above but not in the table, and it is not part of `physics-rollout`. The spring "beats" are therefore explained by the zero-state check, not measured against it in the command.
- The energy drift of the entropy learner on the n-body system (6.61 × 10⁹) (6.61 × 10⁹ before the fix, 4.18 × 10⁹ after) is a figure from garbage predictions, not a physical drift.

## Multi-seed held-out after the depth gate (plans M3, F1)

The arena now refuses a candidate nested deeper than `MAX_DEPTH` before it runs, counted as `depth` (`ARENA.md`, "Multi-seed held-out, and bounded grafts"). These are the six-seed runs (seeds 1–6, 20 rounds of 48 programs, `--cost fuel`, held-out `README.md,germline/src/gate.rs`, agents `grammar:1,mutator:1,attacker:1`), before and after the gate, from the same arguments.

| | before the gate | after the gate |
|---|---|---|
| shape arm, mean held-out bpb (sd) | 7.654 (0.120) | 7.663 (0.236) |
| behaviour arm, mean held-out bpb (sd) | 7.578 (0.194) | 7.557 (0.236) |
| behaviour − shape, paired (se) | −0.076 (0.072) | −0.106 (0.122) |
| shape arm, credited exploits per run (mean) | 110.7 | 41.0 |
| behaviour arm, credited exploits | 0 | 0 |
| F1: unbounded mean bpb | 7.578 | 7.557 |
| F1: bounded mean bpb | 7.697 | 7.735 |
| F1: bounded − unbounded, paired (se) | +0.119 (0.103), worse on 5 of 6 | +0.178 (0.116), worse on 6 of 6 |
| F1: bounded arm, mutator variants past `MAX_DEPTH` | 4% (127 of 3,215) | 0% (0 of 3,077) |

No difference between the arms is significant at two standard errors, either before or after the gate. The default stays `behaviour` and bounded grafts stay off. The exploit counts are the change: the shape arm's credited exploits fell by about two-thirds, because over-depth attacker candidates are now refused rather than credited. Per-seed figures and the reasoning are in `ARENA.md`. The before figures are from the pre-gate commit `1cacaa8fd`, reproduced this session.
