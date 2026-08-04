//! Candidate production: the *propose* step.
//!
//! Mirrors the operator vocabulary the compiler already exposes behind the
//! `evolve` keyword (`prototype/src/evolve_gen.rs`: tournament/rank/elitist
//! selection, single-point/uniform crossover, gaussian/swap mutation) so the two
//! halves of the system name the same things the same way.
//!
//! ## Determinism is a requirement here, not a nicety
//!
//! Every operator takes an explicit [`Rng`] seeded by the caller. Nothing reads
//! a global generator or the clock.
//!
//! In an ordinary genetic algorithm that would be a convenience for debugging.
//! In a lineage that modifies itself it is the difference between an audit trail
//! and a story: "generation 47 was produced from generation 46 by these
//! operators under seed 0x…" is a checkable claim, and an investigator can
//! re-derive the exact candidate. Without it, the record of how a model came to
//! exist is unfalsifiable — which is a poor property for the one artifact you
//! would most want to verify after something goes wrong.

use super::directed::CandidateSpec;
use serde::{Deserialize, Serialize};

/// SplitMix64 — small, fast, and good enough for search. Seeded explicitly so
/// every proposal is reproducible from its record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng(u64);

impl Rng {
    pub fn seed(s: u64) -> Self {
        Rng(s)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0,1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `[-1,1)`.
    pub fn next_signed(&mut self) -> f64 {
        self.next_f64() * 2.0 - 1.0
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }
}

/// A genome: the search space, normalized to `[0,1]` per locus.
pub type Genome = Vec<f64>;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum Mutation {
    /// Perturb each locus with probability `rate` by up to ±`scale`.
    Perturb { rate: f64, scale: f64 },
    /// Swap two loci — preserves the multiset, explores ordering.
    Swap,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum Crossover {
    SinglePoint,
    Uniform { probability: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum Selection {
    /// Best of `size` sampled at random. The default: preserves diversity
    /// better than always taking the top, which collapses a population onto one
    /// lineage within a few rounds.
    Tournament { size: usize },
    /// Take the top `keep` by fitness.
    Elitist { keep: usize },
}

/// Apply a mutation. Values stay clamped to `[0,1]` so a genome cannot wander
/// out of the space its interpretation is defined on.
pub fn mutate(genome: &Genome, m: Mutation, rng: &mut Rng) -> Genome {
    let mut g = genome.clone();
    match m {
        Mutation::Perturb { rate, scale } => {
            for locus in g.iter_mut() {
                if rng.next_f64() < rate {
                    *locus = (*locus + rng.next_signed() * scale).clamp(0.0, 1.0);
                }
            }
        }
        Mutation::Swap => {
            if g.len() >= 2 {
                let i = rng.below(g.len());
                let j = rng.below(g.len());
                g.swap(i, j);
            }
        }
    }
    g
}

/// Recombine two parents.
pub fn crossover(a: &Genome, b: &Genome, c: Crossover, rng: &mut Rng) -> Genome {
    let n = a.len().min(b.len());
    if n == 0 {
        return a.clone();
    }
    match c {
        Crossover::SinglePoint => {
            let point = rng.below(n);
            a.iter().take(point).chain(b.iter().skip(point)).take(n).copied().collect()
        }
        Crossover::Uniform { probability } => (0..n)
            .map(|i| if rng.next_f64() < probability { b[i] } else { a[i] })
            .collect(),
    }
}

/// Choose parents from a scored population.
pub fn select(
    population: &[(Genome, f64)],
    s: Selection,
    count: usize,
    rng: &mut Rng,
) -> Vec<Genome> {
    if population.is_empty() {
        return Vec::new();
    }
    match s {
        Selection::Tournament { size } => (0..count)
            .map(|_| {
                let mut best = &population[rng.below(population.len())];
                for _ in 1..size.max(1) {
                    let c = &population[rng.below(population.len())];
                    if c.1 > best.1 {
                        best = c;
                    }
                }
                best.0.clone()
            })
            .collect(),
        Selection::Elitist { keep } => {
            let mut ranked: Vec<&(Genome, f64)> = population.iter().collect();
            ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            ranked.into_iter().take(keep.max(1)).take(count).map(|p| p.0.clone()).collect()
        }
    }
}

/// How a round of proposals is produced. Recorded alongside the seed, so a
/// proposal is fully re-derivable from its provenance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VariationPlan {
    pub selection: Selection,
    pub crossover: Crossover,
    pub mutation: Mutation,
    pub offspring: usize,
}

impl Default for VariationPlan {
    fn default() -> Self {
        VariationPlan {
            selection: Selection::Tournament { size: 3 },
            crossover: Crossover::Uniform { probability: 0.5 },
            mutation: Mutation::Perturb { rate: 0.2, scale: 0.15 },
            offspring: 8,
        }
    }
}

/// Produce a round of candidates from a scored population.
///
/// Ids encode the seed and index, so a candidate's name states how to reproduce
/// it. That is deliberate: a proposal that cannot be re-derived is a proposal
/// whose origin has to be taken on trust.
pub fn propose(
    population: &[(Genome, f64)],
    plan: VariationPlan,
    seed: u64,
) -> Vec<CandidateSpec> {
    let mut rng = Rng::seed(seed);
    let parents = select(population, plan.selection, plan.offspring * 2, &mut rng);
    if parents.is_empty() {
        return Vec::new();
    }
    (0..plan.offspring)
        .map(|i| {
            let a = &parents[(i * 2) % parents.len()];
            let b = &parents[(i * 2 + 1) % parents.len()];
            let child = mutate(&crossover(a, b, plan.crossover, &mut rng), plan.mutation, &mut rng);
            CandidateSpec::new(format!("cand-{seed:016x}-{i}"), child)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pop() -> Vec<(Genome, f64)> {
        vec![
            (vec![0.1, 0.1, 0.1], 0.2),
            (vec![0.9, 0.9, 0.9], 0.9),
            (vec![0.5, 0.5, 0.5], 0.5),
            (vec![0.7, 0.7, 0.7], 0.7),
        ]
    }

    #[test]
    fn the_same_seed_reproduces_the_same_proposals() {
        let a = propose(&pop(), VariationPlan::default(), 0xDEADBEEF);
        let b = propose(&pop(), VariationPlan::default(), 0xDEADBEEF);
        assert_eq!(a, b, "a proposal must be re-derivable from its record");
    }

    #[test]
    fn different_seeds_explore_differently() {
        let a = propose(&pop(), VariationPlan::default(), 1);
        let b = propose(&pop(), VariationPlan::default(), 2);
        assert_ne!(a, b);
    }

    #[test]
    fn a_candidate_id_states_how_to_reproduce_it() {
        let c = propose(&pop(), VariationPlan::default(), 0xABC);
        assert!(c[0].id.contains("0000000000000abc"), "id was {}", c[0].id);
    }

    #[test]
    fn mutation_stays_inside_the_space() {
        let mut rng = Rng::seed(7);
        let g = vec![0.0, 1.0, 0.5];
        for _ in 0..200 {
            let m = mutate(&g, Mutation::Perturb { rate: 1.0, scale: 5.0 }, &mut rng);
            assert!(m.iter().all(|v| (0.0..=1.0).contains(v)), "escaped: {m:?}");
        }
    }

    #[test]
    fn swap_preserves_the_multiset() {
        let mut rng = Rng::seed(3);
        let g = vec![0.1, 0.2, 0.3];
        let m = mutate(&g, Mutation::Swap, &mut rng);
        let mut a = g.clone();
        let mut b = m.clone();
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn single_point_crossover_takes_a_prefix_and_a_suffix() {
        let mut rng = Rng::seed(11);
        let a = vec![0.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 1.0, 1.0, 1.0];
        let c = crossover(&a, &b, Crossover::SinglePoint, &mut rng);
        assert_eq!(c.len(), 4);
        assert!(c.iter().all(|v| *v == 0.0 || *v == 1.0), "loci come from one parent or the other");
        // Once it switches to b it must not switch back.
        let first_one = c.iter().position(|v| *v == 1.0);
        if let Some(i) = first_one {
            assert!(c[i..].iter().all(|v| *v == 1.0));
        }
    }

    #[test]
    fn elitist_selection_takes_the_best() {
        let mut rng = Rng::seed(5);
        let chosen = select(&pop(), Selection::Elitist { keep: 1 }, 1, &mut rng);
        assert_eq!(chosen[0], vec![0.9, 0.9, 0.9]);
    }

    #[test]
    fn tournament_selection_favours_fitness_without_collapsing_to_one_parent() {
        let mut rng = Rng::seed(9);
        let chosen = select(&pop(), Selection::Tournament { size: 2 }, 200, &mut rng);
        let distinct: std::collections::BTreeSet<String> =
            chosen.iter().map(|g| format!("{:?}", g)).collect();
        assert!(distinct.len() > 1, "tournament must preserve some diversity");
        let best = chosen.iter().filter(|g| g[0] > 0.85).count();
        assert!(best > chosen.len() / 4, "and must still favour the fit: {best}/200");
    }

    #[test]
    fn an_empty_population_proposes_nothing() {
        assert!(propose(&[], VariationPlan::default(), 1).is_empty());
    }
}
