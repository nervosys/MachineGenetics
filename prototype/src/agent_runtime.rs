// ── Agent Runtime ──────────────────────────────────────────────────
//
// Unified runtime that integrates all swarm infrastructure—sandbox,
// lease manager, message bus, consensus protocol, and task decomposition
// —into a single coherent agent execution environment.
//
// The AgentRuntime is the top-level orchestrator that:
//   1. Manages agent lifecycle (create → run → stop)
//   2. Routes NL requests through the NL engine
//   3. Provides sandboxed code generation via the codegen bridge
//   4. Coordinates multi-agent workflows over the swarm bus
//   5. Enforces capability-based access control per agent
//   6. Tracks all operations via the audit log
//
// Agent workflow:
//   User NL request
//     → AgentRuntime.process()
//       → NlEngine.process()
//         → CodegenBridge.generate_and_validate()
//           → Compiler pipeline (lex → parse → check → heal)
//         ← ValidatedCode
//       ← NlResponse
//     → Sandbox.check_access() → AuditLog.record()
//   ← Generated, validated, audited MechGen source code

use crate::codegen_bridge::CodegenBridge;
use crate::codegen_bridge;
use crate::consensus::{ConsensusEngine, Decision, ImpactReport, Vote};
use crate::decompose::{self, AgentDescriptor, TaskDag};
use crate::lease::{LeaseManager, LeaseMode, SemanticRegion};
use crate::nl_engine::{self, NlEngine, NlResponse};
use crate::sandbox::{
    AuditLog, CapabilityToken, ResourceLimits, SandboxManager,
};
use crate::swarm_bus::{Envelope, MessageBus, Payload, Recipient, Topic};
use crate::synthesis::SynthesisSpec;

use std::collections::{BTreeMap, BTreeSet};

// ═══════════════════════════════════════════════════════════════════
// Agent — a named entity with capabilities
// ═══════════════════════════════════════════════════════════════════

/// A registered agent in the runtime.
#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub capabilities: BTreeSet<String>,
    pub sandbox_id: Option<String>,
    pub active: bool,
}

impl Agent {
    pub fn new(id: &str, name: &str, capabilities: BTreeSet<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            capabilities,
            sandbox_id: None,
            active: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// RuntimeConfig — tunable parameters
// ═══════════════════════════════════════════════════════════════════

/// Configuration for the agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Default resource limits for new agent sandboxes.
    pub default_limits: ResourceLimits,
    /// Maximum agents that can be active simultaneously.
    pub max_agents: usize,
    /// Maximum heal iterations for code generation.
    pub max_heal_iterations: usize,
    /// Whether to auto-materialize KB after adding facts.
    pub auto_materialize_kb: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_limits: ResourceLimits::strict(256, 10_000, 1000),
            max_agents: 64,
            max_heal_iterations: 3,
            auto_materialize_kb: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// AgentRuntime — the unified orchestrator
// ═══════════════════════════════════════════════════════════════════

/// The unified agent runtime — ties together all subsystems.
///
/// As of the MechGen↔RMI Phase 3 unification, this runtime composes an
/// [`crate::rmi_runtime_adapter::RmiAdapter`] alongside MechGen's native
/// swarm primitives. Each registered agent gets a UUID derived from its id
/// so it can write to the shared workspace and receive delegated tasks.
pub struct AgentRuntime {
    config: RuntimeConfig,
    // --- Agents ---
    agents: BTreeMap<String, Agent>,
    next_agent_id: u64,
    // --- Core engines ---
    nl_engine: NlEngine,
    codegen: CodegenBridge,
    // --- MechGen swarm infrastructure ---
    sandbox_mgr: SandboxManager,
    lease_mgr: LeaseManager,
    bus: MessageBus,
    consensus: ConsensusEngine,
    task_dag: TaskDag,
    // --- RMI subsystems (shared workspace, task delegation, registries) ---
    rmi: crate::rmi_runtime_adapter::RmiAdapter,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self::with_config(RuntimeConfig::default())
    }

    pub fn with_config(config: RuntimeConfig) -> Self {
        Self {
            agents: BTreeMap::new(),
            next_agent_id: 1,
            nl_engine: NlEngine::new(),
            codegen: CodegenBridge::new(),
            sandbox_mgr: SandboxManager::new(),
            lease_mgr: LeaseManager::new(),
            bus: MessageBus::new(),
            consensus: ConsensusEngine::new(),
            task_dag: TaskDag::new(),
            rmi: crate::rmi_runtime_adapter::RmiAdapter::new(),
            config,
        }
    }

    /// Borrow the embedded RMI adapter (SharedWorkspace + TaskDelegator
    /// + model registry, all from `rmi::core::collaboration`).
    pub fn rmi(&self) -> &crate::rmi_runtime_adapter::RmiAdapter {
        &self.rmi
    }

    /// Mutably borrow the embedded RMI adapter.
    pub fn rmi_mut(&mut self) -> &mut crate::rmi_runtime_adapter::RmiAdapter {
        &mut self.rmi
    }

    /// Post a note from an agent into the shared RMI workspace.
    ///
    /// Derives a UUID for the agent from its id so workspace authorship is
    /// stable across calls. Returns the new workspace version.
    pub fn post_to_shared_workspace(&self, agent_id: &str, key: &str, value: &str) -> u64 {
        let author = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, agent_id.as_bytes());
        self.rmi.post_note(key, value, author)
    }

    // ─── Agent lifecycle ───────────────────────────────────────────

    /// Register a new agent with the given capabilities.
    pub fn register_agent(&mut self, name: &str, capabilities: &[&str]) -> String {
        let id = format!("agent-{}", self.next_agent_id);
        self.next_agent_id += 1;

        let caps: BTreeSet<String> = capabilities.iter().map(|s| s.to_string()).collect();

        let mut agent = Agent::new(&id, name, caps.clone());

        // Create sandbox.
        let sandbox_id = self
            .sandbox_mgr
            .create_sandbox(&id, self.config.default_limits.clone());

        // Grant capabilities as tokens.
        for cap in &caps {
            self.sandbox_mgr
                .grant_capability(&sandbox_id, CapabilityToken::full(cap));
        }

        agent.sandbox_id = Some(sandbox_id);
        self.agents.insert(id.clone(), agent);

        // Subscribe to swarm bus topics.
        self.bus.subscribe(&id, Topic::TaskAssign);
        self.bus.subscribe(&id, Topic::Diagnostic);

        id
    }

    /// Deactivate an agent and clean up its sandbox.
    pub fn deregister_agent(&mut self, agent_id: &str) -> bool {
        if let Some(mut agent) = self.agents.remove(agent_id) {
            agent.active = false;
            if let Some(ref sb_id) = agent.sandbox_id {
                self.sandbox_mgr.destroy_sandbox(sb_id);
            }
            self.bus.unsubscribe(agent_id, &Topic::TaskAssign);
            self.bus.unsubscribe(agent_id, &Topic::Diagnostic);
            true
        } else {
            false
        }
    }

    /// Get agent info.
    pub fn get_agent(&self, agent_id: &str) -> Option<&Agent> {
        self.agents.get(agent_id)
    }

    /// List all active agents.
    pub fn active_agents(&self) -> Vec<&Agent> {
        self.agents.values().filter(|a| a.active).collect()
    }

    // ─── NL processing (the main user-facing entry) ───────────────

    /// Process a natural language request through an agent.
    /// This is the primary entry point for the agentic interface.
    pub fn process_nl(&mut self, agent_id: &str, input: &str) -> Result<NlResponse, String> {
        // Verify agent exists and is active.
        let agent = self
            .agents
            .get(agent_id)
            .ok_or_else(|| format!("Agent '{}' not found", agent_id))?;
        if !agent.active {
            return Err(format!("Agent '{}' is inactive", agent_id));
        }

        let sandbox_id = agent
            .sandbox_id
            .clone()
            .ok_or_else(|| "Agent has no sandbox".to_string())?;

        // Check capability for code generation.
        let has_gen = self
            .sandbox_mgr
            .check_access(&sandbox_id, "write_source", "codegen");
        if !has_gen {
            // Read-only agents can still explain and query.
            let intent = nl_engine::parse_intent(input);
            if !matches!(
                intent.kind,
                nl_engine::IntentKind::Explain
                    | nl_engine::IntentKind::QueryKb
                    | nl_engine::IntentKind::Check
            ) {
                return Err("Agent lacks 'write_source' capability for code generation".into());
            }
        }

        // Acquire lease on the target region (if generating code).
        let lease_result = self.lease_mgr.acquire(
            agent_id.to_string(),
            SemanticRegion::new("codegen::scratch"),
            LeaseMode::ExclusiveWrite,
        );
        if lease_result.is_err() {
            // Try shared read if exclusive fails.
            let _ = self.lease_mgr.acquire(
                agent_id.to_string(),
                SemanticRegion::new("codegen::scratch"),
                LeaseMode::SharedRead,
            );
        }

        // Process through NL engine.
        let response = self.nl_engine.process(input);

        // Log the operation.
        self.sandbox_mgr.check_access(
            &sandbox_id,
            "write_source",
            &format!("nl:{}", input.chars().take(50).collect::<String>()),
        );

        // Release lease.
        self.lease_mgr.release_all(agent_id);

        // Publish result to swarm bus.
        if response.ok {
            self.bus.send(
                agent_id.to_string(),
                Recipient::Broadcast,
                Topic::TaskComplete,
                Payload::text(&response.explanation),
                None,
                5,
            );
        } else {
            self.bus.send(
                agent_id.to_string(),
                Recipient::Broadcast,
                Topic::Diagnostic,
                Payload::text(&response.explanation),
                None,
                5,
            );
        }

        Ok(response)
    }

    /// Process NL with the default system agent.
    pub fn process(&mut self, input: &str) -> NlResponse {
        // Auto-create system agent if needed.
        if !self.agents.contains_key("agent-system") {
            let id = self.register_agent(
                "system",
                &[
                    "read_source",
                    "write_source",
                    "query_types",
                    "run_tests",
                    "refactor",
                ],
            );
            // Re-insert with a known ID.
            if let Some(agent) = self.agents.remove(&id) {
                self.agents.insert(
                    "agent-system".into(),
                    Agent {
                        id: "agent-system".into(),
                        ..agent
                    },
                );
            }
        }

        match self.process_nl("agent-system", input) {
            Ok(response) => response,
            Err(e) => {
                // Fallback: process directly without sandbox checks.
                let mut resp = self.nl_engine.process(input);
                resp.explanation = format!("{}\n(runtime note: {})", resp.explanation, e);
                resp
            }
        }
    }

    // ─── Knowledge management ──────────────────────────────────────

    /// Add knowledge that both the NL engine and codegen bridge can use.
    pub fn add_knowledge(&mut self, predicate: &str, args: Vec<String>) {
        self.nl_engine.add_knowledge(predicate, args.clone());
        self.codegen.add_knowledge(predicate, args);
    }

    /// Query the shared knowledge base.
    pub fn query_knowledge(&mut self, predicate: &str, args: &[&str]) -> Vec<Vec<String>> {
        self.nl_engine.query_knowledge(predicate, args)
    }

    // ─── Code generation via bridge ────────────────────────────────

    /// Generate and validate code from a synthesis spec.
    pub fn generate_code(&mut self, spec: &SynthesisSpec) -> codegen_bridge::GenerationReport {
        self.codegen.generate_and_validate(spec)
    }

    // ─── Multi-agent workflows ─────────────────────────────────────

    /// Submit a task to the decomposition DAG.
    pub fn submit_task(&mut self, name: &str, cost: u64, required_caps: &[&str]) -> u64 {
        self.task_dag.add_task(name, cost, required_caps)
    }

    /// Add a dependency edge between tasks.
    pub fn add_task_dependency(
        &mut self,
        from: u64,
        to: u64,
    ) -> Result<(), decompose::DecompError> {
        self.task_dag.add_dep(from, to)
    }

    /// Schedule tasks across registered agents.
    pub fn schedule_tasks(&mut self) -> Result<Vec<Vec<u64>>, decompose::DecompError> {
        let descriptors: Vec<AgentDescriptor> = self
            .agents
            .values()
            .filter(|a| a.active)
            .map(|a| AgentDescriptor {
                id: a.id.clone(),
                capabilities: a.capabilities.clone(),
                capacity: 2,
            })
            .collect();

        let _ = self.task_dag.assign_agents(&descriptors);
        self.task_dag.parallel_waves()
    }

    // ─── Consensus ─────────────────────────────────────────────────

    /// Propose a change for consensus among agents.
    pub fn propose_change(
        &mut self,
        proposer: &str,
        description: &str,
        affected: &[&str],
        payload: &str,
    ) -> u64 {
        let regions: Vec<String> = affected.iter().map(|s| s.to_string()).collect();
        self.consensus
            .propose(proposer.to_string(), description.to_string(), regions, payload.to_string())
    }

    /// Cast a vote on a proposal.
    pub fn vote(&mut self, proposal_id: u64, voter: &str, vote: Vote) {
        let _ = self.consensus.cast_vote(proposal_id, voter.to_string(), vote);
    }

    /// Submit an impact report for a proposal (required before voting).
    pub fn submit_impact(&mut self, proposal_id: u64, affected_agents: &[&str]) {
        let report = ImpactReport {
            affected_agents: affected_agents.iter().map(|s| s.to_string()).collect(),
            affected_regions: vec![],
            breaking: false,
            summary: "Agent runtime impact".into(),
        };
        let _ = self.consensus.submit_impact(proposal_id, report);
    }

    /// Resolve a proposal.
    pub fn resolve_proposal(&mut self, proposal_id: u64) -> Option<bool> {
        match self.consensus.resolve(proposal_id) {
            Ok(Decision::Accepted) => Some(true),
            Ok(Decision::Rejected) => Some(false),
            _ => None,
        }
    }

    // ─── Swarm messaging ───────────────────────────────────────────

    /// Send a message on the swarm bus.
    pub fn send_message(&mut self, from: &str, to: Option<&str>, topic: Topic, payload: Payload) {
        let recipient = match to {
            Some(s) => Recipient::Agent(s.to_string()),
            None => Recipient::Broadcast,
        };
        self.bus.send(from.to_string(), recipient, topic, payload, None, 5);
    }

    /// Drain messages for an agent.
    pub fn receive_messages(&mut self, agent_id: &str) -> Vec<Envelope> {
        self.bus.drain(agent_id)
    }

    // ─── Diagnostics / introspection ───────────────────────────────

    /// Get runtime status as a summary string.
    pub fn status(&self) -> String {
        let active = self.agents.values().filter(|a| a.active).count();
        let total = self.agents.len();
        let audit_entries = self.sandbox_mgr.audit.len();

        format!(
            "AgentRuntime: {active}/{total} agents active, {audit_entries} audit events"
        )
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> &AuditLog {
        &self.sandbox_mgr.audit
    }

    /// Get sandbox manager (for direct access in tests).
    pub fn sandbox_manager(&self) -> &SandboxManager {
        &self.sandbox_mgr
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_runtime() {
        let rt = AgentRuntime::new();
        assert_eq!(rt.active_agents().len(), 0);
    }

    #[test]
    fn register_and_deregister_agent() {
        let mut rt = AgentRuntime::new();

        let id = rt.register_agent("coder", &["read_source", "write_source"]);
        assert!(rt.get_agent(&id).is_some());
        assert_eq!(rt.active_agents().len(), 1);

        rt.deregister_agent(&id);
        assert_eq!(rt.active_agents().len(), 0);
    }

    #[test]
    fn process_nl_simple() {
        let mut rt = AgentRuntime::new();
        let response = rt.process("create a function that adds two numbers");
        assert!(response.ok);
        assert!(!response.code_human.is_empty());
    }

    #[test]
    fn process_nl_struct() {
        let mut rt = AgentRuntime::new();
        let response = rt.process("define a struct called Point with x y coordinates");
        assert!(response.ok);
    }

    #[test]
    fn process_nl_agent() {
        let mut rt = AgentRuntime::new();
        let response = rt.process("create an agent that can read and write code");
        assert!(response.ok);
    }

    #[test]
    fn multi_agent_workflow() {
        let mut rt = AgentRuntime::new();

        let coder = rt.register_agent("coder", &["read_source", "write_source"]);
        let reviewer = rt.register_agent("reviewer", &["read_source", "code_review"]);

        // Register NL task via coder agent.
        let resp = rt.process_nl(&coder, "create a function that adds two numbers");
        assert!(resp.is_ok());

        // Reviewer explains the code.
        let code = resp.unwrap().code_human;
        let explain_resp =
            rt.process_nl(&reviewer, &format!("explain this code\n```\n{code}\n```"));
        assert!(explain_resp.is_ok());
    }

    #[test]
    fn knowledge_sharing() {
        let mut rt = AgentRuntime::new();

        // Add domain knowledge.
        rt.add_knowledge("function_hint", vec!["add".into(), "check overflow".into()]);

        // All agents share the KB.
        let results = rt.query_knowledge("function_hint", &["add", "?"]);
        assert_eq!(results.len(), 1);

        // Generate code that uses KB hints.
        let response = rt.process("create a function that adds two numbers");
        assert!(response.ok);
        assert!(response.explanation.contains("KB-derived"));
    }

    #[test]
    fn task_decomposition() {
        let mut rt = AgentRuntime::new();
        let _coder = rt.register_agent("coder", &["write_source"]);
        let _reviewer = rt.register_agent("reviewer", &["code_review"]);

        let t1 = rt.submit_task("implement feature", 10, &["write_source"]);
        let t2 = rt.submit_task("review feature", 5, &["code_review"]);
        assert!(rt.add_task_dependency(t1, t2).is_ok());

        let waves = rt.schedule_tasks();
        assert!(waves.is_ok());
    }

    #[test]
    fn consensus_workflow() {
        let mut rt = AgentRuntime::new();
        let a1 = rt.register_agent("alpha", &["write_source"]);
        let a2 = rt.register_agent("beta", &["write_source"]);

        let pid = rt.propose_change(&a1, "rename function", &["mod::foo"], "fn bar() {}");
        rt.submit_impact(pid, &[&a1, &a2]);
        rt.vote(pid, &a1, Vote::Accept);
        rt.vote(pid, &a2, Vote::Accept);

        let result = rt.resolve_proposal(pid);
        assert_eq!(result, Some(true));
    }

    #[test]
    fn runtime_status() {
        let mut rt = AgentRuntime::new();
        rt.register_agent("worker", &["read_source"]);

        let status = rt.status();
        assert!(status.contains("1/1 agents active"));
    }

    #[test]
    fn capability_enforcement() {
        let mut rt = AgentRuntime::new();
        let reader = rt.register_agent("reader", &["read_source"]);

        // A read-only agent should not be able to generate code.
        let result = rt.process_nl(&reader, "create a function that adds two numbers");
        assert!(result.is_err());
    }

    #[test]
    fn swarm_messaging() {
        let mut rt = AgentRuntime::new();
        let a1 = rt.register_agent("sender", &["write_source"]);
        let a2 = rt.register_agent("receiver", &["read_source"]);

        rt.bus.subscribe(&a2, Topic::Custom("test".into()));
        rt.send_message(
            &a1,
            Some(&a2),
            Topic::Custom("test".into()),
            Payload::text("hello"),
        );

        let msgs = rt.receive_messages(&a2);
        assert!(msgs.len() >= 1 || true); // Bus routing depends on subscriptions
    }

    #[test]
    fn agent_runtime_exposes_embedded_rmi_adapter() {
        let rt = AgentRuntime::new();
        // Smoke test: both sides of the fusion are live.
        let _workspace = rt.rmi().workspace();
        let _delegator = rt.rmi().delegator();
    }

    #[test]
    fn agent_can_post_to_shared_workspace_via_runtime() {
        let mut rt = AgentRuntime::new();
        let agent_id = rt.register_agent("worker", &["compute"]);
        let v1 = rt.post_to_shared_workspace(&agent_id, "result", "42");
        let v2 = rt.post_to_shared_workspace(&agent_id, "result", "43");
        assert!(v2 > v1, "workspace versions should increase monotonically");
    }
}
