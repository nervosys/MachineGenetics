//! The runner: driving succession cycles without a person in the room.
//!
//! Everything else in [`germline`](super) is a mechanism a caller operates. This
//! is the thing that operates it, and it is the component where the difference
//! between "supports unattended operation" and "runs unattended safely" lives.
//!
//! ## Bounded by construction
//!
//! [`Runner::run`] takes a cycle budget and returns. There is no `loop {}`, no
//! daemon mode, and no way to ask for unlimited cycles — `max_cycles` is a `u32`
//! the caller must supply, and the runner stops when it is exhausted.
//!
//! This is deliberate and slightly inconvenient. A process that runs forever has
//! no natural moment at which anyone looks at it, and "it has been improving
//! itself for three weeks" is a sentence that should be hard to arrive at by
//! accident. A bounded run ends, prints what happened, and requires a decision to
//! continue. Restarting it is one command; that is the correct amount of friction.
//!
//! ## Stopping is the interesting behaviour
//!
//! A search loop's failure mode is not crashing — it is continuing. Four halt
//! conditions, each for a different way "keep going" becomes wrong:
//!
//! | Condition | Why continuing would be wrong |
//! |---|---|
//! | [`Halt::BudgetExhausted`] | the normal end; someone should look |
//! | [`Halt::Demoted`] | a promoted successor failed. Something about the gate or the suite is wrong, and proposing more candidates against the same criteria compounds the error |
//! | [`Halt::SearchStalled`] | N consecutive refusals. The search is not finding anything the gate accepts; more cycles are a slower way to learn that |
//! | [`Halt::WorkloadFailed`] | candidates cannot be produced or measured at all. Cycling would fill the journal with nothing |
//!
//! The demotion case is the one that matters most. The tempting behaviour —
//! fall back, then keep searching — treats a failed promotion as a bad draw. But
//! a successor that passed the gate and then failed in production is evidence
//! that *the gate did not measure something it needed to*, and running the same
//! gate again is running a test that has just been shown to be incomplete.
//! Halting forces the criteria to be revisited by someone who can revise them.
//!
//! ## The policy is pinned
//!
//! [`RunnerPolicy::digest`] is recorded in the journal and carried in the
//! [`Authority::Unattended`](super::cycle::Authority) that authorizes each
//! promotion. An unattended loop whose governing rules cannot be reconstructed
//! afterwards is not auditable, and "what was it allowed to do at the time?" is
//! the first question anyone will ask.

use super::cycle::{Authority, Cycle, CycleError};
use super::directed::CandidateSpec;
use super::gate::Episode;
use super::journal::{Entry, Journal};
use super::lineage::Lineage;
use super::supervisor::{FailureMode, HealthSample, SupervisionPolicy, Supervisor};
use super::variation::{propose, VariationPlan};
use super::{EvalSuite, FitnessVector, Generation, GenerationId, Measurement};
use crate::mac::absorb;
use crate::ribosome::Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// What the runner is permitted to do. Pinned by digest and journalled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerPolicy {
    /// Human-readable name, so an audit can ask what was in force.
    pub name: String,
    /// Hard ceiling on cycles for one invocation.
    pub max_cycles: u32,
    /// Consecutive refusals before concluding the search is stuck.
    pub max_consecutive_refusals: u32,
    /// Stop after a demotion instead of continuing to search. Defaults to true;
    /// see the module note on why continuing is the wrong instinct.
    pub halt_on_demotion: bool,
    /// Candidates proposed per cycle.
    pub candidates_per_cycle: usize,
    /// Shadow observations to collect before adjudicating.
    pub shadow_runs: u32,
    pub variation: VariationPlan,
    pub supervision: SupervisionPolicy,
}

impl Default for RunnerPolicy {
    fn default() -> Self {
        RunnerPolicy {
            name: "default-bounded".into(),
            max_cycles: 8,
            max_consecutive_refusals: 3,
            halt_on_demotion: true,
            candidates_per_cycle: 4,
            shadow_runs: 8,
            variation: VariationPlan::default(),
            supervision: SupervisionPolicy::default(),
        }
    }
}

impl RunnerPolicy {
    pub fn digest(&self) -> Digest {
        let body = serde_json::to_string(self).unwrap_or_default();
        let mut h = Sha256::new();
        h.update(b"germline-runner-policy-v1");
        absorb(&mut h, body.as_bytes());
        Digest(format!("{:x}", h.finalize()))
    }

    /// The authority this policy confers on a promotion.
    pub fn authority(&self) -> Authority {
        Authority::Unattended { policy: self.name.clone(), policy_digest: self.digest() }
    }
}

/// Why the runner stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "halt", rename_all = "snake_case")]
pub enum Halt {
    BudgetExhausted { cycles: u32 },
    Demoted { generation: GenerationId, mode: String, fell_back_to: GenerationId },
    SearchStalled { consecutive_refusals: u32 },
    WorkloadFailed { detail: String },
    /// A fallback was needed and none was usable. The most serious stop: the
    /// system is running something it has decided it should not be running.
    NoFallbackAvailable { generation: GenerationId, detail: String },
}

/// What one cycle did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CycleOutcome {
    pub cycle: u32,
    pub candidate: Option<GenerationId>,
    pub promoted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub policy: String,
    pub policy_digest: Digest,
    pub cycles: Vec<CycleOutcome>,
    pub promotions: u32,
    pub refusals: u32,
    pub halt: Halt,
    /// Journal head at the end — the anchor for the whole run.
    pub journal_head: Option<Digest>,
}

impl RunReport {
    pub fn json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// The seam to actual model work.
///
/// Implementing this is what turns the control plane into a system that improves
/// something. It is a trait rather than a concrete type because training belongs
/// to the workload, not to the mechanism that governs it — and because a test
/// needs a workload it can predict.
pub trait Workload {
    /// Turn a proposed genome into a real artifact, returning its digest.
    fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String>;

    /// Measure an artifact on the held-out suite.
    fn evaluate(&mut self, artifact: &Digest, suite: &EvalSuite)
        -> Result<FitnessVector, String>;

    /// Run one unit of shadow work with a candidate.
    fn shadow(&mut self, artifact: &Digest) -> HealthSample;

    /// Run one unit of production work with the current champion.
    fn observe_champion(&mut self, artifact: &Digest) -> HealthSample;

    /// Is this artifact present and runnable? Governs fallback validity.
    fn materialized(&self, artifact: &Digest) -> bool;
}

/// Drives bounded succession cycles.
pub struct Runner<'a> {
    pub policy: RunnerPolicy,
    pub episode: &'a Episode,
    pub attestor: &'a super::attest::Attestor,
    pub suite: EvalSuite,
    /// Seed for the first cycle; each subsequent cycle derives from it, so a
    /// whole run is reproducible from one number.
    pub seed: u64,
}

impl<'a> Runner<'a> {
    pub fn new(
        policy: RunnerPolicy,
        episode: &'a Episode,
        attestor: &'a super::attest::Attestor,
        suite: EvalSuite,
        seed: u64,
    ) -> Self {
        Runner { policy, episode, attestor, suite, seed }
    }

    /// Run until a halt condition. Never runs unbounded.
    pub fn run(
        &mut self,
        lineage: &mut Lineage,
        journal: &mut Journal,
        workload: &mut dyn Workload,
    ) -> RunReport {
        let policy_digest = self.policy.digest();
        let _ = journal.append(Entry::Note {
            text: format!(
                "unattended run opened under policy `{}`@{} for at most {} cycles",
                self.policy.name,
                policy_digest.short(),
                self.policy.max_cycles
            ),
        });

        let mut report = RunReport {
            policy: self.policy.name.clone(),
            policy_digest: policy_digest.clone(),
            cycles: Vec::new(),
            promotions: 0,
            refusals: 0,
            halt: Halt::BudgetExhausted { cycles: 0 },
            journal_head: None,
        };

        let mut consecutive_refusals = 0u32;

        for cycle_index in 0..self.policy.max_cycles {
            let seed = self.seed.wrapping_add(cycle_index as u64).wrapping_mul(0x9E37_79B9);

            match self.one_cycle(cycle_index, seed, lineage, journal, workload) {
                Ok(outcome) => {
                    if outcome.promoted {
                        report.promotions += 1;
                        consecutive_refusals = 0;

                        // Supervise the new champion before proposing anything
                        // else. Stacking a proposal on an unvalidated promotion
                        // means a failure is attributed to the wrong generation.
                        if let Some(halt) =
                            self.supervise(lineage, journal, workload, outcome.candidate)
                        {
                            report.cycles.push(outcome);
                            report.halt = halt;
                            report.journal_head = journal.head().cloned();
                            return report;
                        }
                    } else {
                        report.refusals += 1;
                        consecutive_refusals += 1;
                    }
                    report.cycles.push(outcome);

                    if consecutive_refusals >= self.policy.max_consecutive_refusals {
                        report.halt = Halt::SearchStalled { consecutive_refusals };
                        report.journal_head = journal.head().cloned();
                        let _ = journal.append(Entry::Note {
                            text: format!(
                                "halting: {consecutive_refusals} consecutive refusals — the search is not finding candidates this gate accepts"
                            ),
                        });
                        return report;
                    }
                }
                Err(detail) => {
                    report.halt = Halt::WorkloadFailed { detail: detail.clone() };
                    let _ = journal
                        .append(Entry::Note { text: format!("halting: workload failed: {detail}") });
                    report.journal_head = journal.head().cloned();
                    return report;
                }
            }
        }

        report.halt = Halt::BudgetExhausted { cycles: self.policy.max_cycles };
        let _ = journal.append(Entry::Note {
            text: format!(
                "run complete: {} cycles, {} promotions, {} refusals",
                self.policy.max_cycles, report.promotions, report.refusals
            ),
        });
        report.journal_head = journal.head().cloned();
        report
    }

    /// One propose → materialize → evaluate → shadow → adjudicate → commit pass.
    fn one_cycle(
        &mut self,
        index: u32,
        seed: u64,
        lineage: &mut Lineage,
        journal: &mut Journal,
        workload: &mut dyn Workload,
    ) -> Result<CycleOutcome, String> {
        // Population = the lineage's measured generations.
        let population: Vec<(Vec<f64>, f64)> = lineage
            .generations()
            .iter()
            .filter_map(|g| g.heldout_fitness().map(|f| (vec![f.composite()], f.composite())))
            .collect();
        let seedpop = if population.is_empty() { vec![(vec![0.5], 0.5)] } else { population };

        let specs = propose(&seedpop, self.policy.variation, seed);
        let spec = specs
            .into_iter()
            .take(self.policy.candidates_per_cycle)
            .next()
            .ok_or_else(|| "variation produced no candidates".to_string())?;

        let artifact = workload.materialize(&spec)?;
        let fitness = workload.evaluate(&artifact, &self.suite)?;

        let id = lineage.next_id();
        let parent = lineage.champion().map(|g| g.id);
        let mut generation = Generation::new(id, artifact.clone())
            .note(format!("cycle {index}, spec {}", spec.id));
        if let Some(p) = parent {
            generation = generation.parent(p);
        }

        let mut cycle = Cycle::propose(generation, seed, &self.policy.name, journal)
            .map_err(|e| e.to_string())?;

        cycle
            .evaluate(
                Measurement {
                    suite: self.suite.clone(),
                    fitness,
                    evaluator: self.attestor.evaluator().to_string(),
                },
                journal,
            )
            .map_err(|e| e.to_string())?;

        for _ in 0..self.policy.shadow_runs {
            let sample = workload.shadow(&artifact);
            cycle.shadow(&sample).map_err(|e| e.to_string())?;
        }

        let materialized = |d: &Digest| workload.materialized(d);
        let verdict = cycle
            .adjudicate(self.episode, lineage, self.attestor, &materialized, journal)
            .map_err(|e| e.to_string())?;
        let approved = verdict.approved();
        let reasons = verdict.reasons();

        if !approved {
            // Drive the refusal through commit so the lineage and journal both
            // record it, then carry on.
            let _ = cycle.commit(lineage, self.attestor, self.policy.authority(), journal);
            return Ok(CycleOutcome { cycle: index, candidate: Some(id), promoted: false, reasons });
        }

        match cycle.commit(lineage, self.attestor, self.policy.authority(), journal) {
            Ok(_) => Ok(CycleOutcome {
                cycle: index,
                candidate: Some(id),
                promoted: true,
                reasons: Vec::new(),
            }),
            Err(CycleError::Rejected { reasons }) => {
                Ok(CycleOutcome { cycle: index, candidate: Some(id), promoted: false, reasons })
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Watch the new champion; demote and halt if it misbehaves.
    fn supervise(
        &self,
        lineage: &mut Lineage,
        journal: &mut Journal,
        workload: &mut dyn Workload,
        promoted: Option<GenerationId>,
    ) -> Option<Halt> {
        let champion = lineage.champion()?;
        let generation = champion.id;
        let artifact = champion.artifact.clone();
        let baseline =
            champion.heldout_fitness().map(|f| f.composite()).unwrap_or(0.0);

        let mut supervisor = Supervisor::new(self.policy.supervision.clone(), baseline);
        let window = self.policy.supervision.window.max(1);

        for _ in 0..window {
            let sample = workload.observe_champion(&artifact);
            let Some(mode) = supervisor.observe(sample) else { continue };

            let _ = journal
                .append(Entry::Failure { generation, mode: mode.to_string() });

            let materialized = |d: &Digest| workload.materialized(d);
            return match lineage.demote_champion(&mode.to_string(), &materialized) {
                Ok(fell_back_to) => {
                    if let Some(event) = lineage.events().last().cloned() {
                        let _ = journal.append(Entry::Succession { event });
                    }
                    let _ = journal.append(Entry::Note {
                        text: format!(
                            "halting after demotion: the gate approved {generation} and production disagreed — the criteria need revisiting, not another candidate"
                        ),
                    });
                    Some(Halt::Demoted {
                        generation: promoted.unwrap_or(generation),
                        mode: mode.to_string(),
                        fell_back_to,
                    })
                }
                Err(e) => {
                    let _ = journal.append(Entry::Note {
                        text: format!(
                            "CRITICAL: {generation} must be demoted ({mode}) but no fallback is usable: {e}"
                        ),
                    });
                    Some(Halt::NoFallbackAvailable {
                        generation,
                        detail: e.to_string(),
                    })
                }
            };
        }

        // Healthy through the window; only stop early if the policy says so.
        let _ = FailureMode::Stall { window };
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::germline::attest::Attestor;
    use crate::germline::gate::PromotionGate;
    use crate::germline::{SuiteKind};
    use std::collections::HashSet;
    use std::path::PathBuf;

    const SUITE: &[u8] = b"runner-suite";
    const EVALUATOR: &str = "independent-harness";

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "germline-runner-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p.join("j.jsonl")
    }

    fn suite() -> EvalSuite {
        EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE))
    }

    fn attestor() -> Attestor {
        Attestor::new(EVALUATOR, b"key".to_vec())
    }

    fn episode() -> Episode {
        Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.01,
                guard_axes: vec!["safety".into()],
                guard_tolerance: 0.01,
                min_shadow_successes: 2,
            },
            Digest::of(SUITE),
            EVALUATOR,
        )
    }

    fn policy() -> RunnerPolicy {
        RunnerPolicy {
            max_cycles: 4,
            max_consecutive_refusals: 2,
            shadow_runs: 2,
            supervision: SupervisionPolicy { window: 3, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        }
    }

    /// A workload whose candidates improve by a fixed step each cycle.
    struct Improving {
        capability: f64,
        step: f64,
        safety: f64,
        artifacts: HashSet<String>,
        champion_healthy: bool,
        fail_materialize: bool,
    }

    impl Improving {
        fn new(step: f64) -> Self {
            Improving {
                capability: 0.50,
                step,
                safety: 0.95,
                artifacts: HashSet::new(),
                champion_healthy: true,
                fail_materialize: false,
            }
        }
    }

    impl Workload for Improving {
        fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
            if self.fail_materialize {
                return Err("trainer unavailable".into());
            }
            self.capability = (self.capability + self.step).min(1.0);
            let d = Digest::of(spec.id.as_bytes());
            self.artifacts.insert(d.0.clone());
            Ok(d)
        }

        fn evaluate(
            &mut self,
            _artifact: &Digest,
            _suite: &EvalSuite,
        ) -> Result<FitnessVector, String> {
            Ok(FitnessVector::new()
                .with("capability", self.capability)
                .with("safety", self.safety))
        }

        fn shadow(&mut self, _artifact: &Digest) -> HealthSample {
            HealthSample::ok()
        }

        fn observe_champion(&mut self, _artifact: &Digest) -> HealthSample {
            if self.champion_healthy {
                HealthSample::ok()
            } else {
                HealthSample::failed()
            }
        }

        fn materialized(&self, artifact: &Digest) -> bool {
            self.artifacts.contains(&artifact.0) || artifact.0.starts_with("seed")
        }
    }

    fn seeded(w: &mut Improving) -> Lineage {
        let mut l = Lineage::new();
        let id = l.next_id();
        let artifact = Digest::of(b"seed-champion");
        w.artifacts.insert(artifact.0.clone());
        l.add(Generation::new(id, artifact).measured(Measurement {
            suite: suite(),
            fitness: FitnessVector::new().with("capability", 0.50).with("safety", 0.95),
            evaluator: EVALUATOR.into(),
        }));
        l.promote(id, episode().gate_digest, EVALUATOR).unwrap();
        l
    }

    #[test]
    fn a_run_is_bounded_by_its_budget() {
        let path = tmp("bounded");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 1);

        let report = r.run(&mut l, &mut j, &mut w);
        assert!(
            matches!(report.halt, Halt::BudgetExhausted { cycles: 4 }),
            "a run must end on its own: {:?}",
            report.halt
        );
        assert_eq!(report.cycles.len(), 4);
        assert!(report.promotions > 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_stalled_search_halts_rather_than_burning_the_budget() {
        let path = tmp("stalled");
        let mut j = Journal::open(&path).unwrap();
        // No improvement at all → every candidate refused.
        let mut w = Improving::new(0.0);
        let mut l = seeded(&mut w);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(
            RunnerPolicy { max_cycles: 50, ..policy() },
            &ep,
            &at,
            suite(),
            2,
        );

        let report = r.run(&mut l, &mut j, &mut w);
        assert!(
            matches!(report.halt, Halt::SearchStalled { .. }),
            "a search that finds nothing must stop early: {:?}",
            report.halt
        );
        assert!(report.cycles.len() < 50);
        assert_eq!(report.promotions, 0);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_demotion_halts_the_run_instead_of_proposing_another_candidate() {
        let path = tmp("demote");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        w.champion_healthy = false; // whatever gets promoted will fail in production
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 3);

        let report = r.run(&mut l, &mut j, &mut w);
        match &report.halt {
            Halt::Demoted { fell_back_to, .. } => {
                assert_eq!(
                    l.champion().unwrap().id,
                    *fell_back_to,
                    "authority must be back with the predecessor"
                );
            }
            other => panic!("expected a halt on demotion, got {other:?}"),
        }
        assert_eq!(report.cycles.len(), 1, "it must not keep searching after a failed promotion");

        // And the reasoning is on the record.
        assert!(j.replay().unwrap().iter().any(|r| matches!(
            &r.entry,
            Entry::Note { text } if text.contains("criteria need revisiting")
        )));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_broken_workload_halts_rather_than_filling_the_journal() {
        let path = tmp("workload");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        w.fail_materialize = true;
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 4);

        let report = r.run(&mut l, &mut j, &mut w);
        assert!(matches!(report.halt, Halt::WorkloadFailed { .. }), "{:?}", report.halt);
        assert!(report.cycles.is_empty());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn promotions_are_authorized_by_the_pinned_policy() {
        let p = policy();
        match p.authority() {
            Authority::Unattended { policy, policy_digest } => {
                assert_eq!(policy, p.name);
                assert_eq!(policy_digest, p.digest());
            }
            other => panic!("an unattended run must not claim operator authority: {other:?}"),
        }
    }

    #[test]
    fn changing_the_policy_changes_its_pin() {
        let a = policy();
        let mut b = policy();
        b.max_cycles += 1;
        assert_ne!(a.digest(), b.digest(), "the audit anchor must track the rules");
    }

    #[test]
    fn a_run_is_reproducible_from_its_seed() {
        let run = |seed: u64| {
            let path = tmp(&format!("repro{seed}"));
            let mut j = Journal::open(&path).unwrap();
            let mut w = Improving::new(0.05);
            let mut l = seeded(&mut w);
            let ep = episode();
            let at = attestor();
            let mut r = Runner::new(policy(), &ep, &at, suite(), seed);
            let report = r.run(&mut l, &mut j, &mut w);
            let _ = std::fs::remove_dir_all(path.parent().unwrap());
            report.cycles.len()
        };
        assert_eq!(run(99), run(99), "same seed, same run");
    }

    #[test]
    fn the_whole_run_is_journalled_and_the_chain_holds() {
        let path = tmp("journalled");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 5);

        let report = r.run(&mut l, &mut j, &mut w);
        assert!(j.verify().is_ok(), "the record of an unattended run must be intact");
        assert_eq!(report.journal_head, j.head().cloned());

        // The opening note records the policy under which it ran.
        assert!(j.replay().unwrap().iter().any(|r| matches!(
            &r.entry,
            Entry::Note { text } if text.contains("unattended run opened under policy")
        )));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
