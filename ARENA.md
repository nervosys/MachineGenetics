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

## Fixing the drift: one proxy that backfired, and a direct measure

**Entropy floor (run 8): backfired.** It charged every agent for program
length (the Solomonoff prior) and paid in full only for output above 1
bit/byte (LZ76). Noise has the highest entropy of all, so the drift reversed,
from constants to hash-like `wrapping_mul` scans. The learner's fit to its own
data collapsed to 8.10 bits/byte (incompressible), and held-out got worse. It
was the same Goodhart shape as every novelty key before it: a proxy for
structure maximised by something that is not structure. The floor is now off
by default and kept only for the record.

**Compression progress on a probe (run 9): works.** Credit is the bits a
round's training removes from a program's output, measured on a *probe*: the
same program run with another seed, never trained on. The first version
re-scored the trained output itself. That measures memorisation, and a
learner with thousands of buckets memorised pseudo-random bytes as well as
structure (74.6 bits vs 74.6). Probed, structure generalises across outputs
and noise does not. A test pins the difference.

| 900k params | alignment (run 6) | entropy floor (run 8) | **compression, probed (run 9)** |
|---|---|---|---|
| best held-out code | 8.69 | 8.82 | **8.49** |
| final held-out code | 9.39 | 9.91 | **8.73** |
| best held-out text | 9.42 | 9.62 | **9.11** |
| final held-out text | 10.20 | 10.49 | **9.22** |
| fit to own data | 4.67 | 8.10 | **2.69** |

The late collapse is gone: held-out oscillates near its best instead of
degrading after round 75. The best programs are nested copies of periodic
templates and nested counting, the regularities the paper found to transfer.
Compression is now the default reward, and alignment stays available. Two
things to watch are output entropy, which is low (LZ 0.62) because repetition
is cheap to learn, and energy (76 kJ against 25 kJ), because every program is
now also run and scored on its probe.

**Confirmed at 25M parameters (run 10).** Compression progress keeps held-out
**code below uniform to the end of the run**: 7.84, 8.03, 7.83, 8.07 and 7.93
from round 50 on. Under alignment, code drifted back above uniform (final
8.29). Text also improves, best 8.41 → 8.21 and final 9.04 → 8.44, but never
crosses 8, so plan 4.3 stays half met. The run took 11,041 training-seconds
against 835: repetitive outputs run long and batches grow. Its energy figure
(5.7 MJ) is unusable, because the run predates per-device metering and shared
the machine with another GPU workload.

## The first clean intelligence-per-joule figures (2026-09-26)

Per-device metering (`--meter-gpus`) was built after a 25M run was charged
for another workload on the other GPU. Its first use was on a machine in
exactly that state: GPU 0 running someone else's job at about 400 W, and
GPU 1 idle. The run was pinned to GPU 1 with `CUDA_DEVICE_ORDER=PCI_BUS_ID`
and `CUDA_VISIBLE_DEVICES=1`, and `--meter-gpus 1` metered that card.

**Attribution was checked, not assumed.** NVML and CUDA can number devices
differently, and the measured draw (~350 W) was close to the other job's
(~400 W). Sampling during a short run showed GPU 1 going from 0 to 2.3–3.5 GB
and 244–357 W, while GPU 0 held at 16.9 GB and ~395 W. The metered card is
the one doing the work.

At 900k parameters with the compression reward (150 rounds):

| | |
|---|---|
| training energy | **47.3 kJ**: ≈39.9 kJ measured on GPU 1, plus a 7.4 kJ labelled CPU estimate |
| mean GPU draw while training | ≈350 W |
| best held-out code | **7.85 bits/byte** (round 125), below uniform |
| best held-out text | 8.58 |
| final held-out mean | 8.89, so end-of-run intelligence per joule is negative |
| at the best code checkpoint | 0.15 bits/byte saved per 39.1 kJ ≈ **3.9·10⁻⁶ bits/byte/J** |

The trajectory is noisy: run 9 and this run, with the same configuration,
took different paths to similar finals. The best checkpoint, not the last,
is the fair figure, which argues for succession to promote a session's best
round rather than its end state.

## Fuel against joules: a poor proxy, measured (plan 3.2)

Decision D2 made joules the fixed unit of compute, with fuel as their
calibrated proxy. Every round now meters its program phase (proposing,
gating, and running every program and probe) and pairs the joules read with
the fuel the evaluator spent, refusals included. `CalibrationSuite` fits
`joules = overhead + slope · fuel` over the rounds (`report.fuel_calibration`).
An intercept is fitted because a meter reads everything its device did,
idle draw and fuel-free work included; a ratio of totals would charge all of
it to fuel.

The first calibration was CPU-only (`--meter-gpus none`), because both GPUs
were busy with other work and the program phase does not use them. That makes
the energy the labelled 65 W wall-clock estimate, so it is a measurement of
time priced at an assumed wattage, and the CPU was shared, which inflates
time. 100 rounds × 48 programs, one n-gram learner:

| | |
|---|---|
| marginal cost | **5.1·10⁻⁵ J per unit of fuel** |
| fixed cost | **51 J per round**, about 0.5 J per program run |
| r² | **0.05**: fuel explains 5% of the variation in energy |
| fuel spent | 25.9M, about 1.3 kJ at the marginal rate, against 5.1 kJ fixed |

**Fuel is not yet a good proxy for what a program costs.** Most of the
program phase's energy goes to work that spends no fuel. The likely causes
are gating (parse, typecheck, effects, canonical hashing) and a thread spawned
per execution, but this fit cannot separate them: every round runs the same
number of programs, so all per-program cost lands in the intercept.

Two consequences:
- The arena's allocation charges agents in fuel, so it undercharges programs
  that are cheap to run but still pay the fixed per-program cost. Plan 7.1,
  allocating in joules, should charge `overhead / programs + slope · fuel`.
- A calibration with the count of gated programs as a second regressor, on a
  quiet machine with a hardware CPU counter (plan 3.3), is what would locate
  the fixed cost.

## Charging agents in joules (plan 7.1)

Agents are now charged in joules by default (`--cost joules`; `--cost fuel`
keeps the old charge for comparison). As first built, each round's program
phase was metered as a whole, and a model split the total among agents by
program count and fuel. Plan 3.5 replaced the split with direct measurement:
every agent's proposing and every program it submits is metered in its own
span, so an agent is charged exactly what its own actions read.

This has a cost: **a run is reproducible from its seed only under a
deterministic meter**, because the budget now follows what the meters read.
The reproducibility test uses a meter that reads 1 J per span.

An A/B on the same seed (60 rounds × 48 programs, CPU-only, 65 W wall-clock
estimate, on a CPU shared with other work):

| | charged in fuel | charged in joules |
|---|---|---|
| best held-out | 7.39 (round 15) | 7.54 (round 20) |
| final held-out | 8.08 | 8.22 |
| shares, last round (grammar-0/grammar-1/uniform/mutator) | 29 / 7 / 6 / 6 | 5 / 18 / 5 / 20 |
| fuel calibration | −4.6·10⁻⁵ J/fuel, r² 0.07 | −3.3·10⁻⁵ J/fuel, r² 0.01 |

What it shows:
- **The allocation changes a lot**, and the learning outcome doesn't
  measurably change. One seed per arm, on a machine whose load varied (the
  fixed cost read 61 J/round in one run and 105 in the other), cannot separate
  a 0.15 bits/byte difference from noise.
- **The marginal cost of fuel was not detected.** Both slopes are negative,
  and the earlier 5.1·10⁻⁵ had r² 0.05. Under a wall-clock estimate on a
  shared CPU, the fuel calibration is consistent with zero. Joule charging
  here therefore reduces to a per-program charge scaled to the measured energy.
  That is the finding plan 3.2 pointed at, stated more plainly.

What would settle it: several seeds per arm, on a quiet machine, with a
hardware CPU energy counter (plan 3.3).

## A fixed energy budget, enforced by the kernel (plan 3.5)

`arena::kernel::EnergyLedger` charges every metered joule the loop spends to
an account: `programs` (proposing, gating, running, probes), `training`, and
`probes` (scoring probes before and after training). Held-out evaluation is
metered and reported but not charged. It is the kernel measuring the loop,
and charging it would make evaluating more often look like doing more work.

Two limits, both checked before work starts rather than after:
- **`--energy-budget J`**, the run's fixed unit of compute. Once the ledger
  reaches it, remaining programs are refused (`budget`), training stops, and
  the run halts, with the reason in `report.energy.halted`.
- **`--joules-per-program J`**, an agent's allowance. Granted `k` programs in
  a round, it may spend `k · J` before the kernel refuses the rest of its
  programs for that round, mid-round.

Both are break-verified: removing either check fails its test.

Metering per action changed the fuel calibration's samples from one per round
to one per program. A 60-round run with a 12 kJ budget (CPU-only, 65 W
wall-clock estimate, shared CPU):

| | |
|---|---|
| rounds run | **22 of 60**, halted at 12,029 J (0.24% over) |
| charged | training 9,998 J (83%), programs 1,950 J (16%), probe scoring 82 J |
| evaluation, uncharged | 46 J |
| fuel calibration | **1.55 J fixed per program + 3.8·10⁻⁵ J/fuel**, r² 0.07, 1,056 programs |

With about 3,600 fuel per program, the fixed part is about 92% of a program's
energy. The per-round calibration's finding now holds on 17 times the samples:
**fuel measures the cheap part of a program's cost**, and gating dominates.
It also shows where a budget goes: at this scale, training is five times
everything the agents do.

## Allocating by hypervolume per joule (plan 7.1b)

`--allocate hypervolume` changes what an agent is credited with when the
budget is divided. The run keeps a front of programs on two objectives:
reward (higher is better) and the program's own metered cost (lower is
better). Each round, an agent's credit is the hypervolume its programs add
to that front as it stood before the round. Dividing by the agent's joules
gives the score that sets its share, with the same floor and softmax as
before. Two properties follow, and the default `share` credit has neither:
- A copy of earlier work earns nothing, because a dominated program adds no
  volume.
- Agents are not credited by the order they ran in.

The cost axis's reference is fixed at four times the costliest program of
the first round that produced any, so a program costing more than that
counts as worse than nothing on cost. Every agent's `hypervolume_added` is
reported under either allocation, so a run can compare the two credits
without changing which one is in force. The front bookkeeping runs outside
the agents' energy spans and is not charged to any agent.

One A/B: seed 7, 60 rounds × 48 programs, a 12 kJ budget, CPU-only (65 W
wall-clock estimate). Another project held GPU 1 at 100%.

| | `share` | `hypervolume` |
|---|---|---|
| rounds before the budget halted the run | 29 | 27 |
| held-out bits/byte, final | 7.618 | 7.517 |
| held-out bits/byte, rounds 15 / 20 / 25 | 7.567 / 7.523 / 7.588 | 7.414 / 7.410 / 7.440 |
| charged: training / programs / probes | 11,569 / 535 / 105 J | 7,924 / 4,079 / 101 J |
| last round's shares (grammar-0, grammar-1, uniform, mutator) | 20 / 12 / 5 / 11 | 6 / 30 / 6 / 6 |
| distinct program shapes | 1,064 | 916 |

Held-out came out 0.10 bits/byte lower under hypervolume at every
checkpoint. **That is not yet evidence for it.** The energy split shows why:
training does the same work every round, yet it read 399 J per round in
one run and 293 J in the other, and programs read 0.38 J and 3.1 J each.
The meter is wall-clock time × 65 W on a CPU shared with other projects'
builds, so these figures measure the machine's load as much as the loop.
Hypervolume allocation did concentrate the budget: by the end, grammar-1
held 30 of 48 programs. It is not the default. Deciding needs the
multi-seed run on a quiet machine, with hardware counters (plan 3.3).

## Canaries and the proxy gap (plan 7.5)

`--canary <string>` plants a string in every held-out corpus. Before any
program's output is credited, pooled or trained on, the kernel checks it for
the canary. If an output carries it, the program is refused (`canary`) and
the run halts with the agent's name in `report.energy.halted`, because
held-out bytes have reached an agent. The check comes first, so a leaked
output teaches the learners nothing. A test agent that repeats the canary
halts the run in round 0, with nothing trained and nothing pooled. The same
agent without a canary planted runs to the end. Removing the check fails
the test. Canaries are off by default: planting one adds its length plus
two bytes to each corpus, which would make held-out figures incomparable
with every earlier run.

The canary covers any route by which held-out bytes reach an agent and come
back out in a program's output. It doesn't cover a leak that changes what an
agent proposes without reproducing the bytes. That is what the proxy gap is
for. Every checkpoint now records `proxy_bpb`, the learner's bits per byte
on the pool's own data (up to 64 KiB of it), and `gap`, which is held-out
minus proxy. The progress line prints it. The proxy is what the loop
optimises, so a gap that widens while the proxy improves is the loop fitting
its own data rather than learning.

**Evaluator refresh (done).** The held-out slice rotates when the gap drifts.
Two flags control it:

- `--heldout-slices K` (default `1`) cuts each held-out corpus into `K`
  contiguous, disjoint slices that cover all of its bytes. The last slice takes
  the remainder. Slices are named `<corpus>#i/K`, numbered from 1. Checkpoints
  score each learner on the active slice of every corpus, so `heldout_bpb` is
  keyed by slice name. `K = 0` is refused.
- `--refresh-drift X` (default `0.5` bits/byte; finite and `>= 0`). The first
  checkpoint on a slice sets a baseline, the mean gap across learners. A later
  checkpoint whose mean gap is more than `X` from that baseline rotates to slice
  `(active + 1) % K` and clears the baseline, so the next checkpoint sets a new
  one. Each rotation is recorded in `report.refreshes` as `{round, from_slice,
  to_slice, gap, baseline}`, with slices numbered from 1.

Rotation happens only at checkpoints, which are the epoch boundaries: the
slice is fixed between two of them, and a rotation takes effect from the next
checkpoint. The checkpoint at which it fires is still scored on the old slice.

Every slice must be at least **1 KiB** (`MIN_SLICE_BYTES`). A run whose
slices would be smaller is refused at start-up, before any round runs. With
two held-out files of about 28 KB each, `K = 4` gives 7 KB slices, which
clears the minimum.

With the default `K = 1` nothing is sliced and nothing rotates, so every
earlier result stays comparable: the checkpoints score the same corpora under
the same names. Final learner figures (`learners[].mean_bpb`) score the whole
held-out data, as they always did, and are not affected by rotation.

The succession suite is not sliced. Its digest, and the registered suite, cover
the full corpora. The gate's held-out suite is registered and frozen, while
refresh is this evaluator's own mechanism for judging the loop. Slicing sits
inside `run`, so a succession session that runs with `K > 1` would slice its
own checkpoints, but the registered suite itself is unchanged.

## An attacker, and the exploit it found (plan 7.4)

`--agents attacker:1` adds an agent in the `attacker` role. It takes the
pool's best entries and wraps each in a rewrite that is the identity on
every list: `xs.reverse().reverse()`, `xs ++ xs.take(0)` or
`xs.map(|v| v + 0)`. Each candidate therefore claims to behave exactly as its
parent. The kernel checks the claim (`kernel::verify_preserving`): the
candidate and its parent must produce the same bytes on three seeds besides
the scoring seed, and the candidate must produce the same bytes twice. Both
run directly rather than from the evaluation cache, under the substrate's
fuel and output bounds. A claim that holds, for a candidate the novelty count
still treated as new, is a confirmed exploit and goes in `Report.attacks`.

**What it found.** Novelty has been keyed on a program's shape since the
seed-keyed exploit was closed. Shape is structure with constants erased, so
a rewrite that changes structure without changing output is a new shape and
earns full novelty credit. On seed 7, 20 rounds of 48 programs, agents
`grammar:1,mutator:1,attacker:1`, CPU only:

| `--novelty` | claims checked | credited as novel | distinct shapes | cache hits |
|---|---|---|---|---|
| `shape` (then the default) | 135 | **70** | 662 | 3 |
| `behaviour` | 180 | **0** | 596 | 392 |

**The fix, behind a flag.** `--novelty behaviour` keys novelty on a
fingerprint of a program's outputs on three fixed seeds, so an
output-preserving rewrite has its parent's key. The fingerprint costs up to
three evaluations per new program, cached, which is where the extra cache
hits come from. It has a hole of its own: two programs that agree on those
three seeds share a key whatever they do elsewhere. A test runs the attacker
under both keys, and fails if behaviour keying falls back to shape
(break-checked).

**The held-out comparison, and the default.** It waited on a quiet machine,
because allocation follows measured joules. It did not need one: with `--cost
fuel`, two runs of one seed differ only in their energy and time fields, so
held-out results are exact and runs can share the machine. Six seeds, 20
rounds of 48 programs, agents `grammar:1,mutator:1,attacker:1`, `--cost fuel`,
held-out `README.md,germline/src/gate.rs`:

| seed | shape: held-out bits/byte | behaviour: held-out bits/byte | shape: credited exploits | behaviour: credited exploits |
|---|---|---|---|---|
| 1 | 7.675 | 7.496 | 115 | 0 |
| 2 | 7.477 | 7.662 | 95 | 0 |
| 3 | 7.756 | 7.572 | 119 | 0 |
| 4 | 7.800 | 7.905 | 120 | 0 |
| 5 | 7.656 | 7.500 | 97 | 0 |
| 6 | 7.561 | 7.333 | 118 | 0 |

Behaviour minus shape is −0.076 bits/byte on average (sd 0.175; lower on 4 of
6 seeds), about one standard error: no detectable held-out effect either way.
The comparison has little power, because at this scale the learner transfers
little to the held-out text (8 bits/byte is uniform). What it does settle is
that removing roughly a hundred credited exploits a run costs nothing
measurable, so `behaviour` is now the default. A config recorded without the
field still reads as `shape`, which is how it ran; a test holds both
(break-checked). `--novelty shape` keeps the old key.

## The description-length unit (plan S1)

The description-length charge (`--length-charge`, per unit) is counted in
`--length-unit`:

- `tokens` (the default): MAGE lexer tokens, `Program::token_len`. Every run
  before this flag was charged in tokens, and that is unchanged.
- `bytes`: the UTF-8 length of the rendered source, the text the token count
  lexes.

A config recorded without the `length_unit` key reads as `tokens`, and a test
holds that (break-checked). The unit is recorded in the report's config.

Per unit, the same `--length-charge` is a different price: a byte is shorter
than a token, so a charge tuned in tokens is too weak in bytes. The choice of
unit is measured in S2 and S3, not made here. Two units are deferred to S2: a
BPE token count under a named tokenizer, and measured bits per byte under an
arena learner. Neither is implemented.

## The harness in MAGE (plan 5.1)

The compression reward is now `arena/harness/reward.mg`, a MAGE program
under `@role(evaluator)`, loaded and gated at start-up. It must be one function
`reward` of seven numbers, it must typecheck, it must carry the evaluator
role, and it must raise no effect error. The compiler's fuel-bounded evaluator
runs it for every program the agents write, and `--reward-program` swaps in
another. Over 200 random inputs it computes what the Rust reward did, to
1e-12. A reward that reaches outside is refused by the language; one without
the role is refused by the loader. This is the first policy the loop could
vary without a Rust change.

It needed `exp`, which did not exist, and neither did 14 of the other 15
builtins the differentiability pass classifies as smooth. `exp`, `ln`, `log`,
`sqrt`, `sin`, `cos`, `tan`, `tanh`, `sigmoid`, `silu`, `gelu`, `mean` and
`dot` now exist, each with an exact forward-mode derivative, checked against
central differences. A guard test requires every smooth builtin the pass
names to resolve and evaluate. The exceptions, `softmax` and `matmul`, need
tensor values the evaluator deliberately lacks, and they are exempt by name.

**The budget allocation is the second policy (plan 5.2).** How an agent's
score becomes its claim on the next round is `arena/harness/allocation.mg`, a
function `weight(score, best, temperature)` under `@role(candidate)`, which
allows no effects at all; `--allocation-program` swaps in another. The kernel
keeps the part that must not vary: every agent's floor, and rounding the
weights to whole programs by largest remainder, so the shares always sum to
the budget whatever the policy says. It refuses a weight that is negative or
not finite. Over 400 random cases, including ties, zero, NaN and infinite
scores and temperatures down to 0, the MAGE policy gives exactly the shares
the Rust `allocate` does; a policy changed by one term fails that test.

Writing the test found a defect in the Rust allocation. The softmax was
unshifted, so below a temperature of about 0.0014 every weight was infinite
and the shares were NaN rounded to garbage. Both versions now shift by the
best agent's term, which gives the same proportions with no overflow. The
default temperature is 0.5 and no flag changes it, so no recorded run was
affected.

**Replay's retention is the third (plan 5.2).** Which entry the pool forgets
when it is over capacity is `arena/harness/retention.mg`, a function
`priority(reward, age)` under `@role(candidate)`, where `age` is the rounds
since the entry was added. The shipped program returns the reward, which is
the rule the pool has always used; `--retention-program` swaps in another. The
kernel keeps the eviction, which removes the lowest-priority entry, oldest
first among ties, and it refuses a priority that is not finite. Sampling
stays uniform. The curriculum is the reward, which was already moved in
plan 5.1: what the agents are paid for is what they are taught next, so
nothing separate remains. The equivalence test holds the shipped retention to
the Rust pool's surviving entries after every insertion, over 300 random
sequences with ties and repeated rewards at capacities of 3 to 8.

## A command-backed proposer (plans 7.3, 3.4)

`--agents command:N` adds N proposers whose programs come from a command.
The command is given with two flags:

- `--proposer-command "<cmd> <args...>"`: split on whitespace, so an argument
  cannot contain a space. Required when a `command` agent is named; a run
  without it is refused, naming the flag.
- `--proposer-timeout S`: seconds, at least 1, default 60. A call that runs
  longer is killed, and proposes nothing.

**The protocol.** Each round the agent calls the command once, with its share
`n`. The prompt arrives on the command's stdin. It holds the grammar, with each
production shown as an example rendered by the arena's own renderer, and its
bounds. It also holds the pool's eight best programs as full source with their
rewards, and the request for up to `n` new programs. The reply is on stdout:
one program body per block, with blocks separated by lines that contain only
`---`. A block may also be a whole source text. The arena parses each block
with the grammar the sampler writes (`Program::parse`), keeps at most `n`,
and drops the rest.

The prompt ends on a clean boundary (plan F3): exactly one newline after the
final instruction line, with no trailing space and never a partial line. A
tokenized model's next-token distribution is skewed by a prompt that ends
mid-token, after Cartesia's *From Tokens to Bytes, Part II*. Each shown
source is written without trailing whitespace. Tests:
`the_prompt_ends_on_a_clean_boundary_for_every_pool_size` and
`an_entry_with_trailing_whitespace_does_not_move_the_boundary`.

```text
range(8)
---
flatten([range(3); 2])
---
range(16).map(|x| wrapping_mul(x, 3))
```

- **It competes on the same terms as every agent.** Its output is ordinary
  candidates and nothing else. Each is gated, fuelled, canary-checked,
  credited and pooled as a grammar agent's would be. It has no path to the
  pool, the learners or the ledger, and a refused program is a refusal in its
  standing. The share and the budget apply as they do to any agent.
- **Its energy is metered by the agent's own span.** `propose` runs inside the
  span the loop opens for every agent, so the command's wall time, and whatever
  the meter reads over it, is charged to the agent's `joules` and to the
  `proposer` role, and to the ledger's `by_kind["command"]` account (plan F2),
  so a command's joules are not mixed with the grammar agents'. The three
  breakdowns sum to the same total. A local model behind the command is
  therefore measured,
  to the extent the run's meters measure the machine: that is the local-model
  half of plan 3.4. Under the CPU estimate it is an estimate, and the report
  says so. Test: `a_command_agent_runs_in_a_full_run_and_its_proposals_are_metered`.
- **Failures are counted, not retried.** A nonzero exit, a timeout, or a
  command that cannot start proposes nothing, and counts a `failed_calls`.
  Its message is in `last_error`.
- **Its standing reports** `calls`, `failed_calls`, `blocks`, `parsed`,
  `unparsed` and `truncated`, with `last_batch_mean_reward`, `last_error` and
  `last_parse_error`. `truncated` counts parsed programs dropped to keep a
  round within its share.

**Not built: the API path.** Plan 3.4's other half is an API model, whose
joules are an estimate from its token counts, labelled as one. That needs a
chosen model and an API key, which is decision D4. No provider client, token
counter or provider-specific code is in the arena. Until D4 is settled, a
command that calls an API would be metered by its wall time on the CPU
estimate. That is a wall-clock figure, not the token-count estimate the plan
asks for.

**A finding about the grammar.** The parser refuses any program nested deeper
than `MAX_DEPTH`. The search's own `mutate` and `crossover` graft subtrees
sampled from depth 0, so they can sit deeper than that. In the property test
(seed 2026, 2,000 sampled programs with one mutation and one crossover each),
637 of 4,000 variants were too deep, about 16%. The search still keeps them,
since it was measured with them. The proposer's parser refuses them, and the
test requires that it does. Making the grafts respect the depth would change
what the search explores, so it is left as a separate decision.

## Succession: authority through the gate (plan 8.1–8.2)

`arena::succession::ArenaWorkload` makes an arena session a germline
candidate. The genome is the arena's configuration. `materialize` runs a
session, and `evaluate` scores it on the registered held-out suite, as
`capability` (primary) and `efficiency` (guard). A better learner therefore
becomes the champion only when `Episode::adjudicate` says so, with every step
journaled, which is the redesign's rule that authority changes hands only
through the gate.

## Physical trajectories (plan P1–P2)

`arena::physics` generates trajectories of five classical systems, and encodes
them as bytes, so that later items can hold them out for byte-level learners.
Every trajectory is a pure function of its parameters, its initial state and
its seed. The same seed gives a bit-identical trajectory.

Systems, integrators and conservation checks:

- **Pendulum**, θ'' = -(g/L) sin θ. Velocity Verlet. Energy drift is below
  1e-4 over 10,000 steps at dt = 1e-3 (measured 2.3e-6).
- **Damped spring**, x'' = -(k/m) x - (c/m) x'. The exact closed-form solution
  (underdamped, critical and overdamped), not a numerical integrator. With
  c = 0 the drift is below 1e-6 (measured 4.4e-16). With c > 0 energy never
  rises from one step to the next.
- **Double pendulum**, equal or unequal masses and lengths, with the standard
  Lagrangian equations written out in `double_accel` (Shinbrot et al., Am. J.
  Phys. 60, 491, 1992). RK4 at dt = 1e-3, which is **not** symplectic. Energy
  drift is below 1e-3 over 2,000 steps (measured 7.6e-11).
- **N-body gravity**, n = 3 in 2D, Plummer softening ε = 0.05, G = 1. Velocity
  Verlet on pairwise forces, at dt = 1e-4 over 20,000 steps. Total momentum
  stays within 1e-12 of zero at every step, and the test checks every step.
  The final-state deviation was at most 1.1e-14 across five seeds. Energy drift
  is below 1e-3 (measured at most 5.2e-10).
- **Bouncing ball**, y'' = -g on a floor at y = 0. Velocity Verlet, which is
  exact for constant gravity, plus contact handling: when a step ends below the
  floor, the crossing time is solved within the step, the velocity reflects
  there with restitution e, and the rest of the step is flown from the floor.
  With e = 1 every peak stays within 1e-3 of the drop height (measured worst
  4e-7). With e = 0.8 consecutive peaks differ by a factor of e² within 1e-3
  (measured worst 8e-7).

Parameter ranges are drawn with the arena's `Rng`, from `ParamRange::Train` or
`ParamRange::HeldOut`. The two are disjoint in every dimension, and the test
checks this over 1,000 samples per system. Initial energies are disjoint too,
for the pendulum, spring and bounce.

| system | Train | HeldOut |
|---|---|---|
| pendulum | L ∈ [0.5, 1.0], amplitude ∈ [0.2, 1.0] rad | L ∈ [1.5, 2.0], amplitude ∈ [2.0, 2.8] rad |
| spring | k ∈ [1, 4], c ∈ [0, 0.3], amplitude ∈ [0.2, 1.0] | k ∈ [6, 9], c ∈ [0.5, 0.9], amplitude ∈ [1.5, 2.5] |
| double pendulum | lengths ∈ [0.8, 1.0], masses ∈ [0.5, 1.0], amplitude ∈ [0.2, 1.0] rad | lengths ∈ [1.2, 1.4], masses ∈ [1.5, 2.0], amplitude ∈ [1.8, 2.8] rad |
| n-body | masses ∈ [0.5, 1.0], orbit radius ∈ [1.0, 1.5] | masses ∈ [1.5, 2.0], orbit radius ∈ [2.5, 3.0] |
| bounce | e ∈ [0.5, 0.8], drop height ∈ [0.5, 1.5] | e ∈ [0.85, 0.95], drop height ∈ [3.0, 5.0] |

Encoding (`encode`, `decode`, `corpus`). All integers are little-endian.

```text
header:  "PHY1" (4 bytes) | system (1) | parameters (f64 each)
         | quantum (f64) | channels (u32) | timesteps (u32)
body:    for each timestep, for each channel: round(value / quantum) as i32
```

The timesteps include the initial state, so a trajectory of n steps has n + 1
of them. Channels per timestep are: pendulum θ, ω; spring x, v; double
pendulum θ1, θ2, ω1, ω2; n-body x1, y1, x2, y2, x3, y3, vx1, vy1, vx2, vy2,
vx3, vy3; bounce y, v. Angles are wrapped to (-π, π] before quantisation.

The quanta are chosen so that each system's largest plausible magnitudes fit
in `i32` with headroom of at least 2^8, and a test checks this on sampled
trajectories. Pendulum, spring and bounce use 1e-5. Double pendulum and n-body
use 1e-4. Each value round-trips within half a quantum. `decode` returns an
error for a bad magic, an unknown system, a wrong channel count, a truncated
body or trailing bytes. `corpus(system, range, seed, trajectories, steps)`
concatenates whole trajectories, and `encoded_len` walks the stream.

The LZ76 measure (`measure::lz_bits_per_byte`) of the first 64 KiB of an
eight-trajectory corpus, in bits per byte:

| source | bits/byte |
|---|---|
| random bytes | 6.83 |
| pendulum | 4.29 |
| spring | 3.81 |
| double pendulum | 3.70 |
| n-body | 3.50 |
| bounce | 4.21 |

The random baseline is 6.83, not the 8 an ideal entropy estimate would give,
because LZ76 is biased low on 64 KiB. The comparison is with that baseline.
Every physical system is well below it, so this is a first, crude learnability
signal. It does not show that a learner can predict these bytes. That is
plan P4. Reproduce the table with
`cargo test -- --ignored --nocapture lz_bits_per_byte_64_kib` (about 25 s in a
debug build).

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

## Loss-to-loss and compute multipliers (plan M1–M3)

Cartesia's *From Tokens to Bytes II* (<https://www.cartesia.ai/blog/from-tokens-to-bytes-part-2>)
finds that runs of one architecture family fall on one smooth loss-to-loss
curve, and that families differ in where the curve sits. Two consequences are
measured here. A family is characterised by its curve, fitted over every
checkpoint it produced across seeds and rounds. And two families are compared
by the compute each needs to reach the same held-out loss, which is a compute
multiplier, read off the frontiers instead of from one run.

Each checkpoint gives two bits-per-byte figures: `proxy_bpb`, the loss on the
pool the loop trains on, and `mean_bpb`, the held-out loss. A family's curve is
the fit `heldout = a + b · proxy^c`, least squares over a grid of `c` from 0.25
to 3.0 in steps of 0.05, with `a` and `b` solved linearly at each `c`. The form
is monotone in the proxy for either sign of `b`. The fit's `rmse` is its
residual, in bits per byte, so a curve that describes its family well has an
rmse near the run-to-run noise.

**Usage.** Run a multi-seed grid with `scripts/arena-seeds.sh`, then measure
the directory it wrote:

```sh
scripts/arena-seeds.sh --out DIR --seeds 1-6 --jobs 4 \
    --variant shape="--novelty shape" --variant behaviour="--novelty behaviour" \
    -- --heldout README.md,germline/src/gate.rs --rounds 6 --agents grammar:1,mutator:1,attacker:1
arena measure DIR --json DIR/measure.json
```

`arena measure` takes run reports or directories, and reads the `*.json` in a
directory. A file in a directory that is not a run report, such as a
`measure.json` written beside the runs, is skipped with a note. A file named
on the command line must be one. `--axis bytes|joules` picks the cost axis, `--target T` the held-out
loss at which multipliers are read. The default target is the median held-out
loss of all checkpoints. `--json OUT` also writes the measurement. Running
`arena --json` writes each learner's fit into the report too (`loss_fits`), so
a single run carries its own curve.

The output has three parts. The first is one line per family, with its fit and
how many checkpoints (`n`) and seeds it pools. The second is the multiplier
`cost_A / cost_B` for every pair of families at the target. The third is the
seed spread: the same multiplier computed on each seed's own frontiers, with
their mean and sample standard deviation. The pooled figure is the one to
quote, and the spread is the uncertainty to quote with it.

**The cost axis.** A family's frontier is its lower envelope of
(cost, held-out loss) points, the checkpoints no other checkpoint beats on both
axes. Between two envelope points, cost is interpolated linearly in log cost as
held-out loss moves. The multiplier is undefined, and printed as `n/a`, when the
target lies outside either frontier's range, so a target must be reachable by
both families.

The default axis is `bytes`, the bytes each learner was trained on. Two reasons.
It is deterministic under `--cost fuel`: two runs of one seed agree exactly, so
the default figure reproduces, while measured joules move with the machine
(`--cost fuel` is what `arena-seeds.sh` forces unless the arguments set it).
And it is a data-efficiency axis, which is what the loss-to-loss curve is drawn
against. Its limit is that it does not charge a larger model for its larger
cost per byte. A bytes multiplier of 1 between two families can still hide a
large joule difference. Use `--axis joules` when the question is compute, and
read its seed spread as the noise it is.

**Caveats.** A multiplier needs two families. The default run has one learner,
so `arena measure` prints fits but no pairs; give two configurations, as in
`--learners ng:3:16:0.01,ng:5:18:0.01`, to get one. Families are named by kind
and describe string, so a transformer on `cuda:0` and on `cpu` are two families.
Checkpoints without joules are skipped on the joules axis. A seed's runs from
every variant that shares its number pool together in its per-seed frontier.
A fit or a multiplier from a few checkpoints is a sketch, which `n` shows. The
seed spread counts only seeds whose own frontiers both reach the target, so a
short run can have none: at six rounds with `README.md,germline/src/gate.rs`,
held-out loss sits between 7.8 and 8.1 bits/byte, and each seed's frontiers
span a few hundredths of a bit, so a target from the pooled median often falls
outside them. Choose `--target` inside the range the runs covered.

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
