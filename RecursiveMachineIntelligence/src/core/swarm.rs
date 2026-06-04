//! Multi-agent swarm coordination for autonomous AI model development.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Swarm Configuration
// ============================================================================

/// Configuration for swarm behavior and coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    /// Maximum number of agents allowed in the swarm
    pub max_agents: usize,
    /// Threshold for consensus (0.0 to 1.0)
    pub consensus_threshold: f64,
    /// Resource sharing mode between agents
    pub resource_sharing: ResourceSharingMode,
    /// Strategy for agent collaboration
    pub collaboration_strategy: CollaborationStrategy,
    /// Timeout for task completion
    pub task_timeout: Duration,
    /// Enable automatic load balancing
    pub auto_load_balance: bool,
    /// Heartbeat interval for agent health checks
    pub heartbeat_interval: Duration,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_agents: 16,
            consensus_threshold: 0.66,
            resource_sharing: ResourceSharingMode::Cooperative,
            collaboration_strategy: CollaborationStrategy::Emergent,
            task_timeout: Duration::from_secs(3600),
            auto_load_balance: true,
            heartbeat_interval: Duration::from_secs(30),
        }
    }
}

/// How resources (compute, memory, data) are shared between agents in the swarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceSharingMode {
    /// Agents freely pool and share resources to maximize collective throughput.
    /// This is the default for [`SwarmConfig`].
    Cooperative,
    /// Agents compete for a shared resource pool; allocation favors the most
    /// productive agents.
    Competitive,
    /// Resources are allocated according to per-agent priority weights.
    Prioritized,
    /// Each agent operates on its own resources with no sharing.
    Isolated,
}

/// Strategy governing how agents coordinate to complete tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationStrategy {
    /// No central planner; coordination arises from local agent interactions.
    /// This is the default for [`SwarmConfig`].
    Emergent,
    /// A single coordinator assigns work and aggregates results.
    Centralized,
    /// Agents work the same problem independently and the best result wins.
    Competitive,
    /// Work is split (map) across agents and their outputs combined (reduce).
    MapReduce,
    /// Agents produce diverse solutions that are combined into an ensemble.
    Ensemble,
}

// ============================================================================
// Agent Registration
// ============================================================================

/// Registration record describing a single agent participating in the swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAgentInfo {
    /// Unique identifier assigned when the agent registers.
    pub id: Uuid,
    /// Human-readable agent name.
    pub name: String,
    /// Capability tags the agent advertises (e.g. `"training"`,
    /// `"architecture_design"`, or `"all"`). These are matched against the
    /// capability a task requires during scheduling.
    pub capabilities: Vec<String>,
    /// Current lifecycle state of the agent.
    pub state: SwarmAgentState,
    /// IDs of tasks currently assigned to this agent.
    pub assigned_tasks: Vec<Uuid>,
    /// Unix timestamp (seconds since the epoch) of the agent's last heartbeat.
    pub last_heartbeat: u64,
    /// Running performance statistics for this agent.
    pub metrics: AgentMetrics,
}

/// Lifecycle state of a swarm agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmAgentState {
    /// Registered and available to accept work.
    Idle,
    /// Currently executing one or more assigned tasks.
    Working,
    /// Participating in a consensus vote.
    Voting,
    /// Temporarily withheld from scheduling.
    Suspended,
    /// Considered offline (e.g. missed heartbeats).
    Disconnected,
}

/// Running performance statistics tracked per agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMetrics {
    /// Number of tasks the agent has completed successfully.
    pub tasks_completed: usize,
    /// Number of tasks the agent has failed.
    pub tasks_failed: usize,
    /// Average task completion time, in milliseconds.
    pub avg_completion_time_ms: f64,
    /// Fraction of the agent's resources currently in use, from 0.0 to 1.0.
    pub resource_utilization: f64,
    /// Aggregate quality score for the agent's outputs, typically 0.0 to 1.0.
    pub quality_score: f64,
}

// ============================================================================
// Task System
// ============================================================================

/// A unit of work submitted to the swarm for an agent to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    /// Unique identifier for this task.
    pub id: Uuid,
    /// The kind of work to perform and its type-specific parameters.
    pub task_type: SwarmTaskType,
    /// Scheduling priority; higher-priority tasks are assigned first.
    pub priority: TaskPriority,
    /// IDs of tasks that must complete before this one can run.
    pub dependencies: Vec<Uuid>,
    /// Current execution status.
    pub status: TaskStatus,
    /// Agent currently responsible for the task, if it has been assigned.
    pub assigned_agent: Option<Uuid>,
    /// Unix timestamp (seconds since the epoch) when the task was created.
    pub created_at: u64,
    /// Optional Unix timestamp (seconds) by which the task should complete.
    pub deadline: Option<u64>,
    /// Free-form key/value metadata. The constructor stores the task
    /// description under the `"description"` key.
    pub metadata: HashMap<String, String>,
}

impl SwarmTask {
    /// Creates a pending, normal-priority task of the given type. `description`
    /// is stored in [`metadata`](Self::metadata) under the `"description"` key,
    /// and `created_at` is set to the current time.
    pub fn new(task_type: SwarmTaskType, description: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_type,
            priority: TaskPriority::Normal,
            dependencies: Vec::new(),
            status: TaskStatus::Pending,
            assigned_agent: None,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            deadline: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("description".to_string(), description.to_string());
                m
            },
        }
    }

    /// Sets the task's scheduling priority (builder style).
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Adds a prerequisite task that must complete before this one (builder
    /// style). May be called repeatedly to add multiple dependencies.
    pub fn with_dependency(mut self, task_id: Uuid) -> Self {
        self.dependencies.push(task_id);
        self
    }
}

/// The category of work a [`SwarmTask`] represents, with its parameters.
///
/// Each variant maps to a required agent capability used during scheduling
/// (see [`SwarmCoordinator::schedule`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmTaskType {
    /// Design a model architecture. Requires the `"architecture_design"`
    /// capability.
    DesignArchitecture {
        /// Expected input tensor shape.
        input_shape: Vec<usize>,
        /// Desired output tensor shape.
        output_shape: Vec<usize>,
        /// Constraints the architecture must satisfy.
        constraints: ModelConstraints,
    },
    /// Train a designed model. Requires the `"training"` capability.
    TrainModel {
        /// ID of the architecture to train.
        architecture_id: Uuid,
        /// Training hyperparameters and schedule.
        config: TrainingConfig,
    },
    /// Evaluate a trained model. Requires the `"evaluation"` capability.
    EvaluateModel {
        /// ID of the model to evaluate.
        model_id: Uuid,
        /// Names of the metrics to compute.
        metrics: Vec<String>,
    },
    /// Search for better hyperparameters. Requires the `"optimization"`
    /// capability.
    OptimizeHyperparameters {
        /// ID of the model to tune.
        model_id: Uuid,
        /// Search space mapping each hyperparameter name to its range.
        search_space: HashMap<String, ParameterRange>,
    },
    /// Combine multiple models into one. Requires the `"merging"` capability.
    MergeModels {
        /// IDs of the models to merge.
        model_ids: Vec<Uuid>,
        /// How the models should be merged.
        strategy: MergeStrategy,
    },
    /// An application-defined task. The required capability is the value of
    /// `name` (so an agent must advertise that name, or `"all"`).
    Custom {
        /// Task name, also used as the required capability tag.
        name: String,
        /// Arbitrary task parameters.
        params: HashMap<String, String>,
    },
}

/// Scheduling priority of a task. Ordered from lowest to highest, so
/// `Low < Normal < High < Critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Lowest priority.
    Low,
    /// Default priority for newly created tasks.
    Normal,
    /// Above-normal priority.
    High,
    /// Highest priority; scheduled ahead of all others.
    Critical,
}

/// Execution status of a [`SwarmTask`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Queued and waiting to be assigned.
    Pending,
    /// Assigned to an agent and currently executing.
    InProgress,
    /// Finished successfully.
    Completed,
    /// Finished unsuccessfully.
    Failed,
    /// Abandoned before completion.
    Cancelled,
    /// Waiting on unmet dependencies.
    Blocked,
}

/// Constraints a candidate model must satisfy. Each `Option` field is an
/// upper or target bound that is ignored when `None`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelConstraints {
    /// Maximum number of model parameters allowed.
    pub max_parameters: Option<usize>,
    /// Maximum model memory footprint, in bytes.
    pub max_memory_bytes: Option<usize>,
    /// Maximum inference latency, in milliseconds.
    pub max_latency_ms: Option<f64>,
    /// Target accuracy to reach, as a ratio from 0.0 to 1.0.
    pub target_accuracy: Option<f64>,
    /// Capabilities required of agents that work on this model.
    pub required_capabilities: Vec<String>,
}

/// Hyperparameters and schedule for a [`SwarmTaskType::TrainModel`] task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Number of training epochs.
    pub epochs: usize,
    /// Mini-batch size.
    pub batch_size: usize,
    /// Learning rate.
    pub learning_rate: f64,
    /// Name of the optimizer to use (e.g. `"adam"`).
    pub optimizer: String,
    /// Additional optimizer-specific hyperparameters keyed by name.
    pub hyperparameters: HashMap<String, f64>,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: 100,
            batch_size: 32,
            learning_rate: 0.001,
            optimizer: "adam".to_string(),
            hyperparameters: HashMap::new(),
        }
    }
}

/// The range of values a hyperparameter may take during a search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterRange {
    /// Real-valued range sampled uniformly between `min` and `max`.
    Continuous {
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
    /// Integer range from `min` to `max` inclusive.
    Discrete {
        /// Inclusive lower bound.
        min: i64,
        /// Inclusive upper bound.
        max: i64,
    },
    /// One value chosen from a fixed set of categorical `choices`.
    Categorical {
        /// The allowed values.
        choices: Vec<String>,
    },
    /// Real-valued range sampled on a logarithmic scale between `min` and
    /// `max` (useful for learning rates and similar parameters).
    LogScale {
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
}

/// Strategy for combining multiple models into one in a
/// [`SwarmTaskType::MergeModels`] task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Average the corresponding weights of all models.
    WeightAveraging,
    /// Weighted averaging in the style of federated learning.
    FederatedAveraging,
    /// Distill the models into a single student model.
    Distillation,
    /// Keep the models separate and combine their predictions as an ensemble.
    Ensemble,
    /// Merge only selected layers or parameters from each model.
    SelectiveMerge,
}

// ============================================================================
// Consensus Protocol
// ============================================================================

/// A proposal put before the swarm for agents to vote on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique identifier for this proposal.
    pub id: Uuid,
    /// What the proposal concerns and its associated payload.
    pub proposal_type: ProposalType,
    /// ID of the agent that raised the proposal.
    pub proposer: Uuid,
    /// Human-readable description of the proposal.
    pub description: String,
    /// Votes cast so far.
    pub votes: Vec<Vote>,
    /// Current status of the proposal.
    pub status: ProposalStatus,
    /// Unix timestamp (seconds) when the proposal was created.
    pub created_at: u64,
    /// Unix timestamp (seconds) after which voting is considered closed.
    pub deadline: u64,
}

impl Proposal {
    /// Creates a new proposal open for voting, with `created_at` set to now and
    /// a voting `deadline` one hour later.
    pub fn new(proposal_type: ProposalType, proposer: Uuid, description: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            id: Uuid::new_v4(),
            proposal_type,
            proposer,
            description: description.to_string(),
            votes: Vec::new(),
            status: ProposalStatus::Voting,
            created_at: now,
            deadline: now + 3600,
        }
    }

    /// Records a vote on this proposal.
    pub fn add_vote(&mut self, vote: Vote) {
        self.votes.push(vote);
    }

    /// Returns the fraction of cast votes that are [`VoteDecision::Approve`],
    /// from 0.0 to 1.0. Abstentions and rejections count against the ratio.
    /// Returns 0.0 when no votes have been cast.
    pub fn approval_ratio(&self) -> f64 {
        if self.votes.is_empty() {
            return 0.0;
        }
        let approvals = self
            .votes
            .iter()
            .filter(|v| matches!(v.decision, VoteDecision::Approve))
            .count();
        approvals as f64 / self.votes.len() as f64
    }

    /// Returns `true` when the [`approval_ratio`](Self::approval_ratio) meets or
    /// exceeds `threshold` (a value from 0.0 to 1.0, e.g.
    /// [`SwarmConfig::consensus_threshold`]).
    pub fn has_consensus(&self, threshold: f64) -> bool {
        self.approval_ratio() >= threshold
    }
}

/// The subject of a [`Proposal`], carrying the relevant target or payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    /// Proposes adopting the architecture with the given ID.
    Architecture(Uuid),
    /// Proposes a training configuration for the model with the given ID.
    TrainingConfig(Uuid),
    /// Proposes deploying the model with the given ID.
    ModelDeployment(Uuid),
    /// Proposes a change to the named workflow.
    WorkflowChange(String),
    /// Proposes a resource allocation mapping each agent ID to its share.
    ResourceAllocation(HashMap<Uuid, f64>),
    /// An application-defined proposal identified by name.
    Custom(String),
}

/// A single vote cast by an agent on a [`Proposal`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    /// ID of the agent casting the vote.
    pub voter: Uuid,
    /// The voter's decision.
    pub decision: VoteDecision,
    /// Voter's confidence in the decision, from 0.0 to 1.0.
    pub confidence: f64,
    /// Optional free-text rationale for the vote.
    pub justification: Option<String>,
    /// Unix timestamp (seconds) when the vote was cast.
    pub timestamp: u64,
}

impl Vote {
    /// Creates a vote with full confidence (1.0), no justification, and the
    /// current timestamp.
    pub fn new(voter: Uuid, decision: VoteDecision) -> Self {
        Self {
            voter,
            decision,
            confidence: 1.0,
            justification: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Sets the voter's confidence (builder style), clamped to the range
    /// 0.0 to 1.0.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Attaches a free-text justification to the vote (builder style).
    pub fn with_justification(mut self, justification: &str) -> Self {
        self.justification = Some(justification.to_string());
        self
    }
}

/// A voter's decision on a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteDecision {
    /// In favor of the proposal.
    Approve,
    /// Against the proposal.
    Reject,
    /// Declines to take a side; counts against the approval ratio.
    Abstain,
}

/// Lifecycle status of a [`Proposal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Open and collecting votes.
    Voting,
    /// Reached consensus and was accepted.
    Approved,
    /// Did not reach consensus and was rejected.
    Rejected,
    /// Closed without resolution because its deadline passed.
    Expired,
    /// Retracted by the proposer before resolution.
    Withdrawn,
}

// ============================================================================
// Collaborative Workflow
// ============================================================================

/// An ordered sequence of stages that the swarm executes collaboratively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborativeWorkflow {
    /// Unique identifier for this workflow.
    pub id: Uuid,
    /// Human-readable workflow name.
    pub name: String,
    /// The stages, executed in order.
    pub stages: Vec<WorkflowStage>,
    /// Index into [`stages`](Self::stages) of the stage currently executing.
    pub current_stage: usize,
    /// Current status of the workflow.
    pub status: WorkflowStatus,
    /// Results recorded for completed stages, keyed by stage index.
    pub stage_results: HashMap<usize, StageResult>,
}

impl CollaborativeWorkflow {
    /// Creates an empty workflow in the [`WorkflowStatus::Created`] state.
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            stages: Vec::new(),
            current_stage: 0,
            status: WorkflowStatus::Created,
            stage_results: HashMap::new(),
        }
    }

    /// Appends a stage to the workflow (builder style).
    pub fn add_stage(mut self, stage: WorkflowStage) -> Self {
        self.stages.push(stage);
        self
    }

    /// Returns the stage currently being executed, or `None` if the workflow
    /// has no stages.
    pub fn current(&self) -> Option<&WorkflowStage> {
        self.stages.get(self.current_stage)
    }

    /// Advances to the next stage. Returns `true` if there was a next stage to
    /// move to; if already at the last stage, sets the status to
    /// [`WorkflowStatus::Completed`] and returns `false`.
    pub fn advance(&mut self) -> bool {
        if self.current_stage < self.stages.len() - 1 {
            self.current_stage += 1;
            true
        } else {
            self.status = WorkflowStatus::Completed;
            false
        }
    }
}

/// A single stage within a [`CollaborativeWorkflow`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStage {
    /// Human-readable stage name.
    pub name: String,
    /// Execution mode for the stage's tasks.
    pub stage_type: StageType,
    /// Tasks that make up this stage.
    pub tasks: Vec<SwarmTask>,
    /// Condition that must hold for the stage to be considered finished.
    pub completion: CompletionCriteria,
    /// Maximum time the stage may run before timing out.
    pub timeout: Duration,
}

impl WorkflowStage {
    /// Creates a stage with no tasks, [`CompletionCriteria::AllTasks`], and a
    /// default one-hour timeout.
    pub fn new(name: &str, stage_type: StageType) -> Self {
        Self {
            name: name.to_string(),
            stage_type,
            tasks: Vec::new(),
            completion: CompletionCriteria::AllTasks,
            timeout: Duration::from_secs(3600),
        }
    }

    /// Appends a task to the stage (builder style).
    pub fn add_task(mut self, task: SwarmTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Sets the stage's completion criteria (builder style).
    pub fn with_completion(mut self, criteria: CompletionCriteria) -> Self {
        self.completion = criteria;
        self
    }

    /// Sets the stage's timeout (builder style).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// How the tasks within a [`WorkflowStage`] are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageType {
    /// All tasks run concurrently.
    Parallel,
    /// Agents compete on the tasks; the best result is kept.
    Competitive,
    /// Agents must reach consensus to complete the stage.
    Consensus,
    /// Tasks are mapped across agents and their outputs reduced.
    MapReduce,
    /// Tasks are repeated over multiple refinement iterations.
    Iterative,
}

/// Condition under which a [`WorkflowStage`] is considered complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionCriteria {
    /// Every task in the stage must finish.
    AllTasks,
    /// At least the given number of tasks must finish.
    MinTasks(usize),
    /// Consensus must reach the given approval ratio (0.0 to 1.0).
    ConsensusReached(f64),
    /// A named metric must meet or exceed a threshold.
    QualityThreshold {
        /// Name of the metric to check.
        metric: String,
        /// Minimum acceptable value for the metric.
        threshold: f64,
    },
    /// The stage completes as soon as the first task finishes.
    FirstComplete,
    /// An application-defined completion rule identified by name.
    Custom(String),
}

/// Lifecycle status of a [`CollaborativeWorkflow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStatus {
    /// Constructed but not yet started.
    Created,
    /// Currently executing.
    Running,
    /// Temporarily halted and resumable.
    Paused,
    /// Finished all stages successfully.
    Completed,
    /// Terminated due to a failure.
    Failed,
    /// Abandoned before completion.
    Cancelled,
}

/// Outcome recorded for a completed [`WorkflowStage`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageResult {
    /// Index of the stage these results belong to.
    pub stage: usize,
    /// Name of the stage.
    pub name: String,
    /// Whether the stage completed successfully.
    pub success: bool,
    /// Named output values produced by the stage.
    pub outputs: HashMap<String, String>,
    /// Unix timestamp (seconds) when the stage completed.
    pub completed_at: u64,
}

// ============================================================================
// Model Development Pipeline
// ============================================================================

/// An end-to-end model development run, pairing a problem definition with the
/// collaborative workflow that produces and selects candidate models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDevelopmentPipeline {
    /// Pipeline name, derived from the problem type by default.
    pub name: String,
    /// The kind of problem being solved (e.g. `"classification"`).
    pub problem_type: String,
    /// Constraints every candidate model must satisfy.
    pub constraints: ModelConstraints,
    /// The workflow that drives exploration, training, and evaluation.
    pub workflow: CollaborativeWorkflow,
    /// ID of the best model found so far, if any.
    pub best_model: Option<Uuid>,
    /// IDs of all candidate models produced.
    pub candidates: Vec<Uuid>,
    /// Aggregate pipeline metrics keyed by name.
    pub metrics: HashMap<String, f64>,
}

impl ModelDevelopmentPipeline {
    /// Creates a pipeline for the given problem type, seeded with the
    /// [default workflow](Self::default_workflow) and default constraints.
    pub fn new(problem_type: &str) -> Self {
        let workflow = Self::default_workflow(problem_type);
        Self {
            name: format!("{}_pipeline", problem_type),
            problem_type: problem_type.to_string(),
            constraints: ModelConstraints::default(),
            workflow,
            best_model: None,
            candidates: Vec::new(),
            metrics: HashMap::new(),
        }
    }

    /// Replaces the pipeline's model constraints (builder style).
    pub fn with_constraints(mut self, constraints: ModelConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Builds the default five-stage development workflow: parallel
    /// exploration, consensus architecture selection, competitive training,
    /// iterative optimization, and parallel evaluation.
    fn default_workflow(problem_type: &str) -> CollaborativeWorkflow {
        CollaborativeWorkflow::new(&format!("{}_development", problem_type))
            .add_stage(
                WorkflowStage::new("exploration", StageType::Parallel)
                    .with_completion(CompletionCriteria::MinTasks(3)),
            )
            .add_stage(
                WorkflowStage::new("architecture_selection", StageType::Consensus)
                    .with_completion(CompletionCriteria::ConsensusReached(0.66)),
            )
            .add_stage(
                WorkflowStage::new("training", StageType::Competitive).with_completion(
                    CompletionCriteria::QualityThreshold {
                        metric: "validation_accuracy".to_string(),
                        threshold: 0.9,
                    },
                ),
            )
            .add_stage(
                WorkflowStage::new("optimization", StageType::Iterative).with_completion(
                    CompletionCriteria::QualityThreshold {
                        metric: "test_accuracy".to_string(),
                        threshold: 0.95,
                    },
                ),
            )
            .add_stage(
                WorkflowStage::new("evaluation", StageType::Parallel)
                    .with_completion(CompletionCriteria::AllTasks),
            )
    }
}

// ============================================================================
// Autonomous Model Builder
// ============================================================================

/// Fluent builder that assembles a [`ModelDevelopmentPipeline`] and matching
/// [`SwarmConfig`] for fully autonomous model development.
#[derive(Debug, Default)]
pub struct AutonomousModelBuilder {
    problem_type: String,
    constraints: ModelConstraints,
    collaboration_strategy: Option<CollaborationStrategy>,
    target_metrics: HashMap<String, f64>,
    max_iterations: usize,
}

impl AutonomousModelBuilder {
    /// Starts a builder for the given problem type with default constraints
    /// and a default cap of 100 iterations.
    pub fn new(problem_type: &str) -> Self {
        Self {
            problem_type: problem_type.to_string(),
            constraints: ModelConstraints::default(),
            collaboration_strategy: None,
            target_metrics: HashMap::new(),
            max_iterations: 100,
        }
    }

    /// Sets the constraints candidate models must satisfy (builder style).
    pub fn with_constraints(mut self, constraints: ModelConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Sets the collaboration strategy for the resulting swarm (builder style).
    /// If unset, [`build`](Self::build) defaults to
    /// [`CollaborationStrategy::Emergent`].
    pub fn collaboration_strategy(mut self, strategy: CollaborationStrategy) -> Self {
        self.collaboration_strategy = Some(strategy);
        self
    }

    /// Adds a named target metric and its desired value (builder style). May be
    /// called multiple times to set several targets.
    pub fn target_metric(mut self, name: &str, value: f64) -> Self {
        self.target_metrics.insert(name.to_string(), value);
        self
    }

    /// Sets the maximum number of development iterations (builder style).
    pub fn max_iterations(mut self, iterations: usize) -> Self {
        self.max_iterations = iterations;
        self
    }

    /// Consumes the builder and produces the configured pipeline together with
    /// the [`SwarmConfig`] that should run it.
    pub fn build(self) -> (ModelDevelopmentPipeline, SwarmConfig) {
        let pipeline =
            ModelDevelopmentPipeline::new(&self.problem_type).with_constraints(self.constraints);

        let config = SwarmConfig {
            collaboration_strategy: self
                .collaboration_strategy
                .unwrap_or(CollaborationStrategy::Emergent),
            ..Default::default()
        };

        (pipeline, config)
    }
}

// ============================================================================
// Swarm Coordinator
// ============================================================================

/// Central coordinator that manages agents, schedules tasks, runs consensus
/// votes, and drives workflows for a swarm.
#[derive(Debug)]
pub struct SwarmCoordinator {
    config: SwarmConfig,
    agents: HashMap<Uuid, SwarmAgentInfo>,
    task_queue: VecDeque<SwarmTask>,
    active_tasks: HashMap<Uuid, SwarmTask>,
    completed_tasks: Vec<SwarmTask>,
    proposals: HashMap<Uuid, Proposal>,
    workflows: HashMap<Uuid, CollaborativeWorkflow>,
    state: CoordinatorState,
}

/// Lifecycle state of a [`SwarmCoordinator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinatorState {
    /// Created but not yet started.
    Starting,
    /// Actively scheduling and coordinating.
    Running,
    /// Temporarily halted; scheduling is suspended.
    Paused,
    /// In the process of shutting down.
    ShuttingDown,
}

impl SwarmCoordinator {
    /// Creates a coordinator with the given configuration in the
    /// [`CoordinatorState::Starting`] state.
    pub fn new(config: SwarmConfig) -> Self {
        Self {
            config,
            agents: HashMap::new(),
            task_queue: VecDeque::new(),
            active_tasks: HashMap::new(),
            completed_tasks: Vec::new(),
            proposals: HashMap::new(),
            workflows: HashMap::new(),
            state: CoordinatorState::Starting,
        }
    }

    /// Registers a new agent with the given name and capabilities, returning
    /// its assigned ID. The agent starts [`Idle`](SwarmAgentState::Idle).
    ///
    /// # Errors
    ///
    /// Returns [`RmiError::ResourceExhausted`] if the swarm already holds
    /// [`SwarmConfig::max_agents`] agents.
    pub fn register_agent(&mut self, name: &str, capabilities: Vec<String>) -> Result<Uuid> {
        if self.agents.len() >= self.config.max_agents {
            return Err(RmiError::ResourceExhausted(
                "Maximum agents reached".to_string(),
            ));
        }

        let id = Uuid::new_v4();
        let info = SwarmAgentInfo {
            id,
            name: name.to_string(),
            capabilities,
            state: SwarmAgentState::Idle,
            assigned_tasks: Vec::new(),
            last_heartbeat: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            metrics: AgentMetrics::default(),
        };

        self.agents.insert(id, info);
        Ok(id)
    }

    /// Removes an agent from the swarm. Any tasks it had in progress are reset
    /// to [`Pending`](TaskStatus::Pending) and pushed back to the front of the
    /// queue for reassignment.
    ///
    /// # Errors
    ///
    /// Returns [`RmiError::Agent`] if no agent with `agent_id` exists.
    pub fn unregister_agent(&mut self, agent_id: Uuid) -> Result<()> {
        if let Some(agent) = self.agents.remove(&agent_id) {
            for task_id in agent.assigned_tasks {
                if let Some(task) = self.active_tasks.get_mut(&task_id) {
                    task.assigned_agent = None;
                    task.status = TaskStatus::Pending;
                    self.task_queue.push_front(task.clone());
                }
            }
            Ok(())
        } else {
            Err(RmiError::Agent(format!("Agent {} not found", agent_id)))
        }
    }

    /// Enqueues a task for scheduling and returns its ID.
    pub fn submit_task(&mut self, task: SwarmTask) -> Uuid {
        let id = task.id;
        self.task_queue.push_back(task);
        id
    }

    /// Looks up the status of a task by ID, searching active, queued, and
    /// completed tasks. Returns `None` if no such task is known.
    pub fn task_status(&self, task_id: Uuid) -> Option<TaskStatus> {
        if let Some(task) = self.active_tasks.get(&task_id) {
            return Some(task.status);
        }
        for task in &self.task_queue {
            if task.id == task_id {
                return Some(task.status);
            }
        }
        for task in &self.completed_tasks {
            if task.id == task_id {
                return Some(task.status);
            }
        }
        None
    }

    /// Registers a proposal for voting and returns its ID.
    pub fn create_proposal(&mut self, proposal: Proposal) -> Uuid {
        let id = proposal.id;
        self.proposals.insert(id, proposal);
        id
    }

    /// Records a vote on a proposal. If the proposal then reaches
    /// [`SwarmConfig::consensus_threshold`], its status is set to
    /// [`ProposalStatus::Approved`].
    ///
    /// # Errors
    ///
    /// Returns [`RmiError::Agent`] if no proposal with `proposal_id` exists.
    pub fn submit_vote(&mut self, proposal_id: Uuid, vote: Vote) -> Result<()> {
        if let Some(proposal) = self.proposals.get_mut(&proposal_id) {
            proposal.add_vote(vote);
            if proposal.has_consensus(self.config.consensus_threshold) {
                proposal.status = ProposalStatus::Approved;
            }
            Ok(())
        } else {
            Err(RmiError::Agent(format!(
                "Proposal {} not found",
                proposal_id
            )))
        }
    }

    /// Returns the full proposal record by ID, or `None` if unknown.
    pub fn proposal_status(&self, proposal_id: Uuid) -> Option<&Proposal> {
        self.proposals.get(&proposal_id)
    }

    /// Registers a workflow, marks it [`Running`](WorkflowStatus::Running), and
    /// returns its ID.
    pub fn start_workflow(&mut self, workflow: CollaborativeWorkflow) -> Uuid {
        let id = workflow.id;
        let mut workflow = workflow;
        workflow.status = WorkflowStatus::Running;
        self.workflows.insert(id, workflow);
        id
    }

    /// Returns the workflow record by ID, or `None` if unknown.
    pub fn workflow_status(&self, workflow_id: Uuid) -> Option<&CollaborativeWorkflow> {
        self.workflows.get(&workflow_id)
    }

    /// Returns an iterator over all registered agents.
    pub fn agents(&self) -> impl Iterator<Item = &SwarmAgentInfo> {
        self.agents.values()
    }

    /// Returns the number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Returns the number of tasks waiting in the queue.
    pub fn pending_tasks(&self) -> usize {
        self.task_queue.len()
    }

    /// Returns the number of tasks currently assigned and in progress.
    pub fn active_tasks(&self) -> usize {
        self.active_tasks.len()
    }

    /// Assigns queued tasks to idle agents. For each idle agent, the
    /// highest-priority pending task it is [capable](SwarmTaskType) of handling
    /// is removed from the queue, marked [`InProgress`](TaskStatus::InProgress),
    /// and the agent transitions to [`Working`](SwarmAgentState::Working).
    pub fn schedule(&mut self) {
        let idle_agents: Vec<Uuid> = self
            .agents
            .iter()
            .filter(|(_, a)| a.state == SwarmAgentState::Idle)
            .map(|(id, _)| *id)
            .collect();

        for agent_id in idle_agents {
            if let Some(mut task) = self.next_task_for_agent(agent_id) {
                task.assigned_agent = Some(agent_id);
                task.status = TaskStatus::InProgress;

                if let Some(agent) = self.agents.get_mut(&agent_id) {
                    agent.state = SwarmAgentState::Working;
                    agent.assigned_tasks.push(task.id);
                }

                self.active_tasks.insert(task.id, task);
            }
        }
    }

    fn next_task_for_agent(&mut self, agent_id: Uuid) -> Option<SwarmTask> {
        let agent = self.agents.get(&agent_id)?;
        let mut best_idx = None;
        let mut best_priority = TaskPriority::Low;

        for (idx, task) in self.task_queue.iter().enumerate() {
            if task.status != TaskStatus::Pending {
                continue;
            }
            if self.agent_can_handle(agent, &task.task_type) && task.priority >= best_priority {
                best_idx = Some(idx);
                best_priority = task.priority;
            }
        }

        best_idx.map(|idx| self.task_queue.remove(idx).expect("best_idx is valid task_queue index"))
    }

    fn agent_can_handle(&self, agent: &SwarmAgentInfo, task_type: &SwarmTaskType) -> bool {
        let required_cap = match task_type {
            SwarmTaskType::DesignArchitecture { .. } => "architecture_design",
            SwarmTaskType::TrainModel { .. } => "training",
            SwarmTaskType::EvaluateModel { .. } => "evaluation",
            SwarmTaskType::OptimizeHyperparameters { .. } => "optimization",
            SwarmTaskType::MergeModels { .. } => "merging",
            SwarmTaskType::Custom { name, .. } => name.as_str(),
        };

        agent
            .capabilities
            .iter()
            .any(|c| c == required_cap || c == "all")
    }

    /// Marks an active task finished. The task moves to the completed list with
    /// status [`Completed`](TaskStatus::Completed) or
    /// [`Failed`](TaskStatus::Failed) per `success`, and its agent returns to
    /// [`Idle`](SwarmAgentState::Idle) with its completed/failed metric
    /// incremented.
    ///
    /// # Errors
    ///
    /// Returns [`RmiError::Agent`] if `task_id` is not an active task.
    pub fn complete_task(&mut self, task_id: Uuid, success: bool) -> Result<()> {
        if let Some(mut task) = self.active_tasks.remove(&task_id) {
            task.status = if success {
                TaskStatus::Completed
            } else {
                TaskStatus::Failed
            };
            if let Some(agent_id) = task.assigned_agent {
                if let Some(agent) = self.agents.get_mut(&agent_id) {
                    agent.state = SwarmAgentState::Idle;
                    agent.assigned_tasks.retain(|&id| id != task_id);
                    if success {
                        agent.metrics.tasks_completed += 1;
                    } else {
                        agent.metrics.tasks_failed += 1;
                    }
                }
            }
            self.completed_tasks.push(task);
            Ok(())
        } else {
            Err(RmiError::Agent(format!("Task {} not found", task_id)))
        }
    }

    /// Transitions the coordinator to [`CoordinatorState::Running`].
    pub fn start(&mut self) {
        self.state = CoordinatorState::Running;
    }
    /// Transitions the coordinator to [`CoordinatorState::Paused`].
    pub fn pause(&mut self) {
        self.state = CoordinatorState::Paused;
    }
    /// Transitions the coordinator to [`CoordinatorState::ShuttingDown`].
    pub fn stop(&mut self) {
        self.state = CoordinatorState::ShuttingDown;
    }
    /// Returns `true` only while the coordinator is in
    /// [`CoordinatorState::Running`].
    pub fn is_running(&self) -> bool {
        self.state == CoordinatorState::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== SwarmConfig =====

    #[test]
    fn swarm_config_defaults() {
        let cfg = SwarmConfig::default();
        assert_eq!(cfg.max_agents, 16);
        assert!((cfg.consensus_threshold - 0.66).abs() < 1e-9);
        assert_eq!(cfg.resource_sharing, ResourceSharingMode::Cooperative);
        assert_eq!(cfg.collaboration_strategy, CollaborationStrategy::Emergent);
        assert!(cfg.auto_load_balance);
    }

    // ===== SwarmTask =====

    #[test]
    fn task_creation() {
        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "test".into(),
                params: HashMap::new(),
            },
            "a test task",
        );
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.priority, TaskPriority::Normal);
        assert!(task.dependencies.is_empty());
        assert_eq!(task.metadata.get("description").unwrap(), "a test task");
    }

    #[test]
    fn task_builder_chain() {
        let dep_id = Uuid::new_v4();
        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "d",
        )
        .with_priority(TaskPriority::Critical)
        .with_dependency(dep_id);

        assert_eq!(task.priority, TaskPriority::Critical);
        assert_eq!(task.dependencies, vec![dep_id]);
    }

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    // ===== Proposal / Voting =====

    #[test]
    fn proposal_approval_ratio_empty() {
        let p = Proposal::new(ProposalType::Custom("test".into()), Uuid::new_v4(), "desc");
        assert!((p.approval_ratio()).abs() < 1e-9);
        assert!(!p.has_consensus(0.5));
    }

    #[test]
    fn proposal_approval_ratio_all_approve() {
        let mut p = Proposal::new(ProposalType::Custom("test".into()), Uuid::new_v4(), "desc");
        p.add_vote(Vote::new(Uuid::new_v4(), VoteDecision::Approve));
        p.add_vote(Vote::new(Uuid::new_v4(), VoteDecision::Approve));
        assert!((p.approval_ratio() - 1.0).abs() < 1e-9);
        assert!(p.has_consensus(0.66));
    }

    #[test]
    fn proposal_mixed_votes() {
        let mut p = Proposal::new(ProposalType::Custom("test".into()), Uuid::new_v4(), "desc");
        p.add_vote(Vote::new(Uuid::new_v4(), VoteDecision::Approve));
        p.add_vote(Vote::new(Uuid::new_v4(), VoteDecision::Reject));
        p.add_vote(Vote::new(Uuid::new_v4(), VoteDecision::Abstain));
        // 1 approve out of 3 = 0.333...
        assert!((p.approval_ratio() - 1.0 / 3.0).abs() < 1e-9);
        assert!(!p.has_consensus(0.5));
    }

    #[test]
    fn vote_with_confidence_clamped() {
        let v = Vote::new(Uuid::new_v4(), VoteDecision::Approve).with_confidence(2.5);
        assert!((v.confidence - 1.0).abs() < 1e-9);

        let v2 = Vote::new(Uuid::new_v4(), VoteDecision::Reject).with_confidence(-1.0);
        assert!(v2.confidence.abs() < 1e-9);
    }

    #[test]
    fn vote_with_justification() {
        let v = Vote::new(Uuid::new_v4(), VoteDecision::Approve).with_justification("looks good");
        assert_eq!(v.justification.as_deref(), Some("looks good"));
    }

    // ===== CollaborativeWorkflow =====

    #[test]
    fn workflow_creation_and_stages() {
        let wf = CollaborativeWorkflow::new("my_flow")
            .add_stage(WorkflowStage::new("stage_a", StageType::Parallel))
            .add_stage(WorkflowStage::new("stage_b", StageType::Consensus));

        assert_eq!(wf.name, "my_flow");
        assert_eq!(wf.stages.len(), 2);
        assert_eq!(wf.current_stage, 0);
        assert_eq!(wf.status, WorkflowStatus::Created);
        assert_eq!(wf.current().unwrap().name, "stage_a");
    }

    #[test]
    fn workflow_advance() {
        let mut wf = CollaborativeWorkflow::new("flow")
            .add_stage(WorkflowStage::new("a", StageType::Parallel))
            .add_stage(WorkflowStage::new("b", StageType::Iterative));

        assert!(wf.advance()); // a → b
        assert_eq!(wf.current_stage, 1);
        assert_eq!(wf.current().unwrap().name, "b");

        assert!(!wf.advance()); // at last stage → sets Completed
        assert_eq!(wf.status, WorkflowStatus::Completed);
    }

    #[test]
    fn workflow_stage_builder() {
        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "t",
        );
        let stage = WorkflowStage::new("s", StageType::MapReduce)
            .add_task(task)
            .with_completion(CompletionCriteria::FirstComplete)
            .with_timeout(Duration::from_secs(60));

        assert_eq!(stage.tasks.len(), 1);
        assert_eq!(stage.timeout, Duration::from_secs(60));
        assert_eq!(stage.stage_type, StageType::MapReduce);
    }

    // ===== ModelDevelopmentPipeline =====

    #[test]
    fn pipeline_default_workflow_stages() {
        let pipeline = ModelDevelopmentPipeline::new("classification");
        assert_eq!(pipeline.problem_type, "classification");
        assert_eq!(pipeline.workflow.stages.len(), 5);
        assert_eq!(pipeline.workflow.stages[0].name, "exploration");
        assert_eq!(pipeline.workflow.stages[1].name, "architecture_selection");
        assert_eq!(pipeline.workflow.stages[2].name, "training");
        assert_eq!(pipeline.workflow.stages[3].name, "optimization");
        assert_eq!(pipeline.workflow.stages[4].name, "evaluation");
    }

    #[test]
    fn pipeline_with_constraints() {
        let constraints = ModelConstraints {
            max_parameters: Some(1_000_000),
            max_memory_bytes: Some(512 * 1024 * 1024),
            max_latency_ms: Some(10.0),
            target_accuracy: Some(0.95),
            required_capabilities: vec!["gpu".into()],
        };
        let pipeline =
            ModelDevelopmentPipeline::new("detection").with_constraints(constraints.clone());
        assert_eq!(pipeline.constraints.max_parameters, Some(1_000_000));
        assert_eq!(pipeline.constraints.target_accuracy, Some(0.95));
    }

    // ===== AutonomousModelBuilder =====

    #[test]
    fn autonomous_builder() {
        let (pipeline, config) = AutonomousModelBuilder::new("segmentation")
            .collaboration_strategy(CollaborationStrategy::MapReduce)
            .target_metric("iou", 0.85)
            .max_iterations(50)
            .build();

        assert_eq!(pipeline.problem_type, "segmentation");
        assert_eq!(
            config.collaboration_strategy,
            CollaborationStrategy::MapReduce
        );
    }

    // ===== SwarmCoordinator =====

    #[test]
    fn coordinator_register_agent() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let id = coord
            .register_agent("agent1", vec!["training".into()])
            .unwrap();
        assert_eq!(coord.agent_count(), 1);

        let agent = coord.agents().next().unwrap();
        assert_eq!(agent.id, id);
        assert_eq!(agent.name, "agent1");
        assert_eq!(agent.state, SwarmAgentState::Idle);
    }

    #[test]
    fn coordinator_max_agents_exceeded() {
        let config = SwarmConfig {
            max_agents: 2,
            ..Default::default()
        };
        let mut coord = SwarmCoordinator::new(config);
        coord.register_agent("a", vec![]).unwrap();
        coord.register_agent("b", vec![]).unwrap();
        let err = coord.register_agent("c", vec![]).unwrap_err();
        assert!(format!("{}", err).contains("Maximum agents"));
    }

    #[test]
    fn coordinator_unregister_agent() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let id = coord.register_agent("agent1", vec![]).unwrap();
        coord.unregister_agent(id).unwrap();
        assert_eq!(coord.agent_count(), 0);
    }

    #[test]
    fn coordinator_unregister_unknown() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let err = coord.unregister_agent(Uuid::new_v4()).unwrap_err();
        assert!(format!("{}", err).contains("not found"));
    }

    #[test]
    fn coordinator_submit_and_query_task() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "desc",
        );
        let id = coord.submit_task(task);
        assert_eq!(coord.pending_tasks(), 1);
        assert_eq!(coord.task_status(id), Some(TaskStatus::Pending));
    }

    #[test]
    fn coordinator_schedule_assigns_tasks() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let agent_id = coord.register_agent("a", vec!["all".into()]).unwrap();

        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "desc",
        );
        let task_id = coord.submit_task(task);

        coord.schedule();
        assert_eq!(coord.pending_tasks(), 0);
        assert_eq!(coord.active_tasks(), 1);
        assert_eq!(coord.task_status(task_id), Some(TaskStatus::InProgress));

        // Agent should be Working
        let agent = coord.agents().find(|a| a.id == agent_id).unwrap();
        assert_eq!(agent.state, SwarmAgentState::Working);
    }

    #[test]
    fn coordinator_schedule_respects_capability() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord
            .register_agent("trainer", vec!["training".into()])
            .unwrap();

        // Submit a design task — agent only has "training" capability
        let task = SwarmTask::new(
            SwarmTaskType::DesignArchitecture {
                input_shape: vec![3, 224, 224],
                output_shape: vec![1000],
                constraints: ModelConstraints::default(),
            },
            "design a model",
        );
        coord.submit_task(task);

        coord.schedule();
        // Task should remain pending — no agent has "architecture_design"
        assert_eq!(coord.pending_tasks(), 1);
        assert_eq!(coord.active_tasks(), 0);
    }

    #[test]
    fn coordinator_complete_task_success() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let agent_id = coord.register_agent("a", vec!["all".into()]).unwrap();

        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "desc",
        );
        let task_id = coord.submit_task(task);
        coord.schedule();

        coord.complete_task(task_id, true).unwrap();
        assert_eq!(coord.task_status(task_id), Some(TaskStatus::Completed));
        assert_eq!(coord.active_tasks(), 0);

        let agent = coord.agents().find(|a| a.id == agent_id).unwrap();
        assert_eq!(agent.state, SwarmAgentState::Idle);
        assert_eq!(agent.metrics.tasks_completed, 1);
        assert_eq!(agent.metrics.tasks_failed, 0);
    }

    #[test]
    fn coordinator_complete_task_failure() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_agent("a", vec!["all".into()]).unwrap();

        let task = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "desc",
        );
        let task_id = coord.submit_task(task);
        coord.schedule();

        coord.complete_task(task_id, false).unwrap();
        assert_eq!(coord.task_status(task_id), Some(TaskStatus::Failed));
    }

    #[test]
    fn coordinator_complete_unknown_task() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let err = coord.complete_task(Uuid::new_v4(), true).unwrap_err();
        assert!(format!("{}", err).contains("not found"));
    }

    #[test]
    fn coordinator_proposal_workflow() {
        let mut coord = SwarmCoordinator::new(SwarmConfig {
            consensus_threshold: 0.5,
            ..Default::default()
        });
        let proposer = Uuid::new_v4();
        let proposal = Proposal::new(
            ProposalType::Custom("deploy model".into()),
            proposer,
            "Deploy v2",
        );
        let id = coord.create_proposal(proposal);

        // Status should be Voting
        assert_eq!(
            coord.proposal_status(id).unwrap().status,
            ProposalStatus::Voting
        );

        // Single approve vote should flip to Approved (1/1 = 1.0 >= 0.5)
        coord
            .submit_vote(id, Vote::new(Uuid::new_v4(), VoteDecision::Approve))
            .unwrap();
        assert_eq!(
            coord.proposal_status(id).unwrap().status,
            ProposalStatus::Approved
        );
    }

    #[test]
    fn coordinator_vote_unknown_proposal() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let err = coord
            .submit_vote(
                Uuid::new_v4(),
                Vote::new(Uuid::new_v4(), VoteDecision::Approve),
            )
            .unwrap_err();
        assert!(format!("{}", err).contains("not found"));
    }

    #[test]
    fn coordinator_start_workflow() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        let wf = CollaborativeWorkflow::new("test_flow")
            .add_stage(WorkflowStage::new("s1", StageType::Parallel));
        let id = coord.start_workflow(wf);
        let status = coord.workflow_status(id).unwrap();
        assert_eq!(status.status, WorkflowStatus::Running);
    }

    #[test]
    fn coordinator_lifecycle() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        assert!(!coord.is_running());
        coord.start();
        assert!(coord.is_running());
        coord.pause();
        assert!(!coord.is_running());
        coord.start();
        assert!(coord.is_running());
        coord.stop();
        assert!(!coord.is_running());
    }

    #[test]
    fn coordinator_schedule_priority() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_agent("a", vec!["all".into()]).unwrap();

        // Submit low-priority first, then critical
        let low = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "low",
        )
        .with_priority(TaskPriority::Low);
        let critical = SwarmTask::new(
            SwarmTaskType::Custom {
                name: "x".into(),
                params: HashMap::new(),
            },
            "critical",
        )
        .with_priority(TaskPriority::Critical);
        let low_id = coord.submit_task(low);
        let crit_id = coord.submit_task(critical);

        coord.schedule();
        // Only one agent → should pick highest priority task (Critical)
        assert_eq!(coord.task_status(crit_id), Some(TaskStatus::InProgress));
        assert_eq!(coord.task_status(low_id), Some(TaskStatus::Pending));
    }

    // ===== TrainingConfig =====

    #[test]
    fn training_config_defaults() {
        let tc = TrainingConfig::default();
        assert_eq!(tc.epochs, 100);
        assert_eq!(tc.batch_size, 32);
        assert!((tc.learning_rate - 0.001).abs() < 1e-9);
        assert_eq!(tc.optimizer, "adam");
    }
}
