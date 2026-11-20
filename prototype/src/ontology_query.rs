//! Budgeted queries over the ontology.
//!
//! `--emit-ontology` builds an ontology of ~63,000 BPE tokens (as emitted, LF line ends). An agent that
//! has never seen MAGE cannot afford to load it whole; it needs the few facts
//! one task requires. This module answers with *slices of what
//! `ontology::build()` produces* -- there is no second source of truth -- cut
//! to a token budget counted by the built-in BPE (`Vocab::mage()`).
//!
//! Two calls: [`index`] (the classes, with counts and one line each -- the
//! agent's first call) and [`query`] (entries matching a term).
//!
//! Matching tiers, strongest first: exact name, case-insensitive name
//! (leading `-`/`@`/`+` ignored), name prefix, name substring, description
//! substring. Within a tier the order is the class order below, then the
//! ontology's own row order, so answers are deterministic.

use serde_json::{json, Value};

/// Default response budget in BPE tokens.
pub const DEFAULT_BUDGET: usize = 600;

/// Every ontology section with the one-line description the index shows.
/// The order is the order of results within a match tier. A test checks this
/// table against `ontology::build()` in both directions.
const KINDS: &[(&str, &str)] = &[
    ("keywords", "reserved words"),
    ("sigils", "sigils and AST nodes"),
    ("types", "built-in types"),
    ("effects", "effect annotations"),
    ("vocabulary", "list, map, string functions"),
    ("diagnostics", "error codes and fixes"),
    ("builtins", "other functions, io/fs ops"),
    ("ast_kinds", "AST node kinds"),
    ("examples", "runnable programs"),
    ("ir_ops", "IR operations"),
    ("op_families", "IR op groups"),
    ("layer_map", "layer to IR op"),
    ("framewerx_modules", "framework modules"),
    ("cli_flags", "command-line flags"),
    ("rap_methods", "RAP methods"),
    ("heal_patterns", "repair patterns"),
    ("recovery_stages", "recovery stages"),
    ("abl", "binary IR container"),
    ("hardware_accelerators", "compute backends"),
    ("bench_backends", "benchmark backends"),
    ("wrapper_protocol", "model wrapper"),
    ("ci_floors", "CI floors"),
    ("project_layout", "repo directories"),
    ("docs", "documents"),
];

const NAME_KEYS: &[&str] =
    &["name", "flag", "method", "keyword", "sigil", "stage", "surface_name", "path", "kind"];
const DESC_KEYS: &[&str] = &["summary", "purpose", "description", "protects"];

struct Entry {
    kind: &'static str,
    name: String,
    /// Extra names the entry answers to (e.g. a layer's IR op name).
    alias: Option<String>,
    desc: String,
    body: Value,
}

fn tokens(v: &Value) -> usize {
    crate::bpe::Vocab::mage().count(&serde_json::to_string(v).unwrap_or_default())
}

fn str_of<'a>(row: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| row.get(*k).and_then(|v| v.as_str()))
}

/// Every entry, built once per process.
fn load() -> &'static [Entry] {
    static ENTRIES: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    ENTRIES.get_or_init(build_entries)
}

fn build_entries() -> Vec<Entry> {
    let onto = crate::ontology::build();
    let sections = &onto["sections"];
    let mut out = Vec::new();
    for (kind, _) in KINDS {
        let Some(sec) = sections.get(*kind) else { continue };
        match sec {
            Value::Array(rows) => {
                for row in rows {
                    let name = str_of(row, NAME_KEYS).unwrap_or("").to_string();
                    // The other name an entry answers to: a layer's IR op, a
                    // diagnostic's category (`UseAfterMove` for `E0382`), a
                    // capability op without its namespace (`read_line`).
                    let alias_key = match *kind {
                        "layer_map" => Some("op_name"),
                        "diagnostics" => Some("category"),
                        "builtins" => Some("op"),
                        _ => None,
                    };
                    let alias = alias_key
                        .and_then(|k| row.get(k))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    out.push(Entry {
                        kind,
                        name,
                        alias,
                        desc: str_of(row, DESC_KEYS).unwrap_or("").to_string(),
                        body: row.clone(),
                    });
                }
            }
            Value::Object(map) => {
                for (k, v) in map {
                    out.push(Entry { kind, name: k.clone(), alias: None, desc: String::new(), body: v.clone() });
                }
            }
            _ => {}
        }
    }
    out
}

/// Strip leading sigil characters so `check` finds `--check`.
fn loose(s: &str) -> String {
    s.trim_start_matches(['-', '@', '+']).to_lowercase()
}

/// Match tier of an entry for a term: 1 (best) .. 5, or `None`.
fn tier(e: &Entry, term: &str) -> Option<u8> {
    let lt = loose(term);
    let names = std::iter::once(e.name.as_str()).chain(e.alias.as_deref());
    let mut best: Option<u8> = None;
    for n in names {
        let ln = loose(n);
        let t = if n == term {
            1
        } else if n.eq_ignore_ascii_case(term) || (!lt.is_empty() && ln == lt) {
            2
        } else if !lt.is_empty() && ln.starts_with(&lt) {
            3
        } else if !lt.is_empty() && ln.contains(&lt) {
            4
        } else {
            continue;
        };
        best = Some(best.map_or(t, |b| b.min(t)));
    }
    if best.is_none() && !lt.is_empty() && e.desc.to_lowercase().contains(&term.to_lowercase()) {
        best = Some(5);
    }
    best
}

/// Names of directly related entries, as `kind:name`, at most eight.
fn related(e: &Entry, all: &[Entry]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let add = |kind: &str, name: &str, out: &mut Vec<String>| {
        let s = format!("{kind}:{name}");
        if !(kind == e.kind && name == e.name) && !out.contains(&s) {
            out.push(s);
        }
    };
    let fam_name = |code: i64| {
        all.iter()
            .find(|x| x.kind == "op_families" && x.body["code"].as_i64() == Some(code))
            .map(|x| x.name.clone())
    };
    match e.kind {
        "ir_ops" => {
            if let Some(f) = e.body["family"].as_i64().and_then(fam_name) {
                add("op_families", &f, &mut out);
            }
            for x in all.iter().filter(|x| x.kind == "layer_map" && x.alias.as_deref() == Some(&e.name)) {
                add("layer_map", &x.name, &mut out);
            }
        }
        "op_families" => {
            let code = e.body["code"].as_i64();
            for x in all.iter().filter(|x| x.kind == "ir_ops" && x.body["family"].as_i64() == code) {
                add("ir_ops", &x.name, &mut out);
            }
        }
        "layer_map" => {
            if let Some(op) = &e.alias {
                add("ir_ops", op, &mut out);
            }
            if let Some(f) = e.body["family"].as_i64().and_then(fam_name) {
                add("op_families", &f, &mut out);
            }
        }
        "sigils" => {
            for k in e.body["ast_kind"].as_str().unwrap_or("").split('/') {
                if all.iter().any(|x| x.kind == "ast_kinds" && x.name == k) {
                    add("ast_kinds", k, &mut out);
                }
            }
        }
        "ast_kinds" => {
            for x in all.iter().filter(|x| {
                x.kind == "sigils"
                    && x.body["ast_kind"].as_str().is_some_and(|a| a.split('/').any(|k| k == e.name))
            }) {
                add("sigils", &x.name, &mut out);
            }
        }
        "examples" => {
            for ex in e.body["exercises"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
                for x in all.iter().filter(|x| {
                    matches!(x.kind, "sigils" | "keywords" | "types") && x.name == ex
                }) {
                    add(x.kind, &x.name, &mut out);
                }
            }
        }
        _ => {}
    }
    // The same name elsewhere in the ontology (`net` the keyword and `net`
    // the type, a flag and the method that mirrors it).
    let me = loose(&e.name);
    if !me.is_empty() {
        for x in all.iter().filter(|x| x.kind != e.kind && loose(&x.name) == me) {
            add(x.kind, &x.name, &mut out);
        }
    }
    out.truncate(8);
    out
}

fn finish(mut v: Value) -> Value {
    // `tokens` is part of the text it counts; iterate to a fixed point and
    // report the larger of the two figures if it never settles.
    let mut n = 0usize;
    for _ in 0..6 {
        v["tokens"] = json!(n);
        let c = tokens(&v);
        if c == n {
            return v;
        }
        n = n.max(c);
    }
    v["tokens"] = json!(n);
    v
}

/// The first call: every class with its entry count and a one-line meaning.
pub fn index() -> Value {
    let onto = crate::ontology::build();
    let rows: Vec<Value> = KINDS
        .iter()
        .map(|(k, about)| {
            let n = match &onto["sections"][*k] {
                Value::Array(a) => a.len(),
                Value::Object(m) => m.len(),
                _ => 0,
            };
            json!({ "kind": k, "n": n, "about": about })
        })
        .collect();
    finish(json!({
        "ok": true,
        "use": "then query a name, code or word; whole ontology: 63000 tokens",
        "kinds": rows,
    }))
}

/// Entries matching `term`, best first, whole entries only, within `budget`
/// BPE tokens (the size of the response itself, header included).
pub fn query(term: &str, kind: Option<&str>, budget: usize) -> Value {
    let term = term.trim();
    if term.is_empty() {
        return json!({ "ok": false, "error": "empty term; try the index first" });
    }
    if let Some(k) = kind {
        if !KINDS.iter().any(|(n, _)| *n == k) {
            return json!({
                "ok": false,
                "error": format!("unknown kind {k:?}"),
                "kinds": KINDS.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            });
        }
    }
    let all = load();
    let mut found: Vec<(u8, usize)> = all
        .iter()
        .enumerate()
        .filter(|(_, e)| kind.is_none_or(|k| e.kind == k))
        .filter_map(|(i, e)| tier(e, term).map(|t| (t, i)))
        .collect();
    // Stable by construction: tier, then index (class order, then row order).
    found.sort();
    let total = found.len();

    let shell = |hits: &[Value], truncated: bool| {
        finish(json!({
            "ok": true,
            "query": term,
            "kind": kind,
            "budget": budget,
            "matches": total,
            "returned": hits.len(),
            "omitted": total - hits.len(),
            "truncated": truncated,
            "hits": hits,
        }))
    };

    // Running estimate per hit (cheap), then one exact check at the end: BPE
    // counts of concatenated text can differ by a token or two at the seams.
    let base = tokens(&shell(&[], true)) + 8;
    let mut used = base;
    let mut hits: Vec<Value> = Vec::new();
    for (_, i) in &found {
        let e = &all[*i];
        let hit = json!({
            "kind": e.kind,
            "name": e.name,
            "entry": e.body,
            "related": related(e, &all),
        });
        let t = tokens(&hit) + 1;
        if used + t <= budget {
            used += t;
            hits.push(hit);
        }
    }
    while !hits.is_empty() && tokens(&shell(&hits, true)) > budget {
        hits.pop();
    }
    shell(&hits, hits.len() < total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The agent's questions: names that exist in the ontology and the places
    /// an agent actually starts from.
    const QUESTIONS: &[&str] = &[
        "Vec", "borrow", "Result", "net", "train", "--check", "effect", "Option",
        "match", "contract", "@ens", "matmul", "Attention", "--eval", "rap", "heal",
        "tensor", "backend", "agent", "layer",
        // What an agent meets after an error or in someone else's code.
        "E0382", "E0507", "fetch_add", "UseAfterMove",
    ];

    #[test]
    fn kinds_table_matches_the_ontology() {
        let onto = crate::ontology::build();
        let secs = onto["sections"].as_object().unwrap();
        for (k, _) in KINDS {
            assert!(secs.contains_key(*k), "KINDS names `{k}`, which is not an ontology section");
        }
        for k in secs.keys() {
            assert!(KINDS.iter().any(|(n, _)| n == k), "section `{k}` has no index description");
        }
    }

    #[test]
    fn index_counts_equal_the_ontology_and_fit_800_tokens() {
        let ix = index();
        let onto = crate::ontology::build();
        for row in ix["kinds"].as_array().unwrap() {
            let k = row["kind"].as_str().unwrap();
            let n = onto["counts"][k].as_u64().unwrap_or_else(|| {
                // `abl` and `layer_map` have no entry in `counts`.
                match &onto["sections"][k] {
                    Value::Array(a) => a.len() as u64,
                    Value::Object(m) => m.len() as u64,
                    _ => 0,
                }
            });
            assert_eq!(row["n"].as_u64(), Some(n), "{k}");
        }
        let t = ix["tokens"].as_u64().unwrap() as usize;
        assert_eq!(t, tokens(&ix), "reported tokens must be the real count");
        assert!(t <= 800, "index is {t} tokens");
    }

    #[test]
    fn matching_tiers_and_order() {
        let r = query("matmul", Some("ir_ops"), 600);
        assert_eq!(r["hits"][0]["name"], "matmul");
        assert!(r["hits"][0]["related"].as_array().unwrap().iter().any(|x| x == "op_families:Neural"));
        // `check` finds `--check` by case/dash-insensitive name.
        let r = query("check", Some("cli_flags"), 600);
        assert_eq!(r["hits"][0]["name"], "--check");
        // exact beats case-insensitive beats prefix.
        let r = query("MATMUL", Some("ir_ops"), 600);
        assert_eq!(r["hits"][0]["name"], "matmul");
        assert_eq!(query("net", None, 600), query("net", None, 600), "deterministic");
    }

    #[test]
    fn budget_cuts_whole_entries_and_reports_it() {
        let wide = query("a", None, 100_000);
        let total = wide["matches"].as_u64().unwrap();
        assert_eq!(wide["omitted"], 0);
        assert_eq!(wide["truncated"], false);
        let cut = query("a", None, 300);
        assert_eq!(cut["matches"].as_u64().unwrap(), total);
        assert!(cut["truncated"].as_bool().unwrap());
        assert_eq!(
            cut["returned"].as_u64().unwrap() + cut["omitted"].as_u64().unwrap(),
            total
        );
        assert!(tokens(&cut) <= 300, "{} tokens", tokens(&cut));
        assert_eq!(cut["tokens"].as_u64().unwrap() as usize, tokens(&cut));
        // Every returned entry is whole: it equals the ontology's row.
        let first = &cut["hits"][0];
        let onto = crate::ontology::build();
        let sec = &onto["sections"][first["kind"].as_str().unwrap()];
        assert!(sec.as_array().unwrap().iter().any(|r| *r == first["entry"]));
    }


    /// The first hit for a code, a category name or a builtin is the entry
    /// that is exactly that, in the class an agent would expect.
    #[test]
    fn a_code_a_category_and_a_builtin_find_their_own_entry_first() {
        for (term, kind, name) in [
            ("E0382", "diagnostics", "E0382"),
            ("E0507", "diagnostics", "E0507"),
            ("UseAfterMove", "diagnostics", "E0382"),
            ("MoveOutOfBorrow", "diagnostics", "E0507"),
            ("e0502", "diagnostics", "E0502"),
            ("fetch_add", "builtins", "fetch_add"),
            ("read_line", "builtins", "io.read_line"),
            ("fs.exists", "builtins", "fs.exists"),
            ("unreachable", "builtins", "unreachable"),
        ] {
            let r = query(term, None, DEFAULT_BUDGET);
            assert_eq!(r["hits"][0]["kind"], kind, "`{term}`");
            assert_eq!(r["hits"][0]["name"], name, "`{term}`");
        }
        // The entry carries what an agent needs to act on the error.
        let e = &query("E0382", None, DEFAULT_BUDGET)["hits"][0]["entry"];
        for key in ["category", "summary", "fix", "emitted_by", "gap"] {
            assert!(!e[key].is_null(), "E0382 has no `{key}`");
        }
    }

    /// Terms an agent plausibly meets, from the tables that define them rather
    /// than from a list written here: every diagnostic code and category,
    /// every lexer keyword, every CLI flag, every RAP method, every builtin
    /// the resolver, evaluator or spec names. Each must find an entry of the
    /// class it belongs to by name (or alias), and fit the default budget.
    ///
    /// Not probed, and why: sigils, types, effects, IR ops and layer names
    /// (their sections are the tables, so the probe would be the section read
    /// against itself); the 7 warning classes of diagnostics.mg, which are
    /// not codes (the code they carry is probed).
    #[test]
    fn every_term_an_agent_can_meet_has_an_entry() {
        let mut terms: Vec<(String, &str)> = Vec::new();
        for c in crate::hir::DiagnosticCategory::ALL {
            terms.push((c.code().to_string(), "diagnostics"));
            terms.push((format!("{c:?}"), "diagnostics"));
        }
        let kb = crate::kb::load().expect("kb loads");
        for f in kb.facts("diagnostic_row") {
            if f[0].len() == 5 && f[0].starts_with('E') {
                terms.push((f[0].clone(), "diagnostics"));
            }
        }
        for (k, _) in crate::lexer::KEYWORDS {
            terms.push((k.to_string(), "keywords"));
        }
        let onto = crate::ontology::build();
        for r in onto["sections"]["cli_flags"].as_array().unwrap() {
            terms.push((r["flag"].as_str().unwrap().to_string(), "cli_flags"));
        }
        for r in onto["sections"]["rap_methods"].as_array().unwrap() {
            terms.push((r["method"].as_str().unwrap().to_string(), "rap_methods"));
        }
        for n in crate::resolve::STD_FNS {
            terms.push((n.to_string(), "builtins"));
        }
        for (n, _, _) in crate::resolve::VOCABULARY {
            terms.push((n.to_string(), "vocabulary"));
        }
        for (ns, op, _, _) in crate::eval::CAPABILITY_OPS {
            terms.push((format!("{ns}.{op}"), "builtins"));
            terms.push((op.to_string(), "builtins"));
        }
        for (ns, _) in crate::hir::CAPABILITY_NAMESPACES {
            terms.push((ns.to_string(), "builtins"));
        }
        terms.push(("fetch_add".to_string(), "builtins"));
        assert!(terms.len() > 250, "the probe is {} terms", terms.len());

        let all = load();
        for (term, kind) in &terms {
            let r = query(term, None, DEFAULT_BUDGET);
            assert!(r["matches"].as_u64().unwrap() >= 1, "`{term}` matches nothing");
            assert!(r["returned"].as_u64().unwrap() >= 1, "`{term}`: nothing fits the budget");
            assert!(tokens(&r) <= DEFAULT_BUDGET, "`{term}` answered in {} tokens", tokens(&r));
            assert!(
                all.iter().any(|e| e.kind == *kind && tier(e, term).is_some_and(|t| t <= 2)),
                "`{term}` has no `{kind}` entry by name"
            );
        }
    }
    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert_eq!(query("", None, 600)["ok"], false);
        assert_eq!(query("x", Some("nope"), 600)["ok"], false);
    }

    /// The measurement: answers to realistic questions fit the default budget
    /// and are a small fraction of the whole file.
    #[test]
    fn realistic_questions_fit_the_default_budget() {
        let whole = tokens(&crate::ontology::build());
        let mut sizes = Vec::new();
        for q in QUESTIONS {
            let r = query(q, None, DEFAULT_BUDGET);
            assert_eq!(r["ok"], true, "{q}");
            let t = tokens(&r);
            assert!(t <= DEFAULT_BUDGET, "`{q}` answered in {t} tokens");
            assert!(r["returned"].as_u64().unwrap() >= 1, "`{q}` found nothing");
            sizes.push(t);
        }
        sizes.sort();
        let ix = tokens(&index());
        eprintln!(
            "ontology-query: {} questions, tokens min/median/max = {}/{}/{}; index {}; whole ontology {}",
            sizes.len(), sizes[0], sizes[sizes.len() / 2], sizes[sizes.len() - 1], ix, whole
        );
        assert!(sizes[sizes.len() - 1] * 50 < whole, "an answer is not a small slice of the whole");
    }
}
