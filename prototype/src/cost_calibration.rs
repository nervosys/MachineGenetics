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
}

impl CalibrationSuite {
    pub fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
        }
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
        CalibrationReport {
            target_reports,
        }
    }

    /// Load standardized benchmark samples for built-in constructs.
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
