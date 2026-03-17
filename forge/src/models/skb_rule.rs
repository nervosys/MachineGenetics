use serde::{Deserialize, Serialize};

/// A Safety Knowledge Base rule attached to a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkbRule {
    /// Rule identifier (e.g., "borrow:shared-iter", "lifetime:scoped-thread")
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Rule severity
    pub severity: SkbSeverity,
    /// Which module items this rule applies to
    pub applies_to: Vec<String>,
}

/// Severity levels for SKB rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkbSeverity {
    /// Informational — no enforcement
    Info,
    /// Warning — reported but not blocking
    Warning,
    /// Error — blocks compilation
    Error,
    /// Deny — blocks compilation and publish
    Deny,
}

impl SkbRule {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        severity: SkbSeverity,
    ) -> Self {
        SkbRule {
            id: id.into(),
            description: description.into(),
            severity,
            applies_to: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_skb_rule() {
        let rule = SkbRule::new(
            "borrow:shared-iter",
            "Shared iteration requires immutable borrow",
            SkbSeverity::Error,
        );
        assert_eq!(rule.id, "borrow:shared-iter");
        assert!(matches!(rule.severity, SkbSeverity::Error));
    }
}
