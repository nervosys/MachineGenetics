//! Plan Phase 4: every diagnostic the compiler produces is a fact in
//! `kb/rules/diagnostics.mg`, and this file holds the code to those facts:
//!
//! - each `tested_by` fact names a witness here and each witness has a fact. A
//!   witness runs the real compiler on a small source, and is true only if the
//!   diagnostic comes out and a corrected source does not produce it;
//! - each `DiagnosticCategory` has its code stated, and each stated category
//!   exists;
//! - each module that names a category in production code has that stated, as
//!   an `enforced_by` on the code or on a warning class carrying it;
//! - each quoted code in the production source is a fact, or is on `TABLE_ONLY`.

use mage_prototype::hir::DiagnosticCategory;
use mage_prototype::{abl_bridge, kb, lexer, parser};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `(severity, code, message)` of each record `--check --json` prints.
fn check(src: &str, flags: &[&str]) -> Vec<(String, String, String)> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mage-diag-{}-{n}.mg", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mage-parse"))
        .arg("--check")
        .arg("--json")
        .args(flags)
        .arg(&path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    let text = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|e| panic!("--check --json printed {text:?}: {e}"));
    records(&value)
}

/// `run_check_json` prints one object: `{ok, file, error_count, diagnostics,
/// effects}`. Each entry of `diagnostics` is `{severity, line, col, code,
/// category, message, fix}`.
fn records(value: &serde_json::Value) -> Vec<(String, String, String)> {
    let field = |d: &serde_json::Value, k: &str| d[k].as_str().unwrap().to_string();
    value["diagnostics"]
        .as_array()
        .expect("--check --json prints a diagnostics array")
        .iter()
        .map(|d| (field(d, "severity"), field(d, "code"), field(d, "message")))
        .collect()
}

fn emits(src: &str, code: &str) -> bool {
    check(src, &[]).iter().any(|(_, c, _)| c == code)
}

/// A control: it reached the checks, so the code's absence means something.
fn absent(src: &str, code: &str) -> bool {
    let out = check(src, &[]);
    out.iter().all(|(_, c, _)| c != code && (c != "E0001" || code == "E0001"))
}

// ── Witnesses ───────────────────────────────────────────────────────

const WITNESSES: &[(&str, &str, fn() -> bool)] = &[
    ("E0000", "lex_error", lex_error),
    ("E0001", "syntax_error", syntax_error),
    ("E0382", "use_after_move_classified", use_after_move_classified),
    ("E0502", "borrow_conflict_classified", borrow_conflict_classified),
    ("E0308", "type_mismatch", type_mismatch),
    ("E0412", "unresolved_type", unresolved_type),
    ("E0425", "unresolved_name", unresolved_name),
    ("E0428", "duplicate_definition", duplicate_definition),
    ("E0550", "undeclared_effect", undeclared_effect),
    ("E0551", "role_violation", role_violation),
    ("E0552", "core_violation", core_violation),
    ("E0553", "param_domain", param_domain),
    ("E0554", "deprecated", deprecated),
    ("E0560", "spec_violation", spec_violation),
    ("E0561", "contract_scope", contract_scope),
    ("E0562", "unit_mismatch", unit_mismatch),
    ("E0570", "kb_rule", kb_rule),
    ("E0710", "net_shape", net_shape),
    ("E9999", "grad_unchecked", grad_unchecked),
    ("no_derivative_rule", "no_derivative_rule", no_derivative_rule),
    ("param_no_gradient", "param_no_gradient", param_no_gradient),
    ("grad_unchecked", "grad_unchecked", grad_unchecked),
    ("bound_discarded", "bound_discarded", bound_discarded),
    ("unknown_layer", "unknown_layer", unknown_layer),
    ("kb_fact_left_out", "kb_fact_left_out", kb_fact_left_out),
    ("kb_rule_left_out", "kb_rule_left_out", kb_rule_left_out),
];

/// A warning, by code and by message text: several warnings share a code.
fn warns(src: &str, code: &str, message: &str) -> bool {
    check(src, &[]).iter().any(|(s, c, m)| s == "warning" && c == code && m.contains(message))
}

/// The category code `heal` gives an uncategorised diagnostic with this text.
fn healed_code(message: &str) -> Option<String> {
    let graph = &mage_prototype::heal::heal_to_graphs(&[mage_prototype::hir::Diagnostic::error(message)])[0];
    graph.root.category.map(|c| c.code().to_string())
}

/// The lowering's warnings for a source, which are plain strings.
fn lowering_warnings(src: &str) -> Vec<String> {
    let module = parser::parse(&lexer::lex(src)).unwrap_or_else(|e| panic!("does not parse: {e:?}\n{src}"));
    abl_bridge::lower_module(&module).diagnostics
}

fn lex_error() -> bool {
    emits("f g() -> i32 { 1 } `", "E0000") && absent("f g() -> i32 { 1 }", "E0000")
}

fn syntax_error() -> bool {
    emits("f g( -> i32 { 1 }", "E0001") && absent("f g() -> i32 { 1 }", "E0001")
}

fn use_after_move_classified() -> bool {
    healed_code("use of moved value `x`").as_deref() == Some("E0382")
        && healed_code("value of `x` is still valid").as_deref() != Some("E0382")
}

fn borrow_conflict_classified() -> bool {
    healed_code("cannot borrow `x` as mutable more than once").as_deref() == Some("E0502")
        && healed_code("value of `x` is still valid").as_deref() != Some("E0502")
}

fn type_mismatch() -> bool {
    emits("f bad() -> i32 { 1b }", "E0308") && absent("f ok() -> i32 { 1 }", "E0308")
}

fn unresolved_type() -> bool {
    emits("f bad(x: f64[ft]) -> f64[ft] { x }", "E0412")
        && absent("f good(x: f64[m]) -> f64[m] { x }", "E0412")
}

fn unresolved_name() -> bool {
    emits("f g() -> i32 { zz }", "E0425") && absent("f g() -> i32 { 1 }", "E0425")
}

fn duplicate_definition() -> bool {
    emits("f g() -> i32 { 1 }\nf g() -> i32 { 2 }", "E0428")
        && absent("f g() -> i32 { 1 }\nf h() -> i32 { 2 }", "E0428")
}

fn undeclared_effect() -> bool {
    emits("f x() / io { connect(); }\n", "E0550") && absent("f x() / net { connect(); }\n", "E0550")
}

fn role_violation() -> bool {
    emits("@role(candidate)\nf gen(s: usize) -> [usize] { heldout.read(\"t\") }", "E0551")
        && absent("@role(evaluator)\nf gen(s: usize) -> [usize] / heldout { heldout.read(\"t\") }", "E0551")
}

/// `--core` is what enables E0552; the control is the same source without the
/// domain construct.
fn core_violation() -> bool {
    let bad = "@role(candidate)\nf g() -> i64 { 1 }\nagent A { }\n";
    let good = "@role(candidate)\nf g() -> i64 { 1 }\n";
    check(bad, &["--core"]).iter().any(|(_, c, _)| c == "E0552")
        && !check(good, &["--core"]).iter().any(|(_, c, _)| c == "E0552")
}

fn param_domain() -> bool {
    emits("@param(0.0)\nconst LR: f64 = 0.5;", "E0553")
        && absent("@param(0.0, 1.0)\nconst LR: f64 = 0.5;", "E0553")
}

fn deprecated() -> bool {
    emits("evolve Opt { genome: i32 }", "E0554") && absent("f opt() -> i32 { 1 }", "E0554")
}

/// A refuted contract: `0b` is false on every execution.
fn spec_violation() -> bool {
    emits("@req(false)\nf add(a: i64, b: i64) -> i64 { a + b }\n", "E0560")
        && absent("@req(a >= 0)\nf add(a: i64, b: i64) -> i64 { a + b }\n", "E0560")
}

fn contract_scope() -> bool {
    emits("sp add {\n    @req(result > 0)\n}\nf add(a: i64, b: i64) -> i64 { a + b }\n", "E0561")
        && absent("sp add {\n    @req(a > 0)\n}\nf add(a: i64, b: i64) -> i64 { a + b }\n", "E0561")
}

fn unit_mismatch() -> bool {
    emits("f bad(d: f64[m], t: f64[s]) -> f64[m] { d + t }", "E0562")
        && absent("f good(d: f64[m], t: f64[m]) -> f64[m] { d + t }", "E0562")
}

fn kb_rule() -> bool {
    emits("kb K {\n    fact p(a);\n    rule r(x: i32) { x }\n}\n", "E0570")
        && absent("kb K {\n    fact p(a);\n    rule r(x: i32) { p(x) }\n}\n", "E0570")
}

fn net_shape() -> bool {
    emits("net Chain { layer a: Linear(256, 512); layer b: Linear(256, 128); }", "E0710")
        && absent("net Chain { layer a: Linear(256, 512); layer b: Linear(512, 128); }", "E0710")
}

/// A grad of an expression `grad` does not analyse is a warning, and a
/// product it does analyse is not.
fn grad_unchecked() -> bool {
    warns("f p(w: f64) -> f64 { grad(w |> abs, w) }", "E9999", "not analysed")
        && !warns("f p(w: f64) -> f64 { grad(w * w, w) }", "E9999", "not analysed")
}

/// `backward` runs on the `train` path in main.rs, which `--check` does not reach.
fn no_derivative_rule() -> bool {
    use mage_prototype::autograd::{backward, GradOp, GradTape};
    let warned = |op: GradOp| {
        let mut tape = GradTape::new();
        let x = tape.param("x");
        let c = tape.unary(op, x);
        let loss = tape.unary(GradOp::Sum, c);
        backward(&tape, loss, &["x".into()])
            .diagnostics
            .iter()
            .any(|d| d.message.contains("no derivative rule for call `zorp`"))
    };
    warned(GradOp::Call("zorp".into())) && !warned(GradOp::ReLU)
}

/// Reached through `backward` only, as `tape_no_gradient_warning` does.
fn param_no_gradient() -> bool {
    use mage_prototype::autograd::{backward, GradOp, GradTape};
    let warned = |connected: bool| {
        let mut tape = GradTape::new();
        let x = tape.param("x");
        let c = if connected { x } else { tape.constant() };
        let loss = tape.unary(GradOp::Sum, c);
        backward(&tape, loss, &["x".into()])
            .diagnostics
            .iter()
            .any(|d| d.message.contains("parameter `x` has no gradient"))
    };
    warned(false) && !warned(true)
}

fn bound_discarded() -> bool {
    warns("f describe[T](v: T) -> s ~> T: TotallyMadeUpTrait { \"described\" }", "E9999", "`T: TotallyMadeUpTrait`")
        && !warns("f describe[T](v: T) -> s { \"described\" }", "E9999", "is parsed and then discarded")
}

fn unknown_layer() -> bool {
    lowering_warnings("net N {\n    layer a1: Zorp(4);\n    forward { a1 }\n}\n")
        .iter()
        .any(|d| d.contains("unknown layer type `Zorp`"))
        && !lowering_warnings("net N {\n    layer a1: Linear(4);\n    forward { a1 }\n}\n")
            .iter()
            .any(|d| d.contains("unknown layer type"))
}

fn kb_fact_left_out() -> bool {
    lowering_warnings("kb K {\n    fact p(1);\n    fact q(1 + 2);\n}\n")
        .iter()
        .any(|d| d.contains("fact `q` left out"))
        && !lowering_warnings("kb K {\n    fact p(1);\n    fact q(3);\n}\n")
            .iter()
            .any(|d| d.contains("left out"))
}

fn kb_rule_left_out() -> bool {
    lowering_warnings("kb K {\n    fact p(1);\n    rule r(x: i32) { p(x) && x == 1 }\n}\n")
        .iter()
        .any(|d| d.contains("rule `r` left out"))
        && !lowering_warnings("kb K {\n    fact p(1);\n    rule r(x: i32) { p(x) }\n}\n")
            .iter()
            .any(|d| d.contains("left out"))
}

// ── The tests ───────────────────────────────────────────────────────

#[test]
fn every_tested_by_has_a_witness_and_every_witness_a_tested_by() {
    let kb = kb::load().expect("the knowledge base loads");
    let stated: BTreeSet<(String, String)> =
        kb.facts("tested_by").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
    let have: BTreeSet<(String, String)> =
        WITNESSES.iter().map(|(i, w, _)| (i.to_string(), w.to_string())).collect();
    assert_eq!(stated, have);
}

#[test]
fn every_diagnostic_happens() {
    let failed: Vec<&str> = WITNESSES.iter().filter(|(_, _, f)| !f()).map(|(i, _, _)| *i).collect();
    assert!(failed.is_empty(), "no witness: {failed:?}");
}

#[test]
fn every_category_is_stated() {
    let source = include_str!("../src/hir.rs").replace("\r\n", "\n");
    let start = source.find("pub enum DiagnosticCategory {").unwrap();
    let body = &source[start..start + source[start..].find("\n}").unwrap()];
    let declared: BTreeSet<String> = body
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && l.ends_with(','))
        .map(|l| l.trim_end_matches(',').to_string())
        .collect();
    let all: BTreeSet<String> = DiagnosticCategory::ALL.iter().map(|c| format!("{c:?}")).collect();
    assert_eq!(declared, all, "DiagnosticCategory::ALL is not every variant");

    let kb = kb::load().expect("the knowledge base loads");
    let stated: BTreeSet<(String, String)> = kb
        .facts("diagnostic_row")
        .into_iter()
        .filter(|f| f[0].starts_with('E') && all.contains(&f[2]))
        .map(|f| (f[0].clone(), f[2].clone()))
        .collect();
    let have: BTreeSet<(String, String)> =
        DiagnosticCategory::ALL.iter().map(|c| (c.code().to_string(), format!("{c:?}"))).collect();
    assert_eq!(stated, have);
}

fn production(path: &Path) -> String {
    let source = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let end = source.find("#[cfg(test)]").unwrap_or(source.len());
    source[..end].to_string()
}

/// Every module that emits a category. `heal`, `main`, `rap`, `recover` and
/// `nl_engine` are not here: they relay parser errors, or classify messages.
const EMITTERS: &[&str] = &[
    "autograd", "core_subset", "differentiable", "effects", "logic", "params",
    "resolve", "shape", "types", "units", "verify",
];

#[test]
fn every_emit_site_is_stated() {
    let kb = kb::load().expect("the knowledge base loads");
    let carries: BTreeMap<String, String> =
        kb.facts("warning_carries").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
    let stated: BTreeSet<(String, String)> = kb
        .facts("enforced_by")
        .into_iter()
        .filter(|f| EMITTERS.contains(&f[1].as_str()))
        .map(|f| (carries.get(&f[0]).unwrap_or(&f[0]).clone(), f[1].clone()))
        .collect();
    let mut found = BTreeSet::new();
    for module in EMITTERS {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(format!("{module}.rs"));
        let source = production(&path);
        for c in DiagnosticCategory::ALL {
            if source.contains(&format!("DiagnosticCategory::{c:?}")) {
                found.insert((c.code().to_string(), module.to_string()));
            }
        }
    }
    assert_eq!(stated, found);
}

/// Codes in the source that nothing emits: `aci.rs`'s pattern table, which no
/// production path reaches. The list is exact.
const TABLE_ONLY: &[&str] = &["E0277", "E0505", "E0599"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_code_in_the_source_is_stated() {
    let mut files = Vec::new();
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut found = BTreeSet::new();
    for path in files {
        for line in production(&path).lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for quoted in line.split('"').skip(1).step_by(2) {
                if quoted.len() == 5
                    && quoted.starts_with('E')
                    && quoted[1..].chars().all(|c| c.is_ascii_digit())
                {
                    found.insert(quoted.to_string());
                }
            }
        }
    }
    let kb = kb::load().expect("the knowledge base loads");
    let mut expected: BTreeSet<String> = kb
        .facts("diagnostic_row")
        .into_iter()
        .map(|f| f[0].clone())
        .filter(|i| i.starts_with('E'))
        .collect();
    expected.extend(TABLE_ONLY.iter().map(|c| c.to_string()));
    assert_eq!(found, expected);
}
