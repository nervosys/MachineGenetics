// MLIR→LLVM backend target validation for x86-64, AArch64, and WASM.
// (ROADMAP Step 45)
//
// Runs the Redox compiler lowering pipeline for each target architecture
// and validates the produced LLVM IR ops are correct and target-appropriate.

use redox_mlir::dialect::*;
use redox_mlir::lowering::{
    LlvmOp, OptLevel, PipelineConfig, PipelineResult,
    run_pipeline,
};


// ── Backend Target ─────────────────────────────────────────────────────────

/// Supported backend targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendTarget {
    X86_64,
    AArch64,
    Wasm32,
}

impl BackendTarget {
    /// LLVM triple string.
    pub fn triple(&self) -> &str {
        match self {
            BackendTarget::X86_64 => "x86_64-unknown-linux-gnu",
            BackendTarget::AArch64 => "aarch64-unknown-linux-gnu",
            BackendTarget::Wasm32 => "wasm32-unknown-unknown",
        }
    }

    /// Human-readable short name.
    pub fn name(&self) -> &str {
        match self {
            BackendTarget::X86_64 => "x86-64",
            BackendTarget::AArch64 => "aarch64",
            BackendTarget::Wasm32 => "wasm32",
        }
    }

    /// Return all supported targets.
    pub fn all() -> &'static [BackendTarget] {
        &[BackendTarget::X86_64, BackendTarget::AArch64, BackendTarget::Wasm32]
    }

    /// Data model pointer width in bytes.
    pub fn pointer_width(&self) -> u32 {
        match self {
            BackendTarget::X86_64 | BackendTarget::AArch64 => 8,
            BackendTarget::Wasm32 => 4,
        }
    }

    /// Whether this target supports SIMD vector extensions.
    pub fn supports_simd(&self) -> bool {
        match self {
            BackendTarget::X86_64 => true,   // SSE/AVX
            BackendTarget::AArch64 => true,  // NEON/SVE
            BackendTarget::Wasm32 => true,   // WASM SIMD
        }
    }

    /// Whether this target supports GPU dispatch.
    pub fn supports_gpu(&self) -> bool {
        match self {
            BackendTarget::X86_64 | BackendTarget::AArch64 => true,
            BackendTarget::Wasm32 => false,
        }
    }

    /// Maximum native vector width in bits.
    pub fn max_vector_width(&self) -> u32 {
        match self {
            BackendTarget::X86_64 => 512,    // AVX-512
            BackendTarget::AArch64 => 128,   // NEON (SVE is variable)
            BackendTarget::Wasm32 => 128,    // WASM SIMD v128
        }
    }
}

impl std::fmt::Display for BackendTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ── Target-Specific LLVM Constraints ───────────────────────────────────────

/// LLVM op constraint for a specific target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConstraint {
    /// The LLVM op name pattern this constraint applies to.
    pub op_pattern: String,
    /// Whether this op is allowed on the target.
    pub allowed: bool,
    /// Reason (for diagnostics).
    pub reason: String,
}

/// Constraints database for target validation.
pub struct TargetConstraints {
    target: BackendTarget,
    constraints: Vec<TargetConstraint>,
}

impl TargetConstraints {
    /// Build constraints for a target.
    pub fn for_target(target: BackendTarget) -> Self {
        let mut constraints = Vec::new();
        match target {
            BackendTarget::X86_64 => {
                // x86-64 supports everything
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.load".to_string(),
                    allowed: true,
                    reason: "x86-64 SSE/AVX support".to_string(),
                });
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.store".to_string(),
                    allowed: true,
                    reason: "x86-64 SSE/AVX support".to_string(),
                });
            }
            BackendTarget::AArch64 => {
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.load".to_string(),
                    allowed: true,
                    reason: "AArch64 NEON/SVE support".to_string(),
                });
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.store".to_string(),
                    allowed: true,
                    reason: "AArch64 NEON/SVE support".to_string(),
                });
            }
            BackendTarget::Wasm32 => {
                // WASM has limited intrinsics
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.load".to_string(),
                    allowed: true,
                    reason: "WASM SIMD v128".to_string(),
                });
                constraints.push(TargetConstraint {
                    op_pattern: "llvm.intr.masked.store".to_string(),
                    allowed: true,
                    reason: "WASM SIMD v128".to_string(),
                });
            }
        }
        TargetConstraints { target, constraints }
    }

    /// Check if an LLVM op is valid for this target.
    pub fn is_allowed(&self, op: &LlvmOp) -> bool {
        for c in &self.constraints {
            if op.name == c.op_pattern {
                return c.allowed;
            }
        }
        // Ops not in the constraint set are allowed by default
        true
    }

    pub fn target(&self) -> BackendTarget {
        self.target
    }
}

// ── Validation Result ──────────────────────────────────────────────────────

/// A validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationDiagnostic {
    pub target: BackendTarget,
    pub op_name: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// Result of validating a pipeline output against a backend target.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub target: BackendTarget,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub llvm_op_count: usize,
    pub standard_op_count: usize,
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count()
    }

    pub fn format_text(&self) -> String {
        let mut out = format!(
            "=== {} ({}) ===\n  standard ops: {}\n  llvm ops: {}\n",
            self.target.name(),
            self.target.triple(),
            self.standard_op_count,
            self.llvm_op_count,
        );
        if self.diagnostics.is_empty() {
            out.push_str("  result: OK\n");
        } else {
            for d in &self.diagnostics {
                out.push_str(&format!("  [{}] {}: {}\n", d.severity, d.op_name, d.message));
            }
        }
        out
    }
}

// ── Target Validator ───────────────────────────────────────────────────────

/// Validates pipeline output against a specific backend target.
pub struct TargetValidator {
    target: BackendTarget,
    constraints: TargetConstraints,
}

impl TargetValidator {
    pub fn new(target: BackendTarget) -> Self {
        TargetValidator {
            target,
            constraints: TargetConstraints::for_target(target),
        }
    }

    /// Validate a pipeline result against this target.
    pub fn validate(&self, result: &PipelineResult) -> ValidationResult {
        let mut diagnostics = Vec::new();

        // Check each LLVM op against target constraints
        for op in &result.llvm_ops {
            if !self.constraints.is_allowed(op) {
                diagnostics.push(ValidationDiagnostic {
                    target: self.target,
                    op_name: op.name.clone(),
                    severity: Severity::Error,
                    message: format!("op '{}' not supported on {}", op.name, self.target),
                });
            }
        }

        // Target-specific validations
        self.validate_gpu_ops(result, &mut diagnostics);
        self.validate_vector_ops(result, &mut diagnostics);
        self.validate_pointer_ops(result, &mut diagnostics);

        ValidationResult {
            target: self.target,
            diagnostics,
            llvm_op_count: result.llvm_ops.len(),
            standard_op_count: result.standard_ops.len(),
        }
    }

    fn validate_gpu_ops(&self, result: &PipelineResult, diags: &mut Vec<ValidationDiagnostic>) {
        if !self.target.supports_gpu() {
            for op in &result.standard_ops {
                if op.dialect == LoweringTarget::Gpu {
                    diags.push(ValidationDiagnostic {
                        target: self.target,
                        op_name: op.name.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "GPU ops not supported on {} — requires host-side dispatch",
                            self.target,
                        ),
                    });
                }
            }
        }
    }

    fn validate_vector_ops(&self, result: &PipelineResult, diags: &mut Vec<ValidationDiagnostic>) {
        for op in &result.standard_ops {
            if op.dialect == LoweringTarget::Vector {
                // Check if vectorization width exceeds target
                if let Some(width_str) = op.comment.split("width=").nth(1) {
                    if let Ok(w) = width_str.split_whitespace().next().unwrap_or("0").parse::<u32>() {
                        let max = self.target.max_vector_width();
                        if w * 32 > max {
                            diags.push(ValidationDiagnostic {
                                target: self.target,
                                op_name: op.name.clone(),
                                severity: Severity::Warning,
                                message: format!(
                                    "vector width {} exceeds max {} bits on {}",
                                    w * 32, max, self.target,
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    fn validate_pointer_ops(&self, result: &PipelineResult, diags: &mut Vec<ValidationDiagnostic>) {
        // On wasm32, warn about pointer-width-dependent operations
        if self.target == BackendTarget::Wasm32 {
            for op in &result.llvm_ops {
                if op.comment.contains("memref") {
                    diags.push(ValidationDiagnostic {
                        target: self.target,
                        op_name: op.name.clone(),
                        severity: Severity::Info,
                        message: "memory op on 32-bit target — pointers are 4 bytes".to_string(),
                    });
                }
            }
        }
    }
}

// ── Multi-Target Validation ────────────────────────────────────────────────

/// Validate a pipeline result against all supported targets.
pub fn validate_all_targets(result: &PipelineResult) -> Vec<ValidationResult> {
    BackendTarget::all()
        .iter()
        .map(|t| TargetValidator::new(*t).validate(result))
        .collect()
}

/// Run the full pipeline with a given config and validate for all targets.
pub fn compile_and_validate(
    ops: Vec<RedoxOp>,
    config: &PipelineConfig,
) -> (PipelineResult, Vec<ValidationResult>) {
    let result = run_pipeline(ops, config);
    let validations = validate_all_targets(&result);
    (result, validations)
}

/// Format a multi-target validation report.
pub fn format_report(validations: &[ValidationResult]) -> String {
    let mut out = String::from("Backend Target Validation Report\n");
    out.push_str(&"=".repeat(40));
    out.push('\n');
    for v in validations {
        out.push_str(&v.format_text());
    }
    let all_ok = validations.iter().all(|v| v.is_ok());
    out.push_str(&format!("\nOverall: {}\n", if all_ok { "PASS" } else { "FAIL" }));
    out
}

// ── Test Harness ───────────────────────────────────────────────────────────

/// A test case for backend validation.
#[derive(Debug, Clone)]
pub struct BackendTestCase {
    pub name: String,
    pub ops: Vec<RedoxOp>,
    pub config: PipelineConfig,
    /// Targets expected to pass validation.
    pub expected_pass: Vec<BackendTarget>,
}

/// Run a test case and return per-target results.
pub fn run_test_case(tc: &BackendTestCase) -> Vec<(BackendTarget, ValidationResult)> {
    let result = run_pipeline(tc.ops.clone(), &tc.config);
    tc.expected_pass.iter().map(|t| {
        let v = TargetValidator::new(*t).validate(&result);
        (*t, v)
    }).collect()
}

// ── Standard Test Suite ────────────────────────────────────────────────────

/// Build the standard test suite covering core Redox ops across all targets.
pub fn standard_test_suite() -> Vec<BackendTestCase> {
    let debug = PipelineConfig { opt_level: OptLevel::Debug };
    let release = PipelineConfig { opt_level: OptLevel::Release };
    let all = BackendTarget::all().to_vec();

    vec![
        BackendTestCase {
            name: "ownership_move".to_string(),
            ops: vec![RedoxOp::Move(MoveOp {
                source_type: owned_type("i32"),
                result_type: owned_type("i32"),
            })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "ownership_copy".to_string(),
            ops: vec![RedoxOp::Copy(CopyOp { source_type: owned_type("i64") })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "ownership_borrow_drop".to_string(),
            ops: vec![
                RedoxOp::Borrow(BorrowOp {
                    source_type: owned_type("String"),
                    mode: BorrowMode::Shared,
                    region: region_type("'a"),
                }),
                RedoxOp::Drop(DropOp { value_type: owned_type("String") }),
            ],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "contracts_debug".to_string(),
            ops: vec![
                RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }),
                RedoxOp::ContractEnsure(EnsureOp { message: "result > 0".into(), has_return_value: true }),
            ],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "contracts_release_elided".to_string(),
            ops: vec![
                RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }),
                RedoxOp::ContractInvariant(InvariantOp {
                    message: "len > 0".into(),
                    kind: InvariantKind::Loop,
                }),
            ],
            config: release.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "effects".to_string(),
            ops: vec![
                RedoxOp::EffectDecl(EffectDeclOp {
                    effects: vec!["IO".into()],
                    handlers: vec![],
                }),
                RedoxOp::EffectPerform(EffectPerformOp {
                    effect: effect_type("IO"),
                    arg_types: vec![],
                    result_type: None,
                }),
            ],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "vectorize_small".to_string(),
            ops: vec![RedoxOp::PerfVectorize(VectorizeOp { width: 4 })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "cpu_placement".to_string(),
            ops: vec![RedoxOp::PerfPlace(PlaceOp {
                target: PlaceTarget::Cpu,
                priority: None,
            })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "gpu_placement".to_string(),
            ops: vec![RedoxOp::PerfPlace(PlaceOp {
                target: PlaceTarget::Gpu,
                priority: None,
            })],
            config: debug.clone(),
            // GPU not supported on WASM
            expected_pass: vec![BackendTarget::X86_64, BackendTarget::AArch64],
        },
        BackendTestCase {
            name: "cost_query".to_string(),
            ops: vec![RedoxOp::PerfCostQuery(CostQueryOp {
                target_hw: "x86_64".into(),
                metric: CostMetric::LatencyNs,
            })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "capability_gate".to_string(),
            ops: vec![RedoxOp::CapabilityGate(CapabilityGateOp {
                token: CapabilityType { capabilities: vec!["net".into()] },
            })],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "full_program_debug".to_string(),
            ops: vec![
                RedoxOp::Copy(CopyOp { source_type: owned_type("i32") }),
                RedoxOp::ContractRequire(RequireOp { message: "y > 0".into() }),
                RedoxOp::Borrow(BorrowOp {
                    source_type: owned_type("String"),
                    mode: BorrowMode::Shared,
                    region: region_type("'a"),
                }),
                RedoxOp::Drop(DropOp { value_type: owned_type("String") }),
            ],
            config: debug.clone(),
            expected_pass: all.clone(),
        },
        BackendTestCase {
            name: "full_program_release".to_string(),
            ops: vec![
                RedoxOp::Move(MoveOp {
                    source_type: owned_type("Vec<u8>"),
                    result_type: owned_type("Vec<u8>"),
                }),
                RedoxOp::ContractRequire(RequireOp { message: "!empty".into() }),
                RedoxOp::PerfVectorize(VectorizeOp { width: 4 }),
                RedoxOp::CapabilityGate(CapabilityGateOp {
                    token: CapabilityType { capabilities: vec!["fs".into()] },
                }),
                RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") }),
            ],
            config: release.clone(),
            expected_pass: all.clone(),
        },
    ]
}

/// Run the complete standard test suite.
pub fn run_standard_suite() -> Vec<(String, Vec<(BackendTarget, ValidationResult)>)> {
    standard_test_suite().iter().map(|tc| {
        (tc.name.clone(), run_test_case(tc))
    }).collect()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn debug_cfg() -> PipelineConfig {
        PipelineConfig { opt_level: OptLevel::Debug }
    }

    fn release_cfg() -> PipelineConfig {
        PipelineConfig { opt_level: OptLevel::Release }
    }

    // ── BackendTarget tests ──

    #[test]
    fn target_triples() {
        assert!(BackendTarget::X86_64.triple().contains("x86_64"));
        assert!(BackendTarget::AArch64.triple().contains("aarch64"));
        assert!(BackendTarget::Wasm32.triple().contains("wasm32"));
    }

    #[test]
    fn target_names() {
        assert_eq!(BackendTarget::X86_64.name(), "x86-64");
        assert_eq!(BackendTarget::AArch64.name(), "aarch64");
        assert_eq!(BackendTarget::Wasm32.name(), "wasm32");
    }

    #[test]
    fn target_all_returns_three() {
        assert_eq!(BackendTarget::all().len(), 3);
    }

    #[test]
    fn target_pointer_widths() {
        assert_eq!(BackendTarget::X86_64.pointer_width(), 8);
        assert_eq!(BackendTarget::AArch64.pointer_width(), 8);
        assert_eq!(BackendTarget::Wasm32.pointer_width(), 4);
    }

    #[test]
    fn target_simd_support() {
        assert!(BackendTarget::X86_64.supports_simd());
        assert!(BackendTarget::AArch64.supports_simd());
        assert!(BackendTarget::Wasm32.supports_simd());
    }

    #[test]
    fn target_gpu_support() {
        assert!(BackendTarget::X86_64.supports_gpu());
        assert!(BackendTarget::AArch64.supports_gpu());
        assert!(!BackendTarget::Wasm32.supports_gpu());
    }

    #[test]
    fn target_vector_widths() {
        assert_eq!(BackendTarget::X86_64.max_vector_width(), 512);
        assert_eq!(BackendTarget::AArch64.max_vector_width(), 128);
        assert_eq!(BackendTarget::Wasm32.max_vector_width(), 128);
    }

    #[test]
    fn target_display() {
        assert_eq!(format!("{}", BackendTarget::X86_64), "x86-64");
    }

    // ── Constraint tests ──

    #[test]
    fn constraints_allow_standard_ops() {
        for target in BackendTarget::all() {
            let c = TargetConstraints::for_target(*target);
            let op = LlvmOp { name: "llvm.store".to_string(), comment: String::new() };
            assert!(c.is_allowed(&op));
        }
    }

    #[test]
    fn constraints_allow_vector_intrinsics() {
        for target in BackendTarget::all() {
            let c = TargetConstraints::for_target(*target);
            let op = LlvmOp { name: "llvm.intr.masked.load".to_string(), comment: String::new() };
            assert!(c.is_allowed(&op));
        }
    }

    // ── Validation tests ──

    #[test]
    fn validate_simple_move_all_targets() {
        let ops = vec![RedoxOp::Move(MoveOp {
            source_type: owned_type("i32"),
            result_type: owned_type("i32"),
        })];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_copy_all_targets() {
        let ops = vec![RedoxOp::Copy(CopyOp { source_type: owned_type("f64") })];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_borrow_drop_all_targets() {
        let ops = vec![
            RedoxOp::Borrow(BorrowOp {
                source_type: owned_type("Vec<u8>"),
                mode: BorrowMode::Exclusive,
                region: region_type("'b"),
            }),
            RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_contracts_debug_all_targets() {
        let ops = vec![
            RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }),
            RedoxOp::ContractEnsure(EnsureOp { message: "ret".into(), has_return_value: true }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_contracts_release_all_targets() {
        let ops = vec![
            RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }),
        ];
        let result = run_pipeline(ops, &release_cfg());
        // Release elides contracts, so 0 LLVM ops
        assert!(result.llvm_ops.is_empty());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok());
        }
    }

    #[test]
    fn validate_effects_all_targets() {
        let ops = vec![
            RedoxOp::EffectPerform(EffectPerformOp {
                effect: effect_type("IO"),
                arg_types: vec![],
                result_type: None,
            }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_vectorize_all_targets() {
        let ops = vec![RedoxOp::PerfVectorize(VectorizeOp { width: 4 })];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_gpu_x86_ok() {
        let ops = vec![RedoxOp::PerfPlace(PlaceOp {
            target: PlaceTarget::Gpu,
            priority: None,
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::X86_64).validate(&result);
        assert!(v.is_ok());
    }

    #[test]
    fn validate_gpu_aarch64_ok() {
        let ops = vec![RedoxOp::PerfPlace(PlaceOp {
            target: PlaceTarget::Gpu,
            priority: None,
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::AArch64).validate(&result);
        assert!(v.is_ok());
    }

    #[test]
    fn validate_gpu_wasm_rejected() {
        let ops = vec![RedoxOp::PerfPlace(PlaceOp {
            target: PlaceTarget::Gpu,
            priority: None,
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::Wasm32).validate(&result);
        assert!(!v.is_ok());
        assert!(v.error_count() > 0);
    }

    #[test]
    fn validate_cost_query_all_targets() {
        let ops = vec![RedoxOp::PerfCostQuery(CostQueryOp {
            target_hw: "aarch64".into(),
            metric: CostMetric::EnergyPj,
        })];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_capability_gate_all_targets() {
        let ops = vec![RedoxOp::CapabilityGate(CapabilityGateOp {
            token: CapabilityType { capabilities: vec!["fs".into()] },
        })];
        let result = run_pipeline(ops, &debug_cfg());
        for target in BackendTarget::all() {
            let v = TargetValidator::new(*target).validate(&result);
            assert!(v.is_ok(), "failed for {}", target);
        }
    }

    #[test]
    fn validate_wasm_memref_info_diagnostic() {
        let ops = vec![RedoxOp::Copy(CopyOp { source_type: owned_type("i32") })];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::Wasm32).validate(&result);
        assert!(v.is_ok()); // info-level is not an error
        let infos: Vec<_> = v.diagnostics.iter()
            .filter(|d| d.severity == Severity::Info)
            .collect();
        assert!(!infos.is_empty());
    }

    // ── Multi-target validation ──

    #[test]
    fn validate_all_targets_simple() {
        let ops = vec![RedoxOp::Move(MoveOp {
            source_type: owned_type("u64"),
            result_type: owned_type("u64"),
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let results = validate_all_targets(&result);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn compile_and_validate_api() {
        let ops = vec![
            RedoxOp::Copy(CopyOp { source_type: owned_type("i32") }),
            RedoxOp::Drop(DropOp { value_type: owned_type("i32") }),
        ];
        let (result, validations) = compile_and_validate(ops, &debug_cfg());
        assert!(!result.llvm_ops.is_empty());
        assert_eq!(validations.len(), 3);
    }

    #[test]
    fn format_report_produces_output() {
        let ops = vec![RedoxOp::Move(MoveOp {
            source_type: owned_type("i32"),
            result_type: owned_type("i32"),
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let validations = validate_all_targets(&result);
        let report = format_report(&validations);
        assert!(report.contains("x86-64"));
        assert!(report.contains("aarch64"));
        assert!(report.contains("wasm32"));
        assert!(report.contains("PASS"));
    }

    #[test]
    fn format_report_with_errors() {
        let ops = vec![RedoxOp::PerfPlace(PlaceOp {
            target: PlaceTarget::Gpu,
            priority: None,
        })];
        let result = run_pipeline(ops, &debug_cfg());
        let validations = validate_all_targets(&result);
        let report = format_report(&validations);
        assert!(report.contains("FAIL"));
    }

    // ── Test harness / suite tests ──

    #[test]
    fn standard_suite_has_cases() {
        let suite = standard_test_suite();
        assert!(suite.len() >= 10);
    }

    #[test]
    fn run_standard_suite_all_expected_pass() {
        let results = run_standard_suite();
        for (name, target_results) in &results {
            for (target, v) in target_results {
                assert!(
                    v.is_ok(),
                    "test '{}' failed on {}: {:?}",
                    name, target, v.diagnostics,
                );
            }
        }
    }

    #[test]
    fn full_pipeline_x86_debug() {
        let ops = vec![
            RedoxOp::Copy(CopyOp { source_type: owned_type("i32") }),
            RedoxOp::ContractRequire(RequireOp { message: "y > 0".into() }),
            RedoxOp::Borrow(BorrowOp {
                source_type: owned_type("String"),
                mode: BorrowMode::Shared,
                region: region_type("'a"),
            }),
            RedoxOp::Drop(DropOp { value_type: owned_type("String") }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::X86_64).validate(&result);
        assert!(v.is_ok());
        assert_eq!(result.standard_ops.len(), 4);
        assert_eq!(result.llvm_ops.len(), 6);
    }

    #[test]
    fn full_pipeline_aarch64_debug() {
        let ops = vec![
            RedoxOp::Move(MoveOp {
                source_type: owned_type("Vec<u8>"),
                result_type: owned_type("Vec<u8>"),
            }),
            RedoxOp::PerfVectorize(VectorizeOp { width: 4 }),
            RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::AArch64).validate(&result);
        assert!(v.is_ok());
    }

    #[test]
    fn full_pipeline_wasm32_debug() {
        let ops = vec![
            RedoxOp::Copy(CopyOp { source_type: owned_type("f64") }),
            RedoxOp::EffectPerform(EffectPerformOp {
                effect: effect_type("IO"),
                arg_types: vec![],
                result_type: None,
            }),
            RedoxOp::CapabilityGate(CapabilityGateOp {
                token: CapabilityType { capabilities: vec!["net".into()] },
            }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::Wasm32).validate(&result);
        assert!(v.is_ok());
    }

    #[test]
    fn full_pipeline_wasm32_no_gpu() {
        let ops = vec![
            RedoxOp::PerfPlace(PlaceOp { target: PlaceTarget::Gpu, priority: None }),
        ];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::Wasm32).validate(&result);
        assert!(!v.is_ok());
        assert!(v.diagnostics.iter().any(|d|
            d.severity == Severity::Error && d.message.contains("GPU")
        ));
    }

    #[test]
    fn validation_result_format() {
        let ops = vec![RedoxOp::Copy(CopyOp { source_type: owned_type("i32") })];
        let result = run_pipeline(ops, &debug_cfg());
        let v = TargetValidator::new(BackendTarget::X86_64).validate(&result);
        let text = v.format_text();
        assert!(text.contains("x86-64"));
        assert!(text.contains("OK"));
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(format!("{}", Severity::Info), "info");
    }
}
