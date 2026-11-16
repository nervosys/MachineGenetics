//! With a cache directory, `--check` and `--pipeline` are answered by the
//! incremental engine, and print exactly what the direct drivers print.
//!
//! `tests/incremental_equivalence.rs` proves the engine's values equal the old
//! drivers'. This holds the surface: stdout, stderr and the exit status of the
//! binary, run twice against one cache directory (cold, then warm) and once
//! with no cache directory at all.

use std::path::{Path, PathBuf};
use std::process::Command;

const CLEAN: &str = "\
f add(a: i32, b: i32) -> i32 { a + b }
f double(x: i32) -> i32 { x * 2 }
f main() -> i32 { add(double(1), 2) }
";

/// An unknown name and a type mismatch: errors, so the exit status is 1.
const FAILING: &str = "\
f ok(a: i32) -> i32 { a }
f bad(a: i32) -> i32 { zz }
f worse() -> bool { 1 + 1 }
";

/// Does not parse.
const BROKEN: &str = "f oops( -> i32 { 1 }\n";

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mage-engine-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

#[derive(Debug, PartialEq, Eq)]
struct Run {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

fn run(args: &[&str], file: &Path, cache: Option<&Path>, via_env: bool) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mage-parse"));
    cmd.env_remove("MAGE_CACHE_DIR").args(args).arg(file);
    if let Some(c) = cache {
        if via_env {
            cmd.env("MAGE_CACHE_DIR", c);
        } else {
            cmd.arg("--cache-dir").arg(c);
        }
    }
    let out = cmd.output().expect("spawn mage-parse");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code(),
    }
}

/// Direct (no cache), engine cold, engine warm: all three identical.
fn assert_same(tag: &str, src: &str, args: &[&str]) -> Run {
    let dir = scratch(tag);
    let file = dir.join("input.mg");
    std::fs::write(&file, src).unwrap();
    let cache = dir.join("cache");
    let direct = run(args, &file, None, false);
    let cold = run(args, &file, Some(&cache), false);
    let warm = run(args, &file, Some(&cache), false);
    assert_eq!(cold, direct, "{tag} {args:?}: engine (cold) differs from the direct driver");
    assert_eq!(warm, direct, "{tag} {args:?}: engine (warm) differs from the direct driver");
    // The warm run really was answered from the cache the cold run wrote.
    assert!(
        std::fs::read_dir(&cache).map(|d| d.count() > 0).unwrap_or(false),
        "{tag} {args:?}: --cache-dir wrote nothing, so the engine did not answer"
    );
    let _ = std::fs::remove_dir_all(&dir);
    direct
}

#[test]
fn check_json_is_identical_with_and_without_a_cache() {
    let ok = assert_same("json-ok", CLEAN, &["--check", "--json"]);
    assert_eq!(ok.code, Some(0), "{ok:?}");
    assert!(ok.stdout.contains("\"ok\": true"), "{}", ok.stdout);
    let bad = assert_same("json-bad", FAILING, &["--check", "--json"]);
    assert_eq!(bad.code, Some(1), "{bad:?}");
    assert!(bad.stdout.contains("\"ok\": false"), "{}", bad.stdout);
    let broken = assert_same("json-broken", BROKEN, &["--check", "--json"]);
    assert_eq!(broken.code, Some(1), "{broken:?}");
    assert_same("json-compact", FAILING, &["--check", "--json", "--compact"]);
}

#[test]
fn human_check_is_identical_with_and_without_a_cache() {
    let ok = assert_same("human-ok", CLEAN, &["--check"]);
    assert_eq!(ok.code, Some(0), "{ok:?}");
    assert!(ok.stderr.contains("Status: OK"), "{}", ok.stderr);
    let bad = assert_same("human-bad", FAILING, &["--check"]);
    assert_eq!(bad.code, Some(1), "{bad:?}");
    assert!(bad.stderr.contains("Errors:"), "{}", bad.stderr);
    let broken = assert_same("human-broken", BROKEN, &["--check"]);
    assert_eq!(broken.code, Some(1), "{broken:?}");
    assert!(broken.stderr.contains("E0001"), "{}", broken.stderr);
    assert_same("human-nosafe", FAILING, &["--check", "--no-elision"]);
    assert_same("human-core", CLEAN, &["--check", "--core"]);
    assert_same("human-tokens", CLEAN, &["--check", "--token-report"]);
}

#[test]
fn pipeline_is_identical_with_and_without_a_cache() {
    let ok = assert_same("pipe-ok", CLEAN, &["--pipeline"]);
    assert_eq!(ok.code, Some(0), "{ok:?}");
    assert!(ok.stdout.contains("func") || !ok.stdout.trim().is_empty(), "MLIR on stdout: {ok:?}");
    let bad = assert_same("pipe-bad", FAILING, &["--pipeline"]);
    assert_eq!(bad.code, Some(1), "{bad:?}");
    let broken = assert_same("pipe-broken", BROKEN, &["--pipeline"]);
    assert_eq!(broken.code, Some(1), "{broken:?}");
    assert_same("pipe-nosafe", CLEAN, &["--pipeline", "--no-elision"]);
    assert_same("pipe-tokens", CLEAN, &["--pipeline", "--token-report"]);
}

#[test]
fn the_corpus_prints_the_same_either_way() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("examples");
    let mut files = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "mg") {
                out.push(p);
            }
        }
    }
    walk(&root, &mut files);
    files.sort();
    assert!(files.len() >= 8, "found {} example programs", files.len());
    let cache = scratch("corpus");
    for f in &files {
        for args in [&["--check"][..], &["--check", "--json"][..], &["--pipeline"][..]] {
            let direct = run(args, f, None, false);
            let engine = run(args, f, Some(&cache), false);
            assert_eq!(engine, direct, "{} {args:?}", f.display());
        }
    }
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn the_strict_policy_is_part_of_the_cached_answer() {
    // One cache directory, both policies, both orders: a lax answer must not
    // be served to a strict run, or the reverse.
    let dir = scratch("policy");
    let file = dir.join("input.mg");
    let src = "\
struct Cfg { n: i32, name: String }
fn render(c: Cfg, t: String) -> String { t }
fn run(c: Cfg) -> String {
    val a = render(c, \"x\")
    render(c, a)
}
";
    std::fs::write(&file, src).unwrap();
    let cache = dir.join("cache");
    let strict_args = ["--check", "--json", "--borrowck-strict"];
    let lax_args = ["--check", "--json"];
    let strict_direct = run(&strict_args, &file, None, false);
    let lax_direct = run(&lax_args, &file, None, false);
    assert_ne!(strict_direct.stdout, lax_direct.stdout);
    for _ in 0..2 {
        assert_eq!(run(&lax_args, &file, Some(&cache), false), lax_direct);
        assert_eq!(run(&strict_args, &file, Some(&cache), false), strict_direct);
    }
    for _ in 0..2 {
        assert_eq!(run(&["--check"], &file, Some(&cache), false), run(&["--check"], &file, None, false));
        assert_eq!(
            run(&["--check", "--borrowck-strict"], &file, Some(&cache), false),
            run(&["--check", "--borrowck-strict"], &file, None, false)
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cache_directory_can_come_from_the_environment() {
    let dir = scratch("env");
    let file = dir.join("input.mg");
    std::fs::write(&file, CLEAN).unwrap();
    let cache = dir.join("cache");
    let direct = run(&["--check", "--json"], &file, None, false);
    let via_env = run(&["--check", "--json"], &file, Some(&cache), true);
    assert_eq!(via_env, direct);
    assert!(std::fs::read_dir(&cache).map(|d| d.count() > 0).unwrap_or(false), "MAGE_CACHE_DIR was not used");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_cache_directory_nothing_is_written() {
    let dir = scratch("none");
    let file = dir.join("input.mg");
    std::fs::write(&file, CLEAN).unwrap();
    let before = std::fs::read_dir(&dir).unwrap().count();
    run(&["--check"], &file, None, false);
    run(&["--pipeline"], &file, None, false);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);
    let _ = std::fs::remove_dir_all(&dir);
}
