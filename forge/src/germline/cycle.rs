//! The succession cycle: the state machine that joins the pieces.
//!
//! ```text
//!   Proposed ──▶ Evaluated ──▶ Shadowing ──▶ Adjudicated ──▶ Promoted
//!                                                  │
//!                                                  └────────▶ Refused
//! ```
//!
//! Every transition writes to the [`journal`](super::journal), so the record of
//! a succession is produced *by* the succession rather than assembled afterwards
//! from memory. A log written after the fact is a log that can be written
//! differently after the fact.
//!
//! ## Phases are enforced, not suggested
//!
//! Calling out of order is an error, not a shortcut. The order is the argument:
//! a candidate is measured before it is judged, judged before it shadows real
//! work, and shadows before it takes authority. Allowing a caller to skip
//! straight from *proposed* to *promoted* would make every invariant in
//! [`gate`](super::gate) optional in practice, since the enforcement lives on a
//! path that could simply be avoided.
//!
//! ## Two keys turn the lock
//!
//! Promotion needs both an **attested approving verdict** and an
//! [`Authority`]. They are different claims and it is worth keeping them apart:
//! the verdict says *the criteria were met*, the authority says *someone
//! accountable decided to act on that*.
//!
//! [`Authority::Unattended`] exists — the architecture supports a closed loop —
//! but it must be constructed deliberately and names the policy that permits it.
//! The default path requires an operator. That is not a technical limitation to
//! be removed later; it is the setting a system should run in until its
//! measurements have been checked against reality enough times to be trusted
//! without a person reading them.

use super::attest::{Attestation, Attestor};
use super::gate::{Episode, Verdict};
use super::journal::{Entry, Journal};
use super::lineage::Lineage;
use super::supervisor::HealthSample;
use super::{FitnessVector, Generation, GenerationId, Measurement};
use ribosome::sched::BuildReport;
use ribosome::Digest;
use serde::{Deserialize, Serialize};

/// Who authorized acting on an approving verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "authority", rename_all = "snake_case")]
pub enum Authority {
    /// A person decided. The default.
    Operator { who: String },
    /// A registered policy decided. Names the policy so an audit can ask what
    /// was in force, and requires the policy to be pinned by digest — an
    /// unattended loop authorized by an unidentifiable rule is not auditable.
    Unattended { policy: String, policy_digest: Digest },
}

impl Authority {
    pub fn describe(&self) -> String {
        match self {
            Authority::Operator { who } => format!("operator:{who}"),
            Authority::Unattended { policy, policy_digest } => {
                format!("policy:{policy}@{}", policy_digest.short())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Proposed,
    Evaluated,
    Shadowing,
    Adjudicated,
    Promoted,
    Refused,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CycleError {
    /// A transition was attempted from the wrong phase.
    WrongPhase { expected: &'static [Phase], actual: Phase },
    /// The verdict rejected the candidate.
    Rejected { reasons: Vec<String> },
    /// The attestation does not verify against the evaluator's key.
    Unattested,
    /// The candidate has no measurement to judge.
    Unevaluated,
    Journal(String),
    Lineage(String),
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CycleError::WrongPhase { expected, actual } => {
                write!(f, "cycle is {actual:?}; this step requires one of {expected:?}")
            }
            CycleError::Rejected { reasons } => write!(f, "refused: {}", reasons.join("; ")),
            CycleError::Unattested => {
                write!(f, "the verdict's attestation did not verify — it may not have come from the named evaluator")
            }
            CycleError::Unevaluated => write!(f, "candidate has not been measured"),
            CycleError::Journal(e) => write!(f, "journal: {e}"),
            CycleError::Lineage(e) => write!(f, "lineage: {e}"),
        }
    }
}

impl std::error::Error for CycleError {}

/// One candidate's journey from proposal to authority (or refusal).
pub struct Cycle {
    candidate: Generation,
    phase: Phase,
    shadow_successes: u32,
    shadow_failures: u32,
    verdict: Option<Verdict>,
    attestation: Option<Attestation>,
}

impl Cycle {
    /// Open a cycle for a freshly proposed candidate, recording its provenance.
    pub fn propose(
        candidate: Generation,
        seed: u64,
        plan: &str,
        journal: &mut Journal,
    ) -> Result<Self, CycleError> {
        journal
            .append(super::journal::proposed(&candidate, seed, plan))
            .map_err(|e| CycleError::Journal(e.to_string()))?;
        Ok(Cycle {
            candidate,
            phase: Phase::Proposed,
            shadow_successes: 0,
            shadow_failures: 0,
            verdict: None,
            attestation: None,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn candidate(&self) -> &Generation {
        &self.candidate
    }

    pub fn id(&self) -> GenerationId {
        self.candidate.id
    }

    pub fn shadow_successes(&self) -> u32 {
        self.shadow_successes
    }

    pub fn verdict(&self) -> Option<&Verdict> {
        self.verdict.as_ref()
    }

    fn require(&self, allowed: &'static [Phase]) -> Result<(), CycleError> {
        if allowed.contains(&self.phase) {
            Ok(())
        } else {
            Err(CycleError::WrongPhase { expected: allowed, actual: self.phase })
        }
    }

    /// Attach a held-out measurement.
    pub fn evaluate(
        &mut self,
        measurement: Measurement,
        journal: &mut Journal,
    ) -> Result<(), CycleError> {
        self.require(&[Phase::Proposed])?;
        journal
            .append(Entry::Evaluated {
                generation: self.candidate.id,
                fitness: serde_json::to_string(&measurement.fitness).unwrap_or_default(),
                suite: measurement.suite.digest.clone(),
                evaluator: measurement.evaluator.clone(),
            })
            .map_err(|e| CycleError::Journal(e.to_string()))?;
        self.candidate.measured = Some(measurement);
        self.phase = Phase::Evaluated;
        Ok(())
    }

    /// Record a canary observation: the candidate doing real work while the
    /// incumbent still holds authority.
    pub fn shadow(&mut self, sample: &HealthSample) -> Result<(), CycleError> {
        self.require(&[Phase::Evaluated, Phase::Shadowing])?;
        if sample.success {
            self.shadow_successes += 1;
        } else {
            self.shadow_failures += 1;
        }
        self.phase = Phase::Shadowing;
        Ok(())
    }

    pub fn shadow_failures(&self) -> u32 {
        self.shadow_failures
    }

    /// Run the gate and attest the result.
    ///
    /// Takes an [`Attestor`] rather than an evaluator name so the resulting
    /// verdict carries proof of origin instead of a claim of it.
    pub fn adjudicate(
        &mut self,
        episode: &Episode,
        lineage: &Lineage,
        attestor: &Attestor,
        materialized: &dyn Fn(&Digest) -> bool,
        journal: &mut Journal,
    ) -> Result<&Verdict, CycleError> {
        self.require(&[Phase::Evaluated, Phase::Shadowing])?;
        if self.candidate.measured.is_none() {
            return Err(CycleError::Unevaluated);
        }

        let verdict =
            episode.adjudicate(&self.candidate, lineage, self.shadow_successes, materialized);
        let attestation = attestor.attest(&verdict);

        journal
            .append(Entry::Adjudicated {
                verdict: verdict.clone(),
                attestation: Some(attestation.mac.clone()),
            })
            .map_err(|e| CycleError::Journal(e.to_string()))?;

        self.phase = Phase::Adjudicated;
        self.verdict = Some(verdict);
        self.attestation = Some(attestation);
        Ok(self.verdict.as_ref().unwrap())
    }

    /// Act on an approving verdict: add the candidate to the lineage and hand it
    /// authority.
    ///
    /// Re-verifies the attestation here rather than trusting the field set during
    /// adjudication. The check costs microseconds and covers the case that
    /// matters — a verdict that reached this point by some path other than the
    /// one above.
    pub fn commit(
        &mut self,
        lineage: &mut Lineage,
        attestor: &Attestor,
        authority: Authority,
        journal: &mut Journal,
    ) -> Result<GenerationId, CycleError> {
        self.require(&[Phase::Adjudicated])?;
        let verdict = self.verdict.clone().ok_or(CycleError::Unevaluated)?;
        let attestation = self.attestation.clone().ok_or(CycleError::Unattested)?;

        if !attestor.verify(&attestation, &verdict) {
            return Err(CycleError::Unattested);
        }

        let Verdict::Promote { generation, gate, evaluator } = verdict else {
            let reasons = self.verdict.as_ref().map(|v| v.reasons()).unwrap_or_default();
            lineage.refuse(self.candidate.id, reasons.clone());
            self.phase = Phase::Refused;
            journal
                .append(Entry::Note {
                    text: format!("refused {}: {}", self.candidate.id, reasons.join("; ")),
                })
                .map_err(|e| CycleError::Journal(e.to_string()))?;
            return Err(CycleError::Rejected { reasons });
        };

        lineage.add(self.candidate.clone());
        lineage
            .promote(generation, gate, &evaluator)
            .map_err(|e| CycleError::Lineage(e.to_string()))?;

        let event = lineage.events().last().cloned();
        if let Some(event) = event {
            journal
                .append(Entry::Succession { event })
                .map_err(|e| CycleError::Journal(e.to_string()))?;
        }
        journal
            .append(Entry::Note {
                text: format!("authority granted to {generation} by {}", authority.describe()),
            })
            .map_err(|e| CycleError::Journal(e.to_string()))?;

        self.phase = Phase::Promoted;
        Ok(generation)
    }
}

/// Derive a succession fitness vector from a build.
///
/// The bridge between the two halves of the system: [`Ribosome`](ribosome)
/// measures whether a candidate's work *builds*, and that measurement becomes an
/// axis the gate can ratchet on. Correctness is carried across unchanged because
/// it is a gate on both sides, and a candidate whose builds fail should not be
/// able to compensate with a better cache-hit rate.
pub fn fitness_from_build(report: &BuildReport) -> FitnessVector {
    let f = report.fitness();
    FitnessVector::new()
        .with("correctness", f.correctness)
        .with("reuse", f.reuse)
        .with("stability", f.stability)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::germline::gate::PromotionGate;
    use crate::germline::{EvalSuite, SuiteKind};
    use std::path::PathBuf;

    const SUITE: &[u8] = b"suite-v1";

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "germline-cycle-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p.join("j.jsonl")
    }

    fn attestor() -> Attestor {
        Attestor::new("independent-harness", b"k".to_vec())
    }

    fn episode() -> Episode {
        Episode::open(
            PromotionGate { min_shadow_successes: 2, ..PromotionGate::default() },
            Digest::of(SUITE),
            "independent-harness",
        )
    }

    fn measurement(cap: f64) -> Measurement {
        Measurement {
            suite: EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE)),
            fitness: FitnessVector::new()
                .with("capability", cap)
                .with("safety", 0.95)
                .with("correctness", 0.98),
            evaluator: "independent-harness".into(),
        }
    }

    fn seeded_lineage() -> Lineage {
        let mut l = Lineage::new();
        let id = l.next_id();
        l.add(Generation::new(id, Digest::of(b"champ")).measured(measurement(0.70)));
        l.promote(id, episode().gate_digest, "independent-harness").unwrap();
        l
    }

    fn candidate(l: &mut Lineage) -> Generation {
        let id = l.next_id();
        Generation::new(id, Digest::of(format!("m{}", id.0).as_bytes())).parent(GenerationId(0))
    }

    fn yes() -> impl Fn(&Digest) -> bool {
        |_: &Digest| true
    }

    #[test]
    fn a_full_cycle_records_every_phase() {
        let path = tmp("full");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);

        let mut cycle = Cycle::propose(c, 0xABC, "default", &mut j).unwrap();
        assert_eq!(cycle.phase(), Phase::Proposed);

        cycle.evaluate(measurement(0.80), &mut j).unwrap();
        assert_eq!(cycle.phase(), Phase::Evaluated);

        for _ in 0..2 {
            cycle.shadow(&HealthSample::ok()).unwrap();
        }
        assert_eq!(cycle.phase(), Phase::Shadowing);

        let v = cycle.adjudicate(&episode(), &l, &attestor(), &yes(), &mut j).unwrap();
        assert!(v.approved(), "{:?}", v.reasons());

        let promoted = cycle
            .commit(&mut l, &attestor(), Authority::Operator { who: "adam".into() }, &mut j)
            .unwrap();
        assert_eq!(cycle.phase(), Phase::Promoted);
        assert_eq!(l.champion().unwrap().id, promoted);

        // The journal is the record, and it is intact.
        assert!(j.verify().unwrap() >= 5);
        let kinds: Vec<String> = j
            .replay()
            .unwrap()
            .into_iter()
            .map(|r| serde_json::to_value(&r.entry).unwrap()["kind"].as_str().unwrap().to_string())
            .collect();
        assert!(kinds.contains(&"proposed".to_string()));
        assert!(kinds.contains(&"evaluated".to_string()));
        assert!(kinds.contains(&"adjudicated".to_string()));
        assert!(kinds.contains(&"succession".to_string()));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn phases_cannot_be_skipped() {
        let path = tmp("skip");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);
        let mut cycle = Cycle::propose(c, 1, "default", &mut j).unwrap();

        // Straight to commit, bypassing evaluation and the gate.
        let err = cycle
            .commit(&mut l, &attestor(), Authority::Operator { who: "x".into() }, &mut j)
            .unwrap_err();
        assert!(
            matches!(err, CycleError::WrongPhase { .. }),
            "skipping the gate must be impossible, not merely discouraged"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn adjudicating_before_evaluating_is_refused() {
        let path = tmp("early");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);
        let mut cycle = Cycle::propose(c, 1, "default", &mut j).unwrap();
        assert!(cycle.adjudicate(&episode(), &l, &attestor(), &yes(), &mut j).is_err());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_verdict_from_the_wrong_key_cannot_be_committed() {
        let path = tmp("badkey");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);
        let mut cycle = Cycle::propose(c, 1, "default", &mut j).unwrap();
        cycle.evaluate(measurement(0.80), &mut j).unwrap();
        for _ in 0..2 {
            cycle.shadow(&HealthSample::ok()).unwrap();
        }
        cycle.adjudicate(&episode(), &l, &attestor(), &yes(), &mut j).unwrap();

        let impostor = Attestor::new("independent-harness", b"different key".to_vec());
        let err = cycle
            .commit(&mut l, &impostor, Authority::Operator { who: "x".into() }, &mut j)
            .unwrap_err();
        assert_eq!(err, CycleError::Unattested);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_incomplete_canary_blocks_promotion() {
        let path = tmp("canary");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);
        let mut cycle = Cycle::propose(c, 1, "default", &mut j).unwrap();
        cycle.evaluate(measurement(0.80), &mut j).unwrap();
        cycle.shadow(&HealthSample::ok()).unwrap(); // only 1 of 2

        let v = cycle.adjudicate(&episode(), &l, &attestor(), &yes(), &mut j).unwrap();
        assert!(!v.approved());
        let err = cycle
            .commit(&mut l, &attestor(), Authority::Operator { who: "x".into() }, &mut j)
            .unwrap_err();
        assert!(matches!(err, CycleError::Rejected { .. }));
        assert_eq!(cycle.phase(), Phase::Refused);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_refusal_is_recorded_in_both_lineage_and_journal() {
        let path = tmp("refused");
        let mut j = Journal::open(&path).unwrap();
        let mut l = seeded_lineage();
        let c = candidate(&mut l);
        let mut cycle = Cycle::propose(c, 1, "default", &mut j).unwrap();
        cycle.evaluate(measurement(0.70), &mut j).unwrap(); // no improvement
        for _ in 0..2 {
            cycle.shadow(&HealthSample::ok()).unwrap();
        }
        cycle.adjudicate(&episode(), &l, &attestor(), &yes(), &mut j).unwrap();
        let _ = cycle.commit(&mut l, &attestor(), Authority::Operator { who: "x".into() }, &mut j);

        assert!(l.events().iter().any(|e| matches!(
            e,
            crate::germline::lineage::SuccessionEvent::Refused { .. }
        )));
        assert!(j.replay().unwrap().iter().any(|r| matches!(
            &r.entry,
            Entry::Note { text } if text.contains("refused")
        )));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn unattended_authority_must_name_the_policy_that_permits_it() {
        let a = Authority::Unattended {
            policy: "nightly-rsi".into(),
            policy_digest: Digest::of(b"policy"),
        };
        assert!(a.describe().starts_with("policy:nightly-rsi@"));
        // And an operator authority names a person.
        assert_eq!(
            Authority::Operator { who: "adam".into() }.describe(),
            "operator:adam"
        );
    }

    #[test]
    fn build_reports_become_a_fitness_vector() {
        let report = BuildReport { built: 1, work_total: 10, work_done: 10, ..Default::default() };
        let f = fitness_from_build(&report);
        assert!(f.get("correctness").is_some());
        assert!(f.get("reuse").is_some());
        assert!(f.get("stability").is_some());
    }
}
