//! # std::spec — Specification & Contracts
//!
//! Formal specification primitives: preconditions, postconditions,
//! invariants, and verification.

// ---------------------------------------------------------------------------
// Contract functions
// ---------------------------------------------------------------------------

/// Assert a precondition. Panics at runtime if `cond` is false.
/// In verification mode, the solver checks this statically.
pub fn require(cond: bool, msg: &String);

/// Assert a postcondition. Panics at runtime if `cond` is false.
pub fn ensure(cond: bool, msg: &String);

/// Assert a class/loop invariant. Panics at runtime if `cond` is false.
pub fn invariant(cond: bool, msg: &String);

/// Mark a code path as unreachable. Panics if reached at runtime.
pub fn unreachable(msg: &String) -> !;

// ---------------------------------------------------------------------------
// Specification blocks
// ---------------------------------------------------------------------------

/// A specification block that groups related contracts.
pub struct SpecBlock {
    name: String,
    preconditions: Vec<SpecClause>,
    postconditions: Vec<SpecClause>,
    invariants: Vec<SpecClause>,
}

pub struct SpecClause {
    description: String,
    check: fn() -> bool,
}

impl SpecBlock {
    /// Create a named specification block.
    pub fn new(name: &String) -> SpecBlock {
        SpecBlock {
            name: name.to_owned(),
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            invariants: Vec::new(),
        }
    }

    /// Add a precondition.
    pub fn pre(&mut self, desc: &String, check: fn() -> bool) -> &mut SpecBlock {
        self.preconditions.push(SpecClause { description: desc.to_owned(), check });
        self
    }

    /// Add a postcondition.
    pub fn post(&mut self, desc: &String, check: fn() -> bool) -> &mut SpecBlock {
        self.postconditions.push(SpecClause { description: desc.to_owned(), check });
        self
    }

    /// Add an invariant.
    pub fn inv(&mut self, desc: &String, check: fn() -> bool) -> &mut SpecBlock {
        self.invariants.push(SpecClause { description: desc.to_owned(), check });
        self
    }

    /// Verify all clauses. Returns a report.
    pub fn verify(&self) -> VerifyReport {
        let mut results = Vec::new();

        for clause in &self.preconditions {
            results.push(SpecResult {
                kind: SpecKind::Precondition,
                description: clause.description.clone(),
                passed: (clause.check)(),
            });
        }

        for clause in &self.postconditions {
            results.push(SpecResult {
                kind: SpecKind::Postcondition,
                description: clause.description.clone(),
                passed: (clause.check)(),
            });
        }

        for clause in &self.invariants {
            results.push(SpecResult {
                kind: SpecKind::Invariant,
                description: clause.description.clone(),
                passed: (clause.check)(),
            });
        }

        let all_passed = results.iter().all(|r| r.passed);
        VerifyReport { name: self.name.clone(), results, passed: all_passed }
    }
}

// ---------------------------------------------------------------------------
// Verification report
// ---------------------------------------------------------------------------

pub enum SpecKind {
    Precondition,
    Postcondition,
    Invariant,
}

pub struct SpecResult {
    kind: SpecKind,
    description: String,
    passed: bool,
}

pub struct VerifyReport {
    name: String,
    results: Vec<SpecResult>,
    passed: bool,
}

impl VerifyReport {
    pub fn is_ok(&self) -> bool { self.passed }
    pub fn failures(&self) -> Vec<&SpecResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }
}
