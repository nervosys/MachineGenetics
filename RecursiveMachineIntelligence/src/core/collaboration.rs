//! Multi-Agent Collaboration Runtime
//!
//! This module provides the integration layer that connects all framework
//! components into a cohesive multi-agent system designed for AI-to-AI
//! collaboration:
//!
//! - **`AgentRuntime`**: Central hub wiring Agent ↔ MessageBus ↔ Registry
//! - **`SharedWorkspace`**: Blackboard pattern for collaborative data sharing
//! - **`AgentPipeline`**: Compositional agent processing chains
//! - **`TaskDelegator`**: Capability-based task routing and delegation
//! - **`ModelRegistry`**: Versioned model store for sharing trained models
//!
//! ## Design Philosophy
//!
//! Traditional frameworks treat agents as isolated processes that happen to
//! communicate. RMI treats multi-agent collaboration as a first-class primitive:
//! agents share a workspace, discover each other's capabilities, delegate tasks
//! based on competence, and compose into pipelines — all without human
//! orchestration.
//!
//! ## Example
//!
//! ```
//! use rmi::core::collaboration::{AgentRuntime, RuntimeConfig, SharedWorkspace};
//! use rmi::core::agent::{Agent, AgentCapability};
//!
//! let runtime = AgentRuntime::new(RuntimeConfig::default());
//!
//! // Spawn agents — they auto-register on the message bus
//! let trainer = Agent::builder()
//!     .name("trainer")
//!     .role("training")
//!     .capability(AgentCapability::TrainingOrchestration)
//!     .build()
//!     .unwrap();
//! let trainer_id = runtime.spawn(trainer).unwrap();
//!
//! let reasoner = Agent::builder()
//!     .name("reasoner")
//!     .role("reasoning")
//!     .capability(AgentCapability::SymbolicReasoning)
//!     .build()
//!     .unwrap();
//! let reasoner_id = runtime.spawn(reasoner).unwrap();
//!
//! // Agents can now collaborate via shared workspace
//! let ws = runtime.workspace();
//! ws.put("config", b"learning_rate=0.001".to_vec(), trainer_id);
//! assert!(ws.get("config").is_some());
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::core::agent::Agent;
use crate::core::message_bus::{Envelope, MessageBus, Subscription, Topic};
use crate::distributed::discovery::{DiscoveryConfig, ServiceInfo, ServiceRegistry};
use crate::distributed::transport::{NodeAddr, TransportProtocol};
use crate::error::{Result, RmiError};

// ============================================================================
// Shared Workspace (Blackboard Pattern)
// ============================================================================

/// A shared workspace for multi-agent collaborative data sharing.
///
/// Implements the **Blackboard pattern**: agents post intermediate results
/// (tensors, models, configs, metrics) to named keys. Other agents can read
/// these values or watch for changes. This enables implicit coordination
/// without explicit message passing.
///
/// Thread-safe via `RwLock`. All operations are synchronous for simplicity;
/// async watchers are provided via `tokio::sync::mpsc` channels.
pub struct SharedWorkspace {
    /// Key-value store
    entries: RwLock<HashMap<String, WorkspaceEntry>>,
    /// Global version counter (monotonically increasing)
    version: AtomicU64,
    /// Watchers: pattern → list of notification channels
    watchers: RwLock<HashMap<String, Vec<mpsc::Sender<WorkspaceEvent>>>>,
}

/// An entry in the shared workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    /// Key name
    pub key: String,
    /// Serialized value (arbitrary bytes)
    pub value: Vec<u8>,
    /// Agent that wrote this entry
    pub author: Uuid,
    /// Entry version (incremented on each write)
    pub version: u64,
    /// Creation timestamp
    pub created_at: f64,
    /// Last modification timestamp
    pub updated_at: f64,
    /// Arbitrary metadata
    pub metadata: HashMap<String, String>,
}

/// Event emitted when a workspace entry changes.
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    /// A new key was created
    Created {
        /// Key name
        key: String,
        /// Author agent ID
        author: Uuid,
        /// Version
        version: u64,
    },
    /// An existing key was updated
    Updated {
        /// Key name
        key: String,
        /// Author agent ID
        author: Uuid,
        /// New version
        version: u64,
    },
    /// A key was deleted
    Deleted {
        /// Key name
        key: String,
    },
}

impl SharedWorkspace {
    /// Create a new empty shared workspace.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            version: AtomicU64::new(0),
            watchers: RwLock::new(HashMap::new()),
        }
    }

    /// Write a value to the workspace. Returns the new version number.
    pub fn put(&self, key: &str, value: Vec<u8>, author: Uuid) -> u64 {
        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let mut entries = self.entries.write().unwrap();
        let is_update = entries.contains_key(key);

        let entry = WorkspaceEntry {
            key: key.to_string(),
            value,
            author,
            version,
            created_at: if is_update {
                entries.get(key).map_or(now, |e| e.created_at)
            } else {
                now
            },
            updated_at: now,
            metadata: if is_update {
                entries
                    .get(key)
                    .map_or_else(HashMap::new, |e| e.metadata.clone())
            } else {
                HashMap::new()
            },
        };

        entries.insert(key.to_string(), entry);
        drop(entries);

        // Notify watchers
        let event = if is_update {
            WorkspaceEvent::Updated {
                key: key.to_string(),
                author,
                version,
            }
        } else {
            WorkspaceEvent::Created {
                key: key.to_string(),
                author,
                version,
            }
        };
        self.notify_watchers(key, event);

        version
    }

    /// Write a value with metadata.
    pub fn put_with_metadata(
        &self,
        key: &str,
        value: Vec<u8>,
        author: Uuid,
        metadata: HashMap<String, String>,
    ) -> u64 {
        let version = self.put(key, value, author);

        // Update metadata
        let mut entries = self.entries.write().unwrap();
        if let Some(entry) = entries.get_mut(key) {
            entry.metadata = metadata;
        }

        version
    }

    /// Read a value from the workspace.
    pub fn get(&self, key: &str) -> Option<WorkspaceEntry> {
        self.entries.read().unwrap().get(key).cloned()
    }

    /// Check if a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.entries.read().unwrap().contains_key(key)
    }

    /// Delete a key. Returns true if the key existed.
    pub fn delete(&self, key: &str) -> bool {
        let existed = self.entries.write().unwrap().remove(key).is_some();
        if existed {
            self.notify_watchers(
                key,
                WorkspaceEvent::Deleted {
                    key: key.to_string(),
                },
            );
        }
        existed
    }

    /// Watch for changes to keys matching a prefix.
    ///
    /// Returns a receiver channel that will receive `WorkspaceEvent` notifications
    /// whenever a key with the given prefix is created, updated, or deleted.
    pub fn watch(&self, prefix: &str) -> mpsc::Receiver<WorkspaceEvent> {
        let (tx, rx) = mpsc::channel(256);
        let mut watchers = self.watchers.write().unwrap();
        watchers.entry(prefix.to_string()).or_default().push(tx);
        rx
    }

    /// List all keys in the workspace.
    pub fn list_keys(&self) -> Vec<String> {
        self.entries.read().unwrap().keys().cloned().collect()
    }

    /// List keys matching a prefix.
    pub fn list_keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.entries
            .read()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Get the current global version.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    /// Store a tensor in the workspace (convenience for f32 arrays).
    pub fn put_tensor(&self, key: &str, tensor: &ndarray::ArrayD<f32>, author: Uuid) -> u64 {
        let shape = tensor.shape().to_vec();
        let data: Vec<u8> = tensor.iter().flat_map(|f| f.to_le_bytes()).collect();

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "tensor_f32".to_string());
        metadata.insert("shape".to_string(), format!("{:?}", shape));

        self.put_with_metadata(key, data, author, metadata)
    }

    /// Retrieve a tensor from the workspace (convenience for f32 arrays).
    pub fn get_tensor(&self, key: &str) -> Option<ndarray::ArrayD<f32>> {
        let entry = self.get(key)?;
        let shape_str = entry.metadata.get("shape")?;

        // Parse shape from debug format "[a, b, c]"
        let shape: Vec<usize> = shape_str
            .trim_matches(|c| c == '[' || c == ']')
            .split(", ")
            .filter_map(|s| s.parse().ok())
            .collect();

        if shape.is_empty() {
            return None;
        }

        let values: Vec<f32> = entry
            .value
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&shape), values).ok()
    }

    /// Notify watchers whose prefix matches the key.
    fn notify_watchers(&self, key: &str, event: WorkspaceEvent) {
        let mut watchers = self.watchers.write().unwrap();
        // Clean up dead channels while iterating
        for (prefix, senders) in watchers.iter_mut() {
            if key.starts_with(prefix.as_str()) || prefix == "*" {
                senders.retain(|tx| tx.try_send(event.clone()).is_ok());
            }
        }
    }
}

impl Default for SharedWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Model Registry
// ============================================================================

/// A versioned model registry for sharing trained models between agents.
///
/// Agents can register models with metrics, retrieve the latest or a specific
/// version, and search by performance criteria. This enables:
/// - **Transfer learning**: One agent trains, others fine-tune
/// - **Ensemble**: Combine models from multiple agents
/// - **Distillation**: Teacher-student across agents
pub struct ModelRegistry {
    /// Models keyed by name, each with a version history
    models: RwLock<HashMap<String, Vec<ModelEntry>>>,
    /// Global version counter
    next_version: AtomicU64,
}

/// A single model entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Model name
    pub name: String,
    /// Version number
    pub version: u64,
    /// Agent that registered this model
    pub author: Uuid,
    /// Serialized model data
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    /// Performance metrics (e.g., "accuracy" → 0.95)
    pub metrics: HashMap<String, f64>,
    /// Creation timestamp
    pub created_at: f64,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Arbitrary metadata
    pub metadata: HashMap<String, String>,
}

impl ModelRegistry {
    /// Create a new empty model registry.
    pub fn new() -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
            next_version: AtomicU64::new(1),
        }
    }

    /// Register a new model version. Returns the version number.
    pub fn register(
        &self,
        name: &str,
        data: Vec<u8>,
        author: Uuid,
        metrics: HashMap<String, f64>,
    ) -> u64 {
        let version = self.next_version.fetch_add(1, Ordering::SeqCst);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let entry = ModelEntry {
            name: name.to_string(),
            version,
            author,
            data,
            metrics,
            created_at: now,
            tags: Vec::new(),
            metadata: HashMap::new(),
        };

        let mut models = self.models.write().unwrap();
        models.entry(name.to_string()).or_default().push(entry);

        version
    }

    /// Register a model with tags.
    pub fn register_with_tags(
        &self,
        name: &str,
        data: Vec<u8>,
        author: Uuid,
        metrics: HashMap<String, f64>,
        tags: Vec<String>,
    ) -> u64 {
        let version = self.register(name, data, author, metrics);

        let mut models = self.models.write().unwrap();
        if let Some(versions) = models.get_mut(name) {
            if let Some(entry) = versions.last_mut() {
                entry.tags = tags;
            }
        }

        version
    }

    /// Get the latest version of a model.
    pub fn get_latest(&self, name: &str) -> Option<ModelEntry> {
        self.models
            .read()
            .unwrap()
            .get(name)
            .and_then(|versions| versions.last().cloned())
    }

    /// Get a specific version of a model.
    pub fn get_version(&self, name: &str, version: u64) -> Option<ModelEntry> {
        self.models
            .read()
            .unwrap()
            .get(name)
            .and_then(|versions| versions.iter().find(|e| e.version == version).cloned())
    }

    /// List all model names with their latest version number.
    pub fn list_models(&self) -> Vec<(String, u64)> {
        self.models
            .read()
            .unwrap()
            .iter()
            .filter_map(|(name, versions)| versions.last().map(|v| (name.clone(), v.version)))
            .collect()
    }

    /// Find models that meet a minimum metric threshold.
    pub fn find_by_metric(&self, metric: &str, min_value: f64) -> Vec<ModelEntry> {
        self.models
            .read()
            .unwrap()
            .values()
            .flatten()
            .filter(|entry| entry.metrics.get(metric).is_some_and(|&v| v >= min_value))
            .cloned()
            .collect()
    }

    /// Find models by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<ModelEntry> {
        self.models
            .read()
            .unwrap()
            .values()
            .flatten()
            .filter(|entry| entry.tags.iter().any(|t| t == tag))
            .cloned()
            .collect()
    }

    /// Get the total number of model versions across all names.
    pub fn total_versions(&self) -> usize {
        self.models.read().unwrap().values().map(|v| v.len()).sum()
    }

    /// Get the number of unique model names.
    pub fn model_count(&self) -> usize {
        self.models.read().unwrap().len()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Task Delegator
// ============================================================================

/// Capability-based task delegation.
///
/// The `TaskDelegator` discovers agents with the required capabilities via
/// the `ServiceRegistry`, selects the best candidate (lowest load), and
/// routes the task via the `MessageBus`. This enables fully autonomous
/// task distribution without human intervention.
pub struct TaskDelegator {
    /// Service registry for capability lookup
    registry: Arc<ServiceRegistry>,
    /// Message bus for sending tasks
    bus: Arc<MessageBus>,
    /// Pending delegated tasks
    pending: RwLock<HashMap<Uuid, DelegatedTask>>,
}

/// A task that has been delegated to another agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedTask {
    /// Unique task ID
    pub task_id: Uuid,
    /// Agent the task was assigned to
    pub assigned_to: Uuid,
    /// Sender who delegated the task
    pub delegated_by: Uuid,
    /// When the task was delegated
    pub delegated_at: f64,
    /// Timeout duration in seconds
    pub timeout_seconds: f64,
    /// Task payload
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    /// Capability that was requested
    pub capability: String,
}

impl TaskDelegator {
    /// Create a new task delegator.
    pub fn new(registry: Arc<ServiceRegistry>, bus: Arc<MessageBus>) -> Self {
        Self {
            registry,
            bus,
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Delegate a task to the best available agent with the given capability.
    ///
    /// Returns the task ID that can be used to track the delegation.
    pub async fn delegate(&self, capability: &str, payload: Vec<u8>, sender: Uuid) -> Result<Uuid> {
        self.delegate_with_timeout(capability, payload, sender, Duration::from_secs(300))
            .await
    }

    /// Delegate a task with a custom timeout.
    pub async fn delegate_with_timeout(
        &self,
        capability: &str,
        payload: Vec<u8>,
        sender: Uuid,
        timeout: Duration,
    ) -> Result<Uuid> {
        // Find the best agent
        let agent = self.find_best_agent(capability).ok_or_else(|| {
            RmiError::Agent(format!("No agent found with capability '{}'", capability))
        })?;

        let task_id = Uuid::new_v4();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        // Record the delegation
        let delegated_task = DelegatedTask {
            task_id,
            assigned_to: agent.id,
            delegated_by: sender,
            delegated_at: now,
            timeout_seconds: timeout.as_secs_f64(),
            payload: payload.clone(),
            capability: capability.to_string(),
        };
        self.pending
            .write()
            .unwrap()
            .insert(task_id, delegated_task);

        // Send the task via the message bus
        let topic = Topic::new("task.assign");
        let mut envelope = Envelope::new(topic, sender, payload);
        envelope.target = Some(agent.id);
        envelope
            .headers
            .insert("task_id".to_string(), task_id.to_string());
        envelope
            .headers
            .insert("capability".to_string(), capability.to_string());

        self.bus.publish(envelope).await?;

        Ok(task_id)
    }

    /// Find the best agent for a capability (lowest load).
    pub fn find_best_agent(&self, capability: &str) -> Option<ServiceInfo> {
        let mut candidates = self.registry.find_by_capability(capability);
        // Sort by load (ascending) — prefer least loaded agent
        candidates.sort_by(|a, b| {
            a.load
                .partial_cmp(&b.load)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.into_iter().next()
    }

    /// Get a pending task by ID.
    pub fn get_pending(&self, task_id: Uuid) -> Option<DelegatedTask> {
        self.pending.read().unwrap().get(&task_id).cloned()
    }

    /// Mark a task as completed (removes from pending).
    pub fn complete(&self, task_id: Uuid) -> Option<DelegatedTask> {
        self.pending.write().unwrap().remove(&task_id)
    }

    /// Get the number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.pending.read().unwrap().len()
    }

    /// Get all timed-out tasks.
    pub fn timed_out_tasks(&self) -> Vec<DelegatedTask> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        self.pending
            .read()
            .unwrap()
            .values()
            .filter(|t| now > t.delegated_at + t.timeout_seconds)
            .cloned()
            .collect()
    }
}

// ============================================================================
// Agent Pipeline
// ============================================================================

/// A composable pipeline of agents where the output of one stage feeds
/// into the next.
///
/// Pipelines use the `SharedWorkspace` as the communication medium:
/// each stage reads from an input key and writes to an output key.
/// This enables flexible agent composition without tight coupling.
///
/// ```text
/// [Agent A] --writes--> workspace["features"]
///                            |
/// [Agent B] --reads--> workspace["features"]
///           --writes--> workspace["predictions"]
///                            |
/// [Agent C] --reads--> workspace["predictions"]
///           --writes--> workspace["evaluation"]
/// ```
pub struct AgentPipeline {
    /// Pipeline stages
    stages: Vec<PipelineStage>,
    /// Shared workspace used for inter-stage communication
    workspace: Arc<SharedWorkspace>,
    /// Pipeline name
    name: String,
}

/// A single stage in an agent pipeline.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Stage name
    pub name: String,
    /// Agent responsible for this stage
    pub agent_id: Uuid,
    /// Workspace key to read input from
    pub input_key: String,
    /// Workspace key to write output to
    pub output_key: String,
}

impl AgentPipeline {
    /// Create a new pipeline.
    pub fn new(name: &str, workspace: Arc<SharedWorkspace>) -> Self {
        Self {
            stages: Vec::new(),
            workspace,
            name: name.to_string(),
        }
    }

    /// Add a stage to the pipeline.
    pub fn then(mut self, name: &str, agent_id: Uuid, input_key: &str, output_key: &str) -> Self {
        self.stages.push(PipelineStage {
            name: name.to_string(),
            agent_id,
            input_key: input_key.to_string(),
            output_key: output_key.to_string(),
        });
        self
    }

    /// Get the number of stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Get pipeline name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the stages.
    pub fn stages(&self) -> &[PipelineStage] {
        &self.stages
    }

    /// Get the shared workspace.
    pub fn workspace(&self) -> &Arc<SharedWorkspace> {
        &self.workspace
    }

    /// Validate that all stage connections are consistent
    /// (each stage's input_key matches a previous stage's output_key,
    /// except the first stage).
    pub fn validate(&self) -> Result<()> {
        let mut available_keys: Vec<&str> = Vec::new();

        for (i, stage) in self.stages.iter().enumerate() {
            if i > 0 && !available_keys.contains(&stage.input_key.as_str()) {
                return Err(RmiError::InvalidConfig(format!(
                    "Pipeline stage '{}' expects input key '{}' which is not produced by any earlier stage",
                    stage.name, stage.input_key
                )));
            }
            available_keys.push(&stage.output_key);
        }

        Ok(())
    }
}

// ============================================================================
// Agent Runtime
// ============================================================================

/// Configuration for the agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum number of agents allowed
    pub max_agents: usize,
    /// Message buffer size per agent
    pub message_buffer_size: usize,
    /// Heartbeat interval for health checks
    pub heartbeat_interval: Duration,
    /// Default task timeout
    pub task_timeout: Duration,
    /// Whether to enable the shared workspace
    pub enable_workspace: bool,
    /// Whether to auto-subscribe agents to capability-based topics
    pub auto_subscribe: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_agents: 64,
            message_buffer_size: 1024,
            heartbeat_interval: Duration::from_secs(30),
            task_timeout: Duration::from_secs(300),
            enable_workspace: true,
            auto_subscribe: true,
        }
    }
}

/// Entry for a managed agent in the runtime.
struct AgentEntry {
    /// The agent instance
    agent: Arc<Agent>,
    /// Subscription receiver for bus messages
    _bus_rx: Option<mpsc::Receiver<Envelope>>,
    /// Capabilities as strings for registry
    capabilities: Vec<String>,
}

/// The central multi-agent collaboration runtime.
///
/// `AgentRuntime` is the integration layer that connects all framework
/// components into a cohesive multi-agent system. It:
///
/// - Manages agent lifecycles (spawn, remove)
/// - Bridges agents to the `MessageBus` via topic subscriptions
/// - Registers agents in the `ServiceRegistry` for capability discovery
/// - Provides a `SharedWorkspace` for collaborative data sharing
/// - Offers a `ModelRegistry` for sharing trained models
/// - Routes tasks to capable agents via the `TaskDelegator`
///
/// This is the primary entry point for building multi-agent AI systems.
pub struct AgentRuntime {
    /// Managed agents
    agents: RwLock<HashMap<Uuid, AgentEntry>>,
    /// The message bus
    bus: Arc<MessageBus>,
    /// Service registry for discovery
    registry: Arc<ServiceRegistry>,
    /// Shared workspace
    workspace: Arc<SharedWorkspace>,
    /// Model registry
    model_registry: Arc<ModelRegistry>,
    /// Task delegator
    delegator: Arc<TaskDelegator>,
    /// Runtime configuration
    config: RuntimeConfig,
}

impl AgentRuntime {
    /// Create a new agent runtime with the given configuration.
    pub fn new(config: RuntimeConfig) -> Self {
        let bus = Arc::new(MessageBus::new());
        let registry = Arc::new(ServiceRegistry::new(DiscoveryConfig::default()));
        let workspace = Arc::new(SharedWorkspace::new());
        let model_registry = Arc::new(ModelRegistry::new());
        let delegator = Arc::new(TaskDelegator::new(registry.clone(), bus.clone()));

        Self {
            agents: RwLock::new(HashMap::new()),
            bus,
            registry,
            workspace,
            model_registry,
            delegator,
            config,
        }
    }

    /// Create a runtime with default configuration.
    pub fn default_runtime() -> Self {
        Self::new(RuntimeConfig::default())
    }

    /// Spawn an agent into the runtime.
    ///
    /// This will:
    /// 1. Register the agent in the service registry with its capabilities
    /// 2. Subscribe the agent to relevant message bus topics
    /// 3. Store the agent for lifecycle management
    ///
    /// Returns the agent's UUID.
    pub fn spawn(&self, agent: Agent) -> Result<Uuid> {
        let agents = self.agents.read().unwrap();
        if agents.len() >= self.config.max_agents {
            return Err(RmiError::ResourceExhausted(format!(
                "Maximum agents ({}) reached",
                self.config.max_agents
            )));
        }
        drop(agents);

        let agent_id = agent.identity.uuid;
        let agent_name = agent.identity.name.clone();

        // Build capability strings from AgentCapability enum
        let capabilities: Vec<String> = agent
            .identity
            .capabilities
            .iter()
            .map(|c| format!("{:?}", c))
            .collect();

        // Register in service registry
        let node_addr =
            NodeAddr::with_id(agent_id, "local://in-process", TransportProtocol::InProcess);
        let service_info = ServiceInfo::new(&agent_name, node_addr, capabilities.clone());
        self.registry.register(service_info)?;

        // Subscribe to per-agent topic for direct messaging
        let bus_rx = if self.config.auto_subscribe {
            let agent_topic = format!("agent.{}", agent_id);
            let sub = Subscription::new(&agent_topic, agent_id);
            let rx = self.bus.subscribe(sub);

            // Also subscribe to broadcast topics based on capabilities
            for cap in &capabilities {
                let cap_topic = format!("capability.{}", cap);
                let cap_sub = Subscription::new(&cap_topic, agent_id);
                // Subscribe but we don't track these receivers individually;
                // they route to the same agent channel in the bus
                let _cap_rx = self.bus.subscribe(cap_sub);
            }

            Some(rx)
        } else {
            None
        };

        // Store the agent
        let entry = AgentEntry {
            agent: Arc::new(agent),
            _bus_rx: bus_rx,
            capabilities,
        };

        self.agents.write().unwrap().insert(agent_id, entry);

        Ok(agent_id)
    }

    /// Remove an agent from the runtime.
    pub fn remove(&self, agent_id: Uuid) -> Result<()> {
        let removed = self.agents.write().unwrap().remove(&agent_id);
        if removed.is_none() {
            return Err(RmiError::Agent(format!(
                "Agent {} not found in runtime",
                agent_id
            )));
        }

        // Deregister from service registry
        self.registry.deregister(agent_id);

        Ok(())
    }

    /// Get a reference to an agent by ID.
    pub fn get_agent(&self, agent_id: Uuid) -> Option<Arc<Agent>> {
        self.agents
            .read()
            .unwrap()
            .get(&agent_id)
            .map(|e| e.agent.clone())
    }

    /// Send a message from one agent to another via the message bus.
    pub async fn send(&self, from: Uuid, to: Uuid, topic: &str, payload: Vec<u8>) -> Result<()> {
        let envelope = Envelope::new(Topic::new(topic), from, payload).with_target(to);
        self.bus.publish(envelope).await
    }

    /// Broadcast a message from an agent to all subscribers of a topic.
    pub async fn broadcast(&self, from: Uuid, topic: &str, payload: Vec<u8>) -> Result<u32> {
        self.bus.broadcast(topic, from, payload).await
    }

    /// Delegate a task to the best agent with the given capability.
    pub async fn delegate(&self, capability: &str, payload: Vec<u8>, sender: Uuid) -> Result<Uuid> {
        self.delegator.delegate(capability, payload, sender).await
    }

    /// Get the shared workspace.
    pub fn workspace(&self) -> Arc<SharedWorkspace> {
        self.workspace.clone()
    }

    /// Get the model registry.
    pub fn model_registry(&self) -> Arc<ModelRegistry> {
        self.model_registry.clone()
    }

    /// Get the message bus.
    pub fn bus(&self) -> Arc<MessageBus> {
        self.bus.clone()
    }

    /// Get the service registry.
    pub fn registry(&self) -> Arc<ServiceRegistry> {
        self.registry.clone()
    }

    /// Get the task delegator.
    pub fn delegator(&self) -> Arc<TaskDelegator> {
        self.delegator.clone()
    }

    /// Get the number of agents in the runtime.
    pub fn agent_count(&self) -> usize {
        self.agents.read().unwrap().len()
    }

    /// Get all agent IDs.
    pub fn agent_ids(&self) -> Vec<Uuid> {
        self.agents.read().unwrap().keys().copied().collect()
    }

    /// Find agents by capability.
    pub fn find_agents_by_capability(&self, capability: &str) -> Vec<Arc<Agent>> {
        self.agents
            .read()
            .unwrap()
            .values()
            .filter(|e| e.capabilities.iter().any(|c| c == capability))
            .map(|e| e.agent.clone())
            .collect()
    }

    /// Create a pipeline using this runtime's workspace.
    pub fn create_pipeline(&self, name: &str) -> AgentPipeline {
        AgentPipeline::new(name, self.workspace.clone())
    }

    /// Get runtime statistics.
    pub fn stats(&self) -> RuntimeStats {
        let bus_stats = self.bus.stats();
        RuntimeStats {
            agent_count: self.agent_count(),
            registered_services: self.registry.len(),
            workspace_entries: self.workspace.len(),
            model_count: self.model_registry.model_count(),
            pending_delegations: self.delegator.pending_count(),
            bus_messages_published: bus_stats.messages_published,
            bus_messages_delivered: bus_stats.messages_delivered,
        }
    }
}

/// Statistics for the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Number of active agents
    pub agent_count: usize,
    /// Number of registered services
    pub registered_services: usize,
    /// Number of workspace entries
    pub workspace_entries: usize,
    /// Number of registered models
    pub model_count: usize,
    /// Number of pending task delegations
    pub pending_delegations: usize,
    /// Total messages published on the bus
    pub bus_messages_published: u64,
    /// Total messages delivered on the bus
    pub bus_messages_delivered: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::agent::AgentCapability;

    // ===== SharedWorkspace Tests =====

    #[test]
    fn workspace_put_and_get() {
        let ws = SharedWorkspace::new();
        let author = Uuid::new_v4();

        let v = ws.put("key1", b"value1".to_vec(), author);
        assert_eq!(v, 1);

        let entry = ws.get("key1").unwrap();
        assert_eq!(entry.key, "key1");
        assert_eq!(entry.value, b"value1");
        assert_eq!(entry.author, author);
        assert_eq!(entry.version, 1);
    }

    #[test]
    fn workspace_update_increments_version() {
        let ws = SharedWorkspace::new();
        let author = Uuid::new_v4();

        let v1 = ws.put("key", b"v1".to_vec(), author);
        let v2 = ws.put("key", b"v2".to_vec(), author);
        assert!(v2 > v1);

        let entry = ws.get("key").unwrap();
        assert_eq!(entry.value, b"v2");
    }

    #[test]
    fn workspace_delete() {
        let ws = SharedWorkspace::new();
        ws.put("key", b"val".to_vec(), Uuid::new_v4());
        assert!(ws.delete("key"));
        assert!(!ws.delete("key"));
        assert!(ws.get("key").is_none());
    }

    #[test]
    fn workspace_list_keys() {
        let ws = SharedWorkspace::new();
        let id = Uuid::new_v4();
        ws.put("a", vec![], id);
        ws.put("b", vec![], id);
        ws.put("c", vec![], id);

        let mut keys = ws.list_keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn workspace_list_keys_with_prefix() {
        let ws = SharedWorkspace::new();
        let id = Uuid::new_v4();
        ws.put("model.weights", vec![], id);
        ws.put("model.config", vec![], id);
        ws.put("data.input", vec![], id);

        let model_keys = ws.list_keys_with_prefix("model.");
        assert_eq!(model_keys.len(), 2);
    }

    #[test]
    fn workspace_contains() {
        let ws = SharedWorkspace::new();
        assert!(!ws.contains("key"));
        ws.put("key", vec![], Uuid::new_v4());
        assert!(ws.contains("key"));
    }

    #[test]
    fn workspace_len_and_empty() {
        let ws = SharedWorkspace::new();
        assert!(ws.is_empty());
        assert_eq!(ws.len(), 0);

        ws.put("k", vec![], Uuid::new_v4());
        assert!(!ws.is_empty());
        assert_eq!(ws.len(), 1);
    }

    #[test]
    fn workspace_tensor_roundtrip() {
        let ws = SharedWorkspace::new();
        let author = Uuid::new_v4();

        let tensor = ndarray::ArrayD::from_shape_vec(
            ndarray::IxDyn(&[2, 3]),
            vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0],
        )
        .unwrap();

        ws.put_tensor("weights", &tensor, author);

        let retrieved = ws.get_tensor("weights").unwrap();
        assert_eq!(retrieved.shape(), &[2, 3]);
        assert_eq!(
            retrieved.as_slice().unwrap(),
            &[1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn workspace_metadata() {
        let ws = SharedWorkspace::new();
        let mut meta = HashMap::new();
        meta.insert("format".to_string(), "onnx".to_string());

        ws.put_with_metadata("model", b"data".to_vec(), Uuid::new_v4(), meta);

        let entry = ws.get("model").unwrap();
        assert_eq!(entry.metadata.get("format").unwrap(), "onnx");
    }

    #[tokio::test]
    async fn workspace_watch_notifications() {
        let ws = SharedWorkspace::new();
        let author = Uuid::new_v4();

        let mut rx = ws.watch("model");

        ws.put("model.weights", b"data".to_vec(), author);

        // Should receive a Created event
        let event = rx.try_recv();
        assert!(event.is_ok());
        match event.unwrap() {
            WorkspaceEvent::Created { key, .. } => assert_eq!(key, "model.weights"),
            _ => panic!("Expected Created event"),
        }
    }

    // ===== ModelRegistry Tests =====

    #[test]
    fn model_registry_register_and_get() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();
        let mut metrics = HashMap::new();
        metrics.insert("accuracy".to_string(), 0.95);

        let v = reg.register("resnet", b"model_data".to_vec(), author, metrics);
        assert!(v >= 1);

        let entry = reg.get_latest("resnet").unwrap();
        assert_eq!(entry.name, "resnet");
        assert_eq!(entry.data, b"model_data");
        assert_eq!(entry.metrics["accuracy"], 0.95);
    }

    #[test]
    fn model_registry_multiple_versions() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();

        let v1 = reg.register("net", b"v1".to_vec(), author, HashMap::new());
        let v2 = reg.register("net", b"v2".to_vec(), author, HashMap::new());
        assert!(v2 > v1);

        let latest = reg.get_latest("net").unwrap();
        assert_eq!(latest.data, b"v2");

        let first = reg.get_version("net", v1).unwrap();
        assert_eq!(first.data, b"v1");
    }

    #[test]
    fn model_registry_find_by_metric() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();

        let mut good = HashMap::new();
        good.insert("accuracy".to_string(), 0.98);
        reg.register("good_model", b"good".to_vec(), author, good);

        let mut bad = HashMap::new();
        bad.insert("accuracy".to_string(), 0.60);
        reg.register("bad_model", b"bad".to_vec(), author, bad);

        let results = reg.find_by_metric("accuracy", 0.90);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "good_model");
    }

    #[test]
    fn model_registry_find_by_tag() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();

        reg.register_with_tags(
            "resnet",
            vec![],
            author,
            HashMap::new(),
            vec!["production".to_string(), "vision".to_string()],
        );
        reg.register_with_tags(
            "bert",
            vec![],
            author,
            HashMap::new(),
            vec!["nlp".to_string()],
        );

        let vision = reg.find_by_tag("vision");
        assert_eq!(vision.len(), 1);
        assert_eq!(vision[0].name, "resnet");
    }

    #[test]
    fn model_registry_list_models() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();

        reg.register("a", vec![], author, HashMap::new());
        reg.register("b", vec![], author, HashMap::new());

        let models = reg.list_models();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn model_registry_counts() {
        let reg = ModelRegistry::new();
        let author = Uuid::new_v4();

        assert_eq!(reg.model_count(), 0);
        assert_eq!(reg.total_versions(), 0);

        reg.register("x", vec![], author, HashMap::new());
        reg.register("x", vec![], author, HashMap::new());
        reg.register("y", vec![], author, HashMap::new());

        assert_eq!(reg.model_count(), 2);
        assert_eq!(reg.total_versions(), 3);
    }

    // ===== TaskDelegator Tests =====

    #[test]
    fn delegator_find_best_agent() {
        let registry = Arc::new(ServiceRegistry::new(DiscoveryConfig::default()));
        let bus = Arc::new(MessageBus::new());
        let delegator = TaskDelegator::new(registry.clone(), bus);

        // Register two agents with different loads
        let addr1 = NodeAddr::new("127.0.0.1:8001", TransportProtocol::Tcp);
        let svc1 = ServiceInfo::new("agent1", addr1, vec!["Training".to_string()]).with_load(0.8);
        registry.register(svc1).unwrap();

        let addr2 = NodeAddr::new("127.0.0.1:8002", TransportProtocol::Tcp);
        let svc2 = ServiceInfo::new("agent2", addr2, vec!["Training".to_string()]).with_load(0.2);
        registry.register(svc2).unwrap();

        // Should select lower-load agent
        let best = delegator.find_best_agent("Training").unwrap();
        assert!((best.load - 0.2).abs() < 1e-6);
    }

    #[test]
    fn delegator_no_capable_agent() {
        let registry = Arc::new(ServiceRegistry::new(DiscoveryConfig::default()));
        let bus = Arc::new(MessageBus::new());
        let delegator = TaskDelegator::new(registry, bus);

        let result = delegator.find_best_agent("NonexistentCapability");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delegator_delegate_and_complete() {
        let registry = Arc::new(ServiceRegistry::new(DiscoveryConfig::default()));
        let bus = Arc::new(MessageBus::new());
        let delegator = TaskDelegator::new(registry.clone(), bus);

        // Register a capable agent
        let addr = NodeAddr::new("127.0.0.1:9000", TransportProtocol::Tcp);
        let svc = ServiceInfo::new("trainer", addr, vec!["Training".to_string()]);
        registry.register(svc).unwrap();

        let sender = Uuid::new_v4();
        let task_id = delegator
            .delegate("Training", b"train_config".to_vec(), sender)
            .await
            .unwrap();

        assert_eq!(delegator.pending_count(), 1);
        assert!(delegator.get_pending(task_id).is_some());

        delegator.complete(task_id);
        assert_eq!(delegator.pending_count(), 0);
    }

    // ===== AgentPipeline Tests =====

    #[test]
    fn pipeline_creation_and_stages() {
        let ws = Arc::new(SharedWorkspace::new());
        let pipeline = AgentPipeline::new("test_pipeline", ws)
            .then("extract", Uuid::new_v4(), "raw_data", "features")
            .then("train", Uuid::new_v4(), "features", "model")
            .then("evaluate", Uuid::new_v4(), "model", "results");

        assert_eq!(pipeline.name(), "test_pipeline");
        assert_eq!(pipeline.stage_count(), 3);
        assert_eq!(pipeline.stages()[0].name, "extract");
        assert_eq!(pipeline.stages()[1].input_key, "features");
        assert_eq!(pipeline.stages()[2].output_key, "results");
    }

    #[test]
    fn pipeline_validation_valid() {
        let ws = Arc::new(SharedWorkspace::new());
        let pipeline = AgentPipeline::new("valid", ws)
            .then("a", Uuid::new_v4(), "input", "mid")
            .then("b", Uuid::new_v4(), "mid", "output");

        assert!(pipeline.validate().is_ok());
    }

    #[test]
    fn pipeline_validation_invalid() {
        let ws = Arc::new(SharedWorkspace::new());
        let pipeline = AgentPipeline::new("invalid", ws)
            .then("a", Uuid::new_v4(), "input", "mid")
            .then("b", Uuid::new_v4(), "missing_key", "output");

        assert!(pipeline.validate().is_err());
    }

    // ===== AgentRuntime Tests =====

    #[test]
    fn runtime_spawn_agent() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let agent = Agent::builder()
            .name("test-agent")
            .role("tester")
            .capability(AgentCapability::TrainingOrchestration)
            .build()
            .unwrap();

        let id = runtime.spawn(agent).unwrap();
        assert_eq!(runtime.agent_count(), 1);
        assert!(runtime.get_agent(id).is_some());
    }

    #[test]
    fn runtime_spawn_multiple_agents() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        for i in 0..5 {
            let agent = Agent::builder()
                .name(&format!("agent-{}", i))
                .build()
                .unwrap();
            runtime.spawn(agent).unwrap();
        }

        assert_eq!(runtime.agent_count(), 5);
    }

    #[test]
    fn runtime_max_agents_exceeded() {
        let config = RuntimeConfig {
            max_agents: 2,
            ..Default::default()
        };
        let runtime = AgentRuntime::new(config);

        runtime
            .spawn(Agent::builder().name("a").build().unwrap())
            .unwrap();
        runtime
            .spawn(Agent::builder().name("b").build().unwrap())
            .unwrap();

        let err = runtime
            .spawn(Agent::builder().name("c").build().unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("Maximum agents"));
    }

    #[test]
    fn runtime_remove_agent() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let agent = Agent::builder().name("removable").build().unwrap();
        let id = runtime.spawn(agent).unwrap();

        assert_eq!(runtime.agent_count(), 1);
        runtime.remove(id).unwrap();
        assert_eq!(runtime.agent_count(), 0);
    }

    #[test]
    fn runtime_remove_unknown_agent() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());
        let err = runtime.remove(Uuid::new_v4()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn runtime_find_by_capability() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let trainer = Agent::builder()
            .name("trainer")
            .capability(AgentCapability::TrainingOrchestration)
            .build()
            .unwrap();
        runtime.spawn(trainer).unwrap();

        let reasoner = Agent::builder()
            .name("reasoner")
            .capability(AgentCapability::SymbolicReasoning)
            .build()
            .unwrap();
        runtime.spawn(reasoner).unwrap();

        let trainers = runtime.find_agents_by_capability("TrainingOrchestration");
        assert_eq!(trainers.len(), 1);
        assert_eq!(trainers[0].identity.name, "trainer");

        let reasoners = runtime.find_agents_by_capability("SymbolicReasoning");
        assert_eq!(reasoners.len(), 1);
    }

    #[test]
    fn runtime_workspace_shared() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let agent = Agent::builder().name("writer").build().unwrap();
        let id = runtime.spawn(agent).unwrap();

        let ws = runtime.workspace();
        ws.put("shared_key", b"shared_value".to_vec(), id);

        // Another reference to workspace should see the same data
        let ws2 = runtime.workspace();
        assert!(ws2.get("shared_key").is_some());
    }

    #[test]
    fn runtime_model_registry() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());
        let reg = runtime.model_registry();

        let author = Uuid::new_v4();
        let mut metrics = HashMap::new();
        metrics.insert("loss".to_string(), 0.01);

        reg.register("model_v1", b"weights".to_vec(), author, metrics);
        assert_eq!(reg.model_count(), 1);
    }

    #[tokio::test]
    async fn runtime_send_message() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let a = Agent::builder().name("sender").build().unwrap();
        let b = Agent::builder().name("receiver").build().unwrap();

        let a_id = runtime.spawn(a).unwrap();
        let b_id = runtime.spawn(b).unwrap();

        // Should not error
        let result = runtime
            .send(a_id, b_id, "test.topic", b"hello".to_vec())
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn runtime_broadcast() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let a = Agent::builder().name("broadcaster").build().unwrap();
        let a_id = runtime.spawn(a).unwrap();

        let result = runtime
            .broadcast(a_id, "system.announce", b"ping".to_vec())
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn runtime_stats() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let agent = Agent::builder().name("stats-agent").build().unwrap();
        runtime.spawn(agent).unwrap();

        let ws = runtime.workspace();
        ws.put("k", vec![], Uuid::new_v4());

        let stats = runtime.stats();
        assert_eq!(stats.agent_count, 1);
        assert_eq!(stats.workspace_entries, 1);
    }

    #[test]
    fn runtime_create_pipeline() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let a = Agent::builder().name("a").build().unwrap();
        let b = Agent::builder().name("b").build().unwrap();
        let a_id = runtime.spawn(a).unwrap();
        let b_id = runtime.spawn(b).unwrap();

        let pipeline = runtime
            .create_pipeline("test")
            .then("step1", a_id, "input", "mid")
            .then("step2", b_id, "mid", "output");

        assert_eq!(pipeline.stage_count(), 2);
        assert!(pipeline.validate().is_ok());
    }

    #[test]
    fn runtime_agent_ids() {
        let runtime = AgentRuntime::new(RuntimeConfig::default());

        let a = Agent::builder().name("a").build().unwrap();
        let b = Agent::builder().name("b").build().unwrap();
        let a_id = runtime.spawn(a).unwrap();
        let b_id = runtime.spawn(b).unwrap();

        let ids = runtime.agent_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&a_id));
        assert!(ids.contains(&b_id));
    }
}
