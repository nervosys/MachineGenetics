//! The action DAG.
//!
//! Mirrors the task-DAG machinery the compiler already uses for swarm work
//! (`prototype/src/decompose.rs`: topological order, parallel waves, critical
//! path, capability-based assignment) rather than inventing a second scheduler
//! vocabulary. An agent that can reason about one can reason about the other.
//!
//! The one addition here is that edges are **derived, not declared**. A caller
//! states which action produces which logical output; dependency edges are then
//! computed from what each action consumes. Hand-declared edges are the standard
//! way a build graph goes subtly wrong — an edge you forgot is a race, an edge
//! you added needlessly is lost parallelism, and neither shows up until the
//! build is large. Deriving them makes both impossible by construction.

use super::Action;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Index into [`ActionGraph::actions`].
pub type ActionId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A dependency cycle, reported with one participating action for the agent
    /// to start from.
    Cycle { involving: String },
    /// Two actions promise the same logical output. Whichever ran last would
    /// win, non-deterministically — so this is rejected rather than resolved.
    DuplicateOutput { path: String, first: String, second: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Cycle { involving } => {
                write!(f, "dependency cycle involving action `{involving}`")
            }
            GraphError::DuplicateOutput { path, first, second } => write!(
                f,
                "output `{path}` is produced by both `{first}` and `{second}`"
            ),
        }
    }
}

impl std::error::Error for GraphError {}

/// A set of actions plus the dependency edges derived from their inputs.
#[derive(Debug, Default)]
pub struct ActionGraph {
    pub actions: Vec<Action>,
    /// `deps[i]` = actions that must complete before `i`.
    deps: Vec<BTreeSet<ActionId>>,
    /// logical output path -> producing action.
    producers: BTreeMap<String, ActionId>,
}

impl ActionGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an action, deriving its dependencies from inputs already claimed by
    /// other actions' outputs. Inputs nobody produces are *source* inputs — they
    /// exist already, and contribute their digest to the key but no edge.
    pub fn add(&mut self, action: Action) -> Result<ActionId, GraphError> {
        let id = self.actions.len();

        for out in &action.outputs {
            if let Some(&prev) = self.producers.get(out) {
                return Err(GraphError::DuplicateOutput {
                    path: out.clone(),
                    first: self.actions[prev].name.clone(),
                    second: action.name.clone(),
                });
            }
        }

        let mut d = BTreeSet::new();
        for input in &action.inputs {
            if let Some(&producer) = self.producers.get(&input.path) {
                d.insert(producer);
            }
        }

        for out in &action.outputs {
            self.producers.insert(out.clone(), id);
        }
        self.deps.push(d);
        self.actions.push(action);
        Ok(id)
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn deps_of(&self, id: ActionId) -> &BTreeSet<ActionId> {
        &self.deps[id]
    }

    /// Actions in an order where every dependency precedes its dependents.
    pub fn topological_order(&self) -> Result<Vec<ActionId>, GraphError> {
        let mut indegree = vec![0usize; self.len()];
        for (id, d) in self.deps.iter().enumerate() {
            indegree[id] = d.len();
        }

        let mut queue: VecDeque<ActionId> =
            (0..self.len()).filter(|&i| indegree[i] == 0).collect();
        let mut order = Vec::with_capacity(self.len());

        while let Some(id) = queue.pop_front() {
            order.push(id);
            for (other, d) in self.deps.iter().enumerate() {
                if d.contains(&id) {
                    indegree[other] -= 1;
                    if indegree[other] == 0 {
                        queue.push_back(other);
                    }
                }
            }
        }

        if order.len() != self.len() {
            let stuck = (0..self.len())
                .find(|i| !order.contains(i))
                .map(|i| self.actions[i].name.clone())
                .unwrap_or_default();
            return Err(GraphError::Cycle { involving: stuck });
        }
        Ok(order)
    }

    /// Actions grouped into waves: everything in wave *n* may run concurrently,
    /// and depends only on waves `< n`.
    ///
    /// Waves are a coarser schedule than strictly necessary — a true dataflow
    /// scheduler starts an action the moment *its own* dependencies finish
    /// rather than waiting for the whole wave. The scheduler exploits that (see
    /// [`super::sched`]); waves remain the unit an agent *reasons* about,
    /// because "what can run now" is a question with a simple answer.
    pub fn parallel_waves(&self) -> Result<Vec<Vec<ActionId>>, GraphError> {
        let order = self.topological_order()?;
        let mut depth = vec![0usize; self.len()];
        for &id in &order {
            let d = self.deps[id].iter().map(|&p| depth[p] + 1).max().unwrap_or(0);
            depth[id] = d;
        }
        let max = depth.iter().copied().max().map(|m| m + 1).unwrap_or(0);
        let mut waves = vec![Vec::new(); max];
        for id in 0..self.len() {
            waves[depth[id]].push(id);
        }
        Ok(waves)
    }

    /// The longest dependency chain by accumulated `cost` — the lower bound on
    /// build time with unlimited workers, and therefore the only part of the
    /// graph where optimization effort pays.
    pub fn critical_path(&self) -> Result<(u64, Vec<ActionId>), GraphError> {
        let order = self.topological_order()?;
        let mut best = vec![0u64; self.len()];
        let mut prev = vec![None; self.len()];

        for &id in &order {
            let mut base = 0;
            for &p in &self.deps[id] {
                if best[p] > base {
                    base = best[p];
                    prev[id] = Some(p);
                }
            }
            best[id] = base + self.actions[id].cost;
        }

        let Some(end) = (0..self.len()).max_by_key(|&i| best[i]) else {
            return Ok((0, Vec::new()));
        };
        let mut path = vec![end];
        let mut cur = end;
        while let Some(p) = prev[cur] {
            path.push(p);
            cur = p;
        }
        path.reverse();
        Ok((best[end], path))
    }

    /// The plan an agent inspects before committing to a build — the no-exec
    /// view, mirroring `--describe=abl`.
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct NodeView<'a> {
            id: ActionId,
            name: &'a str,
            tool: &'a str,
            platform: String,
            key: String,
            cost: u64,
            deps: Vec<ActionId>,
            inputs: Vec<&'a str>,
            outputs: Vec<&'a str>,
        }
        #[derive(Serialize)]
        struct GraphView<'a> {
            actions: usize,
            waves: Vec<Vec<ActionId>>,
            critical_path_cost: u64,
            critical_path: Vec<ActionId>,
            nodes: Vec<NodeView<'a>>,
        }

        let waves = self.parallel_waves().unwrap_or_default();
        let (cp_cost, cp) = self.critical_path().unwrap_or((0, Vec::new()));
        let nodes = self
            .actions
            .iter()
            .enumerate()
            .map(|(id, a)| NodeView {
                id,
                name: &a.name,
                tool: &a.tool,
                platform: a.platform.tag(),
                key: a.key().0,
                cost: a.cost,
                deps: self.deps[id].iter().copied().collect(),
                inputs: a.inputs.iter().map(|i| i.path.as_str()).collect(),
                outputs: a.outputs.iter().map(|s| s.as_str()).collect(),
            })
            .collect();

        serde_json::to_string_pretty(&GraphView {
            actions: self.len(),
            waves,
            critical_path_cost: cp_cost,
            critical_path: cp,
            nodes,
        })
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Digest;

    fn src(name: &str, out: &str) -> Action {
        Action::new(name, "tool@1").input(format!("{name}.mg"), Digest::of(name.as_bytes())).output(out)
    }

    #[test]
    fn edges_are_derived_from_inputs() {
        let mut g = ActionGraph::new();
        let a = g.add(src("a", "a.abl")).unwrap();
        let b = g
            .add(Action::new("b", "tool@1").input("a.abl", Digest::of(b"x")).output("b.abl"))
            .unwrap();
        assert!(g.deps_of(b).contains(&a), "b consumes a's output, so b depends on a");
        assert!(g.deps_of(a).is_empty());
    }

    #[test]
    fn source_inputs_create_no_edge() {
        let mut g = ActionGraph::new();
        let a = g.add(src("a", "a.abl")).unwrap();
        assert!(g.deps_of(a).is_empty(), "an input nobody produces is a source");
    }

    #[test]
    fn duplicate_outputs_are_rejected() {
        let mut g = ActionGraph::new();
        g.add(src("a", "same.abl")).unwrap();
        let err = g.add(src("b", "same.abl")).unwrap_err();
        assert!(matches!(err, GraphError::DuplicateOutput { .. }));
    }

    #[test]
    fn topological_order_respects_dependencies() {
        let mut g = ActionGraph::new();
        let a = g.add(src("a", "a.abl")).unwrap();
        let b = g
            .add(Action::new("b", "tool@1").input("a.abl", Digest::of(b"x")).output("b.abl"))
            .unwrap();
        let c = g
            .add(Action::new("c", "tool@1").input("b.abl", Digest::of(b"y")).output("c.abl"))
            .unwrap();
        let order = g.topological_order().unwrap();
        let pos = |id| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(a) < pos(b) && pos(b) < pos(c));
    }

    #[test]
    fn independent_actions_share_a_wave() {
        let mut g = ActionGraph::new();
        g.add(src("a", "a.abl")).unwrap();
        g.add(src("b", "b.abl")).unwrap();
        g.add(
            Action::new("link", "tool@1")
                .input("a.abl", Digest::of(b"1"))
                .input("b.abl", Digest::of(b"2"))
                .output("out.abl"),
        )
        .unwrap();
        let waves = g.parallel_waves().unwrap();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0].len(), 2, "a and b are independent");
        assert_eq!(waves[1].len(), 1);
    }

    #[test]
    fn critical_path_follows_cost_not_length() {
        let mut g = ActionGraph::new();
        // cheap chain of two vs one expensive action
        g.add(Action::new("cheap1", "t@1").output("c1").cost(1)).unwrap();
        g.add(Action::new("cheap2", "t@1").input("c1", Digest::of(b"a")).output("c2").cost(1))
            .unwrap();
        let big = g.add(Action::new("expensive", "t@1").output("e").cost(100)).unwrap();
        let (cost, path) = g.critical_path().unwrap();
        assert_eq!(cost, 100);
        assert_eq!(path, vec![big], "the expensive single action dominates");
    }

    #[test]
    fn cycles_are_detected() {
        // Built by hand: derived edges cannot produce a cycle, but a caller
        // constructing a graph programmatically can still create one, and the
        // scheduler must refuse rather than hang.
        let mut g = ActionGraph::new();
        g.add(Action::new("a", "t@1").output("x")).unwrap();
        g.add(Action::new("b", "t@1").input("x", Digest::of(b"1")).output("y")).unwrap();
        g.deps[0].insert(1); // a now depends on b as well
        let err = g.topological_order().unwrap_err();
        assert!(matches!(err, GraphError::Cycle { .. }));
    }

    #[test]
    fn empty_graph_is_well_defined() {
        let g = ActionGraph::new();
        assert_eq!(g.topological_order().unwrap(), Vec::<ActionId>::new());
        assert_eq!(g.critical_path().unwrap(), (0, Vec::new()));
        assert!(g.parallel_waves().unwrap().is_empty());
    }

    #[test]
    fn json_plan_is_machine_readable() {
        let mut g = ActionGraph::new();
        g.add(src("a", "a.abl")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&g.to_json()).unwrap();
        assert_eq!(v["actions"], 1);
        assert!(v["nodes"][0]["key"].as_str().unwrap().len() == 64, "keys are exposed to agents");
    }
}
