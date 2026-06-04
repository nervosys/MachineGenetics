//! Federation - Cross-Cluster Agent Communication
//!
//! Provides hierarchical agent organization and cross-cluster communication:
//!
//! - **Cluster Management**: Logical grouping of agents
//! - **Hierarchical Organization**: Parent-child cluster relationships
//! - **Resource Sharing**: Policies for cross-cluster resource access
//! - **Gateway**: Cross-cluster message routing

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Cluster
// ============================================================================

/// Cluster status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClusterStatus {
    /// Cluster is forming
    Forming,
    /// Cluster is healthy and operational
    Healthy,
    /// Cluster has degraded capacity
    Degraded,
    /// Cluster is partitioned
    Partitioned,
    /// Cluster is shutting down
    ShuttingDown,
}

/// A logical cluster of agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Cluster ID
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Cluster region/location
    pub region: String,
    /// Member agent IDs
    pub members: HashSet<Uuid>,
    /// Cluster status
    pub status: ClusterStatus,
    /// Parent cluster (for hierarchy)
    pub parent: Option<Uuid>,
    /// Child clusters
    pub children: HashSet<Uuid>,
    /// Cluster capacity
    pub capacity: ClusterCapacity,
    /// Resource sharing policy
    pub sharing_policy: ResourceSharingPolicy,
    /// Gateway nodes (entry points for cross-cluster communication)
    pub gateways: Vec<Uuid>,
    /// Created timestamp
    pub created_at: f64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Cluster {
    /// Create a new cluster.
    pub fn new(name: &str, region: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            region: region.to_string(),
            members: HashSet::new(),
            status: ClusterStatus::Forming,
            parent: None,
            children: HashSet::new(),
            capacity: ClusterCapacity::default(),
            sharing_policy: ResourceSharingPolicy::default(),
            gateways: Vec::new(),
            created_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Add a member agent.
    pub fn add_member(&mut self, agent_id: Uuid) -> bool {
        if self.members.len() >= self.capacity.max_agents {
            return false;
        }
        self.members.insert(agent_id);
        if self.status == ClusterStatus::Forming && !self.members.is_empty() {
            self.status = ClusterStatus::Healthy;
        }
        true
    }

    /// Remove a member agent.
    pub fn remove_member(&mut self, agent_id: &Uuid) -> bool {
        let removed = self.members.remove(agent_id);
        self.gateways.retain(|g| g != agent_id);
        removed
    }

    /// Add a child cluster.
    pub fn add_child(&mut self, cluster_id: Uuid) {
        self.children.insert(cluster_id);
    }

    /// Remove a child cluster.
    pub fn remove_child(&mut self, cluster_id: &Uuid) {
        self.children.remove(cluster_id);
    }

    /// Set a gateway node.
    pub fn add_gateway(&mut self, agent_id: Uuid) {
        if self.members.contains(&agent_id) && !self.gateways.contains(&agent_id) {
            self.gateways.push(agent_id);
        }
    }

    /// Get member count.
    #[inline]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if an agent is a member.
    #[inline]
    pub fn has_member(&self, agent_id: &Uuid) -> bool {
        self.members.contains(agent_id)
    }

    /// Get utilization ratio.
    pub fn utilization(&self) -> f64 {
        if self.capacity.max_agents == 0 {
            return 0.0;
        }
        self.members.len() as f64 / self.capacity.max_agents as f64
    }
}

/// Cluster capacity limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterCapacity {
    /// Maximum number of agents
    pub max_agents: usize,
    /// Total compute units available
    pub compute_units: f64,
    /// Total memory in MB
    pub memory_mb: u64,
    /// Total storage in MB  
    pub storage_mb: u64,
    /// Network bandwidth in Mbps
    pub bandwidth_mbps: f64,
}

impl Default for ClusterCapacity {
    fn default() -> Self {
        Self {
            max_agents: 256,
            compute_units: 1000.0,
            memory_mb: 65536,
            storage_mb: 1048576,
            bandwidth_mbps: 10000.0,
        }
    }
}

// ============================================================================
// Resource Sharing
// ============================================================================

/// Policy for sharing resources across clusters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSharingPolicy {
    /// Allow sharing with other clusters
    pub enabled: bool,
    /// Maximum percentage of resources to share (0.0 - 1.0)
    pub max_share_ratio: f64,
    /// Clusters allowed to request resources
    pub allowed_clusters: Option<HashSet<Uuid>>,
    /// Clusters explicitly blocked
    pub blocked_clusters: HashSet<Uuid>,
    /// Priority when multiple requests compete
    pub priority: SharePriority,
    /// Billing/accounting mode
    pub accounting: AccountingMode,
    /// Rate limit (max requests per minute)
    pub rate_limit: Option<u32>,
}

impl Default for ResourceSharingPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_share_ratio: 0.5,
            allowed_clusters: None, // All allowed by default
            blocked_clusters: HashSet::new(),
            priority: SharePriority::FairShare,
            accounting: AccountingMode::BestEffort,
            rate_limit: None,
        }
    }
}

/// Priority scheme for resource sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SharePriority {
    /// All clusters get equal access
    FairShare,
    /// Local cluster always prioritized
    LocalFirst,
    /// Parent cluster prioritized
    HierarchyBased,
    /// Based on explicit priority values
    Weighted,
}

/// Accounting mode for cross-cluster resource usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccountingMode {
    /// No tracking
    BestEffort,
    /// Track usage for billing
    Metered,
    /// Tit-for-tat reciprocal
    Reciprocal,
}

/// A resource request from one cluster to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    /// Request ID
    pub id: Uuid,
    /// Requesting cluster
    pub from_cluster: Uuid,
    /// Target cluster
    pub to_cluster: Uuid,
    /// Requested resource type
    pub resource_type: ResourceType,
    /// Requested amount
    pub amount: f64,
    /// Request priority
    pub priority: u8,
    /// Duration needed
    pub duration: Duration,
    /// Status
    pub status: RequestStatus,
    /// Timestamp
    pub created_at: f64,
}

/// Types of resources that can be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Compute cycles
    Compute,
    /// Memory
    Memory,
    /// Storage
    Storage,
    /// Network bandwidth
    Bandwidth,
    /// Agent slots
    AgentSlots,
}

/// Status of a resource request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequestStatus {
    /// Pending review
    Pending,
    /// Approved and active
    Approved,
    /// Denied
    Denied,
    /// Completed (resource returned)
    Completed,
    /// Expired
    Expired,
}

impl ResourceRequest {
    /// Create a new resource request.
    pub fn new(
        from_cluster: Uuid,
        to_cluster: Uuid,
        resource_type: ResourceType,
        amount: f64,
        duration: Duration,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            from_cluster,
            to_cluster,
            resource_type,
            amount,
            priority: 5,
            duration,
            status: RequestStatus::Pending,
            created_at: now,
        }
    }
}

// ============================================================================
// Federation Gateway
// ============================================================================

/// Cross-cluster message routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMessage {
    /// Message ID
    pub id: Uuid,
    /// Source cluster
    pub source_cluster: Uuid,
    /// Destination cluster
    pub dest_cluster: Uuid,
    /// Source agent
    pub source_agent: Uuid,
    /// Destination agent (or broadcast)
    pub dest_agent: Option<Uuid>,
    /// Message type
    pub msg_type: FederationMessageType,
    /// Payload
    pub payload: Vec<u8>,
    /// TTL (hop count)
    pub ttl: u8,
    /// Timestamp
    pub timestamp: f64,
}

/// Types of federation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FederationMessageType {
    /// Task delegation
    TaskDelegation,
    /// Result return
    ResultReturn,
    /// Resource request
    ResourceRequest,
    /// Resource response
    ResourceResponse,
    /// Cluster status update
    StatusUpdate,
    /// Agent migration
    AgentMigration,
    /// Knowledge sync
    KnowledgeSync,
    /// Heartbeat
    Heartbeat,
}

/// Gateway for cross-cluster communication.
pub struct FederationGateway {
    /// Routing table: dest_cluster -> gateway_node
    routes: RwLock<HashMap<Uuid, Vec<Uuid>>>,
    /// Message queue for outbound messages
    outbound: RwLock<VecDeque<FederationMessage>>,
    /// Message queue for inbound messages
    inbound: RwLock<VecDeque<FederationMessage>>,
    /// Messages forwarded count
    forwarded: std::sync::atomic::AtomicU64,
    /// Maximum outbound queue size
    max_queue_size: usize,
}

impl FederationGateway {
    /// Create a new federation gateway.
    pub fn new(_local_cluster: Uuid) -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            outbound: RwLock::new(VecDeque::new()),
            inbound: RwLock::new(VecDeque::new()),
            forwarded: std::sync::atomic::AtomicU64::new(0),
            max_queue_size: 10_000,
        }
    }

    /// Add a route to a remote cluster.
    pub fn add_route(&self, dest_cluster: Uuid, gateway_nodes: Vec<Uuid>) {
        self.routes
            .write()
            .unwrap()
            .insert(dest_cluster, gateway_nodes);
    }

    /// Remove a route.
    pub fn remove_route(&self, dest_cluster: Uuid) {
        self.routes.write().unwrap().remove(&dest_cluster);
    }

    /// Get route to a cluster.
    pub fn get_route(&self, dest_cluster: Uuid) -> Option<Vec<Uuid>> {
        self.routes.read().unwrap().get(&dest_cluster).cloned()
    }

    /// Enqueue a message for sending.
    pub fn send(&self, msg: FederationMessage) -> Result<()> {
        let mut outbound = self.outbound.write().unwrap();
        if outbound.len() >= self.max_queue_size {
            return Err(RmiError::ResourceExhausted(
                "Federation outbound queue full".to_string(),
            ));
        }
        outbound.push_back(msg);
        Ok(())
    }

    /// Receive an inbound message.
    pub fn receive(&self, msg: FederationMessage) {
        if msg.ttl == 0 {
            return; // Drop expired messages
        }

        let mut inbound = self.inbound.write().unwrap();
        if inbound.len() < self.max_queue_size {
            inbound.push_back(msg);
        }
    }

    /// Take next outbound message.
    pub fn next_outbound(&self) -> Option<FederationMessage> {
        self.outbound.write().unwrap().pop_front()
    }

    /// Take next inbound message.
    pub fn next_inbound(&self) -> Option<FederationMessage> {
        self.inbound.write().unwrap().pop_front()
    }

    /// Forward a message (decrement TTL and route).
    pub fn forward(&self, mut msg: FederationMessage) -> Result<()> {
        if msg.ttl == 0 {
            return Err(RmiError::Protocol("Message TTL expired".to_string()));
        }
        msg.ttl -= 1;
        self.forwarded
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.send(msg)
    }

    /// Get pending outbound count.
    pub fn outbound_count(&self) -> usize {
        self.outbound.read().unwrap().len()
    }

    /// Get pending inbound count.
    pub fn inbound_count(&self) -> usize {
        self.inbound.read().unwrap().len()
    }

    /// Get total forwarded count.
    pub fn forwarded_count(&self) -> u64 {
        self.forwarded.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Check if a route exists.
    pub fn has_route(&self, dest_cluster: Uuid) -> bool {
        self.routes.read().unwrap().contains_key(&dest_cluster)
    }

    /// Get all known cluster IDs.
    pub fn known_clusters(&self) -> Vec<Uuid> {
        self.routes.read().unwrap().keys().copied().collect()
    }
}

// ============================================================================
// Federation Manager
// ============================================================================

/// Manages the entire federation topology.
pub struct FederationManager {
    /// All known clusters
    clusters: RwLock<HashMap<Uuid, Cluster>>,
    /// Resource requests
    requests: RwLock<HashMap<Uuid, ResourceRequest>>,
    /// Federation gateway
    pub gateway: FederationGateway,
    /// Local cluster ID
    local_cluster_id: Uuid,
}

impl FederationManager {
    /// Create a new federation manager.
    pub fn new(local_cluster: Cluster) -> Self {
        let id = local_cluster.id;
        let mut clusters = HashMap::new();
        clusters.insert(id, local_cluster);

        Self {
            clusters: RwLock::new(clusters),
            requests: RwLock::new(HashMap::new()),
            gateway: FederationGateway::new(id),
            local_cluster_id: id,
        }
    }

    /// Get local cluster ID.
    #[inline]
    pub fn local_cluster_id(&self) -> Uuid {
        self.local_cluster_id
    }

    /// Register a remote cluster.
    pub fn register_cluster(&self, cluster: Cluster) {
        let id = cluster.id;
        self.clusters.write().unwrap().insert(id, cluster);
    }

    /// Unregister a cluster.
    pub fn unregister_cluster(&self, cluster_id: Uuid) {
        self.clusters.write().unwrap().remove(&cluster_id);
        self.gateway.remove_route(cluster_id);
    }

    /// Get a cluster by ID.
    pub fn get_cluster(&self, cluster_id: Uuid) -> Option<Cluster> {
        self.clusters.read().unwrap().get(&cluster_id).cloned()
    }

    /// Get the local cluster.
    pub fn local_cluster(&self) -> Option<Cluster> {
        self.get_cluster(self.local_cluster_id)
    }

    /// Get all clusters.
    pub fn all_clusters(&self) -> Vec<Cluster> {
        self.clusters.read().unwrap().values().cloned().collect()
    }

    /// Get cluster count.
    pub fn cluster_count(&self) -> usize {
        self.clusters.read().unwrap().len()
    }

    /// Add agent to local cluster.
    pub fn add_local_agent(&self, agent_id: Uuid) -> Result<()> {
        let mut clusters = self.clusters.write().unwrap();
        let cluster = clusters
            .get_mut(&self.local_cluster_id)
            .ok_or_else(|| RmiError::Agent("Local cluster not found".to_string()))?;

        if !cluster.add_member(agent_id) {
            return Err(RmiError::ResourceExhausted(
                "Cluster at capacity".to_string(),
            ));
        }
        Ok(())
    }

    /// Set parent-child relationship between clusters.
    pub fn set_hierarchy(&self, parent_id: Uuid, child_id: Uuid) -> Result<()> {
        let mut clusters = self.clusters.write().unwrap();

        // Update child's parent
        {
            let child = clusters
                .get_mut(&child_id)
                .ok_or_else(|| RmiError::Agent("Child cluster not found".to_string()))?;
            child.parent = Some(parent_id);
        }

        // Update parent's children
        {
            let parent = clusters
                .get_mut(&parent_id)
                .ok_or_else(|| RmiError::Agent("Parent cluster not found".to_string()))?;
            parent.add_child(child_id);
        }

        Ok(())
    }

    /// Submit a resource request.
    pub fn request_resources(&self, request: ResourceRequest) -> Result<Uuid> {
        // Check sharing policy
        let clusters = self.clusters.read().unwrap();
        let target = clusters
            .get(&request.to_cluster)
            .ok_or_else(|| RmiError::Agent("Target cluster not found".to_string()))?;

        if !target.sharing_policy.enabled {
            return Err(RmiError::Agent(
                "Target cluster does not allow sharing".to_string(),
            ));
        }

        if target
            .sharing_policy
            .blocked_clusters
            .contains(&request.from_cluster)
        {
            return Err(RmiError::Agent("Cluster is blocked".to_string()));
        }

        if let Some(ref allowed) = target.sharing_policy.allowed_clusters {
            if !allowed.contains(&request.from_cluster) {
                return Err(RmiError::Agent("Cluster not in allowed list".to_string()));
            }
        }

        drop(clusters);

        let id = request.id;
        self.requests.write().unwrap().insert(id, request);
        Ok(id)
    }

    /// Approve a resource request.
    pub fn approve_request(&self, request_id: Uuid) -> Result<()> {
        let mut requests = self.requests.write().unwrap();
        let request = requests
            .get_mut(&request_id)
            .ok_or_else(|| RmiError::Agent("Request not found".to_string()))?;
        request.status = RequestStatus::Approved;
        Ok(())
    }

    /// Deny a resource request.
    pub fn deny_request(&self, request_id: Uuid) -> Result<()> {
        let mut requests = self.requests.write().unwrap();
        let request = requests
            .get_mut(&request_id)
            .ok_or_else(|| RmiError::Agent("Request not found".to_string()))?;
        request.status = RequestStatus::Denied;
        Ok(())
    }

    /// Get pending resource requests.
    pub fn pending_requests(&self) -> Vec<ResourceRequest> {
        self.requests
            .read()
            .unwrap()
            .values()
            .filter(|r| r.status == RequestStatus::Pending)
            .cloned()
            .collect()
    }

    /// Get federation statistics.
    pub fn stats(&self) -> FederationStats {
        let clusters = self.clusters.read().unwrap();
        let total_agents: usize = clusters.values().map(|c| c.member_count()).sum();
        let requests = self.requests.read().unwrap();

        FederationStats {
            total_clusters: clusters.len(),
            total_agents,
            pending_requests: requests
                .values()
                .filter(|r| r.status == RequestStatus::Pending)
                .count(),
            active_grants: requests
                .values()
                .filter(|r| r.status == RequestStatus::Approved)
                .count(),
            messages_forwarded: self.gateway.forwarded_count(),
        }
    }
}

/// Federation statistics.
#[derive(Debug, Clone, Default)]
pub struct FederationStats {
    /// Total clusters in federation
    pub total_clusters: usize,
    /// Total agents across all clusters
    pub total_agents: usize,
    /// Pending resource requests
    pub pending_requests: usize,
    /// Active resource grants
    pub active_grants: usize,
    /// Total messages forwarded
    pub messages_forwarded: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_creation() {
        let cluster = Cluster::new("training-cluster", "us-west-2");
        assert_eq!(cluster.name, "training-cluster");
        assert_eq!(cluster.region, "us-west-2");
        assert_eq!(cluster.status, ClusterStatus::Forming);
        assert_eq!(cluster.member_count(), 0);
    }

    #[test]
    fn test_cluster_members() {
        let mut cluster = Cluster::new("test", "local");
        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();

        assert!(cluster.add_member(agent1));
        assert_eq!(cluster.status, ClusterStatus::Healthy);
        assert!(cluster.has_member(&agent1));

        assert!(cluster.add_member(agent2));
        assert_eq!(cluster.member_count(), 2);

        cluster.remove_member(&agent1);
        assert!(!cluster.has_member(&agent1));
        assert_eq!(cluster.member_count(), 1);
    }

    #[test]
    fn test_cluster_capacity() {
        let mut cluster = Cluster::new("small", "local");
        cluster.capacity.max_agents = 2;

        assert!(cluster.add_member(Uuid::new_v4()));
        assert!(cluster.add_member(Uuid::new_v4()));
        assert!(!cluster.add_member(Uuid::new_v4())); // At capacity
        assert_eq!(cluster.utilization(), 1.0);
    }

    #[test]
    fn test_cluster_hierarchy() {
        let mut parent = Cluster::new("parent", "us-east");
        let child = Cluster::new("child", "us-west");
        let child_id = child.id;

        parent.add_child(child_id);
        assert!(parent.children.contains(&child_id));
    }

    #[test]
    fn test_cluster_gateway() {
        let mut cluster = Cluster::new("test", "local");
        let agent = Uuid::new_v4();
        cluster.add_member(agent);
        cluster.add_gateway(agent);
        assert_eq!(cluster.gateways.len(), 1);

        // Non-member can't be gateway
        let non_member = Uuid::new_v4();
        cluster.add_gateway(non_member);
        assert_eq!(cluster.gateways.len(), 1);
    }

    #[test]
    fn test_resource_request() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let req = ResourceRequest::new(
            from,
            to,
            ResourceType::Compute,
            100.0,
            Duration::from_secs(3600),
        );

        assert_eq!(req.from_cluster, from);
        assert_eq!(req.to_cluster, to);
        assert_eq!(req.status, RequestStatus::Pending);
    }

    #[test]
    fn test_federation_gateway() {
        let local = Uuid::new_v4();
        let gw = FederationGateway::new(local);

        let remote = Uuid::new_v4();
        gw.add_route(remote, vec![Uuid::new_v4()]);
        assert!(gw.has_route(remote));
        assert_eq!(gw.known_clusters().len(), 1);

        let msg = FederationMessage {
            id: Uuid::new_v4(),
            source_cluster: local,
            dest_cluster: remote,
            source_agent: Uuid::new_v4(),
            dest_agent: None,
            msg_type: FederationMessageType::StatusUpdate,
            payload: vec![1, 2, 3],
            ttl: 5,
            timestamp: 0.0,
        };

        gw.send(msg).unwrap();
        assert_eq!(gw.outbound_count(), 1);

        let outbound = gw.next_outbound().unwrap();
        assert_eq!(outbound.ttl, 5);
    }

    #[test]
    fn test_federation_gateway_forward() {
        let gw = FederationGateway::new(Uuid::new_v4());
        let msg = FederationMessage {
            id: Uuid::new_v4(),
            source_cluster: Uuid::new_v4(),
            dest_cluster: Uuid::new_v4(),
            source_agent: Uuid::new_v4(),
            dest_agent: None,
            msg_type: FederationMessageType::TaskDelegation,
            payload: vec![],
            ttl: 3,
            timestamp: 0.0,
        };

        gw.forward(msg).unwrap();
        let forwarded = gw.next_outbound().unwrap();
        assert_eq!(forwarded.ttl, 2); // Decremented
        assert_eq!(gw.forwarded_count(), 1);
    }

    #[test]
    fn test_federation_gateway_ttl_expired() {
        let gw = FederationGateway::new(Uuid::new_v4());
        let msg = FederationMessage {
            id: Uuid::new_v4(),
            source_cluster: Uuid::new_v4(),
            dest_cluster: Uuid::new_v4(),
            source_agent: Uuid::new_v4(),
            dest_agent: None,
            msg_type: FederationMessageType::TaskDelegation,
            payload: vec![],
            ttl: 0,
            timestamp: 0.0,
        };

        assert!(gw.forward(msg).is_err());
    }

    #[test]
    fn test_federation_manager() {
        let local = Cluster::new("local-cluster", "us-west-2");
        let local_id = local.id;
        let fm = FederationManager::new(local);

        assert_eq!(fm.local_cluster_id(), local_id);
        assert_eq!(fm.cluster_count(), 1);

        // Add the agent
        fm.add_local_agent(Uuid::new_v4()).unwrap();
        let cluster = fm.local_cluster().unwrap();
        assert_eq!(cluster.member_count(), 1);
    }

    #[test]
    fn test_federation_manager_hierarchy() {
        let parent = Cluster::new("parent", "us-east");
        let parent_id = parent.id;
        let child = Cluster::new("child", "us-west");
        let child_id = child.id;

        let fm = FederationManager::new(parent);
        fm.register_cluster(child);

        fm.set_hierarchy(parent_id, child_id).unwrap();

        let p = fm.get_cluster(parent_id).unwrap();
        assert!(p.children.contains(&child_id));

        let c = fm.get_cluster(child_id).unwrap();
        assert_eq!(c.parent, Some(parent_id));
    }

    #[test]
    fn test_federation_resource_request() {
        let cluster1 = Cluster::new("cluster1", "us-east");
        let c1_id = cluster1.id;
        let cluster2 = Cluster::new("cluster2", "us-west");
        let c2_id = cluster2.id;

        let fm = FederationManager::new(cluster1);
        fm.register_cluster(cluster2);

        let req = ResourceRequest::new(
            c1_id,
            c2_id,
            ResourceType::Compute,
            50.0,
            Duration::from_secs(3600),
        );

        let req_id = fm.request_resources(req).unwrap();
        assert_eq!(fm.pending_requests().len(), 1);

        fm.approve_request(req_id).unwrap();
        assert_eq!(fm.pending_requests().len(), 0);
    }

    #[test]
    fn test_federation_blocked_request() {
        let cluster1 = Cluster::new("cluster1", "us-east");
        let c1_id = cluster1.id;
        let mut cluster2 = Cluster::new("cluster2", "us-west");
        let c2_id = cluster2.id;

        // Block cluster1
        cluster2.sharing_policy.blocked_clusters.insert(c1_id);

        let fm = FederationManager::new(cluster1);
        fm.register_cluster(cluster2);

        let req = ResourceRequest::new(
            c1_id,
            c2_id,
            ResourceType::Memory,
            100.0,
            Duration::from_secs(60),
        );

        assert!(fm.request_resources(req).is_err());
    }

    #[test]
    fn test_federation_stats() {
        let cluster = Cluster::new("local", "us-west");
        let fm = FederationManager::new(cluster);

        let stats = fm.stats();
        assert_eq!(stats.total_clusters, 1);
        assert_eq!(stats.total_agents, 0);
        assert_eq!(stats.pending_requests, 0);
    }

    #[test]
    fn test_sharing_policy_disabled() {
        let cluster1 = Cluster::new("cluster1", "us-east");
        let c1_id = cluster1.id;
        let mut cluster2 = Cluster::new("cluster2", "us-west");
        let c2_id = cluster2.id;
        cluster2.sharing_policy.enabled = false;

        let fm = FederationManager::new(cluster1);
        fm.register_cluster(cluster2);

        let req = ResourceRequest::new(
            c1_id,
            c2_id,
            ResourceType::Compute,
            10.0,
            Duration::from_secs(60),
        );
        assert!(fm.request_resources(req).is_err());
    }

    #[test]
    fn test_cluster_status_forming_to_healthy() {
        let mut cluster = Cluster::new("test", "local");
        assert_eq!(cluster.status, ClusterStatus::Forming);
        cluster.add_member(Uuid::new_v4());
        assert_eq!(cluster.status, ClusterStatus::Healthy);
    }

    #[test]
    fn test_cluster_remove_last_member() {
        let mut cluster = Cluster::new("test", "local");
        let agent = Uuid::new_v4();
        cluster.add_member(agent);
        cluster.remove_member(&agent);
        assert_eq!(cluster.member_count(), 0);
    }

    #[test]
    fn test_cluster_duplicate_member() {
        let mut cluster = Cluster::new("test", "local");
        let agent = Uuid::new_v4();
        assert!(cluster.add_member(agent));
        assert!(cluster.add_member(agent)); // idempotent insert
        assert_eq!(cluster.member_count(), 1); // still 1 (HashSet)
    }

    #[test]
    fn test_federation_deny_request() {
        let cluster1 = Cluster::new("c1", "us-east");
        let c1_id = cluster1.id;
        let cluster2 = Cluster::new("c2", "us-west");
        let c2_id = cluster2.id;

        let fm = FederationManager::new(cluster1);
        fm.register_cluster(cluster2);

        let req = ResourceRequest::new(
            c1_id,
            c2_id,
            ResourceType::Compute,
            50.0,
            Duration::from_secs(3600),
        );
        let req_id = fm.request_resources(req).unwrap();
        fm.deny_request(req_id).unwrap();
        assert_eq!(fm.pending_requests().len(), 0);
    }

    #[test]
    fn test_federation_multi_cluster_stats() {
        let c1 = Cluster::new("c1", "us-east");
        let c2 = Cluster::new("c2", "us-west");
        let c3 = Cluster::new("c3", "eu-west");
        let fm = FederationManager::new(c1);
        fm.register_cluster(c2);
        fm.register_cluster(c3);

        let stats = fm.stats();
        assert_eq!(stats.total_clusters, 3);
    }

    #[test]
    fn test_federation_gateway_no_route() {
        let gw = FederationGateway::new(Uuid::new_v4());
        let unknown = Uuid::new_v4();
        assert!(!gw.has_route(unknown));
    }
}
