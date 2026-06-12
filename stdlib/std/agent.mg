//! # std::agent — Agent Primitives (MechGen-unique)
//!
//! Core building blocks for AI agent systems: agents, swarms, messages,
//! capability leases, code regions, and the swarm bus.
//! This module is unique to MechGen — there is no Rust equivalent.
//! All agent operations declare the `agent` effect.
//!
//! MechGen agents combine neural reasoning (LLM, vision, RL) with
//! symbolic knowledge (rules, facts, queries) for neurosymbolic AI.

// ---------------------------------------------------------------------------
// Agent trait
// ---------------------------------------------------------------------------

/// The core trait for an autonomous agent.
/// Every agent can receive messages, produce actions, and participate in swarms.
pub trait Agent {
    /// The type of messages this agent can handle.
    type Msg;
    /// The type of actions this agent can produce.
    type Action;

    /// Handle an incoming message and optionally produce an action.
    pub fn handle(&mut self, msg: Self::Msg) -> Option<Self::Action> / agent;

    /// Called once when the agent is started.
    pub fn on_start(&mut self) / agent {}

    /// Called once when the agent is stopped.
    pub fn on_stop(&mut self) / agent {}

    /// The agent's unique identifier.
    pub fn id(&self) -> AgentId;

    /// Current capabilities held by this agent.
    pub fn capabilities(&self) -> &[Capability];
}

/// Unique identifier for an agent instance.
pub struct AgentId {
    value: u128,
}

impl AgentId {
    /// Generate a new random agent ID.
    pub fn new() -> AgentId / agent;

    /// Create from a specific value (e.g., for tests).
    pub fn from(value: u128) -> AgentId {
        AgentId { value }
    }
}

impl std::fmt::Display for AgentId {
    pub fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::FmtError>;
}

// ---------------------------------------------------------------------------
// Swarm
// ---------------------------------------------------------------------------

/// A coordinated group of agents working together.
pub struct Swarm<A: Agent> {
    agents: Vec<Arc<A>>,
    bus: Bus,
    consensus: ConsensusStrategy,
    _running: bool,
}

impl Swarm<A: Agent> {
    /// Create a new swarm with the given consensus strategy.
    pub fn new(strategy: ConsensusStrategy) -> Swarm<A> / agent;

    /// Add an agent to the swarm.
    pub fn add(&mut self, agent: A) -> AgentId / agent;

    /// Remove an agent by ID.
    pub fn remove(&mut self, id: &AgentId) -> Result<A, AgentError> / agent;

    /// Send a message to all agents in the swarm.
    pub fn broadcast(&self, msg: &A::Msg) / agent;

    /// Send a message to a specific agent.
    pub fn send(&self, id: &AgentId, msg: A::Msg) -> Result<(), AgentError> / agent;

    /// Run the swarm until all agents are idle or stopped.
    pub fn run(&mut self) -> Result<Vec<A::Action>, AgentError> / agent;

    /// Run the swarm with a timeout.
    pub fn run_with_timeout(&mut self, dur: std::time::Duration) -> Result<Vec<A::Action>, AgentError> / agent;

    /// Gather a consensus from agent responses.
    pub fn consensus(&self, responses: &[A::Action]) -> Option<A::Action> / agent;

    /// Number of agents.
    pub fn size(&self) -> usize { self.agents.len() }

    /// Get the shared bus.
    pub fn bus(&self) -> &Bus { &self.bus }
}

/// Consensus strategies for swarm decision-making.
pub enum ConsensusStrategy {
    /// Majority vote (> 50% agreement).
    Majority,
    /// Unanimous agreement required.
    Unanimous,
    /// First response wins (fastest agent).
    FirstResponse,
    /// Weighted voting (agents have different weights).
    Weighted(Vec<f64>),
    /// Custom consensus function.
    Custom(fn(&[_]) -> Option<_>),
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A message passed between agents.
pub struct Message<T> {
    from: AgentId,
    to: Option<AgentId>,       // None = broadcast
    payload: T,
    timestamp: std::time::Instant,
    correlation_id: Option<u128>,
}

impl Message<T> {
    /// Create a new directed message.
    pub fn new(from: AgentId, to: AgentId, payload: T) -> Message<T> / agent;

    /// Create a broadcast message.
    pub fn broadcast(from: AgentId, payload: T) -> Message<T> / agent;

    /// Whether this is a broadcast message.
    pub fn is_broadcast(&self) -> bool { self.to.is_none() }
}

// ---------------------------------------------------------------------------
// Capability & Lease
// ---------------------------------------------------------------------------

/// A capability token granting access to a resource or operation.
pub struct Capability {
    name: String,
    scope: CapabilityScope,
    expires: Option<std::time::Instant>,
}

pub enum CapabilityScope {
    /// Access to a single resource instance.
    Instance(String),
    /// Access to all resources of a kind.
    Kind(String),
    /// Global access.
    Global,
}

/// A time-bounded lease on a capability.
pub struct Lease {
    capability: Capability,
    holder: AgentId,
    granted_at: std::time::Instant,
    expires_at: std::time::Instant,
}

impl Lease {
    /// Check if the lease is still valid.
    pub fn is_valid(&self) -> bool / time;

    /// Remaining duration on the lease.
    pub fn remaining(&self) -> std::time::Duration / time;

    /// Renew the lease for an additional duration.
    pub fn renew(&mut self, dur: std::time::Duration) -> Result<(), AgentError> / agent;

    /// Release the lease early.
    pub fn release(&mut self) / agent;
}

// ---------------------------------------------------------------------------
// Region
// ---------------------------------------------------------------------------

/// A code region with explicit capability requirements.
/// Regions enable fine-grained access control within agents.
pub struct Region {
    name: String,
    required_capabilities: Vec<String>,
}

impl Region {
    /// Define a new region with the given capability requirements.
    pub fn new(name: &String, caps: &[String]) -> Region {
        Region {
            name: name.to_owned(),
            required_capabilities: caps.iter().map(|c| c.to_owned()).collect(),
        }
    }

    /// Execute a closure within this region, verifying capabilities.
    pub fn enter<T>(&self, agent: &Agent, body: fn() -> T) -> Result<T, AgentError> / agent;
}

// ---------------------------------------------------------------------------
// Bus
// ---------------------------------------------------------------------------

/// The swarm communication bus. Agents publish and subscribe to topics.
pub struct Bus {
    _inner: Arc<_BusInner>,
}

impl Bus {
    /// Create a new bus.
    pub fn new() -> Bus / agent;

    /// Publish a message to a topic.
    pub fn publish<T: std::json::Serialize>(&self, topic: &String, payload: &T) / agent;

    /// Subscribe to a topic. Returns a receiver for messages.
    pub fn subscribe<T: std::json::Deserialize>(&self, topic: &String) -> std::sync::Receiver<T> / agent;

    /// Unsubscribe from a topic.
    pub fn unsubscribe(&self, topic: &String) / agent;

    /// List active topics.
    pub fn topics(&self) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// Agent Persistent Memory (P9 — 4-tier model)
// ---------------------------------------------------------------------------

/// Tier of agent memory persistence.
pub enum MemoryTier {
    /// Ephemeral: lives within a single request/invocation.
    Ephemeral,
    /// Session: persists for the duration of a conversation/session.
    Session,
    /// Project: persisted per-project, survives across sessions.
    Project,
    /// Global: shared across all projects and agents in the ecosystem.
    Global,
}

/// A single entry in agent memory.
pub struct MemoryEntry {
    key: String,
    value: String,
    tier: MemoryTier,
    created_at: std::time::Instant,
    updated_at: std::time::Instant,
    ttl: Option<std::time::Duration>,
    tags: Vec<String>,
}

/// Trait for agent memory stores. Implement per-tier backends.
pub trait MemoryStore {
    /// Retrieve an entry by key.
    pub fn get(&self, key: &String) -> Option<MemoryEntry> / agent;

    /// Store or update an entry.
    pub fn set(&mut self, key: &String, value: &String, tags: &[String]) -> Result<(), AgentError> / agent;

    /// Remove an entry.
    pub fn remove(&mut self, key: &String) -> Result<(), AgentError> / agent;

    /// Query entries by tag.
    pub fn query_by_tag(&self, tag: &String) -> Vec<MemoryEntry> / agent;

    /// List all keys (bounded by limit).
    pub fn keys(&self, limit: usize) -> Vec<String> / agent;

    /// Clear all entries.
    pub fn clear(&mut self) -> Result<(), AgentError> / agent;
}

/// Unified 4-tier memory manager for agents.
pub struct Memory {
    ephemeral: Box<MemoryStore>,
    session: Box<MemoryStore>,
    project: Box<MemoryStore>,
    global: Box<MemoryStore>,
}

impl Memory {
    /// Create a new Memory with the given per-tier backends.
    pub fn new(
        ephemeral: Box<MemoryStore>,
        session: Box<MemoryStore>,
        project: Box<MemoryStore>,
        global: Box<MemoryStore>,
    ) -> Memory / agent;

    /// Get a value, searching tiers in order: ephemeral -> session -> project -> global.
    pub fn get(&self, key: &String) -> Option<MemoryEntry> / agent;

    /// Set a value at a specific tier.
    pub fn set(&mut self, tier: MemoryTier, key: &String, value: &String, tags: &[String]) -> Result<(), AgentError> / agent;

    /// Remove from a specific tier.
    pub fn remove(&mut self, tier: MemoryTier, key: &String) -> Result<(), AgentError> / agent;

    /// Query across all tiers by tag.
    pub fn query_by_tag(&self, tag: &String) -> Vec<MemoryEntry> / agent;

    /// Promote an entry from one tier to a higher-persistence tier.
    pub fn promote(&mut self, key: &String, from: MemoryTier, to: MemoryTier) -> Result<(), AgentError> / agent;

    /// Flush ephemeral and session tiers (e.g., on session end).
    pub fn flush_transient(&mut self) -> Result<(), AgentError> / agent;
}

// ---------------------------------------------------------------------------
// Swarm Orchestration Patterns (P8, P10)
// ---------------------------------------------------------------------------

/// Map-reduce over a swarm: distribute work, collect, and merge.
///
/// ```mg
/// let results = swarm_map_reduce(&swarm, items, map_fn, reduce_fn);
/// ```
pub fn swarm_map_reduce<A: Agent, T, R>(
    swarm: &Swarm<A>,
    items: &[T],
    map_fn: fn(&A, &T) -> R,
    reduce_fn: fn(&[R]) -> R,
) -> Result<R, AgentError> / agent;

/// Pipeline: chain agents sequentially, output of one feeds input of next.
///
/// ```mg
/// let result = swarm_pipeline(&stages, input);
/// ```
pub fn swarm_pipeline<T>(
    stages: &[Box<Agent<Msg = T, Action = T>>],
    input: T,
) -> Result<T, AgentError> / agent;

/// Saga: multi-step workflow with compensating rollbacks on failure.
///
/// Each step is a (forward_fn, rollback_fn) pair. If any step fails,
/// all completed steps are rolled back in reverse order.
pub fn swarm_saga<A: Agent, T>(
    swarm: &Swarm<A>,
    steps: &[(fn(&A, &T) -> Result<T, AgentError>, fn(&A, &T) -> Result<(), AgentError>)],
    input: T,
) -> Result<T, AgentError> / agent;

/// Fan-out: send the same task to N agents, collect all results.
pub fn swarm_fan_out<A: Agent, T, R>(
    swarm: &Swarm<A>,
    task: &T,
) -> Result<Vec<R>, AgentError> / agent;

/// Race: send task to all agents, return the first successful result.
pub fn swarm_race<A: Agent, T, R>(
    swarm: &Swarm<A>,
    task: &T,
    timeout: std::time::Duration,
) -> Result<R, AgentError> / agent;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub struct AgentError {
    kind: AgentErrorKind,
    message: String,
}

pub enum AgentErrorKind {
    NotFound,
    CapabilityDenied,
    LeaseExpired,
    ConsensusFailure,
    Timeout,
    BusError,
    MemoryError,
    SagaRollbackFailed,
    Other,
}
