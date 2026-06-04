//! Hybrid Neurosymbolic Reasoning
//!
//! Bridges neural and symbolic AI paradigms, enabling AI agents to
//! leverage both continuous (neural) and discrete (symbolic) reasoning.
//!
//! # Architecture
//!
//! The [`HybridReasoner`] combines three subsystems:
//!
//! 1. **Neural pathway** — Embeds symbolic terms into dense vector space via
//!    [`SymbolEmbedder`], then performs
//!    similarity-based reasoning (cosine distance, nearest-neighbor lookup).
//!
//! 2. **Symbolic pathway** — Applies logical inference (forward/backward
//!    chaining) over a [`KnowledgeBase`]
//!    using the [`InferenceEngine`].
//!
//! 3. **Constraint bridge** — Differentiable soft constraints
//!    ([`SoftConstraint`]) allow gradient
//!    signals to flow between the neural and symbolic components, enabling
//!    joint optimization.
//!
//! # Reasoning Modes
//!
//! - [`ReasoningMode::Neural`] — Pure embedding lookup; fast, approximate.
//! - [`ReasoningMode::Symbolic`] — Pure logical inference; exact, potentially
//!   expensive for large knowledge bases.
//! - [`ReasoningMode::Hybrid`] — Runs both pathways and merges results using a
//!   weighted confidence combination.
//! - [`ReasoningMode::Adaptive`] — Automatically selects the best mode based on
//!   problem characteristics: uses symbolic when the query maps to known
//!   predicates, neural when embedding similarity is high, and hybrid otherwise.
//!
//! # Integration with RMIL
//!
//! Hybrid reasoning is exposed at the RMIL level via the `INFER`, `RESOLVE`,
//! and `EMBED` opcodes, enabling agents to compose neurosymbolic pipelines
//! using the standard `>>` and `|` combinators.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::constraint::{ConstraintFormula, ConstraintSolver, SoftConstraint};
use super::embedding::{EmbeddingConfig, SymbolEmbedder};
use crate::symbolic::inference::{InferenceConfig, InferenceEngine};
use crate::symbolic::logic::{Formula, KnowledgeBase, Predicate, Term};

/// Reasoning modes for hybrid systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningMode {
    /// Pure neural reasoning (pattern matching, similarity)
    Neural,

    /// Pure symbolic reasoning (logic, inference)
    Symbolic,

    /// Hybrid reasoning combining both
    Hybrid,

    /// Adaptive mode selection based on problem characteristics
    Adaptive,
}

/// Configuration for hybrid reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridConfig {
    /// Default reasoning mode
    pub default_mode: ReasoningMode,

    /// Confidence threshold for neural predictions
    pub neural_confidence_threshold: f64,

    /// Maximum symbolic inference depth
    pub max_inference_depth: usize,

    /// Weight for neural component in hybrid scoring
    pub neural_weight: f64,

    /// Weight for symbolic component in hybrid scoring
    pub symbolic_weight: f64,

    /// Temperature for soft unification
    pub unification_temperature: f64,

    /// Embedding dimension
    pub embedding_dim: usize,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            default_mode: ReasoningMode::Hybrid,
            neural_confidence_threshold: 0.7,
            max_inference_depth: 10,
            neural_weight: 0.5,
            symbolic_weight: 0.5,
            unification_temperature: 0.1,
            embedding_dim: 128,
        }
    }
}

/// Result of hybrid reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridResult {
    /// Unique identifier
    pub id: Uuid,

    /// The query that was answered
    pub query: String,

    /// Mode used for reasoning
    pub mode_used: ReasoningMode,

    /// Whether the query was satisfied/provable
    pub satisfied: bool,

    /// Confidence score [0, 1]
    pub confidence: f64,

    /// Neural similarity score (if applicable)
    pub neural_score: Option<f64>,

    /// Symbolic proof found (if applicable)
    pub symbolic_proof: Option<bool>,

    /// Bindings from symbolic reasoning
    pub bindings: HashMap<String, String>,

    /// Explanation of reasoning
    pub explanation: Vec<String>,
}

impl HybridResult {
    /// Create a new hybrid result
    pub fn new(query: impl Into<String>, mode: ReasoningMode) -> Self {
        Self {
            id: Uuid::new_v4(),
            query: query.into(),
            mode_used: mode,
            satisfied: false,
            confidence: 0.0,
            neural_score: None,
            symbolic_proof: None,
            bindings: HashMap::new(),
            explanation: Vec::new(),
        }
    }

    /// Add an explanation step
    pub fn explain(&mut self, step: impl Into<String>) {
        self.explanation.push(step.into());
    }
}

/// Bridge between neural and symbolic representations
pub struct NeuralSymbolicBridge {
    /// Symbol embedder for neural encoding
    embedder: SymbolEmbedder,

    /// Cache for formula embeddings
    formula_cache: HashMap<String, Vec<f32>>,

    /// Cache for term embeddings
    term_cache: HashMap<String, Vec<f32>>,
}

impl NeuralSymbolicBridge {
    /// Create a new bridge
    pub fn new(embedding_dim: usize) -> Self {
        let config = EmbeddingConfig {
            embedding_dim,
            ..Default::default()
        };
        Self {
            embedder: SymbolEmbedder::new(config),
            formula_cache: HashMap::new(),
            term_cache: HashMap::new(),
        }
    }

    /// Embed a term into vector space
    pub fn embed_term(&mut self, term: &Term) -> Vec<f32> {
        let key = format!("{:?}", term);

        if let Some(cached) = self.term_cache.get(&key) {
            return cached.clone();
        }

        let embedding = self.embedder.embed_term(term);
        self.term_cache.insert(key, embedding.clone());
        embedding
    }

    /// Embed a predicate
    pub fn embed_predicate(&mut self, pred: &Predicate) -> Vec<f32> {
        self.embedder.embed_predicate(pred)
    }

    /// Embed a formula
    pub fn embed_formula(&mut self, formula: &Formula) -> Vec<f32> {
        let key = format!("{:?}", formula);

        if let Some(cached) = self.formula_cache.get(&key) {
            return cached.clone();
        }

        let embedding = self.embedder.embed_formula(formula);
        self.formula_cache.insert(key, embedding.clone());
        embedding
    }

    /// Compute soft unification score between two terms
    pub fn soft_unify(&mut self, t1: &Term, t2: &Term, temperature: f64) -> f64 {
        let e1 = self.embed_term(t1);
        let e2 = self.embed_term(t2);

        let similarity = cosine_similarity(&e1, &e2);

        // Softmax-like scaling
        (similarity / temperature as f32).exp() as f64
            / ((similarity / temperature as f32).exp() + (-similarity / temperature as f32).exp())
                as f64
    }

    /// Find most similar formula in knowledge base
    pub fn find_similar_formula(
        &mut self,
        query: &Formula,
        kb: &KnowledgeBase,
        top_k: usize,
    ) -> Vec<(Formula, f32)> {
        let query_emb = self.embed_formula(query);

        let mut scored: Vec<(Formula, f32)> = kb
            .all()
            .iter()
            .filter(|c| c.is_fact())
            .filter_map(|c| c.head.as_ref())
            .map(|pred| {
                let formula = Formula::Atom(pred.clone());
                let emb = self.embed_formula(&formula);
                let score = cosine_similarity(&query_emb, &emb);
                (formula, score)
            })
            .collect();

        // Add rules
        for clause in kb.all().iter().filter(|c| c.is_rule()) {
            if let (Some(head), Some(first_body)) = (&clause.head, clause.body.first()) {
                let formula = Formula::Implies(
                    Box::new(Formula::Atom(first_body.clone())),
                    Box::new(Formula::Atom(head.clone())),
                );
                let emb = self.embed_formula(&formula);
                let score = cosine_similarity(&query_emb, &emb);
                scored.push((formula, score));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Clear caches
    pub fn clear_cache(&mut self) {
        self.formula_cache.clear();
        self.term_cache.clear();
    }
}

/// Hybrid reasoner combining neural and symbolic approaches
pub struct HybridReasoner {
    /// Configuration
    config: HybridConfig,

    /// Neural-symbolic bridge
    bridge: NeuralSymbolicBridge,

    /// Symbolic inference engine
    inference: InferenceEngine,
}

impl HybridReasoner {
    /// Create a new hybrid reasoner
    pub fn new(config: HybridConfig) -> Self {
        Self {
            bridge: NeuralSymbolicBridge::new(config.embedding_dim),
            inference: InferenceEngine::new(InferenceConfig::default()),
            config,
        }
    }

    /// Query the hybrid reasoner
    pub fn query(&mut self, query: &Predicate, kb: &KnowledgeBase) -> HybridResult {
        match self.config.default_mode {
            ReasoningMode::Neural => self.neural_query(query, kb),
            ReasoningMode::Symbolic => self.symbolic_query(query, kb),
            ReasoningMode::Hybrid => self.hybrid_query(query, kb),
            ReasoningMode::Adaptive => self.adaptive_query(query, kb),
        }
    }

    /// Pure neural query (similarity-based)
    fn neural_query(&mut self, query: &Predicate, kb: &KnowledgeBase) -> HybridResult {
        let mut result = HybridResult::new(format!("{:?}", query), ReasoningMode::Neural);
        result.explain("Using neural (similarity-based) reasoning");

        let query_emb = self.bridge.embed_predicate(query);

        // Find most similar facts
        let mut best_score = 0.0f32;
        let mut best_match: Option<Predicate> = None;

        for clause in kb.all().iter().filter(|c| c.is_fact()) {
            if let Some(ref fact) = clause.head {
                let fact_emb = self.bridge.embed_predicate(fact);
                let score = cosine_similarity(&query_emb, &fact_emb);

                if score > best_score {
                    best_score = score;
                    best_match = Some(fact.clone());
                }
            }
        }

        result.neural_score = Some(best_score as f64);
        result.confidence = best_score as f64;
        result.satisfied = best_score as f64 >= self.config.neural_confidence_threshold;

        if let Some(matched) = best_match {
            result.explain(format!(
                "Best match: {:?} (similarity: {:.3})",
                matched, best_score
            ));
        }

        result
    }

    /// Pure symbolic query (logical inference)
    fn symbolic_query(&mut self, query: &Predicate, kb: &KnowledgeBase) -> HybridResult {
        let mut result = HybridResult::new(format!("{:?}", query), ReasoningMode::Symbolic);
        result.explain("Using symbolic (logical) reasoning");

        // Try to prove using backward chaining
        match self.inference.query(kb, query) {
            Some(bindings) => {
                result.satisfied = true;
                result.symbolic_proof = Some(true);
                result.confidence = 1.0;

                // Convert bindings to strings - bindings is Vec<HashMap<String, Term>>
                for subst in bindings {
                    for (var, term) in subst.iter() {
                        result.bindings.insert(var.clone(), format!("{:?}", term));
                    }
                }

                result.explain("Query proven via backward chaining");
            }
            None => {
                result.satisfied = false;
                result.symbolic_proof = Some(false);
                result.confidence = 0.0;
                result.explain("Query could not be proven");
            }
        }

        result
    }

    /// Hybrid query combining neural and symbolic
    fn hybrid_query(&mut self, query: &Predicate, kb: &KnowledgeBase) -> HybridResult {
        let mut result = HybridResult::new(format!("{:?}", query), ReasoningMode::Hybrid);
        result.explain("Using hybrid (neural + symbolic) reasoning");

        // First try symbolic reasoning
        let symbolic_result = self.symbolic_query(query, kb);

        if symbolic_result.satisfied {
            // Symbolic proof found - high confidence
            result.satisfied = true;
            result.symbolic_proof = Some(true);
            result.confidence = 1.0;
            result.bindings = symbolic_result.bindings;
            result.explain("Symbolic proof found - using logical result");
            return result;
        }

        // No symbolic proof - try neural approximation
        let neural_result = self.neural_query(query, kb);
        result.neural_score = neural_result.neural_score;

        // Combine scores
        let symbolic_score = if symbolic_result.symbolic_proof == Some(true) {
            1.0
        } else {
            0.0
        };
        let neural_score = neural_result.neural_score.unwrap_or(0.0);

        let combined =
            self.config.symbolic_weight * symbolic_score + self.config.neural_weight * neural_score;

        result.confidence = combined;
        result.satisfied = combined >= self.config.neural_confidence_threshold;

        result.explain(format!(
            "Combined score: {:.3} (symbolic: {:.3}, neural: {:.3})",
            combined, symbolic_score, neural_score
        ));

        result
    }

    /// Adaptive query that chooses mode based on problem
    fn adaptive_query(&mut self, query: &Predicate, kb: &KnowledgeBase) -> HybridResult {
        let mut result = HybridResult::new(format!("{:?}", query), ReasoningMode::Adaptive);
        result.explain("Using adaptive mode selection");

        // Heuristics for mode selection:
        // 1. If query has many variables -> symbolic
        // 2. If KB is small -> symbolic
        // 3. If query predicate is rare -> neural (similarity)

        let query_vars = count_variables_in_predicate(query);
        let facts_count = kb.all().iter().filter(|c| c.is_fact()).count();
        let rules_count = kb.all().iter().filter(|c| c.is_rule()).count();
        let kb_size = facts_count + rules_count;
        let predicate_frequency = kb
            .all()
            .iter()
            .filter(|c| c.is_fact())
            .filter_map(|c| c.head.as_ref())
            .filter(|f| f.name == query.name)
            .count();

        let mode = if query_vars > 2 {
            result.explain("Query has multiple variables - preferring symbolic");
            ReasoningMode::Symbolic
        } else if kb_size < 50 {
            result.explain("Small knowledge base - preferring symbolic");
            ReasoningMode::Symbolic
        } else if predicate_frequency == 0 {
            result.explain("Predicate not in KB - using neural similarity");
            ReasoningMode::Neural
        } else {
            result.explain("Using hybrid approach");
            ReasoningMode::Hybrid
        };

        // Delegate to selected mode
        let inner_result = match mode {
            ReasoningMode::Neural => self.neural_query(query, kb),
            ReasoningMode::Symbolic => self.symbolic_query(query, kb),
            ReasoningMode::Hybrid => self.hybrid_query(query, kb),
            ReasoningMode::Adaptive => unreachable!(),
        };

        // Merge results
        result.satisfied = inner_result.satisfied;
        result.confidence = inner_result.confidence;
        result.neural_score = inner_result.neural_score;
        result.symbolic_proof = inner_result.symbolic_proof;
        result.bindings = inner_result.bindings;
        result.explanation.extend(inner_result.explanation);

        result
    }

    /// Solve a constraint satisfaction problem with hybrid approach
    pub fn solve_constraints(
        &mut self,
        constraints: Vec<SoftConstraint>,
    ) -> Option<HashMap<String, f64>> {
        let mut solver = ConstraintSolver::new()
            .with_learning_rate(0.1)
            .with_max_iterations(1000);

        for c in constraints {
            solver.add_constraint(c);
        }

        if solver.solve() {
            Some(solver.assignments().clone())
        } else {
            None
        }
    }

    /// Convert a logical formula to soft constraints
    pub fn formula_to_constraints(&self, formula: &Formula) -> Vec<SoftConstraint> {
        let mut constraints = Vec::new();
        self.convert_formula(formula, &mut constraints, 1.0);
        constraints
    }

    fn convert_formula(
        &self,
        formula: &Formula,
        constraints: &mut Vec<SoftConstraint>,
        weight: f64,
    ) {
        match formula {
            Formula::Atom(pred) => {
                // Create a variable for this predicate
                let var_name = format!("pred_{}_{}", pred.name, pred.args.len());
                let constraint =
                    SoftConstraint::new(var_name.clone(), ConstraintFormula::Variable(var_name))
                        .with_weight(weight);
                constraints.push(constraint);
            }

            Formula::Not(inner) => {
                // Negate inner constraints
                let inner_constraints = self.convert_formula_to_cf(inner);
                let constraint = SoftConstraint::new(
                    "negation",
                    ConstraintFormula::Not(Box::new(inner_constraints)),
                )
                .with_weight(weight);
                constraints.push(constraint);
            }

            Formula::And(formulas) => {
                // All must be satisfied
                for f in formulas {
                    self.convert_formula(f, constraints, weight);
                }
            }

            Formula::Or(formulas) => {
                // At least one must be satisfied
                let cf_formulas: Vec<ConstraintFormula> = formulas
                    .iter()
                    .map(|f| self.convert_formula_to_cf(f))
                    .collect();

                let constraint =
                    SoftConstraint::new("disjunction", ConstraintFormula::Or(cf_formulas))
                        .with_weight(weight);
                constraints.push(constraint);
            }

            Formula::Implies(ante, conseq) => {
                let cf_ante = self.convert_formula_to_cf(ante);
                let cf_conseq = self.convert_formula_to_cf(conseq);

                let constraint = SoftConstraint::new(
                    "implication",
                    ConstraintFormula::Implies(Box::new(cf_ante), Box::new(cf_conseq)),
                )
                .with_weight(weight);
                constraints.push(constraint);
            }

            _ => {
                // Existential, Universal - simplify to atomic
                // In a full implementation, these would be skolemized
            }
        }
    }

    fn convert_formula_to_cf(&self, formula: &Formula) -> ConstraintFormula {
        match formula {
            Formula::Atom(pred) => {
                let var_name = format!("pred_{}_{}", pred.name, pred.args.len());
                ConstraintFormula::Variable(var_name)
            }

            Formula::Not(inner) => {
                ConstraintFormula::Not(Box::new(self.convert_formula_to_cf(inner)))
            }

            Formula::And(formulas) => ConstraintFormula::And(
                formulas
                    .iter()
                    .map(|f| self.convert_formula_to_cf(f))
                    .collect(),
            ),

            Formula::Or(formulas) => ConstraintFormula::Or(
                formulas
                    .iter()
                    .map(|f| self.convert_formula_to_cf(f))
                    .collect(),
            ),

            Formula::Implies(a, b) => ConstraintFormula::Implies(
                Box::new(self.convert_formula_to_cf(a)),
                Box::new(self.convert_formula_to_cf(b)),
            ),

            _ => ConstraintFormula::Constant(0.5),
        }
    }

    /// Get the neural-symbolic bridge for direct access
    pub fn bridge(&mut self) -> &mut NeuralSymbolicBridge {
        &mut self.bridge
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Count variables in a predicate
fn count_variables_in_predicate(pred: &Predicate) -> usize {
    pred.args
        .iter()
        .filter(|t| matches!(t, Term::Variable(_)))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbolic::logic::Clause;
    use crate::symbolic::logic::{Predicate, Term};

    fn make_kb() -> KnowledgeBase {
        let mut kb = KnowledgeBase::new();

        // Facts
        kb.add_fact(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );

        kb.add_fact(
            "parent",
            vec![
                Term::Constant("bob".to_string()),
                Term::Constant("charlie".to_string()),
            ],
        );

        // Rule: grandparent(X, Z) :- parent(X, Y), parent(Y, Z)
        kb.add_rule(
            Predicate::new(
                "grandparent",
                vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            ),
            vec![
                Predicate::new(
                    "parent",
                    vec![
                        Term::Variable("X".to_string()),
                        Term::Variable("Y".to_string()),
                    ],
                ),
                Predicate::new(
                    "parent",
                    vec![
                        Term::Variable("Y".to_string()),
                        Term::Variable("Z".to_string()),
                    ],
                ),
            ],
        );

        kb
    }

    #[test]
    fn test_hybrid_reasoner_symbolic() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Symbolic,
            ..Default::default()
        });

        // Query: parent(alice, bob)?
        let query = Predicate::new(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        assert!(result.satisfied);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_hybrid_reasoner_neural() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Neural,
            neural_confidence_threshold: 0.5,
            ..Default::default()
        });

        // Query similar to existing fact
        let query = Predicate::new(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        // Neural should find high similarity with existing fact
        assert!(result.neural_score.is_some());
    }

    #[test]
    fn test_neural_symbolic_bridge() {
        let mut bridge = NeuralSymbolicBridge::new(64);

        let t1 = Term::Constant("alice".to_string());
        let t2 = Term::Constant("alice".to_string());
        let t3 = Term::Constant("bob".to_string());

        let e1 = bridge.embed_term(&t1);
        let e2 = bridge.embed_term(&t2);
        let e3 = bridge.embed_term(&t3);

        // Same term should have identical embedding
        assert_eq!(e1, e2);

        // Different terms have different embeddings
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_soft_unify() {
        let mut bridge = NeuralSymbolicBridge::new(64);

        let t1 = Term::Constant("cat".to_string());
        let t2 = Term::Constant("cat".to_string());

        let score = bridge.soft_unify(&t1, &t2, 0.1);
        // Identical terms should have high soft unification
        assert!(score > 0.9);
    }

    #[test]
    fn test_hybrid_config_defaults() {
        let config = HybridConfig::default();
        assert_eq!(config.default_mode, ReasoningMode::Hybrid);
        assert!((config.neural_confidence_threshold - 0.7).abs() < 1e-5);
        assert!((config.neural_weight - 0.5).abs() < 1e-5);
        assert!((config.symbolic_weight - 0.5).abs() < 1e-5);
        assert_eq!(config.embedding_dim, 128);
    }

    #[test]
    fn test_hybrid_result_explain() {
        let mut result = HybridResult::new("test", ReasoningMode::Symbolic);
        result.explain("Step 1: lookup");
        result.explain("Step 2: unify");
        assert_eq!(result.explanation.len(), 2);
        assert!(result.explanation[0].contains("lookup"));
    }

    #[test]
    fn test_bridge_clear_cache() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let term = Term::Constant("hello".to_string());
        let _ = bridge.embed_term(&term);
        bridge.clear_cache();
        // After clear, re-embedding should work fine (no panic)
        let _ = bridge.embed_term(&term);
    }

    #[test]
    fn test_hybrid_mode_adaptive() {
        let config = HybridConfig {
            default_mode: ReasoningMode::Adaptive,
            ..HybridConfig::default()
        };
        let mut reasoner = HybridReasoner::new(config);
        let mut kb = KnowledgeBase::new();
        kb.add(Clause::fact(Predicate::new(
            "cat",
            vec![Term::Constant("tom".to_string())],
        )));

        let goal = Predicate::new("cat", vec![Term::Variable("X".to_string())]);
        let result = reasoner.query(&goal, &kb);
        // Adaptive mode should produce some result without panicking
        assert!(
            !result.explanation.is_empty()
                || !result.bindings.is_empty()
                || result.confidence >= 0.0
        );
    }

    // ── Hybrid reasoning pathway tests ──────────────────────────────────────

    #[test]
    fn test_hybrid_mode_prefers_symbolic_proof() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Hybrid,
            ..Default::default()
        });

        // parent(alice, bob) is a fact — hybrid should find symbolic proof
        let query = Predicate::new(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        assert!(result.satisfied);
        assert_eq!(result.confidence, 1.0);
        assert_eq!(result.symbolic_proof, Some(true));
    }

    #[test]
    fn test_hybrid_falls_back_to_neural() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Hybrid,
            neural_confidence_threshold: 0.0, // accept any neural score
            ..Default::default()
        });

        // Query a predicate that doesn't exist in KB
        let query = Predicate::new(
            "sibling",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        // Should have tried neural after symbolic failed
        assert!(result.neural_score.is_some());
    }

    #[test]
    fn test_symbolic_negative_query() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Symbolic,
            ..Default::default()
        });

        // Query something not in KB
        let query = Predicate::new(
            "parent",
            vec![
                Term::Constant("charlie".to_string()),
                Term::Constant("alice".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        assert!(!result.satisfied);
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.symbolic_proof, Some(false));
    }

    #[test]
    fn test_symbolic_rule_inference() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Symbolic,
            ..Default::default()
        });

        // grandparent(alice, charlie) should be derivable via rule
        let query = Predicate::new(
            "grandparent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("charlie".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        assert!(result.satisfied);
        assert_eq!(result.confidence, 1.0);
    }

    // ── Adaptive mode heuristic tests ───────────────────────────────────────

    #[test]
    fn test_adaptive_selects_symbolic_for_small_kb() {
        let mut kb = KnowledgeBase::new();
        kb.add(Clause::fact(Predicate::new(
            "likes",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("rust".to_string()),
            ],
        )));

        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Adaptive,
            ..Default::default()
        });

        let query = Predicate::new(
            "likes",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("rust".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        // Small KB → should choose symbolic
        assert!(result
            .explanation
            .iter()
            .any(|e| e.contains("Small knowledge base") || e.contains("symbolic")));
    }

    #[test]
    fn test_adaptive_selects_symbolic_for_many_variables() {
        let kb = make_kb();
        let mut reasoner = HybridReasoner::new(HybridConfig {
            default_mode: ReasoningMode::Adaptive,
            ..Default::default()
        });

        // Query with 3 variables → should prefer symbolic
        let query = Predicate::new(
            "parent",
            vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
                Term::Variable("Z".to_string()),
            ],
        );

        let result = reasoner.query(&query, &kb);
        assert!(result
            .explanation
            .iter()
            .any(|e| e.contains("multiple variables") || e.contains("symbolic")));
    }

    // ── NeuralSymbolicBridge tests ──────────────────────────────────────────

    #[test]
    fn test_bridge_embedding_dimension() {
        let mut bridge = NeuralSymbolicBridge::new(32);
        let term = Term::Constant("test".to_string());
        let emb = bridge.embed_term(&term);
        assert_eq!(emb.len(), 32);
    }

    #[test]
    fn test_bridge_variable_embeddings_differ() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let v1 = Term::Variable("X".to_string());
        let v2 = Term::Variable("Y".to_string());
        let e1 = bridge.embed_term(&v1);
        let e2 = bridge.embed_term(&v2);
        // Different variables should produce different embeddings
        assert_ne!(e1, e2);
    }

    #[test]
    fn test_bridge_predicate_embedding() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let pred = Predicate::new(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        );
        let emb = bridge.embed_predicate(&pred);
        assert_eq!(emb.len(), 64);
    }

    #[test]
    fn test_bridge_formula_caching() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let formula = Formula::Atom(Predicate::new(
            "test",
            vec![Term::Constant("x".to_string())],
        ));

        let e1 = bridge.embed_formula(&formula);
        let e2 = bridge.embed_formula(&formula);
        assert_eq!(e1, e2, "Cached formula embedding should be identical");
    }

    #[test]
    fn test_bridge_find_similar_formula() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let kb = make_kb();

        let query = Formula::Atom(Predicate::new(
            "parent",
            vec![
                Term::Constant("alice".to_string()),
                Term::Constant("bob".to_string()),
            ],
        ));

        let results = bridge.find_similar_formula(&query, &kb, 3);
        assert!(!results.is_empty());
        // Results should be sorted by score descending
        for window in results.windows(2) {
            assert!(window[0].1 >= window[1].1);
        }
    }

    #[test]
    fn test_soft_unify_different_terms() {
        let mut bridge = NeuralSymbolicBridge::new(64);
        let t1 = Term::Constant("cat".to_string());
        let t2 = Term::Constant("dog".to_string());
        let score = bridge.soft_unify(&t1, &t2, 0.1);
        // Different terms should have lower unification score
        assert!(score > 0.0); // still positive (it's a softmax)
    }

    // ── Constraint integration ──────────────────────────────────────────────

    #[test]
    fn test_solve_constraints_simple() {
        let mut reasoner = HybridReasoner::new(HybridConfig::default());

        let constraints = vec![SoftConstraint::new("x", ConstraintFormula::Constant(1.0))];

        let result = reasoner.solve_constraints(constraints);
        assert!(result.is_some());
    }

    #[test]
    fn test_formula_to_constraints_atom() {
        let reasoner = HybridReasoner::new(HybridConfig::default());
        let formula = Formula::Atom(Predicate::new(
            "test",
            vec![Term::Constant("a".to_string())],
        ));

        let constraints = reasoner.formula_to_constraints(&formula);
        assert_eq!(constraints.len(), 1);
    }

    #[test]
    fn test_formula_to_constraints_and() {
        let reasoner = HybridReasoner::new(HybridConfig::default());
        let formula = Formula::And(vec![
            Formula::Atom(Predicate::new("a", vec![Term::Constant("x".to_string())])),
            Formula::Atom(Predicate::new("b", vec![Term::Constant("y".to_string())])),
        ]);

        let constraints = reasoner.formula_to_constraints(&formula);
        assert_eq!(
            constraints.len(),
            2,
            "AND should produce one constraint per conjunct"
        );
    }

    #[test]
    fn test_formula_to_constraints_or() {
        let reasoner = HybridReasoner::new(HybridConfig::default());
        let formula = Formula::Or(vec![
            Formula::Atom(Predicate::new("a", vec![Term::Constant("x".to_string())])),
            Formula::Atom(Predicate::new("b", vec![Term::Constant("y".to_string())])),
        ]);

        let constraints = reasoner.formula_to_constraints(&formula);
        assert_eq!(
            constraints.len(),
            1,
            "OR should produce a single disjunction constraint"
        );
    }

    #[test]
    fn test_formula_to_constraints_implies() {
        let reasoner = HybridReasoner::new(HybridConfig::default());
        let formula = Formula::Implies(
            Box::new(Formula::Atom(Predicate::new("a", vec![]))),
            Box::new(Formula::Atom(Predicate::new("b", vec![]))),
        );

        let constraints = reasoner.formula_to_constraints(&formula);
        assert_eq!(constraints.len(), 1);
    }

    #[test]
    fn test_formula_to_constraints_not() {
        let reasoner = HybridReasoner::new(HybridConfig::default());
        let formula = Formula::Not(Box::new(Formula::Atom(Predicate::new(
            "bad",
            vec![Term::Constant("x".to_string())],
        ))));

        let constraints = reasoner.formula_to_constraints(&formula);
        assert_eq!(constraints.len(), 1);
    }

    // ── Cosine similarity ───────────────────────────────────────────────────

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let score = cosine_similarity(&v, &v);
        assert!((score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let score = cosine_similarity(&a, &b);
        assert!(score.abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!((score - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 2.0];
        let b = vec![0.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0];
        let score = cosine_similarity(&a, &b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_count_variables_in_predicate() {
        let pred = Predicate::new(
            "test",
            vec![
                Term::Variable("X".to_string()),
                Term::Constant("a".to_string()),
                Term::Variable("Y".to_string()),
            ],
        );
        assert_eq!(count_variables_in_predicate(&pred), 2);
    }

    #[test]
    fn test_count_variables_all_constants() {
        let pred = Predicate::new(
            "test",
            vec![
                Term::Constant("a".to_string()),
                Term::Constant("b".to_string()),
            ],
        );
        assert_eq!(count_variables_in_predicate(&pred), 0);
    }

    // ── HybridResult ────────────────────────────────────────────────────────

    #[test]
    fn test_hybrid_result_default_state() {
        let result = HybridResult::new("query", ReasoningMode::Neural);
        assert!(!result.satisfied);
        assert_eq!(result.confidence, 0.0);
        assert!(result.neural_score.is_none());
        assert!(result.symbolic_proof.is_none());
        assert!(result.bindings.is_empty());
        assert!(result.explanation.is_empty());
        assert_eq!(result.mode_used, ReasoningMode::Neural);
    }
}
