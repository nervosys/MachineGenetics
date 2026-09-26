// ── Cost Model Calibration ─────────────────────────────────────────
//
// Standardized benchmarks for cost oracle accuracy across targets.
//
// The cost oracle (cost.rs) provides per-construct estimated costs.
// This module provides calibration infrastructure:
//
//   1. CostCalibrationSample — actual measured costs for a construct
//   2. CalibrationTarget — per-target calibration state
//   3. AccuracyMetric — mean absolute error, mean relative error, etc.
//   4. CalibrationSuite — run comparisons between estimated and measured
//   5. CalibrationReport — summary with accuracy grades
//   6. EnergySample / FuelCalibration — fuel against measured joules
//
// Fuel (MAGE_SPEC.md §4.11) is the unit a search compares candidates' cost
// in, and joules are the fixed unit of compute (decision D2). The fit below
// is what makes fuel a *calibrated* proxy: samples come from meters reading
// hardware while the evaluator spends fuel, never from a table. The arena
// feeds one sample per round (`arena::arena::run`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ── Calibration sample ─────────────────────────────────────────────

/// An actual measured cost for a construct on a target.
#[derive(Debug, Clone)]
pub struct CostCalibrationSample {
    pub construct: String,
    pub target: String,
    /// Measured CPU cycles.
    pub measured_cycles: u64,
    /// Measured memory bytes.
    pub measured_memory: u64,
    /// Measured latency in nanoseconds.
    pub measured_latency_ns: u64,
    /// Estimated CPU cycles (from cost oracle).
    pub estimated_cycles: u64,
    /// Estimated memory bytes.
    pub estimated_memory: u64,
    /// Estimated latency in nanoseconds.
    pub estimated_latency_ns: u64,
}

impl CostCalibrationSample {
    pub fn cycles_error(&self) -> f64 {
        (self.measured_cycles as f64 - self.estimated_cycles as f64).abs()
    }

    pub fn cycles_relative_error(&self) -> f64 {
        if self.measured_cycles == 0 {
            if self.estimated_cycles == 0 { 0.0 } else { 1.0 }
        } else {
            self.cycles_error() / self.measured_cycles as f64
        }
    }

    pub fn memory_error(&self) -> f64 {
        (self.measured_memory as f64 - self.estimated_memory as f64).abs()
    }

    pub fn latency_error(&self) -> f64 {
        (self.measured_latency_ns as f64 - self.estimated_latency_ns as f64).abs()
    }

    pub fn latency_relative_error(&self) -> f64 {
        if self.measured_latency_ns == 0 {
            if self.estimated_latency_ns == 0 { 0.0 } else { 1.0 }
        } else {
            self.latency_error() / self.measured_latency_ns as f64
        }
    }
}

// ── Accuracy metric ────────────────────────────────────────────────

/// Accuracy metrics aggregated over a set of calibration samples.
#[derive(Debug, Clone)]
pub struct AccuracyMetric {
    pub name: String,
    /// Mean absolute error.
    pub mae: f64,
    /// Mean relative error (0.0 = perfect, 1.0 = 100% off).
    pub mre: f64,
    /// Maximum relative error across all samples.
    pub max_re: f64,
    /// Number of samples.
    pub sample_count: usize,
}

/// Grade the accuracy of the cost model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccuracyGrade {
    /// MRE < 10%
    Excellent,
    /// MRE < 25%
    Good,
    /// MRE < 50%
    Fair,
    /// MRE >= 50%
    Poor,
}

impl AccuracyGrade {
    pub fn from_mre(mre: f64) -> Self {
        if mre < 0.10 {
            AccuracyGrade::Excellent
        } else if mre < 0.25 {
            AccuracyGrade::Good
        } else if mre < 0.50 {
            AccuracyGrade::Fair
        } else {
            AccuracyGrade::Poor
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AccuracyGrade::Excellent => "Excellent (<10%)",
            AccuracyGrade::Good => "Good (<25%)",
            AccuracyGrade::Fair => "Fair (<50%)",
            AccuracyGrade::Poor => "Poor (>=50%)",
        }
    }
}

// ── Calibration target ─────────────────────────────────────────────

/// Per-target calibration state.
#[derive(Debug, Clone)]
pub struct CalibrationTarget {
    pub target_name: String,
    pub samples: Vec<CostCalibrationSample>,
}

impl CalibrationTarget {
    pub fn new(target: &str) -> Self {
        Self {
            target_name: target.into(),
            samples: Vec::new(),
        }
    }

    pub fn add_sample(&mut self, sample: CostCalibrationSample) {
        self.samples.push(sample);
    }

    /// Compute cycles accuracy across all samples.
    pub fn cycles_accuracy(&self) -> AccuracyMetric {
        self.compute_accuracy("cycles", |s| s.cycles_error(), |s| s.cycles_relative_error())
    }

    /// Compute latency accuracy across all samples.
    pub fn latency_accuracy(&self) -> AccuracyMetric {
        self.compute_accuracy("latency", |s| s.latency_error(), |s| s.latency_relative_error())
    }

    fn compute_accuracy<F, G>(&self, name: &str, abs_err: F, rel_err: G) -> AccuracyMetric
    where
        F: Fn(&CostCalibrationSample) -> f64,
        G: Fn(&CostCalibrationSample) -> f64,
    {
        if self.samples.is_empty() {
            return AccuracyMetric {
                name: name.into(),
                mae: 0.0,
                mre: 0.0,
                max_re: 0.0,
                sample_count: 0,
            };
        }
        let n = self.samples.len() as f64;
        let mae: f64 = self.samples.iter().map(&abs_err).sum::<f64>() / n;
        let mre: f64 = self.samples.iter().map(&rel_err).sum::<f64>() / n;
        let max_re: f64 = self.samples.iter().map(&rel_err).fold(0.0_f64, f64::max);
        AccuracyMetric {
            name: name.into(),
            mae,
            mre,
            max_re,
            sample_count: self.samples.len(),
        }
    }
}

// ── Calibration suite ──────────────────────────────────────────────

/// Runs calibration tests across targets and produces reports.
pub struct CalibrationSuite {
    targets: BTreeMap<String, CalibrationTarget>,
    energy: BTreeMap<String, Vec<EnergySample>>,
}

impl Default for CalibrationSuite {
    fn default() -> Self {
        Self::new()
    }
}

impl CalibrationSuite {
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
            energy: BTreeMap::new(),
        }
    }

    /// Record a measured fuel/energy sample for `target`.
    pub fn add_energy_sample(&mut self, target: &str, sample: EnergySample) {
        self.energy.entry(target.to_string()).or_default().push(sample);
    }

    /// Fuel calibrated to joules on `target`, from its energy samples.
    pub fn fuel_calibration(&self, target: &str) -> Option<FuelCalibration> {
        self.energy.get(target).and_then(|s| fit_fuel_to_joules(s))
    }

    pub fn add_sample(&mut self, sample: CostCalibrationSample) {
        let target = self
            .targets
            .entry(sample.target.clone())
            .or_insert_with(|| CalibrationTarget::new(&sample.target));
        target.add_sample(sample);
    }

    pub fn target_names(&self) -> Vec<String> {
        self.targets.keys().cloned().collect()
    }

    pub fn target(&self, name: &str) -> Option<&CalibrationTarget> {
        self.targets.get(name)
    }

    /// Generate a full calibration report.
    pub fn report(&self) -> CalibrationReport {
        let mut target_reports = Vec::new();
        for (name, target) in &self.targets {
            let cycles_acc = target.cycles_accuracy();
            let latency_acc = target.latency_accuracy();
            target_reports.push(TargetReport {
                target_name: name.clone(),
                sample_count: target.samples.len(),
                cycles_grade: AccuracyGrade::from_mre(cycles_acc.mre),
                latency_grade: AccuracyGrade::from_mre(latency_acc.mre),
                cycles_accuracy: cycles_acc,
                latency_accuracy: latency_acc,
            });
        }
        let fuel = self
            .energy
            .keys()
            .filter_map(|t| self.fuel_calibration(t).map(|c| (t.clone(), c)))
            .collect();
        CalibrationReport {
            target_reports,
            fuel,
        }
    }

    /// Load the built-in sample set — **simulated, not measured**.
    ///
    /// Both halves of every sample are literals typed into this file: the
    /// `measured_*` fields as much as the `estimated_*` ones. Running a report
    /// over them grades how well one table of guesses agrees with another, and
    /// a grade of `Excellent` means the two tables were written to match.
    ///
    /// The name said "benchmark samples", which is what these will be once
    /// something profiles hardware and feeds the result in. Until then a caller
    /// that treats a report built from this as calibration has measured
    /// nothing, and `MAGE_ONTOLOGY.md` said exactly that until it was checked.
    pub fn load_standard_benchmarks(&mut self) {
        // Simulated measured costs for x86_64 target.
        // In a real implementation, these come from hardware profiling data.
        let benchmarks = vec![
            // (construct, target, measured_cycles, mem, lat, est_cycles, est_mem, est_lat)
            ("Vec::push", "x86_64", 6, 0, 4, 5, 0, 3),
            ("Vec::push (realloc)", "x86_64", 55, 2048, 45, 50, 2048, 40),
            ("stack array", "x86_64", 1, 0, 1, 1, 0, 1),
            ("HashMap insert", "x86_64", 22, 0, 17, 20, 0, 15),
            ("Box alloc", "x86_64", 35, 8, 28, 30, 8, 25),
            ("Rc clone", "x86_64", 4, 0, 4, 3, 0, 3),
            ("Arc clone", "x86_64", 9, 0, 9, 8, 0, 8),
            ("String alloc", "x86_64", 32, 24, 28, 30, 24, 25),
            ("format!", "x86_64", 45, 72, 38, 40, 64, 35),
            ("async fn", "x86_64", 6, 72, 6, 5, 64, 5),
            ("Mutex.lock", "x86_64", 18, 0, 18, 15, 0, 15),
            ("Swarm.broadcast", "x86_64", 120, 0, 230, 100, 0, 200),
            ("Bus.publish", "x86_64", 55, 140, 90, 50, 128, 80),
            // aarch64 targets
            ("Vec::push", "aarch64", 4, 0, 3, 5, 0, 3),
            ("Box alloc", "aarch64", 28, 8, 22, 30, 8, 25),
            ("Arc clone", "aarch64", 7, 0, 7, 8, 0, 8),
            ("Swarm.broadcast", "aarch64", 90, 0, 180, 100, 0, 200),
            // wasm32 targets
            ("Vec::push", "wasm32", 8, 0, 6, 5, 0, 3),
            ("Box alloc", "wasm32", 40, 8, 35, 30, 8, 25),
            ("Swarm.broadcast", "wasm32", 150, 0, 300, 100, 0, 200),
        ];

        for (construct, target, mc, mm, ml, ec, em, el) in benchmarks {
            self.add_sample(CostCalibrationSample {
                construct: construct.into(),
                target: target.into(),
                measured_cycles: mc,
                measured_memory: mm,
                measured_latency_ns: ml,
                estimated_cycles: ec,
                estimated_memory: em,
                estimated_latency_ns: el,
            });
        }
    }
}

// ── Fuel against joules ────────────────────────────────────────────

/// One measurement: the fuel a phase of work spent, and the joules the
/// meters recorded over the same phase.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnergySample {
    pub fuel: u64,
    pub joules: f64,
    /// False when any meter behind `joules` was an estimate (a wall-clock
    /// power figure, say) rather than a hardware counter.
    pub measured: bool,
}

/// The least-squares line `joules = overhead_joules + joules_per_fuel * fuel`.
///
/// The intercept matters. A meter reads everything its device did over the
/// phase, including idle draw and work that spends no fuel (parsing,
/// typechecking), so a ratio of totals would charge all of that to fuel.
/// The slope is the marginal cost of fuel; `r_squared` says how much of the
/// variation in energy fuel explains, which is how good a proxy it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FuelCalibration {
    pub joules_per_fuel: f64,
    pub overhead_joules: f64,
    pub r_squared: f64,
    pub samples: usize,
    /// True only if every sample was measured; otherwise the fit inherits the
    /// estimate's label.
    pub all_measured: bool,
}

impl FuelCalibration {
    /// Predicted joules for `fuel`.
    pub fn joules(&self, fuel: u64) -> f64 {
        self.overhead_joules + self.joules_per_fuel * fuel as f64
    }
}

/// Fit fuel to joules. `None` with fewer than three samples, with fuel that
/// never varied (the slope is then undetermined, not zero), or with a
/// non-finite reading.
pub fn fit_fuel_to_joules(samples: &[EnergySample]) -> Option<FuelCalibration> {
    if samples.len() < 3 || samples.iter().any(|s| !s.joules.is_finite()) {
        return None;
    }
    let n = samples.len() as f64;
    let mx = samples.iter().map(|s| s.fuel as f64).sum::<f64>() / n;
    let my = samples.iter().map(|s| s.joules).sum::<f64>() / n;
    let sxx: f64 = samples.iter().map(|s| (s.fuel as f64 - mx).powi(2)).sum();
    if sxx == 0.0 {
        return None;
    }
    let sxy: f64 = samples.iter().map(|s| (s.fuel as f64 - mx) * (s.joules - my)).sum();
    let syy: f64 = samples.iter().map(|s| (s.joules - my).powi(2)).sum();
    let slope = sxy / sxx;
    // Energy that never varied leaves nothing for fuel to explain: 0, not
    // the 1 a flat line's zero residuals would suggest.
    let r_squared = if syy == 0.0 { 0.0 } else { (sxy * sxy) / (sxx * syy) };
    Some(FuelCalibration {
        joules_per_fuel: slope,
        overhead_joules: my - slope * mx,
        r_squared,
        samples: samples.len(),
        all_measured: samples.iter().all(|s| s.measured),
    })
}

// ── Calibration report ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TargetReport {
    pub target_name: String,
    pub sample_count: usize,
    pub cycles_grade: AccuracyGrade,
    pub latency_grade: AccuracyGrade,
    pub cycles_accuracy: AccuracyMetric,
    pub latency_accuracy: AccuracyMetric,
}

#[derive(Debug, Clone)]
pub struct CalibrationReport {
    pub target_reports: Vec<TargetReport>,
    /// Fuel calibrated to joules, per target with enough energy samples.
    pub fuel: Vec<(String, FuelCalibration)>,
}

impl CalibrationReport {
    /// Overall grade across all targets (worst grade wins).
    pub fn overall_grade(&self) -> AccuracyGrade {
        let mut worst = AccuracyGrade::Excellent;
        for tr in &self.target_reports {
            worst = worse_grade(worst, tr.cycles_grade);
            worst = worse_grade(worst, tr.latency_grade);
        }
        worst
    }

    /// Format the report as a human-readable string.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("=== Cost Model Calibration Report ===\n\n");
        for tr in &self.target_reports {
            out.push_str(&format!(
                "Target: {} ({} samples)\n",
                tr.target_name, tr.sample_count
            ));
            out.push_str(&format!(
                "  Cycles:  MAE={:.1}, MRE={:.1}%, MaxRE={:.1}% — {}\n",
                tr.cycles_accuracy.mae,
                tr.cycles_accuracy.mre * 100.0,
                tr.cycles_accuracy.max_re * 100.0,
                tr.cycles_grade.label(),
            ));
            out.push_str(&format!(
                "  Latency: MAE={:.1}, MRE={:.1}%, MaxRE={:.1}% — {}\n",
                tr.latency_accuracy.mae,
                tr.latency_accuracy.mre * 100.0,
                tr.latency_accuracy.max_re * 100.0,
                tr.latency_grade.label(),
            ));
            out.push('\n');
        }
        for (target, c) in &self.fuel {
            out.push_str(&format!(
                "Fuel on {target}: {:.3e} J/fuel + {:.3} J overhead, r²={:.2}, {} samples, {}\n",
                c.joules_per_fuel,
                c.overhead_joules,
                c.r_squared,
                c.samples,
                if c.all_measured { "measured" } else { "includes estimates" },
            ));
        }
        out.push_str(&format!("Overall: {}\n", self.overall_grade().label()));
        out
    }
}

fn worse_grade(a: AccuracyGrade, b: AccuracyGrade) -> AccuracyGrade {
    fn rank(g: AccuracyGrade) -> u8 {
        match g {
            AccuracyGrade::Excellent => 0,
            AccuracyGrade::Good => 1,
            AccuracyGrade::Fair => 2,
            AccuracyGrade::Poor => 3,
        }
    }
    if rank(b) > rank(a) { b } else { a }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // A calibration sample is six measured numbers plus its two identifiers;
    // this is the record constructor for a test fixture.
    #[allow(clippy::too_many_arguments)]
    fn sample(construct: &str, target: &str, mc: u64, mm: u64, ml: u64, ec: u64, em: u64, el: u64) -> CostCalibrationSample {
        CostCalibrationSample {
            construct: construct.into(),
            target: target.into(),
            measured_cycles: mc,
            measured_memory: mm,
            measured_latency_ns: ml,
            estimated_cycles: ec,
            estimated_memory: em,
            estimated_latency_ns: el,
        }
    }

    // ── CostCalibrationSample ─────────────────────────────────────

    #[test]
    fn sample_cycles_error_exact() {
        let s = sample("push", "x86_64", 10, 0, 5, 10, 0, 5);
        assert!((s.cycles_error()).abs() < 0.01);
        assert!((s.cycles_relative_error()).abs() < 0.01);
    }

    #[test]
    fn sample_cycles_error_off() {
        let s = sample("push", "x86_64", 10, 0, 5, 12, 0, 7);
        assert!((s.cycles_error() - 2.0).abs() < 0.01);
        assert!((s.cycles_relative_error() - 0.20).abs() < 0.01);
    }

    #[test]
    fn sample_latency_relative_error() {
        let s = sample("push", "x86_64", 10, 0, 100, 10, 0, 120);
        assert!((s.latency_relative_error() - 0.20).abs() < 0.01);
    }

    #[test]
    fn sample_zero_measured() {
        let s = sample("noop", "x86_64", 0, 0, 0, 5, 0, 0);
        assert!((s.cycles_relative_error() - 1.0).abs() < 0.01);
        assert!((s.latency_relative_error()).abs() < 0.01);
    }

    // ── AccuracyGrade ─────────────────────────────────────────────

    #[test]
    fn grade_thresholds() {
        assert_eq!(AccuracyGrade::from_mre(0.05), AccuracyGrade::Excellent);
        assert_eq!(AccuracyGrade::from_mre(0.15), AccuracyGrade::Good);
        assert_eq!(AccuracyGrade::from_mre(0.35), AccuracyGrade::Fair);
        assert_eq!(AccuracyGrade::from_mre(0.75), AccuracyGrade::Poor);
    }

    // ── CalibrationTarget ─────────────────────────────────────────

    #[test]
    fn target_accuracy_perfect() {
        let mut target = CalibrationTarget::new("x86_64");
        target.add_sample(sample("a", "x86_64", 10, 0, 5, 10, 0, 5));
        target.add_sample(sample("b", "x86_64", 20, 0, 10, 20, 0, 10));
        let acc = target.cycles_accuracy();
        assert!((acc.mre).abs() < 0.01);
        assert_eq!(acc.sample_count, 2);
    }

    #[test]
    fn target_accuracy_imperfect() {
        let mut target = CalibrationTarget::new("x86_64");
        target.add_sample(sample("a", "x86_64", 10, 0, 100, 12, 0, 110));
        let acc = target.cycles_accuracy();
        assert!((acc.mre - 0.20).abs() < 0.01);
    }

    #[test]
    fn target_empty() {
        let target = CalibrationTarget::new("x86_64");
        let acc = target.cycles_accuracy();
        assert_eq!(acc.sample_count, 0);
        assert_eq!(acc.mae, 0.0);
    }

    // ── Fuel against joules ───────────────────────────────────────

    fn es(fuel: u64, joules: f64, measured: bool) -> EnergySample {
        EnergySample { fuel, joules, measured }
    }

    #[test]
    fn a_linear_cost_is_recovered_with_its_overhead() {
        // 2 J of idle draw per phase, 0.001 J per unit of fuel.
        let s: Vec<_> = [100u64, 5_000, 20_000, 80_000].iter().map(|&f| es(f, 2.0 + 0.001 * f as f64, true)).collect();
        let c = fit_fuel_to_joules(&s).unwrap();
        assert!((c.joules_per_fuel - 0.001).abs() < 1e-12, "{c:?}");
        assert!((c.overhead_joules - 2.0).abs() < 1e-9, "{c:?}");
        assert!((c.r_squared - 1.0).abs() < 1e-12);
        assert!((c.joules(10_000) - 12.0).abs() < 1e-9);
        assert!(c.all_measured);
    }

    #[test]
    fn energy_unrelated_to_fuel_has_no_explanatory_power() {
        let s = vec![es(10, 5.0, true), es(20, 1.0, true), es(30, 5.0, true), es(40, 1.0, true)];
        let c = fit_fuel_to_joules(&s).unwrap();
        assert!(c.r_squared < 0.25, "{c:?}");
    }

    #[test]
    fn an_estimate_anywhere_labels_the_fit() {
        let s = vec![es(1, 1.0, true), es(2, 2.0, false), es(3, 3.0, true)];
        assert!(!fit_fuel_to_joules(&s).unwrap().all_measured);
    }

    #[test]
    fn an_undetermined_fit_is_refused_rather_than_guessed() {
        assert!(fit_fuel_to_joules(&[es(1, 1.0, true), es(2, 2.0, true)]).is_none(), "two points");
        assert!(fit_fuel_to_joules(&[es(5, 1.0, true), es(5, 2.0, true), es(5, 3.0, true)]).is_none(), "constant fuel");
        assert!(fit_fuel_to_joules(&[es(1, 1.0, true), es(2, f64::NAN, true), es(3, 3.0, true)]).is_none(), "NaN");
    }

    #[test]
    fn the_suite_reports_fuel_per_target_from_samples_fed_in() {
        let mut suite = CalibrationSuite::new();
        for f in [1_000u64, 2_000, 4_000] {
            suite.add_energy_sample("arena-cpu", es(f, 0.5 + 0.002 * f as f64, false));
        }
        suite.add_energy_sample("thin", es(1, 1.0, true));
        let r = suite.report();
        assert_eq!(r.fuel.len(), 1, "a target without enough samples has no calibration");
        assert_eq!(r.fuel[0].0, "arena-cpu");
        assert!(r.to_text().contains("J/fuel"));
        assert!(r.to_text().contains("includes estimates"));
    }

    // ── CalibrationSuite ──────────────────────────────────────────

    #[test]
    fn suite_multi_target() {
        let mut suite = CalibrationSuite::new();
        suite.add_sample(sample("a", "x86_64", 10, 0, 5, 10, 0, 5));
        suite.add_sample(sample("a", "aarch64", 8, 0, 4, 8, 0, 4));
        assert_eq!(suite.target_names().len(), 2);
    }

    #[test]
    fn suite_standard_benchmarks() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        assert!(suite.target_names().contains(&"x86_64".to_string()));
        assert!(suite.target_names().contains(&"aarch64".to_string()));
        assert!(suite.target_names().contains(&"wasm32".to_string()));
    }

    #[test]
    fn suite_report_generation() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        let report = suite.report();
        assert_eq!(report.target_reports.len(), 3);
    }

    #[test]
    fn suite_x86_cycles_grade() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        let report = suite.report();
        let x86 = report.target_reports.iter().find(|r| r.target_name == "x86_64").unwrap();
        // The built-in benchmarks have low error — should be Excellent or Good.
        assert!(matches!(x86.cycles_grade, AccuracyGrade::Excellent | AccuracyGrade::Good));
    }

    #[test]
    fn suite_wasm_higher_error() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        let report = suite.report();
        let wasm = report.target_reports.iter().find(|r| r.target_name == "wasm32").unwrap();
        let x86 = report.target_reports.iter().find(|r| r.target_name == "x86_64").unwrap();
        // wasm32 estimates are less accurate than x86_64.
        assert!(wasm.cycles_accuracy.mre >= x86.cycles_accuracy.mre);
    }

    // ── CalibrationReport ─────────────────────────────────────────

    #[test]
    fn report_overall_grade() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        let report = suite.report();
        // Should have some grade — not crash.
        let _grade = report.overall_grade();
    }

    #[test]
    fn report_text_format() {
        let mut suite = CalibrationSuite::new();
        suite.load_standard_benchmarks();
        let text = suite.report().to_text();
        assert!(text.contains("Cost Model Calibration Report"));
        assert!(text.contains("x86_64"));
        assert!(text.contains("Overall:"));
    }

    // ── Edge cases ────────────────────────────────────────────────

    #[test]
    fn memory_error_calculation() {
        let s = sample("alloc", "x86_64", 10, 100, 5, 10, 150, 5);
        assert!((s.memory_error() - 50.0).abs() < 0.01);
    }

    #[test]
    fn worse_grade_fn() {
        assert_eq!(worse_grade(AccuracyGrade::Excellent, AccuracyGrade::Poor), AccuracyGrade::Poor);
        assert_eq!(worse_grade(AccuracyGrade::Fair, AccuracyGrade::Good), AccuracyGrade::Fair);
    }
}
