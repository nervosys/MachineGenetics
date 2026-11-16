//! `--borrowck-strict`: the borrow checker under Rust's move rule
//! (`borrowck::Policy::STRICT`), opt-in on `--check` and `--pipeline`.
//!
//! Without the flag the default policy reports only the transfers of ownership
//! a program itself evidences, and output is unchanged.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A by-value call that does not itself consume its argument, then a reuse.
/// Rust rejects it; the inferred default does not.
const REUSED: &str = "\
struct Cfg { n: i32, name: String }
fn render(c: Cfg, t: String) -> String { t }
fn run(c: Cfg) -> String {
    val a = render(c, \"x\")
    render(c, a)
}
";

fn write(tag: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mage-strict-{tag}-{}.mg", std::process::id()));
    std::fs::write(&path, src).unwrap();
    path
}

fn mage(args: &[&str], path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mage-parse"))
        .env_remove("MAGE_CACHE_DIR")
        .args(args)
        .arg(path)
        .output()
        .unwrap()
}

fn codes(out: &Output) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).expect("--check --json prints JSON");
    value["diagnostics"].as_array().unwrap().iter().map(|d| d["code"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_reused_by_value_argument_is_e0382_only_under_the_flag() {
    let path = write("json", REUSED);
    let lax = mage(&["--check", "--json"], &path);
    assert!(lax.status.success(), "default policy rejected it: {}", String::from_utf8_lossy(&lax.stdout));
    assert!(!codes(&lax).contains(&"E0382".to_string()));

    let strict = mage(&["--check", "--json", "--borrowck-strict"], &path);
    let _ = std::fs::remove_file(&path);
    assert!(!strict.status.success());
    assert!(codes(&strict).contains(&"E0382".to_string()), "{}", String::from_utf8_lossy(&strict.stdout));
}

#[test]
fn the_flag_works_for_the_human_check_wherever_it_is_written() {
    let path = write("human", REUSED);
    assert!(mage(&["--check"], &path).status.success());
    for args in [&["--check", "--borrowck-strict"][..], &["--borrowck-strict", "--check"][..]] {
        let out = mage(args, &path);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?}: {err}");
        assert!(err.contains("E0382") && err.contains("use of moved value: `c`"), "{args:?}: {err}");
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_flag_works_for_the_pipeline() {
    let path = write("pipeline", REUSED);
    assert!(mage(&["--pipeline"], &path).status.success());
    let out = mage(&["--pipeline", "--borrowck-strict"], &path);
    let _ = std::fs::remove_file(&path);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success() && err.contains("E0382"), "{err}");
}

fn corpus(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if !matches!(name.as_str(), "target" | ".git" | ".claude" | "node_modules") {
                corpus(&p, out);
            }
        } else if name.ends_with(".mg") {
            out.push(p);
        }
    }
}

/// The corpus was written against the inferred policy and freely reuses values
/// Rust would call moved. Under the strict policy, the findings are in the
/// example programs and nowhere else (the compiler's own tests and benchmarks
/// are clean); `examples/cli-tool` is known to have one.
#[test]
fn strict_findings_in_the_corpus_are_in_example_programs_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    corpus(root, &mut files);
    assert!(files.len() > 50, "found {} .mg files", files.len());
    let mut flagged = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let strict = mage_prototype::check::check_source_with(&src, true, false, mage_prototype::borrowck::Policy::STRICT);
        let lax = mage_prototype::check::check_source(&src, true, false);
        let moved = |c: &mage_prototype::check::Checked| c.findings.iter().filter(|x| x.code == "E0382").count();
        assert!(moved(&strict) >= moved(&lax), "{}: strict is stricter, never laxer", f.display());
        if moved(&strict) > 0 {
            flagged.push(f.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
    assert!(flagged.iter().any(|f| f == "examples/cli-tool/src/main.mg"), "{flagged:?}");
    assert!(flagged.iter().all(|f| f.starts_with("examples/")), "strict findings outside examples/: {flagged:?}");
}
