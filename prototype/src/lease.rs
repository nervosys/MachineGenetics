// ── Semantic Lease Manager ─────────────────────────────────────────
//
// Manages concurrent access to semantic regions of a codebase by multiple
// agents.  Three lease modes:
//
//   SharedRead        – many agents can read simultaneously
//   ExclusiveWrite    – one agent writes; blocks all others
//   Restructuring     – structural refactors; blocks overlapping regions
//
// Detects deadlocks via a wait-for graph and supports configurable
// timeouts with automatic expiry.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt;

// ── Semantic Region ────────────────────────────────────────────────

/// Identifies a contiguous semantic region in the codebase.
/// Regions form a tree: `module::item::block`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SemanticRegion {
    /// Dotted path, e.g. `"crate::foo::Bar::baz"`.
    pub path: String,
}

impl SemanticRegion {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// True if `self` is a prefix of (or equal to) `other`.
    pub fn contains(&self, other: &SemanticRegion) -> bool {
        other.path == self.path || other.path.starts_with(&format!("{}::", self.path))
    }

    /// True if either region contains the other.
    pub fn overlaps(&self, other: &SemanticRegion) -> bool {
        self.contains(other) || other.contains(self)
    }
}

impl fmt::Display for SemanticRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)
    }
}

// ── Lease Mode ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseMode {
    SharedRead,
    ExclusiveWrite,
    Restructuring,
}

impl fmt::Display for LeaseMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaseMode::SharedRead => write!(f, "SharedRead"),
            LeaseMode::ExclusiveWrite => write!(f, "ExclusiveWrite"),
            LeaseMode::Restructuring => write!(f, "Restructuring"),
        }
    }
}

impl LeaseMode {
    /// Can two leases of these modes coexist on overlapping regions?
    pub fn compatible(a: LeaseMode, b: LeaseMode) -> bool {
        matches!((a, b), (LeaseMode::SharedRead, LeaseMode::SharedRead))
    }
}

// ── Agent + Lease IDs ─────────────────────────────────────────────

pub type AgentId = String;
pub type LeaseId = u64;

// ── Lease ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Lease {
    pub id: LeaseId,
    pub agent: AgentId,
    pub region: SemanticRegion,
    pub mode: LeaseMode,
    /// Logical timestamp when acquired.
    pub acquired_at: u64,
    /// If `Some(t)`, the lease expires at logical time `t`.
    pub expires_at: Option<u64>,
}

// ── Errors ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseError {
    /// Another lease conflicts.
    Conflict { holder: AgentId, region: SemanticRegion, mode: LeaseMode },
    /// Granting this lease would create a deadlock cycle.
    Deadlock { cycle: Vec<AgentId> },
    /// Lease not found.
    NotFound(LeaseId),
    /// Lease expired.
    Expired(LeaseId),
}

impl fmt::Display for LeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaseError::Conflict { holder, region, mode } => {
                write!(f, "conflict: agent {holder} holds {mode} on {region}")
            }
            LeaseError::Deadlock { cycle } => {
                write!(f, "deadlock detected: {}", cycle.join(" → "))
            }
            LeaseError::NotFound(id) => write!(f, "lease {id} not found"),
            LeaseError::Expired(id) => write!(f, "lease {id} expired"),
        }
    }
}

// ── Lease Manager ─────────────────────────────────────────────────

pub struct LeaseManager {
    next_id: LeaseId,
    clock: u64,
    active: BTreeMap<LeaseId, Lease>,
    /// Default timeout (in logical ticks) for new leases. 0 = no timeout.
    pub default_timeout: u64,
    /// Wait-for graph: agent → set of agents it is waiting on.
    wait_for: HashMap<AgentId, HashSet<AgentId>>,
}

impl LeaseManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            clock: 0,
            active: BTreeMap::new(),
            default_timeout: 0,
            wait_for: HashMap::new(),
        }
    }

    /// Advance the logical clock by `n` ticks and expire stale leases.
    pub fn tick(&mut self, n: u64) {
        self.clock += n;
        self.expire_stale();
    }

    /// Current logical time.
    pub fn now(&self) -> u64 {
        self.clock
    }

    /// Try to acquire a lease. Returns the lease ID on success.
    pub fn acquire(
        &mut self,
        agent: AgentId,
        region: SemanticRegion,
        mode: LeaseMode,
    ) -> Result<LeaseId, LeaseError> {
        self.expire_stale();

        // Check for conflicts with existing leases.
        let blockers = self.find_conflicts(&agent, &region, mode);
        if !blockers.is_empty() {
            // Record wait-for edges for deadlock detection.
            let blocker_agents: HashSet<AgentId> =
                blockers.iter().map(|l| l.agent.clone()).collect();

            // Before adding edges, check if this would create a cycle.
            if self.would_deadlock(&agent, &blocker_agents) {
                let cycle = self.find_cycle(&agent, &blocker_agents);
                // Don't add the wait-for edges.
                return Err(LeaseError::Deadlock { cycle });
            }

            // Return the first conflict.
            let first = &blockers[0];
            return Err(LeaseError::Conflict {
                holder: first.agent.clone(),
                region: first.region.clone(),
                mode: first.mode,
            });
        }

        // Remove any wait-for edges for this agent (they got what they wanted).
        self.wait_for.remove(&agent);

        let id = self.next_id;
        self.next_id += 1;

        let expires_at =
            if self.default_timeout > 0 { Some(self.clock + self.default_timeout) } else { None };

        let lease = Lease { id, agent, region, mode, acquired_at: self.clock, expires_at };
        self.active.insert(id, lease);
        Ok(id)
    }

    /// Release a lease by ID.
    pub fn release(&mut self, lease_id: LeaseId) -> Result<(), LeaseError> {
        if self.active.remove(&lease_id).is_some() {
            Ok(())
        } else {
            Err(LeaseError::NotFound(lease_id))
        }
    }

    /// Release all leases held by an agent.
    pub fn release_all(&mut self, agent: &str) {
        self.active.retain(|_, l| l.agent != agent);
        self.wait_for.remove(agent);
    }

    /// List active leases.
    pub fn active_leases(&self) -> Vec<&Lease> {
        self.active.values().collect()
    }

    /// List leases held by a specific agent.
    pub fn agent_leases(&self, agent: &str) -> Vec<&Lease> {
        self.active.values().filter(|l| l.agent == agent).collect()
    }

    /// All semantic regions currently locked (any mode).
    pub fn locked_regions(&self) -> BTreeSet<&SemanticRegion> {
        self.active.values().map(|l| &l.region).collect()
    }

    /// Produce a JSON-serialisable snapshot of the lease table.
    pub fn to_json(&self) -> String {
        let mut entries = Vec::new();
        for l in self.active.values() {
            entries.push(format!(
                "{{\"id\":{},\"agent\":\"{}\",\"region\":\"{}\",\"mode\":\"{}\",\"acquired_at\":{},\"expires_at\":{}}}",
                l.id, l.agent, l.region, l.mode, l.acquired_at,
                match l.expires_at { Some(t) => t.to_string(), None => "null".to_string() }
            ));
        }
        format!("[{}]", entries.join(","))
    }

    // ── Internal ──────────────────────────────────────────────────

    fn find_conflicts(&self, agent: &str, region: &SemanticRegion, mode: LeaseMode) -> Vec<&Lease> {
        self.active
            .values()
            .filter(|l| {
                l.agent != agent
                    && l.region.overlaps(region)
                    && !LeaseMode::compatible(l.mode, mode)
            })
            .collect()
    }

    fn expire_stale(&mut self) {
        let now = self.clock;
        self.active.retain(|_, l| match l.expires_at {
            Some(t) => t > now,
            None => true,
        });
    }

    /// Would adding wait-for edges from `agent` to `blockers` create a cycle?
    fn would_deadlock(&self, agent: &AgentId, blockers: &HashSet<AgentId>) -> bool {
        // BFS from each blocker; if we reach `agent`, there is a cycle.
        let mut visited = HashSet::new();
        let mut queue: VecDeque<&AgentId> = blockers.iter().collect();

        while let Some(current) = queue.pop_front() {
            if current == agent {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(waiting_on) = self.wait_for.get(current) {
                    for next in waiting_on {
                        queue.push_back(next);
                    }
                }
            }
        }
        false
    }

    /// Extract the deadlock cycle for reporting.
    fn find_cycle(&self, agent: &AgentId, blockers: &HashSet<AgentId>) -> Vec<AgentId> {
        // BFS to find the shortest cycle.
        let mut visited: HashMap<AgentId, AgentId> = HashMap::new(); // child → parent
        let mut queue: VecDeque<AgentId> = Vec::new().into();

        for b in blockers {
            visited.insert(b.clone(), agent.clone());
            queue.push_back(b.clone());
        }

        while let Some(current) = queue.pop_front() {
            if &current == agent {
                // Reconstruct path.
                let mut path = vec![agent.clone()];
                let mut node = agent.clone();
                loop {
                    if let Some(parent) = visited.get(&node) {
                        path.push(parent.clone());
                        if parent == agent {
                            break;
                        }
                        node = parent.clone();
                    } else {
                        break;
                    }
                }
                path.reverse();
                return path;
            }
            if let Some(waiting_on) = self.wait_for.get(&current) {
                for next in waiting_on {
                    if !visited.contains_key(next) {
                        visited.insert(next.clone(), current.clone());
                        queue.push_back(next.clone());
                    }
                }
            }
        }
        vec![agent.clone()] // fallback
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mgr() -> LeaseManager {
        LeaseManager::new()
    }

    fn region(path: &str) -> SemanticRegion {
        SemanticRegion::new(path)
    }

    // ── Region overlap ────────────────────────────────────────────

    #[test]
    fn region_contains_self() {
        let r = region("crate::foo");
        assert!(r.contains(&r));
    }

    #[test]
    fn region_contains_child() {
        let parent = region("crate::foo");
        let child = region("crate::foo::bar");
        assert!(parent.contains(&child));
        assert!(!child.contains(&parent));
    }

    #[test]
    fn region_overlap_symmetric() {
        let a = region("crate::foo");
        let b = region("crate::foo::bar");
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn region_no_overlap() {
        let a = region("crate::foo");
        let b = region("crate::bar");
        assert!(!a.overlaps(&b));
    }

    // ── Lease mode compatibility ──────────────────────────────────

    #[test]
    fn shared_read_compatible() {
        assert!(LeaseMode::compatible(LeaseMode::SharedRead, LeaseMode::SharedRead));
    }

    #[test]
    fn exclusive_incompatible_with_shared() {
        assert!(!LeaseMode::compatible(LeaseMode::ExclusiveWrite, LeaseMode::SharedRead));
        assert!(!LeaseMode::compatible(LeaseMode::SharedRead, LeaseMode::ExclusiveWrite));
    }

    #[test]
    fn exclusive_incompatible_with_exclusive() {
        assert!(!LeaseMode::compatible(LeaseMode::ExclusiveWrite, LeaseMode::ExclusiveWrite));
    }

    #[test]
    fn restructuring_incompatible_with_all() {
        assert!(!LeaseMode::compatible(LeaseMode::Restructuring, LeaseMode::SharedRead));
        assert!(!LeaseMode::compatible(LeaseMode::Restructuring, LeaseMode::ExclusiveWrite));
        assert!(!LeaseMode::compatible(LeaseMode::Restructuring, LeaseMode::Restructuring));
    }

    // ── Basic acquire / release ───────────────────────────────────

    #[test]
    fn acquire_and_release() {
        let mut m = mgr();
        let id =
            m.acquire("agent-a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        assert_eq!(m.active_leases().len(), 1);
        m.release(id).unwrap();
        assert_eq!(m.active_leases().len(), 0);
    }

    #[test]
    fn multiple_shared_reads() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::SharedRead).unwrap();
        m.acquire("b".into(), region("crate::foo"), LeaseMode::SharedRead).unwrap();
        m.acquire("c".into(), region("crate::foo"), LeaseMode::SharedRead).unwrap();
        assert_eq!(m.active_leases().len(), 3);
    }

    #[test]
    fn exclusive_blocks_shared() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        let err = m.acquire("b".into(), region("crate::foo"), LeaseMode::SharedRead).unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { .. }));
    }

    #[test]
    fn shared_blocks_exclusive() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::SharedRead).unwrap();
        let err =
            m.acquire("b".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { .. }));
    }

    // ── Overlapping regions ───────────────────────────────────────

    #[test]
    fn parent_exclusive_blocks_child_write() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        let err = m
            .acquire("b".into(), region("crate::foo::bar"), LeaseMode::ExclusiveWrite)
            .unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { .. }));
    }

    #[test]
    fn child_exclusive_blocks_parent_write() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo::bar"), LeaseMode::ExclusiveWrite).unwrap();
        let err =
            m.acquire("b".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { .. }));
    }

    #[test]
    fn non_overlapping_regions_ok() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        m.acquire("b".into(), region("crate::bar"), LeaseMode::ExclusiveWrite).unwrap();
        assert_eq!(m.active_leases().len(), 2);
    }

    // ── Same agent can hold multiple leases ───────────────────────

    #[test]
    fn same_agent_multiple_regions() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        m.acquire("a".into(), region("crate::bar"), LeaseMode::ExclusiveWrite).unwrap();
        assert_eq!(m.agent_leases("a").len(), 2);
    }

    // ── Restructuring ─────────────────────────────────────────────

    #[test]
    fn restructuring_blocks_all() {
        let mut m = mgr();
        m.acquire("a".into(), region("crate::mod"), LeaseMode::Restructuring).unwrap();
        let err =
            m.acquire("b".into(), region("crate::mod::item"), LeaseMode::SharedRead).unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { holder, .. } if holder == "a"));
    }

    // ── Timeouts ──────────────────────────────────────────────────

    #[test]
    fn lease_expires_after_timeout() {
        let mut m = mgr();
        m.default_timeout = 10;
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        assert_eq!(m.active_leases().len(), 1);

        // Advance past timeout.
        m.tick(11);
        assert_eq!(m.active_leases().len(), 0);

        // Now another agent can acquire.
        m.acquire("b".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        assert_eq!(m.active_leases().len(), 1);
    }

    #[test]
    fn lease_alive_before_timeout() {
        let mut m = mgr();
        m.default_timeout = 10;
        m.acquire("a".into(), region("crate::foo"), LeaseMode::ExclusiveWrite).unwrap();
        m.tick(5);
        assert_eq!(m.active_leases().len(), 1); // still alive
    }

    // ── Deadlock detection ────────────────────────────────────────

    #[test]
    fn simple_deadlock_detected() {
        let mut m = mgr();
        // A holds R1, B holds R2.
        m.acquire("a".into(), region("r1"), LeaseMode::ExclusiveWrite).unwrap();
        m.acquire("b".into(), region("r2"), LeaseMode::ExclusiveWrite).unwrap();

        // B tries to get R1 → blocked by A.  Record wait-for edge B→A.
        let err = m.acquire("b".into(), region("r1"), LeaseMode::ExclusiveWrite).unwrap_err();
        assert!(matches!(err, LeaseError::Conflict { .. }));
        m.wait_for.entry("b".into()).or_default().insert("a".into());

        // A tries to get R2 → would create cycle A→B→A.
        let err = m.acquire("a".into(), region("r2"), LeaseMode::ExclusiveWrite).unwrap_err();
        assert!(matches!(err, LeaseError::Deadlock { .. }));
    }

    // ── release_all ───────────────────────────────────────────────

    #[test]
    fn release_all_clears_agent() {
        let mut m = mgr();
        m.acquire("a".into(), region("r1"), LeaseMode::SharedRead).unwrap();
        m.acquire("a".into(), region("r2"), LeaseMode::SharedRead).unwrap();
        assert_eq!(m.agent_leases("a").len(), 2);
        m.release_all("a");
        assert_eq!(m.agent_leases("a").len(), 0);
    }

    // ── JSON output ───────────────────────────────────────────────

    #[test]
    fn to_json_format() {
        let mut m = mgr();
        m.acquire("agent-x".into(), region("crate::m"), LeaseMode::SharedRead).unwrap();
        let json = m.to_json();
        assert!(json.contains("\"agent\":\"agent-x\""));
        assert!(json.contains("\"mode\":\"SharedRead\""));
        assert!(json.contains("\"region\":\"crate::m\""));
    }

    // ── locked_regions ────────────────────────────────────────────

    #[test]
    fn locked_regions_lists_all() {
        let mut m = mgr();
        m.acquire("a".into(), region("r1"), LeaseMode::SharedRead).unwrap();
        m.acquire("b".into(), region("r2"), LeaseMode::ExclusiveWrite).unwrap();
        let regions = m.locked_regions();
        assert_eq!(regions.len(), 2);
    }

    // ── Release not-found ─────────────────────────────────────────

    #[test]
    fn release_not_found() {
        let mut m = mgr();
        let err = m.release(999).unwrap_err();
        assert_eq!(err, LeaseError::NotFound(999));
    }
}
