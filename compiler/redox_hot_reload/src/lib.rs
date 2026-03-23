// redox_hot_reload: Function-level live patching with rollback support.
//
// Provides a hot-reload runtime that tracks function versions, applies
// patches atomically, and supports rollback to previous versions.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Patch status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatchStatus {
    Pending,
    Applied,
    RolledBack,
    Failed,
}

impl PatchStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::RolledBack => "rolled-back",
            Self::Failed => "failed",
        }
    }
}

// ---------------------------------------------------------------------------
// Function version
// ---------------------------------------------------------------------------

pub type VersionId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionVersion {
    pub version: VersionId,
    pub body_hash: u64,
    pub source_snippet: String,
}

// ---------------------------------------------------------------------------
// Patch
// ---------------------------------------------------------------------------

pub type PatchId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub id: PatchId,
    pub function_name: String,
    pub from_version: VersionId,
    pub to_version: VersionId,
    pub status: PatchStatus,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Rollback record
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RollbackRecord {
    pub patch_id: PatchId,
    pub function_name: String,
    pub restored_version: VersionId,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Hot-reload error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ReloadError {
    FunctionNotFound(String),
    VersionMismatch { expected: VersionId, actual: VersionId },
    PatchNotFound(PatchId),
    PatchAlreadyApplied(PatchId),
    NothingToRollback(String),
    ValidationFailed(String),
}

impl std::fmt::Display for ReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FunctionNotFound(n) => write!(f, "function not found: {n}"),
            Self::VersionMismatch { expected, actual } =>
                write!(f, "version mismatch: expected {expected}, got {actual}"),
            Self::PatchNotFound(id) => write!(f, "patch not found: {id}"),
            Self::PatchAlreadyApplied(id) => write!(f, "patch already applied: {id}"),
            Self::NothingToRollback(n) => write!(f, "nothing to rollback for: {n}"),
            Self::ValidationFailed(msg) => write!(f, "validation failed: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Validation hook
// ---------------------------------------------------------------------------

pub type ValidationFn = fn(&str, &FunctionVersion) -> bool;

// ---------------------------------------------------------------------------
// Hot-reload runtime
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct HotReloadRuntime {
    functions: HashMap<String, Vec<FunctionVersion>>,
    patches: Vec<Patch>,
    rollbacks: Vec<RollbackRecord>,
    next_patch_id: PatchId,
    validators: Vec<(&'static str, ValidationFn)>,
}

impl HotReloadRuntime {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            patches: Vec::new(),
            rollbacks: Vec::new(),
            next_patch_id: 1,
            validators: Vec::new(),
        }
    }

    /// Register a function with its initial version.
    pub fn register(&mut self, name: &str, body_hash: u64, source: &str) {
        let ver = FunctionVersion { version: 1, body_hash, source_snippet: source.to_string() };
        self.functions.insert(name.to_string(), vec![ver]);
    }

    /// Add a validation hook (applied before every patch).
    pub fn add_validator(&mut self, label: &'static str, f: ValidationFn) {
        self.validators.push((label, f));
    }

    /// Look up current version of a function.
    pub fn current_version(&self, name: &str) -> Option<&FunctionVersion> {
        self.functions.get(name).and_then(|vs| vs.last())
    }

    /// Apply a patch: bring function from current version to a new version.
    pub fn apply_patch(
        &mut self,
        function_name: &str,
        new_body_hash: u64,
        new_source: &str,
        description: &str,
    ) -> Result<PatchId, ReloadError> {
        let versions = self.functions.get_mut(function_name)
            .ok_or_else(|| ReloadError::FunctionNotFound(function_name.to_string()))?;

        let current = versions.last().unwrap();
        let from_version = current.version;
        let to_version = from_version + 1;

        let new_ver = FunctionVersion {
            version: to_version,
            body_hash: new_body_hash,
            source_snippet: new_source.to_string(),
        };

        // Run validators
        for (label, vfn) in &self.validators {
            if !vfn(function_name, &new_ver) {
                let pid = self.next_patch_id;
                self.next_patch_id += 1;
                self.patches.push(Patch {
                    id: pid,
                    function_name: function_name.to_string(),
                    from_version,
                    to_version,
                    status: PatchStatus::Failed,
                    description: description.to_string(),
                });
                return Err(ReloadError::ValidationFailed((*label).to_string()));
            }
        }

        let pid = self.next_patch_id;
        self.next_patch_id += 1;

        versions.push(new_ver);

        self.patches.push(Patch {
            id: pid,
            function_name: function_name.to_string(),
            from_version,
            to_version,
            status: PatchStatus::Applied,
            description: description.to_string(),
        });

        Ok(pid)
    }

    /// Roll back the most recent patch for a function.
    pub fn rollback(
        &mut self,
        function_name: &str,
        reason: &str,
    ) -> Result<RollbackRecord, ReloadError> {
        let versions = self.functions.get_mut(function_name)
            .ok_or_else(|| ReloadError::FunctionNotFound(function_name.to_string()))?;

        if versions.len() < 2 {
            return Err(ReloadError::NothingToRollback(function_name.to_string()));
        }

        versions.pop();
        let restored = versions.last().unwrap().version;

        // Mark the last applied patch for this function as rolled-back
        let patch_id = self.patches.iter_mut().rev()
            .find(|p| p.function_name == function_name && p.status == PatchStatus::Applied)
            .map(|p| { p.status = PatchStatus::RolledBack; p.id });

        let record = RollbackRecord {
            patch_id: patch_id.unwrap_or(0),
            function_name: function_name.to_string(),
            restored_version: restored,
            reason: reason.to_string(),
        };

        self.rollbacks.push(record.clone());
        Ok(record)
    }

    /// List all registered function names.
    pub fn function_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.functions.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// History of all versions for a function.
    pub fn version_history(&self, name: &str) -> Option<&[FunctionVersion]> {
        self.functions.get(name).map(|v| v.as_slice())
    }

    /// All patches.
    pub fn patches(&self) -> &[Patch] {
        &self.patches
    }

    /// All rollbacks.
    pub fn rollbacks(&self) -> &[RollbackRecord] {
        &self.rollbacks
    }

    /// Count of registered functions.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Count of applied patches (currently active).
    pub fn applied_patch_count(&self) -> usize {
        self.patches.iter().filter(|p| p.status == PatchStatus::Applied).count()
    }

    /// Summary report.
    pub fn summary(&self) -> RuntimeSummary {
        RuntimeSummary {
            functions: self.function_count(),
            total_patches: self.patches.len(),
            applied: self.applied_patch_count(),
            rolled_back: self.patches.iter().filter(|p| p.status == PatchStatus::RolledBack).count(),
            failed: self.patches.iter().filter(|p| p.status == PatchStatus::Failed).count(),
        }
    }
}

impl Default for HotReloadRuntime {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSummary {
    pub functions: usize,
    pub total_patches: usize,
    pub applied: usize,
    pub rolled_back: usize,
    pub failed: usize,
}

pub fn format_summary(s: &RuntimeSummary) -> String {
    format!(
        "functions={} patches={} applied={} rolled_back={} failed={}",
        s.functions, s.total_patches, s.applied, s.rolled_back, s.failed,
    )
}

// ---------------------------------------------------------------------------
// Pre-built example runtime
// ---------------------------------------------------------------------------

pub fn build_sample_runtime() -> HotReloadRuntime {
    let mut rt = HotReloadRuntime::new();
    rt.register("main", 0x1000, "fn main() {}");
    rt.register("process", 0x2000, "fn process(x: i32) -> i32 { x }");
    rt.register("validate", 0x3000, "fn validate(s: &str) -> bool { !s.is_empty() }");
    let _ = rt.apply_patch("process", 0x2001, "fn process(x: i32) -> i32 { x * 2 }", "double output");
    let _ = rt.apply_patch("process", 0x2002, "fn process(x: i32) -> i32 { x * 3 }", "triple output");
    rt
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_status_label() {
        assert_eq!(PatchStatus::Applied.label(), "applied");
        assert_eq!(PatchStatus::RolledBack.label(), "rolled-back");
    }

    #[test]
    fn test_register_function() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 42, "fn foo() {}");
        assert_eq!(rt.function_count(), 1);
    }

    #[test]
    fn test_current_version() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 42, "fn foo() {}");
        let v = rt.current_version("foo").unwrap();
        assert_eq!(v.version, 1);
        assert_eq!(v.body_hash, 42);
    }

    #[test]
    fn test_current_version_missing() {
        let rt = HotReloadRuntime::new();
        assert!(rt.current_version("nope").is_none());
    }

    #[test]
    fn test_apply_patch() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        let pid = rt.apply_patch("foo", 2, "v2", "update").unwrap();
        assert_eq!(pid, 1);
        assert_eq!(rt.current_version("foo").unwrap().version, 2);
    }

    #[test]
    fn test_apply_patch_unknown_fn() {
        let mut rt = HotReloadRuntime::new();
        let err = rt.apply_patch("nope", 1, "v1", "x").unwrap_err();
        assert_eq!(err, ReloadError::FunctionNotFound("nope".to_string()));
    }

    #[test]
    fn test_rollback() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "update").unwrap();
        let rec = rt.rollback("foo", "bug found").unwrap();
        assert_eq!(rec.restored_version, 1);
        assert_eq!(rt.current_version("foo").unwrap().version, 1);
    }

    #[test]
    fn test_rollback_nothing() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        let err = rt.rollback("foo", "impossible").unwrap_err();
        assert_eq!(err, ReloadError::NothingToRollback("foo".to_string()));
    }

    #[test]
    fn test_rollback_missing_fn() {
        let mut rt = HotReloadRuntime::new();
        let err = rt.rollback("no", "x").unwrap_err();
        assert_eq!(err, ReloadError::FunctionNotFound("no".to_string()));
    }

    #[test]
    fn test_version_history() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "update").unwrap();
        let hist = rt.version_history("foo").unwrap();
        assert_eq!(hist.len(), 2);
    }

    #[test]
    fn test_function_names() {
        let mut rt = HotReloadRuntime::new();
        rt.register("beta", 1, "");
        rt.register("alpha", 2, "");
        let names = rt.function_names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_patches_list() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "a").unwrap();
        assert_eq!(rt.patches().len(), 1);
        assert_eq!(rt.patches()[0].status, PatchStatus::Applied);
    }

    #[test]
    fn test_rollback_marks_patch() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "a").unwrap();
        rt.rollback("foo", "revert").unwrap();
        assert_eq!(rt.patches()[0].status, PatchStatus::RolledBack);
    }

    #[test]
    fn test_validator_blocks_patch() {
        fn deny_all(_: &str, _: &FunctionVersion) -> bool { false }
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.add_validator("deny", deny_all);
        let err = rt.apply_patch("foo", 2, "v2", "blocked").unwrap_err();
        assert!(matches!(err, ReloadError::ValidationFailed(_)));
    }

    #[test]
    fn test_validator_failed_patch_recorded() {
        fn deny_all(_: &str, _: &FunctionVersion) -> bool { false }
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.add_validator("deny", deny_all);
        let _ = rt.apply_patch("foo", 2, "v2", "blocked");
        assert_eq!(rt.patches()[0].status, PatchStatus::Failed);
    }

    #[test]
    fn test_summary() {
        let rt = build_sample_runtime();
        let s = rt.summary();
        assert_eq!(s.functions, 3);
        assert_eq!(s.applied, 2);
        assert_eq!(s.rolled_back, 0);
    }

    #[test]
    fn test_format_summary() {
        let s = RuntimeSummary { functions: 3, total_patches: 2, applied: 2, rolled_back: 0, failed: 0 };
        let text = format_summary(&s);
        assert!(text.contains("functions=3"));
    }

    #[test]
    fn test_sample_runtime() {
        let rt = build_sample_runtime();
        assert_eq!(rt.function_count(), 3);
        let v = rt.current_version("process").unwrap();
        assert_eq!(v.version, 3);
    }

    #[test]
    fn test_default_runtime() {
        let rt = HotReloadRuntime::default();
        assert_eq!(rt.function_count(), 0);
    }

    #[test]
    fn test_multiple_rollbacks() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "a").unwrap();
        rt.apply_patch("foo", 3, "v3", "b").unwrap();
        rt.rollback("foo", "revert b").unwrap();
        assert_eq!(rt.current_version("foo").unwrap().version, 2);
        rt.rollback("foo", "revert a").unwrap();
        assert_eq!(rt.current_version("foo").unwrap().version, 1);
    }

    #[test]
    fn test_error_display() {
        let e = ReloadError::FunctionNotFound("bar".to_string());
        assert_eq!(format!("{e}"), "function not found: bar");
    }

    #[test]
    fn test_rollbacks_list() {
        let mut rt = HotReloadRuntime::new();
        rt.register("foo", 1, "v1");
        rt.apply_patch("foo", 2, "v2", "x").unwrap();
        rt.rollback("foo", "oops").unwrap();
        assert_eq!(rt.rollbacks().len(), 1);
        assert_eq!(rt.rollbacks()[0].reason, "oops");
    }
}
