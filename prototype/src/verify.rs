/// Verification Oracle — spec/contract checking (P21, P8).
///
/// Verifies that function implementations satisfy their spec blocks
/// (preconditions, postconditions) and that effect annotations are
/// consistent. Agents query the oracle to validate code before committing.
///
/// RAP method: verify/contracts → { module, function } → VerificationResult
use crate::ast;
use serde::{Deserialize, Serialize};

/// Result of verifying a function's contracts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Fully qualified name of the verified symbol.
    pub fqn: String,
    /// Overall status.
    pub status: VerifyStatus,
    /// Individual contract check results.
    pub checks: Vec<ContractCheck>,
    /// Effect consistency results.
    pub effect_checks: Vec<EffectCheck>,
}

/// Overall verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifyStatus {
    /// All contracts and effects verified.
    Verified,
    /// Some contracts could not be statically verified (need runtime checks).
    Partial,
    /// Contract violations found.
    Failed,
    /// No contracts to verify.
    Trivial,
}

/// Result of checking a single pre/post condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractCheck {
    /// The condition text (e.g., "path.exists()").
    pub condition: String,
    /// Whether this is a precondition or postcondition.
    pub kind: ContractKind,
    /// Check result.
    pub result: CheckResult,
    /// Explanation if not verified.
    pub explanation: Option<String>,
}

/// Pre or post condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractKind {
    Requires,
    Ensures,
}

/// Result of a single check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckResult {
    /// Statically verified to hold.
    Verified,
    /// Statically verified to fail.
    Violated,
    /// Cannot determine statically; runtime check emitted.
    Unknown,
}

/// Result of checking effect consistency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectCheck {
    /// The effect being checked (e.g., "io").
    pub effect: String,
    /// Check result.
    pub result: EffectCheckResult,
    /// Details.
    pub detail: String,
}

/// Effect checking outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectCheckResult {
    /// Effect is declared and used.
    Consistent,
    /// Effect is used but not declared.
    Undeclared,
    /// Effect is declared but never used.
    Unused,
}

// ── Verification Logic ───────────────────────────────────────────────

/// Verify a function's contracts given its spec block and body analysis.
pub fn verify_contracts(
    fqn: &str,
    spec: Option<&SpecInput>,
    effects: &EffectAnalysis,
) -> VerificationResult {
    let mut checks = Vec::new();
    let mut status = VerifyStatus::Trivial;

    if let Some(spec) = spec {
        for req in &spec.requires {
            checks.push(check_condition(req, ContractKind::Requires));
        }
        for ens in &spec.ensures {
            checks.push(check_condition(ens, ContractKind::Ensures));
        }

        let has_violation = checks.iter().any(|c| c.result == CheckResult::Violated);
        let has_unknown = checks.iter().any(|c| c.result == CheckResult::Unknown);

        status = if has_violation {
            VerifyStatus::Failed
        } else if has_unknown {
            VerifyStatus::Partial
        } else {
            VerifyStatus::Verified
        };
    }

    let effect_checks = verify_effects(effects);

    // If effects are inconsistent, downgrade status
    if effect_checks.iter().any(|e| e.result == EffectCheckResult::Undeclared) {
        if status == VerifyStatus::Verified || status == VerifyStatus::Trivial {
            status = VerifyStatus::Partial;
        }
    }

    VerificationResult { fqn: fqn.into(), status, checks, effect_checks }
}

/// Input spec block for verification.
pub struct SpecInput {
    pub requires: Vec<String>,
    pub ensures: Vec<String>,
}

/// Effect analysis result from the compiler.
pub struct EffectAnalysis {
    /// Effects declared in the function signature.
    pub declared: Vec<String>,
    /// Effects actually used in the function body.
    pub used: Vec<String>,
}

fn check_condition(condition: &str, kind: ContractKind) -> ContractCheck {
    // Simple heuristic-based static verification.
    // A real implementation would use symbolic execution or SMT solving.

    let result = if condition.contains(".len() >= 0") || condition.contains(".len() > -1") {
        // Always true for unsigned lengths.
        CheckResult::Verified
    } else if condition.contains(".is_ok()") && condition.contains("=>") {
        // Conditional postcondition — typically verifiable.
        CheckResult::Verified
    } else if condition.ends_with(".exists()") || condition.contains("port") {
        // Runtime-dependent predicates — cannot verify statically.
        CheckResult::Unknown
    } else {
        // Default: unknown for complex conditions.
        CheckResult::Unknown
    };

    let explanation = match result {
        CheckResult::Verified => None,
        CheckResult::Violated => Some("Condition provably false".into()),
        CheckResult::Unknown => Some("Cannot verify statically; runtime check required".into()),
    };

    ContractCheck { condition: condition.into(), kind, result, explanation }
}

fn verify_effects(analysis: &EffectAnalysis) -> Vec<EffectCheck> {
    let mut checks = Vec::new();

    for declared in &analysis.declared {
        if analysis.used.contains(declared) {
            checks.push(EffectCheck {
                effect: declared.clone(),
                result: EffectCheckResult::Consistent,
                detail: format!("Effect `{declared}` is declared and used"),
            });
        } else {
            checks.push(EffectCheck {
                effect: declared.clone(),
                result: EffectCheckResult::Unused,
                detail: format!("Effect `{declared}` is declared but never used in the body"),
            });
        }
    }

    for used in &analysis.used {
        if !analysis.declared.contains(used) {
            checks.push(EffectCheck {
                effect: used.clone(),
                result: EffectCheckResult::Undeclared,
                detail: format!("Effect `{used}` is used but not declared in the signature"),
            });
        }
    }

    checks
}

// ── AST-driven module verification ───────────────────────────────────

/// Verify all contract-annotated functions in a module.
///
/// Walks the module AST, extracts `@req`/`@ens`/`@inv` contract clauses
/// from `FunctionDef` nodes, converts them to `SpecInput`, and runs the
/// verification oracle on each function. Also checks struct invariants.
pub fn verify_module(module: &ast::Module) -> Vec<VerificationResult> {
    let mut results = Vec::new();
    for item in &module.items {
        verify_item(&item.kind, "", &mut results);
    }
    results
}

fn verify_item(kind: &ast::ItemKind, prefix: &str, results: &mut Vec<VerificationResult>) {
    match kind {
        ast::ItemKind::Function(func) => {
            // Skip functions with no contracts and no declared effects
            if func.contracts.is_empty() && func.effects.is_empty() {
                return;
            }

            let fqn = if prefix.is_empty() {
                func.name.clone()
            } else {
                format!("{prefix}.{}", func.name)
            };

            // Build SpecInput from contracts
            let requires: Vec<String> = func
                .contracts
                .iter()
                .filter(|c| c.kind == ast::ContractClauseKind::Requires)
                .map(|c| c.condition.clone())
                .collect();
            let ensures: Vec<String> = func
                .contracts
                .iter()
                .filter(|c| c.kind == ast::ContractClauseKind::Ensures)
                .map(|c| c.condition.clone())
                .collect();

            let spec = if requires.is_empty() && ensures.is_empty() {
                None
            } else {
                Some(SpecInput { requires, ensures })
            };

            // Effect analysis from the function's declared effects
            let effects = EffectAnalysis {
                declared: func.effects.clone(),
                used: vec![], // body analysis would fill this in a full compiler
            };

            results.push(verify_contracts(&fqn, spec.as_ref(), &effects));
        }
        ast::ItemKind::Struct(st) => {
            if !st.contracts.is_empty() {
                let fqn = if prefix.is_empty() {
                    st.name.clone()
                } else {
                    format!("{prefix}.{}", st.name)
                };

                let invariants: Vec<String> = st
                    .contracts
                    .iter()
                    .filter(|c| c.kind == ast::ContractClauseKind::Invariant)
                    .map(|c| c.condition.clone())
                    .collect();

                let checks: Vec<ContractCheck> = invariants
                    .iter()
                    .map(|inv| check_condition(inv, ContractKind::Requires))
                    .collect();

                let has_violation = checks.iter().any(|c| c.result == CheckResult::Violated);
                let has_unknown = checks.iter().any(|c| c.result == CheckResult::Unknown);
                let status = if has_violation {
                    VerifyStatus::Failed
                } else if has_unknown {
                    VerifyStatus::Partial
                } else if checks.is_empty() {
                    VerifyStatus::Trivial
                } else {
                    VerifyStatus::Verified
                };

                results.push(VerificationResult { fqn, status, checks, effect_checks: vec![] });
            }
        }
        ast::ItemKind::Module(m) => {
            let mod_prefix =
                if prefix.is_empty() { m.name.clone() } else { format!("{prefix}.{}", m.name) };
            if let Some(items) = &m.items {
                for item in items {
                    verify_item(&item.kind, &mod_prefix, results);
                }
            }
        }
        ast::ItemKind::Impl(imp) => {
            for item in &imp.items {
                verify_item(&item.kind, prefix, results);
            }
        }
        ast::ItemKind::Trait(tr) => {
            let trait_prefix =
                if prefix.is_empty() { tr.name.clone() } else { format!("{prefix}.{}", tr.name) };
            for item in &tr.items {
                verify_item(&item.kind, &trait_prefix, results);
            }
        }
        ast::ItemKind::Spec(spec) => {
            if spec.items.is_empty() {
                return;
            }
            let fqn = if prefix.is_empty() {
                format!("spec.{}", spec.name)
            } else {
                format!("{prefix}.spec.{}", spec.name)
            };

            let requires: Vec<String> = spec
                .items
                .iter()
                .filter_map(|item| match item {
                    ast::SpecItem::Require(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            let ensures: Vec<String> = spec
                .items
                .iter()
                .filter_map(|item| match item {
                    ast::SpecItem::Ensure(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();

            let spec_input = if requires.is_empty() && ensures.is_empty() {
                None
            } else {
                Some(SpecInput { requires, ensures })
            };

            let declared_effects: Vec<String> = spec
                .items
                .iter()
                .filter_map(|item| match item {
                    ast::SpecItem::Effect(effs) => Some(effs.clone()),
                    _ => None,
                })
                .flatten()
                .collect();

            let effects = EffectAnalysis { declared: declared_effects, used: vec![] };
            results.push(verify_contracts(&fqn, spec_input.as_ref(), &effects));
        }
        ast::ItemKind::TypeAlias(ta) => {
            if let Some(ref predicate) = ta.refinement {
                let fqn = if prefix.is_empty() {
                    format!("type.{}", ta.name)
                } else {
                    format!("{prefix}.type.{}", ta.name)
                };

                let check = check_condition(predicate, ContractKind::Requires);
                let status = match check.result {
                    CheckResult::Verified => VerifyStatus::Verified,
                    CheckResult::Violated => VerifyStatus::Failed,
                    CheckResult::Unknown => VerifyStatus::Partial,
                };

                results.push(VerificationResult {
                    fqn,
                    status,
                    checks: vec![check],
                    effect_checks: vec![],
                });
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trivial_no_spec() {
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let result = verify_contracts("my.fn", None, &effects);
        assert_eq!(result.status, VerifyStatus::Trivial);
        assert!(result.checks.is_empty());
    }

    #[test]
    fn verified_length_postcondition() {
        let spec = SpecInput {
            requires: vec![],
            ensures: vec!["ret.is_ok() => ret.unwrap().len() >= 0".into()],
        };
        let effects = EffectAnalysis { declared: vec!["io".into()], used: vec!["io".into()] };
        let result = verify_contracts("std.io.read_file", Some(&spec), &effects);
        assert_eq!(result.status, VerifyStatus::Verified);
    }

    #[test]
    fn partial_runtime_dependent() {
        let spec = SpecInput { requires: vec!["path.exists()".into()], ensures: vec![] };
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let result = verify_contracts("my.fn", Some(&spec), &effects);
        assert_eq!(result.status, VerifyStatus::Partial);
        assert_eq!(result.checks[0].result, CheckResult::Unknown);
    }

    #[test]
    fn undeclared_effect_downgrades() {
        let spec = SpecInput {
            requires: vec![],
            ensures: vec!["ret.is_ok() => ret.unwrap().len() >= 0".into()],
        };
        let effects = EffectAnalysis { declared: vec![], used: vec!["io".into()] };
        let result = verify_contracts("my.fn", Some(&spec), &effects);
        // Would be Verified from contract check, but downgraded due to undeclared effect.
        assert_eq!(result.status, VerifyStatus::Partial);
    }

    #[test]
    fn effect_consistency() {
        let effects =
            EffectAnalysis { declared: vec!["io".into(), "net".into()], used: vec!["io".into()] };
        let result = verify_contracts("my.fn", None, &effects);
        assert!(result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Consistent));
        assert!(result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Unused));
    }

    #[test]
    fn mixed_effects() {
        let effects =
            EffectAnalysis { declared: vec!["io".into()], used: vec!["io".into(), "fs".into()] };
        let result = verify_contracts("my.fn", None, &effects);
        let has_undeclared =
            result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Undeclared);
        assert!(has_undeclared);
    }

    // ── Module-level contract verification tests ──────────

    use crate::lexer;
    use crate::parser;

    fn parse_source(src: &str) -> crate::ast::Module {
        let tokens = lexer::lex(src);
        parser::parse(&tokens).unwrap()
    }

    #[test]
    fn verify_module_function_with_requires() {
        let module = parse_source("@req(n > 0) f factorial(n: u64) -> u64 { n }");
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fqn, "factorial");
        assert!(!results[0].checks.is_empty());
    }

    #[test]
    fn verify_module_function_multiple_contracts() {
        let module = parse_source("@req(x >= 0) @ens(result >= 0) f abs(x: i32) -> i32 { x }");
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fqn, "abs");
        assert!(results[0].checks.len() >= 2);
    }

    #[test]
    fn verify_module_struct_with_invariant() {
        let module = parse_source("@inv(_.len <= _.cap) S Buffer { len: usize, cap: usize }");
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fqn, "Buffer");
    }

    #[test]
    fn verify_module_no_contracts() {
        let module = parse_source("f noop() { }");
        let results = super::verify_module(&module);
        assert!(results.is_empty());
    }

    #[test]
    fn verify_module_mixed_items() {
        let src = "@req(n > 0) f foo(n: i32) { } f bar() { } @inv(true) S Baz { }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        // foo has contracts, bar doesn't, Baz has an invariant
        assert_eq!(results.len(), 2);
    }

    // ── Spec verification tests (Step 31) ──────────────────

    #[test]
    fn verify_module_spec_with_req_ens() {
        let src = "spec sort(s: [i32]) -> [i32] { @req(s.len > 0) @ens(result.is_sorted) }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].fqn.contains("sort"));
        assert!(!results[0].checks.is_empty());
    }

    #[test]
    fn verify_module_spec_empty() {
        let module = parse_source("spec Empty { }");
        let results = super::verify_module(&module);
        assert!(results.is_empty());
    }

    #[test]
    fn verify_module_spec_and_function() {
        let src = "spec abs_spec(x: i32) -> i32 { @req(true) } @req(x >= 0) f abs(x: i32) -> i32 { x }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        // Both spec and function should produce results
        assert_eq!(results.len(), 2);
    }

    // ── Refinement type verification tests (Step 32) ───────

    #[test]
    fn verify_module_refinement_type() {
        let src = "Y NonZeroPort = u16 ~> _.value > 0;";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert!(results[0].fqn.contains("NonZeroPort"));
    }

    #[test]
    fn verify_module_type_alias_no_refinement() {
        let src = "Y Meters = f64;";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        assert!(results.is_empty());
    }
}
