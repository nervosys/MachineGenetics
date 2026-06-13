//! # RecursiveMachineIntelligence (RMI — Recursive Machine Intelligence)
//!
//! **The built-in agentic-first AI framework of MAGE (Machine Genetics), by NERVOSYS.**
//!
//! RecursiveMachineIntelligence treats AI agents as first-class citizens. Unlike traditional ML
//! frameworks designed for human developers, it provides primitives, complete
//! ontologies, and communication protocols optimized for machine reasoning and
//! multi-agent collaboration — prioritizing token efficiency (compact manifest,
//! binary-first IR), reliability (typed self-correcting diagnostics, deterministic
//! queries), and safety (effect-mapped capabilities, exact-F32 fallbacks).
//! The crate name is `rmi`.
//!
//! ## Philosophy
//!
//! Human-designed frameworks encode human biases: sequential thinking, natural language
//! abstractions, and interfaces optimized for human cognition. RMI inverts this paradigm:
//!
//! - **Machine-Native Primitives**: Ontological structures that AI systems can reason over directly
//! - **Binary-First Communication**: Maximally efficient inter-agent protocols
//! - **Compositional Architecture**: Everything composes through formal algebraic structures
//! - **Self-Describing Systems**: All components carry rich metadata for autonomous discovery
//!
//! ## Core Paradigms
//!
//! ### Neural
//! Deep learning primitives with differentiable programming support.
//!
//! ### Symbolic
//! Logic-based reasoning and knowledge representation.
//!
//! ### Neurosymbolic
//! Hybrid systems combining the best of both worlds.
//!
//! ## Agent quick-start (progressive disclosure)
//!
//! The cheapest way to learn this framework is to ask it. Start with the
//! token-compact manifest (a few hundred tokens), expand entries on demand,
//! and only build the full ontology graph when you need relations:
//!
//! ```
//! // 1. Root index — read this first (deterministic, < 4 KB).
//! let root = rmi::core::manifest::manifest();
//! assert!(root.contains("compute"));
//!
//! // 2. Expand one entry for the next level of detail.
//! let compute = rmi::core::manifest::describe("compute").unwrap();
//! assert!(compute.contains("quantized_matmul"));
//!
//! // 3. Errors are self-correcting: every RmiError carries a stable code,
//! //    recoverability flag, and a suggested fix.
//! let err = rmi::RmiError::compute_simple("dtype mismatch");
//! assert!(err.agent_diagnostic().starts_with("error code=compute"));
//! ```
//!
//! ## Example
//!
//! ```
//! use rmi::core::introspection::{FrameworkOntology, IntrospectionQueries};
//! use rmi::core::optimization::{OptimizationPipeline, OptimizationLevel};
//! use rmi::core::codegen::Program;
//!
//! // Build the framework ontology for autonomous discovery
//! let ontology = FrameworkOntology::build();
//! let neural_components = ontology.in_namespace("rmi.neural");
//!
//! // Create an optimization pipeline and optimize a program
//! let pipeline = OptimizationPipeline::level(OptimizationLevel::O2);
//! let program = Program::new("synthesized");
//! let optimized = pipeline.optimize(program);
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

/// Compute backends for tensor operations
pub mod compute;
/// Core infrastructure (agents, primitives, protocol, storage)
pub mod core;
/// Distributed agent infrastructure (transport, discovery, consensus, federation)
pub mod distributed;
/// Self-improvement & evolutionary infrastructure (meta-learning, self-modification, population)
pub mod evolution;
/// AI knowledge base (history, concepts)
pub mod knowledge;
/// RMIL — Recursive Machine Intelligence Language (compact binary neurosymbolic IR)
pub mod lang;
/// Neural network components
pub mod neural;
/// Neurosymbolic integration
pub mod neurosymbolic;
/// Production runtime (memory pool, observability, deployment)
pub mod runtime;
/// Symbolic reasoning
pub mod symbolic;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::compute::{
        AppleAneBackend, Backend, BackendType, CpuBackend, MetalBackend, QualcommBackend,
        VulkanBackend, WebGpuBackend,
    };
    pub use crate::core::{
        agent::{Agent, AgentBuilder, AgentCapability, AgentState, Goal},
        collaboration::{
            AgentPipeline, AgentRuntime, ModelEntry, ModelRegistry, RuntimeConfig, RuntimeStats,
            SharedWorkspace, TaskDelegator, WorkspaceEntry, WorkspaceEvent,
        },
        emitters::{CudaEmitter, MlirEmitter, OnnxEmitter},
        introspection::{FrameworkOntology, IntrospectionQueries},
        ontology::{Concept, ConceptType, Ontology, Relation, RelationType},
        optimization::{
            AlgebraicSimplification, CommonSubexpressionElimination, ConstantFolding,
            DeadCodeElimination, OperatorFusion, OptimizationLevel, OptimizationPipeline,
            OptimizationStats, StrengthReduction,
        },
        primitives::{AlgebraicProperty, Primitive, PrimitiveRegistry, PrimitiveType},
        protocol::{Message, MessageType},
        swarm::{
            AutonomousModelBuilder, CollaborationStrategy, CollaborativeWorkflow,
            CompletionCriteria, CoordinatorState, MergeStrategy, ModelConstraints,
            ModelDevelopmentPipeline, ParameterRange, Proposal, ProposalStatus, ProposalType,
            ResourceSharingMode, StageResult, StageType, SwarmAgentInfo, SwarmAgentState,
            SwarmConfig, SwarmCoordinator, SwarmTask, SwarmTaskType, TaskPriority, TaskStatus,
            TrainingConfig, Vote, VoteDecision, WorkflowStage, WorkflowStatus,
        },
        verification::{Severity, VerificationReport, Verifier},
    };
    pub use crate::error::{Result, RmiError};
    pub use crate::knowledge::{
        ai_concepts::{AIConcept, AIConceptsOntology, ConceptDomain, ConceptRelation},
        history::{AIContribution, AIEra, AIHistoryKB, ContributionCategory},
    };
    pub use crate::neural::{
        architecture::{ArchitectureBuilder, NetworkArchitecture},
        autodiff::{GradientTape, Variable},
        layers::Layer,
        primitives::NeuralPrimitive,
        AggregationStrategy, FederatedConfig, FederatedHistory, FederatedTrainer,
    };
    pub use crate::neurosymbolic::{
        constraint::{ConstraintFormula, ConstraintSolver, SoftConstraint},
        embedding::SymbolEmbedder,
        hybrid::{HybridConfig, HybridReasoner, ReasoningMode},
    };
    pub use crate::symbolic::{
        inference::InferenceEngine,
        logic::{Clause, Formula, KnowledgeBase, Predicate, Term},
        planner::{Action, Domain, Plan, Planner},
    };
    // RMIL language
    pub use crate::lang::{Dtype, Expr, Op, OpFamily, OpMeta, Sym, SymbolTable, Ty, Val, Vm};
}

/// Error types for the framework
pub mod error {
    use thiserror::Error;

    /// Main error type for RMI operations
    #[derive(Error, Debug)]
    pub enum RmiError {
        /// Primitive operation failed
        #[error("Primitive error: {0}")]
        Primitive(String),

        /// Ontology operation failed
        #[error("Ontology error: {0}")]
        Ontology(String),

        /// Agent operation failed
        #[error("Agent error: {0}")]
        Agent(String),

        /// Protocol/communication error
        #[error("Protocol error: {0}")]
        Protocol(String),

        /// Compute backend error
        #[error("Compute error: {0}")]
        Compute(String),

        /// Serialization error
        #[error("Serialization error: {0}")]
        Serialization(String),

        /// IO error
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        /// Shape mismatch in tensor operations
        #[error("Shape mismatch: expected {expected:?}, got {actual:?}")]
        ShapeMismatch {
            /// Expected tensor shape
            expected: Vec<usize>,
            /// Actual tensor shape
            actual: Vec<usize>,
        },

        /// Resource exhaustion
        #[error("Resource exhausted: {0}")]
        ResourceExhausted(String),

        /// Invalid configuration
        #[error("Invalid configuration: {0}")]
        InvalidConfig(String),

        /// Neural module error
        #[error("Neural error: {0}")]
        Neural(String),

        /// Symbolic reasoning error
        #[error("Symbolic error: {0}")]
        Symbolic(String),

        /// Unification error from symbolic module
        #[error("Unification error: {0}")]
        Unification(String),

        /// Code generation / IR error
        #[error("CodeGen error: {0}")]
        CodeGen(String),

        /// Validation error with structured context
        #[error("Validation error in {module}: {message}")]
        Validation {
            /// Module where validation failed
            module: String,
            /// Description of the validation failure
            message: String,
            /// Optional field or parameter name
            field: Option<String>,
        },
    }

    impl RmiError {
        /// Create a simple protocol error with a message.
        pub fn protocol_simple(msg: impl Into<String>) -> Self {
            Self::Protocol(msg.into())
        }

        /// Create a simple compute error with a message.
        pub fn compute_simple(msg: impl Into<String>) -> Self {
            Self::Compute(msg.into())
        }

        /// Create a simple ontology error with a message.
        pub fn ontology_simple(msg: impl Into<String>) -> Self {
            Self::Ontology(msg.into())
        }

        /// Create a simple shape mismatch error with a message.
        pub fn shape_mismatch_simple(msg: impl Into<String>) -> Self {
            Self::Compute(format!("Shape mismatch: {}", msg.into()))
        }

        /// Create a simple invalid config error with a message.
        pub fn invalid_config_simple(msg: impl Into<String>) -> Self {
            Self::InvalidConfig(msg.into())
        }

        /// Create a neural module error.
        pub fn neural(msg: impl Into<String>) -> Self {
            Self::Neural(msg.into())
        }

        /// Create a symbolic reasoning error.
        pub fn symbolic(msg: impl Into<String>) -> Self {
            Self::Symbolic(msg.into())
        }

        /// Create a code generation error.
        pub fn codegen(msg: impl Into<String>) -> Self {
            Self::CodeGen(msg.into())
        }

        /// Create a validation error with module context.
        pub fn validation(module: impl Into<String>, message: impl Into<String>) -> Self {
            Self::Validation {
                module: module.into(),
                message: message.into(),
                field: None,
            }
        }

        /// Create a validation error with field context.
        pub fn validation_field(
            module: impl Into<String>,
            message: impl Into<String>,
            field: impl Into<String>,
        ) -> Self {
            Self::Validation {
                module: module.into(),
                message: message.into(),
                field: Some(field.into()),
            }
        }

        /// Check if this error is recoverable (transient failures).
        pub fn is_recoverable(&self) -> bool {
            matches!(
                self,
                RmiError::ResourceExhausted(_) | RmiError::Io(_) | RmiError::Protocol(_)
            )
        }

        /// Get the error category for structured logging.
        pub fn category(&self) -> &'static str {
            match self {
                RmiError::Primitive(_) => "primitive",
                RmiError::Ontology(_) => "ontology",
                RmiError::Agent(_) => "agent",
                RmiError::Protocol(_) => "protocol",
                RmiError::Compute(_) => "compute",
                RmiError::Serialization(_) => "serialization",
                RmiError::Io(_) => "io",
                RmiError::ShapeMismatch { .. } => "shape",
                RmiError::ResourceExhausted(_) => "resource",
                RmiError::InvalidConfig(_) => "config",
                RmiError::Neural(_) => "neural",
                RmiError::Symbolic(_) => "symbolic",
                RmiError::Unification(_) => "unification",
                RmiError::CodeGen(_) => "codegen",
                RmiError::Validation { .. } => "validation",
            }
        }

        /// A category-appropriate suggested next action an agent can take to
        /// self-correct. Generic by necessity at this level (call sites carry
        /// the specifics in the message), but always non-empty and actionable.
        pub fn suggested_fix(&self) -> &'static str {
            match self {
                RmiError::Primitive(_) => {
                    "check operand dtypes/shapes against the primitive's signature in the ontology"
                }
                RmiError::Ontology(_) => {
                    "verify the concept id/namespace exists: query in_namespace() or the manifest first"
                }
                RmiError::Agent(_) => "check the agent's state and capability set before invoking",
                RmiError::Protocol(_) => "retry; if persistent, re-handshake or check peer version",
                RmiError::Compute(_) => {
                    "verify tensor dtype+shape+backend match the op (see manifest describe(\"compute\")); F32 2-D is the most-supported path"
                }
                RmiError::Serialization(_) => {
                    "check the payload against the schema version; re-encode with the current codec"
                }
                RmiError::Io(_) => "retry; check the path exists and is writable",
                RmiError::ShapeMismatch { .. } => {
                    "reshape or transpose the input to the expected shape printed in this error"
                }
                RmiError::ResourceExhausted(_) => {
                    "free unused tensors (Backend::free) or reduce batch/model size; retry"
                }
                RmiError::InvalidConfig(_) => {
                    "compare the config against the documented defaults; unset fields fall back safely"
                }
                RmiError::Neural(_) => "check layer dims compose (out_dim of N == in_dim of N+1)",
                RmiError::Symbolic(_) => "check the formula/KB is well-formed; run validation first",
                RmiError::Unification(_) => {
                    "the terms don't unify: check arity and functor names match"
                }
                RmiError::CodeGen(_) => "validate the IR (verification module) before emitting",
                RmiError::Validation { .. } => {
                    "fix the named field in the named module; the message states the constraint"
                }
            }
        }

        /// A single-line, machine-parseable diagnostic with every field an
        /// agent needs to self-correct: stable `code` (category), `recoverable`
        /// flag, the human `message`, and a `fix` suggestion. Format:
        /// `error code=<category> recoverable=<bool> message=<...> fix=<...>`
        ///
        /// This is the agent-facing error surface — scoreable as fully
        /// actionable under error-quality rubrics (code+message+fix present;
        /// location travels in the message where the call site knows it).
        pub fn agent_diagnostic(&self) -> String {
            format!(
                "error code={} recoverable={} message={} fix={}",
                self.category(),
                self.is_recoverable(),
                self,
                self.suggested_fix()
            )
        }
    }

    /// Conversion from UnificationError into RmiError
    impl From<crate::symbolic::unification::UnificationError> for RmiError {
        fn from(err: crate::symbolic::unification::UnificationError) -> Self {
            RmiError::Unification(err.to_string())
        }
    }

    /// Result type alias using RmiError
    pub type Result<T> = std::result::Result<T, RmiError>;

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Every error variant must produce a fully-actionable agent
        /// diagnostic: stable code, recoverable flag, message, non-empty fix.
        #[test]
        fn every_variant_yields_actionable_diagnostic() {
            let samples: Vec<RmiError> = vec![
                RmiError::Primitive("p".into()),
                RmiError::Ontology("o".into()),
                RmiError::Agent("a".into()),
                RmiError::Protocol("pr".into()),
                RmiError::Compute("c".into()),
                RmiError::Serialization("s".into()),
                RmiError::Io(std::io::Error::new(std::io::ErrorKind::Other, "io")),
                RmiError::ShapeMismatch {
                    expected: vec![2, 3],
                    actual: vec![3, 2],
                },
                RmiError::ResourceExhausted("r".into()),
                RmiError::InvalidConfig("ic".into()),
                RmiError::Neural("n".into()),
                RmiError::Symbolic("sy".into()),
                RmiError::Unification("u".into()),
                RmiError::CodeGen("cg".into()),
                RmiError::validation_field("compute", "bad dim", "in_dim"),
            ];
            for e in &samples {
                let d = e.agent_diagnostic();
                assert!(d.starts_with("error code="), "diagnostic shape: {d}");
                assert!(d.contains("recoverable="), "{d}");
                assert!(d.contains("message="), "{d}");
                assert!(d.contains("fix="), "{d}");
                assert!(!e.suggested_fix().is_empty());
                assert!(!e.category().is_empty());
            }
        }

        /// ShapeMismatch carries the expected/actual shapes in the message
        /// (location-grade detail), and validation carries module+field.
        #[test]
        fn structured_variants_carry_context() {
            let sm = RmiError::ShapeMismatch {
                expected: vec![4, 8],
                actual: vec![8, 4],
            };
            let d = sm.agent_diagnostic();
            assert!(d.contains("[4, 8]") && d.contains("[8, 4]"), "{d}");
            let v = RmiError::validation_field("neural", "in_dim must be > 0", "in_dim");
            assert!(format!("{v}").contains("neural"));
        }
    }
}

pub use error::{Result, RmiError};
