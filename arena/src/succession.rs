//! Succession: arena sessions as germline candidates.
//!
//! Plan tasks 8.1–8.2. The arena makes learners better; nothing in it gives a
//! better learner *authority*. The redesign's rule is that authority changes
//! hands only through `germline`'s gate (`Episode::adjudicate`), so this module
//! implements germline's [`Workload`] with the arena:
//!
//! * a candidate's **genome** is the arena's configuration — learning rate,
//!   optimiser steps per round, the description-length charge, and the budget
//!   allocation's temperature — each a `Locus::Param` in `[0,1]` decoded onto
//!   its range;
//! * **materialize** runs an arena session with that configuration and keeps
//!   its report under a digest of the configuration and the result;
//! * **evaluate** scores it on the **registered held-out suite**: `capability`
//!   is `1 − bits/byte ÷ 8` (a uniform model scores 0), and `efficiency` is
//!   `1 / (1 + kJ)`. With `capability` as the gate's primary axis and
//!   `efficiency` as its guard, a successor must predict held-out data better
//!   without costing materially more energy — intelligence per joule, in the
//!   gate's own terms.
//!
//! The suite is checked, not trusted: `evaluate` refuses a suite whose digest
//! is not the digest of the corpora this workload holds, so a candidate cannot
//! be scored on data nobody registered.

use crate::arena::{refuse_text_overlap, refuse_train_text_overlap, run, Config, Corpus, Report, Reward};
use crate::energy::{Basis, Meter, WallClock};
use crate::learner::{LearnerConfig, LearnerSpec};
use germline::attest::Attestor;
use germline::directed::CandidateSpec;
use germline::gate::{DominanceRule, Episode, PromotionGate};
use germline::journal::Journal;
use germline::lineage::Lineage;
use germline::runner::{Halt, Runner, RunnerPolicy, Workload};
use germline::supervisor::HealthSample;
use germline::variation::{Genome, Locus, ParamSearch, Selection, VariationPlan};
use germline::{EvalSuite, FitnessVector, Generation, Measurement};
use ribosome::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// The genome's loci, in order, and the ranges they decode onto.
pub const LOCI: [&str; 4] = ["lr", "learner_steps", "length_charge", "temperature"];

/// Decode a genome onto an arena configuration, starting from `base`.
/// Missing or non-scalar loci leave the base value.
pub fn decode(base: &Config, genome: &Genome) -> Config {
    let p = |i: usize| genome.get(i).and_then(Locus::as_param).map(|v| v.clamp(0.0, 1.0));
    let mut cfg = base.clone();
    if let Some(v) = p(0) {
        // Log-uniform 1e-3 .. 1e-1.
        let lr = 10f64.powf(-3.0 + 2.0 * v);
        cfg.learners = cfg
            .learners
            .iter()
            .map(|s| match *s {
                LearnerSpec::Ngram(c) => LearnerSpec::Ngram(LearnerConfig { lr, ..c }),
                LearnerSpec::Transformer(c) => LearnerSpec::Transformer(crate::learner::TransformerConfig { lr, ..c }),
                LearnerSpec::Entropy(c) => LearnerSpec::Entropy(crate::learner::EntropyConfig { lr, ..c }),
                LearnerSpec::Ssm(c) => LearnerSpec::Ssm(crate::learner::SsmConfig { lr, ..c }),
            })
            .collect();
    }
    if let Some(v) = p(1) {
        cfg.learner_steps = 1 + (v * 15.0).round() as usize;
    }
    if let Some(v) = p(2) {
        cfg.length_charge = 0.01 * v;
    }
    if let Some(v) = p(3) {
        cfg.temperature = 0.1 + 1.9 * v;
    }
    // A genome longer than the four narrow loci is a wide one: its extra loci
    // are read as `WIDE_LOCI`, and a train-text locus after them only if present.
    if genome.len() >= NARROW_LOCI + WIDE_LOCI.len() {
        apply_wide(&mut cfg, genome);
    }
    cfg
}

/// Loci of the narrow genome; the wide genome's first four, unchanged.
pub const NARROW_LOCI: usize = 4;

/// The loci a wide genome appends after the narrow four, in order. The agent
/// loci decode to a count `0..=3` (bins of 1/4); `learner_kind` bins over
/// n-gram / entropy / ssm (thirds); `learner_size` is n-gram `log2_buckets`
/// 10..=14, entropy `log2_buckets` 12..=15 or ssm `d` in {8, 16, 32};
/// `replay_per_round` is `0..=32`. A further `train_text_per_round` locus
/// (`0..=16`) follows only when the base configuration has `--train-text`.
pub const WIDE_LOCI: [&str; 6] =
    ["agents_grammar", "agents_uniform", "agents_mutator", "learner_kind", "learner_size", "replay_per_round"];

/// The wide genome's optional last locus.
pub const TRAIN_TEXT_LOCUS: &str = "train_text_per_round";

/// The agents the wide genome chooses counts for.
const WIDE_AGENTS: [&str; 3] = ["grammar", "uniform", "mutator"];

/// Which of `n` equal bins of `[0,1]` holds `v`; 1.0 falls in the last.
fn bin(v: f64, n: usize) -> usize {
    ((v.clamp(0.0, 1.0) * n as f64).floor() as usize).min(n - 1)
}

/// The middle of bin `i` of `n`, so `bin(bin_mid(i, n), n) == i`.
fn bin_mid(i: usize, n: usize) -> f64 {
    (i.min(n - 1) as f64 + 0.5) / n as f64
}

fn lr_of(s: &LearnerSpec) -> f64 {
    match s {
        LearnerSpec::Ngram(c) => c.lr,
        LearnerSpec::Transformer(c) => c.lr,
        LearnerSpec::Entropy(c) => c.lr,
        LearnerSpec::Ssm(c) => c.lr,
    }
}

/// Read the wide loci (index `NARROW_LOCI..`) of `genome` onto `cfg`.
fn apply_wide(cfg: &mut Config, genome: &Genome) {
    let p = |i: usize| genome.get(NARROW_LOCI + i).and_then(Locus::as_param).map(|v| v.clamp(0.0, 1.0)).unwrap_or(0.0);
    // Agent mix: counts for the three chosen agents, then the base's other agents unchanged.
    let mut agents: Vec<(String, usize)> = Vec::new();
    for (i, name) in WIDE_AGENTS.iter().enumerate() {
        let n = bin(p(i), 4);
        if n > 0 {
            agents.push((name.to_string(), n));
        }
    }
    if agents.is_empty() {
        agents.push(("grammar".to_string(), 1));
    }
    agents.extend(cfg.agents.iter().filter(|(k, _)| !WIDE_AGENTS.contains(&k.as_str())).cloned());
    cfg.agents = agents;
    // The first learner (the one the gate scores): kind and size, lr kept.
    let lr = cfg.learners.first().map(lr_of).unwrap_or(0.02);
    let size = p(4);
    let spec = match bin(p(3), 3) {
        0 => LearnerSpec::Ngram(LearnerConfig { orders: 3, log2_buckets: 10 + (size * 4.0).round() as u32, lr }),
        1 => LearnerSpec::Entropy(crate::learner::EntropyConfig {
            orders: 4,
            log2_buckets: 12 + (size * 3.0).round() as u32,
            target_len: 4.0,
            lr,
        }),
        _ => LearnerSpec::Ssm(crate::learner::SsmConfig {
            d: [8, 16, 32][bin(size, 3)],
            lr,
            bptt: crate::ssm::DEFAULT_BPTT,
        }),
    };
    match cfg.learners.first_mut() {
        Some(first) => *first = spec,
        None => cfg.learners.push(spec),
    }
    cfg.replay_per_round = (p(5) * 32.0).round() as usize;
    if genome.len() > NARROW_LOCI + WIDE_LOCI.len() {
        cfg.train_text_per_round = (p(6) * 16.0).round() as usize;
    }
}

/// The wide genome of a configuration: [`encode`]'s four loci, then
/// [`WIDE_LOCI`], then the train-text locus when `with_train_text`. The inverse
/// of [`decode`] on what the wide genome can express: counts above 3 clamp to 3,
/// a learner of another kind (a transformer) encodes as an n-gram, and sizes
/// outside the ranges clamp.
pub fn encode_wide(cfg: &Config, with_train_text: bool) -> Genome {
    let mut g = encode(cfg);
    let count = |name: &str| cfg.agents.iter().filter(|(k, _)| k == name).map(|(_, n)| *n).sum::<usize>().min(3);
    for name in WIDE_AGENTS {
        g.push(Locus::param(bin_mid(count(name), 4)));
    }
    let (kind, size) = match cfg.learners.first() {
        Some(LearnerSpec::Entropy(c)) => (1, ((c.log2_buckets as f64 - 12.0) / 3.0).clamp(0.0, 1.0)),
        Some(LearnerSpec::Ssm(c)) => {
            let i = [8usize, 16, 32].iter().position(|d| *d >= c.d).unwrap_or(2);
            (2, bin_mid(i, 3))
        }
        Some(LearnerSpec::Ngram(c)) => (0, ((c.log2_buckets as f64 - 10.0) / 4.0).clamp(0.0, 1.0)),
        _ => (0, ((12.0 - 10.0) / 4.0f64).clamp(0.0, 1.0)),
    };
    g.push(Locus::param(bin_mid(kind, 3)));
    g.push(Locus::param(size));
    g.push(Locus::param((cfg.replay_per_round as f64 / 32.0).clamp(0.0, 1.0)));
    if with_train_text {
        g.push(Locus::param((cfg.train_text_per_round as f64 / 16.0).clamp(0.0, 1.0)));
    }
    g
}

/// `--genome`: the narrow four loci, or the wide genome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GenomeKind {
    #[default]
    Narrow,
    Wide,
}

impl GenomeKind {
    pub fn parse(s: &str) -> Result<GenomeKind, String> {
        match s {
            "narrow" => Ok(GenomeKind::Narrow),
            "wide" => Ok(GenomeKind::Wide),
            other => Err(format!("--genome: `{other}` is neither narrow nor wide")),
        }
    }

    /// The seed champion's genome under this kind, for `base`.
    pub fn encode(self, base: &Config) -> Genome {
        match self {
            GenomeKind::Narrow => encode(base),
            GenomeKind::Wide => encode_wide(base, !base.train_text.is_empty()),
        }
    }
}

/// The genome of a configuration, the inverse of [`decode`] for the ranges
/// above (for seeding a lineage with a known configuration).
pub fn encode(cfg: &Config) -> Genome {
    let lr = cfg
        .learners
        .first()
        .map(|s| match s {
            LearnerSpec::Ngram(c) => c.lr,
            LearnerSpec::Transformer(c) => c.lr,
            LearnerSpec::Entropy(c) => c.lr,
            LearnerSpec::Ssm(c) => c.lr,
        })
        .unwrap_or(0.02);
    vec![
        Locus::param(((lr.log10() + 3.0) / 2.0).clamp(0.0, 1.0)),
        Locus::param(((cfg.learner_steps as f64 - 1.0) / 15.0).clamp(0.0, 1.0)),
        Locus::param((cfg.length_charge / 0.01).clamp(0.0, 1.0)),
        Locus::param(((cfg.temperature - 0.1) / 1.9).clamp(0.0, 1.0)),
    ]
}

/// The digest a suite of `corpora` is registered under.
pub fn suite_digest(corpora: &[Corpus]) -> Digest {
    let mut h = Sha256::new();
    for c in corpora {
        h.update(c.name().as_bytes());
        h.update(c.sha256().as_bytes());
    }
    Digest(format!("{:x}", h.finalize()))
}

/// Where the `efficiency` axis gets its joules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EfficiencyBasis {
    /// The session's metered (or wall-clock estimated) training joules.
    /// Depends on machine load. The default.
    #[default]
    Meter,
    /// An estimate from a deterministic count of training work: see
    /// [`work_units`] and [`WORK_JOULES_PER_UNIT`].
    Work,
}

/// Joules assumed per unit of [`work_units`] (one parameter touched by one
/// trained byte). Calibrated on 2026-11-13 on the development machine from one
/// run of the default configuration (`arena --rounds 20 --programs 48 --cost
/// fuel` on README.md and germline/src/gate.rs): the ngram learner (3×2^12,
/// 3,145,984 parameters) trained on 1,308,200 bytes, so 4.12e12 work units, in
/// 32.4 s, which is 2,103 J at the 65 W wall-clock estimate. 2,103 / 4.12e12 =
/// 5.1e-10, rounded to one significant figure. It is a fixed convention, not a
/// measurement of any machine: it only puts the `work` axis on a scale
/// comparable to the `meter` one.
pub const WORK_JOULES_PER_UNIT: f64 = 5.0e-10;

/// Deterministic training work of one learner: `parameters × bytes trained on`.
/// One formula for every learner kind. For dense learners (transformer, SSM)
/// every trained byte updates on the order of all parameters, so this is
/// proportional to the multiply-adds. For count-based learners it
/// overestimates (a byte touches a few table rows, not the table), but it still
/// rises with both the learner's size and the data it was given, which is all
/// the axis needs to rank candidates of one kind against each other.
pub fn work_units(parameters: usize, train_bytes: u64) -> f64 {
    parameters as f64 * train_bytes as f64
}

impl EfficiencyBasis {
    pub fn parse(s: &str) -> Result<EfficiencyBasis, String> {
        match s {
            "meter" => Ok(EfficiencyBasis::Meter),
            "work" => Ok(EfficiencyBasis::Work),
            other => Err(format!("--efficiency-basis: `{other}` is neither meter nor work")),
        }
    }

    /// The text recorded as the `how` of the estimate under `work`.
    pub fn work_description() -> String {
        format!("work (deterministic, J per unit = {WORK_JOULES_PER_UNIT:e})")
    }
}

struct Built {
    mean_bpb: f64,
    joules: Option<f64>,
    /// Seconds to score a kilobyte, as the session measured it.
    latency_s_per_kb: f64,
    /// How the training energy was obtained. Efficiency is only as good as this.
    energy: Basis,
    /// Canonical hashes of the session's best programs: what it learned from.
    definitions: Vec<String>,
}

/// The `latency` axis: `1 / (1 + milliseconds per kilobyte)`. Higher is better,
/// strictly decreasing in the time to score a kilobyte, and in `(0, 1]`.
///
/// Milliseconds rather than seconds because the arena's learners score a
/// kilobyte in fractions of a millisecond, and a seconds-based map would sit at
/// 1.0 for all of them, leaving the axis unable to tell two learners apart.
pub fn latency_axis(s_per_kb: f64) -> f64 {
    1.0 / (1.0 + s_per_kb.max(0.0) * 1000.0)
}

/// The basis of a session's training energy. `Measured` only when every
/// training span was read from a hardware counter. Otherwise `Estimated`, with
/// the estimate's own description, or `Unavailable` when no meter gave a figure.
fn session_energy_basis(report: &Report) -> Basis {
    let Some(learner) = report.learners.first() else {
        return Basis::Unavailable { why: "the session trained no learner".into() };
    };
    if learner.train_joules.is_none() {
        return Basis::Unavailable { why: "no meter produced a training figure".into() };
    }
    let all_measured = report
        .checkpoints
        .iter()
        .rev()
        .find(|c| c.learner == 0)
        .is_some_and(|c| c.joules_all_measured);
    if all_measured {
        return Basis::Measured;
    }
    let how = report
        .energy_bases
        .iter()
        .find_map(|r| match &r.basis {
            Basis::Estimated { how } => Some(how.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "estimated".into());
    Basis::Estimated { how }
}

/// Germline's [`Workload`], backed by arena sessions.
pub struct ArenaWorkload {
    pub base: Config,
    pub corpora: Vec<Corpus>,
    /// Watts assumed by the CPU estimate that meters each session. Sessions
    /// are metered by wall clock here so a test needs no GPU; a caller with
    /// hardware counters measures the same way the arena binary does.
    pub cpu_watts: f64,
    /// The files `corpora` were read from, in order, when there are any. The
    /// workload re-reads them before each cycle and refuses to continue if they
    /// no longer hash to the suite it was built on. Empty means nothing is
    /// re-checked (the corpora were not read from files).
    pub heldout_files: Vec<String>,
    /// Emit the `latency` axis from [`evaluate`](Workload::evaluate). Off by
    /// default: a journal's axis set is fixed when it is written, and an axis
    /// that old journals never measured would make every candidate incomparable
    /// with the champion they recorded. The CLI turns it on for
    /// `--objectives ...latency`.
    pub with_latency: bool,
    /// Where `efficiency` gets its joules. `Meter` (the default) is the
    /// session's own figure; `Work` is deterministic.
    pub efficiency_basis: EfficiencyBasis,
    /// Files of the optional validation suite and the digest they were pinned
    /// under. Kept only so they can be re-hashed before each cycle; nothing here
    /// is ever scored by this workload.
    pub validation_files: Vec<String>,
    pub validation_pin: Option<Digest>,
    /// The digest the training-text files (`Config::train_text`) were pinned
    /// under at startup, re-checked before each cycle like the suites above.
    pub train_text_pin: Option<Digest>,
    /// The same for the dev-text files (`Config::dev_text`, `--reward transfer`).
    pub dev_text_pin: Option<Digest>,
    built: HashMap<String, Built>,
    /// Joules every session has charged, all accounts (programs, training,
    /// probes), as the kernel's ledger totals them. This is the running count
    /// [`Workload::joules_spent`] reports to the energy budget.
    spent: f64,
    /// Set when a session's energy was not read in full, so `spent` would be an
    /// undercount. The budget then refuses to count on it.
    spend_incomplete: bool,
    /// Set when any session's joules were estimated rather than read.
    spend_estimated: bool,
}

/// Read held-out files into corpora named by their paths: the naming and order
/// the arena binary uses, so [`suite_digest`] matches across the two.
pub fn read_corpora(paths: &[String]) -> Result<Vec<Corpus>, String> {
    paths
        .iter()
        .map(|p| {
            let bytes = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
            Ok(Corpus::new(p.clone(), bytes))
        })
        .collect()
}

impl ArenaWorkload {
    pub fn new(base: Config, corpora: Vec<Corpus>) -> ArenaWorkload {
        ArenaWorkload {
            base,
            corpora,
            cpu_watts: 65.0,
            heldout_files: Vec::new(),
            with_latency: false,
            efficiency_basis: EfficiencyBasis::Meter,
            validation_files: Vec::new(),
            validation_pin: None,
            train_text_pin: None,
            dev_text_pin: None,
            built: HashMap::new(),
            spent: 0.0,
            spend_incomplete: false,
            spend_estimated: false,
        }
    }

    /// The basis of the joules [`Workload::joules_spent`] counts. `Measured`
    /// only if every session read its counters in full; otherwise `Estimated`,
    /// stating the model, or `Unavailable` when nothing has been measured yet.
    pub fn spend_basis(&self) -> Basis {
        if self.spend_incomplete {
            Basis::Unavailable { why: "a session's energy was not read in full, so the total is an undercount".into() }
        } else if self.built.is_empty() {
            Basis::Unavailable { why: "no session has run".into() }
        } else if self.spend_estimated {
            Basis::Estimated { how: format!("wall-clock seconds × {} W assumed", self.cpu_watts) }
        } else {
            Basis::Measured
        }
    }

    /// The basis of the training energy behind `artifact`'s efficiency figure.
    pub fn energy_basis(&self, artifact: &Digest) -> Option<Basis> {
        self.built.get(&artifact.0).map(|b| b.energy.clone())
    }

    /// The suite this workload's corpora are registered as.
    pub fn suite(&self) -> EvalSuite {
        EvalSuite::new("arena-heldout", germline::SuiteKind::HeldOut, suite_digest(&self.corpora))
    }

    /// Re-read [`Self::heldout_files`] and confirm they still hash to the suite
    /// this workload was built on. Nothing is checked when no files were named.
    pub fn check_heldout_files(&self) -> Result<(), String> {
        if self.heldout_files.is_empty() {
            return Ok(());
        }
        let now = read_corpora(&self.heldout_files)?;
        if suite_digest(&now) != suite_digest(&self.corpora) {
            return Err(format!(
                "the held-out data changed on disk since this run was built ({}); the suite is no longer the one the gate pinned",
                self.heldout_files.join(", ")
            ));
        }
        Ok(())
    }

    /// Re-read the validation files and confirm they still hash to the pinned
    /// digest. Nothing is checked when no validation suite was named.
    pub fn check_validation_files(&self) -> Result<(), String> {
        let Some(pin) = &self.validation_pin else { return Ok(()) };
        if &suite_digest(&read_corpora(&self.validation_files)?) != pin {
            return Err(format!(
                "the validation data changed on disk since this run was built ({})",
                self.validation_files.join(", ")
            ));
        }
        Ok(())
    }

    /// Re-read the training-text files and confirm they still hash to the
    /// pinned digest. A changed file would change what every later candidate
    /// trains on, so the run stops, as for a changed suite.
    pub fn check_train_text_files(&self) -> Result<(), String> {
        let Some(pin) = &self.train_text_pin else { return Ok(()) };
        if &suite_digest(&read_corpora(&self.base.train_text)?) != pin {
            return Err(format!(
                "the training text changed on disk since this run was built ({})",
                self.base.train_text.join(", ")
            ));
        }
        Ok(())
    }

    /// Re-read the dev-text files and confirm they still hash to the pinned
    /// digest: a changed file would change what the transfer reward pays for.
    pub fn check_dev_text_files(&self) -> Result<(), String> {
        let Some(pin) = &self.dev_text_pin else { return Ok(()) };
        if &suite_digest(&read_corpora(&self.base.dev_text)?) != pin {
            return Err(format!("the dev text changed on disk since this run was built ({})", self.base.dev_text.join(", ")));
        }
        Ok(())
    }

    fn meters(&self) -> Vec<Box<dyn Meter>> {
        vec![Box::new(WallClock::new("cpu-estimate", self.cpu_watts))]
    }
}

impl Workload for ArenaWorkload {
    fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
        let cfg = decode(&self.base, &spec.genome);
        let report = run(cfg.clone(), self.corpora.clone(), self.meters(), vec![])?;
        // What this session cost, as the kernel's ledger charged it. A span no
        // meter read, or a session with no reading at all, leaves the count short,
        // so the budget is then refused rather than given a figure it cannot trust.
        if report.energy.unmetered_spans > 0 || report.energy.charged.is_empty() {
            self.spend_incomplete = true;
        } else {
            self.spent += report.energy.total();
            self.spend_estimated |= !report.energy.all_measured;
        }
        let learner = report.learners.first().ok_or("the session trained no learner")?;
        // Identity is the configuration and what it produced, so the same
        // genome under the same seed materializes to the same artifact.
        let identity = serde_json::json!({
            "config": cfg,
            "mean_bpb": learner.mean_bpb.to_bits(),
            "pool_bpb": learner.pool_bpb.to_bits(),
        });
        let d = Digest(format!("{:x}", Sha256::digest(identity.to_string().as_bytes())));
        let definitions =
            report.best_programs.iter().map(|(_, _, _, exact)| exact.clone()).filter(|e| !e.is_empty()).collect();
        let (joules, energy) = match self.efficiency_basis {
            EfficiencyBasis::Meter => (learner.train_joules, session_energy_basis(&report)),
            EfficiencyBasis::Work => (
                Some(work_units(learner.parameters, learner.train_bytes) * WORK_JOULES_PER_UNIT),
                Basis::Estimated { how: EfficiencyBasis::work_description() },
            ),
        };
        let built = Built {
            mean_bpb: learner.mean_bpb,
            joules,
            latency_s_per_kb: learner.latency_s_per_kb,
            energy,
            definitions,
        };
        self.built.insert(d.0.clone(), built);
        Ok(d)
    }

    fn evaluate(&mut self, artifact: &Digest, suite: &EvalSuite) -> Result<FitnessVector, String> {
        if suite.digest != suite_digest(&self.corpora) {
            return Err("the suite is not the one these corpora are registered as".into());
        }
        let b = self.built.get(&artifact.0).ok_or("no such artifact")?;
        if !b.mean_bpb.is_finite() {
            return Err("held-out bits/byte is not finite".into());
        }
        if self.with_latency && !b.latency_s_per_kb.is_finite() {
            return Err("latency is not finite".into());
        }
        let capability = (1.0 - b.mean_bpb / 8.0).clamp(0.0, 1.0);
        // Efficiency is the joules the meters gave. Its basis is `b.energy`, and
        // a caller that needs it reads `energy_basis`.
        let efficiency = b.joules.map(|j| 1.0 / (1.0 + j / 1000.0)).unwrap_or(0.0);
        let mut f = FitnessVector::new().with("capability", capability).with("efficiency", efficiency);
        if self.with_latency {
            f = f.with("latency", latency_axis(b.latency_s_per_kb));
        }
        Ok(f)
    }

    /// Every joule the sessions behind this workload have charged, the seed's
    /// included. `None` once any session's energy was not read in full, so a
    /// budget refuses to run on it.
    fn joules_spent(&self) -> Option<f64> {
        (!self.spend_incomplete).then_some(self.spent)
    }

    /// Refuse to start a cycle on held-out data that has changed under the run.
    fn before_cycle(&mut self) -> Result<(), String> {
        self.check_heldout_files()?;
        self.check_validation_files()?;
        self.check_train_text_files()?;
        self.check_dev_text_files()
    }

    fn shadow(&mut self, artifact: &Digest) -> HealthSample {
        match self.built.get(&artifact.0) {
            Some(b) if b.mean_bpb.is_finite() => HealthSample::ok(),
            _ => HealthSample::failed(),
        }
    }

    fn observe_champion(&mut self, artifact: &Digest) -> HealthSample {
        self.shadow(artifact)
    }

    fn materialized(&self, artifact: &Digest) -> bool {
        self.built.contains_key(&artifact.0)
    }

    /// The session's best programs by canonical hash, for the journal.
    fn attribution(&self, artifact: &Digest) -> Vec<String> {
        self.built.get(&artifact.0).map(|b| b.definitions.clone()).unwrap_or_default()
    }

    /// The knowledge base the session ran under: the one compiled into this
    /// binary, which every session in it shares.
    fn knowledge_base(&self, artifact: &Digest) -> Option<String> {
        self.built.contains_key(&artifact.0).then(|| mage_prototype::kb_proposal::version(mage_prototype::kb::FILES))
    }
}

/// The evaluator identity the command-line gate names. The gate refuses a
/// verdict whose evaluator is the challenger's own.
pub const CLI_EVALUATOR: &str = "arena-evaluator";

/// The attestation key when `ARENA_ATTEST_KEY` is unset. HMAC is symmetric, so
/// this only says the verdict came from a process that knows the key; it does
/// not stop anyone with this binary from minting one. See `germline::attest`.
const DEV_ATTEST_KEY: &str = "arena-development-key";

/// The gate the command-line run is adjudicated under. An episode pins its
/// digest when it opens, so changing these numbers is a visible change of gate.
///
/// The margin is not zero. With zero, a candidate that ties the champion on
/// `capability` promotes, and `capability` is clamped to 0 for any learner
/// above 8 bits/byte, so a run of learners that have not learned yet would
/// promote every candidate on a tie. Germline's own note is that a margin
/// below the noise floor "promotes coin flips". `capability` is
/// `1 - bits/byte / 8`, so `0.01` is about 0.08 bits/byte. That number was
/// chosen by hand and has not been calibrated against the measured noise.
///
/// `dominance` is set by `--objectives`. Its rule (b) holds each objective to its
/// own `--objective-tolerance`, and to zero where none is given. The guard
/// tolerance above applies to the guard check only. Rule (b)'s ratchet applies to
/// every objective.
fn cli_gate(dominance: Option<DominanceRule>) -> PromotionGate {
    PromotionGate {
        primary_axis: "capability".into(),
        min_improvement: 0.01,
        guard_axes: vec!["efficiency".into()],
        // Wall-clock energy is noisy at this scale; the guard is present, and loose.
        guard_tolerance: 0.25,
        // Shadow runs here are a finiteness check, so the germline default holds.
        min_shadow_successes: 8,
        dominance,
    }
}

const USAGE: &str = "usage: arena succession --heldout FILE[,FILE...] --journal PATH [--cycles N] [--seed N] \
[--stop-file PATH] [--force] [--json] [--rounds N] [--programs N] [--candidates N] [--agents K:N,...] \
[--proposer-command \"CMD ARGS\"] [--proposer-timeout S] \
[--objectives capability[,efficiency][,latency]] [--dominance-min F] [--dominance-floor F] \
[--objective-tolerance AXIS=F,...] [--allow-estimated-energy] [--efficiency-basis meter|work] \
[--energy-budget JOULES] [--param-search es|perturb] [--cost joules|fuel] [--validation FILE[,FILE...]] \
[--train-text FILE[,FILE...]] [--train-text-per-round N] [--reward alignment|compression|transfer] [--dev-text FILE[,FILE...]] [--genome narrow|wide] \
[--patience N] [--mutation-scale F] \
[--learners SPEC[,SPEC...]]  (SPEC: ng:orders:log2buckets:lr | tf:d:layers:heads:ctx:lr[:gpu] | \
ent:orders:log2buckets:target_len:lr | ssm:d:lr[:bptt]; the first spec is the one the gate scores)";

/// One champion's score on the validation suite, beside its held-out score.
/// Informational only: the gate never sees it and the journal does not record it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ValidationRecord {
    pub generation: u64,
    /// `capability` on the validation corpora, computed as the gate's is.
    pub capability: f64,
    /// Mean bits per byte on the validation corpora. `capability` clamps to 0
    /// above 8, so this is the figure that still moves there.
    pub bpb: f64,
    /// The `capability` the gate measured for this generation on the held-out suite.
    pub heldout_capability: Option<f64>,
}

/// Fail if any validation file is also a held-out file, by the strings as given
/// or by canonical path: a shared file defeats the point of a second suite.
fn check_validation_disjoint(heldout: &[String], validation: &[String]) -> Result<(), String> {
    let canon = |p: &String| std::fs::canonicalize(p).ok();
    for v in validation {
        let cv = canon(v);
        if heldout.iter().any(|h| h == v || (cv.is_some() && canon(h) == cv)) {
            return Err(format!("--validation: `{v}` is also a --heldout file; the validation suite must be disjoint from it"));
        }
    }
    Ok(())
}

/// Score every champion `lineage` has had (the seed and each promotion) on the
/// validation corpora. Each champion's genome is decoded to the same
/// configuration, seed included, that built it, and run as its own session whose
/// only corpora are `validation`. This runs after the succession run has ended,
/// through no [`ArenaWorkload`] and no gate, so no validation figure can reach a
/// `FitnessVector`, the journal, or the joule count the energy budget reads.
pub fn score_validation(
    base: &Config,
    cpu_watts: f64,
    lineage: &Lineage,
    validation: &[Corpus],
) -> Result<Vec<ValidationRecord>, String> {
    let mut out = Vec::new();
    for g in lineage.generations().iter().filter(|g| g.promoted_under.is_some()) {
        let Some(genome) = &g.genome else { continue };
        let meters: Vec<Box<dyn Meter>> = vec![Box::new(WallClock::new("cpu-estimate", cpu_watts))];
        let report = run(decode(base, genome), validation.to_vec(), meters, vec![])?;
        let learner = report.learners.first().ok_or("the validation session trained no learner")?;
        out.push(ValidationRecord {
            generation: g.id.0,
            capability: (1.0 - learner.mean_bpb / 8.0).clamp(0.0, 1.0),
            bpb: learner.mean_bpb,
            heldout_capability: g.heldout_fitness().and_then(|f| f.get("capability")),
        });
    }
    Ok(out)
}

/// Parse `--objective-tolerance` items: `AXIS=VALUE`, comma-separated. A value
/// must be finite and non-negative, and the axis must be a known one.
fn parse_tolerances(list: &str) -> Result<Vec<(String, f64)>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|item| {
            let (axis, value) = item
                .split_once('=')
                .ok_or_else(|| format!("--objective-tolerance: `{item}` should be AXIS=VALUE"))?;
            let axis = axis.trim();
            if !OBJECTIVE_AXES.contains(&axis) {
                return Err(format!(
                    "--objective-tolerance: `{axis}` is not an axis; the axes are {}",
                    OBJECTIVE_AXES.join(", ")
                ));
            }
            let t: f64 = value
                .trim()
                .parse()
                .map_err(|_| format!("--objective-tolerance: `{}` is not a number", value.trim()))?;
            if !(t.is_finite() && t >= 0.0) {
                return Err(format!("--objective-tolerance: `{axis}={}` must be a finite, non-negative number", value.trim()));
            }
            Ok((axis.to_string(), t))
        })
        .collect()
}

/// Check the tolerances against the run's objectives, and return them as the
/// rule's ordered map. The primary axis has tolerance zero, so it may not be
/// given one here. Efficiency may be given one: under rule (b) its guard status
/// does not change its tolerance.
fn validate_tolerances(objectives: Option<&[String]>, given: &[(String, f64)]) -> Result<BTreeMap<String, f64>, String> {
    let mut out = BTreeMap::new();
    for (axis, t) in given {
        let Some(objs) = objectives else {
            return Err("--objective-tolerance needs --objectives: tolerances apply to the dominance rule".into());
        };
        if axis == "capability" {
            return Err("--objective-tolerance: capability is the primary axis, and its tolerance is always zero".into());
        }
        if !objs.iter().any(|o| o == axis) {
            return Err(format!("--objective-tolerance: `{axis}` is not among --objectives"));
        }
        if out.insert(axis.clone(), *t).is_some() {
            return Err(format!("--objective-tolerance: `{axis}` is given more than once"));
        }
    }
    Ok(out)
}

/// The objective axes `--objectives` may name.
const OBJECTIVE_AXES: [&str; 3] = ["capability", "efficiency", "latency"];

/// The default margin on the objective axes for rule (b), the same as the
/// primary margin.
const DEFAULT_DOMINANCE_MIN: f64 = 0.01;

fn num(v: &str, flag: &str) -> Result<u64, String> {
    v.parse().map_err(|_| format!("{flag}: `{v}` is not a number"))
}

/// `--energy-budget`: a positive, finite number of joules.
fn parse_budget(v: &str) -> Result<f64, String> {
    v.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && *x > 0.0)
        .ok_or_else(|| format!("--energy-budget: `{v}` is not a positive number of joules"))
}

/// `--param-search`: `es` for the self-adaptive evolution strategy at its stated
/// defaults, or `perturb` for the uniform random perturbation (the default).
fn parse_param_search(v: &str) -> Result<ParamSearch, String> {
    match v {
        "es" => Ok(ParamSearch::es()),
        "perturb" => Ok(ParamSearch::Perturb),
        other => Err(format!("--param-search: `{other}` should be `es` or `perturb`")),
    }
}

fn parse_objectives(list: &str) -> Result<Vec<String>, String> {
    let names: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if names.is_empty() {
        return Err(format!("--objectives needs at least one of {}", OBJECTIVE_AXES.join(", ")));
    }
    for n in &names {
        if !OBJECTIVE_AXES.contains(&n.as_str()) {
            return Err(format!("--objectives: `{n}` is not an axis; the axes are {}", OBJECTIVE_AXES.join(", ")));
        }
    }
    if !names.iter().any(|n| n == "capability") {
        return Err("--objectives must include capability, the primary axis the search is for".into());
    }
    Ok(names)
}

fn parse_agents(list: &str) -> Result<Vec<(String, usize)>, String> {
    list.split(',')
        .map(|p| {
            let (k, n) = p.split_once(':').unwrap_or((p, "1"));
            Ok((k.to_string(), num(n, "--agents")? as usize))
        })
        .collect()
}

/// The runner policy's name, which is part of its digest. Only `work` changes
/// it, so a `meter` run's digest is what it always was and a journal of one
/// basis cannot be resumed under the other.
fn policy_name(basis: EfficiencyBasis) -> String {
    match basis {
        EfficiencyBasis::Meter => "succession-cli".into(),
        EfficiencyBasis::Work => "succession-cli+efficiency-work".into(),
    }
}

/// [`policy_name`], plus the opt-in flags that change what a run is: `+train-text`
/// when training text is set, `+reward-transfer` under `--reward transfer`. With them off the name is `policy_name`'s, so the
/// digest of a run without the flag does not move.
fn full_policy_name(basis: EfficiencyBasis, train_text: bool, learners: Option<&str>, transfer: bool, wide: bool) -> String {
    let mut name = policy_name(basis);
    if train_text {
        name.push_str("+train-text");
    }
    if let Some(l) = learners {
        name = format!("{name}+learners={l}");
    }
    if transfer {
        name.push_str("+reward-transfer");
    }
    // The wide genome's length also depends on `--train-text`, which is already in the name.
    if wide {
        name.push_str("+genome-wide");
    }
    name
}

/// `--patience`: a whole number of refusals, at least 1.
fn parse_patience(v: &str) -> Result<u32, String> {
    v.parse::<u32>().ok().filter(|n| *n >= 1).ok_or_else(|| format!("--patience: `{v}` is not a whole number of at least 1"))
}

/// `--mutation-scale`: the `Perturb` step, finite and in (0, 1].
fn parse_mutation_scale(v: &str) -> Result<f64, String> {
    v.parse::<f64>()
        .ok()
        .filter(|x| x.is_finite() && *x > 0.0 && *x <= 1.0)
        .ok_or_else(|| format!("--mutation-scale: `{v}` is not a number in (0, 1]"))
}

/// Apply the opt-in search-budget flags: `--mutation-scale` replaces the scale
/// of the plan's `Perturb` (rate unchanged), and the return value is the
/// runner's `max_consecutive_refusals` (`--patience`, else the runner default).
/// Under `--param-search es` the plan's `Perturb` is skipped (the ES has its own
/// step sizes, see `germline::variation::propose`), so a scale would do
/// nothing and is refused.
fn apply_search_budget(
    variation: &mut VariationPlan,
    patience: Option<u32>,
    mutation_scale: Option<f64>,
) -> Result<u32, String> {
    if let Some(s) = mutation_scale {
        if !variation.param_search.is_default() {
            return Err("--mutation-scale does not apply under --param-search es: the ES sets its own step sizes".into());
        }
        if let germline::variation::Mutation::Perturb { scale, .. } = &mut variation.mutation {
            *scale = s;
        }
    }
    Ok(patience.unwrap_or(RunnerPolicy::default().max_consecutive_refusals))
}

/// [`full_policy_name`], plus `+patience=N` and `+mscale=F` when those opt-in
/// flags are given. Both enter the policy digest anyway (the digest is of the
/// whole serialized policy), the suffix keeps the name honest too.
fn with_budget_suffix(mut name: String, patience: Option<u32>, mutation_scale: Option<f64>) -> String {
    if let Some(p) = patience {
        name.push_str(&format!("+patience={p}"));
    }
    if let Some(s) = mutation_scale {
        name.push_str(&format!("+mscale={s}"));
    }
    name
}

/// Refuse an efficiency axis that is not measured, unless the caller accepts an
/// estimate. Efficiency is joules per unit of work, so a wall-clock estimate
/// (seconds times an assumed wattage) makes "more efficient" mean "faster".
/// With `--allow-estimated-energy` the run proceeds, and says so once.
fn check_energy(objectives: Option<&[String]>, basis: &Basis, allow_estimated: bool) -> Result<(), String> {
    if !objectives.is_some_and(|o| o.iter().any(|n| n == "efficiency")) {
        return Ok(());
    }
    match basis {
        Basis::Measured => Ok(()),
        Basis::Estimated { how } if allow_estimated => {
            eprintln!(
                "succession: warning: the efficiency axis is an estimate ({how}), not a measurement. \
                 Here \"more efficient\" means \"faster\" to the extent that time tracks energy."
            );
            Ok(())
        }
        Basis::Estimated { how } => Err(format!(
            "--objectives includes efficiency, but this run's energy is estimated ({how}), not measured. \
             The efficiency axis would be wall-clock seconds times an assumed wattage, so a candidate that is \
             \"more efficient\" would only be one that is faster. Pass --allow-estimated-energy to proceed with \
             that understanding, or drop efficiency from --objectives."
        )),
        Basis::Unavailable { why } => Err(format!(
            "--objectives includes efficiency, but no energy figure was produced ({why}). Efficiency would be a \
             constant, and --allow-estimated-energy does not change that."
        )),
    }
}

/// The objectives a run's gate and selection read, for the header and the JSON
/// report. With no `--objectives`, the gate is the scalar primary-axis gate.
fn objectives_json(objectives: Option<&[String]>, dominance: Option<&DominanceRule>) -> serde_json::Value {
    serde_json::json!({
        "mode": if objectives.is_some() { "pareto" } else { "scalar" },
        "axes": objectives.map(<[String]>::to_vec).unwrap_or_else(|| vec!["capability".into(), "efficiency".into()]),
        "primary": "capability",
        "guards": ["efficiency"],
        "dominance": dominance,
    })
}

fn objectives_text(objectives: Option<&[String]>, dominance: Option<&DominanceRule>) -> String {
    match (objectives, dominance) {
        (Some(o), Some(r)) => {
            let tol = if r.tolerances.is_empty() {
                "zero tolerance on every objective".to_string()
            } else {
                r.tolerances.iter().map(|(a, t)| format!("{a} {t:.4}")).collect::<Vec<_>>().join(", ")
            };
            format!(
                "{} (primary capability) — Pareto parent selection; promotion by rule (a) or by dominance rule (b) at margin {:.4}, primary floor {:.4}, tolerances: {tol}",
                o.join(", "),
                r.min_improvement,
                r.primary_floor
            )
        }
        (Some(o), None) => format!("{} (primary capability) — Pareto parent selection", o.join(", ")),
        (None, _) => "capability (primary), efficiency (guard) — scalar selection; promotion by rule (a) only".into(),
    }
}

fn param_search_text(search: &ParamSearch) -> String {
    match search {
        ParamSearch::Perturb => "uniform random perturbation".into(),
        ParamSearch::SelfAdaptiveEs { rate, sigma_init, sigma_min, sigma_max, .. } => format!(
            "self-adaptive evolution strategy (rate {rate}, step {sigma_init} in [{sigma_min}, {sigma_max}] of range)"
        ),
    }
}

fn basis_text(basis: &Basis) -> String {
    match basis {
        Basis::Measured => "measured (hardware energy counter)".into(),
        Basis::Estimated { how } => format!("estimated — {how}"),
        Basis::Unavailable { why } => format!("unavailable — {why}"),
    }
}

fn halt_text(h: &Halt) -> String {
    match h {
        Halt::BudgetExhausted { cycles } => format!("budget exhausted after {cycles} cycles"),
        Halt::StopRequested { after_cycles } => {
            format!("stop requested (the stop file existed before the next cycle), after {after_cycles} cycles")
        }
        Halt::SearchStalled { consecutive_refusals } => {
            format!("search stalled: {consecutive_refusals} consecutive refusals")
        }
        Halt::WorkloadFailed { detail } => format!("workload failed: {detail}"),
        Halt::EnergyExhausted { spent, budget, after_cycles } => {
            format!("energy budget reached: spent {spent:.3} J of {budget:.3} J, after {after_cycles} cycles")
        }
        Halt::Demoted { generation, mode, fell_back_to } => {
            format!("demoted: {generation} failed in {mode}; fell back to {fell_back_to}")
        }
        Halt::NoFallbackAvailable { generation, detail } => {
            format!("no usable fallback after {generation} failed: {detail}")
        }
    }
}

/// The process exit code for a halt. A clean stop, whatever its reason, is 0.
/// A workload failure is 1. A demotion means a gate criterion needs a person,
/// so it is 2, as is a failure with nothing to fall back to.
fn exit_code(h: &Halt) -> i32 {
    match h {
        Halt::BudgetExhausted { .. }
        | Halt::StopRequested { .. }
        | Halt::SearchStalled { .. }
        | Halt::EnergyExhausted { .. } => 0,
        Halt::WorkloadFailed { .. } => 1,
        Halt::Demoted { .. } | Halt::NoFallbackAvailable { .. } => 2,
    }
}

/// `arena succession`: propose, evaluate, gate and promote for a bounded number
/// of cycles, with no person in the loop. Returns the process exit code.
///
/// The run is resumable: if `--journal` already holds a run, its records are
/// replayed (chain and succession checks included) and the run continues from
/// where it stopped. `--force` moves an existing journal aside and starts over.
pub fn cli(args: &[String]) -> Result<i32, String> {
    let mut heldout: Vec<String> = Vec::new();
    let mut validation: Vec<String> = Vec::new();
    let mut cycles: u32 = 8;
    let mut seed: u64 = 0;
    let mut journal: Option<String> = None;
    let mut stop_file: Option<String> = None;
    let mut force = false;
    let mut json = false;
    let mut candidates: usize = 4;
    let mut objectives: Option<Vec<String>> = None;
    let mut dominance_min = DEFAULT_DOMINANCE_MIN;
    let mut dominance_floor: Option<f64> = None;
    let mut objective_tolerance: Vec<(String, f64)> = Vec::new();
    let mut allow_estimated = false;
    let mut efficiency_basis = EfficiencyBasis::Meter;
    let mut energy_budget: Option<f64> = None;
    let mut param_search = ParamSearch::Perturb;
    let mut base = Config::default();
    // Set only by `--learners`; it joins the policy name, so default runs keep their digest.
    let mut learners_flag: Option<String> = None;
    let mut genome_kind = GenomeKind::Narrow;
    // Opt-in search budget; each joins the policy name when given.
    let mut patience: Option<u32> = None;
    let mut mutation_scale: Option<f64> = None;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        if matches!(flag, "-h" | "--help") {
            println!("{USAGE}");
            return Ok(0);
        }
        let takes_value = !matches!(flag, "--force" | "--json" | "--allow-estimated-energy");
        let v = if takes_value {
            args.get(i + 1).map(String::as_str).ok_or_else(|| format!("{flag} needs a value"))?
        } else {
            ""
        };
        match flag {
            "--heldout" => heldout.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string)),
            "--validation" => validation.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string)),
            "--train-text" => base.train_text.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string)),
            "--train-text-per-round" => base.train_text_per_round = num(v, flag)? as usize,
            "--dev-text" => base.dev_text.extend(v.split(',').filter(|s| !s.is_empty()).map(str::to_string)),
            "--reward" => {
                base.reward = match v {
                    "alignment" => Reward::Alignment,
                    "compression" => Reward::Compression,
                    "transfer" => Reward::Transfer,
                    other => return Err(format!("--reward: `{other}` is not alignment, compression or transfer")),
                }
            }
            "--genome" => genome_kind = GenomeKind::parse(v)?,
            "--cycles" => cycles = num(v, flag)? as u32,
            "--seed" => seed = num(v, flag)?,
            "--journal" => journal = Some(v.to_string()),
            "--stop-file" => stop_file = Some(v.to_string()),
            "--force" => force = true,
            "--json" => json = true,
            "--rounds" => base.rounds = num(v, flag)? as usize,
            "--programs" => base.programs_per_round = num(v, flag)? as usize,
            "--candidates" => candidates = num(v, flag)? as usize,
            "--agents" => base.agents = parse_agents(v)?,
            "--proposer-command" => {
                base.proposer_command = Some(v.split_whitespace().map(str::to_string).collect())
            }
            "--proposer-timeout" => base.proposer_timeout_s = num(v, flag)?,
            "--objectives" => objectives = Some(parse_objectives(v)?),
            "--dominance-min" => {
                dominance_min = v
                    .parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite() && *x >= 0.0)
                    .ok_or_else(|| format!("--dominance-min: `{v}` is not a non-negative number"))?
            }
            "--dominance-floor" => {
                dominance_floor = Some(
                    v.parse::<f64>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| format!("--dominance-floor: `{v}` is not a number"))?,
                )
            }
            "--objective-tolerance" => objective_tolerance.extend(parse_tolerances(v)?),
            "--allow-estimated-energy" => allow_estimated = true,
            "--efficiency-basis" => efficiency_basis = EfficiencyBasis::parse(v)?,
            "--energy-budget" => energy_budget = Some(parse_budget(v)?),
            "--param-search" => param_search = parse_param_search(v)?,
            "--patience" => patience = Some(parse_patience(v)?),
            "--mutation-scale" => mutation_scale = Some(parse_mutation_scale(v)?),
            "--learners" => {
                base.learners = crate::learner::parse_learner_specs(v)?;
                learners_flag = Some(v.to_string());
            }
            "--cost" => {
                base.cost_unit = match v {
                    "joules" => crate::arena::CostUnit::Joules,
                    "fuel" => crate::arena::CostUnit::Fuel,
                    other => return Err(format!("--cost: `{other}` is neither joules nor fuel")),
                }
            }
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
        i += if takes_value { 2 } else { 1 };
    }

    if heldout.is_empty() {
        return Err(format!("--heldout is required: without held-out data there is nothing to measure against\n{USAGE}"));
    }
    check_validation_disjoint(&heldout, &validation)?;
    // Training text may be neither a held-out nor a validation file, by name,
    // canonical path or content; checked here so a usage error costs nothing.
    if !base.train_text.is_empty() {
        refuse_train_text_overlap(&base.train_text, "--heldout", &read_corpora(&heldout)?)?;
        refuse_train_text_overlap(&base.train_text, "--validation", &read_corpora(&validation)?)?;
    }
    // Dev text belongs to the transfer reward alone, and may be none of the
    // held-out, validation or training-text files: same checks, same place.
    match (base.reward == Reward::Transfer, base.dev_text.is_empty()) {
        (true, true) => return Err(format!("--reward transfer needs --dev-text FILE[,FILE...]
{USAGE}")),
        (false, false) => return Err(format!("--dev-text is only used with --reward transfer
{USAGE}")),
        _ => {}
    }
    if !base.dev_text.is_empty() {
        refuse_text_overlap(&base.dev_text, "--dev-text", "dev text", "--heldout", &read_corpora(&heldout)?)?;
        refuse_text_overlap(&base.dev_text, "--dev-text", "dev text", "--validation", &read_corpora(&validation)?)?;
        refuse_text_overlap(&base.dev_text, "--dev-text", "dev text", "--train-text", &read_corpora(&base.train_text)?)?;
    }
    let journal_path = PathBuf::from(
        journal.ok_or_else(|| format!("--journal PATH is required: the journal is the run's record\n{USAGE}"))?,
    );
    if cycles == 0 {
        return Err("--cycles must be at least 1".into());
    }
    let key = std::env::var("ARENA_ATTEST_KEY").unwrap_or_else(|_| {
        eprintln!("succession: ARENA_ATTEST_KEY is unset; attesting with the development key");
        DEV_ATTEST_KEY.to_string()
    });
    // Checked before the seed runs, so a usage error costs nothing.
    if dominance_floor.is_some() && objectives.is_none() {
        return Err("--dominance-floor needs --objectives: the floor belongs to the dominance rule".into());
    }
    let tolerances = validate_tolerances(objectives.as_deref(), &objective_tolerance)?;
    param_search.validate()?;
    // Checked before the seed runs, so a usage error costs nothing.
    apply_search_budget(&mut VariationPlan { param_search: param_search.clone(), ..VariationPlan::default() }, patience, mutation_scale)?;

    let mut w = ArenaWorkload::new(base.clone(), read_corpora(&heldout)?);
    w.heldout_files = heldout.clone();
    // The validation suite is read here and re-checked before every cycle, like
    // the held-out one, but it is held by the CLI and never by the workload.
    let validation_corpora = read_corpora(&validation)?;
    let validation_digest = (!validation.is_empty()).then(|| suite_digest(&validation_corpora));
    w.validation_files = validation.clone();
    w.validation_pin = validation_digest.clone();
    w.train_text_pin = (!base.train_text.is_empty())
        .then(|| read_corpora(&base.train_text).map(|c| suite_digest(&c)))
        .transpose()?;
    w.dev_text_pin = (!base.dev_text.is_empty())
        .then(|| read_corpora(&base.dev_text).map(|c| suite_digest(&c)))
        .transpose()?;
    w.with_latency = objectives.as_ref().is_some_and(|o| o.iter().any(|n| n == "latency"));
    w.efficiency_basis = efficiency_basis;
    let suite = w.suite();
    let digest = suite.digest.clone();

    // The champion a fresh run starts from: the base configuration, measured on
    // the suite. Built the same way on every invocation, so a resumed journal
    // replays onto the same genesis.
    let base_genome = genome_kind.encode(&base);
    let seed_art = w.materialize(&CandidateSpec::new("seed", base_genome.clone()))?;
    let seed_fit = w.evaluate(&seed_art, &suite)?;
    let basis = w.energy_basis(&seed_art).unwrap_or_else(|| Basis::Unavailable {
        why: "the seed session recorded no energy basis".into(),
    });
    // `work` is an estimate by construction, but a deterministic one the caller
    // chose by name, so choosing it is the acceptance `--allow-estimated-energy`
    // asks for; the basis is still printed and recorded in the JSON.
    if efficiency_basis == EfficiencyBasis::Meter {
        check_energy(objectives.as_deref(), &basis, allow_estimated)?;
    }
    let dominance = objectives.as_ref().map(|o| DominanceRule {
        objectives: o.clone(),
        min_improvement: dominance_min,
        primary_floor: dominance_floor.unwrap_or(0.0),
        tolerances: tolerances.clone(),
    });
    let mut genesis = Lineage::new();
    let id = genesis.next_id();
    let mut g = Generation::new(id, seed_art.clone()).measured(Measurement {
        suite: suite.clone(),
        fitness: seed_fit,
        evaluator: CLI_EVALUATOR.into(),
    });
    g.genome = Some(base_genome);
    genesis.add(g);
    let ep = Episode::open(cli_gate(dominance.clone()), suite.digest.clone(), CLI_EVALUATOR);
    // The seed earns authority the way a candidate does: by its shadow runs,
    // counted, with as many as the runner would give a candidate.
    let shadow_runs = RunnerPolicy::default().shadow_runs;
    let shadow_ok = (0..shadow_runs).filter(|_| w.shadow(&seed_art).success).count() as u32;
    let approval = ep
        .approve(genesis.get(id).expect("the seed was just added"), &genesis, shadow_ok, &|d| w.materialized(d))
        .map_err(|v| format!("the seed is refused: {:?}", v.reasons()))?;
    genesis.promote(approval).map_err(|e| e.to_string())?;
    let at = Attestor::new(CLI_EVALUATOR, key.into_bytes());

    if force && journal_path.exists() {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let backup = PathBuf::from(format!("{}.{ts}.bak", journal_path.display()));
        std::fs::rename(&journal_path, &backup).map_err(|e| format!("--force: moving the old journal aside: {e}"))?;
        eprintln!("succession: --force: the existing journal was moved to {}", backup.display());
    }
    let mut journal = Journal::open(&journal_path).map_err(|e| format!("{}: {e}", journal_path.display()))?;

    // Parents are chosen by Pareto rank over the named objectives when they are
    // given. Without them the variation plan is the default, as it always was.
    let mut variation = match &objectives {
        Some(o) => VariationPlan {
            selection: Selection::Pareto { objectives: o.clone() },
            ..VariationPlan::default()
        },
        None => VariationPlan::default(),
    };
    variation.param_search = param_search.clone();
    let max_refusals = apply_search_budget(&mut variation, patience, mutation_scale)?;
    let policy = RunnerPolicy {
        name: with_budget_suffix(
            full_policy_name(efficiency_basis, !base.train_text.is_empty(), learners_flag.as_deref(), base.reward == Reward::Transfer, genome_kind == GenomeKind::Wide),
            patience,
            mutation_scale,
        ),
        max_consecutive_refusals: max_refusals,
        max_cycles: cycles,
        candidates_per_cycle: candidates,
        stop_file: stop_file.map(PathBuf::from),
        variation,
        energy_budget_joules: energy_budget,
        ..RunnerPolicy::default()
    };
    let mut runner = Runner::new(policy, &ep, &at, suite.clone(), seed);
    let (report, lineage) = if journal.is_empty() {
        let r = runner.run(&mut genesis, &mut journal, &mut w);
        (r, genesis)
    } else {
        let records = journal.len();
        let mut resumed = Lineage::replay(genesis, &journal, None)
            .map_err(|e| format!("cannot resume from {}: {e}", journal_path.display()))?;
        eprintln!(
            "succession: resumed from {} ({records} journal records, {} proposals)",
            journal_path.display(),
            resumed.proposals
        );
        if energy_budget.is_some() {
            eprintln!(
                "succession: energy: {:.3} J restored from the journal; journals written before a budget was set count as zero",
                resumed.energy_spent
            );
        }
        runner = runner.resume(&resumed);
        let r = runner.run(&mut resumed.lineage, &mut journal, &mut w);
        (r, resumed.lineage)
    };

    // The spend's basis is read after the run, because a cycle can add to it.
    let spend_basis = w.spend_basis();
    // Scored only now, after the run, from the lineage it left.
    let validation_records = match &validation_digest {
        Some(_) => Some(score_validation(&base, w.cpu_watts, &lineage, &validation_corpora)?),
        None => None,
    };
    if json {
        let mut out = serde_json::json!({
            "suite_digest": digest.0,
            "journal": journal_path.display().to_string(),
            "objectives": objectives_json(objectives.as_deref(), dominance.as_ref()),
            "energy_basis": basis,
            "energy_spend_basis": spend_basis,
            "report": report,
        });
        if let (Some(d), Some(records)) = (&validation_digest, &validation_records) {
            out["validation_suite_digest"] = serde_json::json!(d.0);
            out["validation"] = serde_json::json!(records);
        }
        println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
    } else {
        println!("succession: held-out suite digest {}", digest.0);
        println!("objectives: {}", objectives_text(objectives.as_deref(), dominance.as_ref()));
        println!("energy basis: {}", basis_text(&basis));
        println!("continuous loci: {}", param_search_text(&param_search));
        println!("policy `{}`@{}, at most {cycles} cycles, journal {}", report.policy, report.policy_digest.short(), journal_path.display());
        for c in &report.cycles {
            match c.candidate {
                Some(gen) if c.promoted => println!("cycle {}: {gen} promoted", c.cycle),
                Some(gen) => println!("cycle {}: {gen} refused: {}", c.cycle, c.reasons.join("; ")),
                None => println!("cycle {}: no candidate: {}", c.cycle, c.reasons.join("; ")),
            }
        }
        println!(
            "{} cycles run, {} promotions, {} refusals",
            report.cycles.len(),
            report.promotions,
            report.refusals
        );
        if let (Some(spent), Some(budget)) = (report.energy_spent_joules, energy_budget) {
            println!(
                "energy: spent {spent:.3} J of a {budget:.3} J budget, the seed included (spend basis: {})",
                basis_text(&spend_basis)
            );
        }
        println!("halt: {}", halt_text(&report.halt));
        if let (Some(d), Some(records)) = (&validation_digest, &validation_records) {
            println!("validation suite digest {}", d.0);
            for r in records {
                let held = r.heldout_capability.map_or("n/a".to_string(), |h| format!("{h:.4}"));
                println!("validation: gen{} capability {:.4}, {:.3} bits/byte (held-out {held})", r.generation, r.capability, r.bpb);
            }
        }
        if let Some(champ) = lineage.champion() {
            println!("champion: {}", champ.id);
        }
    }
    Ok(exit_code(&report.halt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::Substrate;
    use germline::attest::Attestor;
    use germline::gate::{Episode, PromotionGate};
    use germline::journal::Journal;
    use germline::lineage::Lineage;
    use germline::runner::{Runner, RunnerPolicy};
    use germline::supervisor::SupervisionPolicy;
    use germline::{Generation, Measurement};

    fn base() -> Config {
        Config {
            rounds: 3,
            programs_per_round: 8,
            substrate: Substrate { fuel: 20_000, max_bytes: 64 },
            learners: vec![LearnerSpec::Ngram(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.02 })],
            agents: vec![("grammar".into(), 1), ("mutator".into(), 1)],
            eval_every: 3,
            replay_per_round: 2,
            ..Config::default()
        }
    }

    fn corpora() -> Vec<Corpus> {
        vec![Corpus::new("text", b"the quick brown fox jumps over the lazy dog. ".repeat(8))]
    }

    #[test]
    fn decode_and_encode_round_trip() {
        let cfg = decode(&base(), &vec![Locus::param(0.5), Locus::param(0.4), Locus::param(0.3), Locus::param(0.2)]);
        let back = encode(&cfg);
        let again = decode(&base(), &back);
        assert_eq!(again.learner_steps, cfg.learner_steps);
        assert!((again.length_charge - cfg.length_charge).abs() < 1e-12);
        assert!((again.temperature - cfg.temperature).abs() < 1e-12);
    }

    fn vals(g: &Genome) -> Vec<f64> {
        g.iter().map(|l| l.as_param().expect("a param locus")).collect()
    }

    #[test]
    fn narrow_genome_is_the_four_loci_and_the_default() {
        let g = GenomeKind::default().encode(&base());
        assert_eq!(GenomeKind::default(), GenomeKind::Narrow);
        assert_eq!(g, encode(&base()));
        assert_eq!(g.len(), NARROW_LOCI);
        // Digest of the policy is the old one.
        assert_eq!(full_policy_name(EfficiencyBasis::Meter, false, None, false, false), "succession-cli");
        // A narrow genome decodes without touching agents, learner kind or replay.
        let cfg = decode(&base(), &g);
        assert_eq!(cfg.agents, base().agents);
        assert_eq!(cfg.replay_per_round, base().replay_per_round);
        assert!(matches!(cfg.learners[0], LearnerSpec::Ngram(c) if c.log2_buckets == 8 && c.orders == 2));
    }

    #[test]
    fn wide_genome_round_trips() {
        let cfgs = [
            Config {
                agents: vec![("grammar".into(), 2), ("uniform".into(), 1), ("mutator".into(), 3), ("attacker".into(), 1)],
                learners: vec![
                    LearnerSpec::Entropy(crate::learner::EntropyConfig { orders: 4, log2_buckets: 14, target_len: 4.0, lr: 0.02 }),
                    LearnerSpec::Ngram(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 }),
                ],
                replay_per_round: 17,
                train_text: vec!["x.txt".into()],
                train_text_per_round: 9,
                ..base()
            },
            Config {
                agents: vec![("uniform".into(), 3)],
                learners: vec![LearnerSpec::Ssm(crate::learner::SsmConfig { d: 32, lr: 0.01, bptt: crate::ssm::DEFAULT_BPTT })],
                replay_per_round: 0,
                ..base()
            },
            Config {
                learners: vec![LearnerSpec::Ngram(LearnerConfig { orders: 3, log2_buckets: 13, lr: 0.02 })],
                replay_per_round: 32,
                ..base()
            },
        ];
        for cfg in cfgs {
            let tt = !cfg.train_text.is_empty();
            let g = encode_wide(&cfg, tt);
            assert_eq!(g.len(), NARROW_LOCI + WIDE_LOCI.len() + usize::from(tt));
            let got = decode(&cfg, &g);
            let mut want = cfg.agents.clone();
            want.sort();
            let mut have = got.agents.clone();
            have.sort();
            assert_eq!(have, want, "agents");
            assert_eq!(got.replay_per_round, cfg.replay_per_round);
            assert_eq!(got.train_text_per_round, cfg.train_text_per_round);
            assert_eq!(got.learners.len(), cfg.learners.len(), "the rest of the learners are kept");
            // (locus 0's lr applies to every learner, as in the narrow genome.)
            for (a, b) in got.learners[1..].iter().zip(&cfg.learners[1..]) {
                assert_eq!(std::mem::discriminant(a), std::mem::discriminant(b));
            }
            match (got.learners[0], cfg.learners[0]) {
                (LearnerSpec::Ngram(a), LearnerSpec::Ngram(b)) => assert_eq!(a.log2_buckets, b.log2_buckets),
                (LearnerSpec::Entropy(a), LearnerSpec::Entropy(b)) => assert_eq!(a.log2_buckets, b.log2_buckets),
                (LearnerSpec::Ssm(a), LearnerSpec::Ssm(b)) => assert_eq!(a.d, b.d),
                (a, b) => panic!("kind changed: {a:?} vs {b:?}"),
            }
            assert!((lr_of(&got.learners[0]) - lr_of(&cfg.learners[0])).abs() < 1e-9);
            // Decoding twice from the encoded genome is a fixed point.
            assert_eq!(encode_wide(&got, tt).len(), g.len());
            assert_eq!(vals(&encode_wide(&got, tt))[4..], vals(&g)[4..]);
        }
    }

    #[test]
    fn wide_bins_edges() {
        let tt = Config { train_text: vec!["x.txt".into()], ..base() };
        let loci = |v: [f64; 7]| -> Genome {
            let mut g = vec![Locus::param(0.5); NARROW_LOCI];
            g.extend(v.iter().map(|x| Locus::param(*x)));
            g
        };
        let lo = decode(&tt, &loci([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        // All three agent counts zero: grammar:1, and the base's other agents are kept.
        assert_eq!(lo.agents, vec![("grammar".to_string(), 1)]);
        assert!(matches!(lo.learners[0], LearnerSpec::Ngram(c) if c.log2_buckets == 10));
        assert_eq!((lo.replay_per_round, lo.train_text_per_round), (0, 0));
        let hi = decode(&tt, &loci([1.0; 7]));
        assert_eq!(hi.agents, vec![("grammar".to_string(), 3), ("uniform".to_string(), 3), ("mutator".to_string(), 3)]);
        assert!(matches!(hi.learners[0], LearnerSpec::Ssm(c) if c.d == 32));
        assert_eq!((hi.replay_per_round, hi.train_text_per_round), (32, 16));
        let ent = decode(&tt, &loci([0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0]));
        assert!(matches!(ent.learners[0], LearnerSpec::Entropy(c) if c.log2_buckets == 12 && c.orders == 4));
        let ent_hi = decode(&tt, &loci([0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0]));
        assert!(matches!(ent_hi.learners[0], LearnerSpec::Entropy(c) if c.log2_buckets == 15));
        let ng_hi = decode(&tt, &loci([0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
        assert!(matches!(ng_hi.learners[0], LearnerSpec::Ngram(c) if c.log2_buckets == 14));
        // Without a train-text locus, train_text_per_round is the base's.
        let nt = decode(&base(), &loci([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])[..NARROW_LOCI + WIDE_LOCI.len()].to_vec());
        assert_eq!(nt.train_text_per_round, base().train_text_per_round);
        // The train-text locus exists only when the base has train text.
        assert_eq!(GenomeKind::Wide.encode(&base()).len(), 10);
        assert_eq!(GenomeKind::Wide.encode(&tt).len(), 11);
        // Agents outside the three survive.
        let atk = Config { agents: vec![("attacker".into(), 2), ("grammar".into(), 1)], ..base() };
        let d = decode(&atk, &loci([0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        assert_eq!(d.agents, vec![("grammar".to_string(), 3), ("attacker".to_string(), 2)]);
    }

    #[test]
    fn a_bad_genome_value_is_a_usage_error() {
        assert_eq!(GenomeKind::parse("narrow"), Ok(GenomeKind::Narrow));
        assert_eq!(GenomeKind::parse("wide"), Ok(GenomeKind::Wide));
        let args: Vec<String> =
            ["--heldout", "x", "--journal", "y", "--genome", "huge"].iter().map(|s| s.to_string()).collect();
        let e = cli(&args).expect_err("cli refuses it");
        assert!(e.contains("--genome") && e.contains("huge"), "{e}");
    }

    #[test]
    fn search_budget_flags_are_validated_named_and_applied() {
        let run = |extra: &[&str]| {
            let mut a: Vec<String> = ["--heldout", "x", "--journal", "y"].iter().map(|s| s.to_string()).collect();
            a.extend(extra.iter().map(|s| s.to_string()));
            cli(&a).expect_err("refused")
        };
        assert!(run(&["--patience", "0"]).contains("--patience"));
        assert!(run(&["--patience", "x"]).contains("--patience"));
        assert!(run(&["--mutation-scale", "0"]).contains("--mutation-scale"));
        assert!(run(&["--mutation-scale", "1.5"]).contains("--mutation-scale"));
        assert!(run(&["--mutation-scale", "nan"]).contains("--mutation-scale"));
        let es = run(&["--param-search", "es", "--mutation-scale", "0.3"]);
        assert!(es.contains("--mutation-scale") && es.contains("es"), "{es}");

        // Names: unchanged by default, suffixed when given.
        let n = || full_policy_name(EfficiencyBasis::Work, false, None, false, true);
        assert_eq!(with_budget_suffix(n(), None, None), n());
        assert_eq!(with_budget_suffix(n(), Some(10), None), format!("{}+patience=10", n()));
        assert_eq!(with_budget_suffix(n(), Some(10), Some(0.35)), format!("{}+patience=10+mscale=0.35", n()));

        // Values reach the policy, and the digest moves only when a flag is given.
        let mut v = VariationPlan::default();
        assert_eq!(apply_search_budget(&mut v, None, None), Ok(3));
        assert_eq!(v, VariationPlan::default());
        let mut v = VariationPlan::default();
        assert_eq!(apply_search_budget(&mut v, Some(1), Some(0.35)), Ok(1));
        assert_eq!(v.mutation, germline::variation::Mutation::Perturb { rate: 0.2, scale: 0.35 });
        let d = |p: u32, v: VariationPlan| RunnerPolicy { max_consecutive_refusals: p, variation: v, ..RunnerPolicy::default() }.digest();
        assert_eq!(d(3, VariationPlan::default()), RunnerPolicy::default().digest());
        assert_ne!(d(1, VariationPlan::default()), RunnerPolicy::default().digest());
        assert_ne!(d(3, v), RunnerPolicy::default().digest());
    }

    #[test]
    fn an_unregistered_suite_is_refused() {
        let mut w = ArenaWorkload::new(base(), corpora());
        let d = w.materialize(&CandidateSpec::new("c", encode(&base()))).expect("materializes");
        let wrong = EvalSuite::new("other", germline::SuiteKind::HeldOut, Digest::of(b"not these corpora"));
        assert!(w.evaluate(&d, &wrong).is_err());
        let right = w.suite();
        let f = w.evaluate(&d, &right).expect("scores on its registered suite");
        assert!(f.get("capability").is_some() && f.get("efficiency").is_some());
    }

    #[test]
    fn a_session_is_handed_on_the_knowledge_base_it_ran_under() {
        let mut w = ArenaWorkload::new(base(), corpora());
        let d = w.materialize(&CandidateSpec::new("c", encode(&base()))).expect("materializes");
        assert_eq!(w.knowledge_base(&d), Some(mage_prototype::kb_proposal::version(mage_prototype::kb::FILES)));
        assert_eq!(w.knowledge_base(&Digest::of(b"never materialized")), None);
    }

    #[test]
    fn a_heldout_file_changed_under_the_run_is_refused_before_the_cycle() {
        let dir = std::env::temp_dir().join(format!("arena-suite-change-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heldout.txt");
        std::fs::write(&path, b"the first held-out text").unwrap();
        let p = path.display().to_string();
        let mut w = ArenaWorkload::new(base(), read_corpora(std::slice::from_ref(&p)).unwrap());
        w.heldout_files = vec![p.clone()];
        assert!(w.before_cycle().is_ok(), "unchanged data passes the check");
        std::fs::write(&path, b"a different held-out text").unwrap();
        let err = w.before_cycle().expect_err("changed data is refused");
        assert!(err.contains("changed on disk"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_workload_with_no_files_has_nothing_to_recheck() {
        let w = ArenaWorkload::new(base(), corpora());
        assert!(w.check_heldout_files().is_ok());
    }

    #[test]
    fn clean_halts_exit_zero_and_failures_do_not() {
        assert_eq!(exit_code(&Halt::BudgetExhausted { cycles: 3 }), 0);
        assert_eq!(exit_code(&Halt::StopRequested { after_cycles: 0 }), 0);
        assert_eq!(exit_code(&Halt::SearchStalled { consecutive_refusals: 3 }), 0);
        assert_eq!(exit_code(&Halt::WorkloadFailed { detail: "x".into() }), 1);
        assert_eq!(exit_code(&Halt::Demoted { generation: germline::GenerationId(2), mode: "m".into(), fell_back_to: germline::GenerationId(1) }), 2);
    }

    #[test]
    fn arena_sessions_succeed_only_through_the_gate() {
        // Seed a lineage with the base configuration as champion, then let
        // germline's runner propose, build and adjudicate arena sessions.
        let mut w = ArenaWorkload::new(base(), corpora());
        let suite = w.suite();
        let seed_spec = CandidateSpec::new("seed", encode(&base()));
        let seed_art = w.materialize(&seed_spec).unwrap();
        let seed_fit = w.evaluate(&seed_art, &suite).unwrap();
        let mut lineage = Lineage::new();
        let id = lineage.next_id();
        let mut g = Generation::new(id, seed_art).measured(Measurement {
            suite: suite.clone(),
            fitness: seed_fit,
            evaluator: "arena-evaluator".into(),
        });
        g.genome = Some(encode(&base()));
        lineage.add(g);
        let ep = Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.0,
                guard_axes: vec!["efficiency".into()],
                // Wall-clock energy is noisy at test scale; the guard is
                // present, and loose.
                guard_tolerance: 0.5,
                min_shadow_successes: 1,
                dominance: None,
            },
            suite.digest.clone(),
            "arena-evaluator",
        );
        // The seed takes authority the way every successor must: on the gate's
        // approval. With no incumbent, its measurement beats nothing.
        let seed = lineage.get(id).unwrap().clone();
        let approval = ep
            .approve(&seed, &lineage, 1, &|d| w.materialized(d))
            .unwrap_or_else(|v| panic!("the seed is refused: {:?}", v.reasons()));
        lineage.promote(approval).unwrap();
        let at = Attestor::new("arena-evaluator", b"arena test key".to_vec());
        let dir = std::env::temp_dir().join(format!("arena-succession-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut journal = Journal::open(&dir.join("journal.jsonl")).unwrap();
        let policy = RunnerPolicy {
            max_cycles: 3,
            max_consecutive_refusals: 3,
            shadow_runs: 1,
            supervision: SupervisionPolicy { window: 2, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        };
        let mut runner = Runner::new(policy, &ep, &at, suite, 11);
        let report = runner.run(&mut lineage, &mut journal, &mut w);
        // Every proposed session is attributed, in the journal, to the
        // programs it learned from, by canonical hash (plan 2.4).
        let attributed: Vec<Vec<String>> = journal
            .replay()
            .unwrap()
            .into_iter()
            .filter_map(|r| match r.entry {
                germline::journal::Entry::Proposed { definitions, .. } => Some(definitions),
                _ => None,
            })
            .collect();
        assert!(!attributed.is_empty() && attributed.iter().all(|d| !d.is_empty() && d.iter().all(|h| h.len() == 64)), "{attributed:?}");
        let _ = std::fs::remove_dir_all(&dir);

        assert!(!report.cycles.is_empty(), "the runner drove arena sessions");
        assert_eq!(report.journal_head, journal.head().cloned(), "and journaled every step");
        // Whatever holds authority was adjudicated: the champion is either the
        // seed or a generation the gate promoted, and it is materialized.
        let champ = lineage.champion().expect("a champion");
        assert!(w.materialized(&champ.artifact));
        if champ.id != id {
            assert!(report.promotions > 0);
            assert!(champ.genome.is_some(), "a promoted session carries its configuration");
        }
    }

    #[test]
    fn a_validation_file_that_is_also_a_heldout_file_is_a_usage_error() {
        let dir = std::env::temp_dir().join(format!("arena-validation-overlap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let held = dir.join("held.txt");
        std::fs::write(&held, b"held-out text").unwrap();
        let other = dir.join("other.txt");
        std::fs::write(&other, b"validation text").unwrap();
        let h = held.display().to_string();
        let o = other.display().to_string();
        // Same string, and the same file spelled another way.
        assert!(check_validation_disjoint(&[h.clone()], &[h.clone()]).is_err());
        let alias = dir.join(".").join("held.txt").display().to_string();
        assert!(check_validation_disjoint(&[h.clone()], &[alias]).is_err());
        assert!(check_validation_disjoint(&[h.clone()], &[o.clone()]).is_ok());
        // Through the CLI the overlap is refused before anything runs.
        let args: Vec<String> = ["--heldout", &h, "--validation", &h, "--journal", "unused.jsonl"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let err = cli(&args).expect_err("overlap is refused");
        assert!(err.contains("--validation") && err.contains("--heldout"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Seed, then three gated cycles, as the CLI does, on a fuel-metered base.
    fn drive_small_run() -> (Vec<(Option<u64>, bool, Vec<String>)>, Lineage) {
        let cfg = Config { cost_unit: crate::arena::CostUnit::Fuel, ..base() };
        let mut w = ArenaWorkload::new(cfg.clone(), corpora());
        let suite = w.suite();
        let genome = encode(&cfg);
        let seed_art = w.materialize(&CandidateSpec::new("seed", genome.clone())).unwrap();
        let seed_fit = w.evaluate(&seed_art, &suite).unwrap();
        let mut lineage = Lineage::new();
        let id = lineage.next_id();
        let mut g = Generation::new(id, seed_art).measured(Measurement {
            suite: suite.clone(),
            fitness: seed_fit,
            evaluator: "arena-evaluator".into(),
        });
        g.genome = Some(genome);
        lineage.add(g);
        let ep = Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.0,
                guard_axes: vec!["efficiency".into()],
                guard_tolerance: 0.5,
                min_shadow_successes: 1,
                dominance: None,
            },
            suite.digest.clone(),
            "arena-evaluator",
        );
        let seed = lineage.get(id).unwrap().clone();
        let approval = ep.approve(&seed, &lineage, 1, &|d| w.materialized(d)).unwrap();
        lineage.promote(approval).unwrap();
        let at = Attestor::new("arena-evaluator", b"arena test key".to_vec());
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir()
            .join(format!("arena-validation-run-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        let mut journal = Journal::open(&dir.join("journal.jsonl")).unwrap();
        let policy = RunnerPolicy {
            max_cycles: 3,
            max_consecutive_refusals: 3,
            shadow_runs: 1,
            supervision: SupervisionPolicy { window: 2, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        };
        let mut runner = Runner::new(policy, &ep, &at, suite, 11);
        let report = runner.run(&mut lineage, &mut journal, &mut w);
        let _ = std::fs::remove_dir_all(&dir);
        let cycles = report.cycles.iter().map(|c| (c.candidate.map(|g| g.0), c.promoted, c.reasons.clone())).collect();
        (cycles, lineage)
    }

    #[test]
    fn validation_scores_every_champion_and_the_gate_ignores_it() {
        let cfg = Config { cost_unit: crate::arena::CostUnit::Fuel, ..base() };
        let validation = vec![Corpus::new("val", b"pack my box with five dozen liquor jugs. ".repeat(8))];
        let (with_cycles, lineage) = drive_small_run();
        let records = score_validation(&cfg, 65.0, &lineage, &validation).expect("scores");
        // One record per champion: the seed, and one for each promotion.
        let promotions = with_cycles.iter().filter(|c| c.1).count();
        assert_eq!(records.len(), 1 + promotions, "{records:?}");
        assert_eq!(records[0].generation, 0, "the seed comes first");
        for r in &records {
            assert!((0.0..=1.0).contains(&r.capability));
            assert!(r.heldout_capability.is_some(), "the held-out figure is the gate's own");
        }
        // The same run with no validation step has the same promotions and
        // refusals, reasons included: validation is scored after, from the
        // lineage, and nothing it computes is visible to the gate.
        let (without_cycles, _) = drive_small_run();
        assert_eq!(with_cycles, without_cycles);
    }

    #[test]
    fn the_latency_axis_is_in_the_unit_interval_and_falls_as_time_rises() {
        assert_eq!(latency_axis(0.0), 1.0);
        assert_eq!(latency_axis(-1.0), 1.0, "a negative time is clamped to instant");
        let mut prev = f64::INFINITY;
        for s_per_kb in [0.0, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1.0, 100.0] {
            let v = latency_axis(s_per_kb);
            assert!((0.0..=1.0).contains(&v), "{s_per_kb} s/kB maps to {v}");
            assert!(v < prev, "slower must score lower: {s_per_kb} s/kB gave {v}, after {prev}");
            prev = v;
        }
    }

    #[test]
    fn evaluate_emits_latency_only_when_asked_and_records_the_energy_basis() {
        let mut w = ArenaWorkload::new(base(), corpora());
        let d = w.materialize(&CandidateSpec::new("c", encode(&base()))).expect("materializes");
        let suite = w.suite();
        let without = w.evaluate(&d, &suite).expect("scores");
        assert!(without.get("latency").is_none(), "latency is opt-in, so old journals stay comparable");
        w.with_latency = true;
        let with = w.evaluate(&d, &suite).expect("scores");
        let lat = with.get("latency").expect("latency is emitted when asked");
        assert!((0.0..=1.0).contains(&lat), "{lat}");
        assert!(with.get("efficiency").is_some(), "efficiency is kept");
        // The meters here are wall-clock estimates, and the workload says so.
        assert!(matches!(w.energy_basis(&d), Some(Basis::Estimated { .. })), "{:?}", w.energy_basis(&d));
    }

    #[test]
    fn an_unknown_efficiency_basis_is_a_usage_error() {
        assert_eq!(EfficiencyBasis::parse("meter"), Ok(EfficiencyBasis::Meter));
        assert_eq!(EfficiencyBasis::parse("work"), Ok(EfficiencyBasis::Work));
        let e = EfficiencyBasis::parse("watts").expect_err("unknown basis");
        assert!(e.contains("--efficiency-basis") && e.contains("watts"), "{e}");
        let args: Vec<String> =
            ["--heldout", "x", "--journal", "y", "--efficiency-basis", "watts"].iter().map(|s| s.to_string()).collect();
        assert!(cli(&args).expect_err("cli refuses it").contains("neither meter nor work"));
    }

    #[test]
    fn a_bad_learners_spec_is_a_usage_error() {
        for bad in ["bogus", "ssm:0:0.01", "ent:3:12:0.5:0.02"] {
            let args: Vec<String> =
                ["--heldout", "x", "--journal", "y", "--learners", bad].iter().map(|s| s.to_string()).collect();
            let e = cli(&args).expect_err("cli refuses it");
            assert!(e.contains("--learners"), "{bad}: {e}");
        }
    }

    #[test]
    fn work_efficiency_is_deterministic_and_a_real_figure() {
        let mut cfg = base();
        cfg.cost_unit = crate::arena::CostUnit::Fuel;
        let eval = || {
            let mut w = ArenaWorkload::new(cfg.clone(), corpora());
            w.efficiency_basis = EfficiencyBasis::Work;
            let d = w.materialize(&CandidateSpec::new("c", encode(&cfg))).expect("materializes");
            let f = w.evaluate(&d, &w.suite()).expect("scores");
            let basis = w.energy_basis(&d);
            (f.get("capability").unwrap(), f.get("efficiency").unwrap(), basis)
        };
        let (c1, e1, b1) = eval();
        let (c2, e2, _) = eval();
        assert_eq!(c1.to_bits(), c2.to_bits(), "capability is bit-identical");
        assert_eq!(e1.to_bits(), e2.to_bits(), "work efficiency is bit-identical");
        assert!(e1 > 0.0 && e1 < 1.0, "a real figure, not a constant: {e1}");
        match b1 {
            Some(Basis::Estimated { how }) => assert!(how.starts_with("work (deterministic"), "{how}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn work_units_rise_with_parameters_and_bytes() {
        assert!(work_units(10, 100) < work_units(20, 100));
        assert!(work_units(10, 100) < work_units(10, 200));
        assert_eq!(work_units(0, 100), 0.0);
    }

    #[test]
    fn a_changed_training_text_stops_the_next_cycle() {
        let dir = std::env::temp_dir().join(format!("arena-train-pin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("train.txt");
        std::fs::write(&f, b"training text, first version").unwrap();
        let path = f.display().to_string();
        let cfg = Config { train_text: vec![path.clone()], ..base() };
        let mut w = ArenaWorkload::new(cfg, corpora());
        w.train_text_pin = Some(suite_digest(&read_corpora(&[path.clone()]).unwrap()));
        assert!(w.check_train_text_files().is_ok());
        std::fs::write(&f, b"training text, second version").unwrap();
        let err = w.check_train_text_files().unwrap_err();
        assert!(err.contains("training text changed"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn train_text_joins_the_policy_name_only_when_set() {
        let b = EfficiencyBasis::default();
        assert_eq!(full_policy_name(b, false, None, false, false), "succession-cli");
        assert_eq!(full_policy_name(b, false, Some("ng:2:8:0.05"), false, false), "succession-cli+learners=ng:2:8:0.05");
        assert_eq!(full_policy_name(b, true, None, false, false), "succession-cli+train-text");
        assert_eq!(full_policy_name(b, true, None, false, true), "succession-cli+train-text+genome-wide");
    }

    #[test]
    fn training_text_that_is_a_held_out_or_validation_file_is_a_usage_error() {
        let dir = std::env::temp_dir();
        let write = |n: &str, s: &str| {
            let p = dir.join(format!("succ-train-{}-{n}", std::process::id()));
            std::fs::write(&p, s).unwrap();
            p.to_string_lossy().into_owned()
        };
        let (h, v, t) = (write("h", "held out words. ".repeat(100).as_str()), write("v", "validation words. ".repeat(100).as_str()), write("t", "training words. ".repeat(100).as_str()));
        let hv = write("hcopy", "held out words. ".repeat(100).as_str());
        let cli_args = |train: &str| -> Vec<String> {
            ["--heldout", &h, "--validation", &v, "--train-text", train, "--journal", "x.jsonl"].iter().map(|s| s.to_string()).collect()
        };
        for bad in [&h, &v, &hv] {
            let e = cli(&cli_args(bad)).unwrap_err();
            assert!(e.contains("--train-text"), "{bad}: {e}");
        }
        // A disjoint file passes both checks.
        let train = [t.clone()];
        assert!(refuse_train_text_overlap(&train, "--heldout", &read_corpora(&[h.clone()]).unwrap()).is_ok());
        assert!(refuse_train_text_overlap(&train, "--validation", &read_corpora(&[v.clone()]).unwrap()).is_ok());
    }

    #[test]
    fn transfer_reward_joins_the_policy_name_and_its_dev_text_is_checked() {
        let b = EfficiencyBasis::default();
        assert_eq!(full_policy_name(b, false, None, true, false), "succession-cli+reward-transfer");
        let dir = std::env::temp_dir();
        let write = |n: &str, s: &str| {
            let p = dir.join(format!("succ-dev-{}-{n}", std::process::id()));
            std::fs::write(&p, s).unwrap();
            p.to_string_lossy().into_owned()
        };
        let (h, v, t, d) = (
            write("h", "held out words. ".repeat(100).as_str()),
            write("v", "validation words. ".repeat(100).as_str()),
            write("t", "training words. ".repeat(100).as_str()),
            write("d", "dev words. ".repeat(100).as_str()),
        );
        let hcopy = write("hcopy", "held out words. ".repeat(100).as_str());
        let args = |extra: &[&str]| -> Vec<String> {
            ["--heldout", &h, "--validation", &v, "--train-text", &t, "--journal", "x.jsonl"]
                .iter()
                .chain(extra)
                .map(|s| s.to_string())
                .collect()
        };
        let e = cli(&args(&["--reward", "transfer"])).unwrap_err();
        assert!(e.contains("needs --dev-text"), "{e}");
        let e = cli(&args(&["--dev-text", &d])).unwrap_err();
        assert!(e.contains("only used with --reward transfer"), "{e}");
        for bad in [&h, &v, &t, &hcopy] {
            let e = cli(&args(&["--reward", "transfer", "--dev-text", bad])).unwrap_err();
            assert!(e.contains("--dev-text"), "{bad}: {e}");
        }
        // The pin: a changed dev file stops the next cycle.
        let cfg = Config { reward: Reward::Transfer, dev_text: vec![d.clone()], ..base() };
        let mut w = ArenaWorkload::new(cfg, corpora());
        w.dev_text_pin = Some(suite_digest(&read_corpora(&[d.clone()]).unwrap()));
        assert!(w.check_dev_text_files().is_ok());
        std::fs::write(&d, b"dev words, changed").unwrap();
        assert!(w.check_dev_text_files().unwrap_err().contains("dev text changed"));
    }

    #[test]
    fn only_the_work_basis_changes_the_policy_digest() {
        // The default basis keeps the policy name every earlier journal was written under.
        assert_eq!(policy_name(EfficiencyBasis::default()), "succession-cli");
        assert_ne!(policy_name(EfficiencyBasis::Work), policy_name(EfficiencyBasis::Meter));
        let digest = |b| RunnerPolicy { name: policy_name(b), ..RunnerPolicy::default() }.digest();
        let legacy = RunnerPolicy { name: "succession-cli".into(), ..RunnerPolicy::default() }.digest();
        assert_eq!(digest(EfficiencyBasis::Meter), legacy);
        assert_ne!(digest(EfficiencyBasis::Work), legacy);
    }

    #[test]
    fn efficiency_on_estimated_energy_needs_the_flag_and_is_never_allowed_unavailable() {
        let objs: Vec<String> = ["capability", "efficiency"].iter().map(|s| s.to_string()).collect();
        let est = Basis::Estimated { how: "wall-clock seconds × 65 W assumed".into() };
        let refused = check_energy(Some(&objs), &est, false).expect_err("estimated without the flag");
        assert!(refused.contains("--allow-estimated-energy") && refused.contains("faster"), "{refused}");
        assert!(check_energy(Some(&objs), &est, true).is_ok(), "the flag proceeds");
        assert!(check_energy(Some(&objs), &Basis::Measured, false).is_ok());
        let none = Basis::Unavailable { why: "no meter".into() };
        assert!(check_energy(Some(&objs), &none, true).is_err(), "the flag does not make a missing figure real");
        // Without efficiency among the objectives, the energy basis does not gate the run.
        let no_eff: Vec<String> = vec!["capability".into(), "latency".into()];
        assert!(check_energy(Some(&no_eff), &est, false).is_ok());
        assert!(check_energy(None, &est, false).is_ok());
    }

    #[test]
    fn parse_objectives_names_the_axes_and_requires_the_primary() {
        assert_eq!(
            parse_objectives("capability,efficiency,latency").unwrap(),
            vec!["capability", "efficiency", "latency"]
        );
        assert!(parse_objectives("capability,speed").unwrap_err().contains("not an axis"));
        assert!(parse_objectives("efficiency,latency").unwrap_err().contains("primary"));
        assert!(parse_objectives("").is_err());
    }

    #[test]
    fn tolerances_parse_per_axis_and_refuse_bad_values() {
        let ok = parse_tolerances("latency=0.005, efficiency=0.01").unwrap();
        assert_eq!(ok, vec![("latency".to_string(), 0.005), ("efficiency".to_string(), 0.01)]);
        assert!(parse_tolerances("latency=-0.1").unwrap_err().contains("non-negative"));
        assert!(parse_tolerances("latency=NaN").unwrap_err().contains("finite"));
        assert!(parse_tolerances("latency=inf").unwrap_err().contains("finite"));
        assert!(parse_tolerances("speed=0.1").unwrap_err().contains("not an axis"));
        assert!(parse_tolerances("latency").unwrap_err().contains("AXIS=VALUE"));
    }

    #[test]
    fn tolerances_must_name_a_listed_non_primary_objective() {
        let objs: Vec<String> = ["capability", "latency"].iter().map(|s| s.to_string()).collect();
        let lat = vec![("latency".to_string(), 0.005)];
        let map = validate_tolerances(Some(&objs), &lat).unwrap();
        assert_eq!(map.get("latency"), Some(&0.005));
        assert!(validate_tolerances(None, &lat).unwrap_err().contains("needs --objectives"));
        let cap = vec![("capability".to_string(), 0.01)];
        assert!(validate_tolerances(Some(&objs), &cap).unwrap_err().contains("always zero"));
        // Efficiency is an objective like any other under rule (b), so it may be given one.
        let eff_objs: Vec<String> = ["capability", "efficiency"].iter().map(|s| s.to_string()).collect();
        let eff = vec![("efficiency".to_string(), 0.01)];
        assert_eq!(validate_tolerances(Some(&eff_objs), &eff).unwrap().get("efficiency"), Some(&0.01));
        let unlisted = vec![("latency".to_string(), 0.01)];
        let only_cap: Vec<String> = vec!["capability".into()];
        assert!(validate_tolerances(Some(&only_cap), &unlisted).unwrap_err().contains("not among"));
        let twice = vec![("latency".to_string(), 0.01), ("latency".to_string(), 0.02)];
        assert!(validate_tolerances(Some(&objs), &twice).unwrap_err().contains("more than once"));
    }

    #[test]
    fn an_unknown_cost_unit_is_a_usage_error() {
        let args: Vec<String> = ["--cost", "watts"].iter().map(|s| s.to_string()).collect();
        let err = cli(&args).unwrap_err();
        assert!(err.contains("--cost"), "{err}");
    }
}
