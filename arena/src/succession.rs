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
use germline::directed::CandidateSpec;
use germline::runner::Workload;
use germline::supervisor::HealthSample;
use germline::variation::{Genome, Locus};
use germline::{EvalSuite, FitnessVector};
use ribosome::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;

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
    built: HashMap<String, Built>,
}

impl ArenaWorkload {
    pub fn new(base: Config, corpora: Vec<Corpus>) -> ArenaWorkload {
        ArenaWorkload { base, corpora, cpu_watts: 65.0, built: HashMap::new() }
    }

    /// The suite this workload's corpora are registered as.
    pub fn suite(&self) -> EvalSuite {
        EvalSuite::new("arena-heldout", germline::SuiteKind::HeldOut, suite_digest(&self.corpora))
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
