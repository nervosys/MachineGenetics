//! Code Emitters for Multiple Target Languages
//!
//! Translates the RMI IR (Intermediate Representation) into executable code
//! for various target platforms. Each emitter implements the [`CodeEmitter`]
//! trait and produces syntactically-valid output for its target language.
//!
//! # Supported Targets
//!
//! | Target | Emitter | Use Case |
//! |--------|---------|----------|
//! | CUDA | [`CudaEmitter`] | GPU kernel generation |
//! | MLIR | [`MlirEmitter`] | Compiler IR interop |
//! | ONNX | [`OnnxEmitter`] | Model interchange |

use crate::core::codegen::{
    ActivationKind, BinaryOpKind, CodeEmitter, EmitTarget, Function, IRNode, IROperation, IRType,
    IRValue, NormalizeKind, Padding, PrimitiveType, Program, ReduceOpKind, UnaryOpKind,
};
use crate::error::Result;

// ============================================================================
// CUDA/PTX Emitter — GPU kernel generation
// ============================================================================

/// Emits CUDA C++ kernel code for GPU execution.
///
/// Generated code uses CUDA runtime API and assumes standard NVIDIA GPU
/// compilation pipeline (nvcc). Kernels are emitted with appropriate
/// thread/block configuration.
pub struct CudaEmitter {
    /// Target compute capability (e.g., 80 for sm_80)
    compute_capability: u32,
    /// Block size for 1D kernels
    block_size: u32,
}

impl CudaEmitter {
    /// Create a new CUDA emitter.
    pub fn new() -> Self {
        Self {
            compute_capability: 80,
            block_size: 256,
        }
    }

    /// Set target compute capability (e.g., 70, 80, 90).
    pub fn compute_capability(mut self, cc: u32) -> Self {
        self.compute_capability = cc;
        self
    }

    /// Set default block size for kernel launches.
    pub fn block_size(mut self, bs: u32) -> Self {
        self.block_size = bs;
        self
    }

    fn emit_function(&self, func: &Function) -> Result<String> {
        let mut out = String::new();

        // Determine if this should be a __global__ kernel or __device__ function
        let is_kernel = func.attrs.get("kernel").map_or(true, |v| v == "true");
        let qualifier = if is_kernel {
            "__global__"
        } else {
            "__device__"
        };

        // Function signature
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(name, ty)| format!("{} {}", self.emit_type(ty), name))
            .collect();

        // Add size parameter for kernels
        let mut all_params = params;
        if is_kernel {
            all_params.push("int n".to_string());
        }

        out.push_str(&format!(
            "{} void {}({}) {{\n",
            qualifier,
            func.name,
            all_params.join(", "),
        ));

        // Thread index computation for kernels
        if is_kernel {
            out.push_str("    int idx = blockIdx.x * blockDim.x + threadIdx.x;\n");
            out.push_str("    if (idx >= n) return;\n\n");
        }

        // Emit nodes
        for node in &func.nodes {
            let line = self.emit_node(node)?;
            if !line.is_empty() {
                out.push_str(&format!("    {}\n", line));
            }
        }

        out.push_str("}\n");

        // Emit launch wrapper for kernels
        if is_kernel {
            out.push('\n');
            out.push_str(&self.emit_launch_wrapper(func)?);
        }

        Ok(out)
    }

    fn emit_launch_wrapper(&self, func: &Function) -> Result<String> {
        let mut out = String::new();

        let params: Vec<String> = func
            .params
            .iter()
            .map(|(name, ty)| format!("{} {}", self.emit_type(ty), name))
            .collect();

        let mut all_params = params.clone();
        all_params.push("int n".to_string());

        let call_args: Vec<String> = func.params.iter().map(|(name, _)| name.clone()).collect();
        let mut call_args_full = call_args;
        call_args_full.push("n".to_string());

        out.push_str(&format!(
            "void launch_{}({}) {{\n",
            func.name,
            all_params.join(", "),
        ));
        out.push_str(&format!(
            "    int grid = (n + {} - 1) / {};\n",
            self.block_size, self.block_size
        ));
        out.push_str(&format!(
            "    {}<<<grid, {}>>>({});\n",
            func.name,
            self.block_size,
            call_args_full.join(", ")
        ));
        out.push_str("    cudaDeviceSynchronize();\n");
        out.push_str("}\n");

        Ok(out)
    }

    fn emit_type(&self, ty: &IRType) -> String {
        match ty {
            IRType::Primitive(PrimitiveType::F16) => "half".to_string(),
            IRType::Primitive(PrimitiveType::F32) => "float".to_string(),
            IRType::Primitive(PrimitiveType::F64) => "double".to_string(),
            IRType::Primitive(PrimitiveType::BF16) => "__nv_bfloat16".to_string(),
            IRType::Primitive(PrimitiveType::I8) => "int8_t".to_string(),
            IRType::Primitive(PrimitiveType::I16) => "int16_t".to_string(),
            IRType::Primitive(PrimitiveType::I32) => "int".to_string(),
            IRType::Primitive(PrimitiveType::I64) => "int64_t".to_string(),
            IRType::Primitive(PrimitiveType::U8) => "uint8_t".to_string(),
            IRType::Primitive(PrimitiveType::U16) => "uint16_t".to_string(),
            IRType::Primitive(PrimitiveType::U32) => "uint32_t".to_string(),
            IRType::Primitive(PrimitiveType::U64) => "uint64_t".to_string(),
            IRType::Primitive(PrimitiveType::Bool) => "bool".to_string(),
            IRType::Primitive(PrimitiveType::Void) => "void".to_string(),
            IRType::Tensor { dtype, .. } => {
                format!("{}*", self.emit_type(&IRType::Primitive(dtype.clone())))
            }
            _ => "void*".to_string(),
        }
    }

    fn emit_node(&self, node: &IRNode) -> Result<String> {
        let var = format!("v{}", node.id);
        let inputs: Vec<String> = node.inputs.iter().map(|i| format!("v{}", i)).collect();
        let ctype = self.emit_type(&node.output_type);

        let expr = match &node.op {
            IROperation::Parameter { name, .. } => {
                return Ok(format!("{} {} = {}[idx];", ctype, var, name))
            }
            IROperation::Constant => {
                if let Some(IRValue::F64(v)) = node.attrs.get("value") {
                    format!("{:.6}f", v)
                } else {
                    "0.0f".to_string()
                }
            }
            IROperation::BinaryOp { op } => match op {
                BinaryOpKind::Add => format!("{} + {}", inputs[0], inputs[1]),
                BinaryOpKind::Sub => format!("{} - {}", inputs[0], inputs[1]),
                BinaryOpKind::Mul => format!("{} * {}", inputs[0], inputs[1]),
                BinaryOpKind::Div => format!("{} / {}", inputs[0], inputs[1]),
                BinaryOpKind::Pow => format!("powf({}, {})", inputs[0], inputs[1]),
                BinaryOpKind::Min => format!("fminf({}, {})", inputs[0], inputs[1]),
                BinaryOpKind::Max => format!("fmaxf({}, {})", inputs[0], inputs[1]),
                _ => format!("/* {:?}({}, {}) */", op, inputs[0], inputs[1]),
            },
            IROperation::UnaryOp { op } => match op {
                UnaryOpKind::Neg => format!("-{}", inputs[0]),
                UnaryOpKind::Abs => format!("fabsf({})", inputs[0]),
                UnaryOpKind::Sqrt => format!("sqrtf({})", inputs[0]),
                UnaryOpKind::Exp => format!("expf({})", inputs[0]),
                UnaryOpKind::Log => format!("logf({})", inputs[0]),
                UnaryOpKind::Sin => format!("sinf({})", inputs[0]),
                UnaryOpKind::Cos => format!("cosf({})", inputs[0]),
                UnaryOpKind::Tanh => format!("tanhf({})", inputs[0]),
                UnaryOpKind::Ceil => format!("ceilf({})", inputs[0]),
                UnaryOpKind::Floor => format!("floorf({})", inputs[0]),
                UnaryOpKind::Round => format!("roundf({})", inputs[0]),
                _ => format!("/* {:?}({}) */", op, inputs[0]),
            },
            IROperation::Activation { kind } => match kind {
                ActivationKind::ReLU => format!("fmaxf(0.0f, {})", inputs[0]),
                ActivationKind::LeakyReLU => {
                    format!("({0} > 0.0f ? {0} : 0.01f * {0})", inputs[0])
                }
                ActivationKind::Sigmoid => format!("1.0f / (1.0f + expf(-{}))", inputs[0]),
                ActivationKind::Tanh => format!("tanhf({})", inputs[0]),
                ActivationKind::GeLU => {
                    format!(
                            "0.5f * {0} * (1.0f + tanhf(0.7978845608f * ({0} + 0.044715f * {0} * {0} * {0})))",
                            inputs[0]
                        )
                }
                ActivationKind::SiLU => {
                    format!("{0} / (1.0f + expf(-{0}))", inputs[0])
                }
                ActivationKind::Softplus => format!("logf(1.0f + expf({}))", inputs[0]),
                _ => format!("/* {:?}({}) */", kind, inputs[0]),
            },
            IROperation::Return => {
                // In CUDA kernels, write result back to output array
                return Ok(format!(
                    "output[idx] = {};",
                    inputs.first().unwrap_or(&"0.0f".to_string())
                ));
            }
            _ => format!("/* {:?} */", node.op),
        };

        Ok(format!("{} {} = {};", ctype, var, expr))
    }
}

impl Default for CudaEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeEmitter for CudaEmitter {
    fn emit(&self, program: &Program) -> Result<String> {
        let mut output = String::new();
        output.push_str("// Auto-generated by RMI CodeGen\n");
        output.push_str(&format!("// Program: {}\n", program.name));
        output.push_str(&format!(
            "// Target: CUDA sm_{}\n\n",
            self.compute_capability
        ));
        output.push_str("#include <cuda_runtime.h>\n");
        output.push_str("#include <cstdint>\n");
        output.push_str("#include <cmath>\n\n");

        for func in &program.functions {
            output.push_str(&self.emit_function(func)?);
            output.push('\n');
        }

        Ok(output)
    }

    fn target(&self) -> EmitTarget {
        EmitTarget::CUDA
    }
}

// ============================================================================
// MLIR Emitter — Compiler IR dialect generation
// ============================================================================

/// Emits MLIR (Multi-Level IR) code in a custom RMI dialect.
///
/// Targets the standard MLIR dialects (`func`, `arith`, `tensor`, `linalg`)
/// for maximum interoperability with compiler infrastructure like LLVM,
/// XLA, and IREE.
pub struct MlirEmitter {
    /// Target dialect prefix (default: "rmi")
    dialect: String,
}

impl MlirEmitter {
    /// Create a new MLIR emitter.
    pub fn new() -> Self {
        Self {
            dialect: "rmi".to_string(),
        }
    }

    /// Set the target dialect name.
    pub fn dialect(mut self, d: impl Into<String>) -> Self {
        self.dialect = d.into();
        self
    }

    fn emit_function(&self, func: &Function) -> Result<String> {
        let mut out = String::new();

        // Function signature
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(name, ty)| format!("%{}: {}", name, self.emit_type(ty)))
            .collect();

        let ret_type = self.emit_type(&func.return_type);

        out.push_str(&format!(
            "func.func @{}({}) -> {} {{\n",
            func.name,
            params.join(", "),
            ret_type,
        ));

        // Emit nodes as SSA values
        for node in &func.nodes {
            let line = self.emit_node(node)?;
            if !line.is_empty() {
                out.push_str(&format!("  {}\n", line));
            }
        }

        out.push_str("}\n");
        Ok(out)
    }

    fn emit_type(&self, ty: &IRType) -> String {
        match ty {
            IRType::Primitive(PrimitiveType::F16) => "f16".to_string(),
            IRType::Primitive(PrimitiveType::F32) => "f32".to_string(),
            IRType::Primitive(PrimitiveType::F64) => "f64".to_string(),
            IRType::Primitive(PrimitiveType::BF16) => "bf16".to_string(),
            IRType::Primitive(PrimitiveType::I8) => "i8".to_string(),
            IRType::Primitive(PrimitiveType::I16) => "i16".to_string(),
            IRType::Primitive(PrimitiveType::I32) => "i32".to_string(),
            IRType::Primitive(PrimitiveType::I64) => "i64".to_string(),
            IRType::Primitive(PrimitiveType::U8) => "ui8".to_string(),
            IRType::Primitive(PrimitiveType::U16) => "ui16".to_string(),
            IRType::Primitive(PrimitiveType::U32) => "ui32".to_string(),
            IRType::Primitive(PrimitiveType::U64) => "ui64".to_string(),
            IRType::Primitive(PrimitiveType::Bool) => "i1".to_string(),
            IRType::Primitive(PrimitiveType::Void) => "()".to_string(),
            IRType::Tensor { dtype, shape } => {
                let dims: Vec<String> = shape
                    .iter()
                    .map(|d| match d {
                        crate::core::codegen::Dimension::Static(n) => n.to_string(),
                        crate::core::codegen::Dimension::Dynamic => "?".to_string(),
                        crate::core::codegen::Dimension::Symbolic(s) => format!("?/*{}*/", s),
                    })
                    .collect();
                let elem = self.emit_type(&IRType::Primitive(dtype.clone()));
                format!("tensor<{}x{}>", dims.join("x"), elem)
            }
            IRType::Function { inputs, output } => {
                let ins: Vec<String> = inputs.iter().map(|t| self.emit_type(t)).collect();
                format!("({}) -> {}", ins.join(", "), self.emit_type(output))
            }
            IRType::Tuple(types) => {
                let ts: Vec<String> = types.iter().map(|t| self.emit_type(t)).collect();
                format!("tuple<{}>", ts.join(", "))
            }
            _ => "!rmi.unknown".to_string(),
        }
    }

    fn emit_node(&self, node: &IRNode) -> Result<String> {
        let var = format!("%v{}", node.id);
        let inputs: Vec<String> = node.inputs.iter().map(|i| format!("%v{}", i)).collect();
        let ty = self.emit_type(&node.output_type);

        let line = match &node.op {
            IROperation::Parameter { name, .. } => {
                format!("{} = \"rmi.identity\"(%{}) : ({}) -> {}", var, name, ty, ty)
            }
            IROperation::Constant => {
                if let Some(IRValue::F64(v)) = node.attrs.get("value") {
                    format!("{} = arith.constant {:.6} : {}", var, v, ty)
                } else if let Some(IRValue::I64(v)) = node.attrs.get("value") {
                    format!("{} = arith.constant {} : {}", var, v, ty)
                } else {
                    format!("{} = arith.constant 0.0 : {}", var, ty)
                }
            }
            IROperation::BinaryOp { op } => {
                let mlir_op = match op {
                    BinaryOpKind::Add => "arith.addf",
                    BinaryOpKind::Sub => "arith.subf",
                    BinaryOpKind::Mul => "arith.mulf",
                    BinaryOpKind::Div => "arith.divf",
                    BinaryOpKind::And => "arith.andi",
                    BinaryOpKind::Or => "arith.ori",
                    _ => "rmi.binop",
                };
                format!(
                    "{} = {}({}, {}) : {}",
                    var, mlir_op, inputs[0], inputs[1], ty
                )
            }
            IROperation::UnaryOp { op } => {
                let mlir_op = match op {
                    UnaryOpKind::Neg => "arith.negf",
                    UnaryOpKind::Sqrt => "math.sqrt",
                    UnaryOpKind::Exp => "math.exp",
                    UnaryOpKind::Log => "math.log",
                    UnaryOpKind::Sin => "math.sin",
                    UnaryOpKind::Cos => "math.cos",
                    UnaryOpKind::Tanh => "math.tanh",
                    UnaryOpKind::Abs => "math.absf",
                    UnaryOpKind::Ceil => "math.ceil",
                    UnaryOpKind::Floor => "math.floor",
                    _ => "rmi.unaryop",
                };
                format!("{} = {}({}) : {}", var, mlir_op, inputs[0], ty)
            }
            IROperation::MatMul { .. } => {
                format!(
                    "{} = linalg.matmul ins({}, {} : {}, {}) outs(%init : {})",
                    var, inputs[0], inputs[1], ty, ty, ty
                )
            }
            IROperation::Activation { kind } => {
                let op_name = match kind {
                    ActivationKind::ReLU => "rmi.relu",
                    ActivationKind::GeLU => "rmi.gelu",
                    ActivationKind::SiLU => "rmi.silu",
                    ActivationKind::Sigmoid => "rmi.sigmoid",
                    ActivationKind::Tanh => "rmi.tanh",
                    ActivationKind::Softmax => "rmi.softmax",
                    ActivationKind::LeakyReLU => "rmi.leaky_relu",
                    ActivationKind::Softplus => "rmi.softplus",
                };
                format!("{} = {}({}) : ({}) -> {}", var, op_name, inputs[0], ty, ty)
            }
            IROperation::Reduce { op, axes } => {
                let op_name = match op {
                    ReduceOpKind::Sum => "rmi.reduce_sum",
                    ReduceOpKind::Mean => "rmi.reduce_mean",
                    ReduceOpKind::Max => "rmi.reduce_max",
                    ReduceOpKind::Min => "rmi.reduce_min",
                    ReduceOpKind::Prod => "rmi.reduce_prod",
                    ReduceOpKind::Any => "rmi.reduce_any",
                    ReduceOpKind::All => "rmi.reduce_all",
                };
                let axes_str: Vec<String> = axes.iter().map(|a| a.to_string()).collect();
                format!(
                    "{} = {}({}) {{axes = [{}]}} : ({}) -> {}",
                    var,
                    op_name,
                    inputs[0],
                    axes_str.join(", "),
                    ty,
                    ty
                )
            }
            IROperation::Normalize { kind, eps } => {
                let op_name = match kind {
                    NormalizeKind::LayerNorm => "rmi.layer_norm",
                    NormalizeKind::BatchNorm => "rmi.batch_norm",
                    NormalizeKind::GroupNorm => "rmi.group_norm",
                    NormalizeKind::InstanceNorm => "rmi.instance_norm",
                    NormalizeKind::RMSNorm => "rmi.rms_norm",
                };
                format!(
                    "{} = {}({}) {{eps = {}}} : ({}) -> {}",
                    var, op_name, inputs[0], eps, ty, ty
                )
            }
            IROperation::Return => {
                return Ok(format!(
                    "func.return {} : {}",
                    inputs.first().unwrap_or(&"%none".to_string()),
                    ty
                ));
            }
            _ => format!(
                "{} = \"{}.op\"({}) : {}",
                var,
                self.dialect,
                inputs.join(", "),
                ty
            ),
        };

        Ok(line)
    }
}

impl Default for MlirEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeEmitter for MlirEmitter {
    fn emit(&self, program: &Program) -> Result<String> {
        let mut output = String::new();
        output.push_str("// Auto-generated by RMI CodeGen\n");
        output.push_str(&format!("// Program: {}\n\n", program.name));
        output.push_str("module {\n\n");

        for func in &program.functions {
            output.push_str(&self.emit_function(func)?);
            output.push('\n');
        }

        output.push_str("} // end module\n");
        Ok(output)
    }

    fn target(&self) -> EmitTarget {
        EmitTarget::MLIR
    }
}

// ============================================================================
// ONNX Emitter — Model interchange format
// ============================================================================

/// Emits ONNX-compatible graph description in protobuf text format.
///
/// This emitter produces a human-readable text representation of the ONNX
/// graph. For binary `.onnx` files, the output should be fed through an
/// ONNX protobuf serializer.
///
/// Follows the ONNX operator specification (opset 18+).
pub struct OnnxEmitter {
    /// ONNX opset version
    opset_version: i64,
    /// Producer name
    producer: String,
}

impl OnnxEmitter {
    /// Create a new ONNX emitter.
    pub fn new() -> Self {
        Self {
            opset_version: 18,
            producer: "RMI CodeGen".to_string(),
        }
    }

    /// Set ONNX opset version.
    pub fn opset(mut self, version: i64) -> Self {
        self.opset_version = version;
        self
    }

    fn emit_graph(&self, func: &Function) -> Result<String> {
        let mut out = String::new();

        // Graph inputs
        out.push_str(&format!("  graph {} {{\n", func.name));
        for (name, ty) in &func.params {
            out.push_str(&format!(
                "    input: {{ name: \"{}\" type: {} }}\n",
                name,
                self.emit_tensor_type(ty)
            ));
        }

        // Nodes
        for node in &func.nodes {
            let line = self.emit_node(node)?;
            if !line.is_empty() {
                out.push_str(&format!("    {}\n", line));
            }
        }

        // Output
        if let Some(ret_id) = func.return_node {
            out.push_str(&format!(
                "    output: {{ name: \"v{}\" type: {} }}\n",
                ret_id,
                self.emit_tensor_type(&func.return_type)
            ));
        }

        out.push_str("  }\n");
        Ok(out)
    }

    fn emit_tensor_type(&self, ty: &IRType) -> String {
        match ty {
            IRType::Tensor { dtype, shape } => {
                let elem = match dtype {
                    PrimitiveType::F16 => "FLOAT16",
                    PrimitiveType::F32 => "FLOAT",
                    PrimitiveType::F64 => "DOUBLE",
                    PrimitiveType::I8 => "INT8",
                    PrimitiveType::I16 => "INT16",
                    PrimitiveType::I32 => "INT32",
                    PrimitiveType::I64 => "INT64",
                    PrimitiveType::U8 => "UINT8",
                    PrimitiveType::Bool => "BOOL",
                    _ => "FLOAT",
                };
                let dims: Vec<String> = shape
                    .iter()
                    .map(|d| match d {
                        crate::core::codegen::Dimension::Static(n) => n.to_string(),
                        crate::core::codegen::Dimension::Dynamic => "?".to_string(),
                        crate::core::codegen::Dimension::Symbolic(s) => s.clone(),
                    })
                    .collect();
                format!("{{ elem_type: {} shape: [{}] }}", elem, dims.join(", "))
            }
            IRType::Primitive(PrimitiveType::F32) => "{ elem_type: FLOAT shape: [1] }".to_string(),
            IRType::Primitive(PrimitiveType::F64) => "{ elem_type: DOUBLE shape: [1] }".to_string(),
            IRType::Primitive(PrimitiveType::I64) => "{ elem_type: INT64 shape: [1] }".to_string(),
            _ => "{ elem_type: FLOAT shape: [] }".to_string(),
        }
    }

    fn emit_node(&self, node: &IRNode) -> Result<String> {
        let output = format!("v{}", node.id);
        let inputs: Vec<String> = node.inputs.iter().map(|i| format!("v{}", i)).collect();

        let line = match &node.op {
            IROperation::Parameter { name, .. } => {
                format!(
                    "node {{ op_type: \"Identity\" input: \"{}\" output: \"{}\" }}",
                    name, output
                )
            }
            IROperation::Constant => {
                let val = if let Some(IRValue::F64(v)) = node.attrs.get("value") {
                    format!("{:.6}", v)
                } else {
                    "0.0".to_string()
                };
                format!(
                    "node {{ op_type: \"Constant\" output: \"{}\" attribute {{ name: \"value\" f: {} }} }}",
                    output, val
                )
            }
            IROperation::BinaryOp { op } => {
                let onnx_op = match op {
                    BinaryOpKind::Add => "Add",
                    BinaryOpKind::Sub => "Sub",
                    BinaryOpKind::Mul => "Mul",
                    BinaryOpKind::Div => "Div",
                    BinaryOpKind::Pow => "Pow",
                    BinaryOpKind::Min => "Min",
                    BinaryOpKind::Max => "Max",
                    BinaryOpKind::And => "And",
                    BinaryOpKind::Or => "Or",
                    BinaryOpKind::Eq => "Equal",
                    BinaryOpKind::Lt => "Less",
                    BinaryOpKind::Gt => "Greater",
                    BinaryOpKind::Le => "LessOrEqual",
                    BinaryOpKind::Ge => "GreaterOrEqual",
                    BinaryOpKind::Ne => "Not",
                };
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" input: \"{}\" output: \"{}\" }}",
                    onnx_op, inputs[0], inputs[1], output
                )
            }
            IROperation::UnaryOp { op } => {
                let onnx_op = match op {
                    UnaryOpKind::Neg => "Neg",
                    UnaryOpKind::Abs => "Abs",
                    UnaryOpKind::Sqrt => "Sqrt",
                    UnaryOpKind::Exp => "Exp",
                    UnaryOpKind::Log => "Log",
                    UnaryOpKind::Sin => "Sin",
                    UnaryOpKind::Cos => "Cos",
                    UnaryOpKind::Tanh => "Tanh",
                    UnaryOpKind::Ceil => "Ceil",
                    UnaryOpKind::Floor => "Floor",
                    UnaryOpKind::Round => "Round",
                    UnaryOpKind::Not => "Not",
                };
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" output: \"{}\" }}",
                    onnx_op, inputs[0], output
                )
            }
            IROperation::MatMul { .. } => {
                format!(
                    "node {{ op_type: \"MatMul\" input: \"{}\" input: \"{}\" output: \"{}\" }}",
                    inputs[0], inputs[1], output
                )
            }
            IROperation::Activation { kind } => {
                let onnx_op = match kind {
                    ActivationKind::ReLU => "Relu",
                    ActivationKind::LeakyReLU => "LeakyRelu",
                    ActivationKind::Sigmoid => "Sigmoid",
                    ActivationKind::Tanh => "Tanh",
                    ActivationKind::Softmax => "Softmax",
                    ActivationKind::GeLU => "Gelu",
                    ActivationKind::SiLU => "Silu",
                    ActivationKind::Softplus => "Softplus",
                };
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" output: \"{}\" }}",
                    onnx_op, inputs[0], output
                )
            }
            IROperation::Reduce { op, axes } => {
                let onnx_op = match op {
                    ReduceOpKind::Sum => "ReduceSum",
                    ReduceOpKind::Mean => "ReduceMean",
                    ReduceOpKind::Max => "ReduceMax",
                    ReduceOpKind::Min => "ReduceMin",
                    ReduceOpKind::Prod => "ReduceProd",
                    _ => "ReduceSum",
                };
                let axes_str: Vec<String> = axes.iter().map(|a| a.to_string()).collect();
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" output: \"{}\" attribute {{ name: \"axes\" ints: [{}] }} }}",
                    onnx_op, inputs[0], output, axes_str.join(", ")
                )
            }
            IROperation::Normalize { kind, eps } => {
                let onnx_op = match kind {
                    NormalizeKind::BatchNorm => "BatchNormalization",
                    NormalizeKind::LayerNorm => "LayerNormalization",
                    NormalizeKind::InstanceNorm => "InstanceNormalization",
                    _ => "LayerNormalization",
                };
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" output: \"{}\" attribute {{ name: \"epsilon\" f: {} }} }}",
                    onnx_op, inputs[0], output, eps
                )
            }
            IROperation::Conv {
                dims,
                stride,
                padding,
            } => {
                let pads_str = match padding {
                    Padding::Valid => "0, 0".to_string(),
                    Padding::Same => "1, 1".to_string(),
                    Padding::Explicit(p) => p
                        .iter()
                        .map(|(a, b)| format!("{}, {}", a, b))
                        .collect::<Vec<_>>()
                        .join(", "),
                };
                let stride_str = if stride.is_empty() {
                    "1".to_string()
                } else {
                    stride
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!(
                    "node {{ op_type: \"Conv\" input: \"{}\" input: \"{}\" output: \"{}\" attribute {{ name: \"strides\" ints: [{}] }} attribute {{ name: \"pads\" ints: [{}] }} }}  // {}D",
                    inputs[0], inputs.get(1).map_or("", |s| s.as_str()), output, stride_str, pads_str, dims
                )
            }
            IROperation::Pool { kind, kernel, .. } => {
                let onnx_op = match kind {
                    crate::core::codegen::PoolKind::Max => "MaxPool",
                    crate::core::codegen::PoolKind::Avg => "AveragePool",
                    crate::core::codegen::PoolKind::Global => "GlobalAveragePool",
                };
                let kernel_str = kernel
                    .iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "node {{ op_type: \"{}\" input: \"{}\" output: \"{}\" attribute {{ name: \"kernel_shape\" ints: [{}] }} }}",
                    onnx_op, inputs[0], output, kernel_str,
                )
            }
            IROperation::Dropout { rate } => {
                format!(
                    "node {{ op_type: \"Dropout\" input: \"{}\" output: \"{}\" attribute {{ name: \"ratio\" f: {} }} }}",
                    inputs[0], output, rate
                )
            }
            IROperation::TensorReshape => {
                format!(
                    "node {{ op_type: \"Reshape\" input: \"{}\" output: \"{}\" }}",
                    inputs[0], output
                )
            }
            IROperation::TensorTranspose => {
                format!(
                    "node {{ op_type: \"Transpose\" input: \"{}\" output: \"{}\" }}",
                    inputs[0], output
                )
            }
            IROperation::TensorConcat { axis } => {
                let ins = inputs
                    .iter()
                    .map(|i| format!("input: \"{}\"", i))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(
                    "node {{ op_type: \"Concat\" {} output: \"{}\" attribute {{ name: \"axis\" i: {} }} }}",
                    ins, output, axis
                )
            }
            IROperation::Return => {
                return Ok(String::new()); // Return handled at graph level
            }
            _ => format!(
                "node {{ op_type: \"RMI_{}\" input: \"{}\" output: \"{}\" }}",
                format!("{:?}", node.op)
                    .split_whitespace()
                    .next()
                    .unwrap_or("Op"),
                inputs.join("\" input: \""),
                output
            ),
        };

        Ok(line)
    }
}

impl Default for OnnxEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeEmitter for OnnxEmitter {
    fn emit(&self, program: &Program) -> Result<String> {
        let mut output = String::new();
        output.push_str("# Auto-generated by RMI CodeGen\n");
        output.push_str(&format!("# ONNX Model: {}\n\n", program.name));
        output.push_str(&format!(
            "ir_version: 9\nproducer_name: \"{}\"\nopset_import {{ version: {} }}\n\n",
            self.producer, self.opset_version,
        ));

        for func in &program.functions {
            output.push_str(&self.emit_graph(func)?);
            output.push('\n');
        }

        Ok(output)
    }

    fn target(&self) -> EmitTarget {
        EmitTarget::ONNX
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::codegen::FunctionBuilder;

    fn sample_program() -> Program {
        let mut fb = FunctionBuilder::new(
            "forward",
            vec![
                ("x".to_string(), IRType::Primitive(PrimitiveType::F32)),
                ("y".to_string(), IRType::Primitive(PrimitiveType::F32)),
            ],
            IRType::Primitive(PrimitiveType::F32),
        );
        let x = fb.param(0);
        let y = fb.param(1);
        let sum = fb.binary_op(BinaryOpKind::Add, x, y);
        let out = fb.activation(ActivationKind::ReLU, sum);
        fb.ret(out);

        let mut prog = Program::new("test_model");
        prog.add_function(fb.build());
        prog
    }

    fn tensor_program() -> Program {
        let ty = IRType::tensor(PrimitiveType::F32, vec![32, 784]);
        let out_ty = IRType::tensor(PrimitiveType::F32, vec![32, 10]);
        let mut fb = FunctionBuilder::new("classify", vec![("input".to_string(), ty)], out_ty);
        let x = fb.param(0);
        let h = fb.activation(ActivationKind::ReLU, x);
        let _r = fb.reduce(ReduceOpKind::Mean, h, vec![1]);
        fb.ret(h);

        let mut prog = Program::new("classifier");
        prog.add_function(fb.build());
        prog
    }

    // ── CUDA ─────────────────────────────────────────────────────────

    #[test]
    fn cuda_emitter_basic() {
        let prog = sample_program();
        let emitter = CudaEmitter::new();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("__global__"));
        assert!(code.contains("blockIdx"));
        assert!(code.contains("fmaxf(0.0f,"));
        assert!(code.contains("launch_forward"));
    }

    #[test]
    fn cuda_emitter_target() {
        let emitter = CudaEmitter::new().compute_capability(90);
        let prog = sample_program();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("sm_90"));
    }

    // ── MLIR ─────────────────────────────────────────────────────────

    #[test]
    fn mlir_emitter_basic() {
        let prog = sample_program();
        let emitter = MlirEmitter::new();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("module {"));
        assert!(code.contains("func.func @forward"));
        assert!(code.contains("arith.addf"));
        assert!(code.contains("rmi.relu"));
    }

    #[test]
    fn mlir_tensor_types() {
        let prog = tensor_program();
        let emitter = MlirEmitter::new();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("tensor<"));
        assert!(code.contains("f32>"));
    }

    // ── ONNX ─────────────────────────────────────────────────────────

    #[test]
    fn onnx_emitter_basic() {
        let prog = sample_program();
        let emitter = OnnxEmitter::new();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("ir_version: 9"));
        assert!(code.contains("opset_import"));
        assert!(code.contains("op_type: \"Add\""));
        assert!(code.contains("op_type: \"Relu\""));
    }

    #[test]
    fn onnx_tensor_program() {
        let prog = tensor_program();
        let emitter = OnnxEmitter::new();
        let code = emitter.emit(&prog).unwrap();
        assert!(code.contains("graph classify"));
        assert!(code.contains("FLOAT"));
    }

    // ── Cross-emitter ────────────────────────────────────────────────

    #[test]
    fn all_emitters_produce_output() {
        let prog = sample_program();

        let emitters: Vec<Box<dyn CodeEmitter>> = vec![
            Box::new(CudaEmitter::new()),
            Box::new(MlirEmitter::new()),
            Box::new(OnnxEmitter::new()),
        ];

        for emitter in &emitters {
            let code = emitter.emit(&prog).unwrap();
            assert!(
                !code.is_empty(),
                "Emitter {:?} produced empty output",
                emitter.target()
            );
            assert!(
                code.len() > 50,
                "Emitter {:?} produced suspiciously short output ({} bytes)",
                emitter.target(),
                code.len()
            );
        }
    }

    #[test]
    fn emitter_targets_correct() {
        assert_eq!(CudaEmitter::new().target(), EmitTarget::CUDA);
        assert_eq!(MlirEmitter::new().target(), EmitTarget::MLIR);
        assert_eq!(OnnxEmitter::new().target(), EmitTarget::ONNX);
    }
}
