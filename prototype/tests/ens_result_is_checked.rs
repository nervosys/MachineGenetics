//! Plan C1: `result` and `old` in `@ens`, checked at run time.
//!
//! A postcondition relating a function's output to its input could not be
//! written: `MAGE_SPEC.md` §12.1 said neither `result` nor `old` was in scope.
//! Now `result` names the return value and `old(x)` a parameter as passed,
//! `--check` typechecks both, and `--eval` checks every `@ens` at every return
//! of the function it belongs to. A violation fails the run and names the
//! contract.
//!
//! Spawned against the built binary, like `refuted_contract_is_a_diagnostic`,
//! because what is pinned is what the CLI reports.

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
fn run(name: &str, src: &str, args: &[&str]) -> (String, String, i32) {
    let dir = std::env::temp_dir().join(format!("mage-ens-{}-{name}", std::process::id()));
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

const HOLDS: &str = "sp withdraw {\n    @req(balance >= amount, \"sufficient funds\")\n    \
                     @ens(result == old(balance) - amount, \"correct deduction\")\n}\n\n\
                     f withdraw(balance: u64, amount: u64) -> u64 { balance - amount }\n\n\
                     f main() -> u64 { withdraw(10, 3) }\n";

/// The same contract over a body that deducts twice.
const VIOLATED: &str = "sp withdraw {\n    @req(balance >= amount, \"sufficient funds\")\n    \
                        @ens(result == old(balance) - amount, \"correct deduction\")\n}\n\n\
                        f withdraw(balance: u64, amount: u64) -> u64 { balance - amount - amount }\n\n\
                        f main() -> u64 { withdraw(10, 3) }\n";

#[test]
fn a_postcondition_naming_result_checks_and_runs() {
    let (_, stderr, code) = run("holds-check", HOLDS, &["--check"]);
    assert!(stderr.contains("Errors: 0"), "got:\n{stderr}");
    assert_eq!(code, 0, "got:\n{stderr}");
    let (stdout, stderr, code) = run("holds-eval", HOLDS, &["--eval"]);
    assert_eq!(stdout.trim(), "7", "stderr:\n{stderr}");
    assert_eq!(code, 0, "stderr:\n{stderr}");
}

#[test]
fn a_violated_postcondition_fails_the_run_and_names_the_contract() {
    // It typechecks: a wrong body is a run-time fact, not a type error.
    let (_, stderr, _) = run("violated-check", VIOLATED, &["--check"]);
    assert!(stderr.contains("Errors: 0"), "got:\n{stderr}");

    let (stdout, stderr, code) = run("violated-eval", VIOLATED, &["--eval"]);
    let all = format!("{stdout}{stderr}");
    assert!(
        all.contains(
            "contract violated: `@ens(result == old(balance) - amount, \"correct deduction\")` \
             of `withdraw` (sp withdraw, line 3:5) is false: `withdraw` returned 4"
        ),
        "got:\n{all}"
    );
    assert_ne!(code, 0, "a violated postcondition must fail the run; got:\n{all}");
}

#[test]
fn result_in_a_precondition_is_e0561() {
    let src = "sp g {\n    @req(result > 0)\n}\n\nf g(x: i64) -> i64 { x }\n";
    let (stdout, _, code) = run("req-result", src, &["--check", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("--json emits JSON");
    let hits: Vec<_> = v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["code"] == "E0561")
        .collect();
    assert_eq!(hits.len(), 1, "got:\n{stdout}");
    assert_eq!(hits[0]["category"], "ContractScope");
    assert_ne!(code, 0, "got:\n{stdout}");
}

#[test]
fn old_of_a_non_parameter_is_e0561() {
    let src = "sp g {\n    @ens(result == old(y))\n}\n\nf g(x: i64) -> i64 { x }\n";
    let (_, stderr, code) = run("old-nonparam", src, &["--check"]);
    assert!(stderr.contains("`y` is not a parameter of `g`"), "got:\n{stderr}");
    assert_ne!(code, 0, "got:\n{stderr}");
}
