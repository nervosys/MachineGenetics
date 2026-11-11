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
