// ── ACI Subsystem ──────────────────────────────────────────────────
//
// Agentic Compiler Intelligence: four cooperative engines that provide
// context-aware intelligence to agents interacting with the compiler.
//
// Engines:
//   1. DynamicWarningEngine   — severity adjustment, noise filtering
//   2. IntelligentDebugEngine — root cause analysis, fix suggestions
//   3. PerformanceAdvisor     — hotspot detection, optimisation hints
//   4. SwarmCoordIntelligence — agent load balancing, task routing
//
// Plus 8 RAP endpoints surfacing each engine's output.

use std::collections::BTreeMap;

// ── Severity & Warning ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Hint => write!(f, "hint"),
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Warning {
    pub code: String,
    pub message: String,
    pub location: String,
    pub severity: Severity,
    pub suppressed: bool,
}

// ── 1. Dynamic Warning Engine ──────────────────────────────────────

pub struct DynamicWarningEngine {
    warnings: Vec<Warning>,
    suppression_rules: Vec<String>, // code patterns to suppress
    escalation_rules: BTreeMap<String, Severity>, // code → forced severity
    occurrence_count: BTreeMap<String, usize>,
}

impl DynamicWarningEngine {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            suppression_rules: Vec::new(),
            escalation_rules: BTreeMap::new(),
            occurrence_count: BTreeMap::new(),
        }
    }

    pub fn add_suppression(&mut self, code_pattern: &str) {
        self.suppression_rules.push(code_pattern.into());
    }

    pub fn add_escalation(&mut self, code: &str, severity: Severity) {
        self.escalation_rules.insert(code.into(), severity);
    }

    pub fn emit(&mut self, mut warning: Warning) {
        *self.occurrence_count.entry(warning.code.clone()).or_insert(0) += 1;

        // Apply suppression.
        if self.suppression_rules.iter().any(|r| warning.code.contains(r.as_str())) {
            warning.suppressed = true;
        }

        // Apply escalation.
        if let Some(sev) = self.escalation_rules.get(&warning.code) {
            warning.severity = *sev;
        }

        // Frequency-based escalation: if seen ≥5 times, bump to Warning minimum.
        if let Some(&count) = self.occurrence_count.get(&warning.code) {
            if count >= 5 && warning.severity < Severity::Warning {
                warning.severity = Severity::Warning;
            }
        }

        self.warnings.push(warning);
    }

    pub fn active_warnings(&self) -> Vec<&Warning> {
        self.warnings.iter().filter(|w| !w.suppressed).collect()
    }

    pub fn all_warnings(&self) -> &[Warning] {
        &self.warnings
    }

    pub fn warning_count_by_severity(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for w in self.active_warnings() {
            *counts.entry(w.severity.to_string()).or_insert(0) += 1;
        }
        counts
    }
}

// ── 2. Intelligent Debug Engine ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DebugDiagnosis {
    pub error_code: String,
    pub root_cause: String,
    pub fix_suggestion: String,
    pub confidence: f64, // 0.0 – 1.0
    pub related_locations: Vec<String>,
}

pub struct IntelligentDebugEngine {
    /// Pattern database: error code → (root cause, fix suggestion, confidence).
    patterns: BTreeMap<String, (String, String, f64)>,
    history: Vec<DebugDiagnosis>,
}

impl IntelligentDebugEngine {
    pub fn new() -> Self {
        let mut patterns = BTreeMap::new();
        // Built-in patterns.
        patterns.insert("E0505".into(), ("value borrowed while still in use".into(), "shorten the borrow scope or clone the value".into(), 0.85));
        patterns.insert("E0382".into(), ("use of moved value".into(), "clone before move or restructure ownership".into(), 0.90));
        patterns.insert("E0277".into(), ("trait bound not satisfied".into(), "add the required trait implementation or bound".into(), 0.80));
        patterns.insert("E0308".into(), ("type mismatch".into(), "check expected vs actual types, add conversion".into(), 0.85));
        patterns.insert("E0599".into(), ("method not found".into(), "import the trait or check method name spelling".into(), 0.75));
        patterns.insert("contract_violation".into(), ("contract precondition/postcondition failed".into(), "ensure input satisfies @req or output satisfies @ens".into(), 0.90));
        patterns.insert("effect_leak".into(), ("undeclared effect escapes function boundary".into(), "add the effect to the function signature".into(), 0.95));
        patterns.insert("capability_denied".into(), ("agent lacks required capability".into(), "grant the capability or request escalation".into(), 0.88));
        Self { patterns, history: Vec::new() }
    }

    pub fn add_pattern(&mut self, code: &str, root_cause: &str, fix: &str, confidence: f64) {
        self.patterns.insert(code.into(), (root_cause.into(), fix.into(), confidence));
    }

    pub fn diagnose(&mut self, error_code: &str, location: &str) -> DebugDiagnosis {
        let (root_cause, fix, confidence) = self.patterns.get(error_code)
            .cloned()
            .unwrap_or_else(|| ("unknown error".into(), "investigate manually".into(), 0.1));
        let diagnosis = DebugDiagnosis {
            error_code: error_code.into(),
            root_cause,
            fix_suggestion: fix,
            confidence,
            related_locations: vec![location.into()],
        };
        self.history.push(diagnosis.clone());
        diagnosis
    }

    pub fn history(&self) -> &[DebugDiagnosis] {
        &self.history
    }
}

// ── 3. Performance Advisor ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PerfHotspot {
    pub location: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub suggestion: String,
}

impl PerfHotspot {
    pub fn is_violation(&self) -> bool {
        self.value > self.threshold
    }
}

pub struct PerformanceAdvisor {
    thresholds: BTreeMap<String, f64>,     // metric → threshold
    hotspots: Vec<PerfHotspot>,
    suggestions: BTreeMap<String, String>, // metric → generic suggestion
}

impl PerformanceAdvisor {
    pub fn new() -> Self {
        let mut thresholds = BTreeMap::new();
        thresholds.insert("cyclomatic_complexity".into(), 10.0);
        thresholds.insert("token_count".into(), 500.0);
        thresholds.insert("allocation_count".into(), 20.0);
        thresholds.insert("nesting_depth".into(), 5.0);
        thresholds.insert("parameter_count".into(), 7.0);

        let mut suggestions = BTreeMap::new();
        suggestions.insert("cyclomatic_complexity".into(), "extract helper functions to reduce branching".into());
        suggestions.insert("token_count".into(), "split into smaller functions for agent readability".into());
        suggestions.insert("allocation_count".into(), "use arena allocation or stack buffers".into());
        suggestions.insert("nesting_depth".into(), "use early returns or guard clauses".into());
        suggestions.insert("parameter_count".into(), "group parameters into a configuration struct".into());

        Self { thresholds, hotspots: Vec::new(), suggestions }
    }

    pub fn set_threshold(&mut self, metric: &str, value: f64) {
        self.thresholds.insert(metric.into(), value);
    }

    pub fn analyze(&mut self, location: &str, metrics: &BTreeMap<String, f64>) -> Vec<PerfHotspot> {
        let mut new_hotspots = Vec::new();
        for (metric, &value) in metrics {
            if let Some(&threshold) = self.thresholds.get(metric) {
                if value > threshold {
                    let suggestion = self.suggestions.get(metric)
                        .cloned()
                        .unwrap_or_else(|| format!("reduce {metric}"));
                    let hotspot = PerfHotspot {
                        location: location.into(),
                        metric: metric.clone(),
                        value,
                        threshold,
                        suggestion,
                    };
                    new_hotspots.push(hotspot.clone());
                    self.hotspots.push(hotspot);
                }
            }
        }
        new_hotspots
    }

    pub fn all_hotspots(&self) -> &[PerfHotspot] {
        &self.hotspots
    }

    pub fn hotspots_for(&self, location: &str) -> Vec<&PerfHotspot> {
        self.hotspots.iter().filter(|h| h.location == location).collect()
    }
}

// ── 4. Swarm Coordination Intelligence ─────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentLoad {
    pub agent_id: String,
    pub active_tasks: usize,
    pub capacity: usize,
    pub specializations: Vec<String>,
}

impl AgentLoad {
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 { return 1.0; }
        self.active_tasks as f64 / self.capacity as f64
    }

    pub fn available(&self) -> bool {
        self.active_tasks < self.capacity
    }
}

pub struct SwarmCoordIntelligence {
    agents: Vec<AgentLoad>,
}

impl SwarmCoordIntelligence {
    pub fn new() -> Self {
        Self { agents: Vec::new() }
    }

    pub fn register(&mut self, agent: AgentLoad) {
        self.agents.push(agent);
    }

    /// Pick the best agent for a task requiring a given specialization.
    pub fn route_task(&self, specialization: &str) -> Option<&AgentLoad> {
        self.agents.iter()
            .filter(|a| a.available() && a.specializations.iter().any(|s| s == specialization))
            .min_by(|a, b| a.utilization().partial_cmp(&b.utilization()).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Return agents sorted by utilization (least loaded first).
    pub fn load_balance_order(&self) -> Vec<&AgentLoad> {
        let mut sorted: Vec<_> = self.agents.iter().collect();
        sorted.sort_by(|a, b| a.utilization().partial_cmp(&b.utilization()).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    /// Detect overloaded agents (utilization > threshold).
    pub fn overloaded(&self, threshold: f64) -> Vec<&AgentLoad> {
        self.agents.iter().filter(|a| a.utilization() > threshold).collect()
    }

    /// Overall swarm utilization.
    pub fn swarm_utilization(&self) -> f64 {
        if self.agents.is_empty() { return 0.0; }
        let total_cap: usize = self.agents.iter().map(|a| a.capacity).sum();
        let total_active: usize = self.agents.iter().map(|a| a.active_tasks).sum();
        if total_cap == 0 { return 0.0; }
        total_active as f64 / total_cap as f64
    }
}

// ── RAP Endpoints ──────────────────────────────────────────────────

pub struct AciRapEndpoints;

impl AciRapEndpoints {
    pub fn warning_summary(engine: &DynamicWarningEngine) -> String {
        let counts = engine.warning_count_by_severity();
        let entries: Vec<String> = counts.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect();
        format!("{{\"active\":{},{{{}}}}}", engine.active_warnings().len(), entries.join(","))
    }

    pub fn debug_diagnose(engine: &mut IntelligentDebugEngine, code: &str, loc: &str) -> String {
        let d = engine.diagnose(code, loc);
        format!(
            "{{\"code\":\"{}\",\"root_cause\":\"{}\",\"fix\":\"{}\",\"confidence\":{}}}",
            d.error_code, d.root_cause, d.fix_suggestion, d.confidence
        )
    }

    pub fn perf_hotspots(advisor: &PerformanceAdvisor) -> String {
        let entries: Vec<String> = advisor.all_hotspots().iter().map(|h| {
            format!("{{\"location\":\"{}\",\"metric\":\"{}\",\"value\":{:.1},\"threshold\":{:.1}}}",
                h.location, h.metric, h.value, h.threshold)
        }).collect();
        format!("{{\"hotspots\":[{}]}}", entries.join(","))
    }

    pub fn swarm_load(intel: &SwarmCoordIntelligence) -> String {
        let entries: Vec<String> = intel.agents.iter().map(|a| {
            format!("{{\"agent\":\"{}\",\"utilization\":{:.2},\"available\":{}}}",
                a.agent_id, a.utilization(), a.available())
        }).collect();
        format!("{{\"swarm_utilization\":{:.2},\"agents\":[{}]}}",
            intel.swarm_utilization(), entries.join(","))
    }

    pub fn route_task(intel: &SwarmCoordIntelligence, spec: &str) -> String {
        match intel.route_task(spec) {
            Some(a) => format!("{{\"routed_to\":\"{}\",\"utilization\":{:.2}}}", a.agent_id, a.utilization()),
            None => "{\"routed_to\":null}".into(),
        }
    }

    pub fn overloaded(intel: &SwarmCoordIntelligence, threshold: f64) -> String {
        let agents: Vec<String> = intel.overloaded(threshold).iter()
            .map(|a| format!("\"{}\"", a.agent_id))
            .collect();
        format!("{{\"overloaded\":[{}]}}", agents.join(","))
    }

    pub fn debug_history(engine: &IntelligentDebugEngine) -> String {
        let entries: Vec<String> = engine.history().iter().map(|d| {
            format!("{{\"code\":\"{}\",\"confidence\":{}}}", d.error_code, d.confidence)
        }).collect();
        format!("{{\"history\":[{}]}}", entries.join(","))
    }

    pub fn warning_details(engine: &DynamicWarningEngine) -> String {
        let entries: Vec<String> = engine.active_warnings().iter().map(|w| {
            format!("{{\"code\":\"{}\",\"severity\":\"{}\",\"message\":\"{}\"}}",
                w.code, w.severity, w.message)
        }).collect();
        format!("{{\"warnings\":[{}]}}", entries.join(","))
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn warn(code: &str, sev: Severity) -> Warning {
        Warning { code: code.into(), message: format!("msg:{code}"), location: "file.mg:1".into(), severity: sev, suppressed: false }
    }

    // ── Dynamic Warning Engine ────────────────────────────────────

    #[test]
    fn emit_and_count() {
        let mut e = DynamicWarningEngine::new();
        e.emit(warn("W001", Severity::Warning));
        e.emit(warn("W002", Severity::Hint));
        assert_eq!(e.active_warnings().len(), 2);
    }

    #[test]
    fn suppression() {
        let mut e = DynamicWarningEngine::new();
        e.add_suppression("W001");
        e.emit(warn("W001", Severity::Warning));
        e.emit(warn("W002", Severity::Warning));
        assert_eq!(e.active_warnings().len(), 1);
    }

    #[test]
    fn escalation() {
        let mut e = DynamicWarningEngine::new();
        e.add_escalation("W001", Severity::Critical);
        e.emit(warn("W001", Severity::Hint));
        assert_eq!(e.active_warnings()[0].severity, Severity::Critical);
    }

    #[test]
    fn frequency_escalation() {
        let mut e = DynamicWarningEngine::new();
        for _ in 0..5 {
            e.emit(warn("W001", Severity::Hint));
        }
        // The 5th occurrence should be escalated to Warning.
        let last = e.all_warnings().last().unwrap();
        assert!(last.severity >= Severity::Warning);
    }

    #[test]
    fn severity_counts() {
        let mut e = DynamicWarningEngine::new();
        e.emit(warn("W001", Severity::Warning));
        e.emit(warn("W002", Severity::Warning));
        e.emit(warn("E001", Severity::Error));
        let counts = e.warning_count_by_severity();
        assert_eq!(counts.get("warning"), Some(&2));
        assert_eq!(counts.get("error"), Some(&1));
    }

    // ── Intelligent Debug Engine ──────────────────────────────────

    #[test]
    fn diagnose_known_error() {
        let mut e = IntelligentDebugEngine::new();
        let d = e.diagnose("E0505", "src/main.mg:42");
        assert!(d.confidence > 0.5);
        assert!(d.root_cause.contains("borrow"));
    }

    #[test]
    fn diagnose_unknown_error() {
        let mut e = IntelligentDebugEngine::new();
        let d = e.diagnose("E9999", "unknown");
        assert!(d.confidence < 0.5);
    }

    #[test]
    fn custom_pattern() {
        let mut e = IntelligentDebugEngine::new();
        e.add_pattern("CUSTOM01", "custom root", "custom fix", 0.99);
        let d = e.diagnose("CUSTOM01", "loc");
        assert_eq!(d.confidence, 0.99);
    }

    #[test]
    fn debug_history_recorded() {
        let mut e = IntelligentDebugEngine::new();
        e.diagnose("E0505", "a");
        e.diagnose("E0382", "b");
        assert_eq!(e.history().len(), 2);
    }

    // ── Performance Advisor ───────────────────────────────────────

    #[test]
    fn detect_hotspot() {
        let mut a = PerformanceAdvisor::new();
        let mut metrics = BTreeMap::new();
        metrics.insert("cyclomatic_complexity".into(), 15.0);
        let hotspots = a.analyze("fn foo", &metrics);
        assert_eq!(hotspots.len(), 1);
        assert!(hotspots[0].is_violation());
    }

    #[test]
    fn no_hotspot_below_threshold() {
        let mut a = PerformanceAdvisor::new();
        let mut metrics = BTreeMap::new();
        metrics.insert("cyclomatic_complexity".into(), 5.0);
        let hotspots = a.analyze("fn bar", &metrics);
        assert!(hotspots.is_empty());
    }

    #[test]
    fn custom_threshold() {
        let mut a = PerformanceAdvisor::new();
        a.set_threshold("cyclomatic_complexity", 3.0);
        let mut metrics = BTreeMap::new();
        metrics.insert("cyclomatic_complexity".into(), 4.0);
        assert_eq!(a.analyze("fn x", &metrics).len(), 1);
    }

    #[test]
    fn hotspots_for_location() {
        let mut a = PerformanceAdvisor::new();
        let mut m1 = BTreeMap::new();
        m1.insert("cyclomatic_complexity".into(), 15.0);
        let mut m2 = BTreeMap::new();
        m2.insert("token_count".into(), 600.0);
        a.analyze("fn a", &m1);
        a.analyze("fn b", &m2);
        assert_eq!(a.hotspots_for("fn a").len(), 1);
        assert_eq!(a.hotspots_for("fn b").len(), 1);
    }

    // ── Swarm Coordination Intelligence ───────────────────────────

    #[test]
    fn route_to_least_loaded() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 3, capacity: 4, specializations: vec!["parse".into()] });
        sci.register(AgentLoad { agent_id: "a2".into(), active_tasks: 1, capacity: 4, specializations: vec!["parse".into()] });
        let routed = sci.route_task("parse").unwrap();
        assert_eq!(routed.agent_id, "a2");
    }

    #[test]
    fn route_no_match() {
        let sci = SwarmCoordIntelligence::new();
        assert!(sci.route_task("parse").is_none());
    }

    #[test]
    fn overloaded_detection() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 4, capacity: 4, specializations: vec![] });
        sci.register(AgentLoad { agent_id: "a2".into(), active_tasks: 1, capacity: 4, specializations: vec![] });
        assert_eq!(sci.overloaded(0.9).len(), 1);
    }

    #[test]
    fn swarm_utilization() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 2, capacity: 4, specializations: vec![] });
        sci.register(AgentLoad { agent_id: "a2".into(), active_tasks: 2, capacity: 4, specializations: vec![] });
        assert!((sci.swarm_utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn agent_utilization() {
        let a = AgentLoad { agent_id: "a".into(), active_tasks: 3, capacity: 6, specializations: vec![] };
        assert!((a.utilization() - 0.5).abs() < 0.01);
        assert!(a.available());
    }

    // ── RAP Endpoints ─────────────────────────────────────────────

    #[test]
    fn rap_warning_summary() {
        let mut e = DynamicWarningEngine::new();
        e.emit(warn("W001", Severity::Warning));
        let json = AciRapEndpoints::warning_summary(&e);
        assert!(json.contains("\"active\":1"));
    }

    #[test]
    fn rap_debug_diagnose() {
        let mut e = IntelligentDebugEngine::new();
        let json = AciRapEndpoints::debug_diagnose(&mut e, "E0505", "loc");
        assert!(json.contains("\"code\":\"E0505\""));
    }

    #[test]
    fn rap_perf_hotspots() {
        let mut a = PerformanceAdvisor::new();
        let mut m = BTreeMap::new();
        m.insert("nesting_depth".into(), 8.0);
        a.analyze("fn deep", &m);
        let json = AciRapEndpoints::perf_hotspots(&a);
        assert!(json.contains("nesting_depth"));
    }

    #[test]
    fn rap_swarm_load() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 1, capacity: 4, specializations: vec![] });
        let json = AciRapEndpoints::swarm_load(&sci);
        assert!(json.contains("swarm_utilization"));
    }

    #[test]
    fn rap_route_task() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 0, capacity: 4, specializations: vec!["lint".into()] });
        let json = AciRapEndpoints::route_task(&sci, "lint");
        assert!(json.contains("\"routed_to\":\"a1\""));
    }

    #[test]
    fn rap_route_task_none() {
        let sci = SwarmCoordIntelligence::new();
        let json = AciRapEndpoints::route_task(&sci, "lint");
        assert!(json.contains("null"));
    }

    #[test]
    fn rap_overloaded() {
        let mut sci = SwarmCoordIntelligence::new();
        sci.register(AgentLoad { agent_id: "a1".into(), active_tasks: 4, capacity: 4, specializations: vec![] });
        let json = AciRapEndpoints::overloaded(&sci, 0.9);
        assert!(json.contains("\"a1\""));
    }

    #[test]
    fn rap_debug_history() {
        let mut e = IntelligentDebugEngine::new();
        e.diagnose("E0505", "a");
        let json = AciRapEndpoints::debug_history(&e);
        assert!(json.contains("E0505"));
    }
}
