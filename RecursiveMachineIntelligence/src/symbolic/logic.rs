//! First-Order Logic Primitives
//!
//! Provides fundamental logic constructs for symbolic reasoning,
//! designed for efficient manipulation by AI agents.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

/// A logical term (variable, constant, or function application)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Term {
    /// Variable (starts with uppercase by convention)
    Variable(String),

    /// Constant symbol
    Constant(String),

    /// Integer literal
    Integer(i64),

    /// Float literal (stored as bits for hashing)
    Float(u64),

    /// Function application: f(t1, t2, ...)
    Function {
        /// Function name
        name: String,
        /// Function arguments
        args: Vec<Term>,
    },

    /// List term
    List(Vec<Term>),

    /// Cons cell for list pattern matching
    Cons(Box<Term>, Box<Term>),
}

impl Term {
    /// Create a variable term
    pub fn var(name: impl Into<String>) -> Self {
        Term::Variable(name.into())
    }

    /// Create a constant term
    pub fn constant(name: impl Into<String>) -> Self {
        Term::Constant(name.into())
    }

    /// Create an integer term
    pub fn int(n: i64) -> Self {
        Term::Integer(n)
    }

    /// Create a float term
    pub fn float(f: f64) -> Self {
        Term::Float(f.to_bits())
    }

    /// Create a function application
    pub fn func(name: impl Into<String>, args: Vec<Term>) -> Self {
        Term::Function {
            name: name.into(),
            args,
        }
    }

    /// Create a list term
    pub fn list(terms: Vec<Term>) -> Self {
        Term::List(terms)
    }

    /// Check if this is a variable
    pub fn is_variable(&self) -> bool {
        matches!(self, Term::Variable(_))
    }

    /// Check if this is ground (no variables)
    pub fn is_ground(&self) -> bool {
        match self {
            Term::Variable(_) => false,
            Term::Constant(_) | Term::Integer(_) | Term::Float(_) => true,
            Term::Function { args, .. } => args.iter().all(|t| t.is_ground()),
            Term::List(terms) => terms.iter().all(|t| t.is_ground()),
            Term::Cons(head, tail) => head.is_ground() && tail.is_ground(),
        }
    }

    /// Get all variables in this term
    pub fn variables(&self) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_variables(&mut vars);
        vars
    }

    fn collect_variables(&self, vars: &mut HashSet<String>) {
        match self {
            Term::Variable(name) => {
                vars.insert(name.clone());
            }
            Term::Function { args, .. } => {
                for arg in args {
                    arg.collect_variables(vars);
                }
            }
            Term::List(terms) => {
                for term in terms {
                    term.collect_variables(vars);
                }
            }
            Term::Cons(head, tail) => {
                head.collect_variables(vars);
                tail.collect_variables(vars);
            }
            _ => {}
        }
    }

    /// Apply a substitution to this term
    pub fn apply_substitution(&self, subst: &HashMap<String, Term>) -> Term {
        match self {
            Term::Variable(name) => subst.get(name).cloned().unwrap_or_else(|| self.clone()),
            Term::Function { name, args } => Term::Function {
                name: name.clone(),
                args: args.iter().map(|t| t.apply_substitution(subst)).collect(),
            },
            Term::List(terms) => {
                Term::List(terms.iter().map(|t| t.apply_substitution(subst)).collect())
            }
            Term::Cons(head, tail) => Term::Cons(
                Box::new(head.apply_substitution(subst)),
                Box::new(tail.apply_substitution(subst)),
            ),
            _ => self.clone(),
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Variable(name) => write!(f, "{}", name),
            Term::Constant(name) => write!(f, "{}", name),
            Term::Integer(n) => write!(f, "{}", n),
            Term::Float(bits) => write!(f, "{}", f64::from_bits(*bits)),
            Term::Function { name, args } => {
                write!(f, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Term::List(terms) => {
                write!(f, "[")?;
                for (i, term) in terms.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", term)?;
                }
                write!(f, "]")
            }
            Term::Cons(head, tail) => {
                write!(f, "[{} | {}]", head, tail)
            }
        }
    }
}

/// A predicate (relation) application
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Predicate {
    /// Predicate name
    pub name: String,

    /// Arguments
    pub args: Vec<Term>,
}

impl Predicate {
    /// Create a new predicate
    pub fn new(name: impl Into<String>, args: Vec<Term>) -> Self {
        Self {
            name: name.into(),
            args,
        }
    }

    /// Arity of the predicate
    pub fn arity(&self) -> usize {
        self.args.len()
    }

    /// Check if ground
    pub fn is_ground(&self) -> bool {
        self.args.iter().all(|t| t.is_ground())
    }

    /// Get all variables
    pub fn variables(&self) -> HashSet<String> {
        let mut vars = HashSet::new();
        for arg in &self.args {
            vars.extend(arg.variables());
        }
        vars
    }

    /// Apply substitution
    pub fn apply_substitution(&self, subst: &HashMap<String, Term>) -> Self {
        Predicate {
            name: self.name.clone(),
            args: self
                .args
                .iter()
                .map(|t| t.apply_substitution(subst))
                .collect(),
        }
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", arg)?;
        }
        write!(f, ")")
    }
}

/// A logical formula
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Formula {
    /// Atomic formula (predicate)
    Atom(Predicate),

    /// Negation
    Not(Box<Formula>),

    /// Conjunction
    And(Vec<Formula>),

    /// Disjunction
    Or(Vec<Formula>),

    /// Implication
    Implies(Box<Formula>, Box<Formula>),

    /// Bi-conditional
    Iff(Box<Formula>, Box<Formula>),

    /// Universal quantification
    ForAll {
        /// Quantified variable names
        variables: Vec<String>,
        /// Formula body
        formula: Box<Formula>,
    },

    /// Existential quantification
    Exists {
        /// Quantified variable names
        variables: Vec<String>,
        /// Formula body
        formula: Box<Formula>,
    },

    /// Equality
    Equals(Term, Term),

    /// True constant
    True,

    /// False constant
    False,
}

impl Formula {
    /// Create an atomic formula
    pub fn atom(pred: Predicate) -> Self {
        Formula::Atom(pred)
    }

    /// Create a negation
    #[allow(clippy::should_implement_trait)]
    pub fn not(f: Formula) -> Self {
        Formula::Not(Box::new(f))
    }

    /// Create a conjunction
    pub fn and(formulas: Vec<Formula>) -> Self {
        if formulas.is_empty() {
            Formula::True
        } else if formulas.len() == 1 {
            formulas.into_iter().next().unwrap()
        } else {
            Formula::And(formulas)
        }
    }

    /// Create a disjunction
    pub fn or(formulas: Vec<Formula>) -> Self {
        if formulas.is_empty() {
            Formula::False
        } else if formulas.len() == 1 {
            formulas.into_iter().next().unwrap()
        } else {
            Formula::Or(formulas)
        }
    }

    /// Create an implication
    pub fn implies(antecedent: Formula, consequent: Formula) -> Self {
        Formula::Implies(Box::new(antecedent), Box::new(consequent))
    }

    /// Create a universal quantification
    pub fn forall(variables: Vec<String>, formula: Formula) -> Self {
        Formula::ForAll {
            variables,
            formula: Box::new(formula),
        }
    }

    /// Create an existential quantification
    pub fn exists(variables: Vec<String>, formula: Formula) -> Self {
        Formula::Exists {
            variables,
            formula: Box::new(formula),
        }
    }

    /// Get all free variables
    pub fn free_variables(&self) -> HashSet<String> {
        match self {
            Formula::Atom(pred) => pred.variables(),
            Formula::Not(f) => f.free_variables(),
            Formula::And(fs) | Formula::Or(fs) => {
                fs.iter().flat_map(|f| f.free_variables()).collect()
            }
            Formula::Implies(a, b) | Formula::Iff(a, b) => {
                let mut vars = a.free_variables();
                vars.extend(b.free_variables());
                vars
            }
            Formula::ForAll { variables, formula } | Formula::Exists { variables, formula } => {
                let mut vars = formula.free_variables();
                for v in variables {
                    vars.remove(v);
                }
                vars
            }
            Formula::Equals(t1, t2) => {
                let mut vars = t1.variables();
                vars.extend(t2.variables());
                vars
            }
            Formula::True | Formula::False => HashSet::new(),
        }
    }

    /// Check if the formula is in negation normal form
    pub fn is_nnf(&self) -> bool {
        match self {
            Formula::Atom(_) | Formula::True | Formula::False | Formula::Equals(_, _) => true,
            Formula::Not(inner) => matches!(**inner, Formula::Atom(_)),
            Formula::And(fs) | Formula::Or(fs) => fs.iter().all(|f| f.is_nnf()),
            Formula::ForAll { formula, .. } | Formula::Exists { formula, .. } => formula.is_nnf(),
            _ => false,
        }
    }

    /// Convert to negation normal form
    pub fn to_nnf(&self) -> Formula {
        match self {
            Formula::Atom(_) | Formula::True | Formula::False | Formula::Equals(_, _) => {
                self.clone()
            }
            Formula::Not(inner) => {
                match &**inner {
                    Formula::Atom(_) | Formula::True | Formula::False | Formula::Equals(_, _) => {
                        self.clone()
                    }
                    Formula::Not(f) => f.to_nnf(),
                    Formula::And(fs) => Formula::Or(
                        fs.iter()
                            .map(|f| Formula::not(f.clone()).to_nnf())
                            .collect(),
                    ),
                    Formula::Or(fs) => Formula::And(
                        fs.iter()
                            .map(|f| Formula::not(f.clone()).to_nnf())
                            .collect(),
                    ),
                    Formula::Implies(a, b) => {
                        // ¬(A → B) ≡ A ∧ ¬B
                        Formula::And(vec![a.to_nnf(), Formula::not((**b).clone()).to_nnf()])
                    }
                    Formula::ForAll { variables, formula } => Formula::Exists {
                        variables: variables.clone(),
                        formula: Box::new(Formula::not((**formula).clone()).to_nnf()),
                    },
                    Formula::Exists { variables, formula } => Formula::ForAll {
                        variables: variables.clone(),
                        formula: Box::new(Formula::not((**formula).clone()).to_nnf()),
                    },
                    _ => self.clone(),
                }
            }
            Formula::And(fs) => Formula::And(fs.iter().map(|f| f.to_nnf()).collect()),
            Formula::Or(fs) => Formula::Or(fs.iter().map(|f| f.to_nnf()).collect()),
            Formula::Implies(a, b) => {
                // A → B ≡ ¬A ∨ B
                Formula::Or(vec![Formula::not((**a).clone()).to_nnf(), b.to_nnf()])
            }
            Formula::Iff(a, b) => {
                // A ↔ B ≡ (A → B) ∧ (B → A)
                let forward = Formula::implies((**a).clone(), (**b).clone());
                let backward = Formula::implies((**b).clone(), (**a).clone());
                Formula::and(vec![forward, backward]).to_nnf()
            }
            Formula::ForAll { variables, formula } => Formula::ForAll {
                variables: variables.clone(),
                formula: Box::new(formula.to_nnf()),
            },
            Formula::Exists { variables, formula } => Formula::Exists {
                variables: variables.clone(),
                formula: Box::new(formula.to_nnf()),
            },
        }
    }

    /// Apply substitution to formula
    pub fn apply_substitution(&self, subst: &HashMap<String, Term>) -> Formula {
        match self {
            Formula::Atom(pred) => Formula::Atom(pred.apply_substitution(subst)),
            Formula::Not(f) => Formula::Not(Box::new(f.apply_substitution(subst))),
            Formula::And(fs) => {
                Formula::And(fs.iter().map(|f| f.apply_substitution(subst)).collect())
            }
            Formula::Or(fs) => {
                Formula::Or(fs.iter().map(|f| f.apply_substitution(subst)).collect())
            }
            Formula::Implies(a, b) => Formula::Implies(
                Box::new(a.apply_substitution(subst)),
                Box::new(b.apply_substitution(subst)),
            ),
            Formula::Iff(a, b) => Formula::Iff(
                Box::new(a.apply_substitution(subst)),
                Box::new(b.apply_substitution(subst)),
            ),
            Formula::ForAll { variables, formula } => {
                let filtered: HashMap<_, _> = subst
                    .iter()
                    .filter(|(k, _)| !variables.contains(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                Formula::ForAll {
                    variables: variables.clone(),
                    formula: Box::new(formula.apply_substitution(&filtered)),
                }
            }
            Formula::Exists { variables, formula } => {
                let filtered: HashMap<_, _> = subst
                    .iter()
                    .filter(|(k, _)| !variables.contains(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                Formula::Exists {
                    variables: variables.clone(),
                    formula: Box::new(formula.apply_substitution(&filtered)),
                }
            }
            Formula::Equals(t1, t2) => {
                Formula::Equals(t1.apply_substitution(subst), t2.apply_substitution(subst))
            }
            Formula::True | Formula::False => self.clone(),
        }
    }
}

impl fmt::Display for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Formula::Atom(pred) => write!(f, "{}", pred),
            Formula::Not(inner) => write!(f, "¬{}", inner),
            Formula::And(fs) => {
                write!(f, "(")?;
                for (i, formula) in fs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ∧ ")?;
                    }
                    write!(f, "{}", formula)?;
                }
                write!(f, ")")
            }
            Formula::Or(fs) => {
                write!(f, "(")?;
                for (i, formula) in fs.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ∨ ")?;
                    }
                    write!(f, "{}", formula)?;
                }
                write!(f, ")")
            }
            Formula::Implies(a, b) => write!(f, "({} → {})", a, b),
            Formula::Iff(a, b) => write!(f, "({} ↔ {})", a, b),
            Formula::ForAll { variables, formula } => {
                write!(f, "∀{}.{}", variables.join(","), formula)
            }
            Formula::Exists { variables, formula } => {
                write!(f, "∃{}.{}", variables.join(","), formula)
            }
            Formula::Equals(t1, t2) => write!(f, "{} = {}", t1, t2),
            Formula::True => write!(f, "⊤"),
            Formula::False => write!(f, "⊥"),
        }
    }
}

/// A Horn clause (used in logic programming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clause {
    /// Unique identifier
    pub id: Uuid,

    /// Head (conclusion) - None for goals
    pub head: Option<Predicate>,

    /// Body (conditions)
    pub body: Vec<Predicate>,

    /// Source annotation
    pub source: Option<String>,
}

impl Clause {
    /// Create a fact (clause with no body)
    pub fn fact(head: Predicate) -> Self {
        Self {
            id: Uuid::new_v4(),
            head: Some(head),
            body: Vec::new(),
            source: None,
        }
    }

    /// Create a rule
    pub fn rule(head: Predicate, body: Vec<Predicate>) -> Self {
        Self {
            id: Uuid::new_v4(),
            head: Some(head),
            body,
            source: None,
        }
    }

    /// Create a goal (query)
    pub fn goal(body: Vec<Predicate>) -> Self {
        Self {
            id: Uuid::new_v4(),
            head: None,
            body,
            source: None,
        }
    }

    /// Check if this is a fact
    pub fn is_fact(&self) -> bool {
        self.head.is_some() && self.body.is_empty()
    }

    /// Check if this is a rule
    pub fn is_rule(&self) -> bool {
        self.head.is_some() && !self.body.is_empty()
    }

    /// Check if this is a goal
    pub fn is_goal(&self) -> bool {
        self.head.is_none()
    }

    /// Get all variables
    pub fn variables(&self) -> HashSet<String> {
        let mut vars = HashSet::new();
        if let Some(ref head) = self.head {
            vars.extend(head.variables());
        }
        for pred in &self.body {
            vars.extend(pred.variables());
        }
        vars
    }

    /// Apply substitution
    pub fn apply_substitution(&self, subst: &HashMap<String, Term>) -> Self {
        Clause {
            id: self.id,
            head: self.head.as_ref().map(|h| h.apply_substitution(subst)),
            body: self
                .body
                .iter()
                .map(|p| p.apply_substitution(subst))
                .collect(),
            source: self.source.clone(),
        }
    }

    /// Rename variables with fresh names
    pub fn rename_variables(&self, suffix: &str) -> Self {
        let vars = self.variables();
        let subst: HashMap<String, Term> = vars
            .into_iter()
            .map(|v| (v.clone(), Term::Variable(format!("{}_{}", v, suffix))))
            .collect();
        self.apply_substitution(&subst)
    }
}

impl fmt::Display for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.head {
            Some(head) => {
                write!(f, "{}", head)?;
                if !self.body.is_empty() {
                    write!(f, " :- ")?;
                    for (i, pred) in self.body.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", pred)?;
                    }
                }
                write!(f, ".")
            }
            None => {
                write!(f, "?- ")?;
                for (i, pred) in self.body.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", pred)?;
                }
                write!(f, ".")
            }
        }
    }
}

/// A knowledge base of clauses
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeBase {
    /// All clauses indexed by predicate name
    clauses: HashMap<String, Vec<Clause>>,

    /// All clauses in insertion order
    all_clauses: Vec<Clause>,
}

impl KnowledgeBase {
    /// Create a new empty knowledge base
    pub fn new() -> Self {
        Self {
            clauses: HashMap::new(),
            all_clauses: Vec::new(),
        }
    }

    /// Add a clause to the knowledge base
    pub fn add(&mut self, clause: Clause) {
        if let Some(ref head) = clause.head {
            self.clauses
                .entry(head.name.clone())
                .or_default()
                .push(clause.clone());
        }
        self.all_clauses.push(clause);
    }

    /// Add a fact
    pub fn add_fact(&mut self, name: impl Into<String>, args: Vec<Term>) {
        let pred = Predicate::new(name, args);
        self.add(Clause::fact(pred));
    }

    /// Add a rule
    pub fn add_rule(&mut self, head: Predicate, body: Vec<Predicate>) {
        self.add(Clause::rule(head, body));
    }

    /// Get clauses matching a predicate name
    pub fn get(&self, name: &str) -> &[Clause] {
        self.clauses.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get all clauses
    pub fn all(&self) -> &[Clause] {
        &self.all_clauses
    }

    /// Number of clauses
    pub fn len(&self) -> usize {
        self.all_clauses.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.all_clauses.is_empty()
    }

    /// Clear the knowledge base
    pub fn clear(&mut self) {
        self.clauses.clear();
        self.all_clauses.clear();
    }

    /// Get all predicate names
    pub fn predicate_names(&self) -> impl Iterator<Item = &String> {
        self.clauses.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_creation() {
        let x = Term::var("X");
        let const_a = Term::constant("a");
        let func = Term::func("f", vec![x.clone(), const_a.clone()]);

        assert!(x.is_variable());
        assert!(!const_a.is_variable());
        assert!(const_a.is_ground());
        assert!(!func.is_ground());
    }

    #[test]
    fn test_substitution() {
        let term = Term::func("f", vec![Term::var("X"), Term::var("Y")]);

        let mut subst = HashMap::new();
        subst.insert("X".to_string(), Term::constant("a"));

        let result = term.apply_substitution(&subst);

        match result {
            Term::Function { args, .. } => {
                assert_eq!(args[0], Term::constant("a"));
                assert_eq!(args[1], Term::var("Y"));
            }
            _ => panic!("Expected function"),
        }
    }

    #[test]
    fn test_predicate() {
        let pred = Predicate::new(
            "parent",
            vec![Term::constant("alice"), Term::constant("bob")],
        );

        assert_eq!(pred.arity(), 2);
        assert!(pred.is_ground());
        assert_eq!(format!("{}", pred), "parent(alice, bob)");
    }

    #[test]
    fn test_clause() {
        // parent(X, Y), parent(Y, Z) -> grandparent(X, Z)
        let head = Predicate::new("grandparent", vec![Term::var("X"), Term::var("Z")]);
        let body = vec![
            Predicate::new("parent", vec![Term::var("X"), Term::var("Y")]),
            Predicate::new("parent", vec![Term::var("Y"), Term::var("Z")]),
        ];

        let clause = Clause::rule(head, body);

        assert!(clause.is_rule());
        assert!(!clause.is_fact());
        assert_eq!(clause.variables().len(), 3);
    }

    #[test]
    fn test_knowledge_base() {
        let mut kb = KnowledgeBase::new();

        kb.add_fact(
            "parent",
            vec![Term::constant("alice"), Term::constant("bob")],
        );
        kb.add_fact(
            "parent",
            vec![Term::constant("bob"), Term::constant("charlie")],
        );

        assert_eq!(kb.len(), 2);
        assert_eq!(kb.get("parent").len(), 2);
        assert_eq!(kb.get("child").len(), 0);
    }

    #[test]
    fn test_formula_nnf() {
        // ¬(A ∧ B) should become ¬A ∨ ¬B
        let a = Formula::Atom(Predicate::new("A", vec![]));
        let b = Formula::Atom(Predicate::new("B", vec![]));
        let formula = Formula::not(Formula::and(vec![a, b]));

        let nnf = formula.to_nnf();

        match nnf {
            Formula::Or(fs) => {
                assert_eq!(fs.len(), 2);
                for f in fs {
                    assert!(matches!(f, Formula::Not(_)));
                }
            }
            _ => panic!("Expected disjunction"),
        }
    }
}
