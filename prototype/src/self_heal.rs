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
//! Type-checker diagnostics carry no source span today, so a heal pattern's
//! insertion lands at line 1, column 1. For a span-less diagnostic with a
//! single-insertion fix the loop also tries the insertion after each token of
//! the function the message names (every function body when it names none).
//! These are guesses; the three acceptance rules are what make one stick, and
//! the step log marks the fix as relocated.
//!
//! **Run time** ([`debug_and_repair`]). Every `@test`, and every zero-argument
//! function with contracts, runs under the fuel-bounded evaluator. A failure is
//! reported with the error, the contract clause or assertion that failed, and
//! a capped call trace. Then single-site mutations of the non-test functions
//! the failing runs can reach are tried in a fixed order — comparison swaps,
//! `+`/`-` swaps, integer literal ±1, swapped `if`/`else` branches — and the
//! first mutant is accepted that checks with no new errors, keeps the intent,
//! and makes **every** run pass. Such a repair changes what the program
//! computes by construction, and is always reported as semantics-changing,
//! with the runs it turned from failing to passing as its evidence.
//!
//! In keeping with `verdict.rs`, nothing here carries a score: an outcome is
//! the list of what was checked and what it found.

use serde::Serialize;

use crate::ast::Module;
use crate::eval::{self, BoundedError};
use crate::heal;
use crate::hir;
use crate::intent::{self, Intent};
use crate::lexer::{self, Token, TokenKind};
use crate::parser;
use crate::recover;

/// Fuel for each run of a test or contract probe, unless the caller says.
pub const DEFAULT_FUEL: u64 = 200_000;
/// Compile-time rounds, unless the caller says.
pub const DEFAULT_MAX_ROUNDS: usize = 8;
/// Most mutants the run-time repair tries.
pub const MAX_MUTANTS: usize = 500;
/// Most candidates one compile-time round evaluates.
pub const MAX_CANDIDATES_PER_ROUND: usize = 200;
/// Most relocated positions tried for one span-less insertion.
const MAX_RELOCATIONS: usize = 64;
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

/// Every top-level function with a body, in source order.
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
                let name = tokens[i + 1].text.clone();
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
                if let Some(open) = open
                    && let Some(close) = matching_close(tokens, open)
                {
                    out.push(FnBody { name, open, close });
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
    /// `heal pattern`, `heal pattern (relocated to L:C)`, or `recover:<stage>`.
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

/// The function a span-less message names: "function `count`: …".
fn named_function(message: &str) -> Option<&str> {
    let rest = message.split("function `").nth(1)?;
    rest.split('`').next()
}

/// The candidates for one round, in generation order, deduplicated by the
/// source they produce, at most [`MAX_CANDIDATES_PER_ROUND`].
pub fn candidates(source: &str, errors: &[DiagSummary]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |c: Candidate, out: &mut Vec<Candidate>| {
        if c.source != source && out.len() < MAX_CANDIDATES_PER_ROUND && seen.insert(c.source.clone()) {
            out.push(c);
        }
    };
    let tokens = lexer::lex(source);
    let bodies = function_bodies(&tokens);
    for d in errors {
        let healed = heal::heal_one(&to_hir(d));
        for fix in &healed.fixes {
            if fix.edits.is_empty() {
                continue;
            }
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
                    },
                    &mut out,
                );
            }
            // A span-less diagnostic's insertion was placed at 1:1 for want
            // of a place. Try it after each token of the named function.
            let single_insert = fix.edits.len() == 1
                && fix.edits[0].start_line == fix.edits[0].end_line
                && fix.edits[0].start_col == fix.edits[0].end_col;
            if d.line == 0 && single_insert {
                let text = &fix.edits[0].new_text;
                let named = named_function(&d.message);
                let mut tried = 0;
                for b in bodies.iter().filter(|b| named.is_none_or(|n| n == b.name)) {
                    // After each token inside the body, not after its braces.
                    for t in &tokens[b.open + 1..b.close] {
                        if tried == MAX_RELOCATIONS {
                            break;
                        }
                        tried += 1;
                        let at = tok_end(t);
                        let (line, col) = line_col(source, at);
                        push(
                            Candidate {
                                fix_id: fix.id.clone(),
                                origin: format!("heal pattern (relocated to {line}:{col})"),
                                description: fix.description.clone(),
                                diagnostic: Some(d.clone()),
                                semantics_preserving: fix.semantics_preserving,
                                prior: fix.confidence,
                                source: splice(source, at, at, text),
                            },
                            &mut out,
                        );
                    }
                }
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
                },
                &mut out,
            );
        }
    }
    out
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
        let cands = candidates(&current, &errors);
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
        });
        current = c.source.clone();
        errors = after;
    }
    HealOutcome { source: current, steps, remaining: errors, rounds, stopped, rejections, intent_note }
}

// ── Run-time debugging and repair ────────────────────────────────────

/// An accepted run-time repair and its evidence.
#[derive(Debug, Clone, Serialize)]
pub struct Repair {
    pub function: String,
    /// What changed, e.g. "`<` -> `<=`".
    pub mutation: String,
    pub line: usize,
    pub col: usize,
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
    /// Functions the repair search mutated (those the failing runs reach).
    pub searched: Vec<String>,
    pub note: Option<String>,
}

impl DebugReport {
    pub fn failing(&self) -> usize {
        self.runs.iter().filter(|r| !r.passed).count()
    }
}

/// One single-site mutation of the source.
#[derive(Debug, Clone)]
struct Mutant {
    function: String,
    mutation: String,
    at: usize,
    source: String,
}

/// The top-level functions each function's body names in call position
/// (`name(`), a cheap over-approximation of the call graph.
fn callees(tokens: &[Token], body: &FnBody, known: &std::collections::HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for w in tokens[body.open..body.close].windows(2) {
        if w[0].kind == TokenKind::Ident && w[1].kind == TokenKind::LParen && known.contains(w[0].text.as_str()) {
            out.push(w[0].text.clone());
        }
    }
    out
}

/// Ends a value, so a following `+`/`-` is binary.
fn ends_value(k: TokenKind) -> bool {
    matches!(
        k,
        TokenKind::Ident | TokenKind::IntLiteral | TokenKind::FloatLiteral | TokenKind::RParen | TokenKind::RBrack
    )
}

/// Every mutant of the bodies of `functions`, kind-major in the order
/// comparison, arithmetic, literal, branch swap; within a kind, by function
/// in source order, then by position. At most `cap`.
fn mutants(source: &str, tokens: &[Token], bodies: &[&FnBody], cap: usize) -> Vec<Mutant> {
    let mut out = Vec::new();
    let add = |out: &mut Vec<Mutant>, function: &str, mutation: String, at: usize, src: String| {
        if out.len() < cap {
            out.push(Mutant { function: function.to_string(), mutation, at, source: src });
        }
    };
    // Comparison swaps.
    for b in bodies {
        for t in &tokens[b.open + 1..b.close] {
            let to = match t.kind {
                TokenKind::Lt => "<=",
                TokenKind::Le => "<",
                TokenKind::Gt => ">=",
                TokenKind::Ge => ">",
                TokenKind::Eq => "!=",
                TokenKind::Neq => "==",
                _ => continue,
            };
            let s = splice(source, t.span.offset, tok_end(t), to);
            add(&mut out, &b.name, format!("`{}` -> `{to}`", t.text), t.span.offset, s);
        }
    }
    // Arithmetic `+` <-> `-`, binary only.
    for b in bodies {
        for k in b.open + 1..b.close {
            let t = &tokens[k];
            let to = match t.kind {
                TokenKind::Plus => "-",
                TokenKind::Minus => "+",
                _ => continue,
            };
            if !ends_value(tokens[k - 1].kind) {
                continue;
            }
            let s = splice(source, t.span.offset, tok_end(t), to);
            add(&mut out, &b.name, format!("`{}` -> `{to}`", t.text), t.span.offset, s);
        }
    }
    // Integer literal ±1, decimal literals without suffix only.
    for b in bodies {
        for t in &tokens[b.open + 1..b.close] {
            if t.kind != TokenKind::IntLiteral || !t.text.bytes().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(n) = t.text.parse::<i64>() else { continue };
            for m in [n.checked_add(1), n.checked_sub(1).filter(|m| *m >= 0)].into_iter().flatten() {
                let s = splice(source, t.span.offset, tok_end(t), &m.to_string());
                add(&mut out, &b.name, format!("`{n}` -> `{m}`"), t.span.offset, s);
            }
        }
    }
    // Swap the branches of `if c { A } else { B }` (not `else if`, not `? :`).
    for b in bodies {
        for k in b.open + 1..b.close {
            let t = &tokens[k];
            if t.kind != TokenKind::Question || t.text != "if" {
                continue;
            }
            let mut nest = 0i32;
            let mut then_open = None;
            for (j, u) in tokens.iter().enumerate().take(b.close).skip(k + 1) {
                match u.kind {
                    TokenKind::LParen | TokenKind::LBrack => nest += 1,
                    TokenKind::RParen | TokenKind::RBrack => nest -= 1,
                    TokenKind::LBrace if nest == 0 => {
                        // `@P { … }` in the condition is a struct literal.
                        if j >= 2 && tokens[j - 2].kind == TokenKind::At {
                            break;
                        }
                        then_open = Some(j);
                        break;
                    }
                    _ => {}
                }
            }
            let Some(then_open) = then_open else { continue };
            let Some(then_close) = matching_close(tokens, then_open) else { continue };
            let (Some(e), Some(eo)) = (tokens.get(then_close + 1), tokens.get(then_close + 2)) else { continue };
            if e.kind != TokenKind::KwElse || eo.kind != TokenKind::LBrace {
                continue;
            }
            let Some(else_close) = matching_close(tokens, then_close + 2) else { continue };
            let then_text = &source[tokens[then_open].span.offset..tok_end(&tokens[then_close])];
            let else_text = &source[eo.span.offset..tok_end(&tokens[else_close])];
            let mut s = String::with_capacity(source.len());
            s.push_str(&source[..tokens[then_open].span.offset]);
            s.push_str(else_text);
            s.push_str(&source[tok_end(&tokens[then_close])..eo.span.offset]);
            s.push_str(then_text);
            s.push_str(&source[tok_end(&tokens[else_close])..]);
            add(&mut out, &b.name, "swap `if`/`else` branches".to_string(), t.span.offset, s);
        }
    }
    out
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

/// Run every probe of `intent` on `source`; when any fails, report it and
/// search single-site mutations for one that makes every probe pass. See the
/// module documentation for the acceptance rules.
pub fn debug_and_repair(source: &str, intent: &Intent, fuel: u64) -> DebugReport {
    let probes = probes(intent);
    let mut report =
        DebugReport { runs: Vec::new(), repair: None, mutants_tried: 0, mutant_cap: MAX_MUTANTS, searched: Vec::new(), note: None };
    let Some(module) = parse(source) else {
        report.note = Some("the source does not parse, so nothing was run".into());
        return report;
    };
    if probes.is_empty() {
        report.note = Some("no @test functions and no zero-argument functions with contracts: nothing to run".into());
        return report;
    }
    report.runs = run_probes(&module, &probes, fuel);
    let failing: Vec<&Run> = report.runs.iter().filter(|r| !r.passed).collect();
    if failing.is_empty() {
        return report;
    }

    // The non-test functions the failing runs can reach.
    let tokens = lexer::lex(source);
    let bodies = function_bodies(&tokens);
    let known: std::collections::HashSet<&str> = bodies.iter().map(|b| b.name.as_str()).collect();
    let tests: std::collections::HashSet<&str> = intent.tests.iter().map(String::as_str).collect();
    let mut reached: Vec<String> = Vec::new();
    let mut stack: Vec<String> = failing.iter().map(|r| r.name.clone()).collect();
    while let Some(f) = stack.pop() {
        if reached.contains(&f) {
            continue;
        }
        reached.push(f.clone());
        if let Some(b) = bodies.iter().find(|b| b.name == f) {
            stack.extend(callees(&tokens, b, &known));
        }
    }
    let targets: Vec<&FnBody> =
        bodies.iter().filter(|b| reached.contains(&b.name) && !tests.contains(b.name.as_str())).collect();
    report.searched = targets.iter().map(|b| b.name.clone()).collect();
    if targets.is_empty() {
        report.note = Some("the failing runs reach no non-test function to repair".into());
        return report;
    }

    let baseline = errors_of(source);
    let failing_names: Vec<String> = failing.iter().map(|r| r.name.clone()).collect();
    for m in mutants(source, &tokens, &targets, MAX_MUTANTS) {
        report.mutants_tried += 1;
        let Some(mmod) = parse(&m.source) else { continue };
        if !intent.violations_in(&intent::infer(&mmod)).is_empty() {
            continue;
        }
        // Cheapest decisive check first: the runs that failed.
        if failing_names.iter().any(|n| passes(&mmod, n, fuel).is_err()) {
            continue;
        }
        if !no_new_errors(&baseline, &errors_of(&m.source)) {
            continue;
        }
        let runs_after = run_probes(&mmod, &probes, fuel);
        if runs_after.iter().all(|r| r.passed) {
            let (line, col) = line_col(source, m.at);
            report.repair = Some(Repair {
                function: m.function,
                mutation: m.mutation,
                line,
                col,
                semantics_preserving: false,
                fixed: failing_names,
                runs_after,
                source: m.source,
            });
            return report;
        }
    }
    report.note = Some(format!(
        "no single-site mutation of {} made every run pass ({} tried, cap {})",
        report.searched.join(", "),
        report.mutants_tried,
        MAX_MUTANTS
    ));
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
            DebugReport {
                runs: Vec::new(),
                repair: None,
                mutants_tried: 0,
                mutant_cap: MAX_MUTANTS,
                searched: Vec::new(),
                note: Some("the source does not parse, so nothing was run".into()),
            },
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
        assert_eq!((step.errors_before, step.errors_after), (1, 0));
        assert_eq!(step.tests_kept_passing, vec!["t_twice".to_string()]);
        assert!(out.source.contains("len(xs) as i64"), "{}", out.source);
        // The healed program keeps its intent and its test still runs.
        assert!(intent.violations_in(&intent::infer_source(&out.source).unwrap()).is_empty());
        assert!(passes(&parse(&out.source).unwrap(), "t_twice", FUEL).is_ok());
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
        // No single-site mutation makes `seven` return 7 from 100.
        let src = "f seven() -> i64 {\n    100\n}\n\n@t\nf t_seven() {\n    assert(seven() == 7, \"seven\")\n}\n";
        let report = heal_program(src, DEFAULT_MAX_ROUNDS, FUEL);
        assert!(!report.ok);
        assert!(report.debug.repair.is_none());
        assert_eq!((report.tests_total, report.tests_passing), (1, 0));
        assert!(report.debug.note.as_deref().unwrap_or("").contains("no single-site mutation"), "{:?}", report.debug.note);
        assert!(report.debug.mutants_tried > 0);
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
    }

    #[test]
    fn heal_branch_swap_mutant_is_well_formed() {
        let src = "f g(a: i64) -> i64 {\n    if a > 0 { 1 } else { 2 }\n}\n";
        let tokens = lexer::lex(src);
        let bodies = function_bodies(&tokens);
        let refs: Vec<&FnBody> = bodies.iter().collect();
        let swap = mutants(src, &tokens, &refs, MAX_MUTANTS)
            .into_iter()
            .find(|m| m.mutation.contains("swap"))
            .expect("a branch swap");
        assert!(swap.source.contains("if a > 0 { 2 } else { 1 }"), "{}", swap.source);
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
}
