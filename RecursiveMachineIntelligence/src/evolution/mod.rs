//! Self-Improvement & Evolutionary Infrastructure
//!
//! Meta-learning, self-modification, and evolutionary algorithms:
//!
//! - **Meta-Learning**: Learning-to-learn, architecture search, hyperparameter optimization
//! - **Self-Modification**: Safe code patches, sandboxed execution, rollback
//! - **Population**: Evolutionary algorithms, genetic operators, selection strategies

pub mod meta_learning;
pub mod population;
pub mod self_modification;

pub use meta_learning::{
    ArchitectureCandidate, ArchitectureSearchAgent, HyperparamRange, HyperparameterOptimizer,
    LayerSpec, LearnerProfile, LearningCurve, OptStrategy, SearchSpace, TaskFamily,
    TaskPerformance, Trial,
};
pub use population::{
    crossover_binary, crossover_real, mutate_binary, mutate_real, select, EvolutionConfig,
    EvolutionEngine, GenerationStats, Genome, Individual, SelectionStrategy, TreeNode,
};
pub use self_modification::{
    CodePatch, PatchKind, PatchPayload, PatchStatus, RollbackManager, SafetyGuard, SafetyVerdict,
    Sandbox, SandboxLimits, SandboxResult, TestSummary,
};
