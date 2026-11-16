//! `--check` and `--check --json` report ownership errors: E0382 (use after
//! move) and E0502 (borrow conflict), from `src/borrowck.rs`.

use std::process::{Command, Output};

fn run(src: &str, flags: &[&str]) -> Output {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mage-borrowck-{}-{n}.mg", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_mage-parse")).arg("--check").args(flags).arg(&path).output().unwrap();
    let _ = std::fs::remove_file(&path);
    out
}

/// `(code, message)` of each record `--check --json` prints.
fn codes(src: &str) -> Vec<(String, String)> {
    let out = run(src, &["--json"]);
    let text = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("--check --json printed {text:?}: {e}"));
    value["diagnostics"]
        .as_array()
        .expect("a diagnostics array")
        .iter()
        .map(|d| (d["code"].as_str().unwrap().to_string(), d["message"].as_str().unwrap().to_string()))
        .collect()
}

/// `let b = a` moves `a`; returning it afterwards uses it again.
const MOVED: &str = "f dup(a: s) -> s {\n    v b = a\n    a\n}\n";
/// The same function returning the new binding.
const FIXED: &str = "f dup(a: s) -> s {\n    v b = a\n    b\n}\n";

#[test]
fn check_json_reports_use_after_move_as_e0382() {
    let found = codes(MOVED);
    assert!(
        found.iter().any(|(c, m)| c == "E0382" && m.contains("use of moved value: `a`") && m.contains("fn `dup`")),
        "{found:?}"
    );
    assert!(codes(FIXED).iter().all(|(c, _)| c != "E0382"));
}

#[test]
fn check_json_fails_the_verdict_and_names_the_rule() {
    let out = run(MOVED, &["--json"]);
    assert!(!out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["ok"], false);
    assert!(value["verdict"].to_string().contains("E0382"), "{}", value["verdict"]);
}

#[test]
fn the_human_check_prints_the_code_and_exits_nonzero() {
    let out = run(MOVED, &[]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("E0382") && err.contains("use of moved value: `a`"), "{err}");
    assert!(run(FIXED, &[]).status.success());
}

#[test]
fn check_json_reports_a_borrow_conflict_as_e0502() {
    let bad = "f peek(x: &s) { }\nf host(a: s) {\n    v r = &a\n    v w = &!a\n    peek(r)\n}\n";
    let good = "f peek(x: &s) { }\nf host(a: s) {\n    v r = &a\n    peek(r)\n    v w = &!a\n    peek(w)\n}\n";
    assert!(codes(bad).iter().any(|(c, m)| c == "E0502" && m.contains("cannot borrow `a` as mutable")), "{:?}", codes(bad));
    assert!(codes(good).iter().all(|(c, _)| c != "E0502"));
}

/// `(code, line, col)` of each record `--check --json` prints.
fn positions(src: &str) -> Vec<(String, u64, u64)> {
    let out = run(src, &["--json"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["code"].as_str().unwrap().to_string(), d["line"].as_u64().unwrap(), d["col"].as_u64().unwrap()))
        .collect()
}

#[test]
fn check_json_gives_a_use_after_move_the_line_and_column_of_its_statement() {
    // `a` is moved on line 2 and used again on line 4 (inside the `if`).
    let src = "// header\nf dup(a: s, n: i32) -> s {\n    v b = a\n    ? n > 0 {\n        v c = a\n    }\n    b\n}\n";
    assert_eq!(positions(src), vec![("E0382".to_string(), 5, 9)]);
    // The same, with text added above: the position follows it.
    assert_eq!(positions(&format!("\n\n{src}")), vec![("E0382".to_string(), 7, 9)]);
}

#[test]
fn check_json_gives_a_borrow_conflict_the_line_of_the_conflicting_statement() {
    let src = "f peek(x: &s) { }\nf host(a: s) {\n    v r = &a\n    v moved = a\n    peek(r)\n}\n";
    assert_eq!(positions(src), vec![("E0502".to_string(), 4, 5)]);
}

#[test]
fn the_human_check_prints_line_and_column() {
    let out = run(MOVED, &[]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("[E0382] 3:5:error: use of moved value: `a`"), "{err}");
}
