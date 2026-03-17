/// Verification Oracle — spec/contract checking (P21, P8).
///
/// Verifies that function implementations satisfy their spec blocks
/// (preconditions, postconditions) and that effect annotations are
/// consistent. Agents query the oracle to validate code before committing.
///
/// RAP method: verify/contracts → { module, function } → VerificationResult
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
pub fn verify_contracts(fqn: &str, spec: Option<&SpecInput>, effects: &EffectAnalysis) -> VerificationResult {
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

    VerificationResult {
        fqn: fqn.into(),
        status,
        checks,
        effect_checks,
    }
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
        let spec = SpecInput {
            requires: vec!["path.exists()".into()],
            ensures: vec![],
        };
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
        let effects = EffectAnalysis {
            declared: vec!["io".into(), "net".into()],
            used: vec!["io".into()],
        };
        let result = verify_contracts("my.fn", None, &effects);
        assert!(result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Consistent));
        assert!(result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Unused));
    }

    #[test]
    fn mixed_effects() {
        let effects = EffectAnalysis {
            declared: vec!["io".into()],
            used: vec!["io".into(), "fs".into()],
        };
        let result = verify_contracts("my.fn", None, &effects);
        let has_undeclared = result.effect_checks.iter().any(|e| e.result == EffectCheckResult::Undeclared);
        assert!(has_undeclared);
    }
}
