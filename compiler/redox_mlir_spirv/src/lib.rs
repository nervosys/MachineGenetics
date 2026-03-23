//! # Redox MLIR SPIR-V Dialect Pipeline
//!
//! Implements the SPIR-V dialect pipeline for Vulkan and OpenCL GPU compute.
//!
//! Pipeline:
//! ```text
//! Redox Dialect → Standard MLIR → SPIR-V Dialect → SPIR-V Binary Module
//! ```
//!
//! Supports:
//! - SPIR-V execution models (GLCompute for Vulkan, Kernel for OpenCL)
//! - Memory models (GLSL450, OpenCL)
//! - Addressing models (Logical, Physical32, Physical64)
//! - Capability negotiation (Shader, Kernel, Float64, Int64, etc.)
//! - Descriptor set / binding layout for Vulkan
//! - Workgroup / invocation mapping
//! - SPIR-V module assembly and validation
//!
//! Reference: REDOX_PROPOSAL.md §5.4.1 — "Custom MLIR dialect pipelines
//! (SPIR-V for GPU, CIRCT for FPGA, StableHLO for NPU/TPU)"
//!
//! (ROADMAP Step 57)

use std::collections::BTreeMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Execution Environment
// ═══════════════════════════════════════════════════════════════════════════

/// SPIR-V execution model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionModel {
    /// Vulkan compute shaders.
    GLCompute,
    /// OpenCL kernels.
    Kernel,
}

impl fmt::Display for ExecutionModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionModel::GLCompute => write!(f, "GLCompute"),
            ExecutionModel::Kernel => write!(f, "Kernel"),
        }
    }
}

/// SPIR-V memory model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryModel {
    /// GLSL 450 memory model (Vulkan).
    GLSL450,
    /// OpenCL memory model.
    OpenCL,
}

impl fmt::Display for MemoryModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryModel::GLSL450 => write!(f, "GLSL450"),
            MemoryModel::OpenCL => write!(f, "OpenCL"),
        }
    }
}

/// SPIR-V addressing model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressingModel {
    /// Logical addressing (Vulkan — no raw pointers).
    Logical,
    /// Physical 32-bit addressing (OpenCL 32-bit).
    Physical32,
    /// Physical 64-bit addressing (OpenCL 64-bit).
    Physical64,
}

impl fmt::Display for AddressingModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddressingModel::Logical => write!(f, "Logical"),
            AddressingModel::Physical32 => write!(f, "Physical32"),
            AddressingModel::Physical64 => write!(f, "Physical64"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Capabilities
// ═══════════════════════════════════════════════════════════════════════════

/// SPIR-V capabilities that can be requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    Shader,
    Kernel,
    Float64,
    Float16,
    Int64,
    Int16,
    Int8,
    StorageBuffer16BitAccess,
    GroupNonUniform,
    GroupNonUniformArithmetic,
    GroupNonUniformBallot,
    SubgroupDispatch,
    VariablePointers,
    PhysicalStorageBufferAddresses,
    AtomicFloat32AddEXT,
    CooperativeMatrixKHR,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// A set of SPIR-V capabilities.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    caps: Vec<Capability>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        CapabilitySet { caps: Vec::new() }
    }

    pub fn add(&mut self, cap: Capability) {
        if !self.caps.contains(&cap) {
            self.caps.push(cap);
            self.caps.sort();
        }
    }

    pub fn has(&self, cap: Capability) -> bool {
        self.caps.contains(&cap)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.caps.iter()
    }

    pub fn len(&self) -> usize {
        self.caps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }

    /// Minimum capabilities for Vulkan compute.
    pub fn vulkan_compute() -> Self {
        let mut set = CapabilitySet::new();
        set.add(Capability::Shader);
        set
    }

    /// Minimum capabilities for OpenCL kernel.
    pub fn opencl_kernel() -> Self {
        let mut set = CapabilitySet::new();
        set.add(Capability::Kernel);
        set
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Target Environment
// ═══════════════════════════════════════════════════════════════════════════

/// SPIR-V target environment version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpirvVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
    V1_4,
    V1_5,
    V1_6,
}

impl SpirvVersion {
    pub fn major_minor(&self) -> (u32, u32) {
        match self {
            SpirvVersion::V1_0 => (1, 0),
            SpirvVersion::V1_1 => (1, 1),
            SpirvVersion::V1_2 => (1, 2),
            SpirvVersion::V1_3 => (1, 3),
            SpirvVersion::V1_4 => (1, 4),
            SpirvVersion::V1_5 => (1, 5),
            SpirvVersion::V1_6 => (1, 6),
        }
    }
}

impl fmt::Display for SpirvVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (maj, min) = self.major_minor();
        write!(f, "{maj}.{min}")
    }
}

/// Vulkan API version target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VulkanVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
}

impl VulkanVersion {
    /// Maximum SPIR-V version supported by this Vulkan version.
    pub fn max_spirv_version(&self) -> SpirvVersion {
        match self {
            VulkanVersion::V1_0 => SpirvVersion::V1_0,
            VulkanVersion::V1_1 => SpirvVersion::V1_3,
            VulkanVersion::V1_2 => SpirvVersion::V1_5,
            VulkanVersion::V1_3 => SpirvVersion::V1_6,
        }
    }
}

impl fmt::Display for VulkanVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VulkanVersion::V1_0 => write!(f, "Vulkan 1.0"),
            VulkanVersion::V1_1 => write!(f, "Vulkan 1.1"),
            VulkanVersion::V1_2 => write!(f, "Vulkan 1.2"),
            VulkanVersion::V1_3 => write!(f, "Vulkan 1.3"),
        }
    }
}

/// OpenCL version target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenClVersion {
    V1_2,
    V2_0,
    V3_0,
}

impl fmt::Display for OpenClVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenClVersion::V1_2 => write!(f, "OpenCL 1.2"),
            OpenClVersion::V2_0 => write!(f, "OpenCL 2.0"),
            OpenClVersion::V3_0 => write!(f, "OpenCL 3.0"),
        }
    }
}

/// Target environment for SPIR-V compilation.
#[derive(Debug, Clone)]
pub enum TargetEnv {
    Vulkan(VulkanVersion),
    OpenCL(OpenClVersion),
}

impl TargetEnv {
    pub fn execution_model(&self) -> ExecutionModel {
        match self {
            TargetEnv::Vulkan(_) => ExecutionModel::GLCompute,
            TargetEnv::OpenCL(_) => ExecutionModel::Kernel,
        }
    }

    pub fn memory_model(&self) -> MemoryModel {
        match self {
            TargetEnv::Vulkan(_) => MemoryModel::GLSL450,
            TargetEnv::OpenCL(_) => MemoryModel::OpenCL,
        }
    }

    pub fn addressing_model(&self) -> AddressingModel {
        match self {
            TargetEnv::Vulkan(_) => AddressingModel::Logical,
            TargetEnv::OpenCL(_) => AddressingModel::Physical64,
        }
    }
}

impl fmt::Display for TargetEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetEnv::Vulkan(v) => write!(f, "{v}"),
            TargetEnv::OpenCL(v) => write!(f, "{v}"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Types
// ═══════════════════════════════════════════════════════════════════════════

/// SPIR-V scalar and composite types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpirvType {
    Void,
    Bool,
    Int { width: u32, signed: bool },
    Float { width: u32 },
    Vector { element: Box<SpirvType>, count: u32 },
    Array { element: Box<SpirvType>, length: u32 },
    RuntimeArray { element: Box<SpirvType> },
    Struct { members: Vec<SpirvType> },
    Pointer { storage_class: StorageClass, pointee: Box<SpirvType> },
    Function { return_type: Box<SpirvType>, params: Vec<SpirvType> },
    Image,
    Sampler,
    SampledImage,
}

impl SpirvType {
    pub fn i32() -> Self {
        SpirvType::Int { width: 32, signed: true }
    }
    pub fn u32() -> Self {
        SpirvType::Int { width: 32, signed: false }
    }
    pub fn f32() -> Self {
        SpirvType::Float { width: 32 }
    }
    pub fn f64() -> Self {
        SpirvType::Float { width: 64 }
    }
    pub fn vec4_f32() -> Self {
        SpirvType::Vector { element: Box::new(SpirvType::f32()), count: 4 }
    }
}

/// SPIR-V storage classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageClass {
    UniformConstant,
    Input,
    Uniform,
    Output,
    Workgroup,
    CrossWorkgroup,
    Private,
    Function,
    Generic,
    PushConstant,
    AtomicCounter,
    StorageBuffer,
    PhysicalStorageBuffer,
}

impl fmt::Display for StorageClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Operations
// ═══════════════════════════════════════════════════════════════════════════

/// A SPIR-V dialect operation in the pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpirvOp {
    /// SPIR-V opcode mnemonic (e.g., "OpStore", "OpLoad", "OpFunctionCall").
    pub opcode: String,
    /// Operand descriptions.
    pub operands: Vec<String>,
    /// Comment for debugging.
    pub comment: String,
}

impl SpirvOp {
    pub fn new(opcode: &str, comment: &str) -> Self {
        SpirvOp { opcode: opcode.to_string(), operands: Vec::new(), comment: comment.to_string() }
    }

    pub fn with_operands(mut self, operands: &[&str]) -> Self {
        self.operands = operands.iter().map(|o| o.to_string()).collect();
        self
    }
}

impl fmt::Display for SpirvOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.operands.is_empty() {
            write!(f, "{}", self.opcode)
        } else {
            write!(f, "{} {}", self.opcode, self.operands.join(", "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Descriptor Binding (Vulkan)
// ═══════════════════════════════════════════════════════════════════════════

/// A Vulkan descriptor set binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorBinding {
    pub set: u32,
    pub binding: u32,
    pub name: String,
    pub storage_class: StorageClass,
    pub element_type: SpirvType,
}

impl DescriptorBinding {
    pub fn storage_buffer(set: u32, binding: u32, name: &str, element_type: SpirvType) -> Self {
        DescriptorBinding {
            set,
            binding,
            name: name.to_string(),
            storage_class: StorageClass::StorageBuffer,
            element_type,
        }
    }

    pub fn uniform_buffer(set: u32, binding: u32, name: &str, element_type: SpirvType) -> Self {
        DescriptorBinding {
            set,
            binding,
            name: name.to_string(),
            storage_class: StorageClass::Uniform,
            element_type,
        }
    }
}

/// A descriptor set layout.
#[derive(Debug, Clone)]
pub struct DescriptorSetLayout {
    pub set: u32,
    pub bindings: Vec<DescriptorBinding>,
}

impl DescriptorSetLayout {
    pub fn new(set: u32) -> Self {
        DescriptorSetLayout { set, bindings: Vec::new() }
    }

    pub fn add_binding(&mut self, binding: DescriptorBinding) {
        self.bindings.push(binding);
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Workgroup Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Workgroup dimensions for compute dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkgroupSize {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl WorkgroupSize {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        WorkgroupSize { x, y, z }
    }

    pub fn linear(size: u32) -> Self {
        WorkgroupSize { x: size, y: 1, z: 1 }
    }

    pub fn square(size: u32) -> Self {
        WorkgroupSize { x: size, y: size, z: 1 }
    }

    /// Total number of invocations in a workgroup.
    pub fn total_invocations(&self) -> u32 {
        self.x * self.y * self.z
    }
}

impl fmt::Display for WorkgroupSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Entry Point
// ═══════════════════════════════════════════════════════════════════════════

/// A SPIR-V entry point (kernel / compute shader).
#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub name: String,
    pub execution_model: ExecutionModel,
    pub workgroup_size: WorkgroupSize,
    pub interface_vars: Vec<String>,
}

impl EntryPoint {
    pub fn compute(name: &str, workgroup_size: WorkgroupSize) -> Self {
        EntryPoint {
            name: name.to_string(),
            execution_model: ExecutionModel::GLCompute,
            workgroup_size,
            interface_vars: Vec::new(),
        }
    }

    pub fn kernel(name: &str, workgroup_size: WorkgroupSize) -> Self {
        EntryPoint {
            name: name.to_string(),
            execution_model: ExecutionModel::Kernel,
            workgroup_size,
            interface_vars: Vec::new(),
        }
    }

    pub fn add_interface_var(&mut self, var: &str) {
        self.interface_vars.push(var.to_string());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SPIR-V Module
// ═══════════════════════════════════════════════════════════════════════════

/// A SPIR-V module representing a compiled compute shader / kernel.
#[derive(Debug, Clone)]
pub struct SpirvModule {
    pub target_env: TargetEnv,
    pub spirv_version: SpirvVersion,
    pub capabilities: CapabilitySet,
    pub memory_model: MemoryModel,
    pub addressing_model: AddressingModel,
    pub entry_points: Vec<EntryPoint>,
    pub descriptor_sets: Vec<DescriptorSetLayout>,
    pub ops: Vec<SpirvOp>,
}

impl SpirvModule {
    pub fn new(target_env: TargetEnv) -> Self {
        let memory_model = target_env.memory_model();
        let addressing_model = target_env.addressing_model();
        let spirv_version = match &target_env {
            TargetEnv::Vulkan(v) => v.max_spirv_version(),
            TargetEnv::OpenCL(_) => SpirvVersion::V1_2,
        };
        let capabilities = match &target_env {
            TargetEnv::Vulkan(_) => CapabilitySet::vulkan_compute(),
            TargetEnv::OpenCL(_) => CapabilitySet::opencl_kernel(),
        };
        SpirvModule {
            target_env,
            spirv_version,
            capabilities,
            memory_model,
            addressing_model,
            entry_points: Vec::new(),
            descriptor_sets: Vec::new(),
            ops: Vec::new(),
        }
    }

    pub fn add_capability(&mut self, cap: Capability) {
        self.capabilities.add(cap);
    }

    pub fn add_entry_point(&mut self, entry: EntryPoint) {
        self.entry_points.push(entry);
    }

    pub fn add_descriptor_set(&mut self, layout: DescriptorSetLayout) {
        self.descriptor_sets.push(layout);
    }

    pub fn add_op(&mut self, op: SpirvOp) {
        self.ops.push(op);
    }

    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    /// Emit a human-readable SPIR-V assembly representation.
    pub fn to_assembly(&self) -> String {
        let mut asm = String::new();
        asm.push_str(&format!("; SPIR-V {}\n", self.spirv_version));
        asm.push_str(&format!("; Target: {}\n", self.target_env));

        // Capabilities
        for cap in self.capabilities.iter() {
            asm.push_str(&format!("OpCapability {cap}\n"));
        }

        // Memory model
        asm.push_str(&format!("OpMemoryModel {} {}\n", self.addressing_model, self.memory_model));

        // Entry points
        for ep in &self.entry_points {
            asm.push_str(&format!("OpEntryPoint {} %main \"{}\"", ep.execution_model, ep.name));
            for var in &ep.interface_vars {
                asm.push_str(&format!(" %{var}"));
            }
            asm.push('\n');

            asm.push_str(&format!(
                "OpExecutionMode %main LocalSize {} {} {}\n",
                ep.workgroup_size.x, ep.workgroup_size.y, ep.workgroup_size.z
            ));
        }

        // Decorations for descriptor bindings
        for ds in &self.descriptor_sets {
            for binding in &ds.bindings {
                asm.push_str(&format!(
                    "OpDecorate %{} DescriptorSet {}\n",
                    binding.name, binding.set
                ));
                asm.push_str(&format!(
                    "OpDecorate %{} Binding {}\n",
                    binding.name, binding.binding
                ));
            }
        }

        // Operations
        for op in &self.ops {
            asm.push_str(&format!("{op}\n"));
        }

        asm
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lowering: Generic → SPIR-V
// ═══════════════════════════════════════════════════════════════════════════

/// A generic LLVM-dialect op to lower to SPIR-V.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericOp {
    pub name: String,
    pub comment: String,
}

impl GenericOp {
    pub fn new(name: &str, comment: &str) -> Self {
        GenericOp { name: name.to_string(), comment: comment.to_string() }
    }
}

/// Lower a generic LLVM-dialect op to SPIR-V ops.
pub fn lower_to_spirv(op: &GenericOp, env: &TargetEnv) -> Vec<SpirvOp> {
    match op.name.as_str() {
        "llvm.store" => {
            vec![SpirvOp::new("OpStore", &format!("store → {env}"))]
        }
        "llvm.load" => {
            vec![SpirvOp::new("OpLoad", &format!("load → {env}"))]
        }
        "llvm.call" => {
            vec![SpirvOp::new("OpFunctionCall", &format!("call → {env}"))]
        }
        "llvm.cond_br" => match env {
            TargetEnv::Vulkan(_) => {
                vec![
                    SpirvOp::new("OpSelectionMerge", "structured control flow"),
                    SpirvOp::new("OpBranchConditional", &format!("branch → {env}")),
                ]
            }
            TargetEnv::OpenCL(_) => {
                vec![SpirvOp::new("OpBranchConditional", &format!("branch → {env}"))]
            }
        },
        "llvm.intr.masked.load" | "llvm.intr.masked.store" => {
            let is_load = op.name.contains("load");
            if is_load {
                vec![SpirvOp::new("OpLoad", &format!("vector load → {env}"))]
            } else {
                vec![SpirvOp::new("OpStore", &format!("vector store → {env}"))]
            }
        }
        "llvm.mlir.constant" => {
            vec![SpirvOp::new("OpConstant", &format!("constant → {env}"))]
        }
        "gpu.launch_func" => match env {
            TargetEnv::Vulkan(_) => vec![
                SpirvOp::new("OpVariable", "kernel argument setup (Vulkan)"),
                SpirvOp::new("OpFunctionCall", "dispatch compute (Vulkan)"),
            ],
            TargetEnv::OpenCL(_) => vec![SpirvOp::new("OpFunctionCall", "enqueue kernel (OpenCL)")],
        },
        _ => {
            vec![SpirvOp::new(&format!("; unsupported: {}", op.name), "fallback")]
        }
    }
}

/// Lower a sequence of generic ops to SPIR-V.
pub fn lower_all_to_spirv(ops: &[GenericOp], env: &TargetEnv) -> Vec<SpirvOp> {
    ops.iter().flat_map(|op| lower_to_spirv(op, env)).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Pipeline Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the SPIR-V pipeline.
#[derive(Debug, Clone)]
pub struct SpirvPipelineConfig {
    pub target_env: TargetEnv,
    pub workgroup_size: WorkgroupSize,
    pub entry_point_name: String,
    /// Additional capabilities to request.
    pub extra_capabilities: Vec<Capability>,
}

impl SpirvPipelineConfig {
    pub fn vulkan(name: &str, workgroup_size: WorkgroupSize) -> Self {
        SpirvPipelineConfig {
            target_env: TargetEnv::Vulkan(VulkanVersion::V1_2),
            workgroup_size,
            entry_point_name: name.to_string(),
            extra_capabilities: Vec::new(),
        }
    }

    pub fn opencl(name: &str, workgroup_size: WorkgroupSize) -> Self {
        SpirvPipelineConfig {
            target_env: TargetEnv::OpenCL(OpenClVersion::V2_0),
            workgroup_size,
            entry_point_name: name.to_string(),
            extra_capabilities: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Full Pipeline
// ═══════════════════════════════════════════════════════════════════════════

/// Result of running the SPIR-V compilation pipeline.
#[derive(Debug, Clone)]
pub struct SpirvPipelineResult {
    pub module: SpirvModule,
    pub generic_ops: Vec<GenericOp>,
    pub spirv_ops: Vec<SpirvOp>,
    pub validation_diags: Vec<ValidationDiag>,
}

/// Run the full SPIR-V pipeline: Generic ops → SPIR-V module.
pub fn run_spirv_pipeline(
    generic_ops: Vec<GenericOp>,
    config: &SpirvPipelineConfig,
) -> SpirvPipelineResult {
    let mut module = SpirvModule::new(config.target_env.clone());

    // Add extra capabilities
    for cap in &config.extra_capabilities {
        module.add_capability(*cap);
    }

    // Create entry point
    let entry = match config.target_env {
        TargetEnv::Vulkan(_) => {
            EntryPoint::compute(&config.entry_point_name, config.workgroup_size)
        }
        TargetEnv::OpenCL(_) => EntryPoint::kernel(&config.entry_point_name, config.workgroup_size),
    };
    module.add_entry_point(entry);

    // Lower ops
    let spirv_ops = lower_all_to_spirv(&generic_ops, &config.target_env);
    for op in &spirv_ops {
        module.add_op(op.clone());
    }

    // Validate
    let validation_diags = validate_module(&module);

    SpirvPipelineResult { module, generic_ops, spirv_ops, validation_diags }
}

// ═══════════════════════════════════════════════════════════════════════════
// Validation
// ═══════════════════════════════════════════════════════════════════════════

/// Validation diagnostic for SPIR-V modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationDiag {
    pub severity: DiagSeverity,
    pub message: String,
}

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
        write!(f, "{sev}: {}", self.message)
    }
}

/// Validate a SPIR-V module for correctness.
pub fn validate_module(module: &SpirvModule) -> Vec<ValidationDiag> {
    let mut diags = Vec::new();

    // Must have at least one entry point
    if module.entry_points.is_empty() {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: "module has no entry points".to_string(),
        });
    }

    // Validate entry point execution models match target
    let expected_model = module.target_env.execution_model();
    for ep in &module.entry_points {
        if ep.execution_model != expected_model {
            diags.push(ValidationDiag {
                severity: DiagSeverity::Error,
                message: format!(
                    "entry point '{}' uses {} but target requires {}",
                    ep.name, ep.execution_model, expected_model
                ),
            });
        }

        // Validate workgroup size
        if ep.workgroup_size.total_invocations() == 0 {
            diags.push(ValidationDiag {
                severity: DiagSeverity::Error,
                message: format!("entry point '{}' has zero workgroup invocations", ep.name),
            });
        }

        // Vulkan max workgroup size check (common limit: 1024 invocations)
        if matches!(module.target_env, TargetEnv::Vulkan(_))
            && ep.workgroup_size.total_invocations() > 1024
        {
            diags.push(ValidationDiag {
                severity: DiagSeverity::Warning,
                message: format!(
                    "entry point '{}': workgroup size {} exceeds common Vulkan limit (1024)",
                    ep.name,
                    ep.workgroup_size.total_invocations()
                ),
            });
        }
    }

    // Validate capabilities for target
    match &module.target_env {
        TargetEnv::Vulkan(_) => {
            if !module.capabilities.has(Capability::Shader) {
                diags.push(ValidationDiag {
                    severity: DiagSeverity::Error,
                    message: "Vulkan target requires Shader capability".to_string(),
                });
            }
        }
        TargetEnv::OpenCL(_) => {
            if !module.capabilities.has(Capability::Kernel) {
                diags.push(ValidationDiag {
                    severity: DiagSeverity::Error,
                    message: "OpenCL target requires Kernel capability".to_string(),
                });
            }
        }
    }

    // Memory model validation
    let expected_mm = module.target_env.memory_model();
    if module.memory_model != expected_mm {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: format!(
                "memory model {} doesn't match target expectation {}",
                module.memory_model, expected_mm
            ),
        });
    }

    // Addressing model validation
    let expected_am = module.target_env.addressing_model();
    if module.addressing_model != expected_am {
        diags.push(ValidationDiag {
            severity: DiagSeverity::Error,
            message: format!(
                "addressing model {} doesn't match target expectation {}",
                module.addressing_model, expected_am
            ),
        });
    }

    // Validate descriptor set bindings don't conflict
    let mut binding_map: BTreeMap<(u32, u32), String> = BTreeMap::new();
    for ds in &module.descriptor_sets {
        for binding in &ds.bindings {
            let key = (binding.set, binding.binding);
            if let Some(existing) = binding_map.get(&key) {
                diags.push(ValidationDiag {
                    severity: DiagSeverity::Error,
                    message: format!(
                        "duplicate descriptor binding ({}, {}): '{}' conflicts with '{}'",
                        key.0, key.1, binding.name, existing
                    ),
                });
            } else {
                binding_map.insert(key, binding.name.clone());
            }
        }
    }

    diags
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Execution Model ──────────────────────────────────────────────────

    #[test]
    fn execution_model_display() {
        assert_eq!(ExecutionModel::GLCompute.to_string(), "GLCompute");
        assert_eq!(ExecutionModel::Kernel.to_string(), "Kernel");
    }

    // ── Memory Model ─────────────────────────────────────────────────────

    #[test]
    fn memory_model_display() {
        assert_eq!(MemoryModel::GLSL450.to_string(), "GLSL450");
        assert_eq!(MemoryModel::OpenCL.to_string(), "OpenCL");
    }

    // ── Capabilities ─────────────────────────────────────────────────────

    #[test]
    fn capability_set_add_dedup() {
        let mut caps = CapabilitySet::new();
        caps.add(Capability::Shader);
        caps.add(Capability::Shader); // duplicate
        caps.add(Capability::Float64);
        assert_eq!(caps.len(), 2);
        assert!(caps.has(Capability::Shader));
        assert!(caps.has(Capability::Float64));
        assert!(!caps.has(Capability::Kernel));
    }

    #[test]
    fn vulkan_compute_caps() {
        let caps = CapabilitySet::vulkan_compute();
        assert!(caps.has(Capability::Shader));
        assert!(!caps.has(Capability::Kernel));
    }

    #[test]
    fn opencl_kernel_caps() {
        let caps = CapabilitySet::opencl_kernel();
        assert!(caps.has(Capability::Kernel));
        assert!(!caps.has(Capability::Shader));
    }

    // ── SPIR-V Version ───────────────────────────────────────────────────

    #[test]
    fn spirv_version_display() {
        assert_eq!(SpirvVersion::V1_5.to_string(), "1.5");
        assert_eq!(SpirvVersion::V1_0.major_minor(), (1, 0));
    }

    #[test]
    fn vulkan_spirv_version_mapping() {
        assert_eq!(VulkanVersion::V1_0.max_spirv_version(), SpirvVersion::V1_0);
        assert_eq!(VulkanVersion::V1_2.max_spirv_version(), SpirvVersion::V1_5);
        assert_eq!(VulkanVersion::V1_3.max_spirv_version(), SpirvVersion::V1_6);
    }

    // ── Target Environment ───────────────────────────────────────────────

    #[test]
    fn vulkan_target_env() {
        let env = TargetEnv::Vulkan(VulkanVersion::V1_2);
        assert_eq!(env.execution_model(), ExecutionModel::GLCompute);
        assert_eq!(env.memory_model(), MemoryModel::GLSL450);
        assert_eq!(env.addressing_model(), AddressingModel::Logical);
    }

    #[test]
    fn opencl_target_env() {
        let env = TargetEnv::OpenCL(OpenClVersion::V2_0);
        assert_eq!(env.execution_model(), ExecutionModel::Kernel);
        assert_eq!(env.memory_model(), MemoryModel::OpenCL);
        assert_eq!(env.addressing_model(), AddressingModel::Physical64);
    }

    // ── SPIR-V Types ─────────────────────────────────────────────────────

    #[test]
    fn spirv_type_constructors() {
        assert_eq!(SpirvType::i32(), SpirvType::Int { width: 32, signed: true });
        assert_eq!(SpirvType::u32(), SpirvType::Int { width: 32, signed: false });
        assert_eq!(SpirvType::f32(), SpirvType::Float { width: 32 });
    }

    #[test]
    fn spirv_type_vec4() {
        let v4 = SpirvType::vec4_f32();
        match v4 {
            SpirvType::Vector { element, count } => {
                assert_eq!(*element, SpirvType::f32());
                assert_eq!(count, 4);
            }
            _ => panic!("expected Vector"),
        }
    }

    // ── Descriptor Binding ───────────────────────────────────────────────

    #[test]
    fn descriptor_binding_storage() {
        let b = DescriptorBinding::storage_buffer(0, 0, "input", SpirvType::f32());
        assert_eq!(b.set, 0);
        assert_eq!(b.binding, 0);
        assert_eq!(b.storage_class, StorageClass::StorageBuffer);
    }

    #[test]
    fn descriptor_set_layout() {
        let mut layout = DescriptorSetLayout::new(0);
        layout.add_binding(DescriptorBinding::storage_buffer(0, 0, "in", SpirvType::f32()));
        layout.add_binding(DescriptorBinding::storage_buffer(0, 1, "out", SpirvType::f32()));
        assert_eq!(layout.binding_count(), 2);
    }

    // ── Workgroup Size ───────────────────────────────────────────────────

    #[test]
    fn workgroup_size_linear() {
        let wg = WorkgroupSize::linear(256);
        assert_eq!(wg.total_invocations(), 256);
        assert_eq!(wg.x, 256);
        assert_eq!(wg.y, 1);
        assert_eq!(wg.z, 1);
    }

    #[test]
    fn workgroup_size_square() {
        let wg = WorkgroupSize::square(16);
        assert_eq!(wg.total_invocations(), 256);
    }

    #[test]
    fn workgroup_size_3d() {
        let wg = WorkgroupSize::new(8, 8, 4);
        assert_eq!(wg.total_invocations(), 256);
        assert_eq!(wg.to_string(), "(8, 8, 4)");
    }

    // ── Entry Point ──────────────────────────────────────────────────────

    #[test]
    fn entry_point_compute() {
        let mut ep = EntryPoint::compute("main_cs", WorkgroupSize::linear(64));
        assert_eq!(ep.execution_model, ExecutionModel::GLCompute);
        ep.add_interface_var("gl_GlobalInvocationID");
        assert_eq!(ep.interface_vars.len(), 1);
    }

    #[test]
    fn entry_point_kernel() {
        let ep = EntryPoint::kernel("vector_add", WorkgroupSize::linear(256));
        assert_eq!(ep.execution_model, ExecutionModel::Kernel);
    }

    // ── SPIR-V Module ────────────────────────────────────────────────────

    #[test]
    fn module_vulkan_defaults() {
        let m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        assert_eq!(m.memory_model, MemoryModel::GLSL450);
        assert_eq!(m.addressing_model, AddressingModel::Logical);
        assert_eq!(m.spirv_version, SpirvVersion::V1_5);
        assert!(m.capabilities.has(Capability::Shader));
    }

    #[test]
    fn module_opencl_defaults() {
        let m = SpirvModule::new(TargetEnv::OpenCL(OpenClVersion::V2_0));
        assert_eq!(m.memory_model, MemoryModel::OpenCL);
        assert_eq!(m.addressing_model, AddressingModel::Physical64);
        assert!(m.capabilities.has(Capability::Kernel));
    }

    #[test]
    fn module_add_ops() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_op(SpirvOp::new("OpStore", "test"));
        m.add_op(SpirvOp::new("OpLoad", "test"));
        assert_eq!(m.op_count(), 2);
    }

    #[test]
    fn module_assembly() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::linear(64)));
        m.add_op(SpirvOp::new("OpStore", "test"));
        let asm = m.to_assembly();
        assert!(asm.contains("SPIR-V 1.5"));
        assert!(asm.contains("OpCapability Shader"));
        assert!(asm.contains("OpMemoryModel Logical GLSL450"));
        assert!(asm.contains("OpEntryPoint GLCompute"));
        assert!(asm.contains("LocalSize 64 1 1"));
        assert!(asm.contains("OpStore"));
    }

    #[test]
    fn module_assembly_with_descriptors() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::linear(64)));
        let mut ds = DescriptorSetLayout::new(0);
        ds.add_binding(DescriptorBinding::storage_buffer(0, 0, "input_buf", SpirvType::f32()));
        ds.add_binding(DescriptorBinding::storage_buffer(0, 1, "output_buf", SpirvType::f32()));
        m.add_descriptor_set(ds);
        let asm = m.to_assembly();
        assert!(asm.contains("OpDecorate %input_buf DescriptorSet 0"));
        assert!(asm.contains("OpDecorate %input_buf Binding 0"));
        assert!(asm.contains("OpDecorate %output_buf Binding 1"));
    }

    // ── Lowering ─────────────────────────────────────────────────────────

    #[test]
    fn lower_store_to_spirv() {
        let env = TargetEnv::Vulkan(VulkanVersion::V1_2);
        let op = GenericOp::new("llvm.store", "test");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].opcode, "OpStore");
    }

    #[test]
    fn lower_load_to_spirv() {
        let env = TargetEnv::OpenCL(OpenClVersion::V2_0);
        let op = GenericOp::new("llvm.load", "test");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result[0].opcode, "OpLoad");
    }

    #[test]
    fn lower_branch_vulkan() {
        let env = TargetEnv::Vulkan(VulkanVersion::V1_2);
        let op = GenericOp::new("llvm.cond_br", "test");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result.len(), 2); // SelectionMerge + BranchConditional
        assert_eq!(result[0].opcode, "OpSelectionMerge");
    }

    #[test]
    fn lower_branch_opencl() {
        let env = TargetEnv::OpenCL(OpenClVersion::V2_0);
        let op = GenericOp::new("llvm.cond_br", "test");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result.len(), 1); // Just BranchConditional
    }

    #[test]
    fn lower_gpu_launch_vulkan() {
        let env = TargetEnv::Vulkan(VulkanVersion::V1_2);
        let op = GenericOp::new("gpu.launch_func", "dispatch");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn lower_gpu_launch_opencl() {
        let env = TargetEnv::OpenCL(OpenClVersion::V2_0);
        let op = GenericOp::new("gpu.launch_func", "dispatch");
        let result = lower_to_spirv(&op, &env);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn lower_all_sequence() {
        let env = TargetEnv::Vulkan(VulkanVersion::V1_2);
        let ops = vec![
            GenericOp::new("llvm.load", "load"),
            GenericOp::new("llvm.store", "store"),
            GenericOp::new("llvm.mlir.constant", "const"),
        ];
        let result = lower_all_to_spirv(&ops, &env);
        assert_eq!(result.len(), 3);
    }

    // ── Validation ───────────────────────────────────────────────────────

    #[test]
    fn validate_valid_vulkan_module() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::linear(64)));
        let diags = validate_module(&m);
        assert!(diags.is_empty());
    }

    #[test]
    fn validate_valid_opencl_module() {
        let mut m = SpirvModule::new(TargetEnv::OpenCL(OpenClVersion::V2_0));
        m.add_entry_point(EntryPoint::kernel("vector_add", WorkgroupSize::linear(256)));
        let diags = validate_module(&m);
        assert!(diags.is_empty());
    }

    #[test]
    fn validate_no_entry_point() {
        let m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        let diags = validate_module(&m);
        assert!(diags.iter().any(|d| d.message.contains("no entry points")));
    }

    #[test]
    fn validate_wrong_execution_model() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        // Add a Kernel entry point to a Vulkan target
        m.add_entry_point(EntryPoint::kernel("wrong", WorkgroupSize::linear(64)));
        let diags = validate_module(&m);
        assert!(
            diags.iter().any(|d| d.message.contains("Kernel") && d.message.contains("GLCompute"))
        );
    }

    #[test]
    fn validate_zero_workgroup() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::new(0, 0, 0)));
        let diags = validate_module(&m);
        assert!(diags.iter().any(|d| d.message.contains("zero workgroup")));
    }

    #[test]
    fn validate_large_workgroup_vulkan() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::linear(2048)));
        let diags = validate_module(&m);
        assert!(diags.iter().any(|d| d.message.contains("exceeds common Vulkan limit")));
    }

    #[test]
    fn validate_large_workgroup_opencl_no_warning() {
        let mut m = SpirvModule::new(TargetEnv::OpenCL(OpenClVersion::V2_0));
        m.add_entry_point(EntryPoint::kernel("main", WorkgroupSize::linear(2048)));
        let diags = validate_module(&m);
        // OpenCL doesn't have the 1024 limit warning
        assert!(diags.iter().all(|d| !d.message.contains("Vulkan limit")));
    }

    #[test]
    fn validate_duplicate_descriptor_binding() {
        let mut m = SpirvModule::new(TargetEnv::Vulkan(VulkanVersion::V1_2));
        m.add_entry_point(EntryPoint::compute("main", WorkgroupSize::linear(64)));
        let mut ds = DescriptorSetLayout::new(0);
        ds.add_binding(DescriptorBinding::storage_buffer(0, 0, "buf_a", SpirvType::f32()));
        ds.add_binding(DescriptorBinding::storage_buffer(0, 0, "buf_b", SpirvType::f32())); // conflict!
        m.add_descriptor_set(ds);
        let diags = validate_module(&m);
        assert!(diags.iter().any(|d| d.message.contains("duplicate descriptor binding")));
    }

    // ── Full Pipeline ────────────────────────────────────────────────────

    #[test]
    fn full_pipeline_vulkan() {
        let config = SpirvPipelineConfig::vulkan("main_cs", WorkgroupSize::linear(64));
        let ops = vec![GenericOp::new("llvm.load", "load"), GenericOp::new("llvm.store", "store")];
        let result = run_spirv_pipeline(ops, &config);
        assert!(result.validation_diags.is_empty());
        assert_eq!(result.spirv_ops.len(), 2);
        assert!(result.module.capabilities.has(Capability::Shader));
    }

    #[test]
    fn full_pipeline_opencl() {
        let config = SpirvPipelineConfig::opencl("vector_add", WorkgroupSize::linear(256));
        let ops = vec![
            GenericOp::new("llvm.load", "load data"),
            GenericOp::new("llvm.mlir.constant", "constant"),
            GenericOp::new("llvm.store", "store result"),
        ];
        let result = run_spirv_pipeline(ops, &config);
        assert!(result.validation_diags.is_empty());
        assert_eq!(result.spirv_ops.len(), 3);
        assert!(result.module.capabilities.has(Capability::Kernel));
    }

    #[test]
    fn full_pipeline_with_extra_caps() {
        let mut config = SpirvPipelineConfig::vulkan("double_compute", WorkgroupSize::linear(128));
        config.extra_capabilities.push(Capability::Float64);
        let ops = vec![GenericOp::new("llvm.load", "load f64")];
        let result = run_spirv_pipeline(ops, &config);
        assert!(result.module.capabilities.has(Capability::Float64));
    }

    // ── SpirvOp Display ──────────────────────────────────────────────────

    #[test]
    fn spirv_op_display_no_operands() {
        let op = SpirvOp::new("OpStore", "test");
        assert_eq!(format!("{op}"), "OpStore");
    }

    #[test]
    fn spirv_op_display_with_operands() {
        let op = SpirvOp::new("OpStore", "test").with_operands(&["%var", "%val"]);
        assert_eq!(format!("{op}"), "OpStore %var, %val");
    }

    // ── Storage Class Display ────────────────────────────────────────────

    #[test]
    fn storage_class_display() {
        assert_eq!(StorageClass::StorageBuffer.to_string(), "StorageBuffer");
        assert_eq!(StorageClass::Workgroup.to_string(), "Workgroup");
    }
}
