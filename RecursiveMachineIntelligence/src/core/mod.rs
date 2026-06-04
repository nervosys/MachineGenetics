//! Core module containing fundamental framework components.
//!
//! This module provides the foundational abstractions that all other
//! modules build upon:
//!
//! - **Primitives**: Atomic computational operations with algebraic properties
//! - **Ontology**: Machine-readable knowledge representation
//! - **Agent**: Autonomous computational entities
//! - **Protocol**: Binary communication between agents
//! - **Storage**: Efficient data persistence for agents
//! - **Message Bus**: High-performance inter-agent communication
//! - **CodeGen**: Low-level program synthesis and IR for machine code generation
//! - **Emitters**: Multi-target code emitters (CUDA, MLIR, ONNX)
//! - **Optimization**: IR optimization passes (DCE, constant folding, CSE, fusion)
//! - **Verification**: Static analysis and verification of generated IR
//! - **Introspection**: Self-describing framework ontology for agent observability

pub mod agent;
pub mod codegen;
/// Multi-agent collaboration runtime (AgentRuntime, SharedWorkspace, etc.)
pub mod collaboration;
/// Framework discoverability (catalogs, recipes, capability descriptors)
pub mod discoverability;
pub mod emitters;
pub mod introspection;
/// Token-compact progressive-disclosure manifest (cheap root index + `describe`)
pub mod manifest;
pub mod message_bus;
pub mod ontology;
pub mod optimization;
pub mod primitives;
pub mod protocol;
pub mod storage;
pub mod swarm;
pub mod verification;

pub use agent::{Agent, AgentBuilder, AgentCapability, AgentState};
pub use codegen::{
    ActivationKind, BinaryOpKind, CodeEmitter, Crossover, Dimension, EmitTarget, Function,
    FunctionBuilder, IRNode, IROperation, IRType, IRValue, MutationType, Mutator, NormalizeKind,
    Padding, PrimitiveType as IRPrimitiveType, Program, ProgramBuilder, ProgramMetadata,
    ReduceOpKind, RustEmitter, UnaryOpKind,
};
pub use collaboration::{
    AgentPipeline, AgentRuntime, DelegatedTask, ModelEntry, ModelRegistry, PipelineStage,
    RuntimeConfig, RuntimeStats, SharedWorkspace, TaskDelegator, WorkspaceEntry, WorkspaceEvent,
};
pub use emitters::{CudaEmitter, MlirEmitter, OnnxEmitter};
pub use message_bus::{
    Communicator, DeadLetterQueue, Envelope, MessageBus, MessageHandler, Subscription, Topic,
};
pub use ontology::{Concept, ConceptId, Ontology, Relation, RelationType};
pub use optimization::{
    AlgebraicSimplification, CommonSubexpressionElimination, ConstantFolding, DeadCodeElimination,
    OperatorFusion, OptimizationLevel, OptimizationPass, OptimizationPipeline, OptimizationStats,
    RmilFusionPass, RmilOptStats, RmilOptimizer, RmilPass, StrengthReduction,
};
pub use primitives::{AlgebraicProperty, Primitive, PrimitiveRegistry, PrimitiveType};
pub use protocol::{Message, MessageType, Protocol};
pub use storage::{
    CheckpointManager, CheckpointMeta, CheckpointType, ConsistentHashRing, DistributedStorage,
    KeyValueStore, ShardInfo, StorageDataType, StorageMetadata, TensorStorage,
};
pub use verification::{
    Diagnostic, ResourceChecker, Severity, ShapeInference, TerminationAnalyzer, TypeChecker,
    VerificationPass, VerificationReport, Verifier,
};
