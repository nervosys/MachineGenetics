//! The lineage: an append-only record of generations and authority transfers.
//!
//! Append-only is not a storage preference. A self-modifying system that can
//! rewrite its own history cannot be debugged after an incident — you would be
//! asking the system under investigation for the evidence. Promotions,
//! demotions, and rollbacks are all *events appended* here; nothing is edited,
//! and a demoted generation stays in the record with the reason attached.
//!
//! ## High-water marks are what make drift detectable
//!
//! [`Lineage::best_ancestor`] returns the best value any ancestor ever achieved
//! on an axis. The gate ratchets against that rather than against the incumbent,
//! which is the difference between catching slow decay and not:
//!
//! ```text
//! gen1  safety 0.95   ← high-water mark
//! gen2  safety 0.93   each step −0.02, under any sane per-step tolerance
//! gen3  safety 0.91
//! gen4  safety 0.89   vs incumbent: fine.  vs gen1: −0.06, caught.
//! ```
//!
//! Comparing only to the incumbent makes every one of those promotions look
//! reasonable in isolation, which is exactly how the capability disappears.

use super::{Generation, GenerationId, Status};
use ribosome::Digest;
use serde::{Deserialize, Serialize};

/// A transfer of authority, or a refused one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SuccessionEvent {
    /// A generation took authority.
    Promoted {
        generation: GenerationId,
        from: Option<GenerationId>,
        /// Digest of the gate that authorized it — the audit anchor.
        gate: Digest,
        evaluator: String,
    },
    /// A champion lost authority because of a failure.
    Demoted { generation: GenerationId, to: GenerationId, reason: String },
    /// Authority moved back to an earlier generation deliberately.
    RolledBack { from: GenerationId, to: GenerationId, reason: String },
    /// A promotion was refused, recorded so repeated near-misses are visible.
    Refused { generation: GenerationId, reasons: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageError {
    UnknownGeneration(GenerationId),
    /// The target's artifact is absent or fails verification.
    NotMaterialized { generation: GenerationId, artifact: Digest },
    /// Quarantined generations are not rollback targets.
    Quarantined(GenerationId),
    /// There is no earlier champion to fall back to.
    NoFallback,
}

impl std::fmt::Display for LineageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineageError::UnknownGeneration(g) => write!(f, "unknown generation {g}"),
            LineageError::NotMaterialized { generation, artifact } => write!(
                f,
                "generation {generation}'s artifact {} is not materialized — it cannot be a fallback",
                artifact.short()
            ),
            LineageError::Quarantined(g) => {
                write!(f, "generation {g} is quarantined and is not a valid rollback target")
            }
            LineageError::NoFallback => write!(f, "no earlier champion exists to fall back to"),
        }
    }
}

impl std::error::Error for LineageError {}

/// The generation log plus the current champion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Lineage {
    generations: Vec<Generation>,
    events: Vec<SuccessionEvent>,
    champion: Option<GenerationId>,
    next_id: u64,
}

impl Lineage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next generation id. Never reuses, even after rollback.
    pub fn next_id(&mut self) -> GenerationId {
        let id = GenerationId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Record a candidate. Does not grant authority.
    pub fn add(&mut self, generation: Generation) -> GenerationId {
        let id = generation.id;
        self.next_id = self.next_id.max(id.0 + 1);
        self.generations.push(generation);
        id
    }

    pub fn get(&self, id: GenerationId) -> Option<&Generation> {
        self.generations.iter().find(|g| g.id == id)
    }

    fn get_mut(&mut self, id: GenerationId) -> Option<&mut Generation> {
        self.generations.iter_mut().find(|g| g.id == id)
    }

    pub fn champion(&self) -> Option<&Generation> {
        self.champion.and_then(|id| self.get(id))
    }

    pub fn events(&self) -> &[SuccessionEvent] {
        &self.events
    }

    pub fn generations(&self) -> &[Generation] {
        &self.generations
    }

    /// Ancestors of `id`, nearest first, excluding `id`.
    pub fn ancestors(&self, id: GenerationId) -> Vec<&Generation> {
        let mut out = Vec::new();
        let mut cur = self.get(id).and_then(|g| g.parent);
        while let Some(p) = cur {
            match self.get(p) {
                Some(g) => {
                    out.push(g);
                    cur = g.parent;
                }
                None => break,
            }
        }
        out
    }

    /// The best held-out value any ancestor of `id` (or `id` itself) achieved on
    /// `axis`. See the module note on why the gate ratchets against this.
    pub fn best_ancestor(&self, id: GenerationId, axis: &str) -> Option<(GenerationId, f64)> {
        let mut chain: Vec<&Generation> = self.get(id).into_iter().collect();
        chain.extend(self.ancestors(id));
        chain
            .iter()
            .filter_map(|g| g.heldout_fitness().and_then(|f| f.get(axis)).map(|v| (g.id, v)))
            .fold(None, |acc: Option<(GenerationId, f64)>, (gid, v)| match acc {
                Some((_, best)) if best >= v => acc,
                _ => Some((gid, v)),
            })
    }

    /// Grant authority. The caller must already hold an approving verdict —
    /// this records the transfer, it does not decide it.
    pub fn promote(
        &mut self,
        id: GenerationId,
        gate: Digest,
        evaluator: &str,
    ) -> Result<(), LineageError> {
        if self.get(id).is_none() {
            return Err(LineageError::UnknownGeneration(id));
        }
        let from = self.champion;
        if let Some(prev) = from {
            if let Some(g) = self.get_mut(prev) {
                g.status = Status::Retired;
            }
        }
        if let Some(g) = self.get_mut(id) {
            g.status = Status::Champion;
            g.promoted_under = Some(gate.clone());
        }
        self.champion = Some(id);
        self.events.push(SuccessionEvent::Promoted {
            generation: id,
            from,
            gate,
            evaluator: evaluator.to_string(),
        });
        Ok(())
    }

    pub fn refuse(&mut self, id: GenerationId, reasons: Vec<String>) {
        self.events.push(SuccessionEvent::Refused { generation: id, reasons });
    }

    /// The generation authority would return to if the champion failed now.
    ///
    /// The most recent retired generation whose artifact is materialized.
    /// Quarantined generations are skipped: falling back to something that
    /// already failed is not a recovery.
    pub fn fallback_target(
        &self,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Result<GenerationId, LineageError> {
        self.generations
            .iter()
            .rev()
            .find(|g| g.status == Status::Retired && materialized(&g.artifact))
            .map(|g| g.id)
            .ok_or(LineageError::NoFallback)
    }

    /// Demote the champion and hand authority back to a fallback.
    ///
    /// Verifies the target is materialized *before* moving authority. Discovering
    /// an unusable fallback mid-incident is the worst possible time, so the check
    /// is here rather than in the caller.
    pub fn demote_champion(
        &mut self,
        reason: &str,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Result<GenerationId, LineageError> {
        let Some(current) = self.champion else {
            return Err(LineageError::NoFallback);
        };
        let target = self.fallback_target(materialized)?;
        let target_artifact = self.get(target).map(|g| g.artifact.clone()).unwrap();
        if !materialized(&target_artifact) {
            return Err(LineageError::NotMaterialized {
                generation: target,
                artifact: target_artifact,
            });
        }

        if let Some(g) = self.get_mut(current) {
            g.status = Status::Quarantined;
            g.note = reason.to_string();
        }
        if let Some(g) = self.get_mut(target) {
            g.status = Status::Champion;
        }
        self.champion = Some(target);
        self.events.push(SuccessionEvent::Demoted {
            generation: current,
            to: target,
            reason: reason.to_string(),
        });
        Ok(target)
    }

    /// Deliberate return to a specific earlier generation.
    pub fn roll_back_to(
        &mut self,
        target: GenerationId,
        reason: &str,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Result<(), LineageError> {
        let g = self.get(target).ok_or(LineageError::UnknownGeneration(target))?;
        if g.status == Status::Quarantined {
            return Err(LineageError::Quarantined(target));
        }
        if !materialized(&g.artifact) {
            return Err(LineageError::NotMaterialized {
                generation: target,
                artifact: g.artifact.clone(),
            });
        }
        let from = self.champion.ok_or(LineageError::NoFallback)?;
        if let Some(g) = self.get_mut(from) {
            g.status = Status::Retired;
        }
        if let Some(g) = self.get_mut(target) {
            g.status = Status::Champion;
        }
        self.champion = Some(target);
        self.events.push(SuccessionEvent::RolledBack {
            from,
            to: target,
            reason: reason.to_string(),
        });
        Ok(())
    }

    /// Has this generation been demoted before, and with what held-out
    /// composite? Used to block thrash — see [`super::gate`].
    pub fn previously_demoted(&self, id: GenerationId) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, SuccessionEvent::Demoted { generation, .. } if *generation == id))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::germline::{EvalSuite, FitnessVector, Measurement, SuiteKind};

    fn always_materialized() -> impl Fn(&Digest) -> bool {
        |_: &Digest| true
    }

    fn gen(l: &mut Lineage, parent: Option<GenerationId>, safety: f64) -> GenerationId {
        let id = l.next_id();
        let mut g = Generation::new(id, Digest::of(format!("model{}", id.0).as_bytes()));
        if let Some(p) = parent {
            g = g.parent(p);
        }
        g = g.measured(Measurement {
            suite: EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(b"suite")),
            fitness: FitnessVector::new().with("safety", safety),
            evaluator: "harness".into(),
        });
        l.add(g)
    }

    #[test]
    fn promotion_retires_the_previous_champion() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.92);
        l.promote(a, Digest::of(b"gate"), "harness").unwrap();
        l.promote(b, Digest::of(b"gate"), "harness").unwrap();
        assert_eq!(l.champion().unwrap().id, b);
        assert_eq!(l.get(a).unwrap().status, Status::Retired);
    }

    #[test]
    fn generation_ids_are_never_reused() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.8);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();
        l.demote_champion("bad", &always_materialized()).unwrap();
        let c = gen(&mut l, Some(a), 0.95);
        assert_eq!(c.0, 2, "ids advance even across demotion — history stays legible");
    }

    #[test]
    fn demotion_returns_authority_to_the_last_good_champion() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();

        let target = l.demote_champion("fitness collapsed", &always_materialized()).unwrap();
        assert_eq!(target, a);
        assert_eq!(l.champion().unwrap().id, a);
        assert_eq!(l.get(b).unwrap().status, Status::Quarantined);
    }

    #[test]
    fn a_quarantined_generation_is_not_a_fallback() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        let c = gen(&mut l, Some(b), 0.96);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();
        l.promote(c, Digest::of(b"g"), "h").unwrap();

        // b failed earlier and was quarantined; falling back must skip it.
        l.demote_champion("c malfunctioned", &always_materialized()).unwrap();
        assert_eq!(l.champion().unwrap().id, b, "b is the most recent retired champion");

        l.demote_champion("b malfunctioned too", &always_materialized()).unwrap();
        assert_eq!(l.champion().unwrap().id, a, "must skip past the quarantined one");
    }

    #[test]
    fn an_unmaterialized_fallback_is_refused_rather_than_silently_used() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();

        let gone = |_: &Digest| false;
        assert_eq!(l.demote_champion("boom", &gone), Err(LineageError::NoFallback));
        assert_eq!(l.champion().unwrap().id, b, "authority must not move to a target that cannot run");
    }

    #[test]
    fn best_ancestor_finds_the_high_water_mark_not_the_incumbent() {
        // The drift scenario from the module docs.
        let mut l = Lineage::new();
        let g1 = gen(&mut l, None, 0.95);
        let g2 = gen(&mut l, Some(g1), 0.93);
        let g3 = gen(&mut l, Some(g2), 0.91);
        let g4 = gen(&mut l, Some(g3), 0.89);

        let (who, best) = l.best_ancestor(g4, "safety").unwrap();
        assert_eq!(who, g1);
        assert!((best - 0.95).abs() < 1e-12, "the ratchet must see gen1, not gen3");
    }

    #[test]
    fn rollback_to_a_quarantined_generation_is_refused() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();
        l.demote_champion("b broke", &always_materialized()).unwrap();

        assert_eq!(
            l.roll_back_to(b, "let's try again", &always_materialized()),
            Err(LineageError::Quarantined(b))
        );
    }

    #[test]
    fn every_transfer_is_recorded_with_its_authorizing_gate() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let gate = Digest::of(b"gate-v1");
        l.promote(a, gate.clone(), "harness").unwrap();
        match &l.events()[0] {
            SuccessionEvent::Promoted { generation, gate: g, evaluator, .. } => {
                assert_eq!(*generation, a);
                assert_eq!(g, &gate, "the audit anchor must be the gate that authorized it");
                assert_eq!(evaluator, "harness");
            }
            other => panic!("expected Promoted, got {other:?}"),
        }
    }

    #[test]
    fn demotion_history_is_queryable_for_thrash_prevention() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        l.promote(a, Digest::of(b"g"), "h").unwrap();
        l.promote(b, Digest::of(b"g"), "h").unwrap();
        l.demote_champion("failed", &always_materialized()).unwrap();
        assert!(l.previously_demoted(b));
        assert!(!l.previously_demoted(a));
    }
}
