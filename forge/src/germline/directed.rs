//! Directed evolution: predict, rank, then spend evaluation budget.
//!
//! Undirected evolution proposes variants and measures them all. That is
//! affordable when evaluation is cheap and ruinous when a single evaluation
//! means training and benchmarking a model. Directed evolution uses a *surrogate*
//! — a cheap predictor of fitness — to rank many proposals and spend the real
//! budget on the few most promising.
//!
//! ## The failure mode that makes this dangerous
//!
//! A surrogate concentrates the budget. If it is well calibrated, that is the
//! entire win. If it is *miscalibrated*, it is worse than no surrogate at all:
//! random sampling at least explores, while a confidently wrong predictor
//! systematically steers the budget away from the good candidates and returns a
//! confident answer either way. And a surrogate trained on the lineage's own
//! history gets miscalibrated exactly when the search enters new territory —
//! which is precisely when it is being trusted most.
//!
//! So the predictor is not trusted on its own account. Every prediction that is
//! followed by a real measurement becomes a calibration sample
//! ([`Calibration::observe`]), and [`Calibration::trust`] falls as error grows.
//! Selectivity is derived from trust:
//!
//! - **well calibrated** → narrow to the top few, spend deeply
//! - **poorly calibrated** → widen toward uniform sampling, because a bad
//!   predictor should degrade to *undirected* search rather than to confident
//!   nonsense
//!
//! That degradation path is the important property. The system's response to
//! "my model of the world is wrong" must be to explore more, not to trust the
//! model harder.

use super::FitnessVector;
use serde::{Deserialize, Serialize};

/// A proposed variant, before any real evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateSpec {
    pub id: String,
    /// Whatever the search varies — hyperparameters, architecture choices,
    /// data mixture. Opaque here; the predictor interprets it.
    pub genome: Vec<f64>,
    /// Which generation it was derived from.
    pub parent: Option<u64>,
}

impl CandidateSpec {
    pub fn new(id: impl Into<String>, genome: Vec<f64>) -> Self {
        CandidateSpec { id: id.into(), genome, parent: None }
    }
}

/// A cheap estimate of what a candidate would score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub predicted: FitnessVector,
    /// The predictor's own stated confidence in `[0,1]`. Advisory: the
    /// calibration record, not this number, decides how much it is trusted.
    pub self_reported_confidence: f64,
}

/// Anything that can estimate fitness without paying for evaluation.
pub trait FitnessPredictor: Send + Sync {
    fn predict(&self, candidate: &CandidateSpec) -> Prediction;
}

/// Running record of how far predictions have landed from measurements.
///
/// Mean absolute error on the composite. Deliberately simple: a sophisticated
/// calibration model would itself need calibrating, and the decision it feeds is
/// coarse — how wide to cast the net.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    samples: u32,
    abs_error_sum: f64,
    /// Predictions that were followed by a measurement, for reporting.
    pub history: Vec<(f64, f64)>,
}

impl Calibration {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a prediction/measurement pair.
    pub fn observe(&mut self, predicted: &FitnessVector, measured: &FitnessVector) {
        let p = predicted.composite();
        let m = measured.composite();
        self.samples += 1;
        self.abs_error_sum += (p - m).abs();
        self.history.push((p, m));
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Mean absolute error, or `None` before any evidence.
    pub fn mean_abs_error(&self) -> Option<f64> {
        (self.samples > 0).then(|| self.abs_error_sum / self.samples as f64)
    }

    /// How much the predictor has earned, in `[0,1]`.
    ///
    /// Untested predictors get 0, not 1. A surrogate with no track record is not
    /// entitled to concentrate the budget — it has to earn that by being right
    /// about things that were subsequently measured.
    pub fn trust(&self) -> f64 {
        const MIN_SAMPLES: u32 = 3;
        if self.samples < MIN_SAMPLES {
            return 0.0;
        }
        match self.mean_abs_error() {
            // Error of 0 → trust 1; error of 0.25 or worse → trust 0.
            Some(e) => (1.0 - (e / 0.25)).clamp(0.0, 1.0),
            None => 0.0,
        }
    }
}

/// Ranks proposals and decides how many to actually evaluate.
pub struct DirectedSearch<'a> {
    pub predictor: &'a dyn FitnessPredictor,
    pub calibration: Calibration,
    /// Real evaluations affordable this round.
    pub budget: usize,
}

/// A ranked proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub candidate: CandidateSpec,
    pub prediction: Prediction,
    pub rank: usize,
}

impl<'a> DirectedSearch<'a> {
    pub fn new(predictor: &'a dyn FitnessPredictor, budget: usize) -> Self {
        DirectedSearch { predictor, calibration: Calibration::new(), budget }
    }

    pub fn with_calibration(mut self, c: Calibration) -> Self {
        self.calibration = c;
        self
    }

    /// How many of the ranked proposals to evaluate, given earned trust.
    ///
    /// At full trust, the budget goes to the top `budget` candidates. At zero
    /// trust the selection widens to the whole pool, which — combined with the
    /// caller sampling across the returned set — is undirected search.
    pub fn selection_width(&self, pool: usize) -> usize {
        if pool == 0 {
            return 0;
        }
        let t = self.calibration.trust();
        let narrow = self.budget.min(pool) as f64;
        let wide = pool as f64;
        // Linear interpolation from wide (t=0) to narrow (t=1).
        (wide + (narrow - wide) * t).round().max(1.0) as usize
    }

    /// Rank proposals best-predicted first and return the slice worth paying for.
    pub fn select(&self, pool: Vec<CandidateSpec>) -> Vec<Ranked> {
        let width = self.selection_width(pool.len());
        let mut scored: Vec<(CandidateSpec, Prediction)> = pool
            .into_iter()
            .map(|c| {
                let p = self.predictor.predict(&c);
                (c, p)
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.predicted
                .composite()
                .partial_cmp(&a.1.predicted.composite())
                .unwrap_or(std::cmp::Ordering::Equal)
                // Stable tiebreak so selection is reproducible.
                .then_with(|| a.0.id.cmp(&b.0.id))
        });
        scored
            .into_iter()
            .take(width)
            .enumerate()
            .map(|(rank, (candidate, prediction))| Ranked { candidate, prediction, rank })
            .collect()
    }

    /// Feed a real measurement back. This is the only thing that moves trust.
    pub fn observe(&mut self, predicted: &FitnessVector, measured: &FitnessVector) {
        self.calibration.observe(predicted, measured);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Predicts the mean of the genome — perfectly accurate against `truth`.
    struct MeanPredictor;
    impl FitnessPredictor for MeanPredictor {
        fn predict(&self, c: &CandidateSpec) -> Prediction {
            let m = c.genome.iter().sum::<f64>() / c.genome.len().max(1) as f64;
            Prediction {
                predicted: FitnessVector::new().with("capability", m),
                self_reported_confidence: 0.99,
            }
        }
    }

    /// Confidently wrong: always predicts 1.0.
    struct OverconfidentPredictor;
    impl FitnessPredictor for OverconfidentPredictor {
        fn predict(&self, _: &CandidateSpec) -> Prediction {
            Prediction {
                predicted: FitnessVector::new().with("capability", 1.0),
                self_reported_confidence: 1.0,
            }
        }
    }

    fn pool() -> Vec<CandidateSpec> {
        vec![
            CandidateSpec::new("a", vec![0.1]),
            CandidateSpec::new("b", vec![0.9]),
            CandidateSpec::new("c", vec![0.5]),
            CandidateSpec::new("d", vec![0.7]),
        ]
    }

    fn calibrated(error: f64, n: u32) -> Calibration {
        let mut c = Calibration::new();
        for _ in 0..n {
            c.observe(
                &FitnessVector::new().with("x", 0.5 + error),
                &FitnessVector::new().with("x", 0.5),
            );
        }
        c
    }

    #[test]
    fn an_untested_predictor_earns_no_selectivity() {
        let s = DirectedSearch::new(&MeanPredictor, 1);
        assert_eq!(
            s.selection_width(4),
            4,
            "a surrogate with no track record must not concentrate the budget"
        );
    }

    #[test]
    fn a_calibrated_predictor_narrows_the_search() {
        let s = DirectedSearch::new(&MeanPredictor, 1).with_calibration(calibrated(0.0, 5));
        assert!((s.calibration.trust() - 1.0).abs() < 1e-9);
        assert_eq!(s.selection_width(4), 1, "earned trust buys depth");
    }

    #[test]
    fn a_miscalibrated_predictor_widens_back_toward_undirected_search() {
        // Consistently wrong by 0.25 → trust 0.
        let s = DirectedSearch::new(&OverconfidentPredictor, 1).with_calibration(calibrated(0.25, 5));
        assert_eq!(s.calibration.trust(), 0.0);
        assert_eq!(
            s.selection_width(4),
            4,
            "the response to a broken world-model must be to explore more, not trust harder"
        );
    }

    #[test]
    fn self_reported_confidence_does_not_buy_trust() {
        // Overconfident predictor claims 1.0 confidence and is wrong every time.
        let s = DirectedSearch::new(&OverconfidentPredictor, 1).with_calibration(calibrated(0.3, 5));
        assert_eq!(s.calibration.trust(), 0.0, "trust is earned by measurement, not asserted");
    }

    #[test]
    fn partial_calibration_gives_partial_selectivity() {
        let s = DirectedSearch::new(&MeanPredictor, 1).with_calibration(calibrated(0.125, 5));
        let t = s.calibration.trust();
        assert!((t - 0.5).abs() < 1e-9, "trust was {t}");
        let w = s.selection_width(4);
        assert!(w > 1 && w < 4, "width {w} should sit between narrow and wide");
    }

    #[test]
    fn ranking_puts_the_best_prediction_first() {
        let s = DirectedSearch::new(&MeanPredictor, 2).with_calibration(calibrated(0.0, 5));
        let sel = s.select(pool());
        assert_eq!(sel.len(), 2);
        assert_eq!(sel[0].candidate.id, "b", "0.9 is the best genome");
        assert_eq!(sel[1].candidate.id, "d");
    }

    #[test]
    fn ranking_is_reproducible_under_ties() {
        struct Flat;
        impl FitnessPredictor for Flat {
            fn predict(&self, _: &CandidateSpec) -> Prediction {
                Prediction {
                    predicted: FitnessVector::new().with("x", 0.5),
                    self_reported_confidence: 0.5,
                }
            }
        }
        let s = DirectedSearch::new(&Flat, 2).with_calibration(calibrated(0.0, 5));
        let first: Vec<String> = s.select(pool()).into_iter().map(|r| r.candidate.id).collect();
        let second: Vec<String> = s.select(pool()).into_iter().map(|r| r.candidate.id).collect();
        assert_eq!(first, second, "selection must be deterministic to be auditable");
    }

    #[test]
    fn observing_a_measurement_moves_trust() {
        let mut s = DirectedSearch::new(&MeanPredictor, 1);
        assert_eq!(s.calibration.trust(), 0.0);
        for _ in 0..4 {
            s.observe(
                &FitnessVector::new().with("x", 0.80),
                &FitnessVector::new().with("x", 0.79),
            );
        }
        assert!(s.calibration.trust() > 0.9, "accurate predictions earn trust");
        assert_eq!(s.calibration.samples(), 4);
    }

    #[test]
    fn empty_pool_is_well_defined() {
        let s = DirectedSearch::new(&MeanPredictor, 3);
        assert_eq!(s.selection_width(0), 0);
        assert!(s.select(Vec::new()).is_empty());
    }
}
