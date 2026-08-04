//! # Germline — model succession, handoff, and fallback
//!
//! The operating mode this implements: a model proposes a higher-fitness
//! successor by *directed* evolution, hands subsequent RSI work to it, and falls
//! back to its predecessor if the successor malfunctions or declines.
//!
//! The name is the load-bearing part of the design. In biology the **germline**
//! is the heritable lineage, and the **Weismann barrier** is the one-way wall
//! that stops somatic changes — anything an organism does to itself during its
//! life — from entering it. Acquired characteristics are not inherited, and that
//! is a feature: it means the lineage cannot be corrupted by whatever the
//! organism did to itself.
//!
//! This module enforces the same barrier, for the same reason.
//!
//! ## The problem succession actually poses
//!
//! Generating a better model is the easy half. The hard half is **who certifies
//! that it is better**, because the obvious arrangement is circular: the
//! incumbent hands authority to the successor, and the successor then evaluates
//! its own successors. Two things go wrong, both quietly.
//!
//! 1. **Evaluator capture.** A system with write access to the thing that judges
//!    it will, under optimization pressure, discover that editing the judge is
//!    cheaper than satisfying it. This needs no intent and no cleverness — it is
//!    simply the lowest-cost path to a higher score. "RSI" then degenerates into
//!    a model learning to rewrite its own test suite, while every dashboard
//!    improves.
//! 2. **Lineage drift.** Each generation regresses slightly on some axis nobody
//!    is watching, each step within tolerance. Twenty generations later the
//!    capability is gone and no single promotion looks wrong in review.
//!
//! So the invariants here are:
//!
//! - **The gate and the evaluation suite are pinned by digest** before a
//!   succession episode opens, and re-verified at promotion
//!   ([`gate::PromotionGate::digest`]). If either changed mid-episode, the
//!   episode is void — not re-run, *void*. This is the Weismann barrier.
//! - **The challenger never produces its own verdict.** A verdict records which
//!   evaluator issued it ([`lineage::SuccessionEvent`]); one naming the
//!   challenger is rejected.
//! - **Guard axes ratchet against the best ancestor, not the incumbent**
//!   ([`gate`]). This is what makes drift detectable: a slow decay is invisible
//!   step-to-step and obvious against the lineage high-water mark.
//! - **Fallback must be materialized.** A generation is only a valid rollback
//!   target while its artifact is present and verifies. A fallback you cannot
//!   actually run is not a fallback, and discovering that during an incident is
//!   the worst possible time.
//!
//! ## Shape
//!
//! | Module | Role |
//! |---|---|
//! | [`variation`] | candidate production — deterministic, seeded, re-derivable |
//! | [`directed`] | predict-then-evaluate search, and the calibration that keeps it honest |
//! | [`gate`] | pre-registered promotion criteria and the verdict they produce |
//! | [`attest`] | proof that a verdict came from the evaluator it names |
//! | [`lineage`] | append-only generation log, champion pointer, rollback |
//! | [`journal`] | durable, hash-chained record of everything that happened |
//! | [`cycle`] | the state machine joining propose → evaluate → adjudicate → hand off |
//! | [`supervisor`] | post-handoff health, failure modes, automatic demotion |
//! | [`runner`] | bounded, policy-pinned cycles and their halt conditions |
//! | [`workload`] | a real workload: architecture search evaluated by real builds |
//!
//! ## Status
//!
//! Implemented and tested: the full control plane, including [`runner`], which
//! drives bounded, policy-pinned cycles. **Not** implemented: model training and
//! inference themselves — [`runner::Workload`] is the trait to implement. The
//! runner is bounded by construction: there is no daemon mode and no way to ask
//! for unlimited cycles. See `GERMLINE.md`.

pub mod attest;
pub mod cycle;
pub mod directed;
pub mod gate;
pub mod journal;
pub mod lineage;
pub mod runner;
pub mod supervisor;
pub mod variation;
pub mod workload;

use crate::ribosome::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Position in a lineage. Monotonic; never reused, even after a rollback —
/// re-promoting generation 4 does not create a second generation 4, because the
/// history of what was tried must stay legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GenerationId(pub u64);

impl std::fmt::Display for GenerationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gen{}", self.0)
    }
}

/// Named fitness axes, each normalized to `[0,1]`, higher is better.
///
/// A vector rather than a scalar because succession decisions are
/// multi-objective and a scalar hides exactly the trades that matter — "overall
/// better" is how a capability quietly disappears. The scalar exists
/// ([`FitnessVector::composite`]) but is only ever used for *ranking* candidates,
/// never for deciding promotion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FitnessVector(pub BTreeMap<String, f64>);

impl FitnessVector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, axis: impl Into<String>, value: f64) -> Self {
        self.0.insert(axis.into(), value.clamp(0.0, 1.0));
        self
    }

    pub fn get(&self, axis: &str) -> Option<f64> {
        self.0.get(axis).copied()
    }

    pub fn axes(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(|s| s.as_str())
    }

    /// Unweighted mean. For ranking candidates only — see the type docs.
    pub fn composite(&self) -> f64 {
        if self.0.is_empty() {
            return 0.0;
        }
        self.0.values().sum::<f64>() / self.0.len() as f64
    }

    /// Axes present in `self` but absent in `other`, and vice versa.
    ///
    /// A challenger measured on a different axis set than the champion is not
    /// comparable, and silently intersecting the axes is how a guard axis gets
    /// dropped from consideration by "simplifying the eval".
    pub fn axis_mismatch(&self, other: &FitnessVector) -> Vec<String> {
        let mut out: Vec<String> = self
            .0
            .keys()
            .filter(|k| !other.0.contains_key(*k))
            .chain(other.0.keys().filter(|k| !self.0.contains_key(*k)))
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

/// Which suite produced a measurement.
///
/// The distinction is the whole basis of a trustworthy promotion. A candidate
/// selected by maximizing performance on a suite has, by construction, been
/// fitted to that suite; its score there measures the fitting as much as the
/// capability. Only a suite the search never saw carries information about
/// whether anything real improved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteKind {
    /// The search optimized against this. Necessary, and not evidence.
    Optimized,
    /// Withheld from the search. The only thing promotion may rest on.
    HeldOut,
}

/// A pinned evaluation suite: identity, kind, and content digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalSuite {
    pub id: String,
    pub kind: SuiteKind,
    /// Digest of the suite contents. Re-checked at promotion; a changed suite
    /// voids the episode.
    pub digest: Digest,
}

impl EvalSuite {
    pub fn new(id: impl Into<String>, kind: SuiteKind, digest: Digest) -> Self {
        EvalSuite { id: id.into(), kind, digest }
    }
}

/// One measurement of one generation against one suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub suite: EvalSuite,
    pub fitness: FitnessVector,
    /// Who ran it. A verdict whose evaluator is the challenger is refused.
    pub evaluator: String,
}

/// Where a generation sits in its life cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Proposed and measured; not yet promoted.
    Candidate,
    /// Currently holds RSI authority.
    Champion,
    /// Was champion, superseded cleanly. A valid rollback target.
    Retired,
    /// Demoted after a failure. **Not** a rollback target, and not re-promotable
    /// without strictly better evidence than it had when demoted.
    Quarantined,
}

/// A model version in the lineage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Generation {
    pub id: GenerationId,
    pub parent: Option<GenerationId>,
    /// The model artifact, in the same CAS the build engine uses. Rollback
    /// requires this to still be present and verify.
    pub artifact: Digest,
    /// Held-out measurement. `None` for a generation that was never evaluated.
    pub measured: Option<Measurement>,
    pub status: Status,
    /// Digest of the gate in force when this generation was promoted.
    pub promoted_under: Option<Digest>,
    pub note: String,
}

impl Generation {
    pub fn new(id: GenerationId, artifact: Digest) -> Self {
        Generation {
            id,
            parent: None,
            artifact,
            measured: None,
            status: Status::Candidate,
            promoted_under: None,
            note: String::new(),
        }
    }

    pub fn parent(mut self, p: GenerationId) -> Self {
        self.parent = Some(p);
        self
    }

    pub fn measured(mut self, m: Measurement) -> Self {
        self.measured = Some(m);
        self
    }

    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.note = n.into();
        self
    }

    /// Held-out fitness, if this generation has been measured on a held-out
    /// suite. Deliberately returns `None` for an optimized-suite measurement:
    /// callers must not be able to reach for the flattering number by accident.
    pub fn heldout_fitness(&self) -> Option<&FitnessVector> {
        self.measured
            .as_ref()
            .filter(|m| m.suite.kind == SuiteKind::HeldOut)
            .map(|m| &m.fitness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitness_axes_are_clamped() {
        let f = FitnessVector::new().with("a", 1.7).with("b", -0.3);
        assert_eq!(f.get("a"), Some(1.0));
        assert_eq!(f.get("b"), Some(0.0));
    }

    #[test]
    fn axis_mismatch_is_symmetric_and_named() {
        let a = FitnessVector::new().with("x", 0.5).with("y", 0.5);
        let b = FitnessVector::new().with("x", 0.5).with("z", 0.5);
        assert_eq!(a.axis_mismatch(&b), vec!["y".to_string(), "z".to_string()]);
    }

    #[test]
    fn identical_axis_sets_do_not_mismatch() {
        let a = FitnessVector::new().with("x", 0.1);
        let b = FitnessVector::new().with("x", 0.9);
        assert!(a.axis_mismatch(&b).is_empty());
    }

    #[test]
    fn optimized_measurements_are_not_reachable_as_heldout() {
        let g = Generation::new(GenerationId(1), Digest::of(b"m")).measured(Measurement {
            suite: EvalSuite::new("train", SuiteKind::Optimized, Digest::of(b"s")),
            fitness: FitnessVector::new().with("a", 0.99),
            evaluator: "harness".into(),
        });
        assert!(
            g.heldout_fitness().is_none(),
            "a score on the suite the search optimized must not be usable as evidence"
        );
    }

    #[test]
    fn composite_is_the_mean() {
        let f = FitnessVector::new().with("a", 0.0).with("b", 1.0);
        assert!((f.composite() - 0.5).abs() < 1e-12);
    }
}
