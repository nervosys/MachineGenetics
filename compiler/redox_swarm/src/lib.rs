// Redox Swarm SDK — orchestrator, synthesizer, and verifier agent roles.
//
// Provides the core swarm collaboration model (§7 of REDOX_PROPOSAL.md):
// - Agent roles (Orchestrator, Synthesizerr, Verifier, Architect, etc.)
// - Swarm topology (DAG of agents)
// - Orchestration patterns (MapReduce, Pipeline, ScatterGather, Saga)
// - Task decomposition and assignment
//
// (ROADMAP Step 50)

use std::collections::BTreeMap;
use std::fmt;

// ── Agent Identity ─────────────────────────────────────────────────────────

/// Unique agent identifier within a swarm.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(id: &str) -> Self {
        AgentId(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Agent Roles (§7.2) ────────────────────────────────────────────────────

/// Role taxonomy for swarm agents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentRole {
    /// Decomposes tasks, assigns regions, manages consensus. Singleton per task.
    Orchestrator,
    /// Designs APIs, proposes types/traits. Singleton or quorum.
    Architect,
    /// Writes implementation within assigned regions. Many in parallel.
    Synthesizer,
    /// Validates changes, emits diagnostics. Many in parallel, read-only.
    Reviewer,
    /// Combines swarm output, resolves semantic conflicts. One per merge boundary.
    Integrator,
    /// Validates correctness, runs tests, emits certificates. Many in parallel.
    Verifier,
    /// Performance optimization within assigned regions. Many in parallel.
    Optimizer,
    /// Generates/updates documentation. Many in parallel.
    Documenter,
    /// Custom user-defined role.
    Custom(String),
}

impl AgentRole {
    /// Default capabilities for this role.
    pub fn default_capabilities(&self) -> Vec<&'static str> {
        match self {
            AgentRole::Orchestrator => vec![
                "read_all", "decompose_task", "assign_region", "manage_consensus",
            ],
            AgentRole::Architect => vec![
                "read_all", "modify_interfaces", "propose_types", "propose_traits",
            ],
            AgentRole::Synthesizer => vec![
                "read_all", "modify_region", "query_types", "query_borrow",
            ],
            AgentRole::Reviewer => vec![
                "read_all", "query_types", "query_borrow", "emit_diagnostics",
            ],
            AgentRole::Integrator => vec![
                "read_all", "merge_changes", "resolve_conflicts",
            ],
            AgentRole::Verifier => vec![
                "read_all", "run_tests", "run_miri", "check_contracts",
            ],
            AgentRole::Optimizer => vec![
                "read_all", "modify_region", "query_perf",
            ],
            AgentRole::Documenter => vec![
                "read_all", "modify_docs", "query_types",
            ],
            AgentRole::Custom(_) => vec!["read_all"],
        }
    }

    /// Whether this role can modify code.
    pub fn can_write(&self) -> bool {
        matches!(
            self,
            AgentRole::Orchestrator
                | AgentRole::Architect
                | AgentRole::Synthesizer
                | AgentRole::Integrator
                | AgentRole::Optimizer
                | AgentRole::Documenter
        )
    }

    /// Whether multiple instances can run in parallel.
    pub fn allows_parallel(&self) -> bool {
        matches!(
            self,
            AgentRole::Synthesizer
                | AgentRole::Reviewer
                | AgentRole::Verifier
                | AgentRole::Optimizer
                | AgentRole::Documenter
                | AgentRole::Custom(_)
        )
    }
}

impl fmt::Display for AgentRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentRole::Orchestrator => write!(f, "orchestrator"),
            AgentRole::Architect => write!(f, "architect"),
            AgentRole::Synthesizer => write!(f, "synthesizer"),
            AgentRole::Reviewer => write!(f, "reviewer"),
            AgentRole::Integrator => write!(f, "integrator"),
            AgentRole::Verifier => write!(f, "verifier"),
            AgentRole::Optimizer => write!(f, "optimizer"),
            AgentRole::Documenter => write!(f, "documenter"),
            AgentRole::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

// ── Agent Descriptor ───────────────────────────────────────────────────────

/// An agent in the swarm, with role and assignment.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: AgentId,
    pub role: AgentRole,
    pub assigned_region: Option<String>,
    pub status: AgentStatus,
}

impl Agent {
    pub fn new(id: &str, role: AgentRole) -> Self {
        Agent {
            id: AgentId::new(id),
            role,
            assigned_region: None,
            status: AgentStatus::Idle,
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.assigned_region = Some(region.to_string());
        self
    }

    pub fn is_idle(&self) -> bool {
        self.status == AgentStatus::Idle
    }

    pub fn is_active(&self) -> bool {
        self.status == AgentStatus::Active
    }
}

/// Agent lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Active,
    Blocked(String),
    Completed,
    Failed(String),
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Blocked(r) => write!(f, "blocked: {r}"),
            AgentStatus::Completed => write!(f, "completed"),
            AgentStatus::Failed(r) => write!(f, "failed: {r}"),
        }
    }
}

// ── Task Decomposition ─────────────────────────────────────────────────────

/// A task to be executed by the swarm.
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub region: Option<String>,
    pub dependencies: Vec<String>,
    pub assigned_to: Option<AgentId>,
    pub status: TaskStatus,
}

impl Task {
    pub fn new(id: &str, description: &str) -> Self {
        Task {
            id: id.to_string(),
            description: description.to_string(),
            region: None,
            dependencies: Vec::new(),
            assigned_to: None,
            status: TaskStatus::Pending,
        }
    }

    pub fn with_region(mut self, region: &str) -> Self {
        self.region = Some(region.to_string());
        self
    }

    pub fn with_dependency(mut self, dep: &str) -> Self {
        self.dependencies.push(dep.to_string());
        self
    }

    pub fn is_ready(&self, completed: &[String]) -> bool {
        self.dependencies.iter().all(|d| completed.contains(d))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed(String),
}

// ── Swarm Topology ─────────────────────────────────────────────────────────

/// A swarm: a DAG of agents organized by role.
pub struct Swarm {
    name: String,
    agents: BTreeMap<AgentId, Agent>,
    tasks: Vec<Task>,
    completed_tasks: Vec<String>,
    edges: Vec<(AgentId, AgentId)>,
}

impl Swarm {
    pub fn new(name: &str) -> Self {
        Swarm {
            name: name.to_string(),
            agents: BTreeMap::new(),
            tasks: Vec::new(),
            completed_tasks: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add an agent to the swarm.
    pub fn add_agent(&mut self, agent: Agent) {
        self.agents.insert(agent.id.clone(), agent);
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, id: &AgentId) -> Option<&Agent> {
        self.agents.get(id)
    }

    /// Get a mutable reference to an agent.
    pub fn get_agent_mut(&mut self, id: &AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(id)
    }

    /// Add a directed edge (dependency) between agents.
    pub fn add_edge(&mut self, from: &AgentId, to: &AgentId) {
        self.edges.push((from.clone(), to.clone()));
    }

    /// All agents in the swarm.
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }

    /// Count agents by role.
    pub fn count_by_role(&self, role: &AgentRole) -> usize {
        self.agents.values().filter(|a| &a.role == role).count()
    }

    /// All agents with a specific role.
    pub fn agents_with_role(&self, role: &AgentRole) -> Vec<&Agent> {
        self.agents.values().filter(|a| &a.role == role).collect()
    }

    /// Total number of agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    // ── Task Management ──

    /// Add a task to the swarm.
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Get all ready tasks (dependencies satisfied, not yet assigned).
    pub fn ready_tasks(&self) -> Vec<&Task> {
        self.tasks.iter()
            .filter(|t| t.status == TaskStatus::Pending && t.is_ready(&self.completed_tasks))
            .collect()
    }

    /// Assign a task to an agent.
    pub fn assign_task(&mut self, task_id: &str, agent_id: &AgentId) -> bool {
        let task = self.tasks.iter_mut().find(|t| t.id == task_id);
        if let Some(task) = task {
            if task.status != TaskStatus::Pending {
                return false;
            }
            task.assigned_to = Some(agent_id.clone());
            task.status = TaskStatus::Assigned;
            if let Some(agent) = self.agents.get_mut(agent_id) {
                agent.status = AgentStatus::Active;
            }
            true
        } else {
            false
        }
    }

    /// Mark a task as completed.
    pub fn complete_task(&mut self, task_id: &str) -> bool {
        let task = self.tasks.iter_mut().find(|t| t.id == task_id);
        if let Some(task) = task {
            task.status = TaskStatus::Completed;
            self.completed_tasks.push(task_id.to_string());
            if let Some(ref agent_id) = task.assigned_to {
                if let Some(agent) = self.agents.get_mut(agent_id) {
                    agent.status = AgentStatus::Completed;
                }
            }
            true
        } else {
            false
        }
    }

    /// Mark a task as failed.
    pub fn fail_task(&mut self, task_id: &str, reason: &str) -> bool {
        let task = self.tasks.iter_mut().find(|t| t.id == task_id);
        if let Some(task) = task {
            task.status = TaskStatus::Failed(reason.to_string());
            if let Some(ref agent_id) = task.assigned_to {
                if let Some(agent) = self.agents.get_mut(agent_id) {
                    agent.status = AgentStatus::Failed(reason.to_string());
                }
            }
            true
        } else {
            false
        }
    }

    /// All tasks.
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    /// Number of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.completed_tasks.len()
    }
}

// ── Orchestrator ───────────────────────────────────────────────────────────

/// The orchestrator: decomposes tasks, assigns to agents, manages lifecycle.
pub struct Orchestrator {
    swarm: Swarm,
}

impl Orchestrator {
    pub fn new(swarm_name: &str) -> Self {
        Orchestrator {
            swarm: Swarm::new(swarm_name),
        }
    }

    pub fn swarm(&self) -> &Swarm {
        &self.swarm
    }

    pub fn swarm_mut(&mut self) -> &mut Swarm {
        &mut self.swarm
    }

    /// Register an agent in the swarm.
    pub fn register_agent(&mut self, agent: Agent) {
        self.swarm.add_agent(agent);
    }

    /// Decompose a high-level task into subtasks by region.
    pub fn decompose_task(
        &mut self,
        task_id: &str,
        description: &str,
        regions: Vec<String>,
    ) -> Vec<String> {
        let mut subtask_ids = Vec::new();
        for (i, region) in regions.iter().enumerate() {
            let sub_id = format!("{task_id}_{i}");
            let sub_desc = format!("{description} [region: {region}]");
            let task = Task::new(&sub_id, &sub_desc).with_region(region);
            self.swarm.add_task(task);
            subtask_ids.push(sub_id);
        }
        subtask_ids
    }

    /// Auto-assign ready tasks to idle agents with matching roles.
    pub fn auto_assign(&mut self) -> Vec<(String, AgentId)> {
        let mut assignments = Vec::new();

        // Collect ready task IDs.
        let ready_ids: Vec<String> = self.swarm.ready_tasks()
            .iter()
            .map(|t| t.id.clone())
            .collect();

        // Collect idle synthesizer/verifier agent IDs.
        let idle_ids: Vec<AgentId> = self.swarm.agents
            .values()
            .filter(|a| a.is_idle() && a.role.allows_parallel())
            .map(|a| a.id.clone())
            .collect();

        let mut idle_iter = idle_ids.into_iter();
        for task_id in ready_ids {
            if let Some(agent_id) = idle_iter.next() {
                if self.swarm.assign_task(&task_id, &agent_id) {
                    assignments.push((task_id, agent_id));
                }
            }
        }
        assignments
    }

    /// Run a full orchestration cycle: auto-assign, return assignments.
    pub fn orchestrate(&mut self) -> Vec<(String, AgentId)> {
        self.auto_assign()
    }
}

// ── Orchestration Patterns (§7.10) ─────────────────────────────────────────

/// MapReduce pattern: distribute work across N agents, combine results.
#[derive(Debug, Clone)]
pub struct MapReducePattern {
    pub name: String,
    pub inputs: Vec<String>,
    pub agent_count: usize,
    pub timeout_ms: u64,
}

impl MapReducePattern {
    pub fn new(name: &str, inputs: Vec<String>, agents: usize) -> Self {
        MapReducePattern {
            name: name.to_string(),
            inputs,
            agent_count: agents,
            timeout_ms: 30_000,
        }
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Partition inputs into chunks for N agents.
    pub fn partition(&self) -> Vec<Vec<String>> {
        let n = self.agent_count.max(1);
        let mut chunks: Vec<Vec<String>> = (0..n).map(|_| Vec::new()).collect();
        for (i, input) in self.inputs.iter().enumerate() {
            chunks[i % n].push(input.clone());
        }
        chunks
    }
}

/// Pipeline pattern: staged processing with backpressure.
#[derive(Debug, Clone)]
pub struct PipelinePattern {
    pub name: String,
    pub stages: Vec<PipelineStage>,
    pub queue_bound: usize,
}

#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub name: String,
    pub agent_count: usize,
}

impl PipelineStage {
    pub fn new(name: &str, agents: usize) -> Self {
        PipelineStage {
            name: name.to_string(),
            agent_count: agents,
        }
    }
}

impl PipelinePattern {
    pub fn new(name: &str) -> Self {
        PipelinePattern {
            name: name.to_string(),
            stages: Vec::new(),
            queue_bound: 1000,
        }
    }

    pub fn add_stage(mut self, stage: PipelineStage) -> Self {
        self.stages.push(stage);
        self
    }

    pub fn with_queue_bound(mut self, bound: usize) -> Self {
        self.queue_bound = bound;
        self
    }

    /// Total agents needed across all stages.
    pub fn total_agents(&self) -> usize {
        self.stages.iter().map(|s| s.agent_count).sum()
    }

    /// Validate: stages non-empty, agents > 0 per stage.
    pub fn validate(&self) -> Result<(), String> {
        if self.stages.is_empty() {
            return Err("pipeline has no stages".to_string());
        }
        for stage in &self.stages {
            if stage.agent_count == 0 {
                return Err(format!("stage '{}' has 0 agents", stage.name));
            }
        }
        Ok(())
    }
}

/// ScatterGather pattern: broadcast work, collect responses with quorum.
#[derive(Debug, Clone)]
pub struct ScatterGatherPattern {
    pub name: String,
    pub agent_count: usize,
    pub quorum: f64,
    pub timeout_ms: u64,
}

impl ScatterGatherPattern {
    pub fn new(name: &str, agents: usize) -> Self {
        ScatterGatherPattern {
            name: name.to_string(),
            agent_count: agents,
            quorum: 0.8,
            timeout_ms: 60_000,
        }
    }

    pub fn with_quorum(mut self, q: f64) -> Self {
        self.quorum = q.clamp(0.0, 1.0);
        self
    }

    /// Minimum responses needed to satisfy quorum.
    pub fn quorum_count(&self) -> usize {
        ((self.agent_count as f64) * self.quorum).ceil() as usize
    }

    /// Check if quorum is met given N responses.
    pub fn quorum_met(&self, responses: usize) -> bool {
        responses >= self.quorum_count()
    }
}

/// Saga pattern: distributed transaction with compensation.
#[derive(Debug, Clone)]
pub struct SagaPattern {
    pub name: String,
    pub steps: Vec<SagaStep>,
    pub on_failure: SagaFailurePolicy,
}

#[derive(Debug, Clone)]
pub struct SagaStep {
    pub action: String,
    pub compensate: String,
}

impl SagaStep {
    pub fn new(action: &str, compensate: &str) -> Self {
        SagaStep {
            action: action.to_string(),
            compensate: compensate.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SagaFailurePolicy {
    CompensateAll,
    CompensateCompleted,
    Abort,
}

impl SagaPattern {
    pub fn new(name: &str) -> Self {
        SagaPattern {
            name: name.to_string(),
            steps: Vec::new(),
            on_failure: SagaFailurePolicy::CompensateAll,
        }
    }

    pub fn add_step(mut self, step: SagaStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn with_failure_policy(mut self, policy: SagaFailurePolicy) -> Self {
        self.on_failure = policy;
        self
    }

    /// Validate: every action has a compensating action.
    pub fn validate(&self) -> Result<(), String> {
        if self.steps.is_empty() {
            return Err("saga has no steps".to_string());
        }
        for (i, step) in self.steps.iter().enumerate() {
            if step.compensate.is_empty() {
                return Err(format!("step {} ('{}') has no compensation", i, step.action));
            }
        }
        Ok(())
    }

    /// Execute compensation for completed steps (indices 0..failed_at).
    pub fn compensation_plan(&self, failed_at: usize) -> Vec<&str> {
        self.steps[..failed_at].iter()
            .rev()
            .map(|s| s.compensate.as_str())
            .collect()
    }
}

// ── Pattern Verification (§7.10.2) ────────────────────────────────────────

/// Result of verifying an orchestration pattern.
#[derive(Debug, Clone)]
pub struct PatternVerification {
    pub pattern_name: String,
    pub checks: Vec<PatternCheck>,
}

#[derive(Debug, Clone)]
pub struct PatternCheck {
    pub property: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl PatternVerification {
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }
}

/// Verify a pipeline pattern (stage contracts chain, agents > 0).
pub fn verify_pipeline(pattern: &PipelinePattern) -> PatternVerification {
    let mut checks = Vec::new();

    // Non-empty stages.
    checks.push(PatternCheck {
        property: "non_empty_stages".to_string(),
        passed: !pattern.stages.is_empty(),
        detail: None,
    });

    // All stages have agents > 0.
    let all_have_agents = pattern.stages.iter().all(|s| s.agent_count > 0);
    checks.push(PatternCheck {
        property: "all_stages_have_agents".to_string(),
        passed: all_have_agents,
        detail: None,
    });

    // Queue bound is positive.
    checks.push(PatternCheck {
        property: "positive_queue_bound".to_string(),
        passed: pattern.queue_bound > 0,
        detail: None,
    });

    PatternVerification {
        pattern_name: pattern.name.clone(),
        checks,
    }
}

/// Verify a saga pattern (all steps have compensation).
pub fn verify_saga(pattern: &SagaPattern) -> PatternVerification {
    let mut checks = Vec::new();

    checks.push(PatternCheck {
        property: "non_empty_steps".to_string(),
        passed: !pattern.steps.is_empty(),
        detail: None,
    });

    let all_have_comp = pattern.steps.iter().all(|s| !s.compensate.is_empty());
    checks.push(PatternCheck {
        property: "all_steps_have_compensation".to_string(),
        passed: all_have_comp,
        detail: if !all_have_comp {
            Some("some steps lack compensation actions".to_string())
        } else {
            None
        },
    });

    PatternVerification {
        pattern_name: pattern.name.clone(),
        checks,
    }
}

/// Verify a scatter-gather pattern (quorum in range, agents > 0).
pub fn verify_scatter_gather(pattern: &ScatterGatherPattern) -> PatternVerification {
    let mut checks = Vec::new();

    checks.push(PatternCheck {
        property: "has_agents".to_string(),
        passed: pattern.agent_count > 0,
        detail: None,
    });

    checks.push(PatternCheck {
        property: "quorum_in_range".to_string(),
        passed: (0.0..=1.0).contains(&pattern.quorum),
        detail: None,
    });

    checks.push(PatternCheck {
        property: "quorum_achievable".to_string(),
        passed: pattern.quorum_count() <= pattern.agent_count,
        detail: None,
    });

    PatternVerification {
        pattern_name: pattern.name.clone(),
        checks,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AgentId ──

    #[test]
    fn agent_id_equality() {
        let a = AgentId::new("alpha");
        let b = AgentId::new("alpha");
        assert_eq!(a, b);
    }

    #[test]
    fn agent_id_display() {
        let a = AgentId::new("synth-01");
        assert_eq!(format!("{a}"), "synth-01");
    }

    // ── AgentRole ──

    #[test]
    fn role_capabilities() {
        let caps = AgentRole::Orchestrator.default_capabilities();
        assert!(caps.contains(&"read_all"));
        assert!(caps.contains(&"manage_consensus"));
    }

    #[test]
    fn role_can_write() {
        assert!(AgentRole::Orchestrator.can_write());
        assert!(AgentRole::Synthesizer.can_write());
        assert!(!AgentRole::Reviewer.can_write());
        assert!(!AgentRole::Verifier.can_write());
    }

    #[test]
    fn role_allows_parallel() {
        assert!(AgentRole::Synthesizer.allows_parallel());
        assert!(AgentRole::Verifier.allows_parallel());
        assert!(!AgentRole::Orchestrator.allows_parallel());
        assert!(!AgentRole::Integrator.allows_parallel());
    }

    #[test]
    fn role_display() {
        assert_eq!(format!("{}", AgentRole::Orchestrator), "orchestrator");
        assert_eq!(format!("{}", AgentRole::Synthesizer), "synthesizer");
        assert_eq!(format!("{}", AgentRole::Verifier), "verifier");
        assert_eq!(format!("{}", AgentRole::Custom("foo".to_string())), "custom:foo");
    }

    // ── Agent ──

    #[test]
    fn agent_creation() {
        let a = Agent::new("synth-01", AgentRole::Synthesizer);
        assert_eq!(a.id, AgentId::new("synth-01"));
        assert!(a.is_idle());
        assert!(a.assigned_region.is_none());
    }

    #[test]
    fn agent_with_region() {
        let a = Agent::new("synth-01", AgentRole::Synthesizer)
            .with_region("module::foo");
        assert_eq!(a.assigned_region.as_deref(), Some("module::foo"));
    }

    #[test]
    fn agent_status_display() {
        assert_eq!(format!("{}", AgentStatus::Idle), "idle");
        assert_eq!(format!("{}", AgentStatus::Active), "active");
        assert!(format!("{}", AgentStatus::Failed("x".to_string())).contains("failed"));
    }

    // ── Task ──

    #[test]
    fn task_ready_no_deps() {
        let t = Task::new("t1", "do something");
        assert!(t.is_ready(&[]));
    }

    #[test]
    fn task_ready_with_satisfied_deps() {
        let t = Task::new("t2", "step two")
            .with_dependency("t1");
        assert!(!t.is_ready(&[]));
        assert!(t.is_ready(&["t1".to_string()]));
    }

    #[test]
    fn task_with_region() {
        let t = Task::new("t1", "work").with_region("module::bar");
        assert_eq!(t.region.as_deref(), Some("module::bar"));
    }

    // ── Swarm ──

    #[test]
    fn swarm_add_agents() {
        let mut swarm = Swarm::new("test-swarm");
        swarm.add_agent(Agent::new("orch", AgentRole::Orchestrator));
        swarm.add_agent(Agent::new("synth-1", AgentRole::Synthesizer));
        swarm.add_agent(Agent::new("synth-2", AgentRole::Synthesizer));
        assert_eq!(swarm.agent_count(), 3);
        assert_eq!(swarm.count_by_role(&AgentRole::Synthesizer), 2);
    }

    #[test]
    fn swarm_task_lifecycle() {
        let mut swarm = Swarm::new("test");
        let agent_id = AgentId::new("synth-1");
        swarm.add_agent(Agent::new("synth-1", AgentRole::Synthesizer));
        swarm.add_task(Task::new("t1", "implement foo"));

        assert_eq!(swarm.ready_tasks().len(), 1);
        assert!(swarm.assign_task("t1", &agent_id));
        assert_eq!(swarm.ready_tasks().len(), 0);
        assert!(swarm.complete_task("t1"));
        assert_eq!(swarm.completed_count(), 1);
    }

    #[test]
    fn swarm_task_dependencies() {
        let mut swarm = Swarm::new("test");
        swarm.add_task(Task::new("t1", "first"));
        swarm.add_task(Task::new("t2", "second").with_dependency("t1"));

        assert_eq!(swarm.ready_tasks().len(), 1);
        assert_eq!(swarm.ready_tasks()[0].id, "t1");

        swarm.complete_task("t1");
        assert_eq!(swarm.ready_tasks().len(), 1);
        assert_eq!(swarm.ready_tasks()[0].id, "t2");
    }

    #[test]
    fn swarm_fail_task() {
        let mut swarm = Swarm::new("test");
        swarm.add_agent(Agent::new("a1", AgentRole::Synthesizer));
        swarm.add_task(Task::new("t1", "fragile"));
        swarm.assign_task("t1", &AgentId::new("a1"));
        assert!(swarm.fail_task("t1", "compile error"));
        let agent = swarm.get_agent(&AgentId::new("a1")).unwrap();
        assert!(matches!(agent.status, AgentStatus::Failed(_)));
    }

    // ── Orchestrator ──

    #[test]
    fn orchestrator_decompose() {
        let mut orch = Orchestrator::new("build-swarm");
        let subs = orch.decompose_task(
            "refactor",
            "refactor crate",
            vec!["module_a".to_string(), "module_b".to_string()],
        );
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0], "refactor_0");
        assert_eq!(subs[1], "refactor_1");
        assert_eq!(orch.swarm().tasks().len(), 2);
    }

    #[test]
    fn orchestrator_auto_assign() {
        let mut orch = Orchestrator::new("test");
        orch.register_agent(Agent::new("s1", AgentRole::Synthesizer));
        orch.register_agent(Agent::new("s2", AgentRole::Synthesizer));
        orch.swarm_mut().add_task(Task::new("t1", "task 1"));
        orch.swarm_mut().add_task(Task::new("t2", "task 2"));

        let assignments = orch.auto_assign();
        assert_eq!(assignments.len(), 2);
    }

    #[test]
    fn orchestrator_orchestrate() {
        let mut orch = Orchestrator::new("pipeline");
        orch.register_agent(Agent::new("v1", AgentRole::Verifier));
        orch.decompose_task("verify", "verify all", vec!["mod_a".to_string()]);

        let assignments = orch.orchestrate();
        assert_eq!(assignments.len(), 1);
    }

    // ── MapReduce ──

    #[test]
    fn map_reduce_partition() {
        let mr = MapReducePattern::new(
            "analyze",
            vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            3,
        );
        let chunks = mr.partition();
        assert_eq!(chunks.len(), 3);
        let total: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn map_reduce_single_agent() {
        let mr = MapReducePattern::new("all", vec!["x".into(), "y".into()], 1);
        let chunks = mr.partition();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2);
    }

    // ── Pipeline ──

    #[test]
    fn pipeline_total_agents() {
        let p = PipelinePattern::new("etl")
            .add_stage(PipelineStage::new("fetch", 4))
            .add_stage(PipelineStage::new("parse", 8))
            .add_stage(PipelineStage::new("store", 2));
        assert_eq!(p.total_agents(), 14);
    }

    #[test]
    fn pipeline_validate_ok() {
        let p = PipelinePattern::new("good")
            .add_stage(PipelineStage::new("s1", 2));
        assert!(p.validate().is_ok());
    }

    #[test]
    fn pipeline_validate_empty() {
        let p = PipelinePattern::new("bad");
        assert!(p.validate().is_err());
    }

    #[test]
    fn pipeline_validate_zero_agents() {
        let p = PipelinePattern::new("bad")
            .add_stage(PipelineStage::new("s1", 0));
        assert!(p.validate().is_err());
    }

    // ── ScatterGather ──

    #[test]
    fn scatter_gather_quorum() {
        let sg = ScatterGatherPattern::new("audit", 10)
            .with_quorum(0.8);
        assert_eq!(sg.quorum_count(), 8);
        assert!(!sg.quorum_met(7));
        assert!(sg.quorum_met(8));
        assert!(sg.quorum_met(10));
    }

    #[test]
    fn scatter_gather_full_quorum() {
        let sg = ScatterGatherPattern::new("all", 5)
            .with_quorum(1.0);
        assert_eq!(sg.quorum_count(), 5);
    }

    // ── Saga ──

    #[test]
    fn saga_validate_ok() {
        let s = SagaPattern::new("refactor")
            .add_step(SagaStep::new("modify_a", "rollback_a"))
            .add_step(SagaStep::new("modify_b", "rollback_b"));
        assert!(s.validate().is_ok());
    }

    #[test]
    fn saga_validate_missing_comp() {
        let s = SagaPattern::new("bad")
            .add_step(SagaStep::new("action", ""));
        assert!(s.validate().is_err());
    }

    #[test]
    fn saga_compensation_plan() {
        let s = SagaPattern::new("test")
            .add_step(SagaStep::new("a", "undo_a"))
            .add_step(SagaStep::new("b", "undo_b"))
            .add_step(SagaStep::new("c", "undo_c"));
        // If step at index 2 fails, compensate steps 0 and 1 in reverse order.
        let plan = s.compensation_plan(2);
        assert_eq!(plan, vec!["undo_b", "undo_a"]);
    }

    #[test]
    fn saga_failure_policy() {
        let s = SagaPattern::new("test")
            .with_failure_policy(SagaFailurePolicy::Abort);
        assert_eq!(s.on_failure, SagaFailurePolicy::Abort);
    }

    // ── Pattern Verification ──

    #[test]
    fn verify_pipeline_passes() {
        let p = PipelinePattern::new("good")
            .add_stage(PipelineStage::new("s1", 4))
            .add_stage(PipelineStage::new("s2", 2));
        let v = verify_pipeline(&p);
        assert!(v.all_passed());
    }

    #[test]
    fn verify_saga_passes() {
        let s = SagaPattern::new("good")
            .add_step(SagaStep::new("x", "undo_x"));
        let v = verify_saga(&s);
        assert!(v.all_passed());
    }

    #[test]
    fn verify_saga_fails_no_compensation() {
        let s = SagaPattern::new("bad")
            .add_step(SagaStep::new("x", ""));
        let v = verify_saga(&s);
        assert!(!v.all_passed());
    }

    #[test]
    fn verify_scatter_gather_passes() {
        let sg = ScatterGatherPattern::new("audit", 10)
            .with_quorum(0.5);
        let v = verify_scatter_gather(&sg);
        assert!(v.all_passed());
    }

    // ── Full Scenario ──

    #[test]
    fn full_swarm_scenario() {
        let mut orch = Orchestrator::new("flight-controller-swarm");

        // Register agents.
        orch.register_agent(Agent::new("arch-01", AgentRole::Architect));
        orch.register_agent(Agent::new("synth-01", AgentRole::Synthesizer)
            .with_region("module::control"));
        orch.register_agent(Agent::new("synth-02", AgentRole::Synthesizer)
            .with_region("module::sensors"));
        orch.register_agent(Agent::new("verify-01", AgentRole::Verifier));
        orch.register_agent(Agent::new("verify-02", AgentRole::Verifier));

        assert_eq!(orch.swarm().agent_count(), 5);
        assert_eq!(orch.swarm().count_by_role(&AgentRole::Synthesizer), 2);

        // Decompose into subtasks.
        let subs = orch.decompose_task(
            "impl",
            "implement flight controller",
            vec!["module::control".to_string(), "module::sensors".to_string()],
        );
        assert_eq!(subs.len(), 2);

        // Orchestrate: auto-assign ready tasks to idle parallel agents.
        let assignments = orch.orchestrate();
        assert!(!assignments.is_empty());

        // Complete tasks.
        for (task_id, _) in &assignments {
            orch.swarm_mut().complete_task(task_id);
        }
        assert_eq!(orch.swarm().completed_count(), assignments.len());
    }
}
