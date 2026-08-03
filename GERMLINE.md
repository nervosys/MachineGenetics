# Germline — model succession, handoff, and fallback

> **Status: control plane implemented, workload not.**
> `forge/src/germline/` — 57 tests. The lineage, promotion gate, directed search,
> and supervisor are built and tested ✅. Model training and inference are **not**
> here and are not claimed ◻ — this decides *whether* a successor takes over, not
> how it is produced. Marked per the rules in [DOCS.md](DOCS.md).

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

## 7. What is not built

◻ Model training, inference, and the actual mutation operators. The compiler
already has genetic operators (`prototype/src/evolve_gen.rs`: tournament/roulette/
rank/elitist selection, single-point/two-point/uniform crossover, bit-flip/
Gaussian/swap mutation) behind the `evolve` keyword; wiring them to this control
plane is the next step.

◻ The unattended episode runner — today a caller drives propose → evaluate →
adjudicate → promote → supervise explicitly. That is deliberate for now: the loop
should be run with a human watching before it is run without one.

◻ Persistence. The lineage serializes to JSON but is not yet stored durably, and a
succession record that does not survive a restart is not an audit trail.

◻ Cryptographic signing of verdicts. The lineage records *which* evaluator issued
a verdict; nothing yet proves it. `prototype/src/certs.rs` is the substrate.

## 8. An honest note on the goal

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

## 9. Reproducing

```powershell
cargo test --manifest-path forge/Cargo.toml --test germline   # 13 succession scenarios
cargo test --manifest-path forge/Cargo.toml germline          # + 44 unit tests
```

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
| `a_full_cycle_promotes_fails_falls_back_and_then_succeeds` | the whole loop |
