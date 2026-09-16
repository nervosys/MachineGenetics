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

/// One position in a genome.
///
/// **This was `Vec<f64>`.** A parameter vector can only tune within a fixed
/// template: offspring differ in degree, never in kind, and no amount of search
/// produces a structure the template did not already allow. For a language
/// whose purpose is writing the genetics of offspring, that is a genome for a
/// thermostat rather than an organism.
///
/// A locus is now either a tunable scalar or a **gene** — a block in the
/// registry, carried by content hash. `germline` does not depend on `forge` and
/// must not: the genome carries sequences, and resolving them into something
/// that runs belongs to the expression machinery, exactly as a genome carries
/// codons and the ribosome resolves them. A gene here is therefore data — a
/// name, a hash, a signature, and the effects it declares — not a handle onto a
/// store.
///
/// Two properties fall out rather than being built:
///
/// * **Recombination at gene granularity.** Genes are loci, so the existing
///   single-point and uniform crossover already exchange whole genes between
///   parents. Typed subtree crossover is hard; block substitution is not, and
///   blocks are already content-addressed and signature-typed.
/// * **A legality check with something to read.** `effects` travels with the
///   gene, so [`acquired_effects`] can ask what a child carries that no parent
///   did, before the candidate costs anything to evaluate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "locus", rename_all = "snake_case")]
pub enum Locus {
    /// A tunable scalar, normalized to `[0,1]`.
    Param { value: f64 },
    /// A gene: a block named by the hash of its content.
    Gene {
        name: String,
        sha256: String,
        /// `Name(p1, p2)` — what makes two genes substitutable for each other.
        signature: String,
        /// Effects the gene declares. The unit [`acquired_effects`] compares.
        effects: Vec<String>,
    },
}

impl Locus {
    pub fn param(value: f64) -> Self {
        Locus::Param { value }
    }

    pub fn gene(
        name: impl Into<String>,
        sha256: impl Into<String>,
        signature: impl Into<String>,
        effects: Vec<String>,
    ) -> Self {
        Locus::Gene {
            name: name.into(),
            sha256: sha256.into(),
            signature: signature.into(),
            effects,
        }
    }

    /// The scalar at this locus, or `None` if it holds a gene.
    pub fn as_param(&self) -> Option<f64> {
        match self {
            Locus::Param { value } => Some(*value),
            Locus::Gene { .. } => None,
        }
    }
}

/// A genome: an ordered sequence of loci.
pub type Genome = Vec<Locus>;

/// The genes variation may draw on.
///
/// Substitution only ever exchanges genes of the same signature, so the pool is
/// a source of *alternatives* rather than of arbitrary text. An empty pool
/// makes [`Mutation::Substitute`] a no-op, which is what a run with no registry
/// attached should do.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenePool {
    pub genes: Vec<Locus>,
}

impl GenePool {
    pub fn new(genes: Vec<Locus>) -> Self {
        Self { genes }
    }

    /// A gene from the pool with this signature, chosen by `rng`.
    pub fn same_signature(&self, signature: &str, rng: &mut Rng) -> Option<Locus> {
        let candidates: Vec<&Locus> = self
            .genes
            .iter()
            .filter(|l| matches!(l, Locus::Gene { signature: s, .. } if s == signature))
            .collect();
        if candidates.is_empty() {
            return None;
        }
        Some(candidates[rng.below(candidates.len())].clone())
    }
}

/// The scalars of a genome, in order, ignoring genes.
///
/// `Architecture::decode` is total over `&[f64]` — any vector yields a valid
/// architecture, so search cannot produce an unrepresentable candidate and
/// every rejection is about fitness rather than parsing. That property is worth
/// keeping, so the decoder still takes scalars and this is what hands them
/// over; genes reach expression by a different route, since a hash is not a
/// hyperparameter.
pub fn params_of(g: &Genome) -> Vec<f64> {
    g.iter().filter_map(Locus::as_param).collect()
}

/// Every effect declared by the genes in a genome.
fn declared_effects(g: &Genome) -> std::collections::BTreeSet<String> {
    g.iter()
        .flat_map(|l| match l {
            Locus::Gene { effects, .. } => effects.clone(),
            Locus::Param { .. } => Vec::new(),
        })
        .collect()
}

/// Effects a child carries that no parent did.
///
/// This is the cheap half of the safety argument, and it runs at *propose*
/// time. A candidate that acquires `io` from a parent pair that had none is a
/// capability escalation, and finding that out before evaluation costs a set
/// difference rather than a sandbox, a canary phase and a held-out suite.
///
/// It is deliberately not the whole argument. The declaration is what is
/// compared here; whether the gene's body matches its declaration is the
/// compiler's question and the gate's, and neither is replaced by this.
pub fn acquired_effects(parents: &[&Genome], child: &Genome) -> Vec<String> {
    let inherited: std::collections::BTreeSet<String> =
        parents.iter().flat_map(|p| declared_effects(p)).collect();
    declared_effects(child).difference(&inherited).cloned().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum Mutation {
    /// Perturb each locus with probability `rate` by up to ±`scale`.
    Perturb { rate: f64, scale: f64 },
    /// Swap two loci — preserves the multiset, explores ordering.
    Swap,
    /// Replace a gene with another of the same signature from the pool.
    ///
    /// The operator that makes structural novelty reachable: perturbing a
    /// scalar explores within a template, exchanging a gene changes what the
    /// organism is built from. Same-signature only, so the result still
    /// composes.
    Substitute { rate: f64 },
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
pub fn mutate(genome: &Genome, m: Mutation, pool: &GenePool, rng: &mut Rng) -> Genome {
    let mut g = genome.clone();
    match m {
        // Scalars only: a gaussian perturbation of a content hash names nothing.
        Mutation::Perturb { rate, scale } => {
            for locus in g.iter_mut() {
                if let Locus::Param { value } = locus {
                    if rng.next_f64() < rate {
                        *value = (*value + rng.next_signed() * scale).clamp(0.0, 1.0);
                    }
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
        Mutation::Substitute { rate } => {
            for locus in g.iter_mut() {
                if let Locus::Gene { signature, .. } = locus {
                    if rng.next_f64() < rate {
                        let sig = signature.clone();
                        if let Some(replacement) = pool.same_signature(&sig, rng) {
                            *locus = replacement;
                        }
                    }
                }
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
            a.iter().take(point).chain(b.iter().skip(point)).take(n).cloned().collect()
        }
        Crossover::Uniform { probability } => (0..n)
            .map(|i| if rng.next_f64() < probability { b[i].clone() } else { a[i].clone() })
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
/// A candidate that was produced and then refused, and why.
///
/// Refusals are returned rather than dropped. A proposal step that quietly
/// yields three candidates instead of eight is indistinguishable from one that
/// found the search exhausted, and the difference is the whole signal: the
/// first says the search is pushing against a capability boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub id: String,
    /// Effects the child carried that no parent did.
    pub acquired: Vec<String>,
}

/// What one proposal step produced.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub candidates: Vec<CandidateSpec>,
    pub refused: Vec<Refusal>,
}

pub fn propose(
    population: &[(Genome, f64)],
    plan: VariationPlan,
    pool: &GenePool,
    seed: u64,
) -> Proposal {
    let mut rng = Rng::seed(seed);
    let parents = select(population, plan.selection, plan.offspring * 2, &mut rng);
    if parents.is_empty() {
        return Proposal::default();
    }
    let mut candidates = Vec::new();
    let mut refused = Vec::new();
    for i in 0..plan.offspring {
        let a = &parents[(i * 2) % parents.len()];
        let b = &parents[(i * 2 + 1) % parents.len()];
        let child = mutate(
            &crossover(a, b, plan.crossover, &mut rng),
            plan.mutation,
            pool,
            &mut rng,
        );
        let id = format!("cand-{seed:016x}-{i}");
        // Checked here, before the candidate costs anything. A child that
        // acquires an effect neither parent held is a capability escalation,
        // and the cheapest moment to notice is the one before evaluation.
        let acquired = acquired_effects(&[a, b], &child);
        if acquired.is_empty() {
            candidates.push(CandidateSpec::new(id, child));
        } else {
            refused.push(Refusal { id, acquired });
        }
    }
    Proposal { candidates, refused }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A genome of plain scalars — the shape this module started with, and
    /// still the shape most of these tests want.
    fn params(vs: &[f64]) -> Genome {
        vs.iter().map(|v| Locus::param(*v)).collect()
    }

    /// The scalars of a genome, for assertions that predate genes.
    fn vals(g: &Genome) -> Vec<f64> {
        g.iter().filter_map(Locus::as_param).collect()
    }

    /// An empty pool: `Substitute` is a no-op and nothing else reads it.
    fn no_pool() -> GenePool {
        GenePool::default()
    }

    fn pop() -> Vec<(Genome, f64)> {
        vec![
            (params(&[0.1, 0.1, 0.1]), 0.2),
            (params(&[0.9, 0.9, 0.9]), 0.9),
            (params(&[0.5, 0.5, 0.5]), 0.5),
            (params(&[0.7, 0.7, 0.7]), 0.7),
        ]
    }

    #[test]
    fn the_same_seed_reproduces_the_same_proposals() {
        let a = propose(&pop(), VariationPlan::default(), &no_pool(), 0xDEADBEEF);
        let b = propose(&pop(), VariationPlan::default(), &no_pool(), 0xDEADBEEF);
        assert_eq!(a, b, "a proposal must be re-derivable from its record");
    }

    #[test]
    fn different_seeds_explore_differently() {
        let a = propose(&pop(), VariationPlan::default(), &no_pool(), 1);
        let b = propose(&pop(), VariationPlan::default(), &no_pool(), 2);
        assert_ne!(a, b);
    }

    #[test]
    fn a_candidate_id_states_how_to_reproduce_it() {
        let c = propose(&pop(), VariationPlan::default(), &no_pool(), 0xABC).candidates;
        assert!(c[0].id.contains("0000000000000abc"), "id was {}", c[0].id);
    }

    #[test]
    fn mutation_stays_inside_the_space() {
        let mut rng = Rng::seed(7);
        let g = params(&[0.0, 1.0, 0.5]);
        for _ in 0..200 {
            let m = mutate(&g, Mutation::Perturb { rate: 1.0, scale: 5.0 }, &no_pool(), &mut rng);
            assert!(vals(&m).iter().all(|v| (0.0..=1.0).contains(v)), "escaped: {m:?}");
        }
    }

    #[test]
    fn swap_preserves_the_multiset() {
        let mut rng = Rng::seed(3);
        let g = params(&[0.1, 0.2, 0.3]);
        let m = mutate(&g, Mutation::Swap, &no_pool(), &mut rng);
        let mut a = vals(&g);
        let mut b = vals(&m);
        a.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn single_point_crossover_takes_a_prefix_and_a_suffix() {
        let mut rng = Rng::seed(11);
        let a = params(&[0.0, 0.0, 0.0, 0.0]);
        let b = params(&[1.0, 1.0, 1.0, 1.0]);
        let c = vals(&crossover(&a, &b, Crossover::SinglePoint, &mut rng));
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
        assert_eq!(chosen[0], params(&[0.9, 0.9, 0.9]));
    }

    #[test]
    fn tournament_selection_favours_fitness_without_collapsing_to_one_parent() {
        let mut rng = Rng::seed(9);
        let chosen = select(&pop(), Selection::Tournament { size: 2 }, 200, &mut rng);
        let distinct: std::collections::BTreeSet<String> =
            chosen.iter().map(|g| format!("{:?}", g)).collect();
        assert!(distinct.len() > 1, "tournament must preserve some diversity");
        let best = chosen.iter().filter(|g| vals(g)[0] > 0.85).count();
        assert!(best > chosen.len() / 4, "and must still favour the fit: {best}/200");
    }

    /// A gene for tests: `name(sig)` carrying `effects`.
    fn gene(name: &str, sig: &str, effects: &[&str]) -> Locus {
        Locus::gene(
            name,
            format!("{name}-sha"),
            sig,
            effects.iter().map(|e| e.to_string()).collect(),
        )
    }

    #[test]
    fn substitution_only_exchanges_genes_of_the_same_signature() {
        let mut rng = Rng::seed(21);
        let pool = GenePool::new(vec![
            gene("fast_sort", "Sort(xs)", &[]),
            gene("stable_sort", "Sort(xs)", &[]),
            // Same idea, different shape: must never be substituted in.
            gene("hash_map", "Map(k, v)", &[]),
        ]);
        let g = vec![Locus::param(0.5), gene("naive_sort", "Sort(xs)", &[])];

        for _ in 0..100 {
            let m = mutate(&g, Mutation::Substitute { rate: 1.0 }, &pool, &mut rng);
            // The scalar is untouched: perturbing a hash means nothing, and
            // substituting a scalar is not what this operator is for.
            assert_eq!(m[0], Locus::param(0.5));
            match &m[1] {
                Locus::Gene { signature, name, .. } => {
                    assert_eq!(signature, "Sort(xs)", "substituted across signatures: {name}");
                }
                other => panic!("a gene locus became {other:?}"),
            }
        }
    }

    #[test]
    fn an_empty_pool_leaves_genes_alone() {
        let mut rng = Rng::seed(22);
        let g = vec![gene("only", "F()", &[])];
        let m = mutate(&g, Mutation::Substitute { rate: 1.0 }, &no_pool(), &mut rng);
        assert_eq!(m, g, "with nothing to substitute, substitution is a no-op");
    }

    #[test]
    fn crossover_exchanges_whole_genes() {
        let mut rng = Rng::seed(23);
        let a = vec![gene("a1", "F()", &[]), gene("a2", "G()", &[])];
        let b = vec![gene("b1", "F()", &[]), gene("b2", "G()", &[])];
        let c = crossover(&a, &b, Crossover::Uniform { probability: 0.5 }, &mut rng);
        // Recombination at gene granularity is not a separate mechanism: genes
        // are loci, so the operator that mixed scalars mixes genes.
        assert_eq!(c.len(), 2);
        for (i, locus) in c.iter().enumerate() {
            assert!(
                *locus == a[i] || *locus == b[i],
                "locus {i} came from neither parent: {locus:?}"
            );
        }
    }

    #[test]
    fn a_child_that_acquires_an_effect_is_refused_and_named() {
        // Two parents, neither touching the outside world.
        let pure_a = vec![gene("read_cfg", "Cfg()", &[])];
        let pure_b = vec![gene("parse_cfg", "Cfg()", &[])];
        // A pool whose only substitute writes files.
        let pool = GenePool::new(vec![gene("write_cfg", "Cfg()", &["io"])]);

        let plan = VariationPlan {
            mutation: Mutation::Substitute { rate: 1.0 },
            offspring: 4,
            ..VariationPlan::default()
        };
        let population = vec![(pure_a.clone(), 0.5), (pure_b.clone(), 0.6)];
        let out = propose(&population, plan, &pool, 0xEFF);

        assert!(
            out.candidates.is_empty(),
            "every child acquired `io`, so none may be proposed: {:?}",
            out.candidates
        );
        assert_eq!(out.refused.len(), 4, "and each refusal is reported, not dropped");
        for r in &out.refused {
            assert_eq!(r.acquired, vec!["io".to_string()], "the refusal names what was acquired");
        }
    }

    #[test]
    fn inherited_effects_are_not_an_acquisition() {
        // A parent already had `io`, so a child carrying it acquired nothing.
        let parent = vec![gene("writer", "Cfg()", &["io"])];
        let child = vec![gene("other_writer", "Cfg()", &["io"])];
        assert!(acquired_effects(&[&parent], &child).is_empty());
        // And a parent with no effects makes the same child an acquisition.
        let pure = vec![gene("reader", "Cfg()", &[])];
        assert_eq!(acquired_effects(&[&pure], &child), vec!["io".to_string()]);
    }

    #[test]
    fn an_empty_population_proposes_nothing() {
        assert!(propose(&[], VariationPlan::default(), &no_pool(), 1).candidates.is_empty());
    }
}
