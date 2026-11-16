//! What the incremental compiler promises, one property at a time.
//!
//! The statistics are the evidence: each test edits a source and then asserts
//! on *which queries ran*, not just on the answer (the answer is held to
//! `check_source` separately, here and in `tests/incremental_equivalence.rs`).

use super::disk::DiskCache;
use super::engine::Stats;
use super::mage::*;
use crate::check::check_source;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::path::PathBuf;

const BASE: &str = "\
f add(a: i32, b: i32) -> i32 { a + b }
f double(x: i32) -> i32 { x * 2 }
f triple(x: i32) -> i32 { x * 3 }
f main() -> i32 { add(double(1), triple(2)) }
";

const ITEM_ROWS: [&str; 6] = ["resolve_item", "pass/verify", "pass/logic", "pass/contract", "pass/shape", "pass/borrowck"];

fn run(c: &Compiler, f: File) {
    c.compile(f).expect("compiles");
}

fn executed(s: &Stats, q: &str) -> u64 {
    s.executed(q)
}

/// The keys of the executions (not disk reloads) of `query`.
fn ran(s: &Stats, query: &str) -> Vec<String> {
    s.events.iter().filter(|e| e.query == query && e.why != "disk").map(|e| e.key.clone()).collect()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mage-query-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn persistent(dir: &PathBuf, build: &str) -> Compiler {
    Compiler::new(Config { cache_dir: Some(dir.clone()), build_id: Some(build.into()), ..Config::default() })
}

#[test]
fn an_unchanged_source_is_a_memo_hit() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    let rev = c.revision();
    c.reset_stats();

    assert_eq!(c.set_source("a.mg", BASE), f);
    assert_eq!(c.revision(), rev, "identical text is not a new revision");
    run(&c, f);
    let t = c.stats().total();
    assert_eq!(t.misses + t.reexecutions, 0, "nothing re-ran: {:?}", c.stats().events);
    assert!(t.hits > 0);
}

#[test]
fn editing_one_function_body_reruns_only_that_functions_queries() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    c.reset_stats();

    let edited = BASE.replace("x * 2", "x * 4");
    c.set_source("a.mg", &edited);
    run(&c, f);
    let s = c.stats();

    // Every per-item query ran exactly once: for `double`.
    for row in ITEM_ROWS {
        assert_eq!(executed(&s, row), 1, "{row} should have run once, ran {:?}", ran(&s, row));
        let keys = ran(&s, row);
        assert!(keys.iter().all(|k| k.contains("\"double\"")), "{row} ran for something other than `double`: {keys:?}");
    }
    assert_eq!(executed(&s, "pass/ai"), 0, "`ai` only concerns nets, trains and kbs, and none changed");
    // Whole-module analyses ran once each (see mage.rs for why).
    for row in ["types", "effects", "diff", "mlir"] {
        assert_eq!(executed(&s, row), 1, "{row}");
    }
    // The list of names did not change, so it cut off: no other function's
    // resolution was invalidated by it.
    let names = &s.queries["names"];
    assert_eq!((names.reexecutions, names.early_cutoffs), (1, 1), "{names:?}");
    // The answer is still right.
    let got = c.checked(f).unwrap();
    assert_eq!(format!("{:?}", got.findings), format!("{:?}", check_source(&edited, true, false).findings));
}

const OWNERSHIP: &str = "\
f take(x: s) -> s { x }
f eat(x: s) -> s { take(x) }
f peek(x: s) { }
f moved(a: s) {
take(a)
take(a)
}
f via_callee(a: s) {
eat(a)
peek(a)
}
f other(n: i32) -> i32 { n + 1 }
";

#[test]
fn editing_one_function_body_reruns_the_borrow_check_for_that_item_only() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", OWNERSHIP);
    run(&c, f);
    let base = c.checked(f).unwrap();
    let codes: Vec<&str> = base.findings.iter().map(|x| x.code.as_str()).collect();
    assert_eq!(codes.iter().filter(|c| **c == "E0382").count(), 2, "{:?}", base.findings);
    c.reset_stats();

    // `other`'s body changes; nothing it says about ownership does.
    let edited = OWNERSHIP.replace("n + 1", "n + 2");
    c.set_source("a.mg", &edited);
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "borrowck/summary"), 1, "the summary is rebuilt: any body may have changed");
    assert_eq!(s.queries["borrowck/summary"].early_cutoffs, 1, "{:?}", s.queries["borrowck/summary"]);
    assert_eq!(ran(&s, "pass/borrowck").len(), 1, "{:?}", ran(&s, "pass/borrowck"));
    assert!(ran(&s, "pass/borrowck")[0].contains("\"other\""), "{:?}", ran(&s, "pass/borrowck"));
    assert_eq!(format!("{:?}", c.checked(f).unwrap().findings), format!("{:?}", check_source(&edited, true, false).findings));

    // An edit that changes which parameters `eat` consumes moves the summary,
    // so every check, `via_callee` (which calls it) included, re-runs although its
    // text is the same, and the answer follows: `eat` no longer consumes, so E0382 goes.
    c.reset_stats();
    let edited2 = edited.replace("f eat(x: s) -> s { take(x) }", "f eat(x: s) -> s { \"k\" }");
    c.set_source("a.mg", &edited2);
    run(&c, f);
    let s = c.stats();
    let keys = ran(&s, "pass/borrowck");
    assert!(keys.iter().any(|k| k.contains("\"eat\"")) && keys.iter().any(|k| k.contains("\"via_callee\"")), "{keys:?}");
    // (The summary itself changed, so every item's check re-ran: the price of
    // one summary rather than a dependency per callee.)
    let got = c.checked(f).unwrap();
    assert_eq!(got.findings.iter().filter(|x| x.code == "E0382").count(), 1, "{:?}", got.findings);
    assert_eq!(format!("{:?}", got.findings), format!("{:?}", check_source(&edited2, true, false).findings));
}

#[test]
fn a_whitespace_edit_cuts_off_at_the_parse() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    c.reset_stats();

    // A blank line between two functions shifts every later token's span, so
    // the token stream changes. The AST has no spans (outside contract clauses
    // and kb lines), so the parse comes out identical.
    c.set_source("a.mg", &BASE.replace("f triple", "\n\nf triple"));
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "tokens"), 1);
    assert_eq!(executed(&s, "parsed"), 1);
    assert_eq!(s.queries["parsed"].early_cutoffs, 1, "the parse is the cutoff point");
    for q in ["elided", "items", "names", "resolve", "types", "effects", "diff", "mlir", "diagnostics", "pass/verify", "resolve_item"] {
        assert_eq!(executed(&s, q), 0, "{q} ran after a whitespace-only edit");
    }
    // `checked` re-ran to rebuild its findings from the new tokens (lex errors
    // carry token spans); everything under it was answered from memory.
    assert!(executed(&s, "checked") <= 1);
}

#[test]
fn a_comment_edit_that_moves_no_token_cuts_off_at_the_tokens() {
    let c = Compiler::in_memory();
    let src = format!("{BASE}// version a\n");
    let f = c.set_source("a.mg", &src);
    run(&c, f);
    c.reset_stats();

    // Same length, so no later token's offset changes. The lexer drops
    // comments, so the token stream is identical and the cutoff is at `tokens`.
    c.set_source("a.mg", &src.replace("version a", "version b"));
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "tokens"), 1);
    assert_eq!(s.queries["tokens"].early_cutoffs, 1, "the token stream is the cutoff point");
    assert_eq!(executed(&s, "parsed"), 0);
    let t = s.total();
    assert_eq!(t.misses + t.reexecutions, 1, "only the lexer ran: {:?}", s.events);
}

#[test]
fn contract_clauses_carry_spans_so_a_line_shift_reaches_those_items_only() {
    let src = "@req(x > 0)\nf pos(x: i32) -> i32 { x }\n\nf other() -> i32 { 1 }\n";
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", src);
    run(&c, f);
    c.reset_stats();

    // A comment above the contract moves its clause's span, which is in the
    // AST: `pos` changed, `other` did not.
    c.set_source("a.mg", &format!("// note\n{src}"));
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "parsed"), 1);
    assert_eq!(s.queries["parsed"].early_cutoffs, 0, "the AST really changed");
    assert_eq!(ran(&s, "pass/verify").len(), 1, "{:?}", ran(&s, "pass/verify"));
    assert!(ran(&s, "pass/verify")[0].contains("\"pos\""));
    assert_eq!(executed(&s, "resolve_item"), 1);
    let want = check_source(&format!("// note\n{src}"), true, false);
    assert_eq!(format!("{:?}", c.checked(f).unwrap().findings), format!("{:?}", want.findings));
}

#[test]
fn a_signature_edit_reaches_callers_through_the_whole_module_checker() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    assert!(c.checked(f).unwrap().verdict().accepted(), "the base source is clean");
    c.reset_stats();

    // `double` now takes a `str`. `main` is untouched and now calls it with an
    // integer: its type error is a consequence of an edit to another function.
    let edited = BASE.replace("f double(x: i32) -> i32 { x * 2 }", "f double(x: str) -> i32 { 2 }");
    c.set_source("a.mg", &edited);
    run(&c, f);
    let s = c.stats();
    let got = c.checked(f).unwrap();
    assert!(!got.verdict().accepted(), "the caller's type error must appear: {:?}", got.findings);
    assert_eq!(format!("{:?}", got.findings), format!("{:?}", check_source(&edited, true, false).findings));
    assert_eq!(executed(&s, "types"), 1, "the type checker re-ran");
    // Names did not change, so resolution stayed per item: only `double`.
    assert_eq!(executed(&s, "resolve_item"), 1);
}

#[test]
fn renaming_a_function_changes_the_name_list_and_so_every_resolution() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    c.reset_stats();

    let edited = BASE.replace("f triple", "f thrice").replace("triple(2)", "thrice(2)");
    c.set_source("a.mg", &edited);
    run(&c, f);
    let s = c.stats();
    // `names` changed (no cutoff), and every item's resolution reads it.
    assert_eq!(s.queries["names"].early_cutoffs, 0);
    assert_eq!(executed(&s, "resolve_item"), 4);
    assert_eq!(format!("{:?}", c.checked(f).unwrap().findings), format!("{:?}", check_source(&edited, true, false).findings));
}

#[test]
fn a_new_function_is_the_only_new_item_work() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", BASE);
    run(&c, f);
    c.reset_stats();

    let edited = format!("f fresh() -> i32 {{ 7 }}\n{BASE}");
    c.set_source("a.mg", &edited);
    run(&c, f);
    let s = c.stats();
    // Inserted at the top, so every index shifted; identities are by name,
    // so the four old items are untouched. The new name changes the name list,
    // which re-resolves all five; the item passes are per item.
    assert_eq!(executed(&s, "pass/verify"), 1);
    assert!(ran(&s, "pass/verify")[0].contains("\"fresh\""));
    assert_eq!(executed(&s, "resolve_item"), 5);
}

#[test]
fn items_are_identified_by_name_and_hashed_exactly() {
    let c = Compiler::in_memory();
    let f = c.set_source("a.mg", "f a(x: i32) -> i32 { x + 1 }\nf b(y: i32) -> i32 { y + 1 }\nf a(z: i32) -> i32 { z }\n");
    let items = c.items(f).unwrap();
    let ids: Vec<String> = items.iter().map(|i| i.id.to_string()).collect();
    assert_eq!(ids, ["Function a", "Function b", "Function a#1"]);
    // `a` and `b` differ only in names: the same program, two different items.
    assert_eq!(items[0].canon, items[1].canon, "alpha-equivalent");
    assert_ne!(items[0].hash, items[1].hash, "but not interchangeable: diagnostics quote names");
}

#[test]
fn a_pass_registers_in_one_line_and_is_incremental() {
    fn long_names(item: &crate::ast::Item) -> PassOut {
        let mut out = PassOut::default();
        if let crate::ast::ItemKind::Function(f) = &item.kind
            && f.name.len() > 5
        {
            out.diags.push(Diagnostic::categorized(
                Severity::Warning,
                format!("function `{}` has a long name", f.name),
                DiagnosticCategory::Other,
                None,
            ));
        }
        out
    }
    fn count(m: &crate::ast::Module) -> PassOut {
        PassOut::diags(vec![Diagnostic::categorized(
            Severity::Info,
            format!("{} items", m.items.len()),
            DiagnosticCategory::Other,
            None,
        )])
    }
    let registry = Registry::builtin().item_pass("long-names", "1", long_names).module_pass("count", "1", count);
    let c = Compiler::new(Config { registry, ..Config::default() });
    let src = format!("{BASE}f extraordinarily() -> i32 {{ 1 }}\n");
    let f = c.set_source("a.mg", &src);
    run(&c, f);
    let d = c.diagnostics(f).unwrap();
    let msgs: Vec<&str> = d.all.iter().map(|d| d.message.as_str()).collect();
    assert!(msgs.contains(&"function `extraordinarily` has a long name"), "{msgs:?}");
    assert!(msgs.contains(&"5 items"), "{msgs:?}");
    // Unioned after the built-in passes.
    assert_eq!(msgs.last(), Some(&"5 items"));
    let findings = c.checked(f).unwrap().findings;
    assert!(findings.iter().any(|f| f.message == "5 items" && f.severity == "note"));

    // Editing `double` re-runs the item pass for `double` alone, the module
    // pass once.
    c.reset_stats();
    c.set_source("a.mg", &src.replace("x * 2", "x * 5"));
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "pass/long-names"), 1);
    assert!(ran(&s, "pass/long-names")[0].contains("\"double\""));
    assert_eq!(executed(&s, "module-pass/count"), 1);
    // And a whitespace edit re-runs neither.
    c.reset_stats();
    c.set_source("a.mg", &src.replace("x * 2", "x * 5").replace("f main", "\n\nf main"));
    run(&c, f);
    let s = c.stats();
    assert_eq!(executed(&s, "pass/long-names") + executed(&s, "module-pass/count"), 0);
}

#[test]
fn a_fresh_engine_over_the_same_directory_hits_the_cache() {
    let dir = tmp("warm");
    let first = persistent(&dir, "t1");
    let f = first.set_source("a.mg", BASE);
    run(&first, f);
    let t = first.stats().total();
    assert!(t.disk_stores > 0 && t.unpersistable == 0, "{t:?}");
    drop(first); // dropping flushes the run's answers to a segment

    let second = persistent(&dir, "t1");
    let f = second.set_source("a.mg", BASE);
    run(&second, f);
    let s = second.stats();
    for (q, qs) in &s.queries {
        if ["source", "settings", "item", "item_hash"].contains(q) {
            continue;
        }
        assert_eq!(qs.misses + qs.reexecutions, 0, "`{q}` ran on a warm start: {qs:?}");
    }
    assert!(s.total().disk_hits > 0);
    assert_eq!(
        format!("{:?}", second.checked(f).unwrap().findings),
        format!("{:?}", check_source(BASE, true, false).findings)
    );

    // Another build id is another address space: nothing is reused.
    let other = persistent(&dir, "t2");
    let f = other.set_source("a.mg", BASE);
    run(&other, f);
    assert_eq!(other.stats().total().disk_hits, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_edit_in_a_new_process_re_runs_one_function_and_loads_the_rest() {
    let dir = tmp("edit");
    let first = persistent(&dir, "t1");
    let f = first.set_source("a.mg", BASE);
    run(&first, f);
    drop(first); // flushes to a segment

    // A new process, an edited file.
    let second = persistent(&dir, "t1");
    let f = second.set_source("a.mg", &BASE.replace("x * 2", "x * 4"));
    run(&second, f);
    let s = second.stats();
    for row in ITEM_ROWS {
        assert_eq!(executed(&s, row), 1, "{row}: {:?}", ran(&s, row));
        assert_eq!(s.queries[row].disk_hits, 3, "{row} should load the other three functions from disk");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_whitespace_edit_in_a_new_process_still_hits_for_every_analysis() {
    let dir = tmp("ws");
    let first = persistent(&dir, "t1");
    let f = first.set_source("a.mg", BASE);
    run(&first, f);
    drop(first); // flushes to a segment

    // Tokens and the parse are keyed on the source text, so they re-run; the
    // analyses are keyed on the AST and the items, which did not change.
    let second = persistent(&dir, "t1");
    let f = second.set_source("a.mg", &BASE.replace("f triple", "\n\nf triple"));
    run(&second, f);
    let s = second.stats();
    for q in ["types", "effects", "mlir", "resolve", "diagnostics", "pass/verify"] {
        assert_eq!(executed(&s, q), 0, "{q} re-ran for a whitespace edit in a new process");
    }
    assert!(s.queries["types"].disk_hits >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_corrupted_entry_is_recomputed_not_trusted() {
    let dir = tmp("corrupt");
    let first = persistent(&dir, "t1");
    let f = first.set_source("a.mg", BASE);
    run(&first, f);
    drop(first); // dropping flushes the run's answers to a segment

    // Damage every record of the final diagnostics, and forge one of the types
    // record's payloads while leaving its integrity hash alone.
    let mut damaged = 0;
    for seg in DiskCache::open(&dir).unwrap().segment_files() {
        let text = std::fs::read_to_string(&seg).unwrap();
        let mut out = String::new();
        for line in text.lines() {
            if line.contains("\"query\":\"checked\"") {
                out.push_str(&line[..line.len() / 2]);
                damaged += 1;
            } else if line.contains("\"query\":\"types\"") {
                out.push_str(&line.replacen("\\\"diags\\\":[]", "\\\"diags\\\":[1]", 1));
                damaged += 1;
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        std::fs::write(&seg, out).unwrap();
    }
    assert_eq!(damaged, 2);

    let second = persistent(&dir, "t1");
    let f = second.set_source("a.mg", BASE);
    run(&second, f);
    let s = second.stats();
    assert_eq!(s.queries["checked"].disk_corrupt, 1);
    assert_eq!(executed(&s, "checked"), 1, "recomputed");
    assert_eq!(s.queries["types"].disk_corrupt, 1);
    assert_eq!(executed(&s, "types"), 1, "recomputed");
    assert_eq!(
        format!("{:?}", second.checked(f).unwrap().findings),
        format!("{:?}", check_source(BASE, true, false).findings)
    );
    drop(second);
    // And the record was rewritten (a newer segment shadows the damaged
    // line): a third engine is clean.
    let third = persistent(&dir, "t1");
    let f = third.set_source("a.mg", BASE);
    run(&third, f);
    assert_eq!(third.stats().total().disk_corrupt, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_ast_cache_keeps_what_serde_drops() {
    // `line` on a kb fact and rule is `#[serde(skip)]`. If the cache lost it,
    // a warm run would print a different diagnostic line than a cold one.
    let src = "kb K {\n    fact edge(1);\n    fact edge(2);\n    rule path(x: i32) { edge(x) }\n}\nf main() -> i32 { 1 }\n";
    let dir = tmp("kb");
    let first = persistent(&dir, "t1");
    let f = first.set_source("a.mg", src);
    run(&first, f);
    drop(first); // flushes to a segment
    let want = format!("{:?}", crate::parser::parse(&crate::lexer::lex(src)).unwrap());

    let second = persistent(&dir, "t1");
    let f = second.set_source("a.mg", src);
    assert_eq!(format!("{:?}", second.parsed(f).unwrap().module.as_ref().unwrap()), want);
    assert_eq!(second.stats().queries["parsed"].disk_hits, 1, "decoded from disk, not re-parsed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_parse_error_is_an_answer_and_is_cached() {
    let c = Compiler::in_memory();
    let f = c.set_source("bad.mg", "f broken( -> i32 {\n");
    let r = c.compile(f).unwrap();
    assert!(r.parse_error.is_some());
    let got = c.checked(f).unwrap();
    assert_eq!(format!("{:?}", got.findings), format!("{:?}", check_source("f broken( -> i32 {\n", true, false).findings));
    assert!(!got.verdict().accepted());
}
