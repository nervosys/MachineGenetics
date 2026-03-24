// Phase 1 integration tests: end-to-end test that a simulated agent can:
// 1. Query the SKB (Safety Knowledge Base)
// 2. Acquire a semantic lease
// 3. Compile code with safety elision
// 4. Receive structured diagnostics
// 5. Query the cost oracle
//
// (ROADMAP Step 46)

pub use redox_cost_oracle::{Cost, CostOracle, CostSubject, Target};
pub use redox_diagnostics::{
    Applicability, Diagnostic, DiagnosticGraph, DiagnosticNode, Edit, Fix, SafetyCategory,
    Severity as DiagSeverity, Span,
};
pub use redox_safety_cfg::{CheckLevel, Preset, SafetyConfig};
pub use redox_semantic_lease::{AgentId, LeaseKind, LeaseManager, SemanticRegion};
pub use redox_skb::{Database, SafetyKnowledgeBase, Severity as SkbSeverity, seed_corpus};

use std::time::Duration;

// ── Simulated Agent ────────────────────────────────────────────────────────

/// A simulated swarm agent that exercises the full Phase 1 pipeline.
pub struct SimulatedAgent {
    pub id: AgentId,
    pub skb: SafetyKnowledgeBase,
    pub lease_manager: LeaseManager,
    pub safety_config: SafetyConfig,
    pub cost_oracle: CostOracle,
    pub diagnostics: Vec<DiagnosticGraph>,
}

impl SimulatedAgent {
    /// Create a new simulated agent with seeded components.
    pub fn new(name: &str) -> Self {
        SimulatedAgent {
            id: AgentId::new(name),
            skb: seed_corpus(),
            lease_manager: LeaseManager::with_default_timeout(),
            safety_config: SafetyConfig::from_preset(Preset::AgentDev),
            cost_oracle: CostOracle::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Create with a specific safety preset.
    pub fn with_preset(name: &str, preset: Preset) -> Self {
        SimulatedAgent {
            id: AgentId::new(name),
            skb: seed_corpus(),
            lease_manager: LeaseManager::new(Duration::from_secs(60)),
            safety_config: SafetyConfig::from_preset(preset),
            cost_oracle: CostOracle::new(),
            diagnostics: Vec::new(),
        }
    }

    // ── Step 1: Query SKB ──

    /// Query the SKB for ownership rules.
    pub fn query_ownership_rules(&self) -> Vec<&redox_skb::Rule> {
        self.skb.query_active_in_database(Database::Ownership)
    }

    /// Query the SKB for borrow-checking rules.
    pub fn query_borrow_rules(&self) -> Vec<&redox_skb::Rule> {
        self.skb.query_active_in_database(Database::Borrow)
    }

    /// Get total rule count.
    pub fn total_rules(&self) -> usize {
        self.skb.total_rules()
    }

    // ── Step 2: Acquire Lease ──

    /// Acquire a shared read lease on a function.
    pub fn acquire_read_lease(&mut self, func_id: u64) -> Result<u64, String> {
        let region = SemanticRegion::Function(func_id);
        self.lease_manager.acquire_shared_read(&self.id, &region)
            .map_err(|e| format!("{:?}", e))
    }

    /// Acquire an exclusive write lease on a function.
    pub fn acquire_write_lease(&mut self, func_id: u64) -> Result<u64, String> {
        let region = SemanticRegion::Function(func_id);
        self.lease_manager.acquire_exclusive_write(&self.id, &region)
            .map_err(|e| format!("{:?}", e))
    }

    /// Release a lease on a function.
    pub fn release_lease(&mut self, func_id: u64) -> Result<(), String> {
        let region = SemanticRegion::Function(func_id);
        self.lease_manager.release(&self.id, &region)
            .map_err(|e| format!("{:?}", e))
    }

    /// Check if a region is writable.
    pub fn is_writable(&self, func_id: u64) -> bool {
        self.lease_manager.is_writable(&SemanticRegion::Function(func_id))
    }

    // ── Step 3: Safety Elision Check ──

    /// Check if borrow checking is active.
    pub fn borrow_check_active(&self) -> bool {
        self.safety_config.borrow_check() != CheckLevel::Skip
    }

    /// Check if bounds checking is active.
    pub fn bounds_check_active(&self) -> bool {
        self.safety_config.bounds_check() != CheckLevel::Skip
    }

    /// Check if all safety passes are enforced.
    pub fn fully_enforced(&self) -> bool {
        self.safety_config.fully_enforced()
    }

    // ── Step 4: Emit Diagnostics ──

    /// Emit a borrow-conflict diagnostic.
    pub fn emit_borrow_conflict(
        &mut self,
        file: &str,
        line: u32,
        message: &str,
    ) -> &DiagnosticGraph {
        let span = Span::new(file, line, 1, 40);
        let root = Diagnostic::error("E0502", message, span.clone())
            .with_category(SafetyCategory::BorrowConflict);
        let context = DiagnosticNode {
            kind: DiagSeverity::Note,
            message: "previous borrow occurs here".to_string(),
            span: Span::new(file, line.saturating_sub(1), 1, 40),
        };
        let fix = Fix::new(
            "clone the value instead of borrowing",
            Applicability::MaybeIncorrect,
            0.7,
        )
        .with_edit(Edit { span, replacement: ".clone()".to_string() });

        let graph = DiagnosticGraph {
            root,
            context: vec![context],
            fixes: vec![fix],
            related: vec![],
            documentation_url: Some("https://doc.rust-lang.org/error-index.html#E0502".to_string()),
        };
        self.diagnostics.push(graph);
        self.diagnostics.last().unwrap()
    }

    /// Emit a type-mismatch warning.
    pub fn emit_type_mismatch_warning(
        &mut self,
        file: &str,
        line: u32,
        message: &str,
    ) -> &DiagnosticGraph {
        let span = Span::new(file, line, 1, 20);
        let root = Diagnostic::warning("W0308", message, span.clone())
            .with_category(SafetyCategory::TypeMismatch);
        let graph = DiagnosticGraph {
            root,
            context: vec![],
            fixes: vec![],
            related: vec![],
            documentation_url: None,
        };
        self.diagnostics.push(graph);
        self.diagnostics.last().unwrap()
    }

    /// Get all recorded diagnostics.
    pub fn all_diagnostics(&self) -> &[DiagnosticGraph] {
        &self.diagnostics
    }

    /// Count error diagnostics.
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter()
            .filter(|d| d.root.severity == DiagSeverity::Error)
            .count()
    }

    // ── Step 5: Query Cost Oracle ──

    /// Query cost of a type on x86-64.
    pub fn query_type_cost(&self, type_name: &str) -> Option<Cost> {
        self.cost_oracle.query(&CostSubject::type_of(type_name), &Target::X86_64).cloned()
    }

    /// Query cost of an operation on a target.
    pub fn query_op_cost(&self, op: &str, target: &Target) -> Option<Cost> {
        self.cost_oracle.query(&CostSubject::operation(op), target).cloned()
    }

    /// Compare cost across targets.
    pub fn compare_costs(&self, subject: &CostSubject) -> redox_cost_oracle::CostComparison {
        self.cost_oracle.compare_all(subject)
    }
}

// ── Integration Scenario Runner ────────────────────────────────────────────

/// Result of running a full integration scenario.
#[derive(Debug)]
pub struct ScenarioResult {
    pub agent_name: String,
    pub skb_rules_queried: usize,
    pub leases_acquired: usize,
    pub leases_released: usize,
    pub safety_enforced: bool,
    pub diagnostics_emitted: usize,
    pub cost_queries: usize,
    pub passed: bool,
    pub notes: Vec<String>,
}

/// Run the standard Phase 1 integration scenario.
pub fn run_phase1_scenario() -> ScenarioResult {
    let mut agent = SimulatedAgent::new("integration-agent-01");
    let mut notes = Vec::new();
    let mut cost_queries = 0u32;

    // Step 1: Query SKB
    let own_count = agent.query_ownership_rules().len();
    let borrow_count = agent.query_borrow_rules().len();
    let total = agent.total_rules();
    notes.push(format!("SKB: {} ownership, {} borrow, {} total rules", own_count, borrow_count, total));

    // Step 2: Acquire leases
    let v1 = agent.acquire_read_lease(100).expect("read lease");
    notes.push(format!("Acquired read lease on func 100, version={v1}"));
    let v2 = agent.acquire_write_lease(200).expect("write lease");
    notes.push(format!("Acquired write lease on func 200, version={v2}"));

    // Step 3: Check safety config
    let enforced = agent.fully_enforced();
    notes.push(format!("Safety fully_enforced={enforced}, borrow_check={}", agent.borrow_check_active()));

    // Step 4: Emit diagnostics
    agent.emit_borrow_conflict("src/main.mg", 42, "cannot borrow `x` as mutable because it is also borrowed as immutable");
    agent.emit_type_mismatch_warning("src/lib.mg", 10, "expected `u32`, found `i32`");
    notes.push(format!("Emitted {} diagnostics ({} errors)", agent.all_diagnostics().len(), agent.error_count()));

    // Step 5: Query cost oracle
    if let Some(cost) = agent.query_type_cost("Vec<T>") {
        notes.push(format!("Vec cost on x86-64: latency={}cy, mem={}B", cost.latency_cycles, cost.memory_bytes));
        cost_queries += 1;
    }
    if let Some(cost) = agent.query_op_cost("add_i32", &Target::AArch64) {
        notes.push(format!("add_i32 on AArch64: latency={}cy", cost.latency_cycles));
        cost_queries += 1;
    }
    let cmp = agent.compare_costs(&CostSubject::operation("mul_i64"));
    notes.push(format!("mul_i64 comparison across targets: {}", cmp.format_text()));
    cost_queries += 1;

    // Release leases
    agent.release_lease(100).expect("release read");
    agent.release_lease(200).expect("release write");
    notes.push("Released all leases".to_string());

    ScenarioResult {
        agent_name: "integration-agent-01".to_string(),
        skb_rules_queried: own_count + borrow_count,
        leases_acquired: 2,
        leases_released: 2,
        safety_enforced: enforced,
        diagnostics_emitted: agent.all_diagnostics().len(),
        cost_queries: cost_queries as usize,
        passed: true,
        notes,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Agent construction ──

    #[test]
    fn create_agent() {
        let agent = SimulatedAgent::new("test-agent");
        assert_eq!(agent.id, AgentId::new("test-agent"));
        assert!(agent.total_rules() > 0);
    }

    #[test]
    fn create_agent_with_preset() {
        let agent = SimulatedAgent::with_preset("prod-agent", Preset::Production);
        assert!(agent.fully_enforced());
    }

    #[test]
    fn agent_preset_agent_dev() {
        let agent = SimulatedAgent::with_preset("dev", Preset::AgentDev);
        assert!(!agent.borrow_check_active()); // AgentDev skips all checks
    }

    // ── SKB queries ──

    #[test]
    fn query_skb_ownership_rules() {
        let agent = SimulatedAgent::new("skb-test");
        let rules = agent.query_ownership_rules();
        assert!(!rules.is_empty());
    }

    #[test]
    fn query_skb_borrow_rules() {
        let agent = SimulatedAgent::new("skb-test");
        let rules = agent.query_borrow_rules();
        assert!(!rules.is_empty());
    }

    #[test]
    fn skb_total_rules_positive() {
        let agent = SimulatedAgent::new("skb-test");
        assert!(agent.total_rules() > 100); // seeded corpus is ~9K
    }

    // ── Lease management ──

    #[test]
    fn acquire_and_release_read_lease() {
        let mut agent = SimulatedAgent::new("lease-test");
        let v = agent.acquire_read_lease(1).unwrap();
        assert_eq!(v, 0); // first read lease on unseen region starts at version 0
        agent.release_lease(1).unwrap();
    }

    #[test]
    fn acquire_and_release_write_lease() {
        let mut agent = SimulatedAgent::new("lease-test");
        let v = agent.acquire_write_lease(2).unwrap();
        assert_eq!(v, 1); // exclusive write bumps version to 1
        assert!(!agent.is_writable(2)); // region is exclusively held, not "writable" by others
        agent.release_lease(2).unwrap();
    }

    #[test]
    fn multiple_read_leases_same_region() {
        let mut agent = SimulatedAgent::new("agent-a");
        let agent_b = AgentId::new("agent-b");
        let region = SemanticRegion::Function(10);
        agent.acquire_read_lease(10).unwrap();
        // Another agent can also acquire shared read
        let v2 = agent.lease_manager.acquire_shared_read(&agent_b, &region);
        assert!(v2.is_ok());
    }

    #[test]
    fn write_lease_blocks_second_write() {
        let mut agent = SimulatedAgent::new("agent-x");
        let agent_y = AgentId::new("agent-y");
        let region = SemanticRegion::Function(20);
        agent.acquire_write_lease(20).unwrap();
        let result = agent.lease_manager.acquire_exclusive_write(&agent_y, &region);
        assert!(result.is_err());
    }

    // ── Safety config ──

    #[test]
    fn safety_agent_dev_preset() {
        let agent = SimulatedAgent::with_preset("dev", Preset::AgentDev);
        assert!(!agent.borrow_check_active()); // AgentDev uses Skip level
        assert!(!agent.bounds_check_active());
    }

    #[test]
    fn safety_production_fully_enforced() {
        let agent = SimulatedAgent::with_preset("prod", Preset::Production);
        assert!(agent.fully_enforced());
    }

    #[test]
    fn safety_ci_pipeline_preset() {
        let agent = SimulatedAgent::with_preset("ci", Preset::CiPipeline);
        assert!(agent.borrow_check_active());
    }

    // ── Diagnostics ──

    #[test]
    fn emit_borrow_conflict_diagnostic() {
        let mut agent = SimulatedAgent::new("diag-test");
        let graph = agent.emit_borrow_conflict("test.mg", 5, "cannot borrow");
        assert_eq!(graph.root.severity, DiagSeverity::Error);
        assert_eq!(graph.root.category, Some(SafetyCategory::BorrowConflict));
        assert!(!graph.fixes.is_empty());
    }

    #[test]
    fn emit_type_mismatch_warning() {
        let mut agent = SimulatedAgent::new("diag-test");
        let graph = agent.emit_type_mismatch_warning("test.mg", 10, "type mismatch");
        assert_eq!(graph.root.severity, DiagSeverity::Warning);
        assert_eq!(graph.root.category, Some(SafetyCategory::TypeMismatch));
    }

    #[test]
    fn diagnostic_error_count() {
        let mut agent = SimulatedAgent::new("diag-test");
        agent.emit_borrow_conflict("a.mg", 1, "err1");
        agent.emit_type_mismatch_warning("b.mg", 2, "warn1");
        agent.emit_borrow_conflict("c.mg", 3, "err2");
        assert_eq!(agent.error_count(), 2);
        assert_eq!(agent.all_diagnostics().len(), 3);
    }

    #[test]
    fn diagnostic_has_fix_with_edit() {
        let mut agent = SimulatedAgent::new("fix-test");
        agent.emit_borrow_conflict("x.mg", 1, "borrow conflict");
        let diag = &agent.all_diagnostics()[0];
        assert!(!diag.fixes.is_empty());
        assert!(!diag.fixes[0].edits.is_empty());
    }

    #[test]
    fn diagnostic_has_context_note() {
        let mut agent = SimulatedAgent::new("ctx-test");
        agent.emit_borrow_conflict("y.mg", 10, "conflict");
        let diag = &agent.all_diagnostics()[0];
        assert!(!diag.context.is_empty());
    }

    // ── Cost oracle ──

    #[test]
    fn query_vec_type_cost() {
        let agent = SimulatedAgent::new("cost-test");
        let cost = agent.query_type_cost("Vec<T>");
        assert!(cost.is_some());
        let c = cost.unwrap();
        assert!(c.latency_cycles > 0);
        assert!(c.memory_bytes > 0);
    }

    #[test]
    fn query_op_cost_add_i32() {
        let agent = SimulatedAgent::new("cost-test");
        let cost = agent.query_op_cost("add_i32", &Target::X86_64);
        assert!(cost.is_some());
    }

    #[test]
    fn query_op_cost_aarch64() {
        let agent = SimulatedAgent::new("cost-test");
        let cost = agent.query_op_cost("add_i32", &Target::AArch64);
        assert!(cost.is_some());
    }

    #[test]
    fn compare_costs_across_targets() {
        let agent = SimulatedAgent::new("cmp-test");
        let cmp = agent.compare_costs(&CostSubject::operation("mul_i64"));
        let text = cmp.format_text();
        assert!(!text.is_empty());
    }

    #[test]
    fn cost_oracle_has_wasm_entries() {
        let agent = SimulatedAgent::new("wasm-test");
        let cost = agent.query_op_cost("add_i32", &Target::Wasm32);
        assert!(cost.is_some());
    }

    // ── Full integration scenario ──

    #[test]
    fn phase1_scenario_passes() {
        let result = run_phase1_scenario();
        assert!(result.passed);
        assert!(result.skb_rules_queried > 0);
        assert_eq!(result.leases_acquired, 2);
        assert_eq!(result.leases_released, 2);
        assert_eq!(result.diagnostics_emitted, 2);
        assert!(result.cost_queries >= 2);
    }

    #[test]
    fn phase1_scenario_notes_not_empty() {
        let result = run_phase1_scenario();
        assert!(!result.notes.is_empty());
        assert!(result.notes.iter().any(|n| n.contains("SKB")));
        assert!(result.notes.iter().any(|n| n.contains("lease")));
    }

    // ── Cross-component interaction ──

    #[test]
    fn skb_query_then_lease_then_compile() {
        let mut agent = SimulatedAgent::new("combo-agent");

        // 1. SKB query
        let rule_count = agent.query_ownership_rules().len();
        assert!(rule_count > 0);

        // 2. Acquire lease
        let _v = agent.acquire_write_lease(42).unwrap();

        // 3. Check safety
        // AgentDev preset skips borrow check

        // 4. Emit diagnostic
        agent.emit_borrow_conflict("combo.mg", 1, "test conflict");
        assert_eq!(agent.error_count(), 1);

        // 5. Query cost
        let cost = agent.query_type_cost("HashMap<K, V>");
        assert!(cost.is_some());

        // 6. Release
        agent.release_lease(42).unwrap();
    }

    #[test]
    fn multiple_agents_concurrent_leases() {
        let mut a = SimulatedAgent::new("agent-alpha");
        let agent_beta = AgentId::new("agent-beta");

        // Alpha gets read on func 100
        a.acquire_read_lease(100).unwrap();

        // Beta also gets shared read on func 100
        let region = SemanticRegion::Function(100);
        a.lease_manager.acquire_shared_read(&agent_beta, &region).unwrap();

        // Both can read
        assert!(a.lease_manager.is_readable(&region));

        // But exclusive write is blocked
        let agent_gamma = AgentId::new("agent-gamma");
        let result = a.lease_manager.acquire_exclusive_write(&agent_gamma, &region);
        assert!(result.is_err());
    }
}
