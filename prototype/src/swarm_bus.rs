// ── Swarm Message Bus ──────────────────────────────────────────────
//
// Typed, zero-copy-friendly message bus for inter-agent communication
// in the Redox compiler swarm.
//
// Design:
//   - `SwarmMessage` enum: typed payloads for all swarm operations
//   - `MessageBus`: in-process pub/sub with topic routing
//   - `Envelope`: header (sender, recipient, timestamp, correlation)
//     + payload
//   - Zero-copy: messages are owned, but the bus avoids cloning when
//     delivering to a single subscriber
//   - Sub-µs latency: designed for single-process, lock-free-style
//     delivery via per-agent mailboxes

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

// ── IDs ────────────────────────────────────────────────────────────

pub type AgentId = String;
pub type MessageId = u64;
pub type CorrelationId = u64;

// ── Topic ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Topic {
    LeaseRequest,
    LeaseGrant,
    CrdtOp,
    ConsensusPropose,
    ConsensusVote,
    TaskAssign,
    TaskComplete,
    Diagnostic,
    Heartbeat,
    Custom(String),
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Topic::LeaseRequest => write!(f, "lease.request"),
            Topic::LeaseGrant => write!(f, "lease.grant"),
            Topic::CrdtOp => write!(f, "crdt.op"),
            Topic::ConsensusPropose => write!(f, "consensus.propose"),
            Topic::ConsensusVote => write!(f, "consensus.vote"),
            Topic::TaskAssign => write!(f, "task.assign"),
            Topic::TaskComplete => write!(f, "task.complete"),
            Topic::Diagnostic => write!(f, "diagnostic"),
            Topic::Heartbeat => write!(f, "heartbeat"),
            Topic::Custom(s) => write!(f, "custom.{s}"),
        }
    }
}

// ── Payload ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// Raw text / JSON payload.
    Text(String),
    /// Binary payload (e.g. serialised CRDT op).
    Binary(Vec<u8>),
    /// Key-value map payload.
    Map(BTreeMap<String, String>),
    /// Empty (e.g. heartbeat).
    Empty,
}

impl Payload {
    pub fn text(s: impl Into<String>) -> Self {
        Payload::Text(s.into())
    }

    pub fn byte_len(&self) -> usize {
        match self {
            Payload::Text(s) => s.len(),
            Payload::Binary(b) => b.len(),
            Payload::Map(m) => m.iter().map(|(k, v)| k.len() + v.len()).sum(),
            Payload::Empty => 0,
        }
    }
}

// ── Envelope ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Envelope {
    pub id: MessageId,
    pub sender: AgentId,
    pub recipient: Recipient,
    pub topic: Topic,
    pub payload: Payload,
    pub timestamp: u64,
    pub correlation: Option<CorrelationId>,
    /// Priority: higher = more urgent. Default 0.
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    Agent(AgentId),
    Broadcast,
    TopicSubscribers,
}

impl fmt::Display for Recipient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recipient::Agent(id) => write!(f, "agent:{id}"),
            Recipient::Broadcast => write!(f, "broadcast"),
            Recipient::TopicSubscribers => write!(f, "topic-subscribers"),
        }
    }
}

// ── Mailbox ────────────────────────────────────────────────────────

struct Mailbox {
    queue: VecDeque<Envelope>,
    subscriptions: Vec<Topic>,
}

impl Mailbox {
    fn new() -> Self {
        Self { queue: VecDeque::new(), subscriptions: Vec::new() }
    }
}

// ── Bus Stats ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct BusStats {
    pub total_sent: u64,
    pub total_delivered: u64,
    pub total_dropped: u64,
    pub bytes_transferred: u64,
}

// ── Message Bus ────────────────────────────────────────────────────

pub struct MessageBus {
    next_id: MessageId,
    clock: u64,
    mailboxes: HashMap<AgentId, Mailbox>,
    /// Max mailbox depth per agent. 0 = unlimited.
    pub max_mailbox_depth: usize,
    pub stats: BusStats,
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            clock: 0,
            mailboxes: HashMap::new(),
            max_mailbox_depth: 0,
            stats: BusStats::default(),
        }
    }

    /// Register an agent on the bus.
    pub fn register_agent(&mut self, agent: AgentId) {
        self.mailboxes.entry(agent).or_insert_with(Mailbox::new);
    }

    /// Subscribe an agent to a topic.
    pub fn subscribe(&mut self, agent: &str, topic: Topic) {
        if let Some(mb) = self.mailboxes.get_mut(agent) {
            if !mb.subscriptions.contains(&topic) {
                mb.subscriptions.push(topic);
            }
        }
    }

    /// Unsubscribe an agent from a topic.
    pub fn unsubscribe(&mut self, agent: &str, topic: &Topic) {
        if let Some(mb) = self.mailboxes.get_mut(agent) {
            mb.subscriptions.retain(|t| t != topic);
        }
    }

    /// Send a message. Returns the message ID assigned.
    pub fn send(
        &mut self,
        sender: AgentId,
        recipient: Recipient,
        topic: Topic,
        payload: Payload,
        correlation: Option<CorrelationId>,
        priority: u8,
    ) -> MessageId {
        let id = self.next_id;
        self.next_id += 1;
        self.clock += 1;

        let bytes = payload.byte_len() as u64;

        let envelope = Envelope {
            id,
            sender,
            recipient: recipient.clone(),
            topic: topic.clone(),
            payload,
            timestamp: self.clock,
            correlation,
            priority,
        };

        self.stats.total_sent += 1;

        match recipient {
            Recipient::Agent(ref agent_id) => {
                self.deliver(agent_id.clone(), envelope);
                self.stats.bytes_transferred += bytes;
            }
            Recipient::Broadcast => {
                let agents: Vec<AgentId> = self.mailboxes.keys().cloned().collect();
                for agent_id in agents {
                    if agent_id != envelope.sender {
                        self.deliver(agent_id, envelope.clone());
                        self.stats.bytes_transferred += bytes;
                    }
                }
            }
            Recipient::TopicSubscribers => {
                let subscribers: Vec<AgentId> = self
                    .mailboxes
                    .iter()
                    .filter(|(aid, mb)| {
                        *aid != &envelope.sender && mb.subscriptions.contains(&topic)
                    })
                    .map(|(aid, _)| aid.clone())
                    .collect();
                for agent_id in subscribers {
                    self.deliver(agent_id, envelope.clone());
                    self.stats.bytes_transferred += bytes;
                }
            }
        }

        id
    }

    /// Receive the next message for an agent (FIFO, priority-ordered).
    pub fn recv(&mut self, agent: &str) -> Option<Envelope> {
        if let Some(mb) = self.mailboxes.get_mut(agent) { mb.queue.pop_front() } else { None }
    }

    /// Peek at the next message without consuming it.
    pub fn peek(&self, agent: &str) -> Option<&Envelope> {
        self.mailboxes.get(agent).and_then(|mb| mb.queue.front())
    }

    /// Number of pending messages for an agent.
    pub fn pending_count(&self, agent: &str) -> usize {
        self.mailboxes.get(agent).map_or(0, |mb| mb.queue.len())
    }

    /// Drain all messages for an agent.
    pub fn drain(&mut self, agent: &str) -> Vec<Envelope> {
        if let Some(mb) = self.mailboxes.get_mut(agent) {
            mb.queue.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Total registered agents.
    pub fn agent_count(&self) -> usize {
        self.mailboxes.len()
    }

    /// JSON snapshot of bus state.
    pub fn to_json(&self) -> String {
        let mut agents = Vec::new();
        for (aid, mb) in &self.mailboxes {
            let subs: Vec<String> = mb.subscriptions.iter().map(|t| format!("\"{}\"", t)).collect();
            agents.push(format!(
                "{{\"agent\":\"{aid}\",\"pending\":{},\"subscriptions\":[{}]}}",
                mb.queue.len(),
                subs.join(",")
            ));
        }
        format!(
            "{{\"agents\":[{}],\"stats\":{{\"sent\":{},\"delivered\":{},\"dropped\":{},\"bytes\":{}}}}}",
            agents.join(","),
            self.stats.total_sent,
            self.stats.total_delivered,
            self.stats.total_dropped,
            self.stats.bytes_transferred
        )
    }

    // ── Internal ──────────────────────────────────────────────────

    fn deliver(&mut self, agent_id: AgentId, mut envelope: Envelope) {
        if let Some(mb) = self.mailboxes.get_mut(&agent_id) {
            if self.max_mailbox_depth > 0 && mb.queue.len() >= self.max_mailbox_depth {
                self.stats.total_dropped += 1;
                return;
            }
            // Insert by priority (higher priority first).
            let pos = mb.queue.iter().position(|e| e.priority < envelope.priority);
            match pos {
                Some(i) => mb.queue.insert(i, envelope),
                None => mb.queue.push_back(envelope),
            }
            self.stats.total_delivered += 1;
        } else {
            self.stats.total_dropped += 1;
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bus() -> MessageBus {
        let mut b = MessageBus::new();
        b.register_agent("alice".into());
        b.register_agent("bob".into());
        b.register_agent("carol".into());
        b
    }

    // ── Basic send / recv ─────────────────────────────────────────

    #[test]
    fn send_to_agent() {
        let mut b = bus();
        let id = b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        assert!(id > 0);
        let env = b.recv("bob").unwrap();
        assert_eq!(env.sender, "alice");
        assert_eq!(env.topic, Topic::Heartbeat);
    }

    #[test]
    fn recv_empty_returns_none() {
        let mut b = bus();
        assert!(b.recv("alice").is_none());
    }

    // ── Broadcast ─────────────────────────────────────────────────

    #[test]
    fn broadcast_reaches_all_except_sender() {
        let mut b = bus();
        b.send("alice".into(), Recipient::Broadcast, Topic::Heartbeat, Payload::Empty, None, 0);
        assert_eq!(b.pending_count("bob"), 1);
        assert_eq!(b.pending_count("carol"), 1);
        assert_eq!(b.pending_count("alice"), 0); // sender excluded
    }

    // ── Topic subscription ────────────────────────────────────────

    #[test]
    fn topic_subscribers_only() {
        let mut b = bus();
        b.subscribe("bob", Topic::CrdtOp);
        b.send(
            "alice".into(),
            Recipient::TopicSubscribers,
            Topic::CrdtOp,
            Payload::text("op1"),
            None,
            0,
        );
        assert_eq!(b.pending_count("bob"), 1);
        assert_eq!(b.pending_count("carol"), 0); // not subscribed
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let mut b = bus();
        b.subscribe("bob", Topic::CrdtOp);
        b.unsubscribe("bob", &Topic::CrdtOp);
        b.send("alice".into(), Recipient::TopicSubscribers, Topic::CrdtOp, Payload::Empty, None, 0);
        assert_eq!(b.pending_count("bob"), 0);
    }

    // ── Priority ordering ─────────────────────────────────────────

    #[test]
    fn higher_priority_first() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::text("low"),
            None,
            0,
        );
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Diagnostic,
            Payload::text("high"),
            None,
            10,
        );
        let first = b.recv("bob").unwrap();
        assert_eq!(first.priority, 10);
        let second = b.recv("bob").unwrap();
        assert_eq!(second.priority, 0);
    }

    // ── Mailbox depth limit ───────────────────────────────────────

    #[test]
    fn mailbox_depth_drops_excess() {
        let mut b = bus();
        b.max_mailbox_depth = 2;
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        assert_eq!(b.pending_count("bob"), 2);
        assert_eq!(b.stats.total_dropped, 1);
    }

    // ── Correlation ───────────────────────────────────────────────

    #[test]
    fn correlation_id_preserved() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::TaskAssign,
            Payload::Empty,
            Some(42),
            0,
        );
        let env = b.recv("bob").unwrap();
        assert_eq!(env.correlation, Some(42));
    }

    // ── Payload variants ──────────────────────────────────────────

    #[test]
    fn text_payload_byte_len() {
        let p = Payload::text("hello");
        assert_eq!(p.byte_len(), 5);
    }

    #[test]
    fn binary_payload() {
        let p = Payload::Binary(vec![0, 1, 2]);
        assert_eq!(p.byte_len(), 3);
    }

    #[test]
    fn map_payload() {
        let mut m = BTreeMap::new();
        m.insert("key".into(), "val".into());
        let p = Payload::Map(m);
        assert_eq!(p.byte_len(), 6); // "key" + "val"
    }

    #[test]
    fn empty_payload_zero_len() {
        assert_eq!(Payload::Empty.byte_len(), 0);
    }

    // ── Drain ─────────────────────────────────────────────────────

    #[test]
    fn drain_clears_mailbox() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        let msgs = b.drain("bob");
        assert_eq!(msgs.len(), 2);
        assert_eq!(b.pending_count("bob"), 0);
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats_tracking() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::text("hi"),
            None,
            0,
        );
        assert_eq!(b.stats.total_sent, 1);
        assert_eq!(b.stats.total_delivered, 1);
        assert_eq!(b.stats.bytes_transferred, 2);
    }

    // ── Peek ──────────────────────────────────────────────────────

    #[test]
    fn peek_does_not_consume() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("bob".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        assert!(b.peek("bob").is_some());
        assert_eq!(b.pending_count("bob"), 1); // still there
    }

    // ── JSON ──────────────────────────────────────────────────────

    #[test]
    fn json_snapshot() {
        let mut b = bus();
        b.subscribe("alice", Topic::CrdtOp);
        let json = b.to_json();
        assert!(json.contains("\"agent\":\"alice\""));
        assert!(json.contains("\"sent\":0"));
    }

    // ── Custom topic ──────────────────────────────────────────────

    #[test]
    fn custom_topic() {
        let mut b = bus();
        b.subscribe("bob", Topic::Custom("my.event".into()));
        b.send(
            "alice".into(),
            Recipient::TopicSubscribers,
            Topic::Custom("my.event".into()),
            Payload::Empty,
            None,
            0,
        );
        assert_eq!(b.pending_count("bob"), 1);
    }

    // ── Unregistered agent ────────────────────────────────────────

    #[test]
    fn send_to_unregistered_drops() {
        let mut b = bus();
        b.send(
            "alice".into(),
            Recipient::Agent("unknown".into()),
            Topic::Heartbeat,
            Payload::Empty,
            None,
            0,
        );
        assert_eq!(b.stats.total_dropped, 1);
    }
}
