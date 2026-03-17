/// Self-Healing Compiler — generates ranked fix candidates for errors.
///
/// Implements Proposal P22: when an agent emits invalid code, the compiler
/// attempts to repair it. Recovery strategies are ranked by confidence.
///
/// The self-healing pipeline:
///   1. Receive a diagnostic (parse error, type error, effect mismatch)
///   2. Match the error against known error patterns
///   3. Generate one or more fix candidates with confidence scores
///   4. Return fixes alongside the original diagnostic
///
/// Agents can accept, reject, or refine — the compiler never silently
/// changes semantics.
use serde::{Deserialize, Serialize};

use crate::hir::{Applicability, Diagnostic, DiagnosticCategory, DiagnosticGraph, Fix, Severity};

// ── Fix Candidate ────────────────────────────────────────────────────

/// A proposed fix for a compiler diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixCandidate {
    /// Unique fix identifier (e.g., "add-missing-return").
    pub id: String,
    /// Human-/agent-readable description of what the fix does.
    pub description: String,
    /// The text edits that implement this fix.
    pub edits: Vec<TextEdit>,
    /// Confidence score: 0.0 (wild guess) to 1.0 (certain).
    pub confidence: f64,
    /// Whether applying this fix preserves program semantics.
    pub semantics_preserving: bool,
    /// Estimated token cost of applying this fix (for agent budgeting).
    pub token_cost: u32,
}

/// A textual replacement at a source location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    /// Start line (1-based).
    pub start_line: u32,
    /// Start column (1-based).
    pub start_col: u32,
    /// End line (1-based, inclusive).
    pub end_line: u32,
    /// End column (1-based, inclusive).
    pub end_col: u32,
    /// Replacement text.
    pub new_text: String,
}

/// A diagnostic enriched with auto-repair candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealedDiagnostic {
    /// The original diagnostic.
    pub diagnostic: Diagnostic,
    /// Ranked list of fix candidates (best first).
    pub fixes: Vec<FixCandidate>,
}

// ── Error Pattern Registry ───────────────────────────────────────────

/// Known error patterns and their fix generators.
struct ErrorPattern {
    /// Pattern name (for logging/debugging).
    name: &'static str,
    /// Returns true if this pattern matches the diagnostic message.
    matches: fn(&str) -> bool,
    /// Given the diagnostic, produce fix candidates.
    generate: fn(&Diagnostic) -> Vec<FixCandidate>,
}

/// The built-in pattern registry.
fn builtin_patterns() -> Vec<ErrorPattern> {
    vec![
        ErrorPattern {
            name: "missing-return-type",
            matches: |msg| msg.contains("expected return type") || msg.contains("missing return"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "add-unit-return".into(),
                    description: "Add explicit `()` return type".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: " -> ()".into(),
                    }],
                    confidence: 0.7,
                    semantics_preserving: true,
                    token_cost: 3,
                }]
            },
        },
        ErrorPattern {
            name: "unexpected-token",
            matches: |msg| msg.contains("unexpected token") || msg.contains("expected"),
            generate: |diag| {
                let mut fixes = Vec::new();
                // Common fixes: missing semicolon, missing brace, missing paren
                if diag.message.contains("`}`") || diag.message.contains("'}'") {
                    fixes.push(FixCandidate {
                        id: "insert-closing-brace".into(),
                        description: "Insert missing `}`".into(),
                        edits: vec![TextEdit {
                            start_line: diag.span.map(|s| s.line).unwrap_or(1),
                            start_col: diag.span.map(|s| s.col).unwrap_or(1),
                            end_line: diag.span.map(|s| s.line).unwrap_or(1),
                            end_col: diag.span.map(|s| s.col).unwrap_or(1),
                            new_text: "}".into(),
                        }],
                        confidence: 0.8,
                        semantics_preserving: true,
                        token_cost: 1,
                    });
                }
                if diag.message.contains("`;`") || diag.message.contains("';'") {
                    fixes.push(FixCandidate {
                        id: "insert-semicolon".into(),
                        description: "Insert missing `;`".into(),
                        edits: vec![TextEdit {
                            start_line: diag.span.map(|s| s.line).unwrap_or(1),
                            start_col: diag.span.map(|s| s.col).unwrap_or(1),
                            end_line: diag.span.map(|s| s.line).unwrap_or(1),
                            end_col: diag.span.map(|s| s.col).unwrap_or(1),
                            new_text: ";".into(),
                        }],
                        confidence: 0.85,
                        semantics_preserving: true,
                        token_cost: 1,
                    });
                }
                fixes
            },
        },
        ErrorPattern {
            name: "undeclared-effect",
            matches: |msg| {
                msg.contains("effect")
                    && (msg.contains("not declared") || msg.contains("undeclared"))
            },
            generate: |diag| {
                // Extract effect name from the message if possible.
                let effect = extract_quoted(&diag.message).unwrap_or("io".to_string());
                vec![FixCandidate {
                    id: "add-effect-annotation".into(),
                    description: format!(
                        "Add `/ {effect}` effect annotation to function signature"
                    ),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: format!(" / {effect}"),
                    }],
                    confidence: 0.75,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },
        ErrorPattern {
            name: "type-mismatch",
            matches: |msg| msg.contains("type mismatch") || msg.contains("mismatched types"),
            generate: |diag| {
                let mut fixes = Vec::new();
                // Suggest wrapping in Option if expected ?T
                if diag.message.contains("Option") || diag.message.contains("?") {
                    fixes.push(FixCandidate {
                        id: "wrap-in-some".into(),
                        description: "Wrap value in `Some(...)`".into(),
                        edits: vec![], // Position-dependent; needs source context
                        confidence: 0.5,
                        semantics_preserving: false,
                        token_cost: 3,
                    });
                }
                // Suggest wrapping in Ok if expected R[T, E]
                if diag.message.contains("Result") || diag.message.contains("R[") {
                    fixes.push(FixCandidate {
                        id: "wrap-in-ok".into(),
                        description: "Wrap value in `Ok(...)`".into(),
                        edits: vec![],
                        confidence: 0.5,
                        semantics_preserving: false,
                        token_cost: 3,
                    });
                }
                fixes
            },
        },
        ErrorPattern {
            name: "unknown-identifier",
            matches: |msg| {
                msg.contains("cannot find")
                    || msg.contains("not found")
                    || msg.contains("undefined")
            },
            generate: |diag| {
                let name = extract_quoted(&diag.message).unwrap_or_default();
                let mut fixes = Vec::new();
                if !name.is_empty() {
                    fixes.push(FixCandidate {
                        id: "add-use-import".into(),
                        description: format!("Add `u {name}` import"),
                        edits: vec![TextEdit {
                            start_line: 1,
                            start_col: 1,
                            end_line: 1,
                            end_col: 1,
                            new_text: format!("u {name}\n"),
                        }],
                        confidence: 0.6,
                        semantics_preserving: true,
                        token_cost: 2,
                    });
                }
                fixes
            },
        },
        ErrorPattern {
            name: "spec-violation",
            matches: |msg| {
                msg.contains("spec") && (msg.contains("violated") || msg.contains("unsatisfied"))
            },
            generate: |_diag| {
                vec![FixCandidate {
                    id: "add-boundary-check".into(),
                    description: "Add boundary check to satisfy spec precondition".into(),
                    edits: vec![],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 5,
                }]
            },
        },
    ]
}

/// Extract the first single-quoted or backtick-quoted token from a message.
fn extract_quoted(msg: &str) -> Option<String> {
    // Try backtick quotes first: `name`
    if let Some(start) = msg.find('`') {
        if let Some(end) = msg[start + 1..].find('`') {
            return Some(msg[start + 1..start + 1 + end].to_string());
        }
    }
    // Try single quotes: 'name'
    if let Some(start) = msg.find('\'') {
        if let Some(end) = msg[start + 1..].find('\'') {
            return Some(msg[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

// ── Healing Engine ───────────────────────────────────────────────────

/// Attempt to heal a list of diagnostics by generating fix candidates.
pub fn heal(diagnostics: &[Diagnostic]) -> Vec<HealedDiagnostic> {
    let patterns = builtin_patterns();

    diagnostics
        .iter()
        .map(|diag| {
            let mut fixes: Vec<FixCandidate> = Vec::new();

            if diag.severity == Severity::Error || diag.severity == Severity::Warning {
                for pattern in &patterns {
                    if (pattern.matches)(&diag.message) {
                        fixes.extend((pattern.generate)(diag));
                    }
                }
            }

            // Sort by confidence descending.
            fixes.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

            HealedDiagnostic { diagnostic: diag.clone(), fixes }
        })
        .collect()
}

/// Heal a single diagnostic.
pub fn heal_one(diag: &Diagnostic) -> HealedDiagnostic {
    heal(std::slice::from_ref(diag)).into_iter().next().unwrap()
}

// ── DiagnosticGraph integration ──────────────────────────────────────

/// Infer a `DiagnosticCategory` from the message text.
fn infer_category(msg: &str) -> DiagnosticCategory {
    if msg.contains("borrow") || msg.contains("move") {
        DiagnosticCategory::BorrowConflict
    } else if msg.contains("type mismatch") || msg.contains("mismatched types") {
        DiagnosticCategory::TypeMismatch
    } else if msg.contains("unresolved name") || msg.contains("not found") {
        DiagnosticCategory::UnresolvedName
    } else if msg.contains("unresolved type") {
        DiagnosticCategory::UnresolvedType
    } else if msg.contains("effect") && msg.contains("undeclared") {
        DiagnosticCategory::UndeclaredEffect
    } else if msg.contains("duplicate") {
        DiagnosticCategory::DuplicateDefinition
    } else if msg.contains("spec") {
        DiagnosticCategory::SpecViolation
    } else if msg.contains("expected") || msg.contains("unexpected") {
        DiagnosticCategory::SyntaxError
    } else {
        DiagnosticCategory::Other
    }
}

/// Convert a `FixCandidate` into the richer `Fix` type.
fn fix_candidate_to_fix(fc: &FixCandidate) -> Fix {
    Fix {
        description: fc.description.clone(),
        applicability: if fc.semantics_preserving {
            Applicability::MachineApplicable
        } else if fc.confidence >= 0.6 {
            Applicability::MaybeIncorrect
        } else {
            Applicability::HasPlaceholders
        },
        preconditions: Vec::new(),
        postconditions: Vec::new(),
        side_effects: if fc.semantics_preserving {
            Vec::new()
        } else {
            vec!["May change program semantics".into()]
        },
        confidence: fc.confidence,
    }
}

/// Convert a `HealedDiagnostic` into a full `DiagnosticGraph`.
pub fn healed_to_graph(hd: &HealedDiagnostic) -> DiagnosticGraph {
    let category = hd.diagnostic.category.unwrap_or_else(|| infer_category(&hd.diagnostic.message));

    let mut root = hd.diagnostic.clone();
    if root.category.is_none() {
        root.category = Some(category);
    }

    DiagnosticGraph {
        root,
        context: Vec::new(),
        fixes: hd.fixes.iter().map(fix_candidate_to_fix).collect(),
        related: Vec::new(),
        documentation_url: None,
    }
}

/// Heal diagnostics and produce full `DiagnosticGraph` objects.
pub fn heal_to_graphs(diagnostics: &[Diagnostic]) -> Vec<DiagnosticGraph> {
    heal(diagnostics).iter().map(healed_to_graph).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::Span;

    fn error_with_span(msg: &str, line: u32, col: u32) -> Diagnostic {
        let mut d = Diagnostic::error(msg);
        d.span = Some(Span { line, col });
        d
    }

    #[test]
    fn heals_missing_semicolon() {
        let diag = error_with_span("expected `;` after expression", 5, 10);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "insert-semicolon");
        assert!(healed.fixes[0].confidence > 0.8);
    }

    #[test]
    fn heals_undeclared_effect() {
        let diag = error_with_span("effect `io` not declared on function `read_file`", 3, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "add-effect-annotation");
        assert!(healed.fixes[0].description.contains("io"));
    }

    #[test]
    fn heals_unknown_identifier() {
        let diag = error_with_span("cannot find `HashMap` in this scope", 1, 5);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "add-use-import");
    }

    #[test]
    fn info_diagnostics_not_healed() {
        let mut diag = Diagnostic::error("unused variable `x`");
        diag.severity = Severity::Info;
        let healed = heal_one(&diag);
        assert!(healed.fixes.is_empty());
    }

    #[test]
    fn fixes_sorted_by_confidence() {
        let diag = error_with_span("expected `}` or `;` after expression", 10, 20);
        let healed = heal_one(&diag);
        assert!(healed.fixes.len() >= 2);
        for w in healed.fixes.windows(2) {
            assert!(w[0].confidence >= w[1].confidence);
        }
    }

    #[test]
    fn extract_quoted_backtick() {
        assert_eq!(extract_quoted("cannot find `Foo` in scope"), Some("Foo".to_string()));
    }

    #[test]
    fn extract_quoted_single_quote() {
        assert_eq!(extract_quoted("expected ';' after"), Some(";".to_string()));
    }

    #[test]
    fn heal_to_graph_produces_category() {
        let diag = Diagnostic::error("type mismatch: expected ?i32, found str");
        let graphs = heal_to_graphs(&[diag]);
        assert_eq!(graphs.len(), 1);
        assert_eq!(graphs[0].root.category, Some(DiagnosticCategory::TypeMismatch));
        // The type-mismatch pattern generates a "wrap-in-some" fix for Option types
        assert!(!graphs[0].fixes.is_empty());
    }

    #[test]
    fn heal_to_graph_borrow_conflict() {
        let diag = Diagnostic::error(
            "cannot borrow `x` as mutable because it is also borrowed as immutable",
        );
        let graphs = heal_to_graphs(&[diag]);
        assert_eq!(graphs[0].root.category, Some(DiagnosticCategory::BorrowConflict));
    }

    #[test]
    fn diagnostic_graph_builder() {
        use crate::hir::{Applicability, DiagnosticGraph, Fix};
        let graph = DiagnosticGraph::from_root(Diagnostic::error("test error"))
            .with_note("this is a note")
            .with_help("try this instead")
            .with_cause("caused by something")
            .with_fix(Fix {
                description: "fix it".into(),
                applicability: Applicability::MachineApplicable,
                preconditions: vec![],
                postconditions: vec!["error resolved".into()],
                side_effects: vec![],
                confidence: 0.95,
            })
            .with_related(&["E0001"]);
        assert_eq!(graph.context.len(), 3);
        assert_eq!(graph.fixes.len(), 1);
        assert_eq!(graph.related, vec!["E0001"]);
        assert!(graph.fixes[0].confidence > 0.9);
    }

    #[test]
    fn diagnostic_graph_display() {
        use crate::hir::{Applicability, DiagnosticGraph, Fix};
        let graph = DiagnosticGraph::from_root(Diagnostic::error("test"))
            .with_note("a note")
            .with_fix(Fix {
                description: "do thing".into(),
                applicability: Applicability::MaybeIncorrect,
                preconditions: vec![],
                postconditions: vec![],
                side_effects: vec![],
                confidence: 0.7,
            });
        let display = format!("{graph}");
        assert!(display.contains("error: test"));
        assert!(display.contains("note: a note"));
        assert!(display.contains("fix[0]"));
        assert!(display.contains("70%"));
    }
}
