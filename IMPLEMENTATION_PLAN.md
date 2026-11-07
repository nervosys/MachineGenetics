# Implementation plan: from tokens to bytes, and world models

Written 2026-10-28, after plans 5.2–9.3 landed (PRs #38–#73, open and stacked
in order). Two sources drive the new work:

- Cartesia's *From Tokens to Bytes* posts
  ([I](https://www.cartesia.ai/blog/from-tokens-to-bytes-part-1),
  [II](https://www.cartesia.ai/blog/from-tokens-to-bytes-part-2)). The lessons
  that apply to MAGE:
  - judge a model by **loss-to-loss** transfer (proxy loss against held-out
    loss) and by **compute multipliers**, not by compression;
  - fixed tokenization is a prior that learned chunking beats, so MAGE's
    BPE-token cost model needs re-measuring;
  - entropy-proxy chunking gets much of the gain cheaply.
- **Physical simulation** as a second domain. The arena's held-out text sits
  near 7.5 of 8 bits/byte at CPU scale, so comparisons have almost no power.
  Simulated trajectories are unlimited, uncontaminated and learnable at small
  scale. MAGE already has most of the tools a simulator needs: pure fuel-bounded
  functions, `@param` domains, the convexity pass, `--solve`, and forward-mode
  `grad`.

Sizes: **S** is under a day, **M** is a few days, **L** is a week or more.
**[H]** marks an item a Haiku subagent can take from a precise brief. Every
item ends the way plans 5–9 did: tests (break-checked where a test guards a
fix), pins updated, a HANDOFF entry, and a stacked PR.

---

## Phase 0: Housekeeping

- [ ] **0.1** Merge PRs #38–#73 in order. This needs the user. **S**
- [ ] **0.2** Keep this file current. Tick items in the PR that does them, so
  the plan never lives only in a session transcript. **S**

## Phase M: Measure the right thing

The yardstick everything later is judged by, so it goes first.

- [x] **M1** Loss-to-loss curves. **M [H]**
  - From arena checkpoints, which already record `proxy_bpb` and held-out
    `mean_bpb`, fit held-out bpb as a function of proxy bpb per learner
    family, across seeds and rounds.
  - Report the fit and its residuals.
  - Done when `arena --json` carries the fit, and a test fits a known
    synthetic curve within tolerance.
- [x] **M2** Joule multipliers. **M [H]**
  - For two learner families, find the joules (or fuel, under `--cost fuel`)
    each needs to reach the same held-out bpb, interpolating each family's
    frontier.
  - This is Cartesia's compute multiplier with joules as the unit.
  - Done when a `measure` subcommand prints multipliers with seed spread, from
    a directory of run JSONs.
- [ ] **M3** A multi-seed runner. **S [H]** *Runner built (`scripts/arena-seeds.sh`);
  reproducing PR #71's 20-round table with it is still to do.*
  - Generalise the `scratchpad/ab.py` used for PR #71 into
    `scripts/arena-seeds.sh`: N seeds by M configs, run in parallel under
    `--cost fuel`, which is deterministic.
  - Done when it reproduces PR #71's table.

## Phase P: Physical simulation as a domain

- [x] **P1** Trajectory generator. **M [H]**
  - Add a deterministic `arena/src/physics.rs` with these systems:
    - a pendulum;
    - a damped spring;
    - a double pendulum, which is chaotic and gives a hard held-out case;
    - n-body gravity (n = 3);
    - a ball bouncing on a floor, the contact case.
  - Use a fixed-step symplectic integrator where it applies, with each
    system's parameters and initial conditions drawn from stated ranges.
  - Done when each system conserves what it should (energy within a stated
    drift, momentum exactly for n-body), with a test per system.
- [x] **P2** Byte encoding. **S [H]**
  - Quantise states to fixed-point integers, encode them little-endian, and
    interleave channels per timestep, with a header naming system, parameters
    and scale.
  - Done when encode→decode round-trips within the quantum, and the encoding
    is documented in ARENA.md.
- [x] **P3** Physics as a held-out suite. **M [H]**
  - `--heldout-physics <system>:<n>:<seed>` generates held-out corpora from
    parameter ranges disjoint from any training range.
  - The corpora are registered like files: digest, canary planting, and
    slicing all apply.
  - Done when a run reports physics held-out bpb, and the succession suite
    digest covers it.
- [x] **P4** Baseline signal check. **S**
  - Train the existing learners on physics trajectories directly, outside the
    agent loop, and record bpb against uniform.
  - Done when MEASUREMENTS.md records whether physics is learnable at CPU
    scale. This is the premise of the whole phase, so if it fails, stop and
    re-plan here.
  - Verdict (negative): the sub-8 held-out figures are mostly byte statistics;
    the learners do not clearly beat held-out order-0 except spring. Stop and
    re-plan; see MEASUREMENTS.md, "Physics baseline (plan P4)".
- [x] **P4b** Re-plan after P4's negative verdict: is it the encoding? **S [H]**
  - Result: `planes` and `diff2` pass the gate (5 of 5 on held-out order-0; 4 of 5 on train order-0); `i32` (1 of 5) and `coarse` (0 of 5) fail. Default not changed; see MEASUREMENTS.md, "Physics encodings (plan P4b)".
  - P4 found the learners do not beat each held-out corpus's own order-0
    entropy. The likely cause is the encoding, not the physics. At a 1e-5
    quantum a sample is about 17 bits, and its low byte moves by hundreds of
    quanta per step, so a byte-counting learner sees noise where only an
    arithmetic predictor could see structure.
  - Re-run P4 under four encodings, each a `physics-baseline --encoding` option:
    - `i32` (today's);
    - a coarse quantum (1e-3, so values fit in 2 bytes);
    - byte-planes (byte k of every value grouped together, as in Blosc's
      shuffle);
    - second differences (the residual against constant-velocity
      extrapolation).
  - **Gate:** an encoding passes if, on at least three of the five systems,
    the larger n-gram's held-out bpb is at least 0.3 bits below that corpus's
    held-out order-0 entropy.
    - If one passes, make it the default and carry on with P5, P6 and the L
      items on it.
    - If none passes, drop physics as arena *data* (P5, P6, and physics in
      L3), and keep physics as *programs* (C1, C3-C5). C4 already shows that
      track working.
  - Done when MEASUREMENTS.md has the four-encoding table and the decision.
- [x] **P4c** Choose the held-out encoding. **S** *`planes` is now the
  `--heldout-physics` default (`physics::HELDOUT_ENCODING`), and corpus names end
  in the encoding. It holds the same values as `i32`, only reordered, so its
  gain is the learner's. `diff2` also passed, but it builds a constant-velocity
  predictor into the bytes.*
- [x] **P5** Can agents write physics-like data? **M**
  - Result: no, not measurably. Over 3 seeds at 20 rounds, the grammar closes 10% (pendulum), 13% (double pendulum) and 33% (spring) of the gap to the physics baseline, the same as a uniform-program control within seed spread. No production added; default unchanged. See ARENA.md, "Physics from programs (plan P5)".
- [x] **P5b** A second-order production. **M**
  - Result: the production works (`--second-order`, default off), and it lowers held-out bits per byte, but grammar agents do not beat their uniform control. At round 20 `second` is 0.6 to 1.4 bits below uniform, and uniform with the production matches it, so the gain is the production's, not the search's. Fraction closed is negative for `second`, because its round-5 start is already low. Kept off. See ARENA.md, "A second-order production (plan P5b)".
  - `scan` carries one accumulator, so the grammar can only write cascades of
    first-order recurrences, which have real poles and cannot oscillate.
    Pendulum and spring need a complex pole pair.
  - Add a two-state recurrence, `(x1, x2) -> (a*x1 - b*x2) >> k` over bounded
    integers, behind a config flag (default off). It must round-trip through
    `Program::parse`, stay within `MAX_DEPTH`, and respect fuel.
  - Re-run P5's grid with it on. Done when ARENA.md says whether grammar agents
    then beat the uniform control on the oscillators, against the seed
    spread.
- [ ] **P6** Rollout metrics. **M**
  - For learners, generate k steps and decode them. Report error against
    horizon and conservation drift, alongside bpb.
  - Done when the report carries error at horizons 1, 10 and 100 for each
    physics suite.

## Phase L: Learners from the byte-level literature

- [x] **L1** Entropy-chunked learner (BLT-like). **M [H]** *Done: `ent:` learner, `arena/src/entropy.rs`. At 4×2¹⁴ it beats `ng` on text (5.15 vs 5.40 bpb) and code (4.61 vs 5.22); at 3×2¹² it is worse (6.29 vs 5.50 text). Single seed. Out of distribution it can score above uniform (9.52 on README after training on the pool) because the chunk table's weight is global; confidence-dependent weights are the fix, not yet done. Physics rows, which waited on P4b, are in the L2 section of MEASUREMENTS.md: under planes, ent passes the gate on four of five systems and fails n-body (7.84 bpb).*
  - The existing n-gram model's next-byte entropy marks chunk boundaries,
    tuned to a target compression.
  - A chunk-level model predicts over chunks, and a byte decoder predicts
    within them.
  - Done when `--learners ent:...` runs, and its bpb on text, code and physics
    is in MEASUREMENTS.md.
- [x] **L2** Diagonal state-space learner. **M [H]** *Done: `ssm:` learner, `arena/src/ssm.rs`, gradient-checked (worst relative error 7.5e-6 against a 1e-4 bar). At 9,248 parameters it is lowest on text (5.08 bpb, against 5.40 for ng and 5.15 for ent), and passes the planes gate on all five systems, level with ng. Single seed, untuned; see the L2 section of MEASUREMENTS.md.*
  - A small linear-recurrent byte model (diagonal SSM with gated output),
    trained with the existing autograd or a hand-written backward pass.
  - SSMs are discretised ODEs, a natural fit for physics.
  - Done when `--learners ssm:...` runs and is gradient-checked against
    central differences.
- [x] **L3** Learner comparison. **M** *Done: `arena measure-curves` over the physics-baseline curves, n-gram as reference, bytes and seconds axes. On physics the SSM needs 0.43 to 0.51 of the n-gram's bytes on the double pendulum and pendulum (per-seed sd 0.03 and 0.14) and 1.29 on n-body at a reachable target; the entropy learner ties the n-gram on physics (about 1.00) despite its text and code advantage. Nothing is advantaged on physics but not text; see the L3 section of MEASUREMENTS.md.*
  - Use M2's joule multipliers for n-gram, transformer, L1 and L2, on text,
    code and each physics system, 6 seeds each.
  - Done when the table is in MEASUREMENTS.md, with the loss-to-loss fits
    from M1.
- [ ] **L4** Learned chunking (H-Net-like), end to end. **L, GPU**
  - Blocked on GPU work; listed so it isn't forgotten.

## Phase S: Re-measure the token-cost thesis

MAGE's sigil surface and the description-length charge (`token_len`, counted in
MAGE lexer tokens) assume cost scales with BPE tokens. Learned chunking can
erase that advantage.

- [x] **S1** Pluggable cost unit. **S [H]** *Done: `--length-unit tokens|bytes`
  (default tokens, `Program::length`). The BPE and bpb units are deferred to S2.*
  - `token_len` becomes `cost(program, unit)` with units:
    - bytes;
    - MAGE lexer tokens (today's unit);
    - a named BPE tokenizer, vendored offline or as a pinned crate;
    - measured bpb under a given arena learner.
  - Done when the arena charge takes `--length-unit`, defaulting to today's.
- [ ] **S2** Sigil against keyword surfaces. **M**
  - Over the 103-file corpus (lossless both ways since PR #65), measure each
    surface in all four S1 units.
  - Done when MAGE_SPEC records which surface is cheaper in which unit, and
    by how much.
- [ ] **S3** Decide the charge. **S**
  - Choose the description-length unit from S2 and L3, or keep tokens with
    the evidence recorded.
  - Done when the decision is in UNIFICATION.md.

## Phase C: Simulation programs in MAGE

Physics as programs agents write, checked by the language, not only as data.

- [x] **C1** `result` and `old` in `@ens`. **M**
  - MAGE_SPEC §12.1 says neither is in scope, so a postcondition relating a
    step's output to its input cannot be written.
  - Add `result` (and `old` for arguments), and check postconditions at run
    time in the fuel-bounded evaluator. A violation is an evaluation error.
  - Done when a postcondition naming `result` is checked, a violating program
    fails with the contract named, and two MAGE_SPEC examples stop being
    labelled "does not parse".
  - Result: `result` (typed as the return type) and `old(param)` are in scope
    in `@ens`; elsewhere in a contract they are E0561 (`ContractScope`).
    `--check` typechecks each clause of a spec sharing a function's name, and
    the evaluator, bounded or not, checks every `@ens` at each return. A
    violation is an evaluation error naming the clause, the function and
    where it was written, and the value returned. `@req` is still not
    evaluated. Code: `prototype/src/contract.rs`; tests there and in
    `prototype/tests/ens_result_is_checked.rs`.
- [ ] **C2** Units of measure. **L**
  - Dimension annotations on numeric types, for example `f64<m/s>`, inferred
    through arithmetic. Adding mismatched units is a type error with a new
    error code.
  - Done when the typechecker refuses `metres + seconds`, accepts
    `metres / seconds` as `m/s`, and the corpus typechecks unchanged.
- [ ] **C3** A physics program task. **M**
  - Agents propose `step(state, params, dt) -> state` under
    `@role(candidate)`.
  - The score is held-out rollout error against P1's trajectories, and
    conservation is enforced as a C1 contract.
  - Done when a run has agents competing on it, and a wrong integrator is
    refused by its contract.
- [x] **C4** System identification with `--solve`. **S [H]**
  - Fit a pendulum's length, and a spring's stiffness and damping, from a
    trajectory: `@param` domains, a squared-error objective, and `--solve`.
  - Done when the known parameters are recovered within tolerance, and the
    solver choice is recorded (convex or ES).
  - Result: spring (`k`, `c`) and pendulum (`L`) both ran ES, since the loop
    is uncertified; each recovers its known value within 0.01 (test
    `prototype/tests/system_id.rs`).
- [ ] **C5** Contact as a convex step. **M**
  - Pose the bouncing-ball contact as a box-constrained quadratic program, so
    6.3's certified projected-gradient path solves it.
  - Done when the convexity pass certifies the objective, and the solved step
    matches P1's reference.

## Phase F: Follow-ups found along the way

- [ ] **F1** Grammar depth. **S**
  - `mutate` and `crossover` exceed `MAX_DEPTH` about 16% of the time (found
    by PR #73's round-trip test).
  - Measure whether bounding the grafts changes held-out results (M3 under
    `--cost fuel`), then bound them or record why not.
- [x] **F2** A separate ledger account for command proposers. **S [H]**
  - Command proposers currently share the `proposer` role's account with
    grammar agents.
  - Done when a command agent's joules appear under its own role or account.
- [x] **F3** Prompt boundaries. **S [H]**
  - Cartesia II's prompt-boundary problem: a prompt that ends mid-token
    skews a tokenized model.
  - Done when the command proposer's prompt is guaranteed to end on a newline
    after `---`, with a test.

## Phase X: Blocked on the user or on hardware

- [ ] **X1** D4: choose the API model, budget and key location. **User**
- [ ] **X2** The API proposer path. **M, needs X1**
  - A token-count joules estimate with its coefficients cited in the code,
    labelled as an estimate, behind PR #73's `Backend` trait.
- [ ] **X3** Plan 3.3: hardware energy counters (RAPL). **Linux**
- [ ] **X4** Plan 4.3: held-out text below uniform, plus a fitted L(J).
  **GPU**
- [ ] **X5** Phase 10:
  - runs at three or more energy budgets;
  - estimate ℛ from the journal;
  - docs and pins.
  **GPU and quiet machine**
- [ ] **X6** L4, the end-to-end H-Net-like learner. **GPU**

## Order

1. M3 → M1 → M2. Measure first.
2. P1 → P2 → P4. Stop if physics isn't learnable at CPU scale. *P4 was
   negative, so P4b gates everything that uses physics as data.*
3. P3, then L1 and L2 in parallel, then L3. *L1 and L2 on text and code can
   start; their physics rows wait on P4b.*
4. S1 → S2 → S3. Independent of P, so it can run alongside.
5. C1 → C4 → C3, then C2 and C5.
6. F1–F3 whenever convenient.

The critical path is **M1 → P4 → L3**: it decides whether the new domain gives
the self-improvement loop a measurable signal, which is the point of the
exercise.

**Where the critical path landed (L3).**
- **Yes, physics gives a measurable signal.** On held-out text every learner
  scores *above* the corpus's own order-0 entropy (5.08-5.40 against 4.67), so
  text gives no headroom at CPU scale. On planes-encoded physics every learner
  beats order-0 by 0.5-1.1 bits. Six seeds vary the trajectories, so the
  margins have a spread to be judged against, which text, with nothing seeded,
  cannot have.
- **No learner is advantaged on physics but not on text.** The entropy learner
  ties the n-gram on physics (bytes multiplier about 1.00, sd at most 0.07)
  despite 0.64 on text and code. The SSM's 0.43-0.51 on pendulum and double
  pendulum is real against its seed sd, but it is also cheaper on text (0.81),
  and it is worse on n-body (1.29 ± 0.05).
- **Next:** wire physics into the loop (P5: can agent programs teach it?) and
  measure joules on the L3 axes once RAPL exists (X3). L3 used bytes and
  seconds.
