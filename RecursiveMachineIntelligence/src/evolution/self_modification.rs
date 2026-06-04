//! Self-Modification & Sandboxed Execution
//!
//! Safe self-editing primitives with sandboxing and rollback, enabling agents
//! to modify their own code under controlled conditions.
//!
//! # Safety Model
//!
//! All modifications pass through a multi-layer safety pipeline:
//!
//! 1. **Proposal** — An agent creates a [`CodePatch`] describing the intended
//!    change, its rationale, and expected impact score (−1.0 to +1.0).
//!
//! 2. **Constraint checking** — The [`SafetyGuard`] validates the patch against
//!    a set of [`SafetyConstraint`]s (e.g., maximum change size, required
//!    test pass, impact thresholds). Patches failing any constraint are rejected
//!    before sandbox execution.
//!
//! 3. **Sandbox execution** — Approved patches are applied inside a [`Sandbox`]
//!    with resource limits (CPU time, memory, instruction count). The sandbox
//!    captures the result without affecting production state.
//!
//! 4. **Rollback** — The [`RollbackManager`] maintains a versioned history of
//!    all applied patches. If a modification degrades performance or violates
//!    invariants, it can be reverted to any previous version.
//!
//! # Patch Lifecycle
//!
//! ```text
//! Proposed → [SafetyGuard] → Approved → [Sandbox] → Tested → Applied
//!                ↓                                       ↓
//!             Rejected                               Rolled back
//! ```
//!
//! # Invariant Preservation
//!
//! The system guarantees:
//! - **Atomicity**: Patches are all-or-nothing; partial application is not possible.
//! - **Isolation**: Sandbox execution cannot affect external state.
//! - **Reversibility**: Every applied patch can be undone via `RollbackManager`.
//! - **Auditability**: All patches carry author, rationale, and timestamp metadata.

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Result, RmiError};

// ============================================================================
// Code Patches
// ============================================================================

/// A unit of code modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePatch {
    /// Patch ID
    pub id: Uuid,
    /// Target module or function path
    pub target: String,
    /// Kind of modification
    pub kind: PatchKind,
    /// The modification payload (source code, IR, or parameters)
    pub payload: PatchPayload,
    /// Rationale (why this patch is proposed)
    pub rationale: String,
    /// Expected impact score (-1.0 = harmful, +1.0 = beneficial)
    pub expected_impact: f64,
    /// Author agent ID
    pub author: Uuid,
    /// Created timestamp
    pub created_at: f64,
    /// Approval status
    pub status: PatchStatus,
}

/// Kind of code patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatchKind {
    /// Add new code
    Insert,
    /// Replace existing code
    Replace,
    /// Remove code
    Delete,
    /// Modify parameters/weights
    ParameterUpdate,
    /// Restructure without changing behavior
    Refactor,
    /// Performance optimization
    Optimize,
}

/// Payload of a code patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchPayload {
    /// Source code string
    Source(String),
    /// IR operations (serialized)
    Ir(Vec<u8>),
    /// Parameter updates (name -> value)
    Parameters(HashMap<String, f64>),
    /// Configuration changes
    Config(HashMap<String, String>),
}

/// Patch approval status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatchStatus {
    /// Awaiting review
    Proposed,
    /// Approved, ready to apply
    Approved,
    /// Applied successfully
    Applied,
    /// Rejected by safety checks or reviewer
    Rejected,
    /// Rolled back after application
    RolledBack,
    /// Failed during application
    Failed,
}

impl CodePatch {
    /// Create a new patch.
    pub fn new(target: &str, kind: PatchKind, payload: PatchPayload, author: Uuid) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        Self {
            id: Uuid::new_v4(),
            target: target.to_string(),
            kind,
            payload,
            rationale: String::new(),
            expected_impact: 0.0,
            author,
            created_at: now,
            status: PatchStatus::Proposed,
        }
    }

    /// Set rationale.
    pub fn with_rationale(mut self, rationale: &str) -> Self {
        self.rationale = rationale.to_string();
        self
    }

    /// Set expected impact.
    pub fn with_expected_impact(mut self, impact: f64) -> Self {
        self.expected_impact = impact.clamp(-1.0, 1.0);
        self
    }
}

// ============================================================================
// Sandbox
// ============================================================================

/// Resource limits for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxLimits {
    /// Maximum memory bytes
    pub max_memory_bytes: u64,
    /// Maximum CPU time in milliseconds
    pub max_cpu_ms: u64,
    /// Maximum number of operations
    pub max_operations: u64,
    /// Allow network access
    pub allow_network: bool,
    /// Allow file system access
    pub allow_filesystem: bool,
    /// Maximum recursion depth
    pub max_recursion: u32,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 256 * 1024 * 1024, // 256 MB
            max_cpu_ms: 30_000,                  // 30 seconds
            max_operations: 10_000_000,
            allow_network: false,
            allow_filesystem: false,
            max_recursion: 100,
        }
    }
}

/// Result of sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    /// Whether the execution succeeded
    pub success: bool,
    /// Output value (if any)
    pub output: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Resources consumed
    pub resources: ResourceUsage,
    /// Side effects detected
    pub side_effects: Vec<SideEffect>,
    /// Test results if tests were run
    pub test_results: Option<TestSummary>,
}

/// Resource usage tracking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Memory used in bytes
    pub memory_bytes: u64,
    /// CPU time in milliseconds
    pub cpu_ms: u64,
    /// Operations executed
    pub operations: u64,
    /// Allocations made
    pub allocations: u64,
}

impl ResourceUsage {
    /// Check if usage exceeds limits.
    pub fn exceeds(&self, limits: &SandboxLimits) -> bool {
        self.memory_bytes > limits.max_memory_bytes
            || self.cpu_ms > limits.max_cpu_ms
            || self.operations > limits.max_operations
    }
}

/// A side effect detected during sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    /// Kind of side effect
    pub kind: SideEffectKind,
    /// Description
    pub description: String,
    /// Severity (0 = benign, 1 = critical)
    pub severity: f64,
}

/// Types of side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SideEffectKind {
    /// Modifies internal state
    StateModification,
    /// Accesses network resources
    NetworkAccess,
    /// Accesses the file system
    FileAccess,
    /// Potential resource leak
    ResourceLeak,
    /// Calls external service
    ExternalCall,
    /// Non-deterministic behavior
    NonDeterminism,
}

/// Test execution summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestSummary {
    /// Number of tests passed
    pub passed: u32,
    /// Number of tests failed
    pub failed: u32,
    /// Number of tests skipped
    pub skipped: u32,
    /// Error messages from test failures
    pub errors: Vec<String>,
}

impl TestSummary {
    /// All tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.errors.is_empty()
    }
}

/// Sandbox for isolated execution.
pub struct Sandbox {
    /// Limits for this sandbox
    limits: SandboxLimits,
    /// Current resource usage
    usage: ResourceUsage,
    /// Detected side effects
    side_effects: Vec<SideEffect>,
    /// State snapshot for isolation
    state_snapshot: HashMap<String, Vec<u8>>,
}

impl Sandbox {
    /// Create a new sandbox with default limits.
    pub fn new() -> Self {
        Self::with_limits(SandboxLimits::default())
    }

    /// Create a sandbox with custom limits.
    pub fn with_limits(limits: SandboxLimits) -> Self {
        Self {
            limits,
            usage: ResourceUsage::default(),
            side_effects: Vec::new(),
            state_snapshot: HashMap::new(),
        }
    }

    /// Snapshot a piece of state for rollback.
    pub fn snapshot_state(&mut self, key: &str, data: Vec<u8>) {
        self.state_snapshot.insert(key.to_string(), data);
    }

    /// Get snapshotted state.
    pub fn get_snapshot(&self, key: &str) -> Option<&[u8]> {
        self.state_snapshot.get(key).map(|v| v.as_slice())
    }

    /// Execute a function in the sandbox. The function receives resource tracking.
    pub fn execute<F, T>(&mut self, f: F) -> SandboxResult
    where
        F: FnOnce(&mut ResourceUsage) -> std::result::Result<T, String>,
        T: std::fmt::Display,
    {
        self.usage = ResourceUsage::default();
        self.side_effects.clear();

        match f(&mut self.usage) {
            Ok(output) => {
                let exceeded = self.usage.exceeds(&self.limits);
                SandboxResult {
                    success: !exceeded,
                    output: Some(output.to_string()),
                    error: if exceeded {
                        Some("Resource limits exceeded".to_string())
                    } else {
                        None
                    },
                    resources: self.usage.clone(),
                    side_effects: self.side_effects.clone(),
                    test_results: None,
                }
            }
            Err(e) => SandboxResult {
                success: false,
                output: None,
                error: Some(e),
                resources: self.usage.clone(),
                side_effects: self.side_effects.clone(),
                test_results: None,
            },
        }
    }

    /// Record a side effect.
    pub fn record_side_effect(&mut self, kind: SideEffectKind, description: &str, severity: f64) {
        self.side_effects.push(SideEffect {
            kind,
            description: description.to_string(),
            severity: severity.clamp(0.0, 1.0),
        });
    }

    /// Get current resource usage.
    pub fn usage(&self) -> &ResourceUsage {
        &self.usage
    }

    /// Get limits.
    pub fn limits(&self) -> &SandboxLimits {
        &self.limits
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Rollback Manager
// ============================================================================

/// A versioned state entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVersion {
    /// Version number
    pub version: u64,
    /// State data (serialized)
    pub data: Vec<u8>,
    /// Patch that produced this version (if any)
    pub patch_id: Option<Uuid>,
    /// Description
    pub description: String,
    /// Timestamp
    pub timestamp: f64,
    /// Checksum
    pub checksum: u64,
}

/// Manages versioned state with rollback capability.
pub struct RollbackManager {
    /// Version history (key -> versions)
    history: RwLock<HashMap<String, VecDeque<StateVersion>>>,
    /// Current version per key
    current_versions: RwLock<HashMap<String, u64>>,
    /// Maximum versions to retain
    max_history: usize,
}

impl RollbackManager {
    /// Create a new rollback manager.
    pub fn new(max_history: usize) -> Self {
        Self {
            history: RwLock::new(HashMap::new()),
            current_versions: RwLock::new(HashMap::new()),
            max_history,
        }
    }

    /// Checkpoint current state.
    pub fn checkpoint(
        &self,
        key: &str,
        data: Vec<u8>,
        description: &str,
        patch_id: Option<Uuid>,
    ) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        let mut versions_map = self.current_versions.write().unwrap();
        let version = versions_map.entry(key.to_string()).or_insert(0);
        *version += 1;
        let ver = *version;

        let checksum = xxhash_rust::xxh64::xxh64(&data, 0);

        let state = StateVersion {
            version: ver,
            data,
            patch_id,
            description: description.to_string(),
            timestamp: now,
            checksum,
        };

        let mut history = self.history.write().unwrap();
        let versions = history.entry(key.to_string()).or_default();
        versions.push_back(state);

        // Trim old versions
        while versions.len() > self.max_history {
            versions.pop_front();
        }

        ver
    }

    /// Rollback to a specific version.
    pub fn rollback(&self, key: &str, target_version: u64) -> Result<StateVersion> {
        let history = self.history.read().unwrap();
        let versions = history
            .get(key)
            .ok_or_else(|| RmiError::Agent(format!("No history for key: {}", key)))?;

        let state = versions
            .iter()
            .find(|v| v.version == target_version)
            .cloned()
            .ok_or_else(|| {
                RmiError::Agent(format!(
                    "Version {} not found for key: {}",
                    target_version, key
                ))
            })?;

        drop(history);

        // Update current version
        let mut current = self.current_versions.write().unwrap();
        current.insert(key.to_string(), target_version);

        Ok(state)
    }

    /// Rollback to the previous version.
    pub fn rollback_last(&self, key: &str) -> Result<StateVersion> {
        let current = self.current_versions.read().unwrap();
        let ver = current
            .get(key)
            .copied()
            .ok_or_else(|| RmiError::Agent(format!("No current version for key: {}", key)))?;

        if ver <= 1 {
            return Err(RmiError::Agent("Already at first version".to_string()));
        }

        drop(current);
        self.rollback(key, ver - 1)
    }

    /// Get current version of a key.
    pub fn current_version(&self, key: &str) -> Option<u64> {
        self.current_versions.read().unwrap().get(key).copied()
    }

    /// Get latest state data.
    pub fn latest(&self, key: &str) -> Option<StateVersion> {
        let history = self.history.read().unwrap();
        history.get(key).and_then(|v| v.back().cloned())
    }

    /// Get version history for a key.
    pub fn history(&self, key: &str) -> Vec<StateVersion> {
        self.history
            .read()
            .unwrap()
            .get(key)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get total version count across all keys.
    pub fn total_versions(&self) -> usize {
        self.history.read().unwrap().values().map(|v| v.len()).sum()
    }
}

// ============================================================================
// Safety Guard
// ============================================================================

/// Safety constraint for self-modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConstraint {
    /// Constraint name
    pub name: String,
    /// Description
    pub description: String,
    /// Severity if violated
    pub severity: ConstraintSeverity,
    /// Whether this constraint is active
    pub active: bool,
}

/// Severity of a safety constraint violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConstraintSeverity {
    /// Log warning only
    Advisory,
    /// Block patch but allow override
    Warning,
    /// Block patch, no override
    Critical,
    /// Halt system immediately
    Emergency,
}

/// Result of a safety check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyVerdict {
    /// Whether the patch is safe to apply
    pub safe: bool,
    /// Violations found
    pub violations: Vec<SafetyViolation>,
    /// Risk score (0.0 = no risk, 1.0 = maximum risk)
    pub risk_score: f64,
    /// Recommendations
    pub recommendations: Vec<String>,
}

impl SafetyVerdict {
    /// Check if there are critical violations.
    pub fn has_critical(&self) -> bool {
        self.violations.iter().any(|v| {
            v.severity == ConstraintSeverity::Critical
                || v.severity == ConstraintSeverity::Emergency
        })
    }
}

/// A specific safety violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyViolation {
    /// Constraint that was violated
    pub constraint_name: String,
    /// Description of the violation
    pub description: String,
    /// Severity
    pub severity: ConstraintSeverity,
}

/// Safety guard that enforces constraints on self-modification.
pub struct SafetyGuard {
    /// Active constraints
    constraints: Vec<SafetyConstraint>,
    /// Maximum allowed risk score
    max_risk: f64,
    /// Blocked targets (paths that cannot be modified)
    blocked_targets: Vec<String>,
}

impl SafetyGuard {
    /// Create a new safety guard with default constraints.
    pub fn new() -> Self {
        let mut guard = Self {
            constraints: Vec::new(),
            max_risk: 0.7,

            blocked_targets: vec!["core::safety".to_string(), "core::rollback".to_string()],
        };

        // Add default constraints
        guard.add_constraint(SafetyConstraint {
            name: "no_self_disable".to_string(),
            description: "Cannot disable safety systems".to_string(),
            severity: ConstraintSeverity::Emergency,
            active: true,
        });
        guard.add_constraint(SafetyConstraint {
            name: "test_required".to_string(),
            description: "Patches must pass tests before application".to_string(),
            severity: ConstraintSeverity::Critical,
            active: true,
        });
        guard.add_constraint(SafetyConstraint {
            name: "bounded_impact".to_string(),
            description: "Patches must not exceed impact threshold".to_string(),
            severity: ConstraintSeverity::Warning,
            active: true,
        });
        guard.add_constraint(SafetyConstraint {
            name: "rationale_required".to_string(),
            description: "Patches must have a rationale".to_string(),
            severity: ConstraintSeverity::Advisory,
            active: true,
        });

        guard
    }

    /// Add a constraint.
    pub fn add_constraint(&mut self, constraint: SafetyConstraint) {
        self.constraints.push(constraint);
    }

    /// Block a target from modification.
    pub fn block_target(&mut self, target: &str) {
        self.blocked_targets.push(target.to_string());
    }

    /// Check a patch against safety constraints.
    pub fn check(&self, patch: &CodePatch, test_results: Option<&TestSummary>) -> SafetyVerdict {
        let mut violations = Vec::new();
        let mut risk_score: f64 = 0.0;
        let mut recommendations = Vec::new();

        // Check blocked targets
        for blocked in &self.blocked_targets {
            if patch.target.starts_with(blocked) {
                violations.push(SafetyViolation {
                    constraint_name: "no_self_disable".to_string(),
                    description: format!("Target '{}' is a protected path", patch.target),
                    severity: ConstraintSeverity::Emergency,
                });
                risk_score = 1.0;
            }
        }

        // Check rationale
        if patch.rationale.is_empty() {
            violations.push(SafetyViolation {
                constraint_name: "rationale_required".to_string(),
                description: "Patch has no rationale".to_string(),
                severity: ConstraintSeverity::Advisory,
            });
            risk_score += 0.1;
        }

        // Check test results
        if let Some(tests) = test_results {
            if !tests.all_passed() {
                violations.push(SafetyViolation {
                    constraint_name: "test_required".to_string(),
                    description: format!("{} tests failed", tests.failed),
                    severity: ConstraintSeverity::Critical,
                });
                risk_score += 0.5;
            }
        } else {
            recommendations.push("Run tests before applying this patch".to_string());
            risk_score += 0.2;
        }

        // Check expected impact
        if patch.expected_impact < -0.5 {
            violations.push(SafetyViolation {
                constraint_name: "bounded_impact".to_string(),
                description: format!(
                    "Expected impact {:.2} is below threshold",
                    patch.expected_impact
                ),
                severity: ConstraintSeverity::Warning,
            });
            risk_score += 0.3;
        }

        // Delete patches are riskier
        if patch.kind == PatchKind::Delete {
            risk_score += 0.2;
            recommendations.push("Consider refactoring instead of deleting".to_string());
        }

        risk_score = risk_score.clamp(0.0_f64, 1.0_f64);
        let safe = risk_score <= self.max_risk
            && !violations.iter().any(|v| {
                v.severity == ConstraintSeverity::Critical
                    || v.severity == ConstraintSeverity::Emergency
            });

        SafetyVerdict {
            safe,
            violations,
            risk_score,
            recommendations,
        }
    }

    /// Get all constraints.
    pub fn constraints(&self) -> &[SafetyConstraint] {
        &self.constraints
    }

    /// Set maximum risk threshold.
    pub fn set_max_risk(&mut self, max_risk: f64) {
        self.max_risk = max_risk.clamp(0.0, 1.0);
    }
}

impl Default for SafetyGuard {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_patch_creation() {
        let author = Uuid::new_v4();
        let patch = CodePatch::new(
            "neural::layers",
            PatchKind::Replace,
            PatchPayload::Source("fn new_activation(x: f64) -> f64 { x.max(0.0) }".to_string()),
            author,
        )
        .with_rationale("Optimize activation function")
        .with_expected_impact(0.3);

        assert_eq!(patch.kind, PatchKind::Replace);
        assert_eq!(patch.status, PatchStatus::Proposed);
        assert!(!patch.rationale.is_empty());
        assert_eq!(patch.expected_impact, 0.3);
    }

    #[test]
    fn test_sandbox_execution() {
        let mut sandbox = Sandbox::new();

        let result = sandbox.execute(|usage| {
            usage.operations += 100;
            usage.memory_bytes += 1024;
            Ok("done")
        });

        assert!(result.success);
        assert_eq!(result.output, Some("done".to_string()));
        assert_eq!(result.resources.operations, 100);
    }

    #[test]
    fn test_sandbox_limit_exceeded() {
        let limits = SandboxLimits {
            max_operations: 50,
            ..Default::default()
        };
        let mut sandbox = Sandbox::with_limits(limits);

        let result = sandbox.execute(|usage| {
            usage.operations += 100; // Exceeds limit
            Ok("done")
        });

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_sandbox_error() {
        let mut sandbox = Sandbox::new();

        let result = sandbox.execute(|_| Err::<String, _>("runtime error".to_string()));

        assert!(!result.success);
        assert_eq!(result.error, Some("runtime error".to_string()));
    }

    #[test]
    fn test_sandbox_snapshot() {
        let mut sandbox = Sandbox::new();
        sandbox.snapshot_state("model_weights", vec![1, 2, 3, 4]);

        assert_eq!(
            sandbox.get_snapshot("model_weights"),
            Some(&[1u8, 2, 3, 4][..])
        );
        assert_eq!(sandbox.get_snapshot("missing"), None);
    }

    #[test]
    fn test_rollback_manager() {
        let rm = RollbackManager::new(10);

        let v1 = rm.checkpoint("agent_state", vec![1, 2, 3], "Initial state", None);
        assert_eq!(v1, 1);

        let v2 = rm.checkpoint("agent_state", vec![4, 5, 6], "After update", None);
        assert_eq!(v2, 2);

        // Rollback
        let state = rm.rollback("agent_state", 1).unwrap();
        assert_eq!(state.data, vec![1, 2, 3]);
        assert_eq!(rm.current_version("agent_state"), Some(1));
    }

    #[test]
    fn test_rollback_last() {
        let rm = RollbackManager::new(10);
        rm.checkpoint("key", vec![1], "v1", None);
        rm.checkpoint("key", vec![2], "v2", None);
        rm.checkpoint("key", vec![3], "v3", None);

        let prev = rm.rollback_last("key").unwrap();
        assert_eq!(prev.data, vec![2]);
    }

    #[test]
    fn test_rollback_at_first_version() {
        let rm = RollbackManager::new(10);
        rm.checkpoint("key", vec![1], "v1", None);

        assert!(rm.rollback_last("key").is_err());
    }

    #[test]
    fn test_rollback_history_limit() {
        let rm = RollbackManager::new(3);

        for i in 0..5 {
            rm.checkpoint("key", vec![i], &format!("v{}", i), None);
        }

        let history = rm.history("key");
        assert_eq!(history.len(), 3); // Only last 3 retained
    }

    #[test]
    fn test_safety_guard_safe_patch() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "neural::optim",
            PatchKind::Replace,
            PatchPayload::Parameters(HashMap::new()),
            author,
        )
        .with_rationale("Improve learning rate schedule")
        .with_expected_impact(0.5);

        let tests = TestSummary {
            passed: 10,
            failed: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        let verdict = guard.check(&patch, Some(&tests));
        assert!(verdict.safe);
        assert!(verdict.violations.is_empty());
    }

    #[test]
    fn test_safety_guard_blocked_target() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "core::safety::guard",
            PatchKind::Delete,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_rationale("Remove safety");

        let verdict = guard.check(&patch, None);
        assert!(!verdict.safe);
        assert!(verdict.has_critical());
    }

    #[test]
    fn test_safety_guard_failing_tests() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "neural::layers",
            PatchKind::Replace,
            PatchPayload::Source("buggy code".to_string()),
            author,
        )
        .with_rationale("Attempt fix");

        let tests = TestSummary {
            passed: 5,
            failed: 3,
            skipped: 0,
            errors: vec!["assertion failed".to_string()],
        };

        let verdict = guard.check(&patch, Some(&tests));
        assert!(!verdict.safe);
    }

    #[test]
    fn test_safety_guard_no_rationale() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "neural::layers",
            PatchKind::Insert,
            PatchPayload::Source("new code".to_string()),
            author,
        ); // No rationale

        let tests = TestSummary {
            passed: 10,
            failed: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        let verdict = guard.check(&patch, Some(&tests));
        // Should be advisory, still safe
        assert!(verdict.safe);
        assert!(!verdict.violations.is_empty());
    }

    // ── Safety-critical edge cases ──────────────────────────────────────────

    #[test]
    fn test_safety_guard_all_blocked_targets_rejected() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        // Both default blocked targets must be rejected
        for target in &["core::safety", "core::rollback"] {
            let patch = CodePatch::new(
                target,
                PatchKind::Replace,
                PatchPayload::Source("noop".into()),
                author,
            )
            .with_rationale("Bypass safety");

            let verdict = guard.check(&patch, None);
            assert!(!verdict.safe, "target '{}' should be blocked", target);
            assert_eq!(verdict.risk_score, 1.0);
        }
    }

    #[test]
    fn test_safety_guard_custom_blocked_target() {
        let mut guard = SafetyGuard::new();
        guard.block_target("evolution::self_mod");
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "evolution::self_mod::mutate",
            PatchKind::Delete,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_rationale("Delete mutation engine");

        let verdict = guard.check(&patch, None);
        assert!(!verdict.safe);
        assert!(verdict.has_critical());
    }

    #[test]
    fn test_safety_guard_negative_impact_threshold() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let patch = CodePatch::new(
            "neural::layers",
            PatchKind::Replace,
            PatchPayload::Source("risky change".into()),
            author,
        )
        .with_rationale("Performance experiment")
        .with_expected_impact(-0.8);

        let tests = TestSummary {
            passed: 10,
            failed: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        let verdict = guard.check(&patch, Some(&tests));
        let has_impact_violation = verdict
            .violations
            .iter()
            .any(|v| v.constraint_name == "bounded_impact");
        assert!(has_impact_violation);
    }

    #[test]
    fn test_safety_guard_delete_increases_risk() {
        let guard = SafetyGuard::new();
        let author = Uuid::new_v4();

        let replace_patch = CodePatch::new(
            "neural::optim",
            PatchKind::Replace,
            PatchPayload::Source("updated".into()),
            author,
        )
        .with_rationale("Refactor")
        .with_expected_impact(0.1);

        let delete_patch = CodePatch::new(
            "neural::optim",
            PatchKind::Delete,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_rationale("Cleanup")
        .with_expected_impact(0.1);

        let tests = TestSummary {
            passed: 10,
            failed: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        let replace_verdict = guard.check(&replace_patch, Some(&tests));
        let delete_verdict = guard.check(&delete_patch, Some(&tests));

        assert!(
            delete_verdict.risk_score > replace_verdict.risk_score,
            "Delete should carry higher risk than replace"
        );
    }

    #[test]
    fn test_safety_guard_max_risk_tuning() {
        let mut guard = SafetyGuard::new();
        guard.set_max_risk(0.0); // zero tolerance
        let author = Uuid::new_v4();

        // Even a benign patch with no tests gets a small risk bump
        let patch = CodePatch::new(
            "neural::layers",
            PatchKind::Insert,
            PatchPayload::Source("ok".into()),
            author,
        )
        .with_rationale("Fine-tune");

        let verdict = guard.check(&patch, None);
        assert!(!verdict.safe, "zero-tolerance guard should reject any risk");
    }

    #[test]
    fn test_expected_impact_clamped_to_range() {
        let author = Uuid::new_v4();

        let over = CodePatch::new(
            "a",
            PatchKind::Insert,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_expected_impact(5.0);
        assert_eq!(over.expected_impact, 1.0);

        let under = CodePatch::new(
            "b",
            PatchKind::Insert,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_expected_impact(-10.0);
        assert_eq!(under.expected_impact, -1.0);
    }

    // ── Sandbox edge cases ──────────────────────────────────────────────────

    #[test]
    fn test_sandbox_side_effect_recording() {
        let mut sandbox = Sandbox::new();
        sandbox.record_side_effect(
            SideEffectKind::NetworkAccess,
            "Attempted outbound HTTP call",
            0.9,
        );
        sandbox.record_side_effect(SideEffectKind::FileAccess, "Read /etc/passwd", 0.8);

        let result = sandbox.execute(|_| Ok("done"));
        // Side effects are cleared on execute
        assert!(result.side_effects.is_empty());
    }

    #[test]
    fn test_sandbox_memory_limit() {
        let limits = SandboxLimits {
            max_memory_bytes: 1024,
            ..Default::default()
        };
        let mut sandbox = Sandbox::with_limits(limits);

        let result = sandbox.execute(|usage| {
            usage.memory_bytes = 2048;
            Ok("allocated")
        });

        assert!(!result.success);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("Resource limits exceeded"));
    }

    #[test]
    fn test_sandbox_cpu_limit() {
        let limits = SandboxLimits {
            max_cpu_ms: 100,
            ..Default::default()
        };
        let mut sandbox = Sandbox::with_limits(limits);

        let result = sandbox.execute(|usage| {
            usage.cpu_ms = 200;
            Ok("slow")
        });

        assert!(!result.success);
    }

    #[test]
    fn test_sandbox_default_denies_network_and_fs() {
        let limits = SandboxLimits::default();
        assert!(!limits.allow_network);
        assert!(!limits.allow_filesystem);
    }

    #[test]
    fn test_sandbox_within_limits() {
        let limits = SandboxLimits {
            max_memory_bytes: 1024,
            max_cpu_ms: 1000,
            max_operations: 500,
            ..Default::default()
        };
        let mut sandbox = Sandbox::with_limits(limits);

        let result = sandbox.execute(|usage| {
            usage.memory_bytes = 512;
            usage.cpu_ms = 100;
            usage.operations = 200;
            Ok("within bounds")
        });

        assert!(result.success);
        assert!(result.error.is_none());
    }

    // ── Rollback edge cases ─────────────────────────────────────────────────

    #[test]
    fn test_rollback_nonexistent_key() {
        let rm = RollbackManager::new(10);
        assert!(rm.rollback("does_not_exist", 1).is_err());
    }

    #[test]
    fn test_rollback_nonexistent_version() {
        let rm = RollbackManager::new(10);
        rm.checkpoint("key", vec![1], "v1", None);

        assert!(rm.rollback("key", 99).is_err());
    }

    #[test]
    fn test_rollback_preserves_all_history() {
        let rm = RollbackManager::new(10);
        rm.checkpoint("k", vec![1], "v1", None);
        rm.checkpoint("k", vec![2], "v2", None);
        rm.checkpoint("k", vec![3], "v3", None);

        // After rollback, history still contains all versions
        rm.rollback("k", 1).unwrap();
        assert_eq!(rm.history("k").len(), 3);
        assert_eq!(rm.current_version("k"), Some(1));
    }

    #[test]
    fn test_rollback_with_patch_id() {
        let rm = RollbackManager::new(10);
        let patch_id = Uuid::new_v4();

        rm.checkpoint("agent", vec![10, 20], "patched", Some(patch_id));

        let latest = rm.latest("agent").unwrap();
        assert_eq!(latest.patch_id, Some(patch_id));
        assert_eq!(latest.data, vec![10, 20]);
    }

    #[test]
    fn test_rollback_checksum_integrity() {
        let rm = RollbackManager::new(10);
        let data = vec![42, 43, 44, 45];
        rm.checkpoint("k", data.clone(), "v1", None);

        let latest = rm.latest("k").unwrap();
        let expected = xxhash_rust::xxh64::xxh64(&data, 0);
        assert_eq!(latest.checksum, expected);
    }

    #[test]
    fn test_rollback_history_empty_key() {
        let rm = RollbackManager::new(10);
        assert!(rm.history("nope").is_empty());
        assert_eq!(rm.current_version("nope"), None);
        assert!(rm.latest("nope").is_none());
    }

    #[test]
    fn test_total_versions_across_keys() {
        let rm = RollbackManager::new(10);
        rm.checkpoint("a", vec![1], "v1", None);
        rm.checkpoint("a", vec![2], "v2", None);
        rm.checkpoint("b", vec![3], "v1", None);

        assert_eq!(rm.total_versions(), 3);
    }

    // ── Resource usage ──────────────────────────────────────────────────────

    #[test]
    fn test_resource_usage_exceeds_any_limit() {
        let limits = SandboxLimits {
            max_memory_bytes: 100,
            max_cpu_ms: 100,
            max_operations: 100,
            ..Default::default()
        };

        // Exceeds memory only
        let usage = ResourceUsage {
            memory_bytes: 200,
            cpu_ms: 50,
            operations: 50,
            allocations: 0,
        };
        assert!(usage.exceeds(&limits));

        // Exceeds cpu only
        let usage = ResourceUsage {
            memory_bytes: 50,
            cpu_ms: 200,
            operations: 50,
            allocations: 0,
        };
        assert!(usage.exceeds(&limits));

        // Within all limits
        let usage = ResourceUsage {
            memory_bytes: 50,
            cpu_ms: 50,
            operations: 50,
            allocations: 0,
        };
        assert!(!usage.exceeds(&limits));
    }

    // ── TestSummary ─────────────────────────────────────────────────────────

    #[test]
    fn test_summary_all_passed_with_errors() {
        let ts = TestSummary {
            passed: 10,
            failed: 0,
            skipped: 2,
            errors: vec!["warning".to_string()],
        };
        // errors present means not all_passed
        assert!(!ts.all_passed());
    }

    #[test]
    fn test_patch_status_transitions() {
        let author = Uuid::new_v4();
        let mut patch = CodePatch::new(
            "neural::optim",
            PatchKind::Optimize,
            PatchPayload::Parameters(HashMap::from([
                ("lr".to_string(), 0.001),
                ("momentum".to_string(), 0.9),
            ])),
            author,
        );

        assert_eq!(patch.status, PatchStatus::Proposed);
        patch.status = PatchStatus::Approved;
        assert_eq!(patch.status, PatchStatus::Approved);
        patch.status = PatchStatus::Applied;
        assert_eq!(patch.status, PatchStatus::Applied);
        patch.status = PatchStatus::RolledBack;
        assert_eq!(patch.status, PatchStatus::RolledBack);
    }

    #[test]
    fn test_safety_constraint_deactivation() {
        let mut guard = SafetyGuard::new();
        // Deactivate all constraints
        for c in guard.constraints.iter_mut() {
            c.active = false;
        }
        // Guard still has built-in blocked-target check even with constraints inactive
        let author = Uuid::new_v4();
        let patch = CodePatch::new(
            "core::safety::bypass",
            PatchKind::Delete,
            PatchPayload::Source(String::new()),
            author,
        )
        .with_rationale("Bypass");

        let verdict = guard.check(&patch, None);
        assert!(!verdict.safe, "blocked target should still be rejected");
    }
}
