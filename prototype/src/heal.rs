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
    /// A diagnostic message this pattern is meant to match.
    ///
    /// Beside the matcher on purpose. `pattern_names()` publishes all 34 of
    /// these names in the ontology, and the only test over that list asserted
    /// it was longer than ten — so a pattern whose matcher stopped matching,
    /// or whose generator stopped producing fixes, stayed published as a
    /// mechanical repair an agent could ask for and never receive.
    /// `every_pattern_matches_its_example_and_produces_a_fix` runs each one.
    /// A new pattern cannot be added without an example, which is the point:
    /// the example is the claim, and the compiler requires it.
    example: &'static str,
    /// Returns true if this pattern matches the diagnostic message.
    matches: fn(&str) -> bool,
    /// Given the diagnostic, produce fix candidates.
    generate: fn(&Diagnostic) -> Vec<FixCandidate>,
}

/// Public enumeration of all built-in heal pattern names. Used by the
/// ontology endpoint so agents can discover what mechanical fixes are
/// available without invoking the full registry construction.
pub fn pattern_names() -> Vec<&'static str> {
    builtin_patterns().into_iter().map(|p| p.name).collect()
}

/// Each pattern's name paired with a diagnostic message it matches.
///
/// The ontology published bare names, which tell an agent that
/// `parse-stray-comma-in-name-position` exists and nothing about when it
/// fires. The example is the cheapest possible answer to "what does this one
/// catch", it is the same string the test runs, and publishing it means the
/// field cannot rot into decoration — it is on the wire.
pub fn patterns_with_examples() -> Vec<(&'static str, &'static str)> {
    builtin_patterns().into_iter().map(|p| (p.name, p.example)).collect()
}

/// The built-in pattern registry.
fn builtin_patterns() -> Vec<ErrorPattern> {
    vec![
        ErrorPattern {
            name: "missing-return-type",
            example: "expected return type",
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
            example: "unexpected token `;`",
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
            example: "function `f` performs undeclared effect [io]",
            matches: |msg| {
                msg.contains("effect")
                    && (msg.contains("not declared") || msg.contains("undeclared"))
            },
            generate: |diag| {
                // The effects are the bracketed list, not the first backticked
                // word — that one is the *function* name, and taking it
                // produced the suggestion "Add `/ P.leak` effect annotation",
                // naming a function where an effect belongs. A repair loop
                // applying it would write an annotation that fails the
                // unknown-effect check one pass later.
                let effect = extract_effect_list(&diag.message)
                    .or_else(|| extract_quoted(&diag.message))
                    .unwrap_or("io".to_string());
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
            example: "type mismatch: I32 vs Str",
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
                // The two branches above fire only when the message names
                // `Option`/`Result`, and the checker's own commonest mismatch
                // does not: `type mismatch: I32 vs Usize` mentions neither. So
                // the pattern published to repair type mismatches produced
                // *nothing* for the mismatch the language actually reports —
                // the ordinary `len(xs)` against an `i32` return, which is the
                // first error most programs hit. Between two numeric types the
                // conversion is mechanical and gets a real edit; otherwise the
                // candidate names both sides so the agent knows which way to
                // move.
                if let Some((expected, found)) = numeric_mismatch(&diag.message) {
                    let (line, col) = diag.span.map(|s| (s.line, s.col)).unwrap_or((1, 1));
                    fixes.push(FixCandidate {
                        id: "insert-numeric-cast".into(),
                        description: format!("Convert the `{found}` to `{expected}` with `as {expected}`"),
                        edits: vec![TextEdit {
                            start_line: line,
                            start_col: col,
                            end_line: line,
                            end_col: col,
                            new_text: format!(" as {expected}"),
                        }],
                        confidence: 0.6,
                        // A narrowing cast can change the value.
                        semantics_preserving: false,
                        token_cost: 3,
                    });
                } else if fixes.is_empty() {
                    fixes.push(FixCandidate {
                        id: "reconcile-types".into(),
                        description: format!(
                            "The two sides disagree: {}. Change one side, or convert \
                             explicitly.",
                            diag.message.trim()
                        ),
                        edits: vec![],
                        confidence: 0.3,
                        semantics_preserving: false,
                        token_cost: 0,
                    });
                }
                fixes
            },
        },
        ErrorPattern {
            name: "unknown-identifier",
            example: "cannot find `foo` in this scope",
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
            example: "spec `positive` violated",
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
        // ── New patterns (Step 34) ──────────────────────────────
        ErrorPattern {
            name: "missing-closing-paren",
            example: "expected `)` to close `(`",
            matches: |msg| msg.contains("expected `)`") || msg.contains("unclosed `(`"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-closing-paren".into(),
                    description: "Insert missing `)`".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ")".into(),
                    }],
                    confidence: 0.85,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },
        ErrorPattern {
            name: "missing-closing-bracket",
            example: "expected `]` to close `[`",
            matches: |msg| msg.contains("expected `]`") || msg.contains("unclosed `[`"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-closing-bracket".into(),
                    description: "Insert missing `]`".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "]".into(),
                    }],
                    confidence: 0.85,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },
        ErrorPattern {
            name: "borrow-conflict",
            example: "cannot borrow `x` as mutable",
            matches: |msg| msg.contains("cannot borrow") || msg.contains("already borrowed"),
            generate: |diag| {
                let mut fixes = Vec::new();
                if diag.message.contains("mutable") {
                    fixes.push(FixCandidate {
                        id: "clone-to-avoid-borrow".into(),
                        description: "Clone the value to avoid simultaneous borrow".into(),
                        edits: vec![],
                        confidence: 0.45,
                        semantics_preserving: false,
                        token_cost: 3,
                    });
                    fixes.push(FixCandidate {
                        id: "scope-borrow".into(),
                        description: "Limit borrow scope with an inner block".into(),
                        edits: vec![],
                        confidence: 0.5,
                        semantics_preserving: true,
                        token_cost: 4,
                    });
                }
                fixes
            },
        },
        ErrorPattern {
            name: "move-after-use",
            example: "use of moved value `x`",
            matches: |msg| {
                msg.contains("use of moved value") || msg.contains("value used after move")
            },
            generate: |_diag| {
                vec![
                    FixCandidate {
                        id: "clone-before-move".into(),
                        description: "Clone the value before the move".into(),
                        edits: vec![],
                        confidence: 0.55,
                        semantics_preserving: false,
                        token_cost: 3,
                    },
                    FixCandidate {
                        id: "borrow-instead-of-move".into(),
                        description: "Pass by reference instead of moving".into(),
                        edits: vec![],
                        confidence: 0.6,
                        semantics_preserving: false,
                        token_cost: 2,
                    },
                ]
            },
        },
        ErrorPattern {
            name: "unused-variable",
            example: "unused variable `x`",
            matches: |msg| msg.contains("unused variable"),
            generate: |diag| {
                let name = extract_quoted(&diag.message).unwrap_or_default();
                let prefixed = format!("_{name}");
                vec![FixCandidate {
                    id: "prefix-underscore".into(),
                    description: format!("Rename to `{prefixed}` to suppress warning"),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + name.len() as u32).unwrap_or(1),
                        new_text: prefixed,
                    }],
                    confidence: 0.9,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },
        ErrorPattern {
            name: "missing-field",
            example: "missing field `y` in initializer",
            matches: |msg| msg.contains("missing field") || msg.contains("field not found"),
            generate: |diag| {
                let field = extract_quoted(&diag.message).unwrap_or_default();
                vec![FixCandidate {
                    id: "add-missing-field".into(),
                    description: format!("Add missing field `{field}` with default value"),
                    edits: vec![],
                    confidence: 0.5,
                    semantics_preserving: false,
                    token_cost: 4,
                }]
            },
        },
        ErrorPattern {
            name: "contract-precondition-fail",
            example: "precondition `n > 0` does not hold",
            matches: |msg| {
                msg.contains("precondition") || (msg.contains("@req") && msg.contains("violated"))
            },
            generate: |diag| {
                let cond = extract_quoted(&diag.message).unwrap_or("condition".into());
                vec![FixCandidate {
                    id: "add-guard-for-precondition".into(),
                    description: format!("Add `? {cond}` guard before call to satisfy @req"),
                    edits: vec![],
                    confidence: 0.55,
                    semantics_preserving: false,
                    token_cost: 5,
                }]
            },
        },
        ErrorPattern {
            name: "contract-postcondition-fail",
            example: "postcondition `ret > 0` does not hold",
            matches: |msg| {
                msg.contains("postcondition") || (msg.contains("@ens") && msg.contains("violated"))
            },
            generate: |_diag| {
                vec![FixCandidate {
                    id: "adjust-return-for-postcondition".into(),
                    description: "Adjust return expression to satisfy @ens contract".into(),
                    edits: vec![],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 6,
                }]
            },
        },
        ErrorPattern {
            name: "invariant-violation",
            example: "invariant `len <= cap` does not hold",
            matches: |msg| {
                msg.contains("invariant") || (msg.contains("@inv") && msg.contains("violated"))
            },
            generate: |diag| {
                let inv = extract_quoted(&diag.message).unwrap_or("invariant".into());
                vec![FixCandidate {
                    id: "restore-invariant".into(),
                    description: format!("Add assertion to restore invariant: {inv}"),
                    edits: vec![],
                    confidence: 0.35,
                    semantics_preserving: false,
                    token_cost: 5,
                }]
            },
        },
        ErrorPattern {
            name: "capability-denied",
            example: "capability `net` denied",
            matches: |msg| {
                msg.contains("capability")
                    && (msg.contains("denied") || msg.contains("not granted"))
            },
            generate: |diag| {
                let cap = extract_quoted(&diag.message).unwrap_or("unknown".into());
                vec![FixCandidate {
                    id: "add-capability".into(),
                    description: format!("Add `{cap}` to agent capabilities list"),
                    edits: vec![],
                    confidence: 0.65,
                    semantics_preserving: false,
                    token_cost: 3,
                }]
            },
        },
        ErrorPattern {
            name: "performance-budget-exceeded",
            example: "performance budget exceeded",
            matches: |msg| {
                msg.contains("performance") && msg.contains("exceeded")
                    || msg.contains("@perf") && msg.contains("violated")
            },
            generate: |_diag| {
                vec![FixCandidate {
                    id: "optimize-algorithm".into(),
                    description: "Consider a more efficient algorithm to meet @perf bound".into(),
                    edits: vec![],
                    confidence: 0.3,
                    semantics_preserving: false,
                    token_cost: 10,
                }]
            },
        },

        // ── Parse-error patterns (added Phase 37, driven by the
        // `reliability-bench` failure clusters). These target the
        // structured `expected X, found Y` messages the parser emits.
        // The heal strategy is intentionally conservative — we insert
        // the missing punctuation at the error column rather than
        // attempting deeper restructuring, so the worst case is a
        // re-parse that still fails (caller already handles that).

        // expected Semi, found ... → insert `;` before the offending token
        ErrorPattern {
            name: "parse-missing-semi",
            example: "expected Semi, found RBrace '}'",
            matches: |msg| msg.starts_with("expected Semi, found ")
                || msg.contains("expected Semi"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-semi-before-token".into(),
                    description: "Insert `;` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ";".into(),
                    }],
                    confidence: 0.65,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },

        // expected RBrace, found ... → insert `}` to close an unbalanced block
        ErrorPattern {
            name: "parse-missing-rbrace",
            example: "expected RBrace, found Eof ''",
            matches: |msg| msg.starts_with("expected RBrace, found ")
                || msg.contains("expected RBrace"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-rbrace-before-token".into(),
                    description: "Insert `}` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "}".into(),
                    }],
                    confidence: 0.55,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },

        // expected RParen, found ... → insert `)`
        ErrorPattern {
            name: "parse-missing-rparen",
            example: "expected RParen, found Semi ';'",
            matches: |msg| msg.starts_with("expected RParen, found ")
                || msg.contains("expected RParen"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-rparen-before-token".into(),
                    description: "Insert `)` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ")".into(),
                    }],
                    confidence: 0.55,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },

        // expected RBrack, found ... → insert `]`
        ErrorPattern {
            name: "parse-missing-rbrack",
            example: "expected RBrack, found Semi ';'",
            matches: |msg| msg.starts_with("expected RBrack, found ")
                || msg.contains("expected RBrack"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-rbrack-before-token".into(),
                    description: "Insert `]` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "]".into(),
                    }],
                    confidence: 0.55,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },

        // expected Colon, found RParen → unit struct field (`x: ()`) or
        // bare type-needed slot. Insert `: _` for type inference.
        ErrorPattern {
            name: "parse-missing-type-colon",
            example: "expected Colon, found RParen ')'",
            matches: |msg| msg.starts_with("expected Colon, found RParen"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-colon-inferred-type".into(),
                    description: "Insert `: _` (inferred type) at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ": _".into(),
                    }],
                    confidence: 0.45,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },

        // expected LBrace, found Semi → empty block instead of bare ;
        ErrorPattern {
            name: "parse-empty-block",
            example: "expected LBrace, found Semi ';'",
            matches: |msg| msg.starts_with("expected LBrace, found Semi"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "replace-semi-with-empty-block".into(),
                    description: "Replace `;` with `{ }` for empty body".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 1).unwrap_or(2),
                        new_text: "{ }".into(),
                    }],
                    confidence: 0.5,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },

        // expected identifier, found Comma → stray comma where the
        // parser expected a name (param list, generic-param list,
        // use-group, struct field, etc.). Delete the offending `,`.
        ErrorPattern {
            name: "parse-stray-comma-in-name-position",
            example: "expected identifier, found Comma ','",
            matches: |msg| msg.starts_with("expected identifier, found Comma"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "delete-stray-comma-name".into(),
                    description: "Delete stray `,` where an identifier was expected".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 1).unwrap_or(2),
                        new_text: String::new(),
                    }],
                    confidence: 0.65,
                    semantics_preserving: true,
                    token_cost: 0,
                }]
            },
        },

        // expected KwF, found Semi → fn keyword consumed but body or
        // signature ended with a stray `;` where the next item should
        // start. Delete the `;` and continue.
        ErrorPattern {
            name: "parse-stray-semi-in-item-position",
            example: "expected KwF, found Semi ';'",
            matches: |msg| msg.starts_with("expected KwF, found Semi"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "delete-stray-semi-item".into(),
                    description: "Delete stray `;` where an item was expected".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 1).unwrap_or(2),
                        new_text: String::new(),
                    }],
                    confidence: 0.6,
                    semantics_preserving: true,
                    token_cost: 0,
                }]
            },
        },

        // expected Semi, found Ident → previous statement ended without
        // a `;` (e.g. dropped trailing semicolon mutation). Insert `;`
        // before the offending identifier.
        ErrorPattern {
            name: "parse-insert-missing-semi",
            example: "expected Semi, found Ident 'x'",
            matches: |msg| msg.starts_with("expected Semi, found Ident"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-missing-semi".into(),
                    description: "Insert missing `;` before the next identifier".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ";".to_string(),
                    }],
                    confidence: 0.6,
                    semantics_preserving: true,
                    token_cost: 1,
                }]
            },
        },

        // expected expression, found Eof → truncated mid-expression
        // (75%-truncation mutation cuts before the expression
        // completes). Splice `()` placeholder at the end and let the
        // structural-balance pass close any open braces afterward.
        ErrorPattern {
            name: "parse-truncated-expr-at-eof",
            example: "expected expression, found Eof ''",
            matches: |msg| msg.starts_with("expected expression, found Eof"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-unit-at-eof".into(),
                    description: "Insert `()` placeholder at EOF".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "()".to_string(),
                    }],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },

        // expected identifier, found Eof → truncated at a name slot
        // (cut just before an identifier). Splice `_` placeholder.
        ErrorPattern {
            name: "parse-truncated-ident-at-eof",
            example: "expected identifier, found Eof ''",
            matches: |msg| msg.starts_with("expected identifier, found Eof"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-underscore-at-eof".into(),
                    description: "Insert `_` placeholder identifier at EOF".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "_".to_string(),
                    }],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 1,
                }]
            },
        },

        // expected expression, found Semi → stray semicolon where an
        // expression should start. Common shape from duplicate-`;`
        // perturbations (`x = ;y;`) and from accidentally-typed extras.
        // Delete the offending `;`.
        ErrorPattern {
            name: "parse-stray-semi",
            example: "expected expression, found Semi ';'",
            matches: |msg| msg.starts_with("expected expression, found Semi"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "delete-stray-semi".into(),
                    description: "Delete stray `;` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 1).unwrap_or(2),
                        new_text: String::new(),
                    }],
                    confidence: 0.7,
                    semantics_preserving: true,
                    token_cost: 0,
                }]
            },
        },

        // expected expression, found RBrace → block / arg / index list
        // closed prematurely. Splice a `()` placeholder before the `}`
        // so the structural shape stays well-formed; downstream may
        // type-error but the parse succeeds.
        ErrorPattern {
            name: "parse-empty-where-expr-expected",
            example: "expected expression, found RBrace '}'",
            matches: |msg| msg.starts_with("expected expression, found RBrace"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-unit-before-rbrace".into(),
                    description: "Insert `()` placeholder before `}`".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: "()".to_string(),
                    }],
                    confidence: 0.55,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },

        // expected expression, found Comma → stray comma at the start of a
        // group or after another comma. Delete it. Common shape from
        // dropped first element / stray token mutations.
        ErrorPattern {
            name: "parse-stray-comma",
            example: "expected expression, found Comma ','",
            matches: |msg| msg.starts_with("expected expression, found Comma"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "delete-stray-comma".into(),
                    description: "Delete stray `,` at the parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 1).unwrap_or(2),
                        new_text: String::new(),
                    }],
                    confidence: 0.6,
                    semantics_preserving: true,
                    token_cost: 0,
                }]
            },
        },

        // expected Colon, found Comma → missing type annotation between
        // a name and the next list element. Insert `: _` (inferred type)
        // before the comma so `name, NextName` becomes `name: _, NextName`.
        ErrorPattern {
            name: "parse-missing-colon-before-comma",
            example: "expected Colon, found Comma ','",
            matches: |msg| msg.starts_with("expected Colon, found Comma"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-colon-inferred-before-comma".into(),
                    description: "Insert `: _` for missing type annotation".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: ": _".into(),
                    }],
                    confidence: 0.45,
                    semantics_preserving: false,
                    token_cost: 2,
                }]
            },
        },

        // expected Assign, found Semi → declaration without an initializer
        // (`let x;`). Insert ` = ()` (unit value) before the semicolon.
        ErrorPattern {
            name: "parse-missing-init",
            example: "expected Assign, found Semi ';'",
            matches: |msg| msg.starts_with("expected Assign, found Semi"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "insert-unit-init".into(),
                    description: "Insert ` = ()` for missing initializer".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col).unwrap_or(1),
                        new_text: " = ()".into(),
                    }],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 3,
                }]
            },
        },

        // expected identifier, found Arrow → missing return-type slot
        // got pushed in. Drop the `->` so the function header completes.
        ErrorPattern {
            name: "parse-stray-arrow",
            example: "expected identifier, found Arrow '->'",
            matches: |msg| msg.starts_with("expected identifier, found Arrow"),
            generate: |diag| {
                vec![FixCandidate {
                    id: "delete-stray-arrow".into(),
                    description: "Delete misplaced `->` at parser's error position".into(),
                    edits: vec![TextEdit {
                        start_line: diag.span.map(|s| s.line).unwrap_or(1),
                        start_col: diag.span.map(|s| s.col).unwrap_or(1),
                        end_line: diag.span.map(|s| s.line).unwrap_or(1),
                        end_col: diag.span.map(|s| s.col + 2).unwrap_or(3),
                        new_text: String::new(),
                    }],
                    confidence: 0.4,
                    semantics_preserving: false,
                    token_cost: 0,
                }]
            },
        },
    ]
}

/// Extract the first single-quoted or backtick-quoted token from a message.
/// The effect annotation a `performs undeclared effects: [FS, Net]` message
/// asks for, in the spelling an annotation uses: `fs, net`.
fn extract_effect_list(msg: &str) -> Option<String> {
    let start = msg.find("effects: [")? + "effects: [".len();
    let end = msg[start..].find(']')? + start;
    let list: Vec<String> = msg[start..end]
        .split(',')
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(list.join(", "))
}

/// `type mismatch: I32 vs Usize` → `("i32", "usize")` — **(expected, found)**,
/// when both sides name a numeric type.
///
/// The order is not cosmetic: it decides which way the suggested cast points,
/// and getting it backwards produces advice that is confidently wrong. The
/// message comes from `unify(subst, a, b)` in `types.rs`, and the two call
/// sites that reach a user pass the *declared* type first — `unify(&ret_ty,
/// &body_ty)` — so the left name is what the code must produce and the right
/// is what it has.
///
/// The checker renders types with the `Ty` variant names (`I32`, `Usize`,
/// `F64`), which are not spellings any program can contain — the source form
/// is the lowercase one, so the suggested cast has to be lowered before it is
/// offered. Returns `None` for anything but a numeric-to-numeric pair, where a
/// cast would be wrong rather than merely lossy.
fn numeric_mismatch(msg: &str) -> Option<(String, String)> {
    let tail = msg.rsplit("type mismatch:").next()?.trim();
    let (a, b) = tail.split_once(" vs ")?;
    let numeric = |s: &str| -> Option<String> {
        let s = s.trim().trim_end_matches(['.', ',', ')']).trim();
        let lower = s.to_ascii_lowercase();
        let ok = matches!(
            lower.as_str(),
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
        );
        ok.then_some(lower)
    };
    let (a, b) = (numeric(a)?, numeric(b)?);
    (a != b).then_some((a, b))
}

fn extract_quoted(msg: &str) -> Option<String> {
    // Try backtick quotes first: `name`
    if let Some(start) = msg.find('`')
        && let Some(end) = msg[start + 1..].find('`') {
            return Some(msg[start + 1..start + 1 + end].to_string());
        }
    // Try single quotes: 'name'
    if let Some(start) = msg.find('\'')
        && let Some(end) = msg[start + 1..].find('\'') {
            return Some(msg[start + 1..start + 1 + end].to_string());
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
    } else if msg.contains("unresolved name")
        || msg.contains("not found")
        || msg.contains("cannot find")
    {
        DiagnosticCategory::UnresolvedName
    } else if msg.contains("unresolved type") {
        DiagnosticCategory::UnresolvedType
    } else if msg.contains("effect") && msg.contains("undeclared") {
        DiagnosticCategory::UndeclaredEffect
    } else if msg.contains("duplicate") {
        DiagnosticCategory::DuplicateDefinition
    } else if msg.contains("precondition")
        || msg.contains("postcondition")
        || msg.contains("invariant")
        || msg.contains("spec")
    {
        DiagnosticCategory::SpecViolation
    } else if msg.contains("expected") || msg.contains("unexpected") {
        DiagnosticCategory::SyntaxError
    } else {
        // `capability`, `performance`/`@perf`, and `unused` each had their own
        // branch here, and each returned `Other` — identical to this fallback.
        // `DiagnosticCategory` (hir.rs) has no variant for any of them, so the
        // branches asserted a distinction the type cannot express and only
        // looked like handling. Removed rather than kept as decoration: if
        // those categories are ever wanted, the enum has to gain them first,
        // and a reader should find that out here rather than after tracing
        // three branches to the same value.
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

    /// Every published pattern matches its own example and produces a fix.
    ///
    /// `pattern_names()` publishes all 34 in `MAGE_ONTOLOGY.json`, where an
    /// agent reads them as the mechanical repairs it can ask for. The only
    /// test over that list asserted it had at least ten entries — true of any
    /// list of ten strings, and silent about whether a single one of them
    /// works. A matcher narrowed by one word, or a generator that returns an
    /// empty vector, would leave the name published and the repair
    /// unavailable, with nothing to notice.
    ///
    /// The example lives beside the matcher, so the two move together, and
    /// `ErrorPattern` requires it — a pattern added without one does not
    /// compile.
    #[test]
    fn every_pattern_matches_its_example_and_produces_a_fix() {
        let patterns = builtin_patterns();
        assert_eq!(
            patterns.len(),
            pattern_names().len(),
            "the published list and the registry are different lengths"
        );

        for p in &patterns {
            assert!(
                (p.matches)(p.example),
                "`{}` does not match its own example message {:?}",
                p.name,
                p.example
            );
            let diag = error_with_span(p.example, 1, 1);
            let fixes = (p.generate)(&diag);
            assert!(
                !fixes.is_empty(),
                "`{}` matches {:?} and generates no fix — it is published as a \
                 repair an agent can ask for and never receive",
                p.name,
                p.example
            );
            // A candidate with no edits is advice, not a repair — legitimate
            // for the contract and capability patterns, where the fix is a
            // decision rather than a text change. What it must not be is
            // *empty of both*: a candidate with no edits and no description
            // gives the caller nothing at all.
            for f in &fixes {
                assert!(
                    !f.edits.is_empty() || !f.description.trim().is_empty(),
                    "`{}` produced fix `{}` with neither edits nor a description",
                    p.name,
                    f.id
                );
                assert!(
                    (0.0..=1.0).contains(&f.confidence),
                    "`{}` fix `{}` has confidence {} outside 0..=1",
                    p.name,
                    f.id,
                    f.confidence
                );
            }
        }

        // And the whole path an agent actually uses: `heal` must return at
        // least one fix for each example, not merely the pattern in isolation.
        for p in &patterns {
            let healed = heal_one(&error_with_span(p.example, 1, 1));
            assert!(
                !healed.fixes.is_empty(),
                "`heal` returns nothing for `{}`'s example {:?}",
                p.name,
                p.example
            );
        }
    }

    /// The numeric cast points the way the checker means.
    ///
    /// `type mismatch: I32 vs Usize` is the message for a function declared
    /// `-> i32` whose body produces a `usize` — the declared type comes first.
    /// Reading it the other way suggests `as usize`, which is confidently
    /// backwards, and a wrong mechanical fix is worse than none: an agent
    /// applies it and gets a second error somewhere else.
    #[test]
    fn a_numeric_cast_converts_toward_the_declared_type() {
        let diag = error_with_span("function `m`: return type mismatch: type mismatch: I32 vs Usize", 1, 30);
        let healed = heal_one(&diag);
        let cast = healed
            .fixes
            .iter()
            .find(|f| f.id == "insert-numeric-cast")
            .expect("a numeric mismatch offers a cast");
        assert_eq!(cast.edits[0].new_text, " as i32", "{}", cast.description);
        assert!(cast.description.contains("`usize`"), "{}", cast.description);

        // Same type on both sides is not a mismatch to cast away, and a
        // non-numeric pair has no mechanical conversion.
        assert!(numeric_mismatch("type mismatch: I32 vs I32").is_none());
        assert!(numeric_mismatch("type mismatch: I32 vs Str").is_none());
        // But it still produces *something* — that was the bug.
        let healed = heal_one(&error_with_span("type mismatch: I32 vs Str", 1, 1));
        assert!(!healed.fixes.is_empty(), "a non-numeric mismatch must still be answered");
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

    /// The suggested annotation must name an *effect*. It named the function:
    /// `extract_quoted` took the first backticked word, which in this message
    /// is the function, so the fix read "Add `/ P.leak` effect annotation".
    #[test]
    fn the_effect_fix_suggests_the_effects_not_the_function() {
        let diag = Diagnostic::error(
            "function `P.leak` performs undeclared effects: [FS, Net] — \
             add them to its `/ effect` annotation",
        );
        let graphs = heal_to_graphs(&[diag]);
        let fixes = &graphs[0].fixes;
        assert!(!fixes.is_empty(), "expected an effect-annotation fix");
        assert!(
            fixes[0].description.contains("/ fs, net"),
            "want the effect list, got {:?}",
            fixes[0].description
        );
        assert!(
            !fixes[0].description.contains("P.leak"),
            "the function name is not an effect: {:?}",
            fixes[0].description
        );
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

    // ── New pattern tests (Step 34) ───────────────────────────

    #[test]
    fn heals_missing_closing_paren() {
        let diag = error_with_span("expected `)` after argument list", 3, 10);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "insert-closing-paren");
    }

    #[test]
    fn heals_missing_closing_bracket() {
        let diag = error_with_span("expected `]` after generic parameters", 2, 5);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "insert-closing-bracket");
    }

    #[test]
    fn heals_borrow_conflict() {
        let diag = error_with_span(
            "cannot borrow `x` as mutable because it is also borrowed as immutable",
            4,
            1,
        );
        let healed = heal_one(&diag);
        assert!(healed.fixes.len() >= 2);
        let ids: Vec<_> = healed.fixes.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.contains(&"scope-borrow"));
        assert!(ids.contains(&"clone-to-avoid-borrow"));
    }

    #[test]
    fn heals_move_after_use() {
        let diag = error_with_span("use of moved value `buf`", 8, 5);
        let healed = heal_one(&diag);
        assert!(healed.fixes.len() >= 2);
        let ids: Vec<_> = healed.fixes.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.contains(&"clone-before-move"));
        assert!(ids.contains(&"borrow-instead-of-move"));
    }

    #[test]
    fn heals_unused_variable() {
        let diag = error_with_span("unused variable `count`", 1, 5);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "prefix-underscore");
        assert!(healed.fixes[0].description.contains("_count"));
    }

    #[test]
    fn heals_missing_field() {
        let diag = error_with_span("missing field `name` in initializer of `User`", 5, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "add-missing-field");
    }

    #[test]
    fn heals_contract_precondition_fail() {
        let diag = error_with_span("precondition `n > 0` violated at call site", 10, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "add-guard-for-precondition");
    }

    #[test]
    fn heals_contract_postcondition_fail() {
        let diag = error_with_span("postcondition `result >= 0` violated", 12, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "adjust-return-for-postcondition");
    }

    #[test]
    fn heals_invariant_violation() {
        let diag = error_with_span("invariant `len <= cap` violated after mutation", 7, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "restore-invariant");
    }

    #[test]
    fn heals_capability_denied() {
        let diag = error_with_span("capability `net` not granted to agent Reviewer", 1, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "add-capability");
    }

    #[test]
    fn heals_performance_budget_exceeded() {
        let diag = error_with_span("performance budget exceeded, @perf violated", 15, 1);
        let healed = heal_one(&diag);
        assert!(!healed.fixes.is_empty());
        assert_eq!(healed.fixes[0].id, "optimize-algorithm");
    }
}
