//! Plan Phase 4: `kb/rules/contracts.mg` and `kb/rules/grammar.mg` against the
//! compiler. The first probes every cell of the contract grid, so a cell the
//! facts call unchecked is checked to be unchecked. The second gives each
//! grammar constraint a witness; the arena's two are in `arena/src/grammar.rs`.

use mage_prototype::ast::{ItemKind, TensorDim, Type};
use mage_prototype::{kb, lexer, parser, recognise};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const KINDS: [(&str, &str); 3] = [("requires", "@req"), ("ensures", "@ens"), ("invariant", "@inv")];
const PLACES: [&str; 4] = ["function", "spec", "orphan_spec", "struct"];
const STAGES: [&str; 4] = ["types", "contract", "verify", "eval"];

/// Writes `src` to a file of its own and returns its path.
fn write_source(src: &str, ext: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mage-rules-{}-{n}.{ext}", std::process::id()));
    std::fs::write(&path, src).unwrap();
    path
}

/// `(severity, code, message)` of each record `--check --json` prints.
fn check(src: &str, flags: &[&str]) -> Vec<(String, String, String)> {
    let path = write_source(src, "mg");
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

/// The error records of `src`. A control must have none: an error in a
/// control is a fault in the probe, not an answer about the cell.
fn errors(src: &str) -> Vec<(String, String, String)> {
    check(src, &[]).into_iter().filter(|(s, _, _)| s == "error").collect()
}

fn control(src: &str) {
    let errs = errors(src);
    assert!(errs.is_empty(), "probe bug: the control has errors {errs:?}\n{src}");
}

/// `main` run by the real evaluator: (exit succeeded, stdout, stderr).
fn eval_main(src: &str) -> (bool, String, String) {
    let path = write_source(src, "mg");
    let out = Command::new(env!("CARGO_BIN_EXE_mage-parse"))
        .arg("--eval")
        .arg(&path)
        .arg("main")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// The source with `clause` in `place`. Except for `orphan_spec` and `struct`,
/// which have no `add` to attach to, it is an `add` and a `main` that calls
/// it, so a run reaches the clause. Those two get a `main` that calls nothing.
fn placed(clause: &str, place: &str) -> String {
    const ADD: &str = "f add(a: i64, b: i64) -> i64 { a + b }\nf main() -> i64 { add(1, 2) }\n";
    match place {
        "function" => format!("{clause}\n{ADD}"),
        "spec" => format!("sp add {{\n    {clause}\n}}\n{ADD}"),
        "orphan_spec" => format!("sp add {{\n    {clause}\n}}\nf main() -> i64 {{ 3 }}\n"),
        "struct" => format!("{clause}\nS Point {{ x: i64 }}\nf main() -> i64 {{ 3 }}\n"),
        other => panic!("no placement `{other}`"),
    }
}

/// Does this stage check a clause of this kind here? Each probe is a clause
/// only that stage objects to, and a control clause it accepts in the same
/// placement, and is true only if the stage's refusal comes from the first and
/// not the second:
/// - `types`: the clause `1`, which is not a bool: E0308. Control: `a > 0`,
///   except where `a` is not in scope.
/// - `contract`: for `@ens` `old(zz) > 0`, otherwise `result > 0`: E0561.
/// - `verify`: the clause `0b`, a bool literal that is false: E0560. The
///   spelling `false` is not a literal here: it is an unresolved name, and it
///   draws E0425 besides E0560.
/// - `eval`: a clause false only at run time, such as `b < 0`, with `main`
///   calling `add(1, 2)`. Run the real evaluator, and look for its contract
///   failure. `verify` cannot refute it. Control: `b > 0`.
fn checked(tag: &str, kind: &str, place: &str, stage: &str) -> bool {
    let clause = |expr: &str| format!("{tag}({expr})");
    match stage {
        "types" => {
            control(&placed(&clause("a > 0"), place));
            emits(&placed(&clause("1"), place), "E0308")
        }
        "contract" => {
            control(&placed(&clause("a > 0"), place));
            let bad = if kind == "ensures" { "old(zz) > 0" } else { "result > 0" };
            emits(&placed(&clause(bad), place), "E0561")
        }
        "verify" => {
            control(&placed(&clause("a >= 0"), place));
            emits(&placed(&clause("0b"), place), "E0560")
        }
        "eval" => {
            let (ok, out, err) = eval_main(&placed(&clause("b > 0"), place));
            assert!(ok && out == "3", "probe bug: the control did not run to 3: {err}");
            let (ok, _, err) = eval_main(&placed(&clause("b < 0"), place));
            !ok && err.contains(&format!("`{}`", clause("b < 0")))
        }
        other => panic!("no stage `{other}`"),
    }
}

#[test]
fn every_cell_of_the_contract_grid_is_as_stated() {
    let kb = kb::load().expect("the knowledge base loads");
    let stated: BTreeSet<(String, String, String)> = kb
        .facts("clause_checked")
        .into_iter()
        .map(|f| (f[0].clone(), f[1].clone(), f[2].clone()))
        .collect();
    let mut found = BTreeSet::new();
    for (kind, tag) in KINDS {
        for place in PLACES {
            for stage in STAGES {
                if checked(tag, kind, place, stage) {
                    found.insert((kind.to_string(), place.to_string(), stage.to_string()));
                }
            }
        }
    }
    let missing: Vec<_> = stated.difference(&found).collect();
    let unstated: Vec<_> = found.difference(&stated).collect();
    assert!(
        missing.is_empty() && unstated.is_empty(),
        "stated but not checked: {missing:#?}\nchecked but not stated: {unstated:#?}"
    );
}

// ── Grammar constraints ─────────────────────────────────────────────

/// Those with a witness in `arena/src/grammar.rs`.
const ARENA: &[&str] = &["max_depth", "max_nest"];

const WITNESSES: &[(&str, fn() -> bool)] = &[
    ("reserved_item_keyword", reserved_item_keyword),
    ("reserved_pattern", reserved_pattern),
    ("keyword_binding", keyword_binding),
    ("macro_call", macro_call),
    ("unit_letters_only", unit_letters_only),
    ("agent_fields", agent_fields),
    ("swarm_labels", swarm_labels),
    ("evolve_fields", evolve_fields),
    ("kb_entries", kb_entries),
    ("clause_on_other_item", clause_on_other_item),
    ("layout_blocks", layout_blocks),
    ("overflowing_dimension", overflowing_dimension),
];

/// A refusal witness: `bad` is refused with E0001 and a message holding each
/// of `says`, and the close control `ok` is not refused with E0001.
fn refused(bad: &str, says: &[&str], ok: &str) -> bool {
    check(bad, &[]).iter().any(|(_, c, m)| c == "E0001" && says.iter().all(|s| m.contains(s)))
        && absent(ok, "E0001")
}

fn reserved_item_keyword() -> bool {
    ["extern", "unsafe"].iter().all(|&w| {
        refused(&format!("{w} f g() -> i64 {{ 1 }}\n"), &[w, "is reserved"], "f g() -> i64 { 1 }\n")
    })
}

fn reserved_pattern() -> bool {
    refused(
        "f g() -> i64 {\n    swarm_race(1)\n}\n",
        &["swarm_race", "is a reserved word"],
        "f g() -> i64 {\n    1\n}\n",
    )
}

fn keyword_binding() -> bool {
    refused(
        "f x() -> i32 {\n  v f = 3\n  0\n}\n",
        &["is a keyword", "binding name"],
        "f x() -> i32 {\n  v g = 3\n  0\n}\n",
    )
}

fn macro_call() -> bool {
    refused(
        "f g() -> i64 {\n    foo!(1)\n    1\n}\n",
        &["MAGE has no macros"],
        "f g() -> i64 {\n    foo(1)\n    1\n}\n",
    )
}

fn unit_letters_only() -> bool {
    refused(
        "f g(d: f64[m2]) -> f64[m2] { d }\n",
        &["expected a unit such as", "found 'm2'"],
        "f g(d: f64[m]) -> f64[m] { d }\n",
    )
}

fn agent_fields() -> bool {
    refused("agent A { brain: x }\n", &["unknown agent field `brain`"], "agent A { capabilities: [x] }\n")
}

fn swarm_labels() -> bool {
    refused("swarm S { dispatch: x }\n", &["`dispatch` is not a swarm field"], "swarm S { agent: A }\n")
}

fn evolve_fields() -> bool {
    refused("evolve E { zzz: 1 }\n", &["is not an evolve field"], "evolve E { genome: i32 }\n")
}

fn kb_entries() -> bool {
    refused("kb K { zzz: 1 }\n", &["is not a kb entry"], "kb K {\n    fact p(1);\n}\n")
}

/// A gap: a clause above an enum is dropped with no word about it, while the
/// same clause above a function is checked.
fn clause_on_other_item() -> bool {
    let on_enum = "@req(0b)\nE Color { Red, Green }\nf main() -> i64 { 3 }\n";
    check(on_enum, &[]).iter().all(|(s, _, _)| s != "error") && !emits(on_enum, "E0560")
        && emits(&placed("@req(0b)", "function"), "E0560")
}

/// A gap: the parser reads the layout-block source, which the recogniser over
/// `kb/grammar.mg` refuses, and reads the braces version, which both accept.
fn layout_blocks() -> bool {
    let kb = kb::load().expect("the knowledge base loads");
    let g = recognise::Grammar::from_kb(&kb).expect("the grammar builds");
    let layout = "f sign(n: i64) -> i64\n  if n > 0\n    1\n  else\n    2\n";
    let braces = "f sign(n: i64) -> i64 {\n  if n > 0 { 1 } else { 2 }\n}\n";
    let parses = |src: &str| parser::parse(&lexer::lex(src)).is_ok();
    parses(layout)
        && recognise::recognise_source(&g, layout).is_err()
        && parses(braces)
        && recognise::recognise_source(&g, braces).is_ok()
}

/// The dimension of the first parameter's tensor type, as the parser read it.
fn first_dim(src: &str) -> Option<u64> {
    let module = parser::parse(&lexer::lex(src)).ok()?;
    let ItemKind::Function(f) = &module.items.first()?.kind else { return None };
    let Type::Tensor { shape, .. } = &f.params.first()?.ty else { return None };
    match shape.first()? {
        TensorDim::Lit(n) => Some(*n),
        TensorDim::Var(_) => None,
    }
}

/// A gap: a dimension one past `u64::MAX` parses with no error and reads as 0,
/// while a dimension that fits reads as itself.
fn overflowing_dimension() -> bool {
    first_dim("f g(x: tensor[f64, 18446744073709551616]) -> i64 { 1 }\n") == Some(0)
        && first_dim("f g(x: tensor[f64, 7]) -> i64 { 1 }\n") == Some(7)
}

#[test]
fn every_grammar_constraint_has_a_witness_and_every_witness_a_constraint() {
    let kb = kb::load().expect("the knowledge base loads");
    let stated: BTreeSet<String> = kb
        .facts("instance_of")
        .into_iter()
        .filter(|f| f[1] == "GrammarConstraint")
        .map(|f| f[0].clone())
        .collect();
    let mut have: BTreeSet<String> = WITNESSES.iter().map(|(n, _)| n.to_string()).collect();
    have.extend(ARENA.iter().map(|n| n.to_string()));
    assert_eq!(stated, have);
}

#[test]
fn every_grammar_constraint_holds() {
    let failed: Vec<&str> = WITNESSES.iter().filter(|(_, f)| !f()).map(|(n, _)| *n).collect();
    assert!(failed.is_empty(), "no witness: {failed:?}");
}
