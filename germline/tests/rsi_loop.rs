//! The closed loop, end to end.
//!
//! This is the integration test for the *architecture* rather than for any one
//! module: it drives a real cycle through both subsystems and asserts that the
//! seams line up.
//!
//! ```text
//!   variation ──▶ directed ──▶ ribosome ──▶ cycle ──▶ gate ──▶ attest
//!    propose       rank        build &      phases   verdict    prove
//!                              measure                          │
//!                                                               ▼
//!   supervisor ◀── lineage ◀── journal ◀───────────────── hand off
//!    demote        rollback     record
//! ```
//!
//! Everything a person would have to do by hand is done here explicitly, which
//! is the point: the loop is *drivable*, and nothing runs on its own.

use germline::attest::Attestor;
use germline::cycle::{fitness_from_build, Authority, Cycle, Phase};
use germline::directed::{CandidateSpec, DirectedSearch, FitnessPredictor, Prediction};
use germline::gate::{Episode, PromotionGate};
use germline::journal::{Entry, Journal};
use germline::lineage::Lineage;
use germline::supervisor::{HealthSample, SupervisionPolicy, Supervisor};
use germline::variation::{propose, VariationPlan};
use germline::{
    EvalSuite, FitnessVector, Generation, Measurement, Status, SuiteKind,
};
use ribosome::cas::Store;
use ribosome::exec::{LocalExecutor, ToolOutput, ToolRegistry};
use ribosome::graph::ActionGraph;
use ribosome::heal::DefaultHealer;
use ribosome::sched::Scheduler;
use ribosome::{Action, Digest, Platform};
use std::path::PathBuf;

const SUITE: &[u8] = b"heldout-suite-v1";
const EVALUATOR: &str = "independent-harness";

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rsi-loop-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn attestor() -> Attestor {
    Attestor::new(EVALUATOR, b"harness signing key".to_vec())
}

fn suite() -> EvalSuite {
    EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE))
}

fn episode(shadow: u32) -> Episode {
    Episode::open(
        PromotionGate { min_shadow_successes: shadow, ..PromotionGate::default() },
        Digest::of(SUITE),
        EVALUATOR,
    )
}

fn measurement(cap: f64, safety: f64) -> Measurement {
    Measurement {
        suite: EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE)),
        fitness: FitnessVector::new()
            .with("capability", cap)
            .with("safety", safety)
            .with("correctness", 0.98),
        evaluator: EVALUATOR.into(),
    }
}

/// Runs a candidate's work through the build engine and returns its build
/// fitness — the Ribosome→Germline seam.
fn build_and_measure(store: &Store, source: &[u8]) -> ribosome::sched::BuildReport {
    let mut tools = ToolRegistry::new();
    tools.register("compile@1", |action, inputs| {
        let src = inputs.values().next().cloned().unwrap_or_default();
        let out_bytes = String::from_utf8_lossy(&src).to_uppercase().into_bytes();
        let mut out = ToolOutput::new();
        for o in &action.outputs {
            out.outputs.insert(o.clone(), out_bytes.clone());
        }
        Ok(out)
    });
    let exec = LocalExecutor::new("worker", Platform::any(), tools);
    let healer = DefaultHealer::default();

    let d = store.cas.put(source).unwrap();
    let mut g = ActionGraph::new();
    g.add(Action::new("compile", "compile@1").input("src.mg", d).output("out.o").cost(10)).unwrap();
    Scheduler::new(store, &exec, &healer).build(&g).unwrap()
}

struct MeanPredictor;
impl FitnessPredictor for MeanPredictor {
    fn predict(&self, c: &CandidateSpec) -> Prediction {
        let m = c.genome.iter().sum::<f64>() / c.genome.len().max(1) as f64;
        Prediction {
            predicted: FitnessVector::new().with("capability", m),
            self_reported_confidence: 0.8,
        }
    }
}

#[test]
fn the_whole_loop_runs_from_proposal_to_authority() {
    let root = tmp("full");
    let store = Store::open(root.join("store"));
    let mut journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let mut lineage = Lineage::new();

    // --- incumbent
    let champ_id = lineage.next_id();
    lineage.add(
        Generation::new(champ_id, Digest::of(b"champion")).measured(measurement(0.70, 0.95)),
    );
    lineage.promote(champ_id, episode(0).gate_digest, EVALUATOR).unwrap();

    // --- propose: deterministic variation over a scored population
    let population = vec![
        (vec![0.6, 0.6, 0.6], 0.60),
        (vec![0.7, 0.7, 0.7], 0.70),
        (vec![0.4, 0.4, 0.4], 0.40),
    ];
    let seed = 0x5EED_u64;
    let candidates = propose(&population, VariationPlan::default(), seed);
    assert!(!candidates.is_empty());
    assert_eq!(
        candidates,
        propose(&population, VariationPlan::default(), seed),
        "the proposal round must be re-derivable from its seed"
    );

    // --- rank: an unproven predictor keeps the search broad
    let mut search = DirectedSearch::new(&MeanPredictor, 2);
    let ranked = search.select(candidates.clone());
    assert_eq!(ranked.len(), candidates.len(), "no track record yet, so no narrowing");

    // --- build: the Ribosome seam supplies a measured axis
    let report = build_and_measure(&store, b"candidate source");
    assert!(report.success(), "{}", report.json());
    let build_fitness = fitness_from_build(&report);
    assert_eq!(build_fitness.get("correctness"), Some(1.0));

    // Feed the measurement back so the predictor's trust reflects reality.
    search.observe(&ranked[0].prediction.predicted, &build_fitness);
    assert_eq!(search.calibration.samples(), 1);

    // --- cycle: propose → evaluate → shadow → adjudicate → commit
    let cand_id = lineage.next_id();
    let candidate = Generation::new(cand_id, Digest::of(b"successor-artifact"))
        .parent(champ_id)
        .note(format!("from {} via seed {seed:#x}", ranked[0].candidate.id));

    let mut cycle = Cycle::propose(candidate, seed, "default", &mut journal).unwrap();
    cycle.evaluate(measurement(0.80, 0.95), &mut journal).unwrap();
    for _ in 0..8 {
        cycle.shadow(&HealthSample::ok()).unwrap();
    }

    let verdict = cycle
        .adjudicate(&episode(8), &lineage, &attestor(), &|_| true, &mut journal)
        .unwrap();
    assert!(verdict.approved(), "{:?}", verdict.reasons());

    let promoted = cycle
        .commit(
            &mut lineage,
            &attestor(),
            Authority::Operator { who: "adam".into() },
            &mut journal,
        )
        .unwrap();

    assert_eq!(cycle.phase(), Phase::Promoted);
    assert_eq!(lineage.champion().unwrap().id, promoted);
    assert_eq!(lineage.get(champ_id).unwrap().status, Status::Retired);

    // --- the record survives and is intact
    assert!(journal.verify().unwrap() >= 5);
    let head = journal.head().unwrap().clone();
    let reopened = Journal::open(root.join("journal.jsonl")).unwrap();
    assert_eq!(reopened.head(), Some(&head), "the audit trail survives a restart");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_promoted_successor_that_fails_is_demoted_and_the_whole_story_is_on_record() {
    let root = tmp("failure");
    let mut journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let mut lineage = Lineage::new();
    let materialized = |_: &Digest| true;

    let champ_id = lineage.next_id();
    lineage.add(
        Generation::new(champ_id, Digest::of(b"champion")).measured(measurement(0.70, 0.95)),
    );
    lineage.promote(champ_id, episode(0).gate_digest, EVALUATOR).unwrap();

    // Promote a successor through the full cycle.
    let cand_id = lineage.next_id();
    let candidate = Generation::new(cand_id, Digest::of(b"successor")).parent(champ_id);
    let mut cycle = Cycle::propose(candidate, 1, "default", &mut journal).unwrap();
    cycle.evaluate(measurement(0.80, 0.95), &mut journal).unwrap();
    cycle
        .adjudicate(&episode(0), &lineage, &attestor(), &materialized, &mut journal)
        .unwrap();
    cycle
        .commit(&mut lineage, &attestor(), Authority::Operator { who: "adam".into() }, &mut journal)
        .unwrap();
    assert_eq!(lineage.champion().unwrap().id, cand_id);

    // It takes over and starts failing real work.
    let mut sup = Supervisor::new(SupervisionPolicy::default(), 0.80);
    let mut failure = None;
    for _ in 0..5 {
        if let Some(f) = sup.observe(HealthSample::failed()) {
            failure = Some(f);
            break;
        }
    }
    let failure = failure.expect("a champion failing every unit must be caught");

    journal
        .append(Entry::Failure { generation: cand_id, mode: failure.to_string() })
        .unwrap();
    let restored = lineage.demote_champion(&failure.to_string(), &materialized).unwrap();
    let event = lineage.events().last().cloned().unwrap();
    journal.append(Entry::Succession { event }).unwrap();

    assert_eq!(restored, champ_id, "authority returns to the predecessor");
    assert_eq!(lineage.get(cand_id).unwrap().status, Status::Quarantined);

    // The journal explains the whole incident, and has not been edited.
    assert!(journal.verify().is_ok());
    let entries = journal.replay().unwrap();
    assert!(entries.iter().any(|r| matches!(&r.entry, Entry::Proposed { .. })));
    assert!(entries.iter().any(|r| matches!(&r.entry, Entry::Failure { .. })));
    assert!(entries.iter().any(|r| matches!(&r.entry, Entry::Succession { .. })));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn tampering_with_the_record_after_the_fact_is_detectable() {
    let root = tmp("tamper");
    let path = root.join("journal.jsonl");
    let mut journal = Journal::open(&path).unwrap();
    let mut lineage = Lineage::new();

    let champ_id = lineage.next_id();
    lineage.add(
        Generation::new(champ_id, Digest::of(b"champion")).measured(measurement(0.70, 0.95)),
    );
    lineage.promote(champ_id, episode(0).gate_digest, EVALUATOR).unwrap();

    let cand_id = lineage.next_id();
    let candidate = Generation::new(cand_id, Digest::of(b"successor")).parent(champ_id);
    let mut cycle = Cycle::propose(candidate, 42, "default", &mut journal).unwrap();
    cycle.evaluate(measurement(0.80, 0.95), &mut journal).unwrap();
    cycle.adjudicate(&episode(0), &lineage, &attestor(), &|_| true, &mut journal).unwrap();
    cycle
        .commit(&mut lineage, &attestor(), Authority::Operator { who: "adam".into() }, &mut journal)
        .unwrap();
    assert!(journal.verify().is_ok());

    // Rewrite the seed in the proposal record — "this candidate came from
    // somewhere else".
    let mut lines: Vec<String> =
        std::fs::read_to_string(&path).unwrap().lines().map(String::from).collect();
    lines[0] = lines[0].replace("\"seed\":42", "\"seed\":99");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();

    assert!(
        Journal::open(&path).unwrap().verify().is_err(),
        "rewriting a candidate's provenance must break the chain"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// ------------------------------------- the loop against a real workload

/// The whole thing, unattended: a bounded runner driving architecture search
/// that is evaluated by actual builds through Ribosome. No stubs in the path.
#[test]
fn the_runner_drives_a_real_workload_to_a_bounded_stop() {
    use germline::runner::{Halt, Runner, RunnerPolicy, Workload};
    use germline::supervisor::SupervisionPolicy;
    use germline::workload::BuildWorkload;

    let root = tmp("real-workload");
    let mut journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let mut workload = BuildWorkload::new(Store::open(root.join("store")));

    // Seed champion: a shallow architecture, actually built.
    let seed_artifact = workload
        .materialize(&CandidateSpec::new("seed", vec![0.0, 0.0, 1.0]))
        .unwrap();
    let mut lineage = Lineage::new();
    let champ = lineage.next_id();
    lineage.add(Generation::new(champ, seed_artifact).measured(Measurement {
        suite: suite(),
        fitness: FitnessVector::new()
            .with("capability", 0.05)
            .with("safety", 0.95)
            .with("compactness", 0.30)
            .with("correctness", 1.0),
        evaluator: EVALUATOR.into(),
    }));
    lineage.promote(champ, episode(0).gate_digest, EVALUATOR).unwrap();

    let policy = RunnerPolicy {
        name: "architecture-search".into(),
        max_cycles: 6,
        max_consecutive_refusals: 3,
        shadow_runs: 2,
        supervision: SupervisionPolicy { window: 3, ..SupervisionPolicy::default() },
        ..RunnerPolicy::default()
    };
    let ep = Episode::open(
        PromotionGate {
            primary_axis: "capability".into(),
            min_improvement: 0.01,
            guard_axes: vec!["safety".into()],
            guard_tolerance: 0.01,
            min_shadow_successes: 2,
        },
        Digest::of(SUITE),
        EVALUATOR,
    );
    let at = attestor();
    let mut runner = Runner::new(policy, &ep, &at, suite(), 0xC0FFEE);

    let report = runner.run(&mut lineage, &mut journal, &mut workload);

    // It stopped on its own, for a stated reason.
    assert!(
        matches!(
            report.halt,
            Halt::BudgetExhausted { .. } | Halt::SearchStalled { .. } | Halt::Demoted { .. }
        ),
        "the run must reach a named stop, not wander: {:?}",
        report.halt
    );
    assert!(!report.cycles.is_empty());

    // Real builds happened, and the record is intact and anchored.
    assert!(!workload.history.is_empty(), "architectures were actually built");
    assert!(journal.verify().is_ok(), "the unattended run's record must be intact");
    assert_eq!(report.journal_head, journal.head().cloned());

    // Whatever holds authority is materialized — the fallback story is real.
    let champion = lineage.champion().unwrap();
    assert!(
        workload.materialized(&champion.artifact),
        "the current champion must actually be runnable"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_unattended_run_that_breaks_falls_back_to_something_runnable() {
    use germline::runner::{Halt, Runner, RunnerPolicy, Workload};
    use germline::supervisor::SupervisionPolicy;
    use germline::workload::BuildWorkload;

    let root = tmp("real-demote");
    let mut journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let mut workload = BuildWorkload::new(Store::open(root.join("store")));

    let seed_artifact = workload
        .materialize(&CandidateSpec::new("seed", vec![0.0, 0.0, 1.0]))
        .unwrap();
    let mut lineage = Lineage::new();
    let champ = lineage.next_id();
    lineage.add(Generation::new(champ, seed_artifact.clone()).measured(Measurement {
        suite: suite(),
        fitness: FitnessVector::new()
            .with("capability", 0.05)
            .with("safety", 0.95)
            .with("compactness", 0.30)
            .with("correctness", 1.0),
        evaluator: EVALUATOR.into(),
    }));
    lineage.promote(champ, episode(0).gate_digest, EVALUATOR).unwrap();

    // Anything promoted will fail in production.
    workload.champion_fails = true;

    let ep = Episode::open(
        PromotionGate {
            primary_axis: "capability".into(),
            min_improvement: 0.01,
            guard_axes: vec!["safety".into()],
            guard_tolerance: 0.01,
            min_shadow_successes: 0,
        },
        Digest::of(SUITE),
        EVALUATOR,
    );
    let at = attestor();
    let mut runner = Runner::new(
        RunnerPolicy {
            max_cycles: 6,
            shadow_runs: 0,
            supervision: SupervisionPolicy { window: 3, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        },
        &ep,
        &at,
        suite(),
        7,
    );

    let report = runner.run(&mut lineage, &mut journal, &mut workload);

    if let Halt::Demoted { fell_back_to, .. } = &report.halt {
        assert_eq!(lineage.champion().unwrap().id, *fell_back_to);
        assert_eq!(*fell_back_to, champ, "authority returned to the seed");
        assert!(
            workload.materialized(&seed_artifact),
            "and the thing it returned to is actually runnable"
        );
        assert!(report.cycles.len() == 1, "it stopped rather than proposing another");
    } else {
        // If nothing was promotable the run stalls instead — also a named stop.
        assert!(
            matches!(report.halt, Halt::SearchStalled { .. } | Halt::BudgetExhausted { .. }),
            "unexpected halt: {:?}",
            report.halt
        );
    }
    assert!(journal.verify().is_ok());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_gate_cannot_be_bypassed_by_driving_the_cycle_out_of_order() {
    let root = tmp("bypass");
    let mut journal = Journal::open(root.join("journal.jsonl")).unwrap();
    let mut lineage = Lineage::new();

    let champ_id = lineage.next_id();
    lineage.add(
        Generation::new(champ_id, Digest::of(b"champion")).measured(measurement(0.70, 0.95)),
    );
    lineage.promote(champ_id, episode(0).gate_digest, EVALUATOR).unwrap();

    // A candidate that would clearly fail the gate: safety down 0.10.
    let cand_id = lineage.next_id();
    let candidate = Generation::new(cand_id, Digest::of(b"unsafe-successor")).parent(champ_id);
    let mut cycle = Cycle::propose(candidate, 7, "default", &mut journal).unwrap();

    // Straight to commit.
    assert!(
        cycle
            .commit(
                &mut lineage,
                &attestor(),
                Authority::Operator { who: "impatient".into() },
                &mut journal
            )
            .is_err(),
        "there must be no path to authority that skips adjudication"
    );

    // And going through the gate honestly refuses it.
    cycle.evaluate(measurement(0.95, 0.85), &mut journal).unwrap();
    let v = cycle
        .adjudicate(&episode(0), &lineage, &attestor(), &|_| true, &mut journal)
        .unwrap();
    assert!(!v.approved(), "a large capability gain must not buy a safety regression");
    assert_eq!(lineage.champion().unwrap().id, champ_id, "the incumbent kept authority");

    let _ = std::fs::remove_dir_all(&root);
}
