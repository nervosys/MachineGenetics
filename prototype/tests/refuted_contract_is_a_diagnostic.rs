//! A refuted contract is an `E0560` error, in both `--check` streams.
//!
//! `DiagnosticCategory::SpecViolation` had a stable code and no producer: its
//! only non-test mention was `heal::infer_category`, classifying a message
//! someone else wrote. `HANDOFF.md` item 34 put that down to the corpus — "12
//! contract clauses with all 12 `Unknown`" — and it was worse than that.
//! **`CheckResult::Violated` was never constructed anywhere**, so no program
//! could have refuted a contract; and had one done so, `--check` printed a `✗`
//! row and exited 0, because the verifier's verdicts were never counted as
//! errors.
//!
//! Spawned against the built binary, because the defect was in what the CLI
//! reports, and the unit tests in `verify.rs` cannot see that.

use std::process::Command;

fn bin() -> std::path::PathBuf {
    // `target/<profile>/deps/<test>` — the binary is two levels up.
    let mut p = std::env::current_exe().expect("test binary path");
    p.pop();
    p.pop();
    p.push(if cfg!(windows) { "mage-parse.exe" } else { "mage-parse" });
    assert!(p.exists(), "mage-parse not built at {p:?}");
    p
}

/// Write `src` to a fresh file and run `mage-parse <args> <file>`.
/// Returns (stdout, stderr, exit code).
fn check(name: &str, src: &str, args: &[&str]) -> (String, String, i32) {
    let dir = std::env::temp_dir().join(format!("mage-refuted-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("m.mg");
    std::fs::write(&file, src).expect("write source");
    let out = Command::new(bin()).args(args).arg(&file).output().expect("spawn mage-parse");
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

const REFUTED: &str = "sp never {\n    @req(xs.len() < 0)\n}\n";

#[test]
fn json_stream_carries_e0560_for_a_refuted_contract() {
    let (stdout, _, code) = check("json", REFUTED, &["--check", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("--json emits JSON");
    let hits: Vec<_> = v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["code"] == "E0560")
        .collect();
    assert_eq!(hits.len(), 1, "one refuted clause, one E0560; got:\n{stdout}");
    assert_eq!(hits[0]["category"], "SpecViolation");
    assert_eq!(hits[0]["severity"], "error");
    assert_eq!(v["ok"], false, "a refuted contract is not ok; got:\n{stdout}");
    assert_ne!(code, 0, "--check --json must fail on a refuted contract");
}

#[test]
fn human_check_counts_a_refuted_contract_as_an_error() {
    let (_, stderr, code) = check("human", REFUTED, &["--check"]);
    assert!(stderr.contains("is false on every execution"), "got:\n{stderr}");
    assert!(stderr.contains("Errors: 1"), "the refutation must be counted; got:\n{stderr}");
    assert_ne!(code, 0, "--check exited 0 over a refuted contract until 2026-09-25");
}

#[test]
fn unadjudicated_contracts_raise_nothing() {
    // `1b` is the sigil spelling of `true`. The verifier cannot prove it (it
    // reports `Unreached`), and that is a gap in the verifier, not a fault in
    // the program — so it must not become an error.
    let (stdout, _, code) =
        check("unknown", "sp fine {\n    @req(1b)\n    @ens(path.exists())\n}\n", &["--check", "--json"]);
    assert!(!stdout.contains("E0560"), "got:\n{stdout}");
    assert_eq!(code, 0, "got:\n{stdout}");
}
