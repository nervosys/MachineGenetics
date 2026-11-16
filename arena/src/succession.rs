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

use crate::arena::{run, Config, Corpus, Report};
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
    cfg
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
        let built = Built {
            mean_bpb: learner.mean_bpb,
            joules: learner.train_joules,
            latency_s_per_kb: learner.latency_s_per_kb,
            energy: session_energy_basis(&report),
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
        self.check_heldout_files()
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
[--objective-tolerance AXIS=F,...] [--allow-estimated-energy] \
[--energy-budget JOULES] [--param-search es|perturb] [--cost joules|fuel]";

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
    let mut energy_budget: Option<f64> = None;
    let mut param_search = ParamSearch::Perturb;
    let mut base = Config::default();
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
            "--energy-budget" => energy_budget = Some(parse_budget(v)?),
            "--param-search" => param_search = parse_param_search(v)?,
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

    let mut w = ArenaWorkload::new(base.clone(), read_corpora(&heldout)?);
    w.heldout_files = heldout.clone();
    w.with_latency = objectives.as_ref().is_some_and(|o| o.iter().any(|n| n == "latency"));
    let suite = w.suite();
    let digest = suite.digest.clone();

    // The champion a fresh run starts from: the base configuration, measured on
    // the suite. Built the same way on every invocation, so a resumed journal
    // replays onto the same genesis.
    let base_genome = encode(&base);
    let seed_art = w.materialize(&CandidateSpec::new("seed", base_genome.clone()))?;
    let seed_fit = w.evaluate(&seed_art, &suite)?;
    let basis = w.energy_basis(&seed_art).unwrap_or_else(|| Basis::Unavailable {
        why: "the seed session recorded no energy basis".into(),
    });
    check_energy(objectives.as_deref(), &basis, allow_estimated)?;
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
    let policy = RunnerPolicy {
        name: "succession-cli".into(),
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
    if json {
        let out = serde_json::json!({
            "suite_digest": digest.0,
            "journal": journal_path.display().to_string(),
            "objectives": objectives_json(objectives.as_deref(), dominance.as_ref()),
            "energy_basis": basis,
            "energy_spend_basis": spend_basis,
            "report": report,
        });
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
