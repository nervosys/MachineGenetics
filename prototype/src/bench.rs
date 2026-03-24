// ── Agentic Benchmarking Suite ─────────────────────────────────────
//
// Metrics collection and benchmarking framework for the MechGen agentic
// compiler pipeline:
//
//   1. Token throughput — tokens processed per unit time
//   2. Parse error rate — errors per source unit
//   3. Synthesis success rate — successful generations vs attempts
//   4. Swarm latency — task dispatch and completion timing
//   5. Benchmark runner with named suites and aggregation

use std::collections::BTreeMap;

// ── Metric sample ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MetricSample {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: u64,
    pub tags: BTreeMap<String, String>,
}

impl MetricSample {
    pub fn new(name: &str, value: f64, unit: &str) -> Self {
        Self { name: name.into(), value, unit: unit.into(), timestamp: 0, tags: BTreeMap::new() }
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }
}

// ── Metric series ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MetricSeries {
    pub name: String,
    pub unit: String,
    samples: Vec<f64>,
}

impl MetricSeries {
    pub fn new(name: &str, unit: &str) -> Self {
        Self { name: name.into(), unit: unit.into(), samples: Vec::new() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.sum() / self.samples.len() as f64
    }

    pub fn min(&self) -> f64 {
        self.samples.iter().copied().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.samples.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn p50(&self) -> f64 {
        self.percentile(50.0)
    }

    pub fn p99(&self) -> f64 {
        self.percentile(99.0)
    }

    fn percentile(&self, pct: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn summary(&self) -> MetricSummary {
        MetricSummary {
            name: self.name.clone(),
            unit: self.unit.clone(),
            count: self.count(),
            mean: self.mean(),
            min: self.min(),
            max: self.max(),
            p50: self.p50(),
            p99: self.p99(),
        }
    }
}

// ── Metric summary ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MetricSummary {
    pub name: String,
    pub unit: String,
    pub count: usize,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p99: f64,
}

impl MetricSummary {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"unit\":\"{}\",\"count\":{},\"mean\":{:.2},\"min\":{:.2},\"max\":{:.2},\"p50\":{:.2},\"p99\":{:.2}}}",
            self.name, self.unit, self.count, self.mean, self.min, self.max, self.p50, self.p99
        )
    }
}

// ── Benchmark result ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub suite_name: String,
    pub passed: bool,
    pub metrics: Vec<MetricSummary>,
    pub issues: Vec<String>,
}

// ── Token throughput tracker ───────────────────────────────────────

pub struct TokenThroughput {
    series: MetricSeries,
}

impl TokenThroughput {
    pub fn new() -> Self {
        Self { series: MetricSeries::new("token_throughput", "tokens/ms") }
    }

    /// Record a measurement: tokens processed in elapsed_ms.
    pub fn record(&mut self, tokens: u64, elapsed_ms: f64) {
        if elapsed_ms > 0.0 {
            self.series.record(tokens as f64 / elapsed_ms);
        }
    }

    pub fn summary(&self) -> MetricSummary {
        self.series.summary()
    }
}

// ── Parse error rate tracker ───────────────────────────────────────

pub struct ParseErrorRate {
    total_units: u64,
    error_units: u64,
    errors_per_unit: MetricSeries,
}

impl ParseErrorRate {
    pub fn new() -> Self {
        Self {
            total_units: 0,
            error_units: 0,
            errors_per_unit: MetricSeries::new("parse_error_rate", "errors/unit"),
        }
    }

    /// Record a parse attempt: number of errors for one source unit.
    pub fn record(&mut self, error_count: u64) {
        self.total_units += 1;
        if error_count > 0 {
            self.error_units += 1;
        }
        self.errors_per_unit.record(error_count as f64);
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_units == 0 {
            return 0.0;
        }
        self.error_units as f64 / self.total_units as f64
    }

    pub fn summary(&self) -> MetricSummary {
        self.errors_per_unit.summary()
    }
}

// ── Synthesis success rate tracker ─────────────────────────────────

pub struct SynthesisRate {
    attempts: u64,
    successes: u64,
    latency: MetricSeries,
}

impl SynthesisRate {
    pub fn new() -> Self {
        Self { attempts: 0, successes: 0, latency: MetricSeries::new("synthesis_latency", "ms") }
    }

    /// Record a synthesis attempt.
    pub fn record(&mut self, success: bool, latency_ms: f64) {
        self.attempts += 1;
        if success {
            self.successes += 1;
        }
        self.latency.record(latency_ms);
    }

    pub fn success_rate(&self) -> f64 {
        if self.attempts == 0 {
            return 0.0;
        }
        self.successes as f64 / self.attempts as f64
    }

    pub fn latency_summary(&self) -> MetricSummary {
        self.latency.summary()
    }
}

// ── Swarm latency tracker ──────────────────────────────────────────

pub struct SwarmLatency {
    dispatch_latency: MetricSeries,
    completion_latency: MetricSeries,
    queue_depth: MetricSeries,
}

impl SwarmLatency {
    pub fn new() -> Self {
        Self {
            dispatch_latency: MetricSeries::new("swarm_dispatch_latency", "ms"),
            completion_latency: MetricSeries::new("swarm_completion_latency", "ms"),
            queue_depth: MetricSeries::new("swarm_queue_depth", "tasks"),
        }
    }

    pub fn record_dispatch(&mut self, latency_ms: f64) {
        self.dispatch_latency.record(latency_ms);
    }

    pub fn record_completion(&mut self, latency_ms: f64) {
        self.completion_latency.record(latency_ms);
    }

    pub fn record_queue_depth(&mut self, depth: u64) {
        self.queue_depth.record(depth as f64);
    }

    pub fn dispatch_summary(&self) -> MetricSummary {
        self.dispatch_latency.summary()
    }

    pub fn completion_summary(&self) -> MetricSummary {
        self.completion_latency.summary()
    }
}

// ── Benchmark runner ───────────────────────────────────────────────

/// Runs named benchmark suites and collects results.
pub struct BenchmarkRunner {
    suites: BTreeMap<String, Box<dyn Fn() -> BenchmarkResult>>,
    results: Vec<BenchmarkResult>,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        Self { suites: BTreeMap::new(), results: Vec::new() }
    }

    pub fn register<F: Fn() -> BenchmarkResult + 'static>(&mut self, name: &str, f: F) {
        self.suites.insert(name.into(), Box::new(f));
    }

    pub fn run_all(&mut self) {
        let names: Vec<String> = self.suites.keys().cloned().collect();
        for name in names {
            if let Some(f) = self.suites.get(&name) {
                let result = f();
                self.results.push(result);
            }
        }
    }

    pub fn run_suite(&mut self, name: &str) -> Option<BenchmarkResult> {
        let f = self.suites.get(name)?;
        let result = f();
        self.results.push(result.clone());
        Some(result)
    }

    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }

    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }

    pub fn report(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Benchmark Report: {} suites, {} passed, {} failed\n",
            self.results.len(),
            self.passed_count(),
            self.failed_count()
        ));
        for r in &self.results {
            out.push_str(&format!(
                "  [{}] {} — {} metrics\n",
                if r.passed { "PASS" } else { "FAIL" },
                r.suite_name,
                r.metrics.len()
            ));
            for issue in &r.issues {
                out.push_str(&format!("    ! {}\n", issue));
            }
        }
        out
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MetricSeries ──────────────────────────────────────────────

    #[test]
    fn series_basic_stats() {
        let mut s = MetricSeries::new("test", "ms");
        s.record(10.0);
        s.record(20.0);
        s.record(30.0);
        assert_eq!(s.count(), 3);
        assert!((s.mean() - 20.0).abs() < 0.01);
        assert!((s.min() - 10.0).abs() < 0.01);
        assert!((s.max() - 30.0).abs() < 0.01);
    }

    #[test]
    fn series_percentiles() {
        let mut s = MetricSeries::new("test", "ms");
        for i in 1..=100 {
            s.record(i as f64);
        }
        assert!((s.p50() - 50.0).abs() < 1.01);
        assert!((s.p99() - 99.0).abs() < 1.01);
    }

    #[test]
    fn series_empty() {
        let s = MetricSeries::new("test", "ms");
        assert_eq!(s.mean(), 0.0);
        assert_eq!(s.p50(), 0.0);
    }

    #[test]
    fn series_summary_json() {
        let mut s = MetricSeries::new("latency", "ms");
        s.record(5.0);
        let json = s.summary().to_json();
        assert!(json.contains("\"name\":\"latency\""));
        assert!(json.contains("\"count\":1"));
    }

    // ── TokenThroughput ───────────────────────────────────────────

    #[test]
    fn token_throughput() {
        let mut tt = TokenThroughput::new();
        tt.record(1000, 10.0); // 100 tokens/ms
        tt.record(2000, 10.0); // 200 tokens/ms
        let s = tt.summary();
        assert!((s.mean - 150.0).abs() < 0.01);
    }

    #[test]
    fn token_throughput_zero_time() {
        let mut tt = TokenThroughput::new();
        tt.record(1000, 0.0); // should be skipped
        assert_eq!(tt.summary().count, 0);
    }

    // ── ParseErrorRate ────────────────────────────────────────────

    #[test]
    fn parse_error_rate() {
        let mut per = ParseErrorRate::new();
        per.record(0); // clean
        per.record(3); // 3 errors
        per.record(0); // clean
        per.record(1); // 1 error
        assert!((per.error_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_error_rate_all_clean() {
        let mut per = ParseErrorRate::new();
        per.record(0);
        per.record(0);
        assert!((per.error_rate()).abs() < 0.01);
    }

    // ── SynthesisRate ─────────────────────────────────────────────

    #[test]
    fn synthesis_rate() {
        let mut sr = SynthesisRate::new();
        sr.record(true, 50.0);
        sr.record(true, 60.0);
        sr.record(false, 100.0);
        assert!((sr.success_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn synthesis_latency() {
        let mut sr = SynthesisRate::new();
        sr.record(true, 10.0);
        sr.record(true, 20.0);
        let s = sr.latency_summary();
        assert!((s.mean - 15.0).abs() < 0.01);
    }

    // ── SwarmLatency ──────────────────────────────────────────────

    #[test]
    fn swarm_latency_dispatch() {
        let mut sl = SwarmLatency::new();
        sl.record_dispatch(5.0);
        sl.record_dispatch(15.0);
        assert!((sl.dispatch_summary().mean - 10.0).abs() < 0.01);
    }

    #[test]
    fn swarm_latency_completion() {
        let mut sl = SwarmLatency::new();
        sl.record_completion(100.0);
        assert!((sl.completion_summary().mean - 100.0).abs() < 0.01);
    }

    // ── BenchmarkRunner ───────────────────────────────────────────

    #[test]
    fn runner_register_and_run() {
        let mut runner = BenchmarkRunner::new();
        runner.register("throughput", || BenchmarkResult {
            suite_name: "throughput".into(),
            passed: true,
            metrics: vec![],
            issues: vec![],
        });
        runner.register("errors", || BenchmarkResult {
            suite_name: "errors".into(),
            passed: false,
            metrics: vec![],
            issues: vec!["high error rate".into()],
        });
        runner.run_all();
        assert_eq!(runner.results().len(), 2);
        assert_eq!(runner.passed_count(), 1);
        assert_eq!(runner.failed_count(), 1);
    }

    #[test]
    fn runner_run_single() {
        let mut runner = BenchmarkRunner::new();
        runner.register("test", || BenchmarkResult {
            suite_name: "test".into(),
            passed: true,
            metrics: vec![],
            issues: vec![],
        });
        let result = runner.run_suite("test").unwrap();
        assert!(result.passed);
    }

    #[test]
    fn runner_report() {
        let mut runner = BenchmarkRunner::new();
        runner.register("suite1", || BenchmarkResult {
            suite_name: "suite1".into(),
            passed: true,
            metrics: vec![],
            issues: vec![],
        });
        runner.run_all();
        let report = runner.report();
        assert!(report.contains("[PASS] suite1"));
    }

    // ── MetricSample ──────────────────────────────────────────────

    #[test]
    fn metric_sample_tags() {
        let s = MetricSample::new("latency", 42.0, "ms")
            .with_tag("module", "parser")
            .with_timestamp(100);
        assert_eq!(s.tags["module"], "parser");
        assert_eq!(s.timestamp, 100);
    }

    // ── Queue depth ───────────────────────────────────────────────

    #[test]
    fn swarm_queue_depth() {
        let mut sl = SwarmLatency::new();
        sl.record_queue_depth(5);
        sl.record_queue_depth(10);
        assert!((sl.queue_depth.mean() - 7.5).abs() < 0.01);
    }
}
