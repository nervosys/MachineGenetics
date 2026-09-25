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

use crate::grammar::{crossover, mutate, Choice, Policy, Program, Rng};
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

pub trait Proposer {
    fn id(&self) -> &str;
    fn kind(&self) -> &'static str;
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
    pub agent: String,
    pub round: usize,
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

    pub fn add(&mut self, program: Program, bytes: Vec<u8>, reward: f64, agent: &str, round: usize) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(PoolEntry { id, program, bytes, reward, agent: agent.to_string(), round });
        if self.entries.len() > self.capacity {
            // Evict the lowest-reward entry, oldest first among ties: replay
            // should keep what taught the most, not merely what is recent.
            let worst = self
                .entries
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.reward.total_cmp(&b.1.reward).then(a.1.id.cmp(&b.1.id)))
                .map(|(i, _)| i)
                .expect("non-empty");
            self.entries.remove(worst);
        }
        id
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
pub fn allocate(total: usize, score: &[f64], floor: usize, temperature: f64) -> Vec<usize> {
    let n = score.len();
    if n == 0 {
        return Vec::new();
    }
    let floor = floor.min(total / n);
    let rest = total - floor * n;
    let best = score.iter().cloned().fold(0.0f64, f64::max);
    let norm: Vec<f64> =
        score.iter().map(|s| if best > 0.0 && s.is_finite() { s / best } else { 0.0 }).collect();
    let t = temperature.max(1e-6);
    let w: Vec<f64> = norm.iter().map(|x| (x / t).exp()).collect();
    let z: f64 = w.iter().sum();
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
}

impl GrammarAgent {
    pub fn new(id: impl Into<String>) -> Self {
        GrammarAgent { id: id.into(), policy: Policy::default(), lr: 0.1, beta: 0.1 }
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
        let lens: Vec<f64> = scored.iter().map(|(c, _)| c.program.token_len() as f64).collect();
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
            "list": named(&crate::grammar::LIST_RULES, l),
            "int": named(&crate::grammar::INT_RULES, i),
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
        UniformAgent { id: id.into(), policy: Policy::default() }
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

/// Varies the pool's best entries — whoever wrote them — by mutation and
/// crossover. Falls back to the prior while the pool is empty.
pub struct MutatorAgent {
    pub id: String,
    /// How many of the pool's best entries to draw parents from.
    pub elite: usize,
    policy: Policy,
}

impl MutatorAgent {
    pub fn new(id: impl Into<String>) -> Self {
        MutatorAgent { id: id.into(), elite: 16, policy: Policy::default() }
    }
}

impl Proposer for MutatorAgent {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        "mutator"
    }

    fn propose(&mut self, n: usize, pool: &Pool, rng: &mut Rng) -> Vec<Candidate> {
        let elite = pool.top(self.elite);
        (0..n)
            .map(|_| {
                if elite.is_empty() {
                    return Candidate { program: self.policy.sample(rng).0, trace: None, parents: vec![] };
                }
                let a = elite[rng.below(elite.len())];
                if rng.next_f64() < 0.5 {
                    Candidate { program: mutate(&a.program, &self.policy, rng), trace: None, parents: vec![a.id] }
                } else {
                    let b = elite[rng.below(elite.len())];
                    Candidate {
                        program: crossover(&a.program, &b.program, rng),
                        trace: None,
                        parents: vec![a.id, b.id],
                    }
                }
            })
            .collect()
    }

    fn feedback(&mut self, _scored: &[Scored]) {}
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
