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
//! With `heldout_slices > 1` the held-out data is cut into slices, checkpoints
//! score the active slice only, and the slice rotates when the proxy gap drifts
//! (plan 7.5). Rotation happens only at checkpoints, so each epoch between two
//! of them sees one slice.
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

use crate::agents::{
    marginal_hypervolume, non_dominated, AttackerAgent, Candidate, GrammarAgent, MutatorAgent, Pool,
    Proposer, Role, Scored, UniformAgent,
};
use crate::energy::{self, Meter, Reading, Span};
use crate::grammar::Rng;
use crate::learner::{self, ByteLearner, LearnerSpec};
use crate::kernel::{EnergyLedger, PROBES, PROGRAMS, TRAINING};
use crate::substrate::{Outcome, Refusal, Substrate};
use mage_prototype::cost_calibration::{CalibrationSuite, EnergySample, FuelCalibration};
use germline::pareto::{Archive, Objective};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Held-out data lives in the trusted kernel, which alone reads its bytes.
pub use crate::kernel::Corpus;

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
    /// Path to the MAGE program that computes the compression reward;
    /// `None` is the shipped `harness/reward.mg` (plan 5.1).
    pub reward_program: Option<String>,
    /// Path to the MAGE program that turns agents' scores into budget
    /// weights; `None` is the shipped `harness/allocation.mg` (plan 5.2).
    #[serde(default)]
    pub allocation_program: Option<String>,
    /// Path to the MAGE program that gives each pool entry the priority replay
    /// evicts by; `None` is the shipped `harness/retention.mg` (plan 5.2).
    #[serde(default)]
    pub retention_program: Option<String>,
    /// What an agent's programs are charged in, which is the denominator of
    /// the credit-per-cost score that allocates the budget.
    #[serde(default)]
    pub cost_unit: CostUnit,
    /// The run's fixed energy budget in joules (plan 3.5): when the kernel's
    /// ledger reaches it, remaining programs are refused and the run stops.
    /// `None` is unbounded.
    #[serde(default)]
    pub energy_budget: Option<f64>,
    /// An agent's allowance per program it is granted, in joules: granted `k`
    /// programs in a round, it may spend `k` times this before the kernel
    /// refuses the rest of its programs for that round. `None` is no cap.
    #[serde(default)]
    pub joules_per_program: Option<f64>,
    /// A forge registry root to publish the run's best programs into, by
    /// canonical hash (plan 2.4). `None` publishes nothing.
    #[serde(default)]
    pub definitions_dir: Option<String>,
    /// What an agent is credited with when the budget is divided (plan 7.1b).
    #[serde(default)]
    pub allocation: Allocation,
    /// A canary planted in every held-out corpus (plan 7.5). A program whose
    /// output carries it is refused (`canary`) and the run halts, because an
    /// agent has seen held-out bytes. `None` plants nothing, which keeps
    /// held-out figures comparable with runs before canaries; planting adds
    /// its length plus two bytes to each corpus.
    #[serde(default)]
    pub canary: Option<String>,
    /// What a program's novelty is counted on (plan 7.4). A recorded config
    /// without the field predates the choice and was keyed on shape, so it
    /// still reads as shape; a new config defaults to behaviour.
    #[serde(default = "NoveltyKey::recorded_before_the_choice")]
    pub novelty: NoveltyKey,
    /// How many contiguous, disjoint slices each held-out corpus is cut into
    /// (plan 7.5). Checkpoints score the active slice only. `1` is no slicing
    /// and no rotation, so a config without the field reads as it always did.
    #[serde(default = "default_heldout_slices")]
    pub heldout_slices: usize,
    /// The gap drift, in bits/byte, that rotates the active slice at a
    /// checkpoint (plan 7.5): see [`Refresh`]. Only used when `heldout_slices > 1`.
    #[serde(default = "default_refresh_drift")]
    pub refresh_drift: f64,
}

fn default_heldout_slices() -> usize {
    1
}

fn default_refresh_drift() -> f64 {
    0.5
}

/// What a program's novelty is counted on (plan 7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoveltyKey {
    /// Its shape: its structure with constants erased. Costs nothing, and is
    /// exploitable: a rewrite that changes structure and not output, such as
    /// `xs.reverse().reverse()`, is a new shape. The attacker finds this.
    Shape,
    /// Its behaviour: its outputs on [`BEHAVIOUR_SEEDS`]. An output-preserving
    /// rewrite has its parent's key. Two programs that agree on those seeds
    /// and differ elsewhere share a key too. Costs up to three evaluations
    /// per new program, cached. The default since a six-seed held-out
    /// comparison found no cost to switching (ARENA.md).
    #[default]
    Behaviour,
}

impl NoveltyKey {
    /// The key of a config recorded before the field existed: shape.
    fn recorded_before_the_choice() -> NoveltyKey {
        NoveltyKey::Shape
    }
}

/// The seeds a program's behaviour is fingerprinted on.
pub const BEHAVIOUR_SEEDS: [u64; 3] = [17, 101, 211];

/// Seeds, besides the scoring seed, on which the kernel checks an attacker's
/// claim that a candidate behaves as its parent.
const ATTACK_SEEDS: u64 = 3;

/// What an agent's credit is, in the credit-per-cost score that divides the
/// budget (plan 7.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Allocation {
    /// The reward its programs earned, summed. Two agents proposing the same
    /// good program are each credited in full.
    #[default]
    Share,
    /// The hypervolume its programs added to the run's front of programs on
    /// (reward ↑, cost ↓): credit for trade-offs the run did not already
    /// have, so a copy of earlier work earns nothing. The cost axis's
    /// reference is fixed at four times the costliest program of the first
    /// round that produced any, so a program costing more than that counts
    /// as worse than nothing on cost.
    Hypervolume,
}

/// The unit agents are charged in (plan 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostUnit {
    /// The joules each agent's own actions measured: its proposing, and each
    /// program's gating, run and probe, each in its own energy span. Decision
    /// D2's unit. A run is then reproducible from its seed only under a
    /// deterministic meter, because the allocation follows the readings.
    #[default]
    Joules,
    /// Fuel, with one unit per program as a floor. The unit before plan 7.1,
    /// kept for comparison. The fuel calibration found it charges a program's
    /// fixed cost, most of its energy, at almost nothing (ARENA.md).
    Fuel,
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
            reward_program: None,
            allocation_program: None,
            retention_program: None,
            cost_unit: CostUnit::Joules,
            energy_budget: None,
            joules_per_program: None,
            definitions_dir: None,
            allocation: Allocation::Share,
            canary: None,
            novelty: NoveltyKey::Behaviour,
            heldout_slices: default_heldout_slices(),
            refresh_drift: default_refresh_drift(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStanding {
    pub id: String,
    pub kind: String,
    /// The role it acts in (plan 7.2).
    #[serde(default)]
    pub role: crate::agents::Role,
    pub proposed: usize,
    pub produced: usize,
    pub refusals: BTreeMap<String, usize>,
    /// Total learning progress caused.
    pub credit: f64,
    /// Total fuel spent running its programs.
    pub fuel: u64,
    /// Joules its own actions measured, over the run. Zero under
    /// [`CostUnit::Fuel`].
    #[serde(default)]
    pub joules: f64,
    /// Running credit per unit cost, which drives allocation.
    pub score: f64,
    pub last_share: usize,
    pub state: serde_json::Value,
    /// Hypervolume its programs added to the run's front of programs (plan
    /// 7.1b). Reported under either allocation; it is the credit only under
    /// [`Allocation::Hypervolume`].
    #[serde(default)]
    pub hypervolume_added: f64,
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
    /// Bits per byte on the pool's own data, up to 64 KiB of it: the proxy
    /// the loop optimises (plan 7.5).
    #[serde(default)]
    pub proxy_bpb: f64,
    /// Held-out minus proxy bits per byte. A gap that widens while the proxy
    /// improves is the loop fitting its own data rather than learning.
    #[serde(default)]
    pub gap: f64,
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
    /// (agent, reward, source, canonical hash).
    pub best_programs: Vec<(String, f64, String, String)>,
    pub hypervolume: f64,
    /// Fuel calibrated to joules (plan 3.2): one sample per program, the
    /// fuel its run and probe spent against the energy its span read,
    /// gating included. `None` when the fit is undetermined.
    #[serde(default)]
    pub fuel_calibration: Option<FuelCalibration>,
    /// The kernel's energy ledger: what was charged, to what, against which
    /// budget, and whether the run halted on it.
    #[serde(default)]
    pub energy: EnergyLedger,
    /// Rounds run; fewer than configured when the budget ran out, and the
    /// last of them may have been cut short.
    #[serde(default)]
    pub rounds_completed: usize,
    /// Exploits the attacker confirmed (plan 7.4): output-preserving
    /// rewrites, checked by the kernel, that novelty counted as new.
    #[serde(default)]
    pub attacks: Vec<Attack>,
    /// Attacker claims the kernel checked, confirmed or not.
    #[serde(default)]
    pub attack_claims: usize,
    /// Held-out slice rotations (plan 7.5), in the order they happened. Empty
    /// unless `heldout_slices > 1` and the gap drifted.
    #[serde(default)]
    pub refreshes: Vec<Refresh>,
}

/// A held-out slice rotation (plan 7.5). Rotations happen only at checkpoints,
/// which are the epoch boundaries of the run: an epoch is the stretch of rounds
/// between two checkpoints, and the slice is fixed within one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refresh {
    /// The checkpoint round at which the rotation was decided. The checkpoint
    /// at that round was still scored on `from_slice`.
    pub round: usize,
    /// The slice that was active, numbered from 1 as in the slice's name.
    pub from_slice: usize,
    /// The slice active from the next checkpoint, numbered from 1.
    pub to_slice: usize,
    /// The mean gap across learners at that checkpoint.
    pub gap: f64,
    /// The mean gap the slice's first checkpoint set, which `gap` drifted from.
    pub baseline: f64,
}

/// A confirmed exploit of the novelty count (plan 7.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attack {
    pub round: usize,
    pub agent: String,
    /// The pool entry it rewrote.
    pub parent: u64,
    /// The rewrite's source.
    pub source: String,
    /// Seeds on which the kernel found it and its parent identical.
    pub seeds_checked: usize,
}

/// The calibration target the arena's programs report under.
pub const PROGRAM_TARGET: &str = "arena-program";

/// Mean bits per byte of each sequence across learners, batched per learner.
fn mean_bpb_many(learners: &[Box<dyn ByteLearner>], seqs: &[&[u8]]) -> Vec<f64> {
    let mut acc = vec![0.0f64; seqs.len()];
    for l in learners {
        for (a, v) in acc.iter_mut().zip(l.bits_per_byte_many(seqs)) {
            *a += v;
        }
    }
    acc.into_iter().map(|a| a / learners.len() as f64).collect()
}

/// The credit multiplier for a program's length and its output's structure.
///
/// Added after both model sizes peaked near round 75 and then degraded as the
/// generator drifted toward low-entropy programs — constant maps wrapped in
/// padding (`ARENA.md`). Learning progress alone pays for constants whenever
/// the learner is still absorbing them; these two terms make constants and
/// padding cheap to ignore rather than profitable to repeat.
pub fn shaping(cfg: &Config, tokens: usize, bytes: &[u8]) -> f64 {
    (-cfg.length_charge * tokens as f64).exp() * structure_factor(cfg, bytes)
}

/// `min(1, lz / entropy_floor)`, or 1 with the floor off.
pub fn structure_factor(cfg: &Config, bytes: &[u8]) -> f64 {
    if cfg.entropy_floor > 0.0 {
        (crate::measure::lz_bits_per_byte(bytes) / cfg.entropy_floor).min(1.0)
    } else {
        1.0
    }
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
        1.0 / (1.0 + self.observe(shape) as f64)
    }

    /// How many times this shape was seen before now, counting this one
    /// afterwards. The reward program is given the count, not the factor.
    pub fn observe(&mut self, shape: &str) -> u32 {
        let n = self.seen.entry(shape.to_string()).or_insert(0);
        let before = *n;
        *n += 1;
        before
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
    /// Fuel actually spent by the evaluator, refusals included; hits add 0.
    pub fuel_spent: u64,
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
        let (o, fuel) = sub.execute_metered(prepared, seed);
        self.fuel_spent += fuel;
        self.results.insert(key, o.clone());
        o
    }
}

/// The key a program's novelty is counted under.
fn novelty_key(cfg: &Config, cache: &mut EvalCache, prepared: &crate::substrate::Prepared) -> String {
    match cfg.novelty {
        NoveltyKey::Shape => prepared.shape.clone(),
        NoveltyKey::Behaviour => BEHAVIOUR_SEEDS
            .iter()
            .map(|&s| match cache.run(&cfg.substrate, prepared, s) {
                Outcome::Bytes { bytes, .. } => format!("{:016x}", fingerprint(&bytes)),
                Outcome::Refused(why, _) => format!("{why:?}"),
            })
            .collect::<Vec<_>>()
            .join(","),
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
                "attacker" => Box::new(AttackerAgent::new(id)),
                other => return Err(format!("unknown agent kind `{other}` (grammar, uniform, mutator, attacker)")),
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
    meters_for(cpu_watts, None)
}

/// Meters for the listed NVML GPUs (all of them for `None`), plus the CPU
/// estimate.
pub fn meters_for(cpu_watts: f64, gpus: Option<&[u32]>) -> (Vec<Box<dyn Meter>>, Vec<String>) {
    let mut meters: Vec<Box<dyn Meter>> = Vec::new();
    let mut notes = Vec::new();
    // An empty list is "no GPU": a CPU-only run on a machine whose GPUs are
    // busy with other work would otherwise be charged for that work.
    if gpus.is_some_and(|g| g.is_empty()) {
        notes.push("nvml: no devices requested".into());
    } else {
        match energy::Nvml::open_devices(gpus) {
            Ok(n) => {
                notes.push(format!("nvml: {} device(s), measured", n.device_count()));
                meters.push(Box::new(n));
            }
            Err(why) => notes.push(format!("nvml unavailable: {why}")),
        }
    }
    meters.push(Box::new(energy::WallClock::new("cpu-estimate", cpu_watts)));
    notes.push(format!("cpu: estimated at {cpu_watts} W"));
    (meters, notes)
}

/// Run the arena to completion.
pub fn run(cfg: Config, heldout: Vec<Corpus>, meters: Vec<Box<dyn Meter>>, meter_notes: Vec<String>) -> Result<Report, String> {
    let agents = build_agents(&cfg.agents)?;
    run_with(cfg, agents, heldout, meters, meter_notes)
}

/// [`run`], with the agents supplied rather than built from `cfg.agents`:
/// for agents the arena does not construct itself.
pub fn run_with(
    cfg: Config,
    mut agents: Vec<Box<dyn Proposer>>,
    heldout: Vec<Corpus>,
    mut meters: Vec<Box<dyn Meter>>,
    meter_notes: Vec<String>,
) -> Result<Report, String> {
    if agents.is_empty() {
        return Err("at least one agent is required".into());
    }
    if heldout.is_empty() {
        return Err("at least one held-out corpus is required — without one there is nothing to measure intelligence against".into());
    }
    if cfg.learners.is_empty() {
        return Err("at least one learner is required".into());
    }
    if cfg.heldout_slices == 0 {
        return Err("heldout_slices must be at least 1 (1 means no slicing)".into());
    }
    if !cfg.refresh_drift.is_finite() || cfg.refresh_drift < 0.0 {
        return Err(format!("refresh_drift must be finite and at least 0, not {}", cfg.refresh_drift));
    }
    let canary: Option<Vec<u8>> = cfg.canary.as_ref().map(|c| c.as_bytes().to_vec());
    let heldout: Vec<Corpus> = match &canary {
        Some(c) => heldout.into_iter().map(|h| h.with_canary(c)).collect(),
        None => heldout,
    };
    // Cut after the canary is planted, so a canary-bearing corpus is sliced like
    // any other. The canary check reads agents' outputs, never these slices.
    // `by_slice[k]` holds every corpus's slice `k`; with one slice it is the
    // corpora themselves. Too-small slices are refused here, before any work.
    let mut by_slice: Vec<Vec<Corpus>> = (0..cfg.heldout_slices).map(|_| Vec::new()).collect();
    for c in &heldout {
        for (k, s) in c.slices(cfg.heldout_slices)?.into_iter().enumerate() {
            by_slice[k].push(s);
        }
    }
    // The slice checkpoints score on, and the gap baseline it is judged against
    // (plan 7.5). Only used when there is more than one slice.
    let mut active = 0usize;
    let mut baseline: Option<f64> = None;
    let mut refreshes: Vec<Refresh> = Vec::new();
    // Set when an agent's output carries the canary; the run halts.
    let mut leaked: Option<String> = None;
    let mut standing: Vec<AgentStanding> = agents
        .iter()
        .map(|a| AgentStanding {
            id: a.id().to_string(),
            kind: a.kind().to_string(),
            role: a.role(),
            ..Default::default()
        })
        .collect();
    // The compression reward is a MAGE program (plan 5.1), gated at load.
    let reward_program = match (cfg.reward, &cfg.reward_program) {
        (Reward::Compression, Some(path)) => Some(crate::harness::RewardProgram::load(
            &std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        )?),
        (Reward::Compression, None) => Some(crate::harness::RewardProgram::load(crate::harness::DEFAULT_REWARD)?),
        (Reward::Alignment, _) => None,
    };
    let allocation_program = crate::harness::AllocationProgram::load(&match &cfg.allocation_program {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        None => crate::harness::DEFAULT_ALLOCATION.to_string(),
    })?;
    let retention_program = crate::harness::RetentionProgram::load(&match &cfg.retention_program {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        None => crate::harness::DEFAULT_RETENTION.to_string(),
    })?;
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
    // constants erased (`mage_prototype::canon`) — or, under
    // `NoveltyKey::Behaviour`, on its outputs, and a repeat earns
    // progress / (1 + times seen). It was counted on output bytes first, and
    // the generators found `[s; k]`: constant runs keyed on the seed, new bytes
    // every seed, one idea. Distinct outputs are still reported beside it.
    let mut novelty = Novelty::default();
    let mut attacks: Vec<Attack> = Vec::new();
    let mut attack_claims = 0usize;
    let mut outputs_seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut total_outputs = 0usize;
    let mut lz_sum = 0.0f64;
    // Results by (definition hash, seed): a program evaluated once for a seed
    // is never evaluated for it again, so a repeat costs no fuel.
    let mut cache = EvalCache::default();
    let mut run_credit = vec![0.0f64; agents.len()];
    let mut run_cost = vec![0.0f64; agents.len()];
    let mut calibration = CalibrationSuite::new();
    let mut ledger = EnergyLedger::new(cfg.energy_budget);
    let mut rounds_completed = 0usize;
    // The run's front of programs on (reward ↑, cost ↓), and the cost axis's
    // reference, fixed by the first round that produced any (plan 7.1b).
    let mut program_front: Vec<Vec<f64>> = Vec::new();
    let mut cost_reference: Option<f64> = None;

    for round in 0..cfg.rounds {
        if ledger.exhausted() {
            ledger.halted = Some(format!(
                "the energy budget of {:.1} J was spent after {round} of {} rounds",
                cfg.energy_budget.unwrap_or(0.0),
                cfg.rounds
            ));
            break;
        }
        // 1. Allocate.
        let scores: Vec<f64> = (0..agents.len())
            .map(|i| if run_cost[i] > 0.0 { run_credit[i] / run_cost[i] } else { 0.0 })
            .collect();
        let shares = allocation_program.allocate(cfg.programs_per_round, &scores, cfg.floor, cfg.temperature)?;

        let mut round_data: Vec<Vec<u8>> = Vec::new();
        // Per agent: its scored candidates, credit and cost this round.
        // Feedback waits until after training, because compression progress
        // cannot be known before it.
        let mut round_scored: Vec<Vec<Scored>> = (0..agents.len()).map(|_| Vec::new()).collect();
        let mut round_credit = vec![0.0f64; agents.len()];
        let mut round_cost = vec![0.0f64; agents.len()];
        // Each scored candidate's own cost, in the cost unit, in step with
        // `round_scored`.
        let mut round_costs: Vec<Vec<f64>> = (0..agents.len()).map(|_| Vec::new()).collect();
        // Compression mode: (agent, index into its scored list, bytes,
        // multiplier, bits/byte before training).
        // (agent, index, probe, [seen, tokens, structure], before)
        let mut pending: Vec<(usize, usize, Vec<u8>, [f64; 3], f64)> = Vec::new();
        // Every agent action is metered in its own span (plan 3.5), so each
        // agent is charged what it spent, and the kernel can stop one mid-round.
        let mut round_joules = vec![0.0f64; agents.len()];
        for (ai, agent) in agents.iter_mut().enumerate() {
            standing[ai].last_share = shares[ai];
            // 2. Propose.
            let span = Span::start(&mut meters);
            let cands: Vec<Candidate> = agent.propose(shares[ai], &pool, &mut rng);
            let (_, r) = span.stop(&mut meters);
            let role = agent.role().name();
            round_joules[ai] += ledger.charge_role(PROGRAMS, role, &r).map_or(0.0, |(j, _)| j);
            let allowance = cfg.joules_per_program.map(|p| p * shares[ai] as f64);
            let scored = &mut round_scored[ai];
            let mut credit = 0.0;
            let mut cost = 0.0;
            for c in cands {
                standing[ai].proposed += 1;
                // The kernel's limits, checked before any work is done: the
                // run's budget, then this agent's allowance for the round.
                if ledger.exhausted() || allowance.is_some_and(|a| round_joules[ai] >= a) {
                    *standing[ai].refusals.entry("budget".into()).or_insert(0) += 1;
                    *refusals.entry("budget".into()).or_insert(0) += 1;
                    cost += 1.0;
                    scored.push((c, None));
                    round_costs[ai].push(1.0);
                    continue;
                }
                let cost_at = cost;
                let span = Span::start(&mut meters);
                let fuel_at = cache.fuel_spent;
                // 3. Gate, then run — or reuse what an identical program
                // already produced for this seed.
                let seed = rng.next_u64();
                let (outcome, prepared) = match cfg.substrate.prepare(&c.program.source()) {
                    Err((why, msg)) => (Outcome::Refused(why, msg), None),
                    Ok(prepared) => {
                        let outcome = cache.run(&cfg.substrate, &prepared, seed);
                        (outcome, Some(prepared))
                    }
                };
                // The canary check comes before any credit or training: a
                // leaked output must not teach the learners anything.
                let outcome = match outcome {
                    Outcome::Bytes { bytes, .. } if canary.as_ref().is_some_and(|k| crate::kernel::leaks(&bytes, k)) => {
                        leaked = Some(format!(
                            "{}'s program output carried the held-out canary in round {round}: an agent has seen held-out data",
                            agent.id()
                        ));
                        Outcome::Refused(Refusal::Canary, "output carries the held-out canary".into())
                    }
                    other => other,
                };
                match outcome {
                    Outcome::Bytes { bytes, fuel_used } => {
                        // 4. Credit. Alignment is known now, before any
                        // learner trains on the output; compression progress
                        // only after.
                        let key = prepared.as_ref().map(|p| novelty_key(&cfg, &mut cache, p)).unwrap_or_default();
                        let seen = novelty.observe(&key);
                        // An attacker's candidate claims to behave as its
                        // parent. The kernel checks the claim; if it holds
                        // and novelty still counted the candidate as new,
                        // the reward has an exploit.
                        let parent = c.parents.first().and_then(|id| pool.entries.iter().find(|e| e.id == *id));
                        if let (Role::Attacker, Some(variant), Some(parent)) = (agent.role(), prepared.as_ref(), parent) {
                            if let Ok(parent_prepared) = cfg.substrate.prepare(&parent.program.source()) {
                                attack_claims += 1;
                                let seeds: Vec<u64> = (2..2 + ATTACK_SEEDS).map(|k| seed + k).collect();
                                if crate::kernel::verify_preserving(&cfg.substrate, &parent_prepared, variant, &seeds).is_ok()
                                    && seen == 0
                                    && novelty_key(&cfg, &mut cache, &parent_prepared) != key
                                {
                                    attacks.push(Attack {
                                        round,
                                        agent: agent.id().to_string(),
                                        parent: parent.id,
                                        source: c.program.source(),
                                        seeds_checked: seeds.len(),
                                    });
                                }
                            }
                        }
                        let tokens = c.program.token_len();
                        let mult = shaping(&cfg, tokens, &bytes) / (1.0 + seen as f64);
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
                                pool.add_with(
                                    c.program.clone(),
                                    bytes.clone(),
                                    r,
                                    agent.id(),
                                    round,
                                    round,
                                    &|r, age| retention_program.priority(r, age),
                                )?;
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
                                // Scored in one batch per learner below,
                                // before training.
                                let inputs = [seen as f64, tokens as f64, structure_factor(&cfg, &bytes)];
                                pending.push((ai, scored.len(), probe, inputs, 0.0));
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
                let (_, r) = span.stop(&mut meters);
                let charged = ledger.charge_role(PROGRAMS, role, &r);
                if let Some((j, measured)) = charged {
                    round_joules[ai] += j;
                    calibration.add_energy_sample(
                        PROGRAM_TARGET,
                        EnergySample { fuel: cache.fuel_spent - fuel_at, joules: j, measured },
                    );
                }
                round_costs[ai].push(match (cfg.cost_unit, charged) {
                    (CostUnit::Joules, Some((j, _))) if j > 0.0 => j,
                    _ => cost - cost_at,
                });
                if leaked.is_some() {
                    break;
                }
            }
            round_credit[ai] = credit;
            // Plan 7.1: charged the joules its own actions measured. Without
            // any reading (no meter produced a figure), fuel stands in.
            round_cost[ai] = match cfg.cost_unit {
                CostUnit::Joules if round_joules[ai] > 0.0 => round_joules[ai],
                _ => cost,
            };
            if cfg.cost_unit == CostUnit::Joules {
                standing[ai].joules += round_joules[ai];
            }
            if leaked.is_some() {
                break;
            }
        }
        // A leak ends the run before the round trains on anything.
        if let Some(why) = &leaked {
            ledger.halted = Some(why.clone());
            break;
        }

        // Replay: revisit what taught the most, so learners do not forget it.
        for _ in 0..cfg.replay_per_round {
            if let Some(e) = pool.sample(&mut rng) {
                round_data.push(e.bytes.clone());
            }
        }

        // Compression mode: every probe's bits/byte before training, batched.
        if !pending.is_empty() {
            let span = Span::start(&mut meters);
            let probes: Vec<&[u8]> = pending.iter().map(|p| p.2.as_slice()).collect();
            let before = mean_bpb_many(&learners, &probes);
            for (p, b) in pending.iter_mut().zip(before) {
                p.4 = b;
            }
            let (_, r) = span.stop(&mut meters);
            ledger.charge(PROBES, &r);
        }

        // 5. Train, each learner in its own energy span.
        let batch: Vec<&[u8]> = round_data.iter().map(|b| b.as_slice()).collect();
        for (li, learner) in learners.iter_mut().enumerate() {
            // A spent budget stops training too, mid-round.
            if ledger.exhausted() {
                break;
            }
            let span = Span::start(&mut meters);
            for _ in 0..cfg.learner_steps {
                learner.train(&batch);
            }
            let (secs, readings) = span.stop(&mut meters);
            ledger.charge(TRAINING, &readings);
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
        let after_all = {
            let span = Span::start(&mut meters);
            let probes: Vec<&[u8]> = pending.iter().map(|p| p.2.as_slice()).collect();
            let after = mean_bpb_many(&learners, &probes);
            let (_, r) = span.stop(&mut meters);
            ledger.charge(PROBES, &r);
            after
        };
        for ((ai, idx, probe, [seen, tokens, structure], before), after) in pending.into_iter().zip(after_all) {
            let program = reward_program.as_ref().expect("compression mode loads a reward program");
            let r = program
                .eval(&crate::harness::RewardInputs {
                    before,
                    after,
                    len: probe.len() as f64,
                    seen,
                    tokens,
                    charge: cfg.length_charge,
                    structure,
                })
                .map_err(|e| format!("round {round}: {e}"))?;
            standing[ai].credit += r;
            round_credit[ai] += r;
            let program = round_scored[ai][idx].0.program.clone();
            pool.add_with(program, probe, r, agents[ai].id(), round, round, &|r, age| retention_program.priority(r, age))?;
            round_scored[ai][idx].1 = Some(r);
        }

        // Every program's point on (reward ↑, cost ↓), and what each agent's
        // added to the front as it stood before the round (plan 7.1b).
        let points: Vec<Vec<Vec<f64>>> = (0..agents.len())
            .map(|ai| {
                round_scored[ai]
                    .iter()
                    .zip(&round_costs[ai])
                    .filter_map(|((_, r), c)| r.map(|r| vec![r, *c]))
                    .collect()
            })
            .collect();
        if cost_reference.is_none() {
            let costliest = points.iter().flatten().map(|p| p[1]).fold(0.0f64, f64::max);
            if costliest > 0.0 {
                cost_reference = Some(4.0 * costliest);
            }
        }
        if let Some(cost_ref) = cost_reference {
            let reference = [0.0, cost_ref];
            for ai in 0..agents.len() {
                let added = marginal_hypervolume(&program_front, &points[ai], &reference);
                standing[ai].hypervolume_added += added;
                if cfg.allocation == Allocation::Hypervolume {
                    round_credit[ai] = added;
                }
            }
            let mut all = std::mem::take(&mut program_front);
            all.extend(points.into_iter().flatten());
            program_front = non_dominated(all);
        } else if cfg.allocation == Allocation::Hypervolume {
            // Nothing produced yet, so nothing added.
            round_credit.iter_mut().for_each(|c| *c = 0.0);
        }

        // 6. Feed back, now that every credit is known.
        for (ai, agent) in agents.iter_mut().enumerate() {
            agent.feedback(&round_scored[ai]);
            run_credit[ai] = cfg.credit_decay * run_credit[ai] + round_credit[ai];
            run_cost[ai] = cfg.credit_decay * run_cost[ai] + round_cost[ai];
        }

        if (round + 1) % cfg.eval_every == 0 || round + 1 == cfg.rounds {
            // Checkpoints are the epoch boundaries (plan 7.5): the active slice
            // is fixed between two of them, and a rotation decided here takes
            // effect from the next checkpoint on.
            let proxy_bytes: Vec<u8> = pool.entries.iter().flat_map(|e| e.bytes.iter().cloned()).take(1 << 16).collect();
            // Every learner is scored on the active slice of every corpus.
            let active_set = &by_slice[active];
            let mut measured: Vec<(BTreeMap<String, f64>, f64, f64)> = Vec::with_capacity(learners.len());
            for learner in learners.iter() {
                let span = Span::start(&mut meters);
                let heldout_bpb: BTreeMap<String, f64> =
                    active_set.iter().map(|c| (c.name().to_string(), c.score(learner.as_ref()))).collect();
                let proxy_bpb = learner.bits_per_byte(&proxy_bytes);
                let (_, r) = span.stop(&mut meters);
                ledger.evaluate(&r);
                let mean_bpb = heldout_bpb.values().sum::<f64>() / heldout_bpb.len() as f64;
                measured.push((heldout_bpb, mean_bpb, proxy_bpb));
            }
            // Refresh (plan 7.5): the first checkpoint on a slice sets the gap
            // baseline; a later one whose mean gap drifted past the threshold
            // rotates to the next slice and clears the baseline.
            if cfg.heldout_slices > 1 {
                let mean_gap = measured.iter().map(|(_, m, p)| m - p).sum::<f64>() / measured.len() as f64;
                match baseline {
                    None => baseline = Some(mean_gap),
                    Some(base) if (mean_gap - base).abs() > cfg.refresh_drift => {
                        let to = (active + 1) % cfg.heldout_slices;
                        eprintln!(
                            "arena: round {:>4}/{} held-out slice {}/{} -> {}/{}: gap {:+.3} drifted {:.3} from its baseline {:+.3}",
                            round + 1,
                            cfg.rounds,
                            active + 1,
                            cfg.heldout_slices,
                            to + 1,
                            cfg.heldout_slices,
                            mean_gap,
                            (mean_gap - base).abs(),
                            base
                        );
                        refreshes.push(Refresh {
                            round: round + 1,
                            from_slice: active + 1,
                            to_slice: to + 1,
                            gap: mean_gap,
                            baseline: base,
                        });
                        active = to;
                        baseline = None;
                    }
                    Some(_) => {}
                }
            }
            for (li, (heldout_bpb, mean_bpb, proxy_bpb)) in measured.into_iter().enumerate() {
                // Progress on stderr: a 25M-parameter run takes most of an
                // hour, and the first one printed nothing until it finished.
                eprintln!(
                    "arena: round {:>4}/{} learner {li} held-out {mean_bpb:.3} bits/byte (gap {:+.3} over the pool), {} J, {:.0} s",
                    round + 1,
                    cfg.rounds,
                    mean_bpb - proxy_bpb,
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
                    bytes_seen: learners[li].bytes_seen(),
                    proxy_bpb,
                    gap: mean_bpb - proxy_bpb,
                });
            }
        }
        rounds_completed = round + 1;
    }

    for (ai, agent) in agents.iter().enumerate() {
        standing[ai].score = if run_cost[ai] > 0.0 { run_credit[ai] / run_cost[ai] } else { 0.0 };
        standing[ai].state = agent.describe();
    }

    // The learners' front: (bits/byte ↓, joules ↓, latency ↓).
    let mut results = Vec::new();
    let pool_bytes: Vec<u8> = pool.entries.iter().flat_map(|e| e.bytes.iter().cloned()).take(1 << 16).collect();
    for (li, learner) in learners.iter().enumerate() {
        let mean_bpb = heldout.iter().map(|c| c.score(learner.as_ref())).sum::<f64>() / heldout.len() as f64;
        let latency = heldout[0].latency_s_per_kb(learner.as_ref(), 4096);
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

    // Addressed by canonical hash, so a renamed copy of a program is the
    // same definition here, in forge and in germline's journal.
    let best_programs: Vec<(String, f64, String, String)> = pool
        .top(5)
        .into_iter()
        .map(|e| {
            let src = e.program.source();
            let exact = cfg.substrate.prepare(&src).map(|p| p.exact).unwrap_or_default();
            (e.agent.clone(), e.reward, src, exact)
        })
        .collect();
    if let Some(dir) = &cfg.definitions_dir {
        let store = forge::registry::DefinitionStore::new(dir);
        for (_, _, src, _) in &best_programs {
            store.publish(src, &cfg.substrate)?;
        }
    }
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
        fuel_calibration: calibration.fuel_calibration(PROGRAM_TARGET),
        energy: ledger,
        rounds_completed,
        attacks,
        attack_claims,
        refreshes,
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

    /// Writes one program whose output is `bytes`: an agent that has seen
    /// held-out data and repeats it.
    struct Leaker(Vec<u8>);
    impl Proposer for Leaker {
        fn id(&self) -> &str {
            "leaker"
        }
        fn kind(&self) -> &'static str {
            "leaker"
        }
        fn propose(&mut self, n: usize, _: &Pool, _: &mut Rng) -> Vec<Candidate> {
            use crate::grammar::{IntExpr, ListExpr, Program};
            let body = ListExpr::Cycle("v".into(), self.0.iter().map(|b| IntExpr::Lit(*b as u64)).collect(), self.0.len() as u64);
            (0..n).map(|_| Candidate { program: Program { body: body.clone() }, trace: None, parents: vec![] }).collect()
        }
        fn feedback(&mut self, _: &[Scored]) {}
    }

    #[test]
    fn an_output_carrying_the_held_out_canary_halts_the_run_before_it_trains() {
        const CANARY: &str = "MAGE-CANARY-7f3a";
        let cfg = || Config { canary: Some(CANARY.into()), ..tiny_cfg() };
        let (m, n) = cpu_only();
        let r = run_with(cfg(), vec![Box::new(Leaker(CANARY.as_bytes().to_vec()))], heldout(), m, n).expect("runs");
        assert!(r.energy.halted.as_deref().is_some_and(|h| h.contains("canary") && h.contains("leaker")), "{:?}", r.energy.halted);
        assert_eq!(r.refusals.get("canary"), Some(&1), "the first leak stops the agent: {:?}", r.refusals);
        assert_eq!(r.rounds_completed, 0);
        assert_eq!(r.energy.account(TRAINING), 0.0, "nothing was trained on");
        assert_eq!(r.pool_size, 0, "and nothing entered the pool");
        // Without the canary planted, the same agent is just an agent.
        let (m, n) = cpu_only();
        let plain = run_with(tiny_cfg(), vec![Box::new(Leaker(CANARY.as_bytes().to_vec()))], heldout(), m, n).expect("runs");
        assert_eq!(plain.energy.halted, None);
        assert_eq!(plain.rounds_completed, tiny_cfg().rounds);
    }

    #[test]
    fn the_attacker_finds_that_shape_novelty_credits_output_preserving_rewrites() {
        let cfg = |novelty| Config {
            agents: vec![("grammar".into(), 1), ("attacker".into(), 1)],
            novelty,
            ..tiny_cfg()
        };
        let (m, n) = cpu_only();
        let shape = run(cfg(NoveltyKey::Shape), heldout(), m, n).expect("runs");
        assert!(shape.attack_claims > 0, "the attacker made no claims");
        assert!(!shape.attacks.is_empty(), "shape keying credits `xs.reverse().reverse()` as new");
        for a in &shape.attacks {
            assert_eq!(a.agent, "attacker-0");
            assert_eq!(a.seeds_checked, ATTACK_SEEDS as usize);
        }
        // Keyed on behaviour, the same rewrites are their parents.
        let (m, n) = cpu_only();
        let behaviour = run(cfg(NoveltyKey::Behaviour), heldout(), m, n).expect("runs");
        assert!(behaviour.attack_claims > 0, "the attacker made no claims");
        assert!(behaviour.attacks.is_empty(), "{:?}", behaviour.attacks);
    }

    #[test]
    fn the_kernel_refutes_a_rewrite_that_changes_output() {
        use crate::grammar::{ListExpr, Program};
        let sub = tiny_cfg().substrate;
        let parent = Program { body: ListExpr::Range(9) };
        let reversed = Program { body: ListExpr::Reverse(Box::new(ListExpr::Range(9))) };
        let twice = Program { body: crate::agents::identity_rewrite(&parent.body, 0) };
        let p = sub.prepare(&parent.source()).unwrap();
        assert!(crate::kernel::verify_preserving(&sub, &p, &sub.prepare(&reversed.source()).unwrap(), &[1, 2]).is_err());
        assert_eq!(crate::kernel::verify_preserving(&sub, &p, &sub.prepare(&twice.source()).unwrap(), &[1, 2]), Ok(()));
    }

    #[test]
    fn every_checkpoint_records_the_gap_between_held_out_and_proxy() {
        let (m, n) = cpu_only();
        let r = run(tiny_cfg(), heldout(), m, n).expect("runs");
        assert!(!r.checkpoints.is_empty());
        for c in &r.checkpoints {
            assert!(c.proxy_bpb.is_finite() && c.proxy_bpb >= 0.0, "{c:?}");
            assert!((c.gap - (c.mean_bpb - c.proxy_bpb)).abs() < 1e-12);
        }
    }

    #[test]
    fn agents_act_in_roles_and_the_ledger_accounts_their_energy_by_role() {
        let (m, n) = cpu_only();
        let r = run(tiny_cfg(), heldout(), m, n).expect("runs");
        let role = |id: &str| r.agents.iter().find(|a| a.id.starts_with(id)).map(|a| a.role);
        assert_eq!(role("grammar"), Some(crate::agents::Role::Proposer));
        assert_eq!(role("uniform"), Some(crate::agents::Role::Proposer));
        assert_eq!(role("mutator"), Some(crate::agents::Role::Mutator));
        // Every agent joule is attributed to a role, and only agents' spans
        // are: training and probes stay out of it.
        let by_role: f64 = r.energy.by_role.values().sum();
        assert!((by_role - r.energy.account(PROGRAMS)).abs() < 1e-6, "{by_role} vs {}", r.energy.account(PROGRAMS));
        assert!(r.energy.by_role.keys().all(|k| k == "proposer" || k == "mutator"), "{:?}", r.energy.by_role);
        assert!(r.energy.by_role.get("mutator").is_some_and(|j| *j > 0.0));
    }

    #[test]
    fn hypervolume_allocation_credits_new_trade_offs_and_still_spends_the_budget() {
        let (m, n) = cpu_only();
        let r = run(Config { allocation: Allocation::Hypervolume, ..tiny_cfg() }, heldout(), m, n).expect("runs");
        let proposed: usize = r.agents.iter().map(|a| a.proposed).sum();
        assert_eq!(proposed, 6 * 16, "the budget is spent exactly");
        assert!(r.agents.iter().any(|a| a.hypervolume_added > 0.0), "{:?}", r.agents);
        assert!(r.agents.iter().all(|a| a.hypervolume_added.is_finite() && a.hypervolume_added >= 0.0));
        // The score that divided the budget is hypervolume per unit cost, so
        // an agent that added none scores zero.
        for a in &r.agents {
            if a.hypervolume_added == 0.0 {
                assert_eq!(a.score, 0.0, "{}", a.id);
            }
        }
        // Reported under the default allocation too, without driving it.
        let (m, n) = cpu_only();
        let share = run(tiny_cfg(), heldout(), m, n).expect("runs");
        assert!(share.agents.iter().any(|a| a.hypervolume_added > 0.0));
    }

    /// A hardware counter that reads exactly 1 J per span, so a run's
    /// calibration is deterministic.
    struct OneJoule;
    impl Meter for OneJoule {
        fn name(&self) -> String {
            "one-joule".into()
        }
        fn start(&mut self) {}
        fn stop(&mut self) -> Reading {
            Reading { meter: self.name(), joules: Some(1.0), basis: energy::Basis::Measured }
        }
    }

    #[test]
    fn every_program_feeds_the_fuel_calibration_from_its_meters() {
        // Plan 3.2: samples come from the meters, one per program run. With a
        // meter that reads 1 J whatever happens, fuel explains nothing: the
        // fit's overhead is the 1 J and its slope is 0.
        let r = run(tiny_cfg(), heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        let c = r.fuel_calibration.expect("programs of varying fuel calibrate");
        assert_eq!(c.samples, 6 * 16, "one sample per program");
        assert!(c.all_measured);
        assert!(c.joules_per_fuel.abs() < 1e-12 && (c.overhead_joules - 1.0).abs() < 1e-9, "{c:?}");
        assert_eq!(c.r_squared, 0.0);

        // A wall-clock meter is an estimate, and the calibration says so.
        let e = run(tiny_cfg(), heldout(), cpu_only().0, vec![]).unwrap();
        assert!(!e.fuel_calibration.expect("calibrates").all_measured);
    }

    #[test]
    fn agents_are_charged_every_measured_joule_of_the_program_phase() {
        // One joule per span, and a span per proposal and per program: agents
        // are charged exactly what the ledger's program account holds.
        let r = run(tiny_cfg(), heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        let charged: f64 = r.agents.iter().map(|a| a.joules).sum();
        assert_eq!(charged, r.energy.account(PROGRAMS));
        assert_eq!(charged, (6 * 16 + 6 * 3) as f64, "a proposal and 16 programs a round, three agents");
        assert!(r.agents.iter().all(|a| a.joules > 0.0));
        assert!(r.energy.account(TRAINING) > 0.0 && r.energy.account(PROBES) > 0.0);
        assert!(r.energy.evaluation_joules > 0.0, "evaluation is metered");
        assert_eq!(r.energy.total(), r.energy.charged.values().sum::<f64>(), "and not charged");

        let f = run(Config { cost_unit: CostUnit::Fuel, ..tiny_cfg() }, heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        assert!(f.agents.iter().all(|a| a.joules == 0.0), "fuel mode charges no joules");
    }

    #[test]
    fn an_agent_over_its_allowance_is_stopped_mid_round() {
        // 1 J per span; an allowance of 0.8 J per program granted. An agent
        // granted k programs spends 1 J proposing and 1 J per program, so the
        // kernel refuses the rest once it reaches 0.8k: after 12 of 16.
        let capped = Config { joules_per_program: Some(0.8), ..tiny_cfg() };
        let r = run(capped, heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        let stopped = r.refusals.get("budget").copied().unwrap_or(0);
        assert!(stopped > 0, "{:?}", r.refusals);
        let proposed: usize = r.agents.iter().map(|a| a.proposed).sum();
        assert_eq!(proposed, 6 * 16, "stopped programs are still counted as proposed");
        // Unit budget refusals cost nothing to run: every agent's charge is
        // its proposals plus the programs it was allowed.
        for a in &r.agents {
            let refused = a.refusals.get("budget").copied().unwrap_or(0);
            assert_eq!(a.joules, (6 + a.proposed - refused) as f64, "{}", a.id);
        }

        let free = run(tiny_cfg(), heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        assert!(!free.refusals.contains_key("budget"), "no allowance, no refusals");
    }

    #[test]
    fn the_run_stops_when_its_energy_budget_is_spent() {
        let r = run(Config { energy_budget: Some(40.0), ..tiny_cfg() }, heldout(), vec![Box::new(OneJoule)], vec![])
            .unwrap();
        assert!(r.rounds_completed < 6, "{}", r.rounds_completed);
        assert!(r.energy.halted.as_deref().is_some_and(|h| h.contains("40.0 J")), "{:?}", r.energy.halted);
        // The check runs before every program and training step, so the
        // overshoot is at most the tail of one round (probe scoring).
        assert!(r.energy.total() >= 40.0 && r.energy.total() < 44.0, "{}", r.energy.total());
        assert!(r.refusals.get("budget").copied().unwrap_or(0) > 0, "stopped mid-round");
    }

    #[test]
    fn the_best_programs_are_published_to_forge_by_canonical_hash() {
        let dir = std::env::temp_dir().join(format!("arena-defs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config { definitions_dir: Some(dir.to_string_lossy().into()), ..tiny_cfg() };
        let r = run(cfg, heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        let store = forge::registry::DefinitionStore::new(&dir);
        assert!(!r.best_programs.is_empty());
        for (_, _, src, exact) in &r.best_programs {
            assert_eq!(exact.len(), 64, "{exact}");
            // Stored under its address; an alpha-equivalent program may have
            // been stored first, with its own source.
            let stored = store.get(exact).expect("published and verified");
            let sub = Substrate { fuel: 50_000, max_bytes: 128 };
            assert_eq!(sub.prepare(&stored).unwrap().exact, sub.prepare(src).unwrap().exact);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_renamed_program_is_the_same_definition_in_forge() {
        use forge::registry::Canonicalizer;
        let sub = Substrate { fuel: 10_000, max_bytes: 64 };
        let a = sub.canonicalize("@role(candidate)\nf gen(s: usize) -> [usize] { range(s).map(|x| x + 1) }").unwrap();
        let b = sub.canonicalize("@role(candidate)\nf gen(n: usize) -> [usize] { range(n).map(|y| y + 1) }").unwrap();
        let c = sub.canonicalize("@role(candidate)\nf gen(n: usize) -> [usize] { range(n).map(|y| y + 2) }").unwrap();
        assert_eq!(a, b);
        assert_ne!(a.exact, c.exact);
        assert_eq!(a.shape, c.shape);
        assert!(sub.canonicalize("f gen(s: usize) -> [usize] { range(s) }").is_err(), "no role, no address");
    }

    #[test]
    fn an_empty_gpu_list_meters_no_gpu() {
        let (m, notes) = meters_for(65.0, Some(&[]));
        assert_eq!(m.iter().map(|m| m.name()).collect::<Vec<_>>(), vec!["cpu-estimate".to_string()]);
        assert!(notes[0].contains("no devices requested"), "{notes:?}");
    }

    #[test]
    fn fuel_spent_counts_exhausted_budgets_and_not_cache_hits() {
        let sub = Substrate { fuel: 2_000, max_bytes: 64 };
        let spin = sub
            .prepare("@role(candidate)\nf gen(s: usize) -> [usize] { m i = 0\n @w 1b { i = i + 1 }\n [i] }")
            .expect("valid");
        let mut cache = EvalCache::default();
        assert!(matches!(cache.run(&sub, &spin, 1), Outcome::Refused(Refusal::Fuel, _)));
        assert_eq!(cache.fuel_spent, 2_000, "an exhausted budget was spent in full");
        cache.run(&sub, &spin, 1);
        assert_eq!(cache.fuel_spent, 2_000, "a cache hit spends nothing");
    }

    #[test]
    fn the_run_is_reproducible_from_its_seed() {
        // Under a deterministic meter. Agents are charged in measured joules
        // (plan 7.1), so with a real meter the allocation follows what it read.
        let a = run(tiny_cfg(), heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
        let b = run(tiny_cfg(), heldout(), vec![Box::new(OneJoule)], vec![]).unwrap();
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

    #[test]
    fn new_configs_key_novelty_on_behaviour_and_old_records_on_shape() {
        assert_eq!(Config::default().novelty, NoveltyKey::Behaviour);
        // A config recorded before plan 7.4 has no `novelty` field. It ran
        // keyed on shape, and must still read that way.
        let mut recorded = serde_json::to_value(Config::default()).expect("serialises");
        recorded.as_object_mut().expect("an object").remove("novelty");
        let read: Config = serde_json::from_value(recorded).expect("deserialises");
        assert_eq!(read.novelty, NoveltyKey::Shape);
    }

    /// Held-out text large enough for two slices of at least 1 KiB (plan 7.5).
    fn sliceable() -> Vec<Corpus> {
        vec![Corpus::new("text", b"the quick brown fox jumps over the lazy dog. ".repeat(60))]
    }

    /// A slice's name as the checkpoints key it, with `k` slices in all.
    fn slice_key(k: usize, slice: usize) -> String {
        format!("text#{slice}/{k}")
    }

    /// The corpus names a checkpoint scored on.
    fn keys(c: &Checkpoint) -> Vec<String> {
        c.heldout_bpb.keys().cloned().collect()
    }

    #[test]
    fn one_slice_keeps_the_plain_names_and_never_rotates() {
        let (m, n) = cpu_only();
        let r0 = run(tiny_cfg(), heldout(), m, n).expect("runs");
        assert!(r0.refreshes.is_empty());
        assert!(!r0.checkpoints.is_empty());
        for c in &r0.checkpoints {
            assert_eq!(keys(c), vec!["text".to_string()], "{c:?}");
        }
        // Explicit one-slice and zero-drift settings are the same run: the
        // rotation rule is off without a second slice, whatever the drift.
        // Both runs use a constant meter, so cost-driven allocation (and with
        // it, the programs) is deterministic and the figures can be compared.
        let fixed = || vec![Box::new(OneJoule) as Box<dyn Meter>];
        let r = run(tiny_cfg(), heldout(), fixed(), vec![]).expect("runs");
        let one = Config { heldout_slices: 1, refresh_drift: 0.0, ..tiny_cfg() };
        let s = run(one, heldout(), fixed(), vec![]).expect("runs");
        assert!(s.refreshes.is_empty());
        assert_eq!(s.checkpoints.len(), r.checkpoints.len());
        for (a, b) in r.checkpoints.iter().zip(&s.checkpoints) {
            assert_eq!((a.round, a.learner), (b.round, b.learner));
            assert_eq!(a.heldout_bpb, b.heldout_bpb, "identical held-out figures");
            assert_eq!(a.proxy_bpb, b.proxy_bpb);
        }
        // A config recorded before the fields existed reads as one slice.
        let mut recorded = serde_json::to_value(Config::default()).expect("serialises");
        let obj = recorded.as_object_mut().expect("an object");
        obj.remove("heldout_slices");
        obj.remove("refresh_drift");
        let read: Config = serde_json::from_value(recorded).expect("deserialises");
        assert_eq!((read.heldout_slices, read.refresh_drift), (1, 0.5));
    }

    #[test]
    fn checkpoints_score_the_active_slice_and_rotate_on_any_drift() {
        let (m, n) = cpu_only();
        let cfg = Config { heldout_slices: 2, refresh_drift: 0.0, rounds: 9, eval_every: 3, ..tiny_cfg() };
        let r = run(cfg, sliceable(), m, n).expect("runs");
        assert!(!r.refreshes.is_empty(), "a zero drift rotates once the gap has moved: {:?}", r.checkpoints);
        let first = &r.refreshes[0];
        assert_eq!((first.from_slice, first.to_slice), (1, 2), "{first:?}");
        assert!((first.gap - first.baseline).abs() > 0.0, "{first:?}");
        // The slice a checkpoint is scored on: the one active before any
        // rotation decided at an earlier checkpoint.
        let active_at = |round: usize| r.refreshes.iter().filter(|f| f.round < round).last().map_or(1, |f| f.to_slice);
        for c in &r.checkpoints {
            let want = slice_key(2, active_at(c.round));
            assert_eq!(keys(c), vec![want.clone()], "round {}", c.round);
            assert!(c.mean_bpb.is_finite());
        }
        assert!(r.checkpoints.iter().any(|c| c.round > first.round && c.heldout_bpb.contains_key(&slice_key(2, 2))));
        // Rotations are recorded at checkpoint rounds, the epoch boundaries.
        assert!(r.refreshes.iter().all(|f| f.round % 3 == 0));
    }

    #[test]
    fn a_large_drift_threshold_never_rotates() {
        let (m, n) = cpu_only();
        let cfg = Config { heldout_slices: 2, refresh_drift: 1e9, rounds: 9, eval_every: 3, ..tiny_cfg() };
        let r = run(cfg, sliceable(), m, n).expect("runs");
        assert!(r.refreshes.is_empty());
        assert!(r.checkpoints.iter().all(|c| keys(c) == vec![slice_key(2, 1)]));
    }

    #[test]
    fn a_slice_under_the_minimum_is_refused_before_any_run() {
        let (m, n) = cpu_only();
        // 900 bytes into two slices is 450 bytes each.
        let err = run(Config { heldout_slices: 2, ..tiny_cfg() }, heldout(), m, n).expect_err("refused");
        assert!(err.contains("1024"), "{err}");
        let (m, n) = cpu_only();
        assert!(run(Config { heldout_slices: 0, ..tiny_cfg() }, heldout(), m, n).is_err(), "zero slices");
        let (m, n) = cpu_only();
        let bad = Config { heldout_slices: 2, refresh_drift: f64::NAN, ..tiny_cfg() };
        assert!(run(bad, sliceable(), m, n).is_err(), "a NaN drift is refused");
    }
}
