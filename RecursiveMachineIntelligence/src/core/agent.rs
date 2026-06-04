//! Agent System - Autonomous AI Entities
//!
//! Agents in air are autonomous computational entities designed to:
//! 1. Reason about and compose primitives
//! 2. Communicate efficiently with other agents
//! 3. Collaborate on complex tasks without human intervention
//! 4. Self-organize into hierarchies and networks
//!
//! Unlike human-operated systems, agents use maximally efficient binary
//! protocols and can directly share tensors, gradients, and neural states.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::core::ontology::{Concept, ConceptId, Ontology};
use crate::core::primitives::{AlgebraicProperty, Primitive, PrimitiveRegistry, PrimitiveType};
use crate::core::protocol::Message;
use crate::error::{Result, RmiError};

/// Agent lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AgentState {
    /// Agent is starting up
    Initializing = 0x01,
    /// Ready for tasks
    Idle = 0x02,
    /// Processing/thinking
    Reasoning = 0x03,
    /// Running computations
    Executing = 0x04,
    /// Exchanging with other agents
    Communicating = 0x05,
    /// Updating internal models
    Learning = 0x06,
    /// Blocked on dependency
    Waiting = 0x07,
    /// Encountered an error
    Error = 0x08,
    /// Shut down
    Terminated = 0x09,
}

impl AgentState {
    /// Convert from raw u8 (used for AtomicU8 storage).
    #[inline]
    fn from_u8(v: u8) -> Self {
        match v {
            0x01 => AgentState::Initializing,
            0x02 => AgentState::Idle,
            0x03 => AgentState::Reasoning,
            0x04 => AgentState::Executing,
            0x05 => AgentState::Communicating,
            0x06 => AgentState::Learning,
            0x07 => AgentState::Waiting,
            0x08 => AgentState::Error,
            0x09 => AgentState::Terminated,
            _ => AgentState::Error,
        }
    }
}

/// Standard capabilities that agents can possess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum AgentCapability {
    // Architecture and design
    /// Design neural architectures
    ArchitectureSearch = 0x0100,
    /// Optimize hyperparameters
    HyperparameterOptimization = 0x0101,
    /// Design loss functions
    LossFunctionDesign = 0x0102,

    // Training and optimization
    /// Orchestrate training
    TrainingOrchestration = 0x0200,
    /// Compute gradients
    GradientComputation = 0x0201,
    /// Distributed training coordination
    DistributedTraining = 0x0202,

    // Reasoning
    /// Symbolic reasoning
    SymbolicReasoning = 0x0300,
    /// Logical inference
    LogicalInference = 0x0301,
    /// Constraint satisfaction
    ConstraintSatisfaction = 0x0302,
    /// Planning
    Planning = 0x0303,

    // Knowledge
    /// Ontology reasoning
    OntologyReasoning = 0x0400,
    /// Knowledge retrieval
    KnowledgeRetrieval = 0x0401,
    /// Knowledge integration
    KnowledgeIntegration = 0x0402,

    // Evaluation
    /// Model evaluation
    ModelEvaluation = 0x0500,
    /// Benchmark execution
    BenchmarkExecution = 0x0501,

    // Meta
    /// Self-improvement
    SelfImprovement = 0x0600,
    /// Agent coordination
    AgentCoordination = 0x0601,
    /// Resource management
    ResourceManagement = 0x0602,
}

/// Unique identity for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Unique identifier
    pub uuid: Uuid,
    /// Public key for signed messages (optional)
    pub public_key: Option<Vec<u8>>,
    /// Semantic name
    pub name: String,
    /// Role description
    pub role: String,
    /// Version
    pub version: String,
    /// Advertised capabilities
    pub capabilities: HashSet<AgentCapability>,
}

impl AgentIdentity {
    /// Create a new agent identity.
    pub fn new(name: &str, role: &str) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            public_key: None,
            name: name.to_string(),
            role: role.to_string(),
            version: "0.1.0".to_string(),
            capabilities: HashSet::new(),
        }
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(data).map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

impl Default for AgentIdentity {
    fn default() -> Self {
        Self::new("agent", "general")
    }
}

/// Runtime context for agent execution.
pub struct AgentContext {
    /// Knowledge available to this agent
    pub ontology: Arc<Ontology>,
    /// Primitive registry
    pub primitives: Arc<PrimitiveRegistry>,
    /// Compute backend
    pub backend: Arc<dyn crate::compute::Backend>,
    /// Memory limit in MB
    pub memory_limit_mb: usize,
    /// Compute budget in FLOPs
    pub compute_budget_flops: f64,
    /// Known peer agents
    pub peer_agents: RwLock<HashMap<Uuid, AgentIdentity>>,
    /// Current task ID
    pub current_task: RwLock<Option<String>>,
    /// Metrics
    pub metrics: RwLock<AgentMetrics>,
}

/// Agent execution metrics.
#[derive(Debug, Clone, Default)]
pub struct AgentMetrics {
    /// Total FLOPs used
    pub total_flops_used: f64,
    /// Messages sent
    pub messages_sent: u64,
    /// Messages received
    pub messages_received: u64,
    /// Tasks completed
    pub tasks_completed: u64,
    /// Errors encountered
    pub errors: u64,
    /// Total reasoning time
    pub reasoning_time_ms: u64,
    /// Total execution time
    pub execution_time_ms: u64,
}

/// Abstract goal that an agent pursues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Goal {
    /// Minimize a loss metric
    MinimizeLoss {
        /// Name of the metric to minimize
        metric_name: String,
        /// Optional target value to reach
        target_value: Option<f64>,
        /// Additional constraints (metric_name -> threshold)
        constraints: HashMap<String, f64>,
    },
    /// Maximize a metric
    MaximizeMetric {
        /// Name of the metric to maximize
        metric_name: String,
        /// Optional target value to reach
        target_value: Option<f64>,
        /// Additional constraints (metric_name -> threshold)
        constraints: HashMap<String, f64>,
    },
    /// Search for optimal architecture
    ArchitectureSearch {
        /// Type of task (classification, regression, etc.)
        task_type: String,
        /// Input data schema (name -> type)
        input_schema: HashMap<String, String>,
        /// Output data schema (name -> type)
        output_schema: HashMap<String, String>,
        /// Resource constraints (memory_mb, latency_ms, etc.)
        resource_constraints: HashMap<String, f64>,
    },
    /// Execute inference
    Inference {
        /// Model identifier to use
        model_id: String,
        /// Input data (serialized)
        input_data: Vec<u8>,
    },
    /// Train a model
    Train {
        /// Model identifier to train
        model_id: String,
        /// Data source path or identifier
        data_source: String,
        /// Number of training epochs
        epochs: u32,
        /// Training batch size
        batch_size: u32,
    },
    /// Reason about concepts
    Reason {
        /// Query to reason about
        query: String,
        /// Context concept IDs
        context_concepts: Vec<ConceptId>,
    },
    /// Custom goal with binary specification
    Custom {
        /// Goal type identifier
        goal_type: String,
        /// Binary specification data
        spec: Vec<u8>,
    },
}

impl Goal {
    /// Get the goal type identifier.
    #[inline]
    pub fn goal_type(&self) -> &str {
        match self {
            Goal::MinimizeLoss { .. } => "minimize_loss",
            Goal::MaximizeMetric { .. } => "maximize_metric",
            Goal::ArchitectureSearch { .. } => "architecture_search",
            Goal::Inference { .. } => "inference",
            Goal::Train { .. } => "train",
            Goal::Reason { .. } => "reason",
            Goal::Custom { goal_type, .. } => goal_type,
        }
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(data).map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

/// Result of goal execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalResult {
    /// Status: "success", "partial", "failed"
    pub status: String,
    /// Result data
    pub data: HashMap<String, serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Resources used
    pub resources_used: HashMap<String, f64>,
}

/// Core agent implementation.
pub struct Agent {
    /// Agent identity
    pub identity: AgentIdentity,
    /// Current state (lock-free atomic for zero-contention state checks)
    state: AtomicU8,
    /// Execution context
    context: Arc<AgentContext>,
    /// Current goal
    current_goal: RwLock<Option<Goal>>,
    /// Goal stack for hierarchical execution
    goal_stack: RwLock<Vec<Goal>>,
    /// Execution trace
    execution_trace: RwLock<Vec<TraceEntry>>,
    /// Message channel sender
    message_tx: Option<mpsc::Sender<Message>>,
    /// Message channel receiver
    message_rx: RwLock<Option<mpsc::Receiver<Message>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraceEntry {
    goal_type: String,
    start_time: i64,
    end_time: Option<i64>,
    status: String,
    error: Option<String>,
}

impl Agent {
    /// Create a new agent with a builder.
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Get current state.
    #[inline]
    pub fn state(&self) -> AgentState {
        AgentState::from_u8(self.state.load(Ordering::Acquire))
    }

    /// Set state.
    #[inline]
    fn set_state(&self, state: AgentState) {
        self.state.store(state as u8, Ordering::Release);
    }

    /// Check if agent has a capability.
    #[inline]
    pub fn has_capability(&self, capability: AgentCapability) -> bool {
        self.identity.capabilities.contains(&capability)
    }

    /// Add a capability.
    pub fn add_capability(&mut self, capability: AgentCapability) {
        self.identity.capabilities.insert(capability);
    }

    /// Set the current goal.
    pub fn set_goal(&self, goal: Goal) {
        let mut stack = self.goal_stack.write().unwrap();
        stack.clear();
        *self.current_goal.write().unwrap() = Some(goal.clone());
        stack.push(goal);
    }

    /// Push a sub-goal onto the stack.
    pub fn push_subgoal(&self, subgoal: Goal) {
        *self.current_goal.write().unwrap() = Some(subgoal.clone());
        self.goal_stack.write().unwrap().push(subgoal);
    }

    /// Pop completed goal.
    pub fn pop_goal(&self) -> Option<Goal> {
        let mut stack = self.goal_stack.write().unwrap();
        let popped = stack.pop();
        *self.current_goal.write().unwrap() = stack.last().cloned();
        popped
    }

    /// Execute a goal asynchronously.
    pub async fn execute(&self, goal: Goal) -> Result<GoalResult> {
        let start = Instant::now();
        self.set_state(AgentState::Reasoning);
        self.set_goal(goal.clone());

        let trace_entry = TraceEntry {
            goal_type: goal.goal_type().to_string(),
            start_time: chrono::Utc::now().timestamp(),
            end_time: None,
            status: "started".to_string(),
            error: None,
        };
        self.execution_trace.write().unwrap().push(trace_entry);

        self.set_state(AgentState::Executing);
        let result = self.execute_goal_internal(&goal).await;

        let execution_time = start.elapsed().as_millis() as u64;

        // Update trace
        if let Some(entry) = self.execution_trace.write().unwrap().last_mut() {
            entry.end_time = Some(chrono::Utc::now().timestamp());
            entry.status = if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
            .to_string();
            if let Err(ref e) = result {
                entry.error = Some(e.to_string());
            }
        }

        // Update metrics
        {
            let mut metrics = self.context.metrics.write().unwrap();
            metrics.execution_time_ms += execution_time;
            if result.is_ok() {
                metrics.tasks_completed += 1;
            } else {
                metrics.errors += 1;
            }
        }

        self.set_state(AgentState::Idle);

        result.map(|mut r| {
            r.execution_time_ms = execution_time;
            r
        })
    }

    async fn execute_goal_internal(&self, goal: &Goal) -> Result<GoalResult> {
        match goal {
            Goal::MinimizeLoss {
                metric_name,
                target_value,
                constraints,
            } => {
                self.execute_optimization(metric_name, *target_value, constraints, true)
                    .await
            }
            Goal::MaximizeMetric {
                metric_name,
                target_value,
                constraints,
            } => {
                self.execute_optimization(metric_name, *target_value, constraints, false)
                    .await
            }
            Goal::ArchitectureSearch {
                task_type,
                resource_constraints,
                ..
            } => {
                self.execute_architecture_search(task_type, resource_constraints)
                    .await
            }
            Goal::Reason {
                query,
                context_concepts,
            } => self.execute_reasoning(query, context_concepts).await,
            _ => Ok(GoalResult {
                status: "success".to_string(),
                data: HashMap::new(),
                error: None,
                execution_time_ms: 0,
                resources_used: HashMap::new(),
            }),
        }
    }

    async fn execute_optimization(
        &self,
        metric_name: &str,
        target_value: Option<f64>,
        _constraints: &HashMap<String, f64>,
        minimize: bool,
    ) -> Result<GoalResult> {
        // Placeholder optimization logic
        let mut data = HashMap::new();
        data.insert("metric".to_string(), serde_json::json!(metric_name));
        data.insert("minimize".to_string(), serde_json::json!(minimize));
        data.insert("target".to_string(), serde_json::json!(target_value));

        Ok(GoalResult {
            status: "success".to_string(),
            data,
            error: None,
            execution_time_ms: 0,
            resources_used: HashMap::new(),
        })
    }

    async fn execute_architecture_search(
        &self,
        task_type: &str,
        _constraints: &HashMap<String, f64>,
    ) -> Result<GoalResult> {
        // Query primitives for architecture building blocks
        let neural_primitives = self
            .context
            .primitives
            .query_by_type(PrimitiveType::NeuralLinear);
        let nonlinear_primitives = self
            .context
            .primitives
            .query_by_type(PrimitiveType::NeuralNonlinear);

        let mut data = HashMap::new();
        data.insert("task_type".to_string(), serde_json::json!(task_type));
        data.insert(
            "available_linear_ops".to_string(),
            serde_json::json!(neural_primitives.len()),
        );
        data.insert(
            "available_nonlinear_ops".to_string(),
            serde_json::json!(nonlinear_primitives.len()),
        );

        Ok(GoalResult {
            status: "success".to_string(),
            data,
            error: None,
            execution_time_ms: 0,
            resources_used: HashMap::new(),
        })
    }

    async fn execute_reasoning(
        &self,
        query: &str,
        context_concepts: &[ConceptId],
    ) -> Result<GoalResult> {
        self.set_state(AgentState::Reasoning);

        let mut inferences = Vec::new();

        // Gather related concepts
        for concept_id in context_concepts {
            if let Some(concept) = self.context.ontology.get(concept_id) {
                inferences.push(serde_json::json!({
                    "concept": concept.label,
                    "type": format!("{:?}", concept.concept_type),
                    "confidence": concept.confidence,
                }));
            }
        }

        let mut data = HashMap::new();
        data.insert("query".to_string(), serde_json::json!(query));
        data.insert("inferences".to_string(), serde_json::json!(inferences));

        Ok(GoalResult {
            status: "success".to_string(),
            data,
            error: None,
            execution_time_ms: 0,
            resources_used: HashMap::new(),
        })
    }

    /// Query available primitives.
    pub fn query_primitives(
        &self,
        required_properties: Option<&HashSet<AlgebraicProperty>>,
        primitive_type: Option<PrimitiveType>,
    ) -> Vec<Arc<dyn Primitive>> {
        if let Some(props) = required_properties {
            self.context.primitives.query_by_properties(props, None)
        } else if let Some(ptype) = primitive_type {
            self.context.primitives.query_by_type(ptype)
        } else {
            Vec::new()
        }
    }

    /// Reason about a concept.
    pub fn reason_about(&self, concept: &Concept) -> HashMap<String, serde_json::Value> {
        let mut result = HashMap::new();

        result.insert(
            "concept_id".to_string(),
            serde_json::json!(concept.id.uri()),
        );
        result.insert(
            "type".to_string(),
            serde_json::json!(format!("{:?}", concept.concept_type)),
        );
        result.insert(
            "confidence".to_string(),
            serde_json::json!(concept.confidence),
        );

        // Get related concepts
        let related_is_a = self
            .context
            .ontology
            .get_related(&concept.id, crate::core::ontology::RelationType::IsA);
        result.insert(
            "parent_concepts".to_string(),
            serde_json::json!(related_is_a
                .iter()
                .map(|c| c.label.clone())
                .collect::<Vec<_>>()),
        );

        result
    }

    /// Send a message to another agent.
    pub async fn send_message(&self, _recipient_id: Uuid, message: Message) -> Result<()> {
        if let Some(ref tx) = self.message_tx {
            tx.send(message)
                .await
                .map_err(|e| RmiError::Protocol(e.to_string()))?;
            self.context.metrics.write().unwrap().messages_sent += 1;
        }
        Ok(())
    }

    /// Receive a message (non-blocking).
    pub async fn try_receive_message(&self) -> Option<Message> {
        let mut rx_guard = self.message_rx.write().unwrap();
        if let Some(ref mut rx) = *rx_guard {
            match rx.try_recv() {
                Ok(msg) => {
                    self.context.metrics.write().unwrap().messages_received += 1;
                    Some(msg)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Serialize agent state.
    pub fn to_binary(&self) -> Vec<u8> {
        let state = AgentSnapshot {
            identity: self.identity.clone(),
            state: self.state(),
            current_goal: self.current_goal.read().unwrap().clone(),
            execution_trace: self.execution_trace.read().unwrap().clone(),
            metrics: self.context.metrics.read().unwrap().clone(),
        };

        let packed = rmp_serde::to_vec(&state).unwrap_or_default();
        lz4_flex::compress_prepend_size(&packed)
    }
}

#[derive(Serialize, Deserialize)]
struct AgentSnapshot {
    identity: AgentIdentity,
    state: AgentState,
    current_goal: Option<Goal>,
    execution_trace: Vec<TraceEntry>,
    metrics: AgentMetrics,
}

impl Serialize for AgentMetrics {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AgentMetrics", 7)?;
        s.serialize_field("total_flops_used", &self.total_flops_used)?;
        s.serialize_field("messages_sent", &self.messages_sent)?;
        s.serialize_field("messages_received", &self.messages_received)?;
        s.serialize_field("tasks_completed", &self.tasks_completed)?;
        s.serialize_field("errors", &self.errors)?;
        s.serialize_field("reasoning_time_ms", &self.reasoning_time_ms)?;
        s.serialize_field("execution_time_ms", &self.execution_time_ms)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for AgentMetrics {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            total_flops_used: f64,
            messages_sent: u64,
            messages_received: u64,
            tasks_completed: u64,
            errors: u64,
            reasoning_time_ms: u64,
            execution_time_ms: u64,
        }
        let h = Helper::deserialize(deserializer)?;
        Ok(AgentMetrics {
            total_flops_used: h.total_flops_used,
            messages_sent: h.messages_sent,
            messages_received: h.messages_received,
            tasks_completed: h.tasks_completed,
            errors: h.errors,
            reasoning_time_ms: h.reasoning_time_ms,
            execution_time_ms: h.execution_time_ms,
        })
    }
}

/// Builder for creating agents.
pub struct AgentBuilder {
    name: String,
    role: String,
    capabilities: HashSet<AgentCapability>,
    ontology: Option<Arc<Ontology>>,
    primitives: Option<Arc<PrimitiveRegistry>>,
    backend: Option<Arc<dyn crate::compute::Backend>>,
    memory_limit_mb: usize,
    compute_budget_flops: f64,
}

impl AgentBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        let uuid = Uuid::new_v4();
        let name = format!("agent-{}", &uuid.to_string()[..8]);
        Self {
            name,
            role: "general".to_string(),
            capabilities: HashSet::new(),
            ontology: None,
            primitives: None,
            backend: None,
            memory_limit_mb: 4096,
            compute_budget_flops: 1e15,
        }
    }

    /// Set agent name.
    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set agent role.
    pub fn role(mut self, role: &str) -> Self {
        self.role = role.to_string();
        self
    }

    /// Add a capability.
    pub fn capability(mut self, capability: AgentCapability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    /// Add multiple capabilities.
    pub fn capabilities(mut self, caps: impl IntoIterator<Item = AgentCapability>) -> Self {
        self.capabilities.extend(caps);
        self
    }

    /// Set knowledge ontology.
    pub fn knowledge(mut self, ontology: Arc<Ontology>) -> Self {
        self.ontology = Some(ontology);
        self
    }

    /// Set primitive registry.
    pub fn primitives(mut self, registry: Arc<PrimitiveRegistry>) -> Self {
        self.primitives = Some(registry);
        self
    }

    /// Set compute backend.
    pub fn backend(mut self, backend: Arc<dyn crate::compute::Backend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Set memory limit.
    pub fn memory_limit_mb(mut self, mb: usize) -> Self {
        self.memory_limit_mb = mb;
        self
    }

    /// Set compute budget.
    pub fn compute_budget(mut self, flops: f64) -> Self {
        self.compute_budget_flops = flops;
        self
    }

    /// Build the agent.
    pub fn build(self) -> Result<Agent> {
        let ontology = self
            .ontology
            .unwrap_or_else(|| Arc::new(Ontology::new("default")));
        let primitives = self
            .primitives
            .unwrap_or_else(|| Arc::new(PrimitiveRegistry::new()));
        let backend = self
            .backend
            .unwrap_or_else(|| Arc::new(crate::compute::cpu::CpuBackend::new()));

        let mut identity = AgentIdentity::new(&self.name, &self.role);
        identity.capabilities = self.capabilities;

        let (tx, rx) = mpsc::channel(1024);

        let context = Arc::new(AgentContext {
            ontology,
            primitives,
            backend,
            memory_limit_mb: self.memory_limit_mb,
            compute_budget_flops: self.compute_budget_flops,
            peer_agents: RwLock::new(HashMap::new()),
            current_task: RwLock::new(None),
            metrics: RwLock::new(AgentMetrics::default()),
        });

        Ok(Agent {
            identity,
            state: AtomicU8::new(AgentState::Idle as u8),
            context,
            current_goal: RwLock::new(None),
            goal_stack: RwLock::new(Vec::new()),
            execution_trace: RwLock::new(Vec::new()),
            message_tx: Some(tx),
            message_rx: RwLock::new(Some(rx)),
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = Agent::builder()
            .name("test-agent")
            .role("tester")
            .capability(AgentCapability::SymbolicReasoning)
            .build()
            .unwrap();

        assert_eq!(agent.identity.name, "test-agent");
        assert_eq!(agent.state(), AgentState::Idle);
        assert!(agent.has_capability(AgentCapability::SymbolicReasoning));
    }

    #[tokio::test]
    async fn test_goal_execution() {
        let agent = Agent::builder().build().unwrap();

        let goal = Goal::Reason {
            query: "What is a neural network?".to_string(),
            context_concepts: vec![],
        };

        let result = agent.execute(goal).await.unwrap();
        assert_eq!(result.status, "success");
    }

    #[tokio::test]
    async fn test_agent_builder_defaults() {
        let agent = Agent::builder().build().unwrap();
        assert_eq!(agent.identity.role, "general");
        assert_eq!(agent.state(), AgentState::Idle);
        assert!(agent.identity.capabilities.is_empty());
    }

    #[tokio::test]
    async fn test_agent_capabilities() {
        let mut agent = Agent::builder()
            .capability(AgentCapability::SymbolicReasoning)
            .capability(AgentCapability::Planning)
            .build()
            .unwrap();

        assert!(agent.has_capability(AgentCapability::SymbolicReasoning));
        assert!(agent.has_capability(AgentCapability::Planning));
        assert!(!agent.has_capability(AgentCapability::ModelEvaluation));

        agent.add_capability(AgentCapability::ModelEvaluation);
        assert!(agent.has_capability(AgentCapability::ModelEvaluation));
    }

    #[tokio::test]
    async fn test_agent_goal_stack() {
        let agent = Agent::builder().build().unwrap();

        let goal1 = Goal::Reason {
            query: "main goal".to_string(),
            context_concepts: vec![],
        };
        let goal2 = Goal::Reason {
            query: "sub-goal".to_string(),
            context_concepts: vec![],
        };

        agent.set_goal(goal1);
        agent.push_subgoal(goal2);

        let popped = agent.pop_goal().unwrap();
        assert_eq!(popped.goal_type(), "reason");

        let popped2 = agent.pop_goal().unwrap();
        assert_eq!(popped2.goal_type(), "reason");

        assert!(agent.pop_goal().is_none());
    }

    #[tokio::test]
    async fn test_goal_type_identifiers() {
        let g1 = Goal::MinimizeLoss {
            metric_name: "loss".into(),
            target_value: Some(0.01),
            constraints: HashMap::new(),
        };
        assert_eq!(g1.goal_type(), "minimize_loss");

        let g2 = Goal::MaximizeMetric {
            metric_name: "acc".into(),
            target_value: None,
            constraints: HashMap::new(),
        };
        assert_eq!(g2.goal_type(), "maximize_metric");

        let g3 = Goal::Train {
            model_id: "m1".into(),
            data_source: "ds".into(),
            epochs: 10,
            batch_size: 32,
        };
        assert_eq!(g3.goal_type(), "train");

        let g4 = Goal::Custom {
            goal_type: "my_custom".into(),
            spec: vec![],
        };
        assert_eq!(g4.goal_type(), "my_custom");
    }

    #[test]
    fn test_agent_identity_binary_roundtrip() {
        let mut id = AgentIdentity::new("test-agent", "architect");
        id.capabilities.insert(AgentCapability::ArchitectureSearch);
        id.capabilities.insert(AgentCapability::HyperparameterOptimization);

        let binary = id.to_binary();
        let restored = AgentIdentity::from_binary(&binary).unwrap();

        assert_eq!(restored.name, "test-agent");
        assert_eq!(restored.role, "architect");
        assert!(restored.capabilities.contains(&AgentCapability::ArchitectureSearch));
    }

    #[test]
    fn test_goal_binary_roundtrip() {
        let goal = Goal::MinimizeLoss {
            metric_name: "cross_entropy".into(),
            target_value: Some(0.05),
            constraints: HashMap::from([("latency_ms".into(), 100.0)]),
        };

        let binary = goal.to_binary();
        let restored = Goal::from_binary(&binary).unwrap();
        assert_eq!(restored.goal_type(), "minimize_loss");
    }

    #[test]
    fn test_agent_state_from_u8() {
        assert_eq!(AgentState::from_u8(0x01), AgentState::Initializing);
        assert_eq!(AgentState::from_u8(0x02), AgentState::Idle);
        assert_eq!(AgentState::from_u8(0x09), AgentState::Terminated);
        // Unknown values map to Error
        assert_eq!(AgentState::from_u8(0xFF), AgentState::Error);
    }

    #[tokio::test]
    async fn test_agent_execute_minimize_loss() {
        let agent = Agent::builder().build().unwrap();

        let goal = Goal::MinimizeLoss {
            metric_name: "mse".into(),
            target_value: Some(0.001),
            constraints: HashMap::new(),
        };

        let result = agent.execute(goal).await.unwrap();
        assert_eq!(result.status, "success");
        assert!(result.data.contains_key("metric"));
    }

    #[tokio::test]
    async fn test_agent_execute_architecture_search() {
        let agent = Agent::builder().build().unwrap();

        let goal = Goal::ArchitectureSearch {
            task_type: "classification".into(),
            input_schema: HashMap::new(),
            output_schema: HashMap::new(),
            resource_constraints: HashMap::new(),
        };

        let result = agent.execute(goal).await.unwrap();
        assert_eq!(result.status, "success");
        assert!(result.data.contains_key("task_type"));
    }

    #[tokio::test]
    async fn test_agent_serialization() {
        let agent = Agent::builder()
            .name("serialize-me")
            .role("tester")
            .build()
            .unwrap();

        let binary = agent.to_binary();
        assert!(!binary.is_empty());
    }

    #[tokio::test]
    async fn test_agent_metrics_tracking() {
        let agent = Agent::builder().build().unwrap();

        let goal = Goal::Reason {
            query: "test".into(),
            context_concepts: vec![],
        };

        let _ = agent.execute(goal).await.unwrap();

        let metrics = agent.context.metrics.read().unwrap();
        assert_eq!(metrics.tasks_completed, 1);
        assert_eq!(metrics.errors, 0);
        assert!(metrics.execution_time_ms < u64::MAX);
    }
}
