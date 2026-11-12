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

use crate::arena::{run, Config, Corpus};
use crate::energy::{Meter, WallClock};
use crate::learner::{LearnerConfig, LearnerSpec};
use germline::attest::Attestor;
use germline::directed::CandidateSpec;
use germline::gate::{Episode, PromotionGate};
use germline::journal::Journal;
use germline::lineage::Lineage;
use germline::runner::{Halt, Runner, RunnerPolicy, Workload};
use germline::supervisor::HealthSample;
use germline::variation::{Genome, Locus};
use germline::{EvalSuite, FitnessVector, Generation, Measurement};
use ribosome::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
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
    /// Canonical hashes of the session's best programs: what it learned from.
    definitions: Vec<String>,
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
    built: HashMap<String, Built>,
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
        ArenaWorkload { base, corpora, cpu_watts: 65.0, heldout_files: Vec::new(), built: HashMap::new() }
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
        self.built.insert(d.0.clone(), Built { mean_bpb: learner.mean_bpb, joules: learner.train_joules, definitions });
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
        let capability = (1.0 - b.mean_bpb / 8.0).clamp(0.0, 1.0);
        let efficiency = b.joules.map(|j| 1.0 / (1.0 + j / 1000.0)).unwrap_or(0.0);
        Ok(FitnessVector::new().with("capability", capability).with("efficiency", efficiency))
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
fn cli_gate() -> PromotionGate {
    PromotionGate {
        primary_axis: "capability".into(),
        min_improvement: 0.01,
        guard_axes: vec!["efficiency".into()],
        // Wall-clock energy is noisy at this scale; the guard is present, and loose.
        guard_tolerance: 0.25,
        // Shadow runs here are a finiteness check, so the germline default holds.
        min_shadow_successes: 8,
        dominance: None,
    }
}

const USAGE: &str = "usage: arena succession --heldout FILE[,FILE...] --journal PATH [--cycles N] [--seed N] \
[--stop-file PATH] [--force] [--json] [--rounds N] [--programs N] [--candidates N] [--agents K:N,...] \
[--proposer-command \"CMD ARGS\"] [--proposer-timeout S]";

fn num(v: &str, flag: &str) -> Result<u64, String> {
    v.parse().map_err(|_| format!("{flag}: `{v}` is not a number"))
}

fn parse_agents(list: &str) -> Result<Vec<(String, usize)>, String> {
    list.split(',')
        .map(|p| {
            let (k, n) = p.split_once(':').unwrap_or((p, "1"));
            Ok((k.to_string(), num(n, "--agents")? as usize))
        })
        .collect()
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
        Halt::BudgetExhausted { .. } | Halt::StopRequested { .. } | Halt::SearchStalled { .. } => 0,
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
    let mut base = Config::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        if matches!(flag, "-h" | "--help") {
            println!("{USAGE}");
            return Ok(0);
        }
        let takes_value = !matches!(flag, "--force" | "--json");
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

    let mut w = ArenaWorkload::new(base.clone(), read_corpora(&heldout)?);
    w.heldout_files = heldout.clone();
    let suite = w.suite();
    let digest = suite.digest.clone();

    // The champion a fresh run starts from: the base configuration, measured on
    // the suite. Built the same way on every invocation, so a resumed journal
    // replays onto the same genesis.
    let base_genome = encode(&base);
    let seed_art = w.materialize(&CandidateSpec::new("seed", base_genome.clone()))?;
    let seed_fit = w.evaluate(&seed_art, &suite)?;
    let mut genesis = Lineage::new();
    let id = genesis.next_id();
    let mut g = Generation::new(id, seed_art.clone()).measured(Measurement {
        suite: suite.clone(),
        fitness: seed_fit,
        evaluator: CLI_EVALUATOR.into(),
    });
    g.genome = Some(base_genome);
    genesis.add(g);
    let ep = Episode::open(cli_gate(), suite.digest.clone(), CLI_EVALUATOR);
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

    let policy = RunnerPolicy {
        name: "succession-cli".into(),
        max_cycles: cycles,
        candidates_per_cycle: candidates,
        stop_file: stop_file.map(PathBuf::from),
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
        runner = runner.resume(&resumed);
        let r = runner.run(&mut resumed.lineage, &mut journal, &mut w);
        (r, resumed.lineage)
    };

    if json {
        let out = serde_json::json!({
            "suite_digest": digest.0,
            "journal": journal_path.display().to_string(),
            "report": report,
        });
        println!("{}", serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?);
    } else {
        println!("succession: held-out suite digest {}", digest.0);
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
}
