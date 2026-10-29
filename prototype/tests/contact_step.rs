//! Contact as a convex step (plan C5): one time step of a ball on a floor, posed
//! as a box-constrained quadratic program, solved by `--solve`.
//!
//! The program is `prototype/examples/contact_step.mg`. Each function fixes its
//! state in its body, and minimises `(yp - y_free)^2 + (vp - v_free)^2` over the
//! box given by the `@param` consts. The convexity pass certifies that sum of
//! squares of affine terms, so the solver must be projected gradient, not ES.
//!
//! The expected values are the reference steps of `sim_bounce` in
//! `arena/src/physics.rs` (the bounce system, P1), with `g = 9.81`:
//!
//! - `contact_free` (`y = 1.0`, `v = 0.0`, `dt = 1e-3`) never reaches the floor.
//!   bounce gives `y = 0.999995095`, `v = -0.00981` for any restitution `e`,
//!   since no contact occurs. The QP matches both components.
//! - `contact_floor` (`y = 0.001`, `v = -1.0`, `dt = 0.01`) penetrates. The QP
//!   projects `yp` to the floor, `yp = 0`. bounce with `e = 0` also ends the step
//!   at `y = 0`, so the position matches. With `e > 0` bounce would rebound and
//!   end above the floor (`y = 0.00688` at `e = 0.8`), so the QP does not match
//!   that step, only the contact instant. The velocity is not part of the match
//!   either; see the test for why.
//!
//! The velocity update and restitution are not in the QP. The QP's `vp` is the
//! unconstrained `v - g * dt`, and in contact bounce sets `v = -e * (...)`
//! instead, which the QP cannot express as a projection. So the contact test
//! asserts the velocity the QP actually computes, not bounce's.

use std::process::Command;

/// Absolute tolerance on the recovered step. Projected gradient on a quadratic
/// reaches far finer than this; the bound guards against a loose stop.
const TOL: f64 = 1e-6;

fn bin() -> std::path::PathBuf {
    // `target/<profile>/deps/<test>` — the binary is two levels up.
    let mut p = std::env::current_exe().expect("test binary path");
    p.pop();
    p.pop();
    p.push(if cfg!(windows) { "mage-parse.exe" } else { "mage-parse" });
    assert!(p.exists(), "mage-parse not built at {p:?}");
    p
}

/// Solve `name` in `contact_step.mg` with `--solve --seed 7`, and return the one
/// JSON line it prints. Asserts a clean exit.
fn solve(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("contact_step.mg");
    let out = Command::new(bin())
        .args(["--solve", path.to_str().unwrap(), name, "--seed", "7"])
        .output()
        .expect("spawn mage-parse");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(0), "{name}: stderr: {stderr}");
    assert_eq!(stdout.trim().lines().count(), 1, "one JSON line: {stdout:?}");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("not JSON ({e}): {stdout}"))
}

/// Assert `v` took the certified convex route, and return `(yp, vp)`.
fn assert_projected_gradient(v: &serde_json::Value) -> (f64, f64) {
    assert_eq!(v["solver"], "ProjectedGradient", "{v}");
    let reason = v["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("certified for minimisation"), "reason: {reason}");
    let x = v["x"].as_array().expect("x");
    assert_eq!(x.len(), 2, "two decision variables (yp, vp): {v}");
    (x[0].as_f64().expect("yp"), x[1].as_f64().expect("vp"))
}

#[test]
fn a_free_step_above_the_floor_is_the_unconstrained_step() {
    // y = 1.0, v = 0.0, dt = 1e-3, g = 9.81. y_free = y + v dt - g dt^2 / 2 and
    // v_free = v - g dt, both as in bounce's `sim_bounce` for a step with no contact.
    let v = solve("contact_free");
    let (yp, vp) = assert_projected_gradient(&v);
    let y_free = 1.0 - 0.5 * 9.81 * 1e-6;
    let v_free = -9.81 * 1e-3;
    assert!((yp - y_free).abs() < TOL, "yp = {yp}, want y_free = {y_free}: {v}");
    assert!((vp - v_free).abs() < TOL, "vp = {vp}, want v_free = {v_free}: {v}");
    // The objective is zero at the unconstrained minimiser.
    let value = v["value"].as_f64().expect("value");
    assert!(value < 1e-12, "residual {value} is not near zero: {v}");
}

#[test]
fn a_step_that_would_penetrate_is_projected_onto_the_floor() {
    // y = 0.001, v = -1.0, dt = 0.01. y_free = -0.0094905 is below the floor, so
    // the box bound `yp >= 0` is active and the solution is yp = 0 exactly.
    let v = solve("contact_floor");
    let (yp, vp) = assert_projected_gradient(&v);
    assert!(yp.abs() < TOL, "yp = {yp}, want the floor 0: {v}");
    // The velocity is the unconstrained v_free = v - g dt, which the box does not
    // touch. Bounce with e = 0 gives v = 0 here: the velocity is not in this QP.
    let v_free = -1.0 - 9.81 * 0.01;
    assert!((vp - v_free).abs() < TOL, "vp = {vp}, want v_free = {v_free}: {v}");
    let value = v["value"].as_f64().expect("value");
    let y_free = 0.001 - 0.01 - 0.5 * 9.81 * 1e-4;
    let expected = (0.0 - y_free) * (0.0 - y_free);
    assert!(
        (value - expected).abs() < 1e-9,
        "value {value} is not (0 - y_free)^2 = {expected}: {v}"
    );
}

#[test]
fn the_contact_objective_is_deterministic_for_a_seed() {
    // Projected gradient is deterministic, so the journal line is byte-identical.
    assert_eq!(solve("contact_floor"), solve("contact_floor"));
    assert_eq!(solve("contact_free"), solve("contact_free"));
}
