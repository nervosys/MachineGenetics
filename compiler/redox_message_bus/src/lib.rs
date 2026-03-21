// Swarm message bus with zero-copy FlatBuffers-inspired serialization.
// (REDOX_PROPOSAL.md §7.6, P44)
//
// Supports three transport layers:
// - SharedMemory: <100ns latency (in-process)
// - UnixSocket: ~1μs latency (local IPC)
// - TcpTls: network transport
//
// Wire protocol: frame header (magic, version, length, CRC-32C),
// routing header (sender, receiver, channel), message payload.

use std::collections::{BTreeMap, VecDeque};

// ── Wire Protocol ──────────────────────────────────────────────────────────

/// Magic bytes for the wire protocol.
pub const FRAME_MAGIC: [u8; 4] = [0x52, 0x44, 0x58, 0x42]; // "RDXB"

/// Wire protocol version.
pub const WIRE_VERSION: u8 = 1;

/// Frame header (fixed 16 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// Magic bytes (4): "RDXB"
    pub magic: [u8; 4],
    /// Protocol version (1 byte)
    pub version: u8,
    /// Flags (1 byte): bit 0 = compressed, bit 1 = encrypted
    pub flags: u8,
    /// Reserved (2 bytes)
    pub reserved: u16,
    /// Payload length in bytes (4 bytes, little-endian)
    pub payload_len: u32,
    /// CRC-32C of the entire frame (header + routing + payload)
    pub crc32c: u32,
}

impl FrameHeader {
    /// Size of the frame header in bytes.
    pub const SIZE: usize = 16;

    pub fn new(payload_len: u32, flags: u8) -> Self {
        FrameHeader {
            magic: FRAME_MAGIC,
            version: WIRE_VERSION,
            flags,
            reserved: 0,
            payload_len,
            crc32c: 0, // computed after serialization
        }
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4] = self.version;
        buf[5] = self.flags;
        buf[6..8].copy_from_slice(&self.reserved.to_le_bytes());
        buf[8..12].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[12..16].copy_from_slice(&self.crc32c.to_le_bytes());
        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(bytes: &[u8; 16]) -> Result<Self, WireError> {
        if bytes[0..4] != FRAME_MAGIC {
            return Err(WireError::BadMagic);
        }
        let version = bytes[4];
        if version != WIRE_VERSION {
            return Err(WireError::UnsupportedVersion(version));
        }
        Ok(FrameHeader {
            magic: FRAME_MAGIC,
            version,
            flags: bytes[5],
            reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
            payload_len: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            crc32c: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        })
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & 0x01 != 0
    }

    pub fn is_encrypted(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

/// Routing header (variable length, follows frame header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingHeader {
    /// Sender agent ID.
    pub sender: String,
    /// Receiver agent ID (empty = broadcast).
    pub receiver: String,
    /// Channel/topic name.
    pub channel: String,
    /// Message sequence number for ordering.
    pub sequence: u64,
    /// Correlation ID for request-response pairing.
    pub correlation_id: u64,
}

impl RoutingHeader {
    pub fn new(sender: &str, receiver: &str, channel: &str) -> Self {
        RoutingHeader {
            sender: sender.to_string(),
            receiver: receiver.to_string(),
            channel: channel.to_string(),
            sequence: 0,
            correlation_id: 0,
        }
    }

    pub fn with_sequence(mut self, seq: u64) -> Self {
        self.sequence = seq;
        self
    }

    pub fn with_correlation(mut self, id: u64) -> Self {
        self.correlation_id = id;
        self
    }

    pub fn is_broadcast(&self) -> bool {
        self.receiver.is_empty()
    }

    /// Serialize to bytes (length-prefixed strings).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        write_str(&mut buf, &self.sender);
        write_str(&mut buf, &self.receiver);
        write_str(&mut buf, &self.channel);
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.correlation_id.to_le_bytes());
        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<(Self, usize), WireError> {
        let mut pos = 0;
        let (sender, n) = read_str(data, pos)?;
        pos += n;
        let (receiver, n) = read_str(data, pos)?;
        pos += n;
        let (channel, n) = read_str(data, pos)?;
        pos += n;
        if pos + 16 > data.len() {
            return Err(WireError::Truncated);
        }
        let sequence = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        let correlation_id = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
        pos += 8;
        Ok((RoutingHeader { sender, receiver, channel, sequence, correlation_id }, pos))
    }
}

// ── Wire Helpers ───────────────────────────────────────────────────────────

fn write_str(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(bytes);
}

fn read_str(data: &[u8], pos: usize) -> Result<(String, usize), WireError> {
    if pos + 2 > data.len() {
        return Err(WireError::Truncated);
    }
    let len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
    if pos + 2 + len > data.len() {
        return Err(WireError::Truncated);
    }
    let s = std::str::from_utf8(&data[pos + 2..pos + 2 + len])
        .map_err(|_| WireError::InvalidUtf8)?;
    Ok((s.to_string(), 2 + len))
}

/// CRC-32C (Castagnoli) — simple software implementation.
pub fn crc32c(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0x82F6_3B78;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Wire protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    BadMagic,
    UnsupportedVersion(u8),
    Truncated,
    InvalidUtf8,
    CrcMismatch { expected: u32, actual: u32 },
    PayloadTooLarge(u32),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::BadMagic => write!(f, "bad frame magic"),
            WireError::UnsupportedVersion(v) => write!(f, "unsupported version: {v}"),
            WireError::Truncated => write!(f, "truncated frame"),
            WireError::InvalidUtf8 => write!(f, "invalid UTF-8"),
            WireError::CrcMismatch { expected, actual } =>
                write!(f, "CRC mismatch: expected {expected:#010x}, got {actual:#010x}"),
            WireError::PayloadTooLarge(n) => write!(f, "payload too large: {n} bytes"),
        }
    }
}

// ── Frame Serialization ────────────────────────────────────────────────────

/// Maximum payload size (16 MiB).
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;

/// A complete wire frame.
#[derive(Debug, Clone)]
pub struct Frame {
    pub header: FrameHeader,
    pub routing: RoutingHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Serialize a frame to bytes with CRC-32C.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WireError> {
        let routing_bytes = self.routing.to_bytes();
        let total_payload = routing_bytes.len() + self.payload.len();
        if total_payload > MAX_PAYLOAD_SIZE as usize {
            return Err(WireError::PayloadTooLarge(total_payload as u32));
        }

        let mut header = self.header.clone();
        header.payload_len = total_payload as u32;
        header.crc32c = 0; // placeholder

        let header_bytes = header.to_bytes();
        let mut buf = Vec::with_capacity(FrameHeader::SIZE + total_payload);
        buf.extend_from_slice(&header_bytes);
        buf.extend_from_slice(&routing_bytes);
        buf.extend_from_slice(&self.payload);

        // Compute CRC over everything (with crc32c field zeroed)
        let crc = crc32c(&buf);
        // Write CRC back into header position (bytes 12..16)
        buf[12..16].copy_from_slice(&crc.to_le_bytes());

        Ok(buf)
    }

    /// Deserialize a frame from bytes, verifying CRC-32C.
    pub fn from_bytes(data: &[u8]) -> Result<Self, WireError> {
        if data.len() < FrameHeader::SIZE {
            return Err(WireError::Truncated);
        }

        let header_bytes: [u8; 16] = data[0..16].try_into().unwrap();
        let header = FrameHeader::from_bytes(&header_bytes)?;

        let total = FrameHeader::SIZE + header.payload_len as usize;
        if data.len() < total {
            return Err(WireError::Truncated);
        }

        // Verify CRC: zero out the CRC field, compute, compare
        let mut check_buf = data[..total].to_vec();
        check_buf[12..16].copy_from_slice(&[0, 0, 0, 0]);
        let computed_crc = crc32c(&check_buf);
        if computed_crc != header.crc32c {
            return Err(WireError::CrcMismatch {
                expected: header.crc32c,
                actual: computed_crc,
            });
        }

        let payload_data = &data[FrameHeader::SIZE..total];
        let (routing, routing_size) = RoutingHeader::from_bytes(payload_data)?;
        let payload = payload_data[routing_size..].to_vec();

        Ok(Frame { header, routing, payload })
    }

    /// Create a new frame from routing + payload.
    pub fn new(routing: RoutingHeader, payload: Vec<u8>) -> Self {
        Frame {
            header: FrameHeader::new(0, 0),
            routing,
            payload,
        }
    }
}

// ── Transport Layer ────────────────────────────────────────────────────────

/// Transport layer kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// In-process shared memory (<100ns).
    SharedMemory,
    /// Unix domain socket (~1μs).
    UnixSocket,
    /// TCP with optional TLS (network).
    TcpTls,
}

impl Transport {
    pub fn name(&self) -> &str {
        match self {
            Transport::SharedMemory => "shared-memory",
            Transport::UnixSocket => "unix-socket",
            Transport::TcpTls => "tcp-tls",
        }
    }

    /// Estimated one-way latency in nanoseconds.
    pub fn estimated_latency_ns(&self) -> u64 {
        match self {
            Transport::SharedMemory => 50,
            Transport::UnixSocket => 1_000,
            Transport::TcpTls => 100_000,
        }
    }
}

impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ── Message Types ──────────────────────────────────────────────────────────

/// Message types on the swarm bus (proposal §7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageKind {
    // Coordination
    TaskAssignment,
    TaskCompleted,
    TaskFailed,
    // Consensus
    ProposeChange,
    VoteOnChange,
    ChangeAccepted,
    // Query
    QueryRequest,
    QueryResponse,
    // Conflict
    ConflictDetected,
    ConflictResolution,
    // Knowledge
    DiscoveredPattern,
    SharedInsight,
    // Health
    Heartbeat,
    // Lease
    LeaseRequest,
    LeaseGranted,
    LeaseRevoked,
}

impl MessageKind {
    pub fn tag(&self) -> u8 {
        match self {
            MessageKind::TaskAssignment => 1,
            MessageKind::TaskCompleted => 2,
            MessageKind::TaskFailed => 3,
            MessageKind::ProposeChange => 10,
            MessageKind::VoteOnChange => 11,
            MessageKind::ChangeAccepted => 12,
            MessageKind::QueryRequest => 20,
            MessageKind::QueryResponse => 21,
            MessageKind::ConflictDetected => 30,
            MessageKind::ConflictResolution => 31,
            MessageKind::DiscoveredPattern => 40,
            MessageKind::SharedInsight => 41,
            MessageKind::Heartbeat => 50,
            MessageKind::LeaseRequest => 60,
            MessageKind::LeaseGranted => 61,
            MessageKind::LeaseRevoked => 62,
        }
    }

    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(MessageKind::TaskAssignment),
            2 => Some(MessageKind::TaskCompleted),
            3 => Some(MessageKind::TaskFailed),
            10 => Some(MessageKind::ProposeChange),
            11 => Some(MessageKind::VoteOnChange),
            12 => Some(MessageKind::ChangeAccepted),
            20 => Some(MessageKind::QueryRequest),
            21 => Some(MessageKind::QueryResponse),
            30 => Some(MessageKind::ConflictDetected),
            31 => Some(MessageKind::ConflictResolution),
            40 => Some(MessageKind::DiscoveredPattern),
            41 => Some(MessageKind::SharedInsight),
            50 => Some(MessageKind::Heartbeat),
            60 => Some(MessageKind::LeaseRequest),
            61 => Some(MessageKind::LeaseGranted),
            62 => Some(MessageKind::LeaseRevoked),
            _ => None,
        }
    }
}

// ── Message Bus ────────────────────────────────────────────────────────────

/// An envelope on the bus (routing + typed payload).
#[derive(Debug, Clone)]
pub struct Envelope {
    pub routing: RoutingHeader,
    pub kind: MessageKind,
    pub payload: Vec<u8>,
}

/// The swarm message bus.
pub struct MessageBus {
    /// Per-agent inboxes.
    inboxes: BTreeMap<String, VecDeque<Envelope>>,
    /// Per-channel subscriber lists.
    channels: BTreeMap<String, Vec<String>>,
    /// Transport preference.
    transport: Transport,
    /// Sequence counter per sender.
    sequences: BTreeMap<String, u64>,
    /// Total messages sent.
    total_sent: u64,
    /// Total messages delivered.
    total_delivered: u64,
}

impl MessageBus {
    pub fn new(transport: Transport) -> Self {
        MessageBus {
            inboxes: BTreeMap::new(),
            channels: BTreeMap::new(),
            transport,
            sequences: BTreeMap::new(),
            total_sent: 0,
            total_delivered: 0,
        }
    }

    /// Register an agent on the bus.
    pub fn register_agent(&mut self, agent_id: &str) {
        self.inboxes.entry(agent_id.to_string()).or_default();
    }

    /// Subscribe an agent to a channel.
    pub fn subscribe(&mut self, agent_id: &str, channel: &str) {
        self.channels.entry(channel.to_string())
            .or_default()
            .push(agent_id.to_string());
    }

    /// Unsubscribe an agent from a channel.
    pub fn unsubscribe(&mut self, agent_id: &str, channel: &str) {
        if let Some(subs) = self.channels.get_mut(channel) {
            subs.retain(|s| s != agent_id);
        }
    }

    /// Send a direct message to a specific agent.
    pub fn send(
        &mut self,
        sender: &str,
        receiver: &str,
        kind: MessageKind,
        payload: Vec<u8>,
    ) -> u64 {
        let seq = self.next_sequence(sender);
        let routing = RoutingHeader::new(sender, receiver, "")
            .with_sequence(seq);
        let envelope = Envelope { routing, kind, payload };

        self.inboxes.entry(receiver.to_string())
            .or_default()
            .push_back(envelope);
        self.total_sent += 1;
        self.total_delivered += 1;
        seq
    }

    /// Publish a message to all subscribers of a channel.
    pub fn publish(
        &mut self,
        sender: &str,
        channel: &str,
        kind: MessageKind,
        payload: Vec<u8>,
    ) -> u64 {
        let seq = self.next_sequence(sender);
        let subscribers: Vec<String> = self.channels.get(channel)
            .cloned()
            .unwrap_or_default();

        for sub in &subscribers {
            if sub != sender {
                let routing = RoutingHeader::new(sender, sub, channel)
                    .with_sequence(seq);
                let envelope = Envelope {
                    routing,
                    kind: kind.clone(),
                    payload: payload.clone(),
                };
                self.inboxes.entry(sub.to_string())
                    .or_default()
                    .push_back(envelope);
                self.total_delivered += 1;
            }
        }
        self.total_sent += 1;
        seq
    }

    /// Broadcast to all registered agents.
    pub fn broadcast(
        &mut self,
        sender: &str,
        kind: MessageKind,
        payload: Vec<u8>,
    ) -> u64 {
        let seq = self.next_sequence(sender);
        let agents: Vec<String> = self.inboxes.keys().cloned().collect();

        for agent in &agents {
            if agent != sender {
                let routing = RoutingHeader::new(sender, agent, "*")
                    .with_sequence(seq);
                let envelope = Envelope {
                    routing,
                    kind: kind.clone(),
                    payload: payload.clone(),
                };
                self.inboxes.get_mut(agent).unwrap().push_back(envelope);
                self.total_delivered += 1;
            }
        }
        self.total_sent += 1;
        seq
    }

    /// Receive the next message for an agent (FIFO).
    pub fn receive(&mut self, agent_id: &str) -> Option<Envelope> {
        self.inboxes.get_mut(agent_id)?.pop_front()
    }

    /// Peek at the next message without consuming it.
    pub fn peek(&self, agent_id: &str) -> Option<&Envelope> {
        self.inboxes.get(agent_id)?.front()
    }

    /// Get the number of pending messages for an agent.
    pub fn pending_count(&self, agent_id: &str) -> usize {
        self.inboxes.get(agent_id).map(|q| q.len()).unwrap_or(0)
    }

    /// Drain all messages for an agent.
    pub fn drain(&mut self, agent_id: &str) -> Vec<Envelope> {
        self.inboxes.get_mut(agent_id)
            .map(|q| q.drain(..).collect())
            .unwrap_or_default()
    }

    /// Get total messages sent.
    pub fn total_sent(&self) -> u64 {
        self.total_sent
    }

    /// Get total messages delivered.
    pub fn total_delivered(&self) -> u64 {
        self.total_delivered
    }

    /// Get the transport.
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// Get agent count.
    pub fn agent_count(&self) -> usize {
        self.inboxes.len()
    }

    /// Get subscriber count for a channel.
    pub fn channel_subscriber_count(&self, channel: &str) -> usize {
        self.channels.get(channel).map(|v| v.len()).unwrap_or(0)
    }

    fn next_sequence(&mut self, sender: &str) -> u64 {
        let seq = self.sequences.entry(sender.to_string()).or_insert(0);
        *seq += 1;
        *seq
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Wire protocol tests ──

    #[test]
    fn test_frame_header_roundtrip() {
        let h = FrameHeader::new(1024, 0x01);
        let bytes = h.to_bytes();
        let h2 = FrameHeader::from_bytes(&bytes).unwrap();
        assert_eq!(h2.payload_len, 1024);
        assert_eq!(h2.flags, 0x01);
        assert!(h2.is_compressed());
        assert!(!h2.is_encrypted());
    }

    #[test]
    fn test_frame_header_bad_magic() {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&[0, 0, 0, 0]);
        let err = FrameHeader::from_bytes(&bytes).unwrap_err();
        assert_eq!(err, WireError::BadMagic);
    }

    #[test]
    fn test_frame_header_bad_version() {
        let mut bytes = FrameHeader::new(0, 0).to_bytes();
        bytes[4] = 99; // bad version
        let err = FrameHeader::from_bytes(&bytes).unwrap_err();
        assert_eq!(err, WireError::UnsupportedVersion(99));
    }

    #[test]
    fn test_routing_header_roundtrip() {
        let r = RoutingHeader::new("alice", "bob", "tasks")
            .with_sequence(42)
            .with_correlation(100);
        let bytes = r.to_bytes();
        let (r2, size) = RoutingHeader::from_bytes(&bytes).unwrap();
        assert_eq!(r2.sender, "alice");
        assert_eq!(r2.receiver, "bob");
        assert_eq!(r2.channel, "tasks");
        assert_eq!(r2.sequence, 42);
        assert_eq!(r2.correlation_id, 100);
        assert_eq!(size, bytes.len());
    }

    #[test]
    fn test_routing_broadcast() {
        let r = RoutingHeader::new("alice", "", "tasks");
        assert!(r.is_broadcast());
    }

    #[test]
    fn test_crc32c_basic() {
        let c = crc32c(b"hello");
        assert_ne!(c, 0);
        // Same input → same CRC
        assert_eq!(c, crc32c(b"hello"));
        // Different input → different CRC
        assert_ne!(c, crc32c(b"world"));
    }

    #[test]
    fn test_crc32c_empty() {
        let c = crc32c(b"");
        assert_eq!(c, 0);
    }

    #[test]
    fn test_frame_roundtrip() {
        let routing = RoutingHeader::new("agent-a", "agent-b", "work");
        let payload = b"some data payload".to_vec();
        let frame = Frame::new(routing, payload);
        let bytes = frame.to_bytes().unwrap();
        let frame2 = Frame::from_bytes(&bytes).unwrap();
        assert_eq!(frame2.routing.sender, "agent-a");
        assert_eq!(frame2.routing.receiver, "agent-b");
        assert_eq!(frame2.payload, b"some data payload");
    }

    #[test]
    fn test_frame_crc_integrity() {
        let frame = Frame::new(
            RoutingHeader::new("a", "b", "c"),
            b"data".to_vec(),
        );
        let mut bytes = frame.to_bytes().unwrap();
        // Corrupt one payload byte
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let err = Frame::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, WireError::CrcMismatch { .. }));
    }

    #[test]
    fn test_frame_truncated() {
        let err = Frame::from_bytes(&[0u8; 4]).unwrap_err();
        assert_eq!(err, WireError::Truncated);
    }

    #[test]
    fn test_frame_empty_payload() {
        let frame = Frame::new(
            RoutingHeader::new("x", "y", "z"),
            Vec::new(),
        );
        let bytes = frame.to_bytes().unwrap();
        let frame2 = Frame::from_bytes(&bytes).unwrap();
        assert!(frame2.payload.is_empty());
    }

    // ── Transport tests ──

    #[test]
    fn test_transport_names() {
        assert_eq!(Transport::SharedMemory.name(), "shared-memory");
        assert_eq!(Transport::UnixSocket.name(), "unix-socket");
        assert_eq!(Transport::TcpTls.name(), "tcp-tls");
    }

    #[test]
    fn test_transport_latency_ordering() {
        assert!(Transport::SharedMemory.estimated_latency_ns() < Transport::UnixSocket.estimated_latency_ns());
        assert!(Transport::UnixSocket.estimated_latency_ns() < Transport::TcpTls.estimated_latency_ns());
    }

    // ── MessageKind tests ──

    #[test]
    fn test_message_kind_roundtrip() {
        let kinds = [
            MessageKind::TaskAssignment, MessageKind::TaskCompleted, MessageKind::TaskFailed,
            MessageKind::ProposeChange, MessageKind::VoteOnChange, MessageKind::ChangeAccepted,
            MessageKind::QueryRequest, MessageKind::QueryResponse,
            MessageKind::ConflictDetected, MessageKind::ConflictResolution,
            MessageKind::DiscoveredPattern, MessageKind::SharedInsight,
            MessageKind::Heartbeat,
            MessageKind::LeaseRequest, MessageKind::LeaseGranted, MessageKind::LeaseRevoked,
        ];
        for kind in &kinds {
            let tag = kind.tag();
            let restored = MessageKind::from_tag(tag).unwrap();
            assert_eq!(&restored, kind);
        }
    }

    #[test]
    fn test_message_kind_unknown_tag() {
        assert!(MessageKind::from_tag(255).is_none());
    }

    // ── Message bus tests ──

    #[test]
    fn test_bus_register_agent() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("alice");
        bus.register_agent("bob");
        assert_eq!(bus.agent_count(), 2);
    }

    #[test]
    fn test_bus_send_receive() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("alice");
        bus.register_agent("bob");
        bus.send("alice", "bob", MessageKind::Heartbeat, vec![1, 2, 3]);
        assert_eq!(bus.pending_count("bob"), 1);
        let msg = bus.receive("bob").unwrap();
        assert_eq!(msg.routing.sender, "alice");
        assert_eq!(msg.kind, MessageKind::Heartbeat);
        assert_eq!(msg.payload, vec![1, 2, 3]);
        assert_eq!(bus.pending_count("bob"), 0);
    }

    #[test]
    fn test_bus_fifo_ordering() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        bus.send("a", "b", MessageKind::TaskAssignment, vec![1]);
        bus.send("a", "b", MessageKind::TaskCompleted, vec![2]);
        let m1 = bus.receive("b").unwrap();
        let m2 = bus.receive("b").unwrap();
        assert_eq!(m1.kind, MessageKind::TaskAssignment);
        assert_eq!(m2.kind, MessageKind::TaskCompleted);
    }

    #[test]
    fn test_bus_publish_subscribe() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("publisher");
        bus.register_agent("sub1");
        bus.register_agent("sub2");
        bus.subscribe("sub1", "events");
        bus.subscribe("sub2", "events");
        bus.publish("publisher", "events", MessageKind::DiscoveredPattern, vec![]);
        assert_eq!(bus.pending_count("sub1"), 1);
        assert_eq!(bus.pending_count("sub2"), 1);
        assert_eq!(bus.pending_count("publisher"), 0); // sender excluded
    }

    #[test]
    fn test_bus_unsubscribe() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("pub");
        bus.register_agent("sub");
        bus.subscribe("sub", "ch");
        bus.unsubscribe("sub", "ch");
        bus.publish("pub", "ch", MessageKind::Heartbeat, vec![]);
        assert_eq!(bus.pending_count("sub"), 0);
    }

    #[test]
    fn test_bus_broadcast() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("sender");
        bus.register_agent("a");
        bus.register_agent("b");
        bus.register_agent("c");
        bus.broadcast("sender", MessageKind::Heartbeat, vec![]);
        assert_eq!(bus.pending_count("a"), 1);
        assert_eq!(bus.pending_count("b"), 1);
        assert_eq!(bus.pending_count("c"), 1);
        assert_eq!(bus.pending_count("sender"), 0);
    }

    #[test]
    fn test_bus_peek() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        let peeked = bus.peek("b").unwrap();
        assert_eq!(peeked.kind, MessageKind::Heartbeat);
        assert_eq!(bus.pending_count("b"), 1); // still there
    }

    #[test]
    fn test_bus_drain() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        bus.send("a", "b", MessageKind::TaskAssignment, vec![]);
        let drained = bus.drain("b");
        assert_eq!(drained.len(), 2);
        assert_eq!(bus.pending_count("b"), 0);
    }

    #[test]
    fn test_bus_sequence_numbers() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        let s1 = bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        let s2 = bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
    }

    #[test]
    fn test_bus_stats() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        bus.send("a", "b", MessageKind::Heartbeat, vec![]);
        assert_eq!(bus.total_sent(), 2);
        assert_eq!(bus.total_delivered(), 2);
    }

    #[test]
    fn test_bus_channel_subscriber_count() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        bus.register_agent("b");
        bus.subscribe("a", "ch");
        bus.subscribe("b", "ch");
        assert_eq!(bus.channel_subscriber_count("ch"), 2);
        assert_eq!(bus.channel_subscriber_count("other"), 0);
    }

    #[test]
    fn test_bus_receive_empty() {
        let mut bus = MessageBus::new(Transport::SharedMemory);
        bus.register_agent("a");
        assert!(bus.receive("a").is_none());
    }

    #[test]
    fn test_wire_error_display() {
        let e = WireError::CrcMismatch { expected: 0x1234, actual: 0x5678 };
        assert!(e.to_string().contains("CRC mismatch"));
    }

    #[test]
    fn test_transport_display() {
        assert_eq!(Transport::SharedMemory.to_string(), "shared-memory");
    }
}
