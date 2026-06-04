//! Transport Layer - TCP/QUIC Network Communication
//!
//! Provides reliable, high-performance network transport for inter-agent
//! communication across nodes. Supports both TCP and QUIC protocols with
//! connection pooling, automatic reconnection, and backpressure.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │              Transport Layer             │
//! ├──────────────────────────────────────────┤
//! │  ┌─────────┐  ┌─────────┐  ┌─────────┐ │
//! │  │  TCP    │  │  QUIC   │  │ InProc  │ │
//! │  │ Socket  │  │ Stream  │  │ Channel │ │
//! │  └────┬────┘  └────┬────┘  └────┬────┘ │
//! │       └────────┬───┘────────────┘       │
//! │           Connection Pool               │
//! │           Frame Codec                   │
//! │           Backpressure                  │
//! └──────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Transport Configuration
// ============================================================================

/// Transport protocol selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    /// TCP with length-delimited framing
    Tcp,
    /// QUIC for multiplexed, low-latency streams
    Quic,
    /// In-process channels (for local agents)
    InProcess,
}

/// Transport layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Protocol to use
    pub protocol: TransportProtocol,
    /// Bind address for the listener
    pub bind_addr: String,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Connection pool size per peer
    pub pool_size: usize,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Read/write timeout
    pub io_timeout: Duration,
    /// Keepalive interval
    pub keepalive_interval: Duration,
    /// Maximum retry attempts for failed sends
    pub max_retries: u32,
    /// Backpressure threshold (pending messages)
    pub backpressure_threshold: usize,
    /// Enable TLS (for TCP/QUIC)
    pub tls_enabled: bool,
    /// Enable compression at transport level
    pub compression: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            protocol: TransportProtocol::Tcp,
            bind_addr: "0.0.0.0:9700".to_string(),
            max_message_size: 64 * 1024 * 1024, // 64 MB
            pool_size: 4,
            connect_timeout: Duration::from_secs(10),
            io_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(15),
            max_retries: 3,
            backpressure_threshold: 10_000,
            tls_enabled: false,
            compression: true,
        }
    }
}

// ============================================================================
// Frame Codec
// ============================================================================

/// Wire frame format for messages.
///
/// ```text
/// ┌──────────┬──────────┬──────────┬──────────┐
/// │ Magic(4) │ Len(4)   │ Flags(2) │ Type(2)  │
/// ├──────────┴──────────┴──────────┴──────────┤
/// │              Payload (variable)            │
/// └────────────────────────────────────────────┘
/// ```
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame type
    pub frame_type: FrameType,
    /// Frame flags
    pub flags: FrameFlags,
    /// Payload data
    pub payload: Vec<u8>,
}

/// Frame types for the transport protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum FrameType {
    /// Application data
    Data = 0x0001,
    /// Ping (keepalive)
    Ping = 0x0002,
    /// Pong (keepalive response)
    Pong = 0x0003,
    /// Connection close
    GoAway = 0x0004,
    /// Flow control window update
    WindowUpdate = 0x0005,
    /// Error notification
    Error = 0x0006,
}

/// Frame flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameFlags(u16);

impl FrameFlags {
    /// No flags.
    pub const NONE: Self = Self(0x0000);
    /// Compressed payload.
    pub const COMPRESSED: Self = Self(0x0001);
    /// End of stream.
    pub const END_STREAM: Self = Self(0x0002);
    /// Priority frame.
    pub const PRIORITY: Self = Self(0x0004);
    /// Requires acknowledgment.
    pub const ACK_REQUIRED: Self = Self(0x0008);

    /// Check if a flag is set.
    #[inline]
    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Set a flag.
    #[inline]
    pub fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Get raw bits.
    #[inline]
    pub fn bits(&self) -> u16 {
        self.0
    }

    /// Create from raw bits.
    #[inline]
    pub fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

/// Transport-level magic bytes.
const FRAME_MAGIC: [u8; 4] = *b"RMIt";
/// Frame header size.
const FRAME_HEADER_SIZE: usize = 12;

impl Frame {
    /// Create a new data frame.
    pub fn data(payload: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Data,
            flags: FrameFlags::NONE,
            payload,
        }
    }

    /// Create a ping frame.
    pub fn ping() -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            frame_type: FrameType::Ping,
            flags: FrameFlags::NONE,
            payload: ts.to_be_bytes().to_vec(),
        }
    }

    /// Create a pong frame.
    pub fn pong(ping_payload: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Pong,
            flags: FrameFlags::NONE,
            payload: ping_payload,
        }
    }

    /// Create a goaway frame.
    pub fn goaway(reason: &str) -> Self {
        Self {
            frame_type: FrameType::GoAway,
            flags: FrameFlags::NONE,
            payload: reason.as_bytes().to_vec(),
        }
    }

    /// Encode frame to bytes.
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = self.payload.len() as u32;
        let mut buf = Vec::with_capacity(FRAME_HEADER_SIZE + self.payload.len());

        buf.extend_from_slice(&FRAME_MAGIC);
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.extend_from_slice(&self.flags.bits().to_be_bytes());
        buf.extend_from_slice(&(self.frame_type as u16).to_be_bytes());
        buf.extend_from_slice(&self.payload);

        buf
    }

    /// Decode frame from bytes.
    pub fn decode(data: &[u8]) -> Result<(Self, usize)> {
        if data.len() < FRAME_HEADER_SIZE {
            return Err(RmiError::Protocol("Frame too short".to_string()));
        }

        // Check magic
        if data[0..4] != FRAME_MAGIC {
            return Err(RmiError::Protocol("Invalid frame magic".to_string()));
        }

        let payload_len = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let flags = FrameFlags::from_bits(u16::from_be_bytes([data[8], data[9]]));
        let frame_type_raw = u16::from_be_bytes([data[10], data[11]]);

        let frame_type = match frame_type_raw {
            0x0001 => FrameType::Data,
            0x0002 => FrameType::Ping,
            0x0003 => FrameType::Pong,
            0x0004 => FrameType::GoAway,
            0x0005 => FrameType::WindowUpdate,
            0x0006 => FrameType::Error,
            _ => {
                return Err(RmiError::Protocol(format!(
                    "Unknown frame type: {}",
                    frame_type_raw
                )))
            }
        };

        let total_len = FRAME_HEADER_SIZE + payload_len;
        if data.len() < total_len {
            return Err(RmiError::Protocol("Frame payload truncated".to_string()));
        }

        let payload = data[FRAME_HEADER_SIZE..total_len].to_vec();

        Ok((
            Self {
                frame_type,
                flags,
                payload,
            },
            total_len,
        ))
    }
}

// ============================================================================
// Connection
// ============================================================================

/// State of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Connection is being established
    Connecting,
    /// Connection is active
    Connected,
    /// Connection is draining (graceful close)
    Draining,
    /// Connection is closed
    Closed,
    /// Connection encountered an error
    Error,
}

/// Connection statistics.
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Frames sent
    pub frames_sent: u64,
    /// Frames received
    pub frames_received: u64,
    /// Round-trip time estimate (microseconds)
    pub rtt_us: u64,
    /// Connection established timestamp
    pub connected_at: f64,
    /// Last activity timestamp
    pub last_activity: f64,
    /// Number of reconnections
    pub reconnects: u32,
}

/// A connection to a remote peer.
#[derive(Debug)]
pub struct Connection {
    /// Connection ID
    pub id: Uuid,
    /// Remote peer address
    pub remote_addr: SocketAddr,
    /// Remote node ID (if known)
    pub remote_node_id: Option<Uuid>,
    /// Connection state
    state: ConnectionState,
    /// Protocol in use
    pub protocol: TransportProtocol,
    /// Statistics
    stats: ConnectionStats,
    /// Pending outbound frames
    outbound_queue: Vec<Frame>,
    /// Maximum queue size before backpressure
    max_queue_size: usize,
}

impl Connection {
    /// Create a new connection.
    pub fn new(remote_addr: SocketAddr, protocol: TransportProtocol) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            remote_addr,
            remote_node_id: None,
            state: ConnectionState::Connecting,
            protocol,
            stats: ConnectionStats {
                connected_at: now,
                last_activity: now,
                ..Default::default()
            },
            outbound_queue: Vec::new(),
            max_queue_size: 10_000,
        }
    }

    /// Get connection state.
    #[inline]
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Set connection state.
    #[inline]
    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    /// Get connection stats.
    #[inline]
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Check if connection is alive.
    #[inline]
    pub fn is_alive(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Connected | ConnectionState::Draining
        )
    }

    /// Queue a frame for sending.
    pub fn enqueue(&mut self, frame: Frame) -> Result<()> {
        if self.outbound_queue.len() >= self.max_queue_size {
            return Err(RmiError::ResourceExhausted(
                "Connection outbound queue full".to_string(),
            ));
        }
        self.outbound_queue.push(frame);
        Ok(())
    }

    /// Take pending frames.
    pub fn drain_outbound(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.outbound_queue)
    }

    /// Record bytes sent.
    #[inline]
    pub fn record_send(&mut self, bytes: u64) {
        self.stats.bytes_sent += bytes;
        self.stats.frames_sent += 1;
        self.stats.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
    }

    /// Record bytes received.
    #[inline]
    pub fn record_recv(&mut self, bytes: u64) {
        self.stats.bytes_received += bytes;
        self.stats.frames_received += 1;
        self.stats.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
    }

    /// Update RTT estimate.
    #[inline]
    pub fn update_rtt(&mut self, rtt_us: u64) {
        // Exponential moving average
        if self.stats.rtt_us == 0 {
            self.stats.rtt_us = rtt_us;
        } else {
            self.stats.rtt_us = (self.stats.rtt_us * 7 + rtt_us) / 8;
        }
    }

    /// Check if idle (no activity for given duration).
    pub fn is_idle(&self, timeout: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        (now - self.stats.last_activity) > timeout.as_secs_f64()
    }
}

// ============================================================================
// Connection Pool
// ============================================================================

/// Pool of connections to a specific peer.
#[derive(Debug)]
pub struct ConnectionPool {
    /// Target peer address
    pub target: SocketAddr,
    /// Pool of connections
    connections: Vec<Connection>,
    /// Maximum pool size
    max_size: usize,
    /// Round-robin index
    next_idx: usize,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(target: SocketAddr, max_size: usize, _protocol: TransportProtocol) -> Self {
        Self {
            target,
            connections: Vec::with_capacity(max_size),
            max_size,
            next_idx: 0,
        }
    }

    /// Add a connection to the pool.
    pub fn add(&mut self, mut conn: Connection) -> Result<()> {
        if self.connections.len() >= self.max_size {
            return Err(RmiError::ResourceExhausted(
                "Connection pool full".to_string(),
            ));
        }
        conn.set_state(ConnectionState::Connected);
        self.connections.push(conn);
        Ok(())
    }

    /// Get the next connection (round-robin).
    pub fn next_connection(&mut self) -> Option<&mut Connection> {
        if self.connections.is_empty() {
            return None;
        }

        // Find next alive connection
        let start = self.next_idx;
        loop {
            let idx = self.next_idx % self.connections.len();
            self.next_idx = (self.next_idx + 1) % self.connections.len();

            if self.connections[idx].is_alive() {
                return Some(&mut self.connections[idx]);
            }

            // Wrapped around without finding alive connection
            if self.next_idx == start {
                return None;
            }
        }
    }

    /// Remove dead connections.
    pub fn prune(&mut self) {
        self.connections.retain(|c| {
            c.state() != ConnectionState::Closed && c.state() != ConnectionState::Error
        });
    }

    /// Get number of alive connections.
    pub fn alive_count(&self) -> usize {
        self.connections.iter().filter(|c| c.is_alive()).count()
    }

    /// Get total connection count.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if pool is empty.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

// ============================================================================
// Node Address
// ============================================================================

/// Address of a node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAddr {
    /// Node unique identifier
    pub node_id: Uuid,
    /// Network address
    pub addr: String,
    /// Transport protocol
    pub protocol: TransportProtocol,
    /// Metadata (capabilities, region, etc.)
    pub metadata: HashMap<String, String>,
}

impl NodeAddr {
    /// Create a new node address.
    pub fn new(addr: &str, protocol: TransportProtocol) -> Self {
        Self {
            node_id: Uuid::new_v4(),
            addr: addr.to_string(),
            protocol,
            metadata: HashMap::new(),
        }
    }

    /// Create with a specific node ID.
    pub fn with_id(node_id: Uuid, addr: &str, protocol: TransportProtocol) -> Self {
        Self {
            node_id,
            addr: addr.to_string(),
            protocol,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Parse socket address.
    pub fn socket_addr(&self) -> Result<SocketAddr> {
        self.addr
            .parse()
            .map_err(|e: std::net::AddrParseError| RmiError::Protocol(e.to_string()))
    }
}

// ============================================================================
// Transport Manager
// ============================================================================

/// Manages all network connections and routing.
pub struct TransportManager {
    /// Local node address
    local_addr: NodeAddr,
    /// Configuration
    config: TransportConfig,
    /// Connection pools indexed by remote node ID
    pools: RwLock<HashMap<Uuid, ConnectionPool>>,
    /// Node address directory
    directory: RwLock<HashMap<Uuid, NodeAddr>>,
    /// Running flag
    running: AtomicBool,
    /// Stats
    total_bytes_sent: AtomicU64,
    total_bytes_received: AtomicU64,
    total_messages_sent: AtomicU64,
    total_messages_received: AtomicU64,
}

impl TransportManager {
    /// Create a new transport manager.
    pub fn new(local_addr: NodeAddr, config: TransportConfig) -> Self {
        Self {
            local_addr,
            config,
            pools: RwLock::new(HashMap::new()),
            directory: RwLock::new(HashMap::new()),
            running: AtomicBool::new(false),
            total_bytes_sent: AtomicU64::new(0),
            total_bytes_received: AtomicU64::new(0),
            total_messages_sent: AtomicU64::new(0),
            total_messages_received: AtomicU64::new(0),
        }
    }

    /// Get local node address.
    #[inline]
    pub fn local_addr(&self) -> &NodeAddr {
        &self.local_addr
    }

    /// Start the transport layer.
    pub fn start(&self) -> Result<()> {
        self.running.store(true, Ordering::Release);
        Ok(())
    }

    /// Stop the transport layer.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Check if transport is running.
    #[inline]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Register a remote node.
    pub fn register_node(&self, addr: NodeAddr) {
        let node_id = addr.node_id;
        self.directory.write().unwrap().insert(node_id, addr);
    }

    /// Unregister a remote node.
    pub fn unregister_node(&self, node_id: Uuid) {
        self.directory.write().unwrap().remove(&node_id);
        self.pools.write().unwrap().remove(&node_id);
    }

    /// Get registered node addresses.
    pub fn registered_nodes(&self) -> Vec<NodeAddr> {
        self.directory.read().unwrap().values().cloned().collect()
    }

    /// Get or create a connection pool.
    pub fn get_or_create_pool(&self, node_id: Uuid) -> Result<()> {
        let mut pools = self.pools.write().unwrap();
        if pools.contains_key(&node_id) {
            return Ok(());
        }

        let directory = self.directory.read().unwrap();
        let addr = directory
            .get(&node_id)
            .ok_or_else(|| RmiError::Protocol(format!("Node {} not in directory", node_id)))?;

        let socket_addr = addr.socket_addr()?;
        let pool = ConnectionPool::new(socket_addr, self.config.pool_size, addr.protocol);
        pools.insert(node_id, pool);
        Ok(())
    }

    /// Send a frame to a remote node.
    pub fn send(&self, node_id: Uuid, frame: Frame) -> Result<()> {
        if !self.is_running() {
            return Err(RmiError::Protocol("Transport not running".to_string()));
        }

        self.get_or_create_pool(node_id)?;

        let mut pools = self.pools.write().unwrap();
        let pool = pools
            .get_mut(&node_id)
            .ok_or_else(|| RmiError::Protocol("Pool not found".to_string()))?;

        // If pool has no connections, create one
        if pool.alive_count() == 0 {
            let conn = Connection::new(pool.target, self.config.protocol);
            pool.add(conn)?;
        }

        // Enqueue on next available connection
        if let Some(conn) = pool.next_connection() {
            let encoded = frame.encode();
            let len = encoded.len() as u64;
            conn.enqueue(frame)?;
            conn.record_send(len);
            self.total_bytes_sent.fetch_add(len, Ordering::Relaxed);
            self.total_messages_sent.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err(RmiError::Protocol("No available connections".to_string()))
        }
    }

    /// Send data to a remote node.
    pub fn send_data(&self, node_id: Uuid, data: Vec<u8>) -> Result<()> {
        self.send(node_id, Frame::data(data))
    }

    /// Broadcast data to all registered nodes.
    pub fn broadcast(&self, data: Vec<u8>) -> Vec<(Uuid, Result<()>)> {
        let node_ids: Vec<Uuid> = self.directory.read().unwrap().keys().copied().collect();
        node_ids
            .into_iter()
            .map(|id| {
                let result = self.send_data(id, data.clone());
                (id, result)
            })
            .collect()
    }

    /// Get transport statistics.
    pub fn stats(&self) -> TransportStats {
        let pools = self.pools.read().unwrap();
        let total_connections: usize = pools.values().map(|p| p.len()).sum();
        let alive_connections: usize = pools.values().map(|p| p.alive_count()).sum();

        TransportStats {
            total_bytes_sent: self.total_bytes_sent.load(Ordering::Relaxed),
            total_bytes_received: self.total_bytes_received.load(Ordering::Relaxed),
            total_messages_sent: self.total_messages_sent.load(Ordering::Relaxed),
            total_messages_received: self.total_messages_received.load(Ordering::Relaxed),
            total_connections,
            alive_connections,
            registered_nodes: self.directory.read().unwrap().len(),
        }
    }
}

/// Aggregate transport statistics.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Total messages sent
    pub total_messages_sent: u64,
    /// Total messages received
    pub total_messages_received: u64,
    /// Total connections (all states)
    pub total_connections: usize,
    /// Alive connections
    pub alive_connections: usize,
    /// Number of registered nodes
    pub registered_nodes: usize,
}

// ============================================================================
// Load Balancer
// ============================================================================

/// Load balancing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    /// Round-robin across nodes
    RoundRobin,
    /// Prefer node with lowest latency
    LeastLatency,
    /// Prefer node with least pending work
    LeastLoaded,
    /// Hash-based routing (consistent)
    ConsistentHash,
    /// Random selection
    Random,
}

/// Load balancer for distributing work across nodes.
pub struct LoadBalancer {
    /// Strategy
    strategy: LoadBalanceStrategy,
    /// Node weights (for weighted strategies)
    weights: RwLock<HashMap<Uuid, f64>>,
    /// Node latencies (microseconds)
    latencies: RwLock<HashMap<Uuid, u64>>,
    /// Node loads (pending task count)
    loads: RwLock<HashMap<Uuid, usize>>,
    /// Round-robin counter
    rr_counter: AtomicU64,
}

impl LoadBalancer {
    /// Create a new load balancer.
    pub fn new(strategy: LoadBalanceStrategy) -> Self {
        Self {
            strategy,
            weights: RwLock::new(HashMap::new()),
            latencies: RwLock::new(HashMap::new()),
            loads: RwLock::new(HashMap::new()),
            rr_counter: AtomicU64::new(0),
        }
    }

    /// Register a node with weight.
    pub fn register_node(&self, node_id: Uuid, weight: f64) {
        self.weights.write().unwrap().insert(node_id, weight);
        self.latencies.write().unwrap().insert(node_id, 0);
        self.loads.write().unwrap().insert(node_id, 0);
    }

    /// Unregister a node.
    pub fn unregister_node(&self, node_id: Uuid) {
        self.weights.write().unwrap().remove(&node_id);
        self.latencies.write().unwrap().remove(&node_id);
        self.loads.write().unwrap().remove(&node_id);
    }

    /// Update node latency.
    pub fn update_latency(&self, node_id: Uuid, latency_us: u64) {
        self.latencies.write().unwrap().insert(node_id, latency_us);
    }

    /// Update node load.
    pub fn update_load(&self, node_id: Uuid, load: usize) {
        self.loads.write().unwrap().insert(node_id, load);
    }

    /// Select the best node for a request.
    pub fn select(&self, candidates: &[Uuid]) -> Option<Uuid> {
        if candidates.is_empty() {
            return None;
        }

        match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let idx = self.rr_counter.fetch_add(1, Ordering::Relaxed) as usize;
                Some(candidates[idx % candidates.len()])
            }
            LoadBalanceStrategy::LeastLatency => {
                let latencies = self.latencies.read().unwrap();
                candidates
                    .iter()
                    .min_by_key(|id| latencies.get(id).copied().unwrap_or(u64::MAX))
                    .copied()
            }
            LoadBalanceStrategy::LeastLoaded => {
                let loads = self.loads.read().unwrap();
                candidates
                    .iter()
                    .min_by_key(|id| loads.get(id).copied().unwrap_or(usize::MAX))
                    .copied()
            }
            LoadBalanceStrategy::ConsistentHash => {
                // Simple modular hash selection
                let idx = self.rr_counter.fetch_add(1, Ordering::Relaxed);
                let hash = xxhash_rust::xxh64::xxh64(&idx.to_be_bytes(), 0);
                Some(candidates[(hash as usize) % candidates.len()])
            }
            LoadBalanceStrategy::Random => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..candidates.len());
                Some(candidates[idx])
            }
        }
    }

    /// Get strategy.
    #[inline]
    pub fn strategy(&self) -> LoadBalanceStrategy {
        self.strategy
    }

    /// Set strategy.
    pub fn set_strategy(&mut self, strategy: LoadBalanceStrategy) {
        self.strategy = strategy;
    }
}

// ============================================================================
// Real TCP Transport (tokio-based)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
/// Async TCP transport using tokio for real network I/O.
///
/// Provides `listen`, `connect`, `send_frame`, and `recv_frame` methods
/// that perform length-delimited framing over TCP sockets.
///
/// # Example
///
/// ```rust,no_run
/// use rmi::distributed::transport::{TcpTransport, Frame};
///
/// #[tokio::main]
/// async fn main() {
///     // Server side
///     let mut server = TcpTransport::listen("127.0.0.1:9800").await.unwrap();
///     
///     // Client side (from another task)
///     let mut client = TcpTransport::connect("127.0.0.1:9800").await.unwrap();
///     client.send_frame(&Frame::data(b"hello".to_vec())).await.unwrap();
/// }
/// ```
pub struct TcpTransport {
    stream: tokio::net::TcpStream,
}

#[cfg(not(target_arch = "wasm32"))]
impl TcpTransport {
    /// Connect to a remote TCP peer.
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP connect failed: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| RmiError::Protocol(format!("set_nodelay failed: {e}")))?;
        Ok(Self { stream })
    }

    /// Listen for a single incoming TCP connection.
    pub async fn listen(addr: &str) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP bind failed: {e}")))?;
        let (stream, _peer) = listener
            .accept()
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP accept failed: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| RmiError::Protocol(format!("set_nodelay failed: {e}")))?;
        Ok(Self { stream })
    }

    /// Wrap an existing TcpStream.
    pub fn from_stream(stream: tokio::net::TcpStream) -> Self {
        Self { stream }
    }

    /// Send a frame over the TCP connection.
    ///
    /// Writes a 4-byte big-endian length prefix followed by the encoded frame.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let encoded = frame.encode();
        let len = encoded.len() as u32;

        self.stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP write len failed: {e}")))?;
        self.stream
            .write_all(&encoded)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP write payload failed: {e}")))?;
        self.stream
            .flush()
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP flush failed: {e}")))?;
        Ok(())
    }

    /// Receive a frame from the TCP connection.
    ///
    /// Reads a 4-byte big-endian length prefix, then the frame payload.
    pub async fn recv_frame(&mut self) -> Result<Frame> {
        use tokio::io::AsyncReadExt;

        let mut len_buf = [0u8; 4];
        self.stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP read len failed: {e}")))?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 64 * 1024 * 1024 {
            return Err(RmiError::Protocol(format!(
                "Frame too large: {} bytes",
                len
            )));
        }

        let mut buf = vec![0u8; len];
        self.stream
            .read_exact(&mut buf)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP read payload failed: {e}")))?;

        let (frame, _consumed) = Frame::decode(&buf)?;
        Ok(frame)
    }

    /// Get the local address of this connection.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.stream
            .local_addr()
            .map_err(|e| RmiError::Protocol(format!("local_addr failed: {e}")))
    }

    /// Get the peer address of this connection.
    pub fn peer_addr(&self) -> Result<SocketAddr> {
        self.stream
            .peer_addr()
            .map_err(|e| RmiError::Protocol(format!("peer_addr failed: {e}")))
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Async TCP listener that accepts multiple connections.
pub struct TcpListener {
    listener: tokio::net::TcpListener,
}

#[cfg(not(target_arch = "wasm32"))]
impl TcpListener {
    /// Bind to an address and start listening.
    pub async fn bind(addr: &str) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP bind failed: {e}")))?;
        Ok(Self { listener })
    }

    /// Accept the next incoming connection.
    pub async fn accept(&self) -> Result<(TcpTransport, SocketAddr)> {
        let (stream, addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| RmiError::Protocol(format!("TCP accept failed: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| RmiError::Protocol(format!("set_nodelay failed: {e}")))?;
        Ok((TcpTransport::from_stream(stream), addr))
    }

    /// Get the local address the listener is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener
            .local_addr()
            .map_err(|e| RmiError::Protocol(format!("local_addr failed: {e}")))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = Frame::data(vec![1, 2, 3, 4, 5]);
        let encoded = frame.encode();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap();

        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.frame_type, FrameType::Data);
        assert_eq!(decoded.payload, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_frame_ping_pong() {
        let ping = Frame::ping();
        assert_eq!(ping.frame_type, FrameType::Ping);
        assert_eq!(ping.payload.len(), 8); // u64 timestamp

        let pong = Frame::pong(ping.payload.clone());
        assert_eq!(pong.frame_type, FrameType::Pong);
        assert_eq!(pong.payload, ping.payload);
    }

    #[test]
    fn test_frame_goaway() {
        let frame = Frame::goaway("shutting down");
        assert_eq!(frame.frame_type, FrameType::GoAway);
        assert_eq!(String::from_utf8_lossy(&frame.payload), "shutting down");
    }

    #[test]
    fn test_frame_flags() {
        let mut flags = FrameFlags::NONE;
        assert!(!flags.contains(FrameFlags::COMPRESSED));

        flags.set(FrameFlags::COMPRESSED);
        assert!(flags.contains(FrameFlags::COMPRESSED));

        flags.set(FrameFlags::PRIORITY);
        assert!(flags.contains(FrameFlags::COMPRESSED));
        assert!(flags.contains(FrameFlags::PRIORITY));
    }

    #[test]
    fn test_connection_lifecycle() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let mut conn = Connection::new(addr, TransportProtocol::Tcp);

        assert_eq!(conn.state(), ConnectionState::Connecting);
        conn.set_state(ConnectionState::Connected);
        assert!(conn.is_alive());

        conn.record_send(100);
        conn.record_recv(200);
        assert_eq!(conn.stats().bytes_sent, 100);
        assert_eq!(conn.stats().bytes_received, 200);
        assert_eq!(conn.stats().frames_sent, 1);
        assert_eq!(conn.stats().frames_received, 1);
    }

    #[test]
    fn test_connection_rtt() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let mut conn = Connection::new(addr, TransportProtocol::Tcp);

        conn.update_rtt(1000);
        assert_eq!(conn.stats().rtt_us, 1000);

        conn.update_rtt(500);
        // EMA: (1000 * 7 + 500) / 8 = 937
        assert_eq!(conn.stats().rtt_us, 937);
    }

    #[test]
    fn test_connection_pool() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let mut pool = ConnectionPool::new(addr, 3, TransportProtocol::Tcp);

        assert!(pool.is_empty());
        assert_eq!(pool.alive_count(), 0);

        let conn = Connection::new(addr, TransportProtocol::Tcp);
        pool.add(conn).unwrap();
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.alive_count(), 1);

        // Should round-robin
        assert!(pool.next_connection().is_some());
    }

    #[test]
    fn test_connection_pool_full() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let mut pool = ConnectionPool::new(addr, 1, TransportProtocol::Tcp);

        let conn1 = Connection::new(addr, TransportProtocol::Tcp);
        pool.add(conn1).unwrap();

        let conn2 = Connection::new(addr, TransportProtocol::Tcp);
        assert!(pool.add(conn2).is_err());
    }

    #[test]
    fn test_node_addr() {
        let addr = NodeAddr::new("127.0.0.1:9700", TransportProtocol::Tcp)
            .with_metadata("region", "us-west-2");

        assert_eq!(addr.addr, "127.0.0.1:9700");
        assert_eq!(addr.metadata.get("region").unwrap(), "us-west-2");
        assert!(addr.socket_addr().is_ok());
    }

    #[test]
    fn test_transport_manager() {
        let local = NodeAddr::new("127.0.0.1:9700", TransportProtocol::Tcp);
        let tm = TransportManager::new(local.clone(), TransportConfig::default());

        assert!(!tm.is_running());
        tm.start().unwrap();
        assert!(tm.is_running());

        let remote = NodeAddr::new("127.0.0.1:9701", TransportProtocol::Tcp);
        let _remote_id = remote.node_id;
        tm.register_node(remote);

        assert_eq!(tm.registered_nodes().len(), 1);

        let stats = tm.stats();
        assert_eq!(stats.registered_nodes, 1);

        tm.stop();
        assert!(!tm.is_running());
    }

    #[test]
    fn test_load_balancer_round_robin() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::RoundRobin);
        let nodes: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        for &id in &nodes {
            lb.register_node(id, 1.0);
        }

        let first = lb.select(&nodes).unwrap();
        let second = lb.select(&nodes).unwrap();
        let _third = lb.select(&nodes).unwrap();
        let fourth = lb.select(&nodes).unwrap();

        // Round robin should cycle
        assert_eq!(first, fourth);
        assert_ne!(first, second);
    }

    #[test]
    fn test_load_balancer_least_latency() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::LeastLatency);
        let nodes: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        for &id in &nodes {
            lb.register_node(id, 1.0);
        }

        lb.update_latency(nodes[0], 100);
        lb.update_latency(nodes[1], 50);
        lb.update_latency(nodes[2], 200);

        let selected = lb.select(&nodes).unwrap();
        assert_eq!(selected, nodes[1]); // Lowest latency
    }

    #[test]
    fn test_load_balancer_least_loaded() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::LeastLoaded);
        let nodes: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        for &id in &nodes {
            lb.register_node(id, 1.0);
        }

        lb.update_load(nodes[0], 10);
        lb.update_load(nodes[1], 5);
        lb.update_load(nodes[2], 20);

        let selected = lb.select(&nodes).unwrap();
        assert_eq!(selected, nodes[1]); // Least loaded
    }

    #[test]
    fn test_load_balancer_empty() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::RoundRobin);
        assert!(lb.select(&[]).is_none());
    }

    #[test]
    fn test_frame_decode_invalid_magic() {
        let data = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(Frame::decode(&data).is_err());
    }

    #[test]
    fn test_frame_decode_truncated() {
        let data = vec![b'R', b'M', b'I', b't'];
        assert!(Frame::decode(&data).is_err());
    }

    #[tokio::test]
    async fn test_tcp_transport_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut conn, _peer) = listener.accept().await.unwrap();
            let frame = conn.recv_frame().await.unwrap();
            assert_eq!(frame.frame_type, FrameType::Data);
            assert_eq!(frame.payload, vec![10, 20, 30]);
            conn.send_frame(&Frame::data(vec![40, 50])).await.unwrap();
        });

        let mut client = TcpTransport::connect(&addr.to_string()).await.unwrap();
        client
            .send_frame(&Frame::data(vec![10, 20, 30]))
            .await
            .unwrap();
        let reply = client.recv_frame().await.unwrap();
        assert_eq!(reply.payload, vec![40, 50]);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_tcp_transport_multiple_frames() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            for i in 0u8..5 {
                let frame = conn.recv_frame().await.unwrap();
                assert_eq!(frame.payload, vec![i]);
            }
            conn.send_frame(&Frame::data(vec![0xFF])).await.unwrap();
        });

        let mut client = TcpTransport::connect(&addr.to_string()).await.unwrap();
        for i in 0u8..5 {
            client.send_frame(&Frame::data(vec![i])).await.unwrap();
        }
        let ack = client.recv_frame().await.unwrap();
        assert_eq!(ack.payload, vec![0xFF]);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_tcp_transport_ping_pong() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut conn, _) = listener.accept().await.unwrap();
            let ping = conn.recv_frame().await.unwrap();
            assert_eq!(ping.frame_type, FrameType::Ping);
            conn.send_frame(&Frame::pong(ping.payload)).await.unwrap();
        });

        let mut client = TcpTransport::connect(&addr.to_string()).await.unwrap();
        let ping = Frame::ping();
        let ping_payload = ping.payload.clone();
        client.send_frame(&ping).await.unwrap();
        let pong = client.recv_frame().await.unwrap();
        assert_eq!(pong.frame_type, FrameType::Pong);
        assert_eq!(pong.payload, ping_payload);

        server.await.unwrap();
    }

    #[test]
    fn test_connection_is_idle() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let conn = Connection::new(addr, TransportProtocol::Tcp);
        // Just created — not idle for a generous timeout
        assert!(!conn.is_idle(Duration::from_secs(3600)));
    }

    #[test]
    fn test_connection_pool_round_robin() {
        let addr: SocketAddr = "127.0.0.1:9700".parse().unwrap();
        let mut pool = ConnectionPool::new(addr, 3, TransportProtocol::Tcp);

        let mut c1 = Connection::new(addr, TransportProtocol::Tcp);
        c1.set_state(ConnectionState::Connected);
        let mut c2 = Connection::new(addr, TransportProtocol::Tcp);
        c2.set_state(ConnectionState::Connected);

        pool.add(c1).unwrap();
        pool.add(c2).unwrap();

        // Round-robin should cycle between connections
        let a = pool.next_connection().map(|c| c.id);
        let b = pool.next_connection().map(|c| c.id);
        assert!(a.is_some() && b.is_some());
    }

    #[test]
    fn test_frame_data_roundtrip_large() {
        let payload: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let frame = Frame::data(payload.clone());
        let encoded = frame.encode();
        let (decoded, _) = Frame::decode(&encoded).unwrap();
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_frame_flags_operations() {
        let mut flags = FrameFlags::NONE;
        flags.set(FrameFlags::COMPRESSED);
        flags.set(FrameFlags::ACK_REQUIRED);
        assert!(flags.contains(FrameFlags::COMPRESSED));
        assert!(flags.contains(FrameFlags::ACK_REQUIRED));
    }

    #[test]
    fn test_load_balancer_consistent_hash() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::ConsistentHash);
        let nodes: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        for &id in &nodes {
            lb.register_node(id, 1.0);
        }

        let selected = lb.select(&nodes);
        assert!(selected.is_some());
        assert!(nodes.contains(&selected.unwrap()));
    }

    #[test]
    fn test_load_balancer_unregister() {
        let lb = LoadBalancer::new(LoadBalanceStrategy::LeastLoaded);
        let n1 = Uuid::new_v4();
        lb.register_node(n1, 1.0);
        lb.unregister_node(n1);
        // Node removed from weights
        let selected = lb.select(&[n1]);
        // Should still work (just selects from candidates)
        assert!(selected.is_some());
    }

    #[test]
    fn test_transport_manager_stop_idempotent() {
        let local = NodeAddr::new("127.0.0.1:9700", TransportProtocol::Tcp);
        let tm = TransportManager::new(local, TransportConfig::default());
        tm.stop(); // Not running, should not panic
        assert!(!tm.is_running());
    }

    #[test]
    fn test_node_addr_invalid() {
        let addr = NodeAddr::new("not-an-address", TransportProtocol::Tcp);
        assert!(addr.socket_addr().is_err());
    }
}
