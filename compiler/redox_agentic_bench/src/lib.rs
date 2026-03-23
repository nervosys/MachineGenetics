// redox_agentic_bench: Agentic benchmarking suite — token throughput,
// parse error rate, synthesis success rate, swarm latency.

// ---------------------------------------------------------------------------
// Metric kind
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgenticMetric {
    TokenThroughput,
    ParseErrorRate,
    SynthesisSuccessRate,
    SwarmLatency,
    RoundTripTime,
    ContextWindowUtilisation,
}

impl AgenticMetric {
    pub fn label(self) -> &'static str {
        match self {
            Self::TokenThroughput => "token-throughput",
            Self::ParseErrorRate => "parse-error-rate",
            Self::SynthesisSuccessRate => "synthesis-success-rate",
            Self::SwarmLatency => "swarm-latency",
            Self::RoundTripTime => "round-trip-time",
            Self::ContextWindowUtilisation => "context-window-utilisation",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::TokenThroughput => "tok/s",
            Self::ParseErrorRate => "%",
            Self::SynthesisSuccessRate => "%",
            Self::SwarmLatency => "ms",
            Self::RoundTripTime => "ms",
            Self::ContextWindowUtilisation => "%",
        }
    }
}

// ---------------------------------------------------------------------------
// Sample
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BenchSample {
    pub label: String,
    pub metric: AgenticMetric,
    pub value: f64,
}

// ---------------------------------------------------------------------------
// Scenario
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BenchScenario {
    pub name: String,
    pub metric: AgenticMetric,
    pub samples: Vec<BenchSample>,
}

impl BenchScenario {
    pub fn new(name: &str, metric: AgenticMetric) -> Self {
        Self { name: name.to_string(), metric, samples: Vec::new() }
    }

    pub fn add(&mut self, label: &str, value: f64) {
        self.samples.push(BenchSample {
            label: label.to_string(),
            metric: self.metric,
            value,
        });
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().map(|s| s.value).sum::<f64>() / self.samples.len() as f64
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().map(|s| s.value).fold(f64::INFINITY, f64::min)
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().map(|s| s.value).fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

// ---------------------------------------------------------------------------
// Scenario result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioResult {
    pub name: String,
    pub metric: AgenticMetric,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub samples: usize,
    pub grade: PerformanceGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceGrade {
    Excellent,
    Good,
    Acceptable,
    NeedsWork,
}

impl PerformanceGrade {
    pub fn label(self) -> &'static str {
        match self {
            Self::Excellent => "excellent",
            Self::Good => "good",
            Self::Acceptable => "acceptable",
            Self::NeedsWork => "needs-work",
        }
    }
}

// ---------------------------------------------------------------------------
// Grading functions (per metric)
// ---------------------------------------------------------------------------

pub fn grade_throughput(tok_per_sec: f64) -> PerformanceGrade {
    if tok_per_sec >= 1000.0 { PerformanceGrade::Excellent }
    else if tok_per_sec >= 500.0 { PerformanceGrade::Good }
    else if tok_per_sec >= 100.0 { PerformanceGrade::Acceptable }
    else { PerformanceGrade::NeedsWork }
}

pub fn grade_error_rate(pct: f64) -> PerformanceGrade {
    if pct <= 1.0 { PerformanceGrade::Excellent }
    else if pct <= 5.0 { PerformanceGrade::Good }
    else if pct <= 15.0 { PerformanceGrade::Acceptable }
    else { PerformanceGrade::NeedsWork }
}

pub fn grade_success_rate(pct: f64) -> PerformanceGrade {
    if pct >= 95.0 { PerformanceGrade::Excellent }
    else if pct >= 80.0 { PerformanceGrade::Good }
    else if pct >= 60.0 { PerformanceGrade::Acceptable }
    else { PerformanceGrade::NeedsWork }
}

pub fn grade_latency(ms: f64) -> PerformanceGrade {
    if ms <= 50.0 { PerformanceGrade::Excellent }
    else if ms <= 200.0 { PerformanceGrade::Good }
    else if ms <= 1000.0 { PerformanceGrade::Acceptable }
    else { PerformanceGrade::NeedsWork }
}

pub fn grade_scenario(scenario: &BenchScenario) -> PerformanceGrade {
    let m = scenario.mean();
    match scenario.metric {
        AgenticMetric::TokenThroughput => grade_throughput(m),
        AgenticMetric::ParseErrorRate => grade_error_rate(m),
        AgenticMetric::SynthesisSuccessRate => grade_success_rate(m),
        AgenticMetric::SwarmLatency | AgenticMetric::RoundTripTime => grade_latency(m),
        AgenticMetric::ContextWindowUtilisation => grade_success_rate(m),
    }
}

pub fn evaluate_scenario(scenario: &BenchScenario) -> ScenarioResult {
    ScenarioResult {
        name: scenario.name.clone(),
        metric: scenario.metric,
        mean: scenario.mean(),
        min: scenario.min_val(),
        max: scenario.max_val(),
        samples: scenario.sample_count(),
        grade: grade_scenario(scenario),
    }
}

// ---------------------------------------------------------------------------
// Suite
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgenticBenchSuite {
    pub name: String,
    pub scenarios: Vec<BenchScenario>,
}

impl AgenticBenchSuite {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), scenarios: Vec::new() }
    }

    pub fn add(&mut self, scenario: BenchScenario) {
        self.scenarios.push(scenario);
    }

    pub fn evaluate_all(&self) -> Vec<ScenarioResult> {
        self.scenarios.iter().map(|s| evaluate_scenario(s)).collect()
    }

    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

pub fn format_report(results: &[ScenarioResult]) -> String {
    let mut out = String::from("=== Agentic Benchmark Report ===\n");
    for r in results {
        out.push_str(&format!(
            "  {} [{}]: mean={:.2}{} min={:.2} max={:.2} n={} grade={}\n",
            r.name, r.metric.label(), r.mean, r.metric.unit(),
            r.min, r.max, r.samples, r.grade.label(),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Pre-built suite
// ---------------------------------------------------------------------------

pub fn build_standard_suite() -> AgenticBenchSuite {
    let mut suite = AgenticBenchSuite::new("Redox Agentic Benchmark v1");

    // Token throughput
    let mut tt = BenchScenario::new("Token throughput", AgenticMetric::TokenThroughput);
    tt.add("small prompt", 1200.0);
    tt.add("medium prompt", 850.0);
    tt.add("large prompt", 420.0);
    suite.add(tt);

    // Parse error rate
    let mut pe = BenchScenario::new("Parse error rate", AgenticMetric::ParseErrorRate);
    pe.add("clean code", 0.5);
    pe.add("messy code", 8.0);
    pe.add("generated code", 2.5);
    suite.add(pe);

    // Synthesis success rate
    let mut ss = BenchScenario::new("Synthesis success", AgenticMetric::SynthesisSuccessRate);
    ss.add("simple fn", 98.0);
    ss.add("complex fn", 72.0);
    ss.add("module", 85.0);
    suite.add(ss);

    // Swarm latency
    let mut sl = BenchScenario::new("Swarm latency", AgenticMetric::SwarmLatency);
    sl.add("2-agent", 45.0);
    sl.add("5-agent", 120.0);
    sl.add("10-agent", 350.0);
    suite.add(sl);

    suite
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- AgenticMetric --
    #[test]
    fn test_metric_labels() {
        assert_eq!(AgenticMetric::TokenThroughput.label(), "token-throughput");
        assert_eq!(AgenticMetric::SwarmLatency.label(), "swarm-latency");
    }

    #[test]
    fn test_metric_units() {
        assert_eq!(AgenticMetric::TokenThroughput.unit(), "tok/s");
        assert_eq!(AgenticMetric::ParseErrorRate.unit(), "%");
    }

    // -- BenchScenario --
    #[test]
    fn test_scenario_add() {
        let mut sc = BenchScenario::new("test", AgenticMetric::TokenThroughput);
        sc.add("a", 100.0);
        sc.add("b", 200.0);
        assert_eq!(sc.sample_count(), 2);
    }

    #[test]
    fn test_scenario_mean() {
        let mut sc = BenchScenario::new("test", AgenticMetric::TokenThroughput);
        sc.add("a", 100.0);
        sc.add("b", 200.0);
        assert!((sc.mean() - 150.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scenario_min_max() {
        let mut sc = BenchScenario::new("test", AgenticMetric::TokenThroughput);
        sc.add("a", 100.0);
        sc.add("b", 200.0);
        assert!((sc.min_val() - 100.0).abs() < f64::EPSILON);
        assert!((sc.max_val() - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scenario_empty_mean() {
        let sc = BenchScenario::new("empty", AgenticMetric::TokenThroughput);
        assert_eq!(sc.mean(), 0.0);
    }

    // -- grading --
    #[test]
    fn test_grade_throughput() {
        assert_eq!(grade_throughput(1500.0), PerformanceGrade::Excellent);
        assert_eq!(grade_throughput(600.0), PerformanceGrade::Good);
        assert_eq!(grade_throughput(150.0), PerformanceGrade::Acceptable);
        assert_eq!(grade_throughput(50.0), PerformanceGrade::NeedsWork);
    }

    #[test]
    fn test_grade_error_rate() {
        assert_eq!(grade_error_rate(0.5), PerformanceGrade::Excellent);
        assert_eq!(grade_error_rate(3.0), PerformanceGrade::Good);
        assert_eq!(grade_error_rate(10.0), PerformanceGrade::Acceptable);
        assert_eq!(grade_error_rate(20.0), PerformanceGrade::NeedsWork);
    }

    #[test]
    fn test_grade_success_rate() {
        assert_eq!(grade_success_rate(98.0), PerformanceGrade::Excellent);
        assert_eq!(grade_success_rate(85.0), PerformanceGrade::Good);
        assert_eq!(grade_success_rate(70.0), PerformanceGrade::Acceptable);
        assert_eq!(grade_success_rate(40.0), PerformanceGrade::NeedsWork);
    }

    #[test]
    fn test_grade_latency() {
        assert_eq!(grade_latency(30.0), PerformanceGrade::Excellent);
        assert_eq!(grade_latency(100.0), PerformanceGrade::Good);
        assert_eq!(grade_latency(500.0), PerformanceGrade::Acceptable);
        assert_eq!(grade_latency(2000.0), PerformanceGrade::NeedsWork);
    }

    #[test]
    fn test_grade_labels() {
        assert_eq!(PerformanceGrade::Excellent.label(), "excellent");
        assert_eq!(PerformanceGrade::NeedsWork.label(), "needs-work");
    }

    // -- evaluate_scenario --
    #[test]
    fn test_evaluate_scenario() {
        let mut sc = BenchScenario::new("test", AgenticMetric::TokenThroughput);
        sc.add("a", 1200.0);
        let result = evaluate_scenario(&sc);
        assert_eq!(result.grade, PerformanceGrade::Excellent);
        assert_eq!(result.samples, 1);
    }

    // -- Suite --
    #[test]
    fn test_suite_empty() {
        let suite = AgenticBenchSuite::new("empty");
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
        let results = suite.evaluate_all();
        assert_eq!(results.len(), 4);
    }

    // -- format_report --
    #[test]
    fn test_format_report() {
        let suite = build_standard_suite();
        let results = suite.evaluate_all();
        let text = format_report(&results);
        assert!(text.contains("Benchmark Report"));
        assert!(text.contains("token-throughput"));
    }

    // -- grade_scenario delegation --
    #[test]
    fn test_grade_scenario_parse_error() {
        let mut sc = BenchScenario::new("test", AgenticMetric::ParseErrorRate);
        sc.add("a", 0.5);
        assert_eq!(grade_scenario(&sc), PerformanceGrade::Excellent);
    }

    #[test]
    fn test_grade_scenario_synthesis() {
        let mut sc = BenchScenario::new("test", AgenticMetric::SynthesisSuccessRate);
        sc.add("a", 50.0);
        assert_eq!(grade_scenario(&sc), PerformanceGrade::NeedsWork);
    }

    #[test]
    fn test_grade_scenario_rtt() {
        let mut sc = BenchScenario::new("test", AgenticMetric::RoundTripTime);
        sc.add("a", 30.0);
        assert_eq!(grade_scenario(&sc), PerformanceGrade::Excellent);
    }

    #[test]
    fn test_grade_scenario_ctx_window() {
        let mut sc = BenchScenario::new("test", AgenticMetric::ContextWindowUtilisation);
        sc.add("a", 90.0);
        assert_eq!(grade_scenario(&sc), PerformanceGrade::Good);
    }
}
