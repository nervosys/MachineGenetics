//! The scheduler: cache, dispatch, heal, report.
//!
//! Per action, in order:
//!
//! 1. **Key it.** Deterministically, from its inputs ([`super::key`]).
//! 2. **Ask the cache.** A hit whose output blobs all verify is the answer; no
//!    worker is involved. This is where a build system earns its keep, so the
//!    report counts hits and misses as a first-class number rather than a log line.
//! 3. **Otherwise execute** on a worker that satisfies the action's platform.
//! 4. **On failure, heal** ([`super::heal`]) and retry per the remedy.
//! 5. **Record** outputs into the CAS and the claim into the action cache.
//!
//! ## Failure does not abort the build
//!
//! A failed action fails its dependents — transitively and explicitly — and the
//! scheduler carries on with everything unaffected. Aborting on first error is
//! right for a human at a terminal who will fix one thing and re-run; it is wrong
//! for an agent, which wants the complete set of problems in one round trip so it
//! can plan a single repair. `SKIPPED` is therefore a real outcome with a named
//! cause, not silence.
//!
//! ## What the RSI loop consumes
//!
//! [`BuildReport::fitness`] reduces a build to four normalized axes. This is the
//! measurable hook the recursive-self-improvement loop needs: a candidate
//! toolchain, cache policy, or scheduler change is *evaluated* by building a
//! corpus and comparing fitness. Nothing here selects or mutates anything — that
//! is the evolutionary layer above, and it is designed but not built (see
//! `RIBOSOME.md` §7). What exists is the honest measurement it would need.

use super::cas::{ActionResult, CasError, Store};
use super::exec::{Executor, Inputs};
use super::graph::{ActionGraph, ActionId, GraphError};
use super::heal::{relax_platform, HealEvent, Healer, Remedy};
use super::{Action, Digest};
use serde::Serialize;
use std::collections::BTreeMap;

/// How one action turned out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Served from the action cache. No worker ran.
    CacheHit { key: String },
    /// Executed and recorded.
    Built { key: String, worker: String },
    /// Executed successfully, but only after healing.
    Healed { key: String, worker: String, attempts: u32 },
    /// Attempted and failed.
    Failed { key: String, error: String },
    /// Never attempted: a dependency failed.
    Skipped { because: String },
}

impl Outcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Outcome::CacheHit { .. } | Outcome::Built { .. } | Outcome::Healed { .. })
    }
}

/// One action's line in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionReport {
    pub id: ActionId,
    pub name: String,
    pub platform: String,
    pub outcome: Outcome,
}

/// The result of a build — the artifact an agent reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BuildReport {
    pub actions: Vec<ActionReport>,
    pub heal_events: Vec<HealEvent>,
    pub cache_hits: usize,
    pub built: usize,
    pub healed: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Accumulated `cost` of actions that ran *and succeeded*. Cache hits
    /// contribute nothing, which is the point.
    pub work_done: u64,
    /// Accumulated `cost` of actions served from cache.
    ///
    /// Tracked directly rather than inferred as `work_total - work_done`,
    /// because those two differ by more than the cache: a failed or skipped
    /// action is neither done nor cached. Inferring it reported a build that
    /// failed everything as 100% cache reuse — see [`BuildReport::cache_hit_ratio`].
    pub work_cached: u64,
    /// Accumulated `cost` of every action in the graph, run or not.
    pub work_total: u64,
    /// Lower bound on wall time with unlimited workers.
    pub critical_path_cost: u64,
    /// Every logical output the build produced, and the CAS digest holding it.
    ///
    /// A build report that says a build succeeded but not where the artifacts
    /// are is incomplete: the caller would have to recompute action keys with
    /// resolved input digests just to find its own output. Cache hits populate
    /// this too, so a fully-cached build reports the same artifacts as a cold
    /// one — otherwise "nothing to do" and "nothing produced" would be
    /// indistinguishable.
    pub outputs: BTreeMap<String, Digest>,
}

impl BuildReport {
    pub fn success(&self) -> bool {
        self.failed == 0 && self.skipped == 0
    }

    /// Fraction of requested work served from cache, by cost.
    ///
    /// This was `1 - work_done/work_total`, which is only the same thing when
    /// every action either runs or hits the cache. It silently was not: a
    /// failed action contributes to neither term, so a build that failed
    /// *everything* reported a cache hit ratio of **1.0** — perfect reuse, on a
    /// build that reused nothing. Found by running a real compiler through the
    /// CLI and reading the report, not by a test; the tests all used graphs that
    /// succeeded.
    ///
    /// It matters beyond cosmetics because [`Fitness::reuse`] is a selection
    /// signal for the RSI loop, and the old form paid a candidate its highest
    /// possible reuse score for breaking the build. The correctness gate in
    /// [`Fitness::composite`] contained the damage; it should not have had to.
    pub fn cache_hit_ratio(&self) -> f64 {
        if self.work_total == 0 {
            return 1.0;
        }
        self.work_cached as f64 / self.work_total as f64
    }

    /// Four normalized axes in `[0,1]`, higher is better. See the module note.
    ///
    /// Kept deliberately small and explicit rather than a single opaque score:
    /// an agent optimizing a build wants to know *which* axis it moved, and a
    /// scalar hides regressions that trade one axis for another.
    pub fn fitness(&self) -> Fitness {
        let total = self.actions.len().max(1) as f64;
        Fitness {
            // Did it work at all?
            correctness: (self.actions.iter().filter(|a| a.outcome.is_success()).count() as f64)
                / total,
            // How much work was avoided?
            reuse: self.cache_hit_ratio(),
            // How well does the graph parallelize? Ratio of total work to the
            // serial lower bound — 1.0 means fully serial.
            parallelism: if self.critical_path_cost == 0 {
                1.0
            } else {
                (self.critical_path_cost as f64 / self.work_total.max(1) as f64).min(1.0)
            },
            // How much did the infrastructure misbehave? 1.0 = no healing needed.
            stability: 1.0 - (self.heal_events.len() as f64 / total).min(1.0),
        }
    }

    pub fn json(&self) -> String {
        #[derive(Serialize)]
        struct View<'a> {
            success: bool,
            #[serde(flatten)]
            inner: &'a BuildReport,
            cache_hit_ratio: f64,
            fitness: Fitness,
        }
        serde_json::to_string_pretty(&View {
            success: self.success(),
            inner: self,
            cache_hit_ratio: self.cache_hit_ratio(),
            fitness: self.fitness(),
        })
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

/// The evaluation signal for the RSI loop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Fitness {
    pub correctness: f64,
    pub reuse: f64,
    pub parallelism: f64,
    pub stability: f64,
}

impl Fitness {
    /// A scalar in `[0,1]`, for when a selection step needs a total order.
    ///
    /// Correctness is a **gate, not a weight**. A weighted sum cannot express
    /// "correctness dominates": with any fixed weights, a build that fails a
    /// third of its actions but caches perfectly ties or beats a fully-correct
    /// cold build, and a selection loop run on that signal will happily evolve
    /// toward a fast build system that does not work.
    ///
    /// So the range is split. A build where every action succeeded scores in
    /// `[0.5, 1.0]`; a build that failed anything scores strictly below `0.5`,
    /// however good its other axes are. Within each band the secondary axes
    /// order things — including among broken builds, where getting more actions
    /// through is genuinely better.
    ///
    /// This matters more here than in an ordinary build system: this number is
    /// the selection pressure in an RSI loop, and a fitness function with a
    /// loophole is a specification of what the population will exploit.
    pub fn composite(&self) -> f64 {
        let secondary = 0.50 * self.reuse + 0.25 * self.parallelism + 0.25 * self.stability;
        if self.correctness >= 1.0 {
            0.5 + 0.5 * secondary
        } else {
            // Strictly < 0.5 for any correctness < 1: the secondary term is
            // capped at 1.0 and scaled so it can never bridge the band.
            0.5 * self.correctness * (0.9 + 0.1 * secondary)
        }
    }
}

/// Drives a graph to completion against a fleet and a store.
pub struct Scheduler<'a> {
    pub store: &'a Store,
    pub executor: &'a dyn Executor,
    pub healer: &'a dyn Healer,
}

impl<'a> Scheduler<'a> {
    pub fn new(store: &'a Store, executor: &'a dyn Executor, healer: &'a dyn Healer) -> Self {
        Scheduler { store, executor, healer }
    }

    /// Answer "what would you do?" without doing it.
    ///
    /// The no-exec introspection path: keys, cache status, and worker assignment
    /// for every action, computed without running one. An agent uses this to
    /// predict cost before committing — and to verify a build is reproducible by
    /// checking that a second plan is all hits.
    pub fn plan(&self, graph: &ActionGraph) -> Result<String, GraphError> {
        #[derive(Serialize)]
        struct PlanNode {
            id: ActionId,
            name: String,
            key: String,
            cached: bool,
            platform: String,
            schedulable: bool,
        }
        let order = graph.topological_order()?;
        let nodes: Vec<PlanNode> = order
            .iter()
            .map(|&id| {
                let a = &graph.actions[id];
                let key = a.key();
                PlanNode {
                    id,
                    name: a.name.clone(),
                    cached: self.store.actions.get(&key).is_some(),
                    key: key.0,
                    platform: a.platform.tag(),
                    schedulable: self.executor.can_run(a),
                }
            })
            .collect();
        let cached = nodes.iter().filter(|n| n.cached).count();
        let unschedulable: Vec<&str> =
            nodes.iter().filter(|n| !n.schedulable).map(|n| n.name.as_str()).collect();
        Ok(serde_json::to_string_pretty(&serde_json::json!({
            "actions": nodes.len(),
            "already_cached": cached,
            "would_execute": nodes.len() - cached,
            "unschedulable": unschedulable,
            "nodes": nodes,
        }))
        .unwrap_or_default())
    }

    /// Run the graph.
    pub fn build(&self, graph: &ActionGraph) -> Result<BuildReport, GraphError> {
        let order = graph.topological_order()?;
        let (cp_cost, _) = graph.critical_path()?;

        let mut report = BuildReport {
            critical_path_cost: cp_cost,
            work_total: graph.actions.iter().map(|a| a.cost).sum(),
            ..Default::default()
        };

        // Logical output path -> digest, for feeding downstream actions.
        let mut produced: BTreeMap<String, Digest> = BTreeMap::new();
        // Actions that failed or were skipped, so dependents can be skipped with
        // a named cause rather than mysteriously.
        let mut broken: BTreeMap<ActionId, String> = BTreeMap::new();

        for &id in &order {
            let action = &graph.actions[id];

            if let Some(cause) = graph
                .deps_of(id)
                .iter()
                .find_map(|d| broken.get(d).map(|c| format!("{} ({c})", graph.actions[*d].name)))
            {
                report.skipped += 1;
                broken.insert(id, format!("dependency `{cause}` did not build"));
                report.actions.push(ActionReport {
                    id,
                    name: action.name.clone(),
                    platform: action.platform.tag(),
                    outcome: Outcome::Skipped { because: format!("dependency `{cause}` failed") },
                });
                continue;
            }

            let outcome = self.run_one(action, &mut produced, &mut report);
            if !outcome.is_success() {
                broken.insert(id, "failed".to_string());
            }
            report.actions.push(ActionReport {
                id,
                name: action.name.clone(),
                platform: action.platform.tag(),
                outcome,
            });
        }

        report.outputs = produced;
        Ok(report)
    }

    /// Cache-check, execute, heal. Returns the outcome and updates `produced`.
    fn run_one(
        &self,
        action: &Action,
        produced: &mut BTreeMap<String, Digest>,
        report: &mut BuildReport,
    ) -> Outcome {
        // Rebind inputs to the digests actually produced upstream. The graph was
        // built with placeholder digests for not-yet-built outputs; the real key
        // can only be known once dependencies have run. This is why keys are
        // computed here and not once at graph construction.
        let mut resolved = action.clone();
        for i in resolved.inputs.iter_mut() {
            if let Some(d) = produced.get(&i.path) {
                i.digest = d.clone();
            }
        }

        let mut attempt = 0u32;
        let mut current = resolved;

        loop {
            let key = current.key();

            // 1. Cache.
            if let Some(cached) = self.store.actions.get(&key) {
                match self.verify_and_adopt(&cached, &current, produced) {
                    Ok(()) => {
                        report.cache_hits += 1;
                        report.work_cached += current.cost;
                        return Outcome::CacheHit { key: key.0 };
                    }
                    Err(cas_err) => {
                        // The claim exists but its blobs do not verify. Heal.
                        let remedy = self.healer.on_cas_error(&current, &cas_err, attempt);
                        report.heal_events.push(HealEvent {
                            action: current.name.clone(),
                            failure: cas_err.to_string(),
                            remedy: remedy.clone(),
                        });
                        match remedy {
                            Remedy::EvictAndRetry { attempt: n, blob } => {
                                let _ = self.store.actions.invalidate(&key);
                                let _ = self.store.cas.evict(&Digest(blob));
                                attempt = n;
                                continue;
                            }
                            Remedy::Escalate { reason } => {
                                report.failed += 1;
                                return Outcome::Failed { key: key.0, error: reason };
                            }
                            // Retry/Substitute are not meaningful answers to a
                            // corrupt cache entry; treat them as escalation
                            // rather than looping.
                            other => {
                                report.failed += 1;
                                return Outcome::Failed {
                                    key: key.0,
                                    error: format!("unusable remedy for cache corruption: {other:?}"),
                                };
                            }
                        }
                    }
                }
            }

            // 2. Materialize inputs.
            let mut inputs = Inputs::new();
            let mut input_err = None;
            for i in &current.inputs {
                match self.store.cas.get(&i.digest) {
                    Ok(bytes) => {
                        inputs.insert(i.path.clone(), bytes);
                    }
                    Err(e) => {
                        input_err = Some(e);
                        break;
                    }
                }
            }
            if let Some(e) = input_err {
                let remedy = self.healer.on_cas_error(&current, &e, attempt);
                report.heal_events.push(HealEvent {
                    action: current.name.clone(),
                    failure: e.to_string(),
                    remedy: remedy.clone(),
                });
                if let Remedy::EvictAndRetry { attempt: n, .. } = remedy {
                    attempt = n;
                    continue;
                }
                report.failed += 1;
                return Outcome::Failed { key: key.0, error: e.to_string() };
            }

            // 3. Execute.
            match self.executor.run(&current, &inputs) {
                Ok(out) => {
                    let mut result = ActionResult::ok(&current.platform.tag());
                    result.stderr = out.stderr;
                    for (path, bytes) in &out.outputs {
                        match self.store.cas.put(bytes) {
                            Ok(d) => {
                                produced.insert(path.clone(), d.clone());
                                result.outputs.insert(path.clone(), d);
                            }
                            Err(e) => {
                                report.failed += 1;
                                return Outcome::Failed { key: key.0, error: e.to_string() };
                            }
                        }
                    }
                    // A shared store refuses claims from unverified toolchains.
                    // The action still ran and its outputs are still in the CAS
                    // for downstream actions; what is withheld is the claim that
                    // *another machine* may reuse this result.
                    if self.store.may_publish(&current) {
                        let _ = self.store.actions.put(&key, &result);
                    }
                    report.work_done += current.cost;

                    return if attempt == 0 {
                        report.built += 1;
                        Outcome::Built {
                            key: key.0,
                            worker: self.executor.name().to_string(),
                        }
                    } else {
                        report.healed += 1;
                        Outcome::Healed {
                            key: key.0,
                            worker: self.executor.name().to_string(),
                            attempts: attempt + 1,
                        }
                    };
                }

                Err(err) => {
                    let remedy = self.healer.on_exec_error(&current, &err, attempt);
                    report.heal_events.push(HealEvent {
                        action: current.name.clone(),
                        failure: err.to_string(),
                        remedy: remedy.clone(),
                    });
                    match remedy {
                        Remedy::Retry { attempt: n, .. } => {
                            attempt = n;
                            continue;
                        }
                        Remedy::Substitute { attempt: n, .. } => {
                            // Re-key: the relaxed action caches separately.
                            current = relax_platform(&current);
                            attempt = n;
                            continue;
                        }
                        Remedy::EvictAndRetry { attempt: n, blob } => {
                            let _ = self.store.cas.evict(&Digest(blob));
                            attempt = n;
                            continue;
                        }
                        Remedy::Escalate { reason } => {
                            report.failed += 1;
                            return Outcome::Failed { key: key.0, error: reason };
                        }
                    }
                }
            }
        }
    }

    /// A cache hit is only a hit if every promised blob is present and verifies.
    fn verify_and_adopt(
        &self,
        cached: &ActionResult,
        action: &Action,
        produced: &mut BTreeMap<String, Digest>,
    ) -> Result<(), CasError> {
        for out in &action.outputs {
            let d = cached
                .outputs
                .get(out)
                .ok_or_else(|| CasError::Missing(Digest(format!("<output {out}>"))))?;
            // Reading is what verifies: `Cas::get` rehashes.
            self.store.cas.get(d)?;
        }
        for (path, d) in &cached.outputs {
            produced.insert(path.clone(), d.clone());
        }
        Ok(())
    }
}
