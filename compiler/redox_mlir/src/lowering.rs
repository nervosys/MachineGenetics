//! # MLIR Progressive Lowering Pipeline
//!
//! Implements the progressive lowering pipeline that converts Redox MLIR dialect
//! operations into standard MLIR dialects and ultimately into the LLVM dialect:
//!
//! ```text
//! Redox Dialect → Standard MLIR (Func / MemRef / CF / SCF / Vector / GPU) → LLVM Dialect
//! ```
//!
//! ## Pipeline Stages
//!
//! 1. **Ownership lowering** — `redox.move`, `redox.copy`, `redox.borrow`, `redox.drop`
//!    → SSA copy, `memref.copy`, `memref.view`, destructor call sequences
//! 2. **Contract lowering** — `redox.contract.require/ensure/invariant`
//!    → `cf.assert` (debug) or elided (release)
//! 3. **Effect lowering** — `redox.effect.perform/handle`
//!    → `func.call` to handler implementations
//! 4. **Performance lowering** — `redox.perf.place`, `redox.perf.vectorize`
//!    → `gpu.launch_func`, `vector.*` ops
//! 5. **Capability lowering** — `redox.capability.gate`
//!    → `scf.if` on runtime capability check
//! 6. **LLVM lowering** — all standard dialects → `llvm.*` ops
//!
//! Reference: REDOX_PROPOSAL.md §14.9

use crate::dialect::*;

// ===========================================================================
// Lowered IR representation
// ===========================================================================

/// A lowered operation in a standard MLIR dialect (post Redox dialect lowering).
#[derive(Debug, Clone, PartialEq)]
pub struct LoweredOp {
    /// The target dialect this op belongs to.
    pub dialect: LoweringTarget,
    /// The fully qualified operation name (e.g. `"llvm.store"`, `"cf.assert"`).
    pub name: String,
    /// Human-readable description for debugging.
    pub comment: String,
}

impl LoweredOp {
    fn new(dialect: LoweringTarget, name: impl Into<String>, comment: impl Into<String>) -> Self {
        Self { dialect, name: name.into(), comment: comment.into() }
    }
}

// ===========================================================================
// Pipeline configuration
// ===========================================================================

/// Optimization level controlling how contracts and checks are lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// Debug: contracts become `cf.assert`, bounds checks preserved.
    Debug,
    /// Release: contracts elided, bounds checks in `no_bounds_check` regions removed.
    Release,
}

/// Configuration for the lowering pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub opt_level: OptLevel,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self { opt_level: OptLevel::Debug }
    }
}

// ===========================================================================
// Stage 1: Ownership lowering
// ===========================================================================

fn lower_move(op: &MoveOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Std,
        "std.copy_ssa",
        format!("move {} (SSA value copy + source invalidation)", op.source_type),
    )]
}

fn lower_copy(op: &CopyOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::MemRef,
        "memref.copy",
        format!("copy {} (memref duplication)", op.source_type),
    )]
}

fn lower_borrow(op: &BorrowOp) -> Vec<LoweredOp> {
    let op_name = match op.mode {
        BorrowMode::Shared => "memref.view",
        BorrowMode::Exclusive => "memref.view",
        BorrowMode::Inferred => "memref.view",
    };
    vec![LoweredOp::new(
        LoweringTarget::MemRef,
        op_name,
        format!("borrow {} {} in region '{}'", op.mode, op.source_type, op.region.name),
    )]
}

fn lower_drop(op: &DropOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Func,
        "func.call",
        format!("drop {} (destructor call sequence)", op.value_type),
    )]
}

// ===========================================================================
// Stage 2: Contract lowering
// ===========================================================================

fn lower_require(op: &RequireOp, config: &PipelineConfig) -> Vec<LoweredOp> {
    match config.opt_level {
        OptLevel::Debug => vec![LoweredOp::new(
            LoweringTarget::Cf,
            "cf.assert",
            format!("precondition: {}", op.message),
        )],
        OptLevel::Release => vec![], // Elided in release
    }
}

fn lower_ensure(op: &EnsureOp, config: &PipelineConfig) -> Vec<LoweredOp> {
    match config.opt_level {
        OptLevel::Debug => vec![LoweredOp::new(
            LoweringTarget::Cf,
            "cf.assert",
            format!("postcondition: {}", op.message),
        )],
        OptLevel::Release => vec![],
    }
}

fn lower_invariant(op: &InvariantOp, config: &PipelineConfig) -> Vec<LoweredOp> {
    match config.opt_level {
        OptLevel::Debug => vec![LoweredOp::new(
            LoweringTarget::Cf,
            "cf.assert",
            format!("{} invariant: {}", op.kind, op.message),
        )],
        OptLevel::Release => vec![],
    }
}

// ===========================================================================
// Stage 3: Effect lowering
// ===========================================================================

fn lower_effect_decl(_op: &EffectDeclOp) -> Vec<LoweredOp> {
    // Effect declarations are metadata-only; no-op in lowered IR
    vec![LoweredOp::new(
        LoweringTarget::Redox,
        "redox.effect.decl",
        "effect declaration (metadata preserved)",
    )]
}

fn lower_effect_perform(op: &EffectPerformOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Func,
        "func.call",
        format!("perform effect {} → handler call", op.effect.effect_name),
    )]
}

fn lower_effect_handle(op: &EffectHandleOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Func,
        "func.call",
        format!("handle effect {} → handler dispatch", op.effect.effect_name),
    )]
}

// ===========================================================================
// Stage 4: Performance lowering
// ===========================================================================

fn lower_place(op: &PlaceOp) -> Vec<LoweredOp> {
    match op.target {
        PlaceTarget::Gpu => vec![LoweredOp::new(
            LoweringTarget::Gpu,
            "gpu.launch_func",
            format!("place on GPU (priority: {:?})", op.priority),
        )],
        PlaceTarget::Npu => vec![LoweredOp::new(
            LoweringTarget::Func,
            "func.call",
            "place on NPU (vendor runtime dispatch)",
        )],
        PlaceTarget::Cpu | PlaceTarget::Auto => vec![LoweredOp::new(
            LoweringTarget::Func,
            "func.call",
            format!("place on {} (default path)", op.target),
        )],
    }
}

fn lower_vectorize(op: &VectorizeOp) -> Vec<LoweredOp> {
    vec![
        LoweredOp::new(
            LoweringTarget::Vector,
            "vector.transfer_read",
            format!("vectorize width={} (read)", op.width),
        ),
        LoweredOp::new(
            LoweringTarget::Vector,
            "vector.transfer_write",
            format!("vectorize width={} (write)", op.width),
        ),
    ]
}

fn lower_no_bounds_check(config: &PipelineConfig) -> Vec<LoweredOp> {
    match config.opt_level {
        OptLevel::Release => vec![LoweredOp::new(
            LoweringTarget::Redox,
            "redox.perf.no_bounds_check",
            "bounds check elided (release)",
        )],
        OptLevel::Debug => vec![LoweredOp::new(
            LoweringTarget::Redox,
            "redox.perf.no_bounds_check",
            "bounds check preserved (debug)",
        )],
    }
}

fn lower_autotune(op: &AutotuneOp) -> Vec<LoweredOp> {
    // Autotuning generates N cloned variants — stays in Redox dialect
    vec![LoweredOp::new(
        LoweringTarget::Redox,
        "redox.perf.autotune",
        format!("autotune: {} variants, metric={:?}", op.variants, op.metric),
    )]
}

fn lower_cost_query(op: &CostQueryOp) -> Vec<LoweredOp> {
    // Cost query is evaluated at compile time → constant
    vec![LoweredOp::new(
        LoweringTarget::Arith,
        "arith.constant",
        format!("cost_query({}, {}) → compile-time constant", op.target_hw, op.metric),
    )]
}

// ===========================================================================
// Stage 5: Capability lowering
// ===========================================================================

fn lower_capability_decl(_op: &CapabilityDeclOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Redox,
        "redox.capability.decl",
        "capability declaration (metadata)",
    )]
}

fn lower_capability_check(_op: &CapabilityCheckOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Redox,
        "redox.capability.check",
        "compile-time capability check",
    )]
}

fn lower_capability_gate(_op: &CapabilityGateOp) -> Vec<LoweredOp> {
    vec![LoweredOp::new(
        LoweringTarget::Scf,
        "scf.if",
        "capability-gated region → runtime conditional",
    )]
}

// ===========================================================================
// Main lowering entry point
// ===========================================================================

/// Lower a single Redox dialect operation to standard MLIR dialect ops.
pub fn lower_op(op: &RedoxOp, config: &PipelineConfig) -> Vec<LoweredOp> {
    match op {
        // Ownership
        RedoxOp::Move(m) => lower_move(m),
        RedoxOp::Copy(c) => lower_copy(c),
        RedoxOp::Borrow(b) => lower_borrow(b),
        RedoxOp::Drop(d) => lower_drop(d),
        // Effects
        RedoxOp::EffectDecl(e) => lower_effect_decl(e),
        RedoxOp::EffectPerform(e) => lower_effect_perform(e),
        RedoxOp::EffectHandle(e) => lower_effect_handle(e),
        // Contracts
        RedoxOp::ContractRequire(r) => lower_require(r, config),
        RedoxOp::ContractEnsure(e) => lower_ensure(e, config),
        RedoxOp::ContractInvariant(i) => lower_invariant(i, config),
        // Performance
        RedoxOp::PerfPlace(p) => lower_place(p),
        RedoxOp::PerfVectorize(v) => lower_vectorize(v),
        RedoxOp::PerfNoBoundsCheck(_) => lower_no_bounds_check(config),
        RedoxOp::PerfAutotune(a) => lower_autotune(a),
        RedoxOp::PerfCostQuery(c) => lower_cost_query(c),
        // Capabilities
        RedoxOp::CapabilityDecl(c) => lower_capability_decl(c),
        RedoxOp::CapabilityCheck(c) => lower_capability_check(c),
        RedoxOp::CapabilityGate(c) => lower_capability_gate(c),
    }
}

/// Lower a sequence of Redox dialect ops into standard MLIR ops.
pub fn lower_ops(ops: &[RedoxOp], config: &PipelineConfig) -> Vec<LoweredOp> {
    ops.iter().flat_map(|op| lower_op(op, config)).collect()
}

// ===========================================================================
// Stage 6: LLVM Dialect lowering
// ===========================================================================

/// An LLVM dialect operation (final lowering target).
#[derive(Debug, Clone, PartialEq)]
pub struct LlvmOp {
    pub name: String,
    pub comment: String,
}

impl LlvmOp {
    fn new(name: impl Into<String>, comment: impl Into<String>) -> Self {
        Self { name: name.into(), comment: comment.into() }
    }
}

/// Lower a standard MLIR op to LLVM dialect.
pub fn lower_to_llvm(op: &LoweredOp) -> Vec<LlvmOp> {
    match op.dialect {
        LoweringTarget::Std => {
            vec![LlvmOp::new("llvm.store", format!("std → llvm: {}", op.comment))]
        }
        LoweringTarget::MemRef => vec![
            LlvmOp::new("llvm.load", format!("memref → llvm: {}", op.comment)),
            LlvmOp::new("llvm.store", format!("memref → llvm: {}", op.comment)),
        ],
        LoweringTarget::Func => {
            vec![LlvmOp::new("llvm.call", format!("func → llvm: {}", op.comment))]
        }
        LoweringTarget::Cf => {
            vec![LlvmOp::new("llvm.cond_br", format!("cf → llvm: {}", op.comment))]
        }
        LoweringTarget::Gpu => {
            vec![LlvmOp::new("llvm.call", format!("gpu → llvm: {}", op.comment))]
        }
        LoweringTarget::Vector => {
            if op.name.contains("read") {
                vec![LlvmOp::new("llvm.intr.masked.load", format!("vector → llvm: {}", op.comment))]
            } else {
                vec![LlvmOp::new(
                    "llvm.intr.masked.store",
                    format!("vector → llvm: {}", op.comment),
                )]
            }
        }
        LoweringTarget::Arith => {
            vec![LlvmOp::new("llvm.mlir.constant", format!("arith → llvm: {}", op.comment))]
        }
        LoweringTarget::Scf => {
            vec![LlvmOp::new("llvm.cond_br", format!("scf → llvm: {}", op.comment))]
        }
        LoweringTarget::Redox => {
            // Metadata-only ops don't produce LLVM output
            vec![]
        }
    }
}

/// Lower all standard MLIR ops to LLVM dialect.
pub fn lower_all_to_llvm(ops: &[LoweredOp]) -> Vec<LlvmOp> {
    ops.iter().flat_map(|op| lower_to_llvm(op)).collect()
}

// ===========================================================================
// Full pipeline: Redox dialect → standard MLIR → LLVM dialect
// ===========================================================================

/// Result of running the full pipeline.
#[derive(Debug, Clone)]
pub struct PipelineResult {
    /// Redox dialect ops (input).
    pub redox_ops: Vec<RedoxOp>,
    /// Standard MLIR ops (after Redox lowering).
    pub standard_ops: Vec<LoweredOp>,
    /// LLVM dialect ops (final output).
    pub llvm_ops: Vec<LlvmOp>,
}

/// Run the full progressive lowering pipeline:
/// Redox Dialect → Standard MLIR → LLVM Dialect.
pub fn run_pipeline(ops: Vec<RedoxOp>, config: &PipelineConfig) -> PipelineResult {
    let standard_ops = lower_ops(&ops, config);
    let llvm_ops = lower_all_to_llvm(&standard_ops);
    PipelineResult { redox_ops: ops, standard_ops, llvm_ops }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir_to_mlir::*;

    fn debug_config() -> PipelineConfig {
        PipelineConfig { opt_level: OptLevel::Debug }
    }

    fn release_config() -> PipelineConfig {
        PipelineConfig { opt_level: OptLevel::Release }
    }

    // -- Ownership lowering --------------------------------------------------

    #[test]
    fn lower_move_to_std() {
        let op = RedoxOp::Move(MoveOp {
            source_type: owned_type("i32"),
            result_type: owned_type("i32"),
        });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].dialect, LoweringTarget::Std);
        assert_eq!(lowered[0].name, "std.copy_ssa");
    }

    #[test]
    fn lower_copy_to_memref() {
        let op = RedoxOp::Copy(CopyOp { source_type: owned_type("i32") });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::MemRef);
        assert_eq!(lowered[0].name, "memref.copy");
    }

    #[test]
    fn lower_borrow_to_memref_view() {
        let op = RedoxOp::Borrow(BorrowOp {
            source_type: owned_type("String"),
            mode: BorrowMode::Shared,
            region: region_type("'a"),
        });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].name, "memref.view");
    }

    #[test]
    fn lower_drop_to_func_call() {
        let op = RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Func);
        assert_eq!(lowered[0].name, "func.call");
    }

    // -- Contract lowering (debug vs release) --------------------------------

    #[test]
    fn lower_require_debug_emits_assert() {
        let op = RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].name, "cf.assert");
    }

    #[test]
    fn lower_require_release_elided() {
        let op = RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() });
        let lowered = lower_op(&op, &release_config());
        assert!(lowered.is_empty());
    }

    #[test]
    fn lower_ensure_debug_emits_assert() {
        let op =
            RedoxOp::ContractEnsure(EnsureOp { message: "sorted".into(), has_return_value: true });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered.len(), 1);
        assert_eq!(lowered[0].name, "cf.assert");
    }

    #[test]
    fn lower_invariant_release_elided() {
        let op = RedoxOp::ContractInvariant(InvariantOp {
            message: "len > 0".into(),
            kind: InvariantKind::Loop,
        });
        let lowered = lower_op(&op, &release_config());
        assert!(lowered.is_empty());
    }

    // -- Effect lowering -----------------------------------------------------

    #[test]
    fn lower_effect_decl_is_metadata() {
        let op = RedoxOp::EffectDecl(EffectDeclOp { effects: vec!["IO".into()], handlers: vec![] });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Redox);
    }

    #[test]
    fn lower_effect_perform_to_func_call() {
        let op = RedoxOp::EffectPerform(EffectPerformOp {
            effect: effect_type("IO"),
            arg_types: vec![],
            result_type: None,
        });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].name, "func.call");
    }

    // -- Performance lowering ------------------------------------------------

    #[test]
    fn lower_place_gpu_to_launch() {
        let op = RedoxOp::PerfPlace(PlaceOp { target: PlaceTarget::Gpu, priority: None });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Gpu);
        assert_eq!(lowered[0].name, "gpu.launch_func");
    }

    #[test]
    fn lower_place_cpu_to_func() {
        let op = RedoxOp::PerfPlace(PlaceOp { target: PlaceTarget::Cpu, priority: Some(10) });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Func);
    }

    #[test]
    fn lower_vectorize_produces_two_ops() {
        let op = RedoxOp::PerfVectorize(VectorizeOp { width: 8 });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered.len(), 2);
        assert_eq!(lowered[0].name, "vector.transfer_read");
        assert_eq!(lowered[1].name, "vector.transfer_write");
    }

    #[test]
    fn lower_cost_query_to_arith_constant() {
        let op = RedoxOp::PerfCostQuery(CostQueryOp {
            target_hw: "x86_64".into(),
            metric: CostMetric::LatencyNs,
        });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Arith);
        assert_eq!(lowered[0].name, "arith.constant");
    }

    // -- Capability lowering -------------------------------------------------

    #[test]
    fn lower_capability_gate_to_scf_if() {
        let op = RedoxOp::CapabilityGate(CapabilityGateOp {
            token: CapabilityType { capabilities: vec!["net".into()] },
        });
        let lowered = lower_op(&op, &debug_config());
        assert_eq!(lowered[0].dialect, LoweringTarget::Scf);
        assert_eq!(lowered[0].name, "scf.if");
    }

    // -- LLVM dialect lowering -----------------------------------------------

    #[test]
    fn lower_std_to_llvm_store() {
        let op = LoweredOp::new(LoweringTarget::Std, "std.copy_ssa", "move");
        let llvm = lower_to_llvm(&op);
        assert_eq!(llvm.len(), 1);
        assert_eq!(llvm[0].name, "llvm.store");
    }

    #[test]
    fn lower_memref_to_llvm_load_store() {
        let op = LoweredOp::new(LoweringTarget::MemRef, "memref.copy", "copy");
        let llvm = lower_to_llvm(&op);
        assert_eq!(llvm.len(), 2);
        assert_eq!(llvm[0].name, "llvm.load");
        assert_eq!(llvm[1].name, "llvm.store");
    }

    #[test]
    fn lower_func_to_llvm_call() {
        let op = LoweredOp::new(LoweringTarget::Func, "func.call", "drop");
        let llvm = lower_to_llvm(&op);
        assert_eq!(llvm[0].name, "llvm.call");
    }

    #[test]
    fn lower_vector_to_llvm_intrinsics() {
        let read = LoweredOp::new(LoweringTarget::Vector, "vector.transfer_read", "vec read");
        let write = LoweredOp::new(LoweringTarget::Vector, "vector.transfer_write", "vec write");
        assert_eq!(lower_to_llvm(&read)[0].name, "llvm.intr.masked.load");
        assert_eq!(lower_to_llvm(&write)[0].name, "llvm.intr.masked.store");
    }

    #[test]
    fn lower_redox_metadata_no_llvm() {
        let op = LoweredOp::new(LoweringTarget::Redox, "redox.effect.decl", "metadata");
        let llvm = lower_to_llvm(&op);
        assert!(llvm.is_empty());
    }

    // -- Full pipeline: end-to-end integration test --------------------------

    #[test]
    fn full_pipeline_simple_program_debug() {
        // Simulate: fn add_and_drop(x: i32, s: String) -> i32 {
        //   let y = x;          // copy
        //   require(y > 0);     // contract
        //   let r = &s;         // borrow
        //   drop(s);            // drop
        //   y
        // }
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

        let result = run_pipeline(ops, &debug_config());

        // Redox → Standard: 4 input ops produce 4 standard ops
        assert_eq!(result.redox_ops.len(), 4);
        assert_eq!(result.standard_ops.len(), 4);

        // Standard ops: memref.copy, cf.assert, memref.view, func.call
        assert_eq!(result.standard_ops[0].name, "memref.copy");
        assert_eq!(result.standard_ops[1].name, "cf.assert");
        assert_eq!(result.standard_ops[2].name, "memref.view");
        assert_eq!(result.standard_ops[3].name, "func.call");

        // Standard → LLVM: all produce LLVM ops
        assert!(!result.llvm_ops.is_empty());
        // memref.copy → load+store (2), cf.assert → cond_br (1),
        // memref.view → load+store (2), func.call → call (1) = 6
        assert_eq!(result.llvm_ops.len(), 6);
    }

    #[test]
    fn full_pipeline_release_elides_contracts() {
        let ops = vec![
            RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }),
            RedoxOp::Move(MoveOp {
                source_type: owned_type("i32"),
                result_type: owned_type("i32"),
            }),
            RedoxOp::ContractEnsure(EnsureOp {
                message: "result valid".into(),
                has_return_value: true,
            }),
        ];

        let result = run_pipeline(ops, &release_config());

        // In release: contracts elided → only the move survives
        assert_eq!(result.standard_ops.len(), 1);
        assert_eq!(result.standard_ops[0].name, "std.copy_ssa");

        // LLVM: just the store from the move
        assert_eq!(result.llvm_ops.len(), 1);
        assert_eq!(result.llvm_ops[0].name, "llvm.store");
    }

    // -- Integration: MIR → Redox Dialect → Standard → LLVM -----------------

    #[test]
    fn end_to_end_mir_through_full_pipeline() {
        // Build a MIR body
        let body = MirBody {
            name: "example".into(),
            blocks: vec![
                BasicBlock {
                    label: "bb0".into(),
                    statements: vec![
                        Statement::Assign(
                            Place::local(0),
                            Rvalue::Use(Operand::Move(Place::local(1))),
                        ),
                        Statement::Assign(
                            Place::local(2),
                            Rvalue::Ref { mutable: false, place: Place::local(0) },
                        ),
                    ],
                    terminator: Terminator::Drop { place: Place::local(1), target: 1 },
                },
                BasicBlock {
                    label: "bb1".into(),
                    statements: vec![],
                    terminator: Terminator::Return,
                },
            ],
            local_types: vec![
                LocalType { local: 0, type_name: "String".into(), is_copy: false },
                LocalType { local: 1, type_name: "String".into(), is_copy: false },
                LocalType { local: 2, type_name: "String".into(), is_copy: false },
            ],
        };

        // Step 1: MIR → Redox dialect
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        let redox_ops = all_ops(&blocks);

        // Should have: move + borrow + drop = 3 ops
        assert_eq!(redox_ops.len(), 3);
        assert_eq!(redox_ops[0].op_name(), "redox.move");
        assert_eq!(redox_ops[1].op_name(), "redox.borrow");
        assert_eq!(redox_ops[2].op_name(), "redox.drop");

        // Verify all ops are valid
        let errors = verify_ops(&redox_ops);
        assert!(errors.is_empty(), "verification errors: {errors:?}");

        // Step 2: Run through full lowering pipeline
        let result = run_pipeline(redox_ops, &debug_config());

        // Standard: std.copy_ssa + memref.view + func.call = 3 ops
        assert_eq!(result.standard_ops.len(), 3);

        // LLVM: store(1) + load+store(2) + call(1) = 4 ops
        assert_eq!(result.llvm_ops.len(), 4);

        // Verify LLVM op names
        assert_eq!(result.llvm_ops[0].name, "llvm.store");
        assert_eq!(result.llvm_ops[1].name, "llvm.load");
        assert_eq!(result.llvm_ops[2].name, "llvm.store");
        assert_eq!(result.llvm_ops[3].name, "llvm.call");
    }
}
