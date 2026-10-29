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
use super::directed::{Calibration, CandidateSpec, DirectedSearch, FitnessPredictor};
use super::gate::Episode;
use super::journal::{Entry, Journal};
use super::lineage::Lineage;
use super::manifest::{self, ProposerRecord};
use super::supervisor::{FailureMode, HealthSample, SupervisionPolicy, Supervisor};
use crate::variation::{propose, GenePool, Legality, Locus, Proposal, VariationPlan};
use super::{EvalSuite, FitnessVector, Generation, GenerationId, Measurement};
use ribosome::mac::absorb;
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

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

    /// Content hashes of the definitions `artifact` was built from, for the
    /// journal's attribution (plan 2.4). None by default.
    fn attribution(&self, _artifact: &Digest) -> Vec<String> {
        Vec::new()
    }
}

/// An external source of candidate edits: an agent, not the built-in search.
///
/// A proposer's edits are not sampled from the lineage's own population. Each
/// one must carry a [`crate::manifest::PredictionManifest`], and the proposer's
/// trust, earned from how its predictions scored, decides how many of its
/// candidates a cycle admits (see [`admission_width`]).
pub trait Proposer {
    /// The name the proposer is known by in the journal and in its record.
    fn name(&self) -> &str;

    /// Candidate edits against the current champion's held-out fitness, if
    /// there is one. Each should state its manifest; a candidate that does not
    /// is refused, not materialized.
    fn propose(&mut self, champion: Option<&FitnessVector>, seed: u64) -> Vec<CandidateSpec>;
}

/// How many of a proposer's candidates a cycle admits.
///
/// This is the trust mechanism for external edits: `max(1, round(trust *
/// candidates_per_cycle))`. A proposer with no record admits one candidate
/// per cycle, and a proven one may admit the whole batch. The floor of one
/// keeps an untrusted proposer able to make progress, so that it can earn
/// trust at all.
pub fn admission_width(trust: f64, candidates_per_cycle: usize) -> usize {
    ((trust.clamp(0.0, 1.0) * candidates_per_cycle as f64).round() as usize).max(1)
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
    /// The surrogate, if there is one.
    ///
    /// **`None` is the honest default and was the only behaviour.** `directed`
    /// defines `FitnessPredictor`, `DirectedSearch` and the calibration that
    /// earns trust, and this module imported exactly one name from it —
    /// `CandidateSpec`. The loop proposed eight offspring and ran
    /// `.take(candidates_per_cycle).next()`, which is element 0 for any
    /// non-zero policy: seven proposals computed and dropped, a policy field
    /// that changed nothing, and lib.rs describing the crate as proposing "a
    /// higher-fitness successor by *directed* evolution".
    ///
    /// With `None` the loop samples uniformly from the proposals, which is
    /// undirected search and says so. With a predictor it ranks them and
    /// narrows as trust is earned.
    predictor: Option<Box<dyn FitnessPredictor>>,
    /// Trust in that predictor — moved only by measurements, never asserted.
    calibration: Calibration,
    /// The external proposer, if one is attached. See [`Proposer`].
    proposer: Option<Box<dyn Proposer>>,
    /// Each proposer's standing, by name. Moved only by scored predictions.
    proposer_records: BTreeMap<String, ProposerRecord>,
    /// The cycle this invocation starts at, and the refusals carried into it:
    /// zero for a fresh run, and what [`Lineage::replay`] found for a resumed
    /// one, so the cycle seeds and the stall count continue where they were.
    first_cycle: u32,
    carried_refusals: u32,
}

impl<'a> Runner<'a> {
    pub fn new(
        policy: RunnerPolicy,
        episode: &'a Episode,
        attestor: &'a super::attest::Attestor,
        suite: EvalSuite,
        seed: u64,
    ) -> Self {
        Runner {
            policy,
            episode,
            attestor,
            suite,
            seed,
            predictor: None,
            calibration: Calibration::new(),
            proposer: None,
            proposer_records: BTreeMap::new(),
            first_cycle: 0,
            carried_refusals: 0,
        }
    }

    /// Continue a run whose lineage was rebuilt by [`Lineage::replay`]. With
    /// no predictor attached, the resumed run makes the choices the
    /// uninterrupted one would have, and each external proposer resumes with
    /// the trust its journalled predictions earned. A predictor's calibration
    /// is not in the journal and restarts at zero trust.
    pub fn resume(mut self, from: &super::lineage::Resumed) -> Self {
        self.first_cycle = from.proposals;
        self.carried_refusals = from.trailing_refusals;
        for (name, record) in &from.proposers {
            self.proposer_records.insert(name.clone(), record.clone());
        }
        self
    }

    /// Run with a surrogate ranking the proposals.
    ///
    /// The predictor starts at zero trust and earns it by being right about
    /// candidates that were subsequently measured, so attaching one does not
    /// immediately narrow the search — which is the point. A surrogate
    /// concentrates the budget, and a confidently wrong one concentrates it on
    /// the wrong candidates while returning an answer either way.
    pub fn with_predictor(mut self, predictor: Box<dyn FitnessPredictor>) -> Self {
        self.predictor = Some(predictor);
        self
    }

    /// Trust the attached predictor has earned so far. `0.0` with none attached.
    pub fn predictor_trust(&self) -> f64 {
        if self.predictor.is_some() {
            self.calibration.trust()
        } else {
            0.0
        }
    }

    /// Take candidate edits from an external proposer instead of the built-in
    /// search.
    ///
    /// With a proposer attached, the undirected `variation::propose` is not
    /// called, and every candidate must state a prediction. Refused candidates
    /// are journalled as notes and never materialized. A cycle whose candidates
    /// are all refused counts as a refusal toward the stall limit.
    ///
    /// Resumable: [`Lineage::replay`] rebuilds each proposer's record from its
    /// `Calibrated` entries and counts a cycle whose candidates were all
    /// refused (a `CycleRefused` entry), so [`Runner::resume`] continues both
    /// the cycle numbering and the stall.
    pub fn with_proposer(mut self, proposer: Box<dyn Proposer>) -> Self {
        self.proposer_records.entry(proposer.name().to_string()).or_default();
        self.proposer = Some(proposer);
        self
    }

    /// Trust the named proposer has earned, in `[0,1]`. `0.0` if it is
    /// unknown or has fewer than [`ProposerRecord::MIN_SAMPLES`] scored
    /// predictions.
    pub fn proposer_trust(&self, name: &str) -> f64 {
        self.proposer_records.get(name).map_or(0.0, ProposerRecord::trust)
    }

    /// Run until a halt condition. Never runs unbounded.
    pub fn run(
        &mut self,
        lineage: &mut Lineage,
        journal: &mut Journal,
        workload: &mut dyn Workload,
    ) -> RunReport {
        let policy_digest = self.policy.digest();
        let opened = if self.first_cycle == 0 { "opened".to_string() } else { format!("resumed at cycle {}", self.first_cycle) };
        let _ = journal.append(Entry::Note {
            text: format!(
                "unattended run {opened} under policy `{}`@{} for at most {} cycles",
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

        let mut consecutive_refusals = self.carried_refusals;

        for cycle_index in self.first_cycle..self.first_cycle.saturating_add(self.policy.max_cycles) {
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
                        let _ = journal.append(Entry::Note {
                            text: format!(
                                "halting: {consecutive_refusals} consecutive refusals — the search is not finding candidates this gate accepts"
                            ),
                        });
                        // After the note, not before. The head was recorded
                        // first until 2026-09-25, so a stalled run's report
                        // anchored the journal one entry short of its end.
                        // No test had driven a run to a stall until heredity
                        // changed the search path.
                        report.journal_head = journal.head().cloned();
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
        // Population = the lineage's measured generations, each with the genome
        // it was built from, weighted for selection by its clade's
        // productivity rather than its own score. Until 2026-09-25 the
        // "genome" here was the generation's fitness score wrapped as a
        // parameter, so nothing heritable passed from parent to child; that
        // remains the fallback only for generations recorded without one.
        let measured: Vec<&Generation> =
            lineage.generations().iter().filter(|g| g.heldout_fitness().is_some()).collect();
        let pop_ids: Vec<GenerationId> = measured.iter().map(|g| g.id).collect();
        let population: Vec<(crate::variation::Genome, f64)> = measured
            .iter()
            .map(|g| {
                let own = g.heldout_fitness().map(|f| f.composite()).unwrap_or(0.0);
                let genome = g.genome.clone().unwrap_or_else(|| vec![Locus::param(own)]);
                (genome, lineage.clade_score(g.id).unwrap_or(own))
            })
            .collect();
        let seedpop =
            if population.is_empty() { vec![(vec![Locus::param(0.5)], 0.5)] } else { population };

        // An empty pool until a registry is wired in: `Substitute` is then a
        // no-op and the run behaves exactly as it did before genes existed.
        let cpc = self.policy.candidates_per_cycle;
        let champion_fitness: Option<FitnessVector> =
            lineage.champion().and_then(|g| g.heldout_fitness().cloned());

        // Either the built-in search, or an external proposer whose every
        // candidate must state a prediction before anything else happens.
        let (pool, refused) = match self.proposer.as_mut() {
            None => {
                let Proposal { candidates, refused } =
                    propose(&seedpop, self.policy.variation, &GenePool::default(), seed);
                let pool: Vec<CandidateSpec> = candidates.into_iter().take(cpc).collect();
                (pool, refused)
            }
            Some(proposer) => {
                let name = proposer.name().to_string();
                // The suite's axes, from the champion's held-out measurement.
                // With no champion measured yet there are none to name, and
                // any metric is accepted while a manifest is still required.
                let axes_owned: Vec<String> = champion_fitness
                    .as_ref()
                    .map(|f| f.axes().map(str::to_string).collect())
                    .unwrap_or_default();
                let axes: Vec<&str> = axes_owned.iter().map(String::as_str).collect();

                let proposed: Vec<CandidateSpec> = proposer
                    .propose(champion_fitness.as_ref(), seed)
                    .into_iter()
                    .take(cpc)
                    .map(|c| c.by(name.clone()))
                    .collect();
                if proposed.is_empty() {
                    return Err(format!("proposer `{name}` produced no candidates"));
                }

                let mut passing = Vec::new();
                let mut refusals = Vec::new();
                for spec in proposed {
                    match manifest::check(spec.manifest.as_ref(), &axes) {
                        Ok(_) => passing.push(spec),
                        Err(reason) => {
                            let text = format!("refused edit {} from {name}: {reason}", spec.id);
                            let _ = journal.append(Entry::Note { text: text.clone() });
                            refusals.push(text);
                        }
                    }
                }
                if passing.is_empty() {
                    // Every candidate refused. That is a refusal, not a
                    // failure: the cycle makes no promotion, and the run counts
                    // it toward the stall limit like any other refused cycle.
                    // Nothing was proposed to the journal, so this entry is
                    // what lets replay count the cycle.
                    journal
                        .append(Entry::CycleRefused { proposer: name.clone(), refused: refusals.len() as u32 })
                        .map_err(|e| e.to_string())?;
                    return Ok(CycleOutcome {
                        cycle: index,
                        candidate: None,
                        promoted: false,
                        reasons: refusals,
                    });
                }

                // Trust decides the width. The passing candidates beyond it
                // are not admitted this cycle; they are neither refused nor
                // journalled, since nothing was judged about them.
                let trust = self.proposer_records.get(&name).map_or(0.0, ProposerRecord::trust);
                passing.truncate(admission_width(trust, cpc));
                (passing, Vec::new())
            }
        };
        // Rank if there is a surrogate, then *sample* within the width trust has
        // earned. Taking the top-ranked candidate outright would follow an
        // uncalibrated predictor from the first cycle, which is the failure
        // GERMLINE.md names: random sampling at least explores, while a
        // confidently wrong predictor steers the budget away from the good
        // candidates and is equally confident about it. At zero trust the width
        // is the whole pool, so sampling makes this undirected search exactly.
        let ranked = match &self.predictor {
            Some(p) => DirectedSearch { predictor: p.as_ref(), calibration: self.calibration.clone(), budget: 1 }
                .select(pool),
            None => pool
                .into_iter()
                .enumerate()
                .map(|(rank, candidate)| super::directed::Ranked {
                    prediction: super::directed::Prediction {
                        predicted: FitnessVector::new(),
                        self_reported_confidence: 0.0,
                    },
                    candidate,
                    rank,
                })
                .collect(),
        };
        let chosen = if ranked.is_empty() {
            None
        } else {
            // Derived from the cycle seed, so the run stays reproducible.
            let mut rng = crate::variation::Rng::seed(seed ^ 0x5EED_5E1E);
            Some(rng.below(ranked.len()))
        };
        let predicted = chosen.map(|i| ranked[i].prediction.predicted.clone());
        let spec = chosen
            .map(|i| ranked[i].candidate.clone())
            .ok_or_else(|| {
                // "produced no candidates" and "produced candidates and refused
                // all of them" are different facts, and the second is the one
                // worth acting on: the search is pushing against a capability
                // boundary rather than running out of room.
                if refused.is_empty() {
                    "variation produced no candidates".to_string()
                } else {
                    let why: Vec<String> = refused
                        .iter()
                        .map(|r| match &r.because {
                            Legality::Acquired { effects } => {
                                format!("{} acquired {}", r.id, effects.join(", "))
                            }
                            Legality::Unknowable { genes } => format!(
                                "{} carries unchecked gene(s) {}",
                                r.id,
                                genes.join(", ")
                            ),
                            Legality::Inherited => format!("{} (refused with no reason)", r.id),
                        })
                        .collect();
                    // The two reasons are different problems and want different
                    // fixes. An escalation means the search found a capability;
                    // an unchecked gene means the registry never recorded what a
                    // block does, and no amount of further search will help.
                    format!(
                        "variation produced {} candidate(s) and refused every one: {}",
                        refused.len(),
                        why.join("; ")
                    )
                }
            })?;

        let artifact = workload.materialize(&spec)?;
        let fitness = workload.evaluate(&artifact, &self.suite)?;

        // What the proposer predicted, against what the measurement says the
        // edit did, relative to the champion it was proposed against. Taken
        // before `fitness` moves into the measurement.
        let calibration = match (spec.proposer.clone(), spec.manifest.clone()) {
            (Some(name), Some(m)) => match fitness.get(&m.metric) {
                Some(value) => {
                    let base = champion_fitness
                        .as_ref()
                        .and_then(|c| c.get(&m.metric))
                        .unwrap_or(0.0);
                    Some((name, m, value - base))
                }
                None => {
                    let _ = journal.append(Entry::Note {
                        text: format!(
                            "calibration skipped for {}: the measurement has no `{}`",
                            spec.id, m.metric
                        ),
                    });
                    None
                }
            },
            _ => None,
        };

        // The only thing that moves trust. A prediction that is never followed
        // by a measurement teaches the calibration nothing, which is why this
        // sits next to the evaluation rather than anywhere more convenient.
        if self.predictor.is_some() {
            if let Some(p) = &predicted {
                self.calibration.observe(p, &fitness);
            }
        }

        let id = lineage.next_id();
        // The generation the candidate actually derived from; the champion
        // only when the population had no real members to derive from.
        let parent = spec
            .parent
            .and_then(|i| pop_ids.get(i as usize).copied())
            .or_else(|| lineage.champion().map(|g| g.id));
        let mut generation = Generation::new(id, artifact.clone())
            .note(format!("cycle {index}, spec {}", spec.id));
        generation.genome = Some(spec.genome.clone());
        generation.definitions = workload.attribution(&artifact);
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

        // Journalled after the evaluation it scores, so it follows `Evaluated`.
        if let Some((proposer, m, measured)) = calibration {
            let expected = m.predicted_delta();
            let score = manifest::score(expected, measured);
            let record = self.proposer_records.entry(proposer.clone()).or_default();
            record.observe(score);
            let trust = record.trust();
            let _ = journal.append(Entry::Calibrated {
                generation: id,
                proposer,
                metric: m.metric,
                predicted: expected,
                measured,
                score,
                trust,
            });
        }

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
    use crate::attest::Attestor;
    use crate::gate::PromotionGate;
    use crate::lineage::ReplayError;
    use crate::journal::JournalError;
    use crate::{SuiteKind};
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
        l.grant(id, episode().gate_digest, EVALUATOR).unwrap();
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
        // The report anchors the journal's actual end, halting note included.
        assert_eq!(report.journal_head, j.head().cloned(), "a stalled run's report must anchor its whole journal");
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
    fn generations_carry_their_genome_and_their_real_parent() {
        let path = tmp("heredity");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 7);
        let report = r.run(&mut l, &mut j, &mut w);
        assert!(report.promotions > 0);
        let added: Vec<&Generation> = l.generations().iter().skip(1).collect();
        assert!(!added.is_empty());
        for g in &added {
            let genome = g.genome.as_ref().expect("a generation records its genome");
            assert!(!genome.is_empty());
            let p = g.parent.expect("and its parent");
            assert!(l.get(p).is_some(), "the parent is a generation in this lineage");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
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
        let mut r = Runner::new(policy(), &ep, &at, suite(), 11);

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

    /// A workload that records every candidate id it was asked to materialize.
    struct Recording {
        inner: Improving,
        seen: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl Workload for Recording {
        fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
            self.seen.borrow_mut().push(spec.id.clone());
            self.inner.materialize(spec)
        }
        fn evaluate(&mut self, a: &Digest, s: &EvalSuite) -> Result<FitnessVector, String> {
            self.inner.evaluate(a, s)
        }
        fn shadow(&mut self, a: &Digest) -> HealthSample {
            self.inner.shadow(a)
        }
        fn observe_champion(&mut self, a: &Digest) -> HealthSample {
            self.inner.observe_champion(a)
        }
        fn materialized(&self, d: &Digest) -> bool {
            self.inner.materialized(d)
        }
    }

    /// A workload that attributes each artifact to a definition hash.
    struct Attributing(Improving);

    impl Workload for Attributing {
        fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
            self.0.materialize(spec)
        }
        fn evaluate(&mut self, a: &Digest, s: &EvalSuite) -> Result<FitnessVector, String> {
            self.0.evaluate(a, s)
        }
        fn shadow(&mut self, a: &Digest) -> HealthSample {
            self.0.shadow(a)
        }
        fn observe_champion(&mut self, a: &Digest) -> HealthSample {
            self.0.observe_champion(a)
        }
        fn materialized(&self, d: &Digest) -> bool {
            self.0.materialized(d)
        }
        fn attribution(&self, a: &Digest) -> Vec<String> {
            vec![format!("def-{}", a.short())]
        }
    }

    #[test]
    fn the_journal_attributes_each_candidate_to_its_definitions_by_hash() {
        let path = tmp("attribution");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Attributing(Improving::new(0.05));
        let mut l = seeded(&mut w.0);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 3);
        let _ = r.run(&mut l, &mut j, &mut w);
        let proposed: Vec<(Digest, Vec<String>)> = j
            .replay()
            .unwrap()
            .into_iter()
            .filter_map(|r| match r.entry {
                Entry::Proposed { artifact, definitions, .. } => Some((artifact, definitions)),
                _ => None,
            })
            .collect();
        assert!(!proposed.is_empty());
        for (artifact, defs) in &proposed {
            assert_eq!(defs, &vec![format!("def-{}", artifact.short())], "the journal names what it was built from");
        }
        assert!(l.generations().iter().skip(1).all(|g| g.definitions.len() == 1), "and so does the lineage");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        // Without attribution the field is absent, so a journal written before
        // it existed hashes exactly as it did.
        let g = Generation::new(GenerationId(1), Digest::of(b"x"));
        let entry = serde_json::to_string(&crate::journal::proposed(&g, 1, "p")).unwrap();
        assert!(!entry.contains("definitions"), "{entry}");
    }

    fn ids_for(candidates_per_cycle: usize, seed: u64) -> Vec<String> {
        let path = tmp(&format!("cpc{candidates_per_cycle}-{seed}"));
        let mut j = Journal::open(&path).unwrap();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut w = Recording { inner: Improving::new(0.05), seen: seen.clone() };
        let mut l = seeded(&mut w.inner);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(
            RunnerPolicy { candidates_per_cycle, ..policy() },
            &ep,
            &at,
            suite(),
            seed,
        );
        let _ = r.run(&mut l, &mut j, &mut w);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        let out = seen.borrow().clone();
        out
    }

    #[test]
    fn candidates_per_cycle_decides_what_the_search_may_choose_from() {
        // It did not. `.take(n).next()` is element 0 for every non-zero `n`, so
        // the loop proposed eight offspring, evaluated the first, discarded
        // seven, and the policy field changed nothing — while lib.rs described
        // the crate as proposing a successor by *directed* evolution and
        // `directed`'s ranking machinery had no caller outside its own tests.
        let narrow = ids_for(1, 0x5A1D);
        let wide = ids_for(8, 0x5A1D);

        assert!(!narrow.is_empty(), "the run must materialize something");
        for id in &narrow {
            assert!(
                id.ends_with("-0"),
                "with a pool of one there is nothing else to pick: {id}"
            );
        }
        assert_ne!(
            narrow, wide,
            "a wider pool must be able to reach a candidate a pool of one cannot"
        );
    }

    #[test]
    fn a_run_with_a_pool_is_still_reproducible_from_its_seed() {
        // Sampling is what keeps an uncalibrated predictor from steering, and a
        // sampler seeded from anything but the run seed would buy that at the
        // cost of the property the whole journal rests on.
        assert_eq!(ids_for(8, 0xD11), ids_for(8, 0xD11), "same seed, same choices");
        assert_ne!(
            ids_for(8, 0xD11),
            ids_for(8, 0xD12),
            "and different seeds explore differently"
        );
    }

    /// Predicts the candidate's own scalar mean — right, for `Improving`.
    struct Oracle;
    impl crate::directed::FitnessPredictor for Oracle {
        fn predict(&self, c: &CandidateSpec) -> crate::directed::Prediction {
            let p = crate::variation::params_of(&c.genome);
            let m = p.iter().sum::<f64>() / p.len().max(1) as f64;
            crate::directed::Prediction {
                predicted: FitnessVector::new().with("capability", m),
                self_reported_confidence: 0.99,
            }
        }
    }

    #[test]
    fn an_attached_predictor_starts_untrusted() {
        // Trust is earned by being right about things that were subsequently
        // measured, not by attaching a predictor that says it is confident.
        // `Oracle` self-reports 0.99 and that number buys nothing.
        let ep = episode();
        let at = attestor();
        let r = Runner::new(policy(), &ep, &at, suite(), 1).with_predictor(Box::new(Oracle));
        assert_eq!(r.predictor_trust(), 0.0, "an untested predictor is not trusted");
    }

    #[test]
    fn a_run_with_a_predictor_still_completes_and_stays_bounded() {
        // The wiring must not change what the loop is allowed to do. Budget,
        // halting and the journal are unaffected by whether a surrogate ranked
        // the proposals.
        let path = tmp("predicted");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        let mut l = seeded(&mut w);
        let ep = episode();
        let at = attestor();
        let mut r = Runner::new(policy(), &ep, &at, suite(), 0x9001)
            .with_predictor(Box::new(Oracle));
        let report = r.run(&mut l, &mut j, &mut w);
        assert!(
            report.cycles.len() <= policy().max_cycles as usize,
            "a predictor does not buy extra cycles"
        );
        assert!(j.verify().is_ok(), "and the chain still holds");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    // ---- resuming from the journal (plan 8.3) ----

    /// A workload whose result is a pure function of the genome, so a fresh
    /// instance after a restart behaves as the old one would have: capability
    /// is the mean of the genome's parameters.
    #[derive(Default)]
    struct ByGenome {
        capability: std::collections::HashMap<String, f64>,
    }

    impl Workload for ByGenome {
        fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
            let ps = crate::variation::params_of(&spec.genome);
            let cap = if ps.is_empty() { 0.0 } else { ps.iter().sum::<f64>() / ps.len() as f64 };
            let d = Digest::of(serde_json::to_string(&spec.genome).unwrap().as_bytes());
            self.capability.insert(d.0.clone(), cap);
            Ok(d)
        }
        fn evaluate(&mut self, artifact: &Digest, _: &EvalSuite) -> Result<FitnessVector, String> {
            let cap = self.capability.get(&artifact.0).copied().ok_or("not materialized")?;
            Ok(FitnessVector::new().with("capability", cap).with("safety", 0.95))
        }
        fn shadow(&mut self, _: &Digest) -> HealthSample {
            HealthSample::ok()
        }
        fn observe_champion(&mut self, _: &Digest) -> HealthSample {
            HealthSample::ok()
        }
        fn materialized(&self, _: &Digest) -> bool {
            true
        }
    }

    /// What the caller sets up in code before the journal starts.
    fn genesis() -> Lineage {
        let mut l = Lineage::new();
        let id = l.next_id();
        let mut g = Generation::new(id, Digest::of(b"seed-champion")).measured(Measurement {
            suite: suite(),
            fitness: FitnessVector::new().with("capability", 0.5).with("safety", 0.95),
            evaluator: EVALUATOR.into(),
        });
        g.genome = Some(vec![Locus::param(0.5), Locus::param(0.5)]);
        l.add(g);
        l.grant(id, episode().gate_digest, EVALUATOR).unwrap();
        l
    }

    fn patient(max_cycles: u32) -> RunnerPolicy {
        RunnerPolicy { max_cycles, max_consecutive_refusals: 100, ..policy() }
    }

    /// The journal's entries other than notes, which say when a run opened
    /// or resumed and so differ by design.
    fn events(j: &Journal) -> Vec<Entry> {
        j.replay().unwrap().into_iter().map(|r| r.entry).filter(|e| !matches!(e, Entry::Note { .. })).collect()
    }

    #[test]
    fn a_run_killed_at_any_cycle_boundary_resumes_into_the_same_run() {
        // Seed 11 promotes at cycle 3 and refuses the rest, so the kill
        // points fall before, at and after a promotion.
        const CYCLES: u32 = 8;
        let (ep, at) = (episode(), attestor());
        let whole_path = tmp("resume-whole");
        let mut whole_j = Journal::open(&whole_path).unwrap();
        let mut whole = genesis();
        let whole_report = Runner::new(patient(CYCLES), &ep, &at, suite(), 11).run(&mut whole, &mut whole_j, &mut ByGenome::default());
        assert!(
            whole_report.promotions > 0 && whole_report.refusals > 0,
            "the run should both promote and refuse, so both are replayed: {whole_report:?}"
        );

        for kill_at in 1..CYCLES {
            let path = tmp(&format!("resume-{kill_at}"));
            {
                let mut j = Journal::open(&path).unwrap();
                let mut l = genesis();
                Runner::new(patient(kill_at), &ep, &at, suite(), 11).run(&mut l, &mut j, &mut ByGenome::default());
            }
            // A new process: nothing survives but the journal and its head.
            let mut j = Journal::open(&path).unwrap();
            let head = j.head().cloned();
            let mut resumed = Lineage::replay(genesis(), &j, head.as_ref()).unwrap();
            assert_eq!(resumed.proposals, kill_at);
            let report = Runner::new(patient(CYCLES - kill_at), &ep, &at, suite(), 11)
                .resume(&resumed)
                .run(&mut resumed.lineage, &mut j, &mut ByGenome::default());
            assert_eq!(report.cycles.first().map(|c| c.cycle), Some(kill_at), "cycle numbering continues");
            assert_eq!(resumed.lineage.to_json(), whole.to_json(), "killed after cycle {kill_at}");
            assert_eq!(events(&j), events(&whole_j), "killed after cycle {kill_at}");
            assert_eq!(j.verify().err(), None, "killed after cycle {kill_at}");
            let _ = std::fs::remove_dir_all(path.parent().unwrap());
        }
        let _ = std::fs::remove_dir_all(whole_path.parent().unwrap());
    }

    #[test]
    fn replay_rebuilds_a_demotion_exactly() {
        let path = tmp("resume-demoted");
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.05);
        w.champion_healthy = false;
        let mut live = seeded(&mut w);
        let (ep, at) = (episode(), attestor());
        let report = Runner::new(policy(), &ep, &at, suite(), 3).run(&mut live, &mut j, &mut w);
        assert!(matches!(report.halt, Halt::Demoted { .. }), "{:?}", report.halt);
        let replayed = Lineage::replay(seeded(&mut Improving::new(0.05)), &j, None).unwrap();
        assert_eq!(replayed.lineage.to_json(), live.to_json());
        assert!(replayed.lineage.generations().iter().any(|g| g.status == crate::Status::Quarantined));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_stall_that_spans_a_restart_is_still_a_stall() {
        let path = tmp("resume-stall");
        let (ep, at) = (episode(), attestor());
        let stalling = |max_cycles| RunnerPolicy { max_cycles, max_consecutive_refusals: 3, ..policy() };
        let mut j = Journal::open(&path).unwrap();
        let mut w = Improving::new(0.0);
        let mut l = seeded(&mut w);
        let first = Runner::new(stalling(2), &ep, &at, suite(), 2).run(&mut l, &mut j, &mut w);
        assert_eq!((first.refusals, first.promotions), (2, 0));

        let mut resumed = Lineage::replay(seeded(&mut Improving::new(0.0)), &j, None).unwrap();
        assert_eq!(resumed.trailing_refusals, 2);
        let mut w = Improving::new(0.0);
        seeded(&mut w);
        let report = Runner::new(stalling(10), &ep, &at, suite(), 2).resume(&resumed).run(&mut resumed.lineage, &mut j, &mut w);
        assert_eq!(report.halt, Halt::SearchStalled { consecutive_refusals: 3 });
        assert_eq!(report.cycles.len(), 1, "the third refusal in a row, not the third since the restart");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_journal_changed_in_one_byte_does_not_resume() {
        let path = tmp("resume-tamper");
        let mut j = Journal::open(&path).unwrap();
        let mut l = genesis();
        Runner::new(patient(3), &ep_static(), &attestor(), suite(), 11).run(&mut l, &mut j, &mut ByGenome::default());
        let raw = std::fs::read_to_string(&path).unwrap();
        let at = raw.find("\"capability\\\":").expect("a recorded fitness") + "\"capability\\\":".len();
        let mut bytes = raw.into_bytes();
        bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
        std::fs::write(&path, bytes).unwrap();
        let err = Lineage::replay(genesis(), &Journal::open(&path).unwrap(), None).unwrap_err();
        assert!(matches!(err, ReplayError::Journal(JournalError::Broken { .. })), "{err:?}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    fn ep_static() -> Episode {
        episode()
    }

    #[test]
    fn a_truncated_journal_does_not_match_its_published_head() {
        let path = tmp("resume-truncate");
        let mut j = Journal::open(&path).unwrap();
        let mut l = genesis();
        let report = Runner::new(patient(3), &episode(), &attestor(), suite(), 11).run(&mut l, &mut j, &mut ByGenome::default());
        let lines: Vec<String> = std::fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
        std::fs::write(&path, lines[..lines.len() - 3].join("\n") + "\n").unwrap();
        let j = Journal::open(&path).unwrap();
        assert_eq!(Lineage::replay(genesis(), &j, None).err(), None, "the prefix is a valid chain on its own");
        let err = Lineage::replay(genesis(), &j, report.journal_head.as_ref()).unwrap_err();
        assert!(matches!(err, ReplayError::Anchor { .. }), "{err:?}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_well_chained_journal_still_cannot_name_a_champion_the_gate_did_not() {
        // Anyone who can write the file can append records with valid links.
        // What they append is still replayed through the succession rules.
        let (ep, at) = (episode(), attestor());
        let promote = |generation: u64, from: Option<u64>| Entry::Succession {
            event: crate::lineage::SuccessionEvent::Promoted {
                generation: GenerationId(generation),
                from: from.map(GenerationId),
                gate: ep.gate_digest.clone(),
                evaluator: EVALUATOR.into(),
            },
        };
        let forged = |name: &str, entries: Vec<Entry>| {
            let path = tmp(name);
            let mut j = Journal::open(&path).unwrap();
            let mut l = genesis();
            Runner::new(patient(2), &ep, &at, suite(), 11).run(&mut l, &mut j, &mut ByGenome::default());
            for e in entries {
                j.append(e).unwrap();
            }
            let out = Lineage::replay(genesis(), &j, None);
            let _ = std::fs::remove_dir_all(path.parent().unwrap());
            out
        };
        let champion = |name| {
            let path = tmp(name);
            let mut j = Journal::open(&path).unwrap();
            let mut l = genesis();
            Runner::new(patient(2), &ep, &at, suite(), 11).run(&mut l, &mut j, &mut ByGenome::default());
            let _ = std::fs::remove_dir_all(path.parent().unwrap());
            l.champion().map(|g| g.id.0)
        };
        let current = champion("forge-champion");

        // A generation nobody proposed.
        let e = forged("forge-unproposed", vec![promote(99, current)]).unwrap_err();
        assert!(matches!(&e, ReplayError::Inconsistent { detail, .. } if detail.contains("without being proposed")), "{e:?}");

        // A proposed generation promoted over a champion that is not the one
        // in office.
        let proposal = Entry::Proposed {
            generation: GenerationId(50),
            artifact: Digest::of(b"forged"),
            seed: 0,
            plan: "forger".into(),
            definitions: vec![],
            parent: None,
            genome: None,
            note: String::new(),
        };
        let e = forged("forge-stale", vec![proposal.clone(), promote(50, Some(777))]).unwrap_err();
        assert!(matches!(&e, ReplayError::Inconsistent { detail, .. } if detail.contains("the champion was")), "{e:?}");

        // Proposed, and over the right champion, but never measured on a
        // held-out suite.
        let e = forged("forge-unmeasured", vec![proposal, promote(50, current)]).unwrap_err();
        assert!(matches!(&e, ReplayError::Inconsistent { detail, .. } if detail.contains("no held-out measurement")), "{e:?}");
    }

    #[test]
    fn a_journal_from_before_replay_says_so() {
        let path = tmp("resume-predates");
        let mut j = Journal::open(&path).unwrap();
        j.append(Entry::Evaluated {
            generation: GenerationId(1),
            fitness: "{}".into(),
            suite: Digest::of(SUITE),
            evaluator: EVALUATOR.into(),
            suite_id: None,
            suite_kind: None,
        })
        .unwrap();
        assert!(matches!(Lineage::replay(Lineage::new(), &j, None), Err(ReplayError::Predates { index: 0, .. })));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    // ---- external proposers and prediction manifests (plan 5.4) ----

    use crate::manifest::{Direction, PredictionManifest};

    /// Builds each artifact with its genome's parameter as capability, and
    /// counts how many it was asked to build.
    struct Counted {
        capability: std::collections::HashMap<String, f64>,
        materialized: std::rc::Rc<std::cell::Cell<u32>>,
    }

    impl Workload for Counted {
        fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
            self.materialized.set(self.materialized.get() + 1);
            let cap = crate::variation::params_of(&spec.genome).first().copied().unwrap_or(0.0);
            let d = Digest::of(spec.id.as_bytes());
            self.capability.insert(d.0.clone(), cap);
            Ok(d)
        }
        fn evaluate(&mut self, artifact: &Digest, _: &EvalSuite) -> Result<FitnessVector, String> {
            let cap = self.capability.get(&artifact.0).copied().ok_or("not materialized")?;
            Ok(FitnessVector::new().with("capability", cap).with("safety", 0.95))
        }
        fn shadow(&mut self, _: &Digest) -> HealthSample {
            HealthSample::ok()
        }
        fn observe_champion(&mut self, _: &Digest) -> HealthSample {
            HealthSample::ok()
        }
        fn materialized(&self, _: &Digest) -> bool {
            true
        }
    }

    /// Proposes edits that move capability by `step` from the champion. The
    /// manifest (if any) claims `claim` with the step's magnitude, so it is
    /// right exactly when `claim` agrees with the sign of `step`.
    struct Stepper {
        name: &'static str,
        step: f64,
        claim: Direction,
        manifests: bool,
        calls: u32,
    }

    impl Proposer for Stepper {
        fn name(&self) -> &str {
            self.name
        }
        fn propose(&mut self, champion: Option<&FitnessVector>, _seed: u64) -> Vec<CandidateSpec> {
            self.calls += 1;
            let base = champion.and_then(|f| f.get("capability")).unwrap_or(0.5);
            (0..2)
                .map(|i| {
                    let spec = CandidateSpec::new(
                        format!("{}-{}-{i}", self.name, self.calls),
                        vec![Locus::param(base + self.step)],
                    );
                    if self.manifests {
                        spec.with_manifest(PredictionManifest {
                            metric: "capability".into(),
                            direction: self.claim,
                            magnitude: self.step.abs(),
                        })
                    } else {
                        spec
                    }
                })
                .collect()
        }
    }

    struct Driven {
        report: RunReport,
        entries: Vec<Entry>,
        verified: bool,
        materialized: u32,
        trust: f64,
    }

    fn drive(proposer: Stepper, policy: RunnerPolicy, seed: u64) -> Driven {
        let name = proposer.name;
        let path = tmp(&format!("proposer-{name}-{seed}"));
        let mut j = Journal::open(&path).unwrap();
        let materialized = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut w = Counted { capability: Default::default(), materialized: materialized.clone() };
        let mut l = genesis();
        let (ep, at) = (episode(), attestor());
        let mut r = Runner::new(policy, &ep, &at, suite(), seed).with_proposer(Box::new(proposer));
        let report = r.run(&mut l, &mut j, &mut w);
        let driven = Driven {
            report,
            entries: j.replay().unwrap().into_iter().map(|r| r.entry).collect(),
            verified: j.verify().is_ok(),
            materialized: materialized.get(),
            trust: r.proposer_trust(name),
        };
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        driven
    }

    #[test]
    fn an_edit_without_a_manifest_is_refused_and_never_materialized() {
        let d = drive(
            Stepper { name: "stepper", step: 0.05, claim: Direction::Increase, manifests: false, calls: 0 },
            patient(3),
            1,
        );
        assert_eq!(d.materialized, 0, "a candidate that states no prediction is never built");
        assert!(
            d.entries.iter().any(|e| matches!(
                e,
                Entry::Note { text } if text.contains("refused edit")
                    && text.contains("from stepper")
                    && text.contains("a harness edit must state a prediction")
            )),
            "the refusal is on the record, with its reason"
        );
        assert!(!d.entries.iter().any(|e| matches!(e, Entry::Calibrated { .. })), "nothing was scored");
        assert_eq!(d.report.promotions, 0);
        assert!(d.verified, "the chain holds");
    }

    #[test]
    fn a_well_calibrated_proposer_earns_trust_and_each_prediction_is_journalled() {
        let d = drive(
            Stepper { name: "stepper", step: 0.05, claim: Direction::Increase, manifests: true, calls: 0 },
            patient(6),
            2,
        );
        let cal: Vec<(f64, f64)> = d
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Calibrated { score, trust, .. } => Some((*score, *trust)),
                _ => None,
            })
            .collect();
        assert!(cal.len() >= 3, "each evaluated edit with a manifest is scored: {cal:?}");
        assert_eq!(cal[1].1, 0.0, "two predictions are not yet a record");
        assert!(cal[2].1 > 0.0, "three scored predictions earn trust: {cal:?}");
        assert!(d.trust > 0.0, "the runner reports the earned trust");
        for (score, _) in &cal {
            assert!(*score > 0.99, "a prediction that came true scores near 1, got {score}");
        }
        assert!(d.verified, "calibration entries keep the chain intact");
    }

    #[test]
    fn a_badly_calibrated_proposer_stays_untrusted() {
        // Claims a decrease; every edit increases capability. Wrong sign, every time.
        let d = drive(
            Stepper { name: "stepper", step: 0.05, claim: Direction::Decrease, manifests: true, calls: 0 },
            patient(6),
            3,
        );
        let scores: Vec<f64> = d
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Calibrated { score, .. } => Some(*score),
                _ => None,
            })
            .collect();
        assert!(scores.len() >= 3, "{scores:?}");
        assert!(scores.iter().all(|s| *s == 0.0), "a wrong sign scores zero: {scores:?}");
        assert_eq!(d.trust, 0.0, "a proposer that is always wrong earns nothing");
        assert_eq!(admission_width(d.trust, 4), 1, "and admits one candidate a cycle");
        assert!(
            d.materialized as usize <= d.report.cycles.len(),
            "one admitted candidate per cycle at most: {} built over {} cycles",
            d.materialized,
            d.report.cycles.len()
        );
    }

    #[test]
    fn a_proposer_whose_candidates_are_all_refused_stalls_the_search() {
        // A cycle that refuses everything is a refusal, so the stall limit
        // applies to it rather than the run grinding through its budget.
        let d = drive(
            Stepper { name: "stepper", step: 0.05, claim: Direction::Increase, manifests: false, calls: 0 },
            RunnerPolicy { max_cycles: 50, ..policy() },
            4,
        );
        assert!(matches!(d.report.halt, Halt::SearchStalled { .. }), "{:?}", d.report.halt);
        assert!(d.report.cycles.len() < 50);
        assert_eq!(d.report.refusals as usize, d.report.cycles.len(), "every cycle was a refusal");
        assert_eq!(d.materialized, 0);
    }

    #[test]
    fn a_resumed_proposer_keeps_the_trust_its_journal_earned() {
        // The record is rebuilt from the journal alone, summing the same scores
        // in the same order, so the resumed runner starts at exactly the trust
        // the original reached.
        let path = tmp("resume-proposer-trust");
        let (ep, at) = (episode(), attestor());
        let calibrated = Stepper { name: "stepper", step: 0.05, claim: Direction::Increase, manifests: true, calls: 0 };
        let mut j = Journal::open(&path).unwrap();
        let mut l = genesis();
        let mut original = Runner::new(patient(6), &ep, &at, suite(), 2).with_proposer(Box::new(calibrated));
        original.run(&mut l, &mut j, &mut ByGenome::default());
        let original_trust = original.proposer_trust("stepper");
        assert!(original_trust > 0.0, "the original run must earn trust for this to mean anything");

        // A new process: nothing survives but the journal and its head.
        let head = j.head().cloned();
        let resumed = Lineage::replay(genesis(), &j, head.as_ref()).unwrap();
        let scored = j.replay().unwrap().iter().filter(|r| matches!(r.entry, Entry::Calibrated { .. })).count();
        assert_eq!(resumed.proposers["stepper"].samples() as usize, scored, "every scored prediction is rebuilt");

        let fresh = Stepper { name: "stepper", step: 0.05, claim: Direction::Increase, manifests: true, calls: 0 };
        let resumed_runner = Runner::new(patient(0), &ep, &at, suite(), 2)
            .with_proposer(Box::new(fresh))
            .resume(&resumed);
        assert_eq!(resumed_runner.proposer_trust("stepper"), original_trust, "exactly the trust the original earned");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_stall_of_all_refused_cycles_is_counted_on_replay_and_continues() {
        // A proposer that states no predictions refuses every candidate, so its
        // cycles write no `Proposed` entry. Replay must still count them, and a
        // resumed run must continue the stall rather than start a fresh count.
        let path = tmp("resume-refused-cycles");
        let (ep, at) = (episode(), attestor());
        let silent = || Stepper { name: "silent", step: 0.05, claim: Direction::Increase, manifests: false, calls: 0 };
        let mut j = Journal::open(&path).unwrap();
        let mut l = genesis();
        let first_policy = RunnerPolicy { max_cycles: 2, max_consecutive_refusals: 100, ..policy() };
        let first = Runner::new(first_policy, &ep, &at, suite(), 2)
            .with_proposer(Box::new(silent()))
            .run(&mut l, &mut j, &mut ByGenome::default());
        assert_eq!((first.refusals, first.promotions), (2, 0));

        let head = j.head().cloned();
        let mut resumed = Lineage::replay(genesis(), &j, head.as_ref()).unwrap();
        assert_eq!((resumed.proposals, resumed.trailing_refusals), (2, 2), "both refused cycles are counted");

        let stalling = RunnerPolicy { max_cycles: 10, max_consecutive_refusals: 3, ..policy() };
        let report = Runner::new(stalling, &ep, &at, suite(), 2)
            .with_proposer(Box::new(silent()))
            .resume(&resumed)
            .run(&mut resumed.lineage, &mut j, &mut ByGenome::default());
        assert_eq!(report.halt, Halt::SearchStalled { consecutive_refusals: 3 });
        assert_eq!(report.cycles.len(), 1, "the third refusal in a row, not the third since the restart");
        assert_eq!(report.cycles[0].cycle, 2, "cycle numbering continues past the refused cycles");
        assert!(j.verify().is_ok(), "the chain holds");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn admission_width_is_trust_scaled_with_a_floor_of_one() {
        assert_eq!(admission_width(0.0, 4), 1, "no record, one candidate");
        assert_eq!(admission_width(0.5, 4), 2);
        assert_eq!(admission_width(0.9, 4), 4, "round(3.6) is 4");
        assert_eq!(admission_width(1.0, 4), 4);
        assert_eq!(admission_width(0.0, 0), 1, "the floor holds even for an empty batch");
    }
}
