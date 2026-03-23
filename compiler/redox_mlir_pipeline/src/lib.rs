//! # MLIR Pipeline for FPGA, NPU, and TPU Targets
//!
//! Implements an MLIR→CIRCT pipeline for FPGA targets and MLIR
//! StableHLO/TOSA dialect pipelines for NPU/TPU targets.

use std::collections::HashMap;
use std::fmt;

// ── Targets ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HardwareTarget {
    Fpga,
    Npu,
    Tpu,
    Gpu,
    Cpu,
}

impl fmt::Display for HardwareTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fpga => write!(f, "FPGA"),
            Self::Npu => write!(f, "NPU"),
            Self::Tpu => write!(f, "TPU"),
            Self::Gpu => write!(f, "GPU"),
            Self::Cpu => write!(f, "CPU"),
        }
    }
}

// ── Dialects ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MlirDialect {
    Arith,
    Func,
    Scf,
    Memref,
    Tensor,
    Linalg,
    StableHlo,
    Tosa,
    Circt,
    CirctHw,
    CirctComb,
    CirctSeq,
    CirctFirrtl,
    Affine,
}

impl fmt::Display for MlirDialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arith => write!(f, "arith"),
            Self::Func => write!(f, "func"),
            Self::Scf => write!(f, "scf"),
            Self::Memref => write!(f, "memref"),
            Self::Tensor => write!(f, "tensor"),
            Self::Linalg => write!(f, "linalg"),
            Self::StableHlo => write!(f, "stablehlo"),
            Self::Tosa => write!(f, "tosa"),
            Self::Circt => write!(f, "circt"),
            Self::CirctHw => write!(f, "circt.hw"),
            Self::CirctComb => write!(f, "circt.comb"),
            Self::CirctSeq => write!(f, "circt.seq"),
            Self::CirctFirrtl => write!(f, "circt.firrtl"),
            Self::Affine => write!(f, "affine"),
        }
    }
}

// ── IR Operations ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MlirOperation {
    pub name: String,
    pub dialect: MlirDialect,
    pub operands: Vec<String>,
    pub results: Vec<String>,
    pub attributes: HashMap<String, String>,
}

impl MlirOperation {
    pub fn new(name: impl Into<String>, dialect: MlirDialect) -> Self {
        Self {
            name: name.into(),
            dialect,
            operands: Vec::new(),
            results: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    pub fn with_operand(mut self, op: impl Into<String>) -> Self {
        self.operands.push(op.into());
        self
    }

    pub fn with_result(mut self, r: impl Into<String>) -> Self {
        self.results.push(r.into());
        self
    }

    pub fn with_attr(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), val.into());
        self
    }
}

impl fmt::Display for MlirOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.dialect, self.name)?;
        if !self.operands.is_empty() {
            write!(f, "({})", self.operands.join(", "))?;
        }
        if !self.results.is_empty() {
            write!(f, " -> ({})", self.results.join(", "))?;
        }
        Ok(())
    }
}

/// A module of MLIR operations.
#[derive(Debug, Clone)]
pub struct MlirModule {
    pub name: String,
    pub operations: Vec<MlirOperation>,
    pub dialects_used: Vec<MlirDialect>,
}

impl MlirModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), operations: Vec::new(), dialects_used: Vec::new() }
    }

    pub fn add_op(&mut self, op: MlirOperation) {
        if !self.dialects_used.contains(&op.dialect) {
            self.dialects_used.push(op.dialect);
        }
        self.operations.push(op);
    }

    pub fn op_count(&self) -> usize {
        self.operations.len()
    }

    pub fn uses_dialect(&self, d: MlirDialect) -> bool {
        self.dialects_used.contains(&d)
    }
}

// ── Pipeline Passes ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassKind {
    Canonicalize,
    CSE,
    InlinePass,
    Bufferize,
    LowerToAffine,
    LowerToLinalg,
    LowerToStableHlo,
    LowerToTosa,
    LowerToCirct,
    LowerToHw,
    LowerToFirrtl,
    ConvertToVerilog,
    SchedulePass,
    TilePass,
    FusePass,
}

impl fmt::Display for PassKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonicalize => write!(f, "canonicalize"),
            Self::CSE => write!(f, "cse"),
            Self::InlinePass => write!(f, "inline"),
            Self::Bufferize => write!(f, "bufferize"),
            Self::LowerToAffine => write!(f, "lower-to-affine"),
            Self::LowerToLinalg => write!(f, "lower-to-linalg"),
            Self::LowerToStableHlo => write!(f, "lower-to-stablehlo"),
            Self::LowerToTosa => write!(f, "lower-to-tosa"),
            Self::LowerToCirct => write!(f, "lower-to-circt"),
            Self::LowerToHw => write!(f, "lower-to-hw"),
            Self::LowerToFirrtl => write!(f, "lower-to-firrtl"),
            Self::ConvertToVerilog => write!(f, "convert-to-verilog"),
            Self::SchedulePass => write!(f, "schedule"),
            Self::TilePass => write!(f, "tile"),
            Self::FusePass => write!(f, "fuse"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelinePass {
    pub kind: PassKind,
    pub options: HashMap<String, String>,
}

impl PipelinePass {
    pub fn new(kind: PassKind) -> Self {
        Self { kind, options: HashMap::new() }
    }

    pub fn with_option(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.options.insert(key.into(), val.into());
        self
    }
}

impl fmt::Display for PipelinePass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if !self.options.is_empty() {
            let opts: Vec<String> = self.options.iter().map(|(k, v)| format!("{k}={v}")).collect();
            write!(f, "{{{}}}", opts.join(", "))?;
        }
        Ok(())
    }
}

// ── Full Pipeline ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MlirPipeline {
    pub target: HardwareTarget,
    pub passes: Vec<PipelinePass>,
}

impl MlirPipeline {
    pub fn new(target: HardwareTarget) -> Self {
        Self { target, passes: Vec::new() }
    }

    pub fn add_pass(&mut self, pass: PipelinePass) {
        self.passes.push(pass);
    }

    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

impl fmt::Display for MlirPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Pipeline for {}:", self.target)?;
        for (i, p) in self.passes.iter().enumerate() {
            writeln!(f, "  {i}: {p}")?;
        }
        Ok(())
    }
}

/// Build the standard FPGA pipeline: high-level → CIRCT → Verilog.
pub fn build_fpga_pipeline() -> MlirPipeline {
    let mut p = MlirPipeline::new(HardwareTarget::Fpga);
    p.add_pass(PipelinePass::new(PassKind::Canonicalize));
    p.add_pass(PipelinePass::new(PassKind::CSE));
    p.add_pass(PipelinePass::new(PassKind::LowerToLinalg));
    p.add_pass(PipelinePass::new(PassKind::LowerToAffine));
    p.add_pass(PipelinePass::new(PassKind::LowerToCirct));
    p.add_pass(PipelinePass::new(PassKind::LowerToHw));
    p.add_pass(PipelinePass::new(PassKind::LowerToFirrtl));
    p.add_pass(PipelinePass::new(PassKind::SchedulePass));
    p.add_pass(PipelinePass::new(PassKind::ConvertToVerilog));
    p
}

/// Build the NPU pipeline: high-level → StableHLO → TOSA.
pub fn build_npu_pipeline() -> MlirPipeline {
    let mut p = MlirPipeline::new(HardwareTarget::Npu);
    p.add_pass(PipelinePass::new(PassKind::Canonicalize));
    p.add_pass(PipelinePass::new(PassKind::CSE));
    p.add_pass(PipelinePass::new(PassKind::LowerToLinalg));
    p.add_pass(PipelinePass::new(PassKind::LowerToStableHlo));
    p.add_pass(PipelinePass::new(PassKind::TilePass).with_option("tile_size", "16"));
    p.add_pass(PipelinePass::new(PassKind::FusePass));
    p.add_pass(PipelinePass::new(PassKind::LowerToTosa));
    p.add_pass(PipelinePass::new(PassKind::Bufferize));
    p
}

/// Build the TPU pipeline: high-level → StableHLO (TPU-specific tiling).
pub fn build_tpu_pipeline() -> MlirPipeline {
    let mut p = MlirPipeline::new(HardwareTarget::Tpu);
    p.add_pass(PipelinePass::new(PassKind::Canonicalize));
    p.add_pass(PipelinePass::new(PassKind::InlinePass));
    p.add_pass(PipelinePass::new(PassKind::LowerToLinalg));
    p.add_pass(PipelinePass::new(PassKind::LowerToStableHlo));
    p.add_pass(PipelinePass::new(PassKind::TilePass).with_option("tile_size", "128"));
    p.add_pass(PipelinePass::new(PassKind::FusePass));
    p.add_pass(PipelinePass::new(PassKind::SchedulePass));
    p.add_pass(PipelinePass::new(PassKind::Bufferize));
    p
}

/// Select the correct pipeline for a target.
pub fn pipeline_for_target(target: HardwareTarget) -> MlirPipeline {
    match target {
        HardwareTarget::Fpga => build_fpga_pipeline(),
        HardwareTarget::Npu => build_npu_pipeline(),
        HardwareTarget::Tpu => build_tpu_pipeline(),
        _ => {
            // Fallback: minimal pipeline
            let mut p = MlirPipeline::new(target);
            p.add_pass(PipelinePass::new(PassKind::Canonicalize));
            p.add_pass(PipelinePass::new(PassKind::CSE));
            p
        }
    }
}

/// Apply a pipeline to a module (simulate: just report which passes were applied).
pub fn apply_pipeline(module: &MlirModule, pipeline: &MlirPipeline) -> PipelineResult {
    PipelineResult {
        target: pipeline.target,
        module_name: module.name.clone(),
        passes_applied: pipeline.passes.len(),
        input_ops: module.op_count(),
        dialects_touched: module.dialects_used.clone(),
        success: true,
        message: format!("Applied {} passes to '{}'", pipeline.passes.len(), module.name),
    }
}

#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub target: HardwareTarget,
    pub module_name: String,
    pub passes_applied: usize,
    pub input_ops: usize,
    pub dialects_touched: Vec<MlirDialect>,
    pub success: bool,
    pub message: String,
}

impl fmt::Display for PipelineResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} — {}/{} passes on {} ops",
            self.target, if self.success { "OK" } else { "FAIL" },
            self.passes_applied, self.passes_applied, self.input_ops)
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_target_display() {
        assert_eq!(format!("{}", HardwareTarget::Fpga), "FPGA");
        assert_eq!(format!("{}", HardwareTarget::Tpu), "TPU");
    }

    #[test]
    fn test_mlir_dialect_display() {
        assert_eq!(format!("{}", MlirDialect::StableHlo), "stablehlo");
        assert_eq!(format!("{}", MlirDialect::CirctFirrtl), "circt.firrtl");
    }

    #[test]
    fn test_mlir_operation_new() {
        let op = MlirOperation::new("add", MlirDialect::Arith);
        assert_eq!(op.name, "add");
        assert_eq!(op.dialect, MlirDialect::Arith);
    }

    #[test]
    fn test_mlir_operation_builder() {
        let op = MlirOperation::new("matmul", MlirDialect::Linalg)
            .with_operand("%a").with_operand("%b")
            .with_result("%c")
            .with_attr("transpose", "true");
        assert_eq!(op.operands.len(), 2);
        assert_eq!(op.results.len(), 1);
        assert_eq!(op.attributes.get("transpose").unwrap(), "true");
    }

    #[test]
    fn test_mlir_operation_display() {
        let op = MlirOperation::new("add", MlirDialect::Arith)
            .with_operand("%x").with_operand("%y")
            .with_result("%z");
        let s = format!("{op}");
        assert!(s.contains("arith.add"));
        assert!(s.contains("%x, %y"));
    }

    #[test]
    fn test_mlir_module_new() {
        let m = MlirModule::new("test_mod");
        assert_eq!(m.op_count(), 0);
    }

    #[test]
    fn test_mlir_module_add_op() {
        let mut m = MlirModule::new("test_mod");
        m.add_op(MlirOperation::new("add", MlirDialect::Arith));
        assert_eq!(m.op_count(), 1);
        assert!(m.uses_dialect(MlirDialect::Arith));
    }

    #[test]
    fn test_mlir_module_dialects() {
        let mut m = MlirModule::new("m");
        m.add_op(MlirOperation::new("a", MlirDialect::Arith));
        m.add_op(MlirOperation::new("b", MlirDialect::Arith));
        assert_eq!(m.dialects_used.len(), 1); // deduped
    }

    #[test]
    fn test_pass_kind_display() {
        assert_eq!(format!("{}", PassKind::Canonicalize), "canonicalize");
        assert_eq!(format!("{}", PassKind::ConvertToVerilog), "convert-to-verilog");
    }

    #[test]
    fn test_pipeline_pass_display() {
        let p = PipelinePass::new(PassKind::TilePass).with_option("size", "16");
        let s = format!("{p}");
        assert!(s.contains("tile"));
        assert!(s.contains("size=16"));
    }

    #[test]
    fn test_build_fpga_pipeline() {
        let p = build_fpga_pipeline();
        assert_eq!(p.target, HardwareTarget::Fpga);
        assert!(p.pass_count() >= 5);
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::LowerToCirct));
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::ConvertToVerilog));
    }

    #[test]
    fn test_build_npu_pipeline() {
        let p = build_npu_pipeline();
        assert_eq!(p.target, HardwareTarget::Npu);
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::LowerToStableHlo));
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::LowerToTosa));
    }

    #[test]
    fn test_build_tpu_pipeline() {
        let p = build_tpu_pipeline();
        assert_eq!(p.target, HardwareTarget::Tpu);
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::LowerToStableHlo));
    }

    #[test]
    fn test_pipeline_for_target_fpga() {
        let p = pipeline_for_target(HardwareTarget::Fpga);
        assert_eq!(p.target, HardwareTarget::Fpga);
    }

    #[test]
    fn test_pipeline_for_target_fallback() {
        let p = pipeline_for_target(HardwareTarget::Cpu);
        assert_eq!(p.pass_count(), 2); // only canonicalize + cse
    }

    #[test]
    fn test_apply_pipeline() {
        let mut m = MlirModule::new("test");
        m.add_op(MlirOperation::new("add", MlirDialect::Arith));
        let pipeline = build_fpga_pipeline();
        let result = apply_pipeline(&m, &pipeline);
        assert!(result.success);
        assert_eq!(result.input_ops, 1);
        assert!(result.passes_applied > 0);
    }

    #[test]
    fn test_pipeline_result_display() {
        let result = PipelineResult {
            target: HardwareTarget::Fpga,
            module_name: "test".into(),
            passes_applied: 5,
            input_ops: 3,
            dialects_touched: vec![MlirDialect::Arith],
            success: true,
            message: "ok".into(),
        };
        let s = format!("{result}");
        assert!(s.contains("FPGA"));
        assert!(s.contains("OK"));
    }

    #[test]
    fn test_pipeline_display() {
        let p = build_fpga_pipeline();
        let s = format!("{p}");
        assert!(s.contains("FPGA"));
        assert!(s.contains("canonicalize"));
    }

    #[test]
    fn test_fpga_pipeline_has_firrtl() {
        let p = build_fpga_pipeline();
        assert!(p.passes.iter().any(|pass| pass.kind == PassKind::LowerToFirrtl));
    }

    #[test]
    fn test_npu_pipeline_has_tile() {
        let p = build_npu_pipeline();
        let tile = p.passes.iter().find(|pass| pass.kind == PassKind::TilePass).unwrap();
        assert_eq!(tile.options.get("tile_size").unwrap(), "16");
    }

    #[test]
    fn test_tpu_pipeline_has_tile_128() {
        let p = build_tpu_pipeline();
        let tile = p.passes.iter().find(|pass| pass.kind == PassKind::TilePass).unwrap();
        assert_eq!(tile.options.get("tile_size").unwrap(), "128");
    }

    #[test]
    fn test_all_targets_have_pipeline() {
        for target in [HardwareTarget::Fpga, HardwareTarget::Npu, HardwareTarget::Tpu, HardwareTarget::Gpu, HardwareTarget::Cpu] {
            let p = pipeline_for_target(target);
            assert!(p.pass_count() >= 2);
        }
    }

    #[test]
    fn test_pipeline_pass_new() {
        let p = PipelinePass::new(PassKind::CSE);
        assert_eq!(p.kind, PassKind::CSE);
        assert!(p.options.is_empty());
    }

    #[test]
    fn test_mlir_module_does_not_use_dialect() {
        let m = MlirModule::new("empty");
        assert!(!m.uses_dialect(MlirDialect::Tosa));
    }
}
