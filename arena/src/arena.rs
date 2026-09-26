//! The loop.
//!
//! Each round:
//!
//! 1. **Allocate** the round's fixed evaluation budget among agents by credit
//!    per unit cost ([`crate::agents::allocate`]).
//! 2. **Propose.** Each agent writes its share of MAGE programs.
//! 3. **Gate and run** them on the substrate. Refusals are counted by reason.
//! 4. **Credit** each program that produced bytes with the learning progress
//!    it would cause, averaged over learners — measured *before* training, so
//!    a program is judged by what it offers the learner, not by what the
//!    learner already absorbed from it.
//! 5. **Train** every learner on the round's data plus replay from the pool,
//!    inside an energy span of its own, so joules are attributed per learner.
//! 6. **Feed back** to agents, and fold the round into their credit.
//!
//! Every `eval_every` rounds each learner is scored on held-out natural data
//! that no agent can read — bits per byte, an exact likelihood, not a judge.
//! At the end the learners form a Pareto archive over
//! (held-out bits/byte ↓, training joules ↓, prediction latency ↓), and each
//! agent's standing is its credit per unit cost.
//!
//! ## What the numbers mean
//!
//! *Intelligence per joule* is reported as bits saved per byte of held-out data
//! (8 − bpb, against a uniform model) divided by the learner's training joules.
//! It is only as honest as the joule figure beneath it, so the report carries
//! each meter's [`crate::energy::Basis`] beside it.

use crate::agents::{allocate, Candidate, GrammarAgent, MutatorAgent, Pool, Proposer, Scored, UniformAgent};
use crate::energy::{self, Meter, Reading, Span};
use crate::grammar::Rng;
use crate::learner::{self, ByteLearner, LearnerSpec};
use crate::substrate::{Outcome, Refusal, Substrate};
use germline::pareto::{Archive, Objective};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A held-out corpus: data no agent can reach, used only for scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus {
    pub name: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
    /// SHA-256 of the bytes, so a report states exactly what it was scored on.
    pub sha256: String,
    pub len: usize,
}

impl Corpus {
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Corpus {
        use sha2::{Digest, Sha256};
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Corpus { name: name.into(), len: bytes.len(), bytes, sha256 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub rounds: usize,
    pub programs_per_round: usize,
    pub substrate: Substrate,
    pub learners: Vec<LearnerSpec>,
    /// Agents as `(kind, count)`: `grammar`, `uniform`, `mutator`.
    pub agents: Vec<(String, usize)>,
    pub seed: u64,
    pub replay_per_round: usize,
    /// Optimiser steps per learner per round, each over the round's batch.
    pub learner_steps: usize,
    pub pool_capacity: usize,
    pub eval_every: usize,
    /// Evaluations every agent is guaranteed per round.
    pub floor: usize,
    /// Softmax temperature of the budget allocation.
    pub temperature: f64,
    /// Decay of agents' running credit and cost.
    pub credit_decay: f64,
    /// Wattage assumed by the wall-clock estimate of CPU energy.
    pub cpu_watts: f64,
    /// Description-length charge per MAGE token: credit is multiplied by
    /// `exp(-length_charge · tokens)`. The Solomonoff prior self-play
    /// pretraining regularises its generator toward, applied to every agent's
    /// credit rather than one agent's policy, so padding costs whoever pads.
    pub length_charge: f64,
    /// Output entropy floor, bits/byte (LZ76): credit is multiplied by
    /// `min(1, lz / entropy_floor)`. **Off by default, because it was measured
    /// to backfire:** noise has the highest entropy of all, and with the floor
    /// on, the generator drifted from constants to hash-like programs (fit to
    /// its own data 4.67 → 8.10 bits/byte). Kept for the record and for
    /// experiments; `0` disables it.
    pub entropy_floor: f64,
    /// What a program's output is credited with.
    pub reward: Reward,
}

/// The learning signal a program earns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reward {
    /// `|⟨∇L, P ⊙ δθ⟩|` before training (Cowsik et al.): how aligned the
    /// output's gradient is with where the learner is moving.
    Alignment,
    /// Compression progress (Schmidhuber), measured on a **probe**: the same
    /// program run with a different seed, never trained on. The probe's bits
    /// per byte before the round's training minus after it.
    ///
    /// The probe is the point. Re-scoring the trained output itself measures
    /// memorisation, and the first version did exactly that: a learner with
    /// thousands of buckets memorised pseudo-random bytes as well as structure,
    /// and the two earned the same credit (74.6 bits vs 74.6). What a program
    /// is worth is whether learning its output teaches the learner to predict
    /// *more* of it: a hash-like program's outputs for two seeds are unrelated,
    /// and a structured program's share their structure.
    Compression,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rounds: 30,
            programs_per_round: 48,
            substrate: Substrate::default(),
            learners: vec![LearnerSpec::default()],
            agents: vec![("grammar".into(), 2), ("uniform".into(), 1), ("mutator".into(), 1)],
            seed: 1,
            replay_per_round: 16,
            learner_steps: 8,
            pool_capacity: 256,
            eval_every: 5,
            floor: 2,
            temperature: 0.5,
            credit_decay: 0.8,
            cpu_watts: 65.0,
            length_charge: 0.003,
            entropy_floor: 0.0,
            // Measured better than alignment on every held-out figure at 900k
            // parameters, and without the late collapse (ARENA.md, run 9).
            reward: Reward::Compression,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStanding {
    pub id: String,
    pub kind: String,
    pub proposed: usize,
    pub produced: usize,
    pub refusals: BTreeMap<String, usize>,
    /// Total learning progress caused.
    pub credit: f64,
    /// Total fuel spent running its programs.
    pub fuel: u64,
    /// Running credit per unit cost, which drives allocation.
    pub score: f64,
    pub last_share: usize,
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub round: usize,
    pub learner: usize,
    /// Held-out bits per byte, per corpus.
    pub heldout_bpb: BTreeMap<String, f64>,
    pub mean_bpb: f64,
    pub joules: Option<f64>,
    pub joules_all_measured: bool,
    pub seconds: f64,
    pub bytes_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerResult {
    pub index: usize,
    pub config: LearnerSpec,
    /// The learner in words, e.g. `transformer d64 L2 h4 ctx128 lr0.001 on cuda:0`.
    pub describe: String,
    pub parameters: usize,
    /// Bits per byte on the pool's own data — what the learner was trained on.
    /// Beside `mean_bpb` it separates *learned something* from *learned
    /// something that transfers*, which is the question the arena exists to
    /// answer and must not assume.
    pub pool_bpb: f64,
    /// Held-out bits per byte, averaged over corpora.
    pub mean_bpb: f64,
    pub train_joules: Option<f64>,
    pub train_seconds: f64,
    /// Seconds to predict one kilobyte, measured on held-out data.
    pub latency_s_per_kb: f64,
    /// (8 − bpb) / joules: bits saved per byte, per joule of training.
    pub intelligence_per_joule: Option<f64>,
    pub on_front: bool,
    pub hypervolume_contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub config: Config,
    pub heldout: Vec<Corpus>,
    pub meters: Vec<String>,
    pub energy_bases: Vec<Reading>,
    pub agents: Vec<AgentStanding>,
    pub checkpoints: Vec<Checkpoint>,
    pub learners: Vec<LearnerResult>,
    pub refusals: BTreeMap<String, usize>,
    pub pool_size: usize,
    /// Distinct byte sequences produced over the run, against the total — the
    /// generator-diversity figure collapse shows up in first.
    pub distinct_outputs: usize,
    /// Distinct program shapes that produced data — the figure novelty is
    /// measured on, and the one a seed cannot inflate.
    pub distinct_shapes: usize,
    pub total_outputs: usize,
    /// Evaluations answered from the cache instead of run.
    pub cache_hits: usize,
    /// Mean LZ76 entropy-rate estimate of the outputs produced, bits/byte
    /// (`crate::measure`): near 0 for constants, near 8 for noise.
    pub mean_output_lz_bits: f64,
    pub best_programs: Vec<(String, f64, String)>,
    pub hypervolume: f64,
}

/// Mean bits per byte of `bytes` across learners.
fn mean_bpb(learners: &[Box<dyn ByteLearner>], bytes: &[u8]) -> f64 {
    learners.iter().map(|l| l.bits_per_byte(bytes)).sum::<f64>() / learners.len() as f64
}

/// The credit multiplier for a program's length and its output's structure.
///
/// Added after both model sizes peaked near round 75 and then degraded as the
/// generator drifted toward low-entropy programs — constant maps wrapped in
/// padding (`ARENA.md`). Learning progress alone pays for constants whenever
/// the learner is still absorbing them; these two terms make constants and
/// padding cheap to ignore rather than profitable to repeat.
pub fn shaping(cfg: &Config, tokens: usize, bytes: &[u8]) -> f64 {
    let length = (-cfg.length_charge * tokens as f64).exp();
    let structure = if cfg.entropy_floor > 0.0 {
        (crate::measure::lz_bits_per_byte(bytes) / cfg.entropy_floor).min(1.0)
    } else {
        1.0
    };
    length * structure
}

/// Count-based novelty over program shapes.
#[derive(Debug, Default)]
pub struct Novelty {
    seen: std::collections::HashMap<String, u32>,
}

impl Novelty {
    /// The factor a program of this shape earns: `1 / (1 + times seen)`,
    /// counting this occurrence afterwards.
    pub fn discount(&mut self, shape: &str) -> f64 {
        let n = self.seen.entry(shape.to_string()).or_insert(0);
        let f = 1.0 / (1.0 + *n as f64);
        *n += 1;
        f
    }

    pub fn distinct(&self) -> usize {
        self.seen.len()
    }
}

/// Evaluation results by (definition hash, seed mod 256 — the argument the
/// substrate actually passes).
#[derive(Debug, Default)]
pub struct EvalCache {
    results: std::collections::HashMap<(String, u64), Outcome>,
    pub hits: usize,
}

impl EvalCache {
    /// Run `prepared` for `seed`, or return the cached outcome with its fuel
    /// reported as zero, since none was spent.
    pub fn run(&mut self, sub: &Substrate, prepared: &crate::substrate::Prepared, seed: u64) -> Outcome {
        let key = (prepared.exact.clone(), seed % 256);
        if let Some(o) = self.results.get(&key) {
            self.hits += 1;
            return match o.clone() {
                Outcome::Bytes { bytes, .. } => Outcome::Bytes { bytes, fuel_used: 0 },
                refused => refused,
            };
        }
        let o = sub.execute(prepared, seed);
        self.results.insert(key, o.clone());
        o
    }
}

/// FNV-1a over a byte sequence.
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^ bytes.len() as u64
}

fn build_agents(spec: &[(String, usize)]) -> Result<Vec<Box<dyn Proposer>>, String> {
    let mut out: Vec<Box<dyn Proposer>> = Vec::new();
    for (kind, n) in spec {
        for i in 0..*n {
            let id = format!("{kind}-{i}");
            out.push(match kind.as_str() {
                "grammar" => Box::new(GrammarAgent::new(id)),
                "uniform" => Box::new(UniformAgent::new(id)),
                "mutator" => Box::new(MutatorAgent::new(id)),
                other => return Err(format!("unknown agent kind `{other}` (grammar, uniform, mutator)")),
            });
        }
    }
    if out.is_empty() {
        return Err("at least one agent is required".into());
    }
    Ok(out)
}

/// The meters available on this machine: NVML if it opens, and always the
/// CPU wall-clock estimate.
pub fn default_meters(cpu_watts: f64) -> (Vec<Box<dyn Meter>>, Vec<String>) {
    let mut meters: Vec<Box<dyn Meter>> = Vec::new();
    let mut notes = Vec::new();
    match energy::Nvml::open() {
        Ok(n) => {
            notes.push(format!("nvml: {} device(s), measured", n.device_count()));
            meters.push(Box::new(n));
        }
        Err(why) => notes.push(format!("nvml unavailable: {why}")),
    }
    meters.push(Box::new(energy::WallClock::new("cpu-estimate", cpu_watts)));
    notes.push(format!("cpu: estimated at {cpu_watts} W"));
    (meters, notes)
}

/// Run the arena to completion.
pub fn run(cfg: Config, heldout: Vec<Corpus>, mut meters: Vec<Box<dyn Meter>>, meter_notes: Vec<String>) -> Result<Report, String> {
    if heldout.is_empty() {
        return Err("at least one held-out corpus is required — without one there is nothing to measure intelligence against".into());
    }
    if cfg.learners.is_empty() {
        return Err("at least one learner is required".into());
    }
    let mut agents = build_agents(&cfg.agents)?;
    let mut standing: Vec<AgentStanding> = agents
        .iter()
        .map(|a| AgentStanding { id: a.id().to_string(), kind: a.kind().to_string(), ..Default::default() })
        .collect();
    let mut learners: Vec<Box<dyn ByteLearner>> =
        cfg.learners.iter().map(|s| learner::build(*s)).collect::<Result<_, _>>()?;
    let mut joules: Vec<Option<f64>> = vec![Some(0.0); learners.len()];
    let mut all_measured = vec![true; learners.len()];
    let mut seconds = vec![0.0f64; learners.len()];
    let mut last_readings: Vec<Reading> = Vec::new();
    let mut pool = Pool::new(cfg.pool_capacity);
    let mut rng = Rng(cfg.seed);
    let mut refusals: BTreeMap<String, usize> = BTreeMap::new();
    let mut checkpoints = Vec::new();
    // Novelty is counted on a program's *shape* — its structure with
    // constants erased (`mage_prototype::canon`) — and a repeat earns
    // progress / (1 + times seen). It was counted on output bytes first, and
    // the generators found `[s; k]`: constant runs keyed on the seed, new bytes
    // every seed, one idea. Distinct outputs are still reported beside it.
    let mut novelty = Novelty::default();
    let mut outputs_seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut total_outputs = 0usize;
    let mut lz_sum = 0.0f64;
    // Results by (definition hash, seed): a program evaluated once for a seed
    // is never evaluated for it again, so a repeat costs no fuel.
    let mut cache = EvalCache::default();
    let mut run_credit = vec![0.0f64; agents.len()];
    let mut run_cost = vec![0.0f64; agents.len()];

    for round in 0..cfg.rounds {
        // 1. Allocate.
        let scores: Vec<f64> = (0..agents.len())
            .map(|i| if run_cost[i] > 0.0 { run_credit[i] / run_cost[i] } else { 0.0 })
            .collect();
        let shares = allocate(cfg.programs_per_round, &scores, cfg.floor, cfg.temperature);

        let mut round_data: Vec<Vec<u8>> = Vec::new();
        // Per agent: its scored candidates, credit and cost this round.
        // Feedback waits until after training, because compression progress
        // cannot be known before it.
        let mut round_scored: Vec<Vec<Scored>> = (0..agents.len()).map(|_| Vec::new()).collect();
        let mut round_credit = vec![0.0f64; agents.len()];
        let mut round_cost = vec![0.0f64; agents.len()];
        // Compression mode: (agent, index into its scored list, bytes,
        // multiplier, bits/byte before training).
        let mut pending: Vec<(usize, usize, Vec<u8>, f64, f64)> = Vec::new();
        for (ai, agent) in agents.iter_mut().enumerate() {
            standing[ai].last_share = shares[ai];
            // 2. Propose.
            let cands: Vec<Candidate> = agent.propose(shares[ai], &pool, &mut rng);
            let scored = &mut round_scored[ai];
            let mut credit = 0.0;
            let mut cost = 0.0;
            for c in cands {
                standing[ai].proposed += 1;
                // 3. Gate, then run — or reuse what an identical program
                // already produced for this seed.
                let seed = rng.next_u64();
                let (outcome, shape, prepared) = match cfg.substrate.prepare(&c.program.source()) {
                    Err((why, msg)) => (Outcome::Refused(why, msg), None, None),
                    Ok(prepared) => {
                        let outcome = cache.run(&cfg.substrate, &prepared, seed);
                        (outcome, Some(prepared.shape.clone()), Some(prepared))
                    }
                };
                match outcome {
                    Outcome::Bytes { bytes, fuel_used } => {
                        // 4. Credit. Alignment is known now, before any
                        // learner trains on the output; compression progress
                        // only after.
                        let mult = novelty.discount(shape.as_deref().unwrap_or_default())
                            * shaping(&cfg, c.program.token_len(), &bytes);
                        total_outputs += 1;
                        lz_sum += crate::measure::lz_bits_per_byte(&bytes);
                        outputs_seen.insert(fingerprint(&bytes));
                        standing[ai].produced += 1;
                        standing[ai].fuel += fuel_used;
                        // Every candidate costs at least one unit, so a stream
                        // of instant refusals is not free.
                        cost += fuel_used.max(1) as f64;
                        match cfg.reward {
                            Reward::Alignment => {
                                let raw = learners.iter().map(|l| l.progress(&bytes)).sum::<f64>()
                                    / learners.len() as f64;
                                let r = raw * mult;
                                standing[ai].credit += r;
                                credit += r;
                                pool.add(c.program.clone(), bytes.clone(), r, agent.id(), round);
                                scored.push((c, Some(r)));
                            }
                            Reward::Compression => {
                                // The probe: this program, another seed.
                                // (The substrate takes seeds mod 256, so a
                                // different residue is a different input.)
                                let probe = match prepared.as_ref().map(|p| cache.run(&cfg.substrate, p, seed + 1)) {
                                    Some(Outcome::Bytes { bytes: pb, .. }) => pb,
                                    _ => bytes.clone(),
                                };
                                let before = mean_bpb(&learners, &probe);
                                pending.push((ai, scored.len(), probe, mult, before));
                                scored.push((c, None));
                            }
                        }
                        round_data.push(bytes);
                    }
                    Outcome::Refused(why, _) => {
                        let key = serde_json::to_value(&why)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_else(|| format!("{why:?}"));
                        *standing[ai].refusals.entry(key.clone()).or_insert(0) += 1;
                        *refusals.entry(key).or_insert(0) += 1;
                        cost += if why == Refusal::Fuel { cfg.substrate.fuel as f64 } else { 1.0 };
                        scored.push((c, None));
                    }
                }
            }
            round_credit[ai] = credit;
            round_cost[ai] = cost;
        }

        // Replay: revisit what taught the most, so learners do not forget it.
        for _ in 0..cfg.replay_per_round {
            if let Some(e) = pool.sample(&mut rng) {
                round_data.push(e.bytes.clone());
            }
        }

        // 5. Train, each learner in its own energy span.
        let batch: Vec<&[u8]> = round_data.iter().map(|b| b.as_slice()).collect();
        for (li, learner) in learners.iter_mut().enumerate() {
            let span = Span::start(&mut meters);
            for _ in 0..cfg.learner_steps {
                learner.train(&batch);
            }
            let (secs, readings) = span.stop(&mut meters);
            seconds[li] += secs;
            match energy::total(&readings) {
                Some((j, measured)) => {
                    joules[li] = joules[li].map(|acc| acc + j);
                    all_measured[li] &= measured;
                }
                None => joules[li] = None,
            }
            last_readings = readings;
        }

        // Compression progress on each program's probe: what the round's
        // training taught about output it never saw, in bits, discounted by
        // novelty and shaping.
        for (ai, idx, probe, mult, before) in pending {
            let after = mean_bpb(&learners, &probe);
            let r = (before - after).max(0.0) * probe.len() as f64 * mult;
            standing[ai].credit += r;
            round_credit[ai] += r;
            let program = round_scored[ai][idx].0.program.clone();
            pool.add(program, probe, r, agents[ai].id(), round);
            round_scored[ai][idx].1 = Some(r);
        }

        // 6. Feed back, now that every credit is known.
        for (ai, agent) in agents.iter_mut().enumerate() {
            agent.feedback(&round_scored[ai]);
            run_credit[ai] = cfg.credit_decay * run_credit[ai] + round_credit[ai];
            run_cost[ai] = cfg.credit_decay * run_cost[ai] + round_cost[ai];
        }

        if (round + 1) % cfg.eval_every == 0 || round + 1 == cfg.rounds {
            for (li, learner) in learners.iter().enumerate() {
                let heldout_bpb: BTreeMap<String, f64> =
                    heldout.iter().map(|c| (c.name.clone(), learner.bits_per_byte(&c.bytes))).collect();
                let mean_bpb = heldout_bpb.values().sum::<f64>() / heldout_bpb.len() as f64;
                // Progress on stderr: a 25M-parameter run takes most of an
                // hour, and the first one printed nothing until it finished.
                eprintln!(
                    "arena: round {:>4}/{} learner {li} held-out {mean_bpb:.3} bits/byte, {} J, {:.0} s",
                    round + 1,
                    cfg.rounds,
                    joules[li].map(|j| format!("{j:.0}")).unwrap_or_else(|| "n/a".into()),
                    seconds[li]
                );
                checkpoints.push(Checkpoint {
                    round: round + 1,
                    learner: li,
                    heldout_bpb,
                    mean_bpb,
                    joules: joules[li],
                    joules_all_measured: all_measured[li],
                    seconds: seconds[li],
                    bytes_seen: learner.bytes_seen(),
                });
            }
        }
    }

    for (ai, agent) in agents.iter().enumerate() {
        standing[ai].score = if run_cost[ai] > 0.0 { run_credit[ai] / run_cost[ai] } else { 0.0 };
        standing[ai].state = agent.describe();
    }

    // The learners' front: (bits/byte ↓, joules ↓, latency ↓).
    let mut results = Vec::new();
    let pool_bytes: Vec<u8> = pool.entries.iter().flat_map(|e| e.bytes.iter().cloned()).take(1 << 16).collect();
    for (li, learner) in learners.iter().enumerate() {
        let mean_bpb = heldout.iter().map(|c| learner.bits_per_byte(&c.bytes)).sum::<f64>() / heldout.len() as f64;
        let sample: Vec<u8> = heldout[0].bytes.iter().take(4096).cloned().collect();
        let t = std::time::Instant::now();
        let _ = learner.bits_per_byte(&sample);
        let latency = t.elapsed().as_secs_f64() / (sample.len().max(1) as f64 / 1024.0);
        results.push(LearnerResult {
            index: li,
            config: cfg.learners[li],
            describe: learner.describe(),
            parameters: learner.parameters(),
            pool_bpb: learner.bits_per_byte(&pool_bytes),
            mean_bpb,
            train_joules: joules[li],
            train_seconds: seconds[li],
            latency_s_per_kb: latency,
            intelligence_per_joule: joules[li].filter(|j| *j > 0.0).map(|j| (8.0 - mean_bpb) / j),
            on_front: false,
            hypervolume_contribution: 0.0,
        });
    }
    let mut archive: Archive<usize> = Archive::new(
        vec![Objective::minimize("heldout_bpb"), Objective::minimize("joules"), Objective::minimize("latency_s_per_kb")],
        // The reference is "no better than nothing": a uniform model's 8 bits,
        // and a cost ceiling above anything a run here reaches.
        vec![8.0, results.iter().filter_map(|r| r.train_joules).fold(1.0, f64::max) * 2.0, results.iter().map(|r| r.latency_s_per_kb).fold(1e-6, f64::max) * 2.0],
        results.len().max(1),
    );
    for r in &results {
        // Unmeasured energy is NaN, which the archive treats as incomparable
        // rather than as free.
        archive.insert(vec![r.mean_bpb, r.train_joules.unwrap_or(f64::NAN), r.latency_s_per_kb], format!("learner-{}", r.index), None, r.index);
    }
    let front: Vec<usize> = archive.front().iter().map(|e| e.payload).collect();
    let contrib: BTreeMap<usize, f64> = archive
        .contributions()
        .into_iter()
        .map(|(id, c)| (archive.entries().iter().find(|e| e.id == id).map(|e| e.payload).unwrap_or(0), c))
        .collect();
    for r in results.iter_mut() {
        r.on_front = front.contains(&r.index);
        r.hypervolume_contribution = contrib.get(&r.index).copied().unwrap_or(0.0);
    }

    let best_programs =
        pool.top(5).into_iter().map(|e| (e.agent.clone(), e.reward, e.program.source())).collect();
    Ok(Report {
        config: cfg,
        heldout,
        meters: meter_notes,
        energy_bases: last_readings,
        agents: standing,
        checkpoints,
        learners: results,
        refusals,
        pool_size: pool.entries.len(),
        distinct_outputs: outputs_seen.len(),
        distinct_shapes: novelty.distinct(),
        total_outputs,
        cache_hits: cache.hits,
        mean_output_lz_bits: lz_sum / total_outputs.max(1) as f64,
        best_programs,
        hypervolume: archive.hypervolume(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learner::LearnerConfig;

    fn tiny_cfg() -> Config {
        Config {
            rounds: 6,
            programs_per_round: 16,
            substrate: Substrate { fuel: 50_000, max_bytes: 128 },
            learners: vec![
                LearnerSpec::Ngram(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 }),
                LearnerSpec::Ngram(LearnerConfig { orders: 1, log2_buckets: 6, lr: 0.05 }),
            ],
            agents: vec![("grammar".into(), 1), ("uniform".into(), 1), ("mutator".into(), 1)],
            eval_every: 3,
            replay_per_round: 4,
            ..Config::default()
        }
    }

    fn heldout() -> Vec<Corpus> {
        vec![Corpus::new("text", b"the quick brown fox jumps over the lazy dog. ".repeat(20))]
    }

    fn cpu_only() -> (Vec<Box<dyn Meter>>, Vec<String>) {
        (vec![Box::new(energy::WallClock::new("cpu-estimate", 65.0))], vec!["cpu".into()])
    }

    #[test]
    fn a_run_trains_learners_and_credits_agents() {
        let (m, n) = cpu_only();
        let r = run(tiny_cfg(), heldout(), m, n).expect("runs");
        assert_eq!(r.learners.len(), 2);
        for l in &r.learners {
            // Learned its training distribution. Held-out transfer is the
            // experiment, not a precondition — six rounds of synthetic data can
            // leave English text at or above 8 bits, and that is a finding.
            assert!(l.pool_bpb < 6.0, "learner {} learned nothing: {}", l.index, l.pool_bpb);
            assert!(l.mean_bpb.is_finite());
            assert!(l.train_joules.is_some());
        }
        let proposed: usize = r.agents.iter().map(|a| a.proposed).sum();
        assert_eq!(proposed, 6 * 16, "the budget is spent exactly");
        assert!(r.agents.iter().any(|a| a.credit > 0.0));
        assert!(r.learners.iter().any(|l| l.on_front));
        assert!(r.pool_size > 0);
    }

    #[test]
    fn the_run_is_reproducible_from_its_seed() {
        let a = run(tiny_cfg(), heldout(), cpu_only().0, vec![]).unwrap();
        let b = run(tiny_cfg(), heldout(), cpu_only().0, vec![]).unwrap();
        let key = |r: &Report| {
            (r.agents.iter().map(|a| (a.proposed, a.produced)).collect::<Vec<_>>(),
             r.learners.iter().map(|l| l.mean_bpb.to_bits()).collect::<Vec<_>>())
        };
        assert_eq!(key(&a), key(&b), "energy and time vary; what was learned must not");
    }

    #[test]
    fn the_seed_keyed_family_earns_one_programs_credit() {
        // The exploit the byte-level count missed: `[s; k]` for any k, run on
        // any seed, is one shape. Its total novelty over many (k, seed) pairs
        // is the harmonic sum of one program's repeats, not a fresh 1.0 each.
        let sub = Substrate { fuel: 10_000, max_bytes: 64 };
        let mut nov = Novelty::default();
        let mut total = 0.0;
        let mut outputs = std::collections::HashSet::new();
        for k in 1..=5 {
            for seed in 0..8u64 {
                let src = format!("@role(candidate)\nf gen(s: usize) -> [usize] {{ [s; {k}] }}");
                let p = sub.prepare(&src).expect("valid");
                if let Outcome::Bytes { bytes, .. } = sub.execute(&p, seed) {
                    outputs.insert(bytes);
                }
                total += nov.discount(&p.shape);
            }
        }
        assert_eq!(outputs.len(), 40, "the bytes really were all different");
        assert_eq!(nov.distinct(), 1, "and it was one shape");
        let harmonic: f64 = (1..=40).map(|n| 1.0 / n as f64).sum();
        assert!((total - harmonic).abs() < 1e-9, "{total} vs {harmonic}");
        assert!(total < 5.0, "40 outputs earned {total}, not 40");
    }

    #[test]
    fn a_repeated_program_costs_no_fuel_the_second_time() {
        let sub = Substrate { fuel: 10_000, max_bytes: 64 };
        let mut cache = EvalCache::default();
        let a = sub.prepare("@role(candidate)\nf gen(s: usize) -> [usize] { range(8).map(|x| x + s) }").unwrap();
        // Alpha-equivalent, so the same definition hash.
        let b = sub.prepare("@role(candidate)\nf gen(t: usize) -> [usize] { range(8).map(|y| y + t) }").unwrap();
        assert_eq!(a.exact, b.exact);
        let first = cache.run(&sub, &a, 3);
        let again = cache.run(&sub, &b, 3 + 256);
        match (first, again) {
            (Outcome::Bytes { bytes: x, fuel_used: f1 }, Outcome::Bytes { bytes: y, fuel_used: f2 }) => {
                assert_eq!(x, y);
                assert!(f1 > 0);
                assert_eq!(f2, 0, "a cached result spends nothing");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn shaping_discounts_constants_and_padding_and_nothing_else() {
        let cfg = Config { entropy_floor: 1.0, ..Config::default() };
        let constant = vec![42u8; 64];
        let structured: Vec<u8> = (0u8..64).map(|i| i % 7 * 3 + i / 9).collect();
        // Constants earn a fraction; structure earns in full.
        assert!(shaping(&cfg, 20, &constant) < 0.5 * shaping(&cfg, 20, &structured));
        // Padding: the same output from a longer program earns less.
        assert!(shaping(&cfg, 200, &structured) < shaping(&cfg, 20, &structured));
        // Disabled, both terms are 1.
        let off = Config { length_charge: 0.0, entropy_floor: 0.0, ..Config::default() };
        assert_eq!(shaping(&off, 500, &constant), 1.0);
    }

    #[test]
    fn compression_progress_on_a_probe_pays_for_structure_and_not_for_noise() {
        // Train on one output of each program, score a *second* output of the
        // same program. Structure generalises across outputs; pseudo-random
        // bytes are memorised and teach nothing about the next draw. Scoring
        // the trained output itself could not tell these apart (74.6 vs 74.6).
        let mut l = crate::learner::Learner::new(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 });
        let structured_train: Vec<u8> = b"abcabcabcabcabcabcabcabc".to_vec();
        let structured_probe: Vec<u8> = b"bcabcabcabcabcabcabcabca".to_vec();
        let mut rng = Rng(5);
        let noise_train: Vec<u8> = (0..24).map(|_| rng.next_u64() as u8).collect();
        let noise_probe: Vec<u8> = (0..24).map(|_| rng.next_u64() as u8).collect();
        let before = (l.bits_per_byte(&structured_probe), l.bits_per_byte(&noise_probe));
        for _ in 0..8 {
            l.train(&[&structured_train, &noise_train]);
        }
        let gain = |b: f64, a: f64, n: usize| (b - a).max(0.0) * n as f64;
        let s = gain(before.0, l.bits_per_byte(&structured_probe), structured_probe.len());
        let n = gain(before.1, l.bits_per_byte(&noise_probe), noise_probe.len());
        assert!(s > 2.0 * n, "structured {s} vs noise {n}");
    }

    #[test]
    fn a_compression_run_credits_agents_and_trains() {
        let cfg = Config { reward: Reward::Compression, ..tiny_cfg() };
        let (m, n) = cpu_only();
        let r = run(cfg, heldout(), m, n).expect("runs");
        assert!(r.agents.iter().any(|a| a.credit > 0.0));
        assert!(r.learners.iter().all(|l| l.pool_bpb < 8.0));
    }

    #[test]
    fn missing_heldout_data_is_refused() {
        let (m, n) = cpu_only();
        assert!(run(tiny_cfg(), vec![], m, n).is_err());
    }
}
