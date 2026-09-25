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
use crate::learner::{Learner, LearnerConfig};
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
    pub learners: Vec<LearnerConfig>,
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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rounds: 30,
            programs_per_round: 48,
            substrate: Substrate::default(),
            learners: vec![LearnerConfig::default()],
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
    pub config: LearnerConfig,
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
    pub total_outputs: usize,
    pub best_programs: Vec<(String, f64, String)>,
    pub hypervolume: f64,
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
    let mut learners: Vec<Learner> = cfg.learners.iter().map(|c| Learner::new(*c)).collect();
    let mut joules: Vec<Option<f64>> = vec![Some(0.0); learners.len()];
    let mut all_measured = vec![true; learners.len()];
    let mut seconds = vec![0.0f64; learners.len()];
    let mut last_readings: Vec<Reading> = Vec::new();
    let mut pool = Pool::new(cfg.pool_capacity);
    let mut rng = Rng(cfg.seed);
    let mut refusals: BTreeMap<String, usize> = BTreeMap::new();
    let mut checkpoints = Vec::new();
    // How often each exact output has been produced. A repeat earns
    // progress / (1 + times seen): count-based novelty, so a generator cannot
    // hold its budget by emitting the one sequence the learner is currently
    // absorbing. The first run collapsed onto constant byte runs without it.
    let mut seen: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
    let mut distinct_outputs = 0usize;
    let mut run_credit = vec![0.0f64; agents.len()];
    let mut run_cost = vec![0.0f64; agents.len()];

    for round in 0..cfg.rounds {
        // 1. Allocate.
        let scores: Vec<f64> = (0..agents.len())
            .map(|i| if run_cost[i] > 0.0 { run_credit[i] / run_cost[i] } else { 0.0 })
            .collect();
        let shares = allocate(cfg.programs_per_round, &scores, cfg.floor, cfg.temperature);

        let mut round_data: Vec<Vec<u8>> = Vec::new();
        for (ai, agent) in agents.iter_mut().enumerate() {
            standing[ai].last_share = shares[ai];
            // 2. Propose.
            let cands: Vec<Candidate> = agent.propose(shares[ai], &pool, &mut rng);
            let mut scored: Vec<Scored> = Vec::with_capacity(cands.len());
            let mut credit = 0.0;
            let mut cost = 0.0;
            for c in cands {
                standing[ai].proposed += 1;
                // 3. Gate and run.
                let seed = rng.next_u64();
                match cfg.substrate.run(&c.program.source(), seed) {
                    Outcome::Bytes { bytes, fuel_used } => {
                        // 4. Credit, before any learner trains on it.
                        let raw = learners.iter().map(|l| l.progress(&bytes)).sum::<f64>() / learners.len() as f64;
                        let times = seen.entry(fingerprint(&bytes)).or_insert(0);
                        if *times == 0 {
                            distinct_outputs += 1;
                        }
                        let r = raw / (1.0 + *times as f64);
                        *times += 1;
                        standing[ai].produced += 1;
                        standing[ai].credit += r;
                        standing[ai].fuel += fuel_used;
                        credit += r;
                        // Every candidate costs at least one unit, so a stream
                        // of instant refusals is not free.
                        cost += fuel_used.max(1) as f64;
                        pool.add(c.program.clone(), bytes.clone(), r, agent.id(), round);
                        round_data.push(bytes);
                        scored.push((c, Some(r)));
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
            // 6. Feed back.
            agent.feedback(&scored);
            run_credit[ai] = cfg.credit_decay * run_credit[ai] + credit;
            run_cost[ai] = cfg.credit_decay * run_cost[ai] + cost;
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

        if (round + 1) % cfg.eval_every == 0 || round + 1 == cfg.rounds {
            for (li, learner) in learners.iter().enumerate() {
                let heldout_bpb: BTreeMap<String, f64> =
                    heldout.iter().map(|c| (c.name.clone(), learner.bits_per_byte(&c.bytes))).collect();
                let mean_bpb = heldout_bpb.values().sum::<f64>() / heldout_bpb.len() as f64;
                checkpoints.push(Checkpoint {
                    round: round + 1,
                    learner: li,
                    heldout_bpb,
                    mean_bpb,
                    joules: joules[li],
                    joules_all_measured: all_measured[li],
                    seconds: seconds[li],
                    bytes_seen: learner.bytes_seen,
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
        distinct_outputs,
        total_outputs: seen.values().map(|v| *v as usize).sum(),
        best_programs,
        hypervolume: archive.hypervolume(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_cfg() -> Config {
        Config {
            rounds: 6,
            programs_per_round: 16,
            substrate: Substrate { fuel: 50_000, max_bytes: 128 },
            learners: vec![
                LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 },
                LearnerConfig { orders: 1, log2_buckets: 6, lr: 0.05 },
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
    fn missing_heldout_data_is_refused() {
        let (m, n) = cpu_only();
        assert!(run(tiny_cfg(), vec![], m, n).is_err());
    }
}
