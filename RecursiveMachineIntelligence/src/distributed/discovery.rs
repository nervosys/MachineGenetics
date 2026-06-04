//! Agent Discovery - Service Registry and Peer Discovery
//!
//! Provides automatic discovery of agents across a distributed cluster.
//! Supports multiple discovery mechanisms:
//!
//! - **Multicast/Broadcast**: LAN-based automatic discovery
//! - **Registry**: Centralized service registry
//! - **Gossip**: Decentralized peer-to-peer discovery
//! - **Static**: Pre-configured peer list

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::transport::NodeAddr;
use crate::error::{Result, RmiError};

// ============================================================================
// Service Information
// ============================================================================

/// Information about a discovered agent/service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Unique service ID
    pub id: Uuid,
    /// Service name
    pub name: String,
    /// Node address
    pub addr: NodeAddr,
    /// Agent capabilities
    pub capabilities: Vec<String>,
    /// Service version
    pub version: String,
    /// Health status
    pub health: HealthStatus,
    /// Registration timestamp
    pub registered_at: f64,
    /// Last heartbeat timestamp
    pub last_heartbeat: f64,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
    /// Load metric (0.0 = idle, 1.0 = fully loaded)
    pub load: f64,
}

impl ServiceInfo {
    /// Create a new service info.
    pub fn new(name: &str, addr: NodeAddr, capabilities: Vec<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: addr.node_id,
            name: name.to_string(),
            addr,
            capabilities,
            version: "0.1.0".to_string(),
            health: HealthStatus::Healthy,
            registered_at: now,
            last_heartbeat: now,
            metadata: HashMap::new(),
            load: 0.0,
        }
    }

    /// Update heartbeat.
    #[inline]
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
    }

    /// Check if service is stale (no heartbeat for given duration).
    pub fn is_stale(&self, timeout: Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        (now - self.last_heartbeat) > timeout.as_secs_f64()
    }

    /// With metadata.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// With load.
    pub fn with_load(mut self, load: f64) -> Self {
        self.load = load.clamp(0.0, 1.0);
        self
    }
}

/// Health status of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Service is healthy and responsive
    Healthy,
    /// Service is degraded but operational
    Degraded,
    /// Service is unhealthy (not responding)
    Unhealthy,
    /// Service health is unknown
    Unknown,
}

// ============================================================================
// Discovery Method
// ============================================================================

/// Discovery method configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    /// Static peer list
    Static {
        /// Pre-configured peer addresses
        peers: Vec<NodeAddr>,
    },
    /// Centralized registry
    Registry {
        /// Registry server address
        registry_addr: String,
        /// Refresh interval
        refresh_interval: Duration,
    },
    /// Gossip-based discovery
    Gossip {
        /// Seed nodes for initial contact
        seed_nodes: Vec<NodeAddr>,
        /// Gossip interval
        gossip_interval: Duration,
        /// Fanout (number of peers to gossip with each round)
        fanout: usize,
    },
    /// Multicast/broadcast discovery
    Multicast {
        /// Multicast group address
        group_addr: String,
        /// Announce interval
        announce_interval: Duration,
    },
}

/// Discovery configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Discovery method
    pub method: DiscoveryMethod,
    /// Service health check interval
    pub health_check_interval: Duration,
    /// Service expiry timeout (no heartbeat)
    pub service_timeout: Duration,
    /// Maximum number of tracked services
    pub max_services: usize,
    /// Namespace for service isolation
    pub namespace: String,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            method: DiscoveryMethod::Static { peers: Vec::new() },
            health_check_interval: Duration::from_secs(10),
            service_timeout: Duration::from_secs(60),
            max_services: 1000,
            namespace: "default".to_string(),
        }
    }
}

// ============================================================================
// Service Registry
// ============================================================================

/// In-memory service registry for tracking discovered agents.
pub struct ServiceRegistry {
    /// Configuration
    config: DiscoveryConfig,
    /// Registered services by ID
    services: RwLock<HashMap<Uuid, ServiceInfo>>,
    /// Services indexed by capability
    capability_index: RwLock<HashMap<String, Vec<Uuid>>>,
    /// Services indexed by name
    name_index: RwLock<HashMap<String, Vec<Uuid>>>,
    /// Event listeners
    listeners: RwLock<Vec<Box<dyn DiscoveryListener>>>,
}

impl ServiceRegistry {
    /// Create a new service registry.
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            config,
            services: RwLock::new(HashMap::new()),
            capability_index: RwLock::new(HashMap::new()),
            name_index: RwLock::new(HashMap::new()),
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Register a service.
    pub fn register(&self, service: ServiceInfo) -> Result<()> {
        let services = self.services.read().unwrap();
        if services.len() >= self.config.max_services {
            return Err(RmiError::ResourceExhausted(
                "Service registry full".to_string(),
            ));
        }
        drop(services);

        let id = service.id;
        let name = service.name.clone();
        let capabilities = service.capabilities.clone();

        // Insert into main registry
        self.services.write().unwrap().insert(id, service);

        // Update capability index
        {
            let mut cap_idx = self.capability_index.write().unwrap();
            for cap in &capabilities {
                cap_idx.entry(cap.clone()).or_default().push(id);
            }
        }

        // Update name index
        {
            let mut name_idx = self.name_index.write().unwrap();
            name_idx.entry(name).or_default().push(id);
        }

        // Notify listeners
        self.notify_registered(id);

        Ok(())
    }

    /// Deregister a service.
    pub fn deregister(&self, service_id: Uuid) -> Option<ServiceInfo> {
        let removed = self.services.write().unwrap().remove(&service_id);

        if let Some(ref service) = removed {
            // Clean capability index
            let mut cap_idx = self.capability_index.write().unwrap();
            for cap in &service.capabilities {
                if let Some(ids) = cap_idx.get_mut(cap) {
                    ids.retain(|&id| id != service_id);
                }
            }

            // Clean name index
            let mut name_idx = self.name_index.write().unwrap();
            if let Some(ids) = name_idx.get_mut(&service.name) {
                ids.retain(|&id| id != service_id);
            }

            // Notify listeners
            self.notify_deregistered(service_id);
        }

        removed
    }

    /// Update service heartbeat.
    pub fn heartbeat(&self, service_id: Uuid) -> bool {
        let mut services = self.services.write().unwrap();
        if let Some(service) = services.get_mut(&service_id) {
            service.heartbeat();
            true
        } else {
            false
        }
    }

    /// Update service health status.
    pub fn update_health(&self, service_id: Uuid, health: HealthStatus) {
        let mut services = self.services.write().unwrap();
        if let Some(service) = services.get_mut(&service_id) {
            service.health = health;
        }
    }

    /// Update service load.
    pub fn update_load(&self, service_id: Uuid, load: f64) {
        let mut services = self.services.write().unwrap();
        if let Some(service) = services.get_mut(&service_id) {
            service.load = load.clamp(0.0, 1.0);
        }
    }

    /// Get a service by ID.
    pub fn get(&self, service_id: Uuid) -> Option<ServiceInfo> {
        self.services.read().unwrap().get(&service_id).cloned()
    }

    /// Find services by capability.
    pub fn find_by_capability(&self, capability: &str) -> Vec<ServiceInfo> {
        let cap_idx = self.capability_index.read().unwrap();
        let services = self.services.read().unwrap();

        cap_idx
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| services.get(id).cloned())
                    .filter(|s| s.health != HealthStatus::Unhealthy)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find services by name.
    pub fn find_by_name(&self, name: &str) -> Vec<ServiceInfo> {
        let name_idx = self.name_index.read().unwrap();
        let services = self.services.read().unwrap();

        name_idx
            .get(name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| services.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all healthy services.
    pub fn healthy_services(&self) -> Vec<ServiceInfo> {
        self.services
            .read()
            .unwrap()
            .values()
            .filter(|s| s.health == HealthStatus::Healthy)
            .cloned()
            .collect()
    }

    /// Get all services.
    pub fn all_services(&self) -> Vec<ServiceInfo> {
        self.services.read().unwrap().values().cloned().collect()
    }

    /// Get count of registered services.
    pub fn len(&self) -> usize {
        self.services.read().unwrap().len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove stale services.
    pub fn prune_stale(&self) -> Vec<Uuid> {
        let timeout = self.config.service_timeout;
        let stale_ids: Vec<Uuid> = self
            .services
            .read()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.is_stale(timeout))
            .map(|(id, _)| *id)
            .collect();

        for &id in &stale_ids {
            self.deregister(id);
        }

        stale_ids
    }

    /// Add a discovery listener.
    pub fn add_listener(&self, listener: Box<dyn DiscoveryListener>) {
        self.listeners.write().unwrap().push(listener);
    }

    fn notify_registered(&self, service_id: Uuid) {
        let listeners = self.listeners.read().unwrap();
        for listener in listeners.iter() {
            listener.on_service_registered(service_id);
        }
    }

    fn notify_deregistered(&self, service_id: Uuid) {
        let listeners = self.listeners.read().unwrap();
        for listener in listeners.iter() {
            listener.on_service_deregistered(service_id);
        }
    }
}

/// Listener for discovery events.
pub trait DiscoveryListener: Send + Sync {
    /// Called when a new service is registered.
    fn on_service_registered(&self, service_id: Uuid);
    /// Called when a service is deregistered.
    fn on_service_deregistered(&self, service_id: Uuid);
    /// Called when service health changes.
    fn on_health_changed(&self, service_id: Uuid, health: HealthStatus);
}

// ============================================================================
// Gossip Protocol
// ============================================================================

/// Gossip message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Announce own presence
    Announce(Box<ServiceInfo>),
    /// Share known peers
    PeerList(Vec<ServiceInfo>),
    /// Request peer list
    PeerRequest,
    /// Health probe
    Ping(Uuid),
    /// Health probe response
    Pong(Uuid),
    /// Service departure notification
    Leave(Uuid),
}

/// Gossip protocol state.
pub struct GossipState {
    /// Local service info
    pub local_info: ServiceInfo,
    /// Known peers
    peers: RwLock<HashMap<Uuid, ServiceInfo>>,
    /// Gossip round counter
    round: std::sync::atomic::AtomicU64,
    /// Fanout per round
    fanout: usize,
    /// Suspicion map (suspected failures)
    suspects: RwLock<HashMap<Uuid, u32>>,
    /// Suspicion threshold before marking unhealthy
    suspicion_threshold: u32,
}

impl GossipState {
    /// Create new gossip state.
    pub fn new(local_info: ServiceInfo, fanout: usize) -> Self {
        Self {
            local_info,
            peers: RwLock::new(HashMap::new()),
            round: std::sync::atomic::AtomicU64::new(0),
            fanout,
            suspects: RwLock::new(HashMap::new()),
            suspicion_threshold: 3,
        }
    }

    /// Process an incoming gossip message.
    pub fn process_message(&self, msg: GossipMessage) -> Option<GossipMessage> {
        match msg {
            GossipMessage::Announce(info) => {
                self.merge_peer(*info);
                None
            }
            GossipMessage::PeerList(peers) => {
                for peer in peers {
                    self.merge_peer(peer);
                }
                None
            }
            GossipMessage::PeerRequest => {
                let peers = self.known_peers();
                Some(GossipMessage::PeerList(peers))
            }
            GossipMessage::Ping(from_id) => {
                // Clear suspicion
                self.suspects.write().unwrap().remove(&from_id);
                Some(GossipMessage::Pong(self.local_info.id))
            }
            GossipMessage::Pong(from_id) => {
                // Clear suspicion
                self.suspects.write().unwrap().remove(&from_id);
                if let Some(peer) = self.peers.write().unwrap().get_mut(&from_id) {
                    peer.health = HealthStatus::Healthy;
                    peer.heartbeat();
                }
                None
            }
            GossipMessage::Leave(id) => {
                self.peers.write().unwrap().remove(&id);
                None
            }
        }
    }

    /// Merge a discovered peer into local state.
    fn merge_peer(&self, info: ServiceInfo) {
        if info.id == self.local_info.id {
            return; // Don't track ourselves
        }
        let mut peers = self.peers.write().unwrap();
        let entry = peers.entry(info.id).or_insert(info.clone());
        // Update if newer heartbeat
        if info.last_heartbeat > entry.last_heartbeat {
            *entry = info;
        }
    }

    /// Get peers to gossip with this round.
    pub fn select_gossip_targets(&self) -> Vec<ServiceInfo> {
        let peers = self.peers.read().unwrap();
        let all: Vec<&ServiceInfo> = peers.values().collect();

        if all.len() <= self.fanout {
            return all.into_iter().cloned().collect();
        }

        // Random selection
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        let mut selected: Vec<&ServiceInfo> = all;
        selected.shuffle(&mut rng);
        selected.into_iter().take(self.fanout).cloned().collect()
    }

    /// Get all known peers.
    pub fn known_peers(&self) -> Vec<ServiceInfo> {
        self.peers.read().unwrap().values().cloned().collect()
    }

    /// Get count of known peers.
    pub fn peer_count(&self) -> usize {
        self.peers.read().unwrap().len()
    }

    /// Record a failed ping (suspicion).
    pub fn suspect(&self, peer_id: Uuid) {
        let mut suspects = self.suspects.write().unwrap();
        let count = suspects.entry(peer_id).or_insert(0);
        *count += 1;

        if *count >= self.suspicion_threshold {
            // Mark as unhealthy
            if let Some(peer) = self.peers.write().unwrap().get_mut(&peer_id) {
                peer.health = HealthStatus::Unhealthy;
            }
        }
    }

    /// Advance gossip round.
    pub fn advance_round(&self) -> u64 {
        self.round
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Get current round.
    pub fn current_round(&self) -> u64 {
        self.round.load(std::sync::atomic::Ordering::Relaxed)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::TransportProtocol;

    fn make_service(name: &str, capabilities: Vec<&str>) -> ServiceInfo {
        let addr = NodeAddr::new("127.0.0.1:9700", TransportProtocol::Tcp);
        ServiceInfo::new(
            name,
            addr,
            capabilities.into_iter().map(String::from).collect(),
        )
    }

    #[test]
    fn test_service_registry_register() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent-1", vec!["training", "inference"]);
        let id = service.id;

        registry.register(service).unwrap();
        assert_eq!(registry.len(), 1);

        let found = registry.get(id).unwrap();
        assert_eq!(found.name, "agent-1");
    }

    #[test]
    fn test_service_registry_deregister() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent-1", vec!["training"]);
        let id = service.id;

        registry.register(service).unwrap();
        assert_eq!(registry.len(), 1);

        let removed = registry.deregister(id);
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_service_registry_find_by_capability() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());

        let s1 = make_service("trainer", vec!["training", "evaluation"]);
        let s2 = make_service("optimizer", vec!["optimization", "evaluation"]);
        let s3 = make_service("searcher", vec!["architecture_search"]);

        registry.register(s1).unwrap();
        registry.register(s2).unwrap();
        registry.register(s3).unwrap();

        let eval_services = registry.find_by_capability("evaluation");
        assert_eq!(eval_services.len(), 2);

        let search_services = registry.find_by_capability("architecture_search");
        assert_eq!(search_services.len(), 1);

        let missing = registry.find_by_capability("nonexistent");
        assert!(missing.is_empty());
    }

    #[test]
    fn test_service_registry_find_by_name() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());

        let s1 = make_service("trainer", vec!["training"]);
        let s2 = make_service("trainer", vec!["training"]);
        let _s3 = make_service("optimizer", vec!["optimization"]);

        registry.register(s1).unwrap();
        registry.register(s2).unwrap();
        registry.register(_s3).unwrap();

        let trainers = registry.find_by_name("trainer");
        assert_eq!(trainers.len(), 2);
    }

    #[test]
    fn test_service_registry_heartbeat() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent", vec!["training"]);
        let id = service.id;

        registry.register(service).unwrap();
        assert!(registry.heartbeat(id));
        assert!(!registry.heartbeat(Uuid::new_v4()));
    }

    #[test]
    fn test_service_registry_health() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent", vec!["training"]);
        let id = service.id;

        registry.register(service).unwrap();
        registry.update_health(id, HealthStatus::Degraded);

        let found = registry.get(id).unwrap();
        assert_eq!(found.health, HealthStatus::Degraded);
    }

    #[test]
    fn test_service_stale_check() {
        let mut service = make_service("agent", vec!["training"]);
        // Set last heartbeat to long ago
        service.last_heartbeat = 0.0;

        assert!(service.is_stale(Duration::from_secs(1)));
    }

    #[test]
    fn test_gossip_announce() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        let remote = make_service("remote", vec!["inference"]);
        let remote_id = remote.id;

        gossip.process_message(GossipMessage::Announce(Box::new(remote)));
        assert_eq!(gossip.peer_count(), 1);

        let peers = gossip.known_peers();
        assert_eq!(peers[0].id, remote_id);
    }

    #[test]
    fn test_gossip_peer_list() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        let p1 = make_service("peer1", vec!["a"]);
        let p2 = make_service("peer2", vec!["b"]);

        gossip.process_message(GossipMessage::PeerList(vec![p1, p2]));
        assert_eq!(gossip.peer_count(), 2);
    }

    #[test]
    fn test_gossip_ping_pong() {
        let local = make_service("local", vec!["training"]);
        let _local_id = local.id;
        let gossip = GossipState::new(local, 3);

        let result = gossip.process_message(GossipMessage::Ping(Uuid::new_v4()));
        assert!(matches!(result, Some(GossipMessage::Pong(_))));
    }

    #[test]
    fn test_gossip_leave() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        let remote = make_service("remote", vec!["inference"]);
        let remote_id = remote.id;

        gossip.process_message(GossipMessage::Announce(Box::new(remote)));
        assert_eq!(gossip.peer_count(), 1);

        gossip.process_message(GossipMessage::Leave(remote_id));
        assert_eq!(gossip.peer_count(), 0);
    }

    #[test]
    fn test_gossip_suspicion() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        let remote = make_service("remote", vec!["inference"]);
        let remote_id = remote.id;
        gossip.process_message(GossipMessage::Announce(Box::new(remote)));

        // Suspect 3 times should mark unhealthy
        gossip.suspect(remote_id);
        gossip.suspect(remote_id);
        gossip.suspect(remote_id);

        let peers = gossip.known_peers();
        assert_eq!(peers[0].health, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_gossip_self_filter() {
        let local = make_service("local", vec!["training"]);
        let _local_id = local.id;
        let gossip = GossipState::new(local.clone(), 3);

        // Announcing ourselves should be ignored
        gossip.process_message(GossipMessage::Announce(Box::new(local)));
        assert_eq!(gossip.peer_count(), 0);
    }

    #[test]
    fn test_gossip_round() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        assert_eq!(gossip.current_round(), 0);
        gossip.advance_round();
        assert_eq!(gossip.current_round(), 1);
    }

    #[test]
    fn test_service_registry_update_load() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent-1", vec!["training"]);
        let id = service.id;
        registry.register(service).unwrap();

        registry.update_load(id, 0.75);
        let found = registry.get(id).unwrap();
        assert!((found.load - 0.75).abs() < 1e-5);

        // Clamped to [0, 1]
        registry.update_load(id, 2.0);
        let found = registry.get(id).unwrap();
        assert!((found.load - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_service_registry_deregister_nonexistent() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        assert!(registry.deregister(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_service_registry_duplicate_register() {
        let registry = ServiceRegistry::new(DiscoveryConfig::default());
        let service = make_service("agent", vec!["training"]);
        let id = service.id;
        registry.register(service.clone()).unwrap();
        registry.register(service).unwrap(); // same ID re-registers
        assert_eq!(registry.len(), 1);

        let found = registry.get(id);
        assert!(found.is_some());
    }

    #[test]
    fn test_gossip_duplicate_announce() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);

        let remote = make_service("remote", vec!["inference"]);
        gossip.process_message(GossipMessage::Announce(Box::new(remote.clone())));
        gossip.process_message(GossipMessage::Announce(Box::new(remote)));
        // Should still be 1 peer (update, not duplicate)
        assert_eq!(gossip.peer_count(), 1);
    }

    #[test]
    fn test_gossip_leave_nonexistent() {
        let local = make_service("local", vec!["training"]);
        let gossip = GossipState::new(local, 3);
        // Leaving a peer that was never added should not panic
        gossip.process_message(GossipMessage::Leave(Uuid::new_v4()));
        assert_eq!(gossip.peer_count(), 0);
    }
}
