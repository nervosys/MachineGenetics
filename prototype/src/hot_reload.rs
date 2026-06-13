// ── Hot-Reload Runtime ─────────────────────────────────────────────
//
// Function-level live patching for the MAGE compiler pipeline.
//
// Provides:
//   1. PatchUnit — a single function-level replacement
//   2. PatchRegistry — tracks active patches, versions, and rollback history
//   3. HotReloadEngine — validates, applies, and rolls back patches
//   4. MLIR single-function re-lowering stubs
//   5. Rollback management with full history

use std::collections::BTreeMap;

// ── Patch status ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchStatus {
    Pending,
    Applied,
    RolledBack,
    Failed,
}

// ── Patch unit ─────────────────────────────────────────────────────

/// A single function-level replacement unit.
#[derive(Debug, Clone)]
pub struct PatchUnit {
    pub id: u64,
    pub function_name: String,
    pub module_path: String,
    pub old_body: String,
    pub new_body: String,
    pub version: u64,
    pub status: PatchStatus,
    pub validation_errors: Vec<String>,
}

impl PatchUnit {
    pub fn new(
        id: u64,
        function_name: &str,
        module_path: &str,
        old_body: &str,
        new_body: &str,
    ) -> Self {
        Self {
            id,
            function_name: function_name.into(),
            module_path: module_path.into(),
            old_body: old_body.into(),
            new_body: new_body.into(),
            version: 1,
            status: PatchStatus::Pending,
            validation_errors: Vec::new(),
        }
    }

    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.module_path, self.function_name)
    }
}

// ── Validation result ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Ok,
    SignatureMismatch { expected: String, got: String },
    ContractViolation(String),
    TypeCheckFailure(String),
    EffectEscalation { old_effects: Vec<String>, new_effects: Vec<String> },
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, ValidationResult::Ok)
    }
}

// ── MLIR re-lowering stub ──────────────────────────────────────────

/// Generates an MLIR stub for a single function re-lowering.
pub fn mlir_relower_stub(function_name: &str, module_path: &str, body: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Re-lowered: {}::{}\n",
        module_path, function_name
    ));
    out.push_str(&format!(
        "func @{}_patched() {{\n",
        function_name
    ));
    // Emit body lines as comments (stub — real lowering would go through full MLIR pipeline)
    for line in body.lines() {
        out.push_str(&format!("  // {}\n", line));
    }
    out.push_str("  return\n");
    out.push_str("}\n");
    out
}

// ── Rollback entry ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RollbackEntry {
    pub patch_id: u64,
    pub function_name: String,
    pub module_path: String,
    pub restored_body: String,
    pub from_version: u64,
    pub to_version: u64,
}

// ── Patch registry ─────────────────────────────────────────────────

/// Tracks all patches, their versions, and rollback history.
pub struct PatchRegistry {
    patches: BTreeMap<u64, PatchUnit>,
    /// Track active body per qualified function name → body
    active_bodies: BTreeMap<String, String>,
    /// Version counter per function
    versions: BTreeMap<String, u64>,
    /// Full rollback history
    rollback_log: Vec<RollbackEntry>,
    next_id: u64,
}

impl PatchRegistry {
    pub fn new() -> Self {
        Self {
            patches: BTreeMap::new(),
            active_bodies: BTreeMap::new(),
            versions: BTreeMap::new(),
            rollback_log: Vec::new(),
            next_id: 1,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn register(&mut self, mut patch: PatchUnit) -> u64 {
        let id = patch.id;
        let qname = patch.qualified_name();
        let ver = self.versions.entry(qname).or_insert(0);
        *ver += 1;
        patch.version = *ver;
        self.patches.insert(id, patch);
        id
    }

    pub fn get(&self, id: u64) -> Option<&PatchUnit> {
        self.patches.get(&id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut PatchUnit> {
        self.patches.get_mut(&id)
    }

    pub fn active_body(&self, qualified_name: &str) -> Option<&str> {
        self.active_bodies.get(qualified_name).map(|s| s.as_str())
    }

    pub fn version(&self, qualified_name: &str) -> u64 {
        self.versions.get(qualified_name).copied().unwrap_or(0)
    }

    pub fn rollback_history(&self) -> &[RollbackEntry] {
        &self.rollback_log
    }

    pub fn patches_for_module(&self, module_path: &str) -> Vec<&PatchUnit> {
        self.patches
            .values()
            .filter(|p| p.module_path == module_path)
            .collect()
    }

    pub fn all_applied(&self) -> Vec<&PatchUnit> {
        self.patches
            .values()
            .filter(|p| p.status == PatchStatus::Applied)
            .collect()
    }
}

// ── Hot-Reload Engine ──────────────────────────────────────────────

/// Validates, applies, and rolls back function-level patches.
pub struct HotReloadEngine {
    pub registry: PatchRegistry,
    /// Known function signatures for validation: qualified_name → signature string
    signatures: BTreeMap<String, String>,
    /// Known effect sets per function
    effects: BTreeMap<String, Vec<String>>,
}

impl HotReloadEngine {
    pub fn new() -> Self {
        Self {
            registry: PatchRegistry::new(),
            signatures: BTreeMap::new(),
            effects: BTreeMap::new(),
        }
    }

    /// Register a known function signature (for validation).
    pub fn register_signature(&mut self, qualified_name: &str, signature: &str) {
        self.signatures.insert(qualified_name.into(), signature.into());
    }

    /// Register known effects for a function.
    pub fn register_effects(&mut self, qualified_name: &str, fx: Vec<String>) {
        self.effects.insert(qualified_name.into(), fx);
    }

    /// Validate a patch before applying.
    pub fn validate(&self, patch: &PatchUnit) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        let qname = patch.qualified_name();

        // Check signature hasn't changed (stub: compare old_body first line as "signature")
        if let Some(known_sig) = self.signatures.get(&qname) {
            let new_first = patch.new_body.lines().next().unwrap_or("");
            if !new_first.is_empty() && new_first != known_sig.as_str() {
                results.push(ValidationResult::SignatureMismatch {
                    expected: known_sig.clone(),
                    got: new_first.into(),
                });
            }
        }

        // Check for effect escalation: new body must not introduce effects beyond old set
        if let Some(old_fx) = self.effects.get(&qname) {
            // Stub: check for "effect:" markers in new body
            let new_effects: Vec<String> = patch
                .new_body
                .lines()
                .filter_map(|l| l.strip_prefix("effect:"))
                .map(|s| s.trim().to_string())
                .collect();
            let has_new = new_effects.iter().any(|e| !old_fx.contains(e));
            if has_new {
                results.push(ValidationResult::EffectEscalation {
                    old_effects: old_fx.clone(),
                    new_effects,
                });
            }
        }

        if results.is_empty() {
            results.push(ValidationResult::Ok);
        }
        results
    }

    /// Create and register a patch, returning its ID.
    pub fn create_patch(
        &mut self,
        function_name: &str,
        module_path: &str,
        old_body: &str,
        new_body: &str,
    ) -> u64 {
        let id = self.registry.next_id();
        let patch = PatchUnit::new(id, function_name, module_path, old_body, new_body);
        self.registry.register(patch);
        id
    }

    /// Apply a patch by ID. Returns validation results; only applies if all OK.
    pub fn apply(&mut self, patch_id: u64) -> Vec<ValidationResult> {
        let patch = match self.registry.get(patch_id) {
            Some(p) => p.clone(),
            None => return vec![ValidationResult::TypeCheckFailure("Patch not found".into())],
        };

        let results = self.validate(&patch);
        if results.iter().all(|r| r.is_ok()) {
            let qname = patch.qualified_name();

            // Save old body for rollback
            let old = self
                .registry
                .active_body(&qname)
                .unwrap_or(&patch.old_body)
                .to_string();

            let from_ver = self.registry.version(&qname).saturating_sub(1);
            let to_ver = self.registry.version(&qname);

            self.registry
                .active_bodies
                .insert(qname.clone(), patch.new_body.clone());

            if let Some(p) = self.registry.get_mut(patch_id) {
                p.status = PatchStatus::Applied;
            }

            self.registry.rollback_log.push(RollbackEntry {
                patch_id,
                function_name: patch.function_name.clone(),
                module_path: patch.module_path.clone(),
                restored_body: old,
                from_version: from_ver,
                to_version: to_ver,
            });
        } else {
            if let Some(p) = self.registry.get_mut(patch_id) {
                p.status = PatchStatus::Failed;
                p.validation_errors = results
                    .iter()
                    .filter(|r| !r.is_ok())
                    .map(|r| format!("{:?}", r))
                    .collect();
            }
        }
        results
    }

    /// Rollback the most recent patch for a given function.
    pub fn rollback(&mut self, qualified_name: &str) -> bool {
        // Find the last rollback entry for this function
        let entry = self
            .registry
            .rollback_log
            .iter()
            .rev()
            .find(|e| format!("{}::{}", e.module_path, e.function_name) == qualified_name)
            .cloned();

        if let Some(entry) = entry {
            self.registry
                .active_bodies
                .insert(qualified_name.into(), entry.restored_body);

            if let Some(p) = self.registry.get_mut(entry.patch_id) {
                p.status = PatchStatus::RolledBack;
            }
            true
        } else {
            false
        }
    }

    /// Generate MLIR re-lowering stub for an applied patch.
    pub fn mlir_stub(&self, patch_id: u64) -> Option<String> {
        let patch = self.registry.get(patch_id)?;
        if patch.status != PatchStatus::Applied {
            return None;
        }
        Some(mlir_relower_stub(
            &patch.function_name,
            &patch.module_path,
            &patch.new_body,
        ))
    }

    /// Stats as JSON-like string.
    pub fn stats(&self) -> String {
        let total = self.registry.patches.len();
        let applied = self
            .registry
            .patches
            .values()
            .filter(|p| p.status == PatchStatus::Applied)
            .count();
        let rolled_back = self
            .registry
            .patches
            .values()
            .filter(|p| p.status == PatchStatus::RolledBack)
            .count();
        let failed = self
            .registry
            .patches
            .values()
            .filter(|p| p.status == PatchStatus::Failed)
            .count();
        format!(
            "{{\"total\":{},\"applied\":{},\"rolled_back\":{},\"failed\":{}}}",
            total, applied, rolled_back, failed
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> HotReloadEngine {
        let mut eng = HotReloadEngine::new();
        eng.register_signature("math::add", "fn add(a: i32, b: i32) -> i32");
        eng.register_effects("math::add", vec!["pure".into()]);
        eng
    }

    // ── PatchUnit ─────────────────────────────────────────────────

    #[test]
    fn patch_unit_qualified_name() {
        let p = PatchUnit::new(1, "add", "math", "old", "new");
        assert_eq!(p.qualified_name(), "math::add");
        assert_eq!(p.status, PatchStatus::Pending);
    }

    // ── Validation ────────────────────────────────────────────────

    #[test]
    fn validation_ok() {
        let eng = make_engine();
        let p = PatchUnit::new(1, "add", "math", "old body", "fn add(a: i32, b: i32) -> i32\n  a + b");
        let results = eng.validate(&p);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn validation_signature_mismatch() {
        let eng = make_engine();
        let p = PatchUnit::new(1, "add", "math", "old", "fn add(a: f64) -> f64\n  a");
        let results = eng.validate(&p);
        assert!(results.iter().any(|r| matches!(r, ValidationResult::SignatureMismatch { .. })));
    }

    #[test]
    fn validation_effect_escalation() {
        let eng = make_engine();
        let p = PatchUnit::new(1, "add", "math", "old", "fn add(a: i32, b: i32) -> i32\neffect:io\n  a + b");
        let results = eng.validate(&p);
        assert!(results.iter().any(|r| matches!(r, ValidationResult::EffectEscalation { .. })));
    }

    #[test]
    fn validation_no_effect_escalation_when_known() {
        let eng = make_engine();
        let p = PatchUnit::new(1, "add", "math", "old", "fn add(a: i32, b: i32) -> i32\neffect:pure\n  a + b");
        let results = eng.validate(&p);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    // ── Apply and rollback ────────────────────────────────────────

    #[test]
    fn apply_patch() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "a + b", "fn add(a: i32, b: i32) -> i32\n  a + b + 1");
        let results = eng.apply(id);
        assert!(results.iter().all(|r| r.is_ok()));
        assert_eq!(eng.registry.get(id).unwrap().status, PatchStatus::Applied);
        assert_eq!(
            eng.registry.active_body("math::add").unwrap(),
            "fn add(a: i32, b: i32) -> i32\n  a + b + 1"
        );
    }

    #[test]
    fn apply_failing_patch() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "old", "fn add(a: f64) -> f64\n  a");
        let results = eng.apply(id);
        assert!(!results.iter().all(|r| r.is_ok()));
        assert_eq!(eng.registry.get(id).unwrap().status, PatchStatus::Failed);
    }

    #[test]
    fn rollback_patch() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "original body", "fn add(a: i32, b: i32) -> i32\n  a - b");
        eng.apply(id);
        assert!(eng.rollback("math::add"));
        assert_eq!(eng.registry.get(id).unwrap().status, PatchStatus::RolledBack);
        assert_eq!(eng.registry.active_body("math::add").unwrap(), "original body");
    }

    #[test]
    fn rollback_nonexistent() {
        let mut eng = make_engine();
        assert!(!eng.rollback("nonexistent::fn"));
    }

    // ── Version tracking ──────────────────────────────────────────

    #[test]
    fn version_increments() {
        let mut eng = make_engine();
        let id1 = eng.create_patch("add", "math", "v1", "fn add(a: i32, b: i32) -> i32\n  v2");
        let id2 = eng.create_patch("add", "math", "v2", "fn add(a: i32, b: i32) -> i32\n  v3");
        assert_eq!(eng.registry.get(id1).unwrap().version, 1);
        assert_eq!(eng.registry.get(id2).unwrap().version, 2);
    }

    // ── MLIR stub ─────────────────────────────────────────────────

    #[test]
    fn mlir_relower_stub_output() {
        let stub = mlir_relower_stub("add", "math", "a + b\nreturn result");
        assert!(stub.contains("Re-lowered: math::add"));
        assert!(stub.contains("func @add_patched()"));
        assert!(stub.contains("// a + b"));
        assert!(stub.contains("return"));
    }

    #[test]
    fn engine_mlir_stub() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "old", "fn add(a: i32, b: i32) -> i32\n  a + b");
        eng.apply(id);
        let stub = eng.mlir_stub(id).unwrap();
        assert!(stub.contains("func @add_patched()"));
    }

    #[test]
    fn mlir_stub_none_for_pending() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "old", "new");
        assert!(eng.mlir_stub(id).is_none());
    }

    // ── Module filtering ──────────────────────────────────────────

    #[test]
    fn patches_for_module() {
        let mut eng = make_engine();
        eng.create_patch("add", "math", "a", "fn add(a: i32, b: i32) -> i32\n  b");
        eng.create_patch("init", "core", "x", "y");
        let math_patches = eng.registry.patches_for_module("math");
        assert_eq!(math_patches.len(), 1);
        assert_eq!(math_patches[0].function_name, "add");
    }

    // ── Stats ─────────────────────────────────────────────────────

    #[test]
    fn stats_json() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "old", "fn add(a: i32, b: i32) -> i32\n  new");
        eng.apply(id);
        let s = eng.stats();
        assert!(s.contains("\"total\":1"));
        assert!(s.contains("\"applied\":1"));
    }

    // ── Rollback history ──────────────────────────────────────────

    #[test]
    fn rollback_history_recorded() {
        let mut eng = make_engine();
        let id = eng.create_patch("add", "math", "body_v1", "fn add(a: i32, b: i32) -> i32\n  body_v2");
        eng.apply(id);
        let history = eng.registry.rollback_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].patch_id, id);
        assert_eq!(history[0].restored_body, "body_v1");
    }

    // ── All-applied listing ───────────────────────────────────────

    #[test]
    fn all_applied() {
        let mut eng = make_engine();
        let id1 = eng.create_patch("add", "math", "a", "fn add(a: i32, b: i32) -> i32\n  b");
        eng.create_patch("sub", "math", "c", "d");
        eng.apply(id1);
        let applied = eng.registry.all_applied();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].function_name, "add");
    }
}
