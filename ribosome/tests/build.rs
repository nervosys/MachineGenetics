//! End-to-end tests for the Ribosome build engine.
//!
//! These exercise the behaviours a build system is actually judged on — does a
//! rebuild do nothing, does an edit rebuild exactly what it must, does a failure
//! stop the right work and no more — rather than the units, which are covered
//! beside their modules.

use ribosome::cas::Store;
use ribosome::exec::{ExecError, LocalExecutor, PoolExecutor, ToolOutput, ToolRegistry};
use ribosome::graph::ActionGraph;
use ribosome::heal::{DefaultHealer, FALLBACK_KEY};
use ribosome::sched::{Outcome, Scheduler};
use ribosome::{Action, Digest, Platform};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir()
        .join(format!("ribosome-it-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Counts how many times each tool actually ran, so tests can assert on work
/// *avoided* — the only way to prove a cache is real.
#[derive(Default)]
struct RunCounter(AtomicUsize);

impl RunCounter {
    fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

/// `compile`: uppercases its input. `link`: concatenates its inputs.
fn tools(counter: Arc<RunCounter>) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    let c1 = counter.clone();
    r.register("compile@1", move |action, inputs| {
        c1.0.fetch_add(1, Ordering::Relaxed);
        let src = inputs.values().next().cloned().unwrap_or_default();
        let upper = String::from_utf8_lossy(&src).to_uppercase().into_bytes();
        let mut out = ToolOutput::new();
        for o in &action.outputs {
            out.outputs.insert(o.clone(), upper.clone());
        }
        Ok(out)
    });
    let c2 = counter;
    r.register("link@1", move |action, inputs| {
        c2.0.fetch_add(1, Ordering::Relaxed);
        let mut buf = Vec::new();
        for i in &action.inputs {
            buf.extend_from_slice(&inputs[&i.path]);
        }
        let mut out = ToolOutput::new();
        for o in &action.outputs {
            out.outputs.insert(o.clone(), buf.clone());
        }
        Ok(out)
    });
    r
}

/// Two compiles feeding a link — the smallest graph with real structure.
fn program(store: &Store, a_src: &[u8], b_src: &[u8]) -> ActionGraph {
    let da = store.cas.put(a_src).unwrap();
    let db = store.cas.put(b_src).unwrap();
    let mut g = ActionGraph::new();
    g.add(Action::new("compile:a", "compile@1").input("a.mg", da).output("a.o").cost(10))
        .unwrap();
    g.add(Action::new("compile:b", "compile@1").input("b.mg", db).output("b.o").cost(10))
        .unwrap();
    g.add(
        Action::new("link", "link@1")
            .input("a.o", Digest::of(b"placeholder"))
            .input("b.o", Digest::of(b"placeholder"))
            .output("prog")
            .cost(5),
    )
    .unwrap();
    g
}

#[test]
fn a_cold_build_runs_everything_and_produces_the_right_bytes() {
    let root = tmp("cold");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter.clone()));
    let healer = DefaultHealer::default();
    let g = program(&store, b"alpha", b"beta");

    let report = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();

    assert!(report.success(), "{}", report.json());
    assert_eq!(report.built, 3);
    assert_eq!(report.cache_hits, 0);
    assert_eq!(counter.count(), 3, "every action must actually run on a cold cache");

    // The linked output is the concatenation of the two uppercased sources.
    let link = report.actions.iter().find(|a| a.name == "link").unwrap();
    let Outcome::Built { key, .. } = &link.outcome else { panic!("expected Built") };
    let result = store.actions.get(&Digest(key.clone())).unwrap();
    let bytes = store.cas.get(&result.outputs["prog"]).unwrap();
    assert_eq!(bytes, b"ALPHABETA");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rebuilding_unchanged_sources_does_no_work_at_all() {
    let root = tmp("warm");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter.clone()));
    let healer = DefaultHealer::default();

    let g = program(&store, b"alpha", b"beta");
    Scheduler::new(&store, &exec, &healer).build(&g).unwrap();
    assert_eq!(counter.count(), 3);

    // Same graph, same store, fresh scheduler.
    let g2 = program(&store, b"alpha", b"beta");
    let report = Scheduler::new(&store, &exec, &healer).build(&g2).unwrap();

    assert!(report.success());
    assert_eq!(report.cache_hits, 3, "a no-op rebuild must be entirely cache hits");
    assert_eq!(report.built, 0);
    assert_eq!(counter.count(), 3, "no tool may run a second time");
    assert_eq!(report.work_done, 0);
    assert!((report.cache_hit_ratio() - 1.0).abs() < f64::EPSILON);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn editing_one_source_rebuilds_exactly_it_and_its_dependents() {
    let root = tmp("incremental");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter.clone()));
    let healer = DefaultHealer::default();

    Scheduler::new(&store, &exec, &healer).build(&program(&store, b"alpha", b"beta")).unwrap();
    let after_cold = counter.count();

    // Edit only b.
    let g = program(&store, b"alpha", b"BETA-EDITED");
    let report = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();

    assert!(report.success(), "{}", report.json());
    // compile:a is untouched; compile:b and link must rerun.
    assert_eq!(report.cache_hits, 1, "the unaffected compile must be reused");
    assert_eq!(report.built, 2, "the edited compile and the link must rebuild");
    assert_eq!(counter.count(), after_cold + 2);

    let by_name = |n: &str| {
        report.actions.iter().find(|a| a.name == n).map(|a| a.outcome.clone()).unwrap()
    };
    assert!(matches!(by_name("compile:a"), Outcome::CacheHit { .. }));
    assert!(matches!(by_name("compile:b"), Outcome::Built { .. }));
    assert!(matches!(by_name("link"), Outcome::Built { .. }), "a changed input must invalidate downstream");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failing_action_skips_only_its_dependents() {
    let root = tmp("failure");
    let store = Store::open(&root);
    let mut r = ToolRegistry::new();
    r.register("compile@1", |action, _| {
        // b fails deterministically; a succeeds.
        if action.name.ends_with(":b") {
            return Err(ExecError::Deterministic { exit_code: 1, stderr: "syntax error".into() });
        }
        let mut out = ToolOutput::new();
        for o in &action.outputs {
            out.outputs.insert(o.clone(), b"ok".to_vec());
        }
        Ok(out)
    });
    r.register("link@1", |_, _| panic!("link must never run when a dependency failed"));

    let exec = LocalExecutor::new("w1", Platform::any(), r);
    let healer = DefaultHealer::default();
    let report = Scheduler::new(&store, &exec, &healer)
        .build(&program(&store, b"alpha", b"beta"))
        .unwrap();

    assert!(!report.success());
    assert_eq!(report.failed, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.built, 1, "the independent action still builds — agents want every problem at once");

    let link = report.actions.iter().find(|a| a.name == "link").unwrap();
    match &link.outcome {
        Outcome::Skipped { because } => assert!(because.contains("compile:b"), "the cause must be named: {because}"),
        other => panic!("expected Skipped, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_transient_failure_heals_and_the_build_succeeds() {
    let root = tmp("transient");
    let store = Store::open(&root);
    let attempts = Arc::new(AtomicUsize::new(0));
    let a2 = attempts.clone();

    let mut r = ToolRegistry::new();
    r.register("flaky@1", move |action, _| {
        // Fail once, then succeed — a worker that briefly vanished.
        if a2.fetch_add(1, Ordering::Relaxed) == 0 {
            return Err(ExecError::Transient("worker vanished".into()));
        }
        let mut out = ToolOutput::new();
        for o in &action.outputs {
            out.outputs.insert(o.clone(), b"recovered".to_vec());
        }
        Ok(out)
    });

    let exec = LocalExecutor::new("w1", Platform::any(), r);
    let healer = DefaultHealer::default();
    let mut g = ActionGraph::new();
    g.add(Action::new("flaky", "flaky@1").output("out")).unwrap();

    let report = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();

    assert!(report.success(), "{}", report.json());
    assert_eq!(report.healed, 1);
    assert_eq!(report.heal_events.len(), 1, "healing must be recorded, not silent");
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert!(report.fitness().stability < 1.0, "healing must cost stability so it stays visible");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cache_corruption_is_detected_and_repaired() {
    let root = tmp("corruption");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter.clone()));
    let healer = DefaultHealer::default();

    let g = program(&store, b"alpha", b"beta");
    let first = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();
    assert!(first.success());

    // Rot the compile:a output in place, keeping its filename.
    let a = first.actions.iter().find(|x| x.name == "compile:a").unwrap();
    let Outcome::Built { key, .. } = &a.outcome else { panic!() };
    let result = store.actions.get(&Digest(key.clone())).unwrap();
    let digest = &result.outputs["a.o"];
    let (shard, rest) = digest.0.split_at(2);
    std::fs::write(root.join("cas").join(shard).join(rest), b"CORRUPTED").unwrap();

    let g2 = program(&store, b"alpha", b"beta");
    let report = Scheduler::new(&store, &exec, &healer).build(&g2).unwrap();

    assert!(report.success(), "corruption must be repaired, not fatal: {}", report.json());
    assert!(!report.heal_events.is_empty(), "the repair must be recorded");
    assert!(
        report.heal_events.iter().any(|e| e.failure.contains("corrupt")),
        "the failure must be named as corruption: {:?}",
        report.heal_events
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn gpu_work_routes_to_a_gpu_worker_and_cpu_work_runs_anywhere() {
    let root = tmp("fleet");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());

    let cpu = Box::new(LocalExecutor::new(
        "cpu-node",
        Platform::host("linux", "x86_64"),
        tools(counter.clone()),
    ));
    let gpu = Box::new(LocalExecutor::new(
        "gpu-node",
        Platform::host("linux", "x86_64").with_accelerator("cuda"),
        tools(counter.clone()),
    ));
    let fleet = PoolExecutor::new("fleet", vec![cpu, gpu]);
    let healer = DefaultHealer::default();

    let src = store.cas.put(b"kernel").unwrap();
    let mut g = ActionGraph::new();
    g.add(
        Action::new("autotune", "compile@1")
            .input("k.mg", src.clone())
            .output("k.cubin")
            .platform(Platform::host("linux", "x86_64").with_accelerator("cuda")),
    )
    .unwrap();
    g.add(Action::new("portable", "compile@1").input("p.mg", src).output("p.o"))
        .unwrap();

    let report = Scheduler::new(&store, &fleet, &healer).build(&g).unwrap();
    assert!(report.success(), "{}", report.json());

    let autotune = &g.actions[0];
    let portable = &g.actions[1];
    assert_eq!(fleet.capable_for(autotune), vec!["gpu-node"], "device-pinned work has one home");
    assert_eq!(fleet.capable_for(portable).len(), 2, "portable work runs on either node");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_accelerator_falls_back_only_when_opted_in() {
    let root = tmp("fallback");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    // A CPU-only fleet asked to do CUDA work.
    let exec = LocalExecutor::new("cpu-only", Platform::host("linux", "x86_64"), tools(counter));
    let healer = DefaultHealer::default();

    let src = store.cas.put(b"kernel").unwrap();
    let pinned = Action::new("autotune", "compile@1")
        .input("k.mg", src.clone())
        .output("k.bin")
        .platform(Platform::host("linux", "x86_64").with_accelerator("cuda"));

    let mut g = ActionGraph::new();
    g.add(pinned.clone()).unwrap();
    let strict = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();
    assert!(!strict.success(), "an autotune must not silently run on the wrong device");

    let mut g2 = ActionGraph::new();
    g2.add(pinned.env(FALLBACK_KEY, "1")).unwrap();
    let lenient = Scheduler::new(&store, &exec, &healer).build(&g2).unwrap();
    assert!(lenient.success(), "{}", lenient.json());
    assert_eq!(lenient.healed, 1, "the fallback is a heal, and is recorded as one");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn plan_predicts_the_build_without_running_it() {
    let root = tmp("plan");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter.clone()));
    let healer = DefaultHealer::default();
    let g = program(&store, b"alpha", b"beta");
    let sched = Scheduler::new(&store, &exec, &healer);

    let before: serde_json::Value = serde_json::from_str(&sched.plan(&g).unwrap()).unwrap();
    assert_eq!(before["actions"], 3);
    assert_eq!(before["already_cached"], 0);
    assert_eq!(before["would_execute"], 3);
    assert_eq!(counter.count(), 0, "planning must not execute anything");

    sched.build(&g).unwrap();

    // After building, the leaf actions are known-cached. `link` is not, because
    // its real key depends on upstream digests only known once they are built —
    // the plan reports what is knowable without running, and no more.
    let after: serde_json::Value = serde_json::from_str(&sched.plan(&g).unwrap()).unwrap();
    assert!(
        after["already_cached"].as_u64().unwrap() >= 2,
        "planning after a build must see the cached leaves: {after}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fitness_is_ordered_by_correctness_first() {
    let root = tmp("fitness");
    let store = Store::open(&root);
    let counter = Arc::new(RunCounter::default());
    let exec = LocalExecutor::new("w1", Platform::any(), tools(counter));
    let healer = DefaultHealer::default();

    let g = program(&store, b"alpha", b"beta");
    let cold = Scheduler::new(&store, &exec, &healer).build(&g).unwrap();
    let warm = Scheduler::new(&store, &exec, &healer).build(&program(&store, b"alpha", b"beta")).unwrap();

    assert_eq!(cold.fitness().correctness, 1.0);
    assert!(
        warm.fitness().composite() > cold.fitness().composite(),
        "a fully-cached build must score higher than a cold one"
    );

    // A broken build must never outrank a working one, however fast it was.
    let mut broken = warm.clone();
    broken.failed = 1;
    broken.actions[0].outcome = Outcome::Failed { key: "x".into(), error: "boom".into() };
    assert!(
        broken.fitness().composite() < cold.fitness().composite(),
        "correctness must dominate the composite"
    );

    let _ = std::fs::remove_dir_all(&root);
}
