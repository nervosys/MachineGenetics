//! Neurosymbolic Integration Module
//!
//! Bridges neural and symbolic AI paradigms, enabling AI agents to
//! leverage the strengths of both approaches.

pub mod embedding;
pub mod constraint;
pub mod hybrid;

pub use embedding::{SymbolEmbedder, EmbeddingConfig, embed_term, embed_predicate};
pub use constraint::{DifferentiableConstraint, ConstraintSolver, SoftConstraint};
pub use hybrid::{HybridReasoner, NeuralSymbolicBridge, ReasoningMode};
