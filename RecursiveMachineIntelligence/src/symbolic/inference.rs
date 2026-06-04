//! Inference Engine
//!
//! Provides automated reasoning capabilities for AI agents,
//! including resolution, forward/backward chaining, and proof search.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[cfg(test)]
use super::logic::Term;
use super::logic::{Clause, KnowledgeBase, Predicate};
use super::unification::{predicate_unify::unify_predicates, Substitution};

/// Inference rules that can be applied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InferenceRule {
    /// Modus Ponens: A, A → B ⊢ B
    ModusPonens,

    /// Modus Tollens: ¬B, A → B ⊢ ¬A
    ModusTollens,

    /// Hypothetical Syllogism: A → B, B → C ⊢ A → C
    HypotheticalSyllogism,

    /// Resolution: A ∨ B, ¬A ∨ C ⊢ B ∨ C
    Resolution,

    /// Unit Resolution (simplified)
    UnitResolution,

    /// Factoring: A ∨ A ⊢ A
    Factoring,

    /// Universal Instantiation: ∀x.P(x) ⊢ P(a)
    UniversalInstantiation,

    /// Existential Generalization: P(a) ⊢ ∃x.P(x)
    ExistentialGeneralization,

    /// Conjunction Introduction: A, B ⊢ A ∧ B
    ConjunctionIntro,

    /// Conjunction Elimination: A ∧ B ⊢ A (or B)
    ConjunctionElim,

    /// Disjunction Introduction: A ⊢ A ∨ B
    DisjunctionIntro,
}

/// A single step in a proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofStep {
    /// Unique identifier
    pub id: Uuid,

    /// Step number (1-indexed)
    pub number: usize,

    /// The formula/clause derived
    pub derived: String,

    /// The rule used
    pub rule: InferenceRule,

    /// References to previous steps (or axioms)
    pub premises: Vec<String>,

    /// Substitution applied (if any)
    pub substitution: Option<Substitution>,
}

/// A complete proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// Unique identifier
    pub id: Uuid,

    /// The goal that was proved
    pub goal: String,

    /// Steps in the proof
    pub steps: Vec<ProofStep>,

    /// Whether the proof is complete
    pub complete: bool,

    /// Time taken in milliseconds
    pub time_ms: u64,
}

impl Proof {
    /// Create a new empty proof
    pub fn new(goal: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            goal: goal.into(),
            steps: Vec::new(),
            complete: false,
            time_ms: 0,
        }
    }

    /// Add a step to the proof
    pub fn add_step(&mut self, derived: String, rule: InferenceRule, premises: Vec<String>) {
        let step = ProofStep {
            id: Uuid::new_v4(),
            number: self.steps.len() + 1,
            derived,
            rule,
            premises,
            substitution: None,
        };
        self.steps.push(step);
    }

    /// Mark proof as complete
    pub fn mark_complete(&mut self) {
        self.complete = true;
    }

    /// Check if the proof is valid
    pub fn is_valid(&self) -> bool {
        self.complete && !self.steps.is_empty()
    }
}

/// Configuration for the inference engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Maximum search depth
    pub max_depth: usize,

    /// Maximum number of resolutions
    pub max_resolutions: usize,

    /// Timeout in milliseconds
    pub timeout_ms: u64,

    /// Whether to use iterative deepening
    pub iterative_deepening: bool,

    /// Whether to record full proofs
    pub record_proofs: bool,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            max_depth: 100,
            max_resolutions: 10000,
            timeout_ms: 30000,
            iterative_deepening: true,
            record_proofs: true,
        }
    }
}

/// The inference engine
pub struct InferenceEngine {
    /// Configuration
    config: InferenceConfig,

    /// Counter for clause renaming
    rename_counter: usize,
}

impl InferenceEngine {
    /// Create a new inference engine
    pub fn new(config: InferenceConfig) -> Self {
        Self {
            config,
            rename_counter: 0,
        }
    }

    /// Create with default configuration
    pub fn default_engine() -> Self {
        Self::new(InferenceConfig::default())
    }

    /// Query the knowledge base using backward chaining (SLD resolution)
    pub fn query(&mut self, kb: &KnowledgeBase, goal: &Predicate) -> Option<Vec<Substitution>> {
        let goals = vec![goal.clone()];
        self.solve_goals(kb, &goals, Substitution::new(), 0)
    }

    /// Solve a list of goals
    fn solve_goals(
        &mut self,
        kb: &KnowledgeBase,
        goals: &[Predicate],
        subst: Substitution,
        depth: usize,
    ) -> Option<Vec<Substitution>> {
        // Base case: no goals left
        if goals.is_empty() {
            return Some(vec![subst]);
        }

        // Depth limit
        if depth >= self.config.max_depth {
            return None;
        }

        let goal = &goals[0];
        let remaining = &goals[1..];

        let mut all_solutions = Vec::new();

        // Try each clause in the KB
        for clause in kb.get(&goal.name) {
            // Rename variables to avoid capture
            self.rename_counter += 1;
            let renamed = clause.rename_variables(&self.rename_counter.to_string());

            if let Some(ref head) = renamed.head {
                // Try to unify goal with clause head
                if let Ok(new_subst) = unify_predicates(goal, head) {
                    let combined = compose_subst(&subst, &new_subst);

                    // Add clause body to goals (with substitution applied)
                    let mut new_goals: Vec<Predicate> = renamed
                        .body
                        .iter()
                        .map(|p| p.apply_substitution(&combined))
                        .collect();

                    // Add remaining goals
                    for g in remaining {
                        new_goals.push(g.apply_substitution(&combined));
                    }

                    // Recursively solve
                    if let Some(solutions) = self.solve_goals(kb, &new_goals, combined, depth + 1) {
                        all_solutions.extend(solutions);
                    }
                }
            }
        }

        if all_solutions.is_empty() {
            None
        } else {
            Some(all_solutions)
        }
    }

    /// Forward chaining inference
    pub fn forward_chain(&self, kb: &mut KnowledgeBase) -> Vec<Predicate> {
        let mut derived = Vec::new();
        let mut changed = true;
        let mut iterations = 0;

        while changed && iterations < self.config.max_resolutions {
            changed = false;
            iterations += 1;

            // Get all rules
            let rules: Vec<_> = kb.all().iter().filter(|c| c.is_rule()).cloned().collect();

            for rule in rules {
                // Try to match all body predicates
                if let Some(ref head) = rule.head {
                    if let Some(substs) = self.match_body(kb, &rule.body) {
                        for subst in substs {
                            let new_fact = head.apply_substitution(&subst);

                            // Check if this is new
                            if !self.fact_exists(kb, &new_fact) {
                                derived.push(new_fact.clone());
                                kb.add(Clause::fact(new_fact));
                                changed = true;
                            }
                        }
                    }
                }
            }
        }

        derived
    }

    /// Match all predicates in a body against the knowledge base
    fn match_body(&self, kb: &KnowledgeBase, body: &[Predicate]) -> Option<Vec<Substitution>> {
        if body.is_empty() {
            return Some(vec![Substitution::new()]);
        }

        let first = &body[0];
        let rest = &body[1..];

        let mut results = Vec::new();

        for clause in kb.get(&first.name) {
            if let Some(ref head) = clause.head {
                if clause.is_fact() {
                    if let Ok(subst) = unify_predicates(first, head) {
                        if rest.is_empty() {
                            results.push(subst);
                        } else {
                            // Apply substitution to remaining predicates and recurse
                            let new_rest: Vec<Predicate> =
                                rest.iter().map(|p| p.apply_substitution(&subst)).collect();

                            if let Some(more_substs) = self.match_body(kb, &new_rest) {
                                for s in more_substs {
                                    results.push(compose_subst(&subst, &s));
                                }
                            }
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }

    /// Check if a fact already exists in the KB
    fn fact_exists(&self, kb: &KnowledgeBase, fact: &Predicate) -> bool {
        for clause in kb.get(&fact.name) {
            if clause.is_fact() {
                if let Some(ref head) = clause.head {
                    if head == fact {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Prove a goal and return the proof
    pub fn prove(&mut self, kb: &KnowledgeBase, goal: &Predicate) -> Option<Proof> {
        let mut proof = Proof::new(format!("{}", goal));
        let start = std::time::Instant::now();

        // Add KB facts as axioms
        for clause in kb.all() {
            if clause.is_fact() {
                proof.add_step(
                    format!("{}", clause),
                    InferenceRule::UniversalInstantiation,
                    vec!["Axiom".to_string()],
                );
            }
        }

        // Try to find a proof
        if let Some(solutions) = self.query(kb, goal) {
            if !solutions.is_empty() {
                let subst = &solutions[0];
                let instantiated = goal.apply_substitution(subst);

                proof.add_step(
                    format!("{}", instantiated),
                    InferenceRule::Resolution,
                    vec!["SLD Resolution".to_string()],
                );

                proof.mark_complete();
            }
        }

        proof.time_ms = start.elapsed().as_millis() as u64;

        if proof.complete {
            Some(proof)
        } else {
            None
        }
    }

    /// Resolution refutation
    pub fn refutation(&mut self, kb: &KnowledgeBase, negated_goal: &[Predicate]) -> bool {
        // Convert KB to clauses
        let mut clauses: Vec<Vec<Literal>> = kb.all().iter().map(clause_to_literals).collect();

        // Add negated goal as a clause
        let goal_literals: Vec<Literal> = negated_goal
            .iter()
            .map(|p| Literal::Negative(p.clone()))
            .collect();
        clauses.push(goal_literals);

        // Resolution loop
        let mut new_clauses = HashSet::new();
        let mut iterations = 0;

        while iterations < self.config.max_resolutions {
            iterations += 1;

            for i in 0..clauses.len() {
                for j in (i + 1)..clauses.len() {
                    if let Some(resolvents) = resolve(&clauses[i], &clauses[j]) {
                        for resolvent in resolvents {
                            // Empty clause = contradiction = goal proved
                            if resolvent.is_empty() {
                                return true;
                            }

                            let key = format!("{:?}", resolvent);
                            if !new_clauses.contains(&key) {
                                new_clauses.insert(key);
                                clauses.push(resolvent);
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

impl Default for InferenceEngine {
    fn default() -> Self {
        Self::default_engine()
    }
}

/// A literal (positive or negative predicate)
#[derive(Debug, Clone, PartialEq, Eq)]
enum Literal {
    Positive(Predicate),
    Negative(Predicate),
}

impl Literal {
    fn predicate(&self) -> &Predicate {
        match self {
            Literal::Positive(p) | Literal::Negative(p) => p,
        }
    }

    fn is_positive(&self) -> bool {
        matches!(self, Literal::Positive(_))
    }

    fn apply_substitution(&self, subst: &Substitution) -> Literal {
        match self {
            Literal::Positive(p) => Literal::Positive(p.apply_substitution(subst)),
            Literal::Negative(p) => Literal::Negative(p.apply_substitution(subst)),
        }
    }
}

/// Convert a clause to a list of literals
fn clause_to_literals(clause: &Clause) -> Vec<Literal> {
    let mut literals = Vec::new();

    // Head is positive
    if let Some(ref head) = clause.head {
        literals.push(Literal::Positive(head.clone()));
    }

    // Body predicates are negative (in the implication sense)
    for pred in &clause.body {
        literals.push(Literal::Negative(pred.clone()));
    }

    literals
}

/// Resolve two clauses
fn resolve(c1: &[Literal], c2: &[Literal]) -> Option<Vec<Vec<Literal>>> {
    let mut results = Vec::new();

    for (i, lit1) in c1.iter().enumerate() {
        for (j, lit2) in c2.iter().enumerate() {
            // Check for complementary literals
            if lit1.is_positive() != lit2.is_positive() {
                // Try to unify the predicates
                if let Ok(subst) = unify_predicates(lit1.predicate(), lit2.predicate()) {
                    // Create resolvent
                    let mut resolvent: Vec<Literal> = Vec::new();

                    // Add literals from c1 (except the resolved one)
                    for (k, lit) in c1.iter().enumerate() {
                        if k != i {
                            resolvent.push(lit.apply_substitution(&subst));
                        }
                    }

                    // Add literals from c2 (except the resolved one)
                    for (k, lit) in c2.iter().enumerate() {
                        if k != j {
                            resolvent.push(lit.apply_substitution(&subst));
                        }
                    }

                    results.push(resolvent);
                }
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Compose substitutions
fn compose_subst(s1: &Substitution, s2: &Substitution) -> Substitution {
    let mut result: Substitution = s1
        .iter()
        .map(|(k, v)| (k.clone(), v.apply_substitution(s2)))
        .collect();

    for (k, v) in s2 {
        if !result.contains_key(k) {
            result.insert(k.clone(), v.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let mut kb = KnowledgeBase::new();

        // Facts
        kb.add_fact(
            "parent",
            vec![Term::constant("alice"), Term::constant("bob")],
        );
        kb.add_fact(
            "parent",
            vec![Term::constant("bob"), Term::constant("charlie")],
        );

        // Rule: grandparent(X, Z) :- parent(X, Y), parent(Y, Z)
        kb.add_rule(
            Predicate::new("grandparent", vec![Term::var("X"), Term::var("Z")]),
            vec![
                Predicate::new("parent", vec![Term::var("X"), Term::var("Y")]),
                Predicate::new("parent", vec![Term::var("Y"), Term::var("Z")]),
            ],
        );

        let mut engine = InferenceEngine::default_engine();

        // Query: grandparent(X, charlie)?
        let goal = Predicate::new(
            "grandparent",
            vec![Term::var("X"), Term::constant("charlie")],
        );
        let solutions = engine.query(&kb, &goal);

        assert!(solutions.is_some());
        let solutions = solutions.unwrap();
        assert!(!solutions.is_empty());

        // X should be alice
        assert!(solutions
            .iter()
            .any(|s| s.get("X") == Some(&Term::constant("alice"))));
    }

    #[test]
    fn test_forward_chaining() {
        let mut kb = KnowledgeBase::new();

        // Facts
        kb.add_fact("animal", vec![Term::constant("tweety")]);
        kb.add_fact("bird", vec![Term::constant("tweety")]);

        // Rule: bird(X) -> canfly(X)
        kb.add_rule(
            Predicate::new("canfly", vec![Term::var("X")]),
            vec![Predicate::new("bird", vec![Term::var("X")])],
        );

        let engine = InferenceEngine::default_engine();
        let derived = engine.forward_chain(&mut kb);

        // Should derive canfly(tweety)
        assert!(derived
            .iter()
            .any(|p| { p.name == "canfly" && p.args == vec![Term::constant("tweety")] }));
    }

    #[test]
    fn test_prove() {
        let mut kb = KnowledgeBase::new();

        kb.add_fact("mortal", vec![Term::constant("socrates")]);

        let mut engine = InferenceEngine::default_engine();

        let goal = Predicate::new("mortal", vec![Term::constant("socrates")]);
        let proof = engine.prove(&kb, &goal);

        assert!(proof.is_some());
        let proof = proof.unwrap();
        assert!(proof.is_valid());
    }

    #[test]
    fn test_no_solution() {
        let kb = KnowledgeBase::new();
        let mut engine = InferenceEngine::default_engine();

        let goal = Predicate::new("exists", vec![Term::constant("nothing")]);
        let solutions = engine.query(&kb, &goal);

        assert!(solutions.is_none());
    }

    #[test]
    fn test_proof_validity() {
        let mut proof = Proof::new("test");
        assert!(!proof.is_valid(), "Empty proof is not valid");

        proof.add_step("test".to_string(), InferenceRule::ModusPonens, vec![]);
        assert!(!proof.is_valid(), "Proof not marked complete");

        proof.mark_complete();
        assert!(proof.is_valid());
    }

    #[test]
    fn test_inference_config_defaults() {
        let config = InferenceConfig::default();
        assert_eq!(config.max_depth, 100);
        assert_eq!(config.max_resolutions, 10000);
        assert!(config.iterative_deepening);
        assert!(config.record_proofs);
    }

    #[test]
    fn test_engine_no_solution_complex() {
        let mut engine = InferenceEngine::default_engine();
        let mut kb = KnowledgeBase::new();
        kb.add(Clause::fact(Predicate::new("cat", vec![Term::Constant("tom".to_string())])));

        // Query for something completely unrelated
        let goal = Predicate::new("dog", vec![Term::Variable("X".to_string())]);
        let result = engine.query(&kb, &goal);
        assert!(result.is_none() || result.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_forward_chaining_derives() {
        let engine = InferenceEngine::default_engine();
        let mut kb = KnowledgeBase::new();
        // cat(tom).
        kb.add(Clause::fact(Predicate::new("cat", vec![Term::Constant("tom".to_string())])));
        // animal(X) :- cat(X).
        kb.add(Clause::rule(
            Predicate::new("animal", vec![Term::Variable("X".to_string())]),
            vec![Predicate::new("cat", vec![Term::Variable("X".to_string())])],
        ));

        let derived = engine.forward_chain(&mut kb);
        let has_animal = derived.iter().any(|p| p.name == "animal");
        assert!(has_animal, "Should derive animal(tom)");
    }

}
