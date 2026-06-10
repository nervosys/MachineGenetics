// ── Code Generation Bridge ─────────────────────────────────────────
//
// Bridges the synthesis oracle, knowledge base, and compiler pipeline
// into a single iterative code generation loop.
//
// The bridge implements the generate → validate → heal → refine cycle:
//   1. Query KB for domain constraints relevant to the target spec
//   2. Enrich the SynthesisSpec with KB-derived preconditions/invariants
//   3. Generate candidate implementations via the synthesis oracle
//   4. Validate each candidate through the full compiler pipeline
//   5. If validation fails, apply self-healing fixes
//   6. Rank candidates by (validity, cost score, heal confidence)
//   7. Return the best validated candidate as MechGen source code
//
// This module is the "glue" that makes NL → code generation reliable:
// every generated program is compiler-checked before being returned.

use crate::ast;
use crate::effects;
use crate::fmt;
use crate::heal;
use crate::hir;
use crate::lexer;
use crate::logic;
use crate::parser;
use crate::resolve;
use crate::skb;
use crate::synthesis::{Candidate, SynthesisOracle, SynthesisSpec};
use crate::types;
use crate::verify;


// ═══════════════════════════════════════════════════════════════════
// ValidatedCode — the output of the bridge
// ═══════════════════════════════════════════════════════════════════

/// A code generation result that has passed through the compiler pipeline.
#[derive(Debug, Clone)]
pub struct ValidatedCode {
    /// The generated MechGen source (human-readable syntax).
    pub source_human: String,
    /// The generated MechGen source (agent-compact syntax).
    pub source_agent: String,
    /// The AST module.
    pub module: ast::Module,
    /// The synthesis candidate that produced this code.
    pub candidate: Candidate,
    /// Number of heal iterations applied.
    pub heal_iterations: usize,
    /// Compiler diagnostics (should be empty on success).
    pub diagnostics: Vec<hir::Diagnostic>,
    /// Verification summary.
    pub verification: Vec<verify::VerificationResult>,
    /// KB facts that contributed to this generation.
    pub kb_contributions: Vec<String>,
}

/// Summary of the code generation process.
#[derive(Debug, Clone)]
pub struct GenerationReport {
    /// Whether valid code was produced.
    pub success: bool,
    /// All candidates generated.
    pub candidates_generated: usize,
    /// Candidates that passed validation.
    pub candidates_valid: usize,
    /// Total heal iterations across all candidates.
    pub total_heal_iterations: usize,
    /// The selected result (if successful).
    pub result: Option<ValidatedCode>,
    /// KB queries performed.
    pub kb_queries: usize,
    /// Reasons for failed candidates.
    pub failure_reasons: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════
// CodegenBridge — the main integration point
// ═══════════════════════════════════════════════════════════════════

/// Bridges code synthesis with the KB and compiler pipeline.
pub struct CodegenBridge {
    oracle: SynthesisOracle,
    kb: logic::KnowledgeBase,
    /// Maximum heal iterations per candidate.
    max_heal_iterations: usize,
    /// Safety rules from the safety knowledge base.
    safety_rules: Vec<String>,
}

impl CodegenBridge {
    pub fn new() -> Self {
        Self {
            oracle: SynthesisOracle::new(),
            kb: logic::KnowledgeBase::new("codegen"),
            max_heal_iterations: 3,
            safety_rules: skb::query_rules_by_tag("safety").matches.iter().map(|r| r.description.clone()).collect(),
        }
    }

    /// Create with an existing KB (for shared state with NL engine).
    pub fn with_kb(kb: logic::KnowledgeBase) -> Self {
        Self {
            oracle: SynthesisOracle::new(),
            kb,
            max_heal_iterations: 3,
            safety_rules: skb::query_rules_by_tag("safety").matches.iter().map(|r| r.description.clone()).collect(),
        }
    }

    /// Add domain knowledge for code generation.
    pub fn add_knowledge(&mut self, predicate: &str, args: Vec<String>) {
        self.kb.add_fact(predicate, args);
    }

    /// Add a derivation rule.
    pub fn add_rule(&mut self, rule: logic::Rule) {
        self.kb.add_rule(rule);
    }

    /// Full generation pipeline: spec → validated code.
    pub fn generate_and_validate(&mut self, spec: &SynthesisSpec) -> GenerationReport {
        // Phase 1: Enrich spec with KB knowledge.
        let enriched = self.enrich_spec(spec);

        // Phase 2: Generate candidates.
        let candidates = self.oracle.generate(&enriched);
        let candidates_generated = candidates.len();

        // Phase 3: Validate and heal each candidate.
        let mut validated: Vec<(ValidatedCode, f64)> = Vec::new();
        let mut failure_reasons: Vec<String> = Vec::new();
        let mut total_heal_iterations = 0;

        for candidate in candidates {
            match self.validate_and_heal(&enriched, candidate) {
                Ok((code, score)) => {
                    total_heal_iterations += code.heal_iterations;
                    validated.push((code, score));
                }
                Err((reason, iters)) => {
                    total_heal_iterations += iters;
                    failure_reasons.push(reason);
                }
            }
        }

        // Phase 4: Select the best valid candidate.
        validated.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let candidates_valid = validated.len();
        let result = validated.into_iter().next().map(|(code, _)| code);

        GenerationReport {
            success: result.is_some(),
            candidates_generated,
            candidates_valid,
            total_heal_iterations,
            result,
            kb_queries: 0,
            failure_reasons,
        }
    }

    /// Generate code from a natural-language-derived spec and return source.
    pub fn generate_source(&mut self, spec: &SynthesisSpec) -> Option<String> {
        let report = self.generate_and_validate(spec);
        report.result.map(|r| r.source_human)
    }

    /// Generate validated code from a raw function description.
    pub fn generate_function(
        &mut self,
        name: &str,
        params: &[(&str, &str)],
        return_type: Option<&str>,
        effects: &[&str],
    ) -> GenerationReport {
        let mut spec = SynthesisSpec::new(name);
        for (pname, pty) in params {
            spec = spec.with_param(pname, pty);
        }
        if let Some(ret) = return_type {
            spec = spec.with_return(ret);
        }
        for eff in effects {
            spec = spec.with_effect(eff);
        }
        self.generate_and_validate(&spec)
    }

    // ─── KB Enrichment ─────────────────────────────────────────────

    /// Enrich a synthesis spec with KB-derived constraints.
    fn enrich_spec(&mut self, spec: &SynthesisSpec) -> SynthesisSpec {
        let mut enriched = spec.clone();

        // Query for function-specific constraints.
        let func_constraints = self.kb.query("function_constraint", &[&spec.name, "?"]);
        for row in &func_constraints {
            if row.len() >= 2 {
                enriched.preconditions.push(row[1].clone());
            }
        }

        // Query for type-specific constraints (e.g., "i64 must not overflow").
        for (_, ty) in &spec.params {
            let type_constraints = self.kb.query("type_constraint", &[ty.as_str(), "?"]);
            for row in &type_constraints {
                if row.len() >= 2 && !enriched.preconditions.contains(&row[1]) {
                    enriched.preconditions.push(row[1].clone());
                }
            }
        }

        // Query for effect-specific constraints.
        for eff in &spec.effects {
            let effect_constraints = self.kb.query("effect_constraint", &[eff.as_str(), "?"]);
            for row in &effect_constraints {
                if row.len() >= 2 && !enriched.invariants.contains(&row[1]) {
                    enriched.invariants.push(row[1].clone());
                }
            }
        }

        // Apply safety rules as additional constraints.
        for rule in &self.safety_rules {
            if !enriched.preconditions.contains(rule) {
                enriched.preconditions.push(rule.clone());
            }
        }

        enriched
    }

    // ─── Validation & Healing ──────────────────────────────────────

    /// Validate a candidate through the compiler pipeline, applying heal if needed.
    /// Returns Ok((ValidatedCode, score)) or Err((reason, iterations)).
    fn validate_and_heal(
        &self,
        spec: &SynthesisSpec,
        candidate: Candidate,
    ) -> Result<(ValidatedCode, f64), (String, usize)> {
        let mut current_body = candidate.body.clone();
        let mut heal_iterations = 0;

        for _iteration in 0..=self.max_heal_iterations {
            // Try to compile the candidate body.
            match self.compile_candidate(spec, &current_body) {
                CompileResult::Success {
                    module,
                    diagnostics,
                    verification,
                    kb_contribs,
                } => {
                    let source_human = fmt::format_human(&module);
                    let source_agent = fmt::format_agent(&module);

                    let error_count = diagnostics
                        .iter()
                        .filter(|d| d.severity == hir::Severity::Error)
                        .count();

                    let score = candidate.cost.score()
                        + (error_count as f64) * 100.0
                        + (heal_iterations as f64) * 10.0;

                    let code = ValidatedCode {
                        source_human,
                        source_agent,
                        module,
                        candidate: candidate.clone(),
                        heal_iterations,
                        diagnostics,
                        verification,
                        kb_contributions: kb_contribs,
                    };

                    return Ok((code, score));
                }
                CompileResult::NeedsHeal { diagnostics, healed_source } => {
                    heal_iterations += 1;
                    if let Some(fixed) = healed_source {
                        current_body = fixed;
                    } else {
                        return Err((
                            format!(
                                "Strategy {}: {} errors, no fix available",
                                candidate.strategy,
                                diagnostics.len()
                            ),
                            heal_iterations,
                        ));
                    }
                }
                CompileResult::Fatal(reason) => {
                    return Err((
                        format!("Strategy {}: {}", candidate.strategy, reason),
                        heal_iterations,
                    ));
                }
            }
        }

        Err((
            format!("Strategy {}: exceeded max heal iterations", candidate.strategy),
            heal_iterations,
        ))
    }

    /// Extract the inner body from a synthesis candidate that may already be a complete
    /// function definition. Returns just the inner content between the outermost braces.
    /// If the body is not a complete function definition, returns it unchanged.
    fn strip_function_wrapper(body: &str) -> &str {
        let trimmed = body.trim();
        if trimmed.starts_with("fn ") {
            if let (Some(open), Some(close)) = (trimmed.find('{'), trimmed.rfind('}')) {
                if close > open + 1 {
                    return trimmed[open + 1..close].trim();
                }
            }
        }
        body
    }

    /// Compile a candidate body through the full pipeline.
    fn compile_candidate(
        &self,
        spec: &SynthesisSpec,
        body: &str,
    ) -> CompileResult {
        // Strip any outer function wrapper from synthesis output before re-wrapping.
        let inner = Self::strip_function_wrapper(body);
        let source = self.wrap_in_function(spec, inner);
        // Lex.
        let tokens = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lexer::lex(&source))) {
            Ok(t) => t,
            Err(_) => return CompileResult::Fatal("Lexer panic on generated source".into()),
        };

        // Parse.
        let module = match parser::parse(&tokens) {
            Ok(m) => m,
            Err(e) => return CompileResult::Fatal(format!("Parse error: {}", e.message)),
        };

        // Resolve names.
        let resolver = resolve::resolve(&module);

        // Type check.
        let checker = types::check(&module);

        // Effect inference.
        let effect_infer = effects::infer_effects(&module);

        // Collect all diagnostics.
        let mut all_diags: Vec<hir::Diagnostic> = Vec::new();
        all_diags.extend(resolver.diagnostics.iter().cloned());
        all_diags.extend(checker.diagnostics.iter().cloned());
        all_diags.extend(effect_infer.diagnostics.iter().cloned());

        let errors: Vec<_> = all_diags
            .iter()
            .filter(|d| d.severity == hir::Severity::Error)
            .cloned()
            .collect();

        // Verify contracts.
        let verification = verify::verify_module(&module);

        if errors.is_empty() {
            // Success — all clear.
            CompileResult::Success {
                module,
                diagnostics: all_diags,
                verification,
                kb_contribs: Vec::new(),
            }
        } else {
            // Try to heal.
            let healed = heal::heal(&errors);
            let healed_source = self.apply_best_heal(&source, &healed);

            CompileResult::NeedsHeal {
                diagnostics: errors,
                healed_source,
            }
        }
    }

    /// Wrap a code body in a function matching the spec signature.
    fn wrap_in_function(&self, spec: &SynthesisSpec, body: &str) -> String {
        let params_str: String = spec
            .params
            .iter()
            .map(|(n, t)| format!("{n}: {t}"))
            .collect::<Vec<_>>()
            .join(", ");
        let ret = spec
            .return_type
            .as_deref()
            .map(|t| format!(" -> {t}"))
            .unwrap_or_default();
        let effects_str = if spec.effects.is_empty() {
            String::new()
        } else {
            format!(" / {}", spec.effects.join(", "))
        };

        let mut lines = Vec::new();

        // Contracts.
        for pre in &spec.preconditions {
            lines.push(format!("  @req {pre}"));
        }
        for post in &spec.postconditions {
            lines.push(format!("  @ens {post}"));
        }
        for inv in &spec.invariants {
            lines.push(format!("  @inv {inv}"));
        }

        let contracts = if lines.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", lines.join("\n"))
        };

        format!(
            "fn {name}({params_str}){ret}{effects_str} {{{contracts}  {body}\n}}",
            name = spec.name,
        )
    }

    /// Apply the best heal fix by extracting the function body from the fixed source.
    fn apply_best_heal(
        &self,
        _source: &str,
        healed: &[heal::HealedDiagnostic],
    ) -> Option<String> {
        // Find the highest-confidence fix across all healed diagnostics.
        let best = healed
            .iter()
            .flat_map(|h| h.fixes.iter())
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal));

        // If the best fix provides a replacement, use it.
        // In a full implementation, we'd apply the TextEdits to the source.
        // For now, return None to indicate we couldn't auto-fix.
        if let Some(fix) = best {
            if fix.confidence >= 0.7 && !fix.edits.is_empty() {
                // The fix provides concrete edits — extract the new body.
                let new_text = &fix.edits[0].new_text;
                if !new_text.is_empty() {
                    return Some(new_text.clone());
                }
            }
        }

        None
    }
}

enum CompileResult {
    Success {
        module: ast::Module,
        diagnostics: Vec<hir::Diagnostic>,
        verification: Vec<verify::VerificationResult>,
        kb_contribs: Vec<String>,
    },
    NeedsHeal {
        diagnostics: Vec<hir::Diagnostic>,
        healed_source: Option<String>,
    },
    Fatal(String),
}

// ═══════════════════════════════════════════════════════════════════
// Batch Generation — generate multiple related items at once
// ═══════════════════════════════════════════════════════════════════

/// Generate a batch of related functions from a set of specs.
pub fn batch_generate(specs: &[SynthesisSpec]) -> Vec<GenerationReport> {
    let mut bridge = CodegenBridge::new();
    specs.iter().map(|s| bridge.generate_and_validate(s)).collect()
}

/// Generate a complete module from a high-level description.
pub fn generate_module(
    _module_name: &str,
    function_specs: &[SynthesisSpec],
) -> (String, Vec<GenerationReport>) {
    let mut bridge = CodegenBridge::new();
    let mut items = Vec::new();
    let mut reports = Vec::new();

    for spec in function_specs {
        let report = bridge.generate_and_validate(spec);
        if let Some(ref result) = report.result {
            items.extend(result.module.items.clone());
        }
        reports.push(report);
    }

    let module = ast::Module { items };
    let source = fmt::format_human(&module);

    (source, reports)
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_generation() {
        let mut bridge = CodegenBridge::new();
        let spec = SynthesisSpec::new("add")
            .with_param("a", "i64")
            .with_param("b", "i64")
            .with_return("i64");

        let report = bridge.generate_and_validate(&spec);
        assert!(report.candidates_generated > 0);
    }

    #[test]
    fn kb_enrichment() {
        let mut bridge = CodegenBridge::new();
        bridge.add_knowledge("function_constraint", vec!["add".into(), "a > 0".into()]);
        bridge.add_knowledge("type_constraint", vec!["i64".into(), "must not overflow".into()]);

        let spec = SynthesisSpec::new("add")
            .with_param("a", "i64")
            .with_param("b", "i64")
            .with_return("i64");

        let enriched = bridge.enrich_spec(&spec);
        assert!(enriched.preconditions.len() > spec.preconditions.len());
    }

    #[test]
    fn wrap_function() {
        let bridge = CodegenBridge::new();
        let spec = SynthesisSpec::new("add")
            .with_param("a", "i64")
            .with_param("b", "i64")
            .with_return("i64")
            .with_req("a > 0");

        let source = bridge.wrap_in_function(&spec, "a + b");
        assert!(source.contains("fn add(a: i64, b: i64) -> i64"));
        assert!(source.contains("@req a > 0"));
        assert!(source.contains("a + b"));
    }

    #[test]
    fn generate_function_shortcut() {
        let mut bridge = CodegenBridge::new();
        let report = bridge.generate_function(
            "multiply",
            &[("x", "i64"), ("y", "i64")],
            Some("i64"),
            &[],
        );
        assert!(report.candidates_generated >= 5);
    }

    #[test]
    fn batch_gen() {
        let specs = vec![
            SynthesisSpec::new("foo").with_param("a", "i64").with_return("i64"),
            SynthesisSpec::new("bar").with_param("b", "bool").with_return("bool"),
        ];
        let reports = batch_generate(&specs);
        assert_eq!(reports.len(), 2);
    }

    #[test]
    fn generate_module_test() {
        let specs = vec![
            SynthesisSpec::new("inc").with_param("n", "i64").with_return("i64"),
        ];
        let (source, reports) = generate_module("math_utils", &specs);
        assert!(reports[0].candidates_generated > 0);
        assert_eq!(reports.len(), 1);
    }

    #[test]
    fn safety_kb_integration() {
        let mut bridge = CodegenBridge::new();
        // The safety KB has built-in rules that should enrich certain specs.
        let spec = SynthesisSpec::new("divide")
            .with_param("a", "i64")
            .with_param("b", "i64")
            .with_return("i64");

        let report = bridge.generate_and_validate(&spec);
        assert!(report.candidates_generated > 0);
    }
}
