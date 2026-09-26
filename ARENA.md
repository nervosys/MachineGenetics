# The Arena

> `arena/` — its own crate, depending on `prototype` and `germline`. The
> self-driven improvement loop: agents write MAGE, the compiler gates and runs
> it, learners predict what it outputs, and every joule is either measured or
> labelled an estimate. **Status: built, tested, measured once — and the first
> measurement is mostly a list of what to fix next.** That is recorded below as
> measured, not smoothed.

## The objective

Set on 2026-09-25:

> A self-driven, agentic system for **multi-objective convex and non-convex
> optimization**, focused on **intelligence per second per watt for a fixed
> unit of compute**, supporting **one to many agents collaborating and
> competing**.

Three consequences shape everything here.

**Intelligence per second per watt is intelligence per joule.** A watt is a
joule per second. The ratio alone therefore cannot tell a slow, frugal system
from a fast, hungry one, so latency is kept as its own objective and the
problem is multi-objective by construction: held-out bits per byte ↓, joules ↓,
latency ↓.

**A weighted sum cannot optimise a non-convex front.** Scalarising objectives
with any fixed weights finds only points on the convex hull of the Pareto
front. `germline::FitnessVector::composite` is an unweighted mean, so
`germline::pareto` was added: dominance, fronts, exact hypervolume, and
per-point contribution. A test pins a concave point that every weighting misses
and hypervolume credits.

**Intelligence has to be measured by something no agent can move.** The score
is bits per byte on held-out natural data, which no agent can read. It is an exact
likelihood, computed deterministically — the top of the verification hierarchy
the 2026 RSI literature ranks evaluators by — and not a judge that can be
flattered.

## The design: self-play pretraining, with MAGE as the substrate

From Cowsik et al., *Self-Play Pretraining with Zero Data* (arXiv:2609.30063):
a generator writes programs, a learner predicts their outputs, and the
generator is rewarded for the **learning progress** it causes —
`|⟨∇θL(y), P ⊙ δθ⟩|`, the alignment between the gradient a sequence would
produce and the direction the learner is actually moving. Rewarding difficulty
collapses into noise; rewarding progress keeps the generator on the learner's
frontier. The paper names the expressiveness of its Brainfuck-like substrate
as the limit on scaling. MAGE is a typed language whose compiler can refuse a
bad program before running it.

| piece | module | what it does |
|---|---|---|
| agents | `arena::agents` | `Proposer` trait; `GrammarAgent` (REINFORCE over MAGE productions), `UniformAgent` (fixed prior, the control), `MutatorAgent` (varies anyone's pool entries). One shared pool; a fixed budget per round split by **learning progress per unit of fuel** |
| program space | `arena::grammar` | typed trees over `range`/`map`/`scan`/`filter`/`reverse`/`sort`/`take`/`flatten`/`[e; n]`, rendered to MAGE; mutation and closed-subtree crossover |
| substrate | `arena::substrate` | parse → signature → typecheck → **inferred**-pure effects → fuel-bounded run → bytes mod 256. Each refusal is data, counted by reason |
| fuel | `prototype::eval::run_metered` | every expression one unit; every returned list its length, **precharged before allocation** where a scalar sets the size; depth bounded on a thread with a known stack |
| learner | `arena::learner` | hashed-context softmax with exact gradients and sparse Adam; the learning-progress reward with a lookback between `e/4` and `e/2` |
| energy | `arena::energy` | NVML's cumulative energy counter, loaded at run time → **Measured**; a wall-clock CPU estimate → **Estimated**, with its wattage stated; unreadable → **Unavailable**, never zero |
| selection | `germline::pareto` | learners' front over (bits/byte, joules, latency); hypervolume contribution as credit |

**Convex and non-convex.** The combinatorial half — which program, which
structure — is the agents' search over program trees. The continuous half —
learner width, context order, learning rate — is the `LearnerConfig` of
competing learners, placed on the same front. Nesting an exact continuous
solver inside the combinatorial search is the next step for that half, not
something built yet.

**Collaboration and competition.** Collaboration is the shared pool: any agent
may mutate or recombine any entry, as in Group-Evolving Agents. Competition is
the budget: `agents::allocate` gives each round's fixed evaluations out in
proportion to credit per unit cost, with a floor so one bad round cannot starve
an agent into silence. With one agent, it reduces to a single loop.

## Running it

```sh
cargo run --release --manifest-path arena/Cargo.toml -- \
    --heldout README.md,germline/src/gate.rs \
    --rounds 40 --programs 48 \
    --agents grammar:2,uniform:1,mutator:1 \
    --learners 3:12:0.02,2:10:0.05,1:8:0.1 \
    --json report.json
```

## What the first runs measured (2026-09-25)

40 rounds × 48 programs, four agents, three learners, on dual RTX 3090 Ti with
NVML live. About 19 seconds per run.

**The learned generator out-competes the fixed prior.** `grammar-1` reached a
credit-per-cost score of 9.56 against `uniform-0`'s 2.13 and took 26 of 48
evaluations in the last round. That is the paper's central ablation, reproduced
at toy scale: a learned generator earns more learning progress per unit of
compute than sampling the prior.

**The reward was hacked twice, within one session.**

1. With the reference snapshot as a short moving average, the best programs
   were constant runs — `[129; 6]`, `range(41).map(|v| 232)`. A short lookback
   makes the reward self-confirming: whatever the learner just saw dominates its
   recent movement, so more of the same aligns best. Fixed with the paper's
   `e/2` window plus count-based novelty (repeats earn `progress / (1 + n)`).
   Distinct outputs rose to 80%.
2. The generator then found `[s; k]` — constant runs keyed on the seed, so each
   seed is "new" to the novelty count while the content stays trivial. **Closed
   2026-09-25** by counting novelty on a program's *shape*: its structure with
   bound names canonicalised and constants erased (`mage_prototype::canon`).
   A test pins it: 40 genuinely different outputs from `[s; k]` over five `k`
   and eight seeds earn one shape's harmonic credit (≈4.3), not 40.
3. **The generator then padded its way to new shapes.** The best programs of
   the next run were constant emitters in varied syntactic wrappers —
   `range(19)….map(|v| s).map(|v| 222)`, `[78; 5].reverse().sort().reverse()
   .map(|v| 2)` — each a distinct shape, all saying one thing. **Open.** Each
   fix has moved the exploit one level up — bytes, seeds, syntax — and the
   pressure under all three is the same: an n-gram learner's learning progress
   is largest on constant runs. The two remedies the paper implies are charging
   description length (the Solomonoff prior) to every agent's credit, so
   padding costs, and a learner whose frontier is not constants (next step 1).
   An attacker agent paid to find the next level is plan task 7.4.

**Transfer to natural data is weak, and the learner is why.** The learners fit
their training data (1.9–3.4 bits/byte in distribution), but on held-out text
and code only the largest beats a uniform model (7.51 bits/byte). The two
smaller ones get *worse* the longer they train (9.3 and 12.7): they overfit the
synthetic distribution. An n-gram learner can only transfer byte frequencies.
Copying, recursion and composition — the regularities self-play pretraining
found to transfer — need a learner with in-context mechanisms, which is the
paper's own finding about its transformer. **The loop is not the bottleneck;
the learner class is.**

**Energy is honest and mostly idle.** NVML measured both GPUs across every
training span (≈1,000 J for the largest learner over the run), but the learner
runs on the CPU, so that figure is idle draw attributed to the work, beside a
CPU figure that is an estimate. The report says both. Only a GPU learner makes
the measured joules the ones doing the work.

## The transformer learner, and what it showed (2026-09-25)

**Choosing the backend (plan 4.1), measured.** The repository's own ABL
training path computes attention in scalar Rust loops over host memory, and no
MAGE source has ever trained an attention net through it. candle trained a
3.4M-parameter, 4-layer causal byte model at **20 ms per step, about 408k
tokens/s** on one 3090 Ti, which is 134× faster than its own CPU backend on the
same model. candle it is. On Windows the `gpu` build needs MSVC's
`vcvars64.bat`, `CUDA_PATH`, and `NVCC_APPEND_FLAGS=-Xcompiler /Zc:preprocessor`,
because CUDA 13's headers refuse MSVC's traditional preprocessor.

**The learner (plan 4.2).** `arena::transformer`, behind the `transformer`
feature (candle on CPU, tested in CI) and the `gpu` feature (CUDA). It keeps
its own Adam state so the learning-progress reward uses the real
preconditioner, and it feeds `[BOS, …]` so every byte is predicted and its
bits per byte are comparable with the n-gram learner's. Its in-context test
drives a period-5 pattern below 30% of its starting bits per byte.

**The experiment (plan 4.3): the acceptance bar is not met, and the reason
is new.** Transformer only, 150 rounds × 48 programs, 24 steps a round, on the
GPU:

| round | held-out text | held-out code | joules (measured + estimated) |
|---|---|---|---|
| 25 | 9.69 | 9.45 | 5,848 |
| 75 | 10.40 | 9.64 | 16,193 |
| 150 | 11.22 | 10.48 | 30,881 |

Held-out bits per byte get *worse* with training on both, and the model fits
its own training data only to 5.9 bits per byte after 3,600 steps. A
900k-parameter transformer that cannot fit its data is being fed something
close to incompressible, and that is what the vocabulary produces. Chains of
`wrapping_mul` and `wrapping_add` modulo 256 behave like hash functions, so most
generated programs emit **pseudo-random bytes**. The learning-progress reward
is right to pay nothing for noise, and the only learnable structure left is
constant runs. That is why the generators returned to constants under every
novelty key and with both learners. **The bottleneck is the program
vocabulary as much as the learner.** The paper's substrate naturally produces
copying, repetition and recursion; this one mostly produces noise and
constants. The next step is structure-producing combinators (copy, interleave,
nest, template) in place of hash-like arithmetic, measured by what the learner
can fit.

The GPU transformer's training was the cheaper learner in the mixed run:
2,481 J against the CPU n-gram's 10,167 J for the same rounds.

**The structural vocabulary helps, and is not enough.** Three productions were
added: `copy` (`flatten([xs; n])`), `cycle` (periodic templates) and `nest`
(nested counting). Two measurements follow.

First, a quick proxy, `arena --prior-lz N`. It scores what the uniform prior
writes with the Lempel–Ziv (1976) entropy-rate estimate (`arena::measure`) on
identical 32-byte windows, because the estimate is length-biased and the two
vocabularies write different lengths. The old nine productions score **3.44
bits/byte**; all twelve score **1.84**.

Second, the real test: run 5 repeated exactly, with only the vocabulary
changed.

| | old vocabulary | new vocabulary |
|---|---|---|
| fit to own training data | 5.91 | **4.67** |
| held-out code, best / final | 9.45 / 10.48 | **8.69** / 9.39 |
| held-out text, best / final | 9.69 / 11.22 | **9.42** / 10.20 |
| training joules | 30,881 | **24,969** |

The learner fits more, transfers better and spends less, and held-out
prediction now *improves* for about 75 rounds before degrading. It still never
beats a uniform model on natural data, so plan task 4.3 remains unmet. Two
limits remain. One is scale: 900k parameters and 2.5M bytes, against the
paper's 25M-parameter models trained far longer. The other is a policy that
drifts back toward low-entropy programs as the run goes on (mean output LZ
falls to 1.11), whose best programs still end in constant maps.

**Scale: the first transfer below uniform.** The same run at 25.6M parameters
(width 512, 8 layers, 8 heads, learning rate 3·10⁻⁴), pinned to the idle
second GPU:

| round | held-out text | held-out code | cumulative joules |
|---|---|---|---|
| 25 | 9.49 | 8.80 | 77,023 |
| 50 | 8.73 | 8.15 | 172,723 |
| **75** | **8.41** | **7.79** | 212,089 |
| 100 | 8.61 | 7.85 | 231,360 |
| 150 | 9.04 | 8.29 | 266,951 |

At round 75, held-out **code is predicted at 7.79 bits/byte, better than a
uniform model**. That is the first measured transfer from zero natural data:
the learner saw nothing but MAGE programs written by agents. Scale is a
clear lever: best code goes from 8.69 at 900k parameters to 7.79, and best
text from 9.42 to 8.41. Plan task 4.3 is half met, because text is not yet
below uniform.

Both sizes **peak near round 75 and then degrade**, while their programs'
output entropy falls. That is the policy drift, and it now has a clear
signature: scale moves the curve down, and drift bends it back up. Fixing the
drift comes next: a description-length charge on every agent's credit, and an
entropy floor.

About 0.9 bits/byte on code cost roughly 10× the energy (267 kJ against
25 kJ). NVML sums both GPUs, including the display adapter's ~27 W at rest,
so per-device metering is owed before intelligence-per-joule figures can be
compared across machines. The run also held 24 GB of GPU memory and 9 GB of
host memory, far above what 25M parameters need. That is worth profiling
before scaling further.

## What surfaced in MAGE itself

The arena runs the compiler thousands of times on programs no person wrote,
and in its first hour it found three defects in the language. All three were
fixed on 2026-09-25, and the account is in `HANDOFF.md` items 38–40:

- **Integer overflow depended on the build profile** (a panic in debug, a
  wrap in release). It now traps in every build, and modular arithmetic is
  asked for by name (`MAGE_SPEC.md` §4.10). The generator renders its
  arithmetic as `wrapping_add` and friends.
- **The method-call spelling was not typed at all.** `xs.filter(p).map(f)`
  returned a fresh type variable, so the arena's type gate had been checking
  almost nothing in its own programs. Method calls to vocabulary names are now
  typed as the call, and closures are checked against what their combinator
  expects.
- **`range`'s error recommended syntax that does not parse.** It now
  recommends a form a test evaluates.

## Next, in order

1. **A GPU learner** — a small byte-level transformer behind the same `Learner`
   interface, so the measured joules are the ones doing the work and the
   learner class can express what self-play is supposed to teach.
2. **Structural novelty**, to close the seed-keyed exploit.
3. **The continuous half**: an inner solver over `LearnerConfig` nested inside
   the program search.
4. **An LLM agent** as a fourth `Proposer`, competing on the same budget.
5. **Promotion through `germline`**: learners that survive the front become
   candidates for `Episode::adjudicate`, so authority still changes hands only
   through the gate.
