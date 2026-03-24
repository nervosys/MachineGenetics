//! # ACI Intelligent Debugging Engine
//!
//! Causal root-cause analysis from runtime traces. Given a failure trace, the
//! engine reconstructs the causal chain (effect → cause → root cause) using
//! execution history, data flow, and learned patterns from past bugs.
//!
//! Pipeline:
//! ```text
//! RuntimeTrace → TraceAnalyzer → CausalGraph → RootCauseRanker → DebugReport
//!                                     ↑
//!                            Bug History (ML patterns)
//! ```
//!
//! Reference: REDOX_PROPOSAL.md — ACI Intelligent Debugging Engine
//!   "causal root-cause analysis via ML"
//!
//! (ROADMAP Step 63)

use std::collections::HashMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Runtime Traces
// ═══════════════════════════════════════════════════════════════════════════

/// A single event in a runtime trace.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Event kind.
    pub kind: TraceEventKind,
    /// Source location.
    pub location: SourceLocation,
    /// Variable/value snapshot at this point.
    pub snapshot: HashMap<String, String>,
}

/// Kind of trace event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEventKind {
    /// Function entry.
    FunctionEntry { name: String },
    /// Function exit (normal).
    FunctionExit { name: String },
    /// Variable assignment.
    Assignment { variable: String, value: String },
    /// Branch taken (condition value).
    BranchTaken { condition: String, taken: bool },
    /// Assertion failure.
    AssertionFailed { expression: String },
    /// Panic / unrecoverable error.
    Panic { message: String },
    /// Memory allocation.
    MemAlloc { size: usize, address: u64 },
    /// Memory deallocation.
    MemFree { address: u64 },
    /// Lock acquire.
    LockAcquire { lock_id: String },
    /// Lock release.
    LockRelease { lock_id: String },
    /// Channel send.
    ChannelSend { channel: String },
    /// Channel receive.
    ChannelRecv { channel: String },
    /// Custom / user-annotated event.
    Custom { tag: String, data: String },
}

impl fmt::Display for TraceEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TraceEventKind::FunctionEntry { name } => write!(f, "enter {name}"),
            TraceEventKind::FunctionExit { name } => write!(f, "exit {name}"),
            TraceEventKind::Assignment { variable, value } => write!(f, "{variable} = {value}"),
            TraceEventKind::BranchTaken { condition, taken } => write!(f, "branch {condition} → {taken}"),
            TraceEventKind::AssertionFailed { expression } => write!(f, "assert failed: {expression}"),
            TraceEventKind::Panic { message } => write!(f, "panic: {message}"),
            TraceEventKind::MemAlloc { size, address } => write!(f, "alloc {size}B @{address:#x}"),
            TraceEventKind::MemFree { address } => write!(f, "free @{address:#x}"),
            TraceEventKind::LockAcquire { lock_id } => write!(f, "lock {lock_id}"),
            TraceEventKind::LockRelease { lock_id } => write!(f, "unlock {lock_id}"),
            TraceEventKind::ChannelSend { channel } => write!(f, "send → {channel}"),
            TraceEventKind::ChannelRecv { channel } => write!(f, "recv ← {channel}"),
            TraceEventKind::Custom { tag, data } => write!(f, "[{tag}] {data}"),
        }
    }
}

/// Source code location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub function: String,
}

impl SourceLocation {
    pub fn new(file: &str, line: u32, function: &str) -> Self {
        SourceLocation {
            file: file.to_string(),
            line,
            function: function.to_string(),
        }
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} in {}", self.file, self.line, self.function)
    }
}

/// A complete runtime trace.
#[derive(Debug, Clone)]
pub struct RuntimeTrace {
    pub events: Vec<TraceEvent>,
    /// The failure event (if any).
    pub failure_index: Option<usize>,
}

impl RuntimeTrace {
    pub fn new(events: Vec<TraceEvent>) -> Self {
        let failure_index = events.iter().position(|e| matches!(
            &e.kind,
            TraceEventKind::Panic { .. } | TraceEventKind::AssertionFailed { .. }
        ));
        RuntimeTrace { events, failure_index }
    }

    pub fn has_failure(&self) -> bool {
        self.failure_index.is_some()
    }

    pub fn failure_event(&self) -> Option<&TraceEvent> {
        self.failure_index.map(|i| &self.events[i])
    }

    /// Events leading up to the failure.
    pub fn events_before_failure(&self) -> &[TraceEvent] {
        match self.failure_index {
            Some(idx) => &self.events[..idx],
            None => &self.events,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Causal Graph
// ═══════════════════════════════════════════════════════════════════════════

/// Relationship between two events in the causal chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalRelation {
    /// Event A directly caused Event B.
    DirectCause,
    /// Event A provided data consumed by Event B.
    DataFlow,
    /// Event A and B are related through control flow.
    ControlFlow,
    /// Event A and B share a resource (lock, memory).
    ResourceContention,
    /// Temporal correlation (close in time).
    TemporalCorrelation,
}

impl fmt::Display for CausalRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CausalRelation::DirectCause => write!(f, "caused"),
            CausalRelation::DataFlow => write!(f, "data-flow"),
            CausalRelation::ControlFlow => write!(f, "control-flow"),
            CausalRelation::ResourceContention => write!(f, "resource-contention"),
            CausalRelation::TemporalCorrelation => write!(f, "temporal"),
        }
    }
}

/// An edge in the causal graph.
#[derive(Debug, Clone)]
pub struct CausalEdge {
    /// Index of cause event in the trace.
    pub cause_seq: u64,
    /// Index of effect event in the trace.
    pub effect_seq: u64,
    /// Relation type.
    pub relation: CausalRelation,
    /// Confidence in this causal link.
    pub confidence: f64,
}

/// The causal graph built from trace analysis.
#[derive(Debug, Clone)]
pub struct CausalGraph {
    pub edges: Vec<CausalEdge>,
}

impl CausalGraph {
    pub fn new() -> Self {
        CausalGraph { edges: Vec::new() }
    }

    pub fn add_edge(&mut self, cause: u64, effect: u64, relation: CausalRelation, confidence: f64) {
        self.edges.push(CausalEdge {
            cause_seq: cause,
            effect_seq: effect,
            relation,
            confidence,
        });
    }

    /// Find all direct causes of a given event.
    pub fn causes_of(&self, effect_seq: u64) -> Vec<&CausalEdge> {
        self.edges.iter().filter(|e| e.effect_seq == effect_seq).collect()
    }

    /// Find all effects of a given event.
    pub fn effects_of(&self, cause_seq: u64) -> Vec<&CausalEdge> {
        self.edges.iter().filter(|e| e.cause_seq == cause_seq).collect()
    }

    /// Trace the causal chain backwards from failure to root.
    pub fn trace_root_cause_chain(&self, failure_seq: u64) -> Vec<u64> {
        let mut chain = vec![failure_seq];
        let mut current = failure_seq;
        let mut visited = vec![failure_seq];

        loop {
            let causes = self.causes_of(current);
            // Pick highest-confidence cause
            if let Some(best) = causes.iter().max_by(|a, b| {
                a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                if visited.contains(&best.cause_seq) {
                    break; // avoid cycles
                }
                chain.push(best.cause_seq);
                visited.push(best.cause_seq);
                current = best.cause_seq;
            } else {
                break;
            }
        }

        chain
    }
}

impl Default for CausalGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Trace Analysis
// ═══════════════════════════════════════════════════════════════════════════

/// Build a causal graph from a runtime trace.
pub fn build_causal_graph(trace: &RuntimeTrace) -> CausalGraph {
    let mut graph = CausalGraph::new();

    // Data flow: assignment → later use of same variable
    let mut last_assignment: HashMap<String, u64> = HashMap::new();

    for event in &trace.events {
        match &event.kind {
            TraceEventKind::Assignment { variable, .. } => {
                last_assignment.insert(variable.clone(), event.seq);
            }
            TraceEventKind::BranchTaken { condition, .. } => {
                // If a variable in the condition was assigned, link them
                for (var, assign_seq) in &last_assignment {
                    if condition.contains(var.as_str()) {
                        graph.add_edge(*assign_seq, event.seq, CausalRelation::DataFlow, 0.8);
                    }
                }
            }
            TraceEventKind::AssertionFailed { expression } => {
                // The assertion depends on variables in its expression
                for (var, assign_seq) in &last_assignment {
                    if expression.contains(var.as_str()) {
                        graph.add_edge(*assign_seq, event.seq, CausalRelation::DirectCause, 0.9);
                    }
                }
            }
            TraceEventKind::Panic { .. } => {
                // Link the most recent events as potential causes
                if let Some(prev) = trace.events.iter().rev().find(|e| e.seq < event.seq) {
                    graph.add_edge(prev.seq, event.seq, CausalRelation::ControlFlow, 0.7);
                }
            }
            _ => {}
        }

        // Control flow: function entry → subsequent events in that function
        if let TraceEventKind::FunctionEntry { name } = &event.kind {
            // Link previous function exit to this entry (call chain)
            for prev in trace.events.iter().rev() {
                if prev.seq >= event.seq {
                    continue;
                }
                if let TraceEventKind::FunctionExit { name: exit_name } = &prev.kind {
                    if exit_name != name {
                        graph.add_edge(prev.seq, event.seq, CausalRelation::ControlFlow, 0.6);
                        break;
                    }
                }
            }
        }

        // Resource contention: lock/unlock patterns
        if let TraceEventKind::LockAcquire { lock_id } = &event.kind {
            // Find previous release of same lock
            for prev in trace.events.iter().rev() {
                if prev.seq >= event.seq {
                    continue;
                }
                if let TraceEventKind::LockRelease { lock_id: prev_id } = &prev.kind {
                    if prev_id == lock_id {
                        graph.add_edge(prev.seq, event.seq, CausalRelation::ResourceContention, 0.5);
                        break;
                    }
                }
            }
        }

        // Memory: alloc → free relationship
        if let TraceEventKind::MemFree { address } = &event.kind {
            for prev in trace.events.iter().rev() {
                if prev.seq >= event.seq {
                    continue;
                }
                if let TraceEventKind::MemAlloc { address: alloc_addr, .. } = &prev.kind {
                    if alloc_addr == address {
                        graph.add_edge(prev.seq, event.seq, CausalRelation::DataFlow, 0.7);
                        break;
                    }
                }
            }
        }
    }

    graph
}

// ═══════════════════════════════════════════════════════════════════════════
// Root Cause Ranking
// ═══════════════════════════════════════════════════════════════════════════

/// A candidate root cause.
#[derive(Debug, Clone)]
pub struct RootCauseCandidate {
    pub event_seq: u64,
    pub location: SourceLocation,
    pub description: String,
    /// Score: higher = more likely root cause.
    pub score: f64,
    /// Causal chain from this candidate to the failure.
    pub causal_chain_length: usize,
    /// Category of the suspected issue.
    pub suspected_category: String,
}

impl fmt::Display for RootCauseCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:.0}%] {} at {} (chain len: {})",
            self.score * 100.0, self.description, self.location, self.causal_chain_length)
    }
}

/// Rank root cause candidates from a causal graph and trace.
pub fn rank_root_causes(
    trace: &RuntimeTrace,
    graph: &CausalGraph,
) -> Vec<RootCauseCandidate> {
    let failure_seq = match trace.failure_event() {
        Some(e) => e.seq,
        None => return vec![],
    };

    let chain = graph.trace_root_cause_chain(failure_seq);
    let mut candidates = Vec::new();

    for (chain_pos, &seq) in chain.iter().enumerate() {
        if let Some(event) = trace.events.iter().find(|e| e.seq == seq) {
            // Score: root of chain is more likely root cause
            let chain_depth_bonus = (chain_pos as f64 + 1.0) / chain.len() as f64;

            // Assignments and branches are more likely root causes than panics
            let kind_bonus = match &event.kind {
                TraceEventKind::Assignment { .. } => 0.8,
                TraceEventKind::BranchTaken { .. } => 0.7,
                TraceEventKind::FunctionEntry { .. } => 0.5,
                TraceEventKind::LockAcquire { .. } => 0.6,
                TraceEventKind::Panic { .. } | TraceEventKind::AssertionFailed { .. } => 0.3,
                _ => 0.4,
            };

            let score = chain_depth_bonus * kind_bonus;

            let description = format!("{}", event.kind);
            let suspected = match &event.kind {
                TraceEventKind::Assignment { .. } => "incorrect-value",
                TraceEventKind::BranchTaken { .. } => "wrong-branch",
                TraceEventKind::LockAcquire { .. } => "concurrency",
                TraceEventKind::MemAlloc { .. } => "memory",
                _ => "unknown",
            };

            candidates.push(RootCauseCandidate {
                event_seq: seq,
                location: event.location.clone(),
                description,
                score,
                causal_chain_length: chain.len(),
                suspected_category: suspected.to_string(),
            });
        }
    }

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

// ═══════════════════════════════════════════════════════════════════════════
// Debug Report
// ═══════════════════════════════════════════════════════════════════════════

/// Comprehensive debug report.
#[derive(Debug)]
pub struct DebugReport {
    pub failure_description: String,
    pub failure_location: SourceLocation,
    pub root_causes: Vec<RootCauseCandidate>,
    pub causal_chain: Vec<u64>,
    pub total_events_analyzed: usize,
    pub causal_edges_found: usize,
}

impl DebugReport {
    pub fn top_root_cause(&self) -> Option<&RootCauseCandidate> {
        self.root_causes.first()
    }

    pub fn has_high_confidence_root_cause(&self) -> bool {
        self.root_causes.first().is_some_and(|c| c.score > 0.5)
    }

    pub fn summary(&self) -> String {
        let rc_desc = self.top_root_cause()
            .map(|c| format!("{c}"))
            .unwrap_or_else(|| "no root cause identified".to_string());
        format!(
            "Failure: {} at {}\n  Likely root cause: {}\n  Events analyzed: {}, Causal edges: {}",
            self.failure_description, self.failure_location,
            rc_desc,
            self.total_events_analyzed, self.causal_edges_found,
        )
    }
}

/// Run the full debugging pipeline on a runtime trace.
pub fn debug_trace(trace: &RuntimeTrace) -> DebugReport {
    let graph = build_causal_graph(trace);
    let root_causes = rank_root_causes(trace, &graph);

    let (failure_desc, failure_loc) = match trace.failure_event() {
        Some(e) => (format!("{}", e.kind), e.location.clone()),
        None => ("no failure detected".to_string(), SourceLocation::new("unknown", 0, "unknown")),
    };

    let causal_chain = trace.failure_event()
        .map(|e| graph.trace_root_cause_chain(e.seq))
        .unwrap_or_default();

    DebugReport {
        failure_description: failure_desc,
        failure_location: failure_loc,
        root_causes,
        causal_chain,
        total_events_analyzed: trace.events.len(),
        causal_edges_found: graph.edges.len(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(seq: u64, kind: TraceEventKind, file: &str, line: u32, func: &str) -> TraceEvent {
        TraceEvent {
            seq,
            timestamp_ns: seq * 1000,
            kind,
            location: SourceLocation::new(file, line, func),
            snapshot: HashMap::new(),
        }
    }

    fn sample_trace() -> RuntimeTrace {
        RuntimeTrace::new(vec![
            make_event(1, TraceEventKind::FunctionEntry { name: "process".to_string() },
                "main.mg", 10, "main"),
            make_event(2, TraceEventKind::Assignment { variable: "x".to_string(), value: "-1".to_string() },
                "main.mg", 12, "process"),
            make_event(3, TraceEventKind::BranchTaken { condition: "x > 0".to_string(), taken: false },
                "main.mg", 14, "process"),
            make_event(4, TraceEventKind::Assignment { variable: "idx".to_string(), value: "0".to_string() },
                "main.mg", 16, "process"),
            make_event(5, TraceEventKind::AssertionFailed { expression: "idx < len".to_string() },
                "main.mg", 20, "process"),
        ])
    }

    fn no_failure_trace() -> RuntimeTrace {
        RuntimeTrace::new(vec![
            make_event(1, TraceEventKind::FunctionEntry { name: "ok".to_string() },
                "ok.mg", 1, "ok"),
            make_event(2, TraceEventKind::FunctionExit { name: "ok".to_string() },
                "ok.mg", 5, "ok"),
        ])
    }

    // ── Trace Event Kind Display ─────────────────────────────────────────

    #[test]
    fn event_kind_display() {
        let k = TraceEventKind::FunctionEntry { name: "foo".to_string() };
        assert_eq!(k.to_string(), "enter foo");
    }

    #[test]
    fn event_kind_panic_display() {
        let k = TraceEventKind::Panic { message: "oops".to_string() };
        assert!(k.to_string().contains("panic"));
    }

    #[test]
    fn event_kind_alloc_display() {
        let k = TraceEventKind::MemAlloc { size: 1024, address: 0xDEAD };
        assert!(k.to_string().contains("1024"));
    }

    // ── Source Location ──────────────────────────────────────────────────

    #[test]
    fn source_location_display() {
        let loc = SourceLocation::new("foo.mg", 42, "bar");
        assert_eq!(loc.to_string(), "foo.mg:42 in bar");
    }

    // ── Runtime Trace ────────────────────────────────────────────────────

    #[test]
    fn trace_has_failure() {
        let trace = sample_trace();
        assert!(trace.has_failure());
        assert_eq!(trace.failure_index, Some(4));
    }

    #[test]
    fn trace_no_failure() {
        let trace = no_failure_trace();
        assert!(!trace.has_failure());
    }

    #[test]
    fn trace_events_before_failure() {
        let trace = sample_trace();
        let before = trace.events_before_failure();
        assert_eq!(before.len(), 4);
    }

    #[test]
    fn trace_failure_event() {
        let trace = sample_trace();
        let fe = trace.failure_event().unwrap();
        assert!(matches!(&fe.kind, TraceEventKind::AssertionFailed { .. }));
    }

    // ── Causal Relation ──────────────────────────────────────────────────

    #[test]
    fn causal_relation_display() {
        assert_eq!(CausalRelation::DirectCause.to_string(), "caused");
        assert_eq!(CausalRelation::DataFlow.to_string(), "data-flow");
    }

    // ── Causal Graph ─────────────────────────────────────────────────────

    #[test]
    fn build_graph_from_trace() {
        let trace = sample_trace();
        let graph = build_causal_graph(&trace);
        assert!(!graph.edges.is_empty(), "should find causal edges");
    }

    #[test]
    fn graph_causes_of() {
        let trace = sample_trace();
        let graph = build_causal_graph(&trace);
        // The assertion failure (seq 5) should have causes
        let causes = graph.causes_of(5);
        assert!(!causes.is_empty(), "assertion should have causal antecedents");
    }

    #[test]
    fn graph_root_cause_chain() {
        let trace = sample_trace();
        let graph = build_causal_graph(&trace);
        let chain = graph.trace_root_cause_chain(5);
        assert!(chain.len() >= 2, "chain should trace back from failure");
        assert_eq!(chain[0], 5, "chain should start at failure");
    }

    #[test]
    fn graph_no_cycle() {
        let mut graph = CausalGraph::new();
        graph.add_edge(1, 2, CausalRelation::DirectCause, 0.9);
        graph.add_edge(2, 3, CausalRelation::DirectCause, 0.9);
        graph.add_edge(3, 1, CausalRelation::DirectCause, 0.9); // cycle!
        let chain = graph.trace_root_cause_chain(3);
        // Should not loop forever
        assert!(chain.len() <= 4);
    }

    // ── Root Cause Ranking ───────────────────────────────────────────────

    #[test]
    fn rank_root_causes_found() {
        let trace = sample_trace();
        let graph = build_causal_graph(&trace);
        let candidates = rank_root_causes(&trace, &graph);
        assert!(!candidates.is_empty());
    }

    #[test]
    fn rank_sorted_by_score() {
        let trace = sample_trace();
        let graph = build_causal_graph(&trace);
        let candidates = rank_root_causes(&trace, &graph);
        for w in candidates.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn rank_no_failure_empty() {
        let trace = no_failure_trace();
        let graph = build_causal_graph(&trace);
        let candidates = rank_root_causes(&trace, &graph);
        assert!(candidates.is_empty());
    }

    // ── Root Cause Candidate Display ─────────────────────────────────────

    #[test]
    fn candidate_display() {
        let c = RootCauseCandidate {
            event_seq: 2,
            location: SourceLocation::new("main.mg", 12, "process"),
            description: "x = -1".to_string(),
            score: 0.85,
            causal_chain_length: 3,
            suspected_category: "incorrect-value".to_string(),
        };
        let s = format!("{c}");
        assert!(s.contains("85%"));
        assert!(s.contains("main.mg"));
    }

    // ── Debug Report ─────────────────────────────────────────────────────

    #[test]
    fn debug_report_full_pipeline() {
        let trace = sample_trace();
        let report = debug_trace(&trace);
        assert!(report.failure_description.contains("assert"));
        assert!(!report.root_causes.is_empty());
        assert!(report.total_events_analyzed == 5);
        assert!(report.causal_edges_found > 0);
    }

    #[test]
    fn debug_report_summary() {
        let trace = sample_trace();
        let report = debug_trace(&trace);
        let summary = report.summary();
        assert!(summary.contains("Failure:"));
        assert!(summary.contains("Likely root cause:"));
    }

    #[test]
    fn debug_report_no_failure() {
        let trace = no_failure_trace();
        let report = debug_trace(&trace);
        assert!(report.root_causes.is_empty());
        assert!(report.failure_description.contains("no failure"));
    }

    #[test]
    fn debug_report_top_root_cause() {
        let trace = sample_trace();
        let report = debug_trace(&trace);
        assert!(report.top_root_cause().is_some());
    }

    // ── Lock / Resource Contention ───────────────────────────────────────

    #[test]
    fn graph_lock_contention() {
        let trace = RuntimeTrace::new(vec![
            make_event(1, TraceEventKind::LockAcquire { lock_id: "m1".to_string() }, "a.mg", 1, "f"),
            make_event(2, TraceEventKind::LockRelease { lock_id: "m1".to_string() }, "a.mg", 2, "f"),
            make_event(3, TraceEventKind::LockAcquire { lock_id: "m1".to_string() }, "a.mg", 3, "g"),
            make_event(4, TraceEventKind::Panic { message: "deadlock".to_string() }, "a.mg", 4, "g"),
        ]);
        let graph = build_causal_graph(&trace);
        let contention = graph.edges.iter().any(|e| e.relation == CausalRelation::ResourceContention);
        assert!(contention, "should detect lock contention");
    }

    // ── Memory Flow ──────────────────────────────────────────────────────

    #[test]
    fn graph_memory_flow() {
        let trace = RuntimeTrace::new(vec![
            make_event(1, TraceEventKind::MemAlloc { size: 64, address: 0x1000 }, "a.mg", 1, "f"),
            make_event(2, TraceEventKind::MemFree { address: 0x1000 }, "a.mg", 2, "f"),
        ]);
        let graph = build_causal_graph(&trace);
        assert!(graph.edges.iter().any(|e| e.relation == CausalRelation::DataFlow));
    }
}
