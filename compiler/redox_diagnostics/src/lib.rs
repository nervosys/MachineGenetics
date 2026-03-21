//! # Structured Diagnostics Protocol (SDP)
//!
//! Provides structured, machine-readable diagnostic output as JSON diagnostic graphs.
//! Each diagnostic graph contains an error root node, cause chain, fix candidates
//! with token cost and confidence, and related locations.
//!
//! Reference: REDOX_PROPOSAL.md §6.2

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A span in source code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub file: String,
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
}

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}

/// Safety category for the diagnostic (optional classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyCategory {
    BorrowConflict,
    LifetimeError,
    TypeMismatch,
    MoveError,
    UnsafeUsage,
    UnusedBinding,
    MissingImpl,
    Syntax,
    Other,
}

/// Applicability of a suggested fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Applicability {
    /// Fix is guaranteed to be correct.
    MachineApplicable,
    /// Fix might be incorrect.
    MaybeIncorrect,
    /// Fix has placeholders that need user input.
    HasPlaceholders,
    /// Fix is unspecified/unknown applicability.
    Unspecified,
}

// ---------------------------------------------------------------------------
// Diagnostic graph components
// ---------------------------------------------------------------------------

/// The root diagnostic — the primary error or warning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: String,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<SafetyCategory>,
}

/// A contextual note within the diagnostic chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticNode {
    pub kind: Severity,
    pub message: String,
    pub span: Span,
}

/// A source edit within a fix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
}

/// A proposed fix for a diagnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fix {
    pub description: String,
    pub applicability: Applicability,
    pub edits: Vec<Edit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub postconditions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<String>,
    /// Confidence in [0.0, 1.0].
    pub confidence: f64,
    /// Estimated token cost of applying this fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_cost: Option<u32>,
}

/// A complete diagnostic graph: root + context + fixes + related IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticGraph {
    pub root: Diagnostic,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<DiagnosticNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Builder API
// ---------------------------------------------------------------------------

impl DiagnosticGraph {
    /// Create a new diagnostic graph from a root diagnostic.
    pub fn new(root: Diagnostic) -> Self {
        Self {
            root,
            context: Vec::new(),
            fixes: Vec::new(),
            related: Vec::new(),
            documentation_url: None,
        }
    }

    /// Add a context node.
    pub fn with_context(mut self, node: DiagnosticNode) -> Self {
        self.context.push(node);
        self
    }

    /// Add a fix candidate.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fixes.push(fix);
        self
    }

    /// Add a related error ID.
    pub fn with_related(mut self, id: impl Into<String>) -> Self {
        self.related.push(id.into());
        self
    }

    /// Set the documentation URL.
    pub fn with_doc_url(mut self, url: impl Into<String>) -> Self {
        self.documentation_url = Some(url.into());
        self
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Span {
    pub fn new(file: impl Into<String>, line: u32, col_start: u32, col_end: u32) -> Self {
        Self {
            file: file.into(),
            line,
            col_start,
            col_end,
        }
    }
}

impl Diagnostic {
    pub fn error(id: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            id: id.into(),
            severity: Severity::Error,
            message: message.into(),
            span,
            category: None,
        }
    }

    pub fn warning(id: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            id: id.into(),
            severity: Severity::Warning,
            message: message.into(),
            span,
            category: None,
        }
    }

    pub fn with_category(mut self, cat: SafetyCategory) -> Self {
        self.category = Some(cat);
        self
    }
}

impl Fix {
    pub fn new(description: impl Into<String>, applicability: Applicability, confidence: f64) -> Self {
        Self {
            description: description.into(),
            applicability,
            edits: Vec::new(),
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            side_effects: Vec::new(),
            confidence,
            token_cost: None,
        }
    }

    pub fn with_edit(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn with_precondition(mut self, pre: impl Into<String>) -> Self {
        self.preconditions.push(pre.into());
        self
    }

    pub fn with_postcondition(mut self, post: impl Into<String>) -> Self {
        self.postconditions.push(post.into());
        self
    }

    pub fn with_side_effect(mut self, effect: impl Into<String>) -> Self {
        self.side_effects.push(effect.into());
        self
    }

    pub fn with_token_cost(mut self, cost: u32) -> Self {
        self.token_cost = Some(cost);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_borrow_diagnostic() -> DiagnosticGraph {
        let root = Diagnostic::error(
            "E0502",
            "cannot borrow `x` as mutable because it is also borrowed as immutable",
            Span::new("src/main.rs", 10, 5, 12),
        )
        .with_category(SafetyCategory::BorrowConflict);

        let context = DiagnosticNode {
            kind: Severity::Note,
            message: "immutable borrow occurs here".to_string(),
            span: Span::new("src/main.rs", 8, 9, 15),
        };

        let fix1 = Fix::new(
            "Clone the value before mutating",
            Applicability::MaybeIncorrect,
            0.7,
        )
        .with_edit(Edit {
            span: Span::new("src/main.rs", 9, 1, 1),
            replacement: "let x_clone = x.clone();\n".to_string(),
        })
        .with_precondition("T: Clone")
        .with_postcondition("No borrow conflict")
        .with_postcondition("Possible performance regression")
        .with_side_effect("Introduces heap allocation if T contains Box/Vec/String")
        .with_token_cost(8);

        let fix2 = Fix::new(
            "Restructure to separate immutable and mutable uses",
            Applicability::HasPlaceholders,
            0.9,
        )
        .with_postcondition("No borrow conflict")
        .with_postcondition("No performance regression")
        .with_token_cost(15);

        DiagnosticGraph::new(root)
            .with_context(context)
            .with_fix(fix1)
            .with_fix(fix2)
            .with_related("E0499")
            .with_related("E0503")
            .with_doc_url("https://doc.redox-lang.org/error/E0502")
    }

    #[test]
    fn build_diagnostic_graph() {
        let graph = sample_borrow_diagnostic();
        assert_eq!(graph.root.id, "E0502");
        assert_eq!(graph.root.severity, Severity::Error);
        assert_eq!(graph.root.category, Some(SafetyCategory::BorrowConflict));
        assert_eq!(graph.context.len(), 1);
        assert_eq!(graph.fixes.len(), 2);
        assert_eq!(graph.related, vec!["E0499", "E0503"]);
    }

    #[test]
    fn fix_confidence_ordering() {
        let graph = sample_borrow_diagnostic();
        assert!(graph.fixes[0].confidence < graph.fixes[1].confidence);
    }

    #[test]
    fn fix_has_preconditions() {
        let graph = sample_borrow_diagnostic();
        assert_eq!(graph.fixes[0].preconditions, vec!["T: Clone"]);
        assert!(graph.fixes[1].preconditions.is_empty());
    }

    #[test]
    fn fix_has_token_cost() {
        let graph = sample_borrow_diagnostic();
        assert_eq!(graph.fixes[0].token_cost, Some(8));
        assert_eq!(graph.fixes[1].token_cost, Some(15));
    }

    #[test]
    fn serialize_to_json() {
        let graph = sample_borrow_diagnostic();
        let json = graph.to_json().unwrap();
        assert!(json.contains("\"E0502\""));
        assert!(json.contains("cannot borrow"));
        assert!(json.contains("Clone the value"));
        assert!(json.contains("\"confidence\": 0.7"));
    }

    #[test]
    fn deserialize_from_json() {
        let graph = sample_borrow_diagnostic();
        let json = graph.to_json().unwrap();
        let restored = DiagnosticGraph::from_json(&json).unwrap();
        assert_eq!(graph, restored);
    }

    #[test]
    fn roundtrip_json() {
        let graph = sample_borrow_diagnostic();
        let json1 = graph.to_json().unwrap();
        let restored = DiagnosticGraph::from_json(&json1).unwrap();
        let json2 = restored.to_json().unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn minimal_diagnostic() {
        let graph = DiagnosticGraph::new(Diagnostic::error(
            "E0001",
            "syntax error",
            Span::new("test.rs", 1, 1, 5),
        ));
        let json = graph.to_json().unwrap();
        assert!(json.contains("\"E0001\""));
        // Minimal: no context, fixes, or related
        assert!(!json.contains("context"));
        assert!(!json.contains("fixes"));
    }

    #[test]
    fn warning_diagnostic() {
        let graph = DiagnosticGraph::new(
            Diagnostic::warning("unused_var", "unused variable: `x`", Span::new("lib.rs", 5, 9, 10))
                .with_category(SafetyCategory::UnusedBinding),
        );
        assert_eq!(graph.root.severity, Severity::Warning);
        assert_eq!(graph.root.category, Some(SafetyCategory::UnusedBinding));
    }

    #[test]
    fn type_mismatch_with_fix() {
        let root = Diagnostic::error(
            "E0308",
            "mismatched types: expected `u32`, found `i32`",
            Span::new("src/calc.rs", 42, 12, 20),
        )
        .with_category(SafetyCategory::TypeMismatch);

        let fix = Fix::new("Add explicit cast", Applicability::MachineApplicable, 0.95)
            .with_edit(Edit {
                span: Span::new("src/calc.rs", 42, 12, 20),
                replacement: "value as u32".to_string(),
            })
            .with_postcondition("Types match")
            .with_side_effect("Possible truncation if value > u32::MAX")
            .with_token_cost(3);

        let graph = DiagnosticGraph::new(root).with_fix(fix);

        assert_eq!(graph.fixes[0].applicability, Applicability::MachineApplicable);
        assert_eq!(graph.fixes[0].confidence, 0.95);
        assert_eq!(graph.fixes[0].token_cost, Some(3));
    }

    #[test]
    fn json_contains_all_fields() {
        let graph = sample_borrow_diagnostic();
        let json = graph.to_json().unwrap();

        // Root fields
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"severity\""));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"span\""));
        assert!(json.contains("\"category\""));

        // Span fields
        assert!(json.contains("\"file\""));
        assert!(json.contains("\"line\""));
        assert!(json.contains("\"col_start\""));
        assert!(json.contains("\"col_end\""));

        // Fix fields
        assert!(json.contains("\"description\""));
        assert!(json.contains("\"applicability\""));
        assert!(json.contains("\"confidence\""));
        assert!(json.contains("\"preconditions\""));
        assert!(json.contains("\"postconditions\""));
        assert!(json.contains("\"side_effects\""));
        assert!(json.contains("\"token_cost\""));

        // Graph fields
        assert!(json.contains("\"context\""));
        assert!(json.contains("\"fixes\""));
        assert!(json.contains("\"related\""));
        assert!(json.contains("\"documentation_url\""));
    }

    #[test]
    fn multiple_edits_in_fix() {
        let fix = Fix::new("Split borrow scope", Applicability::MaybeIncorrect, 0.8)
            .with_edit(Edit {
                span: Span::new("src/main.rs", 8, 1, 1),
                replacement: "{\n".to_string(),
            })
            .with_edit(Edit {
                span: Span::new("src/main.rs", 9, 1, 1),
                replacement: "}\n".to_string(),
            });

        assert_eq!(fix.edits.len(), 2);
    }

    #[test]
    fn diagnostic_graph_doc_url() {
        let graph = sample_borrow_diagnostic();
        assert_eq!(
            graph.documentation_url.as_deref(),
            Some("https://doc.redox-lang.org/error/E0502")
        );
    }
}
