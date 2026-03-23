// redox_cost_calibrate: Cost model calibration suite.
//
//  Standardized benchmarks for measuring and calibrating cost oracle
//  accuracy. Includes benchmark scenarios, ground-truth measurements,
//  error metrics (MAE, RMSE, MAPE), and calibration reports.

// ---------------------------------------------------------------------------
// Cost dimension
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostDimension {
    CompileTime,
    CodeSize,
    RuntimeLatency,
    MemoryUsage,
    TokenCount,
    EnergyConsumption,
}

impl CostDimension {
    pub fn label(self) -> &'static str {
        match self {
            Self::CompileTime => "compile-time",
            Self::CodeSize => "code-size",
            Self::RuntimeLatency => "runtime-latency",
            Self::MemoryUsage => "memory-usage",
            Self::TokenCount => "token-count",
            Self::EnergyConsumption => "energy-consumption",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::CompileTime => "ms",
            Self::CodeSize => "bytes",
            Self::RuntimeLatency => "ms",
            Self::MemoryUsage => "bytes",
            Self::TokenCount => "tokens",
            Self::EnergyConsumption => "mJ",
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmark sample
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CostSample {
    pub label: String,
    pub dimension: CostDimension,
    pub predicted: f64,
    pub actual: f64,
}

impl CostSample {
    pub fn error(&self) -> f64 {
        self.predicted - self.actual
    }

    pub fn abs_error(&self) -> f64 {
        self.error().abs()
    }

    pub fn pct_error(&self) -> f64 {
        if self.actual.abs() > f64::EPSILON {
            (self.error() / self.actual).abs() * 100.0
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Error metrics
// ---------------------------------------------------------------------------

pub fn mae(samples: &[CostSample]) -> f64 {
    if samples.is_empty() { return 0.0; }
    samples.iter().map(|s| s.abs_error()).sum::<f64>() / samples.len() as f64
}

pub fn rmse(samples: &[CostSample]) -> f64 {
    if samples.is_empty() { return 0.0; }
    let mse = samples.iter().map(|s| s.error().powi(2)).sum::<f64>() / samples.len() as f64;
    mse.sqrt()
}

pub fn mape(samples: &[CostSample]) -> f64 {
    if samples.is_empty() { return 0.0; }
    samples.iter().map(|s| s.pct_error()).sum::<f64>() / samples.len() as f64
}

pub fn max_error(samples: &[CostSample]) -> f64 {
    samples.iter().map(|s| s.abs_error()).fold(0.0_f64, f64::max)
}

pub fn bias(samples: &[CostSample]) -> f64 {
    if samples.is_empty() { return 0.0; }
    samples.iter().map(|s| s.error()).sum::<f64>() / samples.len() as f64
}

// ---------------------------------------------------------------------------
// Calibration scenario
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationScenario {
    pub name: String,
    pub dimension: CostDimension,
    pub samples: Vec<CostSample>,
}

impl CalibrationScenario {
    pub fn new(name: &str, dim: CostDimension) -> Self {
        Self { name: name.to_string(), dimension: dim, samples: Vec::new() }
    }

    pub fn add_sample(&mut self, label: &str, predicted: f64, actual: f64) {
        self.samples.push(CostSample {
            label: label.to_string(),
            dimension: self.dimension,
            predicted,
            actual,
        });
    }

    pub fn mae(&self) -> f64 { mae(&self.samples) }
    pub fn rmse(&self) -> f64 { rmse(&self.samples) }
    pub fn mape(&self) -> f64 { mape(&self.samples) }
    pub fn max_error(&self) -> f64 { max_error(&self.samples) }
    pub fn bias(&self) -> f64 { bias(&self.samples) }
    pub fn sample_count(&self) -> usize { self.samples.len() }
}

// ---------------------------------------------------------------------------
// Calibration report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioReport {
    pub name: String,
    pub dimension: CostDimension,
    pub mae: f64,
    pub rmse: f64,
    pub mape: f64,
    pub max_error: f64,
    pub bias: f64,
    pub grade: CalibrationGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationGrade {
    Excellent,
    Good,
    Acceptable,
    Poor,
}

impl CalibrationGrade {
    pub fn label(self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Acceptable => "acceptable",
            Self::Poor => "poor",
        }
    }
}

pub fn grade_from_mape(mape: f64) -> CalibrationGrade {
    if mape < 5.0 { CalibrationGrade::Excellent }
    else if mape < 15.0 { CalibrationGrade::Good }
    else if mape < 30.0 { CalibrationGrade::Acceptable }
    else { CalibrationGrade::Poor }
}

pub fn evaluate_scenario(scenario: &CalibrationScenario) -> ScenarioReport {
    let m = scenario.mape();
    ScenarioReport {
        name: scenario.name.clone(),
        dimension: scenario.dimension,
        mae: scenario.mae(),
        rmse: scenario.rmse(),
        mape: m,
        max_error: scenario.max_error(),
        bias: scenario.bias(),
        grade: grade_from_mape(m),
    }
}

// ---------------------------------------------------------------------------
// Calibration suite
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CalibrationSuite {
    pub name: String,
    pub scenarios: Vec<CalibrationScenario>,
}

impl CalibrationSuite {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), scenarios: Vec::new() }
    }

    pub fn add(&mut self, scenario: CalibrationScenario) {
        self.scenarios.push(scenario);
    }

    pub fn evaluate_all(&self) -> Vec<ScenarioReport> {
        self.scenarios.iter().map(|s| evaluate_scenario(s)).collect()
    }

    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

pub fn format_report(reports: &[ScenarioReport]) -> String {
    let mut out = String::from("=== Cost Model Calibration Report ===\n");
    for r in reports {
        out.push_str(&format!(
            "  {} [{}]: MAE={:.2} RMSE={:.2} MAPE={:.1}% bias={:.2} grade={}\n",
            r.name, r.dimension.label(), r.mae, r.rmse, r.mape, r.bias, r.grade.label(),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Pre-built suite
// ---------------------------------------------------------------------------

pub fn build_standard_suite() -> CalibrationSuite {
    let mut suite = CalibrationSuite::new("Redox Cost Model Calibration v1");

    // Compile-time scenario
    let mut ct = CalibrationScenario::new("Compile-time prediction", CostDimension::CompileTime);
    ct.add_sample("small crate", 150.0, 145.0);
    ct.add_sample("medium crate", 800.0, 820.0);
    ct.add_sample("large crate", 3200.0, 3500.0);
    ct.add_sample("workspace build", 12000.0, 11800.0);
    suite.add(ct);

    // Code-size scenario
    let mut cs = CalibrationScenario::new("Code-size prediction", CostDimension::CodeSize);
    cs.add_sample("hello world", 1024.0, 980.0);
    cs.add_sample("cli tool", 51200.0, 52000.0);
    cs.add_sample("web server", 204800.0, 210000.0);
    suite.add(cs);

    // Token-count scenario
    let mut tc = CalibrationScenario::new("Token-count prediction", CostDimension::TokenCount);
    tc.add_sample("10-line fn", 50.0, 48.0);
    tc.add_sample("100-line module", 520.0, 510.0);
    tc.add_sample("1000-line crate", 5200.0, 5100.0);
    tc.add_sample("10000-line project", 52000.0, 51500.0);
    suite.add(tc);

    // Memory-usage scenario
    let mut mu = CalibrationScenario::new("Memory-usage prediction", CostDimension::MemoryUsage);
    mu.add_sample("parsing phase", 10_000_000.0, 10_500_000.0);
    mu.add_sample("type checking", 25_000_000.0, 24_000_000.0);
    mu.add_sample("codegen", 50_000_000.0, 52_000_000.0);
    suite.add(mu);

    suite
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- CostDimension --
    #[test]
    fn test_dimension_labels() {
        assert_eq!(CostDimension::CompileTime.label(), "compile-time");
        assert_eq!(CostDimension::TokenCount.label(), "token-count");
    }

    #[test]
    fn test_dimension_units() {
        assert_eq!(CostDimension::CompileTime.unit(), "ms");
        assert_eq!(CostDimension::CodeSize.unit(), "bytes");
    }

    // -- CostSample --
    #[test]
    fn test_sample_error() {
        let s = CostSample { label: "x".into(), dimension: CostDimension::CompileTime, predicted: 110.0, actual: 100.0 };
        assert!((s.error() - 10.0).abs() < f64::EPSILON);
        assert!((s.abs_error() - 10.0).abs() < f64::EPSILON);
        assert!((s.pct_error() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sample_negative_error() {
        let s = CostSample { label: "x".into(), dimension: CostDimension::CompileTime, predicted: 90.0, actual: 100.0 };
        assert!((s.error() - (-10.0)).abs() < f64::EPSILON);
    }

    // -- Error metrics --
    #[test]
    fn test_mae() {
        let samples = vec![
            CostSample { label: "a".into(), dimension: CostDimension::CompileTime, predicted: 110.0, actual: 100.0 },
            CostSample { label: "b".into(), dimension: CostDimension::CompileTime, predicted: 90.0, actual: 100.0 },
        ];
        assert!((mae(&samples) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rmse() {
        let samples = vec![
            CostSample { label: "a".into(), dimension: CostDimension::CompileTime, predicted: 110.0, actual: 100.0 },
            CostSample { label: "b".into(), dimension: CostDimension::CompileTime, predicted: 90.0, actual: 100.0 },
        ];
        assert!((rmse(&samples) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mape() {
        let samples = vec![
            CostSample { label: "a".into(), dimension: CostDimension::CompileTime, predicted: 110.0, actual: 100.0 },
        ];
        assert!((mape(&samples) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_max_error() {
        let samples = vec![
            CostSample { label: "a".into(), dimension: CostDimension::CompileTime, predicted: 105.0, actual: 100.0 },
            CostSample { label: "b".into(), dimension: CostDimension::CompileTime, predicted: 120.0, actual: 100.0 },
        ];
        assert!((max_error(&samples) - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bias_zero_when_balanced() {
        let samples = vec![
            CostSample { label: "a".into(), dimension: CostDimension::CompileTime, predicted: 110.0, actual: 100.0 },
            CostSample { label: "b".into(), dimension: CostDimension::CompileTime, predicted: 90.0, actual: 100.0 },
        ];
        assert!((bias(&samples)).abs() < f64::EPSILON);
    }

    // -- CalibrationGrade --
    #[test]
    fn test_grade_from_mape() {
        assert_eq!(grade_from_mape(3.0), CalibrationGrade::Excellent);
        assert_eq!(grade_from_mape(10.0), CalibrationGrade::Good);
        assert_eq!(grade_from_mape(20.0), CalibrationGrade::Acceptable);
        assert_eq!(grade_from_mape(50.0), CalibrationGrade::Poor);
    }

    #[test]
    fn test_grade_labels() {
        assert_eq!(CalibrationGrade::Excellent.label(), "excellent");
    }

    // -- CalibrationScenario --
    #[test]
    fn test_scenario_add_sample() {
        let mut sc = CalibrationScenario::new("test", CostDimension::CompileTime);
        sc.add_sample("a", 100.0, 95.0);
        sc.add_sample("b", 200.0, 190.0);
        assert_eq!(sc.sample_count(), 2);
    }

    #[test]
    fn test_scenario_metrics() {
        let mut sc = CalibrationScenario::new("test", CostDimension::CompileTime);
        sc.add_sample("a", 110.0, 100.0);
        sc.add_sample("b", 90.0, 100.0);
        assert!((sc.mae() - 10.0).abs() < f64::EPSILON);
        assert!(sc.bias().abs() < f64::EPSILON);
    }

    // -- evaluate_scenario --
    #[test]
    fn test_evaluate_scenario() {
        let mut sc = CalibrationScenario::new("test", CostDimension::CompileTime);
        sc.add_sample("a", 102.0, 100.0);
        let report = evaluate_scenario(&sc);
        assert_eq!(report.grade, CalibrationGrade::Excellent);
    }

    // -- CalibrationSuite --
    #[test]
    fn test_suite_empty() {
        let suite = CalibrationSuite::new("empty");
        assert!(suite.is_empty());
    }

    #[test]
    fn test_standard_suite_len() {
        let suite = build_standard_suite();
        assert_eq!(suite.len(), 4);
    }

    #[test]
    fn test_standard_suite_evaluate() {
        let suite = build_standard_suite();
        let reports = suite.evaluate_all();
        assert_eq!(reports.len(), 4);
    }

    // -- format_report --
    #[test]
    fn test_format_report() {
        let suite = build_standard_suite();
        let reports = suite.evaluate_all();
        let text = format_report(&reports);
        assert!(text.contains("Calibration Report"));
        assert!(text.contains("compile-time"));
    }

    // -- empty metrics --
    #[test]
    fn test_empty_mae() {
        assert_eq!(mae(&[]), 0.0);
    }

    #[test]
    fn test_empty_rmse() {
        assert_eq!(rmse(&[]), 0.0);
    }

    #[test]
    fn test_empty_mape() {
        assert_eq!(mape(&[]), 0.0);
    }

    #[test]
    fn test_empty_bias() {
        assert_eq!(bias(&[]), 0.0);
    }

    // -- grade boundary --
    #[test]
    fn test_grade_boundaries() {
        assert_eq!(grade_from_mape(4.99), CalibrationGrade::Excellent);
        assert_eq!(grade_from_mape(5.0), CalibrationGrade::Good);
        assert_eq!(grade_from_mape(14.99), CalibrationGrade::Good);
        assert_eq!(grade_from_mape(15.0), CalibrationGrade::Acceptable);
    }
}
