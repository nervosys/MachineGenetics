//! Observability System
//!
//! Structured logging, metrics collection, and distributed tracing:
//!
//! - **MetricsCollector**: Counters, gauges, histograms with labels
//! - **SpanTracer**: Distributed tracing with context propagation
//! - **EventLog**: Structured event logging with severity levels
//! - **Dashboard**: Aggregated system health snapshots

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Metrics
// ============================================================================

/// A labeled metric value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricValue {
    /// Monotonically increasing counter
    Counter(u64),
    /// Gauge (can go up or down)
    Gauge(f64),
    /// Histogram (observed values)
    Histogram(HistogramData),
}

/// Histogram data with bucketed observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramData {
    /// Bucket boundaries
    pub bounds: Vec<f64>,
    /// Counts per bucket
    pub counts: Vec<u64>,
    /// Total sum of observations
    pub sum: f64,
    /// Total count
    pub count: u64,
    /// Min observed value
    pub min: f64,
    /// Max observed value
    pub max: f64,
}

impl HistogramData {
    /// Create with default exponential buckets.
    pub fn new() -> Self {
        Self::with_bounds(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ])
    }

    /// Create with custom bucket boundaries.
    pub fn with_bounds(bounds: Vec<f64>) -> Self {
        let counts = vec![0u64; bounds.len() + 1]; // +1 for +Inf bucket
        Self {
            bounds,
            counts,
            sum: 0.0,
            count: 0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
        }
    }

    /// Observe a value.
    pub fn observe(&mut self, value: f64) {
        self.sum += value;
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Find the right bucket
        let mut placed = false;
        for (i, &bound) in self.bounds.iter().enumerate() {
            if value <= bound {
                self.counts[i] += 1;
                placed = true;
                break;
            }
        }
        if !placed {
            // +Inf bucket
            if let Some(last) = self.counts.last_mut() {
                *last += 1;
            }
        }
    }

    /// Get average.
    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Estimate a percentile (linear interpolation between buckets).
    pub fn percentile(&self, p: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        let target = (p * self.count as f64).ceil() as u64;
        let mut cumulative = 0u64;

        for (i, &count) in self.counts.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                if i < self.bounds.len() {
                    return self.bounds[i];
                }
                return self.max;
            }
        }

        self.max
    }
}

impl Default for HistogramData {
    fn default() -> Self {
        Self::new()
    }
}

/// Metric key with labels.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetricKey {
    /// Metric name
    pub name: String,
    /// Labels
    pub labels: Vec<(String, String)>,
}

impl MetricKey {
    /// Create a new metric key.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            labels: Vec::new(),
        }
    }

    /// Add a label.
    pub fn with_label(mut self, key: &str, value: &str) -> Self {
        self.labels.push((key.to_string(), value.to_string()));
        self.labels.sort();
        self
    }
}

/// Collects and stores metrics.
pub struct MetricsCollector {
    /// Counter metrics
    counters: RwLock<HashMap<MetricKey, AtomicU64>>,
    /// Gauge metrics
    gauges: RwLock<HashMap<MetricKey, f64>>,
    /// Histogram metrics
    histograms: RwLock<HashMap<MetricKey, HistogramData>>,
    /// Prefix for all metric names
    prefix: String,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new(prefix: &str) -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            prefix: prefix.to_string(),
        }
    }

    /// Increment a counter.
    pub fn counter_inc(&self, key: MetricKey, delta: u64) {
        let counters = self.counters.read().unwrap();
        if let Some(counter) = counters.get(&key) {
            counter.fetch_add(delta, Ordering::Relaxed);
        } else {
            drop(counters);
            let mut counters = self.counters.write().unwrap();
            counters
                .entry(key)
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(delta, Ordering::Relaxed);
        }
    }

    /// Get a counter value.
    pub fn counter_get(&self, key: &MetricKey) -> u64 {
        self.counters
            .read()
            .unwrap()
            .get(key)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Set a gauge.
    pub fn gauge_set(&self, key: MetricKey, value: f64) {
        self.gauges.write().unwrap().insert(key, value);
    }

    /// Get a gauge.
    pub fn gauge_get(&self, key: &MetricKey) -> Option<f64> {
        self.gauges.read().unwrap().get(key).copied()
    }

    /// Observe a histogram value.
    pub fn histogram_observe(&self, key: MetricKey, value: f64) {
        let mut histograms = self.histograms.write().unwrap();
        histograms
            .entry(key)
            .or_default()
            .observe(value);
    }

    /// Get histogram data.
    pub fn histogram_get(&self, key: &MetricKey) -> Option<HistogramData> {
        self.histograms.read().unwrap().get(key).cloned()
    }

    /// Get all metric names.
    pub fn metric_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for k in self.counters.read().unwrap().keys() {
            names.push(format!("{}_{}", self.prefix, k.name));
        }
        for k in self.gauges.read().unwrap().keys() {
            names.push(format!("{}_{}", self.prefix, k.name));
        }
        for k in self.histograms.read().unwrap().keys() {
            names.push(format!("{}_{}", self.prefix, k.name));
        }
        names.sort();
        names.dedup();
        names
    }

    /// Export all metrics as a snapshot.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut metrics = HashMap::new();

        for (k, v) in self.counters.read().unwrap().iter() {
            metrics.insert(k.clone(), MetricValue::Counter(v.load(Ordering::Relaxed)));
        }
        for (k, &v) in self.gauges.read().unwrap().iter() {
            metrics.insert(k.clone(), MetricValue::Gauge(v));
        }
        for (k, v) in self.histograms.read().unwrap().iter() {
            metrics.insert(k.clone(), MetricValue::Histogram(v.clone()));
        }

        MetricsSnapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            prefix: self.prefix.clone(),
            metrics,
        }
    }
}

/// Point-in-time metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Timestamp
    pub timestamp: f64,
    /// Prefix
    pub prefix: String,
    /// All metrics
    pub metrics: HashMap<MetricKey, MetricValue>,
}

// ============================================================================
// Distributed Tracing
// ============================================================================

/// A trace context for distributed tracing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    /// Trace ID (shared across spans in the same trace)
    pub trace_id: Uuid,
    /// Current span ID
    pub span_id: Uuid,
    /// Parent span ID (None for root)
    pub parent_span_id: Option<Uuid>,
    /// Sampling flag
    pub sampled: bool,
    /// Baggage items (propagated key-value pairs)
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    /// Create a new root trace context.
    pub fn new() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            span_id: Uuid::new_v4(),
            parent_span_id: None,
            sampled: true,
            baggage: HashMap::new(),
        }
    }

    /// Create a child context.
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id: Uuid::new_v4(),
            parent_span_id: Some(self.span_id),
            sampled: self.sampled,
            baggage: self.baggage.clone(),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// A span representing a unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Span ID
    pub id: Uuid,
    /// Trace context
    pub context: TraceContext,
    /// Operation name
    pub operation: String,
    /// Service name
    pub service: String,
    /// Start time (epoch seconds)
    pub start_time: f64,
    /// Duration in seconds
    pub duration: Option<f64>,
    /// Status
    pub status: SpanStatus,
    /// Tags
    pub tags: HashMap<String, String>,
    /// Events/logs within the span
    pub events: Vec<SpanEvent>,
}

/// Span status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Completed successfully
    Ok,
    /// Ended with error
    Error,
    /// Was cancelled
    Cancelled,
    /// Still in progress
    InProgress,
}

/// An event within a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Timestamp
    pub timestamp: f64,
    /// Attributes
    pub attributes: HashMap<String, String>,
}

impl Span {
    /// Create a new span.
    pub fn new(ctx: TraceContext, operation: &str, service: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: ctx.span_id,
            context: ctx,
            operation: operation.to_string(),
            service: service.to_string(),
            start_time: now,
            duration: None,
            status: SpanStatus::InProgress,
            tags: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Finish the span.
    pub fn finish(&mut self, status: SpanStatus) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.duration = Some(now - self.start_time);
        self.status = status;
    }

    /// Add a tag.
    pub fn tag(&mut self, key: &str, value: &str) {
        self.tags.insert(key.to_string(), value.to_string());
    }

    /// Add an event.
    pub fn event(&mut self, name: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.events.push(SpanEvent {
            name: name.to_string(),
            timestamp: now,
            attributes: HashMap::new(),
        });
    }
}

/// Span collector for a tracing backend.
pub struct SpanCollector {
    /// Collected spans
    spans: RwLock<Vec<Span>>,
    /// Maximum spans to retain
    max_spans: usize,
    /// Service name
    service: String,
}

impl SpanCollector {
    /// Create a new span collector.
    pub fn new(service: &str, max_spans: usize) -> Self {
        Self {
            spans: RwLock::new(Vec::new()),
            max_spans,
            service: service.to_string(),
        }
    }

    /// Start a new root span.
    pub fn start_span(&self, operation: &str) -> Span {
        let ctx = TraceContext::new();
        Span::new(ctx, operation, &self.service)
    }

    /// Start a child span.
    pub fn start_child(&self, parent: &Span, operation: &str) -> Span {
        let ctx = parent.context.child();
        Span::new(ctx, operation, &self.service)
    }

    /// Record a finished span.
    pub fn record(&self, span: Span) {
        let mut spans = self.spans.write().unwrap();
        spans.push(span);
        if spans.len() > self.max_spans {
            { let n = spans.len() - self.max_spans; spans.drain(0..n) };
        }
    }

    /// Get spans for a trace.
    pub fn trace_spans(&self, trace_id: Uuid) -> Vec<Span> {
        self.spans
            .read()
            .unwrap()
            .iter()
            .filter(|s| s.context.trace_id == trace_id)
            .cloned()
            .collect()
    }

    /// Get total recorded spans.
    pub fn span_count(&self) -> usize {
        self.spans.read().unwrap().len()
    }

    /// Get recent spans.
    pub fn recent(&self, count: usize) -> Vec<Span> {
        let spans = self.spans.read().unwrap();
        spans.iter().rev().take(count).cloned().collect()
    }
}

// ============================================================================
// Structured Event Logging
// ============================================================================

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Trace level
    Trace,
    /// Debug level
    Debug,
    /// Informational
    Info,
    /// Warning
    Warn,
    /// Error
    Error,
    /// Fatal
    Fatal,
}

/// A structured log event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    /// Timestamp
    pub timestamp: f64,
    /// Severity
    pub severity: Severity,
    /// Message
    pub message: String,
    /// Source (module or component)
    pub source: String,
    /// Structured fields
    pub fields: HashMap<String, String>,
    /// Associated trace ID (if any)
    pub trace_id: Option<Uuid>,
}

/// Event log collector.
pub struct EventLog {
    /// Events
    events: RwLock<Vec<LogEvent>>,
    /// Maximum events
    max_events: usize,
    /// Minimum severity to retain
    min_severity: Severity,
}

impl EventLog {
    /// Create a new event log.
    pub fn new(max_events: usize, min_severity: Severity) -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            max_events,
            min_severity,
        }
    }

    /// Log an event.
    pub fn log(&self, severity: Severity, source: &str, message: &str) {
        if severity < self.min_severity {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let event = LogEvent {
            timestamp: now,
            severity,
            message: message.to_string(),
            source: source.to_string(),
            fields: HashMap::new(),
            trace_id: None,
        };

        let mut events = self.events.write().unwrap();
        events.push(event);
        if events.len() > self.max_events {
            { let n = events.len() - self.max_events; events.drain(0..n) };
        }
    }

    /// Log with structured fields.
    pub fn log_fields(
        &self,
        severity: Severity,
        source: &str,
        message: &str,
        fields: HashMap<String, String>,
        trace_id: Option<Uuid>,
    ) {
        if severity < self.min_severity {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let event = LogEvent {
            timestamp: now,
            severity,
            message: message.to_string(),
            source: source.to_string(),
            fields,
            trace_id,
        };

        let mut events = self.events.write().unwrap();
        events.push(event);
        if events.len() > self.max_events {
            { let n = events.len() - self.max_events; events.drain(0..n) };
        }
    }

    /// Get events by severity.
    pub fn events_by_severity(&self, severity: Severity) -> Vec<LogEvent> {
        self.events
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.severity == severity)
            .cloned()
            .collect()
    }

    /// Get recent events.
    pub fn recent(&self, count: usize) -> Vec<LogEvent> {
        let events = self.events.read().unwrap();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Get total event count.
    pub fn count(&self) -> usize {
        self.events.read().unwrap().len()
    }

    /// Get error count.
    pub fn error_count(&self) -> usize {
        self.events
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.severity >= Severity::Error)
            .count()
    }
}

// ============================================================================
// Dashboard
// ============================================================================

/// Aggregated health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Overall health status
    pub status: SystemHealth,
    /// Agent count
    pub agent_count: usize,
    /// Active tasks
    pub active_tasks: usize,
    /// Error rate (errors per second)
    pub error_rate: f64,
    /// Average latency in ms
    pub avg_latency_ms: f64,
    /// P99 latency in ms
    pub p99_latency_ms: f64,
    /// Memory usage in bytes
    pub memory_used: u64,
    /// CPU utilization (0.0-1.0)
    pub cpu_utilization: f64,
    /// Uptime in seconds
    pub uptime_secs: f64,
    /// Timestamp
    pub timestamp: f64,
}

/// System health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemHealth {
    /// System is healthy
    Healthy,
    /// System is degraded
    Degraded,
    /// System is critical
    Critical,
    /// Health unknown
    Unknown,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram() {
        let mut hist = HistogramData::new();
        hist.observe(0.003);
        hist.observe(0.05);
        hist.observe(0.5);
        hist.observe(2.0);
        hist.observe(15.0);

        assert_eq!(hist.count, 5);
        assert_eq!(hist.min, 0.003);
        assert_eq!(hist.max, 15.0);
        assert!(hist.avg() > 0.0);
    }

    #[test]
    fn test_histogram_percentile() {
        let mut hist = HistogramData::with_bounds(vec![1.0, 5.0, 10.0]);
        for i in 0..100 {
            hist.observe(i as f64 / 10.0);
        }

        let p50 = hist.percentile(0.5);
        let p99 = hist.percentile(0.99);
        assert!(p50 <= p99);
    }

    #[test]
    fn test_metrics_counter() {
        let mc = MetricsCollector::new("rmi");
        let key = MetricKey::new("requests_total").with_label("method", "GET");

        mc.counter_inc(key.clone(), 1);
        mc.counter_inc(key.clone(), 2);

        assert_eq!(mc.counter_get(&key), 3);
    }

    #[test]
    fn test_metrics_gauge() {
        let mc = MetricsCollector::new("rmi");
        let key = MetricKey::new("temperature");

        mc.gauge_set(key.clone(), 72.5);
        assert_eq!(mc.gauge_get(&key), Some(72.5));
    }

    #[test]
    fn test_metrics_histogram() {
        let mc = MetricsCollector::new("rmi");
        let key = MetricKey::new("latency");

        mc.histogram_observe(key.clone(), 0.1);
        mc.histogram_observe(key.clone(), 0.5);

        let hist = mc.histogram_get(&key).unwrap();
        assert_eq!(hist.count, 2);
    }

    #[test]
    fn test_metrics_snapshot() {
        let mc = MetricsCollector::new("rmi");
        mc.counter_inc(MetricKey::new("a"), 1);
        mc.gauge_set(MetricKey::new("b"), 42.0);

        let snap = mc.snapshot();
        assert_eq!(snap.metrics.len(), 2);
    }

    #[test]
    fn test_trace_context() {
        let root = TraceContext::new();
        let child = root.child();

        assert_eq!(child.trace_id, root.trace_id);
        assert_ne!(child.span_id, root.span_id);
        assert_eq!(child.parent_span_id, Some(root.span_id));
    }

    #[test]
    fn test_span() {
        let ctx = TraceContext::new();
        let mut span = Span::new(ctx, "handle_request", "agent-service");

        span.tag("agent_id", "123");
        span.event("started_processing");
        span.finish(SpanStatus::Ok);

        assert!(span.duration.is_some());
        assert_eq!(span.status, SpanStatus::Ok);
        assert_eq!(span.tags.len(), 1);
        assert_eq!(span.events.len(), 1);
    }

    #[test]
    fn test_span_collector() {
        let sc = SpanCollector::new("test-service", 100);
        let mut span = sc.start_span("operation");
        let mut child = sc.start_child(&span, "sub_operation");

        child.finish(SpanStatus::Ok);
        sc.record(child);

        span.finish(SpanStatus::Ok);
        let trace_id = span.context.trace_id;
        sc.record(span);

        assert_eq!(sc.span_count(), 2);
        assert_eq!(sc.trace_spans(trace_id).len(), 2);
    }

    #[test]
    fn test_event_log() {
        let log = EventLog::new(100, Severity::Info);

        log.log(Severity::Debug, "test", "debug message"); // Below min
        log.log(Severity::Info, "test", "info message");
        log.log(Severity::Error, "test", "error!");

        assert_eq!(log.count(), 2); // Debug was filtered
        assert_eq!(log.error_count(), 1);
    }

    #[test]
    fn test_event_log_fields() {
        let log = EventLog::new(100, Severity::Trace);

        let mut fields = HashMap::new();
        fields.insert("agent_id".to_string(), "abc".to_string());

        log.log_fields(
            Severity::Info,
            "agent",
            "Task completed",
            fields,
            Some(Uuid::new_v4()),
        );

        let recent = log.recent(1);
        assert_eq!(recent.len(), 1);
        assert!(recent[0].trace_id.is_some());
        assert!(recent[0].fields.contains_key("agent_id"));
    }

    #[test]
    fn test_event_log_capacity() {
        let log = EventLog::new(5, Severity::Trace);

        for i in 0..10 {
            log.log(Severity::Info, "test", &format!("msg {}", i));
        }

        assert_eq!(log.count(), 5); // Capped at max
    }

    #[test]
    fn test_metric_key_labels() {
        let k1 = MetricKey::new("foo")
            .with_label("a", "1")
            .with_label("b", "2");
        let k2 = MetricKey::new("foo")
            .with_label("b", "2")
            .with_label("a", "1");
        assert_eq!(k1, k2); // Order-independent due to sorting
    }
}
