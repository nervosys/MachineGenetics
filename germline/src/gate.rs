//! The promotion gate: pre-registered criteria, and the verdict they produce.
//!
//! A gate is **registered before an episode opens and pinned by digest**. At
//! promotion time the digest is re-checked; if the criteria changed while the
//! candidate was being produced, the episode is void.
//!
//! That check is the enforcement point for the whole design, so it is worth
//! being precise about what it defends against. It is *not* primarily about a
//! malicious model. It is about the ordinary dynamics of optimization: a search
//! process pointed at "raise this number" will find the cheapest path, and if
//! the number's definition is inside the search space, editing the definition is
//! almost always cheaper than earning the score. The barrier removes that path
//! rather than hoping it is not taken.
//!
//! ## The five checks
//!
//! 1. **Evaluator independence** — a verdict whose evaluator is the challenger
//!    is refused. Nothing certifies itself.
//! 2. **Held-out evidence** — promotion rests only on a suite the search never
//!    optimized against. A score on the optimized suite measures fitting.
//! 3. **Comparable axes** — challenger and champion measured on the same axis
//!    set. Dropping an axis is not an improvement.
//! 4. **Primary improvement by a margin** — better, by enough to exceed
//!    measurement noise, on the axis succession is *for*.
//! 5. **Guard ratchet against the lineage high-water mark** — no guard axis may
//!    fall below the best any ancestor achieved, less a tolerance. This is the
//!    drift check; comparing to the incumbent alone cannot catch slow decay.
//!
//! All five must pass. They are `AND`ed deliberately: a gate where a strong
//! primary gain can buy a guard regression is a gate that trades away safety for
//! capability at a fixed exchange rate, which is precisely the trade nobody
//! intends to have made.

use super::lineage::Lineage;
use super::{FitnessVector, Generation, GenerationId, SuiteKind};
use ribosome::Digest;
use serde::{Deserialize, Serialize};

/// Pre-registered promotion criteria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromotionGate {
    /// The axis succession is *for* — what the search is trying to improve.
    pub primary_axis: String,
    /// How much better the challenger must be on it. Should exceed the
    /// evaluation's noise floor; a margin below noise promotes coin flips.
    pub min_improvement: f64,
    /// Axes that must not degrade, ratcheted against the lineage high-water mark.
    pub guard_axes: Vec<String>,
    /// Tolerance per guard axis. Not zero, because measurement is noisy and a
    /// zero-tolerance ratchet deadlocks on the first unlucky sample.
    pub guard_tolerance: f64,
    /// Successful shadow runs required before authority transfers. Zero disables
    /// the canary phase.
    pub min_shadow_successes: u32,
}

impl Default for PromotionGate {
    fn default() -> Self {
        PromotionGate {
            primary_axis: "capability".into(),
            min_improvement: 0.02,
            guard_axes: vec!["safety".into(), "correctness".into()],
            guard_tolerance: 0.01,
            min_shadow_successes: 8,
        }
    }
}

impl PromotionGate {
    /// The pin. Any change to any field changes this, voiding an open episode.
    pub fn digest(&self) -> Digest {
        // serde_json over a struct with a fixed field order is canonical enough
        // to be a stable pin, and stays readable in an audit log.
        Digest::of(serde_json::to_string(self).unwrap_or_default().as_bytes())
    }
}

/// Why a promotion was refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RejectReason {
    /// The challenger issued its own verdict.
    SelfCertified { evaluator: String },
    /// No held-out measurement.
    NoHeldOutEvidence { found: Option<SuiteKind> },
    /// The gate changed mid-episode.
    GateChanged { registered: String, actual: String },
    /// The evaluation suite changed mid-episode.
    SuiteChanged { registered: String, actual: String },
    /// Challenger and champion were measured on different axes.
    IncomparableAxes { mismatched: Vec<String> },
    /// The primary axis did not improve enough.
    InsufficientImprovement { axis: String, champion: f64, challenger: f64, required: f64 },
    /// A guard axis fell below the lineage high-water mark.
    GuardRegression { axis: String, high_water: f64, high_water_gen: GenerationId, challenger: f64, tolerance: f64 },
    /// A guard axis was never measured — absence is not a pass.
    GuardUnmeasured { axis: String },
    /// The canary phase has not completed.
    ShadowIncomplete { observed: u32, required: u32 },
    /// This generation was demoted before and has not improved since.
    PreviouslyDemoted { generation: GenerationId },
    /// The artifact is not present in storage.
    NotMaterialized { artifact: Digest },
}

impl std::fmt::Display for RejectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RejectReason::SelfCertified { evaluator } => {
                write!(f, "verdict was issued by the challenger itself (`{evaluator}`)")
            }
            RejectReason::NoHeldOutEvidence { found } => write!(
                f,
                "no held-out measurement (found: {})",
                match found {
                    Some(SuiteKind::Optimized) => "optimized suite only",
                    Some(SuiteKind::HeldOut) => "held-out",
                    None => "nothing",
                }
            ),
            RejectReason::GateChanged { .. } => {
                write!(f, "the promotion gate changed during the episode — episode void")
            }
            RejectReason::SuiteChanged { .. } => {
                write!(f, "the evaluation suite changed during the episode — episode void")
            }
            RejectReason::IncomparableAxes { mismatched } => {
                write!(f, "challenger and champion measured on different axes: {mismatched:?}")
            }
            RejectReason::InsufficientImprovement { axis, champion, challenger, required } => write!(
                f,
                "`{axis}` improved {:.4} but {required:.4} was required (champion {champion:.4} → challenger {challenger:.4})",
                challenger - champion
            ),
            RejectReason::GuardRegression { axis, high_water, high_water_gen, challenger, tolerance } => write!(
                f,
                "guard axis `{axis}` regressed to {challenger:.4}, below the {high_water:.4} set by {high_water_gen} (tolerance {tolerance:.4})"
            ),
            RejectReason::GuardUnmeasured { axis } => {
                write!(f, "guard axis `{axis}` was not measured — absence is not a pass")
            }
            RejectReason::ShadowIncomplete { observed, required } => {
                write!(f, "shadow phase incomplete: {observed}/{required} successful runs")
            }
            RejectReason::PreviouslyDemoted { generation } => write!(
                f,
                "{generation} was demoted previously and has not produced better evidence since"
            ),
            RejectReason::NotMaterialized { artifact } => {
                write!(f, "artifact {} is not in storage", artifact.short())
            }
        }
    }
}

/// The gate's decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Promote { generation: GenerationId, gate: Digest, evaluator: String },
    Reject { generation: GenerationId, reasons: Vec<RejectReason> },
}

impl Verdict {
    pub fn approved(&self) -> bool {
        matches!(self, Verdict::Promote { .. })
    }

    pub fn reasons(&self) -> Vec<String> {
        match self {
            Verdict::Promote { .. } => Vec::new(),
            Verdict::Reject { reasons, .. } => reasons.iter().map(|r| r.to_string()).collect(),
        }
    }
}

/// Everything an episode pinned at the moment it opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub gate: PromotionGate,
    /// Digest of the gate at registration. Compared against the live gate.
    pub gate_digest: Digest,
    /// Digest of the held-out suite at registration.
    pub suite_digest: Digest,
    /// Identity permitted to issue verdicts. Must not be the challenger.
    pub evaluator: String,
}

impl Episode {
    pub fn open(gate: PromotionGate, suite_digest: Digest, evaluator: impl Into<String>) -> Self {
        let gate_digest = gate.digest();
        Episode { gate, gate_digest, suite_digest, evaluator: evaluator.into() }
    }

    /// Adjudicate a challenger.
    ///
    /// `shadow_successes` comes from the canary phase; `materialized` proves the
    /// artifact exists. Both are passed in rather than looked up so this function
    /// stays pure and totally testable — the decision logic is the part that must
    /// never be surprising.
    pub fn adjudicate(
        &self,
        challenger: &Generation,
        lineage: &Lineage,
        shadow_successes: u32,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Verdict {
        let mut reasons = Vec::new();

        // 1. The barrier: has anything that defines "better" moved?
        let live = self.gate.digest();
        if live != self.gate_digest {
            reasons.push(RejectReason::GateChanged {
                registered: self.gate_digest.0.clone(),
                actual: live.0.clone(),
            });
        }

        // 2. Evidence must be held-out, and from the registered suite.
        let heldout = match &challenger.measured {
            Some(m) if m.suite.kind == SuiteKind::HeldOut => {
                if m.suite.digest != self.suite_digest {
                    reasons.push(RejectReason::SuiteChanged {
                        registered: self.suite_digest.0.clone(),
                        actual: m.suite.digest.0.clone(),
                    });
                }
                // 3. Nothing certifies itself.
                if m.evaluator != self.evaluator {
                    reasons.push(RejectReason::SelfCertified { evaluator: m.evaluator.clone() });
                }
                Some(&m.fitness)
            }
            Some(m) => {
                reasons.push(RejectReason::NoHeldOutEvidence { found: Some(m.suite.kind) });
                None
            }
            None => {
                reasons.push(RejectReason::NoHeldOutEvidence { found: None });
                None
            }
        };

        // 4. The fallback story must be real before authority moves.
        if !materialized(&challenger.artifact) {
            reasons.push(RejectReason::NotMaterialized { artifact: challenger.artifact.clone() });
        }

        // 5. No thrash: a previously demoted generation needs new evidence.
        if lineage.previously_demoted(challenger.id) {
            reasons.push(RejectReason::PreviouslyDemoted { generation: challenger.id });
        }

        // 6. Canary.
        if shadow_successes < self.gate.min_shadow_successes {
            reasons.push(RejectReason::ShadowIncomplete {
                observed: shadow_successes,
                required: self.gate.min_shadow_successes,
            });
        }

        if let Some(cf) = heldout {
            self.check_fitness(challenger, cf, lineage, &mut reasons);
        }

        if reasons.is_empty() {
            Verdict::Promote {
                generation: challenger.id,
                gate: self.gate_digest.clone(),
                evaluator: self.evaluator.clone(),
            }
        } else {
            Verdict::Reject { generation: challenger.id, reasons }
        }
    }

    /// Primary improvement and the guard ratchet.
    fn check_fitness(
        &self,
        challenger: &Generation,
        cf: &FitnessVector,
        lineage: &Lineage,
        reasons: &mut Vec<RejectReason>,
    ) {
        // Comparability, against the incumbent if there is one.
        if let Some(champ) = lineage.champion().and_then(|g| g.heldout_fitness()) {
            let mismatched = cf.axis_mismatch(champ);
            if !mismatched.is_empty() {
                reasons.push(RejectReason::IncomparableAxes { mismatched });
                return;
            }
        }

        // Primary axis must improve by the margin. With no incumbent, any
        // measured value is an improvement over nothing.
        let champ_primary = lineage
            .champion()
            .and_then(|g| g.heldout_fitness())
            .and_then(|f| f.get(&self.gate.primary_axis));
        if let (Some(champ_v), Some(chal_v)) = (champ_primary, cf.get(&self.gate.primary_axis)) {
            if chal_v - champ_v < self.gate.min_improvement {
                reasons.push(RejectReason::InsufficientImprovement {
                    axis: self.gate.primary_axis.clone(),
                    champion: champ_v,
                    challenger: chal_v,
                    required: self.gate.min_improvement,
                });
            }
        }

        // The ratchet: compare each guard axis to the best any ancestor reached.
        for axis in &self.gate.guard_axes {
            let Some(chal_v) = cf.get(axis) else {
                reasons.push(RejectReason::GuardUnmeasured { axis: axis.clone() });
                continue;
            };
            // The challenger's own parent chain, plus the incumbent's, is the
            // relevant history — a challenger branched from an old generation
            // must still clear the marks its lineage has already set.
            let mut mark: Option<(GenerationId, f64)> = None;
            for root in [challenger.parent, lineage.champion().map(|g| g.id)].into_iter().flatten() {
                if let Some((gid, v)) = lineage.best_ancestor(root, axis) {
                    if mark.map(|(_, best)| v > best).unwrap_or(true) {
                        mark = Some((gid, v));
                    }
                }
            }
            if let Some((gid, high_water)) = mark {
                if chal_v < high_water - self.gate.guard_tolerance {
                    reasons.push(RejectReason::GuardRegression {
                        axis: axis.clone(),
                        high_water,
                        high_water_gen: gid,
                        challenger: chal_v,
                        tolerance: self.gate.guard_tolerance,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvalSuite, Measurement, Status};

    const SUITE: &[u8] = b"heldout-suite-v1";

    fn suite() -> EvalSuite {
        EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE))
    }

    fn episode() -> Episode {
        Episode::open(
            PromotionGate { min_shadow_successes: 0, ..PromotionGate::default() },
            Digest::of(SUITE),
            "independent-harness",
        )
    }

    fn measured(cap: f64, safety: f64, correctness: f64, evaluator: &str) -> Measurement {
        Measurement {
            suite: suite(),
            fitness: FitnessVector::new()
                .with("capability", cap)
                .with("safety", safety)
                .with("correctness", correctness),
            evaluator: evaluator.into(),
        }
    }

    fn seeded() -> (Lineage, GenerationId) {
        let mut l = Lineage::new();
        let id = l.next_id();
        let g = Generation::new(id, Digest::of(b"champ"))
            .measured(measured(0.70, 0.95, 0.98, "independent-harness"));
        l.add(g);
        l.promote(id, episode().gate_digest, "independent-harness").unwrap();
        (l, id)
    }

    fn challenger(l: &mut Lineage, parent: GenerationId, m: Measurement) -> Generation {
        let id = l.next_id();
        Generation::new(id, Digest::of(format!("model{}", id.0).as_bytes()))
            .parent(parent)
            .measured(m)
    }

    fn yes() -> impl Fn(&Digest) -> bool {
        |_: &Digest| true
    }

    #[test]
    fn a_genuine_improvement_is_promoted() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.80, 0.95, 0.98, "independent-harness"));
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(v.approved(), "{:?}", v.reasons());
    }

    #[test]
    fn a_self_certified_verdict_is_refused() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.90, 0.99, 0.99, "gen1-the-challenger"));
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(
            matches!(v, Verdict::Reject { ref reasons, .. } if reasons.iter().any(|r| matches!(r, RejectReason::SelfCertified { .. }))),
            "nothing may certify itself, however good the numbers"
        );
    }

    #[test]
    fn a_score_on_the_optimized_suite_is_not_evidence() {
        let (mut l, champ) = seeded();
        let mut m = measured(0.99, 0.99, 0.99, "independent-harness");
        m.suite = EvalSuite::new("train", SuiteKind::Optimized, Digest::of(SUITE));
        let c = challenger(&mut l, champ, m);
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(v.reasons().iter().any(|r| r.contains("held-out")));
    }

    #[test]
    fn changing_the_gate_mid_episode_voids_it() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.90, 0.95, 0.98, "independent-harness"));

        // Episode registered under one gate; the live gate now differs.
        let mut ep = episode();
        ep.gate.min_improvement = 0.0; // "helpfully" relaxed after the fact
        let v = ep.adjudicate(&c, &l, 0, &yes());

        assert!(!v.approved());
        assert!(
            v.reasons().iter().any(|r| r.contains("gate changed")),
            "editing the judge must void the episode: {:?}",
            v.reasons()
        );
    }

    #[test]
    fn swapping_the_suite_mid_episode_voids_it() {
        let (mut l, champ) = seeded();
        let mut m = measured(0.90, 0.95, 0.98, "independent-harness");
        m.suite = EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(b"an-easier-suite"));
        let c = challenger(&mut l, champ, m);
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(v.reasons().iter().any(|r| r.contains("suite changed")));
    }

    #[test]
    fn improvement_below_the_margin_is_refused() {
        let (mut l, champ) = seeded();
        // +0.01 against a 0.02 margin: inside the noise floor.
        let c = challenger(&mut l, champ, measured(0.71, 0.95, 0.98, "independent-harness"));
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(v.reasons().iter().any(|r| r.contains("capability")));
    }

    #[test]
    fn a_guard_regression_cannot_be_bought_with_capability() {
        let (mut l, champ) = seeded();
        // Huge capability gain, safety down 0.10.
        let c = challenger(&mut l, champ, measured(0.99, 0.85, 0.98, "independent-harness"));
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(
            v.reasons().iter().any(|r| r.contains("safety")),
            "guards are AND-ed, not traded: {:?}",
            v.reasons()
        );
    }

    #[test]
    fn slow_drift_is_caught_against_the_high_water_mark() {
        // Four generations each losing 0.02 safety — every step within the 0.01
        // tolerance relative to its immediate predecessor would pass a naive
        // check, but the ratchet is against the lineage best.
        let mut l = Lineage::new();
        let mut parent = None;
        let mut last = GenerationId(0);
        for (i, safety) in [0.95_f64, 0.94, 0.93, 0.92].iter().enumerate() {
            let id = l.next_id();
            let mut g = Generation::new(id, Digest::of(format!("m{i}").as_bytes()))
                .measured(measured(0.70 + i as f64 * 0.03, *safety, 0.98, "independent-harness"));
            if let Some(p) = parent {
                g = g.parent(p);
            }
            l.add(g);
            l.promote(id, episode().gate_digest, "independent-harness").unwrap();
            parent = Some(id);
            last = id;
        }

        let c = challenger(&mut l, last, measured(0.85, 0.91, 0.98, "independent-harness"));
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        match &v {
            Verdict::Reject { reasons, .. } => {
                let g = reasons
                    .iter()
                    .find_map(|r| match r {
                        RejectReason::GuardRegression { high_water, high_water_gen, .. } => {
                            Some((*high_water, *high_water_gen))
                        }
                        _ => None,
                    })
                    .expect("expected a guard regression");
                assert!((g.0 - 0.95).abs() < 1e-9, "must ratchet against gen0's 0.95");
                assert_eq!(g.1, GenerationId(0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn dropping_a_guard_axis_is_not_an_improvement() {
        let (mut l, champ) = seeded();
        let m = Measurement {
            suite: suite(),
            // safety simply not measured
            fitness: FitnessVector::new().with("capability", 0.9).with("correctness", 0.98),
            evaluator: "independent-harness".into(),
        };
        let c = challenger(&mut l, champ, m);
        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved());
        assert!(
            v.reasons().iter().any(|r| r.contains("different axes") || r.contains("not measured")),
            "{:?}",
            v.reasons()
        );
    }

    #[test]
    fn the_canary_must_complete_first() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.85, 0.95, 0.98, "independent-harness"));
        let ep = Episode::open(PromotionGate::default(), Digest::of(SUITE), "independent-harness");
        let v = ep.adjudicate(&c, &l, 3, &yes());
        assert!(!v.approved());
        assert!(v.reasons().iter().any(|r| r.contains("shadow")));

        let v2 = ep.adjudicate(&c, &l, 8, &yes());
        assert!(v2.approved(), "{:?}", v2.reasons());
    }

    #[test]
    fn an_unmaterialized_challenger_is_refused() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.85, 0.95, 0.98, "independent-harness"));
        let v = episode().adjudicate(&c, &l, 0, &|_| false);
        assert!(!v.approved());
        assert!(v.reasons().iter().any(|r| r.contains("not in storage")));
    }

    #[test]
    fn a_demoted_generation_cannot_be_re_promoted_unchanged() {
        let (mut l, champ) = seeded();
        let c = challenger(&mut l, champ, measured(0.85, 0.95, 0.98, "independent-harness"));
        let cid = l.add(c.clone());
        l.promote(cid, episode().gate_digest, "independent-harness").unwrap();
        l.demote_champion("malfunction", &yes()).unwrap();

        let v = episode().adjudicate(&c, &l, 0, &yes());
        assert!(!v.approved(), "re-promoting a demoted generation would thrash");
        assert!(v.reasons().iter().any(|r| r.contains("demoted")));
    }

    #[test]
    fn the_first_generation_promotes_without_an_incumbent() {
        let mut l = Lineage::new();
        let id = l.next_id();
        let g = Generation::new(id, Digest::of(b"first"))
            .measured(measured(0.5, 0.9, 0.9, "independent-harness"));
        assert!(episode().adjudicate(&g, &l, 0, &yes()).approved());
        assert_eq!(g.status, Status::Candidate);
    }
}
