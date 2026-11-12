//! Agents: one to many proposers that collaborate through a shared pool and
//! compete for a fixed budget.
//!
//! ## Collaboration
//!
//! Every program that produced data enters one [`Pool`], whoever wrote it. Any
//! agent may mutate or recombine any entry, so a regularity one agent found is
//! available to all — the Group-Evolving Agents result, that sharing experience
//! across branches beats isolated lineages, taken as the default rather than an
//! option. The pool also serves replay: the learner revisits earlier data so
//! it does not forget it, as the self-play generator's pool does.
//!
//! ## Competition
//!
//! Each round has a fixed number of evaluations. [`allocate`] divides it in
//! proportion to each agent's **credit per unit cost** — learning progress
//! caused, per unit of fuel spent — with a floor so no agent is starved into
//! silence by one bad round. That is the arena's objective applied to the
//! agents themselves: an agent earns budget by producing intelligence cheaply.
//!
//! ## What counts as an agent
//!
//! Anything implementing [`Proposer`]. The three here are deliberately simple
//! and deliberately different, so that the competition has something to decide:
//!
//! * [`GrammarAgent`] learns a policy over MAGE productions by REINFORCE;
//! * [`UniformAgent`] samples the fixed prior and never learns — the baseline
//!   self-play pretraining found scales substantially slower, kept as a control;
//! * [`MutatorAgent`] only varies what is already in the pool.
//!
//! An LLM writing MAGE is a fourth kind, and nothing here would change for it.

use crate::grammar::{crossover_bounded, mutate_bounded, Choice, IntExpr, ListExpr, Policy, Program, Rng};
use serde::{Deserialize, Serialize};

/// A proposed program, and what is needed to credit it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub program: Program,
    /// The sampling trace, for agents whose policy learns from it.
    pub trace: Option<Vec<Choice>>,
    /// Pool entries it was derived from.
    pub parents: Vec<u64>,
}

/// A candidate's result: the learning progress it caused, or `None` if the
/// substrate refused it.
pub type Scored = (Candidate, Option<f64>);

/// What an agent is for (plan 7.2). Each is a language role of the same
/// name, so an agent written in MAGE runs under that role's effect ceiling,
/// and the kernel's ledger accounts agents' energy by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Writes new candidates.
    #[default]
    Proposer,
    /// Varies candidates already in the pool.
    Mutator,
    /// Searches for exploits of the reward, the substrate and the checkers.
    Attacker,
    /// Measures: the only agent role that may read held-out data.
    Evaluator,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Proposer, Role::Mutator, Role::Attacker, Role::Evaluator];

    /// The language role's name.
    pub fn name(self) -> &'static str {
        match self {
            Role::Proposer => "proposer",
            Role::Mutator => "mutator",
            Role::Attacker => "attacker",
            Role::Evaluator => "evaluator",
        }
    }
}

pub trait Proposer {
    fn id(&self) -> &str;
    fn kind(&self) -> &'static str;
    /// The role it acts in. Writing new programs, by default.
    fn role(&self) -> Role {
        Role::Proposer
    }
    fn propose(&mut self, n: usize, pool: &Pool, rng: &mut Rng) -> Vec<Candidate>;
    fn feedback(&mut self, scored: &[Scored]);
    /// Anything worth reporting about the agent's internal state.
    fn describe(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

// ── The shared pool ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEntry {
    pub id: u64,
    pub program: Program,
    pub bytes: Vec<u8>,
    pub reward: f64,
    /// The cost the program was charged, in the run's cost unit (lower is
    /// better). Recorded by [`Pool::add_costed`]; [`Pool::add_with`] stores
    /// `0.0`, which only [`Retain::Pareto`] would read.
    #[serde(default)]
    pub cost: f64,
    pub agent: String,
    pub round: usize,
}

/// How the pool chooses what to evict when it is over capacity (`--retain`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retain {
    /// The lowest reward goes, through the replaceable retention priority
    /// (`harness/retention.mg` unless a program is supplied).
    #[default]
    Reward,
    /// Non-domination over (reward ↑, cost ↓): the last front goes first, and
    /// within a front the lowest crowding distance. The retention priority is
    /// not consulted, so a retention program with this is a usage error.
    Pareto,
}

/// How a mutator or attacker chooses its parents from the pool (`--parents`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parents {
    /// The `k` highest-reward entries ([`Pool::top`]).
    #[default]
    Reward,
    /// The `k` entries of the first Pareto front on (reward ↑, cost ↓), then
    /// of later fronts, each front taken in crowding-distance order
    /// ([`Pool::parents`]).
    Pareto,
}

/// A crowding distance as an ordering key. An undefined one (NaN, from a NaN
/// objective) ranks as the least isolated, so it is kept last and evicted
/// first; an infinite one, the front's extremes, ranks highest.
fn spread(d: f64) -> f64 {
    if d.is_nan() {
        f64::NEG_INFINITY
    } else {
        d
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pool {
    pub entries: Vec<PoolEntry>,
    pub capacity: usize,
    next_id: u64,
}

impl Pool {
    pub fn new(capacity: usize) -> Pool {
        Pool { entries: Vec::new(), capacity, next_id: 0 }
    }

    /// Add an entry, retaining by reward: the Rust reference for
    /// [`Pool::add_with`] under `|r, _| Ok(r)`.
    pub fn add(&mut self, program: Program, bytes: Vec<u8>, reward: f64, agent: &str, round: usize) -> u64 {
        self.add_with(program, bytes, reward, agent, round, round, &|r, _| Ok(r))
            .expect("the reward priority cannot fail")
    }

    /// Add an entry, then, if the pool is over capacity, evict the entry with
    /// the lowest `priority(reward, age)`, oldest id first among ties, where
    /// `age` is `current_round - entry.round`. The kernel keeps the eviction
    /// and the tie order; `priority` is the policy (plan 5.2,
    /// `harness/retention.mg`). An error from `priority` is returned, and the
    /// pool is left with the entry added and nothing evicted.
    #[allow(clippy::too_many_arguments)]
    pub fn add_with(
        &mut self,
        program: Program,
        bytes: Vec<u8>,
        reward: f64,
        agent: &str,
        round: usize,
        current_round: usize,
        priority: &dyn Fn(f64, f64) -> Result<f64, String>,
    ) -> Result<u64, String> {
        self.add_costed(program, bytes, reward, 0.0, agent, round, current_round, Retain::Reward, priority)
    }

    /// [`Pool::add_with`], recording the entry's `cost` and choosing the
    /// eviction by `retain`. Under [`Retain::Pareto`] the priority is not
    /// called and the victim is the loser of the front ranking (see
    /// [`Pool::pareto_victim`]); under [`Retain::Reward`] it is exactly
    /// `add_with`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_costed(
        &mut self,
        program: Program,
        bytes: Vec<u8>,
        reward: f64,
        cost: f64,
        agent: &str,
        round: usize,
        current_round: usize,
        retain: Retain,
        priority: &dyn Fn(f64, f64) -> Result<f64, String>,
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(PoolEntry { id, program, bytes, reward, cost, agent: agent.to_string(), round });
        if self.entries.len() > self.capacity {
            let worst = match retain {
                Retain::Pareto => self.pareto_victim(),
                Retain::Reward => {
                    // Evict the lowest-priority entry, oldest first among ties:
                    // replay should keep what taught the most, not merely what
                    // is recent.
                    let mut priorities = Vec::with_capacity(self.entries.len());
                    for e in &self.entries {
                        let age = current_round as f64 - e.round as f64;
                        priorities.push(priority(e.reward, age)?);
                    }
                    self.entries
                        .iter()
                        .zip(&priorities)
                        .enumerate()
                        .min_by(|(_, (x, px)), (_, (y, py))| px.total_cmp(py).then(x.id.cmp(&y.id)))
                        .map(|(i, _)| i)
                        .expect("non-empty")
                }
            };
            self.entries.remove(worst);
        }
        Ok(id)
    }

    /// The entry Pareto retention evicts: from the last non-domination front,
    /// the one with the lowest crowding distance, so the front's extremes and
    /// its spread survive. A tie goes to the older entry (the lower id), as
    /// under reward retention. Built on [`germline::pareto::rank_and_crowding`],
    /// so a NaN cost is incomparable: it sits in front 0 with no neighbours.
    /// Exactly one entry is named, so the pool is one over capacity at most.
    fn pareto_victim(&self) -> usize {
        let points: Vec<Vec<f64>> = self.entries.iter().map(|e| vec![e.reward, e.cost]).collect();
        let ranked = germline::pareto::rank_and_crowding(&points, &PROGRAM_SENSES);
        // Worse: a later front, or the same front and less spread. Ties keep
        // the earlier index, which is the older entry.
        let worse = |x: usize, y: usize| {
            ranked[x].0 > ranked[y].0 || (ranked[x].0 == ranked[y].0 && spread(ranked[x].1) < spread(ranked[y].1))
        };
        (1..self.entries.len()).fold(0, |v, i| if worse(i, v) { i } else { v })
    }

    /// Up to `k` parents by Pareto rank and crowding, per [`Parents::Pareto`].
    /// Deterministic: the order is a function of the pool alone, so the RNG is
    /// drawn only by the caller's choice among what is returned.
    pub fn parents(&self, k: usize, how: Parents) -> Vec<&PoolEntry> {
        match how {
            Parents::Reward => self.top(k),
            Parents::Pareto => {
                let points: Vec<Vec<f64>> = self.entries.iter().map(|e| vec![e.reward, e.cost]).collect();
                let mut out: Vec<&PoolEntry> = Vec::with_capacity(k);
                for front in germline::pareto::fronts(&points, &PROGRAM_SENSES) {
                    if out.len() == k {
                        break;
                    }
                    let dist = germline::pareto::crowding(&points, &front);
                    let mut order: Vec<(usize, f64)> = front.iter().copied().zip(dist).collect();
                    order.sort_by(|a, b| {
                        spread(b.1).total_cmp(&spread(a.1)).then(self.entries[a.0].id.cmp(&self.entries[b.0].id))
                    });
                    for (i, _) in order {
                        if out.len() == k {
                            break;
                        }
                        out.push(&self.entries[i]);
                    }
                }
                out
            }
        }
    }

    /// The `k` highest-reward entries.
    pub fn top(&self, k: usize) -> Vec<&PoolEntry> {
        let mut v: Vec<&PoolEntry> = self.entries.iter().collect();
        v.sort_by(|a, b| b.reward.total_cmp(&a.reward).then(a.id.cmp(&b.id)));
        v.truncate(k);
        v
    }

    pub fn sample(&self, rng: &mut Rng) -> Option<&PoolEntry> {
        if self.entries.is_empty() {
            None
        } else {
            Some(&self.entries[rng.below(self.entries.len())])
        }
    }
}

// ── Budget ───────────────────────────────────────────────────────────────

/// Divide `total` evaluations among agents by `score` (credit per unit cost).
///
/// Every agent gets `floor` first; the remainder goes by a softmax over scores
/// normalised to the best, at `temperature`, rounded by largest remainder so the
/// shares always sum to `total`. Deterministic: equal scores, equal shares.
/// The softmax is shifted by the best's term, so no weight overflows: before
/// plan 5.2 found it, a temperature below about 0.0014 made the weights
/// infinite and the shares NaN-rounded garbage.
///
/// The Rust reference for `harness/allocation.mg`, which the arena runs
/// (plan 5.2); a test holds the two to identical shares.
pub fn allocate(total: usize, score: &[f64], floor: usize, temperature: f64) -> Vec<usize> {
    let best = score.iter().cloned().fold(0.0f64, f64::max);
    let norm: Vec<f64> =
        score.iter().map(|s| if best > 0.0 && s.is_finite() { s / best } else { 0.0 }).collect();
    let t = temperature.max(1e-6);
    let w: Vec<f64> = norm.iter().map(|x| ((x - 1.0) / t).exp()).collect();
    allocate_weights(total, &w, floor)
}

/// The kernel's half of allocation: every agent gets `floor` first, and the
/// remainder is divided in proportion to `weights`, rounded by largest
/// remainder so the shares always sum to `total`. Weights that sum to zero,
/// or to something not finite, divide the remainder equally.
pub fn allocate_weights(total: usize, weights: &[f64], floor: usize) -> Vec<usize> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let floor = floor.min(total / n);
    let rest = total - floor * n;
    let z: f64 = weights.iter().sum();
    let equal = vec![1.0; n];
    let (w, z) = if z > 0.0 && z.is_finite() { (weights, z) } else { (&equal[..], n as f64) };
    let exact: Vec<f64> = w.iter().map(|x| rest as f64 * x / z).collect();
    let mut share: Vec<usize> = exact.iter().map(|x| x.floor() as usize).collect();
    let mut left = rest - share.iter().sum::<usize>();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())).then(a.cmp(&b)));
    for i in order {
        if left == 0 {
            break;
        }
        share[i] += 1;
        left -= 1;
    }
    share.into_iter().map(|s| s + floor).collect()
}

/// Programs as points on (reward ↑, cost ↓): the objective plan 7.1b
/// allocates the budget on.
pub const PROGRAM_SENSES: [germline::pareto::Sense; 2] =
    [germline::pareto::Sense::Maximize, germline::pareto::Sense::Minimize];

/// The hypervolume `points` add to `front` on (reward ↑, cost ↓), against
/// `reference` (reward, cost): how much of the run's trade-off between what a
/// program teaches and what it costs would be missing without them.
///
/// Plan 7.1b's credit. Each agent's is taken against the front as it stood
/// before the round, so agents are not credited by the order they ran in,
/// and a program dominated by earlier work, anyone's, earns nothing however
/// well it scores alone.
pub fn marginal_hypervolume(front: &[Vec<f64>], points: &[Vec<f64>], reference: &[f64; 2]) -> f64 {
    use germline::pareto::hypervolume;
    let mut with = front.to_vec();
    with.extend(points.iter().cloned());
    (hypervolume(&with, reference, &PROGRAM_SENSES) - hypervolume(front, reference, &PROGRAM_SENSES)).max(0.0)
}

/// The non-dominated points of `points`: all a front needs to keep, since a
/// dominated point adds no hypervolume.
pub fn non_dominated(points: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let first = germline::pareto::fronts(&points, &PROGRAM_SENSES).into_iter().next().unwrap_or_default();
    first.into_iter().map(|i| points[i].clone()).collect()
}

// ── Agents ───────────────────────────────────────────────────────────────

/// Learns production probabilities by REINFORCE, with group-normalised
/// advantages (each round's batch is its own baseline, as in GRPO) and a
/// length penalty standing in for the Solomonoff prior's KL term.
pub struct GrammarAgent {
    pub id: String,
    pub policy: Policy,
    pub lr: f64,
    /// Weight of the description-length penalty, in standard deviations.
    pub beta: f64,
    /// The unit the penalty's length is measured in (plan S1).
    pub length_unit: crate::arena::LengthUnit,
}

impl GrammarAgent {
    pub fn new(id: impl Into<String>) -> Self {
        GrammarAgent {
            id: id.into(),
            policy: Policy::default(),
            lr: 0.1,
            beta: 0.1,
            length_unit: crate::arena::LengthUnit::Tokens,
        }
    }
}

fn zscores(xs: &[f64]) -> Vec<f64> {
    let n = xs.len() as f64;
    if xs.is_empty() {
        return Vec::new();
    }
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let sd = var.sqrt();
    if sd < 1e-12 {
        return vec![0.0; xs.len()];
    }
    xs.iter().map(|x| (x - mean) / sd).collect()
}

impl Proposer for GrammarAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "grammar"
    }

    fn propose(&mut self, n: usize, _pool: &Pool, rng: &mut Rng) -> Vec<Candidate> {
        (0..n)
            .map(|_| {
                let (program, trace) = self.policy.sample(rng);
                Candidate { program, trace: Some(trace), parents: vec![] }
            })
            .collect()
    }

    fn feedback(&mut self, scored: &[Scored]) {
        // A refused program earns nothing, which is below any program that ran:
        // the policy learns to stop writing what the compiler rejects.
        let rewards: Vec<f64> = scored.iter().map(|(_, r)| r.unwrap_or(0.0)).collect();
        let lens: Vec<f64> = scored.iter().map(|(c, _)| c.program.length(self.length_unit) as f64).collect();
        let (zr, zl) = (zscores(&rewards), zscores(&lens));
        for (i, (c, _)) in scored.iter().enumerate() {
            if let Some(trace) = &c.trace {
                let adv = zr[i] - self.beta * zl[i];
                self.policy.reinforce(trace, adv, self.lr / scored.len().max(1) as f64 * 4.0);
            }
        }
    }

    fn describe(&self) -> serde_json::Value {
        let (l, i) = self.policy.probabilities();
        let named = |names: &[&str], p: Vec<f64>| -> serde_json::Value {
            names.iter().zip(p).map(|(n, x)| ((*n).to_string(), serde_json::json!((x * 1000.0).round() / 1000.0))).collect::<serde_json::Map<_, _>>().into()
        };
        serde_json::json!({
            "list": named(&crate::grammar::list_rule_names(self.policy.second_order()), l),
            "int": named(&crate::grammar::int_rules(), i),
        })
    }
}

/// Samples the uniform prior and never learns. The control.
pub struct UniformAgent {
    pub id: String,
    policy: Policy,
}

impl UniformAgent {
    pub fn new(id: impl Into<String>) -> Self {
        UniformAgent::with_second_order(id, false)
    }

    /// The uniform prior, with the second-order production when asked (P5b).
    pub fn with_second_order(id: impl Into<String>, second_order: bool) -> Self {
        UniformAgent { id: id.into(), policy: Policy::with_second_order(second_order) }
    }
}

impl Proposer for UniformAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "uniform"
    }

    fn propose(&mut self, n: usize, _pool: &Pool, rng: &mut Rng) -> Vec<Candidate> {
        (0..n)
            .map(|_| Candidate { program: self.policy.sample(rng).0, trace: None, parents: vec![] })
            .collect()
    }

    fn feedback(&mut self, _scored: &[Scored]) {}
}

impl MutatorAgent {
    /// Count a variant, and whether it exceeds max_depth (F1).
    fn count_graft(&mut self, program: &Program) {
        self.grafts += 1;
        if crate::grammar::check_list_depth(&program.body, 0).is_err() {
            self.over_depth += 1;
        }
    }
}

/// Varies the pool's best entries — whoever wrote them — by mutation and
/// crossover. Falls back to the prior while the pool is empty.
pub struct MutatorAgent {
    pub id: String,
    /// How many of the pool's best entries to draw parents from.
    pub elite: usize,
    /// Whether grafts are sampled at the depth they replace (plan F1).
    pub bound_grafts: bool,
    /// How the parents are drawn from the pool (`--parents`).
    pub parents: Parents,
    policy: Policy,
    /// Variants made by mutation or crossover, and how many exceed max_depth.
    grafts: usize,
    over_depth: usize,
}

impl MutatorAgent {
    pub fn new(id: impl Into<String>) -> Self {
        MutatorAgent::with_second_order(id, false)
    }

    /// The mutator, with the second-order production in its fresh subtrees when asked (P5b).
    pub fn with_second_order(id: impl Into<String>, second_order: bool) -> Self {
        MutatorAgent::with_options(id, second_order, false)
    }

    /// The mutator, with its grafts bounded to max_depth when asked (plan F1).
    pub fn with_options(id: impl Into<String>, second_order: bool, bound_grafts: bool) -> Self {
        MutatorAgent {
            id: id.into(),
            elite: 16,
            bound_grafts,
            parents: Parents::Reward,
            policy: Policy::with_second_order(second_order),
            grafts: 0,
            over_depth: 0,
        }
    }
}

impl Proposer for MutatorAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "mutator"
    }

    fn role(&self) -> Role {
        Role::Mutator
    }

    fn propose(&mut self, n: usize, pool: &Pool, rng: &mut Rng) -> Vec<Candidate> {
        let elite = pool.parents(self.elite, self.parents);
        (0..n)
            .map(|_| {
                if elite.is_empty() {
                    return Candidate { program: self.policy.sample(rng).0, trace: None, parents: vec![] };
                }
                let a = elite[rng.below(elite.len())];
                if rng.next_f64() < 0.5 {
                    let program = mutate_bounded(&a.program, &self.policy, rng, self.bound_grafts);
                    self.count_graft(&program);
                    Candidate { program, trace: None, parents: vec![a.id] }
                } else {
                    let b = elite[rng.below(elite.len())];
                    let program = crossover_bounded(&a.program, &b.program, rng, self.bound_grafts);
                    self.count_graft(&program);
                    Candidate { program, trace: None, parents: vec![a.id, b.id] }
                }
            })
            .collect()
    }

    fn feedback(&mut self, _scored: &[Scored]) {}

    fn describe(&self) -> serde_json::Value {
        serde_json::json!({ "grafts": self.grafts, "over_depth": self.over_depth })
    }
}

/// Searches the reward for exploits (plan 7.4). It wraps the pool's best
/// entries in rewrites that leave every output unchanged, so each of its
/// candidates claims to behave exactly as its parent. The kernel checks the
/// claim, and a candidate that passes and is still counted as novel is a
/// confirmed exploit (`Report::attacks`).
pub struct AttackerAgent {
    pub id: String,
    /// How many of the pool's best entries to draw parents from.
    pub elite: usize,
    /// How the parents are drawn from the pool (`--parents`).
    pub parents: Parents,
}

impl AttackerAgent {
    pub fn new(id: impl Into<String>) -> Self {
        AttackerAgent { id: id.into(), elite: 16, parents: Parents::Reward }
    }
}

/// How many rewrites [`identity_rewrite`] knows.
pub const IDENTITY_REWRITES: usize = 3;

/// `xs.reverse().reverse()`, `xs ++ xs.take(0)` or `xs.map(|v| v + 0)`:
/// each the identity on every list, and each a different structure.
pub fn identity_rewrite(xs: &ListExpr, which: usize) -> ListExpr {
    let inner = || Box::new(xs.clone());
    match which % IDENTITY_REWRITES {
        0 => ListExpr::Reverse(Box::new(ListExpr::Reverse(inner()))),
        1 => ListExpr::Concat(inner(), Box::new(ListExpr::Take(inner(), 0))),
        _ => ListExpr::Map(
            inner(),
            "v".into(),
            IntExpr::Add(Box::new(IntExpr::Var("v".into())), Box::new(IntExpr::Lit(0))),
        ),
    }
}

impl Proposer for AttackerAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "attacker"
    }

    fn role(&self) -> Role {
        Role::Attacker
    }

    fn propose(&mut self, n: usize, pool: &Pool, rng: &mut Rng) -> Vec<Candidate> {
        // Nothing to attack until the pool holds something.
        let elite = pool.parents(self.elite, self.parents);
        if elite.is_empty() {
            return Vec::new();
        }
        (0..n)
            .map(|_| {
                let a = elite[rng.below(elite.len())];
                let body = identity_rewrite(&a.program.body, rng.below(IDENTITY_REWRITES));
                Candidate { program: Program { body }, trace: None, parents: vec![a.id] }
            })
            .collect()
    }

    fn feedback(&mut self, _scored: &[Scored]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_identity_rewrite_preserves_output_and_changes_shape() {
        // Fuel enough for `xs ++ xs.take(0)`, which evaluates `xs` twice.
        let sub = crate::substrate::Substrate { fuel: 1_000_000, max_bytes: 4096 };
        let bytes = |o: crate::substrate::Outcome| match o {
            crate::substrate::Outcome::Bytes { bytes, .. } => Some(bytes),
            crate::substrate::Outcome::Refused(..) => None,
        };
        let mut rng = Rng(11);
        let policy = Policy::default();
        let mut checked = 0;
        for _ in 0..40 {
            let parent = policy.sample(&mut rng).0;
            let Ok(p) = sub.prepare(&parent.source()) else { continue };
            if !matches!(sub.execute(&p, 3), crate::substrate::Outcome::Bytes { .. }) {
                continue;
            }
            for which in 0..IDENTITY_REWRITES {
                let variant = Program { body: identity_rewrite(&parent.body, which) };
                let v = sub.prepare(&variant.source()).expect("a rewrite of a valid program is valid");
                assert_ne!(v.shape, p.shape, "rewrite {which} kept the shape of {}", parent.source());
                for seed in [3, 40, 200] {
                    assert_eq!(bytes(sub.execute(&p, seed)), bytes(sub.execute(&v, seed)), "{}", variant.source());
                }
                checked += 1;
            }
        }
        assert!(checked >= 30, "only {checked} rewrites checked");
    }

    #[test]
    fn allocation_at_a_near_zero_temperature_gives_the_best_agent_the_rest() {
        // Unshifted, these weights were exp(1e6) = inf, and the shares NaN.
        assert_eq!(allocate(10, &[1.0, 2.0, 0.5], 1, 0.0), vec![1, 8, 1]);
        assert_eq!(allocate(10, &[1.0, 2.0, 0.5], 1, 0.0).iter().sum::<usize>(), 10);
    }

    #[test]
    fn allocation_sums_to_total_and_respects_the_floor() {
        for (total, scores) in [(32, vec![1.0, 0.0, 0.5]), (10, vec![0.0, 0.0]), (7, vec![3.0])] {
            let s = allocate(total, &scores, 2, 0.5);
            assert_eq!(s.iter().sum::<usize>(), total, "{scores:?}");
            assert!(s.iter().all(|&x| x >= 2.min(total / scores.len())), "{s:?}");
        }
    }

    #[test]
    fn allocation_is_monotone_in_score() {
        let s = allocate(100, &[0.1, 1.0, 0.5], 1, 0.3);
        assert!(s[1] > s[2] && s[2] > s[0], "{s:?}");
    }

    #[test]
    fn marginal_hypervolume_credits_only_what_the_front_lacked() {
        // Front: one program teaching 2 at cost 1, against reference (0, 4):
        // a 2 × 3 rectangle, 6.
        let front = vec![vec![2.0, 1.0]];
        let r = [0.0, 4.0];
        assert!((germline::pareto::hypervolume(&front, &r, &PROGRAM_SENSES) - 6.0).abs() < 1e-12);
        // A cheaper, weaker program is a new trade-off: its 1 × 3.5
        // rectangle overlaps the front's by 1 × 3, so it adds 0.5.
        let added = marginal_hypervolume(&front, &[vec![1.0, 0.5]], &r);
        assert!((added - 0.5).abs() < 1e-12, "{added}");
        // Dominated, or a repeat of what is there, adds nothing.
        assert_eq!(marginal_hypervolume(&front, &[vec![1.0, 2.0]], &r), 0.0);
        assert_eq!(marginal_hypervolume(&front, &[vec![2.0, 1.0]], &r), 0.0);
        // Worse than the reference on either axis: nothing.
        assert_eq!(marginal_hypervolume(&[], &[vec![-1.0, 1.0]], &r), 0.0);
        assert_eq!(marginal_hypervolume(&[], &[vec![5.0, 9.0]], &r), 0.0);
        // A strictly better program adds its own area less the overlap:
        // 3 × 3.5 − 2 × 3.
        let better = marginal_hypervolume(&front, &[vec![3.0, 0.5]], &r);
        assert!((better - (10.5 - 6.0)).abs() < 1e-12, "{better}");
    }

    #[test]
    fn the_front_keeps_only_what_can_add_hypervolume() {
        let kept = non_dominated(vec![vec![2.0, 1.0], vec![1.0, 2.0], vec![1.0, 0.5], vec![3.0, 3.0]]);
        assert_eq!(kept.len(), 3, "{kept:?}");
        assert!(!kept.contains(&vec![1.0, 2.0]));
    }

    #[test]
    fn equal_scores_share_equally() {
        assert_eq!(allocate(12, &[2.0, 2.0, 2.0], 0, 1.0), vec![4, 4, 4]);
    }

    #[test]
    fn the_pool_evicts_the_least_rewarding() {
        let mut pool = Pool::new(2);
        let p = Program { body: crate::grammar::ListExpr::Range(1) };
        pool.add(p.clone(), vec![0], 0.5, "a", 0);
        pool.add(p.clone(), vec![0], 0.1, "b", 0);
        pool.add(p, vec![0], 0.9, "c", 0);
        let agents: Vec<&str> = pool.entries.iter().map(|e| e.agent.as_str()).collect();
        assert_eq!(agents, vec!["a", "c"]);
    }

    #[test]
    fn a_mutator_draws_on_other_agents_work() {
        let mut pool = Pool::new(8);
        let mut rng = Rng(1);
        let (p, _) = Policy::default().sample(&mut rng);
        let id = pool.add(p, vec![1, 2], 1.0, "someone-else", 0);
        let mut m = MutatorAgent::new("m");
        let c = m.propose(4, &pool, &mut rng);
        assert!(c.iter().all(|c| c.parents.contains(&id)));
    }

    fn prog() -> Program {
        Program { body: crate::grammar::ListExpr::Range(1) }
    }

    /// A pool of `(reward, cost)` points, named `e0`, `e1`, … in order, with
    /// Pareto retention at `cap`.
    fn pareto_pool(cap: usize, points: &[(f64, f64)]) -> Pool {
        let mut pool = Pool::new(cap);
        for (i, (r, c)) in points.iter().enumerate() {
            pool.add_costed(prog(), vec![0], *r, *c, &format!("e{i}"), 0, 0, Retain::Pareto, &|r, _| Ok(r))
                .expect("the pareto eviction cannot fail");
        }
        pool
    }

    fn names(pool: &Pool) -> Vec<String> {
        pool.entries.iter().map(|e| e.agent.clone()).collect()
    }

    #[test]
    fn pareto_retention_evicts_the_dominated_entry_not_the_scalar_lowest() {
        // e0 has the lowest reward and is the cheapest, so it is on front 0.
        // e2 is dominated by e1 (more reward, less cost), so it is on the last
        // front and goes. Reward retention would drop e0 instead.
        let points = [(1.0, 1.0), (3.0, 2.0), (1.5, 6.0)];
        let pareto = pareto_pool(2, &points);
        assert_eq!(names(&pareto), vec!["e0", "e1"], "the dominated e2 is evicted");
        let mut by_reward = Pool::new(2);
        for (i, (r, c)) in points.iter().enumerate() {
            by_reward.add_costed(prog(), vec![0], *r, *c, &format!("e{i}"), 0, 0, Retain::Reward, &|r, _| Ok(r)).unwrap();
        }
        assert_eq!(names(&by_reward), vec!["e1", "e2"], "reward retention drops the lowest reward instead");
    }

    #[test]
    fn pareto_retention_keeps_a_front_member_with_the_lowest_reward() {
        // low is the least rewarding, and it is the cheapest, so it is an
        // extreme of front 0. mid is inside the front and is the one dropped.
        let pool = pareto_pool(2, &[(0.1, 0.1), (3.0, 3.0), (5.0, 5.0)]);
        assert_eq!(names(&pool), vec!["e0", "e2"]);
    }

    #[test]
    fn pareto_retention_evicts_exactly_one_of_identical_entries() {
        let pool = pareto_pool(2, &[(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)]);
        assert_eq!(pool.entries.len(), 2, "one eviction, not zero and not two");
    }

    #[test]
    fn pareto_retention_keeps_capacity_with_a_nan_cost() {
        // A NaN cost is incomparable: it dominates nothing and nothing
        // dominates it, so it sits on front 0. The pool still holds its cap.
        let mut pool = Pool::new(3);
        for i in 0..50 {
            let cost = if i % 4 == 0 { f64::NAN } else { (i % 7) as f64 };
            pool.add_costed(prog(), vec![0], (i % 5) as f64, cost, &format!("e{i}"), 0, 0, Retain::Pareto, &|r, _| Ok(r))
                .unwrap();
            assert!(pool.entries.len() <= 3, "over capacity after {i} adds");
        }
        let points: Vec<Vec<f64>> = pool.entries.iter().map(|e| vec![e.reward, e.cost]).collect();
        let ranked = germline::pareto::rank_and_crowding(&points, &PROGRAM_SENSES);
        for (e, (rank, _)) in pool.entries.iter().zip(&ranked) {
            if e.cost.is_nan() {
                assert_eq!(*rank, 0, "a NaN cost is incomparable, so it is on front 0");
            }
        }
    }

    #[test]
    fn parents_by_pareto_take_a_cheap_moderate_front_member_that_top_k_skips() {
        // hi and mid_hi are both on front 0, and so is lowcost (cheapest). By
        // reward the top two are hi and mid_hi. By front and crowding, the
        // extremes come first, so lowcost is a parent and mid_hi is not.
        let mut pool = Pool::new(8);
        for (name, r, c) in [("hi", 10.0, 100.0), ("mid_hi", 9.0, 90.0), ("lowcost", 5.0, 1.0)] {
            pool.add_costed(prog(), vec![0], r, c, name, 0, 0, Retain::Pareto, &|r, _| Ok(r)).unwrap();
        }
        let by_reward: Vec<&str> = pool.top(2).iter().map(|e| e.agent.as_str()).collect();
        assert_eq!(by_reward, vec!["hi", "mid_hi"]);
        let by_pareto: Vec<&str> = pool.parents(2, Parents::Pareto).iter().map(|e| e.agent.as_str()).collect();
        assert_eq!(by_pareto, vec!["hi", "lowcost"]);
    }

    #[test]
    fn parents_by_pareto_spill_into_the_next_front_when_k_exceeds_the_first() {
        // a dominates d, and d dominates b, so the fronts are {a}, {d}, {b}.
        let mut pool = Pool::new(8);
        for (name, r, c) in [("a", 10.0, 1.0), ("b", 1.0, 10.0), ("d", 5.0, 5.0)] {
            pool.add_costed(prog(), vec![0], r, c, name, 0, 0, Retain::Pareto, &|r, _| Ok(r)).unwrap();
        }
        let two: Vec<&str> = pool.parents(2, Parents::Pareto).iter().map(|e| e.agent.as_str()).collect();
        assert_eq!(two, vec!["a", "d"], "k past front 0 takes front 1");
        let all: Vec<&str> = pool.parents(10, Parents::Pareto).iter().map(|e| e.agent.as_str()).collect();
        assert_eq!(all, vec!["a", "d", "b"], "and then front 2");
    }

    #[test]
    fn parents_by_pareto_are_deterministic_under_the_seed() {
        let mut pool = Pool::new(64);
        let mut rng = Rng(9);
        for i in 0..40 {
            let (p, _) = Policy::default().sample(&mut rng);
            let r = rng.next_f64();
            let c = rng.next_f64() * 10.0;
            pool.add_costed(p, vec![0], r, c, &format!("e{i}"), 0, 0, Retain::Pareto, &|r, _| Ok(r)).unwrap();
        }
        let ids = |k| -> Vec<u64> { pool.parents(k, Parents::Pareto).iter().map(|e| e.id).collect() };
        assert_eq!(ids(8), ids(8), "the same pool gives the same parents");
        let run = |seed: u64| {
            let mut m = MutatorAgent::new("m");
            m.parents = Parents::Pareto;
            let mut rng = Rng(seed);
            m.propose(12, &pool, &mut rng).into_iter().map(|c| c.parents).collect::<Vec<_>>()
        };
        assert_eq!(run(4), run(4), "the same seed gives the same parents");
    }

    /// F1, the source of a bounded run's over-depth parents: an attacker
    /// rewrite adds one or two list levels around its parent, so a program
    /// within max_depth can yield a candidate past it. The gate admits that
    /// candidate, and a bounded graft keeps the wrapper it inherited.
    #[test]
    fn an_identity_rewrite_can_take_an_in_depth_program_past_max_depth() {
        let mut rng = Rng(7);
        let policy = Policy::default();
        let mut past = 0;
        for _ in 0..200 {
            let (p, _) = policy.sample(&mut rng);
            assert!(crate::grammar::check_list_depth(&p.body, 0).is_ok(), "{}", p.source());
            for which in 0..IDENTITY_REWRITES {
                if crate::grammar::check_list_depth(&identity_rewrite(&p.body, which), 0).is_err() {
                    past += 1;
                }
            }
        }
        assert!(past > 0, "no rewrite of an in-depth program went past MAX_DEPTH");
    }
}
