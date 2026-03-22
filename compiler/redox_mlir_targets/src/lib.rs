//! # Redox MLIR→LLVM Backend Targets
//!
//! Target-specific lowering passes and validation for RISC-V, AMDGPU, and NVPTX.
//!
//! Each target defines:
//! - **Target triple** and data layout
//! - **Target-specific lowering passes** that transform generic LLVM dialect ops
//!   into target-appropriate instruction sequences
//! - **Target validation** ensuring ops are legal for the target
//! - **Target features** (ISA extensions, compute capabilities)
//!
//! This crate is the target-specific counterpart of `redox_mlir`'s generic
//! lowering pipeline.
//!
//! (ROADMAP Step 56)

use std::collections::BTreeMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Target Architecture
// ═══════════════════════════════════════════════════════════════════════════

/// Supported backend target architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    /// RISC-V 64-bit (with optional extensions: V, M, A, F, D, C, Zb*).
    RiscV64,
    /// RISC-V 32-bit.
    RiscV32,
    /// AMD GPU (GCN / CDNA / RDNA micro-architectures).
    AmdGpu,
    /// NVIDIA GPU (PTX ISA via NVPTX backend).
    Nvptx64,
}

impl fmt::Display for TargetArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetArch::RiscV64 => write!(f, "riscv64"),
            TargetArch::RiscV32 => write!(f, "riscv32"),
            TargetArch::AmdGpu => write!(f, "amdgpu"),
            TargetArch::Nvptx64 => write!(f, "nvptx64"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Triple & Data Layout
// ═══════════════════════════════════════════════════════════════════════════

/// A target triple describing the compilation target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetTriple {
    pub arch: TargetArch,
    pub vendor: String,
    pub os: String,
    pub env: String,
}

impl TargetTriple {
    pub fn new(arch: TargetArch, vendor: &str, os: &str, env: &str) -> Self {
        TargetTriple {
            arch,
            vendor: vendor.to_string(),
            os: os.to_string(),
            env: env.to_string(),
        }
    }

    /// Canonical triple string (e.g., "riscv64-unknown-linux-gnu").
    pub fn to_triple_string(&self) -> String {
        format!("{}-{}-{}-{}", self.arch, self.vendor, self.os, self.env)
    }

    /// Standard RISC-V 64-bit Linux target.
    pub fn riscv64_linux() -> Self {
        TargetTriple::new(TargetArch::RiscV64, "unknown", "linux", "gnu")
    }

    /// Standard RISC-V 32-bit bare-metal target.
    pub fn riscv32_bare() -> Self {
        TargetTriple::new(TargetArch::RiscV32, "unknown", "none", "elf")
    }

    /// AMDGPU target (vendor=amd, os=amdhsa).
    pub fn amdgpu() -> Self {
        TargetTriple::new(TargetArch::AmdGpu, "amd", "amdhsa", "amdgcn")
    }

    /// NVPTX 64-bit target.
    pub fn nvptx64() -> Self {
        TargetTriple::new(TargetArch::Nvptx64, "nvidia", "cuda", "nvptx")
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_triple_string())
    }
}

/// Data layout description for a target (LLVM data layout string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    /// Endianness: "e" for little-endian, "E" for big-endian.
    pub endianness: Endianness,
    /// Natural pointer size in bits.
    pub pointer_size_bits: u32,
    /// Stack alignment in bits.
    pub stack_alignment_bits: u32,
    /// Native integer widths supported.
    pub native_int_widths: Vec<u32>,
}

/// Byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

impl DataLayout {
    /// RISC-V 64-bit data layout.
    pub fn riscv64() -> Self {
        DataLayout {
            endianness: Endianness::Little,
            pointer_size_bits: 64,
            stack_alignment_bits: 128,
            native_int_widths: vec![32, 64],
        }
    }

    /// RISC-V 32-bit data layout.
    pub fn riscv32() -> Self {
        DataLayout {
            endianness: Endianness::Little,
            pointer_size_bits: 32,
            stack_alignment_bits: 64,
            native_int_widths: vec![32],
        }
    }

    /// AMDGPU data layout (flat address space 64-bit).
    pub fn amdgpu() -> Self {
        DataLayout {
            endianness: Endianness::Little,
            pointer_size_bits: 64,
            stack_alignment_bits: 256,
            native_int_widths: vec![32, 64],
        }
    }

    /// NVPTX 64-bit data layout.
    pub fn nvptx64() -> Self {
        DataLayout {
            endianness: Endianness::Little,
            pointer_size_bits: 64,
            stack_alignment_bits: 128,
            native_int_widths: vec![32, 64],
        }
    }

    /// Get the data layout for a given architecture.
    pub fn for_arch(arch: TargetArch) -> Self {
        match arch {
            TargetArch::RiscV64 => Self::riscv64(),
            TargetArch::RiscV32 => Self::riscv32(),
            TargetArch::AmdGpu => Self::amdgpu(),
            TargetArch::Nvptx64 => Self::nvptx64(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Features
// ═══════════════════════════════════════════════════════════════════════════

/// A target feature (ISA extension, compute capability, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetFeature {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

impl TargetFeature {
    pub fn new(name: &str, description: &str, enabled: bool) -> Self {
        TargetFeature {
            name: name.to_string(),
            description: description.to_string(),
            enabled,
        }
    }
}

/// RISC-V ISA extensions.
pub fn riscv_features() -> Vec<TargetFeature> {
    vec![
        TargetFeature::new("m", "Integer multiplication/division", true),
        TargetFeature::new("a", "Atomic instructions", true),
        TargetFeature::new("f", "Single-precision floating-point", true),
        TargetFeature::new("d", "Double-precision floating-point", true),
        TargetFeature::new("c", "Compressed instructions (16-bit)", true),
        TargetFeature::new("v", "Vector extension (RVV 1.0)", false),
        TargetFeature::new("zba", "Address generation (Zba)", false),
        TargetFeature::new("zbb", "Basic bit manipulation (Zbb)", false),
        TargetFeature::new("zbs", "Single-bit instructions (Zbs)", false),
        TargetFeature::new("zicsr", "CSR access instructions", true),
        TargetFeature::new("zifencei", "Instruction fence", true),
    ]
}

/// AMDGPU features (GFX generations).
pub fn amdgpu_features() -> Vec<TargetFeature> {
    vec![
        TargetFeature::new("gfx900", "Vega 10 (GCN5)", false),
        TargetFeature::new("gfx906", "Vega 20 (MI50/MI60)", false),
        TargetFeature::new("gfx908", "CDNA (MI100)", false),
        TargetFeature::new("gfx90a", "CDNA2 (MI200)", false),
        TargetFeature::new("gfx940", "CDNA3 (MI300X)", false),
        TargetFeature::new("gfx1010", "RDNA (RX 5000)", false),
        TargetFeature::new("gfx1030", "RDNA2 (RX 6000)", false),
        TargetFeature::new("gfx1100", "RDNA3 (RX 7000)", false),
        TargetFeature::new("wavefront64", "64-wide wavefronts (GCN/CDNA)", true),
        TargetFeature::new("wavefront32", "32-wide wavefronts (RDNA)", false),
    ]
}

/// NVPTX features (compute capabilities).
pub fn nvptx_features() -> Vec<TargetFeature> {
    vec![
        TargetFeature::new("sm_50", "Maxwell (GTX 900)", false),
        TargetFeature::new("sm_60", "Pascal (GTX 1000 / P100)", false),
        TargetFeature::new("sm_70", "Volta (V100)", false),
        TargetFeature::new("sm_75", "Turing (RTX 2000 / T4)", false),
        TargetFeature::new("sm_80", "Ampere (A100 / RTX 3000)", false),
        TargetFeature::new("sm_86", "Ampere (RTX 3000 Ti)", false),
        TargetFeature::new("sm_89", "Ada Lovelace (RTX 4000 / L40)", false),
        TargetFeature::new("sm_90", "Hopper (H100)", false),
        TargetFeature::new("sm_100", "Blackwell (B100/B200)", false),
        TargetFeature::new("ptx80", "PTX ISA version 8.0", false),
        TargetFeature::new("ptx83", "PTX ISA version 8.3", false),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Full target configuration for code generation.
#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub triple: TargetTriple,
    pub data_layout: DataLayout,
    pub features: Vec<TargetFeature>,
    /// CPU model string (e.g., "generic-rv64", "gfx90a", "sm_80").
    pub cpu: String,
}

impl TargetConfig {
    /// Create a RISC-V 64-bit target with default extensions (RV64IMAFDC).
    pub fn riscv64_default() -> Self {
        TargetConfig {
            triple: TargetTriple::riscv64_linux(),
            data_layout: DataLayout::riscv64(),
            features: riscv_features(),
            cpu: "generic-rv64".to_string(),
        }
    }

    /// Create an AMDGPU target with a specific GFX generation.
    pub fn amdgpu_gfx(gfx: &str) -> Self {
        let mut features = amdgpu_features();
        for f in &mut features {
            if f.name == gfx {
                f.enabled = true;
            }
        }
        TargetConfig {
            triple: TargetTriple::amdgpu(),
            data_layout: DataLayout::amdgpu(),
            features,
            cpu: gfx.to_string(),
        }
    }

    /// Create an NVPTX target with a specific compute capability.
    pub fn nvptx_sm(sm: &str) -> Self {
        let mut features = nvptx_features();
        for f in &mut features {
            if f.name == sm {
                f.enabled = true;
            }
        }
        TargetConfig {
            triple: TargetTriple::nvptx64(),
            data_layout: DataLayout::nvptx64(),
            features,
            cpu: sm.to_string(),
        }
    }

    /// Get the enabled feature string (LLVM-style, e.g., "+m,+a,+f,+d,+c").
    pub fn feature_string(&self) -> String {
        self.features
            .iter()
            .filter(|f| f.enabled)
            .map(|f| format!("+{}", f.name))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Check if a specific feature is enabled.
    pub fn has_feature(&self, name: &str) -> bool {
        self.features.iter().any(|f| f.name == name && f.enabled)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Target-Specific Lowering Ops
// ═══════════════════════════════════════════════════════════════════════════

/// A target-specific lowered instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOp {
    /// The target architecture this instruction is for.
    pub arch: TargetArch,
    /// Instruction mnemonic or LLVM IR representation.
    pub instruction: String,
    /// Human-readable description.
    pub comment: String,
}

impl TargetOp {
    pub fn new(arch: TargetArch, instruction: &str, comment: &str) -> Self {
        TargetOp {
            arch,
            instruction: instruction.to_string(),
            comment: comment.to_string(),
        }
    }
}

impl fmt::Display for TargetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ; {}", self.arch, self.instruction, self.comment)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Generic LLVM IR Op (input to target lowering)
// ═══════════════════════════════════════════════════════════════════════════

/// A generic LLVM-dialect operation to be lowered to target-specific form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericOp {
    /// LLVM dialect op name (e.g., "llvm.store", "llvm.call").
    pub name: String,
    /// Operand types involved.
    pub operand_types: Vec<String>,
    /// Comment from upstream lowering.
    pub comment: String,
}

impl GenericOp {
    pub fn new(name: &str, comment: &str) -> Self {
        GenericOp {
            name: name.to_string(),
            operand_types: Vec::new(),
            comment: comment.to_string(),
        }
    }

    pub fn with_operands(mut self, types: &[&str]) -> Self {
        self.operand_types = types.iter().map(|t| t.to_string()).collect();
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RISC-V Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// RISC-V specific lowering pass.
pub fn lower_riscv(op: &GenericOp, config: &TargetConfig) -> Vec<TargetOp> {
    let arch = config.triple.arch;
    match op.name.as_str() {
        "llvm.store" => {
            vec![TargetOp::new(arch, "sd %rs2, 0(%rs1)", "store doubleword")]
        }
        "llvm.load" => {
            vec![TargetOp::new(arch, "ld %rd, 0(%rs1)", "load doubleword")]
        }
        "llvm.call" => {
            vec![
                TargetOp::new(arch, "auipc ra, %pcrel_hi(target)", "address upper immediate"),
                TargetOp::new(arch, "jalr ra, ra, %pcrel_lo(.L0)", "jump and link register"),
            ]
        }
        "llvm.cond_br" => {
            vec![TargetOp::new(arch, "bne %rs1, x0, .Ltarget", "branch if not equal")]
        }
        "llvm.intr.masked.load" => {
            if config.has_feature("v") {
                vec![
                    TargetOp::new(arch, "vsetvli t0, a0, e32, m1", "set vector length (RVV)"),
                    TargetOp::new(arch, "vle32.v v0, (a1)", "vector load (RVV)"),
                ]
            } else {
                // Fallback: scalar loop
                vec![TargetOp::new(arch, "lw %rd, 0(%rs1) ; loop", "scalar load fallback (no RVV)")]
            }
        }
        "llvm.intr.masked.store" => {
            if config.has_feature("v") {
                vec![
                    TargetOp::new(arch, "vsetvli t0, a0, e32, m1", "set vector length (RVV)"),
                    TargetOp::new(arch, "vse32.v v0, (a1)", "vector store (RVV)"),
                ]
            } else {
                vec![TargetOp::new(arch, "sw %rs2, 0(%rs1) ; loop", "scalar store fallback (no RVV)")]
            }
        }
        "llvm.mlir.constant" => {
            vec![TargetOp::new(arch, "li %rd, <imm>", "load immediate")]
        }
        _ => {
            vec![TargetOp::new(arch, &format!("# unsupported: {}", op.name), "generic fallback")]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// AMDGPU Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// AMDGPU specific lowering pass.
pub fn lower_amdgpu(op: &GenericOp, config: &TargetConfig) -> Vec<TargetOp> {
    let arch = config.triple.arch;
    let is_rdna = config.has_feature("wavefront32")
        || config.cpu.starts_with("gfx10")
        || config.cpu.starts_with("gfx11");

    match op.name.as_str() {
        "llvm.store" => {
            vec![TargetOp::new(arch, "flat_store_dword v[0:1], v2", "flat memory store")]
        }
        "llvm.load" => {
            vec![TargetOp::new(arch, "flat_load_dword v2, v[0:1]", "flat memory load")]
        }
        "llvm.call" => {
            vec![
                TargetOp::new(arch, "s_getpc_b64 s[0:1]", "get PC for call"),
                TargetOp::new(arch, "s_add_u32 s0, s0, target@rel32@lo+4", "compute call target"),
                TargetOp::new(arch, "s_addc_u32 s1, s1, target@rel32@hi+12", "compute call target (hi)"),
                TargetOp::new(arch, "s_swappc_b64 s[30:31], s[0:1]", "swap PC (call)"),
            ]
        }
        "llvm.cond_br" => {
            if is_rdna {
                vec![TargetOp::new(arch, "s_cbranch_scc1 .Ltarget", "RDNA scalar conditional branch")]
            } else {
                vec![TargetOp::new(arch, "s_cbranch_scc1 .Ltarget", "GCN scalar conditional branch")]
            }
        }
        "llvm.intr.masked.load" | "llvm.intr.masked.store" => {
            let wave_width = if is_rdna { 32 } else { 64 };
            let ld_or_st = if op.name.contains("load") { "load" } else { "store" };
            vec![TargetOp::new(
                arch,
                &format!("buffer_{}_{} v0, v1, s[0:3], 0 offen", ld_or_st, if wave_width == 32 { "b32" } else { "dword" }),
                &format!("buffer {ld_or_st} (wave{wave_width})"),
            )]
        }
        "llvm.mlir.constant" => {
            vec![TargetOp::new(arch, "s_mov_b32 s0, <imm>", "scalar move immediate")]
        }
        _ => {
            vec![TargetOp::new(arch, &format!("; unsupported: {}", op.name), "generic fallback")]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// NVPTX Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// NVPTX specific lowering pass.
pub fn lower_nvptx(op: &GenericOp, config: &TargetConfig) -> Vec<TargetOp> {
    let arch = config.triple.arch;
    let sm_level = parse_sm_level(&config.cpu);

    match op.name.as_str() {
        "llvm.store" => {
            vec![TargetOp::new(arch, "st.global.u32 [%rd0], %r0", "global memory store")]
        }
        "llvm.load" => {
            vec![TargetOp::new(arch, "ld.global.u32 %r0, [%rd0]", "global memory load")]
        }
        "llvm.call" => {
            vec![TargetOp::new(arch, "call.uni _target, (params)", "device function call")]
        }
        "llvm.cond_br" => {
            vec![
                TargetOp::new(arch, "setp.ne.u32 %p0, %r0, 0", "set predicate"),
                TargetOp::new(arch, "@%p0 bra .Ltarget", "predicated branch"),
            ]
        }
        "llvm.intr.masked.load" | "llvm.intr.masked.store" => {
            if sm_level >= 80 {
                // Ampere+ supports async copy for global→shared
                let ld_or_st = if op.name.contains("load") { "ld" } else { "st" };
                vec![TargetOp::new(
                    arch,
                    &format!("cp.async.{ld_or_st}.global.shared [%rd0], [%rd1], 4"),
                    &format!("async copy ({ld_or_st}) — SM {sm_level}"),
                )]
            } else {
                let ld_or_st = if op.name.contains("load") { "ld" } else { "st" };
                vec![TargetOp::new(
                    arch,
                    &format!("{ld_or_st}.global.u32 %r0, [%rd0]"),
                    &format!("global {ld_or_st} — SM {sm_level}"),
                )]
            }
        }
        "llvm.mlir.constant" => {
            vec![TargetOp::new(arch, "mov.u32 %r0, <imm>", "move immediate")]
        }
        _ => {
            vec![TargetOp::new(arch, &format!("// unsupported: {}", op.name), "generic fallback")]
        }
    }
}

/// Parse SM compute capability level from a cpu string like "sm_80".
fn parse_sm_level(cpu: &str) -> u32 {
    cpu.strip_prefix("sm_")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(50)
}

// ═══════════════════════════════════════════════════════════════════════════
// Unified Target Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// Lower a generic LLVM-dialect op to target-specific instructions.
pub fn lower_for_target(op: &GenericOp, config: &TargetConfig) -> Vec<TargetOp> {
    match config.triple.arch {
        TargetArch::RiscV64 | TargetArch::RiscV32 => lower_riscv(op, config),
        TargetArch::AmdGpu => lower_amdgpu(op, config),
        TargetArch::Nvptx64 => lower_nvptx(op, config),
    }
}

/// Lower a sequence of generic ops to target-specific instructions.
pub fn lower_all_for_target(ops: &[GenericOp], config: &TargetConfig) -> Vec<TargetOp> {
    ops.iter().flat_map(|op| lower_for_target(op, config)).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Validation
// ═══════════════════════════════════════════════════════════════════════════

/// A validation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationDiag {
    pub severity: DiagSeverity,
    pub message: String,
    /// The op that triggered this diagnostic (index in input sequence).
    pub op_index: usize,
}

/// Severity for validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagSeverity {
    Warning,
    Error,
}

impl fmt::Display for ValidationDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            DiagSeverity::Warning => "warning",
            DiagSeverity::Error => "error",
        };
        write!(f, "{sev}[op {}]: {}", self.op_index, self.message)
    }
}

/// Validate target-specific ops for a given configuration.
///
/// Checks:
/// - Ops are targeting the correct architecture.
/// - Feature-gated instructions have required features enabled.
/// - Known illegal patterns are flagged.
pub fn validate_target_ops(ops: &[TargetOp], config: &TargetConfig) -> Vec<ValidationDiag> {
    let mut diags = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        // Check arch match
        if op.arch != config.triple.arch {
            diags.push(ValidationDiag {
                severity: DiagSeverity::Error,
                message: format!(
                    "op targets {} but config targets {}",
                    op.arch, config.triple.arch
                ),
                op_index: i,
            });
        }

        // Target-specific validation
        match config.triple.arch {
            TargetArch::RiscV64 | TargetArch::RiscV32 => {
                validate_riscv_op(op, config, i, &mut diags);
            }
            TargetArch::AmdGpu => {
                validate_amdgpu_op(op, config, i, &mut diags);
            }
            TargetArch::Nvptx64 => {
                validate_nvptx_op(op, config, i, &mut diags);
            }
        }
    }
    diags
}

fn validate_riscv_op(
    op: &TargetOp,
    config: &TargetConfig,
    index: usize,
    diags: &mut Vec<ValidationDiag>,
) {
    // RVV instructions require 'v' extension
    if (op.instruction.contains("vle") || op.instruction.contains("vse")
        || op.instruction.contains("vsetvli"))
        && !config.has_feature("v")
    {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: "RVV instruction used but 'v' extension not enabled".to_string(),
            op_index: index,
        });
    }

    // RV32 should not emit 64-bit ops like 'ld' or 'sd'
    if config.triple.arch == TargetArch::RiscV32
        && (op.instruction.starts_with("ld ") || op.instruction.starts_with("sd "))
    {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: "64-bit load/store on RV32 target".to_string(),
            op_index: index,
        });
    }
}

fn validate_amdgpu_op(
    op: &TargetOp,
    config: &TargetConfig,
    index: usize,
    diags: &mut Vec<ValidationDiag>,
) {
    // Warn if no GFX target is selected
    let has_gfx = config.features.iter().any(|f| f.name.starts_with("gfx") && f.enabled);
    if !has_gfx {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Warning,
            message: "no GFX target selected; code may not run on hardware".to_string(),
            op_index: index,
        });
    }

    // Check for RDNA-specific features used on GCN
    if op.instruction.contains("b32") && config.has_feature("wavefront64") && !config.has_feature("wavefront32") {
        // b32 buffer ops are RDNA-native; on GCN they use dword
        diags.push(ValidationDiag {
            severity: DiagSeverity::Warning,
            message: "b32 buffer op on wavefront64 target; may use dword variant".to_string(),
            op_index: index,
        });
    }
}

fn validate_nvptx_op(
    op: &TargetOp,
    config: &TargetConfig,
    index: usize,
    diags: &mut Vec<ValidationDiag>,
) {
    let sm_level = parse_sm_level(&config.cpu);

    // async copy requires SM >= 80
    if op.instruction.contains("cp.async") && sm_level < 80 {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: format!(
                "cp.async requires SM >= 80 but target is SM {sm_level}"
            ),
            op_index: index,
        });
    }

    // Warn if SM level is very old
    if sm_level < 60 {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Warning,
            message: format!(
                "SM {sm_level} is legacy; consider targeting SM >= 70 for better performance"
            ),
            op_index: index,
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Info Summary
// ═══════════════════════════════════════════════════════════════════════════

/// Summary information about a target configuration.
pub struct TargetInfo {
    pub arch_name: String,
    pub triple: String,
    pub cpu: String,
    pub features_enabled: Vec<String>,
    pub pointer_size: u32,
    pub endianness: String,
}

impl TargetInfo {
    /// Build a summary from a target config.
    pub fn from_config(config: &TargetConfig) -> Self {
        TargetInfo {
            arch_name: config.triple.arch.to_string(),
            triple: config.triple.to_triple_string(),
            cpu: config.cpu.clone(),
            features_enabled: config
                .features
                .iter()
                .filter(|f| f.enabled)
                .map(|f| f.name.clone())
                .collect(),
            pointer_size: config.data_layout.pointer_size_bits,
            endianness: match config.data_layout.endianness {
                Endianness::Little => "little".to_string(),
                Endianness::Big => "big".to_string(),
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Target Triple ────────────────────────────────────────────────────

    #[test]
    fn riscv64_triple() {
        let t = TargetTriple::riscv64_linux();
        assert_eq!(t.to_triple_string(), "riscv64-unknown-linux-gnu");
    }

    #[test]
    fn riscv32_triple() {
        let t = TargetTriple::riscv32_bare();
        assert_eq!(t.to_triple_string(), "riscv32-unknown-none-elf");
    }

    #[test]
    fn amdgpu_triple() {
        let t = TargetTriple::amdgpu();
        assert_eq!(t.to_triple_string(), "amdgpu-amd-amdhsa-amdgcn");
    }

    #[test]
    fn nvptx64_triple() {
        let t = TargetTriple::nvptx64();
        assert_eq!(t.to_triple_string(), "nvptx64-nvidia-cuda-nvptx");
    }

    // ── Data Layout ──────────────────────────────────────────────────────

    #[test]
    fn data_layout_riscv64() {
        let dl = DataLayout::riscv64();
        assert_eq!(dl.pointer_size_bits, 64);
        assert_eq!(dl.endianness, Endianness::Little);
    }

    #[test]
    fn data_layout_amdgpu() {
        let dl = DataLayout::amdgpu();
        assert_eq!(dl.pointer_size_bits, 64);
        assert_eq!(dl.stack_alignment_bits, 256);
    }

    #[test]
    fn data_layout_nvptx64() {
        let dl = DataLayout::nvptx64();
        assert_eq!(dl.pointer_size_bits, 64);
    }

    #[test]
    fn data_layout_for_arch() {
        let dl = DataLayout::for_arch(TargetArch::RiscV32);
        assert_eq!(dl.pointer_size_bits, 32);
    }

    // ── Target Features ──────────────────────────────────────────────────

    #[test]
    fn riscv_default_features() {
        let feats = riscv_features();
        let enabled: Vec<_> = feats.iter().filter(|f| f.enabled).map(|f| f.name.as_str()).collect();
        assert!(enabled.contains(&"m"));
        assert!(enabled.contains(&"a"));
        assert!(enabled.contains(&"f"));
        assert!(enabled.contains(&"d"));
        assert!(enabled.contains(&"c"));
        assert!(!enabled.contains(&"v")); // RVV off by default
    }

    #[test]
    fn amdgpu_default_features() {
        let feats = amdgpu_features();
        let enabled: Vec<_> = feats.iter().filter(|f| f.enabled).map(|f| f.name.as_str()).collect();
        assert!(enabled.contains(&"wavefront64"));
        assert!(!enabled.contains(&"gfx90a")); // no GFX by default
    }

    #[test]
    fn nvptx_default_features() {
        let feats = nvptx_features();
        // All SM levels off by default
        assert!(feats.iter().all(|f| !f.enabled));
    }

    // ── Target Config ────────────────────────────────────────────────────

    #[test]
    fn riscv64_config() {
        let cfg = TargetConfig::riscv64_default();
        assert_eq!(cfg.cpu, "generic-rv64");
        assert!(cfg.has_feature("m"));
        assert!(!cfg.has_feature("v"));
        let fs = cfg.feature_string();
        assert!(fs.contains("+m"));
        assert!(fs.contains("+a"));
    }

    #[test]
    fn amdgpu_gfx90a_config() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        assert_eq!(cfg.cpu, "gfx90a");
        assert!(cfg.has_feature("gfx90a"));
        assert!(cfg.has_feature("wavefront64"));
    }

    #[test]
    fn nvptx_sm80_config() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        assert_eq!(cfg.cpu, "sm_80");
        assert!(cfg.has_feature("sm_80"));
    }

    #[test]
    fn feature_string_format() {
        let cfg = TargetConfig::nvptx_sm("sm_70");
        let fs = cfg.feature_string();
        assert_eq!(fs, "+sm_70");
    }

    // ── RISC-V Lowering ──────────────────────────────────────────────────

    #[test]
    fn riscv_lower_store() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.store", "test store");
        let result = lower_riscv(&op, &cfg);
        assert_eq!(result.len(), 1);
        assert!(result[0].instruction.contains("sd"));
    }

    #[test]
    fn riscv_lower_load() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.load", "test load");
        let result = lower_riscv(&op, &cfg);
        assert!(result[0].instruction.contains("ld"));
    }

    #[test]
    fn riscv_lower_call() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.call", "test call");
        let result = lower_riscv(&op, &cfg);
        assert_eq!(result.len(), 2);
        assert!(result[0].instruction.contains("auipc"));
        assert!(result[1].instruction.contains("jalr"));
    }

    #[test]
    fn riscv_vector_with_rvv() {
        let mut cfg = TargetConfig::riscv64_default();
        for f in &mut cfg.features {
            if f.name == "v" {
                f.enabled = true;
            }
        }
        let op = GenericOp::new("llvm.intr.masked.load", "vector load");
        let result = lower_riscv(&op, &cfg);
        assert_eq!(result.len(), 2);
        assert!(result[0].instruction.contains("vsetvli"));
        assert!(result[1].instruction.contains("vle32"));
    }

    #[test]
    fn riscv_vector_without_rvv() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.intr.masked.load", "vector load");
        let result = lower_riscv(&op, &cfg);
        assert_eq!(result.len(), 1);
        assert!(result[0].instruction.contains("lw")); // scalar fallback
    }

    #[test]
    fn riscv_lower_constant() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.mlir.constant", "constant");
        let result = lower_riscv(&op, &cfg);
        assert!(result[0].instruction.contains("li"));
    }

    // ── AMDGPU Lowering ─────────────────────────────────────────────────

    #[test]
    fn amdgpu_lower_store() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        let op = GenericOp::new("llvm.store", "test store");
        let result = lower_amdgpu(&op, &cfg);
        assert!(result[0].instruction.contains("flat_store"));
    }

    #[test]
    fn amdgpu_lower_call() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        let op = GenericOp::new("llvm.call", "test call");
        let result = lower_amdgpu(&op, &cfg);
        assert_eq!(result.len(), 4);
        assert!(result[0].instruction.contains("s_getpc"));
        assert!(result[3].instruction.contains("s_swappc"));
    }

    #[test]
    fn amdgpu_rdna_branch() {
        let mut cfg = TargetConfig::amdgpu_gfx("gfx1100");
        for f in &mut cfg.features {
            if f.name == "wavefront32" {
                f.enabled = true;
            }
        }
        let op = GenericOp::new("llvm.cond_br", "branch");
        let result = lower_amdgpu(&op, &cfg);
        assert!(result[0].comment.contains("RDNA"));
    }

    #[test]
    fn amdgpu_gcn_buffer() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        let op = GenericOp::new("llvm.intr.masked.load", "vector load");
        let result = lower_amdgpu(&op, &cfg);
        assert!(result[0].instruction.contains("buffer_load"));
        assert!(result[0].comment.contains("wave64"));
    }

    #[test]
    fn amdgpu_rdna_buffer() {
        let mut cfg = TargetConfig::amdgpu_gfx("gfx1100");
        for f in &mut cfg.features {
            if f.name == "wavefront32" {
                f.enabled = true;
            }
        }
        let op = GenericOp::new("llvm.intr.masked.store", "vector store");
        let result = lower_amdgpu(&op, &cfg);
        assert!(result[0].comment.contains("wave32"));
    }

    // ── NVPTX Lowering ──────────────────────────────────────────────────

    #[test]
    fn nvptx_lower_store() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.store", "test store");
        let result = lower_nvptx(&op, &cfg);
        assert!(result[0].instruction.contains("st.global"));
    }

    #[test]
    fn nvptx_lower_call() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.call", "test call");
        let result = lower_nvptx(&op, &cfg);
        assert!(result[0].instruction.contains("call.uni"));
    }

    #[test]
    fn nvptx_conditioned_branch() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.cond_br", "branch");
        let result = lower_nvptx(&op, &cfg);
        assert_eq!(result.len(), 2);
        assert!(result[0].instruction.contains("setp"));
        assert!(result[1].instruction.contains("bra"));
    }

    #[test]
    fn nvptx_async_copy_sm80() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.intr.masked.load", "vector load");
        let result = lower_nvptx(&op, &cfg);
        assert!(result[0].instruction.contains("cp.async"));
    }

    #[test]
    fn nvptx_no_async_copy_sm70() {
        let cfg = TargetConfig::nvptx_sm("sm_70");
        let op = GenericOp::new("llvm.intr.masked.load", "vector load");
        let result = lower_nvptx(&op, &cfg);
        assert!(!result[0].instruction.contains("cp.async"));
        assert!(result[0].instruction.contains("ld.global"));
    }

    #[test]
    fn nvptx_lower_constant() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.mlir.constant", "constant");
        let result = lower_nvptx(&op, &cfg);
        assert!(result[0].instruction.contains("mov.u32"));
    }

    // ── Unified Lowering ─────────────────────────────────────────────────

    #[test]
    fn unified_riscv_dispatch() {
        let cfg = TargetConfig::riscv64_default();
        let op = GenericOp::new("llvm.store", "test");
        let result = lower_for_target(&op, &cfg);
        assert_eq!(result[0].arch, TargetArch::RiscV64);
    }

    #[test]
    fn unified_amdgpu_dispatch() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        let op = GenericOp::new("llvm.load", "test");
        let result = lower_for_target(&op, &cfg);
        assert_eq!(result[0].arch, TargetArch::AmdGpu);
    }

    #[test]
    fn unified_nvptx_dispatch() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let op = GenericOp::new("llvm.load", "test");
        let result = lower_for_target(&op, &cfg);
        assert_eq!(result[0].arch, TargetArch::Nvptx64);
    }

    #[test]
    fn lower_all_for_target_multi() {
        let cfg = TargetConfig::riscv64_default();
        let ops = vec![
            GenericOp::new("llvm.store", "store"),
            GenericOp::new("llvm.load", "load"),
            GenericOp::new("llvm.call", "call"),
        ];
        let result = lower_all_for_target(&ops, &cfg);
        // store=1 + load=1 + call=2 = 4
        assert_eq!(result.len(), 4);
    }

    // ── Validation ───────────────────────────────────────────────────────

    #[test]
    fn validate_arch_mismatch() {
        let cfg = TargetConfig::riscv64_default();
        let ops = vec![TargetOp::new(TargetArch::AmdGpu, "flat_store", "wrong arch")];
        let diags = validate_target_ops(&ops, &cfg);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, DiagSeverity::Error);
        assert!(diags[0].message.contains("amdgpu"));
    }

    #[test]
    fn validate_riscv_rvv_without_feature() {
        let cfg = TargetConfig::riscv64_default();
        let ops = vec![TargetOp::new(TargetArch::RiscV64, "vle32.v v0, (a1)", "rvv load")];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.iter().any(|d| d.message.contains("'v' extension")));
    }

    #[test]
    fn validate_riscv_rvv_with_feature() {
        let mut cfg = TargetConfig::riscv64_default();
        for f in &mut cfg.features {
            if f.name == "v" {
                f.enabled = true;
            }
        }
        let ops = vec![TargetOp::new(TargetArch::RiscV64, "vle32.v v0, (a1)", "rvv load")];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.is_empty());
    }

    #[test]
    fn validate_riscv32_64bit_op() {
        let mut cfg = TargetConfig::riscv64_default();
        cfg.triple = TargetTriple::riscv32_bare();
        cfg.data_layout = DataLayout::riscv32();
        let ops = vec![TargetOp::new(TargetArch::RiscV32, "ld %rd, 0(%rs1)", "64-bit load on rv32")];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.iter().any(|d| d.message.contains("64-bit")));
    }

    #[test]
    fn validate_amdgpu_no_gfx() {
        // Default config has no gfx enabled
        let mut cfg = TargetConfig::amdgpu_gfx("gfx90a");
        for f in &mut cfg.features {
            f.enabled = false; // disable all
        }
        let ops = vec![TargetOp::new(TargetArch::AmdGpu, "flat_store_dword v[0:1], v2", "store")];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.iter().any(|d| d.message.contains("no GFX target")));
    }

    #[test]
    fn validate_nvptx_async_copy_sm70() {
        let cfg = TargetConfig::nvptx_sm("sm_70");
        let ops = vec![TargetOp::new(
            TargetArch::Nvptx64,
            "cp.async.ld.global.shared [%rd0], [%rd1], 4",
            "async copy",
        )];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.iter().any(|d| d.message.contains("cp.async requires SM >= 80")));
    }

    #[test]
    fn validate_nvptx_async_copy_sm80_ok() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let ops = vec![TargetOp::new(
            TargetArch::Nvptx64,
            "cp.async.ld.global.shared [%rd0], [%rd1], 4",
            "async copy",
        )];
        let diags = validate_target_ops(&ops, &cfg);
        // No error for SM 80
        assert!(diags.iter().all(|d| d.severity != DiagSeverity::Error));
    }

    #[test]
    fn validate_nvptx_legacy_sm_warning() {
        let cfg = TargetConfig::nvptx_sm("sm_50");
        let ops = vec![TargetOp::new(TargetArch::Nvptx64, "st.global.u32 [%rd0], %r0", "store")];
        let diags = validate_target_ops(&ops, &cfg);
        assert!(diags.iter().any(|d| d.message.contains("legacy")));
    }

    // ── Parse SM Level ───────────────────────────────────────────────────

    #[test]
    fn parse_sm_levels() {
        assert_eq!(parse_sm_level("sm_80"), 80);
        assert_eq!(parse_sm_level("sm_70"), 70);
        assert_eq!(parse_sm_level("sm_100"), 100);
        assert_eq!(parse_sm_level("generic"), 50); // fallback
    }

    // ── Target Info ──────────────────────────────────────────────────────

    #[test]
    fn target_info_summary() {
        let cfg = TargetConfig::riscv64_default();
        let info = TargetInfo::from_config(&cfg);
        assert_eq!(info.arch_name, "riscv64");
        assert_eq!(info.pointer_size, 64);
        assert_eq!(info.endianness, "little");
        assert!(info.features_enabled.contains(&"m".to_string()));
    }

    // ── End-to-End: Generic → Target ─────────────────────────────────────

    #[test]
    fn e2e_riscv_pipeline() {
        let cfg = TargetConfig::riscv64_default();
        let ops = vec![
            GenericOp::new("llvm.store", "store"),
            GenericOp::new("llvm.load", "load"),
        ];
        let target_ops = lower_all_for_target(&ops, &cfg);
        let diags = validate_target_ops(&target_ops, &cfg);
        assert!(diags.is_empty());
        assert_eq!(target_ops.len(), 2);
    }

    #[test]
    fn e2e_amdgpu_pipeline() {
        let cfg = TargetConfig::amdgpu_gfx("gfx90a");
        let ops = vec![
            GenericOp::new("llvm.store", "store"),
            GenericOp::new("llvm.call", "call"),
        ];
        let target_ops = lower_all_for_target(&ops, &cfg);
        let diags = validate_target_ops(&target_ops, &cfg);
        // No errors (gfx90a + wavefront64 enabled)
        assert!(diags.iter().all(|d| d.severity != DiagSeverity::Error));
    }

    #[test]
    fn e2e_nvptx_pipeline() {
        let cfg = TargetConfig::nvptx_sm("sm_80");
        let ops = vec![
            GenericOp::new("llvm.store", "store"),
            GenericOp::new("llvm.load", "load"),
            GenericOp::new("llvm.intr.masked.load", "masked load"),
        ];
        let target_ops = lower_all_for_target(&ops, &cfg);
        let diags = validate_target_ops(&target_ops, &cfg);
        assert!(diags.iter().all(|d| d.severity != DiagSeverity::Error));
    }

    // ── GenericOp with operands ──────────────────────────────────────────

    #[test]
    fn generic_op_with_operands() {
        let op = GenericOp::new("llvm.store", "test").with_operands(&["i32", "ptr"]);
        assert_eq!(op.operand_types.len(), 2);
        assert_eq!(op.operand_types[0], "i32");
    }

    // ── TargetOp Display ─────────────────────────────────────────────────

    #[test]
    fn target_op_display() {
        let op = TargetOp::new(TargetArch::RiscV64, "sd %rs2, 0(%rs1)", "store");
        let s = format!("{op}");
        assert!(s.contains("riscv64"));
        assert!(s.contains("sd"));
        assert!(s.contains("store"));
    }
}
