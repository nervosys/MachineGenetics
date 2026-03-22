// Redox Agent Capability System — per-agent bounds, attenuation, enforcement.
//
// Implements §P5 (Capability-Bounded Agents) from REDOX_PROPOSAL.md:
//   - Per-agent capability bounds (read, write, execute, etc.)
//   - Attenuation: child capabilities ≤ parent capabilities
//   - Runtime enforcement at the swarm bus level
//
// (ROADMAP Step 48)

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ── Capability Primitives ──────────────────────────────────────────────────

/// The fundamental capability rights an agent can hold.
/// Modeled after Rust's ownership: read < write < execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Right {
    /// Read source code and metadata.
    Read,
    /// Write / modify source code.
    Write,
    /// Execute compiler passes, run tests.
    Execute,
    /// Manage other agents (spawn, kill, attenuate).
    Manage,
    /// Full administrative privileges.
    Admin,
}

impl Right {
    /// The power level of this right (higher = more powerful).
    pub fn level(&self) -> u8 {
        match self {
            Right::Read => 1,
            Right::Write => 2,
            Right::Execute => 3,
            Right::Manage => 4,
            Right::Admin => 5,
        }
    }

    /// Whether this right subsumes `other`.
    /// Admin subsumes everything. Otherwise, each right is independent.
    pub fn subsumes(&self, other: &Right) -> bool {
        *self == Right::Admin || *self == *other
    }

    /// All rights from lowest to highest.
    pub fn all() -> &'static [Right] {
        &[Right::Read, Right::Write, Right::Execute, Right::Manage, Right::Admin]
    }

    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Right> {
        match s {
            "read" => Some(Right::Read),
            "write" => Some(Right::Write),
            "execute" => Some(Right::Execute),
            "manage" => Some(Right::Manage),
            "admin" => Some(Right::Admin),
            _ => None,
        }
    }
}

impl fmt::Display for Right {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Right::Read => write!(f, "read"),
            Right::Write => write!(f, "write"),
            Right::Execute => write!(f, "execute"),
            Right::Manage => write!(f, "manage"),
            Right::Admin => write!(f, "admin"),
        }
    }
}

/// A scoped capability: a `Right` applied to a specific `Resource`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopedCapability {
    pub right: Right,
    pub resource: Resource,
}

impl ScopedCapability {
    pub fn new(right: Right, resource: Resource) -> Self {
        ScopedCapability { right, resource }
    }

    /// Whether this capability subsumes another (same or broader resource, same or higher right).
    pub fn subsumes(&self, other: &ScopedCapability) -> bool {
        self.right.subsumes(&other.right) && self.resource.contains(&other.resource)
    }
}

impl fmt::Display for ScopedCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.right, self.resource)
    }
}

/// A resource that capabilities are scoped to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resource {
    /// All resources (wildcard).
    All,
    /// A specific crate.
    Crate(String),
    /// A specific module path (e.g. "std::vec").
    Module(String),
    /// A specific file path.
    File(String),
    /// A named channel on the message bus.
    Channel(String),
    /// A protocol method (e.g. "cost/query").
    Method(String),
}

impl Resource {
    /// Whether this resource contains `other` (for attenuation).
    pub fn contains(&self, other: &Resource) -> bool {
        match (self, other) {
            (Resource::All, _) => true,
            (_, Resource::All) => false,
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resource::All => write!(f, "*"),
            Resource::Crate(name) => write!(f, "crate:{name}"),
            Resource::Module(path) => write!(f, "module:{path}"),
            Resource::File(path) => write!(f, "file:{path}"),
            Resource::Channel(ch) => write!(f, "channel:{ch}"),
            Resource::Method(m) => write!(f, "method:{m}"),
        }
    }
}

// ── Capability Bound ───────────────────────────────────────────────────────

/// A capability bound is the complete set of permissions for an agent.
#[derive(Debug, Clone)]
pub struct CapabilityBound {
    capabilities: BTreeSet<ScopedCapability>,
    label: String,
}

impl CapabilityBound {
    /// Create an empty bound with a label.
    pub fn new(label: &str) -> Self {
        CapabilityBound { capabilities: BTreeSet::new(), label: label.to_string() }
    }

    /// Create a bound with full admin rights.
    pub fn admin(label: &str) -> Self {
        let mut bound = Self::new(label);
        bound.grant(ScopedCapability::new(Right::Admin, Resource::All));
        bound
    }

    /// Create a read-only bound on all resources.
    pub fn read_only(label: &str) -> Self {
        let mut bound = Self::new(label);
        bound.grant(ScopedCapability::new(Right::Read, Resource::All));
        bound
    }

    /// Grant a capability.
    pub fn grant(&mut self, cap: ScopedCapability) {
        self.capabilities.insert(cap);
    }

    /// Revoke a specific capability.
    pub fn revoke(&mut self, cap: &ScopedCapability) {
        self.capabilities.remove(cap);
    }

    /// Check if this bound permits a specific capability.
    pub fn permits(&self, cap: &ScopedCapability) -> bool {
        self.capabilities.iter().any(|c| c.subsumes(cap))
    }

    /// Check if this bound permits a right on a resource.
    pub fn has_right(&self, right: Right, resource: &Resource) -> bool {
        self.permits(&ScopedCapability::new(right, resource.clone()))
    }

    /// All capabilities in this bound.
    pub fn capabilities(&self) -> &BTreeSet<ScopedCapability> {
        &self.capabilities
    }

    /// Number of explicit capabilities.
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Whether this bound is empty.
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Label of this bound.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Whether this bound is a superset of (i.e., subsumes) another bound.
    /// For attenuation: `parent.subsumes(&child)` must be true.
    pub fn subsumes(&self, other: &CapabilityBound) -> bool {
        other.capabilities.iter().all(|cap| self.permits(cap))
    }
}

impl fmt::Display for CapabilityBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapabilityBound(\"{}\"", self.label)?;
        for cap in &self.capabilities {
            write!(f, ", {cap}")?;
        }
        write!(f, ")")
    }
}

// ── Attenuation ────────────────────────────────────────────────────────────

/// Result of an attenuation attempt.
#[derive(Debug, Clone)]
pub enum AttenuationResult {
    /// All child capabilities are within parent bounds.
    Valid,
    /// Some child capabilities exceed parent bounds.
    Violation(Vec<AttenuationViolation>),
}

impl AttenuationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, AttenuationResult::Valid)
    }

    pub fn violations(&self) -> &[AttenuationViolation] {
        match self {
            AttenuationResult::Valid => &[],
            AttenuationResult::Violation(v) => v,
        }
    }
}

/// A single attenuation violation.
#[derive(Debug, Clone)]
pub struct AttenuationViolation {
    pub requested: ScopedCapability,
    pub reason: String,
}

impl fmt::Display for AttenuationViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capability {} denied: {}", self.requested, self.reason)
    }
}

/// Check that a child bound doesn't exceed the parent bound (attenuation rule).
pub fn check_attenuation(parent: &CapabilityBound, child: &CapabilityBound) -> AttenuationResult {
    let mut violations = Vec::new();

    for cap in child.capabilities() {
        if !parent.permits(cap) {
            violations.push(AttenuationViolation {
                requested: cap.clone(),
                reason: format!("parent bound \"{}\" does not permit {}", parent.label(), cap),
            });
        }
    }

    if violations.is_empty() {
        AttenuationResult::Valid
    } else {
        AttenuationResult::Violation(violations)
    }
}

/// Attenuate a parent bound to produce a narrower child bound.
/// Returns the intersection: only capabilities the child requests that the parent permits.
pub fn attenuate(parent: &CapabilityBound, requested: &CapabilityBound) -> CapabilityBound {
    let mut child = CapabilityBound::new(&format!("{}:child", parent.label()));
    for cap in requested.capabilities() {
        if parent.permits(cap) {
            child.grant(cap.clone());
        }
    }
    child
}

// ── Agent Identity ─────────────────────────────────────────────────────────

/// A unique agent identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(id: &str) -> Self {
        AgentId(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An agent registered in the capability system.
#[derive(Debug)]
pub struct AgentRecord {
    pub id: AgentId,
    pub bound: CapabilityBound,
    pub parent: Option<AgentId>,
    pub children: Vec<AgentId>,
    pub active: bool,
}

// ── Enforcement Layer ──────────────────────────────────────────────────────

/// The result of an enforcement check.
#[derive(Debug, Clone)]
pub enum EnforcementResult {
    /// Access permitted.
    Allowed,
    /// Access denied with reason.
    Denied(String),
}

impl EnforcementResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, EnforcementResult::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, EnforcementResult::Denied(_))
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            EnforcementResult::Denied(r) => Some(r),
            _ => None,
        }
    }
}

impl fmt::Display for EnforcementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnforcementResult::Allowed => write!(f, "allowed"),
            EnforcementResult::Denied(r) => write!(f, "denied: {r}"),
        }
    }
}

/// Maps a message bus operation to a required capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusOperation {
    /// Sending a message to a specific agent.
    Send,
    /// Publishing to a channel.
    Publish,
    /// Broadcasting to all agents.
    Broadcast,
    /// Subscribing to a channel.
    Subscribe,
    /// Spawning a child agent.
    SpawnChild,
}

impl BusOperation {
    /// The minimum right required for this operation.
    pub fn required_right(&self) -> Right {
        match self {
            BusOperation::Send => Right::Write,
            BusOperation::Publish => Right::Write,
            BusOperation::Broadcast => Right::Execute,
            BusOperation::Subscribe => Right::Read,
            BusOperation::SpawnChild => Right::Manage,
        }
    }
}

impl fmt::Display for BusOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusOperation::Send => write!(f, "send"),
            BusOperation::Publish => write!(f, "publish"),
            BusOperation::Broadcast => write!(f, "broadcast"),
            BusOperation::Subscribe => write!(f, "subscribe"),
            BusOperation::SpawnChild => write!(f, "spawn_child"),
        }
    }
}

/// An enforcement event recorded in the audit log.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub agent: AgentId,
    pub operation: String,
    pub resource: Resource,
    pub result: EnforcementResult,
}

impl fmt::Display for AuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "agent={} op={} resource={} result={}",
            self.agent, self.operation, self.resource, self.result
        )
    }
}

/// The capability enforcement engine — sits between agents and the message bus.
pub struct CapabilityEnforcer {
    agents: BTreeMap<String, AgentRecord>,
    audit_log: Vec<AuditEntry>,
    enforce: bool,
}

impl CapabilityEnforcer {
    /// Create a new enforcer.
    pub fn new() -> Self {
        CapabilityEnforcer { agents: BTreeMap::new(), audit_log: Vec::new(), enforce: true }
    }

    /// Create an enforcer with enforcement disabled (permissive mode).
    pub fn permissive() -> Self {
        CapabilityEnforcer { agents: BTreeMap::new(), audit_log: Vec::new(), enforce: false }
    }

    /// Whether enforcement is active.
    pub fn is_enforcing(&self) -> bool {
        self.enforce
    }

    /// Register a root agent (no parent).
    pub fn register_root(&mut self, id: &str, bound: CapabilityBound) {
        let agent_id = AgentId::new(id);
        self.agents.insert(
            id.to_string(),
            AgentRecord { id: agent_id, bound, parent: None, children: Vec::new(), active: true },
        );
    }

    /// Spawn a child agent with attenuated capabilities.
    /// Enforces: child bound ≤ parent bound.
    pub fn spawn_child(
        &mut self,
        parent_id: &str,
        child_id: &str,
        requested_bound: CapabilityBound,
    ) -> Result<AttenuationResult, EnforcementResult> {
        // Check parent exists and is active.
        let parent = self.agents.get(parent_id).ok_or_else(|| {
            EnforcementResult::Denied(format!("parent agent \"{parent_id}\" not found"))
        })?;

        if !parent.active {
            return Err(EnforcementResult::Denied(format!(
                "parent agent \"{parent_id}\" is inactive"
            )));
        }

        // Check parent has manage right.
        if self.enforce {
            let manage_check =
                self.check_internal(parent_id, BusOperation::SpawnChild, &Resource::All);
            if manage_check.is_denied() {
                return Err(manage_check);
            }
        }

        // Attenuate: child gets only what parent permits.
        let attenuation = check_attenuation(&parent.bound, &requested_bound);
        let child_bound = attenuate(&parent.bound, &requested_bound);
        let parent_agent_id = AgentId::new(parent_id);

        // Record parent→child relationship.
        let child_agent_id = AgentId::new(child_id);
        self.agents.insert(
            child_id.to_string(),
            AgentRecord {
                id: child_agent_id,
                bound: child_bound,
                parent: Some(parent_agent_id),
                children: Vec::new(),
                active: true,
            },
        );

        if let Some(p) = self.agents.get_mut(parent_id) {
            p.children.push(AgentId::new(child_id));
        }

        Ok(attenuation)
    }

    /// Deactivate an agent and all its children.
    pub fn deactivate(&mut self, id: &str) -> bool {
        let children: Vec<String> = self
            .agents
            .get(id)
            .map(|a| a.children.iter().map(|c| c.0.clone()).collect())
            .unwrap_or_default();

        // Recursively deactivate children.
        for child in &children {
            self.deactivate(child);
        }

        if let Some(agent) = self.agents.get_mut(id) {
            agent.active = false;
            true
        } else {
            false
        }
    }

    /// Check whether an agent is permitted to perform a bus operation on a resource.
    pub fn check(
        &mut self,
        agent_id: &str,
        operation: BusOperation,
        resource: &Resource,
    ) -> EnforcementResult {
        let result = self.check_internal(agent_id, operation, resource);

        // Audit log.
        self.audit_log.push(AuditEntry {
            agent: AgentId::new(agent_id),
            operation: operation.to_string(),
            resource: resource.clone(),
            result: result.clone(),
        });

        result
    }

    fn check_internal(
        &self,
        agent_id: &str,
        operation: BusOperation,
        resource: &Resource,
    ) -> EnforcementResult {
        if !self.enforce {
            return EnforcementResult::Allowed;
        }

        let agent = match self.agents.get(agent_id) {
            Some(a) => a,
            None => {
                return EnforcementResult::Denied(format!("agent \"{agent_id}\" not registered"));
            }
        };

        if !agent.active {
            return EnforcementResult::Denied(format!("agent \"{agent_id}\" is inactive"));
        }

        let required = ScopedCapability::new(operation.required_right(), resource.clone());
        if agent.bound.permits(&required) {
            EnforcementResult::Allowed
        } else {
            EnforcementResult::Denied(format!(
                "agent \"{}\" lacks {} on {}",
                agent_id,
                operation.required_right(),
                resource
            ))
        }
    }

    /// Convenience: check a method call.
    pub fn check_method_call(&mut self, agent_id: &str, method: &str) -> EnforcementResult {
        self.check(agent_id, BusOperation::Send, &Resource::Method(method.to_string()))
    }

    /// Convenience: check a channel publish.
    pub fn check_publish(&mut self, agent_id: &str, channel: &str) -> EnforcementResult {
        self.check(agent_id, BusOperation::Publish, &Resource::Channel(channel.to_string()))
    }

    /// Convenience: check a channel subscribe.
    pub fn check_subscribe(&mut self, agent_id: &str, channel: &str) -> EnforcementResult {
        self.check(agent_id, BusOperation::Subscribe, &Resource::Channel(channel.to_string()))
    }

    /// Get an agent record by ID.
    pub fn get_agent(&self, id: &str) -> Option<&AgentRecord> {
        self.agents.get(id)
    }

    /// Number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Number of active agents.
    pub fn active_agent_count(&self) -> usize {
        self.agents.values().filter(|a| a.active).count()
    }

    /// Get the audit log.
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Number of audit entries.
    pub fn audit_count(&self) -> usize {
        self.audit_log.len()
    }

    /// Number of denied operations in the audit log.
    pub fn denial_count(&self) -> usize {
        self.audit_log.iter().filter(|e| e.result.is_denied()).count()
    }

    /// Clear the audit log.
    pub fn clear_audit_log(&mut self) {
        self.audit_log.clear();
    }
}

impl Default for CapabilityEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Preset Bounds ──────────────────────────────────────────────────────────

/// Pre-defined capability bounds for common agent roles.
pub mod presets {
    use super::*;

    /// Observer: read-only access to everything.
    pub fn observer() -> CapabilityBound {
        let mut b = CapabilityBound::new("observer");
        b.grant(ScopedCapability::new(Right::Read, Resource::All));
        b
    }

    /// Developer: read + write to code.
    pub fn developer() -> CapabilityBound {
        let mut b = CapabilityBound::new("developer");
        b.grant(ScopedCapability::new(Right::Read, Resource::All));
        b.grant(ScopedCapability::new(Right::Write, Resource::All));
        b
    }

    /// Builder: read + write + execute.
    pub fn builder() -> CapabilityBound {
        let mut b = CapabilityBound::new("builder");
        b.grant(ScopedCapability::new(Right::Read, Resource::All));
        b.grant(ScopedCapability::new(Right::Write, Resource::All));
        b.grant(ScopedCapability::new(Right::Execute, Resource::All));
        b
    }

    /// Coordinator: full management.
    pub fn coordinator() -> CapabilityBound {
        let mut b = CapabilityBound::new("coordinator");
        for right in Right::all() {
            b.grant(ScopedCapability::new(*right, Resource::All));
        }
        b
    }

    /// Scoped reader: read-only on a specific crate.
    pub fn crate_reader(crate_name: &str) -> CapabilityBound {
        let mut b = CapabilityBound::new(&format!("reader:{crate_name}"));
        b.grant(ScopedCapability::new(Right::Read, Resource::Crate(crate_name.to_string())));
        b
    }

    /// Scoped writer: read + write on a specific crate.
    pub fn crate_writer(crate_name: &str) -> CapabilityBound {
        let mut b = CapabilityBound::new(&format!("writer:{crate_name}"));
        b.grant(ScopedCapability::new(Right::Read, Resource::Crate(crate_name.to_string())));
        b.grant(ScopedCapability::new(Right::Write, Resource::Crate(crate_name.to_string())));
        b
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Right ──

    #[test]
    fn right_levels_are_ordered() {
        assert!(Right::Read.level() < Right::Write.level());
        assert!(Right::Write.level() < Right::Execute.level());
        assert!(Right::Execute.level() < Right::Manage.level());
        assert!(Right::Manage.level() < Right::Admin.level());
    }

    #[test]
    fn right_subsumption() {
        // Admin subsumes everything.
        assert!(Right::Admin.subsumes(&Right::Read));
        assert!(Right::Admin.subsumes(&Right::Write));
        assert!(Right::Admin.subsumes(&Right::Execute));
        assert!(Right::Admin.subsumes(&Right::Manage));
        // Same right subsumes itself.
        assert!(Right::Write.subsumes(&Right::Write));
        assert!(Right::Read.subsumes(&Right::Read));
        // Different non-admin rights don't subsume each other.
        assert!(!Right::Write.subsumes(&Right::Read));
        assert!(!Right::Read.subsumes(&Right::Write));
        assert!(!Right::Manage.subsumes(&Right::Execute));
    }

    #[test]
    fn right_from_str() {
        assert_eq!(Right::from_str("read"), Some(Right::Read));
        assert_eq!(Right::from_str("admin"), Some(Right::Admin));
        assert_eq!(Right::from_str("unknown"), None);
    }

    #[test]
    fn right_display() {
        assert_eq!(format!("{}", Right::Execute), "execute");
    }

    #[test]
    fn all_rights() {
        assert_eq!(Right::all().len(), 5);
    }

    // ── Resource ──

    #[test]
    fn resource_all_contains_everything() {
        assert!(Resource::All.contains(&Resource::Crate("foo".to_string())));
        assert!(Resource::All.contains(&Resource::File("bar.rs".to_string())));
        assert!(Resource::All.contains(&Resource::All));
    }

    #[test]
    fn specific_resource_does_not_contain_all() {
        assert!(!Resource::Crate("foo".to_string()).contains(&Resource::All));
    }

    #[test]
    fn resource_equality_containment() {
        let r = Resource::Crate("foo".to_string());
        assert!(r.contains(&Resource::Crate("foo".to_string())));
        assert!(!r.contains(&Resource::Crate("bar".to_string())));
    }

    #[test]
    fn resource_display() {
        assert_eq!(format!("{}", Resource::All), "*");
        assert_eq!(format!("{}", Resource::Crate("foo".to_string())), "crate:foo");
        assert_eq!(format!("{}", Resource::Channel("ch1".to_string())), "channel:ch1");
        assert_eq!(format!("{}", Resource::Method("cost/query".to_string())), "method:cost/query");
    }

    // ── ScopedCapability ──

    #[test]
    fn scoped_capability_subsumption() {
        let admin_all = ScopedCapability::new(Right::Admin, Resource::All);
        let read_foo = ScopedCapability::new(Right::Read, Resource::Crate("foo".to_string()));
        assert!(admin_all.subsumes(&read_foo));
        assert!(!read_foo.subsumes(&admin_all));
    }

    #[test]
    fn scoped_capability_same_resource_different_right() {
        let write_foo = ScopedCapability::new(Right::Write, Resource::Crate("foo".to_string()));
        let read_foo = ScopedCapability::new(Right::Read, Resource::Crate("foo".to_string()));
        // Different rights on the same resource don't subsume each other (independent model).
        assert!(!write_foo.subsumes(&read_foo));
        assert!(!read_foo.subsumes(&write_foo));
        // Same right subsumes itself.
        assert!(write_foo.subsumes(&write_foo));
        // Admin subsumes all rights.
        let admin_foo = ScopedCapability::new(Right::Admin, Resource::Crate("foo".to_string()));
        assert!(admin_foo.subsumes(&read_foo));
        assert!(admin_foo.subsumes(&write_foo));
    }

    #[test]
    fn scoped_capability_display() {
        let cap = ScopedCapability::new(Right::Write, Resource::Channel("ch1".to_string()));
        assert_eq!(format!("{cap}"), "write:channel:ch1");
    }

    // ── CapabilityBound ──

    #[test]
    fn empty_bound() {
        let b = CapabilityBound::new("empty");
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
        assert_eq!(b.label(), "empty");
    }

    #[test]
    fn admin_bound_permits_everything() {
        let b = CapabilityBound::admin("root");
        assert!(b.has_right(Right::Read, &Resource::All));
        assert!(b.has_right(Right::Write, &Resource::Crate("foo".to_string())));
        assert!(b.has_right(Right::Execute, &Resource::File("bar.rs".to_string())));
        assert!(b.has_right(Right::Admin, &Resource::All));
    }

    #[test]
    fn read_only_bound_blocks_writes() {
        let b = CapabilityBound::read_only("reader");
        assert!(b.has_right(Right::Read, &Resource::All));
        assert!(!b.has_right(Right::Write, &Resource::All));
    }

    #[test]
    fn bound_grant_and_revoke() {
        let mut b = CapabilityBound::new("test");
        let cap = ScopedCapability::new(Right::Write, Resource::Crate("foo".to_string()));
        b.grant(cap.clone());
        assert!(b.permits(&cap));
        b.revoke(&cap);
        assert!(!b.permits(&cap));
    }

    #[test]
    fn bound_subsumption() {
        let parent = CapabilityBound::admin("parent");
        let child = CapabilityBound::read_only("child");
        assert!(parent.subsumes(&child));
        assert!(!child.subsumes(&parent));
    }

    #[test]
    fn bound_display() {
        let b = CapabilityBound::read_only("test");
        let s = format!("{b}");
        assert!(s.contains("test"));
        assert!(s.contains("read:*"));
    }

    // ── Attenuation ──

    #[test]
    fn attenuation_valid_child_within_parent() {
        let parent = presets::developer();
        let child = presets::observer();
        let result = check_attenuation(&parent, &child);
        assert!(result.is_valid());
    }

    #[test]
    fn attenuation_violation_child_exceeds_parent() {
        let parent = presets::observer();
        let child = presets::developer();
        let result = check_attenuation(&parent, &child);
        assert!(!result.is_valid());
        assert!(result.violations().len() > 0);
    }

    #[test]
    fn attenuation_violation_message() {
        let parent = presets::observer();
        let mut child = CapabilityBound::new("rogue");
        child.grant(ScopedCapability::new(Right::Admin, Resource::All));
        let result = check_attenuation(&parent, &child);
        let v = &result.violations()[0];
        assert!(format!("{v}").contains("denied"));
    }

    #[test]
    fn attenuate_function_produces_intersection() {
        let parent = presets::developer(); // read + write
        let mut requested = CapabilityBound::new("want-all");
        requested.grant(ScopedCapability::new(Right::Read, Resource::All));
        requested.grant(ScopedCapability::new(Right::Write, Resource::All));
        requested.grant(ScopedCapability::new(Right::Execute, Resource::All)); // not in parent

        let child = attenuate(&parent, &requested);
        assert!(child.has_right(Right::Read, &Resource::All));
        assert!(child.has_right(Right::Write, &Resource::All));
        assert!(!child.has_right(Right::Execute, &Resource::All)); // stripped
    }

    // ── Presets ──

    #[test]
    fn preset_hierarchy() {
        let obs = presets::observer();
        let dev = presets::developer();
        let bld = presets::builder();
        let coord = presets::coordinator();

        assert!(dev.subsumes(&obs));
        assert!(bld.subsumes(&dev));
        assert!(coord.subsumes(&bld));
        assert!(!obs.subsumes(&dev));
    }

    #[test]
    fn preset_crate_reader() {
        let b = presets::crate_reader("my_crate");
        assert!(b.has_right(Right::Read, &Resource::Crate("my_crate".to_string())));
        assert!(!b.has_right(Right::Write, &Resource::Crate("my_crate".to_string())));
        assert!(!b.has_right(Right::Read, &Resource::Crate("other".to_string())));
    }

    #[test]
    fn preset_crate_writer() {
        let b = presets::crate_writer("my_crate");
        assert!(b.has_right(Right::Read, &Resource::Crate("my_crate".to_string())));
        assert!(b.has_right(Right::Write, &Resource::Crate("my_crate".to_string())));
        assert!(!b.has_right(Right::Execute, &Resource::Crate("my_crate".to_string())));
    }

    // ── AgentId ──

    #[test]
    fn agent_id_display() {
        let id = AgentId::new("agent-01");
        assert_eq!(id.as_str(), "agent-01");
        assert_eq!(format!("{id}"), "agent-01");
    }

    // ── BusOperation ──

    #[test]
    fn bus_operation_required_rights() {
        assert_eq!(BusOperation::Subscribe.required_right(), Right::Read);
        assert_eq!(BusOperation::Send.required_right(), Right::Write);
        assert_eq!(BusOperation::Publish.required_right(), Right::Write);
        assert_eq!(BusOperation::Broadcast.required_right(), Right::Execute);
        assert_eq!(BusOperation::SpawnChild.required_right(), Right::Manage);
    }

    #[test]
    fn bus_operation_display() {
        assert_eq!(format!("{}", BusOperation::Send), "send");
        assert_eq!(format!("{}", BusOperation::SpawnChild), "spawn_child");
    }

    // ── CapabilityEnforcer ──

    #[test]
    fn enforcer_register_root() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("root", presets::coordinator());
        assert_eq!(enforcer.agent_count(), 1);
        assert_eq!(enforcer.active_agent_count(), 1);
    }

    #[test]
    fn enforcer_permits_capable_agent() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("builder", presets::builder());

        let result =
            enforcer.check("builder", BusOperation::Send, &Resource::Channel("build".to_string()));
        assert!(result.is_allowed());
    }

    #[test]
    fn enforcer_denies_incapable_agent() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("reader", presets::observer());

        let result =
            enforcer.check("reader", BusOperation::Send, &Resource::Channel("build".to_string()));
        assert!(result.is_denied());
        assert!(result.reason().unwrap().contains("lacks"));
    }

    #[test]
    fn enforcer_denies_unknown_agent() {
        let mut enforcer = CapabilityEnforcer::new();
        let result = enforcer.check("ghost", BusOperation::Subscribe, &Resource::All);
        assert!(result.is_denied());
        assert!(result.reason().unwrap().contains("not registered"));
    }

    #[test]
    fn enforcer_denies_inactive_agent() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("agent", presets::developer());
        enforcer.deactivate("agent");

        let result = enforcer.check("agent", BusOperation::Subscribe, &Resource::All);
        assert!(result.is_denied());
        assert!(result.reason().unwrap().contains("inactive"));
    }

    #[test]
    fn enforcer_spawn_child_attenuated() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("parent", presets::coordinator());

        let child_bound = presets::developer();
        let result = enforcer.spawn_child("parent", "child", child_bound).unwrap();
        assert!(result.is_valid());

        assert_eq!(enforcer.agent_count(), 2);
        let child = enforcer.get_agent("child").unwrap();
        assert!(child.parent.as_ref().unwrap().as_str() == "parent");
    }

    #[test]
    fn enforcer_spawn_child_strips_excess() {
        let mut enforcer = CapabilityEnforcer::new();
        // Parent has read + write + manage (but NOT execute).
        let mut parent_bound = presets::developer();
        parent_bound.grant(ScopedCapability::new(Right::Manage, Resource::All));
        enforcer.register_root("parent", parent_bound);

        // Child requests execute — parent doesn't have it.
        let result = enforcer.spawn_child("parent", "child", presets::builder()).unwrap();
        assert!(!result.is_valid()); // violation reported
        assert!(result.violations().len() > 0);

        // But child was still created with attenuated caps.
        let child = enforcer.get_agent("child").unwrap();
        assert!(child.bound.has_right(Right::Read, &Resource::All));
        assert!(child.bound.has_right(Right::Write, &Resource::All));
        assert!(!child.bound.has_right(Right::Execute, &Resource::All)); // stripped
    }

    #[test]
    fn enforcer_spawn_child_from_unknown_parent() {
        let mut enforcer = CapabilityEnforcer::new();
        let result = enforcer.spawn_child("ghost", "child", presets::observer());
        assert!(result.is_err());
    }

    #[test]
    fn enforcer_spawn_child_observer_cannot_spawn() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("observer", presets::observer()); // read only

        let result = enforcer.spawn_child("observer", "child", presets::observer());
        assert!(result.is_err()); // observer lacks Manage right
    }

    #[test]
    fn enforcer_deactivate_cascades_to_children() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("root", presets::coordinator());
        // child1 needs Manage to spawn grandchild.
        let mut child1_bound = presets::developer();
        child1_bound.grant(ScopedCapability::new(Right::Manage, Resource::All));
        enforcer.spawn_child("root", "child1", child1_bound).unwrap();
        enforcer.spawn_child("child1", "grandchild", presets::observer()).unwrap();

        enforcer.deactivate("child1");

        assert!(!enforcer.get_agent("child1").unwrap().active);
        assert!(!enforcer.get_agent("grandchild").unwrap().active);
        assert!(enforcer.get_agent("root").unwrap().active);
    }

    #[test]
    fn enforcer_permissive_mode() {
        let mut enforcer = CapabilityEnforcer::permissive();
        assert!(!enforcer.is_enforcing());

        // Even without registration, permissive mode allows everything (except unknown agent).
        enforcer.register_root("any", CapabilityBound::new("empty"));
        let result = enforcer.check("any", BusOperation::Broadcast, &Resource::All);
        assert!(result.is_allowed());
    }

    // ── Audit Log ──

    #[test]
    fn audit_log_records_checks() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("agent", presets::observer());

        enforcer.check("agent", BusOperation::Subscribe, &Resource::All);
        enforcer.check("agent", BusOperation::Send, &Resource::All);

        assert_eq!(enforcer.audit_count(), 2);
        assert_eq!(enforcer.denial_count(), 1); // send denied for observer
    }

    #[test]
    fn audit_log_clear() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("agent", presets::observer());
        enforcer.check("agent", BusOperation::Subscribe, &Resource::All);
        assert_eq!(enforcer.audit_count(), 1);
        enforcer.clear_audit_log();
        assert_eq!(enforcer.audit_count(), 0);
    }

    #[test]
    fn audit_entry_display() {
        let entry = AuditEntry {
            agent: AgentId::new("test"),
            operation: "send".to_string(),
            resource: Resource::Channel("ch1".to_string()),
            result: EnforcementResult::Allowed,
        };
        let s = format!("{entry}");
        assert!(s.contains("test"));
        assert!(s.contains("send"));
        assert!(s.contains("channel:ch1"));
        assert!(s.contains("allowed"));
    }

    // ── Convenience Methods ──

    #[test]
    fn check_method_call() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("dev", presets::developer());

        let result = enforcer.check_method_call("dev", "cost/query");
        assert!(result.is_allowed());
    }

    #[test]
    fn check_publish_denied_for_reader() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("reader", presets::observer());

        let result = enforcer.check_publish("reader", "build-events");
        assert!(result.is_denied());
    }

    #[test]
    fn check_subscribe_allowed_for_reader() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("reader", presets::observer());

        let result = enforcer.check_subscribe("reader", "build-events");
        assert!(result.is_allowed());
    }

    // ── EnforcementResult ──

    #[test]
    fn enforcement_result_display() {
        assert_eq!(format!("{}", EnforcementResult::Allowed), "allowed");
        let denied = EnforcementResult::Denied("no perms".to_string());
        assert_eq!(format!("{denied}"), "denied: no perms");
    }

    // ── Full Scenario ──

    #[test]
    fn full_scenario_coordinator_spawns_hierarchy() {
        let mut enforcer = CapabilityEnforcer::new();

        // Root coordinator.
        enforcer.register_root("coordinator", presets::coordinator());

        // Spawn builder child.
        let r = enforcer.spawn_child("coordinator", "builder", presets::builder()).unwrap();
        assert!(r.is_valid());

        // Builder spawns reader grandchild.
        // Builder has read+write+execute but NOT manage → needs manage to spawn.
        let r = enforcer.spawn_child("builder", "reader", presets::observer());
        assert!(r.is_err()); // builder lacks Manage

        // Let's give builder a bound with manage.
        enforcer.deactivate("builder");
        let mut builder_bound = presets::builder();
        builder_bound.grant(ScopedCapability::new(Right::Manage, Resource::All));
        enforcer.register_root("builder2", builder_bound);
        let r = enforcer.spawn_child("builder2", "reader", presets::observer()).unwrap();
        assert!(r.is_valid());

        // Reader can subscribe but not send.
        let sub = enforcer.check_subscribe("reader", "events");
        assert!(sub.is_allowed());
        let send = enforcer.check_method_call("reader", "build/heal");
        assert!(send.is_denied());
    }

    #[test]
    fn full_scenario_scoped_crate_permissions() {
        let mut enforcer = CapabilityEnforcer::new();
        enforcer.register_root("scoped", presets::crate_writer("redox_skb"));

        // Can write to redox_skb.
        let r =
            enforcer.check("scoped", BusOperation::Send, &Resource::Crate("redox_skb".to_string()));
        assert!(r.is_allowed());

        // Cannot write to other crates.
        let r =
            enforcer.check("scoped", BusOperation::Send, &Resource::Crate("redox_ast".to_string()));
        assert!(r.is_denied());

        // Cannot write to All.
        let r = enforcer.check("scoped", BusOperation::Send, &Resource::All);
        assert!(r.is_denied());
    }
}
