//! `mage-parse --target=abl-train` and `--target=abl-infer` on the checked-in
//! example programs. Before these tests nothing ran either target, so the
//! documented "loss falls" claim had no pin.
//!
//! Pinned: exit status, and that the final loss is well below the first. The
//! exact reduction percentage is not pinned; the test asserts only that loss
//! falls by more than the margin in `MIN_REDUCTION`.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_mage-parse");

/// The repository root: `prototype/` is one level below it. The examples name
/// their checkpoint relative to the repository root, so every run starts there.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn run(target: &str, example: &str) -> Output {
    Command::new(BIN)
        .current_dir(repo_root())
        .arg(target)
        .arg(repo_root().join(example))
        .output()
        .unwrap()
}

/// Parse `first_loss=X last_loss=Y` from the `// train ... done:` line.
fn first_and_last_loss(stdout: &str) -> (f64, f64) {
    let line = stdout
        .lines()
        .find(|l| l.contains("done: first_loss="))
        .unwrap_or_else(|| panic!("no `done:` summary line in:\n{stdout}"));
    let field = |name: &str| -> f64 {
        let key = format!("{name}=");
        let start = line.find(&key).unwrap_or_else(|| panic!("no {name} in: {line}")) + key.len();
        let end = line[start..].find(' ').map_or(line.len(), |i| start + i);
        line[start..end].parse().unwrap_or_else(|e| panic!("{name} in {line}: {e}"))
    };
    (field("first_loss"), field("last_loss"))
}

/// Loss must fall to at most this fraction of its starting value. The measured
/// reduction on `train_demo.mg` is far larger than this, so the margin is
/// conservative: it catches a training loop that stops learning, not a
/// few-percent wobble in the optimiser.
const MAX_FINAL_OVER_FIRST: f64 = 0.5;

#[test]
fn abl_train_demo_loss_falls() {
    let out = run("--target=abl-train", "prototype/examples/train_demo.mg");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "exit {:?}\n{stdout}\n{}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    let (first, last) = first_and_last_loss(&stdout);
    assert!(first.is_finite() && last.is_finite(), "non-finite loss: {first} {last}\n{stdout}");
    assert!(
        last < first * MAX_FINAL_OVER_FIRST,
        "loss did not fall far enough: first={first} last={last}\n{stdout}"
    );
}

#[test]
fn abl_infer_after_train_prints_a_prediction_per_sample() {
    // `train_infer.mg` names a checkpoint, so inference needs the training
    // run first. Both steps are the documented two-step demo.
    let train = run("--target=abl-train", "prototype/examples/train_infer.mg");
    assert!(train.status.success(), "train failed: {}", String::from_utf8_lossy(&train.stderr));

    let out = run("--target=abl-infer", "prototype/examples/train_infer.mg");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "exit {:?}\n{stdout}\n{}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    // Four `inputs:` samples in the example, one prediction line each.
    let preds = stdout.lines().filter(|l| l.contains("input=[") && l.contains("pred=[")).count();
    assert_eq!(preds, 4, "expected one prediction per input sample:\n{stdout}");
}
