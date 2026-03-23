//! # Redox Task Decomposition Engine
//!
//! Dependency-aware parallel work splitting with DAG scheduling.
//! Enforces acyclicity in the assignment graph (no cycles allowed).
//!
//! Core concepts:
//! - **TaskNode**: A unit of work with typed dependencies
//! - **DependencyGraph**: A DAG of TaskNodes with cycle detection
//! - **Decomposer**: Splits a high-level task into subtasks along region boundaries
//! - **Scheduler**: Topological sort → parallel phases → critical path
//! - **AssignmentGraph**: Maps agents to tasks, enforces no cycles

use std::collections::{BTreeMap, BTreeSet, VecDeque};

// ── Task Nodes ──────────────────────────────────────────────────────────────

/// Unique identifier for a task node in the decomposition graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The kind of work a task represents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskKind {
    /// Modify implementation within a single region.
    ImplementRegion,
    /// Modify a shared interface (requires consensus).
    ModifyInterface,
    /// Add new code (module, function, type).
    AddCode,
    /// Remove existing code.
    RemoveCode,
    /// Refactor/restructure without semantic change.
    Refactor,
    /// Verification/testing task.
    Verify,
    /// Integration task (merge results).
    Integrate,
    /// Custom task kind.
    Custom(String),
}

/// The priority level for scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Estimated cost of a task (abstract units for scheduling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cost(pub u64);

impl Default for Cost {
    fn default() -> Self {
        Cost(1)
    }
}

/// A node in the task decomposition graph.
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: TaskId,
    pub description: String,
    pub kind: TaskKind,
    pub region: Option<String>,
    pub priority: Priority,
    pub cost: Cost,
    pub parallelizable: bool,
}

impl TaskNode {
    pub fn new(id: &str, description: &str, kind: TaskKind) -> Self {
        Self {
            id: TaskId::new(id),
            description: description.to_string(),
            kind,
            region: None,
            priority: Priority::default(),
            cost: Cost::default(),
            parallelizable: true,
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.region = Some(region.to_string());
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_cost(mut self, cost: u64) -> Self {
        self.cost = Cost(cost);
        self
    }

    pub fn sequential(mut self) -> Self {
        self.parallelizable = false;
        self
    }
}

// ── Dependency Graph (DAG) ──────────────────────────────────────────────────

/// Errors from the dependency graph and decomposition engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecompositionError {
    /// A cycle was detected in the dependency graph.
    CycleDetected(Vec<TaskId>),
    /// Referenced task does not exist.
    TaskNotFound(TaskId),
    /// Duplicate task ID.
    DuplicateTask(TaskId),
    /// Dependency references a nonexistent task.
    DanglingDependency { from: TaskId, to: TaskId },
    /// Agent assigned to tasks that form a cycle.
    AssignmentCycle(Vec<String>),
}

impl std::fmt::Display for DecompositionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecompositionError::CycleDetected(ids) => {
                let path: Vec<_> = ids.iter().map(|id| id.0.as_str()).collect();
                write!(f, "Cycle detected: {}", path.join(" -> "))
            }
            DecompositionError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            DecompositionError::DuplicateTask(id) => write!(f, "Duplicate task: {}", id),
            DecompositionError::DanglingDependency { from, to } => {
                write!(f, "Dangling dependency: {} -> {}", from, to)
            }
            DecompositionError::AssignmentCycle(agents) => {
                write!(f, "Assignment cycle among agents: {}", agents.join(" -> "))
            }
        }
    }
}

/// A directed acyclic graph of task dependencies.
///
/// Edges point from dependency to dependent (if A must finish before B,
/// edge A -> B exists). Enforces acyclicity on every mutation.
pub struct DependencyGraph {
    nodes: BTreeMap<TaskId, TaskNode>,
    /// Forward edges: task -> set of tasks that depend on it.
    forward: BTreeMap<TaskId, BTreeSet<TaskId>>,
    /// Reverse edges: task -> set of tasks it depends on.
    reverse: BTreeMap<TaskId, BTreeSet<TaskId>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self { nodes: BTreeMap::new(), forward: BTreeMap::new(), reverse: BTreeMap::new() }
    }

    /// Add a task node to the graph.
    pub fn add_task(&mut self, node: TaskNode) -> Result<(), DecompositionError> {
        if self.nodes.contains_key(&node.id) {
            return Err(DecompositionError::DuplicateTask(node.id.clone()));
        }
        let id = node.id.clone();
        self.nodes.insert(id.clone(), node);
        self.forward.entry(id.clone()).or_default();
        self.reverse.entry(id).or_default();
        Ok(())
    }

    /// Add a dependency edge: `dependency` must complete before `dependent`.
    /// Returns error if the edge would create a cycle.
    pub fn add_dependency(
        &mut self,
        dependency: &TaskId,
        dependent: &TaskId,
    ) -> Result<(), DecompositionError> {
        if !self.nodes.contains_key(dependency) {
            return Err(DecompositionError::TaskNotFound(dependency.clone()));
        }
        if !self.nodes.contains_key(dependent) {
            return Err(DecompositionError::TaskNotFound(dependent.clone()));
        }

        // Check if adding this edge would create a cycle:
        // If there's already a path from dependent -> dependency, adding
        // dependency -> dependent would create a cycle.
        if dependency == dependent || self.has_path(dependent, dependency) {
            let mut cycle = vec![dependency.clone(), dependent.clone(), dependency.clone()];
            if dependency == dependent {
                cycle = vec![dependency.clone(), dependency.clone()];
            }
            return Err(DecompositionError::CycleDetected(cycle));
        }

        self.forward.entry(dependency.clone()).or_default().insert(dependent.clone());
        self.reverse.entry(dependent.clone()).or_default().insert(dependency.clone());
        Ok(())
    }

    /// Check if there's a path from `from` to `to` via BFS.
    fn has_path(&self, from: &TaskId, to: &TaskId) -> bool {
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from.clone());
        visited.insert(from.clone());

        while let Some(current) = queue.pop_front() {
            if &current == to {
                return true;
            }
            if let Some(neighbors) = self.forward.get(&current) {
                for next in neighbors {
                    if visited.insert(next.clone()) {
                        queue.push_back(next.clone());
                    }
                }
            }
        }
        false
    }

    /// Get a task node by ID.
    pub fn get_task(&self, id: &TaskId) -> Option<&TaskNode> {
        self.nodes.get(id)
    }

    /// Get all task IDs in the graph.
    pub fn task_ids(&self) -> Vec<&TaskId> {
        self.nodes.keys().collect()
    }

    /// Number of tasks in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get the direct dependencies of a task (what it depends on).
    pub fn dependencies_of(&self, id: &TaskId) -> Vec<&TaskId> {
        self.reverse.get(id).map(|set| set.iter().collect()).unwrap_or_default()
    }

    /// Get the direct dependents of a task (what depends on it).
    pub fn dependents_of(&self, id: &TaskId) -> Vec<&TaskId> {
        self.forward.get(id).map(|set| set.iter().collect()).unwrap_or_default()
    }

    /// Get root tasks (no dependencies — can start immediately).
    pub fn roots(&self) -> Vec<&TaskId> {
        self.reverse.iter().filter(|(_, deps)| deps.is_empty()).map(|(id, _)| id).collect()
    }

    /// Get leaf tasks (nothing depends on them).
    pub fn leaves(&self) -> Vec<&TaskId> {
        self.forward.iter().filter(|(_, deps)| deps.is_empty()).map(|(id, _)| id).collect()
    }

    /// Validate the entire graph: check for dangling deps and cycles.
    pub fn validate(&self) -> Result<(), DecompositionError> {
        // Check for dangling references (should not happen if using add_dependency, but defensive)
        for (from, targets) in &self.forward {
            for to in targets {
                if !self.nodes.contains_key(to) {
                    return Err(DecompositionError::DanglingDependency {
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }

        // Check no cycles via topological sort
        self.topological_sort()?;
        Ok(())
    }

    /// Topological sort using Kahn's algorithm. Returns error if a cycle exists.
    pub fn topological_sort(&self) -> Result<Vec<TaskId>, DecompositionError> {
        let mut in_degree: BTreeMap<TaskId, usize> = BTreeMap::new();
        for id in self.nodes.keys() {
            in_degree.insert(id.clone(), 0);
        }
        for (_, targets) in &self.forward {
            for t in targets {
                *in_degree.entry(t.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<TaskId> =
            in_degree.iter().filter(|(_, deg)| **deg == 0).map(|(id, _)| id.clone()).collect();

        let mut sorted = Vec::new();

        while let Some(current) = queue.pop_front() {
            sorted.push(current.clone());
            if let Some(neighbors) = self.forward.get(&current) {
                for next in neighbors {
                    if let Some(deg) = in_degree.get_mut(next) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(next.clone());
                        }
                    }
                }
            }
        }

        if sorted.len() != self.nodes.len() {
            // Find cycle participants
            let cycle_nodes: Vec<TaskId> =
                in_degree.iter().filter(|(_, deg)| **deg > 0).map(|(id, _)| id.clone()).collect();
            return Err(DecompositionError::CycleDetected(cycle_nodes));
        }

        Ok(sorted)
    }

    /// Total number of edges.
    pub fn edge_count(&self) -> usize {
        self.forward.values().map(|s| s.len()).sum()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ── Scheduler ───────────────────────────────────────────────────────────────

/// A phase of tasks that can execute in parallel.
#[derive(Debug, Clone)]
pub struct Phase {
    pub index: usize,
    pub task_ids: Vec<TaskId>,
}

impl Phase {
    pub fn len(&self) -> usize {
        self.task_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.task_ids.is_empty()
    }
}

/// A schedule: an ordered list of parallel phases.
#[derive(Debug, Clone)]
pub struct Schedule {
    pub phases: Vec<Phase>,
}

impl Schedule {
    /// Total number of sequential phases.
    pub fn depth(&self) -> usize {
        self.phases.len()
    }

    /// Maximum parallelism (widest phase).
    pub fn max_parallelism(&self) -> usize {
        self.phases.iter().map(|p| p.len()).max().unwrap_or(0)
    }

    /// Total number of tasks across all phases.
    pub fn total_tasks(&self) -> usize {
        self.phases.iter().map(|p| p.len()).sum()
    }
}

/// The critical path through the DAG (longest chain of dependent tasks).
#[derive(Debug, Clone)]
pub struct CriticalPath {
    pub tasks: Vec<TaskId>,
    pub total_cost: u64,
}

/// The DAG scheduler: computes parallel phases and critical path.
pub struct Scheduler;

impl Scheduler {
    /// Compute the parallel execution schedule from a dependency graph.
    /// Tasks are grouped into phases where all tasks within a phase
    /// can execute in parallel (all dependencies in earlier phases).
    pub fn schedule(graph: &DependencyGraph) -> Result<Schedule, DecompositionError> {
        let sorted = graph.topological_sort()?;

        // Assign each task to the earliest possible phase
        let mut task_phase: BTreeMap<TaskId, usize> = BTreeMap::new();
        for id in &sorted {
            let deps = graph.dependencies_of(id);
            let phase = if deps.is_empty() {
                0
            } else {
                deps.iter().map(|d| task_phase.get(*d).copied().unwrap_or(0) + 1).max().unwrap_or(0)
            };
            task_phase.insert(id.clone(), phase);
        }

        // Group into phases
        let max_phase = task_phase.values().copied().max().unwrap_or(0);
        let mut phases = Vec::new();
        for i in 0..=max_phase {
            let task_ids: Vec<TaskId> =
                sorted.iter().filter(|id| task_phase.get(*id) == Some(&i)).cloned().collect();
            if !task_ids.is_empty() {
                phases.push(Phase { index: i, task_ids });
            }
        }

        Ok(Schedule { phases })
    }

    /// Compute the critical path (longest cost chain through the DAG).
    pub fn critical_path(graph: &DependencyGraph) -> Result<CriticalPath, DecompositionError> {
        let sorted = graph.topological_sort()?;

        if sorted.is_empty() {
            return Ok(CriticalPath { tasks: Vec::new(), total_cost: 0 });
        }

        // For each task, compute longest path ending at that task
        let mut longest_cost: BTreeMap<TaskId, u64> = BTreeMap::new();
        let mut predecessor: BTreeMap<TaskId, Option<TaskId>> = BTreeMap::new();

        for id in &sorted {
            let node_cost = graph.get_task(id).map(|n| n.cost.0).unwrap_or(1);
            let deps = graph.dependencies_of(id);

            if deps.is_empty() {
                longest_cost.insert(id.clone(), node_cost);
                predecessor.insert(id.clone(), None);
            } else {
                let (best_pred, best_cost) = deps
                    .iter()
                    .map(|d| (*d, longest_cost.get(*d).copied().unwrap_or(0)))
                    .max_by_key(|&(_, c)| c)
                    .unwrap();
                longest_cost.insert(id.clone(), best_cost + node_cost);
                predecessor.insert(id.clone(), Some(best_pred.clone()));
            }
        }

        // Find the task with the longest path
        let (end_task, total_cost) = longest_cost.iter().max_by_key(|(_, c)| *c).unwrap();

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = Some(end_task.clone());
        while let Some(id) = current {
            path.push(id.clone());
            current = predecessor.get(&id).and_then(|p| p.clone());
        }
        path.reverse();

        Ok(CriticalPath { tasks: path, total_cost: *total_cost })
    }
}

// ── Decomposer ──────────────────────────────────────────────────────────────

/// A region of the codebase that can be worked on independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub name: String,
    pub dependencies: Vec<String>,
}

impl Region {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), dependencies: Vec::new() }
    }

    pub fn with_dependency(mut self, dep: &str) -> Self {
        self.dependencies.push(dep.to_string());
        self
    }
}

/// A high-level task description to decompose.
#[derive(Debug, Clone)]
pub struct HighLevelTask {
    pub description: String,
    pub target_regions: Vec<Region>,
    pub requires_consensus: bool,
}

impl HighLevelTask {
    pub fn new(description: &str) -> Self {
        Self {
            description: description.to_string(),
            target_regions: Vec::new(),
            requires_consensus: false,
        }
    }

    pub fn add_region(&mut self, region: Region) {
        self.target_regions.push(region);
    }

    pub fn with_consensus(mut self) -> Self {
        self.requires_consensus = true;
        self
    }
}

/// The result of task decomposition.
pub struct DecompositionPlan {
    pub graph: DependencyGraph,
    pub schedule: Schedule,
    pub critical_path: CriticalPath,
    pub consensus_points: Vec<TaskId>,
}

/// The task decomposer: takes a high-level task and produces a decomposition plan.
pub struct Decomposer;

impl Decomposer {
    /// Decompose a high-level task into a scheduled plan.
    ///
    /// For each target region, creates an implement task.
    /// Respects region dependencies to build the DAG.
    /// If consensus is needed, adds an interface consensus task before all region tasks.
    /// Adds a final integration task that depends on all region tasks.
    pub fn decompose(task: &HighLevelTask) -> Result<DecompositionPlan, DecompositionError> {
        let mut graph = DependencyGraph::new();
        let mut consensus_points = Vec::new();

        // If consensus is required, add a consensus gate at the start
        if task.requires_consensus {
            let consensus_task = TaskNode::new(
                "consensus",
                "Reach consensus on interface changes",
                TaskKind::ModifyInterface,
            )
            .with_priority(Priority::Critical)
            .sequential();
            graph.add_task(consensus_task)?;
            consensus_points.push(TaskId::new("consensus"));
        }

        // Create a task for each target region
        let mut region_task_ids = Vec::new();
        for region in &task.target_regions {
            let id = format!("region_{}", region.name);
            let task_node = TaskNode::new(
                &id,
                &format!("Implement changes in region '{}'", region.name),
                TaskKind::ImplementRegion,
            )
            .with_region(&region.name);
            graph.add_task(task_node)?;

            // If consensus required, region depends on consensus
            if task.requires_consensus {
                graph.add_dependency(&TaskId::new("consensus"), &TaskId::new(&id))?;
            }

            region_task_ids.push((id, region.dependencies.clone()));
        }

        // Add inter-region dependencies
        for (id, deps) in &region_task_ids {
            for dep_name in deps {
                let dep_id = format!("region_{}", dep_name);
                // Only add dependency if the target region task exists
                if region_task_ids.iter().any(|(rid, _)| rid == &dep_id) {
                    graph.add_dependency(&TaskId::new(&dep_id), &TaskId::new(id))?;
                }
            }
        }

        // Add a verification task for each region
        for (id, _) in &region_task_ids {
            let verify_id = format!("verify_{}", &id[7..]); // strip "region_" prefix
            let verify_task = TaskNode::new(
                &verify_id,
                &format!("Verify changes in region '{}'", &id[7..]),
                TaskKind::Verify,
            );
            graph.add_task(verify_task)?;
            graph.add_dependency(&TaskId::new(id), &TaskId::new(&verify_id))?;
        }

        // Add integration task that depends on all verification tasks
        let integrate =
            TaskNode::new("integrate", "Integrate all region changes", TaskKind::Integrate)
                .with_priority(Priority::High)
                .sequential();
        graph.add_task(integrate)?;

        for (id, _) in &region_task_ids {
            let verify_id = format!("verify_{}", &id[7..]);
            graph.add_dependency(&TaskId::new(&verify_id), &TaskId::new("integrate"))?;
        }

        let schedule = Scheduler::schedule(&graph)?;
        let critical_path = Scheduler::critical_path(&graph)?;

        Ok(DecompositionPlan { graph, schedule, critical_path, consensus_points })
    }
}

// ── Assignment Graph ────────────────────────────────────────────────────────

/// An assignment of an agent to a task.
#[derive(Debug, Clone)]
pub struct Assignment {
    pub agent: String,
    pub task_id: TaskId,
}

/// Tracks agent-to-task assignments and enforces no cycles in the
/// assignment dependency graph. A cycle in assignments would mean
/// circular blocking: agent A waits for agent B's task which depends
/// on agent A's task.
pub struct AssignmentGraph {
    assignments: Vec<Assignment>,
    /// agent -> tasks assigned to that agent
    agent_tasks: BTreeMap<String, Vec<TaskId>>,
}

impl AssignmentGraph {
    pub fn new() -> Self {
        Self { assignments: Vec::new(), agent_tasks: BTreeMap::new() }
    }

    /// Assign an agent to a task.
    pub fn assign(&mut self, agent: &str, task_id: TaskId) {
        self.assignments.push(Assignment { agent: agent.to_string(), task_id: task_id.clone() });
        self.agent_tasks.entry(agent.to_string()).or_default().push(task_id);
    }

    /// Verify no cycles exist in the agent-level dependency graph.
    ///
    /// Builds an agent-to-agent dependency graph: agent A depends on agent B
    /// if one of A's tasks depends on one of B's tasks. Checks acyclicity.
    pub fn verify_no_cycles(&self, task_graph: &DependencyGraph) -> Result<(), DecompositionError> {
        // Build task -> agent mapping
        let mut task_owner: BTreeMap<&TaskId, &str> = BTreeMap::new();
        for assignment in &self.assignments {
            task_owner.insert(&assignment.task_id, &assignment.agent);
        }

        // Build agent-to-agent forward edges
        let mut agent_deps: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for (agent, tasks) in &self.agent_tasks {
            for task_id in tasks {
                for dep_id in task_graph.dependencies_of(task_id) {
                    if let Some(&dep_agent) = task_owner.get(dep_id) {
                        if dep_agent != agent.as_str() {
                            agent_deps.entry(agent.as_str()).or_default().insert(dep_agent);
                        }
                    }
                }
            }
        }

        // Topological sort on agent graph (Kahn's algorithm)
        let all_agents: BTreeSet<&str> = self.agent_tasks.keys().map(|s| s.as_str()).collect();
        let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
        for &agent in &all_agents {
            in_degree.insert(agent, 0);
        }
        for (_, deps) in &agent_deps {
            for &dep in deps {
                *in_degree.entry(dep).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<&str> =
            in_degree.iter().filter(|(_, deg)| **deg == 0).map(|(&a, _)| a).collect();
        let mut visited = 0usize;

        while let Some(agent) = queue.pop_front() {
            visited += 1;
            if let Some(deps) = agent_deps.get(agent) {
                for &dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if visited != all_agents.len() {
            let cycle_agents: Vec<String> = in_degree
                .iter()
                .filter(|(_, deg)| **deg > 0)
                .map(|(&a, _)| a.to_string())
                .collect();
            return Err(DecompositionError::AssignmentCycle(cycle_agents));
        }

        Ok(())
    }

    /// Get all assignments.
    pub fn assignments(&self) -> &[Assignment] {
        &self.assignments
    }

    /// Get tasks assigned to a specific agent.
    pub fn tasks_for_agent(&self, agent: &str) -> Vec<&TaskId> {
        self.agent_tasks.get(agent).map(|tasks| tasks.iter().collect()).unwrap_or_default()
    }

    /// Number of agents with assignments.
    pub fn agent_count(&self) -> usize {
        self.agent_tasks.len()
    }
}

impl Default for AssignmentGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TaskNode tests ──

    #[test]
    fn task_node_defaults() {
        let node = TaskNode::new("t1", "do thing", TaskKind::ImplementRegion);
        assert_eq!(node.id, TaskId::new("t1"));
        assert_eq!(node.priority, Priority::Normal);
        assert_eq!(node.cost, Cost(1));
        assert!(node.parallelizable);
        assert!(node.region.is_none());
    }

    #[test]
    fn task_node_builder() {
        let node = TaskNode::new("t1", "do thing", TaskKind::Verify)
            .with_region("core")
            .with_priority(Priority::High)
            .with_cost(5)
            .sequential();
        assert_eq!(node.region.as_deref(), Some("core"));
        assert_eq!(node.priority, Priority::High);
        assert_eq!(node.cost, Cost(5));
        assert!(!node.parallelizable);
    }

    // ── DependencyGraph tests ──

    #[test]
    fn empty_graph() {
        let graph = DependencyGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn add_tasks_and_dependencies() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();

        assert_eq!(graph.len(), 2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.dependencies_of(&TaskId::new("b")), vec![&TaskId::new("a")]);
        assert_eq!(graph.dependents_of(&TaskId::new("a")), vec![&TaskId::new("b")]);
    }

    #[test]
    fn duplicate_task_error() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        let err = graph.add_task(TaskNode::new("a", "A2", TaskKind::AddCode)).unwrap_err();
        assert_eq!(err, DecompositionError::DuplicateTask(TaskId::new("a")));
    }

    #[test]
    fn self_cycle_detected() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        let err = graph.add_dependency(&TaskId::new("a"), &TaskId::new("a")).unwrap_err();
        assert!(matches!(err, DecompositionError::CycleDetected(_)));
    }

    #[test]
    fn two_node_cycle_detected() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        let err = graph.add_dependency(&TaskId::new("b"), &TaskId::new("a")).unwrap_err();
        assert!(matches!(err, DecompositionError::CycleDetected(_)));
    }

    #[test]
    fn three_node_cycle_detected() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();
        let err = graph.add_dependency(&TaskId::new("c"), &TaskId::new("a")).unwrap_err();
        assert!(matches!(err, DecompositionError::CycleDetected(_)));
    }

    #[test]
    fn dependency_on_missing_task() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        let err = graph.add_dependency(&TaskId::new("a"), &TaskId::new("missing")).unwrap_err();
        assert_eq!(err, DecompositionError::TaskNotFound(TaskId::new("missing")));
    }

    #[test]
    fn roots_and_leaves() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();

        assert_eq!(graph.roots(), vec![&TaskId::new("a")]);
        assert_eq!(graph.leaves(), vec![&TaskId::new("c")]);
    }

    #[test]
    fn topological_sort_linear() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();

        let sorted = graph.topological_sort().unwrap();
        let pos_a = sorted.iter().position(|id| id == &TaskId::new("a")).unwrap();
        let pos_b = sorted.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let pos_c = sorted.iter().position(|id| id == &TaskId::new("c")).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn topological_sort_diamond() {
        // a -> b, a -> c, b -> d, c -> d
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("d", "D", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("c")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("d")).unwrap();
        graph.add_dependency(&TaskId::new("c"), &TaskId::new("d")).unwrap();

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted.len(), 4);
        let pos_a = sorted.iter().position(|id| id == &TaskId::new("a")).unwrap();
        let pos_b = sorted.iter().position(|id| id == &TaskId::new("b")).unwrap();
        let pos_c = sorted.iter().position(|id| id == &TaskId::new("c")).unwrap();
        let pos_d = sorted.iter().position(|id| id == &TaskId::new("d")).unwrap();
        assert!(pos_a < pos_b && pos_a < pos_c);
        assert!(pos_b < pos_d && pos_c < pos_d);
    }

    #[test]
    fn validate_ok() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        assert!(graph.validate().is_ok());
    }

    // ── Scheduler tests ──

    #[test]
    fn schedule_linear_chain() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();

        let schedule = Scheduler::schedule(&graph).unwrap();
        assert_eq!(schedule.depth(), 3);
        assert_eq!(schedule.max_parallelism(), 1);
        assert_eq!(schedule.total_tasks(), 3);
    }

    #[test]
    fn schedule_parallel_tasks() {
        // a -> c, b -> c  (a and b can run in parallel)
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::Integrate)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("c")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();

        let schedule = Scheduler::schedule(&graph).unwrap();
        assert_eq!(schedule.depth(), 2);
        assert_eq!(schedule.max_parallelism(), 2);
        // Phase 0: {a, b}, Phase 1: {c}
        assert_eq!(schedule.phases[0].len(), 2);
        assert_eq!(schedule.phases[1].len(), 1);
    }

    #[test]
    fn schedule_diamond() {
        // a -> b, a -> c, b -> d, c -> d
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("d", "D", TaskKind::Integrate)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("c")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("d")).unwrap();
        graph.add_dependency(&TaskId::new("c"), &TaskId::new("d")).unwrap();

        let schedule = Scheduler::schedule(&graph).unwrap();
        // Phase 0: {a}, Phase 1: {b, c}, Phase 2: {d}
        assert_eq!(schedule.depth(), 3);
        assert_eq!(schedule.max_parallelism(), 2);
    }

    #[test]
    fn schedule_empty_graph() {
        let graph = DependencyGraph::new();
        let schedule = Scheduler::schedule(&graph).unwrap();
        assert_eq!(schedule.depth(), 0);
        assert_eq!(schedule.total_tasks(), 0);
    }

    #[test]
    fn schedule_no_deps() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();

        let schedule = Scheduler::schedule(&graph).unwrap();
        assert_eq!(schedule.depth(), 1);
        assert_eq!(schedule.max_parallelism(), 3);
    }

    // ── Critical Path tests ──

    #[test]
    fn critical_path_linear() {
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode).with_cost(3)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode).with_cost(2)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode).with_cost(4)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();

        let cp = Scheduler::critical_path(&graph).unwrap();
        assert_eq!(cp.tasks, vec![TaskId::new("a"), TaskId::new("b"), TaskId::new("c")]);
        assert_eq!(cp.total_cost, 9);
    }

    #[test]
    fn critical_path_diamond() {
        // a(1) -> b(5), a(1) -> c(2), b(5) -> d(1), c(2) -> d(1)
        // Critical path: a -> b -> d = 7
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode).with_cost(1)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode).with_cost(5)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode).with_cost(2)).unwrap();
        graph.add_task(TaskNode::new("d", "D", TaskKind::Integrate).with_cost(1)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("c")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("d")).unwrap();
        graph.add_dependency(&TaskId::new("c"), &TaskId::new("d")).unwrap();

        let cp = Scheduler::critical_path(&graph).unwrap();
        assert_eq!(cp.tasks, vec![TaskId::new("a"), TaskId::new("b"), TaskId::new("d")]);
        assert_eq!(cp.total_cost, 7);
    }

    #[test]
    fn critical_path_empty() {
        let graph = DependencyGraph::new();
        let cp = Scheduler::critical_path(&graph).unwrap();
        assert!(cp.tasks.is_empty());
        assert_eq!(cp.total_cost, 0);
    }

    // ── Decomposer tests ──

    #[test]
    fn decompose_single_region() {
        let mut task = HighLevelTask::new("Fix bug in core");
        task.add_region(Region::new("core"));

        let plan = Decomposer::decompose(&task).unwrap();
        // region_core, verify_core, integrate = 3 tasks
        assert_eq!(plan.graph.len(), 3);
        assert!(!plan.schedule.phases.is_empty());
        assert!(plan.consensus_points.is_empty());
    }

    #[test]
    fn decompose_multiple_independent_regions() {
        let mut task = HighLevelTask::new("Update modules");
        task.add_region(Region::new("alpha"));
        task.add_region(Region::new("beta"));

        let plan = Decomposer::decompose(&task).unwrap();
        // region_alpha, region_beta, verify_alpha, verify_beta, integrate = 5 tasks
        assert_eq!(plan.graph.len(), 5);
        // alpha and beta can run in parallel
        assert!(plan.schedule.max_parallelism() >= 2);
    }

    #[test]
    fn decompose_with_region_deps() {
        let mut task = HighLevelTask::new("Layered update");
        task.add_region(Region::new("base"));
        task.add_region(Region::new("mid").with_dependency("base"));
        task.add_region(Region::new("top").with_dependency("mid"));

        let plan = Decomposer::decompose(&task).unwrap();
        // 3 regions + 3 verify + 1 integrate = 7
        assert_eq!(plan.graph.len(), 7);
        // Linear chain: parallelism limited
        assert!(plan.schedule.depth() >= 4);
    }

    #[test]
    fn decompose_with_consensus() {
        let mut task = HighLevelTask::new("Change API").with_consensus();
        task.add_region(Region::new("api"));
        task.add_region(Region::new("impl"));

        let plan = Decomposer::decompose(&task).unwrap();
        // consensus + region_api + region_impl + verify_api + verify_impl + integrate = 6
        assert_eq!(plan.graph.len(), 6);
        assert_eq!(plan.consensus_points.len(), 1);
        assert_eq!(plan.consensus_points[0], TaskId::new("consensus"));
    }

    // ── AssignmentGraph tests ──

    #[test]
    fn assignment_no_cycle() {
        // a -> b, agent1 does a, agent2 does b
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();

        let mut ag = AssignmentGraph::new();
        ag.assign("agent1", TaskId::new("a"));
        ag.assign("agent2", TaskId::new("b"));

        assert!(ag.verify_no_cycles(&graph).is_ok());
        assert_eq!(ag.agent_count(), 2);
        assert_eq!(ag.tasks_for_agent("agent1").len(), 1);
    }

    #[test]
    fn assignment_cycle_detected() {
        // a -> b, c -> d, but also b -> c
        // agent1: {a, d}, agent2: {b, c}
        // agent1 depends on agent2 (d depends on c which agent2 has)
        // agent2 depends on agent1 (b depends on a which agent1 has)
        // => circular agent dependency
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("c", "C", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("d", "D", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();
        graph.add_dependency(&TaskId::new("b"), &TaskId::new("c")).unwrap();
        graph.add_dependency(&TaskId::new("c"), &TaskId::new("d")).unwrap();

        let mut ag = AssignmentGraph::new();
        ag.assign("agent1", TaskId::new("a"));
        ag.assign("agent2", TaskId::new("b"));
        ag.assign("agent2", TaskId::new("c"));
        ag.assign("agent1", TaskId::new("d"));

        let err = ag.verify_no_cycles(&graph).unwrap_err();
        assert!(matches!(err, DecompositionError::AssignmentCycle(_)));
    }

    #[test]
    fn assignment_same_agent_no_cycle() {
        // All tasks on same agent — can't cycle
        let mut graph = DependencyGraph::new();
        graph.add_task(TaskNode::new("a", "A", TaskKind::AddCode)).unwrap();
        graph.add_task(TaskNode::new("b", "B", TaskKind::AddCode)).unwrap();
        graph.add_dependency(&TaskId::new("a"), &TaskId::new("b")).unwrap();

        let mut ag = AssignmentGraph::new();
        ag.assign("solo", TaskId::new("a"));
        ag.assign("solo", TaskId::new("b"));

        assert!(ag.verify_no_cycles(&graph).is_ok());
    }

    // ── Region tests ──

    #[test]
    fn region_with_dependencies() {
        let r = Region::new("core").with_dependency("base").with_dependency("utils");
        assert_eq!(r.name, "core");
        assert_eq!(r.dependencies, vec!["base", "utils"]);
    }

    // ── Error Display test ──

    #[test]
    fn error_display() {
        let err = DecompositionError::CycleDetected(vec![TaskId::new("a"), TaskId::new("b")]);
        assert!(format!("{}", err).contains("Cycle detected"));

        let err = DecompositionError::TaskNotFound(TaskId::new("x"));
        assert!(format!("{}", err).contains("Task not found: x"));

        let err =
            DecompositionError::AssignmentCycle(vec!["agent1".to_string(), "agent2".to_string()]);
        assert!(format!("{}", err).contains("Assignment cycle"));
    }
}
