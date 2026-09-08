//! An unrecognised flag is answered as a mode error, not as a missing file.
//!
//! The catch-all arm of `main.rs`'s dispatch opens its argument as a path, so
//! every unknown flag used to be answered with `Error reading --whatever: The
//! system cannot find the file specified` — a diagnostic about the filesystem
//! for a mistake in the command line, naming the wrong thing entirely. It is
//! the shape `HANDOFF.md`'s failure taxonomy §6 collects: a diagnostic that
//! names the wrong token.
//!
//! **`main.rs` had already fixed this twice, one flag at a time**, and both
//! fixes carry a comment explaining it. `--version` got an arm because passing
//! it "made the binary try to *open a file called `--version`*"; `--fix` was
//! added to `is_modifier_flag` because `--build=abl --fix spec.json out.abl`
//! "consumed the flag as the input filename". Nobody swept the population, so
//! `--help` — the flag a person types first — still answered `Error reading
//! --help` on 2026-09-08, in an agent-facing CLI whose manifest calls itself
//! the thing to read first.
//!
//! Spawned against the built binary rather than unit-tested, because what
//! broke is the CLI surface and only the binary has one.

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

/// (stdout+stderr, exit code)
fn run(args: &[&str]) -> (String, i32) {
    let out = Command::new(bin()).args(args).output().expect("spawn mage-parse");
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (s, out.status.code().unwrap_or(-1))
}

#[test]
fn unknown_double_dash_flag_names_the_mode_not_a_file() {
    let (out, code) = run(&["--help"]);
    assert_eq!(code, 2, "unknown mode exits 2, matching --describe's no-match path");
    assert!(out.contains("unknown mode: --help"), "should name the flag; got:\n{out}");
    assert!(
        !out.contains("Error reading"),
        "must not report a filesystem error for a command-line mistake; got:\n{out}"
    );
    // The manifest is printed so the answer is actionable rather than a refusal.
    assert!(out.contains("--manifest"), "should list valid modes; got:\n{out}");
}

#[test]
fn unknown_single_dash_flag_is_caught_too() {
    // The first version of the fix tested `starts_with("--")` and left this
    // one still answering `Error reading -h`. `-V` is an accepted alias, so
    // single-dash spellings genuinely reach this dispatch.
    let (out, code) = run(&["-h"]);
    assert_eq!(code, 2, "got:\n{out}");
    assert!(out.contains("unknown mode: -h"), "got:\n{out}");
    assert!(!out.contains("Error reading"), "got:\n{out}");
}

#[test]
fn a_mode_that_exists_only_with_a_value_is_named_not_opened() {
    // `--spine=frame` and `--spine=swarm` are modes; bare `--spine` is not,
    // and used to be opened as a file.
    let (out, code) = run(&["--spine", "whatever.json"]);
    assert_eq!(code, 2, "got:\n{out}");
    assert!(out.contains("unknown mode: --spine"), "got:\n{out}");
}

#[test]
fn known_flags_still_dispatch() {
    // The guard sits immediately before the catch-all, so every real mode is
    // matched earlier. Two spellings that would be caught by a careless
    // implementation of this rule.
    let (out, code) = run(&["--version"]);
    assert_eq!(code, 0, "got:\n{out}");
    assert!(out.contains("mage-parse "), "got:\n{out}");

    let (out, code) = run(&["-V"]);
    assert_eq!(code, 0, "single-dash alias must not be swallowed; got:\n{out}");
    assert!(out.contains("mage-parse "), "got:\n{out}");
}

#[test]
fn bare_dash_still_means_stdin() {
    // The rule excludes `-` on purpose: it is the conventional spelling for
    // standard input, and `--fmt-compact -` is how an editor formats a buffer.
    // Guarding this here as well as in `fmt_stdin_and_out.rs`, because it is
    // what a leading-`-` rule would break first.
    let mut child = Command::new(bin())
        .args(["--fmt-compact", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mage-parse");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"f main() -> i32 { 42 }\n")
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("42"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn a_real_path_still_parses() {
    // The catch-all still has to work: this is the regression the guard could
    // most easily cause.
    let dir = std::env::temp_dir().join("mage_unknown_mode_test");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join("ok.mg");
    std::fs::write(&path, "f main() -> i32 { 1 }\n").expect("write");
    let (out, code) = run(&["--check", path.to_str().expect("utf8")]);
    assert_eq!(code, 0, "got:\n{out}");
    assert!(out.contains("Errors: 0"), "got:\n{out}");
    let _ = std::fs::remove_file(&path);
}
