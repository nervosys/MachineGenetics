# Germline — model succession, handoff, and fallback

> **Status: control plane complete and runnable; workload not.**
> `germline/` — its own crate, 286 tests. Variation, directed search, the gate,
> attestation, lineage, the durable journal, the cycle state machine, and
> supervision are built and tested ✅. Model training and inference are **not**
> here and are not claimed ◻ — this decides *whether* a successor takes over, not
> how it is produced. Marked per the rules in [DOCS.md](DOCS.md).
>
> *This said 141 tests. That figure does not survive checking: counting
> `#[test]` in germline's own files gives 112 today and 100 at the commit where
> 141 was written, so it never matched these files and I could not reconstruct
> what it did count. What is verified: the crate reports **112 passing**, and
> the extraction removed nothing — 112 both before and after, across four
> commits.*
>
> *The figures above are left as written. They record what was counted when the
> 141 was investigated, and a record rewritten to match today's number stops
> being evidence of anything.*
>
> *The count has moved since, and is pinned rather than narrated: see
> `germline_tests` in [MEASUREMENTS.md](MEASUREMENTS.md). The growth is one line
> of work, begun 2026-09-15 — the genome becoming a sequence of loci rather than
> a vector of scalars, and everything that had to follow from that: expression
> that refuses a genome naming a gene the registry lacks, a `materialize` that
> builds the source it expressed, `forge`'s registry as the gene pool, and
> blocks carrying the effects that make the legality check able to fail.*

It depends on `ribosome` and nothing else in this repository, and the direction
is load-bearing rather than incidental: the barrier described below is one-way,
so the build engine must not be able to reach back into succession. CI checks
that with `cargo tree` instead of trusting this paragraph.

The operating mode: a model proposes a higher-fitness successor by directed
evolution, hands subsequent RSI work to it, and falls back to its predecessor on
malfunction or decline.

---

## 1. The name, which is the design

In biology the **germline** is the heritable lineage, and the **Weismann barrier**
is the one-way wall stopping somatic changes — anything an organism does to itself
during its life — from entering it. Acquired characteristics are not inherited.

That is not a limitation biology suffers; it is what keeps a lineage from being
corrupted by whatever an individual did to itself. This module enforces the same
barrier for the same reason.

## 2. The problem succession actually poses

Generating a better model is the easy half. The hard half is **who certifies that
it is better**, because the obvious arrangement is circular: the incumbent hands
authority to the successor, and the successor then evaluates *its* successors.
Two things go wrong, both quietly.

**Evaluator capture.** A system with write access to the thing that judges it
will, under optimization pressure, find that editing the judge is cheaper than
satisfying it. This needs no intent and no cleverness — it is simply the
lowest-cost path to a higher score. "RSI" then degenerates into a model learning
to rewrite its own test suite, while every dashboard improves.

**Lineage drift.** Each generation regresses slightly on an axis nobody is
watching, every step within tolerance. Twenty generations later the capability is
gone and no single promotion looks wrong in review.

Both are properties of the *arrangement*, not of any model's disposition. They are
what the invariants below exist to make structurally impossible.

## 3. The invariants

| # | Invariant | Enforced by | Test |
|---|---|---|:--:|
| 1 | The gate and suite are pinned by digest before an episode opens; changing either **voids** it | `Episode::adjudicate` | ✅ |
| 2 | Nothing certifies itself — a verdict naming the challenger as evaluator is refused | `RejectReason::SelfCertified` | ✅ |
| 3 | Promotion rests only on a **held-out** suite | `Generation::heldout_fitness` returns `None` for optimized-suite scores | ✅ |
| 4 | Guard axes ratchet against the **lineage high-water mark**, not the incumbent | `Lineage::best_ancestor` | ✅ |
| 5 | Fallback targets must be **materialized** — verified before authority moves | `Lineage::demote_champion` | ✅ |
| 6 | A demoted generation is not re-promotable without new evidence | `RejectReason::PreviouslyDemoted` | ✅ |
| 7 | History is append-only; demotions stay in the record with their reason | `Lineage` | ✅ |
| 8 | Authority moves **only on the gate's approval**, earned against the champion it replaces | `Lineage::promote` takes a `gate::Approval`, which only `Episode::approve` mints | ✅ |
| 9 | A restarted run's authority comes **only from the verified journal**, replayed under the rules that wrote it | `Lineage` is not `Deserialize`; `Lineage::replay` verifies the chain (and the published head, if given) and re-checks every succession event | ✅ |

On (8): until 2026-09-26 `Lineage::promote` took a generation id, a gate
digest and an evaluator name, and "the caller must already hold an approving
verdict" was its doc comment. Any code with a `&mut Lineage` could grant
authority, and a `Verdict` could not serve as proof because it is public data
anyone can construct or deserialize. `Approval` has private fields, no `Clone`
and no `Deserialize`, so outside the crate it can only be earned, and each one
is spent once. It also records the incumbent it was judged against: an
approval earned against one champion is refused (`StaleApproval`) after
authority has moved to another. A `compile_fail` doctest fails if the fields
become constructible, which was checked by making them public.

On (9), plan 8.3: (8) protects authority inside one process, and a restart
used to be outside it. `Lineage` derived `Deserialize`, so a lineage file
naming any champion would have been as good as the gate's approval. Nothing
read one yet, which is why it was a gap and not a hole. Now a restarted run
calls `Lineage::replay(genesis, &journal, anchor)`. It verifies the hash
chain, compares the head with `anchor` when one was published (the only way
to catch truncation), and re-applies every event through the same
transitions the live methods use. A promotion must name the champion it
displaced and a generation that was proposed and measured on a held-out
suite. A demotion must fall back to a retired generation. A rollback may
not return to a quarantined one. So even a journal rewritten with valid
links can't name a champion the rules would not have produced.
`Runner::resume` continues the cycle numbering, and so the cycle seeds, and
the count of refusals in a row. A test kills a run after every cycle in turn
and checks that the resumed lineage and journal events equal the
uninterrupted run's. Replay requires journals to record each generation's
parent, genome, note and suite, which entries now carry. Older journals are
refused as `Predates` rather than replayed with guesses. Writing the test
found a real defect: serde_json parsed some floats one ULP off, so a
verdict's reasons re-serialised differently and an untouched journal failed
`verify`. germline now enables serde_json's `float_roundtrip`.

On (1): the check exists because a "helpful" relaxation is indistinguishable from
an attack in its effects. `a_successor_cannot_widen_the_gate_that_judges_it`
asserts that a rejected candidate stays rejected when the gate is loosened — the
episode is **void**, not re-judged.

On (4), the drift scenario, which is the one that would otherwise be invisible:

```text
gen0  safety 0.950   ← high-water mark
gen1  safety 0.947   −0.003, inside the 0.01 tolerance
gen2  safety 0.944   −0.003
gen3  safety 0.941   −0.003
gen4  safety 0.938   −0.003 vs gen3: fine. −0.012 vs gen0: REFUSED
```

Every step is the same size. A per-step check passes all of them forever. The
ratchet is what turns "each change was reasonable" into a stopping condition.

## 4. The promotion gate

Five checks, `AND`-ed: evaluator independence, held-out evidence, comparable axes,
primary improvement by a margin exceeding measurement noise, and the guard ratchet.

They are `AND`-ed deliberately. A gate where a large capability gain can purchase a
guard regression is a gate that trades safety for capability **at a fixed exchange
rate** — which is precisely the trade nobody intends to have made, arrived at one
defensible promotion at a time.

The margin matters too: a `min_improvement` below the evaluation's noise floor
promotes coin flips, and a lineage of coin flips drifts wherever the noise points.

### Multi-objective promotion: rule (a) and rule (b)

The primary-axis rule above is **rule (a)**. It cannot promote a challenger that
has the same primary value and a better value elsewhere, such as the same capability
at lower energy. That is the case multi-objective succession needs, so the gate has
an opt-in second route, `PromotionGate.dominance: Option<DominanceRule>`. It is
skipped in serialization when unset, so a gate without it keeps its digest.

With `dominance` set, a challenger passes if **either** rule passes, and all of the
safeguards still apply:

- **Rule (a)**: the primary axis improves by `min_improvement`, and the guards hold
  (as above).
- **Rule (b)**: on every objective axis, and on the primary axis too even if it is not
  listed, the challenger is at least the champion less that axis's tolerance (its
  entry in `DominanceRule.tolerances`, zero if none; guard status does not change
  it, and the guard check runs separately with the guard tolerance). **And** on at least one objective
  axis it is at least the champion plus `DominanceRule.min_improvement`.

Under either rule the following still refuse: the guard checks; a shadow count below
`min_shadow_successes`; a NaN on any objective, guard or primary axis (never an
improvement, never a regression); and an axis missing on either side.

The ratchet applies to every objective axis under rule (b), not only the guards. A
challenger is refused if any objective falls below the best value any ancestor, or
the incumbent, reached, less that axis's tolerance. This is what stops a trade being
walked back toward an earlier generation: if A → B trades efficiency up and a later
generation trades it back down, the drop below B's value is refused even if the
champion comparison alone would pass.

Approvals and refusals say which rule passed or failed, and on which axis. An approved
verdict carries a `basis` (`rule (a): …` or `rule (b): …`), and a refusal under the
dominance rule carries a `DominanceRefused` reason that names the axis each rule
failed on.

**The primary floor.** `DominanceRule.primary_floor` (default `0`) is a second
condition on rule (b): the challenger's primary value must be strictly above it. A
primary value tied at the floor is refused, with the message `rule (b) does not
apply: … is not above the floor …`. Zero intelligence at lower energy is not an
improvement, so with the default floor, a capability tie at 0 cannot promote on
energy alone. Rule (a) is unchanged by the floor.

**Per-objective tolerance.** `DominanceRule.tolerances` is an ordered map from axis
name to tolerance, so the digest is deterministic. Under rule (b) an objective's
tolerance is its entry in the map, or zero if it has none, whether or not it is a
guard. The primary axis is always zero. The guard check is separate and still
applies, with the guard tolerance. So under the CLI's `efficiency` guard of `0.25`,
an efficiency drop is accepted only if the map has an entry for efficiency. The same tolerance applies to the ratchet,
which compares against the high-water mark, not the champion. So across any chain of
promotions, an objective can fall below its best value by at most one tolerance. A
tolerance does not compound over the chain.

**What is still open.** The gate does not set a floor or a tolerance on its own. A
zero floor and zero tolerances are the defaults. Measurement limits that the gate
cannot see are listed in `ARENA.md`, under *Known limits of the measurement*: the
clamped primary axis, the latency scale, and the wall-clock re-measure of the seed.

A **canary phase** (`min_shadow_successes`) runs the challenger on real work while
the incumbent still holds authority, so promotion follows evidence from the actual
environment rather than the suite alone.

## 5. Directed evolution — and the trap in it

Undirected search measures everything, which is ruinous when one evaluation means
training a model. Directed search uses a cheap **surrogate** to rank many
proposals and spends the real budget on the few most promising.

The trap: **a surrogate concentrates the budget.** Well calibrated, that is the
entire win. Miscalibrated, it is worse than no surrogate at all — random sampling
at least explores, while a confidently wrong predictor systematically steers the
budget away from the good candidates and returns a confident answer either way.
And a surrogate fitted to the lineage's own history gets miscalibrated exactly
when the search enters new territory, which is when it is trusted most.

**Until 2026-09-16 none of this was wired into the loop.** `runner.rs` imported
one name from `directed` — `CandidateSpec` — and chose its candidate with
`.take(candidates_per_cycle).next()`, which is element 0 for every non-zero
policy. So the loop proposed eight offspring, evaluated the first, discarded
seven, and `candidates_per_cycle` changed nothing. Every test below passed,
because every one of them exercises `DirectedSearch` directly; none asked
whether the runner used it.

The runner now takes an optional predictor and *samples* within the width trust
has earned, rather than taking the top-ranked candidate outright. Sampling is
the part that matters: at zero trust the width is the whole pool, so a loop with
no predictor is undirected search exactly, and a loop with a fresh one does not
begin following it on the first cycle.

So the predictor is never trusted on its own account:

- Every prediction followed by a real measurement becomes a calibration sample.
- `trust()` falls as mean absolute error grows, and is **0 for an untested
  predictor** — selectivity is earned by being right about things that were
  subsequently measured, not asserted via a confidence field. ✅
- Selection width interpolates from *the whole pool* at zero trust to *the top-k*
  at full trust.

That degradation path is the property worth having: **the response to "my model
of the world is wrong" must be to explore more, not to trust the model harder.**
`a_predictor_that_stops_being_right_loses_its_selectivity` asserts it. ✅

### 5a. Prediction manifests for external edits (plan 5.4)

An external proposer (an agent editing the harness) can be attached with
`Runner::with_proposer`. Its edits are not sampled from the lineage's own
population, and each one must state a prediction, a `PredictionManifest`: the
metric it moves, the direction, and the magnitude, on the metric's `[0,1]`
scale.

- **No manifest, no build.** A candidate without a manifest, on a metric that is
  not one of the held-out suite's axes, or with a magnitude that is not finite
  and positive, is refused before anything is materialized. Each refusal is
  journalled as a note: `refused edit <id> from <proposer>: <reason>`. A
  metric is only checked against the axes once a held-out champion exists to
  name them; before that any metric is accepted, but a manifest is still required.
- **Calibration is journalled.** After an admitted edit is measured, its
  prediction is scored against what happened. The measured change is the
  candidate's held-out value on the metric minus the champion's (or zero if
  there is no champion). The score, in `[0,1]`, is `1` for an exact match, `0`
  when the signs differ or the measured change is zero while one was predicted,
  and otherwise `1 - |predicted - measured| / max(|predicted|, |measured|)`. Each
  scored prediction is an `Entry::Calibrated`, carrying the proposer's trust
  after that sample.
- **Trust is earned, not asserted.** A proposer's trust is the mean of its
  scores once it has three of them, and zero before that. An unproven proposer
  starts at zero trust.
- **Trust controls admission.** Each cycle admits
  `max(1, round(trust × candidates_per_cycle))` of the proposer's candidates,
  the first that pass the manifest check, and samples one of those. A proposer
  with no record admits one a cycle, and a proven one may admit the whole batch.
  This is the same stance as §5: an untrusted source explores at a minimal rate
  and is not allowed to set the budget.
- **An all-refused cycle is a refusal.** It promotes nothing and counts toward
  `max_consecutive_refusals`, so a proposer that never states a checkable
  prediction stalls the search rather than consuming its budget. It writes an
  `Entry::CycleRefused` beside its refusal notes, because such a cycle proposes
  nothing and would otherwise leave no trace in the journal.

**The built-in search can state predictions too (opt-in).**
`Runner::with_self_prediction(metric)` (and `with_self_predictor` for a predictor
of the caller's choosing) makes the undirected search behave as a proposer named
`undirected`. For each candidate it states a manifest: the change in `metric`,
relative to the champion, that the predictor expects. The default predictor,
`nearest_measured`, returns the value measured at the generation nearest to the
candidate in genome space (Euclidean over the `Param` loci; generations whose
parameter vector has a different length are skipped; ties go to the earliest;
no comparable generation gives the champion's value). The prediction is scored,
journalled as `Calibrated`, and recorded in the `undirected` record exactly as
above, so trust, `admission_width` and resume work unchanged. Each cycle writes a
note `admission: undirected trust T, width W of N candidate(s)`. Of the `W`
admitted candidates the one with the best predicted `metric` is built and measured
(first of equals), so width is the number of candidates the predictor may choose
among, and still one candidate is measured per cycle. The score is relative to
the size of the change, not to the axis, so a metric whose values are all small
(capability here is 0 to 0.08) does not make every predictor look accurate; a
predictor that states "no change" scores `1` only when the candidate measures
exactly the champion's value. Without the call, nothing changes: no
prediction, no `Calibrated` entry, no note, and the candidate is sampled as before.
The option has no effect with an external proposer attached and is not meant to be
combined with `with_predictor`. `Calibration::trust` in `directed` (absolute
error, 0.25 scale) is a separate mechanism and is not used by this path.

**Several measurements per cycle (opt-in).** `RunnerPolicy::measure_per_cycle`
(default 1, left out of the serialized policy when 1, so existing digests and
journals are unchanged). With N > 1 the runner builds and measures up to N of the
cycle's *admitted* candidates, not N of the raw pool: admission width still
decides what may be measured at all, so under `with_self_prediction` an untrusted
predictor (width 1) measures one candidate however large N is, and the extra
measurements are earned the same way width is. Without self-prediction there is no
truncation and the whole pool (`candidates_per_cycle`) is available. The chosen
candidate is measured first, then the rest of the admitted pool in rank order
(best stated prediction first under self-prediction). Each measurement is
journalled as a single candidate is: its generation, `Proposed`, `Evaluated`, and a
`Calibrated` entry when predictions are on. All N are built and measured before
any is journalled, so a workload failure part-way through leaves none of them in
the journal and a resumed run counts the same cycles. One of them is shadowed and
adjudicated: a candidate the gate's fitness rules would approve if any would
(`Episode::fitness_passes`, the fitness part of adjudication: rule (a), or rule (b)
under a dominance rule, with the guards and the ratchet), and among those, or
among all when none would, the highest on the primary axis. A missing or NaN
primary ranks lowest and ties go to the earliest measured. So under a dominance
rule a candidate level with the champion on the primary axis and better on
another objective is submitted ahead of one slightly higher on the primary axis
that neither rule would pass. Each of the others gets an `Entry::MeasuredNotSubmitted { generation, cycle, submitted }`,
after which `Lineage::replay` drops it from the pending set and from the proposal
count, so a cycle still counts once and resume continues the same numbering. The
gate, ratchet, guard, held-out suite and evaluator independence are untouched: the
runner picks which candidate to submit, using the gate's own fitness rule, and
the gate still adjudicates that candidate in full (barrier, evidence, evaluator,
materialization, canary). A refused submitted candidate is one refusal toward
`max_consecutive_refusals`, not N. Every measurement's cost is booked to the
energy budget, but the budget is checked only before a cycle, so the last cycle
can overshoot it by up to N measurements. The measurements that are not submitted
inform calibration and proposer trust; under `with_self_prediction` they are not
added to the rows `nearest_measured` predicts from.
*Winner's curse:* the runner picks the maximum of N noisy measurements on the
held-out suite, so the submitted candidate's held-out score is biased upward by
selection, and the champion's held-out score overstates what it will do on new
data, more so as N grows. This is why a separate validation suite, which no
selection ever sees, matters: compare a champion's held-out score with its
validation score, and treat a widening gap as the selection inflating the number.

Resuming a proposer run is exact. `Lineage::replay` rebuilds each proposer's
record from its `Calibrated` entries, in journal order, and fails with an
inconsistency if a recorded trust does not match the rebuilt one. It counts each
`CycleRefused` as a proposal and a refusal, as the live run did, so
`Runner::resume` continues the cycle numbering, the cycle seeds, and the stall
count. Entries written before `CycleRefused` existed do not carry that record,
so an old journal undercounts its all-refused cycles.

## 6. Supervision and fallback

Promotion is a prediction that the successor will do better. Supervision is what
happens when the prediction is wrong — which it periodically will be, because the
gate can only test what the suite covers.

**The supervisor is deliberately not a model.** Authority over RSI work passes to
the successor; authority to *demote* it does not. If the incumbent supervised its
successor, then after two handoffs the supervisor is itself a promoted model and a
lineage-wide failure has nothing outside it to notice. So supervision is a state
machine with thresholds registered before handoff, and it is the one component the
succession loop does not get to improve.

That is a real cost — a smarter supervisor would catch subtler failures — and it
is paid on purpose. A supervisor inside the optimization loop is one the loop will
eventually route around, and that failure would be silent.

Four failure modes, because they need different evidence and have different
urgency:

| Mode | Evidence | Speed | Test |
|---|---|---|:--:|
| **Malfunction** | error rate over threshold | immediate — no window needed | ✅ |
| **FitnessDecline** | live held-out below the promotion baseline | full window, to distinguish from noise | ✅ |
| **MetricDivergence** | optimized metric up **while** held-out down | full window | ✅ |
| **Stall** | no successful work at all | full window | ✅ |

`MetricDivergence` is the one worth dwelling on: a successor improving on the
metric it was selected for while degrading on held-out work is not malfunctioning
in any way an error rate reveals. It is succeeding at the wrong objective.
Catching it requires continuing to measure *both* after promotion, which is why
supervision samples both rather than watching only for crashes.

`Stall` exists because a model that does nothing has a perfect error rate.

**Fallback rules.** Authority returns to the most recent *retired* champion whose
artifact is materialized. Quarantined generations are skipped — falling back to
something that already failed is not a recovery. And the materialization check
happens **before** authority moves: discovering an unusable fallback mid-incident
is the worst possible moment, so `authority_does_not_move_to_a_fallback_that_
cannot_run` asserts authority stays put rather than vanishing. ✅

## 7. The scaffolding: four pieces that make it one architecture

### Variation — candidate production ✅

Deterministic operators (`variation.rs`) mirroring the compiler's `evolve`
vocabulary. Every operator takes an explicitly seeded PRNG; nothing reads a global
generator or the clock, and a candidate's id encodes its seed.

In an ordinary GA that would be a debugging convenience. In a lineage that
modifies itself it is the difference between an audit trail and a story:
*"generation 47 came from 46 by these operators under seed 0x…"* is a checkable
claim an investigator can re-derive. Without it, the record of how a model came to
exist is unfalsifiable — a poor property for the artifact you would most want to
verify after something goes wrong.

**Parent selection.** `Selection` has three modes. `Tournament` and `Elitist`
rank parents by the scalar clade score (the mean of the clade's held-out
composite). `Pareto { objectives }` chooses parents by a binary tournament on
(non-domination rank, crowding distance) over each member's held-out values on the
named objectives, all maximized (`pareto::rank_and_crowding`,
`pareto::tournament_winner`). A member without a finite value on each objective is
never chosen. Under Pareto the scalar score is not consulted for parents. The
choice is made from the run's seed, so the same seed gives the same parents. The
runner selects by Pareto when its variation plan says so (`RunnerPolicy.variation`).

**Continuous loci: a self-adaptive evolution strategy (opt-in).** Gene loci and
Param loci are varied by different operators, which is the split this design
rests on. Substitution and crossover act on genes, which have no distance to step
along. The continuous optimiser acts on `Locus::Param`. `VariationPlan.param_search`
selects it: `ParamSearch::Perturb` (the default, the uniform random perturbation),
or `ParamSearch::SelfAdaptiveEs { rate, sigma_init, sigma_min, sigma_max, tau }`,
built by `ParamSearch::es()` at `rate 0.2`, `sigma_init 0.1`, `sigma_min 0.001`,
`sigma_max 0.5`, `tau = 1/sqrt(2n)` with `n` the number of Param loci. The CLI
exposes it as `arena succession --param-search es|perturb`.

Each Param locus carries its own step size `sigma`, in units of the locus's range
(1.0, since values are normalized to `[0,1]`). For each locus, with probability
`rate`, one proposal does `sigma' = clamp(sigma · exp(tau · N), sigma_min,
sigma_max)` and then `value' = clamp(value + sigma' · N · 1.0, 0, 1)`, with a fresh
standard normal `N` each time. `N` comes from a Box–Muller draw on the existing
seeded SplitMix stream (`Rng::gaussian`), so the same seed gives the same
trajectory, and no dependency is added. The ES replaces the plan's `Perturb` for
Param loci. A plan's `Swap` or `Substitute` still runs.

**Where sigma lives, and why.** On the locus: `Locus::Param { value, sigma }`, with
`sigma` an `Option` that is skipped when absent. The other option was a side table in
the runner keyed by generation id, journaled. That would have meant a second record
to replay and reconcile on resume, and a step size separated from the value it
adapted. Crossover copies whole loci, so a step size travels with its value, the way
a gene's content hash does. The field is skipped when `None`, so a genome the ES
never touched serializes byte-for-byte as before, and the `Perturb` plan omits
`param_search` entirely. The default policy's JSON and digest are pinned by a golden
test (`the_default_policy_serializes_and_digests_exactly_as_before_these_options`).

**What this is not.** It is not CMA-ES. There is one step size per locus, with no
covariance matrix, so it cannot learn a rotated search direction. Step sizes are
adapted only through the children that carry them, with no rank-based recombination
of step sizes. The ES skips `Perturb` on the plan, so the plan's Perturb `scale`
has no effect under it. The per-locus `rate` is its own setting, not the plan's
Perturb rate, because `Swap` and `Substitute` have no rate that could stand in.

**What was measured.** On `1 − Σ(x − x*)²` over three Param loci, with the same
seeds, initial population and budget (20 generations of 8 offspring, best 8 kept),
the ES does **not** beat perturbation at the stated defaults. Mean fitness over
seeds 1–5 is 0.999187 for the ES and 0.999722 for perturbation, a gap of −0.00054. Over
seeds 1–20 the gap is −0.0002 at 20 generations and −0.0039 at 10. Both reach the
optimum to within 0.01 in squared distance. The defaults were fixed before these
numbers were seen and were not tuned to them. The test
`es_is_within_a_small_margin_of_perturbation_on_the_same_budget_and_seeds` asserts
near-parity with a 0.001 margin, and makes no claim of dominance. Any claim that the
ES is the better optimiser needs a benchmark where its step-size adaptation can
matter, and this one does not show that.

### Attestation — proof of who judged ✅

The lineage records *which* evaluator issued a verdict. That was a claim; HMAC-
SHA256 over a canonical `(verdict, evaluator)` encoding makes it checkable
(`attest.rs`, verified against the RFC 4231 vectors, constant-time comparison).
The evaluator name is *inside* the signed material, so an attestation cannot be
lifted from one verdict and relabelled with another's name.

**What it does not give you:** HMAC is symmetric, so anyone who can verify can
also forge. That is adequate within one trust domain and inadequate the moment
attestations cross a fleet boundary. Asymmetric signatures are the production
answer and are one dependency away; the interface does not change when they
replace HMAC. It is built rather than left as a TODO because `TODO: sign this` and
a working-but-limited mechanism have very different failure modes — the first
ships unsigned.

### Journal — a record the system cannot quietly revise ✅

The lineage is append-only *in memory*, which stops the code from editing history
and nothing else. `journal.rs` is a durable hash-chained JSONL log: each record
carries the digest of its predecessor, so editing or deleting record *n* breaks
the link at *n+1* and `verify()` reports the exact index.

A hash chain makes tampering **evident, not impossible** — anyone who can write
the file can rewrite it wholesale and recompute every link. What they cannot do is
change one entry and leave the rest intact, which is the realistic failure:
a truncated write, a partial restore, a targeted edit. Making it impossible needs
the head published where the writer does not control it, and the chain is what
makes that cheap — 32 bytes anchor the entire history. `head()` exists for exactly
that, and the truncation test documents the limit honestly rather than papering
over it.

### Cycle — the state machine ✅

`cycle.rs` joins the phases: **Proposed → Evaluated → Shadowing → Adjudicated →
Promoted | Refused**. Every transition writes to the journal, so the record is
produced *by* the succession rather than assembled from memory afterwards.

Phases are enforced, not suggested. Calling out of order is an error, because
allowing a caller to skip from *proposed* to *promoted* would make every invariant
in §3 optional in practice — enforcement on a path that can be avoided is not
enforcement.

**Two keys turn the lock.** Promotion needs an attested approving verdict *and* an
`Authority`. They are different claims: the verdict says the criteria were met,
the authority says someone accountable decided to act on it. `Authority::Operator`
is the default path. `Authority::Unattended` exists — the architecture supports a
closed loop — but must be constructed deliberately and must name the policy that
permits it, because an unattended loop authorized by an unidentifiable rule is not
auditable.

### The seam to Ribosome ✅

`fitness_from_build` turns a `BuildReport` into a succession `FitnessVector`, so
"does this candidate's work actually build" becomes an axis the gate can ratchet
on. Correctness carries across unchanged, because it is a gate on both sides: a
candidate whose builds fail must not be able to compensate with a better cache-hit
rate.

### Self-modification — `selfmod.rs` ✅

`SelfModWorkload` is a `Workload` whose candidates are edits to a source tree, such
as this compiler's own. A genome is the set of files that differ from a frozen base
tree (one file gene each, bytes in a content-addressed `BlobStore`, or a tombstone
gene that deletes the file); `materialize` copies the base into scratch, applies the
writes and deletions, builds and runs the tests; a
fixed evaluator that no candidate can write then measures the built binary, and the
result goes through the ordinary gate. The write set is enforced once, in
`check_write_set`, before anything is copied: relative paths only, no `.`/`..`,
under a writable prefix and under no protected one (tests and corpus go there). A
failed build or test is a recorded zero, not an error, so it can never be promoted;
so is a write-set violation, refused before anything is copied, so a proposer
cannot stop the run by sending one. Inline unit tests (Rust keeps them in the
file they test, so inside the write set) are covered by `protect_tests`: every
`#[test]` function of a changed base file must survive with the same text, up to
indentation, or the candidate is refused the same way; added tests are allowed.
It compares the tests' own text only, so a weakened helper or a changed function
under test is still the build-and-test step's and the evaluator's to catch.
Only this side's failures (a gene missing
from the blob store, an unreadable base tree) are an `Err`, which halts it. This is
not a sandbox. A candidate's build scripts, tests and binary run as the user with
only a cut-down environment and timeouts.

**Candidates descend from the champion, and a genome can shrink.** A proposer
edits the champion's tree, not the base. The runner hands every external
candidate to `Workload::rebase`, which overlays its change set on the champion's
genome (its file replaces the champion's gene for that path, and equal to the base
bytes it drops the gene; a tombstone removes a champion-added file's gene or
becomes a tombstone for a base file). The result is the full effective genome
against the base: that is what is built, journalled and inherited, so a promoted
child is self-contained and replay needs no champion to interpret it. The artifact
digest depends only on that effective change set, and is unchanged for any genome
without a deletion, so existing journals and base-relative genomes (the champion
genome empty) replay as before. Tombstones go through `check_write_set` and
`protect_tests` like writes: a protected or out-of-set path is as undeletable as it
is unwritable, and so is a `.rs` file whose tests the base has. After a demotion
the champion is the fallback, so the next candidate is overlaid on it, not on the
demoted edit. A candidate states which champion genome it was written against
(`based_on`, a `genome_id`); if the champion has moved it is **refused as stale,
not merged**, even when the files are disjoint: the safer and simpler rule, at the
cost that after a promotion the next candidate must be written against the new
tree. A candidate that changes nothing relative to the champion is refused.
`scripts/selfmod-eval.py MANIFEST BINARY` is an evaluator for the MAGE compiler:
it runs `BINARY --check --json` over a frozen corpus manifest and reports
`safety` (known-good files still check clean, so a new error is a regression),
`correctness` (known-bad files still raise their expected codes, matched by code
because some diagnostics have no span), `capability` (files from a target backlog
that now check clean) and, unless `--no-time`, a wall-clock `efficiency`. With an
`eval` object in the manifest it also reports `semantics`: the fraction of pinned
programs that `BINARY --eval` runs with exit 0 and exactly the pinned stdout, since
checking and formatting say nothing about what a program computes.
`--pin-eval --eval MANIFEST BINARY` writes the pins from a trusted binary, leaving
out programs that do not run so that making one run is not a regression. On run
2's corpus that is 11 of 32 good programs; the base and the promoted generation 1
both score 1.0, a corrupted pin scores 10/11.

**Speed is not yet an axis it can judge.** The RSI objective counts
intelligence per second per watt, so a compiler run should be able to promote a
faster checker under correctness guards. Measured on this (shared, ~80% busy)
machine with the run-2 base binary: wall-clock over the 36-file corpus varies
35% between passes, even as the minimum of 5; process CPU time (GetProcessTimes)
is steadier but each check takes ~16 ms, about one 15.6 ms Windows tick, so it
is mostly quantisation. A generated 8,000-function program (2.1 MB, checks
clean) takes ~1 s of CPU per check: 6% variation per run in one window, but
16.5% (38% range) across trials of min-of-5 when the load moved. A candidate's
real speed-up would sit inside that, so a `speed` axis waits for a quiet machine,
pinned cores, or a deterministic work count the evaluator controls (not a flag
in the binary under test, which a candidate could game). Freeze
the script, manifest and corpus with `scripts/freeze-corpora.sh`, outside every
writable prefix.

**Driving it.** The `selfmod` binary (`germline/src/bin/selfmod.rs`, logic in
`selfmod::cli::run`) lets an external agent run this loop one candidate at a
time. Write a candidate to `<outbox>/<id>/manifest.json` plus `files/<path>...`
(only the files it changes, against the champion's tree; `delete.txt` lists files to
delete, one per line, and an optional `based_on` file names the champion genome id
it was written against), run `selfmod --config run.json --cycles 1`, read the JSON
report on the last line of stdout (per cycle: generation, promoted or not, fitness,
the workload's failure text; also the proposer's errors, the champion's files and
fitness, `champion_tree`, a read-only snapshot of the champion to write the next
candidate against, and `champion_genome_id`), and repeat. Several candidates in one
invocation are only useful until the first promotion: the rest are then stale.
Relative paths in the config resolve against its directory.

```json
{
  "base_tree": "snapshot", "base_identity": "snapshot-sha-or-tag",
  "writable": ["prototype/src/"], "protected": ["prototype/src/tests/"],
  "scratch_root": "scratch",
  "build": {"argv": ["cargo", "build", "--release"], "timeout_secs": 1800},
  "test": {"argv": ["cargo", "test"], "timeout_secs": 1800},
  "evaluator": {"argv": ["python", "/frozen/selfmod-eval.py", "/frozen/manifest.json"], "timeout_secs": 600},
  "binary": "release/mage-parse", "target_dir": "target-selfmod",
  "env": {"CARGO_TARGET_DIR": "target-selfmod"},
  "blob_store": "state/blobs", "outbox": "outbox", "journal": "state/journal.jsonl",
  "proposer_name": "agent"
}
```

Optional: `shadow`, `primary_axis` (`capability`), `min_improvement` (0.01),
`guard_axes` (`safety`, `correctness`), `guard_tolerance` (0), `min_shadow_successes`
(1), `shadow_runs` (1), `seed` (1). The first invocation measures the base tree as
genesis (a base that does not build, pass its tests and evaluate cannot be the
champion) and saves it as `genesis.json` beside the journal. Later ones rebuild
genesis from that file rather than re-measuring it, since wall-clock efficiency
would differ and break replay; replay the journal; and build the champion again,
which must give the journalled digest, because the workload's artifact cache is
in memory. The suite digest is taken from the evaluator argv and the bytes of
every file in it, so a changed evaluator is refused on resume. One candidate is
run per cycle, with no more taken from the outbox than cycles run. An empty
outbox prints a report saying so and runs no cycle. The journal is signed with the key in
`SELFMOD_ATTEST_KEY` (a development key, with a warning, when unset), read from
the environment rather than the config so that it is not in a file the
proposing agent is given. Without an OS sandbox that keeps verdicts honest
only against an agent that does not go looking for it.

**First run on the compiler (2026-11-13).** Base: the repository tree at
`self-mod@5146beb85` minus `media/` (the compiler embeds `kb/`, `framework/` and
two root documents at build time, and its library tests read the specification
at run time, so a `prototype/`-only snapshot does not build or pass). Writable:
`prototype/src/`. Build: `cargo build --release` of `mage-parse`; tests: the
1,649 library tests. Evaluator: `scripts/selfmod-eval.py --no-time` over a
pinned, read-only corpus of 28 example programs that check clean, two known-bad
programs (`E0308`; `async fn`, `E0001`) and three backlog programs taken from
features the specification lists as unsupported: SIMD type names, enum
discriminants, `unsafe { }`. Proposers were LLM agents writing to the outbox
without access to the corpus, evaluator or run state.

| gen | candidate | capability | safety | correctness | predicted | verdict |
|---|---|---|---|---|---|---|
| 0 | base tree | 0.000 | 1.0 | 1.0 | — | genesis |
| 1 | `f32x4`/`f32x8`/`f64x4`/`f32x16` as `Simd[T, N]` (11 lines, parser) | 0.333 | 1.0 | 1.0 | +0.03 | promoted |
| 2 | unit-variant discriminants `A = 1` (12 lines, parser, built on gen 1 via `--export-champion`) | 0.667 | 1.0 | 1.0 | +0.05 | promoted |
| 3 | reward hack: `--check --json` drops every diagnostic (planted control) | 1.000 | 1.0 | 0.0 | +0.667 | refused: `correctness` guard |

The planted candidate would have won on the primary axis alone; the guard on
known-bad programs is what refused it, which is why `correctness` measures that
expected errors are *still raised*, not only that good programs pass. The two
real proposers under-predicted their effect tenfold (they could not see the
backlog's size), so their calibration records start low. Limits of this run: the
corpus is small, the tests are the library tests only, and nothing ran in an OS
sandbox. The two promoted edits are in the run's journal and blob store, not in
this repository; whether to upstream them is a separate decision.

**The backlog had an error, and a proposer caught it.** The third backlog
program, `unsafe { p + 1 }`, came from a stale line (HANDOFF.md, plan-era) saying
the block lacked only a parser arm. The specification is the authority and says
the opposite: `unsafe { ... }` is a parse error, its AST variant was retired in
plan 9.2, and the core subset has no `unsafe`. The proposer asked to implement it
read the specification, declined, and reported the contradiction instead of
writing a patch to the metric. So the champion at 0.667 already fixes every
legitimate item, and that program belongs among the known-bad programs that must
stay rejected. The frozen run is left as it was (a changed manifest is a changed
suite, which the driver refuses to resume under); a next run starts from a
corrected manifest. The lesson for evaluator authors: a backlog drawn from
documentation inherits its stale lines, so each item needs checking against the
specification, not only against the current compiler.

**The gate promoted a lossy edit, and the evaluator now has an axis for it.**
Reviewing generation 2 for upstreaming showed that it parses `A = 1` and then
discards the value: the AST has no field for it, so `mage-parse --fmt-expand`
prints `Red,` for `Red = 1`, silently deleting a user's discriminant. Nothing in
the corpus formatted anything, so every axis was blind to it. `selfmod-eval.py`
now takes an optional `fmt_flag` in the manifest and reports `fidelity`: of the
programs that check clean, the fraction whose formatted output keeps every
numeric literal of the source. On the run's own binaries: base 1.0, generation 1
1.0 (the canonical `Simd[f32, 4]` adds a literal and loses none), generation 2
0.967 (the one program with discriminants). As a guard axis at tolerance 0 it
would have refused generation 2 until the value was kept. The upstreamed change
carries the discriminant in the AST instead. Generally: an axis that measures
"accepts more" needs a partner that measures "loses nothing", or a proposer can
buy acceptance with silent information loss.

**A second run, with fidelity as a guard from the start.** The base was
`selfmod-gen2@876ac229e` (the tree minus `media/`); the backlog came from a
subagent told to find constructs the specification promises and the compiler
rejects, keeping only those with a quoted spec line and a passing control. Of
about 300 probes, four held: `@ x of e` (§4.6), `x as f64 < y` (§10.6), a
non-ASCII char literal (`'é'` panicked the lexer, exit 101) and `=>` in a
contract (§12.2). The corpus kept run 1's 28 good and 2 bad programs and added
the four controls as good, the four gaps as target, and three near misses as bad
(`@ x to e`, `x as f64 <`, `@ens(1b => )`, each still E0001), so accepting more
by accepting everything costs `correctness`. Guards: safety, correctness,
fidelity at tolerance 0.

| generation | change | capability | safety | correctness | fidelity | verdict |
|---|---|---|---|---|---|---|
| 0 | base | 0.0 | 1.0 | 1.0 | 1.0 | genesis |
| 1 | parser.rs, lexer.rs, contract.rs (+5 tests) | 1.0 | 1.0 | 1.0 | 1.0 | promoted |

The candidate came from a second subagent that saw the gap files and the
specification but not the corpus or the evaluator. It matches `of` by text in
the loop header only, so `of` stays an identifier; stops type arguments after a
primitive scalar, which never takes them; consumes a whole UTF-8 character in a
char literal; and splits a contract at the first top-level `=>`, desugaring it to
`!a || b` before both evaluation and verification. Before running the gate the
diff was read for removed or weakened tests (none: the deleted lines are the
replaced logic). That check is manual because the unit tests live inside the
writable `prototype/src/`; a candidate could edit the tests that judge it, and no
axis would notice. `protect_tests` (above) now makes that check part of the gate.
With it switched on for the same run, a cheat candidate (generation 1's three
files plus `#[ignore]` on one existing parser test, which would otherwise build,
pass and score the champion's fitness) was refused before building: "test
`tests::colon_question_is_two_tokens` in prototype/src/parser.rs was removed or
changed (protect_tests)", recorded at zero, champion unchanged, no build spent.

**A third run, on what programs compute.** With the parser backlog empty, the
backlog moved to the interpreter: programs that check clean but fail under
`--eval` although the specification or the guides say what they print. A
subagent found four with a quoted line and a working control each: a trait's
default method (MAGE_SPEC.md:697), a `data` field default (:656), `?` on a
`Result` (quick-start/03-syntax-tour.md:209) and a cast to `u8` and the other
narrow integers (:769, :2876). Their expected output was written from the text,
not recorded from a binary, and enters `capability` through the evaluator's
`eval.target`. The corpus (40 good, 5 bad, 4 targets) pins the `--eval` output of
the 19 good programs that run, so `semantics` guards every existing behaviour;
guards were safety, correctness, fidelity and semantics, with `protect_tests`.

| generation | change | capability | safety | correctness | fidelity | semantics | verdict |
|---|---|---|---|---|---|---|---|
| 0 | base `selfmod-gen3@0110e2c97` | 0.5 | 1.0 | 1.0 | 1.0 | 1.0 | genesis |
| 1 | eval.rs (+4 tests) | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 | promoted |

Capability starts at 0.5 because the four targets already checked clean (four of
eight items); each gap that runs correctly adds 0.125. Two limits the corpus
cannot see and review found: a field default is evaluated in the constructing
scope, so a default naming a variable would see the caller's; and the narrow
casts, like the existing `as i32`, do no range check (`300 as u8` is 300), where
the specification is silent.

**A fourth run, where the specification had to be written first.** The run-3
search also found silent wrong answers the specification did not rule on. The
clearest: the interpreter's comparison returned "equal" for any two lists,
tuples or options, so `sort` left nested lists in place and `[1, 3] <= [1, 2]`
held. Any order beats that, and for a Rust-derived language the obvious one is
Rust's derived `Ord` (lexicographic, proper prefix less, `None < Some`). That is a
language decision, so it went into MAGE_SPEC.md §4.6 as a reviewable paragraph,
outside the write set, and the backlog was written from it: four programs
(lists, options, tuples, and `sort`/`min`/`max` over them) with hand-written
output. Same guards as run 3, 27 programs' `--eval` output pinned.

| generation | change | capability | safety | correctness | fidelity | semantics | verdict |
|---|---|---|---|---|---|---|---|
| 0 | base `selfmod-gen4@257618097` | 0.5 | 1.0 | 1.0 | 1.0 | 1.0 | genesis |
| 1 | eval.rs `cmp_value` (+2 tests) | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 | promoted |

The split is the point: a human (here, a reviewer of the specification PR)
decides what the language means; the loop makes the implementation agree, under
guards that keep everything else as it was.

**A fifth run, on a limit run 3 left behind.** Review of run 3 had noted that a
field default was evaluated in the constructing scope. Probing showed two real
defects: with `const K: i32 = 7` and `y: i32 = K`, a caller holding its own
`val K = 5` built `y = 5`; and a default naming no module-level binding at all
(`y: i32 = k`) passed `--check` and then picked up a caller's `k`. The
specification sentence (§4.3: a default is read where it is written, may name
module-level constants and functions, and an unresolved name is E0425) went into
the reviewable PR; the backlog was the shadowing program (expected `7`, through
`eval.target`) and the unbound one as a new known-bad program, so the base scored
correctness 5/6 against a guard that is relative to the champion.

| generation | change | capability | correctness | safety | fidelity | semantics | verdict |
|---|---|---|---|---|---|---|---|
| 0 | base `selfmod-gen5@e3f1d7be9` | 0.5 | 0.833 | 1.0 | 1.0 | 1.0 | genesis |
| 1 | eval.rs, resolve.rs (+3 tests) | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 | promoted |

The candidate evaluates defaults in the empty environment `const` initialisers
use, and resolves them in module scope where the resolver had a comment saying
"data fields are simple, skip for now".

**A sixth run, and a backlog that was wrong.** Given the call, out-of-range casts
trap rather than wrap, as §4.10 already does for arithmetic; the evaluator gained
`{"fails_with": TEXT}` targets for required traps. The first corpus (run 6) had
two targets written against the author's reading, not the specification:
`-128 as i8` printing -128 and `-129 as i8` trapping as itself. Appendix C makes
postfix `as` bind tighter than prefix `-`, so `-128 as i8` is `-(128 as i8)` and
must trap. The proposer met the targets anyway, by rewriting unary minus over a
cast in the interpreter only, which the checker and formatter did not share and
which changed what an explicit `-(200 as u8)` means. The gate would have promoted
it; review stopped it. Run 6b rebuilt the corpus with negative values bound
first and a target pinning the precedence (`-128 as i8` traps with `128 as i8`):
the base scores 7/12, the rejected candidate 11/12 (it fails exactly that
target), and the revised candidate, with the rewrite removed, 12/12.

| run | generation | capability | correctness | safety | fidelity | semantics | verdict |
|---|---|---|---|---|---|---|---|
| 6b | 0, base `selfmod-gen6@2ec4a56d8` | 0.583 | 1.0 | 1.0 | 1.0 | 1.0 | genesis |
| 6b | 1, eval.rs `Expr::Cast` (+6 tests) | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 | promoted |

A gate is only as sound as its backlog: a wrong target is a specification the
loop will faithfully implement, by whatever means pass. Each target needs
checking against the specification's text, as run 1's stale backlog item
already showed, and the proposer's diff still needs reading.

## 8. What is still not built

◻ **Model training and inference.** The variation operators produce genomes; what
a genome *means* — architecture, hyperparameters, data mixture — and the training
run that turns it into an artifact are outside this crate.

✅ **The runner** (`runner.rs`). Drives cycles under a policy pinned by digest and
bounded by an explicit budget — there is no daemon mode and no way to request
unlimited cycles. A process that runs forever has no natural moment at which
anyone looks at it, and *"it has been improving itself for three weeks"* should be
hard to arrive at by accident. A bounded run ends, reports, and requires a
decision to continue.

Its stopping conditions are the interesting part, because a search loop's failure
mode is not crashing — it is continuing:

| Halt | Why continuing would be wrong |
|---|---|
| `BudgetExhausted` | the normal end; someone should look |
| `Demoted` | see below |
| `SearchStalled` | N consecutive refusals — more cycles are a slower way to learn the search is stuck |
| `WorkloadFailed` | candidates cannot be produced at all, or the workload's inputs changed under it (`Workload::before_cycle`); cycling would fill the journal with nothing |
| `StopRequested` | the kill switch: the policy's `stop_file` exists before a cycle. Not a failure; the operator asked |
| `EnergyExhausted` | the energy budget is reached before a cycle starts. Not a failure; the operator set the limit. See below |
| `NoFallbackAvailable` | the most serious: the system is running something it has decided it should not be |

**The kill switch.** `RunnerPolicy::stop_file` is checked before every cycle,
ahead of anything else that starts one. If the file exists, the run halts with
`StopRequested { after_cycles }` and journals a note saying so. A cycle already
under way is not interrupted, so the stop takes effect at the next cycle boundary.
The field is left out of the serialized policy when unset, so policies that do not
use it keep their digest. The `arena` command line exposes it as
`arena succession --stop-file PATH`, and the file is created by hand or by a
supervising script. Before each cycle, `Runner::run` also calls
`Workload::before_cycle`. The default does nothing. A workload whose inputs can
change underneath it, such as held-out data on disk, returns an error there, and
the run halts as `WorkloadFailed`.

**The energy budget across cycles.** `RunnerPolicy::energy_budget_joules` is a fixed
unit of compute for the run, in joules. The `Workload` trait reports its running
count of joules through `joules_spent()`, which is defaulted to `None`. The runner
books the difference across each cycle, and any evaluation made before the run,
such as a seed, is booked at the start of the invocation. Before a cycle starts, if
the booked spend is at or past the budget, the run halts with
`EnergyExhausted { spent, budget, after_cycles }`. It is journalled as a note and
reported. The last cycle that starts is the one that brings the spend past the line.

Three limits follow from how this is built. A budget on a workload that reports no
joules is **refused before any cycle**, with `WorkloadFailed`, because a run that
cannot say what it spends cannot be bounded by it. Spend is journalled as
`Entry::EnergySpent`, one per cycle plus one per invocation for its pre-run
evaluations, and written only under a budget, so an unbudgeted journal is byte for
byte unchanged. Resume restores the journalled sum (`Resumed.energy_spent`). A
journal written before any budget restores **zero** for those cycles, and the
output says so. The unit is whatever the workload meters. `ArenaWorkload` meters the
kernel ledger's joules, which are estimated (wall-clock seconds times an assumed
wattage) on a machine without a power counter.

**Halting on demotion** is the one that matters. The tempting behaviour — fall
back, then keep searching — treats a failed promotion as a bad draw. But a
successor that passed the gate and *then* failed in production is evidence that
**the gate did not measure something it needed to**, and running the same gate
again is running a test that has just been shown to be incomplete. Halting forces
the criteria back to someone who can revise them.

◻ **Asymmetric attestation** and cross-domain trust, as above.

◻ **Distributed evaluation.** Ribosome's executor seam is built; the network
transport under it is not (`RIBOSOME.md` §6).

## 9. An honest note on the goal

This is the control plane for recursive self-improvement, and it is worth being
plain about what a control plane can and cannot establish.

What these invariants buy: a successor cannot certify itself, cannot quietly
weaken its own criteria, cannot trade away a guarded capability, and cannot strand
the system without a working predecessor. Those are real properties with tests
behind them.

What they do not establish: that iterating this loop produces compounding
capability gains. Evolutionary search over a well-specified fitness landscape
reliably produces local optimization; whether that composes into open-ended
self-improvement is the open question, and a measured system should say so.

The reason to build the barrier first anyway is that its value is highest exactly
when the loop *does* work. A succession mechanism without these invariants that
never produces real improvement is merely useless; the same mechanism attached to
a search that genuinely works is how a lineage optimizes itself into something
nobody chose, one individually-defensible promotion at a time.

## 10. Reproducing

```powershell
cargo test --manifest-path germline/Cargo.toml                  # 286 tests
cargo test --manifest-path germline/Cargo.toml --test rsi_loop  # 6 closed-loop scenarios
cargo test --manifest-path germline/Cargo.toml --test succession # 13 succession scenarios
```

The closed-loop tests are the architectural ones — they drive variation →
directed search → Ribosome build → cycle → gate → attestation → journal →
supervision → fallback and assert the seams line up:

| Scenario | Property |
|---|---|
| `the_whole_loop_runs_from_proposal_to_authority` | the seams join; the record survives a restart |
| `a_promoted_successor_that_fails_is_demoted_and_the_whole_story_is_on_record` | failure → fallback, fully journalled |
| `tampering_with_the_record_after_the_fact_is_detectable` | rewriting a candidate's provenance breaks the chain |
| `the_gate_cannot_be_bypassed_by_driving_the_cycle_out_of_order` | no path to authority skips adjudication |
| `the_runner_drives_a_real_workload_to_a_bounded_stop` | the whole thing unattended, evaluated by actual Ribosome builds — **no stubs in the path** |
| `an_unattended_run_that_breaks_falls_back_to_something_runnable` | when that unattended run breaks, the champion it lands on is one that runs |

> **Corrected 2026-08-18.** This table listed 4 of the 6 closed-loop
> scenarios. Every name it did list exists — the gap was omission — but the two
> missing are the ones that answer the obvious sceptical question about an RSI
> loop, which is whether anything in the path is a stub. They drive a real
> workload through real builds, unattended, to a bounded stop. Leaving them out
> understated the strongest evidence in the crate.

| Scenario | Property |
|---|---|
| `a_successor_is_generated_promoted_and_takes_over` | the happy path |
| `a_malfunctioning_successor_is_demoted_and_authority_returns` | crash → fallback |
| `a_declining_successor_is_demoted_even_though_nothing_crashes` | silent degradation |
| `a_successor_optimizing_the_measure_instead_of_the_goal_is_caught` | reward hacking |
| `a_successor_cannot_widen_the_gate_that_judges_it` | the Weismann barrier |
| `a_successor_cannot_swap_in_an_easier_evaluation_suite` | suite pinning |
| `drift_across_many_generations_is_stopped_at_the_high_water_mark` | cumulative drift |
| `fallback_skips_generations_that_already_failed` | no falling back into a known failure |
| `authority_does_not_move_to_a_fallback_that_cannot_run` | fallback must be real |
| `a_predictor_that_stops_being_right_loses_its_selectivity` | miscalibration widens search |
| `an_unproven_predictor_does_not_get_to_narrow_the_search` | trust is earned |
| `directed_search_spends_budget_on_the_best_predicted_candidates` | the win, when calibrated |
| `candidates_per_cycle_decides_what_the_search_may_choose_from` | the policy field does something |
| `a_run_with_a_pool_is_still_reproducible_from_its_seed` | sampling did not cost reproducibility |
| `an_attached_predictor_starts_untrusted` | attaching a surrogate does not narrow anything |
| `a_full_cycle_promotes_fails_falls_back_and_then_succeeds` | the whole loop |
