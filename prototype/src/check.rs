//! Every static pass over a source, as one call: what `--check --json` reports,
//! and the [`Verdict`] on it. The human `--check` prints as it goes, and
//! `tests/diagnostics.rs` holds its exit code to the verdict's.

use crate::hir::{self, DiagnosticCategory, Severity};
use crate::verdict::{Refusal, Verdict};
use crate::{abl_shape, core_subset, differentiable, effects, elision, lexer, logic, parser, resolve, types, verify};

/// One record of `--check --json`, error or not.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: &'static str,
    pub line: u32,
    pub col: u32,
    pub code: String,
    pub category: String,
    pub message: String,
    pub fix: String,
}

/// What the static passes found.
#[derive(Debug, Clone)]
pub struct Checked {
    pub findings: Vec<Finding>,
    /// Per function: declared and inferred effects. Empty if it did not parse.
    pub effects: Vec<serde_json::Value>,
}

impl Checked {
    pub fn verdict(&self) -> Verdict {
        let refusals = self
            .findings
            .iter()
            .filter(|f| f.severity == "error")
            .map(|f| Refusal {
                rule: f.code.clone(),
                span: (f.line, f.col).ne(&(0, 0)).then_some((f.line, f.col)),
                message: f.message.clone(),
            })
            .collect();
        Verdict { refusals }
    }
}

/// Run every static pass over `source`, in a deterministic order.
pub fn check_source(source: &str, do_elision: bool, core: bool) -> Checked {
    let mut findings = Vec::new();
    let mut push = |severity: &'static str, line, col, code: &str, category: &str, message: String, fix: &str| {
        findings.push(Finding {
            severity,
            line,
            col,
            code: code.to_string(),
            category: category.to_string(),
            message,
            fix: fix.to_string(),
        });
    };

    let tokens = lexer::lex(source);
    for tok in &tokens {
        if tok.kind == lexer::TokenKind::Error {
            push(
                "error",
                tok.span.line as u32,
                tok.span.col as u32,
                "E0000",
                "LexError",
                "unexpected character".to_string(),
                "remove or correct the invalid token",
            );
        }
    }

    let parsed = parser::parse(&tokens);
    let module = match parsed {
        Ok(m) => Some(m),
        Err(e) => {
            let cat = DiagnosticCategory::SyntaxError;
            push("error", e.line as u32, e.col as u32, cat.code(), "SyntaxError", e.message.clone(), cat.fix_hint());
            None
        }
    };

    // Effect (capability) surface, populated when the module parses.
    let mut effect_surface: Vec<serde_json::Value> = Vec::new();
    if let Some(module) = module {
        let module = if do_elision { elision::elide(&module) } else { module };
        let mut all: Vec<hir::Diagnostic> = Vec::new();
        all.extend(resolve::resolve(&module).diagnostics);
        all.extend(types::check(&module).diagnostics);
        let effect_infer = effects::infer_effects(&module);
        all.extend(effect_infer.diagnostics.clone());
        // The `grad` obligation (§5.6): `grad(e, w)` needs an `e` with a
        // derivative. Reported here as well as in the human path — an
        // agent that validates generated MAGE with `--check --json` must
        // see the same errors a person does, or the two oracles disagree
        // about what the program means.
        all.extend(differentiable::check_grad_obligations(
            &module,
            &differentiable::infer(&module, &effect_infer),
        ));
        // Refuted contracts (E0560), for the same reason: the human path
        // counts them as errors, so this stream must carry them.
        all.extend(verify::diagnostics(&verify::verify_module(&module)));
        // `kb` facts and rules the logic engine cannot evaluate (E0570).
        all.extend(logic::module_diagnostics(&module));
        for (func, declared, inferred) in effect_infer.effect_surface() {
            effect_surface.push(serde_json::json!({
                "function": func,
                "declared": declared,
                "inferred": inferred,
            }));
        }
        if core {
            all.extend(core_subset::check(&module));
        }
        for d in &all {
            let cat = d.category.unwrap_or(DiagnosticCategory::Other);
            let (line, col) = d.span.map(|s| (s.line, s.col)).unwrap_or((0, 0));
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            let code = d.id.clone().unwrap_or_else(|| cat.code().to_string());
            push(sev, line, col, &code, &format!("{cat:?}"), d.message.clone(), cat.fix_hint());
        }

        // Typed-composition gate (§4.5): reject shape-mismatched net compositions.
        for d in &abl_shape::check_module_shapes(&module) {
            push(
                "error",
                0,
                0,
                "E0710",
                "ShapeError",
                d.message.clone(),
                "align the layer dims so each layer's output dim matches the next layer's input dim",
            );
        }
    }

    // Deterministic order so output is cacheable/diffable across runs.
    findings.sort_by(|a, b| {
        (a.line, a.col, &a.code, &a.message).cmp(&(b.line, b.col, &b.code, &b.message))
    });
    Checked { findings, effects: effect_surface }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_source_is_accepted() {
        let v = check_source("f g() -> i32 { 1 }", false, false).verdict();
        assert!(v.accepted(), "{v:?}");
    }

    #[test]
    fn a_refusal_names_its_rule_and_its_place() {
        let v = check_source("f g() -> i32 { zz }", false, false).verdict();
        assert!(!v.accepted());
        assert_eq!(v.refusals[0].rule, "E0425");
        assert!(v.refusals[0].span.is_some() || v.refusals[0].message.contains("zz"));
    }

    /// A warning is a finding and not a refusal.
    #[test]
    fn a_warning_is_not_a_refusal() {
        let c = check_source("f p(w: f64) -> f64 { grad(w |> abs, w) }", false, false);
        assert!(c.findings.iter().any(|f| f.severity == "warning"));
        assert!(c.verdict().accepted(), "{:?}", c.verdict());
    }
}
