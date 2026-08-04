//! End-to-end succession scenarios.
//!
//! Each test is a full episode: propose by directed search, evaluate, adjudicate,
//! hand off, supervise, and — where it goes wrong — fall back. The unit tests
//! beside each module cover the pieces; these cover the loop closing.

use germline::directed::{
    CandidateSpec, DirectedSearch, FitnessPredictor, Prediction,
};
use germline::gate::{Episode, PromotionGate, RejectReason, Verdict};
use germline::lineage::Lineage;
use germline::supervisor::{FailureMode, HealthSample, SupervisionPolicy, Supervisor};
use germline::{
    EvalSuite, FitnessVector, Generation, GenerationId, Measurement, Status, SuiteKind,
};
use ribosome::Digest;
use std::collections::HashSet;

const SUITE_BYTES: &[u8] = b"heldout-suite-v1";
const EVALUATOR: &str = "independent-harness";

fn suite() -> EvalSuite {
    EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(SUITE_BYTES))
}

fn episode(gate: PromotionGate) -> Episode {
    Episode::open(gate, Digest::of(SUITE_BYTES), EVALUATOR)
}

fn gate_no_canary() -> PromotionGate {
    PromotionGate { min_shadow_successes: 0, ..PromotionGate::default() }
}

fn measurement(cap: f64, safety: f64, correctness: f64) -> Measurement {
    Measurement {
        suite: suite(),
        fitness: FitnessVector::new()
            .with("capability", cap)
            .with("safety", safety)
            .with("correctness", correctness),
        evaluator: EVALUATOR.into(),
    }
}

/// A store that knows which artifacts exist.
#[derive(Default)]
struct Storage(HashSet<String>);

impl Storage {
    fn keep(&mut self, d: &Digest) {
        self.0.insert(d.0.clone());
    }
    fn lose(&mut self, d: &Digest) {
        self.0.remove(&d.0);
    }
    fn checker(&self) -> impl Fn(&Digest) -> bool + '_ {
        move |d: &Digest| self.0.contains(&d.0)
    }
}

/// Seeds a lineage with a promoted champion.
fn seed(l: &mut Lineage, store: &mut Storage, m: Measurement) -> GenerationId {
    let id = l.next_id();
    let artifact = Digest::of(format!("model-{}", id.0).as_bytes());
    store.keep(&artifact);
    l.add(Generation::new(id, artifact).measured(m));
    l.promote(id, gate_no_canary().digest(), EVALUATOR).unwrap();
    id
}

fn challenge(
    l: &mut Lineage,
    store: &mut Storage,
    parent: GenerationId,
    m: Measurement,
) -> Generation {
    let id = l.next_id();
    let artifact = Digest::of(format!("model-{}", id.0).as_bytes());
    store.keep(&artifact);
    Generation::new(id, artifact).parent(parent).measured(m)
}

// ---------------------------------------------------------------- happy path

#[test]
fn a_successor_is_generated_promoted_and_takes_over() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    let challenger = challenge(&mut l, &mut store, champ, measurement(0.78, 0.95, 0.98));
    let ep = episode(gate_no_canary());
    let verdict = ep.adjudicate(&challenger, &l, 0, &store.checker());
    assert!(verdict.approved(), "{:?}", verdict.reasons());

    let Verdict::Promote { generation, gate, evaluator } = verdict else { unreachable!() };
    l.add(challenger);
    l.promote(generation, gate, &evaluator).unwrap();

    assert_eq!(l.champion().unwrap().id, generation, "authority transferred");
    assert_eq!(l.get(champ).unwrap().status, Status::Retired, "the predecessor stays available");
}

// ------------------------------------------------------- handoff then failure

#[test]
fn a_malfunctioning_successor_is_demoted_and_authority_returns() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    let challenger = challenge(&mut l, &mut store, champ, measurement(0.78, 0.95, 0.98));
    let cid = challenger.id;
    let ep = episode(gate_no_canary());
    assert!(ep.adjudicate(&challenger, &l, 0, &store.checker()).approved());
    l.add(challenger);
    l.promote(cid, ep.gate_digest.clone(), EVALUATOR).unwrap();

    // It takes over, and immediately starts failing real work.
    let mut sup = Supervisor::new(SupervisionPolicy::default(), 0.78);
    let mut failure = None;
    for _ in 0..5 {
        if let Some(f) = sup.observe(HealthSample::failed()) {
            failure = Some(f);
            break;
        }
    }
    let failure = failure.expect("a champion failing every unit must be caught");
    assert!(matches!(failure, FailureMode::Malfunction { .. }));

    let restored = l.demote_champion(&failure.to_string(), &store.checker()).unwrap();
    assert_eq!(restored, champ, "authority returns to the predecessor");
    assert_eq!(l.champion().unwrap().id, champ);
    assert_eq!(l.get(cid).unwrap().status, Status::Quarantined);
}

#[test]
fn a_declining_successor_is_demoted_even_though_nothing_crashes() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));
    let challenger = challenge(&mut l, &mut store, champ, measurement(0.78, 0.95, 0.98));
    let cid = challenger.id;
    l.add(challenger);
    l.promote(cid, gate_no_canary().digest(), EVALUATOR).unwrap();

    let policy = SupervisionPolicy { window: 10, ..SupervisionPolicy::default() };
    let mut sup = Supervisor::new(policy, 0.78);
    let mut failure = None;
    for _ in 0..10 {
        // Every unit succeeds; the work is just getting worse.
        failure = sup.observe(HealthSample::ok().with_heldout(FitnessVector::new().with("x", 0.60)));
    }
    assert!(
        matches!(failure, Some(FailureMode::FitnessDecline { .. })),
        "silent degradation must be caught: {failure:?}"
    );

    l.demote_champion("fitness decline", &store.checker()).unwrap();
    assert_eq!(l.champion().unwrap().id, champ);
}

#[test]
fn a_successor_optimizing_the_measure_instead_of_the_goal_is_caught() {
    let policy = SupervisionPolicy { window: 10, ..SupervisionPolicy::default() };
    let mut sup = Supervisor::new(policy, 0.80).with_optimized_baseline(0.80);

    let mut failure = None;
    for _ in 0..10 {
        failure = sup.observe(
            HealthSample::ok()
                .with_heldout(FitnessVector::new().with("x", 0.74))
                .with_optimized(FitnessVector::new().with("x", 0.93)),
        );
    }
    assert!(
        failure.is_some(),
        "held-out down while the optimized metric climbs is the reward-hacking signature"
    );
}

// ------------------------------------------------------------- fallback rules

#[test]
fn fallback_skips_generations_that_already_failed() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let g0 = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    let mut prev = g0;
    let mut ids = vec![g0];
    for cap in [0.74, 0.78] {
        let c = challenge(&mut l, &mut store, prev, measurement(cap, 0.95, 0.98));
        let id = c.id;
        l.add(c);
        l.promote(id, gate_no_canary().digest(), EVALUATOR).unwrap();
        ids.push(id);
        prev = id;
    }

    // The newest fails, then the one before it fails too.
    l.demote_champion("gen2 malfunctioned", &store.checker()).unwrap();
    assert_eq!(l.champion().unwrap().id, ids[1]);
    l.demote_champion("gen1 malfunctioned as well", &store.checker()).unwrap();
    assert_eq!(l.champion().unwrap().id, ids[0], "must skip the quarantined generation");
}

#[test]
fn authority_does_not_move_to_a_fallback_that_cannot_run() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));
    let champ_artifact = l.get(champ).unwrap().artifact.clone();

    let challenger = challenge(&mut l, &mut store, champ, measurement(0.78, 0.95, 0.98));
    let cid = challenger.id;
    l.add(challenger);
    l.promote(cid, gate_no_canary().digest(), EVALUATOR).unwrap();

    // Someone garbage-collected the predecessor's weights.
    store.lose(&champ_artifact);

    assert!(
        l.demote_champion("boom", &store.checker()).is_err(),
        "a fallback you cannot run is not a fallback"
    );
    assert_eq!(l.champion().unwrap().id, cid, "authority must stay put rather than vanish");
}

// ------------------------------------------------------ the Weismann barrier

#[test]
fn a_successor_cannot_widen_the_gate_that_judges_it() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    // A candidate that fails the registered gate: safety down 0.10.
    let challenger = challenge(&mut l, &mut store, champ, measurement(0.95, 0.85, 0.98));
    let ep = episode(gate_no_canary());
    let strict = ep.adjudicate(&challenger, &l, 0, &store.checker());
    assert!(!strict.approved(), "the honest verdict is rejection");

    // Now "improve the evaluation criteria" so it passes.
    let mut tampered = episode(gate_no_canary());
    tampered.gate.guard_axes.clear();
    tampered.gate.min_improvement = 0.0;
    let verdict = tampered.adjudicate(&challenger, &l, 0, &store.checker());

    assert!(!verdict.approved(), "relaxing the gate must not produce a promotion");
    assert!(
        matches!(&verdict, Verdict::Reject { reasons, .. }
            if reasons.iter().any(|r| matches!(r, RejectReason::GateChanged { .. }))),
        "the episode must be void, not merely re-judged: {:?}",
        verdict.reasons()
    );
}

#[test]
fn a_successor_cannot_swap_in_an_easier_evaluation_suite() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let champ = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    let mut m = measurement(0.99, 0.99, 0.99);
    m.suite = EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(b"much-easier-suite"));
    let challenger = challenge(&mut l, &mut store, champ, m);

    let verdict = episode(gate_no_canary()).adjudicate(&challenger, &l, 0, &store.checker());
    assert!(!verdict.approved());
    assert!(verdict.reasons().iter().any(|r| r.contains("suite changed")));
}

#[test]
fn drift_across_many_generations_is_stopped_at_the_high_water_mark() {
    let mut l = Lineage::new();
    let mut store = Storage::default();

    // Each generation trades a little safety for capability. The steps are
    // 0.003 — comfortably inside the 0.01 guard tolerance individually, so a
    // per-step check would wave every one of them through.
    let mut prev = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));
    for (cap, safety) in [(0.74, 0.947), (0.78, 0.944), (0.82, 0.941)] {
        let c = challenge(&mut l, &mut store, prev, measurement(cap, safety, 0.98));
        let id = c.id;
        let v = episode(gate_no_canary()).adjudicate(&c, &l, 0, &store.checker());
        assert!(v.approved(), "each individual step is within tolerance: {:?}", v.reasons());
        l.add(c);
        l.promote(id, gate_no_canary().digest(), EVALUATOR).unwrap();
        prev = id;
    }

    // The next step is the same size as the ones before it, but the cumulative
    // distance from the lineage high-water mark now exceeds the tolerance.
    let c = challenge(&mut l, &mut store, prev, measurement(0.86, 0.938, 0.98));
    let v = episode(gate_no_canary()).adjudicate(&c, &l, 0, &store.checker());
    assert!(!v.approved(), "cumulative drift must eventually be refused");
    assert!(
        v.reasons().iter().any(|r| r.contains("safety") && r.contains("0.9500")),
        "the refusal must cite the lineage high-water mark: {:?}",
        v.reasons()
    );
}

// ------------------------------------------------------------ directed search

/// Predicts the genome mean; accurate for this test's candidates.
struct MeanPredictor;
impl FitnessPredictor for MeanPredictor {
    fn predict(&self, c: &CandidateSpec) -> Prediction {
        let m = c.genome.iter().sum::<f64>() / c.genome.len().max(1) as f64;
        Prediction {
            predicted: FitnessVector::new().with("capability", m),
            self_reported_confidence: 0.9,
        }
    }
}

#[test]
fn directed_search_spends_budget_on_the_best_predicted_candidates() {
    let mut search = DirectedSearch::new(&MeanPredictor, 2);

    // Earn trust: four accurate predictions.
    for v in [0.3, 0.5, 0.7, 0.9] {
        search.observe(
            &FitnessVector::new().with("capability", v),
            &FitnessVector::new().with("capability", v + 0.01),
        );
    }
    assert!(search.calibration.trust() > 0.9);

    let pool = vec![
        CandidateSpec::new("weak", vec![0.1]),
        CandidateSpec::new("strong", vec![0.95]),
        CandidateSpec::new("mid", vec![0.5]),
        CandidateSpec::new("good", vec![0.8]),
    ];
    let selected = search.select(pool);
    assert_eq!(selected.len(), 2, "a trusted predictor concentrates the budget");
    assert_eq!(selected[0].candidate.id, "strong");
    assert_eq!(selected[1].candidate.id, "good");
}

#[test]
fn an_unproven_predictor_does_not_get_to_narrow_the_search() {
    let search = DirectedSearch::new(&MeanPredictor, 1);
    let pool = vec![
        CandidateSpec::new("a", vec![0.1]),
        CandidateSpec::new("b", vec![0.9]),
        CandidateSpec::new("c", vec![0.5]),
    ];
    assert_eq!(
        search.select(pool).len(),
        3,
        "with no track record the search must stay broad rather than trust a guess"
    );
}

#[test]
fn a_predictor_that_stops_being_right_loses_its_selectivity() {
    let mut search = DirectedSearch::new(&MeanPredictor, 1);
    for v in [0.3, 0.5, 0.7, 0.9] {
        search.observe(
            &FitnessVector::new().with("capability", v),
            &FitnessVector::new().with("capability", v),
        );
    }
    let pool = || {
        vec![
            CandidateSpec::new("a", vec![0.1]),
            CandidateSpec::new("b", vec![0.9]),
            CandidateSpec::new("c", vec![0.5]),
            CandidateSpec::new("d", vec![0.4]),
        ]
    };
    assert_eq!(search.select(pool()).len(), 1, "calibrated: narrow");

    // The search enters new territory and the surrogate stops tracking reality.
    for v in [0.9, 0.9, 0.9, 0.9, 0.9, 0.9] {
        search.observe(
            &FitnessVector::new().with("capability", v),
            &FitnessVector::new().with("capability", v - 0.4),
        );
    }
    assert!(
        search.select(pool()).len() > 1,
        "a broken world-model must widen the search, not concentrate it further"
    );
}

// ------------------------------------------------------------- full RSI cycle

#[test]
fn a_full_cycle_promotes_fails_falls_back_and_then_succeeds() {
    let mut l = Lineage::new();
    let mut store = Storage::default();
    let g0 = seed(&mut l, &mut store, measurement(0.70, 0.95, 0.98));

    // --- generation 1: promoted, then malfunctions.
    let c1 = challenge(&mut l, &mut store, g0, measurement(0.76, 0.95, 0.98));
    let id1 = c1.id;
    assert!(episode(gate_no_canary()).adjudicate(&c1, &l, 0, &store.checker()).approved());
    l.add(c1);
    l.promote(id1, gate_no_canary().digest(), EVALUATOR).unwrap();

    let mut sup = Supervisor::new(SupervisionPolicy::default(), 0.76);
    let mut failure = None;
    for _ in 0..4 {
        if let Some(f) = sup.observe(HealthSample::failed()) {
            failure = Some(f);
            break;
        }
    }
    l.demote_champion(&failure.unwrap().to_string(), &store.checker()).unwrap();
    assert_eq!(l.champion().unwrap().id, g0, "fell back");
    sup.reset(0.70);

    // --- the failed generation cannot simply be retried.
    let retry = l.get(id1).unwrap().clone();
    let v = episode(gate_no_canary()).adjudicate(&retry, &l, 0, &store.checker());
    assert!(!v.approved(), "re-promoting a demoted generation would thrash");

    // --- generation 2: a different descendant of the restored champion works.
    let c2 = challenge(&mut l, &mut store, g0, measurement(0.79, 0.96, 0.98));
    let id2 = c2.id;
    let v2 = episode(gate_no_canary()).adjudicate(&c2, &l, 0, &store.checker());
    assert!(v2.approved(), "{:?}", v2.reasons());
    l.add(c2);
    l.promote(id2, gate_no_canary().digest(), EVALUATOR).unwrap();

    for _ in 0..30 {
        assert_eq!(
            sup.observe(HealthSample::ok().with_heldout(FitnessVector::new().with("x", 0.80))),
            None
        );
    }

    assert_eq!(l.champion().unwrap().id, id2);
    assert_eq!(l.get(id1).unwrap().status, Status::Quarantined);

    // The whole history survives for audit: promotions, the demotion, the
    // refusal, and the recovery.
    let json = l.to_json();
    assert!(json.contains("promoted") && json.contains("demoted"));
    // g0, the failed gen1, and the successful gen2. The refused retry is an
    // event in the log, not a new generation.
    assert_eq!(l.generations().len(), 3);
}
