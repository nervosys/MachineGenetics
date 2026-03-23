// redox_sandbox: Capability-based sandbox runtime for agent-generated code.
//
// Defines capabilities (file, network, memory, etc.), sandbox policies,
// execution contexts, and a sandbox runtime that enforces capability
// checks before allowing operations.

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Capability
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    FileRead,
    FileWrite,
    NetworkAccess,
    MemoryAlloc,
    ProcessSpawn,
    FfiCall,
    TimerAccess,
    EnvRead,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::FileRead => "file-read",
            Self::FileWrite => "file-write",
            Self::NetworkAccess => "network",
            Self::MemoryAlloc => "memory-alloc",
            Self::ProcessSpawn => "process-spawn",
            Self::FfiCall => "ffi-call",
            Self::TimerAccess => "timer",
            Self::EnvRead => "env-read",
        }
    }

    pub fn all() -> &'static [Capability] {
        &[
            Self::FileRead, Self::FileWrite, Self::NetworkAccess,
            Self::MemoryAlloc, Self::ProcessSpawn, Self::FfiCall,
            Self::TimerAccess, Self::EnvRead,
        ]
    }
}

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_memory_bytes: u64,
    pub max_cpu_ms: u64,
    pub max_open_files: u32,
    pub max_network_connections: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024,
            max_cpu_ms: 5000,
            max_open_files: 16,
            max_network_connections: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Sandbox policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub name: String,
    pub capabilities: HashSet<Capability>,
    pub limits: ResourceLimits,
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
}

impl SandboxPolicy {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            capabilities: HashSet::new(),
            limits: ResourceLimits::default(),
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
        }
    }

    pub fn allow(&mut self, cap: Capability) -> &mut Self {
        self.capabilities.insert(cap);
        self
    }

    pub fn deny(&mut self, cap: Capability) -> &mut Self {
        self.capabilities.remove(&cap);
        self
    }

    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
}

// ---------------------------------------------------------------------------
// Pre-built policies
// ---------------------------------------------------------------------------

pub fn minimal_policy() -> SandboxPolicy {
    let mut p = SandboxPolicy::new("minimal");
    p.allow(Capability::MemoryAlloc);
    p.allow(Capability::TimerAccess);
    p
}

pub fn compute_policy() -> SandboxPolicy {
    let mut p = SandboxPolicy::new("compute");
    p.allow(Capability::MemoryAlloc);
    p.allow(Capability::TimerAccess);
    p.allow(Capability::FfiCall);
    p.limits.max_memory_bytes = 256 * 1024 * 1024;
    p.limits.max_cpu_ms = 30000;
    p
}

pub fn io_policy() -> SandboxPolicy {
    let mut p = SandboxPolicy::new("io");
    p.allow(Capability::MemoryAlloc);
    p.allow(Capability::TimerAccess);
    p.allow(Capability::FileRead);
    p.allow(Capability::FileWrite);
    p.allow(Capability::EnvRead);
    p.limits.max_open_files = 64;
    p
}

pub fn full_policy() -> SandboxPolicy {
    let mut p = SandboxPolicy::new("full");
    for cap in Capability::all() {
        p.allow(*cap);
    }
    p.limits.max_memory_bytes = 1024 * 1024 * 1024;
    p.limits.max_cpu_ms = 120000;
    p.limits.max_open_files = 256;
    p.limits.max_network_connections = 32;
    p
}

// ---------------------------------------------------------------------------
// Sandbox error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    CapabilityDenied(Capability),
    ResourceExceeded(String),
    PathDenied(String),
    AgentNotFound(String),
    PolicyNotFound(String),
}

impl std::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityDenied(c) => write!(f, "capability denied: {}", c.label()),
            Self::ResourceExceeded(m) => write!(f, "resource exceeded: {m}"),
            Self::PathDenied(p) => write!(f, "path denied: {p}"),
            Self::AgentNotFound(a) => write!(f, "agent not found: {a}"),
            Self::PolicyNotFound(p) => write!(f, "policy not found: {p}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Execution context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecContext {
    pub agent_id: String,
    pub policy_name: String,
    pub memory_used: u64,
    pub cpu_used_ms: u64,
    pub open_files: u32,
    pub network_conns: u32,
    pub operations: Vec<String>,
}

impl ExecContext {
    pub fn new(agent_id: &str, policy_name: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            policy_name: policy_name.to_string(),
            memory_used: 0,
            cpu_used_ms: 0,
            open_files: 0,
            network_conns: 0,
            operations: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Sandbox runtime
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct SandboxRuntime {
    policies: HashMap<String, SandboxPolicy>,
    contexts: HashMap<String, ExecContext>,
    audit_log: Vec<AuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub agent_id: String,
    pub operation: String,
    pub allowed: bool,
}

impl SandboxRuntime {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            contexts: HashMap::new(),
            audit_log: Vec::new(),
        }
    }

    pub fn register_policy(&mut self, policy: SandboxPolicy) {
        self.policies.insert(policy.name.clone(), policy);
    }

    pub fn spawn_agent(
        &mut self,
        agent_id: &str,
        policy_name: &str,
    ) -> Result<(), SandboxError> {
        if !self.policies.contains_key(policy_name) {
            return Err(SandboxError::PolicyNotFound(policy_name.to_string()));
        }
        let ctx = ExecContext::new(agent_id, policy_name);
        self.contexts.insert(agent_id.to_string(), ctx);
        Ok(())
    }

    pub fn check_capability(
        &mut self,
        agent_id: &str,
        cap: Capability,
    ) -> Result<(), SandboxError> {
        let ctx = self.contexts.get(agent_id)
            .ok_or_else(|| SandboxError::AgentNotFound(agent_id.to_string()))?;
        let policy = self.policies.get(&ctx.policy_name)
            .ok_or_else(|| SandboxError::PolicyNotFound(ctx.policy_name.clone()))?;

        let allowed = policy.has(cap);
        self.audit_log.push(AuditEntry {
            agent_id: agent_id.to_string(),
            operation: cap.label().to_string(),
            allowed,
        });

        if allowed { Ok(()) } else { Err(SandboxError::CapabilityDenied(cap)) }
    }

    pub fn record_memory(
        &mut self,
        agent_id: &str,
        bytes: u64,
    ) -> Result<(), SandboxError> {
        let ctx = self.contexts.get_mut(agent_id)
            .ok_or_else(|| SandboxError::AgentNotFound(agent_id.to_string()))?;
        let policy_name = ctx.policy_name.clone();
        let policy = self.policies.get(&policy_name)
            .ok_or_else(|| SandboxError::PolicyNotFound(policy_name))?;

        if ctx.memory_used + bytes > policy.limits.max_memory_bytes {
            return Err(SandboxError::ResourceExceeded("memory".to_string()));
        }
        ctx.memory_used += bytes;
        Ok(())
    }

    pub fn context(&self, agent_id: &str) -> Option<&ExecContext> {
        self.contexts.get(agent_id)
    }

    pub fn policy(&self, name: &str) -> Option<&SandboxPolicy> {
        self.policies.get(name)
    }

    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    pub fn agent_count(&self) -> usize {
        self.contexts.len()
    }

    pub fn policy_count(&self) -> usize {
        self.policies.len()
    }

    pub fn summary(&self) -> RuntimeStats {
        RuntimeStats {
            policies: self.policy_count(),
            agents: self.agent_count(),
            audit_entries: self.audit_log.len(),
            denied_count: self.audit_log.iter().filter(|e| !e.allowed).count(),
        }
    }
}

impl Default for SandboxRuntime {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStats {
    pub policies: usize,
    pub agents: usize,
    pub audit_entries: usize,
    pub denied_count: usize,
}

// ---------------------------------------------------------------------------
// Pre-built runtime
// ---------------------------------------------------------------------------

pub fn build_sample_runtime() -> SandboxRuntime {
    let mut rt = SandboxRuntime::new();
    rt.register_policy(minimal_policy());
    rt.register_policy(compute_policy());
    rt.register_policy(io_policy());
    rt.register_policy(full_policy());
    let _ = rt.spawn_agent("agent-1", "minimal");
    let _ = rt.spawn_agent("agent-2", "compute");
    rt
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Capability --
    #[test]
    fn test_capability_label() {
        assert_eq!(Capability::FileRead.label(), "file-read");
        assert_eq!(Capability::FfiCall.label(), "ffi-call");
    }

    #[test]
    fn test_capability_all() {
        assert_eq!(Capability::all().len(), 8);
    }

    // -- SandboxPolicy --
    #[test]
    fn test_policy_allow_deny() {
        let mut p = SandboxPolicy::new("test");
        p.allow(Capability::FileRead);
        assert!(p.has(Capability::FileRead));
        p.deny(Capability::FileRead);
        assert!(!p.has(Capability::FileRead));
    }

    #[test]
    fn test_minimal_policy() {
        let p = minimal_policy();
        assert!(p.has(Capability::MemoryAlloc));
        assert!(!p.has(Capability::NetworkAccess));
    }

    #[test]
    fn test_full_policy() {
        let p = full_policy();
        for cap in Capability::all() {
            assert!(p.has(*cap));
        }
    }

    // -- ResourceLimits --
    #[test]
    fn test_default_limits() {
        let l = ResourceLimits::default();
        assert_eq!(l.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(l.max_network_connections, 0);
    }

    // -- SandboxRuntime --
    #[test]
    fn test_register_policy() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        assert_eq!(rt.policy_count(), 1);
    }

    #[test]
    fn test_spawn_agent() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        assert_eq!(rt.agent_count(), 1);
    }

    #[test]
    fn test_spawn_agent_bad_policy() {
        let mut rt = SandboxRuntime::new();
        let err = rt.spawn_agent("a1", "nonexistent").unwrap_err();
        assert_eq!(err, SandboxError::PolicyNotFound("nonexistent".into()));
    }

    #[test]
    fn test_check_capability_allowed() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        assert!(rt.check_capability("a1", Capability::MemoryAlloc).is_ok());
    }

    #[test]
    fn test_check_capability_denied() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        let err = rt.check_capability("a1", Capability::NetworkAccess).unwrap_err();
        assert_eq!(err, SandboxError::CapabilityDenied(Capability::NetworkAccess));
    }

    #[test]
    fn test_check_capability_unknown_agent() {
        let mut rt = SandboxRuntime::new();
        let err = rt.check_capability("ghost", Capability::FileRead).unwrap_err();
        assert_eq!(err, SandboxError::AgentNotFound("ghost".into()));
    }

    #[test]
    fn test_record_memory() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        rt.record_memory("a1", 1024).unwrap();
        assert_eq!(rt.context("a1").unwrap().memory_used, 1024);
    }

    #[test]
    fn test_record_memory_exceeded() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        let err = rt.record_memory("a1", u64::MAX).unwrap_err();
        assert_eq!(err, SandboxError::ResourceExceeded("memory".into()));
    }

    #[test]
    fn test_audit_log() {
        let mut rt = SandboxRuntime::new();
        rt.register_policy(minimal_policy());
        rt.spawn_agent("a1", "minimal").unwrap();
        let _ = rt.check_capability("a1", Capability::MemoryAlloc);
        let _ = rt.check_capability("a1", Capability::NetworkAccess);
        assert_eq!(rt.audit_log().len(), 2);
        assert!(rt.audit_log()[0].allowed);
        assert!(!rt.audit_log()[1].allowed);
    }

    #[test]
    fn test_summary() {
        let rt = build_sample_runtime();
        let s = rt.summary();
        assert_eq!(s.policies, 4);
        assert_eq!(s.agents, 2);
    }

    #[test]
    fn test_sample_runtime() {
        let rt = build_sample_runtime();
        assert_eq!(rt.policy_count(), 4);
        assert_eq!(rt.agent_count(), 2);
    }

    #[test]
    fn test_default_runtime() {
        let rt = SandboxRuntime::default();
        assert_eq!(rt.agent_count(), 0);
    }

    // -- SandboxError display --
    #[test]
    fn test_error_display() {
        let e = SandboxError::CapabilityDenied(Capability::FileWrite);
        assert!(format!("{e}").contains("file-write"));
    }

    #[test]
    fn test_path_denied_display() {
        let e = SandboxError::PathDenied("/etc/passwd".into());
        assert!(format!("{e}").contains("/etc/passwd"));
    }

    // -- context / policy getters --
    #[test]
    fn test_context_getter() {
        let rt = build_sample_runtime();
        let ctx = rt.context("agent-1").unwrap();
        assert_eq!(ctx.policy_name, "minimal");
    }

    #[test]
    fn test_policy_getter() {
        let rt = build_sample_runtime();
        let p = rt.policy("compute").unwrap();
        assert!(p.has(Capability::FfiCall));
    }
}
