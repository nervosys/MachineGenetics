//! Message Bus - High-Performance Agent Communication Infrastructure
//!
//! This module provides a message bus for coordinating communication between agents:
//!
//! 1. **Pub/Sub**: Topic-based message routing
//! 2. **Request/Reply**: RPC-style communication with correlation
//! 3. **Broadcast**: Efficient one-to-many messaging
//! 4. **Streaming**: Continuous data flow support
//! 5. **Dead Letter Queue**: Failed message handling
//!
//! The bus supports both local (in-process) and distributed (network) modes.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Message Bus Types
// ============================================================================

/// Topic for pub/sub messaging.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Topic {
    /// Topic name (hierarchical, separated by '.')
    pub name: String,
    /// Namespace (for isolation)
    pub namespace: Option<String>,
}

impl Topic {
    /// Create a new topic.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            namespace: None,
        }
    }

    /// Create a topic with namespace.
    pub fn with_namespace(name: &str, namespace: &str) -> Self {
        Self {
            name: name.to_string(),
            namespace: Some(namespace.to_string()),
        }
    }

    /// Get full topic path.
    #[inline]
    pub fn full_path(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}", ns, self.name),
            None => self.name.clone(),
        }
    }

    /// Check if this topic matches a pattern (supports wildcards).
    #[inline]
    pub fn matches(&self, pattern: &str) -> bool {
        let parts: Vec<&str> = self.name.split('.').collect();
        let pattern_parts: Vec<&str> = pattern.split('.').collect();

        let mut i = 0;
        let mut j = 0;

        while i < parts.len() && j < pattern_parts.len() {
            match pattern_parts[j] {
                "#" => return true, // Multi-level wildcard
                "*" => {
                    i += 1;
                    j += 1;
                }
                p if p == parts[i] => {
                    i += 1;
                    j += 1;
                }
                _ => return false,
            }
        }

        i == parts.len() && j == pattern_parts.len()
    }
}

/// Envelope wrapping messages with routing info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Unique message ID
    pub id: Uuid,
    /// Topic
    pub topic: Topic,
    /// Sender agent ID
    pub sender: Uuid,
    /// Target agent ID (None for broadcast)
    pub target: Option<Uuid>,
    /// Payload
    pub payload: Vec<u8>,
    /// Headers
    pub headers: HashMap<String, String>,
    /// Timestamp
    pub timestamp: f64,
    /// Correlation ID for request/reply
    pub correlation_id: Option<Uuid>,
    /// Reply-to topic
    pub reply_to: Option<Topic>,
    /// Priority (0-10)
    pub priority: u8,
    /// TTL in milliseconds
    pub ttl_ms: u64,
    /// Number of delivery attempts
    pub delivery_attempts: u32,
}

impl Envelope {
    /// Create a new envelope.
    pub fn new(topic: Topic, sender: Uuid, payload: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            topic,
            sender,
            target: None,
            payload,
            headers: HashMap::new(),
            timestamp,
            correlation_id: None,
            reply_to: None,
            priority: 5,
            ttl_ms: 30_000, // 30 seconds default
            delivery_attempts: 0,
        }
    }

    /// Set target agent.
    pub fn with_target(mut self, target: Uuid) -> Self {
        self.target = Some(target);
        self
    }

    /// Set header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set correlation ID for request/reply.
    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Set reply-to topic.
    pub fn with_reply_to(mut self, topic: Topic) -> Self {
        self.reply_to = Some(topic);
        self
    }

    /// Set priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.min(10);
        self
    }

    /// Set TTL.
    pub fn with_ttl(mut self, ttl_ms: u64) -> Self {
        self.ttl_ms = ttl_ms;
        self
    }

    /// Check if envelope has expired.
    #[inline]
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let expiry = self.timestamp + (self.ttl_ms as f64 / 1000.0);
        now > expiry
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        let data = rmp_serde::to_vec(self).unwrap_or_default();
        lz4_flex::compress_prepend_size(&data)
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        let decompressed = lz4_flex::decompress_size_prepended(data)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;
        rmp_serde::from_slice(&decompressed).map_err(|e| RmiError::Serialization(e.to_string()))
    }

    /// Create a reply envelope.
    pub fn reply(&self, sender: Uuid, payload: Vec<u8>) -> Option<Self> {
        self.reply_to.as_ref().map(|topic| {
            Self::new(topic.clone(), sender, payload)
                .with_correlation(self.id)
                .with_target(self.sender)
        })
    }
}

// ============================================================================
// Subscription
// ============================================================================

/// Subscription to a topic pattern.
#[derive(Clone)]
pub struct Subscription {
    /// Subscription ID
    pub id: Uuid,
    /// Topic pattern (supports wildcards)
    pub pattern: String,
    /// Subscriber agent ID
    pub subscriber: Uuid,
    /// Filter function (optional)
    #[allow(clippy::type_complexity)]
    filter: Option<Arc<dyn Fn(&Envelope) -> bool + Send + Sync>>,
    /// Max concurrent deliveries
    pub max_concurrent: usize,
    /// Created timestamp
    pub created_at: f64,
}

impl Subscription {
    /// Create a new subscription.
    pub fn new(pattern: &str, subscriber: Uuid) -> Self {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            pattern: pattern.to_string(),
            subscriber,
            filter: None,
            max_concurrent: 100,
            created_at,
        }
    }

    /// Add a filter function.
    pub fn with_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&Envelope) -> bool + Send + Sync + 'static,
    {
        self.filter = Some(Arc::new(filter));
        self
    }

    /// Set max concurrent deliveries.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Check if envelope matches this subscription.
    #[inline]
    pub fn matches(&self, envelope: &Envelope) -> bool {
        if !envelope.topic.matches(&self.pattern) {
            return false;
        }

        if let Some(ref filter) = self.filter {
            return filter(envelope);
        }

        true
    }
}

// ============================================================================
// Message Handler
// ============================================================================

/// Handler for received messages.
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message.
    async fn handle(&self, envelope: Envelope) -> Result<Option<Vec<u8>>>;

    /// Get handler name for debugging.
    fn name(&self) -> &str {
        "anonymous"
    }
}

/// Simple function-based handler.
pub struct FnHandler<F> {
    name: String,
    func: F,
}

impl<F> FnHandler<F>
where
    F: Fn(Envelope) -> Result<Option<Vec<u8>>> + Send + Sync,
{
    /// Create a new function handler.
    pub fn new(name: &str, func: F) -> Self {
        Self {
            name: name.to_string(),
            func,
        }
    }
}

#[async_trait]
impl<F> MessageHandler for FnHandler<F>
where
    F: Fn(Envelope) -> Result<Option<Vec<u8>>> + Send + Sync,
{
    async fn handle(&self, envelope: Envelope) -> Result<Option<Vec<u8>>> {
        (self.func)(envelope)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ============================================================================
// Dead Letter Queue
// ============================================================================

/// Entry in the dead letter queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// Original envelope
    pub envelope: Envelope,
    /// Failure reason
    pub reason: String,
    /// Failure timestamp
    pub failed_at: f64,
    /// Number of retry attempts
    pub retry_count: u32,
}

/// Dead letter queue for failed messages.
pub struct DeadLetterQueue {
    /// Queue entries
    entries: RwLock<VecDeque<DeadLetterEntry>>,
    /// Maximum queue size
    max_size: usize,
    /// Maximum retry attempts before discarding
    max_retries: u32,
}

impl DeadLetterQueue {
    /// Create a new dead letter queue.
    pub fn new(max_size: usize, max_retries: u32) -> Self {
        Self {
            entries: RwLock::new(VecDeque::new()),
            max_size,
            max_retries,
        }
    }

    /// Add an entry to the queue.
    pub fn add(&self, envelope: Envelope, reason: &str) {
        let failed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let entry = DeadLetterEntry {
            envelope,
            reason: reason.to_string(),
            failed_at,
            retry_count: 0,
        };

        let mut entries = self.entries.write().unwrap();
        if entries.len() >= self.max_size {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Get entries ready for retry.
    pub fn get_retryable(&self) -> Vec<DeadLetterEntry> {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .filter(|e| e.retry_count < self.max_retries && !e.envelope.is_expired())
            .cloned()
            .collect()
    }

    /// Mark an entry as retried.
    pub fn mark_retried(&self, envelope_id: Uuid) {
        let mut entries = self.entries.write().unwrap();
        if let Some(entry) = entries.iter_mut().find(|e| e.envelope.id == envelope_id) {
            entry.retry_count += 1;
        }
    }

    /// Remove an entry.
    pub fn remove(&self, envelope_id: Uuid) {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| e.envelope.id != envelope_id);
    }

    /// Get queue length.
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// Message Bus
// ============================================================================

/// Statistics for the message bus.
#[derive(Debug, Clone, Default)]
pub struct BusStats {
    /// Total messages published
    pub messages_published: u64,
    /// Total messages delivered
    pub messages_delivered: u64,
    /// Total messages failed
    pub messages_failed: u64,
    /// Active subscriptions
    pub active_subscriptions: usize,
    /// Active topics
    pub active_topics: usize,
    /// Dead letter queue size
    pub dlq_size: usize,
}

/// High-performance message bus for agent communication.
pub struct MessageBus {
    /// Bus ID
    id: Uuid,
    /// Subscriptions by pattern
    subscriptions: RwLock<HashMap<String, Vec<Subscription>>>,
    /// Subscriber channels
    channels: RwLock<HashMap<Uuid, mpsc::Sender<Envelope>>>,
    /// Pending request/reply futures
    pending_requests: RwLock<HashMap<Uuid, oneshot::Sender<Envelope>>>,
    /// Dead letter queue
    dlq: Arc<DeadLetterQueue>,
    /// Atomic stats counters (lock-free for publish hot path)
    stat_published: AtomicU64,
    stat_delivered: AtomicU64,
    stat_failed: AtomicU64,
    stat_subscriptions: AtomicU64,
    /// Running flag
    running: RwLock<bool>,
    /// Shutdown signal
    shutdown_tx: watch::Sender<bool>,
}

impl MessageBus {
    /// Create a new message bus.
    pub fn new() -> Self {
        let (shutdown_tx, _) = watch::channel(false);

        Self {
            id: Uuid::new_v4(),
            subscriptions: RwLock::new(HashMap::new()),
            channels: RwLock::new(HashMap::new()),
            pending_requests: RwLock::new(HashMap::new()),
            dlq: Arc::new(DeadLetterQueue::new(10000, 3)),
            stat_published: AtomicU64::new(0),
            stat_delivered: AtomicU64::new(0),
            stat_failed: AtomicU64::new(0),
            stat_subscriptions: AtomicU64::new(0),
            running: RwLock::new(true),
            shutdown_tx,
        }
    }

    /// Get bus ID.
    #[inline]
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Subscribe to a topic pattern.
    pub fn subscribe(&self, subscription: Subscription) -> mpsc::Receiver<Envelope> {
        let (tx, rx) = mpsc::channel(subscription.max_concurrent);
        let subscriber = subscription.subscriber;
        let pattern = subscription.pattern.clone();

        // Store channel
        {
            let mut channels = self.channels.write().unwrap();
            channels.insert(subscriber, tx);
        }

        // Add subscription
        {
            let mut subs = self.subscriptions.write().unwrap();
            subs.entry(pattern).or_default().push(subscription);
        }

        // Update stats
        self.stat_subscriptions.fetch_add(1, Ordering::Relaxed);

        rx
    }

    /// Unsubscribe.
    pub fn unsubscribe(&self, subscription_id: Uuid) {
        let mut subs = self.subscriptions.write().unwrap();
        for patterns in subs.values_mut() {
            patterns.retain(|s| s.id != subscription_id);
        }

        // Update stats
        self.stat_subscriptions.fetch_sub(1, Ordering::Relaxed);
    }

    /// Publish a message.
    pub async fn publish(&self, envelope: Envelope) -> Result<()> {
        if !*self.running.read().unwrap() {
            return Err(RmiError::protocol_simple("Bus is shut down"));
        }

        // Update stats (atomic, no lock)
        self.stat_published.fetch_add(1, Ordering::Relaxed);

        // Find matching subscriptions
        let matching_subs: Vec<Subscription> = {
            let subs = self.subscriptions.read().unwrap();
            subs.values()
                .flatten()
                .filter(|s| s.matches(&envelope))
                .cloned()
                .collect()
        };

        // Deliver to subscribers
        let channels = self.channels.read().unwrap();
        for sub in matching_subs {
            if let Some(tx) = channels.get(&sub.subscriber) {
                match tx.try_send(envelope.clone()) {
                    Ok(_) => {
                        self.stat_delivered.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        self.dlq.add(envelope.clone(), "Channel full or closed");
                        self.stat_failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }

        Ok(())
    }

    /// Publish and wait for reply.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn request(&self, mut envelope: Envelope, timeout: Duration) -> Result<Envelope> {
        let (tx, rx) = oneshot::channel();
        let correlation_id = Uuid::new_v4();

        envelope.correlation_id = Some(correlation_id);

        // Store pending request
        {
            let mut pending = self.pending_requests.write().unwrap();
            pending.insert(correlation_id, tx);
        }

        // Publish request
        self.publish(envelope).await?;

        // Wait for reply with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(_)) => Err(RmiError::protocol_simple("Reply channel closed")),
            Err(_) => {
                // Remove pending request
                let mut pending = self.pending_requests.write().unwrap();
                pending.remove(&correlation_id);
                Err(RmiError::protocol_simple("Request timed out"))
            }
        }
    }

    /// Send a reply to a request.
    pub fn reply(&self, reply_envelope: Envelope) -> Result<()> {
        if let Some(correlation_id) = reply_envelope.correlation_id {
            let mut pending = self.pending_requests.write().unwrap();
            if let Some(tx) = pending.remove(&correlation_id) {
                let _ = tx.send(reply_envelope);
            }
        }
        Ok(())
    }

    /// Broadcast to all subscribers of a topic.
    pub async fn broadcast(&self, topic: &str, sender: Uuid, payload: Vec<u8>) -> Result<u32> {
        let envelope = Envelope::new(Topic::new(topic), sender, payload);

        let matching_subs: Vec<Subscription> = {
            let subs = self.subscriptions.read().unwrap();
            subs.values()
                .flatten()
                .filter(|s| envelope.topic.matches(&s.pattern))
                .cloned()
                .collect()
        };

        let count = matching_subs.len() as u32;
        self.publish(envelope).await?;
        Ok(count)
    }

    /// Get bus statistics.
    pub fn stats(&self) -> BusStats {
        BusStats {
            messages_published: self.stat_published.load(Ordering::Relaxed),
            messages_delivered: self.stat_delivered.load(Ordering::Relaxed),
            messages_failed: self.stat_failed.load(Ordering::Relaxed),
            active_subscriptions: self.stat_subscriptions.load(Ordering::Relaxed) as usize,
            active_topics: self.subscriptions.read().unwrap().len(),
            dlq_size: self.dlq.len(),
        }
    }

    /// Get dead letter queue.
    pub fn dlq(&self) -> Arc<DeadLetterQueue> {
        self.dlq.clone()
    }

    /// Shutdown the bus.
    pub fn shutdown(&self) {
        *self.running.write().unwrap() = false;
        let _ = self.shutdown_tx.send(true);
    }

    /// Check if bus is running.
    #[inline]
    pub fn is_running(&self) -> bool {
        *self.running.read().unwrap()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Standard Topics
// ============================================================================

/// Standard topic names used by the framework.
pub mod topics {
    /// Agent lifecycle events.
    pub const AGENT_LIFECYCLE: &str = "system.agent.lifecycle";
    /// Agent discovery.
    pub const AGENT_DISCOVERY: &str = "system.agent.discovery";
    /// Agent heartbeats.
    pub const AGENT_HEARTBEAT: &str = "system.agent.heartbeat";

    /// Task assignments.
    pub const TASK_ASSIGN: &str = "task.assign";
    /// Task progress updates.
    pub const TASK_PROGRESS: &str = "task.progress";
    /// Task completions.
    pub const TASK_COMPLETE: &str = "task.complete";
    /// Task failures.
    pub const TASK_FAILED: &str = "task.failed";

    /// Tensor transfers.
    pub const TENSOR_TRANSFER: &str = "data.tensor";
    /// Gradient sharing.
    pub const GRADIENT_SHARE: &str = "data.gradient";
    /// Model checkpoints.
    pub const MODEL_CHECKPOINT: &str = "data.checkpoint";

    /// Query requests.
    pub const QUERY_REQUEST: &str = "reasoning.query";
    /// Query responses.
    pub const QUERY_RESPONSE: &str = "reasoning.response";
    /// Inference requests.
    pub const INFERENCE_REQUEST: &str = "reasoning.inference";

    /// Coordination proposals.
    pub const PROPOSAL: &str = "consensus.proposal";
    /// Votes.
    pub const VOTE: &str = "consensus.vote";
    /// Commits.
    pub const COMMIT: &str = "consensus.commit";

    /// Metrics.
    pub const METRICS: &str = "monitoring.metrics";
    /// Alerts.
    pub const ALERTS: &str = "monitoring.alerts";
    /// Logs.
    pub const LOGS: &str = "monitoring.logs";
}

// ============================================================================
// Agent Communication Trait
// ============================================================================

/// Trait for agents that can communicate via the message bus.
#[async_trait]
pub trait Communicator: Send + Sync {
    /// Get agent ID.
    fn agent_id(&self) -> Uuid;

    /// Send a message to another agent.
    async fn send(&self, target: Uuid, topic: &str, payload: Vec<u8>) -> Result<()>;

    /// Send a request and wait for reply.
    async fn request(
        &self,
        target: Uuid,
        topic: &str,
        payload: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>>;

    /// Broadcast to all agents on a topic.
    async fn broadcast(&self, topic: &str, payload: Vec<u8>) -> Result<u32>;

    /// Subscribe to a topic.
    fn subscribe(&self, pattern: &str) -> mpsc::Receiver<Envelope>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_matching() {
        let topic = Topic::new("agents.training.loss");

        assert!(topic.matches("agents.training.loss"));
        assert!(topic.matches("agents.*.loss"));
        assert!(topic.matches("agents.#"));
        assert!(topic.matches("#"));

        assert!(!topic.matches("agents.training"));
        assert!(!topic.matches("agents.inference.loss"));
    }

    #[test]
    fn test_envelope_serialization() {
        let envelope = Envelope::new(Topic::new("test.topic"), Uuid::new_v4(), vec![1, 2, 3, 4])
            .with_priority(8)
            .with_header("key", "value");

        let binary = envelope.to_binary();
        let restored = Envelope::from_binary(&binary).unwrap();

        assert_eq!(envelope.id, restored.id);
        assert_eq!(envelope.topic.name, restored.topic.name);
        assert_eq!(envelope.payload, restored.payload);
        assert_eq!(envelope.priority, restored.priority);
    }

    #[test]
    fn test_subscription_matching() {
        let sub = Subscription::new("agents.*", Uuid::new_v4());

        let matching = Envelope::new(Topic::new("agents.test"), Uuid::new_v4(), vec![]);

        let non_matching = Envelope::new(Topic::new("tasks.test"), Uuid::new_v4(), vec![]);

        assert!(sub.matches(&matching));
        assert!(!sub.matches(&non_matching));
    }

    #[test]
    fn test_dead_letter_queue() {
        let dlq = DeadLetterQueue::new(100, 3);

        let envelope = Envelope::new(Topic::new("test"), Uuid::new_v4(), vec![]);
        dlq.add(envelope.clone(), "Test failure");

        assert_eq!(dlq.len(), 1);
        assert!(!dlq.is_empty());

        let retryable = dlq.get_retryable();
        assert_eq!(retryable.len(), 1);

        dlq.mark_retried(envelope.id);
        dlq.mark_retried(envelope.id);
        dlq.mark_retried(envelope.id);

        // After 3 retries, should not be retryable
        let retryable = dlq.get_retryable();
        assert_eq!(retryable.len(), 0);
    }

    #[tokio::test]
    async fn test_message_bus_pubsub() {
        let bus = MessageBus::new();

        let subscriber = Uuid::new_v4();
        let sub = Subscription::new("test.*", subscriber);
        let mut rx = bus.subscribe(sub);

        let sender = Uuid::new_v4();
        let envelope = Envelope::new(Topic::new("test.message"), sender, vec![42]);

        bus.publish(envelope).await.unwrap();

        // Allow time for delivery
        tokio::time::sleep(Duration::from_millis(10)).await;

        let received = rx.try_recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap().payload, vec![42]);
    }

    #[test]
    fn test_bus_stats() {
        let bus = MessageBus::new();

        let stats = bus.stats();
        assert_eq!(stats.messages_published, 0);
        assert_eq!(stats.active_subscriptions, 0);
    }

    #[test]
    fn test_topic_with_namespace() {
        let topic = Topic::with_namespace("loss", "training");
        assert_eq!(topic.full_path(), "training:loss");
    }

    #[test]
    fn test_topic_full_path_no_namespace() {
        let topic = Topic::new("simple");
        assert_eq!(topic.full_path(), "simple");
    }

    #[test]
    fn test_envelope_with_target() {
        let target = Uuid::new_v4();
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![])
            .with_target(target);
        assert_eq!(env.target, Some(target));
    }

    #[test]
    fn test_envelope_with_correlation() {
        let cid = Uuid::new_v4();
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![])
            .with_correlation(cid);
        assert_eq!(env.correlation_id, Some(cid));
    }

    #[test]
    fn test_envelope_with_reply_to() {
        let reply_topic = Topic::new("reply.here");
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![])
            .with_reply_to(reply_topic);
        assert!(env.reply_to.is_some());
        assert_eq!(env.reply_to.unwrap().name, "reply.here");
    }

    #[test]
    fn test_envelope_with_ttl_not_expired() {
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![])
            .with_ttl(60_000);
        assert!(!env.is_expired());
    }

    #[test]
    fn test_envelope_no_ttl_never_expires() {
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![]);
        assert!(!env.is_expired());
    }

    #[test]
    fn test_envelope_with_header() {
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![])
            .with_header("x-trace", "abc123");
        assert_eq!(env.headers.get("x-trace").unwrap(), "abc123");
    }

    #[test]
    fn test_envelope_reply() {
        let sender = Uuid::new_v4();
        let reply_topic = Topic::new("reply.channel");
        let env = Envelope::new(Topic::new("request"), sender, vec![1])
            .with_reply_to(reply_topic);
        let reply = env.reply(Uuid::new_v4(), vec![2]);
        assert!(reply.is_some());
        let r = reply.unwrap();
        assert_eq!(r.topic.name, "reply.channel");
        assert_eq!(r.payload, vec![2]);
    }

    #[test]
    fn test_envelope_reply_none_without_reply_to() {
        let env = Envelope::new(Topic::new("request"), Uuid::new_v4(), vec![]);
        assert!(env.reply(Uuid::new_v4(), vec![]).is_none());
    }

    #[test]
    fn test_subscription_with_max_concurrent() {
        let sub = Subscription::new("test.*", Uuid::new_v4())
            .with_max_concurrent(5);
        assert_eq!(sub.max_concurrent, 5);
    }

    #[test]
    fn test_dlq_remove() {
        let dlq = DeadLetterQueue::new(100, 3);
        let env = Envelope::new(Topic::new("t"), Uuid::new_v4(), vec![]);
        let id = env.id;
        dlq.add(env, "fail");
        assert_eq!(dlq.len(), 1);
        dlq.remove(id);
        assert_eq!(dlq.len(), 0);
    }

    #[test]
    fn test_bus_is_running() {
        let bus = MessageBus::new();
        assert!(bus.is_running());
        bus.shutdown();
        assert!(!bus.is_running());
    }

    #[test]
    fn test_bus_unsubscribe() {
        let bus = MessageBus::new();
        let sub = Subscription::new("test.*", Uuid::new_v4());
        let sub_id = sub.id;
        let _rx = bus.subscribe(sub);
        assert_eq!(bus.stats().active_subscriptions, 1);
        bus.unsubscribe(sub_id);
        assert_eq!(bus.stats().active_subscriptions, 0);
    }

    #[test]
    fn test_topic_constants_defined() {
        assert_eq!(topics::AGENT_LIFECYCLE, "system.agent.lifecycle");
        assert_eq!(topics::TASK_ASSIGN, "task.assign");
        assert_eq!(topics::TENSOR_TRANSFER, "data.tensor");
        assert_eq!(topics::PROPOSAL, "consensus.proposal");
        assert_eq!(topics::METRICS, "monitoring.metrics");
    }

}
