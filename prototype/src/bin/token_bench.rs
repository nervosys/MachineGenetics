//! # `token-bench` — token-efficiency verification for the agent benchmark corpus
//!
//! Loads every task in `benchmarks/tasks/*.json`, re-tokenises both the
//! `solution.rdx_source` (MAGE) and the `rust_equivalent.rs_source`
//! (Rust) using a **shared lexer rule**, and produces:
//!
//! 1. A markdown report (`benchmarks/TOKEN_REPORT.md`) with per-category
//!    aggregates, overall ratio, and the largest outliers.
//! 2. A ratchet on the set of tasks whose claimed `token_count` differs from
//!    the measured count by more than `±REGRESSION_PCT %`. The known set lives
//!    in `benchmarks/token-claims-baseline.txt` and **may only shrink**; the
//!    exit status reports movement against it, not its size.
//!
//!    It was a plain guard — non-zero whenever *any* claim disagreed — which
//!    meant non-zero on every run ever made, because 150 of them do. That is
//!    the subject of `benchmarks/FINDINGS.md` §1 and is a finding about the
//!    corpus rather than a regression. A status that cannot be green cannot
//!    report a 151st, and `check-ci-floors.sh` had given up reading it.
//!
//! ## Lexer rule
//!
//! Both languages run through `tokenize`, which produces one token per:
//!
//! - identifier / keyword (`[A-Za-z_][A-Za-z0-9_]*`)
//! - integer / float literal (incl. `0x`, `0b`, `e`/`E` exponents)
//! - string literal (`"..."` with `\"` escapes)
//! - char literal (`'.'` with `'\\.'`)
//! - **each non-alphanumeric sigil character** as its own token
//!   (matches MAGE's lexer convention for `+f`, `?:`, `&mut`, etc.)
//!
//! This is intentionally simple — relative comparison only needs
//! consistency, not language-perfect tokenisation. Whitespace and `//`
//! line comments are skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

// Reuse the actual MAGE lexer (which recognises atomic sigils like `+f`,
// `?:`, `&mut`) so the MG side of the comparison matches the corpus's
// claimed counting convention.
use mage_prototype::lexer as mg_lexer;
use mage_prototype::bpe::Vocab;

/// Regression threshold: claimed token_count must be within ±10 % of
/// measured token count or the bench exits non-zero.
const REGRESSION_PCT: f64 = 10.0;

/// The known-disagreeing set. May only shrink; see the ratchet in `main`.
const BASELINE_PATH: &str = "benchmarks/token-claims-baseline.txt";

const BASELINE_HEADER: &str = "# Corpus token_count claims that disagree with measurement by more than
# REGRESSION_PCT. One `task_id<TAB>lang` per line, sorted.
#
# This file exists so that `token-bench`'s exit status can mean something. The
# disagreements below are the subject of benchmarks/FINDINGS.md 1 - they are a
# finding about the corpus, not regressions - but with 150 of them permanently
# red, the status could not report a 151st. It can now.
#
# It may only shrink. Correcting a claim means deleting its line in the same
# commit; the bench fails if an entry here has stopped disagreeing.
";

#[derive(Debug)]
struct Task {
    id: String,
    category: String,
    mg_source: String,
    mg_claimed: u32,
    rs_source: String,
    rs_claimed: u32,
}

#[derive(Default, Debug, Clone, Copy)]
struct CategoryAgg {
    n: usize,
    mg_native: u64,
    rs_native: u64,
    mg_shared: u64,
    rs_shared: u64,
    mg_claimed: u64,
    rs_claimed: u64,
    /// Source byte counts — what an LLM's BPE tokenizer actually sees.
    /// More honest than syntactic-token counts for agent-input cost.
    mg_bytes: u64,
    rs_bytes: u64,
    /// Whitespace-stripped byte counts — removes formatting from the
    /// comparison. (LLMs do see whitespace, so this is a lower bound.)
    mg_dense_bytes: u64,
    rs_dense_bytes: u64,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let bench_dir = args
        .iter()
        .position(|a| a == "--dir")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(find_benchmarks_dir);
    let out_path = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| format!("{bench_dir}/TOKEN_REPORT.md"));
    let bpe_opts = match parse_bpe_opts(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("token-bench: {e}");
            return ExitCode::from(2);
        }
    };
    let custom_vocab = match (bpe_opts.enabled, &bpe_opts.vocab_path) {
        (true, Some(p)) => match Vocab::load(p) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("token-bench: vocab {p}: {e}");
                return ExitCode::from(2);
            }
        },
        _ => None,
    };

    let tasks = match load_all_tasks(&bench_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("token-bench: load: {e}");
            return ExitCode::from(2);
        }
    };
    if tasks.is_empty() {
        eprintln!("token-bench: no tasks found in {bench_dir}/tasks/");
        return ExitCode::from(2);
    }

    let mut per_cat: BTreeMap<String, CategoryAgg> = BTreeMap::new();
    let mut regressions: Vec<(String, &'static str, u32, u32)> = Vec::new();
    let mut top_savings: Vec<(String, f64, u32, u32)> = Vec::new();

    for t in &tasks {
        // Native lexers — fair comparison: each language counted via the
        // tokenisation rule its own lexer would use.
        let mg_native = mg_lexer::lex(&t.mg_source)
            .iter()
            .filter(|tok| tok.kind != mg_lexer::TokenKind::Eof
                       && tok.kind != mg_lexer::TokenKind::Error)
            .count() as u32;
        let rs_native = match proc_macro2::TokenStream::from_str(&t.rs_source) {
            Ok(ts) => count_pm2_tokens(ts),
            Err(_) => tokenize(&t.rs_source).len() as u32,
        };

        // Shared rule — apples-to-apples but ignores each lexer's
        // atomic-sigil conventions. Kept for transparency.
        let mg_shared = tokenize(&t.mg_source).len() as u32;
        let rs_shared = tokenize(&t.rs_source).len() as u32;

        check_regression(&t.id, "mage", t.mg_claimed, mg_native, &mut regressions);
        check_regression(&t.id, "rust", t.rs_claimed, rs_native, &mut regressions);

        let mg_bytes = t.mg_source.len() as u64;
        let rs_bytes = t.rs_source.len() as u64;
        let mg_dense = t.mg_source.chars().filter(|c| !c.is_whitespace()).count() as u64;
        let rs_dense = t.rs_source.chars().filter(|c| !c.is_whitespace()).count() as u64;

        let agg = per_cat.entry(t.category.clone()).or_default();
        agg.n += 1;
        agg.mg_native += mg_native as u64;
        agg.rs_native += rs_native as u64;
        agg.mg_shared += mg_shared as u64;
        agg.rs_shared += rs_shared as u64;
        agg.mg_claimed += t.mg_claimed as u64;
        agg.rs_claimed += t.rs_claimed as u64;
        agg.mg_bytes += mg_bytes;
        agg.rs_bytes += rs_bytes;
        agg.mg_dense_bytes += mg_dense;
        agg.rs_dense_bytes += rs_dense;

        if rs_native > 0 {
            let saving = 1.0 - (mg_native as f64 / rs_native as f64);
            top_savings.push((t.id.clone(), saving, mg_native, rs_native));
        }
    }
    top_savings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let report = render_markdown(&per_cat, &top_savings, &regressions, tasks.len());
    if let Err(e) = fs::write(&out_path, &report) {
        eprintln!("token-bench: write {out_path}: {e}");
        return ExitCode::from(2);
    }
    // Also echo a compact summary to stdout.
    print_summary(&per_cat, tasks.len(), &regressions);

    println!("\nFull report: {out_path}");

    // Opt-in BPE section. Nothing here runs without `--bpe`, and it never
    // touches TOKEN_REPORT.md or the baseline ratchet below.
    if bpe_opts.enabled {
        let vocab: &Vocab = custom_vocab.as_ref().unwrap_or_else(|| Vocab::mage());
        let section = render_bpe_section(&bpe_vocab_label(&bpe_opts), &bpe_rows(vocab, &tasks));
        print!("\n{section}");
        if let Some(p) = &bpe_opts.out_path {
            if let Err(e) = fs::write(p, &section) {
                eprintln!("token-bench: write {p}: {e}");
                return ExitCode::from(2);
            }
        }
    }

    // ── The ratchet ──────────────────────────────────────────────────────
    //
    // `!regressions.is_empty() -> exit 1` was correct and useless. 150 of the
    // corpus's claimed `token_count` values disagree with measurement — which
    // is the *finding* of `benchmarks/FINDINGS.md` §1, not a regression — so
    // the guard fired on every run that had ever been made, and a signal that
    // is always red gates nothing. `check-ci-floors.sh` worked around it by
    // ignoring the exit status entirely, which is how a bench ends up unable
    // to fail.
    //
    // Deciding what those 150 claims *mean* is a question about the corpus and
    // is still open. Deciding that no more should appear is not, and that is
    // all this does: the known set is recorded, and the status reports only
    // movement against it. Same shape as `scripts/doc-blocks-baseline.txt`,
    // and it may only shrink.
    let now: BTreeSet<String> = regressions
        .iter()
        .map(|(id, lang, _, _)| format!("{id}\t{lang}"))
        .collect();

    let was: BTreeSet<String> = match fs::read_to_string(BASELINE_PATH) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
        Err(_) => {
            // No baseline yet: write what was measured and fail anyway. A first
            // run must not silently bless whatever it happens to find.
            let mut out = String::from(BASELINE_HEADER);
            for k in &now {
                out.push_str(k);
                out.push('\n');
            }
            if let Err(e) = fs::write(BASELINE_PATH, out) {
                eprintln!("token-bench: write {BASELINE_PATH}: {e}");
                return ExitCode::from(2);
            }
            eprintln!(
                "\ntoken-bench: no baseline; wrote {} entries to {BASELINE_PATH}.\n\
                 Review it before committing - this run blessed whatever it measured.",
                now.len()
            );
            return ExitCode::from(1);
        }
    };

    let fresh: Vec<&String> = now.difference(&was).collect();
    let fixed: Vec<&String> = was.difference(&now).collect();

    if !fresh.is_empty() {
        eprintln!(
            "\ntoken-bench: {} NEW claim(s) disagree with measurement by >{}%:",
            fresh.len(),
            REGRESSION_PCT as i32
        );
        for k in &fresh {
            eprintln!("  x  {k}");
        }
        eprintln!("Fix the claim, or record why it moved. Do not widen the baseline.");
        return ExitCode::from(1);
    }

    if !fixed.is_empty() {
        // A ratchet whose baseline is allowed to go stale stops being one.
        eprintln!(
            "\ntoken-bench: {} baseline entr(ies) no longer disagree:",
            fixed.len()
        );
        for k in &fixed {
            eprintln!("  +  {k}");
        }
        eprintln!("Remove them from {BASELINE_PATH} in this commit.");
        return ExitCode::from(1);
    }

    if !now.is_empty() {
        println!(
            "\ntoken-bench: {} known claim disagreement(s), none new (baseline {BASELINE_PATH}).",
            now.len()
        );
    }
    ExitCode::SUCCESS
}

/// Walk benchmarks/tasks/*.json and parse every task block.
fn load_all_tasks(bench_dir: &str) -> Result<Vec<Task>, String> {
    let tasks_dir = format!("{bench_dir}/tasks");
    let mut tasks = Vec::new();
    let entries = fs::read_dir(&tasks_dir).map_err(|e| format!("read_dir {tasks_dir}: {e}"))?;
    let mut paths: Vec<_> = entries
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
        .collect();
    paths.sort();
    for p in paths {
        let content = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        let arr: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| format!("parse {}: {e}", p.display()))?;
        let items = arr.as_array().ok_or_else(|| {
            format!("{} not a JSON array of tasks", p.display())
        })?;
        for it in items {
            if let Some(t) = parse_task(it) {
                tasks.push(t);
            }
        }
    }
    Ok(tasks)
}

fn parse_task(v: &serde_json::Value) -> Option<Task> {
    let id = v.get("id")?.as_str()?.to_string();
    let category = v.get("category")?.as_str()?.to_string();
    let solution = v.get("solution")?;
    let rust_eq = v.get("rust_equivalent")?;
    Some(Task {
        id,
        category,
        mg_source: solution.get("rdx_source")?.as_str()?.to_string(),
        mg_claimed: solution.get("token_count")?.as_u64()? as u32,
        rs_source: rust_eq.get("rs_source")?.as_str()?.to_string(),
        rs_claimed: rust_eq.get("token_count")?.as_u64()? as u32,
    })
}

fn check_regression(
    id: &str,
    lang: &'static str,
    claimed: u32,
    measured: u32,
    out: &mut Vec<(String, &'static str, u32, u32)>,
) {
    if claimed == 0 || measured == 0 {
        return;
    }
    let pct = ((measured as f64 - claimed as f64) / claimed as f64 * 100.0).abs();
    if pct > REGRESSION_PCT {
        out.push((id.to_string(), lang, claimed, measured));
    }
}

/// Count tokens in a `proc_macro2::TokenStream`. Groups (parens, braces,
/// brackets) contribute 2 (open + close) plus their inner stream.
fn count_pm2_tokens(stream: proc_macro2::TokenStream) -> u32 {
    let mut n = 0u32;
    for tt in stream {
        match tt {
            proc_macro2::TokenTree::Group(g) => {
                n += 2;
                n += count_pm2_tokens(g.stream());
            }
            // Punct, Ident, Literal each count as one.
            _ => n += 1,
        }
    }
    n
}

/// Shared tokeniser — see module docs for the rule.
fn tokenize(src: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // Whitespace.
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // Line comment.
        if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // String literal.
        if c == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < bytes.len() {
                i += 1; // consume closing "
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or(""));
            continue;
        }
        // Char literal.
        if c == b'\'' && i + 2 < bytes.len() {
            let start = i;
            i += 1;
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
            } else {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'\'' {
                i += 1;
                tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or(""));
                continue;
            }
            // Not a char literal after all — fall through to sigil handling.
            i = start;
        }
        // Identifier / keyword.
        if c == b'_' || c.is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len()
                && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric())
            {
                i += 1;
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or(""));
            continue;
        }
        // Number literal (incl. 0x, 0b prefixes, decimal, exponent).
        if c.is_ascii_digit() {
            let start = i;
            // Optional 0x or 0b prefix.
            if c == b'0' && i + 1 < bytes.len() && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X' || bytes[i + 1] == b'b' || bytes[i + 1] == b'B') {
                i += 2;
                while i < bytes.len() && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                        i += 1;
                    }
                }
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                        i += 1;
                    }
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            // Optional type suffix (i32, u8, f64, etc.).
            if i < bytes.len() && (bytes[i] == b'i' || bytes[i] == b'u' || bytes[i] == b'f') {
                let j = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                // Roll back if no digits followed.
                if i == j + 1 {
                    i = j;
                }
            }
            tokens.push(std::str::from_utf8(&bytes[start..i]).unwrap_or(""));
            continue;
        }
        // Single-byte sigil token.
        tokens.push(std::str::from_utf8(&bytes[i..i + 1]).unwrap_or(""));
        i += 1;
    }
    tokens
}

fn render_markdown(
    per_cat: &BTreeMap<String, CategoryAgg>,
    top_savings: &[(String, f64, u32, u32)],
    regressions: &[(String, &'static str, u32, u32)],
    total_tasks: usize,
) -> String {
    let mut out = String::new();
    out.push_str("# MAGE Token-Efficiency Report\n\n");
    out.push_str(&format!(
        "Generated from `benchmarks/tasks/*.json` ({} tasks).  \
         Both MAGE and Rust sources are re-tokenised with the same lexer rule \
         (identifiers, literals, single-character sigils) before counting.\n\n",
        total_tasks
    ));

    let mut total = CategoryAgg::default();
    for a in per_cat.values() {
        total.n += a.n;
        total.mg_native += a.mg_native;
        total.rs_native += a.rs_native;
        total.mg_shared += a.mg_shared;
        total.rs_shared += a.rs_shared;
        total.mg_claimed += a.mg_claimed;
        total.rs_claimed += a.rs_claimed;
        total.mg_bytes += a.mg_bytes;
        total.rs_bytes += a.rs_bytes;
        total.mg_dense_bytes += a.mg_dense_bytes;
        total.rs_dense_bytes += a.rs_dense_bytes;
    }

    out.push_str("## Source bytes (what LLM BPE actually sees)\n\n");
    out.push_str(
        "The most honest measurement for agent-input cost: raw source bytes \
         and whitespace-stripped bytes. LLM BPE tokens correlate roughly with \
         bytes (≈ 3–4 bytes / token for code), so this is what determines an \
         agent's context-window and inference cost.\n\n",
    );
    out.push_str("| Category | Tasks | MAGE bytes | Rust bytes | Ratio | Reduction | Dense MG | Dense RS | Dense ratio |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for (cat, a) in per_cat {
        let r = ratio(a.mg_bytes, a.rs_bytes);
        let rd = ratio(a.mg_dense_bytes, a.rs_dense_bytes);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.3} | {:.1}% | {} | {} | {:.3} |\n",
            cat, a.n, a.mg_bytes, a.rs_bytes, r, (1.0 - r) * 100.0,
            a.mg_dense_bytes, a.rs_dense_bytes, rd,
        ));
    }
    let r_b = ratio(total.mg_bytes, total.rs_bytes);
    let r_db = ratio(total.mg_dense_bytes, total.rs_dense_bytes);
    out.push_str(&format!(
        "| **Total** | **{}** | **{}** | **{}** | **{:.3}** | **{:.1}%** | **{}** | **{}** | **{:.3}** |\n\n",
        total.n, total.mg_bytes, total.rs_bytes, r_b, (1.0 - r_b) * 100.0,
        total.mg_dense_bytes, total.rs_dense_bytes, r_db,
    ));

    out.push_str("## Per-category aggregates (native lexers)\n\n");
    out.push_str(
        "MAGE counted by `prototype::lexer` (atomic sigils like `+f` = 1 token). \
         Rust counted by `proc-macro2` (group delimiters count as 2).\n\n",
    );
    out.push_str("| Category | Tasks | MAGE | Rust | Ratio | Reduction |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|\n");
    for (cat, a) in per_cat {
        let r = ratio(a.mg_native, a.rs_native);
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.3} | {:.1}% |\n",
            cat, a.n, a.mg_native, a.rs_native, r, (1.0 - r) * 100.0,
        ));
    }
    let r_total = ratio(total.mg_native, total.rs_native);
    out.push_str(&format!(
        "| **Total** | **{}** | **{}** | **{}** | **{:.3}** | **{:.1}%** |\n",
        total.n, total.mg_native, total.rs_native, r_total, (1.0 - r_total) * 100.0,
    ));

    out.push_str("\n## Shared-rule cross-check\n\n");
    out.push_str(
        "Same naive tokeniser (whitespace + identifier + literal + single \
         sigil) applied to both. Removes lexer-convention advantage; shows \
         the savings that come from sigil grouping vs from raw character \
         density.\n\n",
    );
    out.push_str("| Category | MAGE | Rust | Ratio |\n|---|---:|---:|---:|\n");
    for (cat, a) in per_cat {
        out.push_str(&format!(
            "| {} | {} | {} | {:.3} |\n",
            cat, a.mg_shared, a.rs_shared, ratio(a.mg_shared, a.rs_shared),
        ));
    }
    out.push_str(&format!(
        "| **Total** | **{}** | **{}** | **{:.3}** |\n",
        total.mg_shared, total.rs_shared, ratio(total.mg_shared, total.rs_shared),
    ));

    out.push_str("\n## Claimed vs measured (corpus integrity)\n\n");
    out.push_str("| Category | MAGE claimed | Rust claimed | Claimed ratio |\n|---|---:|---:|---:|\n");
    for (cat, a) in per_cat {
        out.push_str(&format!(
            "| {} | {} | {} | {:.3} |\n",
            cat, a.mg_claimed, a.rs_claimed, ratio(a.mg_claimed, a.rs_claimed),
        ));
    }
    out.push_str(&format!(
        "| **Total** | **{}** | **{}** | **{:.3}** |\n",
        total.mg_claimed, total.rs_claimed, ratio(total.mg_claimed, total.rs_claimed),
    ));

    out.push_str("\n## Top 10 token savings (MAGE vs Rust)\n\n\
         | Task | Saving | MAGE tokens | Rust tokens |\n\
         |---|---:|---:|---:|\n");
    for (id, saving, mg, rs) in top_savings.iter().take(10) {
        out.push_str(&format!(
            "| {} | {:.1}% | {} | {} |\n",
            id,
            saving * 100.0,
            mg,
            rs,
        ));
    }

    out.push_str(&format!(
        "\n## Regressions (|claimed − measured| > {}%)\n\n",
        REGRESSION_PCT as i32
    ));
    if regressions.is_empty() {
        out.push_str("_None._ Corpus token-count claims agree with the verified lexer within tolerance.\n");
    } else {
        out.push_str("| Task | Lang | Claimed | Measured | Δ |\n|---|---|---:|---:|---:|\n");
        for (id, lang, claimed, measured) in regressions {
            let delta = *measured as i64 - *claimed as i64;
            out.push_str(&format!(
                "| {} | {} | {} | {} | {:+} |\n",
                id, lang, claimed, measured, delta
            ));
        }
    }

    out.push_str(&format!(
        "\n---\n_Lexer rule used: see `prototype/src/bin/token_bench.rs` docs. Regression threshold: ±{}%._\n",
        REGRESSION_PCT as i32
    ));
    out
}

fn print_summary(
    per_cat: &BTreeMap<String, CategoryAgg>,
    total_tasks: usize,
    regressions: &[(String, &'static str, u32, u32)],
) {
    let mut total = CategoryAgg::default();
    for a in per_cat.values() {
        total.n += a.n;
        total.mg_native += a.mg_native;
        total.rs_native += a.rs_native;
        total.mg_shared += a.mg_shared;
        total.rs_shared += a.rs_shared;
        total.mg_bytes += a.mg_bytes;
        total.rs_bytes += a.rs_bytes;
        total.mg_dense_bytes += a.mg_dense_bytes;
        total.rs_dense_bytes += a.rs_dense_bytes;
    }
    println!(
        "token-bench: {} tasks across {} categories",
        total_tasks,
        per_cat.len()
    );
    let r_native = ratio(total.mg_native, total.rs_native);
    let r_shared = ratio(total.mg_shared, total.rs_shared);
    let r_bytes = ratio(total.mg_bytes, total.rs_bytes);
    let r_dense = ratio(total.mg_dense_bytes, total.rs_dense_bytes);
    let mut by_cat: Vec<_> = per_cat.values().copied().collect();
    by_cat.iter_mut().for_each(|_| {});
    let _ = by_cat;

    println!(
        "  source bytes:  MG={:>5}  RS={:>5}  ratio={:.3}  ({:.1}% reduction)  ← BPE-like",
        total.mg_bytes, total.rs_bytes, r_bytes, (1.0 - r_bytes) * 100.0,
    );
    println!(
        "  dense bytes:   MG={:>5}  RS={:>5}  ratio={:.3}  ({:.1}% reduction)  ← whitespace-stripped",
        total.mg_dense_bytes, total.rs_dense_bytes, r_dense, (1.0 - r_dense) * 100.0,
    );
    println!(
        "  native lexers: MG={:>5}  RS={:>5}  ratio={:.3}  ({:.1}% reduction)  ← syntactic tokens",
        total.mg_native, total.rs_native, r_native, (1.0 - r_native) * 100.0,
    );
    println!(
        "  shared rule:   MG={:>5}  RS={:>5}  ratio={:.3}  ({:.1}% reduction)",
        total.mg_shared, total.rs_shared, r_shared, (1.0 - r_shared) * 100.0,
    );
    if !regressions.is_empty() {
        println!("  regressions: {} (claimed vs native-measured)", regressions.len());
    }
}

/// Options for the opt-in BPE measurement.
#[derive(Debug, Default, PartialEq)]
struct BpeOpts {
    /// `--bpe`: count every source with a BPE vocab.
    enabled: bool,
    /// `--vocab FILE`: tiktoken-format vocab to use instead of the shipped one.
    vocab_path: Option<String>,
    /// `--bpe-out FILE`: also write the BPE section to this file.
    out_path: Option<String>,
}

const BPE_SHIPPED_LABEL: &str = "mage-bpe-4096";

const BPE_CAVEAT: &str = "Caveat: the shipped `mage-bpe-4096` vocab was trained on `.mg` files, \
     so it favours MAGE syntax (the benchmark tasks themselves, which live in JSON, are not in its corpus). \
     A neutral vocab would be expected to score MAGE no better than this ratio; \
     it is not a neutral measurement.";

/// Value following `flag`, `Ok(None)` if the flag is absent, `Err` if it has no value.
fn flag_value(args: &[String], flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{flag} requires a value")),
    }
}

/// Parse `--bpe`, `--vocab FILE`, `--bpe-out FILE`. Without `--bpe` the other
/// two are rejected, so the default invocation is exactly what it was before.
fn parse_bpe_opts(args: &[String]) -> Result<BpeOpts, String> {
    let enabled = args.iter().any(|a| a == "--bpe");
    let vocab_path = flag_value(args, "--vocab")?;
    let out_path = flag_value(args, "--bpe-out")?;
    if !enabled && (vocab_path.is_some() || out_path.is_some()) {
        return Err("--vocab and --bpe-out require --bpe".to_string());
    }
    Ok(BpeOpts {
        enabled,
        vocab_path,
        out_path,
    })
}

/// Name printed for the vocab: the shipped label, or the `--vocab` file name.
fn bpe_vocab_label(opts: &BpeOpts) -> String {
    match &opts.vocab_path {
        None => BPE_SHIPPED_LABEL.to_string(),
        Some(p) => Path::new(p)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone()),
    }
}

#[derive(Debug, PartialEq)]
struct BpeRow {
    id: String,
    category: String,
    mg: usize,
    rs: usize,
}

/// BPE token count of each task's MAGE and Rust source.
fn bpe_rows(vocab: &Vocab, tasks: &[Task]) -> Vec<BpeRow> {
    tasks
        .iter()
        .map(|t| BpeRow {
            id: t.id.clone(),
            category: t.category.clone(),
            mg: vocab.count(&t.mg_source),
            rs: vocab.count(&t.rs_source),
        })
        .collect()
}

/// Markdown section: aggregate MAGE/Rust BPE ratio, caveat, per-task table.
fn render_bpe_section(vocab_label: &str, rows: &[BpeRow]) -> String {
    let mg: u64 = rows.iter().map(|r| r.mg as u64).sum();
    let rs: u64 = rows.iter().map(|r| r.rs as u64).sum();
    let r = ratio(mg, rs);
    let mut out = String::new();
    out.push_str(&format!("## BPE token counts (vocab: {vocab_label})\n\n"));
    out.push_str(&format!(
        "**Aggregate:** MG={mg}  RS={rs}  ratio={r:.3}  ({:.1}% reduction)  \u{2190} BPE tokens, vocab: {vocab_label}\n\n",
        (1.0 - r) * 100.0,
    ));
    out.push_str(&format!("{BPE_CAVEAT}\n\n"));
    out.push_str("| Task | Category | MAGE BPE | Rust BPE | Ratio |\n|---|---|---:|---:|---:|\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.3} |\n",
            row.id,
            row.category,
            row.mg,
            row.rs,
            ratio(row.mg as u64, row.rs as u64),
        ));
    }
    out.push_str(&format!(
        "| **Total** | | **{mg}** | **{rs}** | **{r:.3}** |\n"
    ));
    out
}

fn ratio(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        a as f64 / b as f64
    }
}

/// Walk up from the current dir looking for a `benchmarks/` folder.
fn find_benchmarks_dir() -> String {
    let mut p = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(xs: &[&str]) -> Vec<String> {
        std::iter::once("token-bench")
            .chain(xs.iter().copied())
            .map(str::to_string)
            .collect()
    }

    fn task(id: &str, mg: &str, rs: &str) -> Task {
        Task {
            id: id.to_string(),
            category: "test".to_string(),
            mg_source: mg.to_string(),
            mg_claimed: 0,
            rs_source: rs.to_string(),
            rs_claimed: 0,
        }
    }

    #[test]
    fn bpe_flags_absent_means_disabled() {
        let o = parse_bpe_opts(&args(&["--dir", "x"])).unwrap();
        assert_eq!(o, BpeOpts::default());
        assert!(!o.enabled);
    }

    #[test]
    fn bpe_flag_alone_uses_shipped_vocab() {
        let o = parse_bpe_opts(&args(&["--bpe"])).unwrap();
        assert!(o.enabled);
        assert_eq!(o.vocab_path, None);
        assert_eq!(o.out_path, None);
        assert_eq!(bpe_vocab_label(&o), "mage-bpe-4096");
    }

    #[test]
    fn bpe_flags_parse_vocab_and_out() {
        let o = parse_bpe_opts(&args(&[
            "--bpe",
            "--vocab",
            "C:/vocabs/rust.tiktoken",
            "--bpe-out",
            "bpe.md",
        ]))
        .unwrap();
        assert!(o.enabled);
        assert_eq!(o.vocab_path.as_deref(), Some("C:/vocabs/rust.tiktoken"));
        assert_eq!(o.out_path.as_deref(), Some("bpe.md"));
        assert_eq!(bpe_vocab_label(&o), "rust.tiktoken");
    }

    #[test]
    fn bpe_vocab_or_out_without_bpe_is_rejected() {
        assert!(parse_bpe_opts(&args(&["--vocab", "v.tiktoken"])).is_err());
        assert!(parse_bpe_opts(&args(&["--bpe-out", "o.md"])).is_err());
    }

    #[test]
    fn bpe_flag_missing_value_is_rejected() {
        assert!(parse_bpe_opts(&args(&["--bpe", "--vocab"])).is_err());
    }

    #[test]
    fn bpe_rows_count_with_shipped_vocab() {
        let vocab = Vocab::mage();
        let mg = "fn add(a: i64, b: i64) -> i64 { a + b }";
        let rs = "fn add(a: i64, b: i64) -> i64 { a + b }";
        let rows = bpe_rows(vocab, &[task("t1", mg, rs)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].mg, vocab.encode(mg).len());
        assert_eq!(rows[0].rs, vocab.encode(rs).len());
        assert!(rows[0].mg > 0);
    }

    #[test]
    fn bpe_section_has_vocab_label_caveat_and_totals() {
        let rows = vec![
            BpeRow { id: "a".into(), category: "c".into(), mg: 10, rs: 20 },
            BpeRow { id: "b".into(), category: "c".into(), mg: 30, rs: 40 },
        ];
        let s = render_bpe_section("mage-bpe-4096", &rows);
        assert!(s.contains("vocab: mage-bpe-4096"));
        assert!(s.contains("favours MAGE"));
        assert!(s.contains("MG=40  RS=60  ratio=0.667"));
        assert!(s.contains("| **Total** | | **40** | **60** | **0.667** |"));
    }
}
