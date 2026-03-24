//! # std::kb — Knowledge Base and Symbolic Reasoning
//!
//! First-class support for knowledge bases with facts, rules,
//! and logical inference. Knowledge bases can be defined with the
//! `kb` keyword and queried at compile time or runtime.

// ---------------------------------------------------------------------------
// KnowledgeBase type
// ---------------------------------------------------------------------------

/// A queryable collection of facts and rules.
pub struct KnowledgeBase {
    facts: Vec<Fact>,
    rules: Vec<Rule>,
}

/// A ground truth assertion.
pub struct Fact {
    pub name: String,
    pub args: Vec<Value>,
}

/// An inference rule: `head :- body`.
pub struct Rule {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<RuleTerm>,
}

/// A term in a rule body.
pub enum RuleTerm {
    /// Positive goal: `predicate(args)`.
    Goal { name: String, args: Vec<Value> },
    /// Negated goal: `!predicate(args)`.
    Negation { name: String, args: Vec<Value> },
    /// Comparison: `expr op expr`.
    Comparison { left: Value, op: CmpOp, right: Value },
}

/// A value in the knowledge base.
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Var(String),
}

/// Comparison operators for rules.
pub enum CmpOp {
    Eq, Neq, Lt, Gt, Le, Ge,
}

// ---------------------------------------------------------------------------
// KnowledgeBase operations
// ---------------------------------------------------------------------------

impl KnowledgeBase {
    /// Create a new empty knowledge base.
    pub fn new() -> KnowledgeBase;

    /// Assert a fact.
    pub fn assert_fact(&mut self, name: &str, args: &[Value]);

    /// Add a rule.
    pub fn add_rule(&mut self, rule: Rule);

    /// Query the knowledge base. Returns all matching bindings.
    pub fn query(&self, name: &str, args: &[&str]) -> Vec<Vec<Value>>;

    /// Check if a query has at least one solution.
    pub fn holds(&self, name: &str, args: &[&str]) -> bool;

    /// Retract (remove) a fact.
    pub fn retract(&mut self, name: &str, args: &[Value]) -> bool;

    /// Number of facts.
    pub fn fact_count(&self) -> usize;

    /// Number of rules.
    pub fn rule_count(&self) -> usize;

    /// Merge another knowledge base into this one.
    pub fn merge(&mut self, other: &KnowledgeBase);
}

// ---------------------------------------------------------------------------
// Query builder (fluent API)
// ---------------------------------------------------------------------------

/// Fluent API for building KB queries.
pub struct QueryBuilder<'a> {
    kb: &'a KnowledgeBase,
    predicate: String,
    bindings: Vec<Value>,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(kb: &'a KnowledgeBase, predicate: &str) -> QueryBuilder<'a>;
    pub fn bind(mut self, value: Value) -> QueryBuilder<'a>;
    pub fn run(&self) -> Vec<Vec<Value>>;
    pub fn exists(&self) -> bool;
    pub fn first(&self) -> Option<Vec<Value>>;
}
