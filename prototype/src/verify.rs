/// Verification Oracle — spec/contract checking (P21, P8).
///
/// Verifies that function implementations satisfy their spec blocks
/// (preconditions, postconditions) and that effect annotations are
/// consistent. Agents query the oracle to validate code before committing.
///
/// RAP method: verify/contracts → { module, function } → VerificationResult
use crate::ast;
use crate::verdict::Evidence;
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
    /// **Nothing was claimed here**, so nothing was verified.
    ///
    /// Named `Trivial` until 2026-09-07, and the name was the defect rather
    /// than a quibble: "trivial" reads as *it passed, easily*, and the reporter
    /// acted on that reading by **skipping the row**. A function nobody had
    /// specified was therefore invisible in the listing while still being
    /// counted in the summary line above it — `Contracts checked: 2` for a
    /// module with one contract. A coverage hole rendered as coverage, which is
    /// the error `../StatodynamicAnalysis` built its `Unreached` row around:
    /// *the claim is untested, not clean*.
    ///
    /// See `verdict.rs` for the shared vocabulary and the correspondence table.
    Unspecified,
}

impl VerifyStatus {
    /// This status as a verdict in the shared vocabulary (`verdict.rs`).
    ///
    /// Every one of these is *deductive* — contract checking quantifies over
    /// executions rather than sampling them — so nothing here can produce
    /// `Evidenced`. That asymmetry is why both words exist.
    pub fn evidence(&self, fqn: &str) -> Evidence {
        match self {
            VerifyStatus::Verified => Evidence::Proved {
                by: "contract checking".into(),
            },
            // "Could not be verified statically" is a claim nothing
            // adjudicated, not a weak pass. It rendered as `~` beside `✓` —
            // one character away from looking like success.
            VerifyStatus::Partial => Evidence::Unreached {
                why: "some conditions are not decided statically; `--eval` checks a function's @req and @ens, and a struct's @inv each time one is built, at run time, but no run-time check covers a spec with no function, or an undeclared effect".into(),
            },
            VerifyStatus::Failed => Evidence::Refuted { at: fqn.to_string() },
            VerifyStatus::Unspecified => Evidence::Unspecified,
        }
    }

    /// The character a report puts in front of the row.
    ///
    /// `Unspecified` gets `?`, not `-`. A dash reads as *nothing to see here*,
    /// which is exactly the reading that let these rows be dropped.
    pub fn symbol(&self) -> &'static str {
        match self {
            VerifyStatus::Verified => "✓",
            VerifyStatus::Partial => "~",
            VerifyStatus::Failed => "✗",
            VerifyStatus::Unspecified => "?",
        }
    }
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
    /// Where the clause was written, when it has source; the line and column
    /// an `E0560` diagnostic reports.
    #[serde(default)]
    pub span: Option<crate::hir::Span>,
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
    let mut status = VerifyStatus::Unspecified;

    if let Some(spec) = spec {
        for req in &spec.requires {
            checks.push(check_condition(&req.text, ContractKind::Requires, req.span));
        }
        for ens in &spec.ensures {
            checks.push(check_condition(&ens.text, ContractKind::Ensures, ens.span));
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
    if effect_checks.iter().any(|e| e.result == EffectCheckResult::Undeclared)
        && (status == VerifyStatus::Verified || status == VerifyStatus::Unspecified) {
            status = VerifyStatus::Partial;
        }

    VerificationResult { fqn: fqn.into(), status, checks, effect_checks }
}

/// Input spec block for verification.
pub struct SpecInput {
    pub requires: Vec<SpecClause>,
    pub ensures: Vec<SpecClause>,
}

/// One clause of a spec: its condition text, and where it was written.
#[derive(Debug, Clone)]
pub struct SpecClause {
    pub text: String,
    pub span: Option<crate::hir::Span>,
}

impl From<&str> for SpecClause {
    fn from(text: &str) -> Self {
        SpecClause { text: text.into(), span: None }
    }
}

impl From<String> for SpecClause {
    fn from(text: String) -> Self {
        SpecClause { text, span: None }
    }
}

/// A parser span as the line and column a diagnostic reports.
fn hir_span(s: &crate::lexer::Span) -> crate::hir::Span {
    crate::hir::Span { line: s.line as u32, col: s.col as u32 }
}

/// Effect analysis result from the compiler.
pub struct EffectAnalysis {
    /// Effects declared in the function signature.
    pub declared: Vec<String>,
    /// Effects actually used in the function body.
    pub used: Vec<String>,
}

/// How one condition was decided: its result, and the rule that decided it.
///
/// **`Verified` is a proof.** A condition is `Verified` only when one of the
/// rules below shows it true on every execution, and `Violated` only when one
/// shows it false on every execution. Anything else is `Unknown`, which is a
/// claim left to the run-time check, not a pass.
///
/// Until 2026-09-25 nothing could return `Violated`. Before the 2026-11-11
/// rewrite, `Verified` was reached by substring matches: `.len() >= 0` anywhere
/// in the text, and any `.is_ok()` next to a `=>`. Both were unsound. Under the
/// old rule `xs.len() >= 0 && n < 0` was `Verified`, and `ret.is_ok() => <anything>`
/// was `Verified` with no proof of the consequent.
///
/// The rules, tried in this order at each node of the parsed condition:
///
///   * **closed**: the node has no free variables (see [`is_closed`]), so the
///     bounded evaluator runs it. `true` is `Verified`, `false` is `Violated`;
///     an error, fuel exhaustion, or a non-bool is `Unknown`;
///   * **length**: the node is exactly `<path>.len() >= 0` (or `len(<path>) >= 0`,
///     or `> -1`), where `<path>` is a bare name or field chain. That is
///     `Verified`, because a length is never negative. The mirror image
///     `<path>.len() < 0` is `Violated`, for the same reason;
///   * **connectives**: `&&`, `||`, `!` and `=>` combine their parts in
///     three-valued logic (see [`conjunction`], [`disjunction`], [`negation`],
///     [`implication`]).
///
/// Anything else is `Unknown`. There is no solver and no symbolic execution.
/// A condition about a parameter's value is `Unknown` however simple it looks.
struct Decision {
    result: CheckResult,
    rule: String,
}

impl Decision {
    fn verified(rule: impl Into<String>) -> Self {
        Decision { result: CheckResult::Verified, rule: rule.into() }
    }

    fn violated(rule: impl Into<String>) -> Self {
        Decision { result: CheckResult::Violated, rule: rule.into() }
    }

    fn unknown(rule: impl Into<String>) -> Self {
        Decision { result: CheckResult::Unknown, rule: rule.into() }
    }
}

/// Fuel for one closed-condition evaluation. A closed condition is a few
/// arithmetic or `len` steps; the budget only has to stop a runaway.
const CLOSED_FUEL: u64 = 100_000;

/// The builtins a closed condition may call. Each is pure. A call to anything
/// else makes the condition not closed, because the evaluator would run it.
const PURE_BUILTINS: &[&str] = &["len"];

/// The binary operators a closed condition may use. Assignment (`=`, `+=`, ...)
/// is absent on purpose: it is not a pure comparison, and it is parsed as a
/// binary operator too.
const CLOSED_BINARY_OPS: &[&str] = &[
    "+", "-", "*", "/", "%", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "&", "|", "^", "<<", ">>",
];

/// Whether a parsed condition mentions nothing that varies between executions:
/// no parameter, no `ret`/`result`, no field, no name at all except `true`,
/// `false` and `None`, and no call except to a [`PURE_BUILTINS`] entry.
///
/// Decided on the parsed expression, not on its text. Method calls are never
/// closed here, so `[1, 2].len() == 2` is `Unknown`; `len([1, 2]) == 2` is not.
fn is_closed(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::Literal { kind, .. } => !matches!(kind, ast::LiteralKind::FormatString),
        ast::Expr::Ident { name } => matches!(name.as_str(), "true" | "false" | "None"),
        ast::Expr::Binary { op, left, right } => {
            CLOSED_BINARY_OPS.contains(&op.as_str()) && is_closed(left) && is_closed(right)
        }
        ast::Expr::Unary { op, operand } => (op == "-" || op == "!") && is_closed(operand),
        ast::Expr::Call { func, args } => {
            matches!(&**func, ast::Expr::Ident { name } if PURE_BUILTINS.contains(&name.as_str()))
                && args.iter().all(is_closed)
        }
        ast::Expr::ArrayLit { elements } | ast::Expr::TupleLit { elements } => {
            elements.iter().all(is_closed)
        }
        _ => false,
    }
}

/// Run a closed condition on the bounded evaluator, inside a function with no
/// parameters. The condition becomes the function's tail expression, so its
/// value is the condition's value.
fn evaluate_closed(e: &ast::Expr, fuel: u64) -> Decision {
    use crate::eval::{BoundedError, Value};
    let Ok(mut module) = crate::parser::parse(&crate::lexer::lex("f verify_closed_condition() { }"))
    else {
        return Decision::unknown("closed condition could not be set up for evaluation");
    };
    let Some(item) = module.items.first_mut() else {
        return Decision::unknown("closed condition could not be set up for evaluation");
    };
    let ast::ItemKind::Function(fd) = &mut item.kind else {
        return Decision::unknown("closed condition could not be set up for evaluation");
    };
    fd.body.tail_expr = Some(Box::new(e.clone()));
    match crate::eval::run_bounded(&module, "verify_closed_condition", vec![], fuel) {
        Ok(Value::Bool(true)) => Decision::verified("closed condition evaluated to true"),
        Ok(Value::Bool(false)) => Decision::violated("closed condition evaluated to false"),
        Ok(_) => Decision::unknown("closed condition did not evaluate to a bool"),
        Err(BoundedError::FuelExhausted) => {
            Decision::unknown("closed condition ran out of evaluation fuel")
        }
        Err(BoundedError::Error(msg)) => {
            Decision::unknown(format!("closed condition failed to evaluate: {msg}"))
        }
    }
}

/// A bare place: a name, or a field chain rooted at one (`xs`, `self.items`).
fn is_bare_path(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::Ident { .. } => true,
        ast::Expr::FieldAccess { object, .. } => is_bare_path(object),
        _ => false,
    }
}

/// `<path>.len()` or `len(<path>)`, with `<path>` a bare place.
fn is_length_of_path(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::MethodCall { receiver, method, args, .. } => {
            method == "len" && args.is_empty() && is_bare_path(receiver)
        }
        ast::Expr::Call { func, args } => {
            matches!(&**func, ast::Expr::Ident { name } if name == "len")
                && matches!(args.as_slice(), [a] if is_bare_path(a))
        }
        _ => false,
    }
}

/// A decimal integer literal, optionally negated. Anything else (suffixes,
/// underscores, radix prefixes) is `None`, which leaves the condition `Unknown`.
fn int_literal(e: &ast::Expr) -> Option<i64> {
    match e {
        ast::Expr::Literal { value, kind: ast::LiteralKind::Int } => value.trim().parse().ok(),
        ast::Expr::Unary { op, operand } if op == "-" => {
            int_literal(operand).and_then(i64::checked_neg)
        }
        _ => None,
    }
}

/// The length fact, and its mirror. Only the whole node counts: a length
/// inside a larger condition is decided by the connective around it.
fn length_fact(e: &ast::Expr) -> Option<Decision> {
    let ast::Expr::Binary { op, left, right } = e else {
        return None;
    };
    if !is_length_of_path(left) {
        return None;
    }
    match (op.as_str(), int_literal(right)?) {
        (">=", 0) | (">", -1) => Some(Decision::verified("a length is never negative")),
        ("<", 0) => Some(Decision::violated("a length is never negative, so this is false")),
        _ => None,
    }
}

/// `a && b`: false if either part is false, true only if both are.
fn conjunction(a: Decision, b: Decision) -> Decision {
    if a.result == CheckResult::Violated || b.result == CheckResult::Violated {
        Decision::violated("a conjunct is false on every execution")
    } else if a.result == CheckResult::Verified && b.result == CheckResult::Verified {
        Decision::verified("every conjunct is verified")
    } else {
        Decision::unknown("a conjunct is not decided statically; runtime check required")
    }
}

/// `a || b`: true if either part is true, false only if both are.
fn disjunction(a: Decision, b: Decision) -> Decision {
    if a.result == CheckResult::Verified || b.result == CheckResult::Verified {
        Decision::verified("a disjunct is verified")
    } else if a.result == CheckResult::Violated && b.result == CheckResult::Violated {
        Decision::violated("every disjunct is false on every execution")
    } else {
        Decision::unknown("no disjunct is decided statically; runtime check required")
    }
}

/// `!a`: flips a decided result, and leaves an undecided one undecided.
fn negation(a: Decision) -> Decision {
    match a.result {
        CheckResult::Verified => Decision::violated("negation of a verified condition"),
        CheckResult::Violated => Decision::verified("negation of a condition that is false"),
        CheckResult::Unknown => Decision::unknown("negation of an undecided condition"),
    }
}

/// `a => b`: holds when `a` is false, or when `b` holds. It is false only when
/// `a` holds and `b` does not. An implication with an undecided consequent is
/// undecided, even when its antecedent is false in some executions and not in
/// others, because the consequent is then never proven.
fn implication(a: Decision, b: Decision) -> Decision {
    if a.result == CheckResult::Violated {
        Decision::verified("the antecedent is false, so the implication holds")
    } else if b.result == CheckResult::Verified {
        Decision::verified("the consequent is verified")
    } else if a.result == CheckResult::Verified && b.result == CheckResult::Violated {
        Decision::violated("the antecedent is verified and the consequent is false")
    } else {
        Decision::unknown("the consequent is not proven; runtime check required")
    }
}

/// Decide a parsed expression by its rules, recursively.
fn decide_expr(e: &ast::Expr) -> Decision {
    if is_closed(e) {
        return evaluate_closed(e, CLOSED_FUEL);
    }
    if let Some(d) = length_fact(e) {
        return d;
    }
    match e {
        ast::Expr::Binary { op, left, right } if op == "&&" => {
            conjunction(decide_expr(left), decide_expr(right))
        }
        ast::Expr::Binary { op, left, right } if op == "||" => {
            disjunction(decide_expr(left), decide_expr(right))
        }
        ast::Expr::Unary { op, operand } if op == "!" => negation(decide_expr(operand)),
        _ => Decision::unknown(
            "not a closed condition, a length fact, or a connective of those; runtime check required",
        ),
    }
}

/// The byte offset of the first `=>` outside any brackets, if there is one.
///
/// `=>` is not an operator of the expression grammar, so a condition with one
/// does not parse as a whole. It is split here, at the token level, and each
/// side goes through the ordinary expression parser. `=>` binds loosest, so
/// `a || b => c` is `(a || b) => c`.
fn top_level_fat_arrow(tokens: &[crate::lexer::Token]) -> Option<usize> {
    use crate::lexer::TokenKind;
    let mut depth: i32 = 0;
    for t in tokens {
        match t.kind {
            TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth -= 1,
            TokenKind::FatArrow if depth == 0 => return Some(t.span.offset),
            _ => {}
        }
    }
    None
}

/// Decide a condition's source text. Text that does not parse is `Unknown`.
fn decide_text(condition: &str) -> Decision {
    let mut tokens = crate::lexer::lex(condition);
    // `ret` is a keyword (a return), but in a clause it names the return
    // value, as `result` does in `@ens`. It is a place here, so it becomes an
    // identifier, and `ret.len() >= 0` is a bare-path length like `xs.len()`.
    for t in tokens.iter_mut() {
        if t.kind == crate::lexer::TokenKind::KwRet {
            t.kind = crate::lexer::TokenKind::Ident;
        }
    }
    if let Some(at) = top_level_fat_arrow(&tokens) {
        return implication(decide_text(&condition[..at]), decide_text(&condition[at + 2..]));
    }
    match crate::parser::parse_expression(&tokens) {
        Ok(e) => decide_expr(&e),
        Err(err) => Decision::unknown(format!(
            "not an expression ({}); runtime check required",
            err.message
        )),
    }
}

/// A coded `SpecViolation` diagnostic for every refuted clause.
///
/// The verifier used to report only through its own summary line, so an agent
/// reading the structured `--check --json` stream saw a refutation only if some
/// other message happened to mention "spec". `Unknown` and `Unspecified` are
/// not diagnosed: an unadjudicated claim is a gap in the verifier, not a fault
/// in the program, and `verdict.rs` already renders it as one.
pub fn diagnostics(results: &[VerificationResult]) -> Vec<crate::hir::Diagnostic> {
    use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
    let mut out = Vec::new();
    for r in results {
        // No "precondition"/"postcondition" in the message: struct invariants
        // and type refinements are checked as `Requires` too, so the kind
        // would mislabel them.
        for c in r.checks.iter().filter(|c| c.result == CheckResult::Violated) {
            let mut d = Diagnostic::categorized(
                Severity::Error,
                format!("`{}`: contract `{}` is false on every execution", r.fqn, c.condition),
                DiagnosticCategory::SpecViolation,
                None,
            );
            d.span = c.span;
            out.push(d);
        }
    }
    out
}

fn check_condition(
    condition: &str,
    kind: ContractKind,
    span: Option<crate::hir::Span>,
) -> ContractCheck {
    let Decision { result, rule } = decide_text(condition);
    ContractCheck { condition: condition.into(), kind, result, explanation: Some(rule), span }
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

/// The clauses of one kind, with the spans the parser recorded for them.
fn clauses_of(contracts: &[ast::ContractClause], kind: ast::ContractClauseKind) -> Vec<SpecClause> {
    contracts
        .iter()
        .filter(|c| c.kind == kind)
        .map(|c| SpecClause { text: c.condition.clone(), span: c.span.as_ref().map(hir_span) })
        .collect()
}

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
            let requires = clauses_of(&func.contracts, ast::ContractClauseKind::Requires);
            let ensures = clauses_of(&func.contracts, ast::ContractClauseKind::Ensures);

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

                let invariants = clauses_of(&st.contracts, ast::ContractClauseKind::Invariant);

                let checks: Vec<ContractCheck> = invariants
                    .iter()
                    .map(|inv| check_condition(&inv.text, ContractKind::Requires, inv.span))
                    .collect();

                let has_violation = checks.iter().any(|c| c.result == CheckResult::Violated);
                let has_unknown = checks.iter().any(|c| c.result == CheckResult::Unknown);
                let status = if has_violation {
                    VerifyStatus::Failed
                } else if has_unknown {
                    VerifyStatus::Partial
                } else if checks.is_empty() {
                    VerifyStatus::Unspecified
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

            let requires: Vec<SpecClause> = spec
                .items
                .iter()
                .filter_map(|item| match item {
                    ast::SpecItem::Require(s, sp) => {
                        Some(SpecClause { text: s.clone(), span: Some(hir_span(sp)) })
                    }
                    _ => None,
                })
                .collect();
            let ensures: Vec<SpecClause> = spec
                .items
                .iter()
                .filter_map(|item| match item {
                    ast::SpecItem::Ensure(s, sp) => {
                        Some(SpecClause { text: s.clone(), span: Some(hir_span(sp)) })
                    }
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

                let check = check_condition(predicate, ContractKind::Requires, None);
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
        ast::ItemKind::Agent(ad) => {
            let fqn = if prefix.is_empty() {
                format!("agent.{}", ad.name)
            } else {
                format!("{prefix}.agent.{}", ad.name)
            };
            let known_caps = [
                "read_source", "write_source", "query_types", "emit_diagnostics",
                "io_read", "io_write", "net", "alloc_heap", "ffi",
                "exec", "env", "fs", "crypto",
            ];
            let mut checks = Vec::new();
            for cap in &ad.capabilities {
                let result = if known_caps.contains(&cap.as_str()) {
                    CheckResult::Verified
                } else {
                    CheckResult::Unknown
                };
                checks.push(ContractCheck {
                    kind: ContractKind::Requires,
                    condition: format!("capability:{}", cap),
                    result,
                    explanation: None,
                    span: None,
                });
            }
            if !checks.is_empty() {
                let status = if checks.iter().all(|c| c.result == CheckResult::Verified) {
                    VerifyStatus::Verified
                } else if checks.iter().any(|c| c.result == CheckResult::Unknown) {
                    VerifyStatus::Partial
                } else {
                    VerifyStatus::Failed
                };
                results.push(VerificationResult {
                    fqn,
                    status,
                    checks,
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
    fn a_false_contract_is_refuted_and_diagnosed() {
        for cond in ["false", "0b", "xs.len() < 0", " self.items.len() < 0 "] {
            let spec = SpecInput { requires: vec![cond.into()], ensures: vec![] };
            let effects = EffectAnalysis { declared: vec![], used: vec![] };
            let result = verify_contracts("my.fn", Some(&spec), &effects);
            assert_eq!(result.status, VerifyStatus::Failed, "{cond}");
            assert!(matches!(result.status.evidence("my.fn"), Evidence::Refuted { .. }));

            let diags = diagnostics(&[result]);
            assert_eq!(diags.len(), 1, "{cond}");
            let cat = diags[0].category.expect("categorised");
            assert_eq!(cat, crate::hir::DiagnosticCategory::SpecViolation);
            assert_eq!(cat.code(), "E0560");
            assert_eq!(diags[0].severity, crate::hir::Severity::Error);
        }
    }

    #[test]
    fn a_false_consequent_under_an_implication_is_not_refuted() {
        // Holds whenever the antecedent is false; refuting it would be a
        // false accusation, and `Violated` is a compile error.
        for cond in ["ret.is_ok() => ret.len() < 0", "a || xs.len() < 0", "f(x).len() < 0", ".len() < 0"] {
            assert_ne!(
                check_condition(cond, ContractKind::Ensures, None).result,
                CheckResult::Violated,
                "{cond}"
            );
        }
    }

    #[test]
    fn a_refuted_clause_reports_the_span_it_was_written_at() {
        let at = crate::hir::Span { line: 7, col: 9 };
        let spec = SpecInput {
            requires: vec![],
            ensures: vec![SpecClause { text: "false".into(), span: Some(at) }],
        };
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let diags = diagnostics(&[verify_contracts("my.fn", Some(&spec), &effects)]);
        assert_eq!(diags.len(), 1);
        let span = diags[0].span.expect("a refuted clause carries its span");
        assert_eq!((span.line, span.col), (7, 9));
    }

    #[test]
    fn unknown_and_unspecified_produce_no_diagnostic() {
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let spec = SpecInput { requires: vec!["path.exists()".into()], ensures: vec![] };
        let partial = verify_contracts("a", Some(&spec), &effects);
        let unspecified = verify_contracts("b", None, &effects);
        assert!(diagnostics(&[partial, unspecified]).is_empty());
    }

    #[test]
    fn trivial_no_spec() {
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let result = verify_contracts("my.fn", None, &effects);
        assert_eq!(result.status, VerifyStatus::Unspecified);
        assert!(result.checks.is_empty());
    }

    #[test]
    fn length_postcondition_under_implication_is_unknown_when_consequent_is_not_a_bare_path() {
        // Formerly `Verified` by substring. `ret.unwrap()` is not a bare path,
        // so the length fact does not apply, and nothing else proves it.
        let spec = SpecInput {
            requires: vec![],
            ensures: vec!["ret.is_ok() => ret.unwrap().len() >= 0".into()],
        };
        let effects = EffectAnalysis { declared: vec!["io".into()], used: vec!["io".into()] };
        let result = verify_contracts("std.io.read_file", Some(&spec), &effects);
        assert_eq!(result.checks[0].result, CheckResult::Unknown);
        assert_eq!(result.status, VerifyStatus::Partial);
    }

    #[test]
    fn verdicts_are_proofs_not_substring_matches() {
        // Each row: (condition, expected). The first is the unsound case the
        // old `.contains(".len() >= 0")` branch called Verified.
        let cases = [
            ("xs.len() >= 0 && n < 0", CheckResult::Unknown),
            ("ret.is_ok() => ret.len() < 0", CheckResult::Unknown),
            ("ret.is_ok() => ret.len() >= 0", CheckResult::Verified),
            ("1 + 1 == 2", CheckResult::Verified),
            ("1 + 1 == 3", CheckResult::Violated),
            ("false", CheckResult::Violated),
            ("0b", CheckResult::Violated),
            ("n > 0", CheckResult::Unknown),
            ("xs.len() >= 0", CheckResult::Verified),
            ("xs.len() < 0", CheckResult::Violated),
            ("!(xs.len() < 0)", CheckResult::Verified),
            ("n > 0 || xs.len() >= 0", CheckResult::Verified),
            ("n > 0 && xs.len() < 0", CheckResult::Violated),
            ("xs.len() >=", CheckResult::Unknown),
        ];
        for (cond, expected) in cases {
            assert_eq!(check_condition(cond, ContractKind::Ensures, None).result, expected, "{cond}");
        }
    }

    #[test]
    fn a_false_closed_condition_is_diagnosed_as_a_spec_violation() {
        let spec = SpecInput { requires: vec!["1 + 1 == 3".into()], ensures: vec![] };
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let result = verify_contracts("my.fn", Some(&spec), &effects);
        assert_eq!(result.status, VerifyStatus::Failed);
        let diags = diagnostics(&[result]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].category, Some(crate::hir::DiagnosticCategory::SpecViolation));
    }

    #[test]
    fn a_true_closed_condition_is_verified_by_evaluation() {
        let spec = SpecInput { requires: vec!["len([1, 2, 3]) == 3".into()], ensures: vec![] };
        let effects = EffectAnalysis { declared: vec![], used: vec![] };
        let result = verify_contracts("my.fn", Some(&spec), &effects);
        assert_eq!(result.status, VerifyStatus::Verified);
        assert_eq!(result.checks[0].explanation.as_deref(), Some("closed condition evaluated to true"));
    }

    #[test]
    fn a_closed_condition_that_runs_out_of_fuel_is_unknown() {
        // No whitelisted closed expression loops, so exhaustion is exercised
        // with a budget of zero rather than through a clause.
        let tokens = crate::lexer::lex("1 + 1 == 2");
        let e = crate::parser::parse_expression(&tokens).expect("parses");
        assert!(is_closed(&e));
        let d = evaluate_closed(&e, 0);
        assert_eq!(d.result, CheckResult::Unknown);
        assert!(d.rule.contains("fuel"), "{}", d.rule);
    }

    #[test]
    fn an_unparsable_condition_is_unknown() {
        for cond in ["xs.len() >=", "((", "a b c"] {
            assert_eq!(check_condition(cond, ContractKind::Requires, None).result, CheckResult::Unknown, "{cond}");
        }
    }

    #[test]
    fn every_check_states_the_rule_that_decided_it() {
        let c = check_condition("xs.len() >= 0", ContractKind::Requires, None);
        assert_eq!(c.explanation.as_deref(), Some("a length is never negative"));
        let c = check_condition("n > 0", ContractKind::Requires, None);
        assert!(c.explanation.as_deref().unwrap().contains("runtime check required"));
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
            ensures: vec!["xs.len() >= 0".into()],
        };
        let effects = EffectAnalysis { declared: vec![], used: vec!["io".into()] };
        let result = verify_contracts("my.fn", Some(&spec), &effects);
        // The contract is Verified, and the undeclared effect downgrades it.
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

    // ── Agent capability verification tests (Step 33) ──────

    #[test]
    fn verify_agent_known_capabilities() {
        let src = "agent Reviewer { capabilities: [read_source, query_types] }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, VerifyStatus::Verified);
        assert_eq!(results[0].checks.len(), 2);
    }

    #[test]
    fn verify_agent_unknown_capability() {
        let src = "agent Hacker { capabilities: [read_source, teleport] }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, VerifyStatus::Partial);
    }

    #[test]
    fn verify_agent_empty_capabilities() {
        let src = "agent Passive { capabilities: [] }";
        let module = parse_source(src);
        let results = super::verify_module(&module);
        // No capabilities = no checks, no result
        assert!(results.is_empty());
    }
}
