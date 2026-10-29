//! System identification with `--solve` (plan C4): a physical parameter is
//! recovered from a trajectory by minimising a squared-error objective.
//!
//! Each example program hard-codes a noise-free reference trajectory generated
//! from a known parameter, and minimises the squared error of a simulation
//! with the candidate parameter against it. The test runs the built binary,
//! as `solve_cli.rs` does, and checks the recovered value against the known
//! one and the solver path the JSON records.
//!
//! The objective is a loop over an integrator, so the convexity pass cannot
//! certify it: the solver is the (1+1)-ES, and the test asserts that. Seeds are
//! fixed, so the search is deterministic.

use std::process::Command;

/// The tolerance on a recovered parameter, absolute, in the parameter's units.
const TOLERANCE: f64 = 0.01;

fn bin() -> std::path::PathBuf {
    // `target/<profile>/deps/<test>` — the binary is two levels up.
    let mut p = std::env::current_exe().expect("test binary path");
    p.pop();
    p.pop();
    p.push(if cfg!(windows) { "mage-parse.exe" } else { "mage-parse" });
    assert!(p.exists(), "mage-parse not built at {p:?}");
    p
}

/// The path of an example program under `prototype/examples/`.
fn example(file: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join(file)
}

/// Solve `name` in `file` with `--solve --seed 7`, and return the one JSON
/// line it prints. Asserts a clean exit.
fn solve(file: &str, name: &str) -> serde_json::Value {
    let path = example(file);
    let out = Command::new(bin())
        .args(["--solve", path.to_str().unwrap(), name, "--seed", "7"])
        .output()
        .expect("spawn mage-parse");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(0), "{file}: stderr: {stderr}");
    assert_eq!(stdout.trim().lines().count(), 1, "one JSON line: {stdout:?}");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {stdout}"))
}

#[test]
fn spring_recovers_stiffness_and_damping_by_es() {
    // Known: k = 4.0, c = 0.6. The domains are k in [0.5, 10] and c in [0, 3].
    let v = solve("system_id_spring.mg", "sse");
    assert_eq!(v["solver"], "Es", "{v}");
    let reason = v["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("curvature"), "reason: {reason}");
    let x = v["x"].as_array().expect("x");
    assert_eq!(x.len(), 2, "two parameters: {v}");
    let (k, c) = (x[0].as_f64().unwrap(), x[1].as_f64().unwrap());
    assert!((k - 4.0).abs() < TOLERANCE, "k = {k}, want 4.0 within {TOLERANCE}: {v}");
    assert!((c - 0.6).abs() < TOLERANCE, "c = {c}, want 0.6 within {TOLERANCE}: {v}");
    let value = v["value"].as_f64().expect("value");
    assert!(value < 1e-6, "residual {value} is not near zero: {v}");
}

#[test]
fn pendulum_recovers_length_by_es() {
    // Known: L = 1.5 m. The domain is L in [0.5, 3.0].
    let v = solve("system_id_pendulum.mg", "sse");
    assert_eq!(v["solver"], "Es", "{v}");
    let reason = v["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("curvature"), "reason: {reason}");
    let x = v["x"].as_array().expect("x");
    assert_eq!(x.len(), 1, "one parameter: {v}");
    let l = x[0].as_f64().unwrap();
    assert!((l - 1.5).abs() < TOLERANCE, "L = {l}, want 1.5 within {TOLERANCE}: {v}");
    let value = v["value"].as_f64().expect("value");
    assert!(value < 1e-6, "residual {value} is not near zero: {v}");
}

#[test]
fn the_same_seed_gives_the_same_record() {
    // Determinism: the journal line is byte-identical across runs.
    let a = solve("system_id_spring.mg", "sse");
    let b = solve("system_id_spring.mg", "sse");
    assert_eq!(a, b);
}
