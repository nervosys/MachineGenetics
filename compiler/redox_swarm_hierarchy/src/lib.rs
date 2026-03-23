//! # Swarm-of-Swarms Hierarchical Orchestration
//!
//! Hierarchical orchestration for million-LOC+ codebases. Organises agents into
//! nested swarms with delegation, fan-out/fan-in, priority scheduling, and
//! recursive decomposition of large compilation units.

use std::collections::HashMap;
use std::fmt;

// ── Identifiers ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwarmId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId(pub u64);

impl fmt::Display for SwarmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "swarm-{}", self.0)
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent-{}", self.0)
    }
}

// ── Agent & Task ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Parser,
    TypeChecker,
    BorrowChecker,
    Optimizer,
    CodeGen,
    Linker,
    Coordinator,
    Monitor,
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Parser => "parser",
            Self::TypeChecker => "type_checker",
            Self::BorrowChecker => "borrow_checker",
            Self::Optimizer => "optimizer",
            Self::CodeGen => "code_gen",
            Self::Linker => "linker",
            Self::Coordinator => "coordinator",
            Self::Monitor => "monitor",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub label: String,
    pub status: TaskStatus,
    pub assigned_to: Option<AgentId>,
    pub priority: u8,
}

impl Task {
    pub fn new(id: u64, label: impl Into<String>, priority: u8) -> Self {
        Self { id, label: label.into(), status: TaskStatus::Queued, assigned_to: None, priority }
    }
}

#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: AgentId,
    pub role: AgentRole,
    pub current_task: Option<u64>,
    pub tasks_completed: u32,
}

impl AgentHandle {
    pub fn new(id: AgentId, role: AgentRole) -> Self {
        Self { id, role, current_task: None, tasks_completed: 0 }
    }

    pub fn is_idle(&self) -> bool {
        self.current_task.is_none()
    }
}

// ── Swarm ────────────────────────────────────────────────────────────

/// A single swarm of agents working on a coherent sub-problem.
#[derive(Debug)]
pub struct Swarm {
    pub id: SwarmId,
    pub label: String,
    pub agents: Vec<AgentHandle>,
    pub tasks: Vec<Task>,
    pub parent: Option<SwarmId>,
    pub children: Vec<SwarmId>,
}

impl Swarm {
    pub fn new(id: SwarmId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            agents: Vec::new(),
            tasks: Vec::new(),
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn add_agent(&mut self, agent: AgentHandle) {
        self.agents.push(agent);
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn idle_agents(&self) -> Vec<&AgentHandle> {
        self.agents.iter().filter(|a| a.is_idle()).collect()
    }

    pub fn pending_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Queued).collect()
    }

    pub fn completed_tasks(&self) -> usize {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count()
    }

    pub fn failed_tasks(&self) -> usize {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Failed).count()
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Assign the highest-priority queued task to the first idle agent.
    pub fn schedule_next(&mut self) -> Option<(AgentId, u64)> {
        // Find highest-priority queued task
        let task_idx = self.tasks.iter().enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Queued)
            .max_by_key(|(_, t)| t.priority)
            .map(|(i, _)| i)?;

        // Find first idle agent
        let agent_idx = self.agents.iter().position(|a| a.is_idle())?;

        let task_id = self.tasks[task_idx].id;
        let agent_id = self.agents[agent_idx].id;

        self.tasks[task_idx].status = TaskStatus::Running;
        self.tasks[task_idx].assigned_to = Some(agent_id);
        self.agents[agent_idx].current_task = Some(task_id);

        Some((agent_id, task_id))
    }

    /// Mark a task as completed and free its agent.
    pub fn complete_task(&mut self, task_id: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return false;
        };
        task.status = TaskStatus::Completed;
        let agent_id = task.assigned_to;

        if let Some(aid) = agent_id {
            if let Some(agent) = self.agents.iter_mut().find(|a| a.id == aid) {
                agent.current_task = None;
                agent.tasks_completed += 1;
            }
        }
        true
    }

    /// Mark a task as failed and free its agent.
    pub fn fail_task(&mut self, task_id: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return false;
        };
        task.status = TaskStatus::Failed;
        let agent_id = task.assigned_to;

        if let Some(aid) = agent_id {
            if let Some(agent) = self.agents.iter_mut().find(|a| a.id == aid) {
                agent.current_task = None;
            }
        }
        true
    }
}

impl fmt::Display for Swarm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Swarm[{}] '{}' ({} agents, {} tasks, {} children)",
            self.id, self.label, self.agents.len(), self.tasks.len(), self.children.len())
    }
}

// ── SwarmHierarchy ───────────────────────────────────────────────────

/// The top-level hierarchy containing all swarms.
#[derive(Debug)]
pub struct SwarmHierarchy {
    swarms: HashMap<SwarmId, Swarm>,
    root: Option<SwarmId>,
    next_swarm_id: u64,
    next_agent_id: u64,
}

impl SwarmHierarchy {
    pub fn new() -> Self {
        Self { swarms: HashMap::new(), root: None, next_swarm_id: 0, next_agent_id: 0 }
    }

    pub fn create_swarm(&mut self, label: impl Into<String>) -> SwarmId {
        let id = SwarmId(self.next_swarm_id);
        self.next_swarm_id += 1;
        let swarm = Swarm::new(id, label);
        self.swarms.insert(id, swarm);
        if self.root.is_none() {
            self.root = Some(id);
        }
        id
    }

    pub fn create_agent(&mut self, _role: AgentRole) -> AgentId {
        let id = AgentId(self.next_agent_id);
        self.next_agent_id += 1;
        id
    }

    pub fn attach_child(&mut self, parent: SwarmId, child: SwarmId) -> bool {
        if !self.swarms.contains_key(&parent) || !self.swarms.contains_key(&child) {
            return false;
        }
        self.swarms.get_mut(&child).unwrap().parent = Some(parent);
        self.swarms.get_mut(&parent).unwrap().children.push(child);
        true
    }

    pub fn add_agent_to_swarm(&mut self, swarm_id: SwarmId, agent: AgentHandle) -> bool {
        if let Some(swarm) = self.swarms.get_mut(&swarm_id) {
            swarm.add_agent(agent);
            true
        } else {
            false
        }
    }

    pub fn add_task_to_swarm(&mut self, swarm_id: SwarmId, task: Task) -> bool {
        if let Some(swarm) = self.swarms.get_mut(&swarm_id) {
            swarm.add_task(task);
            true
        } else {
            false
        }
    }

    pub fn get_swarm(&self, id: SwarmId) -> Option<&Swarm> {
        self.swarms.get(&id)
    }

    pub fn get_swarm_mut(&mut self, id: SwarmId) -> Option<&mut Swarm> {
        self.swarms.get_mut(&id)
    }

    pub fn root(&self) -> Option<SwarmId> {
        self.root
    }

    pub fn swarm_count(&self) -> usize {
        self.swarms.len()
    }

    pub fn total_agents(&self) -> usize {
        self.swarms.values().map(|s| s.agents.len()).sum()
    }

    pub fn total_tasks(&self) -> usize {
        self.swarms.values().map(|s| s.tasks.len()).sum()
    }

    /// Depth of the hierarchy tree from root.
    pub fn depth(&self) -> usize {
        let Some(root) = self.root else { return 0 };
        self.depth_of(root)
    }

    fn depth_of(&self, id: SwarmId) -> usize {
        let Some(swarm) = self.swarms.get(&id) else { return 0 };
        if swarm.children.is_empty() {
            1
        } else {
            1 + swarm.children.iter().map(|c| self.depth_of(*c)).max().unwrap_or(0)
        }
    }

    /// Fan-out: distribute tasks from parent to child swarms round-robin.
    pub fn fan_out(&mut self, parent_id: SwarmId) -> usize {
        let Some(parent) = self.swarms.get(&parent_id) else { return 0 };
        if parent.children.is_empty() { return 0; }

        let queued: Vec<Task> = parent.tasks.iter()
            .filter(|t| t.status == TaskStatus::Queued)
            .cloned()
            .collect();
        let children: Vec<SwarmId> = parent.children.clone();

        if children.is_empty() || queued.is_empty() { return 0; }

        // Remove queued tasks from parent
        if let Some(p) = self.swarms.get_mut(&parent_id) {
            p.tasks.retain(|t| t.status != TaskStatus::Queued);
        }

        let count = queued.len();
        for (i, task) in queued.into_iter().enumerate() {
            let child_id = children[i % children.len()];
            if let Some(child) = self.swarms.get_mut(&child_id) {
                child.add_task(task);
            }
        }
        count
    }

    /// Fan-in: collect completion statistics from all children back to parent.
    pub fn fan_in(&self, parent_id: SwarmId) -> FanInResult {
        let Some(parent) = self.swarms.get(&parent_id) else {
            return FanInResult { total: 0, completed: 0, failed: 0 };
        };

        let mut total = 0usize;
        let mut completed = 0usize;
        let mut failed = 0usize;

        for child_id in &parent.children {
            if let Some(child) = self.swarms.get(child_id) {
                total += child.tasks.len();
                completed += child.completed_tasks();
                failed += child.failed_tasks();
            }
        }

        FanInResult { total, completed, failed }
    }
}

impl Default for SwarmHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanInResult {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
}

impl FanInResult {
    pub fn all_done(&self) -> bool {
        self.total > 0 && self.completed + self.failed == self.total
    }
}

// ── Decomposition ────────────────────────────────────────────────────

/// Decompose a large crate into sub-swarm work items.
#[derive(Debug, Clone)]
pub struct CrateUnit {
    pub name: String,
    pub estimated_loc: u64,
}

/// Recursively decompose into sub-swarms when modules exceed a LOC threshold.
pub fn decompose_crate(
    hierarchy: &mut SwarmHierarchy,
    parent: SwarmId,
    units: &[CrateUnit],
    loc_threshold: u64,
) -> Vec<SwarmId> {
    let mut created = Vec::new();
    for unit in units {
        if unit.estimated_loc > loc_threshold {
            let child_id = hierarchy.create_swarm(format!("sub:{}", unit.name));
            hierarchy.attach_child(parent, child_id);

            // Split into two halves
            let half = unit.estimated_loc / 2;
            let sub_a = CrateUnit { name: format!("{}_a", unit.name), estimated_loc: half };
            let sub_b = CrateUnit { name: format!("{}_b", unit.name), estimated_loc: unit.estimated_loc - half };
            let sub = decompose_crate(hierarchy, child_id, &[sub_a, sub_b], loc_threshold);
            created.push(child_id);
            created.extend(sub);
        } else {
            // Small enough: add as a task
            let task = Task::new(unit.estimated_loc, unit.name.clone(), 5);
            hierarchy.add_task_to_swarm(parent, task);
        }
    }
    created
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_id_display() {
        assert_eq!(format!("{}", SwarmId(42)), "swarm-42");
    }

    #[test]
    fn test_agent_id_display() {
        assert_eq!(format!("{}", AgentId(7)), "agent-7");
    }

    #[test]
    fn test_agent_role_display() {
        assert_eq!(format!("{}", AgentRole::Parser), "parser");
        assert_eq!(format!("{}", AgentRole::Coordinator), "coordinator");
    }

    #[test]
    fn test_task_new() {
        let t = Task::new(1, "parse", 5);
        assert_eq!(t.status, TaskStatus::Queued);
        assert!(t.assigned_to.is_none());
    }

    #[test]
    fn test_agent_handle_idle() {
        let a = AgentHandle::new(AgentId(0), AgentRole::Parser);
        assert!(a.is_idle());
    }

    #[test]
    fn test_swarm_new() {
        let s = Swarm::new(SwarmId(0), "root");
        assert!(s.is_leaf());
        assert_eq!(s.agents.len(), 0);
    }

    #[test]
    fn test_swarm_add_agent() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_agent(AgentHandle::new(AgentId(0), AgentRole::Parser));
        assert_eq!(s.agents.len(), 1);
    }

    #[test]
    fn test_swarm_schedule_next() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_agent(AgentHandle::new(AgentId(0), AgentRole::Parser));
        s.add_task(Task::new(1, "t1", 5));
        let result = s.schedule_next();
        assert!(result.is_some());
        let (aid, tid) = result.unwrap();
        assert_eq!(aid, AgentId(0));
        assert_eq!(tid, 1);
    }

    #[test]
    fn test_swarm_schedule_priority() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_agent(AgentHandle::new(AgentId(0), AgentRole::Parser));
        s.add_task(Task::new(1, "low", 1));
        s.add_task(Task::new(2, "high", 10));
        let (_, tid) = s.schedule_next().unwrap();
        assert_eq!(tid, 2); // high priority first
    }

    #[test]
    fn test_swarm_complete_task() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_agent(AgentHandle::new(AgentId(0), AgentRole::Parser));
        s.add_task(Task::new(1, "t1", 5));
        s.schedule_next();
        assert!(s.complete_task(1));
        assert_eq!(s.completed_tasks(), 1);
        assert!(s.agents[0].is_idle());
        assert_eq!(s.agents[0].tasks_completed, 1);
    }

    #[test]
    fn test_swarm_fail_task() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_agent(AgentHandle::new(AgentId(0), AgentRole::Parser));
        s.add_task(Task::new(1, "t1", 5));
        s.schedule_next();
        assert!(s.fail_task(1));
        assert_eq!(s.failed_tasks(), 1);
        assert!(s.agents[0].is_idle());
    }

    #[test]
    fn test_swarm_display() {
        let s = Swarm::new(SwarmId(0), "root");
        assert!(format!("{s}").contains("root"));
    }

    #[test]
    fn test_hierarchy_new() {
        let h = SwarmHierarchy::new();
        assert_eq!(h.swarm_count(), 0);
        assert!(h.root().is_none());
    }

    #[test]
    fn test_hierarchy_create_swarm() {
        let mut h = SwarmHierarchy::new();
        let id = h.create_swarm("root");
        assert_eq!(h.swarm_count(), 1);
        assert_eq!(h.root(), Some(id));
    }

    #[test]
    fn test_hierarchy_attach_child() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let child = h.create_swarm("child");
        assert!(h.attach_child(root, child));
        assert_eq!(h.get_swarm(root).unwrap().children.len(), 1);
        assert_eq!(h.get_swarm(child).unwrap().parent, Some(root));
    }

    #[test]
    fn test_hierarchy_depth() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let child = h.create_swarm("child");
        let grandchild = h.create_swarm("grandchild");
        h.attach_child(root, child);
        h.attach_child(child, grandchild);
        assert_eq!(h.depth(), 3);
    }

    #[test]
    fn test_hierarchy_total_agents() {
        let mut h = SwarmHierarchy::new();
        let s1 = h.create_swarm("s1");
        let s2 = h.create_swarm("s2");
        let a1 = AgentHandle::new(AgentId(0), AgentRole::Parser);
        let a2 = AgentHandle::new(AgentId(1), AgentRole::TypeChecker);
        h.add_agent_to_swarm(s1, a1);
        h.add_agent_to_swarm(s2, a2);
        assert_eq!(h.total_agents(), 2);
    }

    #[test]
    fn test_fan_out() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let c1 = h.create_swarm("c1");
        let c2 = h.create_swarm("c2");
        h.attach_child(root, c1);
        h.attach_child(root, c2);

        h.add_task_to_swarm(root, Task::new(1, "t1", 5));
        h.add_task_to_swarm(root, Task::new(2, "t2", 5));
        h.add_task_to_swarm(root, Task::new(3, "t3", 5));

        let distributed = h.fan_out(root);
        assert_eq!(distributed, 3);
        // Parent should have 0 queued
        assert_eq!(h.get_swarm(root).unwrap().pending_tasks().len(), 0);
        // Children should have tasks
        let c1_tasks = h.get_swarm(c1).unwrap().tasks.len();
        let c2_tasks = h.get_swarm(c2).unwrap().tasks.len();
        assert_eq!(c1_tasks + c2_tasks, 3);
    }

    #[test]
    fn test_fan_in() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let c1 = h.create_swarm("c1");
        h.attach_child(root, c1);

        let a = AgentHandle::new(AgentId(0), AgentRole::Parser);
        h.add_agent_to_swarm(c1, a);
        h.add_task_to_swarm(c1, Task::new(1, "t1", 5));
        h.get_swarm_mut(c1).unwrap().schedule_next();
        h.get_swarm_mut(c1).unwrap().complete_task(1);

        let result = h.fan_in(root);
        assert_eq!(result.total, 1);
        assert_eq!(result.completed, 1);
        assert!(result.all_done());
    }

    #[test]
    fn test_fan_in_result_not_done() {
        let r = FanInResult { total: 3, completed: 1, failed: 0 };
        assert!(!r.all_done());
    }

    #[test]
    fn test_decompose_small() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let units = vec![CrateUnit { name: "small".into(), estimated_loc: 100 }];
        let created = decompose_crate(&mut h, root, &units, 500);
        assert!(created.is_empty()); // no sub-swarms
        assert_eq!(h.get_swarm(root).unwrap().tasks.len(), 1);
    }

    #[test]
    fn test_decompose_large() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        let units = vec![CrateUnit { name: "big".into(), estimated_loc: 2000 }];
        let created = decompose_crate(&mut h, root, &units, 500);
        assert!(!created.is_empty());
        assert!(h.depth() > 1);
    }

    #[test]
    fn test_hierarchy_default() {
        let h = SwarmHierarchy::default();
        assert_eq!(h.swarm_count(), 0);
    }

    #[test]
    fn test_create_agent() {
        let mut h = SwarmHierarchy::new();
        let a1 = h.create_agent(AgentRole::Parser);
        let a2 = h.create_agent(AgentRole::TypeChecker);
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_swarm_no_idle_no_schedule() {
        let mut s = Swarm::new(SwarmId(0), "root");
        s.add_task(Task::new(1, "t1", 5));
        assert!(s.schedule_next().is_none()); // no agents
    }

    #[test]
    fn test_complete_nonexistent_task() {
        let mut s = Swarm::new(SwarmId(0), "root");
        assert!(!s.complete_task(999));
    }

    #[test]
    fn test_attach_invalid_parent() {
        let mut h = SwarmHierarchy::new();
        let child = h.create_swarm("child");
        assert!(!h.attach_child(SwarmId(999), child));
    }

    #[test]
    fn test_add_agent_invalid_swarm() {
        let mut h = SwarmHierarchy::new();
        let a = AgentHandle::new(AgentId(0), AgentRole::Parser);
        assert!(!h.add_agent_to_swarm(SwarmId(999), a));
    }

    #[test]
    fn test_fan_out_no_children() {
        let mut h = SwarmHierarchy::new();
        let root = h.create_swarm("root");
        h.add_task_to_swarm(root, Task::new(1, "t1", 5));
        assert_eq!(h.fan_out(root), 0);
    }
}
