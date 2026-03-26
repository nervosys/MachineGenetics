/// MechGen Logic Inference Engine — Datalog-style reasoning for KB blocks.
///
/// Compiles `kb` definitions into an in-memory fact store with
/// bottom-up (semi-naive) evaluation of rules.
///
/// Supports:
///   - Ground facts: `parent("alice", "bob")`
///   - Rules with conjunctive bodies: `ancestor(X, Y) :- parent(X, Z), ancestor(Z, Y)`
///   - Queries: `kb.query("ancestor", &["alice", "?"])`
///   - Stratified negation (future)
///
/// The engine generates MLIR-compatible operation sequences for
/// materializing the knowledge base at compile time.
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::collections::{HashMap, HashSet, BTreeSet};

// ── Terms and atoms ─────────────────────────────────────────────────

/// A term in a logic formula.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Term {
    /// A constant value (ground).
    Const(String),
    /// A logic variable (starts with uppercase by convention).
    Var(String),
}

impl Term {
    pub fn is_var(&self) -> bool {
        matches!(self, Term::Var(_))
    }
}

/// An atom: predicate(term, term, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Atom {
    pub predicate: String,
    pub args: Vec<Term>,
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args: Vec<String> = self.args.iter().map(|t| match t {
            Term::Const(c) => format!("\"{c}\""),
            Term::Var(v) => v.clone(),
        }).collect();
        write!(f, "{}({})", self.predicate, args.join(", "))
    }
}

/// A ground atom (all terms are constants).
pub type GroundAtom = Atom;

/// A rule: head :- body₁, body₂, ..., bodyₙ
#[derive(Debug, Clone)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Atom>,
}

// ── Substitution (variable bindings) ────────────────────────────────

type Binding = HashMap<String, String>;

fn apply_binding(term: &Term, binding: &Binding) -> Term {
    match term {
        Term::Var(v) => {
            if let Some(val) = binding.get(v) {
                Term::Const(val.clone())
            } else {
                term.clone()
            }
        }
        Term::Const(_) => term.clone(),
    }
}

fn apply_binding_atom(atom: &Atom, binding: &Binding) -> Atom {
    Atom {
        predicate: atom.predicate.clone(),
        args: atom.args.iter().map(|t| apply_binding(t, binding)).collect(),
    }
}

fn is_ground(atom: &Atom) -> bool {
    atom.args.iter().all(|t| matches!(t, Term::Const(_)))
}

// ── Unification ─────────────────────────────────────────────────────

/// Try to unify an atom with a ground fact, returning bindings if successful.
fn unify_atom(pattern: &Atom, fact: &Atom, binding: &Binding) -> Option<Binding> {
    if pattern.predicate != fact.predicate || pattern.args.len() != fact.args.len() {
        return None;
    }

    let mut result = binding.clone();
    for (pat, val) in pattern.args.iter().zip(&fact.args) {
        let pat = apply_binding(pat, &result);
        match (&pat, val) {
            (Term::Const(a), Term::Const(b)) => {
                if a != b {
                    return None;
                }
            }
            (Term::Var(v), Term::Const(c)) => {
                result.insert(v.clone(), c.clone());
            }
            _ => return None, // Facts must be ground.
        }
    }
    Some(result)
}

// ── Knowledge base ──────────────────────────────────────────────────

/// An in-memory knowledge base with facts and rules.
pub struct KnowledgeBase {
    pub name: String,
    /// Ground facts, indexed by predicate name.
    facts: HashMap<String, BTreeSet<Vec<String>>>,
    /// Rules for deriving new facts.
    rules: Vec<Rule>,
    /// Whether fixpoint has been reached.
    materialized: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl KnowledgeBase {
    pub fn new(name: &str) -> Self {
        KnowledgeBase {
            name: name.to_string(),
            facts: HashMap::new(),
            rules: Vec::new(),
            materialized: false,
            diagnostics: Vec::new(),
        }
    }

    /// Add a ground fact.
    pub fn add_fact(&mut self, predicate: &str, args: Vec<String>) {
        self.facts.entry(predicate.to_string()).or_default().insert(args);
        self.materialized = false;
    }

    /// Add a rule.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        self.materialized = false;
    }

    /// Get all facts for a predicate.
    pub fn get_facts(&self, predicate: &str) -> Vec<Vec<String>> {
        self.facts
            .get(predicate)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Total number of ground facts across all predicates.
    pub fn fact_count(&self) -> usize {
        self.facts.values().map(|s| s.len()).sum()
    }

    /// Evaluate all rules to fixpoint using semi-naive bottom-up evaluation.
    pub fn materialize(&mut self) {
        if self.materialized {
            return;
        }

        let max_iterations = 1000; // Safety bound.
        for iteration in 0..max_iterations {
            let mut new_facts = Vec::new();

            for rule in &self.rules {
                let derived = self.evaluate_rule(rule);
                for atom in derived {
                    let args: Vec<String> = atom.args.iter().map(|t| match t {
                        Term::Const(c) => c.clone(),
                        Term::Var(_) => unreachable!("derived facts must be ground"),
                    }).collect();

                    let existing = self.facts.entry(atom.predicate.clone()).or_default();
                    if !existing.contains(&args) {
                        new_facts.push((atom.predicate.clone(), args));
                    }
                }
            }

            if new_facts.is_empty() {
                self.materialized = true;
                return;
            }

            for (pred, args) in new_facts {
                self.facts.entry(pred).or_default().insert(args);
            }

            if iteration == max_iterations - 1 {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Warning,
                    format!("kb `{}`: fixpoint not reached after {max_iterations} iterations", self.name),
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
            }
        }

        self.materialized = true;
    }

    /// Evaluate a single rule against current facts.
    fn evaluate_rule(&self, rule: &Rule) -> Vec<GroundAtom> {
        // Start with an empty binding.
        let bindings = vec![Binding::new()];

        // Join each body atom with current facts.
        let mut current_bindings = bindings;
        for body_atom in &rule.body {
            let mut next_bindings = Vec::new();
            let pred_facts = self.get_facts(&body_atom.predicate);

            for binding in &current_bindings {
                for fact_args in &pred_facts {
                    let fact = Atom {
                        predicate: body_atom.predicate.clone(),
                        args: fact_args.iter().map(|a| Term::Const(a.clone())).collect(),
                    };
                    if let Some(new_binding) = unify_atom(body_atom, &fact, binding) {
                        next_bindings.push(new_binding);
                    }
                }
            }
            current_bindings = next_bindings;
        }

        // Apply bindings to head to generate derived facts.
        let mut results = Vec::new();
        for binding in &current_bindings {
            let derived = apply_binding_atom(&rule.head, binding);
            if is_ground(&derived) {
                results.push(derived);
            }
        }
        results
    }

    /// Query the knowledge base. `"?"` in args means wildcard.
    pub fn query(&mut self, predicate: &str, args: &[&str]) -> Vec<Vec<String>> {
        self.materialize();

        let all_facts = self.get_facts(predicate);
        all_facts
            .into_iter()
            .filter(|fact_args| {
                if fact_args.len() != args.len() {
                    return false;
                }
                args.iter().zip(fact_args.iter()).all(|(q, f)| *q == "?" || *q == f)
            })
            .collect()
    }

    /// Generate MLIR operations for materializing this KB.
    pub fn emit_mlir(&mut self) -> Vec<String> {
        self.materialize();

        let mut ops = Vec::new();
        ops.push(format!("MechGen.kb @{} {{", self.name));

        for (pred, fact_set) in &self.facts {
            for args in fact_set {
                let formatted: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
                ops.push(format!(
                    "  MechGen.kb.fact \"{pred}\"({}) : ()",
                    formatted.join(", ")
                ));
            }
        }

        for (i, rule) in self.rules.iter().enumerate() {
            let head_str = format!("{}", rule.head);
            let body_strs: Vec<String> = rule.body.iter().map(|a| format!("{a}")).collect();
            ops.push(format!(
                "  MechGen.kb.rule @rule_{i} \"{head_str}\" :- {}",
                body_strs.join(", ")
            ));
        }

        ops.push("}".to_string());
        ops
    }
}

// ── AST → KB builder ───────────────────────────────────────────────

/// Build a KnowledgeBase from an AST KbDef.
pub fn build_kb(def: &ast::KbDef) -> KnowledgeBase {
    let mut kb = KnowledgeBase::new(&def.name);

    for fact in &def.facts {
        let args: Vec<String> = fact.args.iter().filter_map(|e| match e {
            ast::Expr::Literal { value, kind: ast::LiteralKind::String } => {
                Some(value.trim_matches('"').to_string())
            }
            ast::Expr::Literal { value, .. } => Some(value.clone()),
            ast::Expr::Ident { name } => Some(name.clone()),
            _ => None,
        }).collect();
        kb.add_fact(&fact.name, args);
    }

    for rule in &def.rules {
        let head_args: Vec<Term> = rule.params.iter().map(|p| {
            if p.name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                Term::Var(p.name.clone())
            } else {
                Term::Const(p.name.clone())
            }
        }).collect();

        let head = Atom {
            predicate: rule.name.clone(),
            args: head_args,
        };

        let body: Vec<Atom> = rule.conditions.iter().filter_map(|cond| {
            expr_to_atom(cond)
        }).collect();

        kb.add_rule(Rule { head, body });
    }

    kb
}

/// Try to interpret a condition expression as a logic atom.
fn expr_to_atom(expr: &ast::Expr) -> Option<Atom> {
    match expr {
        ast::Expr::Call { func, args } => {
            let pred = match func.as_ref() {
                ast::Expr::Ident { name } => name.clone(),
                _ => return None,
            };
            let terms: Vec<Term> = args.iter().map(|a| match a {
                ast::Expr::Ident { name } => {
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        Term::Var(name.clone())
                    } else {
                        Term::Const(name.clone())
                    }
                }
                ast::Expr::Literal { value, .. } => {
                    Term::Const(value.trim_matches('"').to_string())
                }
                _ => Term::Const("_".into()),
            }).collect();
            Some(Atom { predicate: pred, args: terms })
        }
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_fact_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["bob".into(), "charlie".into()]);

        let results = kb.query("parent", &["alice", "?"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], vec!["alice", "bob"]);
    }

    #[test]
    fn wildcard_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["alice".into(), "carol".into()]);
        kb.add_fact("parent", vec!["bob".into(), "dave".into()]);

        let results = kb.query("parent", &["alice", "?"]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn transitive_rule() {
        let mut kb = KnowledgeBase::new("family");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["bob".into(), "charlie".into()]);
        kb.add_fact("parent", vec!["charlie".into(), "dave".into()]);

        // ancestor(X, Y) :- parent(X, Y)
        kb.add_rule(Rule {
            head: Atom {
                predicate: "ancestor".into(),
                args: vec![Term::Var("X".into()), Term::Var("Y".into())],
            },
            body: vec![Atom {
                predicate: "parent".into(),
                args: vec![Term::Var("X".into()), Term::Var("Y".into())],
            }],
        });

        // ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
        kb.add_rule(Rule {
            head: Atom {
                predicate: "ancestor".into(),
                args: vec![Term::Var("X".into()), Term::Var("Z".into())],
            },
            body: vec![
                Atom {
                    predicate: "parent".into(),
                    args: vec![Term::Var("X".into()), Term::Var("Y".into())],
                },
                Atom {
                    predicate: "ancestor".into(),
                    args: vec![Term::Var("Y".into()), Term::Var("Z".into())],
                },
            ],
        });

        let results = kb.query("ancestor", &["alice", "?"]);
        assert_eq!(results.len(), 3); // bob, charlie, dave
    }

    #[test]
    fn rule_fixpoint() {
        let mut kb = KnowledgeBase::new("graph");
        kb.add_fact("edge", vec!["a".into(), "b".into()]);
        kb.add_fact("edge", vec!["b".into(), "c".into()]);
        kb.add_fact("edge", vec!["c".into(), "d".into()]);

        // reachable(X, Y) :- edge(X, Y)
        kb.add_rule(Rule {
            head: Atom {
                predicate: "reachable".into(),
                args: vec![Term::Var("X".into()), Term::Var("Y".into())],
            },
            body: vec![Atom {
                predicate: "edge".into(),
                args: vec![Term::Var("X".into()), Term::Var("Y".into())],
            }],
        });

        // reachable(X, Z) :- edge(X, Y), reachable(Y, Z)
        kb.add_rule(Rule {
            head: Atom {
                predicate: "reachable".into(),
                args: vec![Term::Var("X".into()), Term::Var("Z".into())],
            },
            body: vec![
                Atom {
                    predicate: "edge".into(),
                    args: vec![Term::Var("X".into()), Term::Var("Y".into())],
                },
                Atom {
                    predicate: "reachable".into(),
                    args: vec![Term::Var("Y".into()), Term::Var("Z".into())],
                },
            ],
        });

        let all = kb.query("reachable", &["a", "?"]);
        assert_eq!(all.len(), 3); // b, c, d
    }

    #[test]
    fn empty_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);

        let results = kb.query("color", &["grass", "?"]);
        assert!(results.is_empty());
    }

    #[test]
    fn fact_count() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("a", vec!["1".into()]);
        kb.add_fact("a", vec!["2".into()]);
        kb.add_fact("b", vec!["x".into()]);
        assert_eq!(kb.fact_count(), 3);
    }

    #[test]
    fn mlir_emission() {
        let mut kb = KnowledgeBase::new("test_kb");
        kb.add_fact("parent", vec!["a".into(), "b".into()]);
        let ops = kb.emit_mlir();
        assert!(ops[0].contains("MechGen.kb @test_kb"));
        assert!(ops.iter().any(|op| op.contains("MechGen.kb.fact")));
    }

    #[test]
    fn duplicate_facts_deduplicated() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);
        assert_eq!(kb.fact_count(), 1);
    }
}
