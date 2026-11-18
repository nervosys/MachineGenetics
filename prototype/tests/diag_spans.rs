//! Diagnostics from the type checker, the resolver and the effect checker point
//! at the statement that caused them; those no pass can place finer point at the
//! first line of the top-level item they belong to. `--check --json` reports
//! `line`/`col` (0/0 for none) and the human `--check` prints `line:col:`.

use std::process::{Command, Output};

fn run(src: &str, flags: &[&str]) -> Output {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mage-diagspans-{}-{n}.mg", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mage-parse")).env_remove("MAGE_CACHE_DIR").arg("--check").args(flags).arg(&path).output().unwrap();
    let _ = std::fs::remove_file(&path);
    out
}

/// `(line, col)` of each `code` record `--check --json` prints.
fn at(src: &str, code: &str) -> Vec<(u64, u64)> {
    let out = run(src, &["--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("--check --json printed {:?}: {e}", String::from_utf8_lossy(&out.stdout)));
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .map(|d| (d["line"].as_u64().unwrap(), d["col"].as_u64().unwrap()))
        .collect()
}

#[test]
fn a_type_mismatch_points_at_the_statement() {
    // The bad `let` is the fourth line of the second function.
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    v a = 1\n    v b: i32 = \"s\"\n    a\n}\n";
    assert_eq!(at(src, "E0308"), vec![(5, 5)]);
    // A wrong tail expression is reported at the tail, not at the function.
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    v a = 1\n    \"tail\"\n}\n";
    assert_eq!(at(src, "E0308"), vec![(5, 5)]);
    // Inside a nested block, the statement in that block.
    let src = "f g(n: i32) -> i32 {\n    if n > 0 {\n        v z: i32 = \"s\"\n        z\n    } else {\n        0\n    }\n}\n";
    assert_eq!(at(src, "E0308"), vec![(3, 9)]);
}

#[test]
fn an_unresolved_name_points_at_the_statement() {
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    v a = 1\n    v b = zz\n    a\n}\n";
    assert_eq!(at(src, "E0425"), vec![(5, 5)]);
    // In the tail expression.
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    v a = 1\n    a + yy\n}\n";
    assert_eq!(at(src, "E0425"), vec![(5, 5)]);

    // A call the evaluator cannot run (E0714) is placed the same way, on every path:
    // line 3 of the first function, a nested block, and a function further down.
    let src = "f main() -> i64 {\n    val xs = [1, 2]\n    xs.zork(1)\n}\n\nf second(n: i64) -> i64 {\n    val ys = [1]\n    if n > 0 {\n        val q = 1\n        val r = ys.blorp(q)\n        r\n    } else {\n        0\n    }\n}\n\nf third() -> i64 {\n    val zs = [3]\n    zs.quux()\n}\n";
    let want = vec![(3, 5), (10, 9), (19, 5)];
    assert_eq!(at(src, "E0714"), want);
    let positions = |text: &str| -> Vec<String> {
        text.lines().filter_map(|l| l.split("[E0714] ").nth(1)?.split(":warning").next().map(str::to_string)).collect()
    };
    let text = String::from_utf8_lossy(&run(src, &[]).stderr).to_string();
    assert_eq!(positions(&text), vec!["3:5", "10:9", "19:5"], "{text}");
    // Through the query engine (`--cache-dir`), both forms.
    let dir = std::env::temp_dir().join(format!("mage-diagspans-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    let out = run(src, &["--json", "--cache-dir", &d]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let cached: Vec<(u64, u64)> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["code"] == "E0714")
        .map(|x| (x["line"].as_u64().unwrap(), x["col"].as_u64().unwrap()))
        .collect();
    assert_eq!(cached, want);
    let text = String::from_utf8_lossy(&run(src, &["--cache-dir", &d]).stderr).to_string();
    assert_eq!(positions(&text), vec!["3:5", "10:9", "19:5"], "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_duplicate_definition_points_at_the_second_one() {
    // Top level: the item that defines the name again.
    let src = "f g() -> i32 { 1 }\n\nf h() -> i32 { 1 }\nf g() -> i32 { 2 }\n";
    assert_eq!(at(src, "E0428"), vec![(4, 1)]);
    // In a body: the statement that defines it again.
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    f inner() -> i32 { 1 }\n    f inner() -> i32 { 2 }\n    inner()\n}\n";
    assert_eq!(at(src, "E0428"), vec![(5, 5)]);
}

#[test]
fn an_undeclared_effect_points_at_the_statement_that_performs_it() {
    let src = "f first() -> i32 { 1 }\n\nf second() / io {\n    v a = 1\n    net.connect(\"h\");\n    a\n}\n";
    assert_eq!(at(src, "E0550"), vec![(5, 5)]);
    // Through a call: the statement that calls the effectful function.
    let src = "f dial() / net { net.connect(\"h\"); }\n\npub f second() -> i32 {\n    v a = 1\n    dial();\n    a\n}\n";
    assert_eq!(at(src, "E0550"), vec![(5, 5)]);
}

#[test]
fn a_diagnostic_with_no_finer_position_reports_its_items_first_line() {
    // Units: the pass reports the mismatch, not where in the body it was.
    let src = "f first() -> i32 { 1 }\n\nf bad(d: f64[m], t: f64[s]) -> f64[m] { d + t }\n";
    assert_eq!(at(src, "E0562"), vec![(3, 1)]);
    // A `kb` rule the logic engine cannot evaluate.
    let src = "f first() -> i32 { 1 }\n\nkb K {\n    fact p(a);\n    rule r(x: i32) { x }\n}\n";
    assert_eq!(at(src, "E0570"), vec![(3, 1)]);
    // A `train` that names a net the module lacks.
    let src = "f first() -> i32 { 1 }

net Nowhere { layer fc: Linear(3, 1); forward { fc } }
train Fit { net: Nowhear; optimizer: SGD(0.1); loss: MSE; epochs: 1; }
";
    assert_eq!(at(src, "E0713"), vec![(4, 1)]);
}

#[test]
fn the_human_check_prints_the_position() {
    let src = "f first() -> i32 { 1 }\n\nf second() -> i32 {\n    v a = 1\n    v b = zz\n    a\n}\n";
    let err = String::from_utf8_lossy(&run(src, &[]).stderr).into_owned();
    assert!(err.contains("5:5:") && err.contains("zz"), "{err}");
}

#[test]
fn a_diagnostic_with_no_item_to_blame_stays_unplaced() {
    // A syntax error has its own position; a clean file has no diagnostics.
    assert!(at("f g() -> i32 { 1 }\n", "E0308").is_empty());
}
