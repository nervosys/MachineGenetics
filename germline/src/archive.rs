//! A quality-diversity archive: a population with niches, opt-in.
//!
//! A lineage with one champion is a (1+λ) hill climber. A candidate replaces
//! the champion or is discarded, so nothing stands beside the champion for
//! crossover to recombine, and the search stalls at the first local optimum.
//! The objective here is also multi-objective (capability against energy
//! against size), so different trade-offs ought to be able to coexist.
//!
//! This module holds the pinned description of the niches ([`NicheSpec`]) and
//! the cells themselves ([`NicheArchive`]). The decisions live where they
//! always did: [`Episode`](crate::gate::Episode) adjudicates admission,
//! [`Lineage`](crate::lineage::Lineage) applies it, and the journal records it.
//!
//! ## Cells
//!
//! A cell is a vector of bucket indices, one per descriptor axis. Each
//! [`NicheAxis`] names an axis of the candidate's **held-out measurement** and
//! gives the bucket edges: a value `v` falls in bucket `k` where `k` is the
//! number of edges `<= v`. The descriptor is therefore computed from what the
//! evaluator measured and journalled, never from anything the candidate
//! reports about itself, and replay recomputes it. A workload that wants size
//! or energy per evaluation as niche axes reports them as fitness axes.
//!
//! A cell holds at most one elite: the best by the capability axis among the
//! candidates admitted to it.
//!
//! ## Pinned
//!
//! The axes, edges, capability axis, margin and cell cap are one
//! [`NicheSpec`], pinned by [`NicheSpec::digest`] in the episode
//! ([`Episode::with_archive`](crate::gate::Episode::with_archive)) the way the
//! gate's criteria are. Moving a bucket edge after seeing results would let the
//! search redraw the map around what it found, so a spec that differs from the
//! pinned one voids the episode.
//!
//! ## Memory is bounded by construction
//!
//! The grid is the product of the buckets of each axis. [`NicheSpec::validate`]
//! refuses a spec whose grid exceeds `max_cells`, so the archive can never hold
//! more elites than that.

use super::{FitnessVector, GenerationId};
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A cell: one bucket index per descriptor axis, in the spec's axis order.
pub type CellKey = Vec<u32>;

/// One descriptor axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NicheAxis {
    /// The held-out fitness axis the bucket is read from.
    pub axis: String,
    /// Ascending, finite, strictly increasing bucket edges. `n` edges give
    /// `n + 1` buckets.
    pub edges: Vec<f64>,
}

impl NicheAxis {
    pub fn new(axis: impl Into<String>, edges: Vec<f64>) -> Self {
        NicheAxis { axis: axis.into(), edges }
    }

    fn buckets(&self) -> usize {
        self.edges.len() + 1
    }

    fn bucket(&self, v: f64) -> u32 {
        self.edges.iter().take_while(|e| v >= **e).count() as u32
    }
}

/// The pre-registered description of the niches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NicheSpec {
    /// The axis that ranks elites within a cell and picks the champion. It
    /// must be the gate's primary axis ([`Episode::with_archive`](crate::gate::Episode::with_archive)
    /// checks), and is not a descriptor axis.
    pub capability_axis: String,
    pub axes: Vec<NicheAxis>,
    /// Upper bound on occupied cells. The grid must fit in it.
    pub max_cells: usize,
    /// How much better, on the capability axis, a candidate must be to replace
    /// the elite of its cell, and how much better an elite must be to take the
    /// champion's place from the sitting champion. Should exceed the noise
    /// floor, as the gate's margin does.
    pub min_improvement: f64,
}

impl NicheSpec {
    pub fn new(capability_axis: impl Into<String>, axes: Vec<NicheAxis>, max_cells: usize, min_improvement: f64) -> Self {
        NicheSpec { capability_axis: capability_axis.into(), axes, max_cells, min_improvement }
    }

    /// Number of cells in the grid. Saturates rather than overflowing.
    pub fn grid_size(&self) -> usize {
        self.axes.iter().fold(1usize, |n, a| n.saturating_mul(a.buckets()))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.axes.is_empty() {
            return Err("a niche spec needs at least one descriptor axis".into());
        }
        if self.max_cells == 0 {
            return Err("max_cells must be at least 1".into());
        }
        if !self.min_improvement.is_finite() || self.min_improvement < 0.0 {
            return Err("min_improvement must be finite and not negative".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for a in &self.axes {
            if a.axis == self.capability_axis {
                return Err(format!("`{}` is the capability axis and cannot also be a descriptor axis", a.axis));
            }
            if !seen.insert(a.axis.as_str()) {
                return Err(format!("descriptor axis `{}` is listed twice", a.axis));
            }
            if a.edges.is_empty() {
                return Err(format!("descriptor axis `{}` has no bucket edges", a.axis));
            }
            if a.edges.iter().any(|e| !e.is_finite()) || a.edges.windows(2).any(|w| w[0] >= w[1]) {
                return Err(format!("descriptor axis `{}` needs finite, strictly ascending edges", a.axis));
            }
        }
        if self.grid_size() > self.max_cells {
            return Err(format!(
                "the grid has {} cells and max_cells is {}: the archive would exceed its memory bound",
                self.grid_size(),
                self.max_cells
            ));
        }
        Ok(())
    }

    /// The pin. Any change to any field changes this.
    pub fn digest(&self) -> Digest {
        let body = serde_json::to_string(self).unwrap_or_default();
        Digest::of(format!("germline-niche-spec-v1:{body}").as_bytes())
    }

    /// The cell a measurement falls in. `Err` names the descriptor axis that is
    /// missing or not a number: a candidate with no position has no cell, and
    /// is refused rather than placed somewhere convenient.
    pub fn cell_of(&self, f: &FitnessVector) -> Result<CellKey, String> {
        self.axes
            .iter()
            .map(|a| match f.get(&a.axis) {
                Some(v) if !v.is_nan() => Ok(a.bucket(v)),
                _ => Err(a.axis.clone()),
            })
            .collect()
    }
}

/// What the gate decided about a candidate's place in the archive: carried by
/// the [`Approval`](crate::gate::Approval) that [`Lineage::promote`](crate::lineage::Lineage::promote)
/// spends.
#[derive(Debug, Clone, PartialEq)]
pub struct Admission {
    pub cell: CellKey,
    /// The elite the candidate displaces, if the cell was occupied.
    pub replaces: Option<GenerationId>,
}

impl Admission {
    /// For the verdict's basis.
    pub fn describe(&self) -> String {
        match self.replaces {
            None => format!("archive: admitted to the empty cell {:?}, and the guards held", self.cell),
            Some(e) => format!("archive: replaces {e} in cell {:?}, and the guards held", self.cell),
        }
    }
}

/// The cells. Built and changed only by [`Lineage`](crate::lineage::Lineage),
/// which is why this has no public mutators and no `Deserialize`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NicheArchive {
    spec: NicheSpec,
    #[serde(serialize_with = "cells_as_pairs")]
    cells: BTreeMap<CellKey, GenerationId>,
}

fn cells_as_pairs<S: serde::Serializer>(cells: &BTreeMap<CellKey, GenerationId>, s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(cells.iter())
}

impl NicheArchive {
    pub(crate) fn new(spec: NicheSpec) -> Result<Self, String> {
        spec.validate()?;
        Ok(NicheArchive { spec, cells: BTreeMap::new() })
    }

    pub fn spec(&self) -> &NicheSpec {
        &self.spec
    }

    pub fn occupant(&self, cell: &CellKey) -> Option<GenerationId> {
        self.cells.get(cell).copied()
    }

    /// The elites, in cell order.
    pub fn elites(&self) -> impl Iterator<Item = (&CellKey, GenerationId)> {
        self.cells.iter().map(|(c, g)| (c, *g))
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn contains(&self, id: GenerationId) -> bool {
        self.cells.values().any(|g| *g == id)
    }

    pub fn cell_of_generation(&self, id: GenerationId) -> Option<&CellKey> {
        self.cells.iter().find(|(_, g)| **g == id).map(|(c, _)| c)
    }

    /// Seat `id` in `cell`, returning whoever was there.
    pub(crate) fn place(&mut self, cell: CellKey, id: GenerationId) -> Option<GenerationId> {
        self.cells.insert(cell, id)
    }

    /// Empty the cell `id` holds. Returns the cell.
    pub(crate) fn remove(&mut self, id: GenerationId) -> Option<CellKey> {
        let cell = self.cell_of_generation(id)?.clone();
        self.cells.remove(&cell);
        Some(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> NicheSpec {
        NicheSpec::new(
            "capability",
            vec![NicheAxis::new("size", vec![0.33, 0.66]), NicheAxis::new("energy", vec![0.5])],
            6,
            0.02,
        )
    }

    #[test]
    fn a_value_falls_in_the_bucket_counting_the_edges_at_or_below_it() {
        let a = NicheAxis::new("x", vec![0.25, 0.5]);
        assert_eq!(a.bucket(0.0), 0);
        assert_eq!(a.bucket(0.24), 0);
        assert_eq!(a.bucket(0.25), 1, "an edge belongs to the bucket above it");
        assert_eq!(a.bucket(0.5), 2);
        assert_eq!(a.bucket(1.0), 2);
    }

    #[test]
    fn the_cell_is_read_from_the_measurement_and_refuses_what_is_not_measured() {
        let s = spec();
        let f = FitnessVector::new().with("capability", 0.9).with("size", 0.7).with("energy", 0.1);
        assert_eq!(s.cell_of(&f), Ok(vec![2, 0]));
        let missing = FitnessVector::new().with("capability", 0.9).with("size", 0.7);
        assert_eq!(s.cell_of(&missing), Err("energy".to_string()));
        let mut nan = f.clone();
        nan.0.insert("size".into(), f64::NAN);
        assert_eq!(s.cell_of(&nan), Err("size".to_string()));
    }

    #[test]
    fn a_spec_that_does_not_fit_its_bound_is_refused() {
        assert!(spec().validate().is_ok());
        let mut s = spec();
        s.max_cells = 5;
        assert!(s.validate().unwrap_err().contains("memory bound"));
        let mut s = spec();
        s.axes[0].edges = vec![0.5, 0.2];
        assert!(s.validate().unwrap_err().contains("ascending"));
        let mut s = spec();
        s.axes.push(NicheAxis::new("capability", vec![0.5]));
        assert!(s.validate().unwrap_err().contains("capability axis"));
        let mut s = spec();
        s.axes.clear();
        assert!(s.validate().is_err());
    }

    #[test]
    fn any_change_to_the_spec_changes_its_pin() {
        let base = spec().digest();
        let mut s = spec();
        s.axes[1].edges = vec![0.51];
        assert_ne!(s.digest(), base, "moving a bucket edge moves the pin");
        let mut s = spec();
        s.min_improvement = 0.03;
        assert_ne!(s.digest(), base);
        assert_eq!(spec().digest(), base);
    }

    #[test]
    fn placing_replaces_and_removing_empties() {
        let mut a = NicheArchive::new(spec()).unwrap();
        assert_eq!(a.place(vec![0, 0], GenerationId(1)), None);
        assert_eq!(a.place(vec![0, 0], GenerationId(2)), Some(GenerationId(1)));
        assert!(!a.contains(GenerationId(1)));
        assert_eq!(a.remove(GenerationId(2)), Some(vec![0, 0]));
        assert!(a.is_empty());
    }

    // ---- admission, through the gate and the lineage ----------------------

    use crate::gate::{Episode, PromotionGate, RejectReason, Verdict};
    use crate::lineage::{Lineage, LineageError, SuccessionEvent};
    use crate::{EvalSuite, Generation, Measurement, SuiteKind};

    const EV: &str = "independent-harness";

    fn yes(_: &Digest) -> bool {
        true
    }

    fn suite() -> EvalSuite {
        EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(b"niche-suite"))
    }

    fn gate() -> PromotionGate {
        PromotionGate {
            primary_axis: "capability".into(),
            min_improvement: 0.01,
            guard_axes: vec!["safety".into()],
            guard_tolerance: 0.01,
            min_shadow_successes: 0,
            dominance: None,
        }
    }

    fn episode() -> Episode {
        Episode::open(gate(), suite().digest, EV).with_archive(spec()).unwrap()
    }

    fn lineage() -> Lineage {
        Lineage::new().with_archive(spec()).unwrap()
    }

    /// A measured candidate. `size` and `energy` are the descriptor axes.
    fn cand(l: &mut Lineage, parent: Option<GenerationId>, cap: f64, size: f64, energy: f64, safety: f64) -> Generation {
        let id = l.next_id();
        let mut g = Generation::new(id, Digest::of(format!("model-{}", id.0).as_bytes())).measured(Measurement {
            suite: suite(),
            fitness: FitnessVector::new()
                .with("capability", cap)
                .with("size", size)
                .with("energy", energy)
                .with("safety", safety),
            evaluator: EV.into(),
        });
        g.parent = parent;
        g
    }

    /// Adjudicate and, on approval, admit. `Err` is the reasons it was refused.
    fn admit(ep: &Episode, l: &mut Lineage, g: Generation) -> Result<GenerationId, Vec<String>> {
        let id = g.id;
        match ep.approve(&g, l, 0, &yes) {
            Ok(a) => {
                l.add(g);
                l.promote(a).unwrap();
                Ok(id)
            }
            Err(v) => {
                l.refuse(id, v.reasons());
                Err(v.reasons())
            }
        }
    }

    fn rejected(ep: &Episode, l: &Lineage, g: &Generation) -> Vec<RejectReason> {
        match ep.adjudicate(g, l, 0, &yes) {
            Verdict::Reject { reasons, .. } => reasons,
            Verdict::Promote { .. } => Vec::new(),
        }
    }

    #[test]
    fn candidates_are_admitted_to_cells_replace_on_merit_and_are_refused_otherwise() {
        let (ep, mut l) = (episode(), lineage());
        let root = cand(&mut l, None, 0.50, 0.1, 0.1, 0.90);
        let root = admit(&ep, &mut l, root).unwrap();
        assert_eq!(l.champion().map(|g| g.id), Some(root), "the first admission is the champion");

        // A different cell, and lower capability: admitted, and the champion stays.
        let b = cand(&mut l, Some(root), 0.40, 0.5, 0.1, 0.90);
        let b = admit(&ep, &mut l, b).unwrap();
        assert_eq!(l.archive().unwrap().len(), 2);
        assert_eq!(l.champion().map(|g| g.id), Some(root));

        // The same cell as `b`, better by more than the margin: replaces it.
        let c = cand(&mut l, Some(b), 0.45, 0.5, 0.1, 0.90);
        let c = admit(&ep, &mut l, c).unwrap();
        assert_eq!(l.archive().unwrap().occupant(&vec![1, 0]), Some(c));
        assert!(!l.archive().unwrap().contains(b), "the displaced elite is out of the archive");
        assert!(l.get(b).is_some(), "and still in the lineage's record");
        assert!(matches!(
            l.events().iter().rev().find(|e| matches!(e, SuccessionEvent::Admitted { .. })),
            Some(SuccessionEvent::Admitted { generation, replaced: Some(r), .. }) if *generation == c && *r == b
        ));

        // The same cell again, but not better by the margin: refused, and named.
        let d = cand(&mut l, Some(c), 0.46, 0.5, 0.1, 0.90);
        let why = rejected(&ep, &l, &d);
        assert!(matches!(why.as_slice(), [RejectReason::CellOccupied { elite, .. }] if *elite == c), "{why:?}");
        assert!(admit(&ep, &mut l, d).is_err());
        assert_eq!(l.archive().unwrap().occupant(&vec![1, 0]), Some(c), "a refusal leaves the cell alone");

        // Better than the champion in its own cell: replaces it and takes the championship.
        let e = cand(&mut l, Some(root), 0.60, 0.1, 0.1, 0.90);
        let e = admit(&ep, &mut l, e).unwrap();
        assert_eq!(l.champion().map(|g| g.id), Some(e));
        assert!(matches!(l.events().last(), Some(SuccessionEvent::Promoted { generation, .. }) if *generation == e));
    }

    #[test]
    fn the_champion_changes_only_by_the_margin_and_is_always_an_elite() {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.50, 0.1, 0.1, 0.90);
        let root = admit(&ep, &mut l, g).unwrap();
        // An elite in another cell that is better, but by less than the margin
        // (0.02), does not take the championship.
        let g = cand(&mut l, Some(root), 0.51, 0.5, 0.1, 0.90);
        let near = admit(&ep, &mut l, g).unwrap();
        assert_eq!(l.champion().map(|g| g.id), Some(root), "within the margin: the sitting champion stays");
        // By the margin: it does.
        let g = cand(&mut l, Some(root), 0.53, 0.8, 0.1, 0.90);
        let far = admit(&ep, &mut l, g).unwrap();
        assert_eq!(l.champion().map(|g| g.id), Some(far));
        assert!(l.archive().unwrap().contains(l.champion().unwrap().id));
        let _ = near;
    }

    #[test]
    fn a_guard_regression_is_refused_even_into_an_empty_cell() {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.50, 0.1, 0.1, 0.90);
        let root = admit(&ep, &mut l, g).unwrap();

        // Empty cell, better capability, worse safety than its own parent.
        let g = cand(&mut l, Some(root), 0.90, 0.8, 0.1, 0.85);
        assert_eq!(l.archive().unwrap().occupant(&vec![2, 0]), None, "the cell is empty");
        let why = rejected(&ep, &l, &g);
        assert!(matches!(why.as_slice(), [RejectReason::GuardRegression { axis, .. }] if axis == "safety"), "{why:?}");

        // Naming no parent does not help: the champion's lineage is a reference too.
        let g = cand(&mut l, None, 0.90, 0.8, 0.1, 0.85);
        assert!(matches!(rejected(&ep, &l, &g).as_slice(), [RejectReason::GuardRegression { .. }]));

        // Within the tolerance, it is fine.
        let g = cand(&mut l, Some(root), 0.90, 0.8, 0.1, 0.895);
        assert!(admit(&ep, &mut l, g).is_ok());
    }

    #[test]
    fn walking_a_guard_axis_down_through_empty_cells_is_caught() {
        // Each step is within the tolerance of its parent. The ratchet compares
        // against the best any ancestor reached, so the third step is refused,
        // although its cell is empty and its parent is only 0.01 above it.
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.50, 0.1, 0.1, 0.95);
        let root = admit(&ep, &mut l, g).unwrap();
        let g = cand(&mut l, Some(root), 0.40, 0.5, 0.1, 0.945);
        let c1 = admit(&ep, &mut l, g).unwrap();
        let g = cand(&mut l, Some(c1), 0.40, 0.8, 0.1, 0.936);
        let why = rejected(&ep, &l, &g);
        assert!(matches!(why.as_slice(), [RejectReason::GuardRegression { high_water_gen, .. }] if *high_water_gen == root), "{why:?}");
        // Claiming a different parent does not change the champion's lineage mark.
        let mut g = cand(&mut l, Some(c1), 0.40, 0.8, 0.1, 0.935);
        g.parent = Some(c1);
        assert!(!rejected(&ep, &l, &g).is_empty());
    }

    #[test]
    fn a_replacement_may_not_be_worse_than_the_elite_it_displaces_on_a_guard_axis() {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.50, 0.1, 0.1, 0.90);
        let root = admit(&ep, &mut l, g).unwrap();
        let g = cand(&mut l, Some(root), 0.40, 0.5, 0.1, 0.98);
        let strong = admit(&ep, &mut l, g).unwrap();
        // Same cell, higher capability, safety above the root's mark but below the elite's.
        let g = cand(&mut l, Some(root), 0.45, 0.5, 0.1, 0.92);
        let why = rejected(&ep, &l, &g);
        assert!(matches!(why.as_slice(), [RejectReason::GuardBelowElite { elite, .. }] if *elite == strong), "{why:?}");
        // Into an empty cell the same measurement is fine: there is no elite to be below.
        let g = cand(&mut l, Some(root), 0.45, 0.8, 0.1, 0.92);
        assert!(rejected(&ep, &l, &g).is_empty());
    }

    #[test]
    fn a_candidate_with_no_cell_is_refused() {
        let (ep, mut l) = (episode(), lineage());
        let id = l.next_id();
        let g = Generation::new(id, Digest::of(b"m")).measured(Measurement {
            suite: suite(),
            fitness: FitnessVector::new().with("capability", 0.5).with("size", 0.1).with("safety", 0.9),
            evaluator: EV.into(),
        });
        assert!(matches!(rejected(&ep, &l, &g).as_slice(), [RejectReason::NicheUnmeasured { axis }] if axis == "energy"));
    }

    #[test]
    fn the_niches_are_pinned_like_the_gate() {
        let mut l = lineage();
        let g = cand(&mut l, None, 0.5, 0.1, 0.1, 0.9);

        // Moving a bucket edge after registration voids the episode.
        let mut moved = episode();
        moved.archive.as_mut().unwrap().spec.axes[0].edges[0] = 0.4;
        assert!(matches!(rejected(&moved, &l, &g).as_slice(), [RejectReason::ArchiveMismatch { .. }]));

        // An archive other than the pinned one, either way round.
        let other = Lineage::new()
            .with_archive(NicheSpec::new("capability", vec![NicheAxis::new("size", vec![0.5])], 2, 0.02))
            .unwrap();
        assert!(matches!(rejected(&episode(), &other, &g).as_slice(), [RejectReason::ArchiveMismatch { .. }]));
        let plain = Episode::open(gate(), suite().digest, EV);
        assert!(matches!(rejected(&plain, &l, &g).as_slice(), [RejectReason::ArchiveMismatch { .. }]));
        assert!(matches!(rejected(&episode(), &Lineage::new(), &g).as_slice(), [RejectReason::ArchiveMismatch { .. }]));

        // The capability axis is the gate's primary axis, or there is no archive.
        let mut off = gate();
        off.primary_axis = "other".into();
        assert!(Episode::open(off, suite().digest, EV).with_archive(spec()).is_err());
        // An archive cannot be opened on a lineage that already has a champion.
        let mut started = Lineage::new();
        let g = Generation::new(started.next_id(), Digest::of(b"x")).measured(Measurement {
            suite: suite(),
            fitness: FitnessVector::new().with("capability", 0.5).with("safety", 0.9),
            evaluator: EV.into(),
        });
        let id = g.id;
        started.add(g);
        started.grant(id, Digest::of(b"gate"), EV).unwrap();
        assert!(started.with_archive(spec()).is_err());
    }

    #[test]
    fn an_approval_for_one_archive_state_is_not_spent_on_another() {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.5, 0.1, 0.1, 0.9);
        let root = admit(&ep, &mut l, g).unwrap();
        // Two candidates judged against the same cell, the second spent after the first.
        let a = cand(&mut l, Some(root), 0.6, 0.1, 0.1, 0.9);
        let b = cand(&mut l, Some(root), 0.7, 0.1, 0.1, 0.9);
        let (ap_a, ap_b) = (ep.approve(&a, &l, 0, &yes).unwrap(), ep.approve(&b, &l, 0, &yes).unwrap());
        l.add(a);
        l.promote(ap_a).unwrap();
        l.add(b);
        // The champion moved, so this is stale either way; it is refused.
        assert!(matches!(l.promote(ap_b), Err(LineageError::StaleApproval { .. } | LineageError::Archive(_))));
    }

    // ---- demotion ----------------------------------------------------------

    fn three_elites() -> (Episode, Lineage, [GenerationId; 3]) {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.60, 0.1, 0.1, 0.9);
        let a = admit(&ep, &mut l, g).unwrap();
        let g = cand(&mut l, Some(a), 0.50, 0.5, 0.1, 0.9);
        let b = admit(&ep, &mut l, g).unwrap();
        let g = cand(&mut l, Some(a), 0.40, 0.8, 0.1, 0.9);
        let c = admit(&ep, &mut l, g).unwrap();
        (ep, l, [a, b, c])
    }

    #[test]
    fn demoting_the_champion_empties_its_cell_and_falls_back_to_the_best_elite() {
        let (_, mut l, [a, b, c]) = three_elites();
        assert_eq!(l.champion().map(|g| g.id), Some(a));
        let to = l.demote_champion("failed in production", &yes).unwrap();
        assert_eq!(to, b, "the best remaining elite by capability");
        assert_eq!(l.champion().map(|g| g.id), Some(b));
        assert!(!l.archive().unwrap().contains(a));
        assert_eq!(l.archive().unwrap().occupant(&vec![0, 0]), None, "the cell is empty");
        assert!(l.archive().unwrap().contains(c));
        assert!(l.previously_demoted(a));
        // A fallback that is not materialized is skipped, as it always was.
        let c_artifact = l.get(c).unwrap().artifact.clone();
        let to = l.demote_champion("again", &|d: &Digest| *d != c_artifact).unwrap_err();
        assert_eq!(to, LineageError::NoFallback, "the only other elite is not materialized");
        assert_eq!(l.champion().map(|g| g.id), Some(b), "no champion is lost by a failed demotion");
    }

    #[test]
    fn a_displaced_elite_is_not_restored_when_its_replacement_is_demoted() {
        let (ep, mut l, [a, b, _]) = three_elites();
        let g = cand(&mut l, Some(b), 0.55, 0.5, 0.1, 0.9);
        let b2 = admit(&ep, &mut l, g).unwrap();
        assert!(!l.archive().unwrap().contains(b));
        assert_eq!(l.demote_elite(b2, "regressed", &yes), Ok(None), "not the champion: evicted");
        assert_eq!(l.archive().unwrap().occupant(&vec![1, 0]), None, "emptied, and b is not back");
        assert_eq!(l.champion().map(|g| g.id), Some(a), "the champion is untouched");
        assert!(matches!(l.events().last(), Some(SuccessionEvent::Evicted { generation, .. }) if *generation == b2));
        // Demoting through the archive's door is the champion's demotion when it is the champion.
        assert!(l.demote_elite(a, "x", &yes).unwrap().is_some());
        assert!(l.demote_elite(a, "x", &yes).is_err(), "no longer an elite");
    }

    #[test]
    fn the_last_elite_cannot_be_demoted_into_nothing() {
        let (ep, mut l) = (episode(), lineage());
        let g = cand(&mut l, None, 0.5, 0.1, 0.1, 0.9);
        let a = admit(&ep, &mut l, g).unwrap();
        assert_eq!(l.demote_champion("x", &yes), Err(LineageError::NoFallback));
        assert_eq!(l.champion().map(|g| g.id), Some(a));
        assert!(l.archive().unwrap().contains(a), "and it keeps its cell");
    }

    #[test]
    fn a_rollback_under_an_archive_goes_to_an_elite() {
        let (ep, mut l, [a, b, _]) = three_elites();
        let g = cand(&mut l, Some(a), 0.70, 0.1, 0.1, 0.9);
        let a2 = admit(&ep, &mut l, g).unwrap();
        assert!(matches!(l.roll_back_to(a, "x", &yes), Err(LineageError::Archive(_))), "a was displaced");
        l.roll_back_to(b, "operator's choice", &yes).unwrap();
        assert_eq!(l.champion().map(|g| g.id), Some(b));
        assert!(l.archive().unwrap().contains(a2));
    }

    #[test]
    fn a_lineage_without_an_archive_serializes_as_it_did() {
        let json = Lineage::new().to_json();
        assert!(!json.contains("archive"), "{json}");
        assert!(lineage().to_json().contains("archive"));
    }
}
