// ── Task Decomposition Engine ──────────────────────────────────────
//
// Dependency-aware parallel work splitting for multi-agent workflows.
//
// Features:
//   - Task DAG with dependency edges
//   - Critical path computation
//   - Topological ordering for execution waves
//   - Agent assignment with capacity constraints
//   - Parallel wave extraction (all tasks whose deps are satisfied)

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;

// ── IDs ────────────────────────────────────────────────────────────

pub type TaskId = u64;
pub type AgentId = String;

// ── Task ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    /// Estimated cost in abstract units.
    pub cost: u64,
    /// Required capabilities (agent must have all of these).
    pub required_capabilities: BTreeSet<String>,
    /// Task state.
    pub state: TaskState,
    /// Assigned agent (filled during scheduling).
    pub assigned_to: Option<AgentId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Ready,
    InProgress,
    Completed,
    Blocked,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskState::Pending => write!(f, "Pending"),
            TaskState::Ready => write!(f, "Ready"),
            TaskState::InProgress => write!(f, "InProgress"),
            TaskState::Completed => write!(f, "Completed"),
            TaskState::Blocked => write!(f, "Blocked"),
        }
    }
}

// ── Agent Descriptor ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentDescriptor {
    pub id: AgentId,
    pub capabilities: BTreeSet<String>,
    /// Max tasks this agent can handle concurrently.
    pub capacity: usize,
}

// ── Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecompError {
    TaskNotFound(TaskId),
    CyclicDependency(Vec<TaskId>),
    DuplicateEdge(TaskId, TaskId),
    NoCapableAgent(TaskId),
}

impl fmt::Display for DecompError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecompError::TaskNotFound(id) => write!(f, "task {id} not found"),
            DecompError::CyclicDependency(ids) => write!(f, "cycle: {:?}", ids),
            DecompError::DuplicateEdge(a, b) => write!(f, "duplicate edge {a} → {b}"),
            DecompError::NoCapableAgent(id) => write!(f, "no capable agent for task {id}"),
        }
    }
}

// ── Task DAG ───────────────────────────────────────────────────────

pub struct TaskDag {
    tasks: BTreeMap<TaskId, Task>,
    /// Adjacency: task → set of tasks it depends on (predecessors).
    deps: BTreeMap<TaskId, BTreeSet<TaskId>>,
    /// Reverse adjacency: task → set of tasks that depend on it.
    rdeps: BTreeMap<TaskId, BTreeSet<TaskId>>,
    next_id: TaskId,
}

impl TaskDag {
    pub fn new() -> Self {
        Self { tasks: BTreeMap::new(), deps: BTreeMap::new(), rdeps: BTreeMap::new(), next_id: 1 }
    }

    /// Add a task. Returns its ID.
    pub fn add_task(&mut self, name: impl Into<String>, cost: u64, caps: &[&str]) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        let task = Task {
            id,
            name: name.into(),
            cost,
            required_capabilities: caps.iter().map(|s| s.to_string()).collect(),
            state: TaskState::Pending,
            assigned_to: None,
        };
        self.tasks.insert(id, task);
        self.deps.insert(id, BTreeSet::new());
        self.rdeps.insert(id, BTreeSet::new());
        id
    }

    /// Add a dependency edge: `task` depends on `dependency`.
    pub fn add_dep(&mut self, task: TaskId, dependency: TaskId) -> Result<(), DecompError> {
        if !self.tasks.contains_key(&task) {
            return Err(DecompError::TaskNotFound(task));
        }
        if !self.tasks.contains_key(&dependency) {
            return Err(DecompError::TaskNotFound(dependency));
        }
        let deps = self.deps.entry(task).or_default();
        if !deps.insert(dependency) {
            return Err(DecompError::DuplicateEdge(task, dependency));
        }
        self.rdeps.entry(dependency).or_default().insert(task);
        Ok(())
    }

    /// Check for cycles using Kahn's algorithm.
    pub fn has_cycle(&self) -> bool {
        self.topological_order().is_err()
    }

    /// Topological sort (Kahn's). Returns Err with a cycle hint on failure.
    pub fn topological_order(&self) -> Result<Vec<TaskId>, DecompError> {
        let mut in_degree: BTreeMap<TaskId, usize> = BTreeMap::new();
        for (id, deps) in &self.deps {
            in_degree.entry(*id).or_insert(0);
            *in_degree.entry(*id).or_default() = deps.len();
        }

        let mut queue: VecDeque<TaskId> =
            in_degree.iter().filter(|(_, d)| **d == 0).map(|(id, _)| *id).collect();

        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(dependents) = self.rdeps.get(&id) {
                for dep in dependents {
                    if let Some(d) = in_degree.get_mut(dep) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(*dep);
                        }
                    }
                }
            }
        }

        if order.len() == self.tasks.len() {
            Ok(order)
        } else {
            // Find a node still with non-zero in-degree for the error.
            let remaining: Vec<TaskId> =
                in_degree.iter().filter(|(_, d)| **d > 0).map(|(id, _)| *id).collect();
            Err(DecompError::CyclicDependency(remaining))
        }
    }

    /// Extract parallel waves: groups of tasks that can run concurrently.
    /// Each wave contains tasks whose dependencies are all in earlier waves.
    pub fn parallel_waves(&self) -> Result<Vec<Vec<TaskId>>, DecompError> {
        let mut in_degree: BTreeMap<TaskId, usize> = BTreeMap::new();
        for (id, deps) in &self.deps {
            in_degree.insert(*id, deps.len());
        }

        let mut waves = Vec::new();
        let mut remaining = self.tasks.len();

        while remaining > 0 {
            let wave: Vec<TaskId> =
                in_degree.iter().filter(|(_, d)| **d == 0).map(|(id, _)| *id).collect();

            if wave.is_empty() {
                let stuck: Vec<TaskId> = in_degree.keys().copied().collect();
                return Err(DecompError::CyclicDependency(stuck));
            }

            for id in &wave {
                in_degree.remove(id);
                if let Some(dependents) = self.rdeps.get(id) {
                    for dep in dependents {
                        if let Some(d) = in_degree.get_mut(dep) {
                            *d -= 1;
                        }
                    }
                }
            }

            remaining -= wave.len();
            waves.push(wave);
        }

        Ok(waves)
    }

    /// Critical path: longest path through the DAG by cost.
    /// Returns (total_cost, path_of_task_ids).
    pub fn critical_path(&self) -> Result<(u64, Vec<TaskId>), DecompError> {
        let order = self.topological_order()?;
        // dist[id] = (longest_cost_to_reach, predecessor)
        let mut dist: HashMap<TaskId, (u64, Option<TaskId>)> = HashMap::new();
        for id in &order {
            let task_cost = self.tasks[id].cost;
            let deps = &self.deps[id];
            if deps.is_empty() {
                dist.insert(*id, (task_cost, None));
            } else {
                let (best_dep, best_cost) =
                    deps.iter().map(|d| (*d, dist[d].0)).max_by_key(|(_, c)| *c).unwrap();
                dist.insert(*id, (best_cost + task_cost, Some(best_dep)));
            }
        }

        // Find the task with max distance.
        let (end, (total, _)) = dist.iter().max_by_key(|(_, (c, _))| c).unwrap();
        let end = *end;
        let total = *total;

        // Trace back.
        let mut path = vec![end];
        let mut cur = end;
        while let Some(prev) = dist[&cur].1 {
            path.push(prev);
            cur = prev;
        }
        path.reverse();

        Ok((total, path))
    }

    /// Assign agents to ready tasks. Greedy: first capable agent with capacity.
    pub fn assign_agents(
        &mut self,
        agents: &[AgentDescriptor],
    ) -> Result<Vec<(TaskId, AgentId)>, DecompError> {
        self.update_readiness();

        let mut load: HashMap<AgentId, usize> = HashMap::new();
        for a in agents {
            load.insert(a.id.clone(), 0);
        }

        let mut assignments = Vec::new();
        let ready: Vec<TaskId> =
            self.tasks.values().filter(|t| t.state == TaskState::Ready).map(|t| t.id).collect();

        for tid in ready {
            let task = &self.tasks[&tid];
            let mut assigned = false;
            for agent in agents {
                if task.required_capabilities.is_subset(&agent.capabilities) {
                    let current = load.get(&agent.id).copied().unwrap_or(0);
                    if current < agent.capacity {
                        *load.entry(agent.id.clone()).or_default() += 1;
                        assignments.push((tid, agent.id.clone()));
                        if let Some(t) = self.tasks.get_mut(&tid) {
                            t.assigned_to = Some(agent.id.clone());
                            t.state = TaskState::InProgress;
                        }
                        assigned = true;
                        break;
                    }
                }
            }
            if !assigned {
                return Err(DecompError::NoCapableAgent(tid));
            }
        }

        Ok(assignments)
    }

    /// Mark a task as completed and update readiness of dependents.
    pub fn complete_task(&mut self, id: TaskId) -> Result<(), DecompError> {
        let task = self.tasks.get_mut(&id).ok_or(DecompError::TaskNotFound(id))?;
        task.state = TaskState::Completed;
        self.update_readiness();
        Ok(())
    }

    /// Get a task by ID.
    pub fn get_task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// Number of tasks.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// JSON snapshot.
    pub fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for t in self.tasks.values() {
            let assigned = match &t.assigned_to {
                Some(a) => format!("\"{}\"", a),
                None => "null".into(),
            };
            let deps: Vec<String> = self.deps[&t.id].iter().map(|d| d.to_string()).collect();
            entries.push(format!(
                "{{\"id\":{},\"name\":\"{}\",\"cost\":{},\"state\":\"{}\",\"assigned\":{},\"deps\":[{}]}}",
                t.id, t.name, t.cost, t.state, assigned, deps.join(",")
            ));
        }
        format!("[{}]", entries.join(","))
    }

    // ── Internal ──────────────────────────────────────────────────

    fn update_readiness(&mut self) {
        let completed: BTreeSet<TaskId> =
            self.tasks.values().filter(|t| t.state == TaskState::Completed).map(|t| t.id).collect();

        // Collect state updates to avoid borrow conflict.
        let updates: Vec<(TaskId, TaskState)> = self
            .deps
            .iter()
            .filter_map(|(id, deps)| {
                let task = &self.tasks[id];
                if task.state == TaskState::Pending || task.state == TaskState::Blocked {
                    if deps.iter().all(|d| completed.contains(d)) {
                        Some((*id, TaskState::Ready))
                    } else {
                        Some((*id, TaskState::Blocked))
                    }
                } else {
                    None
                }
            })
            .collect();

        for (id, state) in updates {
            self.tasks.get_mut(&id).unwrap().state = state;
        }

        // Tasks with no deps that are still Pending become Ready.
        let no_dep_ids: Vec<TaskId> =
            self.deps.iter().filter(|(_, deps)| deps.is_empty()).map(|(id, _)| *id).collect();
        for id in no_dep_ids {
            let task = self.tasks.get_mut(&id).unwrap();
            if task.state == TaskState::Pending {
                task.state = TaskState::Ready;
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dag() -> TaskDag {
        TaskDag::new()
    }

    fn agent(id: &str, caps: &[&str], capacity: usize) -> AgentDescriptor {
        AgentDescriptor {
            id: id.into(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            capacity,
        }
    }

    // ── Basic construction ────────────────────────────────────────

    #[test]
    fn add_tasks_and_deps() {
        let mut d = dag();
        let a = d.add_task("parse", 5, &[]);
        let b = d.add_task("typecheck", 10, &[]);
        d.add_dep(b, a).unwrap();
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn duplicate_edge_error() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        d.add_dep(b, a).unwrap();
        let err = d.add_dep(b, a).unwrap_err();
        assert!(matches!(err, DecompError::DuplicateEdge(_, _)));
    }

    #[test]
    fn dep_on_missing_task() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let err = d.add_dep(a, 999).unwrap_err();
        assert!(matches!(err, DecompError::TaskNotFound(999)));
    }

    // ── Topological order ─────────────────────────────────────────

    #[test]
    fn topo_order_linear() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        let c = d.add_task("c", 1, &[]);
        d.add_dep(b, a).unwrap();
        d.add_dep(c, b).unwrap();
        let order = d.topological_order().unwrap();
        assert_eq!(order, vec![a, b, c]);
    }

    #[test]
    fn cycle_detected() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        d.add_dep(b, a).unwrap();
        d.add_dep(a, b).unwrap();
        assert!(d.has_cycle());
    }

    // ── Parallel waves ────────────────────────────────────────────

    #[test]
    fn parallel_waves_diamond() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        let c = d.add_task("c", 1, &[]);
        let e = d.add_task("d", 1, &[]); // merge point
        d.add_dep(b, a).unwrap();
        d.add_dep(c, a).unwrap();
        d.add_dep(e, b).unwrap();
        d.add_dep(e, c).unwrap();

        let waves = d.parallel_waves().unwrap();
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec![a]);
        assert!(waves[1].contains(&b) && waves[1].contains(&c));
        assert_eq!(waves[2], vec![e]);
    }

    #[test]
    fn parallel_waves_independent() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        let c = d.add_task("c", 1, &[]);
        let waves = d.parallel_waves().unwrap();
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].len(), 3);
    }

    // ── Critical path ─────────────────────────────────────────────

    #[test]
    fn critical_path_linear() {
        let mut d = dag();
        let a = d.add_task("a", 3, &[]);
        let b = d.add_task("b", 7, &[]);
        let c = d.add_task("c", 2, &[]);
        d.add_dep(b, a).unwrap();
        d.add_dep(c, b).unwrap();
        let (total, path) = d.critical_path().unwrap();
        assert_eq!(total, 12); // 3 + 7 + 2
        assert_eq!(path, vec![a, b, c]);
    }

    #[test]
    fn critical_path_chooses_longest() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b-short", 2, &[]);
        let c = d.add_task("c-long", 10, &[]);
        let e = d.add_task("merge", 1, &[]);
        d.add_dep(b, a).unwrap();
        d.add_dep(c, a).unwrap();
        d.add_dep(e, b).unwrap();
        d.add_dep(e, c).unwrap();
        let (total, path) = d.critical_path().unwrap();
        assert_eq!(total, 12); // 1 + 10 + 1
        assert_eq!(path, vec![a, c, e]);
    }

    // ── Agent assignment ──────────────────────────────────────────

    #[test]
    fn assign_agents_simple() {
        let mut d = dag();
        let a = d.add_task("parse", 5, &["read"]);
        let b = d.add_task("gen", 5, &["write"]);
        let agents = vec![agent("reader", &["read"], 2), agent("writer", &["write", "read"], 2)];
        let assignments = d.assign_agents(&agents).unwrap();
        assert_eq!(assignments.len(), 2);
        // "parse" requires "read" → assigned to "reader" or "writer"
        assert!(assignments.iter().any(|(id, _)| *id == a));
        assert!(assignments.iter().any(|(id, _)| *id == b));
    }

    #[test]
    fn no_capable_agent_error() {
        let mut d = dag();
        d.add_task("needs-x", 1, &["x"]);
        let agents = vec![agent("a", &["y"], 1)];
        let err = d.assign_agents(&agents).unwrap_err();
        assert!(matches!(err, DecompError::NoCapableAgent(_)));
    }

    #[test]
    fn capacity_limits_assignments() {
        let mut d = dag();
        d.add_task("t1", 1, &[]);
        d.add_task("t2", 1, &[]);
        d.add_task("t3", 1, &[]);
        // Agent with capacity 2 can only take 2 tasks.
        let agents = vec![agent("a", &[], 2), agent("b", &[], 2)];
        let assignments = d.assign_agents(&agents).unwrap();
        assert_eq!(assignments.len(), 3);
        let a_count = assignments.iter().filter(|(_, ag)| ag == "a").count();
        assert!(a_count <= 2);
    }

    // ── Task completion ───────────────────────────────────────────

    #[test]
    fn complete_unblocks_dependents() {
        let mut d = dag();
        let a = d.add_task("a", 1, &[]);
        let b = d.add_task("b", 1, &[]);
        d.add_dep(b, a).unwrap();

        // Initially: a is Ready (no deps), b is Blocked.
        let agents = vec![agent("x", &[], 2)];
        let initial = d.assign_agents(&agents).unwrap();
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].0, a); // only a is ready

        d.complete_task(a).unwrap();
        let task_b = d.get_task(b).unwrap();
        assert_eq!(task_b.state, TaskState::Ready);
    }

    // ── JSON ──────────────────────────────────────────────────────

    #[test]
    fn json_output() {
        let mut d = dag();
        d.add_task("foo", 5, &["cap"]);
        let json = d.to_json();
        assert!(json.contains("\"name\":\"foo\""));
        assert!(json.contains("\"cost\":5"));
    }

    // ── Single task ───────────────────────────────────────────────

    #[test]
    fn single_task_critical_path() {
        let mut d = dag();
        let a = d.add_task("only", 42, &[]);
        let (total, path) = d.critical_path().unwrap();
        assert_eq!(total, 42);
        assert_eq!(path, vec![a]);
    }
}
