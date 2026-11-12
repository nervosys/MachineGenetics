//! `mage-parse --heal`: the file is modified only under `--write`, and the
//! exit status says whether the healed source is clean and every run passes.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_mage-parse");

const OFF_BY_ONE: &str = "f adult(age: i64) -> bool {\n    if age > 18 { 1b } else { 0b }\n}\n\n\
                          @t\nf t_eighteen() {\n    assert(adult(18), \"18 is adult\")\n}\n";

fn scratch(name: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mage-heal-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prog.mg");
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn heal_without_write_leaves_the_file_untouched() {
    let path = scratch("nowrite", OFF_BY_ONE);
    let out = Command::new(BIN).arg("--heal").arg(&path).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("+    if age >= 18"), "{stdout}");
    assert!(stdout.contains("not modified; pass --write"), "{stdout}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), OFF_BY_ONE);
}

#[test]
fn heal_with_write_writes_the_healed_source() {
    let path = scratch("write", OFF_BY_ONE);
    let out = Command::new(BIN).arg("--heal").arg(&path).arg("--write").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.contains("if age >= 18"), "{now}");
}

#[test]
fn heal_exits_non_zero_when_it_cannot_heal() {
    let src = "f seven() -> i64 {\n    100\n}\n\n@t\nf t_seven() {\n    assert(seven() == 7, \"seven\")\n}\n";
    let path = scratch("unrepairable", src);
    let out = Command::new(BIN).arg("--heal").arg(&path).arg("--json").output().unwrap();
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("--json prints JSON");
    assert_eq!(v["ok"], false);
    assert_eq!(v["tests_passing"], 0);
    assert_eq!(v["written"], false);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), src);
}

/// The text report carries the debugger's evidence for the repair: the
/// site, its Ochiai score and run counts, the values the comparison saw, and
/// why the candidate was tried first.
#[test]
fn heal_text_reports_the_debugger_evidence() {
    let path = scratch("evidence", OFF_BY_ONE);
    let out = Command::new(BIN).arg("--heal").arg(&path).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    for line in [
        "    adult/binary#0 `>`: Ochiai 1.00; failing 1 / passing 0",
        "      saw t_eighteen: 18 > 18 -> false",
        "  repair: `>` -> `>=` in `adult` at 2:12 [changes semantics]",
        "    site adult/binary#0: Ochiai 1.00; executed by 1 failing and 0 passing run(s)",
        "    observed t_eighteen: 18 > 18 -> false",
    ] {
        assert!(stdout.contains(line), "missing {line:?} in:\n{stdout}");
    }
    assert!(stdout.contains("    tried first because: the comparison saw equal operands"), "{stdout}");
}

/// `range(n)` with `n: i64`: the checker names argument 1 of `range`, and
/// the cast lands there, in the text and the JSON report.
#[test]
fn heal_casts_the_range_argument_the_checker_names() {
    let src = "f count_to(n: i64) -> usize {\n    len(range(n))\n}\n\n\
               @t\nf t_count() {\n    assert(count_to(3) == 3, \"three\")\n}\n";
    let path = scratch("range", src);
    let out = Command::new(BIN).arg("--heal").arg(&path).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains(
            "    compiler evidence: in `count_to`, argument 1 of `range` `n` at 2:15: expected Usize, found I64"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("+    len(range(n as usize))"), "{stdout}");

    let out = Command::new(BIN).arg("--heal").arg(&path).arg("--json").output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("--json prints JSON");
    let ev = &v["compile"]["steps"][0]["evidence"];
    assert_eq!(ev["site"]["kind"], "call_arg", "{v:#}");
    assert_eq!(ev["site"]["callee"], "range");
    assert_eq!((ev["line"].as_u64(), ev["col"].as_u64()), (Some(2), Some(15)));
    assert_eq!(v["ok"], true);
}

/// `--compact` prints the same `--json` report on one line: it parses to the
/// same value as the default pretty form, and the exit status is unchanged.
#[test]
fn heal_compact_json_is_one_line_and_parses_equal_to_pretty() {
    let path = scratch("compact", OFF_BY_ONE);
    let pretty = Command::new(BIN).arg("--heal").arg(&path).arg("--json").output().unwrap();
    let compact =
        Command::new(BIN).arg("--heal").arg(&path).arg("--json").arg("--compact").output().unwrap();
    let p = String::from_utf8_lossy(&pretty.stdout).into_owned();
    let c = String::from_utf8_lossy(&compact.stdout).into_owned();
    assert_eq!(pretty.status.code(), compact.status.code());
    assert!(p.trim_end().lines().count() > 1, "default form is pretty-printed: {p}");
    assert_eq!(c.trim_end().lines().count(), 1, "compact form is one line: {c}");
    let pv: serde_json::Value = serde_json::from_str(&p).expect("pretty output parses");
    let cv: serde_json::Value = serde_json::from_str(c.trim_end()).expect("compact output parses");
    assert_eq!(pv, cv);
}
