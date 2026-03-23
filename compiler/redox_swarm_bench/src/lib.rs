// redox_swarm_bench: Swarm performance benchmarking suite.
//
//  Defines metrics (throughput, latency, conflict rate), benchmark
//  scenarios, a runner that collects results, and summary reporting.

// ---------------------------------------------------------------------------
// Metric kinds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    Throughput,      // tasks / second
    Latency,         // milliseconds per task
    ConflictRate,    // conflicts / total attempts (0.0..1.0)
    AgentUtilization, // fraction of time agents are busy
    QueueDepth,      // average pending tasks
    ErrorRate,       // errors / total tasks
}

impl MetricKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Throughput => "throughput",
            Self::Latency => "latency",
            Self::ConflictRate => "conflict-rate",
            Self::AgentUtilization => "agent-utilization",
            Self::QueueDepth => "queue-depth",
            Self::ErrorRate => "error-rate",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Throughput => "tasks/s",
            Self::Latency => "ms",
            Self::ConflictRate => "ratio",
            Self::AgentUtilization => "ratio",
            Self::QueueDepth => "tasks",
            Self::ErrorRate => "ratio",
        }
    }
}

// ---------------------------------------------------------------------------
// Single metric sample
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSample {
    pub kind: MetricKind,
    pub value: f64,
}

// ---------------------------------------------------------------------------
// Benchmark scenario
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScenarioId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct BenchScenario {
    pub id: ScenarioId,
    pub name: String,
    pub description: String,
    pub agent_count: usize,
    pub task_count: usize,
    pub iterations: usize,
}

impl BenchScenario {
    pub fn new(id: &str, name: &str, desc: &str, agents: usize, tasks: usize, iters: usize) -> Self {
        Self {
            id: ScenarioId(id.to_string()),
            name: name.to_string(),
            description: desc.to_string(),
            agent_count: agents,
            task_count: tasks,
            iterations: iters,
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmark result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BenchResult {
    pub scenario_id: ScenarioId,
    pub samples: Vec<MetricSample>,
}

impl BenchResult {
    pub fn get(&self, kind: MetricKind) -> Option<f64> {
        self.samples.iter().find(|s| s.kind == kind).map(|s| s.value)
    }

    pub fn throughput(&self) -> Option<f64> { self.get(MetricKind::Throughput) }
    pub fn latency(&self) -> Option<f64> { self.get(MetricKind::Latency) }
    pub fn conflict_rate(&self) -> Option<f64> { self.get(MetricKind::ConflictRate) }
}

// ---------------------------------------------------------------------------
// Simulated runner
// ---------------------------------------------------------------------------

/// Simulate running a benchmark scenario and produce metrics.
pub fn run_scenario(scenario: &BenchScenario) -> BenchResult {
    // Simulated: throughput scales with agents, latency inversely
    let base_throughput = 100.0;
    let throughput = base_throughput * scenario.agent_count as f64
        / (1.0 + (scenario.task_count as f64 / 1000.0));
    let latency = 1000.0 / throughput.max(1.0);
    let conflict_rate = if scenario.agent_count > 1 {
        0.01 * (scenario.agent_count as f64 - 1.0)
    } else {
        0.0
    };
    let utilization = (throughput * latency / 1000.0).min(1.0);
    let queue_depth = (scenario.task_count as f64) / scenario.agent_count.max(1) as f64;
    let error_rate = 0.001;

    BenchResult {
        scenario_id: scenario.id.clone(),
        samples: vec![
            MetricSample { kind: MetricKind::Throughput, value: throughput },
            MetricSample { kind: MetricKind::Latency, value: latency },
            MetricSample { kind: MetricKind::ConflictRate, value: conflict_rate },
            MetricSample { kind: MetricKind::AgentUtilization, value: utilization },
            MetricSample { kind: MetricKind::QueueDepth, value: queue_depth },
            MetricSample { kind: MetricKind::ErrorRate, value: error_rate },
        ],
    }
}

// ---------------------------------------------------------------------------
// Benchmark suite
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BenchSuite {
    pub name: String,
    pub scenarios: Vec<BenchScenario>,
}

impl BenchSuite {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), scenarios: Vec::new() }
    }

    pub fn add(&mut self, scenario: BenchScenario) {
        self.scenarios.push(scenario);
    }

    pub fn run_all(&self) -> Vec<BenchResult> {
        self.scenarios.iter().map(|s| run_scenario(s)).collect()
    }

    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Statistical helpers
// ---------------------------------------------------------------------------

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn min_val(values: &[f64]) -> f64 {
    values.iter().cloned().fold(f64::INFINITY, f64::min)
}

pub fn max_val(values: &[f64]) -> f64 {
    values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
}

pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

// ---------------------------------------------------------------------------
// Summary report
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSummary {
    pub kind: MetricKind,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
}

pub fn summarize_metric(results: &[BenchResult], kind: MetricKind) -> MetricSummary {
    let values: Vec<f64> = results.iter().filter_map(|r| r.get(kind)).collect();
    MetricSummary {
        kind,
        mean: mean(&values),
        min: min_val(&values),
        max: max_val(&values),
        std_dev: std_dev(&values),
    }
}

pub fn full_summary(results: &[BenchResult]) -> Vec<MetricSummary> {
    vec![
        summarize_metric(results, MetricKind::Throughput),
        summarize_metric(results, MetricKind::Latency),
        summarize_metric(results, MetricKind::ConflictRate),
        summarize_metric(results, MetricKind::AgentUtilization),
        summarize_metric(results, MetricKind::QueueDepth),
        summarize_metric(results, MetricKind::ErrorRate),
    ]
}

pub fn format_summary(summaries: &[MetricSummary]) -> String {
    let mut out = String::new();
    out.push_str("=== Swarm Benchmark Summary ===\n");
    for s in summaries {
        out.push_str(&format!(
            "  {}: mean={:.3} min={:.3} max={:.3} std={:.3} ({})\n",
            s.kind.label(), s.mean, s.min, s.max, s.std_dev, s.kind.unit(),
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Pre-built scenarios
// ---------------------------------------------------------------------------

pub fn standard_scenarios() -> Vec<BenchScenario> {
    vec![
        BenchScenario::new("single-agent", "Single agent baseline", "1 agent, 100 tasks", 1, 100, 10),
        BenchScenario::new("small-swarm", "Small swarm", "4 agents, 500 tasks", 4, 500, 10),
        BenchScenario::new("medium-swarm", "Medium swarm", "16 agents, 2000 tasks", 16, 2000, 10),
        BenchScenario::new("large-swarm", "Large swarm", "64 agents, 10000 tasks", 64, 10000, 10),
        BenchScenario::new("high-contention", "High contention", "32 agents, 100 tasks", 32, 100, 10),
    ]
}

pub fn build_standard_suite() -> BenchSuite {
    let mut suite = BenchSuite::new("Redox Swarm Standard Benchmarks");
    for s in standard_scenarios() {
        suite.add(s);
    }
    suite
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub metric: MetricKind,
    pub baseline: f64,
    pub candidate: f64,
    pub delta_pct: f64,
}

pub fn compare_results(
    baseline: &BenchResult,
    candidate: &BenchResult,
    kind: MetricKind,
) -> Option<Comparison> {
    let b = baseline.get(kind)?;
    let c = candidate.get(kind)?;
    let delta = if b.abs() > f64::EPSILON { ((c - b) / b) * 100.0 } else { 0.0 };
    Some(Comparison { metric: kind, baseline: b, candidate: c, delta_pct: delta })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- MetricKind --
    #[test]
    fn test_metric_labels() {
        assert_eq!(MetricKind::Throughput.label(), "throughput");
        assert_eq!(MetricKind::ConflictRate.label(), "conflict-rate");
    }

    #[test]
    fn test_metric_units() {
        assert_eq!(MetricKind::Latency.unit(), "ms");
        assert_eq!(MetricKind::QueueDepth.unit(), "tasks");
    }

    // -- BenchScenario --
    #[test]
    fn test_scenario_new() {
        let s = BenchScenario::new("s1", "Test", "desc", 4, 100, 5);
        assert_eq!(s.agent_count, 4);
        assert_eq!(s.task_count, 100);
    }

    // -- run_scenario --
    #[test]
    fn test_run_scenario_produces_all_metrics() {
        let s = BenchScenario::new("x", "x", "x", 4, 100, 1);
        let r = run_scenario(&s);
        assert_eq!(r.samples.len(), 6);
    }

    #[test]
    fn test_run_single_agent_no_conflicts() {
        let s = BenchScenario::new("x", "x", "x", 1, 100, 1);
        let r = run_scenario(&s);
        assert_eq!(r.conflict_rate(), Some(0.0));
    }

    #[test]
    fn test_throughput_scales_with_agents() {
        let s1 = BenchScenario::new("a", "a", "a", 1, 100, 1);
        let s4 = BenchScenario::new("b", "b", "b", 4, 100, 1);
        let r1 = run_scenario(&s1);
        let r4 = run_scenario(&s4);
        assert!(r4.throughput().unwrap() > r1.throughput().unwrap());
    }

    #[test]
    fn test_latency_decreases_with_agents() {
        let s1 = BenchScenario::new("a", "a", "a", 1, 100, 1);
        let s4 = BenchScenario::new("b", "b", "b", 4, 100, 1);
        let r1 = run_scenario(&s1);
        let r4 = run_scenario(&s4);
        assert!(r4.latency().unwrap() < r1.latency().unwrap());
    }

    // -- BenchResult accessors --
    #[test]
    fn test_result_get_existing() {
        let s = BenchScenario::new("x", "x", "x", 2, 100, 1);
        let r = run_scenario(&s);
        assert!(r.throughput().is_some());
        assert!(r.latency().is_some());
        assert!(r.conflict_rate().is_some());
    }

    // -- BenchSuite --
    #[test]
    fn test_suite_add_run() {
        let mut suite = BenchSuite::new("test");
        suite.add(BenchScenario::new("a", "a", "a", 2, 50, 1));
        suite.add(BenchScenario::new("b", "b", "b", 4, 100, 1));
        assert_eq!(suite.len(), 2);
        let results = suite.run_all();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_suite_empty() {
        let suite = BenchSuite::new("empty");
        assert!(suite.is_empty());
    }

    // -- statistical helpers --
    #[test]
    fn test_mean() {
        assert!((mean(&[1.0, 2.0, 3.0]) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_min_max() {
        assert_eq!(min_val(&[3.0, 1.0, 2.0]), 1.0);
        assert_eq!(max_val(&[3.0, 1.0, 2.0]), 3.0);
    }

    #[test]
    fn test_std_dev_single() {
        assert_eq!(std_dev(&[5.0]), 0.0);
    }

    #[test]
    fn test_std_dev_values() {
        let sd = std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert!((sd - 2.138).abs() < 0.01);
    }

    // -- summarize --
    #[test]
    fn test_summarize_metric() {
        let suite = build_standard_suite();
        let results = suite.run_all();
        let s = summarize_metric(&results, MetricKind::Throughput);
        assert!(s.mean > 0.0);
        assert!(s.min <= s.max);
    }

    #[test]
    fn test_full_summary_count() {
        let suite = build_standard_suite();
        let results = suite.run_all();
        let summaries = full_summary(&results);
        assert_eq!(summaries.len(), 6);
    }

    // -- format_summary --
    #[test]
    fn test_format_summary() {
        let suite = build_standard_suite();
        let results = suite.run_all();
        let summaries = full_summary(&results);
        let text = format_summary(&summaries);
        assert!(text.contains("throughput"));
        assert!(text.contains("latency"));
    }

    // -- standard_scenarios --
    #[test]
    fn test_standard_scenarios_count() {
        assert_eq!(standard_scenarios().len(), 5);
    }

    // -- build_standard_suite --
    #[test]
    fn test_standard_suite() {
        let suite = build_standard_suite();
        assert_eq!(suite.len(), 5);
        assert_eq!(suite.name, "Redox Swarm Standard Benchmarks");
    }

    // -- compare_results --
    #[test]
    fn test_compare_results() {
        let s1 = BenchScenario::new("a", "a", "a", 2, 100, 1);
        let s2 = BenchScenario::new("b", "b", "b", 8, 100, 1);
        let r1 = run_scenario(&s1);
        let r2 = run_scenario(&s2);
        let cmp = compare_results(&r1, &r2, MetricKind::Throughput).unwrap();
        assert!(cmp.delta_pct > 0.0); // more agents = higher throughput
        assert!(cmp.candidate > cmp.baseline);
    }

    #[test]
    fn test_compare_missing_metric() {
        let r = BenchResult { scenario_id: ScenarioId("x".into()), samples: vec![] };
        assert!(compare_results(&r, &r, MetricKind::Throughput).is_none());
    }

    // -- ScenarioId --
    #[test]
    fn test_scenario_id_hash() {
        let mut set = std::collections::HashSet::new();
        set.insert(ScenarioId("a".into()));
        set.insert(ScenarioId("b".into()));
        assert_eq!(set.len(), 2);
    }

    // -- error rate --
    #[test]
    fn test_error_rate_low() {
        let s = BenchScenario::new("x", "x", "x", 4, 100, 1);
        let r = run_scenario(&s);
        let er = r.get(MetricKind::ErrorRate).unwrap();
        assert!(er < 0.01);
    }
}
