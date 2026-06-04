//! Symbolic AI Module
//!
//! Provides symbolic reasoning primitives for AI agents to perform
//! logical inference, planning, and knowledge manipulation.

pub mod logic;
pub mod unification;
pub mod inference;
pub mod planner;

pub use logic::{Term, Formula, Predicate, Clause, KnowledgeBase};
pub use unification::{Substitution, unify, occurs_check};
pub use inference::{InferenceRule, InferenceEngine, Proof};
pub use planner::{Action, State, Goal, Planner, Plan};
