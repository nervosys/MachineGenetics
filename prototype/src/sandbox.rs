// ── Capability-Based Sandbox ───────────────────────────────────────
//
// Per-agent isolation with resource limits, capability attenuation,
// and audit logging for the MechGen swarm runtime.
//
// Provides:
//   1. Capability tokens — fine-grained permission grants
//   2. ResourceLimits — mem, CPU, syscall budgets per sandbox
//   3. Sandbox — isolated execution environment per agent
//   4. Capability attenuation — derive restricted child capabilities
//   5. AuditLog — immutable record of all sandbox events

use std::collections::{BTreeMap, BTreeSet};

// ── Capability token ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityToken {
    pub name: String,
    pub scope: CapScope,
    pub attenuated_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapScope {
    /// Full access for this capability.
    Full,
    /// Restricted to specific targets (e.g., file paths, modules).
    Restricted(BTreeSet<String>),
    /// Read-only variant of a capability.
    ReadOnly,
}

impl CapabilityToken {
    pub fn full(name: &str) -> Self {
        Self { name: name.into(), scope: CapScope::Full, attenuated_from: None }
    }

    pub fn read_only(name: &str) -> Self {
        Self { name: name.into(), scope: CapScope::ReadOnly, attenuated_from: None }
    }

    pub fn restricted(name: &str, targets: BTreeSet<String>) -> Self {
        Self { name: name.into(), scope: CapScope::Restricted(targets), attenuated_from: None }
    }

    /// Attenuate: derive a more restricted capability from this one.
    pub fn attenuate(&self, new_scope: CapScope) -> Self {
        Self {
            name: self.name.clone(),
            scope: new_scope,
            attenuated_from: Some(self.name.clone()),
        }
    }

    /// Check whether this token grants access to a given target.
    pub fn allows(&self, target: &str) -> bool {
        match &self.scope {
            CapScope::Full => true,
            CapScope::ReadOnly => true,
            CapScope::Restricted(set) => set.contains(target),
        }
    }

    pub fn is_read_only(&self) -> bool {
        matches!(self.scope, CapScope::ReadOnly)
    }
}

// ── Resource limits ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Max memory in bytes (0 = unlimited).
    pub max_memory_bytes: u64,
    /// Max CPU time in milliseconds (0 = unlimited).
    pub max_cpu_ms: u64,
    /// Max number of syscalls (0 = unlimited).
    pub max_syscalls: u64,
    /// Max number of file operations (0 = unlimited).
    pub max_file_ops: u64,
    /// Max number of network operations (0 = unlimited).
    pub max_network_ops: u64,
}

impl ResourceLimits {
    pub fn unlimited() -> Self {
        Self {
            max_memory_bytes: 0,
            max_cpu_ms: 0,
            max_syscalls: 0,
            max_file_ops: 0,
            max_network_ops: 0,
        }
    }

    pub fn strict(mem_mb: u64, cpu_ms: u64, syscalls: u64) -> Self {
        Self {
            max_memory_bytes: mem_mb * 1024 * 1024,
            max_cpu_ms: cpu_ms,
            max_syscalls: syscalls,
            max_file_ops: 100,
            max_network_ops: 10,
        }
    }
}

// ── Resource usage ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub memory_bytes: u64,
    pub cpu_ms: u64,
    pub syscalls: u64,
    pub file_ops: u64,
    pub network_ops: u64,
}

impl ResourceUsage {
    /// Check whether usage exceeds any non-zero limit.
    pub fn exceeds(&self, limits: &ResourceLimits) -> Option<String> {
        if limits.max_memory_bytes > 0 && self.memory_bytes > limits.max_memory_bytes {
            return Some(format!(
                "memory: {} > {}",
                self.memory_bytes, limits.max_memory_bytes
            ));
        }
        if limits.max_cpu_ms > 0 && self.cpu_ms > limits.max_cpu_ms {
            return Some(format!("cpu: {} > {}", self.cpu_ms, limits.max_cpu_ms));
        }
        if limits.max_syscalls > 0 && self.syscalls > limits.max_syscalls {
            return Some(format!(
                "syscalls: {} > {}",
                self.syscalls, limits.max_syscalls
            ));
        }
        if limits.max_file_ops > 0 && self.file_ops > limits.max_file_ops {
            return Some(format!(
                "file_ops: {} > {}",
                self.file_ops, limits.max_file_ops
            ));
        }
        if limits.max_network_ops > 0 && self.network_ops > limits.max_network_ops {
            return Some(format!(
                "network_ops: {} > {}",
                self.network_ops, limits.max_network_ops
            ));
        }
        None
    }
}

// ── Audit event ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub agent_id: String,
    pub sandbox_id: String,
    pub kind: AuditEventKind,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventKind {
    CapabilityGranted,
    CapabilityDenied,
    CapabilityAttenuated,
    ResourceLimitExceeded,
    SandboxCreated,
    SandboxDestroyed,
    OperationPerformed,
}

// ── Audit log ──────────────────────────────────────────────────────

pub struct AuditLog {
    events: Vec<AuditEvent>,
    next_ts: u64,
}

impl AuditLog {
    pub fn new() -> Self {
        Self { events: Vec::new(), next_ts: 1 }
    }

    pub fn record(
        &mut self,
        agent_id: &str,
        sandbox_id: &str,
        kind: AuditEventKind,
        detail: &str,
    ) {
        let ts = self.next_ts;
        self.next_ts += 1;
        self.events.push(AuditEvent {
            timestamp: ts,
            agent_id: agent_id.into(),
            sandbox_id: sandbox_id.into(),
            kind,
            detail: detail.into(),
        });
    }

    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    pub fn events_for_agent(&self, agent_id: &str) -> Vec<&AuditEvent> {
        self.events.iter().filter(|e| e.agent_id == agent_id).collect()
    }

    pub fn denials(&self) -> Vec<&AuditEvent> {
        self.events
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    AuditEventKind::CapabilityDenied | AuditEventKind::ResourceLimitExceeded
                )
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

// ── Sandbox ────────────────────────────────────────────────────────

pub struct Sandbox {
    pub id: String,
    pub agent_id: String,
    capabilities: BTreeMap<String, CapabilityToken>,
    pub limits: ResourceLimits,
    pub usage: ResourceUsage,
    active: bool,
}

impl Sandbox {
    pub fn new(id: &str, agent_id: &str, limits: ResourceLimits) -> Self {
        Self {
            id: id.into(),
            agent_id: agent_id.into(),
            capabilities: BTreeMap::new(),
            limits,
            usage: ResourceUsage::default(),
            active: true,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn destroy(&mut self) {
        self.active = false;
    }

    /// Grant a capability to this sandbox.
    pub fn grant(&mut self, cap: CapabilityToken) {
        self.capabilities.insert(cap.name.clone(), cap);
    }

    /// Revoke a capability by name.
    pub fn revoke(&mut self, name: &str) -> bool {
        self.capabilities.remove(name).is_some()
    }

    /// Check if the sandbox has a specific capability.
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.contains_key(name)
    }

    /// Check access to a target via a named capability.
    pub fn check_access(&self, capability_name: &str, target: &str) -> bool {
        self.capabilities
            .get(capability_name)
            .map(|c| c.allows(target))
            .unwrap_or(false)
    }

    /// Record resource consumption; returns error if limit exceeded.
    pub fn consume(
        &mut self,
        memory: u64,
        cpu: u64,
        syscalls: u64,
        file_ops: u64,
        network_ops: u64,
    ) -> Result<(), String> {
        self.usage.memory_bytes += memory;
        self.usage.cpu_ms += cpu;
        self.usage.syscalls += syscalls;
        self.usage.file_ops += file_ops;
        self.usage.network_ops += network_ops;

        if let Some(violation) = self.usage.exceeds(&self.limits) {
            Err(violation)
        } else {
            Ok(())
        }
    }

    pub fn capability_names(&self) -> Vec<&str> {
        self.capabilities.keys().map(|s| s.as_str()).collect()
    }

    pub fn capabilities(&self) -> &BTreeMap<String, CapabilityToken> {
        &self.capabilities
    }
}

// ── Sandbox Manager ────────────────────────────────────────────────

/// Manages multiple sandboxes and the audit log.
pub struct SandboxManager {
    sandboxes: BTreeMap<String, Sandbox>,
    pub audit: AuditLog,
    next_id: u64,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            sandboxes: BTreeMap::new(),
            audit: AuditLog::new(),
            next_id: 1,
        }
    }

    /// Create a new sandbox for an agent.
    pub fn create_sandbox(&mut self, agent_id: &str, limits: ResourceLimits) -> String {
        let id = format!("sandbox-{}", self.next_id);
        self.next_id += 1;
        let sb = Sandbox::new(&id, agent_id, limits);
        self.audit.record(
            agent_id,
            &id,
            AuditEventKind::SandboxCreated,
            "Sandbox created",
        );
        self.sandboxes.insert(id.clone(), sb);
        id
    }

    /// Destroy a sandbox.
    pub fn destroy_sandbox(&mut self, sandbox_id: &str) -> bool {
        if let Some(sb) = self.sandboxes.get_mut(sandbox_id) {
            let agent = sb.agent_id.clone();
            sb.destroy();
            self.audit.record(
                &agent,
                sandbox_id,
                AuditEventKind::SandboxDestroyed,
                "Sandbox destroyed",
            );
            true
        } else {
            false
        }
    }

    /// Grant a capability to a sandbox, with audit.
    pub fn grant_capability(&mut self, sandbox_id: &str, cap: CapabilityToken) -> bool {
        if let Some(sb) = self.sandboxes.get_mut(sandbox_id) {
            let agent = sb.agent_id.clone();
            let name = cap.name.clone();
            sb.grant(cap);
            self.audit.record(
                &agent,
                sandbox_id,
                AuditEventKind::CapabilityGranted,
                &format!("Granted: {}", name),
            );
            true
        } else {
            false
        }
    }

    /// Check access and audit the result.
    pub fn check_access(&mut self, sandbox_id: &str, cap_name: &str, target: &str) -> bool {
        if let Some(sb) = self.sandboxes.get(sandbox_id) {
            let agent = sb.agent_id.clone();
            let allowed = sb.check_access(cap_name, target);
            if allowed {
                self.audit.record(
                    &agent,
                    sandbox_id,
                    AuditEventKind::OperationPerformed,
                    &format!("Access {}/{}: allowed", cap_name, target),
                );
            } else {
                self.audit.record(
                    &agent,
                    sandbox_id,
                    AuditEventKind::CapabilityDenied,
                    &format!("Access {}/{}: denied", cap_name, target),
                );
            }
            allowed
        } else {
            false
        }
    }

    /// Record resource consumption in a sandbox, with audit on limit breach.
    pub fn consume(
        &mut self,
        sandbox_id: &str,
        memory: u64,
        cpu: u64,
        syscalls: u64,
    ) -> Result<(), String> {
        if let Some(sb) = self.sandboxes.get_mut(sandbox_id) {
            let agent = sb.agent_id.clone();
            match sb.consume(memory, cpu, syscalls, 0, 0) {
                Ok(()) => Ok(()),
                Err(violation) => {
                    self.audit.record(
                        &agent,
                        sandbox_id,
                        AuditEventKind::ResourceLimitExceeded,
                        &violation,
                    );
                    Err(violation)
                }
            }
        } else {
            Err("Sandbox not found".into())
        }
    }

    pub fn get_sandbox(&self, sandbox_id: &str) -> Option<&Sandbox> {
        self.sandboxes.get(sandbox_id)
    }

    pub fn active_sandboxes(&self) -> Vec<&Sandbox> {
        self.sandboxes.values().filter(|s| s.is_active()).collect()
    }

    pub fn stats(&self) -> String {
        let total = self.sandboxes.len();
        let active = self.sandboxes.values().filter(|s| s.is_active()).count();
        let events = self.audit.len();
        let denials = self.audit.denials().len();
        format!(
            "{{\"sandboxes\":{},\"active\":{},\"events\":{},\"denials\":{}}}",
            total, active, events, denials
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Capability tokens ─────────────────────────────────────────

    #[test]
    fn full_cap_allows_anything() {
        let cap = CapabilityToken::full("fs");
        assert!(cap.allows("any/path"));
        assert!(cap.allows("other"));
    }

    #[test]
    fn restricted_cap_allows_only_targets() {
        let mut targets = BTreeSet::new();
        targets.insert("src/main.rs".into());
        let cap = CapabilityToken::restricted("fs", targets);
        assert!(cap.allows("src/main.rs"));
        assert!(!cap.allows("src/lib.rs"));
    }

    #[test]
    fn read_only_cap() {
        let cap = CapabilityToken::read_only("fs");
        assert!(cap.is_read_only());
        assert!(cap.allows("anything"));
    }

    #[test]
    fn attenuate_cap() {
        let parent = CapabilityToken::full("fs");
        let child = parent.attenuate(CapScope::ReadOnly);
        assert!(child.is_read_only());
        assert_eq!(child.attenuated_from.as_deref(), Some("fs"));
    }

    // ── Resource limits ───────────────────────────────────────────

    #[test]
    fn unlimited_never_exceeds() {
        let limits = ResourceLimits::unlimited();
        let usage = ResourceUsage {
            memory_bytes: u64::MAX,
            cpu_ms: u64::MAX,
            syscalls: u64::MAX,
            file_ops: u64::MAX,
            network_ops: u64::MAX,
        };
        assert!(usage.exceeds(&limits).is_none());
    }

    #[test]
    fn strict_limits_exceeded() {
        let limits = ResourceLimits::strict(1, 100, 50);
        let usage = ResourceUsage {
            memory_bytes: 2 * 1024 * 1024,
            cpu_ms: 0,
            syscalls: 0,
            file_ops: 0,
            network_ops: 0,
        };
        assert!(usage.exceeds(&limits).unwrap().contains("memory"));
    }

    // ── Sandbox ───────────────────────────────────────────────────

    #[test]
    fn sandbox_lifecycle() {
        let mut sb = Sandbox::new("sb-1", "agent-a", ResourceLimits::unlimited());
        assert!(sb.is_active());
        sb.grant(CapabilityToken::full("fs"));
        assert!(sb.has_capability("fs"));
        assert!(sb.check_access("fs", "any"));
        sb.revoke("fs");
        assert!(!sb.has_capability("fs"));
        sb.destroy();
        assert!(!sb.is_active());
    }

    #[test]
    fn sandbox_denies_without_cap() {
        let sb = Sandbox::new("sb-1", "agent-a", ResourceLimits::unlimited());
        assert!(!sb.check_access("fs", "foo"));
    }

    #[test]
    fn sandbox_resource_consumption_ok() {
        let mut sb = Sandbox::new("sb-1", "agent-a", ResourceLimits::strict(10, 1000, 100));
        assert!(sb.consume(1024, 10, 5, 0, 0).is_ok());
    }

    #[test]
    fn sandbox_resource_consumption_exceeded() {
        let mut sb = Sandbox::new("sb-1", "agent-a", ResourceLimits::strict(1, 100, 50));
        let result = sb.consume(2 * 1024 * 1024, 0, 0, 0, 0);
        assert!(result.is_err());
    }

    // ── Sandbox manager ───────────────────────────────────────────

    #[test]
    fn manager_create_destroy() {
        let mut mgr = SandboxManager::new();
        let id = mgr.create_sandbox("agent-a", ResourceLimits::unlimited());
        assert!(mgr.get_sandbox(&id).unwrap().is_active());
        assert_eq!(mgr.active_sandboxes().len(), 1);
        mgr.destroy_sandbox(&id);
        assert!(!mgr.get_sandbox(&id).unwrap().is_active());
        assert_eq!(mgr.active_sandboxes().len(), 0);
    }

    #[test]
    fn manager_grant_and_check_access() {
        let mut mgr = SandboxManager::new();
        let id = mgr.create_sandbox("agent-a", ResourceLimits::unlimited());
        mgr.grant_capability(&id, CapabilityToken::full("fs"));
        assert!(mgr.check_access(&id, "fs", "any"));
        assert!(!mgr.check_access(&id, "network", "any"));
    }

    #[test]
    fn manager_access_denied_audit() {
        let mut mgr = SandboxManager::new();
        let id = mgr.create_sandbox("agent-a", ResourceLimits::unlimited());
        mgr.check_access(&id, "fs", "secret");
        let denials = mgr.audit.denials();
        assert_eq!(denials.len(), 1);
        assert_eq!(denials[0].kind, AuditEventKind::CapabilityDenied);
    }

    #[test]
    fn manager_resource_limit_audit() {
        let mut mgr = SandboxManager::new();
        let id = mgr.create_sandbox("agent-a", ResourceLimits::strict(1, 100, 50));
        let _ = mgr.consume(&id, 2 * 1024 * 1024, 0, 0);
        let denials = mgr.audit.denials();
        assert_eq!(denials.len(), 1);
        assert_eq!(denials[0].kind, AuditEventKind::ResourceLimitExceeded);
    }

    // ── Audit log ─────────────────────────────────────────────────

    #[test]
    fn audit_events_for_agent() {
        let mut mgr = SandboxManager::new();
        let id_a = mgr.create_sandbox("agent-a", ResourceLimits::unlimited());
        mgr.create_sandbox("agent-b", ResourceLimits::unlimited());
        mgr.grant_capability(&id_a, CapabilityToken::full("fs"));
        let events = mgr.audit.events_for_agent("agent-a");
        assert_eq!(events.len(), 2); // created + granted
    }

    #[test]
    fn audit_log_ordering() {
        let mut log = AuditLog::new();
        log.record("a", "sb-1", AuditEventKind::SandboxCreated, "first");
        log.record("a", "sb-1", AuditEventKind::CapabilityGranted, "second");
        assert_eq!(log.events()[0].timestamp, 1);
        assert_eq!(log.events()[1].timestamp, 2);
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats_json() {
        let mut mgr = SandboxManager::new();
        mgr.create_sandbox("agent-a", ResourceLimits::unlimited());
        let s = mgr.stats();
        assert!(s.contains("\"sandboxes\":1"));
        assert!(s.contains("\"active\":1"));
    }

    // ── Capability attenuation chain ──────────────────────────────

    #[test]
    fn attenuation_restricts() {
        let parent = CapabilityToken::full("net");
        let mut targets = BTreeSet::new();
        targets.insert("api.example.com".into());
        let child = parent.attenuate(CapScope::Restricted(targets));
        assert!(child.allows("api.example.com"));
        assert!(!child.allows("evil.com"));
    }

    #[test]
    fn capability_names_listing() {
        let mut sb = Sandbox::new("sb-1", "agent-a", ResourceLimits::unlimited());
        sb.grant(CapabilityToken::full("fs"));
        sb.grant(CapabilityToken::full("net"));
        let names = sb.capability_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"fs"));
        assert!(names.contains(&"net"));
    }
}
