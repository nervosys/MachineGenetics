//! # `token-bench` — token-efficiency verification for the agent benchmark corpus
//!
//! Loads every task in `benchmarks/tasks/*.json`, re-tokenises both the
//! `solution.rdx_source` (MechGen) and the `rust_equivalent.rs_source`
//! (Rust) using a **shared lexer rule**, and produces:
//!
//! 1. A markdown report (`benchmarks/TOKEN_REPORT.md`) with per-category
//!    aggregates, overall ratio, and the largest outliers.
//! 2. A regression guard that exits non-zero when the corpus's claimed
//!    `token_count` differs from the measured count by more than
//!    `±REGRESSION_PCT %`.
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
//!   (matches MechGen's lexer convention for `+f`, `?:`, `&mut`, etc.)
//!
//! This is intentionally simple — relative comparison only needs
//! consistency, not language-perfect tokenisation. Whitespace and `//`
//! line comments are skipped.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

// Reuse the actual MechGen lexer (which recognises atomic sigils like `+f`,
// `?:`, `&mut`) so the MG side of the comparison matches the corpus's
// claimed counting convention.
#[path = "../lexer.rs"]
mod mg_lexer;

/// Regression threshold: claimed token_count must be within ±10 % of
/// measured token count or the bench exits non-zero.
const REGRESSION_PCT: f64 = 10.0;

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
        .unwrap_or_else(|| find_benchmarks_dir());
    let out_path = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| format!("{bench_dir}/TOKEN_REPORT.md"));

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

        check_regression(&t.id, "mechgen", t.mg_claimed, mg_native, &mut regressions);
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

    if !regressions.is_empty() {
        eprintln!(
            "\ntoken-bench: {} regression(s) (claimed vs measured >{}%)",
            regressions.len(),
            REGRESSION_PCT as i32
        );
        return ExitCode::from(1);
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
    out.push_str("# MechGen Token-Efficiency Report\n\n");
    out.push_str(&format!(
        "Generated from `benchmarks/tasks/*.json` ({} tasks).  \
         Both MechGen and Rust sources are re-tokenised with the same lexer rule \
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
    out.push_str("| Category | Tasks | MechGen bytes | Rust bytes | Ratio | Reduction | Dense MG | Dense RS | Dense ratio |\n");
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
        "MechGen counted by `prototype::lexer` (atomic sigils like `+f` = 1 token). \
         Rust counted by `proc-macro2` (group delimiters count as 2).\n\n",
    );
    out.push_str("| Category | Tasks | MechGen | Rust | Ratio | Reduction |\n");
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
    out.push_str("| Category | MechGen | Rust | Ratio |\n|---|---:|---:|---:|\n");
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
    out.push_str("| Category | MechGen claimed | Rust claimed | Claimed ratio |\n|---|---:|---:|---:|\n");
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

    out.push_str(&format!(
        "\n## Top 10 token savings (MechGen vs Rust)\n\n\
         | Task | Saving | MechGen tokens | Rust tokens |\n\
         |---|---:|---:|---:|\n"
    ));
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
