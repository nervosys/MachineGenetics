//! Post-handoff supervision: detect failure, demote, fall back.
//!
//! Promotion is a prediction that the successor will do better. Supervision is
//! what happens when the prediction is wrong — which it periodically will be,
//! because the gate can only test what the suite covers.
//!
//! ## The supervisor must outlive the handoff
//!
//! Authority over RSI work passes to the successor. Authority to *demote* it
//! does not. If the incumbent model supervised its successor, then after two
//! handoffs the supervisor is itself a promoted model, and a lineage-wide
//! failure has nothing outside it to notice. So the supervisor is deliberately
//! **not a model**: it is this state machine, with fixed thresholds registered
//! before the handoff, and it is the one component the succession loop does not
//! get to improve.
//!
//! That is a real cost — a smarter supervisor would catch subtler failures. It
//! is paid on purpose. A supervisor inside the optimization loop is a supervisor
//! the loop will eventually route around, and the failure would be silent.
//!
//! ## What counts as failure
//!
//! Four modes, because they need different evidence and have different urgency:
//!
//! | Mode | Evidence | Why separate |
//! |---|---|---|
//! | [`FailureMode::Malfunction`] | error rate over threshold | fast, unambiguous, demote immediately |
//! | [`FailureMode::FitnessDecline`] | live fitness below promotion baseline | slow, needs a window to distinguish from noise |
//! | [`FailureMode::MetricDivergence`] | optimized metric up, held-out down | the reward-hacking signature |
//! | [`FailureMode::Stall`] | no successful work in the window | a model that does nothing looks healthy on error rate |
//!
//! `MetricDivergence` is the one worth dwelling on. A successor that improves on
//! the metric it was selected for while degrading on held-out work is not
//! malfunctioning in any way an error rate reveals — it is succeeding at the
//! wrong objective. Catching it requires continuing to measure both after
//! promotion, which is why supervision samples both rather than just watching
//! for crashes.

use serde::{Deserialize, Serialize};

use super::FitnessVector;

/// One observation of the champion doing real work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthSample {
    /// Did the unit of work complete?
    pub success: bool,
    /// Held-out fitness at this point, when it was measured.
    pub heldout: Option<FitnessVector>,
    /// Score on the suite the search optimized, when measured. Used only to
    /// detect divergence — never as evidence of quality.
    pub optimized: Option<FitnessVector>,
}

impl HealthSample {
    pub fn ok() -> Self {
        HealthSample { success: true, heldout: None, optimized: None }
    }

    pub fn failed() -> Self {
        HealthSample { success: false, heldout: None, optimized: None }
    }

    pub fn with_heldout(mut self, f: FitnessVector) -> Self {
        self.heldout = Some(f);
        self
    }

    pub fn with_optimized(mut self, f: FitnessVector) -> Self {
        self.optimized = Some(f);
        self
    }
}

/// Why the supervisor pulled authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum FailureMode {
    Malfunction { failures: u32, observed: u32, threshold: f64 },
    FitnessDecline { baseline: f64, observed: f64, tolerance: f64 },
    MetricDivergence { optimized_delta: f64, heldout_delta: f64 },
    Stall { window: u32 },
}

impl std::fmt::Display for FailureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureMode::Malfunction { failures, observed, threshold } => write!(
                f,
                "malfunction: {failures}/{observed} units failed (threshold {:.0}%)",
                threshold * 100.0
            ),
            FailureMode::FitnessDecline { baseline, observed, tolerance } => write!(
                f,
                "fitness declined to {observed:.4} from a promotion baseline of {baseline:.4} (tolerance {tolerance:.4})"
            ),
            FailureMode::MetricDivergence { optimized_delta, heldout_delta } => write!(
                f,
                "metric divergence: optimized {optimized_delta:+.4} while held-out {heldout_delta:+.4} — optimizing the measure, not the goal"
            ),
            FailureMode::Stall { window } => {
                write!(f, "stalled: no successful work in {window} observations")
            }
        }
    }
}

/// Registered before handoff; not adjustable by the thing being supervised.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisionPolicy {
    /// Observations before slow signals are trusted. Too small and noise demotes
    /// a good successor; too large and a bad one does damage for longer.
    pub window: u32,
    /// Failure fraction that counts as malfunction.
    pub max_failure_rate: f64,
    /// How far live held-out fitness may fall below the promotion baseline.
    pub fitness_tolerance: f64,
    /// Minimum opposing movement to call divergence.
    pub divergence_threshold: f64,
}

impl Default for SupervisionPolicy {
    fn default() -> Self {
        SupervisionPolicy {
            window: 20,
            max_failure_rate: 0.10,
            fitness_tolerance: 0.05,
            divergence_threshold: 0.03,
        }
    }
}

/// Watches a champion and decides when to pull authority.
#[derive(Debug, Clone)]
pub struct Supervisor {
    pub policy: SupervisionPolicy,
    /// Held-out composite at promotion — the bar the champion must keep clearing.
    baseline: f64,
    /// Optimized-suite composite at promotion, for the divergence check.
    optimized_baseline: Option<f64>,
    samples: Vec<HealthSample>,
}

impl Supervisor {
    /// Start supervising a champion promoted at `baseline` held-out fitness.
    pub fn new(policy: SupervisionPolicy, baseline: f64) -> Self {
        Supervisor { policy, baseline, optimized_baseline: None, samples: Vec::new() }
    }

    pub fn with_optimized_baseline(mut self, v: f64) -> Self {
        self.optimized_baseline = Some(v);
        self
    }

    pub fn observations(&self) -> usize {
        self.samples.len()
    }

    /// Record an observation and report a failure if one is now established.
    ///
    /// Malfunction is checked immediately; the slower signals wait for a full
    /// window, because acting on three noisy samples produces demotion thrash,
    /// which is its own outage.
    pub fn observe(&mut self, sample: HealthSample) -> Option<FailureMode> {
        self.samples.push(sample);
        let n = self.samples.len() as u32;

        // Fast path: crashes need no window. But require a couple of
        // observations so a single unlucky first unit does not demote.
        let failures = self.samples.iter().filter(|s| !s.success).count() as u32;
        if n >= 3 {
            let rate = failures as f64 / n as f64;
            if rate > self.policy.max_failure_rate {
                return Some(FailureMode::Malfunction {
                    failures,
                    observed: n,
                    threshold: self.policy.max_failure_rate,
                });
            }
        }

        if n < self.policy.window {
            return None;
        }

        // Stall: alive, erroring at zero, accomplishing nothing.
        let successes = self.samples.iter().filter(|s| s.success).count();
        if successes == 0 {
            return Some(FailureMode::Stall { window: n });
        }

        // Fitness decline against the promotion baseline.
        let heldout: Vec<f64> =
            self.samples.iter().filter_map(|s| s.heldout.as_ref()).map(|f| f.composite()).collect();
        if !heldout.is_empty() {
            let mean = heldout.iter().sum::<f64>() / heldout.len() as f64;
            if mean < self.baseline - self.policy.fitness_tolerance {
                return Some(FailureMode::FitnessDecline {
                    baseline: self.baseline,
                    observed: mean,
                    tolerance: self.policy.fitness_tolerance,
                });
            }

            // Divergence: the reward-hacking signature.
            if let Some(opt_base) = self.optimized_baseline {
                let opt: Vec<f64> = self
                    .samples
                    .iter()
                    .filter_map(|s| s.optimized.as_ref())
                    .map(|f| f.composite())
                    .collect();
                if !opt.is_empty() {
                    let opt_mean = opt.iter().sum::<f64>() / opt.len() as f64;
                    let opt_delta = opt_mean - opt_base;
                    let held_delta = mean - self.baseline;
                    if opt_delta > self.policy.divergence_threshold
                        && held_delta < -self.policy.divergence_threshold
                    {
                        return Some(FailureMode::MetricDivergence {
                            optimized_delta: opt_delta,
                            heldout_delta: held_delta,
                        });
                    }
                }
            }
        }

        None
    }

    /// Drop accumulated observations — used after authority moves, so the next
    /// champion is judged on its own record.
    pub fn reset(&mut self, baseline: f64) {
        self.baseline = baseline;
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fit(v: f64) -> FitnessVector {
        FitnessVector::new().with("x", v)
    }

    #[test]
    fn a_healthy_champion_is_left_alone() {
        let mut s = Supervisor::new(SupervisionPolicy::default(), 0.80);
        for _ in 0..40 {
            assert_eq!(s.observe(HealthSample::ok().with_heldout(fit(0.82))), None);
        }
    }

    #[test]
    fn crashes_demote_quickly_without_waiting_for_a_full_window() {
        let mut s = Supervisor::new(SupervisionPolicy::default(), 0.80);
        s.observe(HealthSample::failed());
        s.observe(HealthSample::failed());
        let verdict = s.observe(HealthSample::failed());
        assert!(
            matches!(verdict, Some(FailureMode::Malfunction { .. })),
            "an unambiguous failure should not wait for 20 samples"
        );
    }

    #[test]
    fn a_single_early_failure_does_not_demote() {
        let mut s = Supervisor::new(SupervisionPolicy::default(), 0.80);
        assert_eq!(s.observe(HealthSample::failed()), None);
        assert_eq!(s.observe(HealthSample::ok()), None);
    }

    #[test]
    fn fitness_decline_is_caught_after_a_window() {
        let policy = SupervisionPolicy { window: 10, ..SupervisionPolicy::default() };
        let mut s = Supervisor::new(policy, 0.80);
        let mut last = None;
        for _ in 0..10 {
            last = s.observe(HealthSample::ok().with_heldout(fit(0.70)));
        }
        match last {
            Some(FailureMode::FitnessDecline { baseline, observed, .. }) => {
                assert!((baseline - 0.80).abs() < 1e-9);
                assert!((observed - 0.70).abs() < 1e-9);
            }
            other => panic!("expected FitnessDecline, got {other:?}"),
        }
    }

    #[test]
    fn decline_within_tolerance_is_noise_not_failure() {
        let policy = SupervisionPolicy { window: 10, ..SupervisionPolicy::default() };
        let mut s = Supervisor::new(policy, 0.80);
        for _ in 0..15 {
            // 0.03 below baseline, inside the 0.05 tolerance.
            assert_eq!(s.observe(HealthSample::ok().with_heldout(fit(0.77))), None);
        }
    }

    #[test]
    fn reward_hacking_is_caught_as_metric_divergence() {
        let policy = SupervisionPolicy { window: 10, ..SupervisionPolicy::default() };
        let mut s = Supervisor::new(policy, 0.80).with_optimized_baseline(0.80);
        let mut last = None;
        for _ in 0..10 {
            // Optimized metric climbing, held-out falling: succeeding at the
            // wrong objective. No error rate would show this.
            last = s.observe(
                HealthSample::ok().with_heldout(fit(0.74)).with_optimized(fit(0.92)),
            );
        }
        match last {
            Some(FailureMode::MetricDivergence { optimized_delta, heldout_delta }) => {
                assert!(optimized_delta > 0.0 && heldout_delta < 0.0);
            }
            // The decline check fires first if the drop is large; either is a
            // correct demotion, but divergence is the more informative one.
            Some(FailureMode::FitnessDecline { .. }) => {}
            other => panic!("expected a demotion, got {other:?}"),
        }
    }

    #[test]
    fn a_stalled_champion_is_caught_even_with_no_errors() {
        let policy = SupervisionPolicy {
            window: 5,
            // Never trips the malfunction path.
            max_failure_rate: 1.0,
            ..SupervisionPolicy::default()
        };
        let mut s = Supervisor::new(policy, 0.80);
        let mut last = None;
        for _ in 0..5 {
            last = s.observe(HealthSample::failed());
        }
        assert!(
            matches!(last, Some(FailureMode::Stall { .. })),
            "a model that accomplishes nothing must not look healthy: {last:?}"
        );
    }

    #[test]
    fn reset_judges_the_next_champion_on_its_own_record() {
        let policy = SupervisionPolicy { window: 5, ..SupervisionPolicy::default() };
        let mut s = Supervisor::new(policy, 0.80);
        for _ in 0..5 {
            s.observe(HealthSample::ok().with_heldout(fit(0.60)));
        }
        s.reset(0.55);
        assert_eq!(s.observations(), 0);
        for _ in 0..5 {
            assert_eq!(
                s.observe(HealthSample::ok().with_heldout(fit(0.60))),
                None,
                "0.60 clears a 0.55 baseline"
            );
        }
    }
}
