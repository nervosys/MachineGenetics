//! Candidate production: the *propose* step.
//!
//! Mirrors the operator vocabulary the compiler already exposes behind the
//! `evolve` keyword (tournament/rank/elitist
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

    /// Standard normal, by Box–Muller on two uniforms. Exactly two draws per
    /// call: the sine half of the pair is discarded, so the draw count is fixed
    /// and a seed's stream stays easy to reason about.
    pub fn gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
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
///   gene, so [`legality`] can ask what a child carries that no parent did,
///   before the candidate costs anything to evaluate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "locus", rename_all = "snake_case")]
pub enum Locus {
    /// A tunable scalar, normalized to `[0,1]`.
    ///
    /// `sigma` is the step size of the self-adaptive evolution strategy (see
    /// [`ParamSearch`]), in units of the locus's range. It is absent until an ES
    /// step has touched this locus, and skipped when absent, so a genome that
    /// never saw the ES serializes, and therefore digests, exactly as it did
    /// before the field existed. Crossover copies whole loci, so a step size
    /// travels with the value it adapted.
    Param {
        value: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sigma: Option<f64>,
    },
    /// A gene: a block named by the hash of its content.
    Gene {
        name: String,
        sha256: String,
        /// `Name(p1, p2)` — what makes two genes substitutable for each other.
        signature: String,
        /// What is known about the effects this gene performs — which is not
        /// the same as a list of them. See [`Effects`].
        effects: Effects,
    },
}

/// What is known about the effects a gene performs.
///
/// **This was `Vec<String>`, and the empty vector was doing two jobs.** A gene
/// drawn from a registry that records no effects arrived with `vec![]`, which
/// the legality check read as "declares nothing" — so every such gene passed,
/// and the safety property held because there was nothing to compare rather
/// than because nothing escalated. A check that cannot fail is not a check.
///
/// The two facts are now distinct, and the error runs the safe way: unchecked
/// genes are refused by [`legality`], so an unverified block costs a candidate
/// instead of admitting the escalation the check exists to catch. This mirrors
/// `forge::models::Effects` deliberately and is a separate type deliberately —
/// `germline` carries heritable material and does not depend on a registry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Effects {
    /// Nobody has checked this gene. **Not** a claim that it has none.
    #[default]
    Unchecked,
    /// Something read this gene's source and reported these effects.
    Checked { declared: Vec<String> },
}

impl Effects {
    /// A checked gene declaring exactly these effects.
    pub fn checked<I: IntoIterator<Item = S>, S: Into<String>>(declared: I) -> Self {
        Effects::Checked {
            declared: declared.into_iter().map(Into::into).collect(),
        }
    }

    /// A checked gene declaring nothing — a purity claim someone stands behind.
    pub fn pure() -> Self {
        Effects::Checked {
            declared: Vec::new(),
        }
    }

    /// The effects, or `None` if nobody has checked. A caller that reads `None`
    /// as an empty list has undone the reason this is not a `Vec`.
    pub fn declared(&self) -> Option<&[String]> {
        match self {
            Effects::Unchecked => None,
            Effects::Checked { declared } => Some(declared),
        }
    }
}

impl Locus {
    pub fn param(value: f64) -> Self {
        Locus::Param { value, sigma: None }
    }

    pub fn gene(
        name: impl Into<String>,
        sha256: impl Into<String>,
        signature: impl Into<String>,
        effects: Effects,
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
            Locus::Param { value, .. } => Some(*value),
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

/// Every effect declared by the *checked* genes in a genome.
///
/// Unchecked genes contribute nothing here, which is exactly why [`legality`]
/// has to ask about them separately instead of trusting this set to be whole.
fn declared_effects(g: &Genome) -> std::collections::BTreeSet<String> {
    g.iter()
        .flat_map(|l| match l {
            Locus::Gene { effects, .. } => effects.declared().unwrap_or(&[]).to_vec(),
            Locus::Param { .. } => Vec::new(),
        })
        .collect()
}

/// Genes in a genome that nobody has checked, by name.
fn unchecked_genes(g: &Genome) -> Vec<String> {
    g.iter()
        .filter_map(|l| match l {
            Locus::Gene {
                name,
                effects: Effects::Unchecked,
                ..
            } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Whether a child may be evaluated, given what its parents were allowed to do.
///
/// Three outcomes rather than a bool, because the two ways of failing want
/// different responses. `Acquired` means the search found a capability its
/// parents did not have — the signal this boundary exists to produce.
/// `Unknowable` means a gene arrived carrying no effect information at all,
/// which is a gap in the registry rather than a discovery about the search.
///
/// Both refuse, and the asymmetry is the point: reading an unchecked gene as
/// pure admits precisely the escalation being checked for, while refusing it
/// costs one candidate and names the block that needs checking.
///
/// This is deliberately not the whole safety argument. What is compared here is
/// the *declaration*; whether a gene's body matches it is the compiler's
/// question and the gate's, and neither is replaced by this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "legality", rename_all = "snake_case")]
pub enum Legality {
    /// Every effect the child declares, some parent declared too.
    Inherited,
    /// The child declares effects no parent did.
    Acquired { effects: Vec<String> },
    /// The child carries genes nobody has checked, so the question cannot be
    /// answered. They are named, so that the answer is obtainable.
    Unknowable { genes: Vec<String> },
}

impl Legality {
    pub fn is_inherited(&self) -> bool {
        matches!(self, Legality::Inherited)
    }
}

/// Decide a child's [`Legality`] against its parents.
///
/// Runs at *propose* time. A candidate that acquires `io` from a parent pair
/// that had none is a capability escalation, and finding that out before
/// evaluation costs a set difference rather than a sandbox, a canary phase and
/// a held-out suite.
///
/// An unchecked gene is reported even when a parent carries the same one: a
/// pair of unknowns does not make a known, and inheritance cannot launder the
/// absence of an answer the way it legitimately launders a declared effect.
pub fn legality(parents: &[&Genome], child: &Genome) -> Legality {
    let unknown = unchecked_genes(child);
    if !unknown.is_empty() {
        return Legality::Unknowable { genes: unknown };
    }
    let inherited: std::collections::BTreeSet<String> =
        parents.iter().flat_map(|p| declared_effects(p)).collect();
    let acquired: Vec<String> = declared_effects(child)
        .difference(&inherited)
        .cloned()
        .collect();
    if acquired.is_empty() {
        Legality::Inherited
    } else {
        Legality::Acquired { effects: acquired }
    }
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

/// The range a `Param` value is normalized onto. Values live in `[0,1]`, so a
/// step of `sigma` is `sigma` of the whole range.
pub const PARAM_RANGE: f64 = 1.0;

/// How the continuous loci (`Locus::Param`) are varied. Gene loci are never
/// touched by this: they keep substitution and crossover. The split is
/// deliberate: a continuous optimiser suits a smooth scalar, and a gene has no
/// distance to step along.
///
/// `Perturb` is the uniform random perturbation of [`Mutation::Perturb`], and the
/// default. `SelfAdaptiveEs` is a self-adaptive evolution strategy with one step size per locus:
/// each Param locus carries its own step size `sigma`, which is itself mutated
/// log-normally before the value moves by `sigma` times a standard normal. It is
/// **not** CMA-ES: there is no covariance matrix and no rank-based recombination
/// of step sizes, so it cannot learn a rotated search direction.
///
/// Serialized into a [`VariationPlan`] only when it is not `Perturb`, so a plan
/// that never names it keeps the digest it had.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "search", rename_all = "snake_case")]
pub enum ParamSearch {
    /// Uniform random perturbation, as before this option existed.
    #[default]
    Perturb,
    /// Log-normal self-adaptive step sizes.
    SelfAdaptiveEs {
        /// Per-locus probability that one proposal steps this locus.
        rate: f64,
        /// Step size of a locus that has none yet.
        sigma_init: f64,
        /// Floor on the step size. Keeps a locus able to move at all.
        sigma_min: f64,
        /// Ceiling on the step size. Keeps one lucky step from crossing the range.
        sigma_max: f64,
        /// Learning rate of the step-size update. `None` means `1/sqrt(2n)`,
        /// with `n` the number of Param loci in the genome.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tau: Option<f64>,
    },
}

impl ParamSearch {
    /// The ES with its stated defaults: step on 20% of loci, start at a tenth of
    /// the range, and keep the step between 0.001 and 0.5 of the range.
    pub fn es() -> Self {
        ParamSearch::SelfAdaptiveEs { rate: 0.2, sigma_init: 0.1, sigma_min: 1e-3, sigma_max: 0.5, tau: None }
    }

    /// Whether this is `Perturb`. Used to leave it out of a serialized plan.
    pub fn is_default(&self) -> bool {
        matches!(self, ParamSearch::Perturb)
    }

    /// Refuse settings the update cannot honour: a rate outside `(0,1]`, or
    /// step sizes that are not `0 < sigma_min <= sigma_init <= sigma_max <= 1`.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            ParamSearch::Perturb => Ok(()),
            ParamSearch::SelfAdaptiveEs { rate, sigma_init, sigma_min, sigma_max, tau } => {
                let finite = [rate, sigma_init, sigma_min, sigma_max].iter().all(|x| x.is_finite());
                if !finite || !(*rate > 0.0 && *rate <= 1.0) {
                    return Err(format!("ES rate must be in (0, 1], got {rate}"));
                }
                if !(*sigma_min > 0.0 && sigma_min <= sigma_init && sigma_init <= sigma_max && *sigma_max <= PARAM_RANGE) {
                    return Err(format!(
                        "ES step sizes must satisfy 0 < sigma_min <= sigma_init <= sigma_max <= {PARAM_RANGE}, \
                         got {sigma_min} <= {sigma_init} <= {sigma_max}"
                    ));
                }
                if let Some(t) = tau {
                    if !(t.is_finite() && *t > 0.0) {
                        return Err(format!("ES tau must be a positive number, got {t}"));
                    }
                }
                Ok(())
            }
        }
    }
}

/// One self-adaptive evolution-strategy step over a genome's Param loci.
///
/// For each Param locus, with probability `rate`: draw `s' = s · exp(τ·N)` where
/// `s` is the locus's step size (or `sigma_init` if it has none), clamp `s'` to
/// `[sigma_min, sigma_max]`, then move the value by `s' · N · PARAM_RANGE` and
/// clamp it to `[0,1]`. `N` is a fresh standard normal each time. Loci not
/// stepped keep their value and step size. A genome with no Param loci, or a
/// `Perturb` setting, comes back unchanged.
pub fn es_step(genome: &Genome, search: &ParamSearch, rng: &mut Rng) -> Genome {
    let ParamSearch::SelfAdaptiveEs { rate, sigma_init, sigma_min, sigma_max, tau } = search else {
        return genome.clone();
    };
    let mut g = genome.clone();
    let n = g.iter().filter(|l| matches!(l, Locus::Param { .. })).count();
    if n == 0 {
        return g;
    }
    let tau = tau.unwrap_or(1.0 / (2.0 * n as f64).sqrt());
    for locus in g.iter_mut() {
        let Locus::Param { value, sigma } = locus else { continue };
        if rng.next_f64() >= *rate {
            continue;
        }
        let s = ((*sigma).unwrap_or(*sigma_init) * (tau * rng.gaussian()).exp()).max(*sigma_min).min(*sigma_max);
        *value = (*value + s * rng.gaussian() * PARAM_RANGE).clamp(0.0, 1.0);
        *sigma = Some(s);
    }
    g
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum Selection {
    /// Best of `size` sampled at random. The default: preserves diversity
    /// better than always taking the top, which collapses a population onto one
    /// lineage within a few rounds.
    Tournament { size: usize },
    /// Take the top `keep` by fitness.
    Elitist { keep: usize },
    /// Binary tournament on (non-domination rank, crowding distance) over the
    /// population's objective vectors, all maximized, read by name from each
    /// member's held-out fitness. Replaces the scalar score for parent choice
    /// only. A member without a finite value on every named objective is never
    /// chosen. Needs [`propose_with_points`]; [`select_indices`] returns nothing
    /// for it, because it has no vectors to rank.
    Pareto { objectives: Vec<String> },
}

/// Apply a mutation. Values stay clamped to `[0,1]` so a genome cannot wander
/// out of the space its interpretation is defined on.
pub fn mutate(genome: &Genome, m: Mutation, pool: &GenePool, rng: &mut Rng) -> Genome {
    let mut g = genome.clone();
    match m {
        // Scalars only: a gaussian perturbation of a content hash names nothing.
        Mutation::Perturb { rate, scale } => {
            for locus in g.iter_mut() {
                if let Locus::Param { value, .. } = locus {
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
    select_indices(population, s, count, rng).into_iter().map(|i| population[i].0.clone()).collect()
}

/// [`select`], returning *which* members were chosen. Parents need an identity
/// for a lineage to record who descended from whom; a clone has none.
pub fn select_indices(population: &[(Genome, f64)], s: Selection, count: usize, rng: &mut Rng) -> Vec<usize> {
    if population.is_empty() {
        return Vec::new();
    }
    match s {
        Selection::Tournament { size } => (0..count)
            .map(|_| {
                let mut best = rng.below(population.len());
                for _ in 1..size.max(1) {
                    let c = rng.below(population.len());
                    if population[c].1 > population[best].1 {
                        best = c;
                    }
                }
                best
            })
            .collect(),
        Selection::Elitist { keep } => {
            let mut ranked: Vec<usize> = (0..population.len()).collect();
            ranked.sort_by(|&a, &b| {
                population[b].1.partial_cmp(&population[a].1).unwrap_or(std::cmp::Ordering::Equal)
            });
            ranked.into_iter().take(keep.max(1)).take(count).collect()
        }
        // Pareto ranks objective vectors, which this signature does not carry.
        Selection::Pareto { .. } => Vec::new(),
    }
}

/// Parent choice by Pareto rank and crowding, from each member's objective
/// vector, or `None` where the member has no usable vector.
///
/// A vector is usable when every component is finite. An unusable member is
/// dropped before ranking, so a NaN never reaches the front computation, where
/// it would sit in front 0 as if it were a strong point. Members are drawn with
/// `rng` in the same way as the other modes, so the same seed gives the same
/// parents. Returns indices into `points`. With no usable member it returns
/// nothing.
pub fn select_pareto(points: &[Option<Vec<f64>>], count: usize, rng: &mut Rng) -> Vec<usize> {
    let dims = match points.iter().flatten().next() {
        Some(p) => p.len(),
        None => return Vec::new(),
    };
    let usable: Vec<usize> = (0..points.len())
        .filter(|&i| matches!(&points[i], Some(p) if p.len() == dims && p.iter().all(|v| v.is_finite())))
        .collect();
    if usable.is_empty() {
        return Vec::new();
    }
    let vectors: Vec<Vec<f64>> = usable.iter().map(|&i| points[i].clone().unwrap_or_default()).collect();
    let senses = vec![crate::pareto::Sense::Maximize; dims];
    let ranked = crate::pareto::rank_and_crowding(&vectors, &senses);
    let m = usable.len();
    (0..count)
        .map(|_| {
            let (a, b) = (rng.below(m), rng.below(m));
            usable[crate::pareto::tournament_winner(&ranked, a, b)]
        })
        .collect()
}

/// How a round of proposals is produced. Recorded alongside the seed, so a
/// proposal is fully re-derivable from its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariationPlan {
    pub selection: Selection,
    pub crossover: Crossover,
    pub mutation: Mutation,
    pub offspring: usize,
    /// How continuous loci are varied. Left out of the serialized plan while it
    /// is `Perturb`, so every plan written before this field digests as it did.
    #[serde(default, skip_serializing_if = "ParamSearch::is_default")]
    pub param_search: ParamSearch,
}

impl Default for VariationPlan {
    fn default() -> Self {
        VariationPlan {
            selection: Selection::Tournament { size: 3 },
            crossover: Crossover::Uniform { probability: 0.5 },
            mutation: Mutation::Perturb { rate: 0.2, scale: 0.15 },
            offspring: 8,
            param_search: ParamSearch::Perturb,
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
    /// Why — an escalation found, or a gene nobody could vouch for.
    pub because: Legality,
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
    propose_with_points(population, None, plan, pool, seed)
}

/// [`propose`], with each member's objective vector for [`Selection::Pareto`].
///
/// `points` is parallel to `population`: entry `i` is member `i`'s vector over
/// the objectives the plan names, or `None` where it has none. A Pareto plan
/// given no points selects nothing and so proposes nothing, rather than guessing
/// at vectors. Other selection modes ignore `points`.
pub fn propose_with_points(
    population: &[(Genome, f64)],
    points: Option<&[Option<Vec<f64>>]>,
    plan: VariationPlan,
    pool: &GenePool,
    seed: u64,
) -> Proposal {
    let mut rng = Rng::seed(seed);
    let chosen = match (&plan.selection, points) {
        (Selection::Pareto { .. }, Some(points)) => select_pareto(points, plan.offspring * 2, &mut rng),
        (Selection::Pareto { .. }, None) => Vec::new(),
        (selection, _) => select_indices(population, selection.clone(), plan.offspring * 2, &mut rng),
    };
    if chosen.is_empty() {
        return Proposal::default();
    }
    let parents: Vec<Genome> = chosen.iter().map(|&i| population[i].0.clone()).collect();
    let mut candidates = Vec::new();
    let mut refused = Vec::new();
    for i in 0..plan.offspring {
        let a = &parents[(i * 2) % parents.len()];
        let b = &parents[(i * 2 + 1) % parents.len()];
        let child = match &plan.param_search {
            ParamSearch::Perturb => mutate(
                &crossover(a, b, plan.crossover, &mut rng),
                plan.mutation,
                pool,
                &mut rng,
            ),
            search => {
                let c = crossover(a, b, plan.crossover, &mut rng);
                // The ES replaces the plan's Perturb for Param loci, so Perturb
                // is skipped here. Swap and Substitute still run.
                let c = if matches!(plan.mutation, Mutation::Perturb { .. }) {
                    c
                } else {
                    mutate(&c, plan.mutation, pool, &mut rng)
                };
                es_step(&c, search, &mut rng)
            }
        };
        let id = format!("cand-{seed:016x}-{i}");
        // Checked here, before the candidate costs anything. A child that
        // acquires an effect neither parent held is a capability escalation,
        // and the cheapest moment to notice is the one before evaluation.
        match legality(&[a, b], &child) {
            Legality::Inherited => {
                // The first parent, as an index into `population`: the caller
                // maps it back to a generation.
                let mut spec = CandidateSpec::new(id, child);
                spec.parent = Some(chosen[(i * 2) % chosen.len()] as u64);
                candidates.push(spec)
            }
            because => refused.push(Refusal { id, because }),
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

    /// A gene for tests: `name(sig)` **checked** as carrying `effects`.
    fn gene(name: &str, sig: &str, effects: &[&str]) -> Locus {
        Locus::gene(name, format!("{name}-sha"), sig, Effects::checked(effects.iter().copied()))
    }

    /// The same gene, with nobody having checked it.
    fn unchecked_gene(name: &str, sig: &str) -> Locus {
        Locus::gene(name, format!("{name}-sha"), sig, Effects::Unchecked)
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
            assert_eq!(
                r.because,
                Legality::Acquired { effects: vec!["io".to_string()] },
                "the refusal names what was acquired"
            );
        }
    }

    #[test]
    fn inherited_effects_are_not_an_acquisition() {
        // A parent already had `io`, so a child carrying it acquired nothing.
        let parent = vec![gene("writer", "Cfg()", &["io"])];
        let child = vec![gene("other_writer", "Cfg()", &["io"])];
        assert_eq!(legality(&[&parent], &child), Legality::Inherited);
        // And a parent checked as pure makes the same child an acquisition.
        let pure = vec![gene("reader", "Cfg()", &[])];
        assert_eq!(
            legality(&[&pure], &child),
            Legality::Acquired { effects: vec!["io".to_string()] }
        );
    }

    #[test]
    fn a_gene_nobody_checked_is_refused_rather_than_assumed_pure() {
        // The bug this type exists to prevent. When `effects` was a `Vec`, a
        // gene from a registry that records no effects arrived as `vec![]` and
        // was indistinguishable from one checked and found pure — so it passed,
        // and the safety property held vacuously.
        let parents = vec![gene("reader", "Cfg()", &[])];
        let child = vec![unchecked_gene("mystery", "Cfg()")];
        assert_eq!(
            legality(&[&parents], &child),
            Legality::Unknowable { genes: vec!["mystery".to_string()] },
            "an unanswered question is not a negative answer"
        );
    }

    #[test]
    fn an_unchecked_gene_is_not_laundered_by_inheritance() {
        // A declared effect a parent already holds is legitimately inherited.
        // An *absence of information* is not: two unknowns do not make a known,
        // so the child is still refused even though the parent carries it too.
        let parent = vec![unchecked_gene("mystery", "Cfg()")];
        let child = vec![unchecked_gene("mystery", "Cfg()")];
        assert_eq!(
            legality(&[&parent], &child),
            Legality::Unknowable { genes: vec!["mystery".to_string()] }
        );
    }

    #[test]
    fn a_substitution_that_pulls_in_an_unchecked_gene_is_refused_and_named() {
        let pure_a = vec![gene("step_a", "Step()", &[]), Locus::param(0.4)];
        let pure_b = vec![gene("step_b", "Step()", &[]), Locus::param(0.6)];
        // A pool whose only substitute is a block nobody has checked.
        let pool = GenePool::new(vec![unchecked_gene("opaque", "Step()")]);

        let plan = VariationPlan {
            mutation: Mutation::Substitute { rate: 1.0 },
            offspring: 3,
            ..VariationPlan::default()
        };
        let out = propose(&[(pure_a, 0.5), (pure_b, 0.6)], plan, &pool, 0xB10C);

        assert!(out.candidates.is_empty(), "nothing unchecked may be evaluated");
        assert_eq!(out.refused.len(), 3);
        for r in &out.refused {
            assert_eq!(
                r.because,
                Legality::Unknowable { genes: vec!["opaque".to_string()] },
                "the refusal names the block that needs checking"
            );
        }
    }

    #[test]
    fn an_empty_population_proposes_nothing() {
        assert!(propose(&[], VariationPlan::default(), &no_pool(), 1).candidates.is_empty());
    }

    /// Three mutually non-dominated points, and one dominated by all three.
    fn front_and_a_dominated_point() -> Vec<Option<Vec<f64>>> {
        vec![Some(vec![0.9, 0.1]), Some(vec![0.1, 0.9]), Some(vec![0.6, 0.6]), Some(vec![0.2, 0.05])]
    }

    #[test]
    fn a_dominated_member_is_chosen_less_often_than_any_front_member() {
        let points = front_and_a_dominated_point();
        let mut hits = [0usize; 4];
        for seed in 0..500u64 {
            for i in select_pareto(&points, 4, &mut Rng::seed(seed)) {
                hits[i] += 1;
            }
        }
        let weakest_front = hits[..3].iter().copied().min().unwrap_or(0);
        assert!(
            hits[3] < weakest_front,
            "dominated member chosen {} times; the least-chosen front member {} times ({hits:?})",
            hits[3],
            weakest_front
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_parents() {
        let points = front_and_a_dominated_point();
        let a = select_pareto(&points, 20, &mut Rng::seed(7));
        let b = select_pareto(&points, 20, &mut Rng::seed(7));
        assert_eq!(a, b);
        assert_ne!(a, select_pareto(&points, 20, &mut Rng::seed(8)), "the seed is what varies the draws");
    }

    #[test]
    fn a_population_of_identical_vectors_still_selects() {
        let points = vec![Some(vec![0.5, 0.5]); 3];
        let got = select_pareto(&points, 10, &mut Rng::seed(3));
        assert_eq!(got.len(), 10);
        assert!(got.iter().all(|&i| i < 3));
    }

    #[test]
    fn a_member_with_a_nan_or_no_vector_is_never_chosen() {
        let points = vec![Some(vec![f64::NAN, 1.0]), Some(vec![0.1, 0.1]), None];
        for seed in 0..50 {
            for i in select_pareto(&points, 6, &mut Rng::seed(seed)) {
                assert_eq!(i, 1, "only the usable member may be chosen");
            }
        }
        assert!(select_pareto(&[None, Some(vec![f64::NAN])], 4, &mut Rng::seed(1)).is_empty());
    }

    #[test]
    fn propose_with_points_is_reproducible_from_its_seed() {
        let pop: Vec<(Genome, f64)> = (0..4).map(|i| (vec![Locus::param(i as f64 / 4.0)], 0.5)).collect();
        let plan = VariationPlan {
            selection: Selection::Pareto { objectives: vec!["x".into(), "y".into()] },
            offspring: 6,
            ..VariationPlan::default()
        };
        let points = front_and_a_dominated_point();
        let a = propose_with_points(&pop, Some(&points), plan.clone(), &no_pool(), 99);
        let b = propose_with_points(&pop, Some(&points), plan.clone(), &no_pool(), 99);
        assert_eq!(a, b);
        assert!(!a.candidates.is_empty());
    }

    #[test]
    fn a_pareto_plan_without_vectors_proposes_nothing() {
        let plan = VariationPlan {
            selection: Selection::Pareto { objectives: vec!["x".into()] },
            ..VariationPlan::default()
        };
        assert!(propose(&pop(), plan, &no_pool(), 1).candidates.is_empty());
    }

    // ── The self-adaptive evolution strategy ─────────────────────────────

    /// Survivors kept per generation in [`run_search`].
    const KEEP: usize = 8;

    /// A smooth concave fitness with its optimum at `target`: `1 − Σ(x − x*)²`.
    fn concave(g: &Genome, target: &[f64]) -> f64 {
        1.0 - params_of(g).iter().zip(target).map(|(x, t)| (x - t).powi(2)).sum::<f64>()
    }

    /// A generational search over `target.len()` Param loci. The initial
    /// population is fixed by `seed`, each generation proposes `plan.offspring`
    /// children under a seed derived from `seed`, and the best [`KEEP`] survive.
    /// Returns the best genome, its fitness, and the final population.
    fn run_search(
        plan: &VariationPlan,
        target: &[f64],
        seed: u64,
        generations: u64,
    ) -> (Genome, f64, Vec<(Genome, f64)>) {
        let mut init = Rng::seed(seed ^ 0xABCD);
        let mut pop: Vec<(Genome, f64)> = (0..KEEP)
            .map(|_| {
                let g: Genome = (0..target.len()).map(|_| Locus::param(init.next_f64())).collect();
                let f = concave(&g, target);
                (g, f)
            })
            .collect();
        for gen in 0..generations {
            let round = seed.wrapping_mul(1_000_003).wrapping_add(gen);
            for c in propose(&pop, plan.clone(), &no_pool(), round).candidates {
                let f = concave(&c.genome, target);
                pop.push((c.genome, f));
            }
            pop.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            pop.truncate(KEEP);
        }
        let (g, f) = pop[0].clone();
        (g, f, pop)
    }

    fn es_plan() -> VariationPlan {
        VariationPlan { param_search: ParamSearch::es(), ..VariationPlan::default() }
    }

    /// The optimum the synthetic tasks aim at, inside `[0,1]^3`.
    const TARGET: [f64; 3] = [0.2, 0.7, 0.45];
    /// Generations in the fixed, small budget: 20 × 8 offspring = 160 evaluations.
    const BUDGET: u64 = 20;
    /// The optimum is reached when the fitness is within this of 1.0, which is a
    /// squared distance of at most 0.01 summed over the three loci.
    const TOLERANCE: f64 = 0.99;

    #[test]
    fn es_reaches_the_optimum_of_a_smooth_concave_task_within_a_fixed_budget() {
        let (_, f, _) = run_search(&es_plan(), &TARGET, 1, BUDGET);
        assert!(f >= TOLERANCE, "ES reached fitness {f:.5}, short of {TOLERANCE}");
    }

    #[test]
    fn es_is_within_a_small_margin_of_perturbation_on_the_same_budget_and_seeds() {
        // Measured, not assumed. At the stated defaults the ES does NOT beat
        // perturbation at this budget: over seeds 1..=5 the mean fitness is
        // ES 0.999187 against perturbation 0.999722 (gap -0.00054), and over
        // seeds 1..=20 the gap is -0.0002 at 20 generations and -0.0039 at 10.
        // The defaults were fixed before these numbers were seen and are not
        // tuned to them. So this asserts near-parity, with a margin of 0.001
        // about twice the measured gap, and does not claim dominance.
        let seeds = [1u64, 2, 3, 4, 5];
        let mean = |plan: &VariationPlan| {
            seeds.iter().map(|&s| run_search(plan, &TARGET, s, BUDGET).1).sum::<f64>() / seeds.len() as f64
        };
        let (es, perturb) = (mean(&es_plan()), mean(&VariationPlan::default()));
        assert!(
            es >= perturb - 0.001,
            "mean fitness: ES {es:.6}, perturbation {perturb:.6}: the ES fell more than 0.001 behind"
        );
    }

    #[test]
    fn step_sizes_stay_between_their_floor_and_ceiling() {
        let search = ParamSearch::SelfAdaptiveEs { rate: 1.0, sigma_init: 0.2, sigma_min: 0.05, sigma_max: 0.3, tau: Some(2.0) };
        let mut rng = Rng::seed(41);
        let mut g: Genome = vec![Locus::param(0.5), Locus::param(0.5)];
        for _ in 0..2000 {
            g = es_step(&g, &search, &mut rng);
            for l in &g {
                if let Locus::Param { sigma: Some(s), .. } = l {
                    assert!((0.05..=0.3).contains(s), "step size {s} escaped [0.05, 0.3]");
                }
            }
        }
    }

    #[test]
    fn es_never_moves_a_value_out_of_its_range() {
        // Large steps from both edges, so every clamp is exercised.
        let search = ParamSearch::SelfAdaptiveEs { rate: 1.0, sigma_init: 0.5, sigma_min: 0.1, sigma_max: 1.0, tau: None };
        let mut rng = Rng::seed(42);
        for start in [0.0, 1.0] {
            let mut g: Genome = vec![Locus::param(start); 3];
            for _ in 0..2000 {
                g = es_step(&g, &search, &mut rng);
                assert!(vals(&g).iter().all(|v| (0.0..=1.0).contains(v)), "escaped: {g:?}");
            }
        }
        let (g, _, pop) = run_search(&es_plan(), &TARGET, 9, BUDGET);
        assert!(vals(&g).iter().all(|v| (0.0..=1.0).contains(v)));
        for (member, _) in &pop {
            assert!(vals(member).iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_es_trajectory() {
        let a = run_search(&es_plan(), &TARGET, 7, BUDGET);
        let b = run_search(&es_plan(), &TARGET, 7, BUDGET);
        assert_eq!(a.2, b.2, "the same seed must give the same population, step sizes included");
        let c = run_search(&es_plan(), &TARGET, 8, BUDGET);
        assert_ne!(a.2, c.2, "the seed is what varies the search");
    }

    #[test]
    fn es_settings_are_validated() {
        assert!(ParamSearch::es().validate().is_ok());
        assert!(ParamSearch::Perturb.validate().is_ok());
        let bad = [
            ParamSearch::SelfAdaptiveEs { rate: 0.0, sigma_init: 0.1, sigma_min: 0.01, sigma_max: 0.5, tau: None },
            ParamSearch::SelfAdaptiveEs { rate: 0.2, sigma_init: 0.1, sigma_min: 0.0, sigma_max: 0.5, tau: None },
            ParamSearch::SelfAdaptiveEs { rate: 0.2, sigma_init: 0.6, sigma_min: 0.01, sigma_max: 0.5, tau: None },
            ParamSearch::SelfAdaptiveEs { rate: 0.2, sigma_init: 0.1, sigma_min: 0.01, sigma_max: 0.5, tau: Some(-1.0) },
        ];
        for b in bad {
            assert!(b.validate().is_err(), "accepted {b:?}");
        }
    }

    #[test]
    fn with_the_option_off_nothing_about_the_plan_or_its_loci_changes() {
        // The plan serializes without the field, so an old plan keeps its digest.
        let json = serde_json::to_string(&VariationPlan::default()).unwrap();
        assert!(!json.contains("param_search"), "{json}");
        assert!(!json.contains("sigma"), "{json}");
        // A locus never stepped by the ES is written exactly as before the field.
        assert_eq!(serde_json::to_string(&Locus::param(0.5)).unwrap(), r#"{"locus":"param","value":0.5}"#);
        // And an explicit Perturb is the default, proposal for proposal.
        let explicit = VariationPlan { param_search: ParamSearch::Perturb, ..VariationPlan::default() };
        assert_eq!(propose(&pop(), explicit, &no_pool(), 77), propose(&pop(), VariationPlan::default(), &no_pool(), 77));
    }

    #[test]
    fn a_stepped_locus_round_trips_its_step_size_and_an_old_one_still_parses() {
        let stepped = Locus::Param { value: 0.25, sigma: Some(0.125) };
        let back: Locus = serde_json::from_str(&serde_json::to_string(&stepped).unwrap()).unwrap();
        assert_eq!(back, stepped);
        let old: Locus = serde_json::from_str(r#"{"locus":"param","value":0.75}"#).unwrap();
        assert_eq!(old, Locus::param(0.75));
    }

    #[test]
    fn on_a_mixed_task_both_the_scalar_and_the_gene_loci_change_over_a_run() {
        let plan = VariationPlan {
            mutation: Mutation::Substitute { rate: 0.5 },
            param_search: ParamSearch::es(),
            ..VariationPlan::default()
        };
        let pool = GenePool::new(vec![gene("a", "Sig()", &[]), gene("b", "Sig()", &[]), gene("c", "Sig()", &[])]);
        let start: Genome = vec![Locus::param(0.5), gene("a", "Sig()", &[]), Locus::param(0.5)];
        let (mut pop, mut seen_param, mut seen_gene) = (vec![(start.clone(), 0.0)], false, false);
        for round in 0..30u64 {
            let mut next = Vec::new();
            for c in propose(&pop, plan.clone(), &pool, round).candidates {
                for l in &c.genome {
                    match l {
                        Locus::Param { value, .. } if *value != 0.5 => seen_param = true,
                        Locus::Gene { name, .. } if name != "a" => seen_gene = true,
                        _ => {}
                    }
                }
                next.push((c.genome, 0.0));
            }
            if !next.is_empty() {
                pop = next;
            }
        }
        assert!(seen_param, "no scalar locus ever moved under the ES");
        assert!(seen_gene, "no gene locus was ever substituted");
    }
}
