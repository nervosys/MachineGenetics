//! # std::skb — Safety Knowledge Base
//!
//! A rule-based validation engine for safety verification of code,
//! configurations, and agent behavior.

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// Severity level for a rule.
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A validation rule in the knowledge base.
pub struct Rule {
    id: String,
    name: String,
    description: String,
    severity: Severity,
    category: String,
    check: fn(&Query) -> bool,
}

impl Rule {
    /// Create a new rule.
    pub fn new(
        id: &String,
        name: &String,
        desc: &String,
        severity: Severity,
        category: &String,
        check: fn(&Query) -> bool,
    ) -> Rule {
        Rule {
            id: id.to_owned(),
            name: name.to_owned(),
            description: desc.to_owned(),
            severity,
            category: category.to_owned(),
            check,
        }
    }
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// A query context for rules to evaluate against.
pub struct Query {
    pub source: String,
    pub metadata: HashMap<String, String>,
}

impl Query {
    pub fn new(source: &String) -> Query {
        Query { source: source.to_owned(), metadata: HashMap::new() }
    }

    pub fn with_meta(&mut self, key: &String, value: &String) -> &mut Query {
        self.metadata.insert(key.to_owned(), value.to_owned());
        self
    }
}

// ---------------------------------------------------------------------------
// Validation results
// ---------------------------------------------------------------------------

/// Record of a single rule violation.
pub struct Violation {
    pub rule_id: String,
    pub rule_name: String,
    pub severity: Severity,
    pub message: String,
    pub location: Option<String>,
}

/// Results of validating against a set of rules.
pub struct ValidationResult {
    pub violations: Vec<Violation>,
    pub rules_checked: usize,
    pub passed: bool,
}

impl ValidationResult {
    /// How many errors?
    pub fn error_count(&self) -> usize {
        self.violations.iter().filter(|v| match v.severity { Severity::Error => true, _ => false }).count()
    }

    /// How many warnings?
    pub fn warning_count(&self) -> usize {
        self.violations.iter().filter(|v| match v.severity { Severity::Warning => true, _ => false }).count()
    }
}

// ---------------------------------------------------------------------------
// Knowledge Base
// ---------------------------------------------------------------------------

/// The safety knowledge base: a collection of rules.
pub struct SafetyKB {
    rules: Vec<Rule>,
}

impl SafetyKB {
    /// Create an empty knowledge base.
    pub fn new() -> SafetyKB { SafetyKB { rules: Vec::new() } }

    /// Add a rule.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Validate a query against all rules.
    pub fn validate(&self, query: &Query) -> ValidationResult {
        let mut violations = Vec::new();
        for rule in &self.rules {
            if !(rule.check)(query) {
                violations.push(Violation {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity,
                    message: rule.description.clone(),
                    location: None,
                });
            }
        }
        let passed = violations.iter().all(|v| match v.severity { Severity::Error => false, _ => true });
        ValidationResult {
            violations,
            rules_checked: self.rules.len(),
            passed,
        }
    }

    /// Validate and filter by category.
    pub fn validate_category(&self, query: &Query, category: &String) -> ValidationResult {
        let filtered: Vec<&Rule> = self.rules.iter().filter(|r| &r.category == category).collect();
        let mut violations = Vec::new();
        for rule in &filtered {
            if !(rule.check)(query) {
                violations.push(Violation {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity,
                    message: rule.description.clone(),
                    location: None,
                });
            }
        }
        let passed = violations.iter().all(|v| match v.severity { Severity::Error => false, _ => true });
        ValidationResult {
            violations,
            rules_checked: filtered.len(),
            passed,
        }
    }
}
