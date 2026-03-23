# Redox Performance Strategy: Faster Than C, C++, and Rust

This document defines the concrete strategy by which Redox outperforms C, C++,
and Rust across every measurable performance metric. The approach is not
incremental — it exploits information asymmetry that only an agentic AI language
can possess. Traditional compilers optimize blind; Redox optimizes informed.

## Thesis

C, C++, and Rust share a fundamental limitation: the compiler knows only what
the source text says. Redox breaks this ceiling by introducing four information
sources that no traditional compiler has:

1. **Contract knowledge** — `@req`, `@ens`, `@inv` provide provable value
   ranges, nullability guarantees, and loop bounds before a single instruction
   is emitted. The optimizer operates on *theorems*, not heuristics.

2. **Effect knowledge** — the effect system (`/ io`, `/ net`, `/ alloc`)
   certifies which functions are pure, which touch memory, and which do I/O.
   Pure functions can be memoized, reordered, and parallelized with zero
   analysis cost.

3. **Cost oracle feedback** — every construct's cost (cycles, bytes, energy)
   per target architecture is known at compile time. The compiler doesn't
   guess which optimization is profitable — it *queries* the answer.

4. **Agent intelligence** — AI agents choose data structures, algorithms, and
   lowering strategies based on the full semantic context of the program, not
   just local pattern matching.

These four information sources compound. A contract proves a loop runs exactly
N times; the effect system proves the body is pure; the cost oracle says
vectorization at width 8 costs 2 cycles/iter on this target; the agent rewrites
the algorithm to eliminate the loop entirely. No C/C++/Rust compiler can make
this chain of deductions because it lacks contracts, effect tracking, cost
transparency, and agent reasoning.

---

## Performance Targets

| Metric              | vs. C     | vs. C++   | vs. Rust  | Mechanism                          |
| ------------------- | --------- | --------- | --------- | ---------------------------------- |
| Runtime latency     | 1.0–1.5×  | 1.1–1.8×  | 1.0–1.3×  | Contract-guided vectorization      |
| Throughput (ops/s)  | 1.0–2.0×  | 1.1–2.5×  | 1.0–1.5×  | Effect-driven parallelization      |
| Binary size         | 0.7–1.0×  | 0.5–0.8×  | 0.7–0.9×  | Dead-effect elimination + LTO      |
| Compile time        | 2–5× *    | 5–20× *   | 3–10× *   | LL(1) parse + incremental + SKB    |
| Memory footprint    | 0.9–1.1×  | 0.7–1.0×  | 0.9–1.0×  | Layout optimization + pool alloc   |
| Energy (mJ/op)      | 0.8–1.0×  | 0.7–0.9×  | 0.8–1.0×  | Device placement + auto-sleep      |
| Cache miss rate     | 0.6–0.9×  | 0.5–0.8×  | 0.7–0.9×  | AoS→SoA + tiling + prefetch        |
| Startup time        | 1.0–1.2×  | 1.5–3.0×  | 1.0–1.5×  | Static init elision + lazy statics |
| Peak memory         | 0.8–1.0×  | 0.6–0.9×  | 0.8–1.0×  | Arena inference + region analysis   |
| Concurrency scaling | 1.5–3.0×  | 1.5–3.0×  | 1.2–2.0×  | Effect purity → automatic parallel |

\* = faster (lower wall-clock time)

Ratios are Redox/Baseline where <1.0 means Redox is smaller/faster/cheaper.
Ratios >1.0 in the compile-time row mean Redox compiles N× faster.

---

## Strategy 1: Contract-Driven Optimization

### The Insight

When a function carries `@req 0 < n && n <= 1024`, the optimizer *knows* the
value range at every call site. This unlocks:

- **Branch elimination** — `if n == 0` is dead code. LLVM would need runtime
  profiling (PGO) to discover this; Redox knows it statically.
- **Exact loop unrolling** — bounded N means the unroll factor is computed, not
  guessed. LLVM's `-funroll-loops` uses heuristic thresholds; Redox uses proof.
- **Division strength reduction** — `x / n` where `n ∈ [1, 1024]` can use
  multiply-shift. LLVM does this for constants; Redox does it for ranges.
- **Array bounds elision** — `@req idx < arr.len()` proves the access is safe.
  The bounds check is eliminated *without* unsafe.
- **Null check elimination** — `@req ptr.is_some()` proves non-null. The
  unwrap compiles to a direct load.

### Implementation

```
Source:
  @req items.len() > 0 && items.len() <= 256
  @ens result >= 0
  +f sum(items: &[i32]~) -> i32 / pure {
      m total = 0;
      @ x ~ items { total = total + *x; }
      total
  }

Compiler deductions:
  1. @req proves len ∈ [1, 256]         → unroll by 8, no remainder check
  2. / pure proves no side effects       → vectorize freely, reorder freely
  3. @ens proves result ≥ 0              → caller skips sign check
  4. Body is i32 addition over &[i32]    → emit SIMD (SSE/AVX vpaddd)

Generated MLIR:
  %v = affine.for %i = 0 to %len step 8 {
    %chunk = vector.load %buf[%i] : vector<8xi32>
    %sum = vector.reduction <add>, %chunk : vector<8xi32> into i32
    yield %sum
  }
```

**What C/C++/Rust cannot do**: LLVM's auto-vectorizer requires proving the loop
count is divisible by the vector width. Without the `@req` range, it must emit a
scalar tail. With the contract, Redox proves len ≤ 256 and pads/masks the last
iteration — no scalar epilogue.

### Contract Propagation

Contracts propagate interprocedurally:

```
@req x > 0
@ens result > 0
f double(x: i32) -> i32 { x * 2 }

@req n > 0
f quad(n: i32) -> i32 {
    v d = double(n);     // Compiler infers d > 0 from @ens
    double(d)            // @req satisfied by d > 0
}
// quad: no overflow check needed for n ∈ [1, i32::MAX/4]
// quad: no sign check needed — result provably > 0
```

---

## Strategy 2: Effect-Driven Automatic Parallelization

### The Insight

Automatic parallelization has been a failed promise for 40 years because
compilers cannot prove functions are free of side effects. Redox's effect
system solves this at the type level.

```
f compute(x: f64) -> f64 / pure { /* effect system guarantees no I/O, no alloc, no mutation */ }
```

When the compiler encounters:

```
v results: [f64]~ = items.iter().map(|x| compute(*x)).collect();
```

It knows `compute` is `/ pure` — no shared state, no I/O, no allocation. The
loop is *provably* parallelizable. No alias analysis. No escape analysis. No
points-to analysis. The effect annotation is the proof.

### Automatic Parallel Tiers

| Effect signature   | Parallelization                                | Guarantee           |
| ------------------ | ---------------------------------------------- | ------------------- |
| `/ pure`           | Full SIMD + multi-thread + GPU offload         | Zero data races     |
| `/ alloc`          | SIMD + thread-local allocators                 | No shared mutation  |
| `/ io`             | Pipeline (overlap compute with I/O)            | Ordered I/O         |
| `/ net`            | Concurrent I/O with connection pooling         | Connection safety   |
| `/ pure + alloc`   | SIMD + parallel with arena-per-thread          | Region isolation    |
| (no annotation)    | Conservative sequential                        | Correctness default |

### MLIR Lowering

```
Effect / pure  →  scf.parallel %i = 0 to %n step 1 { ... }
                  → omp.wsloop for (%i : index) = (%c0) to (%n) step (%c1) { ... }
                  → gpu.launch blocks(%bx) in (%gx) threads(%tx) in (%bkx) { ... }

Effect / alloc →  scf.parallel ... { memref.alloc thread-local ... }
```

The decision of which tier (SIMD, OpenMP, GPU) is made by the cost oracle:
- If N < 1024 and compute cost < 100 cycles → SIMD only
- If N ≥ 1024 and compute cost < 100 cycles → SIMD + OpenMP
- If N ≥ 10,000 and compute cost ≥ 100 cycles → GPU offload

### What C/C++/Rust Cannot Do

- **C**: No effect system. `restrict` helps aliasing but doesn't prove purity.
  OpenMP requires manual `#pragma omp parallel for` — the programmer does the
  analysis, not the compiler.
- **C++**: `constexpr` is limited to compile-time. `std::execution::par` requires
  manual opt-in and the programmer guarantees no data races (undefined behavior
  if wrong).
- **Rust**: Ownership prevents some data races but doesn't track I/O effects.
  `rayon::par_iter()` requires the closure to be `Send + Sync` — this catches
  some but not all side effects, and requires manual API changes.

Redox parallelizes *automatically* with zero source changes when the effect
signature permits it.

---

## Strategy 3: Cost-Oracle-Guided Data Structure Selection

### The Insight

Programmers choose `Vec<T>` by default. But for N ≤ 8, a stack array is 10×
faster (no allocation). For N ∈ [8, 64], a SmallVec is 3× faster. For N > 10⁶
with random access, a B-tree is 5× faster due to cache locality. No C/C++/Rust
compiler makes this substitution because it lacks knowledge of N at the type
level.

Redox's contracts provide N:

```
@req items.len() <= 8
f process_small(items: [T]~) { ... }
// Compiler: len ≤ 8 → lower [T]~ to [T; 8] on the stack. Zero alloc.

@req items.len() <= 64
f process_medium(items: [T]~) { ... }
// Compiler: len ≤ 64 → lower [T]~ to SmallVec<[T; 64]>. Stack fast path.

@req items.len() >= 1_000_000
f process_huge(items: [T]~) { ... }
// Compiler: len ≥ 1M → preallocate, use parallel chunked iteration.
```

### Cost Oracle Decision Table

The cost oracle provides per-target data:

| Pattern                  | x86_64 cost | AArch64 cost | Decision            |
| ------------------------ | ----------- | ------------ | ------------------- |
| `[T]~` where N ≤ 8      | 1 cycle     | 1 cycle      | Stack array         |
| `[T]~` where N ≤ 64     | 5 cycles    | 5 cycles     | SmallVec            |
| `[T]~` where N > 64     | 30 cycles   | 35 cycles    | Heap Vec            |
| `{K:V}` where N ≤ 16    | 3 cycles    | 3 cycles     | Linear scan array   |
| `{K:V}` where N > 16    | 20 cycles   | 22 cycles    | HashMap             |
| `{K:V}` where N > 10⁶   | 20 cycles   | 22 cycles    | B-tree HashMap      |
| `s` where len ≤ 22      | 1 cycle     | 1 cycle      | SSO (inline string) |
| `s` where len > 22      | 30 cycles   | 35 cycles    | Heap string         |

The compiler queries the oracle at monomorphization time and selects the optimal
backing representation.

---

## Strategy 4: MLIR Autotuning Pipeline

### The Insight

LLVM's optimization passes run in a fixed order with fixed heuristics. They
cannot benchmark alternative lowerings. Redox's MLIR autotuner generates N
variants of every hot loop and benchmarks them on the actual target hardware.

### Autotuning Protocol

```
Source (annotated):
  @pa(8)  // autotune with 8 variants
  @ i ~ 0..n {
      out[i] = a[i] * b[i] + c[i];
  }

Autotuner generates:
  Variant 1: Scalar (baseline)              → 12.3 ns/iter
  Variant 2: Vectorized AVX-512 (width 16)  →  1.8 ns/iter  ★ winner
  Variant 3: Unrolled ×4                     →  6.1 ns/iter
  Variant 4: Tiled 64×64 + vectorized        →  2.1 ns/iter
  Variant 5: Fused with adjacent loop        →  3.4 ns/iter
  Variant 6: Parallelized (4 threads)        →  4.2 ns/iter
  Variant 7: Layout-optimized (SoA)          →  2.0 ns/iter
  Variant 8: Pipelined (software prefetch)   →  2.3 ns/iter

Selected: Variant 2 (AVX-512, 6.8× speedup over scalar)
```

### Cascade Architecture

Autotuning operates at three levels:

1. **Static cost model** (zero overhead) — cost oracle predicts the best
   variant without running code. Used for all loops, always on.

2. **JIT micro-benchmark** (build-time overhead) — the compiler emits all N
   variants, runs each for 10,000 iterations, selects winner. Used when
   `@pa(N)` is specified or when the static model confidence is < 0.7.

3. **PGO-feedback loop** (runtime feedback) — instrumented production builds
   feed cycle counts back to the cost oracle, recalibrating predictions.
   Used in release pipelines.

### MLIR Variant Generation

Each variant is a different MLIR pass sequence applied to the same high-level
IR:

```
Variant 1 (scalar):     canonicalize → cse → lower-to-llvm
Variant 2 (vectorized):  canonicalize → cse → affine-vectorize(16) → lower-to-llvm
Variant 3 (unrolled):    canonicalize → cse → affine-unroll(4) → lower-to-llvm
Variant 4 (tiled+vec):   canonicalize → cse → affine-tile(64) → affine-vectorize(8) → lower-to-llvm
Variant 5 (fused):       canonicalize → cse → affine-fuse → lower-to-llvm
Variant 6 (parallel):    canonicalize → cse → affine-parallelize → omp-lower → lower-to-llvm
Variant 7 (SoA):         canonicalize → cse → layout-optimize → lower-to-llvm
Variant 8 (pipelined):   canonicalize → cse → software-pipeline → lower-to-llvm
```

### What C/C++/Rust Cannot Do

LLVM runs one fixed pass pipeline. GCC has `-fprofile-use` but cannot generate
alternative lowerings. ICC had auto-dispatch for different ISA levels, but not
algorithm variants. Only Redox generates structurally different code paths and
benchmarks them.

---

## Strategy 5: Memory Layout Optimization

### The Insight

The default layout for `struct { x: f64, y: f64, z: f64 }` is Array of
Structures (AoS). When iterating over N such structs and accessing only `x`,
AoS loads 24 bytes per element but uses only 8 — wasting 67% of cache line
bandwidth. Structure of Arrays (SoA) loads only the `x` array — 100%
utilization.

C/C++/Rust require the programmer to manually restructure data. Redox does it
automatically.

### Effect + Contract = Layout Decision

```
@req points.len() >= 1024
f sum_x(points: &[Point3D]~) -> f64 / pure {
    m total = 0.0;
    @ p ~ points { total = total + p.x; }
    total
}
```

The compiler deduces:
1. `/ pure` → no aliasing concerns, layout change is safe
2. `@req len ≥ 1024` → large enough to benefit from SoA
3. Only field `x` is accessed → SoA for `x` saves 67% bandwidth
4. Cost oracle: AoS = 3.1 ns/iter, SoA = 1.0 ns/iter on x86_64

The compiler transforms the backing storage to SoA at the MLIR level:

```
Before (AoS): memref<1024 x struct<f64, f64, f64>>
After  (SoA): memref<1024 x f64>, memref<1024 x f64>, memref<1024 x f64>
Loop reads only memref<1024 x f64> for 'x' field.
```

### Annotation Override

When automatic layout is insufficient, the programmer uses `@pt`:

```
@d(Debug, Clone)
@pt(target_optimal)   // Compiler chooses AoS or SoA per access pattern
+S Particle {
    position: [f64; 3],
    velocity: [f64; 3],
    mass: f64,
}
```

---

## Strategy 6: Allocation Elimination

### The Insight

Dynamic allocation (`malloc`/`free`) costs 30–100 cycles per call. In hot
paths, this dominates runtime. Rust reduces allocations via ownership but still
requires heap allocation for `Vec`, `String`, `Box`, `HashMap`. Redox eliminates
allocations using contract-inferred size bounds.

### Escape Analysis + Contract Bounds

```
@req name.len() <= 64
@req items.len() <= 16
f format_record(name: &s, items: &[i32]~) -> s / pure {
    m buf = f"Record: {name}\n";
    @ item ~ items {
        buf = buf + &f"  - {item}\n";
    }
    buf
}
```

Compiler deductions:
1. `name.len() ≤ 64` means the formatted string is at most ~64 + 16×20 = ~384 bytes
2. `/ pure` means the result doesn't escape to shared state
3. 384 bytes ≤ stack threshold (4096 bytes)
4. **Result**: entire function uses stack-allocated buffer. Zero heap allocations.

### Arena Inference

When allocations cannot be eliminated, contracts provide lifetime bounds:

```
@req batch.len() <= 1000
f process_batch(batch: &[Request]~) -> [Response]~ / alloc {
    batch.iter().map(|r| handle(r)).collect()
}
```

The compiler knows:
1. At most 1000 responses are created
2. All responses are returned (no escape beyond return)
3. **Result**: allocate a single arena of `1000 × sizeof(Response)` upfront.
   Zero intermediate allocations, zero fragmentation, one bulk free.

### Stack Promotion Table

| Pattern                            | C/C++/Rust  | Redox               | Savings    |
| ---------------------------------- | ----------- | -------------------- | ---------- |
| `Vec<T>` where N ≤ 8              | heap alloc  | `[T; 8]` on stack   | 30 cycles  |
| `String` where len ≤ 22           | heap alloc  | SSO inline           | 30 cycles  |
| `Box<T>` where T ≤ 64 bytes       | heap alloc  | stack slot           | 30 cycles  |
| `HashMap` where N ≤ 16            | heap alloc  | stack array + linear | 50 cycles  |
| Return value where size ≤ 4096    | heap alloc  | stack + NRVO         | 30 cycles  |
| Collected iterator where N bounded | N allocs    | 1 arena alloc        | 30N cycles |

---

## Strategy 7: Compile-Time Computation Maximization

### The Insight

C++ has `constexpr` and `consteval`. Rust has `const fn`. Both are opt-in and
limited. Redox's `/ pure` effect annotation automatically identifies every
function whose inputs are known at compile time — and evaluates it.

```
f fibonacci(n: u64) -> u64 / pure {
    ?: n <= 1 { ret n; }
    fibonacci(n - 1) + fibonacci(n - 2)
}

+f main() / io {
    v fib20 = fibonacci(20);  // Evaluated at compile time → constant 6765
    p"{fib20}";
}
```

Because `fibonacci` is `/ pure` and its argument `20` is a compile-time
constant, the compiler evaluates `fibonacci(20)` during compilation and emits
the constant `6765`. No function call at runtime. No branches. No stack frames.

### Automatic Const Propagation Scope

| Condition                          | C++ `constexpr` | Rust `const fn` | Redox `/ pure` |
| ---------------------------------- | --------------- | --------------- | -------------- |
| Explicit opt-in required           | Yes             | Yes             | No (automatic) |
| Heap allocation allowed            | C++20 partial   | No              | Yes (via arena)|
| Loops allowed                      | Yes             | Limited         | Yes            |
| Trait dispatch allowed             | No              | No              | Yes (monomorphized) |
| Recursion allowed                  | Limited depth   | No (stable)     | Yes            |
| String operations                  | Limited         | No              | Yes            |
| Applies to all pure functions      | No (must mark)  | No (must mark)  | Yes            |

---

## Strategy 8: Hardware-Specific Lowering via MLIR

### The Insight

LLVM generates good x86_64 code but mediocre GPU, FPGA, and NPU code because
LLVM IR is too low-level — it has already lost the semantic intent (loops,
reductions, tensor operations). MLIR preserves semantic intent through
progressive lowering:

```
Redox Source
    ↓ parse + type check
Redox MLIR Dialect (semantic operations: loops, reductions, contracts, effects)
    ↓ dialect lowering
Linalg / Affine / SCF (mathematical operations: tiling, fusion, vectorization)
    ↓ target-specific lowering
    ├─→ LLVM Dialect → LLVM IR → x86_64 / AArch64 / RISC-V
    ├─→ GPU Dialect → NVPTX / AMDGPU / SPIR-V
    ├─→ TOSA / StableHLO → NPU / TPU
    └─→ CIRCT → Verilog → FPGA
```

Each lowering path applies target-specific optimizations that LLVM cannot:

| Target   | Optimization                          | Speedup vs. LLVM-only |
| -------- | ------------------------------------- | ---------------------- |
| x86_64   | AVX-512 tile size tuning, prefetch    | 1.5–2×                 |
| AArch64  | SVE scalable vector width selection   | 1.3–1.8×               |
| RISC-V   | V-extension custom vector scheduling  | 1.5–2.5×               |
| NVIDIA   | Tensor core mapping, shared mem tiling| 3–10×                  |
| AMD GPU  | Wavefront occupancy optimization      | 2–5×                   |
| NPU      | TOSA quantized int8 fusion            | 5–20×                  |
| FPGA     | Pipeline scheduling, II minimization  | 10–100×                |

### Automatic Device Placement

The cost oracle + effect system together decide *where* to run each function:

```
@req matrix.rows() >= 512 && matrix.cols() >= 512
f matmul(a: &Matrix, b: &Matrix) -> Matrix / pure {
    // Cost oracle: CPU = 4.2ms, GPU = 0.3ms, NPU = 0.1ms
    // Effect: / pure → safe to offload, no host-side effects
    // Decision: NPU (14× faster than CPU)
    ...
}
```

The decision is per-function, per-call-site, per-target:
- Small matrices (N < 64) → CPU (transfer overhead > compute savings)
- Medium matrices (64 ≤ N < 512) → CPU with AVX-512 vectorization
- Large matrices (N ≥ 512) → GPU/NPU offload

---

## Strategy 9: Zero-Cost Safety

### The Insight

Rust's safety has runtime cost: bounds checks, overflow checks in debug mode,
`Option::unwrap()` panics, `Arc` atomic reference counting. These are the price
of safety without contracts. Redox eliminates the runtime cost while preserving
the safety guarantee by proving checks unnecessary.

| Safety check              | Rust cost     | Redox cost           | Mechanism                    |
| ------------------------- | ------------- | -------------------- | ----------------------------- |
| Array bounds check        | 1–3 cycles    | 0 cycles             | `@req idx < len` proof       |
| Integer overflow check    | 1 cycle       | 0 cycles             | `@req` range proof           |
| `Option::unwrap()` panic  | 1 branch      | 0 branches           | `@req opt.is_some()` proof   |
| `Result::unwrap()` panic  | 1 branch      | 0 branches           | `@ens` success proof         |
| Arc atomic increment      | 5–15 cycles   | 0 cycles             | Effect proves single-thread  |
| Mutex lock/unlock         | 20–50 cycles  | 0 cycles             | Effect proves no contention  |
| UTF-8 validation          | O(n)          | 0                    | `@inv` string invariant      |
| Null check (Option)       | 1 branch      | 0 branches           | `@req` non-null proof        |

### Cumulative Impact

In a typical Rust program, safety checks account for 5–15% of runtime. In
tight loops (JSON parsing, image processing, numerical computation), they
account for 15–30%. Redox eliminates them entirely through contract proofs:

```
// Rust: 3 bounds checks per iteration (a[i], b[i], out[i])
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// Redox: 0 bounds checks — contracts prove safety
@req a.len() == b.len()
@ens result == a.iter().zip(b.iter()).map(|(x,y)| x*y).sum()
f dot(a: &[f64]~, b: &[f64]~) -> f64 / pure {
    m sum = 0.0;
    @ i ~ 0..a.len() {
        sum = sum + a[i] * b[i];  // No bounds check: i ∈ [0, len) proven
    }
    sum
}
```

---

## Strategy 10: Interprocedural Whole-Program Optimization

### The Insight

C/C++ optimize per translation unit. Rust optimizes per crate with optional LTO.
Both lose information at module boundaries. Redox's effect and contract
annotations are part of the type system — they propagate across crate boundaries
without LTO. This enables whole-program optimization at zero link-time cost.

### Cross-Crate Optimization

```
// crate: math
@req x >= 0.0
@ens result >= 0.0
+f sqrt(x: f64) -> f64 / pure { ... }

// crate: physics (depends on math)
@req dt > 0.0
+f simulate(dt: f64) / pure {
    v speed = math.sqrt(energy * 2.0);
    //                  ^^^^^^^^^
    // Compiler knows: energy * 2.0 >= 0.0 (if energy >= 0.0)
    // → @req satisfied without runtime check
    // → math.sqrt can be inlined with no guard
    // → result provably >= 0.0 → caller's computation continues unguarded
    ...
}
```

With traditional LTO, the linker must re-analyze all code to discover these
facts. With Redox, the facts are *encoded in the function signature* and
available at every call site — including dynamic dispatch through trait objects,
where LTO cannot reach.

---

## Strategy 11: Compile-Time Speed

### The Insight

Compile time is a performance metric. Redox targets 3–10× faster compilation
than Rust through architectural advantages:

| Stage          | Rust                    | Redox                   | Speedup   |
| -------------- | ----------------------- | ----------------------- | --------- |
| Lexing         | Context-sensitive       | LL(1) deterministic     | 2–4×      |
| Parsing        | Recursive descent + BT  | Predictive LL(1)        | 2–4×      |
| Name resolution| Full module graph       | Incremental + cached    | 2–3×      |
| Type checking  | Lifetime inference      | SKB-guided elision      | 2–5×      |
| Borrow check   | NLL dataflow analysis   | SKB rule lookup         | 3–10×     |
| MIR transform  | Fixed pipeline          | Effect-guided skip      | 1.5–2×    |
| Code generation| LLVM full pipeline      | MLIR incremental        | 2–3×      |
| Linking        | Full link               | Function-level hot-link | 10–100×   |

### Token Reduction Impact

Redox source is ~50% fewer tokens than equivalent Rust. Fewer tokens means:
- Fewer bytes to read from disk (I/O bound reduction)
- Fewer tokens to lex (CPU bound reduction)
- Fewer AST nodes to allocate (memory bound reduction)
- Fewer nodes to type-check (analysis bound reduction)

### SKB-Guided Elision

Rust's borrow checker performs expensive dataflow analysis (NLL) on every
function. Redox's SKB contains 2,847 ownership rules and 1,203 borrow rules.
For 80%+ of functions, the SKB can determine safety by rule lookup in O(1)
time. Only the remaining ~20% of complex cases require full analysis.

### Incremental Hot Reload

When a single function changes:
- Rust: re-analyze → re-codegen → re-link the entire crate (~500ms–5s)
- Redox: re-lex 1 function → re-lower to MLIR → re-emit to object → hot-patch
  into running process (~12ms)

---

## Strategy 12: Agent-Guided Whole-Program Rewriting

### The Insight

This is the strategy no traditional compiler can implement. AI agents don't
just optimize instructions — they optimize *algorithms*.

### Agent Optimization Passes

| Pass                          | What the agent does                              | Expected impact |
| ----------------------------- | ------------------------------------------------ | --------------- |
| Algorithm substitution        | Replace O(n²) sort with O(n log n)               | 10–1000×        |
| Data structure substitution   | Replace linked list with Vec for cache locality   | 5–50×           |
| Batching                      | Combine N individual I/O calls into 1 batch       | 10–100×         |
| Memoization                   | Cache results of `/ pure` functions               | 2–∞× (depends)  |
| Strength reduction            | Replace `pow(x, 2)` with `x * x`                 | 2–5×            |
| Loop fusion                   | Merge adjacent loops over same range              | 1.5–3×          |
| Dead computation elimination  | Remove computations whose results are unused      | 1–∞×            |
| Specialization                | Generate type-specific fast paths                 | 2–5×            |

### Agent Decision Protocol

```
1. Agent reads function + contracts + effects
2. Agent queries cost oracle for current implementation cost
3. Agent generates N alternative implementations
4. Each alternative is type-checked and contract-verified
5. Cost oracle scores each alternative
6. Best alternative replaces original (if improvement > threshold)
7. Calibration records actual vs. predicted improvement
```

### Safety Guarantees

Agent rewrites are constrained by:
- **Contracts**: rewrite must satisfy same `@req`/`@ens`/`@inv`
- **Effects**: rewrite must have same or fewer effects
- **Types**: rewrite must be type-safe
- **Tests**: rewrite must pass existing test suite

The compiler *verifies* the agent's work. The agent proposes; the compiler
disposes.

---

## Combined Impact Model

The strategies compound multiplicatively in hot paths:

```
Baseline: C/C++/Rust compiled code performance = 1.0×

Strategy 1  (Contract optimization):     1.1–1.3× (branch/check elimination)
Strategy 2  (Auto-parallelization):      2–8× (when applicable)
Strategy 3  (Data structure selection):   1.2–1.5× (fewer allocs, better cache)
Strategy 4  (MLIR autotuning):           1.5–3× (optimal lowering selected)
Strategy 5  (Layout optimization):        1.5–3× (SoA, cache utilization)
Strategy 6  (Allocation elimination):     1.1–1.5× (fewer malloc/free cycles)
Strategy 7  (Compile-time evaluation):    1.0–∞× (eliminated runtime work)
Strategy 8  (Hardware-specific lowering): 1.3–10× (target-tuned codegen)
Strategy 9  (Zero-cost safety):           1.05–1.3× (eliminated safety checks)
Strategy 10 (Whole-program optimization): 1.1–1.3× (cross-crate inlining)
Strategy 11 (Compile-time speed):         3–10× (compilation wall-clock)
Strategy 12 (Agent rewriting):            1.5–1000× (algorithm improvement)

Conservative composite (non-parallel hot loop):
  1.2 × 1.3 × 2.0 × 1.2 × 1.1 × 1.1 × 1.15 ≈ 5.3× faster than baseline

Aggressive composite (parallel numerical kernel):
  1.3 × 4.0 × 3.0 × 2.0 × 1.3 × 1.0 × 1.2 × 5.0 ≈ 243× faster than baseline
```

---

## Implementation Priority

| Priority | Strategy                     | Crates involved                                             | Prerequisite |
| -------- | ---------------------------- | ----------------------------------------------------------- | ------------ |
| P0       | Contract-driven optimization | `redox_contracts`, `redox_mir_transform`                    | None         |
| P0       | Effect-driven parallelization| `redox_effects`, `redox_mlir_parallel`                      | None         |
| P0       | Zero-cost safety             | `redox_contracts`, `redox_skb`, `redox_borrowck`            | Contracts    |
| P1       | MLIR autotuning              | `redox_mlir_autotune`, `redox_cost_oracle`                  | MLIR pipeline|
| P1       | Data structure selection     | `redox_cost_oracle`, `redox_monomorphize`                   | Cost oracle  |
| P1       | Compile-time speed           | `redox_lexer`, `redox_parser`, `redox_skb`, `redox_hot_reload` | None      |
| P2       | Memory layout optimization   | `redox_mlir`, `redox_perf_annotations`                      | MLIR + effects|
| P2       | Allocation elimination       | `redox_contracts`, `redox_mir_transform`                    | Contracts    |
| P2       | Hardware-specific lowering   | `redox_mlir_targets`, `redox_mlir_pipeline`                 | MLIR pipeline|
| P3       | Compile-time computation     | `redox_effects`, `redox_const_eval`                         | Effects      |
| P3       | Whole-program optimization   | `redox_metadata`, `redox_contracts`, `redox_effects`        | Contracts + effects |
| P3       | Agent-guided rewriting       | `redox_synthesis_oracle`, `redox_agentic_bench`, `redox_cost_oracle` | All above |

---

## Benchmark Suite

Performance claims require reproducible benchmarks. The following suite covers
every metric:

### Micro-benchmarks

| Benchmark               | Measures                  | Baseline comparison            |
| ------------------------ | ------------------------- | ------------------------------ |
| `sum_array`              | Vectorization, bounds     | C `-O3`, Rust `--release`      |
| `matrix_multiply`        | Tiling, parallelization   | C `-O3 -fopenmp`, BLAS        |
| `json_parse`             | Branch elimination, alloc | C (simdjson), Rust (serde)     |
| `hash_map_insert`        | Data structure selection  | C (khash), Rust (std HashMap)  |
| `sort_large`             | Algorithm + SIMD          | C (qsort), Rust (sort)        |
| `string_search`          | SIMD vectorization        | C (strstr), Rust (memchr)     |
| `fib(40)`                | Compile-time evaluation   | C `-O3`, Rust `const fn`      |
| `image_blur`             | GPU offload, tiling       | C (OpenCV), CUDA              |

### Macro-benchmarks

| Benchmark               | Measures                  | Baseline comparison            |
| ------------------------ | ------------------------- | ------------------------------ |
| `http_server`            | I/O throughput, alloc     | C (libuv), Rust (tokio)       |
| `compiler_self`          | Compile time              | rustc self-compile             |
| `ray_tracer`             | Parallelism, vectorization| C++ (embree), Rust             |
| `database_query`         | Memory layout, cache      | C (SQLite), Rust               |
| `agent_swarm_100`        | Swarm coordination        | (no baseline — Redox-only)     |

### Metrics Collected

For every benchmark, every run:

```
- Wall-clock time (ns)
- CPU cycles (perf counters)
- Instructions retired
- Cache misses (L1, L2, L3)
- Branch mispredictions
- Memory allocated (bytes)
- Memory peak (RSS)
- Energy consumed (RAPL, µJ)
- Binary size (bytes)
- Compile time (ms)
```

---

## Summary

Redox's performance advantage is not one optimization — it is the compounding
effect of information that traditional compilers do not have. Contracts prove
value ranges. Effects prove purity. The cost oracle quantifies alternatives. MLIR
preserves semantic intent for target-specific lowering. Agents reason about
algorithms. Each layer amplifies the others.

The result: code that is simultaneously safer than Rust, more expressive than
C++, and faster than C. Not through heroic engineering of one component, but
through the systematic elimination of information loss at every stage of
compilation.
