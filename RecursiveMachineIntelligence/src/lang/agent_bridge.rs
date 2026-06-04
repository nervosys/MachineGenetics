//! Agent protocol integration for the RMIL VM.
//!
//! Wires RMIL agent opcodes (`SEND`, `RECV`, `SPAWN`, `PUBLISH`, `SUBSCRIBE`,
//! `DELEGATE`, `BROADCAST`) to the RecursiveMachineIntelligence agent messaging infrastructure.
//!
//! An [`AgentBridge`] sits between the VM and the message bus, translating
//! RMIL values into protocol messages and back.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::agent_bridge::{AgentBridge, AgentMailbox};
//!
//! let (bridge, mailbox) = AgentBridge::new("agent-1");
//! assert_eq!(bridge.agent_id(), "agent-1");
//! assert!(mailbox.is_empty());
//! ```

use crate::lang::expr::Val;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

// ── Agent mailbox ────────────────────────────────────────────────────────────

/// A message exchanged between RMIL agents.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentMessage {
    /// Sender identifier.
    pub sender: String,
    /// Recipient identifier.
    pub recipient: String,
    /// Topic (for pub/sub).
    pub topic: String,
    /// Payload value.
    pub payload: Val,
    /// Message kind.
    pub kind: MessageKind,
}

/// Kind of agent message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    /// Point-to-point message.
    Direct,
    /// Published to a topic.
    Publish,
    /// Broadcast to all.
    Broadcast,
    /// Delegated task.
    Delegate,
    /// Spawn request.
    Spawn,
    /// Kill request.
    Kill,
}

/// Thread-safe agent mailbox (inbox queue).
#[derive(Debug, Clone)]
pub struct AgentMailbox {
    inbox: Arc<Mutex<VecDeque<AgentMessage>>>,
}

impl AgentMailbox {
    /// Create a new empty mailbox.
    pub fn new() -> Self {
        Self {
            inbox: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Push a message into the mailbox.
    pub fn push(&self, msg: AgentMessage) {
        self.inbox.lock().unwrap().push_back(msg);
    }

    /// Pop the next message from the mailbox.
    pub fn pop(&self) -> Option<AgentMessage> {
        self.inbox.lock().unwrap().pop_front()
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<AgentMessage> {
        self.inbox.lock().unwrap().front().cloned()
    }

    /// Number of queued messages.
    pub fn len(&self) -> usize {
        self.inbox.lock().unwrap().len()
    }

    /// Whether the mailbox is empty.
    pub fn is_empty(&self) -> bool {
        self.inbox.lock().unwrap().is_empty()
    }

    /// Drain all messages.
    pub fn drain(&self) -> Vec<AgentMessage> {
        self.inbox.lock().unwrap().drain(..).collect()
    }
}

impl Default for AgentMailbox {
    fn default() -> Self {
        Self::new()
    }
}

// ── Agent bridge ─────────────────────────────────────────────────────────────

/// Bridge between RMIL VM agent opcodes and the messaging system.
///
/// Handles SEND, RECV, SPAWN, PUBLISH, SUBSCRIBE, DELEGATE, BROADCAST by
/// translating between RMIL `Val` payloads and `AgentMessage` structs.
#[derive(Debug, Clone)]
pub struct AgentBridge {
    /// This agent's identifier.
    agent_id: String,
    /// Incoming message queue.
    mailbox: AgentMailbox,
    /// Outgoing message queue (for external dispatch).
    outbox: Arc<Mutex<VecDeque<AgentMessage>>>,
    /// Active subscriptions (topic → flag).
    subscriptions: Arc<Mutex<HashMap<String, bool>>>,
    /// Spawned child agent IDs.
    spawned: Arc<Mutex<Vec<String>>>,
}

impl AgentBridge {
    /// Create a new agent bridge with the given identifier.
    ///
    /// Returns the bridge and a mailbox handle for delivering messages to it.
    pub fn new(agent_id: &str) -> (Self, AgentMailbox) {
        let mailbox = AgentMailbox::new();
        let bridge = Self {
            agent_id: agent_id.to_string(),
            mailbox: mailbox.clone(),
            outbox: Arc::new(Mutex::new(VecDeque::new())),
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            spawned: Arc::new(Mutex::new(Vec::new())),
        };
        (bridge, mailbox)
    }

    /// Agent identifier.
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    // ── SEND ─────────────────────────────────────────────────────────────

    /// Execute a SEND operation: send a value to a named recipient.
    pub fn send(&self, recipient: &str, payload: Val) {
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: recipient.to_string(),
            topic: String::new(),
            payload,
            kind: MessageKind::Direct,
        };
        self.outbox.lock().unwrap().push_back(msg);
    }

    // ── RECV ─────────────────────────────────────────────────────────────

    /// Execute a RECV operation: receive the next message from the inbox.
    ///
    /// Returns `Val::Nil` if no message is available (non-blocking).
    pub fn recv(&self) -> Val {
        match self.mailbox.pop() {
            Some(msg) => msg.payload,
            None => Val::Nil,
        }
    }

    // ── PUBLISH ──────────────────────────────────────────────────────────

    /// Execute a PUBLISH operation: publish a value to a topic.
    pub fn publish(&self, topic: &str, payload: Val) {
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: String::new(),
            topic: topic.to_string(),
            payload,
            kind: MessageKind::Publish,
        };
        self.outbox.lock().unwrap().push_back(msg);
    }

    // ── SUBSCRIBE ────────────────────────────────────────────────────────

    /// Execute a SUBSCRIBE operation: register interest in a topic.
    pub fn subscribe(&self, topic: &str) {
        self.subscriptions
            .lock()
            .unwrap()
            .insert(topic.to_string(), true);
    }

    /// Check if subscribed to a topic.
    pub fn is_subscribed(&self, topic: &str) -> bool {
        self.subscriptions
            .lock()
            .unwrap()
            .get(topic)
            .copied()
            .unwrap_or(false)
    }

    // ── BROADCAST ────────────────────────────────────────────────────────

    /// Execute a BROADCAST operation: send to all agents.
    pub fn broadcast(&self, payload: Val) {
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: "*".to_string(),
            topic: String::new(),
            payload,
            kind: MessageKind::Broadcast,
        };
        self.outbox.lock().unwrap().push_back(msg);
    }

    // ── DELEGATE ─────────────────────────────────────────────────────────

    /// Execute a DELEGATE operation: delegates a task to another agent.
    pub fn delegate(&self, target: &str, task: Val) {
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: target.to_string(),
            topic: String::new(),
            payload: task,
            kind: MessageKind::Delegate,
        };
        self.outbox.lock().unwrap().push_back(msg);
    }

    // ── SPAWN ────────────────────────────────────────────────────────────

    /// Execute a SPAWN operation: spawn a new child agent.
    ///
    /// Returns the child agent's ID.
    pub fn spawn(&self, name: &str) -> String {
        let child_id = format!("{}/{}", self.agent_id, name);
        self.spawned.lock().unwrap().push(child_id.clone());
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: child_id.clone(),
            topic: String::new(),
            payload: Val::Nil,
            kind: MessageKind::Spawn,
        };
        self.outbox.lock().unwrap().push_back(msg);
        child_id
    }

    // ── KILL ─────────────────────────────────────────────────────────────

    /// Execute a KILL operation: terminate a child agent.
    pub fn kill(&self, target: &str) {
        let msg = AgentMessage {
            sender: self.agent_id.clone(),
            recipient: target.to_string(),
            topic: String::new(),
            payload: Val::Nil,
            kind: MessageKind::Kill,
        };
        self.outbox.lock().unwrap().push_back(msg);
    }

    // ── Outbox access ────────────────────────────────────────────────────

    /// Drain all outgoing messages (for external dispatch).
    pub fn drain_outbox(&self) -> Vec<AgentMessage> {
        self.outbox.lock().unwrap().drain(..).collect()
    }

    /// Number of pending outgoing messages.
    pub fn outbox_len(&self) -> usize {
        self.outbox.lock().unwrap().len()
    }

    /// List of spawned child agent IDs.
    pub fn spawned_children(&self) -> Vec<String> {
        self.spawned.lock().unwrap().clone()
    }

    /// Active subscription topics.
    pub fn active_subscriptions(&self) -> Vec<String> {
        self.subscriptions.lock().unwrap().keys().cloned().collect()
    }
}

// ── Agent network (local multi-agent simulation) ─────────────────────────────

/// A local network of agent bridges for testing multi-agent RMIL programs.
///
/// Routes messages between bridges using agent IDs.
pub struct AgentNetwork {
    agents: HashMap<String, (AgentBridge, AgentMailbox)>,
}

impl AgentNetwork {
    /// Create a new empty network.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Add an agent to the network.
    pub fn add_agent(&mut self, id: &str) -> &AgentBridge {
        let (bridge, mailbox) = AgentBridge::new(id);
        self.agents.insert(id.to_string(), (bridge, mailbox));
        &self.agents[id].0
    }

    /// Get an agent's bridge.
    pub fn agent(&self, id: &str) -> Option<&AgentBridge> {
        self.agents.get(id).map(|(b, _)| b)
    }

    /// Get an agent's mailbox.
    pub fn mailbox(&self, id: &str) -> Option<&AgentMailbox> {
        self.agents.get(id).map(|(_, m)| m)
    }

    /// Number of agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Whether the network is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Route all pending outgoing messages to recipient mailboxes.
    ///
    /// Returns the total number of messages routed.
    pub fn route_messages(&self) -> usize {
        let mut total = 0;
        for (bridge, _) in self.agents.values() {
            let outgoing = bridge.drain_outbox();
            for msg in outgoing {
                match msg.kind {
                    MessageKind::Broadcast => {
                        // Deliver to all agents except sender
                        for (id, (_, mailbox)) in &self.agents {
                            if id != &msg.sender {
                                mailbox.push(msg.clone());
                                total += 1;
                            }
                        }
                    }
                    MessageKind::Publish => {
                        // Deliver to all agents subscribed to the topic
                        for (id, (agent_bridge, mailbox)) in &self.agents {
                            if id != &msg.sender && agent_bridge.is_subscribed(&msg.topic) {
                                mailbox.push(msg.clone());
                                total += 1;
                            }
                        }
                    }
                    _ => {
                        // Direct/delegate/spawn/kill — route to recipient
                        if let Some((_, mailbox)) = self.agents.get(&msg.recipient) {
                            mailbox.push(msg);
                            total += 1;
                        }
                    }
                }
            }
        }
        total
    }
}

impl Default for AgentNetwork {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_creation() {
        let (bridge, mailbox) = AgentBridge::new("test-agent");
        assert_eq!(bridge.agent_id(), "test-agent");
        assert!(mailbox.is_empty());
    }

    #[test]
    fn send_and_outbox() {
        let (bridge, _mailbox) = AgentBridge::new("sender");
        bridge.send("recipient", Val::I64(42));
        assert_eq!(bridge.outbox_len(), 1);

        let msgs = bridge.drain_outbox();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].sender, "sender");
        assert_eq!(msgs[0].recipient, "recipient");
        assert_eq!(msgs[0].payload, Val::I64(42));
        assert_eq!(msgs[0].kind, MessageKind::Direct);
    }

    #[test]
    fn recv_empty() {
        let (bridge, _mailbox) = AgentBridge::new("agent");
        assert_eq!(bridge.recv(), Val::Nil);
    }

    #[test]
    fn recv_with_message() {
        let (bridge, mailbox) = AgentBridge::new("agent");
        mailbox.push(AgentMessage {
            sender: "other".into(),
            recipient: "agent".into(),
            topic: String::new(),
            payload: Val::I64(99),
            kind: MessageKind::Direct,
        });
        assert_eq!(bridge.recv(), Val::I64(99));
        assert!(mailbox.is_empty());
    }

    #[test]
    fn publish_and_subscribe() {
        let (bridge, _) = AgentBridge::new("pub-agent");
        bridge.subscribe("topic/test");
        assert!(bridge.is_subscribed("topic/test"));
        assert!(!bridge.is_subscribed("other"));

        bridge.publish("topic/test", Val::Bool(true));
        let msgs = bridge.drain_outbox();
        assert_eq!(msgs[0].kind, MessageKind::Publish);
        assert_eq!(msgs[0].topic, "topic/test");
    }

    #[test]
    fn broadcast() {
        let (bridge, _) = AgentBridge::new("broadcaster");
        bridge.broadcast(Val::I64(1));
        let msgs = bridge.drain_outbox();
        assert_eq!(msgs[0].kind, MessageKind::Broadcast);
        assert_eq!(msgs[0].recipient, "*");
    }

    #[test]
    fn delegate_task() {
        let (bridge, _) = AgentBridge::new("delegator");
        bridge.delegate("worker", Val::I64(42));
        let msgs = bridge.drain_outbox();
        assert_eq!(msgs[0].kind, MessageKind::Delegate);
        assert_eq!(msgs[0].recipient, "worker");
    }

    #[test]
    fn spawn_child() {
        let (bridge, _) = AgentBridge::new("parent");
        let child_id = bridge.spawn("child-1");
        assert_eq!(child_id, "parent/child-1");

        let children = bridge.spawned_children();
        assert_eq!(children, vec!["parent/child-1"]);

        let msgs = bridge.drain_outbox();
        assert_eq!(msgs[0].kind, MessageKind::Spawn);
    }

    #[test]
    fn kill_agent() {
        let (bridge, _) = AgentBridge::new("parent");
        bridge.kill("parent/child-1");
        let msgs = bridge.drain_outbox();
        assert_eq!(msgs[0].kind, MessageKind::Kill);
        assert_eq!(msgs[0].recipient, "parent/child-1");
    }

    #[test]
    fn mailbox_operations() {
        let mb = AgentMailbox::new();
        assert!(mb.is_empty());
        assert_eq!(mb.len(), 0);

        let msg = AgentMessage {
            sender: "a".into(),
            recipient: "b".into(),
            topic: String::new(),
            payload: Val::I64(1),
            kind: MessageKind::Direct,
        };
        mb.push(msg.clone());
        assert_eq!(mb.len(), 1);
        assert!(!mb.is_empty());

        let peeked = mb.peek().unwrap();
        assert_eq!(peeked, msg);
        assert_eq!(mb.len(), 1); // peek doesn't consume

        let popped = mb.pop().unwrap();
        assert_eq!(popped, msg);
        assert!(mb.is_empty());
    }

    #[test]
    fn mailbox_drain() {
        let mb = AgentMailbox::new();
        for i in 0..5 {
            mb.push(AgentMessage {
                sender: "a".into(),
                recipient: "b".into(),
                topic: String::new(),
                payload: Val::I64(i),
                kind: MessageKind::Direct,
            });
        }
        let all = mb.drain();
        assert_eq!(all.len(), 5);
        assert!(mb.is_empty());
    }

    #[test]
    fn active_subscriptions() {
        let (bridge, _) = AgentBridge::new("sub-agent");
        bridge.subscribe("topic/a");
        bridge.subscribe("topic/b");
        let subs = bridge.active_subscriptions();
        assert_eq!(subs.len(), 2);
    }

    // ── Network tests ────────────────────────────────────────────────────

    #[test]
    fn network_creation() {
        let net = AgentNetwork::new();
        assert!(net.is_empty());
    }

    #[test]
    fn network_add_agents() {
        let mut net = AgentNetwork::new();
        net.add_agent("agent-1");
        net.add_agent("agent-2");
        assert_eq!(net.len(), 2);
        assert!(net.agent("agent-1").is_some());
        assert!(net.agent("unknown").is_none());
    }

    #[test]
    fn network_direct_routing() {
        let mut net = AgentNetwork::new();
        net.add_agent("alice");
        net.add_agent("bob");

        // Alice sends to Bob
        net.agent("alice").unwrap().send("bob", Val::I64(42));
        let routed = net.route_messages();
        assert_eq!(routed, 1);

        // Bob should have the message
        let mb = net.mailbox("bob").unwrap();
        assert_eq!(mb.len(), 1);
        let msg = mb.pop().unwrap();
        assert_eq!(msg.payload, Val::I64(42));
        assert_eq!(msg.sender, "alice");
    }

    #[test]
    fn network_broadcast_routing() {
        let mut net = AgentNetwork::new();
        net.add_agent("announcer");
        net.add_agent("listener-1");
        net.add_agent("listener-2");

        net.agent("announcer").unwrap().broadcast(Val::Bool(true));
        let routed = net.route_messages();
        assert_eq!(routed, 2); // not to self

        assert_eq!(net.mailbox("listener-1").unwrap().len(), 1);
        assert_eq!(net.mailbox("listener-2").unwrap().len(), 1);
        assert!(net.mailbox("announcer").unwrap().is_empty());
    }

    #[test]
    fn network_pubsub_routing() {
        let mut net = AgentNetwork::new();
        net.add_agent("publisher");
        net.add_agent("sub-a");
        net.add_agent("sub-b");
        net.add_agent("no-sub");

        // Subscribe two agents
        net.agent("sub-a").unwrap().subscribe("news");
        net.agent("sub-b").unwrap().subscribe("news");

        // Publish
        net.agent("publisher").unwrap().publish("news", Val::I64(1));
        let routed = net.route_messages();
        assert_eq!(routed, 2); // only subscribed agents

        assert_eq!(net.mailbox("sub-a").unwrap().len(), 1);
        assert_eq!(net.mailbox("sub-b").unwrap().len(), 1);
        assert!(net.mailbox("no-sub").unwrap().is_empty());
    }

    #[test]
    fn network_delegate_routing() {
        let mut net = AgentNetwork::new();
        net.add_agent("boss");
        net.add_agent("worker");

        net.agent("boss").unwrap().delegate("worker", Val::I64(100));
        let routed = net.route_messages();
        assert_eq!(routed, 1);

        let msg = net.mailbox("worker").unwrap().pop().unwrap();
        assert_eq!(msg.kind, MessageKind::Delegate);
        assert_eq!(msg.payload, Val::I64(100));
    }

    #[test]
    fn mailbox_default() {
        let mb = AgentMailbox::default();
        assert!(mb.is_empty());
    }

    #[test]
    fn network_default() {
        let net = AgentNetwork::default();
        assert!(net.is_empty());
    }
}
