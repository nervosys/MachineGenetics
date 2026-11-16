//! Self-healing guided by the program's stated intent: `mage-parse --heal`.
//!
//! Two loops, both of which only *propose* a changed source. Nothing here
//! writes a file; the CLI writes only under `--write`.
//!
//! **Compile time** ([`heal_loop`]). Each round checks the source, collects
//! the fix candidates `heal::heal_one` ranks for every error (and the
//! `recover::recover` result when the source does not parse), applies each to
//! a copy and re-checks it. A candidate is accepted only if
//!
//! 1. the error count strictly drops,
//! 2. the intent ([`crate::intent`]) is kept: every function keeps its
//!    signature, declared effects and contracts, struct invariants are
//!    unchanged, and no `@test` is removed, and
//! 3. every `@test` that passed before the round still passes.
//!
//! Among accepted candidates the loop prefers semantics-preserving ones, then
//! fewer remaining errors, then the pattern's hand-assigned ordering prior
//! (`FixCandidate::confidence`, which is not a probability), then generation
//! order. It stops at zero errors, at a round where nothing is accepted, or at
//! `max_rounds`.
//!
//! Type-checker diagnostics carry no source span. Instead the checker
//! records, beside each type mismatch, the function, what the expression is
//! (a return value, a call argument, …), the expression itself and the two
//! types ([`crate::types::Mismatch`]). A numeric conversion is proposed at
//! that expression only, and the expression is found in the source by
//! re-parsing token ranges of the named function: exactly one range must
//! re-parse to it, or the mismatch is reported as not located. A heal
//! pattern's fix is used only for a diagnostic that has a position.
//!
//! **Run time** ([`debug_and_repair`]). Every `@test`, and every zero-argument
//! function with contracts, runs under the fuel-bounded evaluator. A failure is
//! reported with the error, the contract clause or assertion that failed, and
//! a capped call trace; every run also records coverage
//! ([`eval::Interp::enable_coverage`]): which comparisons, arithmetic and
//! `if`s of each function ran, the operands each comparison saw, the arm each
//! `if` took, and the node that raised a runtime error. Sites of non-test
//! functions that a failing run executed are ranked by the Ochiai score
//! failed / sqrt(total_failed × (failed + passed)), and candidates are tried
//! only there, best site first, and within a site in the order the
//! observations motivate (equal operands → the inclusive/exclusive swap; a
//! failed sign or ordering `@ens` → `+`/`-`; an arm taken only by failing
//! runs → the branch swap or negation; then the other small mutations at that
//! site). A failed `@ens` of a function returning an integer literal adds that
//! literal ±1. Each mutation is made on the tree, mapped to one token by rank,
//! and the edited source must re-parse to the mutated tree. A candidate is
//! accepted when it adds no checker error, keeps the intent, and makes
//! **every** run pass. Such a repair changes what the program computes by
//! construction, and is always reported as semantics-changing, with its
//! site's score, run counts, observations and the runs it fixed.
//!
//! The Ochiai score orders sites; it is not a probability that a site is at
//! fault, and nothing here reads it as one.

use serde::Serialize;

use crate::ast::Module;
use crate::eval::{self, BoundedError};
use crate::heal;
use crate::hir;
use crate::intent::{self, Intent};
use crate::lexer::{self, Token, TokenKind};
use crate::parser;
use crate::recover;
use crate::types;

/// Fuel for each run of a test or contract probe, unless the caller says.
pub const DEFAULT_FUEL: u64 = 200_000;
/// Compile-time rounds, unless the caller says.
pub const DEFAULT_MAX_ROUNDS: usize = 8;
/// Most mutants the run-time repair tries.
pub const MAX_MUTANTS: usize = 500;
/// Most candidates one compile-time round evaluates.
pub const MAX_CANDIDATES_PER_ROUND: usize = 200;
/// Longest token range tried when locating an expression in the source.
const MAX_LOCATE_TOKENS: usize = 256;
/// Longest call trace kept for a failing run.
const TRACE_CAP: usize = 64;
/// Rejections kept verbatim in a step log (the rest are only counted).
const KEPT_REJECTIONS: usize = 12;

// ── Checking ─────────────────────────────────────────────────────────

/// One error the checker reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagSummary {
    pub line: u32,
    pub col: u32,
    pub code: String,
    pub message: String,
}

impl DiagSummary {
    pub fn render(&self) -> String {
        if self.line == 0 {
            format!("[{}] {}", self.code, self.message)
        } else {
            format!("{}:{}: [{}] {}", self.line, self.col, self.code, self.message)
        }
    }
}

/// The errors `--check` reports for `source` (elision on, as `--check` runs).
pub fn errors_of(source: &str) -> Vec<DiagSummary> {
    crate::check::check_source(source, true, false)
        .findings
        .into_iter()
        .filter(|f| f.severity == "error")
        .map(|f| DiagSummary { line: f.line, col: f.col, code: f.code, message: f.message })
        .collect()
}

fn parse(source: &str) -> Option<Module> {
    parser::parse(&lexer::lex(source)).ok()
}

// ── Running tests and contract probes ────────────────────────────────

/// What a probe is: a `@test`, or a zero-argument function with contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    Test,
    Contract,
}

/// One run of one probe.
#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub name: String,
    pub kind: ProbeKind,
    pub passed: bool,
    /// `None` on a pass; the evaluator's error otherwise.
    pub error: Option<String>,
    /// The contract clause (`@req(n > 0)`) or assertion (`assert: msg`) that
    /// failed, when the error names one.
    pub failed_check: Option<String>,
    /// The call trace of a failing run, capped; empty on a pass.
    pub trace: Vec<String>,
    /// For a failing run observed with coverage: the site whose own
    /// evaluation raised the error (`f/binary#2: division by zero …`).
    pub raised_at: Option<String>,
}

/// The probes `intent` names: its `@test`s, then the zero-argument non-test
/// functions with contracts, which can be run without inventing inputs.
pub fn probes(intent: &Intent) -> Vec<(String, ProbeKind)> {
    let mut out: Vec<(String, ProbeKind)> =
        intent.tests.iter().map(|t| (t.clone(), ProbeKind::Test)).collect();
    for f in &intent.functions {
        if !f.is_test && !f.contracts.is_empty() && f.signature.starts_with(&format!("{}()", f.name)) {
            out.push((f.name.clone(), ProbeKind::Contract));
        }
    }
    out
}

/// The clause or assertion an evaluator error names, if any.
pub fn failed_check(error: &str) -> Option<String> {
    if let Some(rest) = error.split("contract violated: ").nth(1) {
        let mut parts = rest.split('`');
        parts.next();
        return parts.next().map(str::to_string);
    }
    if let Some(i) = error.find("assertion failed") {
        let tail = error[i + "assertion failed".len()..].trim_start_matches(':').trim();
        return Some(if tail.is_empty() { "assert".to_string() } else { format!("assert: {tail}") });
    }
    None
}

fn run_probe(module: &Module, name: &str, kind: ProbeKind, fuel: u64, traced: bool) -> Run {
    let (result, trace) = if traced {
        eval::run_bounded_traced(module, name, Vec::new(), fuel, TRACE_CAP)
    } else {
        (eval::run_bounded(module, name, Vec::new(), fuel), Vec::new())
    };
    let error = match result {
        Ok(_) => None,
        Err(BoundedError::FuelExhausted) => Some(format!("fuel exhausted ({fuel} units)")),
        Err(BoundedError::Error(e)) => Some(e),
    };
    Run {
        name: name.to_string(),
        kind,
        passed: error.is_none(),
        failed_check: error.as_deref().and_then(failed_check),
        trace: if error.is_some() { trace } else { Vec::new() },
        raised_at: None,
        error,
    }
}

/// Run every probe once; a failing one is run again with a call trace.
pub fn run_probes(module: &Module, probes: &[(String, ProbeKind)], fuel: u64) -> Vec<Run> {
    probes
        .iter()
        .map(|(name, kind)| {
            let r = run_probe(module, name, *kind, fuel, false);
            if r.passed { r } else { run_probe(module, name, *kind, fuel, true) }
        })
        .collect()
}

fn passes(module: &Module, name: &str, fuel: u64) -> Result<(), String> {
    match eval::run_bounded(module, name, Vec::new(), fuel) {
        Ok(_) => Ok(()),
        Err(BoundedError::FuelExhausted) => Err("fuel exhausted".into()),
        Err(BoundedError::Error(e)) => Err(e),
    }
}

// ── Where functions are, in tokens ───────────────────────────────────

/// A top-level function's body, located in the token stream.
#[derive(Debug, Clone)]
struct FnBody {
    name: String,
    /// Token indices of the body's `{` and its matching `}`.
    open: usize,
    close: usize,
}

fn matching_close(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        match t.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// The body of the function whose `f`/`af`/`uf` keyword is at `i`, named `name`.
fn fn_body_at(tokens: &[Token], i: usize, name: String) -> Option<FnBody> {
    let mut nest = 0i32;
    let mut open = None;
    for (j, u) in tokens.iter().enumerate().skip(i + 2) {
        match u.kind {
            TokenKind::LParen | TokenKind::LBrack => nest += 1,
            TokenKind::RParen | TokenKind::RBrack => nest -= 1,
            TokenKind::LBrace if nest == 0 => {
                open = Some(j);
                break;
            }
            TokenKind::Semi | TokenKind::KwF | TokenKind::Eof if nest == 0 => break,
            _ => {}
        }
    }
    let open = open?;
    let close = matching_close(tokens, open)?;
    Some(FnBody { name, open, close })
}

/// The type an `I` or `extend` header (keyword at `kw`, body brace at `open`)
/// names: the last identifier outside `<…>` after any `for`. `None` when there
/// is none. It must equal `eval::type_head_name` of the parsed type for the
/// methods to be found, so a header it reads differently finds no body.
fn impl_head_name(tokens: &[Token], kw: usize, open: usize) -> Option<String> {
    let header = &tokens[kw + 1..open];
    let start = header.iter().rposition(|t| t.kind == TokenKind::KwFor).map_or(0, |p| p + 1);
    let mut angle = 0i32;
    let mut name = None;
    for t in &header[start..] {
        match t.text.as_str() {
            "<" => angle += 1,
            ">" => angle -= 1,
            _ if angle == 0 && t.kind == TokenKind::Ident => name = Some(t.text.clone()),
            _ => {}
        }
    }
    name
}

/// The methods of an `I`/`extend` block whose braces are `open..=close`,
/// named `head::method`: the same name a site of the method carries.
fn method_bodies(tokens: &[Token], open: usize, close: usize, head: &str) -> Vec<FnBody> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut j = open + 1;
    while j < close {
        match tokens[j].kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth -= 1,
            TokenKind::KwF | TokenKind::KwAf | TokenKind::KwUf
                if depth == 0 && tokens.get(j + 1).is_some_and(|n| n.kind == TokenKind::Ident) =>
            {
                let name = format!("{head}::{}", tokens[j + 1].text);
                if let Some(b) = fn_body_at(tokens, j, name)
                    && b.close < close
                {
                    j = b.close + 1;
                    out.push(b);
                    continue;
                }
            }
            _ => {}
        }
        j += 1;
    }
    out
}

/// Every top-level function with a body, and every method of an `I` or
/// `extend` block as `Type::method`, in source order.
fn function_bodies(tokens: &[Token]) -> Vec<FnBody> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        match t.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth -= 1,
            TokenKind::KwF | TokenKind::KwAf | TokenKind::KwUf
                if depth == 0 && tokens.get(i + 1).is_some_and(|n| n.kind == TokenKind::Ident) =>
            {
                if let Some(b) = fn_body_at(tokens, i, tokens[i + 1].text.clone()) {
                    i = b.close + 1;
                    out.push(b);
                    continue;
                }
            }
            TokenKind::KwI | TokenKind::KwExtend if depth == 0 => {
                let open = tokens[i + 1..].iter().position(|u| u.kind == TokenKind::LBrace).map(|p| p + i + 1);
                if let Some(open) = open
                    && let Some(close) = matching_close(tokens, open)
                {
                    if let Some(head) = impl_head_name(tokens, i, open) {
                        out.extend(method_bodies(tokens, open, close, &head));
                    }
                    i = close + 1;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn tok_end(t: &Token) -> usize {
    t.span.offset + t.span.len
}

fn splice(source: &str, start: usize, end: usize, text: &str) -> String {
    let mut s = String::with_capacity(source.len() + text.len());
    s.push_str(&source[..start]);
    s.push_str(text);
    s.push_str(&source[end..]);
    s
}

// ── Compile-time healing ─────────────────────────────────────────────

/// One accepted compile-time fix, with the evidence it was accepted on.
#[derive(Debug, Clone, Serialize)]
pub struct HealStep {
    pub round: usize,
    /// The diagnostic the fix answers; `None` for a whole-source recovery.
    pub diagnostic: Option<DiagSummary>,
    pub fix_id: String,
    /// `heal pattern`, `compiler evidence`, or `recover:<stage>`.
    pub origin: String,
    pub description: String,
    pub semantics_preserving: bool,
    pub errors_before: usize,
    pub errors_after: usize,
    /// The `@test`s that passed before the step and were run again after it.
    pub tests_kept_passing: Vec<String>,
    /// How many candidates the round evaluated.
    pub candidates_considered: usize,
    /// How many were rejected, by rule.
    pub rejected: Vec<(String, usize)>,
    /// For a fix derived from a type mismatch: the checker's evidence.
    pub evidence: Option<MismatchEvidence>,
}

/// One rejected candidate and why, kept for the report.
#[derive(Debug, Clone, Serialize)]
pub struct Rejection {
    pub round: usize,
    pub fix_id: String,
    pub reason: String,
}

/// What the compile-time loop did.
#[derive(Debug, Clone, Serialize)]
pub struct HealOutcome {
    pub source: String,
    pub steps: Vec<HealStep>,
    pub remaining: Vec<DiagSummary>,
    pub rounds: usize,
    /// Why it stopped: `no errors`, `no candidate accepted`, `max rounds`.
    pub stopped: String,
    /// A sample of rejected candidates (the first few of each round).
    pub rejections: Vec<Rejection>,
    /// Set when the input did not parse, so its intent could not be read
    /// before the first fix: the intent then used is that of the first source
    /// that parsed.
    pub intent_note: Option<String>,
    /// Mismatches the checker reported in the last round for which no fix was
    /// proposed, each with the reason (not located, not a numeric conversion).
    pub unfixed: Vec<MismatchEvidence>,
}

/// The type checker's evidence for one mismatch.
#[derive(Debug, Clone, Serialize)]
pub struct MismatchEvidence {
    pub function: String,
    pub site: types::MismatchSite,
    /// The expression: its source text when located, else the checker's
    /// rendering of it.
    pub expr: String,
    pub expected: String,
    pub found: String,
    /// The diagnostic it explains.
    pub diagnostic: String,
    /// 1-based line and column of the located expression.
    pub line: Option<usize>,
    pub col: Option<usize>,
    /// Why no fix was proposed, when none was.
    pub note: Option<String>,
}

/// A changed source, and where it came from.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub fix_id: String,
    pub origin: String,
    pub description: String,
    pub diagnostic: Option<DiagSummary>,
    pub semantics_preserving: bool,
    /// `FixCandidate::confidence`: an ordering prior, not a probability.
    pub prior: f64,
    pub source: String,
    pub evidence: Option<MismatchEvidence>,
}

fn to_hir(d: &DiagSummary) -> hir::Diagnostic {
    hir::Diagnostic {
        severity: hir::Severity::Error,
        message: d.message.clone(),
        span: (d.line != 0).then_some(hir::Span { line: d.line, col: d.col }),
        id: Some(d.code.clone()),
        category: None,
    }
}

/// The rust-style name of a numeric type, for a cast: `i64`, `usize`, `f64`.
fn cast_name(t: &hir::Ty) -> Option<String> {
    match t {
        hir::Ty::Int(_) | hir::Ty::Uint(_) | hir::Ty::Float(_) => Some(t.to_string().to_lowercase()),
        _ => None,
    }
}

/// Whether `e` binds tighter than `as`, so `e as T` needs no parentheses.
fn is_atomic(e: &crate::ast::Expr) -> bool {
    use crate::ast::Expr;
    matches!(
        e,
        Expr::Literal { .. }
            | Expr::Ident { .. }
            | Expr::Call { .. }
            | Expr::MethodCall { .. }
            | Expr::FieldAccess { .. }
            | Expr::Index { .. }
            | Expr::TupleLit { .. }
            | Expr::ArrayLit { .. }
    )
}

/// The elided rendering two expressions are compared by.
fn expr_key(e: &crate::ast::Expr) -> String {
    format!("{:?}", crate::elision::elide_expr(e))
}

/// Whether `tokens[i..=j]` re-parses, alone, to an expression whose key is `key`.
fn range_is(tokens: &[Token], i: usize, j: usize, key: &str) -> bool {
    let mut slice: Vec<Token> = tokens[i..=j].to_vec();
    let eof = tokens.last().filter(|t| t.kind == TokenKind::Eof).cloned().unwrap_or_else(|| Token {
        kind: TokenKind::Eof,
        span: tokens[j].span.clone(),
        text: String::new(),
    });
    slice.push(eof);
    parser::parse_expression(&slice).is_ok_and(|e| expr_key(&e) == key)
}

/// Every token range inside `lo..hi` (inclusive start, exclusive end) that
/// re-parses to `key`. Only bracket-balanced ranges are parsed. `suffix`
/// restricts the search to ranges ending at `hi - 1`.
fn ranges_matching(tokens: &[Token], lo: usize, hi: usize, key: &str, suffix: bool) -> Vec<(usize, usize)> {
    ranges_ending_where(tokens, lo, hi, key, |j| !suffix || j + 1 == hi)
}

/// [`ranges_matching`] over the ranges whose last token `j` satisfies `ends`.
fn ranges_ending_where(
    tokens: &[Token],
    lo: usize,
    hi: usize,
    key: &str,
    ends: impl Fn(usize) -> bool,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in lo..hi {
        let mut depth = 0i32;
        for j in i..hi.min(i + MAX_LOCATE_TOKENS) {
            match tokens[j].kind {
                TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => depth += 1,
                TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                break;
            }
            if depth == 0 && ends(j) && range_is(tokens, i, j, key) {
                out.push((i, j));
            }
        }
    }
    // Parentheses make no node, so `(x)` re-parses to what `x` does. A range
    // that contains another match is the same expression wrapped: keep the
    // innermost.
    let all = out.clone();
    out.retain(|&(i, j)| !all.iter().any(|&(a, b)| (a, b) != (i, j) && i <= a && b <= j));
    out
}

/// The token ranges of the items in `tokens[from..to]` (exclusive end), split
/// at commas outside any bracket. Empty items are skipped.
fn top_level_items(tokens: &[Token], from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = from;
    let mut depth = 0i32;
    for k in from..to {
        match tokens[k].kind {
            TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                if start < k {
                    out.push((start, k - 1));
                }
                start = k + 1;
            }
            _ => {}
        }
    }
    if start < to {
        out.push((start, to - 1));
    }
    out
}

/// The token ranges of the arguments of the call that starts at `lo` and
/// ends at `hi - 1` (its closing parenthesis), split at top-level commas.
fn call_args(tokens: &[Token], lo: usize, hi: usize) -> Option<Vec<(usize, usize)>> {
    if tokens.get(hi - 1)?.kind != TokenKind::RParen {
        return None;
    }
    // The `(` matching the closing one.
    let mut depth = 0i32;
    let mut open = None;
    for k in (lo..hi).rev() {
        match tokens[k].kind {
            TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth += 1,
            TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => {
                depth -= 1;
                if depth == 0 {
                    open = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    let open = open?;
    let mut out = Vec::new();
    let mut start = open + 1;
    let mut depth = 0i32;
    for k in open + 1..hi - 1 {
        match tokens[k].kind {
            TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                out.push((start, k.checked_sub(1)?));
                start = k + 1;
            }
            _ => {}
        }
    }
    if start < hi - 1 {
        out.push((start, hi - 2));
    }
    Some(out)
}

/// Where the mismatched expression is in `source`, as an inclusive token
/// range, or why it could not be placed. The proof of a location is that the
/// range re-parses to the checker's expression and that no other range in the
/// search space does.
fn locate(tokens: &[Token], bodies: &[FnBody], m: &types::Mismatch) -> Result<(usize, usize), String> {
    let Some(b) = bodies.iter().find(|b| b.name == m.function) else {
        return Err(format!("could not locate: `{}` is not a top-level function with a body", m.function));
    };
    let (lo, hi) = (b.open + 1, b.close);
    let key = expr_key(&m.expr);
    let unique = |found: Vec<(usize, usize)>, within: &str| -> Result<(usize, usize), String> {
        match found.as_slice() {
            [one] => Ok(*one),
            [] => Err(format!("could not locate: no token range {within} re-parses to the expression")),
            many => Err(format!(
                "could not locate: {} token ranges {within} re-parse to the expression",
                many.len()
            )),
        }
    };
    match (&m.site, &m.context) {
        // The tail expression ends right before the body's `}`.
        (types::MismatchSite::Return, _) => {
            unique(ranges_matching(tokens, lo, hi, &key, true), &format!("ending the body of `{}`", m.function))
        }
        // An arm of an `if` or `match`, or an element of a list literal: the
        // enclosing expression must be unique, and the arm must be unique in it.
        (
            types::MismatchSite::IfArm { .. }
            | types::MismatchSite::MatchArm { .. }
            | types::MismatchSite::ListElement { .. },
            Some(ctx),
        ) => {
            let ckey = expr_key(ctx);
            let (cs, ce) = unique(ranges_matching(tokens, lo, hi, &ckey, false), &format!("in `{}`", m.function))?;
            match &m.site {
                // An arm's value ends before the `}` of its block.
                types::MismatchSite::IfArm { .. } => unique(
                    ranges_ending_where(tokens, cs, ce + 1, &key, |j| {
                        j + 1 <= ce && tokens[j + 1].kind == TokenKind::RBrace
                    }),
                    "ending an arm of the `if`",
                ),
                // An arm's value ends before the `}` of its block, or before the
                // comma that separates it from the next arm.
                types::MismatchSite::MatchArm { .. } => unique(
                    ranges_ending_where(tokens, cs, ce + 1, &key, |j| {
                        j + 1 <= ce && matches!(tokens[j + 1].kind, TokenKind::RBrace | TokenKind::Comma)
                    }),
                    "ending an arm of the `match`",
                ),
                // The element is the item at its position in the literal, read
                // by the grammar's commas, when the text alone is not unique.
                types::MismatchSite::ListElement { index } => {
                    if tokens[cs].kind != TokenKind::LBrack || tokens[ce].kind != TokenKind::RBrack {
                        return Err(format!("could not locate: the list of `{}` is not a `[...]` literal", m.function));
                    }
                    let inside = ranges_matching(tokens, cs, ce + 1, &key, false);
                    if inside.len() == 1 {
                        return Ok(inside[0]);
                    }
                    match top_level_items(tokens, cs + 1, ce).get(*index).copied() {
                        Some((i, j)) if range_is(tokens, i, j, &key) => Ok((i, j)),
                        _ => unique(inside, "in the list"),
                    }
                }
                _ => Err(format!("could not locate: `{}` has no arm or element site here", m.function)),
            }
        }
        (site, Some(ctx)) => {
            let ckey = expr_key(ctx);
            let Ok((cs, ce)) = unique(ranges_matching(tokens, lo, hi, &ckey, false), "") else {
                // The enclosing expression is not unique; the expression alone may be.
                return unique(ranges_matching(tokens, lo, hi, &key, false), &format!("in `{}`", m.function));
            };
            match site {
                // The right operand ends where the binary expression ends.
                types::MismatchSite::BinaryOperand { .. } => {
                    unique(ranges_matching(tokens, cs, ce + 1, &key, true), "ending the binary expression")
                }
                types::MismatchSite::CallArg { index, .. } => {
                    let inside = ranges_matching(tokens, cs, ce + 1, &key, false);
                    if inside.len() == 1 {
                        return Ok(inside[0]);
                    }
                    // Several equal sub-expressions: the argument list decides,
                    // read by the grammar's commas.
                    match call_args(tokens, cs, ce + 1).and_then(|a| a.get(*index).copied()) {
                        Some((i, j)) if range_is(tokens, i, j, &key) => Ok((i, j)),
                        _ => unique(inside, "in the call"),
                    }
                }
                _ => unique(ranges_matching(tokens, cs, ce + 1, &key, false), "in its context"),
            }
        }
        _ => unique(ranges_matching(tokens, lo, hi, &key, false), &format!("in `{}`", m.function)),
    }
}

/// The candidates for one round, and the mismatches no candidate answers.
///
/// From the type checker's evidence: a numeric conversion of the
/// mismatched expression, at the one place it is. From `heal::heal_one`: its
/// fixes for a diagnostic that has a position; a diagnostic without one gets
/// none (there is nowhere to put it). From `recover`: a whole-source
/// recovery when the source does not parse. Generation order, deduplicated
/// by the source produced, at most [`MAX_CANDIDATES_PER_ROUND`].
pub fn candidates_with_evidence(source: &str, errors: &[DiagSummary]) -> (Vec<Candidate>, Vec<MismatchEvidence>) {
    let mut out: Vec<Candidate> = Vec::new();
    let mut unfixed = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |c: Candidate, out: &mut Vec<Candidate>| {
        if c.source != source && out.len() < MAX_CANDIDATES_PER_ROUND && seen.insert(c.source.clone()) {
            out.push(c);
        }
    };
    let tokens = lexer::lex(source);
    let bodies = function_bodies(&tokens);
    if let Some(module) = parse(source) {
        let (diags, mismatches) = types::check_with_evidence(&crate::elision::elide(&module));
        for m in &mismatches {
            let message = diags.get(m.diag_index).map(|d| d.message.clone()).unwrap_or_default();
            let mut ev = MismatchEvidence {
                function: m.function.clone(),
                site: m.site.clone(),
                expr: crate::fmt::expr_to_string(&m.expr),
                expected: m.expected.to_string(),
                found: m.found.to_string(),
                diagnostic: message.clone(),
                line: None,
                col: None,
                note: None,
            };
            let (Some(to), Some(_)) = (cast_name(&m.expected), cast_name(&m.found)) else {
                ev.note = Some(format!(
                    "no fix: the evidence determines a conversion only between numeric types ({} -> {})",
                    ev.found, ev.expected
                ));
                unfixed.push(ev);
                continue;
            };
            let (i, j) = match locate(&tokens, &bodies, m) {
                Ok(r) => r,
                Err(why) => {
                    ev.note = Some(why);
                    unfixed.push(ev);
                    continue;
                }
            };
            let (start, end) = (tokens[i].span.offset, tok_end(&tokens[j]));
            let text = &source[start..end];
            let (line, col) = line_col(source, start);
            ev.expr = text.to_string();
            ev.line = Some(line);
            ev.col = Some(col);
            let cast = if is_atomic(&m.expr) { format!("{text} as {to}") } else { format!("({text}) as {to}") };
            let diagnostic = errors.iter().find(|d| d.message == message).cloned();
            push(
                Candidate {
                    fix_id: "insert-numeric-cast".into(),
                    origin: "compiler evidence".into(),
                    description: format!("`{text}` -> `{cast}` at {line}:{col}"),
                    diagnostic,
                    // A numeric conversion can truncate or change sign.
                    semantics_preserving: false,
                    prior: 1.0,
                    source: splice(source, start, end, &cast),
                    evidence: Some(ev),
                },
                &mut out,
            );
        }
    }
    for d in errors.iter().filter(|d| d.line != 0) {
        let healed = heal::heal_one(&to_hir(d));
        for fix in healed.fixes.iter().filter(|f| !f.edits.is_empty()) {
            if let Some(new) = recover::apply_text_edits(source, &fix.edits) {
                push(
                    Candidate {
                        fix_id: fix.id.clone(),
                        origin: "heal pattern".into(),
                        description: fix.description.clone(),
                        diagnostic: Some(d.clone()),
                        semantics_preserving: fix.semantics_preserving,
                        prior: fix.confidence,
                        source: new,
                        evidence: None,
                    },
                    &mut out,
                );
            }
        }
    }
    if parse(source).is_none() {
        let r = recover::recover(source);
        if r.parsed_ok {
            push(
                Candidate {
                    fix_id: format!("recover:{}", r.stage.as_str()),
                    origin: format!("recover:{}", r.stage.as_str()),
                    description: format!(
                        "parse recovery ({}, {} pattern candidate(s) tried)",
                        r.stage.as_str(),
                        r.candidates_tried
                    ),
                    diagnostic: errors.first().cloned(),
                    // Recovery may delete tokens; never claimed preserving.
                    semantics_preserving: false,
                    prior: 0.0,
                    source: r.source,
                    evidence: None,
                },
                &mut out,
            );
        }
    }
    (out, unfixed)
}

/// [`candidates_with_evidence`]'s candidates.
pub fn candidates(source: &str, errors: &[DiagSummary]) -> Vec<Candidate> {
    candidates_with_evidence(source, errors).0
}

/// 1-based line and column of byte offset `at`.
fn line_col(source: &str, at: usize) -> (usize, usize) {
    let before = &source[..at.min(source.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
    (line, col)
}

/// The static half of a candidate's judgement: the errors it leaves, or why
/// it is rejected before any test is run.
///
/// Rejected when the error count does not strictly drop, or when `reference`
/// (the intent being kept) is not kept by the candidate.
pub fn judge_static(
    candidate: &str,
    errors_before: usize,
    reference: Option<&Intent>,
) -> Result<Vec<DiagSummary>, String> {
    let after = errors_of(candidate);
    if after.len() >= errors_before {
        return Err(format!("error count did not drop ({errors_before} -> {})", after.len()));
    }
    if let Some(reference) = reference {
        let Some(now) = intent::infer_source(candidate) else {
            return Err("intent not preserved: the candidate does not parse".into());
        };
        let v = reference.violations_in(&now);
        if !v.is_empty() {
            return Err(format!("intent not preserved: {}", v.join("; ")));
        }
    }
    Ok(after)
}

/// The dynamic half: every test in `passing` must still pass.
fn judge_tests(candidate: &str, passing: &[String], fuel: u64) -> Result<(), String> {
    if passing.is_empty() {
        return Ok(());
    }
    let Some(module) = parse(candidate) else {
        return Err("a passing @test regressed: the candidate does not parse".into());
    };
    for t in passing {
        if let Err(e) = passes(&module, t, fuel) {
            return Err(format!("a passing @test regressed: `{t}`: {e}"));
        }
    }
    Ok(())
}

fn reason_key(reason: &str) -> String {
    reason.split(':').next().unwrap_or(reason).split(" (").next().unwrap_or(reason).to_string()
}

/// The compile-time self-healing loop. See the module documentation for the
/// acceptance rules. `intent` is the intent to keep; `None` when the input
/// did not parse, in which case the first parsing source's intent is used.
pub fn heal_loop(source: &str, intent: Option<&Intent>, max_rounds: usize, fuel: u64) -> HealOutcome {
    let mut current = source.to_string();
    let mut reference: Option<Intent> = intent.cloned();
    let mut intent_note = None;
    let mut steps = Vec::new();
    let mut rejections = Vec::new();
    let mut errors = errors_of(&current);
    let mut rounds = 0;
    let mut unfixed = Vec::new();
    let stopped;
    loop {
        if errors.is_empty() {
            stopped = "no errors".to_string();
            break;
        }
        if rounds == max_rounds {
            stopped = "max rounds".to_string();
            break;
        }
        rounds += 1;
        if reference.is_none()
            && let Some(i) = intent::infer_source(&current)
        {
            reference = Some(i);
        }
        // The tests that pass now are the ones a fix must keep passing.
        let passing: Vec<String> = match (&reference, parse(&current)) {
            (Some(r), Some(m)) => r.tests.iter().filter(|t| passes(&m, t, fuel).is_ok()).cloned().collect(),
            _ => Vec::new(),
        };
        let (cands, left) = candidates_with_evidence(&current, &errors);
        unfixed = left;
        let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
        let mut kept = 0;
        let mut reject = |fix_id: &str, reason: String, tally: &mut std::collections::BTreeMap<String, usize>| {
            *tally.entry(reason_key(&reason)).or_default() += 1;
            if kept < KEPT_REJECTIONS {
                kept += 1;
                rejections.push(Rejection { round: rounds, fix_id: fix_id.to_string(), reason });
            }
        };
        // Static judgement of every candidate, then the survivors ranked.
        let mut survivors: Vec<(usize, Vec<DiagSummary>)> = Vec::new();
        for (k, c) in cands.iter().enumerate() {
            match judge_static(&c.source, errors.len(), reference.as_ref()) {
                Ok(after) => survivors.push((k, after)),
                Err(reason) => reject(&c.fix_id, reason, &mut tally),
            }
        }
        survivors.sort_by(|(a, ea), (b, eb)| {
            let (ca, cb) = (&cands[*a], &cands[*b]);
            cb.semantics_preserving
                .cmp(&ca.semantics_preserving)
                .then(ea.len().cmp(&eb.len()))
                .then(cb.prior.partial_cmp(&ca.prior).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.cmp(b))
        });
        let mut accepted = None;
        for (k, after) in survivors {
            match judge_tests(&cands[k].source, &passing, fuel) {
                Ok(()) => {
                    accepted = Some((k, after));
                    break;
                }
                Err(reason) => reject(&cands[k].fix_id, reason, &mut tally),
            }
        }
        let Some((k, after)) = accepted else {
            stopped = "no candidate accepted".to_string();
            break;
        };
        let c = &cands[k];
        if reference.is_none() {
            intent_note = Some(format!(
                "the input does not parse, so its intent could not be read; the intent kept from \
                 round {rounds} on is that of the source `{}` produced",
                c.fix_id
            ));
        }
        steps.push(HealStep {
            round: rounds,
            diagnostic: c.diagnostic.clone(),
            fix_id: c.fix_id.clone(),
            origin: c.origin.clone(),
            description: c.description.clone(),
            semantics_preserving: c.semantics_preserving,
            errors_before: errors.len(),
            errors_after: after.len(),
            tests_kept_passing: passing.clone(),
            candidates_considered: cands.len(),
            rejected: tally.into_iter().collect(),
            evidence: c.evidence.clone(),
        });
        current = c.source.clone();
        errors = after;
    }
    if errors.is_empty() {
        unfixed.clear();
    }
    HealOutcome { source: current, steps, remaining: errors, rounds, stopped, rejections, intent_note, unfixed }
}

// ── Run-time debugging and repair ────────────────────────────────────

/// One site the failing runs executed: its fault-localization score, what
/// the runs saw there, and what was tried at it.
#[derive(Debug, Clone, Serialize)]
pub struct SiteEvidence {
    /// `function/kind#index`, or `function/return` for a returned literal.
    pub site: String,
    pub function: String,
    /// `binary`, `if`, or `return`.
    pub kind: String,
    /// The operator for a binary site, `if`, or the returned literal.
    pub op: String,
    /// Ochiai: failed / sqrt(total_failed x (failed + passed)). `None` for a
    /// `return` site, which comes from a failed `@ens`, not from coverage.
    pub ochiai: Option<f64>,
    /// Failing and passing runs that executed the site.
    pub failed: usize,
    pub passed: usize,
    /// What the failing runs observed here (`t: 18 > 18 -> false`).
    pub observed: Vec<String>,
    /// Each candidate tried here and rejected, with the reason.
    pub rejected: Vec<(String, String)>,
}

/// An accepted run-time repair and its evidence.
#[derive(Debug, Clone, Serialize)]
pub struct Repair {
    pub function: String,
    /// What changed, e.g. "`>` -> `>=`".
    pub mutation: String,
    pub line: usize,
    pub col: usize,
    pub site: String,
    pub ochiai: Option<f64>,
    pub failed: usize,
    pub passed: usize,
    /// Why this candidate was tried before the others at its site.
    pub motivation: String,
    /// What the failing runs observed at the site.
    pub observed: Vec<String>,
    /// Always false: a repair that makes a failing test pass changes what the
    /// program computes.
    pub semantics_preserving: bool,
    /// The runs that failed before the repair and pass after it.
    pub fixed: Vec<String>,
    /// Every probe run after the repair (all pass, or it was not accepted).
    pub runs_after: Vec<Run>,
    pub source: String,
}

/// What the run-time phase found and did.
#[derive(Debug, Clone, Serialize)]
pub struct DebugReport {
    pub runs: Vec<Run>,
    pub repair: Option<Repair>,
    pub mutants_tried: usize,
    pub mutant_cap: usize,
    /// The sites the failing runs executed, in the order they were tried.
    pub sites: Vec<SiteEvidence>,
    pub note: Option<String>,
}

impl DebugReport {
    pub fn failing(&self) -> usize {
        self.runs.iter().filter(|r| !r.passed).count()
    }

    fn empty(note: Option<String>) -> Self {
        DebugReport { runs: Vec::new(), repair: None, mutants_tried: 0, mutant_cap: MAX_MUTANTS, sites: Vec::new(), note }
    }
}

/// Run every probe once with coverage and a call trace.
fn observe_probes(module: &Module, probes: &[(String, ProbeKind)], fuel: u64) -> Vec<(Run, eval::CoverageData)> {
    probes
        .iter()
        .map(|(name, kind)| {
            let o = eval::run_bounded_observed(module, name, Vec::new(), fuel, Some(TRACE_CAP), true);
            let error = match o.result {
                Ok(_) => None,
                Err(BoundedError::FuelExhausted) => Some(format!("fuel exhausted ({fuel} units)")),
                Err(BoundedError::Error(e)) => Some(e),
            };
            let cov = o.coverage.unwrap_or_default();
            let run = Run {
                name: name.clone(),
                kind: *kind,
                passed: error.is_none(),
                failed_check: error.as_deref().and_then(failed_check),
                trace: if error.is_some() { o.trace } else { Vec::new() },
                raised_at: if error.is_some() {
                    cov.raised.as_ref().map(|(s, m)| format!("{}: {m}", s.render()))
                } else {
                    None
                },
                error,
            };
            (run, cov)
        })
        .collect()
}

/// The function a contract violation names: "… of `f` …".
fn contract_function(error: &str) -> Option<&str> {
    let rest = error.split("contract violated: ").nth(1)?;
    rest.split("` of `").nth(1)?.split('`').next()
}

/// One single-site change of the tree.
#[derive(Debug, Clone, PartialEq)]
enum Mutation {
    /// Replace a binary operator.
    Op(String),
    /// Replace the binary site's right operand, an integer literal.
    RightLiteral(String),
    /// Swap the `then` and `else` blocks of an `if`.
    Swap,
    /// Negate an `if` condition.
    Negate,
    /// Replace the integer literal a function returns.
    TailLiteral(String),
}

/// A site to repair at, and the candidates for it, motivated first.
struct SitePlan {
    evidence: SiteEvidence,
    /// `None` for a `return` site.
    site: Option<eval::SiteId>,
    /// The source position of the site's function among the items.
    item: usize,
    preorder: usize,
    /// `(mutation, its rendering, motivation)`; motivated ones first.
    candidates: Vec<(Mutation, String, String)>,
    motivated: bool,
}

const NO_SPECIFIC: &str = "a small mutation at a ranked site; no observation singles it out";

fn swap_table(op: &str, table: &[(&str, &str)]) -> Option<String> {
    table.iter().find(|(a, _)| *a == op).map(|(_, b)| b.to_string())
}

const INCLUSIVE: &[(&str, &str)] = &[(">", ">="), (">=", ">"), ("<", "<="), ("<=", "<")];
const NEGATION: &[(&str, &str)] = &[("<", ">="), (">=", "<"), (">", "<="), ("<=", ">"), ("==", "!="), ("!=", "==")];
const REVERSAL: &[(&str, &str)] = &[("<", ">"), (">", "<"), ("<=", ">="), (">=", "<=")];

fn is_decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// `n+1` and, when not negative, `n-1`, in the order `up_first` says.
fn plus_minus_one(text: &str, up_first: bool) -> Vec<String> {
    let Ok(n) = text.parse::<i64>() else { return Vec::new() };
    let up = n.checked_add(1).map(|m| m.to_string());
    let down = n.checked_sub(1).filter(|m| *m >= 0).map(|m| m.to_string());
    let v = if up_first { [up, down] } else { [down, up] };
    v.into_iter().flatten().collect()
}

/// The candidates at a coverage site, ordered by what the runs observed.
fn plan_site(
    node: &crate::ast::Expr,
    id: &eval::SiteId,
    failing: &[&(Run, eval::CoverageData)],
    passing: &[&(Run, eval::CoverageData)],
) -> (Vec<(Mutation, String, String)>, String, Vec<String>) {
    use crate::ast::{Expr, LiteralKind};
    let mut motivated: Vec<(Mutation, String, String)> = Vec::new();
    let mut rest: Vec<(Mutation, String, String)> = Vec::new();
    let mut observed = Vec::new();
    let op_text;
    match node {
        Expr::Binary { op, right, .. } => {
            op_text = op.clone();
            for (run, cov) in failing {
                if let Some(rec) = cov.get(id) {
                    // Equal operands first: they are what motivates the
                    // inclusive/exclusive swap, so they must survive the cut.
                    let mut shown: Vec<&eval::Observation> = rec.observations.iter().collect();
                    shown.sort_by_key(|o| o.left != o.right);
                    for o in shown.into_iter().take(4) {
                        observed.push(format!("{}: {} {op} {} -> {}", run.name, o.left, o.right, o.result));
                    }
                    if rec.observations.is_empty() {
                        observed.push(format!("{}: executed {} time(s)", run.name, rec.hits));
                    }
                }
            }
            let render = |to: &str| format!("`{op}` -> `{to}`");
            if INCLUSIVE.iter().chain(NEGATION).any(|(a, _)| *a == op.as_str()) {
                let equal: Vec<&str> = failing
                    .iter()
                    .filter(|(_, c)| c.get(id).is_some_and(|r| r.equal_operands))
                    .map(|(r, _)| r.name.as_str())
                    .collect();
                if let Some(to) = swap_table(op, INCLUSIVE) {
                    if equal.is_empty() {
                        rest.push((Mutation::Op(to.clone()), render(&to), NO_SPECIFIC.into()));
                    } else {
                        let why = format!(
                            "the comparison saw equal operands in failing run(s) {}; \
                             inclusive and exclusive differ exactly there",
                            equal.join(", ")
                        );
                        motivated.push((Mutation::Op(to.clone()), render(&to), why));
                    }
                }
                for table in [NEGATION, REVERSAL] {
                    if let Some(to) = swap_table(op, table) {
                        rest.push((Mutation::Op(to.clone()), render(&to), NO_SPECIFIC.into()));
                    }
                }
            } else if op == "+" || op == "-" {
                let to = if op == "+" { "-" } else { "+" };
                let ens: Vec<&str> = failing
                    .iter()
                    .filter_map(|(r, _)| {
                        let c = r.failed_check.as_deref()?;
                        let named = contract_function(r.error.as_deref()?)? == id.function;
                        (c.starts_with("@ens") && (c.contains('<') || c.contains('>')) && named).then_some(c)
                    })
                    .collect();
                if ens.is_empty() {
                    rest.push((Mutation::Op(to.into()), render(to), NO_SPECIFIC.into()));
                } else {
                    let why = format!(
                        "a failed sign/ordering postcondition of `{}` ({}) and an arithmetic site in it",
                        id.function,
                        ens.join(", ")
                    );
                    motivated.push((Mutation::Op(to.into()), render(to), why));
                }
            }
            if let Expr::Literal { value, kind: LiteralKind::Int } = right.as_ref()
                && is_decimal(value)
                && (is_arith(op) || INCLUSIVE.iter().chain(NEGATION).any(|(a, _)| *a == op.as_str()))
            {
                for m in plus_minus_one(value, true) {
                    rest.push((Mutation::RightLiteral(m.clone()), format!("`{value}` -> `{m}`"), NO_SPECIFIC.into()));
                }
            }
        }
        Expr::If { else_block, .. } => {
            op_text = "if".into();
            let arms = |runs: &[&(Run, eval::CoverageData)]| {
                runs.iter().fold((0u64, 0u64), |(t, e), (_, c)| match c.get(id) {
                    Some(r) => (t + r.then_taken, e + r.else_taken),
                    None => (t, e),
                })
            };
            let (ft, fe) = arms(failing);
            let (pt, pe) = arms(passing);
            for (run, cov) in failing {
                if let Some(r) = cov.get(id) {
                    observed.push(format!("{}: then x{}, else x{}", run.name, r.then_taken, r.else_taken));
                }
            }
            let (m, text) = if else_block.is_some() {
                (Mutation::Swap, "swap `if`/`else` branches".to_string())
            } else {
                (Mutation::Negate, "negate the `if` condition".to_string())
            };
            let only_failing = (ft > 0 && pt == 0).then_some("then").or((fe > 0 && pe == 0).then_some("else"));
            match only_failing {
                Some(arm) => {
                    let why = format!("the `{arm}` arm was taken only in failing runs");
                    motivated.push((m, text, why));
                }
                None => rest.push((m, text, NO_SPECIFIC.into())),
            }
        }
        _ => op_text = String::new(),
    }
    let mut out = motivated;
    for c in rest {
        if !out.iter().any(|o| o.0 == c.0) {
            out.push(c);
        }
    }
    (out, op_text, observed)
}

fn is_arith(op: &str) -> bool {
    matches!(op, "+" | "-" | "*" | "/" | "%")
}

/// The integer literal a function returns as its tail, if it does.
fn tail_literal(fd: &crate::ast::FunctionDef) -> Option<&str> {
    use crate::ast::{Expr, LiteralKind};
    let tail = fd.body_expr.as_deref().or(fd.body.tail_expr.as_deref())?;
    match tail {
        Expr::Literal { value, kind: LiteralKind::Int } if is_decimal(value) => Some(value),
        _ => None,
    }
}

/// Where the function a site names lives: an item of the module, and for a
/// method, its member index within that `I` or `extend` block.
#[derive(Clone, Copy)]
struct FnLoc {
    item: usize,
    member: Option<usize>,
}

/// The function `name` names: a top-level function, or a method as
/// `Type::method`. A name given twice is refused, not picked from: the
/// interpreter keeps one of two same-named methods, and which one ran is not
/// known from the name.
fn locate_function(module: &Module, name: &str) -> Result<FnLoc, String> {
    use crate::ast::ItemKind;
    let tops: Vec<usize> = module
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| matches!(&it.kind, ItemKind::Function(fd) if fd.name == name))
        .map(|(i, _)| i)
        .collect();
    match tops.as_slice() {
        [i] => return Ok(FnLoc { item: *i, member: None }),
        [_, _, ..] => return Err(format!("`{name}` is defined more than once")),
        [] => {}
    }
    let Some((ty, method)) = name.split_once("::") else {
        return Err(format!("no function `{name}` in the module"));
    };
    let mut found = Vec::new();
    for (i, it) in module.items.iter().enumerate() {
        let (target, members) = match &it.kind {
            ItemKind::Impl(ib) => (&ib.self_type, &ib.items),
            ItemKind::Extend(eb) => (&eb.target_type, &eb.items),
            _ => continue,
        };
        if eval::type_head_name(target).as_deref() != Some(ty) {
            continue;
        }
        for (k, m) in members.iter().enumerate() {
            if matches!(&m.kind, ItemKind::Function(fd) if fd.name == method) {
                found.push(FnLoc { item: i, member: Some(k) });
            }
        }
    }
    match found.as_slice() {
        [loc] => Ok(*loc),
        [] => Err(format!("no method `{name}` in the module")),
        _ => Err(format!("method `{name}` is defined more than once")),
    }
}

fn function_mut<'a>(module: &'a mut Module, name: &str) -> Result<&'a mut crate::ast::FunctionDef, String> {
    use crate::ast::ItemKind;
    let loc = locate_function(module, name)?;
    let it = &mut module.items[loc.item];
    let kind = match (&mut it.kind, loc.member) {
        (ItemKind::Function(fd), None) => return Ok(fd),
        (ItemKind::Impl(ib), Some(k)) => &mut ib.items[k].kind,
        (ItemKind::Extend(eb), Some(k)) => &mut eb.items[k].kind,
        _ => return Err(format!("`{name}` is not a function")),
    };
    match kind {
        ItemKind::Function(fd) => Ok(fd),
        _ => Err(format!("`{name}` is not a function")),
    }
}

fn function_ref<'a>(module: &'a Module, name: &str) -> Result<&'a crate::ast::FunctionDef, String> {
    use crate::ast::ItemKind;
    let loc = locate_function(module, name)?;
    let kind = match (&module.items[loc.item].kind, loc.member) {
        (ItemKind::Function(fd), None) => return Ok(fd),
        (ItemKind::Impl(ib), Some(k)) => &ib.items[k].kind,
        (ItemKind::Extend(eb), Some(k)) => &eb.items[k].kind,
        _ => return Err(format!("`{name}` is not a function")),
    };
    match kind {
        ItemKind::Function(fd) => Ok(fd),
        _ => Err(format!("`{name}` is not a function")),
    }
}

/// Every function of the module that a site can name, with its item index:
/// each top-level function, and each method of an `I` or `extend` block as
/// `Type::method`. A method whose type has no head name is left out.
fn callables(module: &Module) -> Vec<(usize, String, &crate::ast::FunctionDef)> {
    use crate::ast::ItemKind;
    let mut out = Vec::new();
    for (i, it) in module.items.iter().enumerate() {
        let (target, members) = match &it.kind {
            ItemKind::Function(fd) => {
                out.push((i, fd.name.clone(), fd));
                continue;
            }
            ItemKind::Impl(ib) => (&ib.self_type, &ib.items),
            ItemKind::Extend(eb) => (&eb.target_type, &eb.items),
            _ => continue,
        };
        let Some(ty) = eval::type_head_name(target) else { continue };
        for m in members {
            if let ItemKind::Function(fd) = &m.kind {
                out.push((i, format!("{ty}::{}", fd.name), fd));
            }
        }
    }
    out
}

/// The tree with one mutation applied, or `None` when the site is not there.
fn mutate(module: &Module, function: &str, site: Option<&eval::SiteId>, m: &Mutation) -> Option<Module> {
    use crate::ast::Expr;
    let mut out = module.clone();
    let fd = function_mut(&mut out, function).ok()?;
    if let Mutation::TailLiteral(new) = m {
        let tail = match &mut fd.body_expr {
            Some(e) => e.as_mut(),
            None => fd.body.tail_expr.as_deref_mut()?,
        };
        let Expr::Literal { value, .. } = tail else { return None };
        *value = new.clone();
        return Some(out);
    }
    let site = site?;
    let node = eval::site_mut(fd, site.kind, site.index)?;
    match (m, node) {
        (Mutation::Op(new), Expr::Binary { op, .. }) => *op = new.clone(),
        (Mutation::RightLiteral(new), Expr::Binary { right, .. }) => {
            let Expr::Literal { value, .. } = right.as_mut() else { return None };
            *value = new.clone();
        }
        (Mutation::Swap, Expr::If { then_block, else_block: Some(e), .. }) => std::mem::swap(then_block, e),
        (Mutation::Negate, Expr::If { cond, .. }) => {
            let inner = std::mem::replace(cond.as_mut(), Expr::Continue);
            **cond = Expr::Unary { op: "!".into(), operand: Box::new(inner) };
        }
        _ => return None,
    }
    Some(out)
}

/// Operator, `if` and integer-literal events of `fd`'s body in source order,
/// as `(node address, token text)`: a binary operator between its operands,
/// a prefix operator and `if` before theirs.
fn token_events(fd: &crate::ast::FunctionDef) -> Vec<(usize, String)> {
    use crate::ast::{Expr, LiteralKind};
    fn go(e: &Expr, out: &mut Vec<(usize, String)>) {
        let addr = e as *const Expr as usize;
        match e {
            Expr::Binary { op, left, right } => {
                go(left, out);
                out.push((addr, op.clone()));
                go(right, out);
            }
            Expr::Unary { op, operand } => {
                out.push((addr, op.clone()));
                go(operand, out);
            }
            Expr::If { .. } => {
                out.push((addr, "if".into()));
                for c in eval::expr_children(e) {
                    go(c, out);
                }
            }
            Expr::Literal { value, kind: LiteralKind::Int } => out.push((addr, value.clone())),
            _ => {
                for c in eval::expr_children(e) {
                    go(c, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for r in eval::function_roots(fd) {
        go(r, &mut out);
    }
    out
}

/// The body token that the node at `addr` (with token text `text`) is
/// spelled by: the k-th token of that text, where the node is the k-th event
/// of that text. The counts must agree, or the mapping is refused.
fn token_of(
    tokens: &[Token],
    body: &FnBody,
    events: &[(usize, String)],
    addr: usize,
    text: &str,
    is_tok: impl Fn(&Token) -> bool,
) -> Result<usize, String> {
    let same: Vec<usize> = events.iter().filter(|(_, t)| t == text).map(|(a, _)| *a).collect();
    let rank = same.iter().position(|a| *a == addr).ok_or("the node is not among the function's events")?;
    let toks: Vec<usize> = (body.open + 1..body.close).filter(|&k| is_tok(&tokens[k])).collect();
    if toks.len() != same.len() {
        return Err(format!(
            "the function has {} `{text}` token(s) and {} `{text}` node(s), so the k-th token is not proof",
            toks.len(),
            same.len()
        ));
    }
    Ok(toks[rank])
}

/// The source with `m` applied at the site, by token, and the byte offset
/// of the change. The caller re-parses it and compares trees.
fn source_edit(
    source: &str,
    tokens: &[Token],
    body: &FnBody,
    fd: &crate::ast::FunctionDef,
    site: Option<&eval::SiteId>,
    m: &Mutation,
) -> Result<(String, usize), String> {
    use crate::ast::Expr;
    if let Mutation::TailLiteral(new) = m {
        let t = tokens.get(body.close.wrapping_sub(1)).ok_or("no tail token")?;
        if t.kind != TokenKind::IntLiteral || Some(t.text.as_str()) != tail_literal(fd) {
            return Err("the token before the body's `}` is not the returned literal".into());
        }
        return Ok((splice(source, t.span.offset, tok_end(t), new), t.span.offset));
    }
    let site = site.ok_or("no site")?;
    let node = eval::function_sites_named(fd, &site.function)
        .into_iter()
        .find(|(id, _)| id == site)
        .map(|(_, e)| e)
        .ok_or("the site is not in the function")?;
    let events = token_events(fd);
    match (m, node) {
        (Mutation::Op(new), Expr::Binary { op, .. }) => {
            let k = token_of(tokens, body, &events, node as *const Expr as usize, op, |t| t.text == *op)?;
            let t = &tokens[k];
            Ok((splice(source, t.span.offset, tok_end(t), new), t.span.offset))
        }
        (Mutation::RightLiteral(new), Expr::Binary { right, .. }) => {
            let Expr::Literal { value, .. } = right.as_ref() else { return Err("operand is not a literal".into()) };
            let addr = right.as_ref() as *const Expr as usize;
            let k = token_of(tokens, body, &events, addr, value, |t| {
                t.kind == TokenKind::IntLiteral && t.text == *value
            })?;
            let t = &tokens[k];
            Ok((splice(source, t.span.offset, tok_end(t), new), t.span.offset))
        }
        (Mutation::Swap | Mutation::Negate, Expr::If { .. }) => {
            let k = token_of(tokens, body, &events, node as *const Expr as usize, "if", |t| {
                t.kind == TokenKind::Question && t.text == "if"
            })?;
            let mut nest = 0i32;
            let mut then_open = None;
            for (j, u) in tokens.iter().enumerate().take(body.close).skip(k + 1) {
                match u.kind {
                    TokenKind::LParen | TokenKind::LBrack => nest += 1,
                    TokenKind::RParen | TokenKind::RBrack => nest -= 1,
                    TokenKind::LBrace if nest == 0 => {
                        then_open = Some(j);
                        break;
                    }
                    _ => {}
                }
            }
            let then_open = then_open.ok_or("no `{` after the condition")?;
            let then_close = matching_close(tokens, then_open).ok_or("unclosed `then` block")?;
            let at = tokens[k].span.offset;
            if *m == Mutation::Negate {
                let (cs, ce) = (tokens[k + 1].span.offset, tok_end(&tokens[then_open - 1]));
                let text = format!("!({})", &source[cs..ce]);
                return Ok((splice(source, cs, ce, &text), at));
            }
            let (e, eo) = (tokens.get(then_close + 1).ok_or("no else")?, tokens.get(then_close + 2).ok_or("no else")?);
            if e.kind != TokenKind::KwElse || eo.kind != TokenKind::LBrace {
                return Err("the `else` is not a plain block".into());
            }
            let else_close = matching_close(tokens, then_close + 2).ok_or("unclosed `else` block")?;
            let then_text = &source[tokens[then_open].span.offset..tok_end(&tokens[then_close])];
            let else_text = &source[eo.span.offset..tok_end(&tokens[else_close])];
            let mut s = String::with_capacity(source.len());
            s.push_str(&source[..tokens[then_open].span.offset]);
            s.push_str(else_text);
            s.push_str(&source[tok_end(&tokens[then_close])..eo.span.offset]);
            s.push_str(then_text);
            s.push_str(&source[tok_end(&tokens[else_close])..]);
            Ok((s, at))
        }
        _ => Err("the mutation does not apply to this node".into()),
    }
}

/// Every top-level function's, and every method's, name and body, for
/// comparing two trees.
fn bodies_key(m: &Module) -> Vec<String> {
    use crate::ast::{FunctionDef, Item, ItemKind};
    fn fn_key(fd: &FunctionDef) -> String {
        format!("{}{:?}{:?}", fd.name, fd.body, fd.body_expr)
    }
    fn members_key(items: &[Item]) -> String {
        items
            .iter()
            .map(|m| match &m.kind {
                ItemKind::Function(fd) => fn_key(fd),
                other => format!("{:?}", std::mem::discriminant(other)),
            })
            .collect()
    }
    m.items
        .iter()
        .map(|it| match &it.kind {
            ItemKind::Function(fd) => fn_key(fd),
            ItemKind::Impl(ib) => members_key(&ib.items),
            ItemKind::Extend(eb) => members_key(&eb.items),
            other => format!("{:?}", std::mem::discriminant(other)),
        })
        .collect()
}

/// Whether every error of `after` was already among `before` (as a
/// multiset of code and message): no new errors.
fn no_new_errors(before: &[DiagSummary], after: &[DiagSummary]) -> bool {
    let mut pool: Vec<(&str, &str)> = before.iter().map(|d| (d.code.as_str(), d.message.as_str())).collect();
    for d in after {
        match pool.iter().position(|p| *p == (d.code.as_str(), d.message.as_str())) {
            Some(i) => {
                pool.swap_remove(i);
            }
            None => return false,
        }
    }
    true
}

/// The sites the failing runs executed, scored and planned, best first.
fn plan(module: &Module, intent: &Intent, observed: &[(Run, eval::CoverageData)]) -> Vec<SitePlan> {
    let failing: Vec<&(Run, eval::CoverageData)> = observed.iter().filter(|(r, _)| !r.passed).collect();
    let passing: Vec<&(Run, eval::CoverageData)> = observed.iter().filter(|(r, _)| r.passed).collect();
    let total_failed = failing.len();
    let tests: std::collections::HashSet<&str> = intent.tests.iter().map(String::as_str).collect();
    let mut plans = Vec::new();
    for (item, name, fd) in callables(module) {
        if tests.contains(fd.name.as_str()) {
            continue;
        }
        for (preorder, (id, node)) in eval::function_sites_named(fd, &name).into_iter().enumerate() {
            if id.kind == eval::SiteKind::Index {
                continue;
            }
            let ran = |runs: &[&(Run, eval::CoverageData)]| runs.iter().filter(|(_, c)| c.get(&id).is_some()).count();
            let (failed, passed) = (ran(&failing), ran(&passing));
            if failed == 0 {
                continue;
            }
            let ochiai = failed as f64 / ((total_failed * (failed + passed)) as f64).sqrt();
            let (candidates, op, observed) = plan_site(node, &id, &failing, &passing);
            if candidates.is_empty() {
                continue;
            }
            let motivated = candidates.first().is_some_and(|c| c.2 != NO_SPECIFIC);
            plans.push(SitePlan {
                evidence: SiteEvidence {
                    site: id.render(),
                    function: id.function.clone(),
                    kind: id.kind.as_str().into(),
                    op,
                    ochiai: Some(ochiai),
                    failed,
                    passed,
                    observed,
                    rejected: Vec::new(),
                },
                site: Some(id),
                item,
                preorder,
                candidates,
                motivated,
            });
        }
    }
    // A failed `@ens` of a function that returns a literal: the literal is
    // the value the clause was false of.
    let mut seen = std::collections::HashSet::new();
    for (run, _) in &failing {
        let (Some(check), Some(err)) = (run.failed_check.as_deref(), run.error.as_deref()) else { continue };
        if !check.starts_with("@ens") {
            continue;
        }
        let Some(f) = contract_function(err) else { continue };
        if tests.contains(f) || !seen.insert(f.to_string()) {
            continue;
        }
        let Some((item, fd)) = module.items.iter().enumerate().find_map(|(i, it)| match &it.kind {
            crate::ast::ItemKind::Function(fd) if fd.name == f => Some((i, fd)),
            _ => None,
        }) else {
            continue;
        };
        let Some(lit) = tail_literal(fd) else { continue };
        let up_first = !check.contains('<');
        let why = format!("`{check}` of `{f}` failed, and `{f}` returns the literal `{lit}`");
        let candidates = plus_minus_one(lit, up_first)
            .into_iter()
            .map(|m| (Mutation::TailLiteral(m.clone()), format!("`{lit}` -> `{m}`"), why.clone()))
            .collect();
        plans.push(SitePlan {
            evidence: SiteEvidence {
                site: format!("{f}/return"),
                function: f.to_string(),
                kind: "return".into(),
                op: lit.to_string(),
                ochiai: None,
                failed: failing.iter().filter(|(r, _)| r.error.as_deref().and_then(contract_function) == Some(f)).count(),
                passed: 0,
                observed: vec![format!("{}: {}", run.name, err)],
                rejected: Vec::new(),
            },
            site: None,
            item,
            preorder: usize::MAX,
            candidates,
            motivated: true,
        });
    }
    plans.sort_by(|a, b| {
        let score = |p: &SitePlan| p.evidence.ochiai.unwrap_or(-1.0);
        score(b)
            .partial_cmp(&score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.motivated.cmp(&a.motivated))
            // At an equal score an `if` comes after operator sites: which arm
            // it takes is decided by its condition.
            .then((a.evidence.kind == "if").cmp(&(b.evidence.kind == "if")))
            .then(b.evidence.failed.cmp(&a.evidence.failed))
            .then(a.item.cmp(&b.item))
            .then(a.preorder.cmp(&b.preorder))
    });
    plans
}

/// Run every probe of `intent` on `source` with coverage; when any fails,
/// rank the sites the failing runs executed and try the candidates the
/// observations motivate, best site first. See the module documentation for
/// the acceptance rules.
pub fn debug_and_repair(source: &str, intent: &Intent, fuel: u64) -> DebugReport {
    let probes = probes(intent);
    let Some(module) = parse(source) else {
        return DebugReport::empty(Some("the source does not parse, so nothing was run".into()));
    };
    if probes.is_empty() {
        return DebugReport::empty(Some(
            "no @test functions and no zero-argument functions with contracts: nothing to run".into(),
        ));
    }
    let observed = observe_probes(&module, &probes, fuel);
    let mut report = DebugReport::empty(None);
    report.runs = observed.iter().map(|(r, _)| r.clone()).collect();
    let failing_names: Vec<String> = report.runs.iter().filter(|r| !r.passed).map(|r| r.name.clone()).collect();
    if failing_names.is_empty() {
        return report;
    }
    let mut plans = plan(&module, intent, &observed);
    if plans.is_empty() {
        report.note = Some(
            "the failing runs executed no comparison, arithmetic or `if` in a non-test function, and no \
             failed `@ens` names a function returning a literal: there is no evidence to repair from"
                .into(),
        );
        return report;
    }

    let tokens = lexer::lex(source);
    let bodies = function_bodies(&tokens);
    let baseline = errors_of(source);
    let original_key = bodies_key(&module);
    'sites: for p in plans.iter_mut() {
        let function = p.evidence.function.clone();
        let fd = match function_ref(&module, &function) {
            Ok(fd) => fd,
            Err(why) => {
                p.evidence.rejected.push(("*".into(), why));
                continue;
            }
        };
        let Some(body) = bodies.iter().find(|b| b.name == function) else {
            p.evidence.rejected.push(("*".into(), "the function's body is not a `{ … }` block in the tokens".into()));
            continue;
        };
        for (m, text, why) in p.candidates.clone() {
            if report.mutants_tried == MAX_MUTANTS {
                break 'sites;
            }
            report.mutants_tried += 1;
            let mut reject = |reason: String| p.evidence.rejected.push((text.clone(), reason));
            let Some(mutated) = mutate(&module, &function, p.site.as_ref(), &m) else {
                reject("the site is not in the tree".into());
                continue;
            };
            let (edited, at) = match source_edit(source, &tokens, body, fd, p.site.as_ref(), &m) {
                Ok(x) => x,
                Err(e) => {
                    reject(format!("no proven source edit: {e}"));
                    continue;
                }
            };
            match parse(&edited) {
                Some(re) if bodies_key(&re) == bodies_key(&mutated) && bodies_key(&re) != original_key => {}
                _ => {
                    reject("the source edit does not re-parse to the mutated tree".into());
                    continue;
                }
            }
            let v = intent.violations_in(&intent::infer(&mutated));
            if !v.is_empty() {
                reject(format!("intent not preserved: {}", v.join("; ")));
                continue;
            }
            if let Some((n, e)) =
                failing_names.iter().find_map(|n| passes(&mutated, n, fuel).err().map(|e| (n, e)))
            {
                reject(format!("`{n}` still fails: {e}"));
                continue;
            }
            let after = errors_of(&edited);
            if !no_new_errors(&baseline, &after) {
                reject(format!("adds a checker error ({} -> {})", baseline.len(), after.len()));
                continue;
            }
            let runs_after = run_probes(&mutated, &probes, fuel);
            if let Some(r) = runs_after.iter().find(|r| !r.passed) {
                reject(format!("`{}` now fails: {}", r.name, r.error.as_deref().unwrap_or("")));
                continue;
            }
            let (line, col) = line_col(source, at);
            report.repair = Some(Repair {
                function: function.clone(),
                mutation: text,
                line,
                col,
                site: p.evidence.site.clone(),
                ochiai: p.evidence.ochiai,
                failed: p.evidence.failed,
                passed: p.evidence.passed,
                motivation: why,
                observed: p.evidence.observed.clone(),
                semantics_preserving: false,
                fixed: failing_names.clone(),
                runs_after,
                source: edited,
            });
            break 'sites;
        }
    }
    report.sites = plans.into_iter().map(|p| p.evidence).collect();
    if report.repair.is_none() {
        report.note = Some(format!(
            "no candidate at the {} ranked site(s) made every run pass ({} tried, cap {})",
            report.sites.len(),
            report.mutants_tried,
            MAX_MUTANTS
        ));
    }
    report
}

// ── The whole pass ───────────────────────────────────────────────────

/// Everything `--heal` reports.
#[derive(Debug, Clone, Serialize)]
pub struct HealReport {
    /// `None` when neither the input nor any healed source parses.
    pub intent: Option<Intent>,
    pub compile: HealOutcome,
    pub debug: DebugReport,
    pub healed: String,
    pub changed: bool,
    /// Errors in the healed source.
    pub errors: Vec<DiagSummary>,
    pub tests_total: usize,
    pub tests_passing: usize,
    /// Zero errors, and every probe passes in the healed source.
    pub ok: bool,
}

/// Infer the intent, run the compile-time loop, then debug and repair the
/// result. The input is never modified; the healed source is returned.
pub fn heal_program(source: &str, max_rounds: usize, fuel: u64) -> HealReport {
    let original_intent = intent::infer_source(source);
    let compile = heal_loop(source, original_intent.as_ref(), max_rounds, fuel);
    let intent = original_intent.or_else(|| intent::infer_source(&compile.source));
    let (debug, healed) = match &intent {
        Some(i) => {
            let d = debug_and_repair(&compile.source, i, fuel);
            let healed = d.repair.as_ref().map_or_else(|| compile.source.clone(), |r| r.source.clone());
            (d, healed)
        }
        None => (
            DebugReport::empty(Some("the source does not parse, so nothing was run".into())),
            compile.source.clone(),
        ),
    };
    let errors = errors_of(&healed);
    let final_runs = debug.repair.as_ref().map_or(&debug.runs, |r| &r.runs_after);
    let tests_total = final_runs.len();
    let tests_passing = final_runs.iter().filter(|r| r.passed).count();
    let parses = parse(&healed).is_some();
    HealReport {
        intent,
        ok: parses && errors.is_empty() && tests_passing == tests_total,
        changed: healed != source,
        compile,
        debug,
        healed,
        errors,
        tests_total,
        tests_passing,
    }
}

// ── Diff ─────────────────────────────────────────────────────────────

/// A unified-style line diff of `a` against `b` (three lines of context);
/// empty when they are equal.
pub fn unified_diff(a: &str, b: &str, a_name: &str, b_name: &str) -> String {
    if a == b {
        return String::new();
    }
    let x: Vec<&str> = a.lines().collect();
    let y: Vec<&str> = b.lines().collect();
    let (n, m) = (x.len(), y.len());
    // (tag, line, index in a, index in b) before the op.
    let mut ops: Vec<(char, &str, usize, usize)> = Vec::new();
    if n.saturating_mul(m) > 4_000_000 {
        // Too large for the quadratic table: one hunk replacing everything.
        ops.extend(x.iter().enumerate().map(|(i, l)| ('-', *l, i, 0)));
        ops.extend(y.iter().enumerate().map(|(j, l)| ('+', *l, n, j)));
    } else {
        let mut lcs = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i][j] = if x[i] == y[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && x[i] == y[j] {
                ops.push((' ', x[i], i, j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || lcs[i + 1][j] >= lcs[i][j + 1]) {
                // Deletions before insertions, as `diff -u` prints them.
                ops.push(('-', x[i], i, j));
                i += 1;
            } else {
                ops.push(('+', y[j], i, j));
                j += 1;
            }
        }
    }
    const CTX: usize = 3;
    let changes: Vec<usize> = ops.iter().enumerate().filter(|(_, o)| o.0 != ' ').map(|(k, _)| k).collect();
    let mut out = format!("--- {a_name}\n+++ {b_name}\n");
    let mut c = 0;
    while c < changes.len() {
        let start = changes[c].saturating_sub(CTX);
        let mut last = changes[c];
        while c + 1 < changes.len() && changes[c + 1] <= last + 2 * CTX + 1 {
            c += 1;
            last = changes[c];
        }
        c += 1;
        let end = (last + CTX + 1).min(ops.len());
        let hunk = &ops[start..end];
        let a_len = hunk.iter().filter(|o| o.0 != '+').count();
        let b_len = hunk.iter().filter(|o| o.0 != '-').count();
        let a_start = hunk[0].2 + usize::from(a_len > 0);
        let b_start = hunk[0].3 + usize::from(b_len > 0);
        out.push_str(&format!("@@ -{a_start},{a_len} +{b_start},{b_len} @@\n"));
        for (tag, line, _, _) in hunk {
            out.push(*tag);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUEL: u64 = DEFAULT_FUEL;

    /// A return-type mismatch the checker reports without a span, and a
    /// passing test beside it.
    const TYPE_ERROR: &str = "f count(xs: [i64]) -> i64 {\n    len(xs)\n}\n\n\
                              f twice(n: i64) -> i64 {\n    n * 2\n}\n\n\
                              @t\nf t_twice() {\n    assert(twice(3) == 6, \"twice\")\n}\n";

    #[test]
    fn heal_fixes_a_type_error_and_keeps_a_passing_test() {
        assert_eq!(errors_of(TYPE_ERROR).len(), 1, "{:?}", errors_of(TYPE_ERROR));
        let intent = intent::infer_source(TYPE_ERROR).unwrap();
        let out = heal_loop(TYPE_ERROR, Some(&intent), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.remaining.is_empty(), "{out:#?}");
        assert_eq!(out.steps.len(), 1, "{out:#?}");
        let step = &out.steps[0];
        assert_eq!(step.fix_id, "insert-numeric-cast");
        // The fix comes from the checker's evidence, at the tail expression.
        assert_eq!(step.origin, "compiler evidence");
        let ev = step.evidence.as_ref().expect("evidence");
        assert_eq!(ev.site, types::MismatchSite::Return);
        assert_eq!((ev.function.as_str(), ev.expr.as_str()), ("count", "len(xs)"));
        assert_eq!((ev.expected.as_str(), ev.found.as_str()), ("I64", "Usize"));
        assert_eq!((ev.line, ev.col), (Some(2), Some(5)));
        assert_eq!(step.candidates_considered, 1, "one located candidate, no guesses");
        assert_eq!((step.errors_before, step.errors_after), (1, 0));
        assert_eq!(step.tests_kept_passing, vec!["t_twice".to_string()]);
        assert!(out.source.contains("len(xs) as i64"), "{}", out.source);
        // The healed program keeps its intent and its test still runs.
        assert!(intent.violations_in(&intent::infer_source(&out.source).unwrap()).is_empty());
        assert!(passes(&parse(&out.source).unwrap(), "t_twice", FUEL).is_ok());
    }

    /// `range` takes a `usize`; the checker names the argument, and the cast
    /// goes on the argument and nowhere else.
    #[test]
    fn heal_casts_a_range_argument_the_checker_names() {
        let src = "f count_to(n: i64) -> usize {\n    len(range(n))\n}\n\n\
                   @t\nf t_count() {\n    assert(count_to(3) == 3, \"three\")\n}\n";
        assert_eq!(errors_of(src).len(), 1, "{:?}", errors_of(src));
        assert_eq!(errors_of(src)[0].message, "`range`: expected Usize, found I64");
        let intent = intent::infer_source(src).unwrap();
        let out = heal_loop(src, Some(&intent), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.remaining.is_empty(), "{out:#?}");
        assert_eq!(out.steps.len(), 1, "{out:#?}");
        let ev = out.steps[0].evidence.as_ref().expect("evidence");
        assert_eq!(ev.site, types::MismatchSite::CallArg { callee: "range".into(), index: 0 });
        assert_eq!((ev.expr.as_str(), ev.line, ev.col), ("n", Some(2), Some(15)));
        assert!(out.source.contains("len(range(n as usize))"), "{}", out.source);
        assert!(passes(&parse(&out.source).unwrap(), "t_count", FUEL).is_ok());
    }

    /// Two token ranges re-parse to the mismatched expression: it is reported
    /// as not located, and nothing is guessed.
    #[test]
    fn heal_reports_a_mismatch_it_cannot_locate() {
        let src = "f g(a: i64, b: usize) -> i64 {\n    val x = a + b;\n    val y = a + b;\n    x\n}\n";
        assert!(!errors_of(src).is_empty());
        let out = heal_loop(src, intent::infer_source(src).as_ref(), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.steps.is_empty(), "{out:#?}");
        assert_eq!(out.source, src);
        assert_eq!(out.stopped, "no candidate accepted");
        assert!(!out.unfixed.is_empty(), "{out:#?}");
        for ev in &out.unfixed {
            let note = ev.note.as_deref().unwrap_or("");
            assert!(note.starts_with("could not locate: 2 token ranges"), "{note}");
            assert_eq!(ev.line, None);
        }
    }

    /// The else arm of an `if` is converted to the then arm's type, inside
    /// its block. The then arm is not touched.
    #[test]
    fn heal_casts_an_if_arm_the_checker_names() {
        let src = "f pick(c: bool, xs: [i64], n: i64) -> i64 {\n    if c { n } else { len(xs) }\n}\n\n\
                   @t\nf t_pick() {\n    assert(pick(1b, [5], 7) == 7, \"then\")\n    assert(pick(0b, [5, 6], 7) == 2, \"else\")\n}\n";
        assert_eq!(errors_of(src).len(), 1, "{:?}", errors_of(src));
        let intent = intent::infer_source(src).unwrap();
        let out = heal_loop(src, Some(&intent), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.remaining.is_empty(), "{out:#?}");
        assert_eq!(out.steps.len(), 1, "{out:#?}");
        let ev = out.steps[0].evidence.as_ref().expect("evidence");
        assert_eq!(ev.site, types::MismatchSite::IfArm { branch: 1 });
        assert_eq!((ev.expr.as_str(), ev.line, ev.col), ("len(xs)", Some(2), Some(23)));
        assert!(out.source.contains("if c { n } else { len(xs) as i64 }"), "{}", out.source);
        assert!(passes(&parse(&out.source).unwrap(), "t_pick", FUEL).is_ok());
    }

    /// A `match` arm whose value is a `usize` among `i64` arms: the cast goes
    /// on that arm's value, before its comma.
    #[test]
    fn heal_casts_a_match_arm_the_checker_names() {
        let src = "f route(k: i64, xs: [i64], n: i64) -> i64 {\n    match k { 0 => n, 1 => len(xs), _ => 0 }\n}\n\n\
                   @t\nf t_route() {\n    assert(route(0, [5], 7) == 7, \"zero\")\n    assert(route(1, [5, 6], 7) == 2, \"one\")\n}\n";
        assert_eq!(errors_of(src).len(), 1, "{:?}", errors_of(src));
        let intent = intent::infer_source(src).unwrap();
        let out = heal_loop(src, Some(&intent), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.remaining.is_empty(), "{out:#?}");
        assert_eq!(out.steps.len(), 1, "{out:#?}");
        let ev = out.steps[0].evidence.as_ref().expect("evidence");
        assert_eq!(ev.site, types::MismatchSite::MatchArm { index: 1 });
        assert_eq!(ev.expr.as_str(), "len(xs)");
        assert!(out.source.contains("1 => len(xs) as i64, _ => 0"), "{}", out.source);
        assert!(passes(&parse(&out.source).unwrap(), "t_route", FUEL).is_ok());
    }

    /// An element of a list literal is converted to the first element's type,
    /// at its own position in the literal.
    #[test]
    fn heal_casts_a_list_element_the_checker_names() {
        let src = "f pack(a: i64, xs: [i64]) -> i64 {\n    val row = [a, len(xs)];\n    row[1]\n}\n\n\
                   @t\nf t_pack() {\n    assert(pack(4, [1, 2]) == 2, \"second\")\n}\n";
        assert_eq!(errors_of(src).len(), 1, "{:?}", errors_of(src));
        let intent = intent::infer_source(src).unwrap();
        let out = heal_loop(src, Some(&intent), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.remaining.is_empty(), "{out:#?}");
        assert_eq!(out.steps.len(), 1, "{out:#?}");
        let ev = out.steps[0].evidence.as_ref().expect("evidence");
        assert_eq!(ev.site, types::MismatchSite::ListElement { index: 1 });
        assert_eq!(ev.expr.as_str(), "len(xs)");
        assert!(out.source.contains("[a, len(xs) as i64]"), "{}", out.source);
        assert!(passes(&parse(&out.source).unwrap(), "t_pack", FUEL).is_ok());
    }

    /// The same expression in two arms: the arm cannot be told apart from
    /// its twin, so it is reported as not located and nothing is tried.
    #[test]
    fn heal_reports_an_arm_it_cannot_locate() {
        let src = "f route(k: i64, xs: [i64], n: i64) -> usize {\n    match k { 0 => len(xs), 1 => n, 2 => n }\n}\n";
        let out = heal_loop(src, intent::infer_source(src).as_ref(), DEFAULT_MAX_ROUNDS, FUEL);
        assert!(out.steps.is_empty(), "{out:#?}");
        assert_eq!(out.source, src);
        assert!(!out.unfixed.is_empty(), "{out:#?}");
        for ev in &out.unfixed {
            let note = ev.note.as_deref().unwrap_or("");
            assert!(note.starts_with("could not locate: 2 token ranges ending an arm of the `match`"), "{note}");
        }
    }

    #[test]
    fn heal_rejects_a_candidate_that_changes_a_signature() {
        let intent = intent::infer_source(TYPE_ERROR).unwrap();
        // Changing the return type to `usize` removes the error too.
        let changed = TYPE_ERROR.replacen("-> i64 {\n    len", "-> usize {\n    len", 1);
        assert!(errors_of(&changed).is_empty(), "{:?}", errors_of(&changed));
        let verdict = judge_static(&changed, 1, Some(&intent));
        let reason = verdict.expect_err("a signature change must be rejected");
        assert!(reason.contains("intent not preserved") && reason.contains("signature of `count`"), "{reason}");
        // Without an intent to keep, the same candidate would pass the static rules.
        assert!(judge_static(&changed, 1, None).is_ok());
    }

    #[test]
    fn heal_rejects_a_candidate_that_breaks_a_passing_test() {
        // A candidate that removes the error and keeps every signature, but
        // changes `twice`, passes the static rules and is caught by the
        // dynamic one.
        let broken = TYPE_ERROR.replacen("len(xs)", "len(xs) as i64", 1).replacen("n * 2", "n * 3", 1);
        assert!(judge_static(&broken, 1, intent::infer_source(TYPE_ERROR).as_ref()).is_ok());
        let e = judge_tests(&broken, &["t_twice".to_string()], FUEL).expect_err("t_twice now fails");
        assert!(e.contains("t_twice"), "{e}");
    }

    const OFF_BY_ONE: &str = "f adult(age: i64) -> bool {\n    if age > 18 { 1b } else { 0b }\n}\n\n\
                              @t\nf t_eighteen() {\n    assert(adult(18), \"18 is adult\")\n}\n\n\
                              @t\nf t_ten() {\n    assert(adult(10) == 0b, \"10 is not\")\n}\n";

    #[test]
    fn heal_debugger_repairs_an_off_by_one_with_evidence() {
        assert!(errors_of(OFF_BY_ONE).is_empty(), "{:?}", errors_of(OFF_BY_ONE));
        let intent = intent::infer_source(OFF_BY_ONE).unwrap();
        let report = debug_and_repair(OFF_BY_ONE, &intent, FUEL);
        let failing: Vec<&Run> = report.runs.iter().filter(|r| !r.passed).collect();
        assert_eq!(failing.len(), 1, "{report:#?}");
        assert_eq!(failing[0].name, "t_eighteen");
        assert_eq!(failing[0].failed_check.as_deref(), Some("assert: 18 is adult"), "{report:#?}");
        assert!(failing[0].trace.iter().any(|l| l.contains("-> adult(18)")), "{:?}", failing[0].trace);
        let repair = report.repair.expect("repaired");
        assert_eq!(repair.mutation, "`>` -> `>=`");
        assert_eq!(repair.function, "adult");
        assert!(!repair.semantics_preserving);
        assert_eq!(repair.fixed, vec!["t_eighteen".to_string()]);
        assert!(repair.runs_after.iter().all(|r| r.passed));
        assert!(repair.source.contains("age >= 18"), "{}", repair.source);
    }

    #[test]
    fn heal_reports_an_unrepairable_program_honestly() {
        // `seven` executes no comparison, arithmetic or `if`, and no `@ens`
        // fails: the runs give nothing to repair from, so nothing is tried.
        let src = "f seven() -> i64 {\n    100\n}\n\n@t\nf t_seven() {\n    assert(seven() == 7, \"seven\")\n}\n";
        let report = heal_program(src, DEFAULT_MAX_ROUNDS, FUEL);
        assert!(!report.ok);
        assert!(report.debug.repair.is_none());
        assert_eq!((report.tests_total, report.tests_passing), (1, 0));
        assert!(report.debug.note.as_deref().unwrap_or("").contains("no evidence to repair from"), "{:?}", report.debug.note);
        assert_eq!(report.debug.mutants_tried, 0);
        assert_eq!(report.healed, src, "an unrepaired program is returned unchanged");
    }

    #[test]
    fn heal_program_combines_both_phases() {
        let report = heal_program(OFF_BY_ONE, DEFAULT_MAX_ROUNDS, FUEL);
        assert!(report.ok, "{report:#?}");
        assert!(report.changed);
        let d = unified_diff(OFF_BY_ONE, &report.healed, "a", "b");
        assert!(d.contains("-    if age > 18 { 1b } else { 0b }"), "{d}");
        assert!(d.contains("+    if age >= 18 { 1b } else { 0b }"), "{d}");
    }

    #[test]
    fn heal_contract_failures_name_the_clause() {
        let src = "@ens(result > 0)\nf one() -> i64 {\n    0\n}\n";
        let intent = intent::infer_source(src).unwrap();
        assert_eq!(probes(&intent), vec![("one".to_string(), ProbeKind::Contract)]);
        let report = debug_and_repair(src, &intent, FUEL);
        assert_eq!(report.runs[0].failed_check.as_deref(), Some("@ens(result > 0)"), "{report:#?}");
        let repair = report.repair.expect("0 -> 1 satisfies the contract");
        assert_eq!(repair.mutation, "`0` -> `1`");
        assert_eq!(repair.site, "one/return");
        assert!(repair.motivation.contains("`@ens(result > 0)` of `one` failed"), "{}", repair.motivation);
    }

    /// The branch swap is made on the tree, mapped to the `if`'s tokens, and
    /// the edited source re-parses to the mutated tree.
    #[test]
    fn heal_branch_swap_edit_reproduces_the_mutated_tree() {
        let src = "f g(a: i64) -> i64 {\n    if a > 0 { 1 } else { 2 }\n}\n";
        let module = parse(src).unwrap();
        let tokens = lexer::lex(src);
        let bodies = function_bodies(&tokens);
        let fd = function_ref(&module, "g").unwrap();
        let site = eval::SiteId { function: "g".into(), kind: eval::SiteKind::If, index: 0 };
        let mutated = mutate(&module, "g", Some(&site), &Mutation::Swap).expect("an if");
        let (edited, _) = source_edit(src, &tokens, &bodies[0], fd, Some(&site), &Mutation::Swap).unwrap();
        assert!(edited.contains("if a > 0 { 2 } else { 1 }"), "{edited}");
        assert_eq!(bodies_key(&parse(&edited).unwrap()), bodies_key(&mutated));
        let (negated, _) = source_edit(src, &tokens, &bodies[0], fd, Some(&site), &Mutation::Negate).unwrap();
        let neg_tree = mutate(&module, "g", Some(&site), &Mutation::Negate).unwrap();
        assert!(negated.contains("if !(a > 0) { 1 }"), "{negated}");
        assert_eq!(bodies_key(&parse(&negated).unwrap()), bodies_key(&neg_tree));
    }

    /// The off-by-one with only the failing test calling `adult`: the
    /// comparison is executed by every failing run and no passing one (Ochiai
    /// 1.0), and it saw 18 against 18.
    #[test]
    fn heal_off_by_one_evidence_shows_equal_operands_and_score() {
        let src = "f adult(age: i64) -> bool {\n    if age > 18 { 1b } else { 0b }\n}\n\n\
                   @t\nf t_eighteen() {\n    assert(adult(18), \"18 is adult\")\n}\n";
        let intent = intent::infer_source(src).unwrap();
        let report = debug_and_repair(src, &intent, FUEL);
        let repair = report.repair.as_ref().expect("repaired");
        assert_eq!(repair.mutation, "`>` -> `>=`");
        assert_eq!(repair.site, "adult/binary#0");
        assert_eq!(repair.ochiai, Some(1.0));
        assert_eq!((repair.failed, repair.passed), (1, 0));
        assert!(repair.observed.iter().any(|o| o == "t_eighteen: 18 > 18 -> false"), "{:?}", repair.observed);
        assert!(repair.motivation.contains("equal operands"), "{}", repair.motivation);
        assert_eq!(report.mutants_tried, 1, "the motivated candidate is tried first: {report:#?}");
        assert_eq!((repair.line, repair.col), (2, 12));
    }

    /// A loop bound sees its equal operands last, after the in-range ones; the
    /// report still shows them, since they are what the swap is made from.
    #[test]
    fn heal_shows_the_equal_operands_a_loop_bound_saw_last() {
        let src = "f sum_to(n: i64) -> i64 {
    m i: i64 = 1
    m total: i64 = 0
                       while i < n {
        total = total + i
        i = i + 1
    }
    total
}

                   @t
f t_five() {
    assert(sum_to(30) == 465, \"1..30\")
}
";
        let intent = intent::infer_source(src).unwrap();
        let report = debug_and_repair(src, &intent, FUEL);
        let repair = report.repair.as_ref().expect("repaired");
        assert_eq!(repair.mutation, "`<` -> `<=`");
        assert_eq!(repair.observed.first().map(String::as_str), Some("t_five: 30 < 30 -> false"), "{:?}", repair.observed);
    }

    /// Fault localization: `x - 1` runs only in the failing test (Ochiai 1.0)
    /// and ranks above the comparison both tests run (Ochiai 0.71); `x + 1`,
    /// which no failing run reaches, is not a site at all.
    #[test]
    fn heal_ranks_a_site_only_the_failing_run_executes_first() {
        let src = "f g(x: i64) -> i64 {\n    if x > 5 { x + 1 } else { x - 1 }\n}\n\n\
                   @t\nf t_big() {\n    assert(g(10) == 11, \"big\")\n}\n\n\
                   @t\nf t_small() {\n    assert(g(0) == 1, \"small\")\n}\n";
        let intent = intent::infer_source(src).unwrap();
        let report = debug_and_repair(src, &intent, FUEL);
        let pos = |s: &str| report.sites.iter().position(|e| e.site == s);
        let minus = pos("g/binary#2").expect("`-` site");
        let cmp = pos("g/binary#0").expect("`>` site");
        assert!(minus < cmp, "{:#?}", report.sites);
        assert_eq!(report.sites[minus].ochiai, Some(1.0));
        assert!((report.sites[cmp].ochiai.unwrap() - 0.5f64.sqrt()).abs() < 1e-9);
        assert_eq!(pos("g/binary#1"), None, "`x + 1` ran only in a passing test");
        let repair = report.repair.expect("repaired");
        assert_eq!((repair.site.as_str(), repair.mutation.as_str()), ("g/binary#2", "`-` -> `+`"));
        assert!(repair.source.contains("{ x + 1 } else { x + 1 }"), "{}", repair.source);
    }

    /// A run that fails with a runtime error names the node that raised it.
    #[test]
    fn heal_names_the_node_that_raised_a_runtime_error() {
        let src = "f div(a: i64, b: i64) -> i64 {\n    a / b\n}\n\n\
                   @t\nf t_div() {\n    assert(div(1, 0) == 0, \"div\")\n}\n";
        let intent = intent::infer_source(src).unwrap();
        let report = debug_and_repair(src, &intent, FUEL);
        let run = &report.runs[0];
        assert!(!run.passed);
        let at = run.raised_at.as_deref().expect("a raising node");
        assert!(at.starts_with("div/binary#0: division by zero"), "{at}");
    }

    /// A rejected candidate is listed under its site with the reason.
    #[test]
    fn heal_reports_why_candidates_were_rejected_per_site() {
        let report = debug_and_repair(OFF_BY_ONE, &intent::infer_source(OFF_BY_ONE).unwrap(), FUEL);
        assert!(report.repair.is_some());
        let tried: usize = report.sites.iter().map(|s| s.rejected.len()).sum();
        assert_eq!(tried + 1, report.mutants_tried, "{:#?}", report.sites);
        for s in &report.sites {
            for (m, why) in &s.rejected {
                assert!(!m.is_empty() && !why.is_empty(), "{s:#?}");
            }
        }
    }

    #[test]
    fn heal_unified_diff_is_empty_for_equal_text_and_marks_changes() {
        assert!(unified_diff("a\nb\n", "a\nb\n", "x", "y").is_empty());
        let d = unified_diff("a\nb\nc\n", "a\nB\nc\n", "x", "y");
        assert!(d.starts_with("--- x\n+++ y\n@@ -1,3 +1,3 @@\n"), "{d}");
        assert!(d.contains("-b\n+B\n"), "{d}");
    }

    #[test]
    fn heal_trace_is_off_by_default_and_capped_when_on() {
        let src = "f down(n: i64) -> i64 {\n    if n == 0 { 0 } else { down(n - 1) }\n}\n";
        let m = parse(src).unwrap();
        let (r, trace) = eval::run_bounded_traced(&m, "down", vec![eval::Value::Int(100)], FUEL, 10);
        assert_eq!(r, Ok(eval::Value::Int(0)));
        assert_eq!(trace.len(), 11, "{trace:?}");
        assert!(trace[0].starts_with("-> down(100)"), "{trace:?}");
        assert!(trace[10].contains("not recorded"), "{trace:?}");
        let r = eval::run_bounded(&m, "down", vec![eval::Value::Int(3)], FUEL);
        assert_eq!(r, Ok(eval::Value::Int(0)));
    }

    /// The comparison in a closure is repaired at its own line and column,
    /// with the equal operands the closure saw as the evidence.
    #[test]
    fn heal_debugger_repairs_an_off_by_one_inside_a_closure() {
        let src = "f adult(age: i64) -> bool {\n    val over = |a| a > 18;\n    over(age)\n}\n\n\
                   @t\nf t_eighteen() {\n    assert(adult(18), \"18 is adult\")\n}\n";
        assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
        let report = debug_and_repair(src, &intent::infer_source(src).unwrap(), FUEL);
        let repair = report.repair.as_ref().expect("repaired");
        assert_eq!(repair.function, "adult");
        assert_eq!(repair.site, "adult/binary#0");
        assert_eq!(repair.mutation, "`>` -> `>=`");
        assert_eq!((repair.line, repair.col), (2, 22));
        assert_eq!((repair.failed, repair.passed), (1, 0));
        assert!(repair.observed.iter().any(|o| o == "t_eighteen: 18 > 18 -> false"), "{:?}", repair.observed);
        assert!(repair.source.contains("val over = |a| a >= 18;"), "{}", repair.source);
    }

    /// The same, for a named nested function's comparison.
    #[test]
    fn heal_debugger_repairs_an_off_by_one_inside_a_nested_function() {
        let src = "f adult(age: i64) -> bool {\n    f over(a: i64) -> bool { a > 18 }\n    over(age)\n}\n\n\
                   @t\nf t_eighteen() {\n    assert(adult(18), \"18 is adult\")\n}\n";
        assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
        let report = debug_and_repair(src, &intent::infer_source(src).unwrap(), FUEL);
        let repair = report.repair.as_ref().expect("repaired");
        assert_eq!(repair.function, "adult");
        assert_eq!(repair.site, "adult/binary#0");
        assert_eq!(repair.mutation, "`>` -> `>=`");
        assert_eq!((repair.line, repair.col), (2, 32));
        assert!(repair.observed.iter().any(|o| o == "t_eighteen: 18 > 18 -> false"), "{:?}", repair.observed);
        assert!(repair.source.contains("{ a >= 18 }"), "{}", repair.source);
    }

    /// The same, for a method: it is named `P::adult`, found in its `I` block
    /// for the mutation, and its token range is that method's body.
    #[test]
    fn heal_debugger_repairs_an_off_by_one_inside_a_method() {
        let src = "S P { age: i64 }\nI P { f adult(&self) -> bool { self.age > 18 } }\n\n\
                   @t\nf t_eighteen() {\n    v p = @P { age: 18 };\n    assert(p.adult(), \"18 is adult\")\n}\n";
        assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
        let report = debug_and_repair(src, &intent::infer_source(src).unwrap(), FUEL);
        let repair = report.repair.as_ref().expect("repaired");
        assert_eq!(repair.function, "P::adult");
        assert_eq!(repair.site, "P::adult/binary#0");
        assert_eq!(repair.mutation, "`>` -> `>=`");
        assert_eq!((repair.line, repair.col), (2, 41));
        assert_eq!((repair.failed, repair.passed), (1, 0));
        assert!(repair.observed.iter().any(|o| o == "t_eighteen: 18 > 18 -> false"), "{:?}", repair.observed);
        assert!(repair.source.contains("self.age >= 18"), "{}", repair.source);
    }

    /// A method name given twice is refused, with the reason, for each site
    /// of it: the interpreter keeps one of the two, so the other is not known
    /// to be the one that ran. No panic, and no repair.
    #[test]
    fn heal_records_why_a_method_it_cannot_resolve_was_not_repaired() {
        let src = "S P { age: i64 }\nI P { f adult(&self) -> bool { self.age > 18 } }\n\
                   I P { f adult(&self) -> bool { self.age > 19 } }\n\n\
                   @t\nf t_eighteen() {\n    v p = @P { age: 18 };\n    assert(p.adult(), \"18 is adult\")\n}\n";
        let report = debug_and_repair(src, &intent::infer_source(src).unwrap(), FUEL);
        assert!(report.repair.is_none(), "{report:#?}");
        let refused: Vec<&(String, String)> =
            report.sites.iter().flat_map(|s| s.rejected.iter()).filter(|(m, _)| m == "*").collect();
        assert!(!refused.is_empty(), "{report:#?}");
        assert!(refused.iter().all(|(_, why)| why.contains("defined more than once")), "{refused:?}");
    }
}
