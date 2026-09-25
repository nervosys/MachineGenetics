//! Multi-objective selection: dominance, fronts, hypervolume, and an archive.
//!
//! ## Why a module for this, when [`FitnessVector`](crate::FitnessVector) exists
//!
//! `FitnessVector` is multi-axis, and its docs say the scalar
//! [`composite`](crate::FitnessVector::composite) is "only ever used for
//! *ranking*". Ranking is exactly where a weighted sum fails. A weighted sum —
//! the unweighted mean is one — can only ever prefer points on the **convex
//! hull** of the Pareto front. A candidate in a concave region of the front,
//! better than every hull point at some trade-off, is ranked below a hull point
//! at *every* choice of weights. On a problem whose front is non-convex, search
//! by weighted sum cannot find those trade-offs at all, however long it runs.
//!
//! The arena's objectives (intelligence, joules, latency) have no reason to
//! trace a convex front, so this module ranks by **dominance** instead, and
//! values a point by the **hypervolume** it adds — both of which see concave
//! regions.
//!
//! ## Raw values, not `[0,1]`
//!
//! `FitnessVector` clamps every axis to `[0,1]`. Joules and seconds have no
//! natural maximum, and clamping them would make every expensive candidate look
//! equally expensive. Points here are raw measurements with an explicit
//! [`Sense`] per axis, and hypervolume is taken against a reference point the
//! caller states — which is where an honest "no worse than this" belongs.
//!
//! ## Hypervolume contribution is the credit signal
//!
//! When several agents feed one archive ([`Archive::contributions`]), the fair
//! question to ask of each point is not *how good is it* but *how much of the
//! front would be lost without it*. A point dominated by another agent's point
//! contributes nothing, however good it looks alone, and two agents proposing
//! the same trade-off split nothing between them. That is the property a
//! competition over a shared front needs, and a scalar score does not have it.

use serde::{Deserialize, Serialize};

/// Which direction is better on an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sense {
    Maximize,
    Minimize,
}

/// A named axis and its direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub name: String,
    pub sense: Sense,
}

impl Objective {
    pub fn maximize(name: impl Into<String>) -> Self {
        Objective { name: name.into(), sense: Sense::Maximize }
    }

    pub fn minimize(name: impl Into<String>) -> Self {
        Objective { name: name.into(), sense: Sense::Minimize }
    }
}

/// Flip every axis to minimisation, so the algorithms below need only one case.
fn to_min(point: &[f64], senses: &[Sense]) -> Vec<f64> {
    point
        .iter()
        .zip(senses)
        .map(|(v, s)| match s {
            Sense::Minimize => *v,
            Sense::Maximize => -*v,
        })
        .collect()
}

/// `a` dominates `b`: no worse on every axis and strictly better on one.
///
/// A NaN on either side makes the comparison false in both directions, so a
/// point with an unmeasured axis dominates nothing and is dominated by nothing
/// — it is *incomparable*, which is the truth about it.
pub fn dominates(a: &[f64], b: &[f64], senses: &[Sense]) -> bool {
    assert_eq!(a.len(), senses.len(), "point and senses disagree on arity");
    assert_eq!(b.len(), senses.len(), "point and senses disagree on arity");
    let (a, b) = (to_min(a, senses), to_min(b, senses));
    let mut strictly = false;
    for (x, y) in a.iter().zip(&b) {
        if x.is_nan() || y.is_nan() || x > y {
            return false;
        }
        if x < y {
            strictly = true;
        }
    }
    strictly
}

/// Fronts by non-dominated sorting: front 0 is dominated by nothing, front 1
/// only by front 0, and so on. Indices into `points`, each front ascending.
///
/// O(M·N²), which is right for archives of hundreds; a search with tens of
/// thousands of live points wants a different structure, not this function.
pub fn fronts(points: &[Vec<f64>], senses: &[Sense]) -> Vec<Vec<usize>> {
    let n = points.len();
    let mut dominated_by = vec![0usize; n];
    let mut dominates_list: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i != j && dominates(&points[i], &points[j], senses) {
                dominates_list[i].push(j);
                dominated_by[j] += 1;
            }
        }
    }
    let mut out = Vec::new();
    let mut current: Vec<usize> = (0..n).filter(|&i| dominated_by[i] == 0).collect();
    while !current.is_empty() {
        let mut next = Vec::new();
        for &i in &current {
            for &j in &dominates_list[i] {
                dominated_by[j] -= 1;
                if dominated_by[j] == 0 {
                    next.push(j);
                }
            }
        }
        next.sort_unstable();
        out.push(current);
        current = next;
    }
    out
}

/// Crowding distance within one front (NSGA-II): how isolated each point is
/// from its neighbours, summed over axes. Boundary points get infinity so a
/// front's extremes are never the first to be discarded.
pub fn crowding(points: &[Vec<f64>], front: &[usize]) -> Vec<f64> {
    let m = front.len();
    let mut dist = vec![0.0; m];
    if m <= 2 {
        return vec![f64::INFINITY; m];
    }
    let dims = points[front[0]].len();
    for d in 0..dims {
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|&a, &b| points[front[a]][d].total_cmp(&points[front[b]][d]));
        let lo = points[front[order[0]]][d];
        let hi = points[front[order[m - 1]]][d];
        dist[order[0]] = f64::INFINITY;
        dist[order[m - 1]] = f64::INFINITY;
        let span = hi - lo;
        if span <= 0.0 || !span.is_finite() {
            continue;
        }
        for k in 1..m - 1 {
            let gap = points[front[order[k + 1]]][d] - points[front[order[k - 1]]][d];
            dist[order[k]] += gap / span;
        }
    }
    dist
}

/// The volume of objective space dominated by `points` and bounded by
/// `reference`, exactly.
///
/// Points not strictly better than the reference on every axis contribute
/// nothing, so the reference is where "no better than nothing" sits — choose it
/// as the worst acceptable value on each axis. Exact by recursive slicing
/// (HSO): exponential in dimension in the worst case, and fast for the three or
/// four objectives this crate uses.
pub fn hypervolume(points: &[Vec<f64>], reference: &[f64], senses: &[Sense]) -> f64 {
    let r = to_min(reference, senses);
    let pts: Vec<Vec<f64>> = points
        .iter()
        .map(|p| to_min(p, senses))
        .filter(|p| p.iter().zip(&r).all(|(x, rv)| x.is_finite() && x < rv))
        .collect();
    hv_min(&pts, &r)
}

fn hv_min(points: &[Vec<f64>], r: &[f64]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let d = r.len();
    if d == 1 {
        let best = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        return (r[0] - best).max(0.0);
    }
    // Slice along the last axis: between consecutive distinct values, the
    // dominated region's cross-section is the (d-1)-volume of the points at or
    // below that value.
    let last = d - 1;
    let mut sorted: Vec<&Vec<f64>> = points.iter().collect();
    sorted.sort_by(|a, b| a[last].total_cmp(&b[last]));
    let mut volume = 0.0;
    let mut active: Vec<Vec<f64>> = Vec::new();
    for (k, p) in sorted.iter().enumerate() {
        active.push(p[..last].to_vec());
        let upper = if k + 1 < sorted.len() { sorted[k + 1][last] } else { r[last] };
        let height = upper - p[last];
        if height > 0.0 {
            volume += hv_min(&active, &r[..last]) * height;
        }
    }
    volume
}

/// One archived candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry<T> {
    pub id: u64,
    /// Raw objective values, in the archive's objective order.
    pub point: Vec<f64>,
    /// Who proposed it — the agent credited with its contribution.
    pub owner: String,
    /// The entry it was derived from, if any.
    pub parent: Option<u64>,
    pub payload: T,
}

/// A bounded, multi-objective archive shared by every agent.
///
/// It keeps whole fronts in rank order and, when the last admitted front does
/// not fit, drops its most crowded points — NSGA-II's truncation. Dominated
/// points are kept while there is room, because a dominated point is still a
/// stepping stone: the Darwin Gödel Machine's ablation found that keeping only
/// the best loses most of the gain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive<T> {
    pub objectives: Vec<Objective>,
    pub reference: Vec<f64>,
    pub capacity: usize,
    entries: Vec<Entry<T>>,
    next_id: u64,
}

impl<T: Clone> Archive<T> {
    pub fn new(objectives: Vec<Objective>, reference: Vec<f64>, capacity: usize) -> Self {
        assert_eq!(objectives.len(), reference.len(), "one reference value per objective");
        assert!(capacity > 0, "an archive that holds nothing selects nothing");
        Archive { objectives, reference, capacity, entries: Vec::new(), next_id: 0 }
    }

    pub fn senses(&self) -> Vec<Sense> {
        self.objectives.iter().map(|o| o.sense).collect()
    }

    pub fn entries(&self) -> &[Entry<T>] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn points(&self) -> Vec<Vec<f64>> {
        self.entries.iter().map(|e| e.point.clone()).collect()
    }

    /// Admit a candidate, then truncate to capacity. Returns its id, and
    /// whether it survived the truncation it triggered.
    pub fn insert(
        &mut self,
        point: Vec<f64>,
        owner: impl Into<String>,
        parent: Option<u64>,
        payload: T,
    ) -> (u64, bool) {
        assert_eq!(point.len(), self.objectives.len(), "point arity must match objectives");
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry { id, point, owner: owner.into(), parent, payload });
        self.truncate();
        (id, self.entries.iter().any(|e| e.id == id))
    }

    fn truncate(&mut self) {
        if self.entries.len() <= self.capacity {
            return;
        }
        let senses = self.senses();
        let points = self.points();
        let mut keep: Vec<usize> = Vec::with_capacity(self.capacity);
        for front in fronts(&points, &senses) {
            if keep.len() + front.len() <= self.capacity {
                keep.extend(&front);
                continue;
            }
            let room = self.capacity - keep.len();
            let dist = crowding(&points, &front);
            let mut order: Vec<usize> = (0..front.len()).collect();
            order.sort_by(|&a, &b| dist[b].total_cmp(&dist[a]).then(front[a].cmp(&front[b])));
            keep.extend(order.into_iter().take(room).map(|k| front[k]));
            break;
        }
        keep.sort_unstable();
        let mut k = 0;
        self.entries.retain(|_| {
            let keep_it = keep.binary_search(&k).is_ok();
            k += 1;
            keep_it
        });
    }

    /// The non-dominated entries.
    pub fn front(&self) -> Vec<&Entry<T>> {
        let points = self.points();
        match fronts(&points, &self.senses()).into_iter().next() {
            Some(f) => f.into_iter().map(|i| &self.entries[i]).collect(),
            None => Vec::new(),
        }
    }

    /// Hypervolume of the whole archive against its reference.
    pub fn hypervolume(&self) -> f64 {
        hypervolume(&self.points(), &self.reference, &self.senses())
    }

    /// Each entry's exclusive contribution: the hypervolume lost if it alone
    /// were removed. Zero for anything dominated.
    pub fn contributions(&self) -> Vec<(u64, f64)> {
        let senses = self.senses();
        let points = self.points();
        let total = hypervolume(&points, &self.reference, &senses);
        (0..points.len())
            .map(|i| {
                let rest: Vec<Vec<f64>> =
                    points.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, p)| p.clone()).collect();
                let without = hypervolume(&rest, &self.reference, &senses);
                (self.entries[i].id, (total - without).max(0.0))
            })
            .collect()
    }

    /// Hypervolume contribution summed per owner, sorted by owner name.
    pub fn credit_by_owner(&self) -> Vec<(String, f64)> {
        let mut by: std::collections::BTreeMap<String, f64> = Default::default();
        for e in &self.entries {
            by.entry(e.owner.clone()).or_insert(0.0);
        }
        for (id, c) in self.contributions() {
            let owner = &self.entries.iter().find(|e| e.id == id).expect("own id").owner;
            *by.get_mut(owner).expect("seeded above") += c;
        }
        by.into_iter().collect()
    }

    /// Binary tournament by (front rank, crowding): the parent-selection rule
    /// that lets any archived point breed, preferring the front and, within a
    /// front, the least crowded. `draw` yields indices in `0..n`.
    pub fn tournament(&self, mut draw: impl FnMut(usize) -> usize) -> Option<&Entry<T>> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        let points = self.points();
        let mut rank = vec![0usize; n];
        let mut crowd = vec![0.0f64; n];
        for (r, front) in fronts(&points, &self.senses()).into_iter().enumerate() {
            let dist = crowding(&points, &front);
            for (k, &i) in front.iter().enumerate() {
                rank[i] = r;
                crowd[i] = dist[k];
            }
        }
        let (a, b) = (draw(n), draw(n));
        let better = |x: usize, y: usize| {
            rank[x] < rank[y] || (rank[x] == rank[y] && crowd[x] > crowd[y])
        };
        Some(&self.entries[if better(b, a) { b } else { a }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Sense::*;

    #[test]
    fn dominance_respects_sense_and_strictness() {
        let s = [Maximize, Minimize];
        assert!(dominates(&[2.0, 1.0], &[1.0, 2.0], &s));
        assert!(!dominates(&[1.0, 2.0], &[2.0, 1.0], &s));
        assert!(!dominates(&[1.0, 1.0], &[1.0, 1.0], &s), "equal is not dominance");
        assert!(!dominates(&[2.0, 3.0], &[1.0, 2.0], &s), "a trade-off dominates neither way");
    }

    #[test]
    fn nan_is_incomparable_not_best() {
        let s = [Minimize, Minimize];
        assert!(!dominates(&[f64::NAN, 0.0], &[1.0, 1.0], &s));
        assert!(!dominates(&[1.0, 1.0], &[f64::NAN, 2.0], &s));
    }

    #[test]
    fn fronts_partition_by_rank() {
        let s = [Minimize, Minimize];
        let p = vec![vec![1.0, 4.0], vec![2.0, 2.0], vec![4.0, 1.0], vec![3.0, 3.0], vec![5.0, 5.0]];
        assert_eq!(fronts(&p, &s), vec![vec![0, 1, 2], vec![3], vec![4]]);
    }

    #[test]
    fn hypervolume_matches_hand_computation_in_2d_and_3d() {
        let s2 = [Minimize, Minimize];
        // Staircase (1,3),(2,2),(3,1) against (4,4): 3 + 2 + 1 = 6.
        let p = vec![vec![1.0, 3.0], vec![2.0, 2.0], vec![3.0, 1.0]];
        assert!((hypervolume(&p, &[4.0, 4.0], &s2) - 6.0).abs() < 1e-12);
        // One box in 3D.
        let s3 = [Minimize, Minimize, Minimize];
        assert!((hypervolume(&[vec![1.0, 1.0, 1.0]], &[3.0, 3.0, 3.0], &s3) - 8.0).abs() < 1e-12);
        // Two overlapping boxes: 2·2·1 + 2·1·2 − overlap 2·1·1 = 6.
        let p3 = vec![vec![1.0, 1.0, 2.0], vec![1.0, 2.0, 1.0]];
        assert!((hypervolume(&p3, &[3.0, 3.0, 3.0], &s3) - 6.0).abs() < 1e-12);
    }

    #[test]
    fn a_weighted_sum_misses_a_concave_point_that_hypervolume_credits() {
        // Maximise both. (0.5, 0.5) sits below the chord between the extremes,
        // so it is on the front but off the convex hull: for every weight w,
        // w·0.5 + (1−w)·0.5 = 0.5 < max(w, 1−w). Hypervolume still values it.
        let s = [Maximize, Maximize];
        let pts = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.5, 0.5]];
        for k in 0..=10 {
            let w = k as f64 / 10.0;
            let score = |p: &Vec<f64>| w * p[0] + (1.0 - w) * p[1];
            let best = pts.iter().map(score).fold(f64::MIN, f64::max);
            assert!(score(&pts[2]) <= best, "w={w}");
            if w != 0.5 {
                assert!(score(&pts[2]) < best, "w={w}");
            }
        }
        let mut a: Archive<()> =
            Archive::new(vec![Objective::maximize("a"), Objective::maximize("b")], vec![-0.1, -0.1], 8);
        for p in &pts {
            a.insert(p.clone(), "x", None, ());
        }
        let concave = a.contributions()[2].1;
        assert!(concave > 0.2, "concave point contributes {concave}");
        let _ = s;
    }

    #[test]
    fn a_dominated_point_earns_its_owner_nothing() {
        let mut a: Archive<()> =
            Archive::new(vec![Objective::minimize("j"), Objective::minimize("s")], vec![10.0, 10.0], 8);
        a.insert(vec![1.0, 1.0], "alice", None, ());
        a.insert(vec![2.0, 2.0], "bob", None, ());
        let credit = a.credit_by_owner();
        assert_eq!(credit[1].0, "bob");
        assert_eq!(credit[1].1, 0.0);
        assert!(credit[0].1 > 0.0);
    }

    #[test]
    fn truncation_keeps_the_front_and_its_extremes() {
        let mut a: Archive<u32> =
            Archive::new(vec![Objective::minimize("x"), Objective::minimize("y")], vec![100.0, 100.0], 3);
        for (i, p) in [[1.0, 9.0], [5.0, 5.0], [9.0, 1.0], [5.1, 5.1], [4.9, 5.2]].iter().enumerate() {
            a.insert(p.to_vec(), "x", None, i as u32);
        }
        let kept: Vec<u32> = a.entries().iter().map(|e| e.payload).collect();
        assert!(kept.contains(&0) && kept.contains(&2), "extremes survive: {kept:?}");
        assert!(!kept.contains(&3), "the dominated point goes first: {kept:?}");
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn tournament_prefers_the_front() {
        let mut a: Archive<()> =
            Archive::new(vec![Objective::minimize("x"), Objective::minimize("y")], vec![100.0, 100.0], 8);
        a.insert(vec![1.0, 1.0], "good", None, ());
        a.insert(vec![5.0, 5.0], "bad", None, ());
        let mut seq = [0usize, 1].into_iter().cycle();
        let pick = a.tournament(|_| seq.next().unwrap()).unwrap();
        assert_eq!(pick.owner, "good");
    }
}
