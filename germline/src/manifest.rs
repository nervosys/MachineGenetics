//! Prediction manifests: an edit must say what it expects to change.
//!
//! A harness edit proposed by an external agent is only as trustworthy as that
//! agent's record of being right. So every such edit states a prediction up
//! front: which metric it moves, in which direction, and by how much. The
//! prediction is checked against what the measurement later shows
//! ([`score`]), and the proposer's running record of those scores is its
//! [`ProposerRecord::trust`], which decides how many of its edits the loop
//! admits per cycle (see `runner`).
//!
//! ## Why a missing manifest is refused rather than tolerated
//!
//! An edit with no stated expectation cannot be scored, so it cannot move the
//! proposer's record in either direction. Admitting it would let a proposer
//! make unfalsifiable changes and be judged only on the ones it chose to
//! describe. The check runs before anything is materialized.
//!
//! ## Trust starts at zero
//!
//! An unproven proposer starts at zero trust and earns it. Until it has
//! [`ProposerRecord::MIN_SAMPLES`] scored predictions it admits the minimum
//! number of candidates, and after that its admission width follows its mean
//! score. This is the same stance as [`super::directed::Calibration`]: a
//! prediction is evidence only once a measurement has followed it.

use serde::{Deserialize, Serialize};

/// The direction an edit is predicted to move its metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Increase,
    Decrease,
}

/// What an edit predicts: a metric, a direction, and a magnitude.
///
/// The magnitude is an absolute change on the metric's `[0,1]` scale, and must
/// be finite and strictly positive ([`check`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictionManifest {
    pub metric: String,
    pub direction: Direction,
    pub magnitude: f64,
}

impl PredictionManifest {
    /// The signed change predicted: `+magnitude` or `-magnitude`.
    pub fn predicted_delta(&self) -> f64 {
        match self.direction {
            Direction::Increase => self.magnitude,
            Direction::Decrease => -self.magnitude,
        }
    }
}

/// Refuse an edit that does not state a checkable prediction.
///
/// Refused when there is no manifest, when its metric is not one of `axes`,
/// and when its magnitude is not finite and strictly positive. If `axes` is
/// empty (no held-out measurement exists yet to name them), any metric is
/// accepted, but a manifest is still required.
pub fn check<'a>(
    manifest: Option<&'a PredictionManifest>,
    axes: &[&str],
) -> Result<&'a PredictionManifest, String> {
    let m = manifest.ok_or_else(|| {
        "a harness edit must state a prediction (metric, direction, magnitude)".to_string()
    })?;
    if !axes.is_empty() && !axes.contains(&m.metric.as_str()) {
        return Err(format!(
            "metric `{}` is not one of the suite's axes ({})",
            m.metric,
            axes.join(", ")
        ));
    }
    if !(m.magnitude.is_finite() && m.magnitude > 0.0) {
        return Err(format!("magnitude must be finite and > 0, got {}", m.magnitude));
    }
    Ok(m)
}

/// How well a predicted change matched the measured one, in `[0,1]`.
///
/// - `1` is an exact match.
/// - A same-sign prediction scores `1 - |predicted - measured| / max(|predicted|, |measured|)`,
///   so being off by half the larger magnitude scores `0.5`.
/// - `0` when the signs differ, when the measured change is zero but the
///   predicted one is not, or when the predicted change is zero but the
///   measured one is not.
/// - Both zero (a predicted non-change that did not happen) scores `1`.
/// - Non-finite inputs score `0`.
pub fn score(predicted_delta: f64, measured_delta: f64) -> f64 {
    if !predicted_delta.is_finite() || !measured_delta.is_finite() {
        return 0.0;
    }
    if measured_delta == 0.0 {
        return if predicted_delta == 0.0 { 1.0 } else { 0.0 };
    }
    if predicted_delta == 0.0 || (predicted_delta < 0.0) != (measured_delta < 0.0) {
        return 0.0;
    }
    let largest = predicted_delta.abs().max(measured_delta.abs());
    (1.0 - (predicted_delta - measured_delta).abs() / largest).clamp(0.0, 1.0)
}

/// A proposer's running record of how its predictions scored.
///
/// Trust is the mean score once there are [`Self::MIN_SAMPLES`] of them, and
/// zero before that. An unproven proposer is not entitled to a wider share of
/// the budget; it earns it by making predictions that come true.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct ProposerRecord {
    samples: u32,
    total_score: f64,
}

impl ProposerRecord {
    /// The same floor [`super::directed::Calibration`] applies.
    pub const MIN_SAMPLES: u32 = 3;

    pub fn observe(&mut self, score: f64) {
        self.samples += 1;
        self.total_score += score.clamp(0.0, 1.0);
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Mean score, or `0.0` before any evidence.
    pub fn mean(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total_score / self.samples as f64
        }
    }

    /// Trust in `[0,1]`: zero below [`Self::MIN_SAMPLES`], then the mean score.
    pub fn trust(&self) -> f64 {
        if self.samples < Self::MIN_SAMPLES {
            0.0
        } else {
            self.mean()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(metric: &str, direction: Direction, magnitude: f64) -> PredictionManifest {
        PredictionManifest { metric: metric.into(), direction, magnitude }
    }

    #[test]
    fn a_missing_manifest_is_refused_with_the_rule_stated() {
        let err = check(None, &["capability", "safety"]).unwrap_err();
        assert!(err.contains("a harness edit must state a prediction"), "{err}");
    }

    #[test]
    fn an_unknown_metric_is_refused_and_the_axes_are_named() {
        let p = m("speed", Direction::Increase, 0.1);
        let err = check(Some(&p), &["capability", "safety"]).unwrap_err();
        assert!(err.contains("speed"), "{err}");
        assert!(err.contains("capability, safety"), "the axes must be named: {err}");
    }

    #[test]
    fn a_bad_magnitude_is_refused() {
        for bad in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            let p = m("capability", Direction::Increase, bad);
            assert!(check(Some(&p), &["capability"]).is_err(), "magnitude {bad} must be refused");
        }
    }

    #[test]
    fn a_well_formed_manifest_is_admitted() {
        let p = m("capability", Direction::Increase, 0.05);
        assert_eq!(check(Some(&p), &["capability"]).unwrap(), &p);
    }

    #[test]
    fn with_no_axes_known_any_metric_is_admitted_but_a_manifest_is_still_required() {
        let p = m("anything", Direction::Decrease, 0.2);
        assert!(check(Some(&p), &[]).is_ok());
        assert!(check(None, &[]).is_err());
    }

    #[test]
    fn predicted_delta_carries_the_direction() {
        assert_eq!(m("c", Direction::Increase, 0.3).predicted_delta(), 0.3);
        assert_eq!(m("c", Direction::Decrease, 0.3).predicted_delta(), -0.3);
    }

    #[test]
    fn an_exact_prediction_scores_one() {
        assert_eq!(score(0.05, 0.05), 1.0);
        assert_eq!(score(-0.05, -0.05), 1.0);
    }

    #[test]
    fn being_off_by_half_scores_half() {
        assert!((score(0.10, 0.05) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn the_wrong_sign_scores_zero() {
        assert_eq!(score(0.05, -0.05), 0.0);
        assert_eq!(score(-0.05, 0.05), 0.0);
    }

    #[test]
    fn a_predicted_change_that_did_not_happen_scores_zero() {
        assert_eq!(score(0.05, 0.0), 0.0);
        assert_eq!(score(0.0, 0.05), 0.0);
        assert_eq!(score(0.0, 0.0), 1.0, "a predicted non-change that did not happen is right");
    }

    #[test]
    fn a_record_is_untrusted_until_it_has_three_scored_predictions() {
        let mut r = ProposerRecord::default();
        assert_eq!(r.trust(), 0.0);
        r.observe(1.0);
        r.observe(1.0);
        assert_eq!(r.trust(), 0.0, "two perfect predictions are not yet a record");
        r.observe(1.0);
        assert_eq!(r.trust(), 1.0);
    }

    #[test]
    fn trust_is_the_mean_score_once_earned() {
        let mut r = ProposerRecord::default();
        for s in [1.0, 0.0, 0.5] {
            r.observe(s);
        }
        assert_eq!(r.samples(), 3);
        assert!((r.trust() - 0.5).abs() < 1e-12);
    }
}
