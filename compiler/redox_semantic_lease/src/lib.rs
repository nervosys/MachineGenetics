// Semantic Lease Manager for the Redox compiler (REDOX_PROPOSAL.md §7.1).
//
// Semantic leases grant shared-read or exclusive-write access to code regions
// (functions, impls, modules, traits, types, crate interfaces). This module
// implements lease acquisition, release, timeout, and deadlock detection.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

// ── Identifiers ────────────────────────────────────────────────────────────

/// Identifies an agent in the swarm.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(name: &str) -> Self {
        AgentId(name.to_string())
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A semantic region — the unit of agent ownership.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticRegion {
    Function(u64),
    Impl(u64),
    Module(u64),
    TraitDef(u64),
    TypeDef(u64),
    CrateInterface,
}

impl std::fmt::Display for SemanticRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticRegion::Function(id) => write!(f, "fn#{id}"),
            SemanticRegion::Impl(id) => write!(f, "impl#{id}"),
            SemanticRegion::Module(id) => write!(f, "mod#{id}"),
            SemanticRegion::TraitDef(id) => write!(f, "trait#{id}"),
            SemanticRegion::TypeDef(id) => write!(f, "type#{id}"),
            SemanticRegion::CrateInterface => write!(f, "crate_interface"),
        }
    }
}

// ── Lease Types ────────────────────────────────────────────────────────────

/// A version snapshot for exclusive-write leases.
pub type Version = u64;

/// The kind of lease held on a region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseKind {
    SharedRead,
    ExclusiveWrite,
    Restructuring,
}

/// A lease record kept by the manager.
#[derive(Debug, Clone)]
pub struct Lease {
    pub region: SemanticRegion,
    pub kind: LeaseKind,
    pub holder: AgentId,
    pub acquired_at: Instant,
    pub version: Version,
}

/// Errors from lease operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseError {
    /// Region is exclusively held by another agent.
    RegionLocked { region: SemanticRegion, holder: AgentId },
    /// Agent does not hold a lease on this region.
    NotHeld { region: SemanticRegion, agent: AgentId },
    /// Acquiring would create a deadlock.
    DeadlockDetected { cycle: Vec<AgentId> },
    /// Lease request timed out (for queue-based waiting).
    Timeout { region: SemanticRegion },
    /// Cannot upgrade: region has other shared readers.
    UpgradeBlocked { region: SemanticRegion, readers: Vec<AgentId> },
}

impl std::fmt::Display for LeaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LeaseError::RegionLocked { region, holder } =>
                write!(f, "region {region} locked by {holder}"),
            LeaseError::NotHeld { region, agent } =>
                write!(f, "agent {agent} does not hold lease on {region}"),
            LeaseError::DeadlockDetected { cycle } => {
                let names: Vec<&str> = cycle.iter().map(|a| a.0.as_str()).collect();
                write!(f, "deadlock detected: {}", names.join(" -> "))
            }
            LeaseError::Timeout { region } =>
                write!(f, "timeout waiting for lease on {region}"),
            LeaseError::UpgradeBlocked { region, readers } => {
                let names: Vec<&str> = readers.iter().map(|a| a.0.as_str()).collect();
                write!(f, "cannot upgrade {region}: readers {}", names.join(", "))
            }
        }
    }
}

// ── Lease Manager ──────────────────────────────────────────────────────────

/// The semantic lease manager (proposal §7.1).
///
/// Manages shared-read and exclusive-write access to semantic regions.
pub struct LeaseManager {
    /// Active leases by region.
    leases: BTreeMap<SemanticRegion, Vec<Lease>>,
    /// Current version per region (monotonically increasing).
    versions: BTreeMap<SemanticRegion, Version>,
    /// Wait queue: agent → set of regions it is waiting for.
    wait_queue: BTreeMap<AgentId, BTreeSet<SemanticRegion>>,
    /// Lease timeout duration.
    timeout: Duration,
    /// Audit log of operations.
    audit_log: Vec<AuditEntry>,
}

/// An entry in the append-only audit log.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: Instant,
    pub agent: AgentId,
    pub region: SemanticRegion,
    pub action: AuditAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditAction {
    AcquireSharedRead,
    AcquireExclusiveWrite,
    AcquireRestructuring,
    Release,
    Revoked,
    UpgradeToWrite,
    DowngradeToRead,
}

impl LeaseManager {
    /// Create a new lease manager with the given timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            leases: BTreeMap::new(),
            versions: BTreeMap::new(),
            wait_queue: BTreeMap::new(),
            timeout,
            audit_log: Vec::new(),
        }
    }

    /// Create a lease manager with the default 5-minute timeout.
    pub fn with_default_timeout() -> Self {
        Self::new(Duration::from_secs(300))
    }

    /// Acquire a shared-read lease on a region.
    pub fn acquire_shared_read(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<Version, LeaseError> {
        // Check for exclusive lock by another agent
        if let Some(leases) = self.leases.get(region) {
            for lease in leases {
                if (lease.kind == LeaseKind::ExclusiveWrite
                    || lease.kind == LeaseKind::Restructuring)
                    && lease.holder != *agent
                {
                    return Err(LeaseError::RegionLocked {
                        region: region.clone(),
                        holder: lease.holder.clone(),
                    });
                }
            }
        }

        let version = self.current_version(region);
        let lease = Lease {
            region: region.clone(),
            kind: LeaseKind::SharedRead,
            holder: agent.clone(),
            acquired_at: Instant::now(),
            version,
        };

        self.leases.entry(region.clone()).or_default().push(lease);
        self.log(agent, region, AuditAction::AcquireSharedRead);
        Ok(version)
    }

    /// Acquire an exclusive-write lease on a region.
    pub fn acquire_exclusive_write(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<Version, LeaseError> {
        // Deadlock detection before acquisition
        self.check_deadlock(agent, region)?;

        if let Some(leases) = self.leases.get(region) {
            for lease in leases {
                if lease.holder != *agent {
                    return Err(LeaseError::RegionLocked {
                        region: region.clone(),
                        holder: lease.holder.clone(),
                    });
                }
            }
        }

        let version = self.bump_version(region);
        let lease = Lease {
            region: region.clone(),
            kind: LeaseKind::ExclusiveWrite,
            holder: agent.clone(),
            acquired_at: Instant::now(),
            version,
        };

        self.leases.entry(region.clone()).or_default().push(lease);
        self.log(agent, region, AuditAction::AcquireExclusiveWrite);
        Ok(version)
    }

    /// Acquire a restructuring lease (exclusive + notifies dependents).
    pub fn acquire_restructuring(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<Version, LeaseError> {
        self.check_deadlock(agent, region)?;

        if let Some(leases) = self.leases.get(region) {
            for lease in leases {
                if lease.holder != *agent {
                    return Err(LeaseError::RegionLocked {
                        region: region.clone(),
                        holder: lease.holder.clone(),
                    });
                }
            }
        }

        let version = self.bump_version(region);
        let lease = Lease {
            region: region.clone(),
            kind: LeaseKind::Restructuring,
            holder: agent.clone(),
            acquired_at: Instant::now(),
            version,
        };

        self.leases.entry(region.clone()).or_default().push(lease);
        self.log(agent, region, AuditAction::AcquireRestructuring);
        Ok(version)
    }

    /// Release all leases an agent holds on a region.
    pub fn release(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<(), LeaseError> {
        if let Some(leases) = self.leases.get_mut(region) {
            let before = leases.len();
            leases.retain(|l| l.holder != *agent);
            if leases.len() == before {
                return Err(LeaseError::NotHeld {
                    region: region.clone(),
                    agent: agent.clone(),
                });
            }
            if leases.is_empty() {
                self.leases.remove(region);
            }
            self.wait_queue.entry(agent.clone()).or_default().remove(region);
            self.log(agent, region, AuditAction::Release);
            Ok(())
        } else {
            Err(LeaseError::NotHeld {
                region: region.clone(),
                agent: agent.clone(),
            })
        }
    }

    /// Release all leases held by an agent across all regions.
    pub fn release_all(&mut self, agent: &AgentId) {
        let regions: Vec<SemanticRegion> = self.leases.keys().cloned().collect();
        for region in regions {
            let _ = self.release(agent, &region);
        }
        self.wait_queue.remove(agent);
    }

    /// Upgrade a shared-read lease to exclusive-write.
    pub fn upgrade_to_write(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<Version, LeaseError> {
        if let Some(leases) = self.leases.get(region) {
            // Check we hold a read lease
            let holds_read = leases.iter().any(|l| l.holder == *agent && l.kind == LeaseKind::SharedRead);
            if !holds_read {
                return Err(LeaseError::NotHeld {
                    region: region.clone(),
                    agent: agent.clone(),
                });
            }
            // Check no other holders
            let other_holders: Vec<AgentId> = leases.iter()
                .filter(|l| l.holder != *agent)
                .map(|l| l.holder.clone())
                .collect();
            if !other_holders.is_empty() {
                return Err(LeaseError::UpgradeBlocked {
                    region: region.clone(),
                    readers: other_holders,
                });
            }
        } else {
            return Err(LeaseError::NotHeld {
                region: region.clone(),
                agent: agent.clone(),
            });
        }

        // Remove old read lease, add write lease
        if let Some(leases) = self.leases.get_mut(region) {
            leases.retain(|l| !(l.holder == *agent && l.kind == LeaseKind::SharedRead));
        }
        let version = self.bump_version(region);
        let lease = Lease {
            region: region.clone(),
            kind: LeaseKind::ExclusiveWrite,
            holder: agent.clone(),
            acquired_at: Instant::now(),
            version,
        };
        self.leases.entry(region.clone()).or_default().push(lease);
        self.log(agent, region, AuditAction::UpgradeToWrite);
        Ok(version)
    }

    /// Downgrade an exclusive-write lease to shared-read.
    pub fn downgrade_to_read(
        &mut self,
        agent: &AgentId,
        region: &SemanticRegion,
    ) -> Result<Version, LeaseError> {
        if let Some(leases) = self.leases.get(region) {
            let holds_write = leases.iter().any(|l| {
                l.holder == *agent
                    && (l.kind == LeaseKind::ExclusiveWrite || l.kind == LeaseKind::Restructuring)
            });
            if !holds_write {
                return Err(LeaseError::NotHeld {
                    region: region.clone(),
                    agent: agent.clone(),
                });
            }
        } else {
            return Err(LeaseError::NotHeld {
                region: region.clone(),
                agent: agent.clone(),
            });
        }

        if let Some(leases) = self.leases.get_mut(region) {
            leases.retain(|l| !(l.holder == *agent && (l.kind == LeaseKind::ExclusiveWrite || l.kind == LeaseKind::Restructuring)));
        }
        let version = self.current_version(region);
        let lease = Lease {
            region: region.clone(),
            kind: LeaseKind::SharedRead,
            holder: agent.clone(),
            acquired_at: Instant::now(),
            version,
        };
        self.leases.entry(region.clone()).or_default().push(lease);
        self.log(agent, region, AuditAction::DowngradeToRead);
        Ok(version)
    }

    /// Revoke all expired leases. Returns list of revoked (agent, region) pairs.
    pub fn revoke_expired(&mut self) -> Vec<(AgentId, SemanticRegion)> {
        let now = Instant::now();
        let mut revoked = Vec::new();

        let regions: Vec<SemanticRegion> = self.leases.keys().cloned().collect();
        for region in regions {
            if let Some(leases) = self.leases.get_mut(&region) {
                let expired: Vec<AgentId> = leases
                    .iter()
                    .filter(|l| now.duration_since(l.acquired_at) > self.timeout)
                    .map(|l| l.holder.clone())
                    .collect();
                for agent in &expired {
                    self.audit_log.push(AuditEntry {
                        timestamp: now,
                        agent: agent.clone(),
                        region: region.clone(),
                        action: AuditAction::Revoked,
                    });
                    revoked.push((agent.clone(), region.clone()));
                }
                leases.retain(|l| now.duration_since(l.acquired_at) <= self.timeout);
                if leases.is_empty() {
                    self.leases.remove(&region);
                }
            }
        }

        revoked
    }

    // ── Deadlock detection ──

    /// Check if acquiring a lease would create a deadlock (cycle in wait-for graph).
    fn check_deadlock(
        &self,
        requesting_agent: &AgentId,
        target_region: &SemanticRegion,
    ) -> Result<(), LeaseError> {
        // Build wait-for graph: agent A waits for agent B if B holds a region A wants
        // Check if granting this request creates a cycle

        // Who holds the target region?
        let holders: Vec<AgentId> = self.leases.get(target_region)
            .map(|ls| ls.iter().map(|l| l.holder.clone()).collect())
            .unwrap_or_default();

        if holders.is_empty() {
            return Ok(()); // No holders, no deadlock
        }

        // BFS from each holder to see if we can reach requesting_agent
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        let mut path: BTreeMap<AgentId, AgentId> = BTreeMap::new();

        for holder in &holders {
            if holder == requesting_agent {
                continue; // already holds it
            }
            queue.push_back(holder.clone());
            visited.insert(holder.clone());
        }

        while let Some(current) = queue.pop_front() {
            // What regions does `current` want?
            if let Some(wanted_regions) = self.wait_queue.get(&current) {
                for wanted in wanted_regions {
                    // Who holds those regions?
                    if let Some(ls) = self.leases.get(wanted) {
                        for lease in ls {
                            if lease.holder == *requesting_agent {
                                // Found cycle: requesting_agent -> [holders] -> ... -> requesting_agent
                                let mut cycle = vec![requesting_agent.clone()];
                                let mut trace = current.clone();
                                cycle.push(trace.clone());
                                while let Some(prev) = path.get(&trace) {
                                    cycle.push(prev.clone());
                                    trace = prev.clone();
                                }
                                cycle.push(requesting_agent.clone());
                                cycle.reverse();
                                return Err(LeaseError::DeadlockDetected { cycle });
                            }
                            if !visited.contains(&lease.holder) {
                                visited.insert(lease.holder.clone());
                                path.insert(lease.holder.clone(), current.clone());
                                queue.push_back(lease.holder.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Register that an agent is waiting for a region (used for deadlock detection).
    pub fn register_wait(&mut self, agent: &AgentId, region: &SemanticRegion) {
        self.wait_queue.entry(agent.clone()).or_default().insert(region.clone());
    }

    /// Unregister wait (call after successful acquisition or cancellation).
    pub fn unregister_wait(&mut self, agent: &AgentId, region: &SemanticRegion) {
        if let Some(set) = self.wait_queue.get_mut(agent) {
            set.remove(region);
            if set.is_empty() {
                self.wait_queue.remove(agent);
            }
        }
    }

    // ── Query methods ──

    /// Check if a region is available for shared read.
    pub fn is_readable(&self, region: &SemanticRegion) -> bool {
        if let Some(leases) = self.leases.get(region) {
            !leases.iter().any(|l| l.kind == LeaseKind::ExclusiveWrite || l.kind == LeaseKind::Restructuring)
        } else {
            true
        }
    }

    /// Check if a region is available for exclusive write.
    pub fn is_writable(&self, region: &SemanticRegion) -> bool {
        !self.leases.contains_key(region)
    }

    /// Get all lease holders for a region.
    pub fn holders(&self, region: &SemanticRegion) -> Vec<(AgentId, LeaseKind)> {
        self.leases.get(region)
            .map(|ls| ls.iter().map(|l| (l.holder.clone(), l.kind.clone())).collect())
            .unwrap_or_default()
    }

    /// Get all regions held by an agent.
    pub fn regions_held_by(&self, agent: &AgentId) -> Vec<(SemanticRegion, LeaseKind)> {
        let mut result = Vec::new();
        for (region, leases) in &self.leases {
            for lease in leases {
                if lease.holder == *agent {
                    result.push((region.clone(), lease.kind.clone()));
                }
            }
        }
        result
    }

    /// Get the number of active leases total.
    pub fn active_lease_count(&self) -> usize {
        self.leases.values().map(|ls| ls.len()).sum()
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Get the timeout duration.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    // ── Internal helpers ──

    fn current_version(&mut self, region: &SemanticRegion) -> Version {
        *self.versions.entry(region.clone()).or_insert(0)
    }

    fn bump_version(&mut self, region: &SemanticRegion) -> Version {
        let v = self.versions.entry(region.clone()).or_insert(0);
        *v += 1;
        *v
    }

    fn log(&mut self, agent: &AgentId, region: &SemanticRegion, action: AuditAction) {
        self.audit_log.push(AuditEntry {
            timestamp: Instant::now(),
            agent: agent.clone(),
            region: region.clone(),
            action,
        });
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> LeaseManager {
        LeaseManager::new(Duration::from_secs(60))
    }

    fn agent(name: &str) -> AgentId {
        AgentId::new(name)
    }

    fn fn_region(id: u64) -> SemanticRegion {
        SemanticRegion::Function(id)
    }

    // ── Basic acquisition ──

    #[test]
    fn test_acquire_shared_read() {
        let mut m = mgr();
        let v = m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        assert_eq!(v, 0);
        assert_eq!(m.active_lease_count(), 1);
    }

    #[test]
    fn test_multiple_shared_readers() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap();
        m.acquire_shared_read(&agent("c"), &fn_region(1)).unwrap();
        assert_eq!(m.active_lease_count(), 3);
    }

    #[test]
    fn test_acquire_exclusive_write() {
        let mut m = mgr();
        let v = m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        assert_eq!(v, 1); // version bumped
        assert_eq!(m.active_lease_count(), 1);
    }

    #[test]
    fn test_exclusive_blocks_shared() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        let err = m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::RegionLocked { .. }));
    }

    #[test]
    fn test_shared_blocks_exclusive() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        let err = m.acquire_exclusive_write(&agent("b"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::RegionLocked { .. }));
    }

    #[test]
    fn test_exclusive_blocks_exclusive() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        let err = m.acquire_exclusive_write(&agent("b"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::RegionLocked { .. }));
    }

    // ── Release ──

    #[test]
    fn test_release() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.release(&agent("a"), &fn_region(1)).unwrap();
        assert_eq!(m.active_lease_count(), 0);
    }

    #[test]
    fn test_release_then_acquire() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.release(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_exclusive_write(&agent("b"), &fn_region(1)).unwrap();
        assert_eq!(m.active_lease_count(), 1);
    }

    #[test]
    fn test_release_not_held() {
        let mut m = mgr();
        let err = m.release(&agent("a"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::NotHeld { .. }));
    }

    #[test]
    fn test_release_all() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_shared_read(&agent("a"), &fn_region(2)).unwrap();
        m.acquire_exclusive_write(&agent("a"), &fn_region(3)).unwrap();
        assert_eq!(m.active_lease_count(), 3);
        m.release_all(&agent("a"));
        assert_eq!(m.active_lease_count(), 0);
    }

    // ── Upgrade / Downgrade ──

    #[test]
    fn test_upgrade_to_write() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        let v = m.upgrade_to_write(&agent("a"), &fn_region(1)).unwrap();
        assert_eq!(v, 1);
        // Now holds exclusive write, not shared read
        let held = m.regions_held_by(&agent("a"));
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].1, LeaseKind::ExclusiveWrite);
    }

    #[test]
    fn test_upgrade_blocked_by_other_reader() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap();
        let err = m.upgrade_to_write(&agent("a"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::UpgradeBlocked { .. }));
    }

    #[test]
    fn test_downgrade_to_read() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        let v = m.downgrade_to_read(&agent("a"), &fn_region(1)).unwrap();
        assert!(v > 0);
        let held = m.regions_held_by(&agent("a"));
        assert_eq!(held[0].1, LeaseKind::SharedRead);
        // Now another agent can read too
        m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap();
        assert_eq!(m.active_lease_count(), 2);
    }

    // ── Restructuring ──

    #[test]
    fn test_restructuring_lease() {
        let mut m = mgr();
        let v = m.acquire_restructuring(&agent("a"), &fn_region(1)).unwrap();
        assert_eq!(v, 1);
        // Blocks reads
        let err = m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::RegionLocked { .. }));
    }

    // ── Query methods ──

    #[test]
    fn test_is_readable_writable() {
        let mut m = mgr();
        assert!(m.is_readable(&fn_region(1)));
        assert!(m.is_writable(&fn_region(1)));

        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        assert!(m.is_readable(&fn_region(1)));
        assert!(!m.is_writable(&fn_region(1)));
    }

    #[test]
    fn test_holders() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_shared_read(&agent("b"), &fn_region(1)).unwrap();
        let holders = m.holders(&fn_region(1));
        assert_eq!(holders.len(), 2);
    }

    #[test]
    fn test_regions_held_by() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_exclusive_write(&agent("a"), &fn_region(2)).unwrap();
        let held = m.regions_held_by(&agent("a"));
        assert_eq!(held.len(), 2);
    }

    // ── Timeout ──

    #[test]
    fn test_revoke_expired() {
        let mut m = LeaseManager::new(Duration::from_millis(0)); // instant timeout
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        // Sleep is not needed since timeout is 0ms — already expired
        std::thread::sleep(Duration::from_millis(1));
        let revoked = m.revoke_expired();
        assert_eq!(revoked.len(), 1);
        assert_eq!(revoked[0].0, agent("a"));
        assert_eq!(m.active_lease_count(), 0);
    }

    #[test]
    fn test_no_revoke_within_timeout() {
        let mut m = LeaseManager::new(Duration::from_secs(3600));
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        let revoked = m.revoke_expired();
        assert_eq!(revoked.len(), 0);
        assert_eq!(m.active_lease_count(), 1);
    }

    // ── Deadlock detection ──

    #[test]
    fn test_deadlock_simple_cycle() {
        let mut m = mgr();
        // Agent A holds fn#1, Agent B holds fn#2
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_exclusive_write(&agent("b"), &fn_region(2)).unwrap();
        // Agent A wants fn#2 (register wait)
        m.register_wait(&agent("a"), &fn_region(2));
        // Agent B wants fn#1 → deadlock
        let err = m.acquire_exclusive_write(&agent("b"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::DeadlockDetected { .. }));
    }

    #[test]
    fn test_no_deadlock_no_cycle() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        // Agent B wants fn#2 (free) — no deadlock
        let result = m.acquire_exclusive_write(&agent("b"), &fn_region(2));
        assert!(result.is_ok());
    }

    #[test]
    fn test_deadlock_three_agent_cycle() {
        let mut m = mgr();
        // A holds 1, B holds 2, C holds 3
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_exclusive_write(&agent("b"), &fn_region(2)).unwrap();
        m.acquire_exclusive_write(&agent("c"), &fn_region(3)).unwrap();
        // A waits for 2, B waits for 3
        m.register_wait(&agent("a"), &fn_region(2));
        m.register_wait(&agent("b"), &fn_region(3));
        // C wants 1 → cycle: C -> A -> B -> C
        let err = m.acquire_exclusive_write(&agent("c"), &fn_region(1)).unwrap_err();
        assert!(matches!(err, LeaseError::DeadlockDetected { .. }));
    }

    // ── Multi-region concurrent access ──

    #[test]
    fn test_independent_regions() {
        let mut m = mgr();
        m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.acquire_exclusive_write(&agent("b"), &fn_region(2)).unwrap();
        m.acquire_exclusive_write(&agent("c"), &fn_region(3)).unwrap();
        assert_eq!(m.active_lease_count(), 3);
    }

    #[test]
    fn test_mixed_region_types() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &SemanticRegion::Module(1)).unwrap();
        m.acquire_exclusive_write(&agent("b"), &SemanticRegion::Impl(2)).unwrap();
        m.acquire_shared_read(&agent("c"), &SemanticRegion::TraitDef(3)).unwrap();
        m.acquire_restructuring(&agent("d"), &SemanticRegion::TypeDef(4)).unwrap();
        assert_eq!(m.active_lease_count(), 4);
    }

    #[test]
    fn test_crate_interface_shared() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &SemanticRegion::CrateInterface).unwrap();
        m.acquire_shared_read(&agent("b"), &SemanticRegion::CrateInterface).unwrap();
        assert_eq!(m.holders(&SemanticRegion::CrateInterface).len(), 2);
    }

    // ── Audit log ──

    #[test]
    fn test_audit_log() {
        let mut m = mgr();
        m.acquire_shared_read(&agent("a"), &fn_region(1)).unwrap();
        m.release(&agent("a"), &fn_region(1)).unwrap();
        let log = m.audit_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].action, AuditAction::AcquireSharedRead);
        assert_eq!(log[1].action, AuditAction::Release);
    }

    // ── Version tracking ──

    #[test]
    fn test_version_bumps() {
        let mut m = mgr();
        let v1 = m.acquire_exclusive_write(&agent("a"), &fn_region(1)).unwrap();
        m.release(&agent("a"), &fn_region(1)).unwrap();
        let v2 = m.acquire_exclusive_write(&agent("b"), &fn_region(1)).unwrap();
        assert!(v2 > v1);
    }

    // ── Display ──

    #[test]
    fn test_region_display() {
        assert_eq!(SemanticRegion::Function(42).to_string(), "fn#42");
        assert_eq!(SemanticRegion::Module(1).to_string(), "mod#1");
        assert_eq!(SemanticRegion::CrateInterface.to_string(), "crate_interface");
    }

    #[test]
    fn test_error_display() {
        let err = LeaseError::RegionLocked {
            region: fn_region(1),
            holder: agent("a"),
        };
        assert!(err.to_string().contains("locked by a"));
    }

    #[test]
    fn test_default_timeout() {
        let m = LeaseManager::with_default_timeout();
        assert_eq!(m.timeout(), Duration::from_secs(300));
    }
}
