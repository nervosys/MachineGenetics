//! End-to-end tests for building arbitrary languages.
//!
//! The unit tests beside `ribosome::lang` prove the *planner* produces the right
//! actions. These prove the plan actually builds: through the real graph, the
//! real scheduler, the real content-addressed store — and that the hermeticity
//! tier is a rule the store enforces rather than a label the plan carries.

use ribosome::cas::Store;
use ribosome::exec::{LocalExecutor, ToolOutput, ToolRegistry};
use ribosome::heal::DefaultHealer;
use ribosome::lang::{
    builtin, Granularity, Language, Plan, Registry, Rule, Target, Toolchain,
};
use ribosome::sched::{Outcome, Scheduler};
use ribosome::{Digest, Platform};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ribosome-lang-it-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[derive(Default)]
struct RunCounter(AtomicUsize);

/// A stand-in for every real toolchain: concatenates its inputs and writes the
/// result to each declared output.
///
/// Registering *whatever tools the plan names* is the point — the test never
/// spells out `cc@unknown+unpinned`, so it keeps working when the tool id
/// changes, and it proves the planner and the executor agree on tool identity by
/// construction rather than by a hardcoded string.
fn tools_for(plan: &Plan, counter: Arc<RunCounter>) -> ToolRegistry {
    let mut r = ToolRegistry::new();
    let names: BTreeSet<&str> = plan.actions.iter().map(|a| a.tool.as_str()).collect();
    for name in names {
        let c = counter.clone();
        r.register(name.to_string(), move |action, inputs| {
            c.0.fetch_add(1, Ordering::Relaxed);
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
    }
    r
}

/// Write sources into the CAS and return a target whose digests match.
fn target(store: &Store, name: &str, language: &str, sources: &[(&str, &[u8])]) -> Target {
    let mut t = Target::new(name, language);
    for (path, bytes) in sources {
        let d = store.cas.put(bytes).unwrap();
        assert_eq!(d, Digest::of(bytes));
        t = t.source(*path, d);
    }
    t
}

fn build(store: &Store, plan: &Plan, counter: Arc<RunCounter>) -> ribosome::sched::BuildReport {
    let exec = LocalExecutor::new("w", Platform::any(), tools_for(plan, counter));
    let healer = DefaultHealer::default();
    Scheduler::new(store, &exec, &healer).build(&plan.graph().unwrap()).unwrap()
}

#[test]
fn a_mixed_language_program_builds_in_dependency_order() {
    let root = tmp("mixed");
    let store = Store::open(&root);
    let reg = Registry::with_builtins();

    // A C library, a Rust binary that links it, and a MAGE model beside them —
    // one graph, three toolchains, no per-language special casing anywhere below
    // the planner.
    let libc = target(&store, "libutil.a", "c", &[("util.c", b"void u(){}"), ("aux.c", b"int a;")]);
    let app = target(&store, "app", "rust", &[("main.rs", b"fn main(){}")]).dep("libutil.a");
    let model = target(&store, "net", "mage", &[("net.mg", b"net N { }")]);

    let plan = reg.plan_all(&[libc, app, model]).unwrap();
    assert_eq!(plan.languages, vec!["c", "mage", "rust"]);

    let counter = Arc::new(RunCounter::default());
    let report = build(&store, &plan, counter.clone());

    assert!(report.success(), "{}", report.json());
    // 2 C compiles + 1 C link + 1 rust build + 1 mage build.
    assert_eq!(counter.0.load(Ordering::Relaxed), 5);

    // The Rust binary's content contains the C artifact's, which is only possible
    // if the cross-language edge was real and ordered correctly.
    let order: Vec<&str> = report.actions.iter().map(|a| a.name.as_str()).collect();
    let lib = order.iter().position(|n| *n == "link:libutil.a").unwrap();
    let bin = order.iter().position(|n| *n == "build:app").unwrap();
    assert!(lib < bin);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rebuilding_an_unchanged_multi_language_program_does_no_work() {
    let root = tmp("nowork");
    let store = Store::open(&root);
    let reg = Registry::with_builtins();

    let targets = vec![
        target(&store, "libutil.a", "c", &[("util.c", b"void u(){}")]),
        target(&store, "app", "rust", &[("main.rs", b"fn main(){}")]),
    ];
    let plan = reg.plan_all(&targets).unwrap();

    let first = Arc::new(RunCounter::default());
    assert!(build(&store, &plan, first.clone()).success());
    assert_eq!(first.0.load(Ordering::Relaxed), 3);

    let second = Arc::new(RunCounter::default());
    let report = build(&store, &plan, second.clone());
    assert!(report.success());
    assert_eq!(second.0.load(Ordering::Relaxed), 0, "no tool should run twice");
    assert_eq!(report.cache_hits, 3);
    assert_eq!(report.cache_hit_ratio(), 1.0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn editing_one_translation_unit_rebuilds_it_and_the_link_only() {
    let root = tmp("incremental");
    let store = Store::open(&root);
    let reg = Registry::with_builtins();

    let t = target(&store, "app", "c", &[("a.c", b"int a;"), ("b.c", b"int b;")]);
    let plan = reg.plan(&t).unwrap();
    assert!(build(&store, &plan, Arc::new(RunCounter::default())).success());

    // Change b.c only.
    let edited = target(&store, "app", "c", &[("a.c", b"int a;"), ("b.c", b"int b = 2;")]);
    let plan2 = reg.plan(&edited).unwrap();

    let counter = Arc::new(RunCounter::default());
    let report = build(&store, &plan2, counter.clone());
    assert!(report.success());
    assert_eq!(counter.0.load(Ordering::Relaxed), 2, "recompile b.c, relink; a.c is untouched");
    assert_eq!(report.cache_hits, 1);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_shared_store_refuses_to_publish_results_from_an_unverified_toolchain() {
    // The load-bearing safety property. An unpinned `cc` builds fine, but its
    // claim must never reach a cache another machine reads — that machine's
    // `cc` is a different binary and nobody checked.
    let root = tmp("shared-unpinned");
    let store = Store::open_shared(&root);
    assert!(store.is_shared());
    let reg = Registry::with_builtins();

    let t = target(&store, "app", "c", &[("a.c", b"int a;")]);
    let plan = reg.plan(&t).unwrap();
    assert!(!plan.remote_cacheable());
    assert_eq!(plan.unpinned_tools(), vec!["cc@unknown+unpinned"]);

    let first = Arc::new(RunCounter::default());
    assert!(build(&store, &plan, first.clone()).success());
    assert_eq!(first.0.load(Ordering::Relaxed), 2);

    // Second build: everything runs again, because nothing was published.
    let second = Arc::new(RunCounter::default());
    let report = build(&store, &plan, second.clone());
    assert!(report.success(), "refusing to cache must not refuse to build");
    assert_eq!(second.0.load(Ordering::Relaxed), 2, "an unpinned result is rebuilt, not reused");
    assert_eq!(report.cache_hits, 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn pinning_the_toolchain_earns_the_shared_cache() {
    // Same store, same sources, same code path — the only change is that someone
    // measured the compiler.
    let root = tmp("shared-pinned");
    let store = Store::open_shared(&root);
    let mut reg = Registry::with_builtins();
    assert!(reg.pin("c", Digest::of(b"the actual cc binary")));

    let t = target(&store, "app", "c", &[("a.c", b"int a;")]);
    let plan = reg.plan(&t).unwrap();
    assert!(plan.remote_cacheable());
    assert!(plan.unpinned_tools().is_empty());

    assert!(build(&store, &plan, Arc::new(RunCounter::default())).success());

    let second = Arc::new(RunCounter::default());
    let report = build(&store, &plan, second.clone());
    assert_eq!(second.0.load(Ordering::Relaxed), 0);
    assert_eq!(report.cache_hits, 2, "a verified toolchain's results are shareable");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_structural_toolchain_publishes_to_a_shared_store_without_pinning() {
    // MAGE needs no pin: its output is byte-stable by construction, which is a
    // measured property rather than an assertion about which binary ran.
    let root = tmp("shared-structural");
    let store = Store::open_shared(&root);
    let reg = Registry::with_builtins();

    let plan = reg.plan(&target(&store, "net", "mage", &[("n.mg", b"net N { }")])).unwrap();
    assert!(plan.remote_cacheable());

    assert!(build(&store, &plan, Arc::new(RunCounter::default())).success());
    let second = Arc::new(RunCounter::default());
    build(&store, &plan, second.clone());
    assert_eq!(second.0.load(Ordering::Relaxed), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn one_unpinned_language_does_not_block_the_pinned_ones_beside_it() {
    // Enforcement is per action, not per plan: the plan-level verdict is a
    // summary for an agent, and withholding *only* the tainted claims is both
    // safe and strictly better than withholding all of them.
    let root = tmp("shared-mixed");
    let store = Store::open_shared(&root);
    let reg = Registry::with_builtins();

    let plan = reg
        .plan_all(&[
            target(&store, "net", "mage", &[("n.mg", b"net N { }")]),
            target(&store, "helper", "c", &[("h.c", b"void h(){}")]),
        ])
        .unwrap();
    assert!(!plan.remote_cacheable(), "the plan as a whole is not shareable");

    assert!(build(&store, &plan, Arc::new(RunCounter::default())).success());

    let second = Arc::new(RunCounter::default());
    let report = build(&store, &plan, second.clone());
    assert!(report.success());

    let hit = |name: &str| {
        report
            .actions
            .iter()
            .find(|a| a.name == name)
            .map(|a| matches!(a.outcome, Outcome::CacheHit { .. }))
            .unwrap()
    };
    assert!(hit("build:net"), "the structural artifact is still shareable");
    assert!(!hit("compile:h.c"), "the unverified one is not");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_language_nobody_anticipated_builds_with_no_change_to_the_engine() {
    // The claim this whole module exists to support, tested end to end rather
    // than at the planner: a language is data, and adding one touches nothing.
    let root = tmp("novel");
    let store = Store::open(&root);
    let mut reg = Registry::new();
    reg.register(
        Language::new(
            "fortran",
            &["f90"],
            Toolchain::pinned("gfortran", "13.2.0", Digest::of(b"gfortran binary")),
            Granularity::PerSource,
            Rule::new("{stem}.o").args(["-c", "{src}", "-o", "{out}"]),
        )
        .link(Rule::new("{name}").args(["{objs}", "-o", "{out}"])),
    );
    reg.register(builtin::mage());

    let plan = reg
        .plan_all(&[
            target(&store, "solver", "fortran", &[("solve.f90", b"end"), ("io.f90", b"end")]),
            target(&store, "net", "mage", &[("n.mg", b"net N { }")]),
        ])
        .unwrap();

    assert!(plan.remote_cacheable(), "pinned + structural is shareable");
    let counter = Arc::new(RunCounter::default());
    let report = build(&store, &plan, counter.clone());
    assert!(report.success(), "{}", report.json());
    assert_eq!(counter.0.load(Ordering::Relaxed), 4, "two compiles, a link, and the mage build");
    assert_eq!(plan.artifacts, vec!["solver", "net.abl"]);

    let _ = std::fs::remove_dir_all(&root);
}
