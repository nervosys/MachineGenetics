//! Distributed Agent Infrastructure
//!
//! Multi-node agent communication, discovery, consensus, and federation:
//!
//! - **Transport**: TCP/QUIC network transport with connection pooling
//! - **Discovery**: Service registry, gossip protocol, health monitoring
//! - **Consensus**: Raft and Byzantine fault tolerance for distributed state
//! - **Federation**: Cross-cluster agent communication and resource sharing

pub mod consensus;
pub mod discovery;
pub mod federation;
pub mod transport;

pub use consensus::{
    BftNode, BftPhase, CheckpointCoordinator, DistributedCheckpoint, LogEntry, RaftCommand,
    RaftNode, RaftRole,
};
pub use discovery::{
    DiscoveryConfig, DiscoveryMethod, GossipState, HealthStatus, ServiceInfo, ServiceRegistry,
};
pub use federation::{
    Cluster, ClusterCapacity, ClusterStatus, FederationGateway, FederationManager,
    FederationMessage, FederationStats, ResourceRequest, ResourceSharingPolicy, ResourceType,
};
pub use transport::{
    Connection, ConnectionPool, Frame, LoadBalancer, NodeAddr, TransportConfig, TransportManager,
    TransportProtocol, TransportStats,
};
