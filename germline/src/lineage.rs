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
//!
//! ## Optionally, a population with niches
//!
//! [`Lineage::with_archive`] gives the lineage a [`NicheArchive`] (see
//! [`crate::archive`]). Without one, nothing in this file behaves differently.
//! With one, the **champion** is still a single generation, defined by a rule
//! over the archive's elites, so fallback, demotion and everything else that
//! needs one champion still has one:
//!
//! - It is always an elite.
//! - After each admission, an elite takes the championship from the sitting
//!   champion when its capability is higher by at least the spec's
//!   `min_improvement`; the sitting champion is kept otherwise, so a hair's
//!   breadth does not churn it. When the sitting champion has just been
//!   replaced in its cell, the best elite takes over. "Best" is the highest
//!   capability, the earliest generation breaking a tie. The gate's
//!   [`DominanceRule`](super::gate::DominanceRule) is not used for this.
//! - Every change of champion is a `Promoted` event, so it replays.
//!
//! **Demotion** of an elite empties its cell. The elite it had displaced is not
//! restored: it was replaced because a later candidate beat it on measured
//! evidence, and the lineage keeps it in the record where the gate can re-admit
//! it. Demoting the champion falls back to the best remaining elite whose
//! artifact is materialized; the lineage is never left without a champion while
//! one remains.
//!
//! ## A lineage is rebuilt, never loaded
//!
//! A lineage names the champion, so a lineage read from a file would be a way
//! to grant authority without the gate. `Lineage` is therefore not
//! `Deserialize`:
//!
//! ```compile_fail
//! let forged: germline::lineage::Lineage = serde_json::from_str("{}").unwrap();
//! ```
//!
//! A restarted process gets its lineage from [`Lineage::replay`] (plan 8.3),
//! which verifies the journal's hash chain and then re-applies every
//! succession event through the same transitions the live methods use,
//! refusing any the lineage at that point would not have allowed.

use super::archive::{Admission, CellKey, NicheArchive, NicheSpec};
use super::gate::Approval;
use super::journal::{Entry, Journal, JournalError};
use super::manifest::ProposerRecord;
use super::{EvalSuite, FitnessVector, Generation, GenerationId, Measurement, Status};
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    /// Under a niche archive: a generation was admitted to a cell, displacing
    /// the elite `replaced` if there was one. Whether the champion changed is a
    /// separate `Promoted` event that follows it.
    Admitted { generation: GenerationId, cell: CellKey, replaced: Option<GenerationId>, gate: Digest, evaluator: String },
    /// Under a niche archive: an elite that was not the champion was demoted,
    /// and its cell emptied.
    Evicted { generation: GenerationId, cell: CellKey, reason: String },
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
    /// The approval was earned against a champion that no longer holds
    /// authority, so it says nothing about the challenger against this one.
    StaleApproval { judged_against: Option<GenerationId>, champion: Option<GenerationId> },
    /// A niche-archive transition that the archive does not allow.
    Archive(String),
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
            LineageError::Archive(why) => write!(f, "niche archive: {why}"),
            LineageError::StaleApproval { judged_against, champion } => write!(
                f,
                "the approval was earned against {} and the champion is now {}",
                judged_against.map(|g| g.to_string()).unwrap_or_else(|| "no champion".into()),
                champion.map(|g| g.to_string()).unwrap_or_else(|| "no champion".into())
            ),
        }
    }
}

impl std::error::Error for LineageError {}

/// Why a journal could not be replayed into a lineage.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    /// The hash chain does not verify.
    Journal(JournalError),
    /// The chain verifies but ends somewhere other than the head published
    /// for it: the journal was truncated, or rewritten from some point on.
    Anchor { expected: Digest, found: Option<Digest> },
    /// A record asks for a transition the lineage did not allow at that point.
    Inconsistent { index: u64, detail: String },
    /// A record was written before the journal carried what replay needs.
    Predates { index: u64, missing: &'static str },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Journal(e) => write!(f, "{e}"),
            ReplayError::Anchor { expected, found } => write!(
                f,
                "the journal ends at {} and its published head is {}",
                found.as_ref().map(|d| d.short()).unwrap_or_else(|| "nothing".into()),
                expected.short()
            ),
            ReplayError::Inconsistent { index, detail } => write!(f, "journal record {index}: {detail}"),
            ReplayError::Predates { index, missing } => {
                write!(f, "journal record {index} predates replay: it does not record {missing}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// A lineage rebuilt from a journal, and where the run that wrote it stopped.
#[derive(Debug, Clone)]
pub struct Resumed {
    pub lineage: Lineage,
    /// Candidates proposed. A resumed runner continues from this cycle, so a
    /// cycle cut off part-way is counted as spent rather than run twice.
    pub proposals: u32,
    /// Refusals since the last promotion, so a stall that spans a restart is
    /// still a stall.
    pub trailing_refusals: u32,
    /// Joules the journal books to the energy budget: the sum of its
    /// `EnergySpent` entries. Zero for a journal written without a budget, so
    /// spend from invocations that had none is not counted.
    pub energy_spent: f64,
    /// Each external proposer's standing, rebuilt from the journal's
    /// `Calibrated` entries in order. A proposer with no scored prediction has
    /// no entry here, which is zero trust.
    pub proposers: BTreeMap<String, ProposerRecord>,
    /// The verified head the lineage was rebuilt from.
    pub head: Option<Digest>,
}

/// The generation log plus the current champion.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Lineage {
    generations: Vec<Generation>,
    events: Vec<SuccessionEvent>,
    champion: Option<GenerationId>,
    next_id: u64,
    /// The niche archive, when the lineage keeps one. Left out of the
    /// serialization when absent, so a lineage without one serializes as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    archive: Option<NicheArchive>,
}

impl Lineage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep a niche archive (see [`crate::archive`]). Refused for an invalid
    /// spec, and for a lineage that already has a champion: its champion would
    /// not be an elite.
    pub fn with_archive(mut self, spec: NicheSpec) -> Result<Self, String> {
        if self.champion.is_some() {
            return Err("an archive must be opened before the lineage has a champion".into());
        }
        self.archive = Some(NicheArchive::new(spec)?);
        Ok(self)
    }

    /// The niche archive, if this lineage keeps one.
    pub fn archive(&self) -> Option<&NicheArchive> {
        self.archive.as_ref()
    }

    /// The elite of `cell`'s capability, finite or nothing.
    fn elite_capability(&self, id: GenerationId) -> Option<f64> {
        let axis = &self.archive.as_ref()?.spec().capability_axis;
        self.get(id)?.heldout_fitness()?.get(axis).filter(|v| v.is_finite())
    }

    /// The best elite by capability, the earliest generation breaking a tie,
    /// among those other than `exclude` that `usable` accepts.
    fn best_elite(&self, exclude: Option<GenerationId>, usable: &dyn Fn(&Generation) -> bool) -> Option<GenerationId> {
        let mut best: Option<(GenerationId, f64)> = None;
        for (_, id) in self.archive.as_ref()?.elites() {
            if Some(id) == exclude || !self.get(id).is_some_and(usable) {
                continue;
            }
            let Some(cap) = self.elite_capability(id) else { continue };
            let better = match best {
                None => true,
                Some((b, bc)) => cap > bc || (cap == bc && id < b),
            };
            if better {
                best = Some((id, cap));
            }
        }
        best.map(|(id, _)| id)
    }

    /// The champion the archive's rule gives now: the sitting champion unless it
    /// has left the archive or an elite beats it by the margin. See the module
    /// note.
    fn champion_after(&self) -> Option<GenerationId> {
        let archive = self.archive.as_ref()?;
        let best = self.best_elite(None, &|_| true)?;
        let Some(cur) = self.champion.filter(|c| archive.contains(*c)) else { return Some(best) };
        if best == cur {
            return Some(cur);
        }
        match (self.elite_capability(best), self.elite_capability(cur)) {
            (Some(b), Some(c)) if b > c && b - c >= archive.spec().min_improvement => Some(best),
            (Some(_), None) => Some(best),
            _ => Some(cur),
        }
    }

    /// Hand the championship to whoever the archive's rule names, if that is not
    /// the sitting champion.
    fn refresh_champion(&mut self, gate: Digest, evaluator: String) {
        if let Some(n) = self.champion_after() {
            if Some(n) != self.champion {
                self.install(n, gate, evaluator);
            }
        }
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

    /// Clade productivity: the mean held-out composite fitness of `id` and
    /// every generation descended from it — the Huxley-Gödel Machine's
    /// measure of how good a lineage is at producing good descendants, as
    /// opposed to how good its founder happened to be. `None` if nothing in
    /// the clade was measured.
    pub fn clade_score(&self, id: GenerationId) -> Option<f64> {
        let mut members = vec![id];
        let mut i = 0;
        while i < members.len() {
            let here = members[i];
            members.extend(self.generations.iter().filter(|g| g.parent == Some(here)).map(|g| g.id));
            i += 1;
        }
        let scores: Vec<f64> = members
            .iter()
            .filter_map(|m| self.get(*m).and_then(|g| g.heldout_fitness()).map(|f| f.composite()))
            .collect();
        if scores.is_empty() {
            None
        } else {
            Some(scores.iter().sum::<f64>() / scores.len() as f64)
        }
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

    /// Grant authority on the gate's [`Approval`], which only
    /// [`Episode::approve`](super::gate::Episode::approve) mints. This records
    /// the transfer; the gate decided it. The approval is consumed.
    pub fn promote(&mut self, approval: Approval) -> Result<(), LineageError> {
        let id = approval.generation();
        if self.get(id).is_none() {
            return Err(LineageError::UnknownGeneration(id));
        }
        if approval.incumbent() != self.champion {
            return Err(LineageError::StaleApproval {
                judged_against: approval.incumbent(),
                champion: self.champion,
            });
        }
        let gate = approval.gate().clone();
        match (&self.archive, approval.admission()) {
            (None, None) => self.install(id, gate, approval.evaluator().to_string()),
            (Some(_), Some(adm)) => self.admit(id, adm.clone(), gate, approval.evaluator().to_string())?,
            (None, Some(_)) => return Err(LineageError::Archive("the approval admits to an archive and the lineage keeps none".into())),
            (Some(_), None) => return Err(LineageError::Archive("the lineage keeps an archive and the approval carries no cell".into())),
        }
        Ok(())
    }

    /// The admission transition: seat `id` in its cell, then let the archive's
    /// champion rule decide whether the championship moves.
    ///
    /// Re-derives the cell from the generation's own held-out measurement and
    /// checks the occupant is the one the gate saw, so an approval cannot seat a
    /// generation in a cell its measurement does not give, or over an elite the
    /// gate did not judge it against.
    fn admit(&mut self, id: GenerationId, adm: Admission, gate: Digest, evaluator: String) -> Result<(), LineageError> {
        let Some(archive) = self.archive.as_ref() else {
            return Err(LineageError::Archive("the lineage keeps no archive".into()));
        };
        if archive.occupant(&adm.cell) != adm.replaces {
            return Err(LineageError::Archive(format!(
                "cell {:?} holds {:?} and the approval was earned against {:?}",
                adm.cell,
                archive.occupant(&adm.cell),
                adm.replaces
            )));
        }
        let derived = self.get(id).and_then(|g| g.heldout_fitness()).map(|f| archive.spec().cell_of(f));
        if derived != Some(Ok(adm.cell.clone())) {
            return Err(LineageError::Archive(format!("{id}'s measurement does not place it in cell {:?}", adm.cell)));
        }
        if archive.contains(id) {
            return Err(LineageError::Archive(format!("{id} is already an elite")));
        }
        self.place_elite(id, adm.cell, adm.replaces, gate.clone(), evaluator.clone());
        self.refresh_champion(gate, evaluator);
        Ok(())
    }

    /// Seat `id` in `cell` and record it. Shared by [`Lineage::admit`] and replay.
    fn place_elite(&mut self, id: GenerationId, cell: CellKey, replaced: Option<GenerationId>, gate: Digest, evaluator: String) {
        if let Some(a) = self.archive.as_mut() {
            a.place(cell.clone(), id);
        }
        self.events.push(SuccessionEvent::Admitted { generation: id, cell, replaced, gate, evaluator });
    }

    /// The promotion transition, shared by [`Lineage::promote`] and replay.
    fn install(&mut self, id: GenerationId, gate: Digest, evaluator: String) {
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
        self.events.push(SuccessionEvent::Promoted { generation: id, from, gate, evaluator });
    }

    /// Promote without the gate, for this crate's unit tests only.
    #[cfg(test)]
    pub(crate) fn grant(&mut self, id: GenerationId, gate: Digest, evaluator: &str) -> Result<(), LineageError> {
        let incumbent = self.champion;
        self.promote(Approval::unchecked(id, gate, evaluator, incumbent))
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
        // Under an archive the fallback is the best other elite that is
        // materialized: the generations it holds are the ones with standing.
        if self.archive.is_some() {
            return self
                .best_elite(self.champion, &|g| materialized(&g.artifact))
                .ok_or(LineageError::NoFallback);
        }
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

        self.apply_demotion(current, target, reason);
        Ok(target)
    }

    /// The demotion transition, shared by [`Lineage::demote_champion`] and
    /// replay.
    fn apply_demotion(&mut self, current: GenerationId, target: GenerationId, reason: &str) {
        if let Some(a) = self.archive.as_mut() {
            // The demoted elite's cell is emptied, not refilled with whoever it
            // displaced; see the module note.
            a.remove(current);
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
    }

    /// Demote an elite of the archive and empty its cell. The champion falls back
    /// ([`Lineage::demote_champion`]) and `Ok(Some(new champion))` is returned;
    /// any other elite is evicted, the champion is untouched, and the result is
    /// `Ok(None)`. Needs an archive.
    pub fn demote_elite(
        &mut self,
        id: GenerationId,
        reason: &str,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Result<Option<GenerationId>, LineageError> {
        let Some(archive) = &self.archive else {
            return Err(LineageError::Archive("the lineage keeps no archive".into()));
        };
        if !archive.contains(id) {
            return Err(LineageError::Archive(format!("{id} is not an elite")));
        }
        if self.champion == Some(id) {
            return self.demote_champion(reason, materialized).map(Some);
        }
        self.apply_eviction(id, reason);
        Ok(None)
    }

    /// The eviction transition, shared by [`Lineage::demote_elite`] and replay.
    fn apply_eviction(&mut self, id: GenerationId, reason: &str) {
        let cell = self.archive.as_mut().and_then(|a| a.remove(id)).unwrap_or_default();
        if let Some(g) = self.get_mut(id) {
            g.status = Status::Quarantined;
            g.note = reason.to_string();
        }
        self.events.push(SuccessionEvent::Evicted { generation: id, cell, reason: reason.to_string() });
    }

    /// Deliberate return to a specific earlier generation. Under an archive the
    /// target must be an elite, and the move is an operator's override of the
    /// champion rule: the rule applies again at the next admission.
    pub fn roll_back_to(
        &mut self,
        target: GenerationId,
        reason: &str,
        materialized: &dyn Fn(&Digest) -> bool,
    ) -> Result<(), LineageError> {
        if self.archive.as_ref().is_some_and(|a| !a.contains(target)) {
            return Err(LineageError::Archive(format!("{target} is not an elite and cannot be rolled back to")));
        }
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
        self.apply_rollback(from, target, reason);
        Ok(())
    }

    /// The rollback transition, shared by [`Lineage::roll_back_to`] and replay.
    fn apply_rollback(&mut self, from: GenerationId, target: GenerationId, reason: &str) {
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

    /// Rebuild a lineage from `journal`, starting from `genesis`.
    ///
    /// `genesis` is what the caller set up in code before the journal began,
    /// such as a seed champion it approved itself; pass [`Lineage::new`] when
    /// the journal records that too. `anchor` is the journal head published
    /// somewhere the writer does not control, when there is one; it is what
    /// catches truncation, which the chain alone cannot (see
    /// [`super::journal`]).
    ///
    /// The chain is verified first, then every succession event is re-applied
    /// with the checks the live transition made: a promotion must name the
    /// champion it displaced and a generation that was proposed and measured
    /// on a held-out suite, a demotion must fall back to a retired generation,
    /// and a rollback may not return to a quarantined one. Materialization
    /// can't be checked here; it belongs to the workload.
    ///
    /// Proposer records are rebuilt the same way: each `Calibrated` entry
    /// scores its proposer, and the trust it carries must match the rebuilt
    /// record. A cycle that proposed nothing (`CycleRefused`) counts as a
    /// proposal and as a refusal, as the live run counted it.
    pub fn replay(genesis: Lineage, journal: &Journal, anchor: Option<&Digest>) -> Result<Resumed, ReplayError> {
        let records = journal.verified().map_err(ReplayError::Journal)?;
        let head = records.last().map(|r| r.link.clone());
        if let Some(expected) = anchor {
            if head.as_ref() != Some(expected) {
                return Err(ReplayError::Anchor { expected: expected.clone(), found: head });
            }
        }
        let mut l = genesis;
        let mut pending: BTreeMap<GenerationId, Generation> = BTreeMap::new();
        let (mut proposals, mut trailing_refusals) = (0u32, 0u32);
        let mut energy_spent = 0.0f64;
        let mut proposers: BTreeMap<String, ProposerRecord> = BTreeMap::new();
        // Under an archive, an admission that moves the championship is followed
        // by the `Promoted` that records it. This is that promotion, owed.
        let mut owed: Option<(GenerationId, Digest, String)> = None;
        for r in &records {
            let bad = |detail: String| ReplayError::Inconsistent { index: r.index, detail };
            if let Some((n, _, _)) = &owed {
                let paid = matches!(
                    &r.entry,
                    Entry::Succession { event: SuccessionEvent::Promoted { generation, .. } } if generation == n
                );
                if !paid {
                    return Err(bad(format!("{n} should have taken the championship after the admission, and the journal goes on without it")));
                }
            }
            match &r.entry {
                Entry::Proposed { generation, artifact, definitions, knowledge_base, parent, genome, note, mate, .. } => {
                    if l.get(*generation).is_some() || pending.contains_key(generation) {
                        return Err(bad(format!("{generation} is proposed twice")));
                    }
                    let mut g = Generation::new(*generation, artifact.clone()).note(note.clone());
                    g.parent = *parent;
                    g.mate = *mate;
                    g.genome = genome.clone();
                    g.definitions = definitions.clone();
                    g.knowledge_base = knowledge_base.clone();
                    l.next_id = l.next_id.max(generation.0 + 1);
                    pending.insert(*generation, g);
                    proposals += 1;
                }
                Entry::Evaluated { generation, fitness, suite, evaluator, suite_id, suite_kind: kind } => {
                    let (Some(id), Some(kind)) = (suite_id, kind) else {
                        return Err(ReplayError::Predates { index: r.index, missing: "the evaluated suite's name and kind" });
                    };
                    let g = pending
                        .get_mut(generation)
                        .ok_or_else(|| bad(format!("{generation} is evaluated without being proposed")))?;
                    let fitness: FitnessVector =
                        serde_json::from_str(fitness).map_err(|e| bad(format!("{generation}'s fitness: {e}")))?;
                    g.measured = Some(Measurement {
                        suite: EvalSuite::new(id.clone(), *kind, suite.clone()),
                        fitness,
                        evaluator: evaluator.clone(),
                    });
                }
                Entry::Succession { event } => match event {
                    SuccessionEvent::Promoted { generation, from, gate, evaluator } => {
                        if *from != l.champion {
                            return Err(bad(format!(
                                "{generation} is promoted over {from:?}, and the champion was {:?}",
                                l.champion
                            )));
                        }
                        if l.archive.is_some() {
                            // The generation was added when it was admitted.
                            if l.champion_after() != Some(*generation) {
                                return Err(bad(format!(
                                    "{generation} is promoted, and the archive's champion rule names {:?}",
                                    l.champion_after()
                                )));
                            }
                            owed = None;
                            l.install(*generation, gate.clone(), evaluator.clone());
                        } else {
                            let g = pending
                                .remove(generation)
                                .ok_or_else(|| bad(format!("{generation} is promoted without being proposed")))?;
                            if g.heldout_fitness().is_none() {
                                return Err(bad(format!("{generation} is promoted with no held-out measurement")));
                            }
                            l.add(g);
                            l.install(*generation, gate.clone(), evaluator.clone());
                        }
                        trailing_refusals = 0;
                    }
                    SuccessionEvent::Admitted { generation, cell, replaced, gate, evaluator } => {
                        let Some(archive) = l.archive.as_ref() else {
                            return Err(bad(format!("{generation} is admitted to an archive the lineage does not keep")));
                        };
                        let g = pending
                            .remove(generation)
                            .ok_or_else(|| bad(format!("{generation} is admitted without being proposed")))?;
                        let Some(f) = g.heldout_fitness() else {
                            return Err(bad(format!("{generation} is admitted with no held-out measurement")));
                        };
                        if archive.spec().cell_of(f) != Ok(cell.clone()) {
                            return Err(bad(format!("{generation}'s measurement does not place it in cell {cell:?}")));
                        }
                        if archive.occupant(cell) != *replaced {
                            return Err(bad(format!(
                                "{generation} is admitted over {replaced:?} in cell {cell:?}, which held {:?}",
                                archive.occupant(cell)
                            )));
                        }
                        l.add(g);
                        l.place_elite(*generation, cell.clone(), *replaced, gate.clone(), evaluator.clone());
                        owed = l
                            .champion_after()
                            .filter(|n| Some(*n) != l.champion)
                            .map(|n| (n, gate.clone(), evaluator.clone()));
                        trailing_refusals = 0;
                    }
                    SuccessionEvent::Evicted { generation, cell, reason } => {
                        let held = l.archive.as_ref().and_then(|a| a.cell_of_generation(*generation)).cloned();
                        if held.as_ref() != Some(cell) {
                            return Err(bad(format!("{generation} is evicted from {cell:?}, and holds {held:?}")));
                        }
                        if l.champion == Some(*generation) {
                            return Err(bad(format!("{generation} is evicted and is the champion; that is a demotion")));
                        }
                        l.apply_eviction(*generation, reason);
                    }
                    SuccessionEvent::Refused { generation, reasons } => {
                        pending.remove(generation);
                        l.refuse(*generation, reasons.clone());
                        trailing_refusals += 1;
                    }
                    SuccessionEvent::Demoted { generation, to, reason } => {
                        if l.champion != Some(*generation) {
                            return Err(bad(format!("{generation} is demoted and was not the champion")));
                        }
                        if let Some(a) = &l.archive {
                            if !a.contains(*to) || to == generation {
                                return Err(bad(format!("{generation} falls back to {to}, which is not another elite")));
                            }
                        } else if l.get(*to).map(|g| g.status) != Some(Status::Retired) {
                            return Err(bad(format!("{generation} falls back to {to}, which is not a retired generation")));
                        }
                        l.apply_demotion(*generation, *to, reason);
                    }
                    SuccessionEvent::RolledBack { from, to, reason } => {
                        if l.champion != Some(*from) {
                            return Err(bad(format!("a rollback from {from}, which was not the champion")));
                        }
                        if l.archive.as_ref().is_some_and(|a| !a.contains(*to)) {
                            return Err(bad(format!("a rollback to {to}, which is not an elite")));
                        }
                        match l.get(*to).map(|g| g.status) {
                            None => return Err(bad(format!("a rollback to unknown {to}"))),
                            Some(Status::Quarantined) => {
                                return Err(bad(format!("a rollback to {to}, which is quarantined")))
                            }
                            Some(_) => {}
                        }
                        l.apply_rollback(*from, *to, reason);
                    }
                },
                // Calibration describes a proposer's record, not the lineage.
                // It is rebuilt into `proposers`, and the trust the live run
                // journalled must be the trust that rebuild arrives at.
                Entry::Calibrated { proposer, score, trust, .. } => {
                    let record = proposers.entry(proposer.clone()).or_default();
                    record.observe(*score);
                    if record.trust() != *trust {
                        return Err(bad(format!(
                            "{proposer}'s recorded trust is {trust}, and its scores give {}",
                            record.trust()
                        )));
                    }
                }
                // A cycle that proposed nothing: a proposal, and a refusal, as
                // the live run counted it.
                Entry::CycleRefused { .. } => {
                    proposals += 1;
                    trailing_refusals += 1;
                }
                Entry::MeasuredNotSubmitted { generation, submitted, .. } => {
                    let g = pending
                        .get(generation)
                        .ok_or_else(|| bad(format!("{generation} is set aside without being proposed")))?;
                    if g.measured.is_none() {
                        return Err(bad(format!("{generation} is set aside without being measured")));
                    }
                    if !pending.contains_key(submitted) && l.get(*submitted).is_none() {
                        return Err(bad(format!("{generation} is set aside for unknown {submitted}")));
                    }
                    pending.remove(generation);
                    proposals = proposals.saturating_sub(1);
                }
                Entry::EnergySpent { joules, .. } => energy_spent += joules,
                // The niches the run pinned. The archive replay rebuilds must be
                // that one; one opened here must open on an empty lineage.
                Entry::ArchiveOpened { spec, digest } => {
                    if spec.digest() != *digest {
                        return Err(bad("the archive's spec does not match its recorded digest".into()));
                    }
                    match &l.archive {
                        Some(a) if a.spec() != spec => {
                            return Err(bad("the journal's archive differs from the one the lineage was started with".into()))
                        }
                        Some(_) => {}
                        None => {
                            if l.champion.is_some() {
                                return Err(bad("an archive is opened after the lineage has a champion".into()));
                            }
                            l.archive = Some(NicheArchive::new(spec.clone()).map_err(|e| bad(e))?);
                        }
                    }
                }
                Entry::Adjudicated { .. } | Entry::Failure { .. } | Entry::Note { .. } => {}
            }
            // An owed promotion is cleared only by its own entry, above.
            if let Some((n, _, _)) = &owed {
                if l.champion == Some(*n) {
                    owed = None;
                }
            }
        }
        // A journal that ends between an admission and the promotion it owes (a
        // crash between the two appends) still rebuilds the lineage the live run
        // had: the archive's rule, applied.
        if let Some((n, gate, evaluator)) = owed {
            l.install(n, gate, evaluator);
        }
        Ok(Resumed { lineage: l, proposals, trailing_refusals, energy_spent, proposers, head })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvalSuite, FitnessVector, Measurement, SuiteKind};

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
    fn a_productive_clade_outranks_a_better_leaf() {
        // A scores 0.5 but its child scores 0.9; C scores 0.6 and has no
        // children. By its own score C is better; by its clade A is (0.7) —
        // which is the Huxley-Gödel finding the runner now selects on.
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.5);
        gen(&mut l, Some(a), 0.9);
        let c = gen(&mut l, None, 0.6);
        let (ca, cc) = (l.clade_score(a).unwrap(), l.clade_score(c).unwrap());
        assert!((ca - 0.7).abs() < 1e-12, "{ca}");
        assert!((cc - 0.6).abs() < 1e-12, "{cc}");
        assert!(ca > cc);
        // Grandchildren count too.
        let b2 = l.generations().iter().find(|g| g.parent == Some(a)).unwrap().id;
        gen(&mut l, Some(b2), 0.2);
        assert!((l.clade_score(a).unwrap() - (0.5 + 0.9 + 0.2) / 3.0).abs() < 1e-12);
    }

    #[test]
    fn promotion_retires_the_previous_champion() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.92);
        l.grant(a, Digest::of(b"gate"), "harness").unwrap();
        l.grant(b, Digest::of(b"gate"), "harness").unwrap();
        assert_eq!(l.champion().unwrap().id, b);
        assert_eq!(l.get(a).unwrap().status, Status::Retired);
    }

    #[test]
    fn generation_ids_are_never_reused() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.8);
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();
        l.demote_champion("bad", &always_materialized()).unwrap();
        let c = gen(&mut l, Some(a), 0.95);
        assert_eq!(c.0, 2, "ids advance even across demotion — history stays legible");
    }

    #[test]
    fn demotion_returns_authority_to_the_last_good_champion() {
        let mut l = Lineage::new();
        let a = gen(&mut l, None, 0.9);
        let b = gen(&mut l, Some(a), 0.95);
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();

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
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();
        l.grant(c, Digest::of(b"g"), "h").unwrap();

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
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();

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
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();
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
        l.grant(a, gate.clone(), "harness").unwrap();
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
        l.grant(a, Digest::of(b"g"), "h").unwrap();
        l.grant(b, Digest::of(b"g"), "h").unwrap();
        l.demote_champion("failed", &always_materialized()).unwrap();
        assert!(l.previously_demoted(b));
        assert!(!l.previously_demoted(a));
    }
}
