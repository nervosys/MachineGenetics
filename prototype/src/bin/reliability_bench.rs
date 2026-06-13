//! # `reliability-bench` — agent-write reliability harness
//!
//! Walks every task in `benchmarks/tasks/*.json`, asks a pluggable
//! **candidate agent** for MAGE source, runs the candidate through
//! the prototype compiler, and records:
//!
//! 1. **Lex success** — no `TokenKind::Error` tokens emitted.
//! 2. **Parse success** — `parser::parse` returns `Ok(Module)`.
//! 3. **Heal applied** — the self-healing pass (Phase B / step 34)
//!    proposed at least one fix (informational, not a failure mode).
//! 4. **Final pass** — after elision + parse, no diagnostics remain.
//!
//! Output: `benchmarks/RELIABILITY_REPORT.md` + a stdout summary +
//! non-zero exit code if any task fails to lex/parse cleanly.
//!
//! ## Why this exists
//!
//! Phase 27 measured **token efficiency** ([`benchmarks/FINDINGS.md`]).
//! Phase 30 measures the other half of the mission — whether agent-
//! emitted MAGE is **reliable**. The mechanics here are language-
//! and-compiler-only; an LLM is *not* required to demonstrate the
//! pipeline. The default backend is a **file oracle** that simply
//! reads `solution.rdx_source` from the corpus JSON — i.e. simulates
//! a perfectly capable agent. The numbers it produces are the
//! **upper bound** on what any real agent can achieve, since they
//! tell us whether the corpus's own reference solutions even pass
//! the compiler.
//!
//! ## Plugging in a real LLM
//!
//! Replace [`FileOracleAgent`] with anything implementing
//! [`CandidateAgent`]. The interface is one async-free method:
//!
//! ```ignore
//! fn propose(&mut self, task: &Task) -> Result<String, String>;
//! ```
//!
//! Future backends: subprocess invoking `claude-code-cli`, HTTP-call
//! to the Anthropic API, locally-loaded ONNX/llama model. The harness
//! infrastructure (corpus walk, compiler pipeline invocation, report
//! generation) is the part that costs effort; the agent backend is a
//! small adapter.

use std::collections::BTreeMap;
use std::fs;
use std::process::ExitCode;
use std::time::Instant;

// Include the prototype's lexer/ast/parser/heal at the bin-crate root so
// the `crate::ast::*` / `crate::lexer::*` / `crate::hir::*` paths inside
// each module resolve.
#[path = "../lexer.rs"]
mod lexer;
#[path = "../ast.rs"]
mod ast;
#[path = "../parser.rs"]
mod parser;
#[path = "../hir.rs"]
mod hir;
#[path = "../heal.rs"]
mod heal;
#[path = "../recover.rs"]
mod recover;

use lexer as mg_lexer;
use parser as mg_parser;
use recover::{apply_text_edits, structural_completion, structural_heal, trim_bad_token};

// ─── Corpus types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Task {
    id: String,
    category: String,
    description: String,
    reference_source: String,
}

// ─── Candidate agent interface ───────────────────────────────────────

/// A candidate agent: given a task, produces a MAGE source string.
///
/// `refine` is the Stage-3 re-prompt hook: when the mechanical 3-stage
/// recovery in `recover::recover` cannot save broken source, the bench
/// calls `refine(task, broken_source, parse_error)` and re-runs the
/// pipeline on the returned source. The default impl is a no-op (returns
/// the broken source unchanged), so a non-zero `refine_succeeded` count
/// is a real signal that the agent contributed beyond its initial
/// proposal.
trait CandidateAgent {
    fn name(&self) -> &str;
    fn propose(&mut self, task: &Task) -> Result<String, String>;
    fn refine(
        &mut self,
        _task: &Task,
        broken_source: &str,
        _parse_error: &str,
    ) -> Result<String, String> {
        Ok(broken_source.to_string())
    }
}

/// Trivial agent that echoes the corpus's reference solution. Useful
/// as an upper bound — it tells us whether the corpus itself passes
/// today's lexer/parser, separate from any LLM error rate.
struct FileOracleAgent;
impl CandidateAgent for FileOracleAgent {
    fn name(&self) -> &str { "file-oracle" }
    fn propose(&mut self, task: &Task) -> Result<String, String> {
        Ok(task.reference_source.clone())
    }
}

/// Returns the corpus solution with **small deterministic mutations**
/// applied — simulates the typical near-correct output a real LLM
/// produces. Mutations are seeded by the task id so re-runs are
/// reproducible; each task gets one shape of perturbation drawn from
/// a fixed list so we exercise different recovery paths.
///
/// Mutation menu (one applied per task):
/// 1. drop the **last** `;` (tests `parse-missing-semi`)
/// 2. drop the **last** `}` (tests `parse-missing-rbrace`)
/// 3. drop the **last** `)` (tests `parse-missing-rparen`)
/// 4. swap `let` ↔ `mut` (no-op for sigil-mode MG; robustness)
/// 5. insert a stray `,` after the first `{` (tests `parse-stray-comma`)
/// 6. **truncate at ~75 % length** — simulates LLM token-budget cutoff
///    mid-output, the most common real-world LLM failure shape
/// 7. **insert duplicate `;;`** — simulates LLM trailing-semicolon drift
/// 8. **swap two adjacent words** — simulates LLM token-order slips
///
/// The point is **not** to measure LLM quality directly — it's to
/// measure how much of the heal pipeline's reach turns into
/// effective-pass on near-correct input.
struct PerturbedOracleAgent {
    seed: u64,
}
impl PerturbedOracleAgent {
    fn new(seed: u64) -> Self { Self { seed } }
}
impl CandidateAgent for PerturbedOracleAgent {
    fn name(&self) -> &str { "perturbed-oracle" }
    fn propose(&mut self, task: &Task) -> Result<String, String> {
        // Per-task deterministic pick over the 8 mutations.
        let mut h: u64 = self.seed;
        for b in task.id.as_bytes() {
            h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(*b as u64);
        }
        let pick = (h % 8) as u8;
        let src = &task.reference_source;
        let mutated = match pick {
            0 => drop_last_byte(src, b';'),
            1 => drop_last_byte(src, b'}'),
            2 => drop_last_byte(src, b')'),
            3 => src.replace(" let ", " mut ").replace(" mut ", " let "),
            4 => insert_after_first(src, b'{', ','),
            5 => truncate_at_fraction(src, 0.75),
            6 => duplicate_first_byte(src, b';'),
            _ => swap_two_words(src, h),
        };
        Ok(mutated)
    }
}

/// Truncate `src` to roughly `frac * len` bytes, preferring a
/// whitespace boundary so the cutoff doesn't bisect an identifier
/// (which would lex-fail rather than parse-fail).
fn truncate_at_fraction(src: &str, frac: f32) -> String {
    let target = ((src.len() as f32) * frac) as usize;
    if target >= src.len() {
        return src.to_string();
    }
    // Find the nearest whitespace at or before `target`.
    let cut = src[..target]
        .rfind(|c: char| c.is_whitespace())
        .unwrap_or(target);
    src[..cut].to_string()
}

/// Insert a duplicate of the first occurrence of `b` right after it.
/// E.g. for `b';'` turns `a;b` into `a;;b`. Tests heal-tolerance of
/// LLM trailing-punctuation drift.
fn duplicate_first_byte(src: &str, b: u8) -> String {
    if let Some(i) = src.find(b as char) {
        let mut s = String::with_capacity(src.len() + 1);
        s.push_str(&src[..=i]);
        s.push(b as char);
        s.push_str(&src[i + 1..]);
        s
    } else {
        src.to_string()
    }
}

/// Swap two adjacent whitespace-separated tokens at a position derived
/// from `seed`. Simulates LLM token-order slips (e.g. `pub fn` →
/// `fn pub`). Stable: same source + same seed → same mutation.
fn swap_two_words(src: &str, seed: u64) -> String {
    let words: Vec<(usize, usize)> = src
        .char_indices()
        .filter(|(i, _)| {
            *i == 0
                || src
                    .as_bytes()
                    .get(*i - 1)
                    .map(|b| b.is_ascii_whitespace())
                    .unwrap_or(false)
        })
        .filter_map(|(start, _)| {
            let end = src[start..]
                .find(|c: char| c.is_whitespace())
                .map(|n| start + n)
                .unwrap_or(src.len());
            if end > start && !src[start..end].is_empty() {
                Some((start, end))
            } else {
                None
            }
        })
        .collect();
    if words.len() < 2 {
        return src.to_string();
    }
    // Pick a deterministic adjacent-word pair near the middle of the
    // source so the mutation reliably perturbs body code, not the
    // signature.
    let idx = (seed as usize % (words.len() - 1)).max(words.len() / 4);
    let idx = idx.min(words.len() - 2);
    let (a_start, a_end) = words[idx];
    let (b_start, b_end) = words[idx + 1];
    if b_start <= a_end {
        return src.to_string();
    }
    let a_word = &src[a_start..a_end];
    let b_word = &src[b_start..b_end];
    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..a_start]);
    out.push_str(b_word);
    out.push_str(&src[a_end..b_start]);
    out.push_str(a_word);
    out.push_str(&src[b_end..]);
    out
}

fn drop_last_byte(src: &str, b: u8) -> String {
    if let Some(i) = src.rfind(b as char) {
        let mut s = src.to_string();
        s.replace_range(i..i + 1, "");
        s
    } else {
        src.to_string()
    }
}

fn insert_after_first(src: &str, anchor: u8, insert: char) -> String {
    if let Some(i) = src.find(anchor as char) {
        let mut s = String::with_capacity(src.len() + 1);
        s.push_str(&src[..=i]);
        s.push(insert);
        s.push_str(&src[i + 1..]);
        s
    } else {
        src.to_string()
    }
}

/// Agent that spawns an external command per task. The command:
/// - receives the task description on **stdin**
/// - must print the MAGE source on **stdout**
/// - non-zero exit code → recorded as `agent refused`
///
/// This is the integration point for any real LLM. Wire a thin
/// adapter script (Claude Code CLI, `curl` to the Anthropic API,
/// a local llama server) and the bench measures it end-to-end.
struct SubprocessAgent {
    cmd: String,
}
impl SubprocessAgent {
    fn new(cmd: String) -> Self { Self { cmd } }
}
impl SubprocessAgent {
    /// Spawn the wrapper command with the given env vars and stdin
    /// payload. Shared by `propose` and `refine`.
    fn run(&self, env: &[(&str, &str)], stdin_bytes: &[u8]) -> Result<String, String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let mut parts = self.cmd.split_whitespace();
        let prog = parts.next().ok_or_else(|| "subprocess: empty command".to_string())?;
        let args: Vec<&str> = parts.collect();

        let mut cmd = Command::new(prog);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| format!("spawn {prog}: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(stdin_bytes);
        }

        let out = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "exit {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}
/// Hybrid backend: propose via [`PerturbedOracleAgent`] (deterministic
/// corpus mutations), refine via subprocess. Lets us measure the
/// Stage-3 refine wrapper's contribution on top of the perturbed
/// baseline without rewriting both code paths.
struct PerturbedWithRefine {
    inner: PerturbedOracleAgent,
    sub: SubprocessAgent,
}
impl CandidateAgent for PerturbedWithRefine {
    fn name(&self) -> &str { "perturbed+refine" }
    fn propose(&mut self, task: &Task) -> Result<String, String> {
        self.inner.propose(task)
    }
    fn refine(
        &mut self,
        task: &Task,
        broken_source: &str,
        parse_error: &str,
    ) -> Result<String, String> {
        self.sub.refine(task, broken_source, parse_error)
    }
}

impl CandidateAgent for SubprocessAgent {
    fn name(&self) -> &str { "subprocess" }
    fn propose(&mut self, task: &Task) -> Result<String, String> {
        self.run(
            &[("RDX_BENCH_MODE", "propose"), ("RDX_TASK_ID", &task.id)],
            task.description.as_bytes(),
        )
    }
    fn refine(
        &mut self,
        task: &Task,
        broken_source: &str,
        parse_error: &str,
    ) -> Result<String, String> {
        self.run(
            &[
                ("RDX_BENCH_MODE", "refine"),
                ("RDX_TASK_ID", &task.id),
                ("RDX_PARSE_ERROR", parse_error),
                ("RDX_TASK_DESCRIPTION", &task.description),
            ],
            broken_source.as_bytes(),
        )
    }
}

// ─── Per-task result ─────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
struct TaskResult {
    id: String,
    category: String,
    lex_ok: bool,
    parse_ok: bool,
    lex_errors: usize,
    parse_error_msg: Option<String>,
    token_count: u32,
    time_us: u128,
    /// True if self-heal proposed at least one fix for the parse failure.
    heal_proposed: bool,
    /// True if applying the top-ranked fix made the program re-parse.
    heal_succeeded: bool,
    /// True if the structural-heal fallback (brace-balance at EOF)
    /// recovered the parse after pattern-based heal failed.
    structural_heal_succeeded: bool,
    /// True if Stage-3 (agent.refine re-prompt) returned source that
    /// parsed after all mechanical recovery failed. Non-zero only when
    /// the agent backend actually contributes — the trait default is
    /// no-op so file/perturbed oracles never trigger this.
    refine_succeeded: bool,
}

// ─── Main ────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let bench_dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| find_benchmarks_dir());
    let out_path = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| format!("{bench_dir}/RELIABILITY_REPORT.md"));

    let tasks = match load_all_tasks(&bench_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("reliability-bench: load: {e}");
            return ExitCode::from(2);
        }
    };
    if tasks.is_empty() {
        eprintln!("reliability-bench: no tasks found in {bench_dir}/tasks/");
        return ExitCode::from(2);
    }

    // Backend selection: --agent <name> picks one of
    //   file-oracle (default)          — echo corpus reference solution
    //   perturbed                       — corpus + small deterministic mutations
    //   subprocess:<cmd>                — spawn external command per task
    let backend_arg = args
        .iter()
        .position(|a| a == "--agent")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "file-oracle".to_string());
    let mut agent: Box<dyn CandidateAgent> = if backend_arg == "file-oracle" {
        Box::new(FileOracleAgent)
    } else if backend_arg == "perturbed" {
        Box::new(PerturbedOracleAgent::new(0xC0FFEE_5EED))
    } else if let Some(cmd) = backend_arg.strip_prefix("subprocess:") {
        Box::new(SubprocessAgent::new(cmd.to_string()))
    } else if let Some(cmd) = backend_arg.strip_prefix("perturbed+refine:") {
        // Hybrid: PerturbedOracleAgent for propose, subprocess for
        // refine. Measures Stage-3 contribution on top of perturbed
        // baseline. The subprocess only needs to handle refine mode.
        Box::new(PerturbedWithRefine {
            inner: PerturbedOracleAgent::new(0xC0FFEE_5EED),
            sub: SubprocessAgent::new(cmd.to_string()),
        })
    } else {
        eprintln!(
            "reliability-bench: unknown --agent `{}`. \
             Try: file-oracle | perturbed | subprocess:<cmd>",
            backend_arg
        );
        return ExitCode::from(2);
    };

    let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());
    let mut per_cat: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new(); // (n, lex_ok, parse_ok)
    for task in &tasks {
        let r = run_one_task(&mut *agent, task);
        let stats = per_cat.entry(task.category.clone()).or_default();
        stats.0 += 1;
        if r.lex_ok { stats.1 += 1; }
        if r.parse_ok { stats.2 += 1; }
        results.push(r);
    }

    let lex_ok = results.iter().filter(|r| r.lex_ok).count();
    let parse_ok = results.iter().filter(|r| r.parse_ok).count();
    let heal_proposed = results.iter().filter(|r| r.heal_proposed).count();
    let heal_succeeded = results.iter().filter(|r| r.heal_succeeded).count();
    let structural_succeeded = results.iter().filter(|r| r.structural_heal_succeeded).count();
    let refine_succeeded = results.iter().filter(|r| r.refine_succeeded).count();
    let total = results.len();

    let report = render_markdown(
        &results, &per_cat, agent.name(), total, lex_ok, parse_ok,
        heal_proposed, heal_succeeded, structural_succeeded,
    );
    if let Err(e) = fs::write(&out_path, &report) {
        eprintln!("reliability-bench: write {out_path}: {e}");
        return ExitCode::from(2);
    }

    let failures = total - parse_ok;
    let total_recovered = heal_succeeded + structural_succeeded + refine_succeeded;
    let effective = parse_ok + total_recovered;
    println!(
        "reliability-bench: {} tasks  |  lex {}/{} ({:.1}%)  parse {}/{} ({:.1}%)  backend={}",
        total,
        lex_ok, total, pct(lex_ok, total),
        parse_ok, total, pct(parse_ok, total),
        agent.name(),
    );
    if failures > 0 {
        println!(
            "  pattern-heal:    proposed {}/{} ({:.1}%)  succeeded {}/{} ({:.1}%)",
            heal_proposed, failures, pct(heal_proposed, failures),
            heal_succeeded, failures, pct(heal_succeeded, failures),
        );
        println!(
            "  structural-heal: succeeded {}/{} ({:.1}% of failures)",
            structural_succeeded, failures, pct(structural_succeeded, failures),
        );
        println!(
            "  refine (stage-3): succeeded {}/{} ({:.1}% of failures)",
            refine_succeeded, failures, pct(refine_succeeded, failures),
        );
        println!(
            "  effective pass:  {}/{} ({:.1}%)  = parse + pattern-heal + structural-heal + refine",
            effective, total, pct(effective, total),
        );
    }
    println!("Full report: {out_path}");

    if parse_ok < total {
        eprintln!(
            "reliability-bench: {} task(s) failed to parse cleanly (see report)",
            total - parse_ok
        );
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn pct(num: usize, denom: usize) -> f64 {
    if denom == 0 { 0.0 } else { num as f64 / denom as f64 * 100.0 }
}

fn run_one_task(agent: &mut dyn CandidateAgent, task: &Task) -> TaskResult {
    let start = Instant::now();
    let mut r = TaskResult { id: task.id.clone(), category: task.category.clone(), ..Default::default() };

    let source = match agent.propose(task) {
        Ok(s) => s,
        Err(e) => {
            r.parse_error_msg = Some(format!("agent refused: {e}"));
            r.time_us = start.elapsed().as_micros();
            return r;
        }
    };

    let tokens = mg_lexer::lex(&source);
    r.token_count = tokens
        .iter()
        .filter(|t| t.kind != mg_lexer::TokenKind::Eof)
        .count() as u32;
    r.lex_errors = tokens
        .iter()
        .filter(|t| t.kind == mg_lexer::TokenKind::Error)
        .count();
    r.lex_ok = r.lex_errors == 0;

    match mg_parser::parse(&tokens) {
        Ok(_) => r.parse_ok = true,
        Err(e) => {
            r.parse_error_msg = Some(format!("{}:{}: {}", e.line, e.col, e.message));
            // Self-healing pass: convert the parse error into a
            // Diagnostic, ask `heal::heal_one` for a ranked fix list,
            // apply the top fix's text edits, and re-parse. Measures
            // the existing repair pipeline's reach on real failures.
            let diag = hir::Diagnostic {
                severity: hir::Severity::Error,
                message: e.message.clone(),
                span: Some(hir::Span {
                    line: e.line as u32,
                    col: e.col as u32,
                }),
                id: None,
                category: None,
            };
            let healed = heal::heal_one(&diag);
            if !healed.fixes.is_empty() {
                r.heal_proposed = true;
                // Multi-pass: try each candidate in confidence order
                // (heal::heal_one returns them already ranked). Stop on
                // the first one whose re-parse succeeds.
                for fix in &healed.fixes {
                    if let Some(patched) = apply_text_edits(&source, &fix.edits) {
                        let tks = mg_lexer::lex(&patched);
                        if mg_parser::parse(&tks).is_ok() {
                            r.heal_succeeded = true;
                            break;
                        }
                        // Layered recovery: if the pattern's edit
                        // alone didn't re-parse, also try the patched
                        // source through structural-balance and
                        // structural-completion. Common for
                        // truncation perturbations where a pattern
                        // inserts a placeholder but braces are still
                        // unbalanced.
                        if let Some(balanced) = structural_heal(&patched) {
                            let tks = mg_lexer::lex(&balanced);
                            if mg_parser::parse(&tks).is_ok() {
                                r.heal_succeeded = true;
                                break;
                            }
                        }
                        if let Some(completed) = structural_completion(&patched) {
                            let tks = mg_lexer::lex(&completed);
                            if mg_parser::parse(&tks).is_ok() {
                                r.heal_succeeded = true;
                                break;
                            }
                        }
                    }
                }
            }
            // Structural-heal fallbacks (run when pattern-heal didn't
            // recover). Try in order: brace balance, completion, then
            // trim-bad-token.
            if !r.heal_succeeded {
                let candidates: Vec<String> = [
                    structural_heal(&source),
                    structural_completion(&source),
                    trim_bad_token(&source),
                ]
                .into_iter()
                .flatten()
                .collect();
                for patched in candidates {
                    let tks = mg_lexer::lex(&patched);
                    if mg_parser::parse(&tks).is_ok() {
                        r.structural_heal_succeeded = true;
                        break;
                    }
                }
            }
            // Stage 3: ask the agent to refine. Trait default is no-op
            // so file/perturbed oracles short-circuit harmlessly; only
            // subprocess backends with a refine-aware wrapper actually
            // recover here.
            if !r.heal_succeeded && !r.structural_heal_succeeded {
                let parse_err_msg = r.parse_error_msg.clone().unwrap_or_default();
                if let Ok(refined) = agent.refine(task, &source, &parse_err_msg) {
                    if refined != source {
                        let tks = mg_lexer::lex(&refined);
                        if tks.iter().all(|t| t.kind != mg_lexer::TokenKind::Error)
                            && mg_parser::parse(&tks).is_ok()
                        {
                            r.refine_succeeded = true;
                        }
                    }
                }
            }
        }
    }
    r.time_us = start.elapsed().as_micros();
    r
}

// ─── Corpus loading ──────────────────────────────────────────────────

fn load_all_tasks(bench_dir: &str) -> Result<Vec<Task>, String> {
    let tasks_dir = format!("{bench_dir}/tasks");
    let entries = fs::read_dir(&tasks_dir).map_err(|e| format!("read_dir {tasks_dir}: {e}"))?;
    let mut paths: Vec<_> = entries
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let content = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        let arr: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("parse {}: {e}", p.display()))?;
        let items = arr.as_array().ok_or_else(|| format!("{} not array", p.display()))?;
        for v in items {
            if let Some(t) = parse_task(v) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

fn parse_task(v: &serde_json::Value) -> Option<Task> {
    Some(Task {
        id: v.get("id")?.as_str()?.to_string(),
        category: v.get("category")?.as_str()?.to_string(),
        description: v.get("task")?.as_str()?.to_string(),
        reference_source: v.get("solution")?.get("rdx_source")?.as_str()?.to_string(),
    })
}

// ─── Report rendering ────────────────────────────────────────────────

fn render_markdown(
    results: &[TaskResult],
    per_cat: &BTreeMap<String, (usize, usize, usize)>,
    backend: &str,
    total: usize,
    lex_ok: usize,
    parse_ok: usize,
    heal_proposed: usize,
    heal_succeeded: usize,
    structural_succeeded: usize,
) -> String {
    let mut out = String::new();
    out.push_str("# MAGE Agent-Write Reliability Report\n\n");
    out.push_str(&format!(
        "Backend: **{}**.  Generated from `benchmarks/tasks/*.json` ({} tasks).\n\n",
        backend, total
    ));

    out.push_str("## Summary\n\n");
    let failures = total - parse_ok;
    out.push_str(&format!(
        "| Stage | Pass | Total | Rate |\n|---|---:|---:|---:|\n\
         | Lex (no error tokens) | {} | {} | {:.1}% |\n\
         | Parse (LL(1) accepts) | {} | {} | {:.1}% |\n\
         | Self-heal proposed a fix (on failures) | {} | {} | {:.1}% |\n\
         | Self-heal made it re-parse | {} | {} | {:.1}% |\n\
         | Structural-heal re-parse (brace balance at EOF) | {} | {} | {:.1}% |\n",
        lex_ok, total, pct(lex_ok, total),
        parse_ok, total, pct(parse_ok, total),
        heal_proposed, failures, pct(heal_proposed, failures),
        heal_succeeded, failures, pct(heal_succeeded, failures),
        structural_succeeded, failures, pct(structural_succeeded, failures),
    ));
    let effective_pass = parse_ok + heal_succeeded + structural_succeeded;
    out.push_str(&format!(
        "\n**Effective pass rate (parse OR pattern-heal OR structural-heal):** {} / {} = {:.1}%\n",
        effective_pass, total, pct(effective_pass, total),
    ));

    out.push_str("\n## Per-category breakdown\n\n");
    out.push_str("| Category | Tasks | Lex OK | Parse OK | Lex % | Parse % |\n|---|---:|---:|---:|---:|---:|\n");
    for (cat, (n, lex, parse)) in per_cat {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}% | {:.1}% |\n",
            cat, n, lex, parse, pct(*lex, *n), pct(*parse, *n),
        ));
    }

    let failures: Vec<&TaskResult> = results
        .iter()
        .filter(|r| !r.parse_ok || !r.lex_ok)
        .collect();
    out.push_str(&format!("\n## Failures ({})\n\n", failures.len()));
    if failures.is_empty() {
        out.push_str("_None._  Every reference solution lexes and parses cleanly through today's compiler.\n");
    } else {
        out.push_str("| Task | Category | Lex errors | Parse error |\n|---|---|---:|---|\n");
        for f in &failures {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                f.id, f.category, f.lex_errors,
                f.parse_error_msg.as_deref().unwrap_or("—"),
            ));
        }
    }

    // Latency histogram — useful for tracking compiler-pipeline cost
    // as part of the same loop, since reliability matters per second
    // for agents.
    let mut sorted_times: Vec<u128> = results.iter().map(|r| r.time_us).collect();
    sorted_times.sort();
    let p50 = sorted_times.get(total / 2).copied().unwrap_or(0);
    let p95 = sorted_times.get(total * 95 / 100).copied().unwrap_or(0);
    let p99 = sorted_times.get(total * 99 / 100).copied().unwrap_or(0);
    out.push_str(&format!(
        "\n## Per-task pipeline latency (lex + parse)\n\n\
         | Percentile | µs |\n|---|---:|\n\
         | p50 | {} |\n| p95 | {} |\n| p99 | {} |\n",
        p50, p95, p99
    ));

    out.push_str(
        "\n---\n_Backend interface: `CandidateAgent::propose(&Task) -> Result<String, String>`. \
         Wire a real LLM by implementing this trait and replacing `FileOracleAgent` in \
         `prototype/src/bin/reliability_bench.rs`._\n",
    );
    out
}

fn find_benchmarks_dir() -> String {
    let mut p = std::env::current_dir().unwrap_or_else(|_| std::path::Path::new(".").to_path_buf());
    loop {
        let cand = p.join("benchmarks");
        if cand.is_dir() {
            return cand.to_string_lossy().to_string();
        }
        if !p.pop() {
            break;
        }
    }
    "benchmarks".to_string()
}
