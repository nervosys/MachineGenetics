//! `mage-parse --pipeline FILE --incremental-stats`: the engine's account of
//! itself, as an agent reads it.
//!
//! Spawned against the built binary, because the surface under test is the
//! command line and its JSON, and only the binary has one.

use std::path::{Path, PathBuf};
use std::process::Command;

const SRC: &str = "\
f add(a: i32, b: i32) -> i32 { a + b }
f double(x: i32) -> i32 { x * 2 }
f triple(x: i32) -> i32 { x * 3 }
f quad(x: i32) -> i32 { x * 4 }
f main() -> i32 { add(double(1), triple(2)) }
";

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mage-parse"))
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mage-inc-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

struct Out {
    json: Option<serde_json::Value>,
    stderr: String,
    code: i32,
}

fn run(args: &[&str], envs: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(bin());
    cmd.args(args).env_remove("MAGE_CACHE_DIR");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn mage-parse");
    Out {
        json: serde_json::from_slice(&out.stdout).ok(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn pipeline(file: &Path, cache: &Path) -> Out {
    run(&["--pipeline", file.to_str().unwrap(), "--incremental-stats", "--cache-dir", cache.to_str().unwrap()], &[])
}

/// Executions (not disk reloads) of `query` that the engine listed.
fn executions(j: &serde_json::Value, query: &str) -> Vec<String> {
    j["stats"]["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|e| e["query"] == query && e["why"] != "disk")
        .map(|e| e["key"].as_str().unwrap().to_string())
        .collect()
}

fn total(j: &serde_json::Value, field: &str) -> u64 {
    j["stats"]["total"][field].as_u64().unwrap_or_else(|| panic!("no total.{field} in {}", j["stats"]["total"]))
}

#[test]
fn stats_are_printed_as_json_and_a_second_run_is_answered_from_the_cache() {
    let dir = scratch("warm");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let cache = dir.join("cache");

    let cold = pipeline(&file, &cache);
    assert_eq!(cold.code, 0, "{}", cold.stderr);
    let cold = cold.json.expect("stdout is one JSON object");
    assert_eq!(cold["summary"]["items"], 5);
    assert_eq!(cold["summary"]["total_errors"], 0);
    assert!(total(&cold, "misses") > 0);
    assert!(total(&cold, "disk_stores") > 0, "a cold run writes the cache");
    assert_eq!(total(&cold, "disk_hits"), 0);

    let warm = pipeline(&file, &cache);
    assert_eq!(warm.code, 0, "{}", warm.stderr);
    let warm = warm.json.expect("json");
    assert_eq!(warm["summary"], cold["summary"], "the same answer, from the cache");
    assert!(total(&warm, "disk_hits") > 0);
    assert_eq!(total(&warm, "disk_corrupt"), 0);
    // Every serialisable query hit. What still runs is the lookup of an item's
    // hash, which exists to compute a cache address.
    for (q, v) in warm["stats"]["queries"].as_object().unwrap() {
        if ["source", "settings", "item", "item_hash"].contains(&q.as_str()) {
            continue;
        }
        assert_eq!(v["misses"], 0, "`{q}` ran on a warm start: {v}");
        assert_eq!(v["reexecutions"], 0, "`{q}` re-ran on a warm start: {v}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editing_one_function_shows_up_as_one_function_re_run() {
    let dir = scratch("edit");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let cache = dir.join("cache");
    assert_eq!(pipeline(&file, &cache).code, 0);

    std::fs::write(&file, SRC.replace("x * 3", "x * 30")).unwrap();
    let edited = pipeline(&file, &cache);
    assert_eq!(edited.code, 0, "{}", edited.stderr);
    let j = edited.json.expect("json");
    for q in ["resolve_item", "pass/verify", "pass/logic", "pass/contract", "pass/shape"] {
        let ran = executions(&j, q);
        assert_eq!(ran.len(), 1, "{q} should have run for one function, ran: {ran:?}");
        assert!(ran[0].contains("\"triple\""), "{q} ran for the wrong function: {ran:?}");
        assert_eq!(j["stats"]["queries"][q]["disk_hits"], 4, "{q}: the other four functions come from the cache");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cache_directory_can_come_from_the_environment() {
    let dir = scratch("env");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let cache = dir.join("cache");
    let args = ["--pipeline", file.to_str().unwrap(), "--incremental-stats"];
    let env = [("MAGE_CACHE_DIR", cache.to_str().unwrap())];
    let first = run(&args, &env);
    assert_eq!(first.code, 0, "{}", first.stderr);
    assert!(total(&first.json.unwrap(), "disk_stores") > 0);
    let second = run(&args, &env);
    assert!(total(&second.json.unwrap(), "disk_hits") > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_cache_directory_nothing_persists_and_nothing_breaks() {
    let dir = scratch("nocache");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let out = run(&["--pipeline", file.to_str().unwrap(), "--incremental-stats"], &[]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let j = out.json.unwrap();
    assert_eq!(total(&j, "disk_stores"), 0);
    assert_eq!(total(&j, "disk_hits"), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failing_source_still_reports_and_exits_one() {
    let dir = scratch("fail");
    let file = dir.join("bad.mg");
    std::fs::write(&file, "f g() -> i32 { zz }\n").unwrap();
    let out = run(&["--pipeline", file.to_str().unwrap(), "--incremental-stats"], &[]);
    assert_eq!(out.code, 1, "{}", out.stderr);
    let j = out.json.expect("the report is printed before the exit");
    assert!(j["summary"]["total_errors"].as_u64().unwrap() > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cache_dir_value_is_not_mistaken_for_the_file() {
    // `--cache-dir DIR` before the path: DIR must vanish with its flag.
    let dir = scratch("order");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let cache = dir.join("cache");
    let out = run(
        &["--cache-dir", cache.to_str().unwrap(), "--pipeline", file.to_str().unwrap(), "--incremental-stats"],
        &[],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.json.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_mode_is_still_an_error() {
    // The new flags are modifiers, not modes: a near-miss spelling is named as
    // a mode error, not opened as a file.
    let out = run(&["--incremental", "x.mg"], &[]);
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("unknown mode: --incremental"), "{}", out.stderr);
    assert!(!out.stderr.contains("Error reading"), "{}", out.stderr);
}

#[test]
fn the_default_pipeline_is_unchanged() {
    // Without the flag, `--pipeline` is the old stage-by-stage driver.
    let dir = scratch("old");
    let file = dir.join("a.mg");
    std::fs::write(&file, SRC).unwrap();
    let out = run(&["--pipeline", file.to_str().unwrap()], &[]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stderr.contains("MAGE End-to-End Pipeline"), "{}", out.stderr);
    assert!(out.json.is_none(), "the old driver prints MLIR, not JSON");
    let _ = std::fs::remove_dir_all(&dir);
}
