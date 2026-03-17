use serde::{Deserialize, Serialize};

/// A specification (contract) block for a published API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecBlock {
    /// The function or method this spec applies to
    pub target: String,
    /// Preconditions (requires clauses)
    pub requires: Vec<String>,
    /// Postconditions (ensures clauses)
    pub ensures: Vec<String>,
    /// Invariants maintained
    pub invariants: Vec<String>,
}

impl SpecBlock {
    pub fn new(target: impl Into<String>) -> Self {
        SpecBlock {
            target: target.into(),
            requires: Vec::new(),
            ensures: Vec::new(),
            invariants: Vec::new(),
        }
    }

    pub fn require(mut self, condition: impl Into<String>) -> Self {
        self.requires.push(condition.into());
        self
    }

    pub fn ensure(mut self, condition: impl Into<String>) -> Self {
        self.ensures.push(condition.into());
        self
    }

    pub fn invariant(mut self, condition: impl Into<String>) -> Self {
        self.invariants.push(condition.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_block_builder() {
        let spec = SpecBlock::new("sort")
            .require("data.len() > 0")
            .ensure("data is sorted ascending")
            .ensure("data.len() unchanged");
        assert_eq!(spec.target, "sort");
        assert_eq!(spec.requires.len(), 1);
        assert_eq!(spec.ensures.len(), 2);
    }
}
