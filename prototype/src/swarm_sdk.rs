// ── Swarm SDK ──────────────────────────────────────────────────────
//
// High-level SDK for building multi-agent compiler swarms.
//
// Components:
//   - `SwarmAgent` trait: the agent interface
//   - `Role` taxonomy: Analyst, Implementer, Reviewer, Orchestrator, etc.
//   - `SwarmConfig`: configuration for a swarm instance
//   - `Orchestrator`: coordinates agents, distributes tasks, collects results
//   - Agent registry with capability discovery

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ── Role Taxonomy ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Analyses code, finds issues, suggests improvements.
    Analyst,
    /// Writes new code or modifies existing code.
    Implementer,
    /// Reviews changes for correctness and style.
    Reviewer,
    /// Runs tests, benchmarks, verification.
    Verifier,
    /// Coordinates other agents, splits tasks, resolves conflicts.
    Orchestrator,
    /// Manages documentation and knowledge base.
    Documentarian,
    /// Handles refactoring and restructuring.
    Refactorer,
    /// Security auditing and vulnerability scanning.
    SecurityAuditor,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Analyst => write!(f, "Analyst"),
            Role::Implementer => write!(f, "Implementer"),
            Role::Reviewer => write!(f, "Reviewer"),
            Role::Verifier => write!(f, "Verifier"),
            Role::Orchestrator => write!(f, "Orchestrator"),
            Role::Documentarian => write!(f, "Documentarian"),
            Role::Refactorer => write!(f, "Refactorer"),
            Role::SecurityAuditor => write!(f, "SecurityAuditor"),
        }
    }
}

// ── Agent Descriptor ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentDescriptor {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub capabilities: BTreeSet<String>,
    pub max_concurrent_tasks: usize,
}

impl AgentDescriptor {
    pub fn new(id: impl Into<String>, name: impl Into<String>, role: Role) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            role,
            capabilities: BTreeSet::new(),
            max_concurrent_tasks: 1,
        }
    }

    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.insert(cap.into());
        self
    }

    pub fn with_max_tasks(mut self, n: usize) -> Self {
        self.max_concurrent_tasks = n;
        self
    }
}

// ── Task Result ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskResult {
    Success(String),
    Failure(String),
    NeedsReview(String),
}

impl fmt::Display for TaskResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskResult::Success(msg) => write!(f, "✓ {msg}"),
            TaskResult::Failure(msg) => write!(f, "✗ {msg}"),
            TaskResult::NeedsReview(msg) => write!(f, "⟳ {msg}"),
        }
    }
}

// ── Swarm Agent Trait ──────────────────────────────────────────────

/// The core trait every swarm agent implements.
pub trait SwarmAgent {
    fn descriptor(&self) -> &AgentDescriptor;
    fn handle_task(&mut self, task_name: &str, payload: &str) -> TaskResult;
    fn heartbeat(&self) -> bool { true }
}

// ── Swarm Config ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub name: String,
    pub max_agents: usize,
    pub task_timeout_ticks: u64,
    pub require_review: bool,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            name: "mechgen-swarm".into(),
            max_agents: 16,
            task_timeout_ticks: 100,
            require_review: true,
        }
    }
}

// ── Assignment Record ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Assignment {
    task_name: String,
    agent_id: String,
    result: Option<TaskResult>,
}

// ── Orchestrator ───────────────────────────────────────────────────

pub struct Orchestrator {
    pub config: SwarmConfig,
    agents: Vec<Box<dyn SwarmAgent>>,
    assignments: Vec<Assignment>,
    round: u64,
}

impl Orchestrator {
    pub fn new(config: SwarmConfig) -> Self {
        Self {
            config,
            agents: Vec::new(),
            assignments: Vec::new(),
            round: 0,
        }
    }

    /// Register an agent with the orchestrator.
    pub fn add_agent(&mut self, agent: Box<dyn SwarmAgent>) -> Result<(), String> {
        if self.agents.len() >= self.config.max_agents {
            return Err("max agents reached".into());
        }
        self.agents.push(agent);
        Ok(())
    }

    /// Number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Find agents matching a role.
    pub fn agents_by_role(&self, role: Role) -> Vec<&AgentDescriptor> {
        self.agents.iter()
            .map(|a| a.descriptor())
            .filter(|d| d.role == role)
            .collect()
    }

    /// Find agents with a specific capability.
    pub fn agents_with_capability(&self, cap: &str) -> Vec<&AgentDescriptor> {
        self.agents.iter()
            .map(|a| a.descriptor())
            .filter(|d| d.capabilities.contains(cap))
            .collect()
    }

    /// Dispatch a task to the first capable agent of the given role.
    pub fn dispatch(&mut self, task_name: &str, payload: &str, role: Role) -> Option<TaskResult> {
        self.round += 1;
        let idx = self.agents.iter()
            .position(|a| a.descriptor().role == role)?;
        let result = self.agents[idx].handle_task(task_name, payload);
        self.assignments.push(Assignment {
            task_name: task_name.into(),
            agent_id: self.agents[idx].descriptor().id.clone(),
            result: Some(result.clone()),
        });
        Some(result)
    }

    /// Dispatch and auto-review: if config requires review, send result to a Reviewer.
    pub fn dispatch_with_review(
        &mut self,
        task_name: &str,
        payload: &str,
        role: Role,
    ) -> Option<(TaskResult, Option<TaskResult>)> {
        let result = self.dispatch(task_name, payload, role)?;
        if self.config.require_review {
            let review = self.dispatch(
                &format!("review:{task_name}"),
                &result.to_string(),
                Role::Reviewer,
            );
            Some((result, review))
        } else {
            Some((result, None))
        }
    }

    /// Check all agents are alive.
    pub fn health_check(&self) -> BTreeMap<String, bool> {
        self.agents.iter()
            .map(|a| (a.descriptor().id.clone(), a.heartbeat()))
            .collect()
    }

    /// Completed assignments.
    pub fn completed_assignments(&self) -> Vec<(&str, &str, &TaskResult)> {
        self.assignments.iter()
            .filter_map(|a| {
                a.result.as_ref().map(|r| (a.task_name.as_str(), a.agent_id.as_str(), r))
            })
            .collect()
    }

    /// Current round number.
    pub fn round(&self) -> u64 {
        self.round
    }

    /// JSON snapshot.
    pub fn to_json(&self) -> String {
        let agents: Vec<String> = self.agents.iter()
            .map(|a| {
                let d = a.descriptor();
                let caps: Vec<String> = d.capabilities.iter().map(|c| format!("\"{}\"", c)).collect();
                format!(
                    "{{\"id\":\"{}\",\"name\":\"{}\",\"role\":\"{}\",\"capabilities\":[{}]}}",
                    d.id, d.name, d.role, caps.join(",")
                )
            })
            .collect();
        format!(
            "{{\"swarm\":\"{}\",\"agents\":[{}],\"assignments\":{},\"round\":{}}}",
            self.config.name,
            agents.join(","),
            self.assignments.len(),
            self.round
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test agent.
    struct TestAgent {
        desc: AgentDescriptor,
    }

    impl TestAgent {
        fn new(id: &str, role: Role) -> Self {
            Self {
                desc: AgentDescriptor::new(id, id, role),
            }
        }

        fn with_cap(id: &str, role: Role, cap: &str) -> Self {
            Self {
                desc: AgentDescriptor::new(id, id, role).with_capability(cap),
            }
        }
    }

    impl SwarmAgent for TestAgent {
        fn descriptor(&self) -> &AgentDescriptor {
            &self.desc
        }

        fn handle_task(&mut self, task_name: &str, _payload: &str) -> TaskResult {
            if task_name.starts_with("review:") {
                TaskResult::Success(format!("reviewed {task_name}"))
            } else {
                TaskResult::Success(format!("completed {task_name}"))
            }
        }
    }

    struct FailingAgent { desc: AgentDescriptor }
    impl FailingAgent {
        fn new(id: &str) -> Self {
            Self { desc: AgentDescriptor::new(id, id, Role::Implementer) }
        }
    }
    impl SwarmAgent for FailingAgent {
        fn descriptor(&self) -> &AgentDescriptor { &self.desc }
        fn handle_task(&mut self, task_name: &str, _: &str) -> TaskResult {
            TaskResult::Failure(format!("failed {task_name}"))
        }
    }

    fn orch() -> Orchestrator {
        Orchestrator::new(SwarmConfig::default())
    }

    // ── Agent registration ────────────────────────────────────────

    #[test]
    fn register_agents() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("a1", Role::Analyst))).unwrap();
        o.add_agent(Box::new(TestAgent::new("a2", Role::Implementer))).unwrap();
        assert_eq!(o.agent_count(), 2);
    }

    #[test]
    fn max_agents_enforced() {
        let mut o = Orchestrator::new(SwarmConfig { max_agents: 1, ..Default::default() });
        o.add_agent(Box::new(TestAgent::new("a1", Role::Analyst))).unwrap();
        assert!(o.add_agent(Box::new(TestAgent::new("a2", Role::Analyst))).is_err());
    }

    // ── Role queries ──────────────────────────────────────────────

    #[test]
    fn agents_by_role() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("a1", Role::Analyst))).unwrap();
        o.add_agent(Box::new(TestAgent::new("a2", Role::Implementer))).unwrap();
        o.add_agent(Box::new(TestAgent::new("a3", Role::Analyst))).unwrap();
        assert_eq!(o.agents_by_role(Role::Analyst).len(), 2);
        assert_eq!(o.agents_by_role(Role::Reviewer).len(), 0);
    }

    // ── Capability queries ────────────────────────────────────────

    #[test]
    fn agents_with_capability() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::with_cap("a1", Role::Analyst, "parse"))).unwrap();
        o.add_agent(Box::new(TestAgent::with_cap("a2", Role::Analyst, "typecheck"))).unwrap();
        assert_eq!(o.agents_with_capability("parse").len(), 1);
    }

    // ── Dispatch ──────────────────────────────────────────────────

    #[test]
    fn dispatch_to_role() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("impl1", Role::Implementer))).unwrap();
        let result = o.dispatch("write_function", "payload", Role::Implementer).unwrap();
        assert!(matches!(result, TaskResult::Success(_)));
    }

    #[test]
    fn dispatch_no_matching_role() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("a1", Role::Analyst))).unwrap();
        assert!(o.dispatch("task", "", Role::Implementer).is_none());
    }

    // ── Dispatch with review ──────────────────────────────────────

    #[test]
    fn dispatch_with_review() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("impl1", Role::Implementer))).unwrap();
        o.add_agent(Box::new(TestAgent::new("rev1", Role::Reviewer))).unwrap();
        let (result, review) = o.dispatch_with_review("task", "", Role::Implementer).unwrap();
        assert!(matches!(result, TaskResult::Success(_)));
        assert!(review.is_some());
    }

    #[test]
    fn dispatch_without_review_config() {
        let mut o = Orchestrator::new(SwarmConfig { require_review: false, ..Default::default() });
        o.add_agent(Box::new(TestAgent::new("impl1", Role::Implementer))).unwrap();
        let (result, review) = o.dispatch_with_review("task", "", Role::Implementer).unwrap();
        assert!(matches!(result, TaskResult::Success(_)));
        assert!(review.is_none());
    }

    // ── Failure handling ──────────────────────────────────────────

    #[test]
    fn failing_agent_reports_failure() {
        let mut o = orch();
        o.add_agent(Box::new(FailingAgent::new("fail1"))).unwrap();
        let result = o.dispatch("task", "", Role::Implementer).unwrap();
        assert!(matches!(result, TaskResult::Failure(_)));
    }

    // ── Health check ──────────────────────────────────────────────

    #[test]
    fn health_check_all_alive() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("a1", Role::Analyst))).unwrap();
        o.add_agent(Box::new(TestAgent::new("a2", Role::Implementer))).unwrap();
        let health = o.health_check();
        assert!(health.values().all(|v| *v));
    }

    // ── Completed assignments ─────────────────────────────────────

    #[test]
    fn completed_assignments_tracked() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("impl1", Role::Implementer))).unwrap();
        o.dispatch("t1", "", Role::Implementer);
        o.dispatch("t2", "", Role::Implementer);
        assert_eq!(o.completed_assignments().len(), 2);
    }

    // ── Rounds ────────────────────────────────────────────────────

    #[test]
    fn round_counter() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::new("impl1", Role::Implementer))).unwrap();
        o.dispatch("t1", "", Role::Implementer);
        o.dispatch("t2", "", Role::Implementer);
        assert_eq!(o.round(), 2);
    }

    // ── JSON ──────────────────────────────────────────────────────

    #[test]
    fn json_output() {
        let mut o = orch();
        o.add_agent(Box::new(TestAgent::with_cap("a1", Role::Analyst, "parse"))).unwrap();
        let json = o.to_json();
        assert!(json.contains("\"role\":\"Analyst\""));
        assert!(json.contains("\"parse\""));
    }

    // ── Agent descriptor builder ──────────────────────────────────

    #[test]
    fn descriptor_builder() {
        let d = AgentDescriptor::new("id", "name", Role::Verifier)
            .with_capability("test")
            .with_capability("bench")
            .with_max_tasks(4);
        assert_eq!(d.capabilities.len(), 2);
        assert_eq!(d.max_concurrent_tasks, 4);
    }

    // ── Role display ──────────────────────────────────────────────

    #[test]
    fn role_display() {
        assert_eq!(format!("{}", Role::SecurityAuditor), "SecurityAuditor");
        assert_eq!(format!("{}", Role::Orchestrator), "Orchestrator");
    }
}
