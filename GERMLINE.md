# Germline — model succession, handoff, and fallback

> **Status: control plane complete, workload not.**
> `forge/src/germline/` — 95 tests. Variation, directed search, the gate,
> attestation, lineage, the durable journal, the cycle state machine, and
> supervision are built and tested ✅. Model training and inference are **not**
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

## 8. What is still not built

◻ **Model training and inference.** The variation operators produce genomes; what
a genome *means* — architecture, hyperparameters, data mixture — and the training
run that turns it into an artifact are outside this crate.

◻ **A daemon.** The loop is drivable and fully tested end to end
(`tests/rsi_loop.rs`), but a caller drives it. `Authority::Unattended` is the
seam where a scheduler would attach. Deliberate: the loop should run with a person
watching before it runs without one.

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
cargo test --manifest-path forge/Cargo.toml --test rsi_loop   # 4 closed-loop scenarios
cargo test --manifest-path forge/Cargo.toml --test germline   # 13 succession scenarios
cargo test --manifest-path forge/Cargo.toml germline          # + 78 unit tests
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
