//! `--solve` as a CLI: the solver choice reaches the command line, and its
//! journal record is what a caller reads.
//!
//! Spawned against the built binary, because what the mode adds is the
//! surface: argument parsing, the JSON on stdout, and the exit code. The
//! solver itself is unit-tested in `src/solve.rs`.

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> std::path::PathBuf {
    // `target/<profile>/deps/<test>` — the binary is two levels up.
    let mut p = std::env::current_exe().expect("test binary path");
    p.pop();
    p.pop();
    p.push(if cfg!(windows) { "mage-parse.exe" } else { "mage-parse" });
    assert!(p.exists(), "mage-parse not built at {p:?}");
    p
}

/// Run `mage-parse` with `args`, optionally piping `stdin`.
/// Returns (stdout, stderr, exit code).
fn run(args: &[&str], stdin: Option<&str>) -> (String, String, i32) {
    let mut cmd = Command::new(bin());
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().expect("spawn mage-parse");
    if let Some(src) = stdin {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(src.as_bytes())
            .expect("write stdin");
    }
    let o = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
        o.status.code().unwrap_or(-1),
    )
}

/// Write `src` to a file unique to `name`, and return its path.
fn write_source(name: &str, src: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("mage_solve_cli_test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(format!("{name}.mg"));
    std::fs::write(&path, src).expect("write source");
    path
}

/// A convex objective with its domain declared by `@param`.
const CONVEX: &str = "@param(-4.0, 4.0)\nconst X: f64 = 0.0;\n\
    f quad(x: f64) -> f64 { (x - 1.5) * (x - 1.5) }";

fn json(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {stdout:?}"))
}

#[test]
fn a_convex_objective_takes_projected_gradient_to_the_optimum() {
    let path = write_source("convex", CONVEX);
    let (stdout, stderr, code) = run(&["--solve", path.to_str().unwrap(), "quad"], None);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout.trim().lines().count(), 1, "one JSON line: {stdout:?}");
    let v = json(&stdout);
    assert_eq!(v["solver"], "ProjectedGradient", "{stdout}");
    assert!(v["reason"].as_str().is_some_and(|r| !r.is_empty()), "{stdout}");
    let x = v["x"][0].as_f64().expect("x[0]");
    assert!((x - 1.5).abs() < 1e-3, "x = {x}, in {stdout}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_uncertified_objective_falls_back_to_es() {
    // `a*b` is neither convex nor concave, so DCP cannot certify it.
    let src = "@param(-2.0, 2.0)\nconst A: f64 = 0.0;\n\
        @param(-2.0, 2.0)\nconst B: f64 = 0.0;\n\
        f prod(a: f64, b: f64) -> f64 { a * b }";
    let path = write_source("uncertified", src);
    let (stdout, stderr, code) = run(&["--solve", path.to_str().unwrap(), "prod"], None);
    assert_eq!(code, 0, "stderr: {stderr}");
    let v = json(&stdout);
    assert_eq!(v["solver"], "Es", "{stdout}");
    assert!(v["reason"].as_str().is_some_and(|r| r.contains("curvature")), "{stdout}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn max_takes_a_concave_objective_to_its_peak() {
    let src = "@param(-3.0, 3.0)\nconst X: f64 = 0.0;\n\
        f peak(x: f64) -> f64 { -(x - 1.0) * (x - 1.0) }";
    let path = write_source("concave", src);
    let (stdout, stderr, code) = run(&["--solve", path.to_str().unwrap(), "peak", "--max"], None);
    assert_eq!(code, 0, "stderr: {stderr}");
    let v = json(&stdout);
    assert_eq!(v["solver"], "ProjectedGradient", "{stdout}");
    let x = v["x"][0].as_f64().expect("x[0]");
    assert!((x - 1.0).abs() < 1e-3, "x = {x}, in {stdout}");
    let value = v["value"].as_f64().expect("value");
    assert!(value.abs() < 1e-6, "value = {value}, in {stdout}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_parameter_without_a_param_const_exits_one_and_says_so() {
    // `b` has no `@param` const, so the domain cannot be read.
    let src = "@param(-2.0, 2.0)\nconst A: f64 = 0.0;\n\
        f quad(a: f64, b: f64) -> f64 { a * a + b * b }";
    let path = write_source("missing_param", src);
    let path_str = path.to_str().unwrap();
    let (stdout, stderr, code) = run(&["--solve", path_str, "quad"], None);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.trim().is_empty(), "no JSON on a failed solve: {stdout:?}");
    assert!(stderr.contains(path_str), "stderr names the file: {stderr}");
    assert!(stderr.contains("no matching @param"), "stderr: {stderr}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_source_piped_on_stdin_is_solved() {
    let (stdout, stderr, code) = run(&["--solve", "-", "quad"], Some(CONVEX));
    assert_eq!(code, 0, "stderr: {stderr}");
    let v = json(&stdout);
    assert_eq!(v["solver"], "ProjectedGradient", "{stdout}");
    let x = v["x"][0].as_f64().expect("x[0]");
    assert!((x - 1.5).abs() < 1e-3, "x = {x}, in {stdout}");
}
