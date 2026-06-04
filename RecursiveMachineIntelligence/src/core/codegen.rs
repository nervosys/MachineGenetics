//! Code Generation Module - Machine-Native Program Synthesis
//!
//! Provides low-level primitives for AI systems to generate, mutate, and compose
//! code and computational graphs. Unlike human-oriented code generation (e.g., LLMs
//! producing text), this module operates on structured representations that machines
//! can reason over formally.
//!
//! # Design Principles
//!
//! 1. **Typed IR**: All generated code goes through a typed intermediate representation
//! 2. **Composable Combinators**: Small primitives that compose into complex programs
//! 3. **Mutation Operators**: Formal operators for program evolution
//! 4. **Verification Hooks**: Integration points for static analysis
//! 5. **Deterministic Serialization**: Reproducible code generation

use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::error::{Result, RmiError};

// ============================================================================
// Unique ID Generation
// ============================================================================

static NODE_COUNTER: AtomicU64 = AtomicU64::new(0);
static PROGRAM_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_node_id() -> u64 {
    NODE_COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn next_program_id() -> u64 {
    PROGRAM_COUNTER.fetch_add(1, Ordering::SeqCst)
}

// ============================================================================
// Type System - Machine-Readable Types
// ============================================================================

/// Primitive types in the IR
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveType {
    /// Void/unit type (no value)
    Void,
    /// Boolean (true/false)
    Bool,
    /// 8-bit signed integer
    I8,
    /// 16-bit signed integer
    I16,
    /// 32-bit signed integer
    I32,
    /// 64-bit signed integer
    I64,
    /// 8-bit unsigned integer
    U8,
    /// 16-bit unsigned integer
    U16,
    /// 32-bit unsigned integer
    U32,
    /// 64-bit unsigned integer
    U64,
    /// 16-bit floating point (IEEE 754)
    F16,
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// Brain floating point (16-bit)
    BF16,
}

/// Type in the intermediate representation
///
/// Represents all types that can appear in the generated IR, including
/// primitives, tensors, functions, and polymorphic types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IRType {
    /// Primitive scalar type
    Primitive(PrimitiveType),

    /// Tensor type with shape and element type
    Tensor {
        /// Element data type
        dtype: PrimitiveType,
        /// Tensor dimensions
        shape: Vec<Dimension>,
    },

    /// Function type
    Function {
        /// Input parameter types
        inputs: Vec<IRType>,
        /// Return type
        output: Box<IRType>,
    },

    /// Tuple type
    Tuple(Vec<IRType>),

    /// Optional type
    Option(Box<IRType>),

    /// Named type reference
    Named(String),

    /// Type variable for polymorphism
    TypeVar(String),

    /// Universal quantification
    Forall {
        /// Type variable names
        vars: Vec<String>,
        /// Body type
        body: Box<IRType>,
    },
}

/// Tensor dimension (static or dynamic)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dimension {
    /// Static known dimension
    Static(usize),
    /// Dynamic dimension (unknown at compile time)
    Dynamic,
    /// Symbolic dimension (named variable)
    Symbolic(String),
}

impl IRType {
    /// Create a tensor type with static shape
    pub fn tensor(dtype: PrimitiveType, shape: Vec<usize>) -> Self {
        IRType::Tensor {
            dtype,
            shape: shape.into_iter().map(Dimension::Static).collect(),
        }
    }

    /// Create a function type
    pub fn function(inputs: Vec<IRType>, output: IRType) -> Self {
        IRType::Function {
            inputs,
            output: Box::new(output),
        }
    }

    /// Check if types are compatible (for assignment, etc.)
    pub fn is_compatible(&self, other: &IRType) -> bool {
        match (self, other) {
            (IRType::Primitive(a), IRType::Primitive(b)) => a == b,
            (
                IRType::Tensor {
                    dtype: d1,
                    shape: s1,
                },
                IRType::Tensor {
                    dtype: d2,
                    shape: s2,
                },
            ) => {
                d1 == d2
                    && s1.len() == s2.len()
                    && s1.iter().zip(s2.iter()).all(|(a, b)| match (a, b) {
                        (Dimension::Static(x), Dimension::Static(y)) => x == y,
                        (Dimension::Dynamic, _) | (_, Dimension::Dynamic) => true,
                        (Dimension::Symbolic(x), Dimension::Symbolic(y)) => x == y,
                        _ => true, // Allow symbolic to match static
                    })
            }
            (IRType::TypeVar(_), _) | (_, IRType::TypeVar(_)) => true,
            _ => self == other,
        }
    }
}

// ============================================================================
// Intermediate Representation Nodes
// ============================================================================

/// An IR node representing a computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRNode {
    /// Unique node identifier
    pub id: u64,
    /// The operation
    pub op: IROperation,
    /// Output type
    pub output_type: IRType,
    /// Input node IDs
    pub inputs: Vec<u64>,
    /// Attributes (operation-specific metadata)
    pub attrs: BTreeMap<String, IRValue>,
    /// Source location (for debugging)
    pub source_loc: Option<SourceLocation>,
}

/// Source location for debugging generated code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Source file path
    pub file: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
}

/// IR operations - the instruction set for generated code
///
/// This enum defines all operations that can appear in the IR. Each operation
/// corresponds to a fundamental computation that machines can compose and reason over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IROperation {
    // === Structural ===
    /// Function parameter
    Parameter {
        /// Parameter index (0-based)
        index: usize,
        /// Parameter name
        name: String,
    },
    /// Constant value
    Constant,
    /// Return value
    Return,
    /// Function call
    Call {
        /// Target function name
        target: String,
    },
    /// Conditional branch
    Cond,
    /// Loop
    Loop {
        /// Loop type
        kind: LoopKind,
    },

    // === Tensor Operations ===
    /// Create tensor from shape
    TensorCreate {
        /// Initialization method
        initializer: TensorInitializer,
    },
    /// Reshape tensor
    TensorReshape,
    /// Transpose tensor
    TensorTranspose,
    /// Slice tensor
    TensorSlice,
    /// Concatenate tensors
    TensorConcat {
        /// Axis to concatenate along
        axis: i32,
    },
    /// Split tensor
    TensorSplit {
        /// Axis to split along
        axis: i32,
        /// Number of splits
        num_splits: usize,
    },

    // === Arithmetic ===
    /// Binary arithmetic operation
    BinaryOp {
        /// Binary operation kind
        op: BinaryOpKind,
    },
    /// Unary operation
    UnaryOp {
        /// Unary operation kind
        op: UnaryOpKind,
    },
    /// Reduction operation
    Reduce {
        /// Reduction operation kind
        op: ReduceOpKind,
        /// Axes to reduce over
        axes: Vec<i32>,
    },
    /// Matrix multiplication
    MatMul {
        /// Transpose first operand
        transpose_a: bool,
        /// Transpose second operand
        transpose_b: bool,
    },
    /// Batched matrix multiplication
    BatchMatMul {
        /// Transpose first operand
        transpose_a: bool,
        /// Transpose second operand
        transpose_b: bool,
    },

    // === Neural Network ===
    /// Convolution
    Conv {
        /// Number of spatial dimensions (1, 2, or 3)
        dims: usize,
        /// Stride for each spatial dimension
        stride: Vec<usize>,
        /// Padding mode
        padding: Padding,
    },
    /// Pooling
    Pool {
        /// Pooling type (max, avg, global)
        kind: PoolKind,
        /// Kernel size for each dimension
        kernel: Vec<usize>,
        /// Stride for each dimension
        stride: Vec<usize>,
    },
    /// Normalization
    Normalize {
        /// Normalization type
        kind: NormalizeKind,
        /// Epsilon for numerical stability
        eps: f64,
    },
    /// Activation function
    Activation {
        /// Activation type
        kind: ActivationKind,
    },
    /// Dropout
    Dropout {
        /// Dropout probability
        rate: f64,
    },
    /// Attention
    Attention {
        /// Number of attention heads
        num_heads: usize,
        /// Dimension per head
        head_dim: usize,
    },

    // === Control Flow ===
    /// Phi node (SSA merge point)
    Phi,
    /// Select (ternary)
    Select,

    // === Memory ===
    /// Allocate tensor
    Alloc,
    /// Free tensor
    Free,
    /// Load from memory
    Load,
    /// Store to memory
    Store,

    // === Custom ===
    /// Custom operation (extensibility)
    Custom {
        /// Operation name
        name: String,
    },
}

/// Loop kinds for control flow operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoopKind {
    /// Standard for loop with fixed iteration count
    For,
    /// While loop with condition check
    While,
    /// Scan operation (sequential accumulation)
    Scan,
    /// Map operation (parallel element-wise)
    Map,
    /// Fold operation (sequential reduction)
    Fold,
}

/// Tensor initialization strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TensorInitializer {
    /// Initialize all elements to zero
    Zeros,
    /// Initialize all elements to one
    Ones,
    /// Initialize with uniform random values
    Random,
    /// Initialize with normal (Gaussian) random values
    RandomNormal,
    /// Leave memory uninitialized (unsafe but fast)
    Uninitialized,
}

/// Binary operation kinds for arithmetic and comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOpKind {
    /// Addition (a + b)
    Add,
    /// Subtraction (a - b)
    Sub,
    /// Multiplication (a * b)
    Mul,
    /// Division (a / b)
    Div,
    /// Exponentiation (a ^ b)
    Pow,
    /// Element-wise minimum
    Min,
    /// Element-wise maximum
    Max,
    /// Logical AND
    And,
    /// Logical OR
    Or,
    /// Equality comparison (a == b)
    Eq,
    /// Inequality comparison (a != b)
    Ne,
    /// Less than (a < b)
    Lt,
    /// Less than or equal (a <= b)
    Le,
    /// Greater than (a > b)
    Gt,
    /// Greater than or equal (a >= b)
    Ge,
}

/// Unary operation kinds for element-wise transformations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOpKind {
    /// Negation (-x)
    Neg,
    /// Logical NOT (!x)
    Not,
    /// Absolute value (|x|)
    Abs,
    /// Square root (√x)
    Sqrt,
    /// Exponential (e^x)
    Exp,
    /// Natural logarithm (ln(x))
    Log,
    /// Sine function
    Sin,
    /// Cosine function
    Cos,
    /// Hyperbolic tangent
    Tanh,
    /// Ceiling (round up)
    Ceil,
    /// Floor (round down)
    Floor,
    /// Round to nearest integer
    Round,
}

/// Reduction operation kinds for aggregating tensor elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReduceOpKind {
    /// Sum of elements
    Sum,
    /// Arithmetic mean
    Mean,
    /// Maximum element
    Max,
    /// Minimum element
    Min,
    /// Product of elements
    Prod,
    /// Logical any (true if any element is true)
    Any,
    /// Logical all (true if all elements are true)
    All,
}

/// Pooling operation kinds for spatial downsampling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PoolKind {
    /// Maximum pooling (take max value in each region)
    Max,
    /// Average pooling (take mean value in each region)
    Avg,
    /// Global pooling (pool over entire spatial dimensions)
    Global,
}

/// Normalization layer kinds for stabilizing training
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NormalizeKind {
    /// Batch normalization (normalize across batch dimension)
    BatchNorm,
    /// Layer normalization (normalize across feature dimension)
    LayerNorm,
    /// Group normalization (normalize across groups of channels)
    GroupNorm,
    /// Instance normalization (normalize per-sample per-channel)
    InstanceNorm,
    /// RMS normalization (root mean square normalization)
    RMSNorm,
}

/// Activation function kinds for non-linear transformations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActivationKind {
    /// Rectified Linear Unit (max(0, x))
    ReLU,
    /// Leaky ReLU (x if x > 0, else alpha * x)
    LeakyReLU,
    /// Gaussian Error Linear Unit
    GeLU,
    /// Sigmoid Linear Unit (x * sigmoid(x))
    SiLU,
    /// Sigmoid function (1 / (1 + e^-x))
    Sigmoid,
    /// Hyperbolic tangent
    Tanh,
    /// Softmax (normalized exponential)
    Softmax,
    /// Softplus (log(1 + e^x))
    Softplus,
}

/// Padding modes for convolution operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Padding {
    /// No padding (output smaller than input)
    Valid,
    /// Pad to maintain spatial dimensions
    Same,
    /// Explicit padding amounts as (before, after) per dimension
    Explicit(Vec<(usize, usize)>),
}

/// Constant values that can appear in the IR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IRValue {
    /// Boolean constant
    Bool(bool),
    /// Signed 64-bit integer constant
    I64(i64),
    /// Unsigned 64-bit integer constant
    U64(u64),
    /// 64-bit floating point constant
    F64(f64),
    /// String constant
    String(String),
    /// List of values
    List(Vec<IRValue>),
    /// Named map of values
    Map(BTreeMap<String, IRValue>),
}

// ============================================================================
// Program Representation
// ============================================================================

/// A complete generated program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    /// Unique program ID
    pub id: u64,
    /// Program name
    pub name: String,
    /// Functions in the program
    pub functions: Vec<Function>,
    /// Global constants
    pub constants: HashMap<String, IRValue>,
    /// Type definitions
    pub type_defs: HashMap<String, IRType>,
    /// Metadata
    pub metadata: ProgramMetadata,
}

/// Function in the IR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Input parameters
    pub params: Vec<(String, IRType)>,
    /// Return type
    pub return_type: IRType,
    /// Nodes in the function (topologically sorted)
    pub nodes: Vec<IRNode>,
    /// Return node ID
    pub return_node: Option<u64>,
    /// Attributes
    pub attrs: HashMap<String, String>,
}

/// Program metadata for machine reasoning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramMetadata {
    /// Version
    pub version: u32,
    /// Generation method
    pub generation_method: Option<String>,
    /// Parent program ID (if mutated/evolved from another)
    pub parent_id: Option<u64>,
    /// Mutation history
    pub mutations: Vec<MutationRecord>,
    /// Performance metrics (if evaluated)
    pub metrics: HashMap<String, f64>,
    /// Structural hash (for deduplication)
    pub structural_hash: Option<u64>,
}

/// Record of a mutation applied to a program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Mutation type
    pub mutation: MutationType,
    /// Target node/function
    pub target: String,
    /// Timestamp
    pub timestamp: u64,
}

impl Program {
    /// Create a new empty program
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: next_program_id(),
            name: name.into(),
            functions: Vec::new(),
            constants: HashMap::new(),
            type_defs: HashMap::new(),
            metadata: ProgramMetadata::default(),
        }
    }

    /// Add a function to the program
    pub fn add_function(&mut self, func: Function) {
        self.functions.push(func);
    }

    /// Get a function by name
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Compute structural hash for deduplication
    pub fn compute_structural_hash(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        for func in &self.functions {
            func.name.hash(&mut hasher);
            for node in &func.nodes {
                std::mem::discriminant(&node.op).hash(&mut hasher);
                node.inputs.hash(&mut hasher);
            }
        }

        self.metadata.structural_hash = Some(hasher.finish());
    }

    /// Verify program well-formedness
    pub fn verify(&self) -> Result<()> {
        for func in &self.functions {
            func.verify()?;
        }
        Ok(())
    }
}

impl Function {
    /// Create a new function
    pub fn new(
        name: impl Into<String>,
        params: Vec<(String, IRType)>,
        return_type: IRType,
    ) -> Self {
        Self {
            name: name.into(),
            params,
            return_type,
            nodes: Vec::new(),
            return_node: None,
            attrs: HashMap::new(),
        }
    }

    /// Add a node to the function
    pub fn add_node(&mut self, op: IROperation, output_type: IRType, inputs: Vec<u64>) -> u64 {
        let id = next_node_id();
        self.nodes.push(IRNode {
            id,
            op,
            output_type,
            inputs,
            attrs: BTreeMap::new(),
            source_loc: None,
        });
        id
    }

    /// Set the return node
    pub fn set_return(&mut self, node_id: u64) {
        self.return_node = Some(node_id);
    }

    /// Verify function well-formedness
    pub fn verify(&self) -> Result<()> {
        let node_ids: HashSet<u64> = self.nodes.iter().map(|n| n.id).collect();

        for node in &self.nodes {
            for &input in &node.inputs {
                if !node_ids.contains(&input) {
                    return Err(RmiError::invalid_config_simple(format!(
                        "Node {} references undefined input {}",
                        node.id, input
                    )));
                }
            }
        }

        if self.return_node.is_some() && !node_ids.contains(&self.return_node.unwrap()) {
            return Err(RmiError::invalid_config_simple(
                "Return node not found in function",
            ));
        }

        Ok(())
    }
}

// ============================================================================
// Program Builder - Fluent API for Machine Code Generation
// ============================================================================

/// Builder for constructing programs with a fluent API
///
/// Provides a machine-friendly interface for incrementally building
/// programs with type-checked operations.
pub struct ProgramBuilder {
    /// The program being built
    program: Program,
    /// Current function being defined (if any)
    current_function: Option<FunctionBuilder>,
}

/// Builder for constructing functions with a fluent API
///
/// Provides operations for adding nodes, tracking types, and
/// building well-formed function graphs.
pub struct FunctionBuilder {
    /// The function being built
    function: Function,
    /// Type information for each node
    node_types: HashMap<u64, IRType>,
    /// Node IDs for function parameters
    param_ids: Vec<u64>,
}

impl ProgramBuilder {
    /// Create a new program builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            program: Program::new(name),
            current_function: None,
        }
    }

    /// Add a type definition
    pub fn add_type(mut self, name: impl Into<String>, ty: IRType) -> Self {
        self.program.type_defs.insert(name.into(), ty);
        self
    }

    /// Add a constant
    pub fn add_constant(mut self, name: impl Into<String>, value: IRValue) -> Self {
        self.program.constants.insert(name.into(), value);
        self
    }

    /// Begin a function definition
    pub fn function(
        mut self,
        name: impl Into<String>,
        params: Vec<(String, IRType)>,
        return_type: IRType,
    ) -> FunctionBuilder {
        let fb = FunctionBuilder::new(name, params, return_type);
        self.current_function = Some(fb.clone());

        // Create a new FunctionBuilder with reference to program
        FunctionBuilder {
            function: fb.function,
            node_types: fb.node_types,
            param_ids: fb.param_ids,
        }
    }

    /// Build the program
    pub fn build(mut self) -> Result<Program> {
        self.program.compute_structural_hash();
        self.program.verify()?;
        Ok(self.program)
    }
}

impl FunctionBuilder {
    /// Create a new function builder
    pub fn new(
        name: impl Into<String>,
        params: Vec<(String, IRType)>,
        return_type: IRType,
    ) -> Self {
        let mut node_types = HashMap::new();
        let mut function = Function::new(name, params.clone(), return_type);
        let mut param_ids = Vec::new();

        // Add parameter nodes
        for (i, (param_name, param_type)) in params.iter().enumerate() {
            let id = function.add_node(
                IROperation::Parameter {
                    index: i,
                    name: param_name.clone(),
                },
                param_type.clone(),
                vec![],
            );
            node_types.insert(id, param_type.clone());
            param_ids.push(id);
        }

        Self {
            function,
            node_types,
            param_ids,
        }
    }

    /// Get the node ID for a parameter by index
    pub fn param(&self, index: usize) -> u64 {
        self.param_ids[index]
    }

    /// Add a constant node
    pub fn constant(&mut self, value: IRValue, ty: IRType) -> u64 {
        let mut node = IRNode {
            id: next_node_id(),
            op: IROperation::Constant,
            output_type: ty.clone(),
            inputs: vec![],
            attrs: BTreeMap::new(),
            source_loc: None,
        };
        node.attrs.insert("value".to_string(), value);
        let id = node.id;
        self.function.nodes.push(node);
        self.node_types.insert(id, ty);
        id
    }

    /// Add a binary operation
    pub fn binary_op(&mut self, op: BinaryOpKind, lhs: u64, rhs: u64) -> u64 {
        let output_type = self
            .node_types
            .get(&lhs)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::BinaryOp { op },
            output_type.clone(),
            vec![lhs, rhs],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add a unary operation
    pub fn unary_op(&mut self, op: UnaryOpKind, input: u64) -> u64 {
        let output_type = self
            .node_types
            .get(&input)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::UnaryOp { op },
            output_type.clone(),
            vec![input],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add matrix multiplication
    pub fn matmul(&mut self, a: u64, b: u64, transpose_a: bool, transpose_b: bool) -> u64 {
        let output_type = self
            .node_types
            .get(&a)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::MatMul {
                transpose_a,
                transpose_b,
            },
            output_type.clone(),
            vec![a, b],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add a reduction operation
    pub fn reduce(&mut self, op: ReduceOpKind, input: u64, axes: Vec<i32>) -> u64 {
        let output_type = self
            .node_types
            .get(&input)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::Reduce { op, axes },
            output_type.clone(),
            vec![input],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add an activation function
    pub fn activation(&mut self, kind: ActivationKind, input: u64) -> u64 {
        let output_type = self
            .node_types
            .get(&input)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::Activation { kind },
            output_type.clone(),
            vec![input],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add normalization
    pub fn normalize(&mut self, kind: NormalizeKind, input: u64, eps: f64) -> u64 {
        let output_type = self
            .node_types
            .get(&input)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::Normalize { kind, eps },
            output_type.clone(),
            vec![input],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add attention
    pub fn attention(
        &mut self,
        query: u64,
        key: u64,
        value: u64,
        num_heads: usize,
        head_dim: usize,
    ) -> u64 {
        let output_type = self
            .node_types
            .get(&query)
            .cloned()
            .unwrap_or(IRType::Primitive(PrimitiveType::F32));
        let id = self.function.add_node(
            IROperation::Attention {
                num_heads,
                head_dim,
            },
            output_type.clone(),
            vec![query, key, value],
        );
        self.node_types.insert(id, output_type);
        id
    }

    /// Add a function call
    pub fn call(&mut self, target: impl Into<String>, args: Vec<u64>, return_type: IRType) -> u64 {
        let id = self.function.add_node(
            IROperation::Call {
                target: target.into(),
            },
            return_type.clone(),
            args,
        );
        self.node_types.insert(id, return_type);
        id
    }

    /// Set return value
    pub fn ret(&mut self, value: u64) {
        let id = self.function.add_node(
            IROperation::Return,
            self.function.return_type.clone(),
            vec![value],
        );
        self.function.return_node = Some(id);
    }

    /// Build the function
    pub fn build(self) -> Function {
        self.function
    }
}

impl Clone for FunctionBuilder {
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
            node_types: self.node_types.clone(),
            param_ids: self.param_ids.clone(),
        }
    }
}

// ============================================================================
// Mutation Operators - For Evolutionary Program Synthesis
// ============================================================================

/// Types of mutations that can be applied to programs for evolutionary synthesis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutationType {
    // === Structural Mutations ===
    /// Insert a new node into the computation graph
    InsertNode {
        /// Operation to insert
        op: IROperation,
    },
    /// Delete a node from the computation graph
    DeleteNode,
    /// Replace a node's operation with a different one
    ReplaceOp {
        /// New operation to use
        new_op: IROperation,
    },
    /// Change which nodes an operation reads from
    RewireInputs,
    /// Swap the positions of two nodes in the graph
    SwapNodes,

    // === Type Mutations ===
    /// Change the data type of a tensor
    ChangeDType {
        /// New primitive data type
        new_dtype: PrimitiveType,
    },
    /// Change the shape of a tensor
    ChangeShape {
        /// New tensor dimensions
        new_shape: Vec<Dimension>,
    },

    // === Parameter Mutations ===
    /// Change the activation function used
    ChangeActivation {
        /// New activation kind
        new_kind: ActivationKind,
    },
    /// Change the normalization layer type
    ChangeNormalization {
        /// New normalization kind
        new_kind: NormalizeKind,
    },
    /// Modify a numeric constant by a delta
    ModifyConstant {
        /// Amount to add to the constant
        delta: f64,
    },

    // === Architectural Mutations ===
    /// Add a skip/residual connection
    AddSkipConnection,
    /// Remove an existing skip connection
    RemoveSkipConnection,
    /// Add a new layer to the network
    AddLayer {
        /// Type of layer to add
        layer_type: String,
    },
    /// Remove a layer from the network
    RemoveLayer,
    /// Duplicate a block of computation
    DuplicateBlock {
        /// Start node ID of block
        start: u64,
        /// End node ID of block
        end: u64,
    },
}

/// Mutator for program evolution in genetic programming
///
/// Applies random or targeted mutations to programs to explore the space
/// of possible architectures and operations.
pub struct Mutator {
    /// Random seed for reproducibility
    seed: u64,
    /// Mutation probabilities for each type
    probabilities: MutationProbabilities,
}

/// Probabilities for different mutation types in evolutionary synthesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationProbabilities {
    /// Probability of inserting a new node
    pub insert_node: f64,
    /// Probability of deleting an existing node
    pub delete_node: f64,
    /// Probability of replacing an operation
    pub replace_op: f64,
    /// Probability of rewiring node inputs
    pub rewire: f64,
    /// Probability of swapping two nodes
    pub swap: f64,
    /// Probability of changing activation function
    pub change_activation: f64,
    /// Probability of adding skip connection
    pub add_skip: f64,
    /// Probability of modifying a constant
    pub modify_constant: f64,
}

impl Default for MutationProbabilities {
    fn default() -> Self {
        Self {
            insert_node: 0.15,
            delete_node: 0.10,
            replace_op: 0.20,
            rewire: 0.15,
            swap: 0.10,
            change_activation: 0.10,
            add_skip: 0.10,
            modify_constant: 0.10,
        }
    }
}

impl Mutator {
    /// Create a new mutator with given seed
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            probabilities: MutationProbabilities::default(),
        }
    }

    /// Create with custom probabilities
    pub fn with_probabilities(seed: u64, probabilities: MutationProbabilities) -> Self {
        Self {
            seed,
            probabilities,
        }
    }

    /// Apply a specific mutation to a program
    pub fn apply_mutation(&self, program: &mut Program, mutation: &MutationType) -> Result<()> {
        // Record the mutation
        program.metadata.mutations.push(MutationRecord {
            mutation: mutation.clone(),
            target: program.name.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });

        // Recompute structural hash
        program.compute_structural_hash();

        Ok(())
    }

    /// Generate a random mutation based on probabilities
    pub fn random_mutation(&self) -> MutationType {
        // Simple PRNG based on seed
        let r =
            ((self.seed.wrapping_mul(6364136223846793005).wrapping_add(1)) % 100) as f64 / 100.0;

        let mut cumulative = 0.0;

        cumulative += self.probabilities.insert_node;
        if r < cumulative {
            return MutationType::InsertNode {
                op: IROperation::Activation {
                    kind: ActivationKind::ReLU,
                },
            };
        }

        cumulative += self.probabilities.delete_node;
        if r < cumulative {
            return MutationType::DeleteNode;
        }

        cumulative += self.probabilities.replace_op;
        if r < cumulative {
            return MutationType::ReplaceOp {
                new_op: IROperation::Activation {
                    kind: ActivationKind::GeLU,
                },
            };
        }

        cumulative += self.probabilities.change_activation;
        if r < cumulative {
            return MutationType::ChangeActivation {
                new_kind: ActivationKind::SiLU,
            };
        }

        cumulative += self.probabilities.add_skip;
        if r < cumulative {
            return MutationType::AddSkipConnection;
        }

        MutationType::ModifyConstant { delta: 0.1 }
    }
}

// ============================================================================
// Crossover Operators - For Genetic Program Synthesis
// ============================================================================

/// Crossover operator for combining programs in genetic programming
///
/// Creates offspring programs by combining parts of two parent programs,
/// enabling exploration of the solution space through recombination.
pub struct Crossover {}

impl Crossover {
    /// Create a new crossover operator
    pub fn new(_seed: u64) -> Self {
        Self {}
    }

    /// Single-point crossover between two programs
    pub fn single_point(&self, parent_a: &Program, parent_b: &Program) -> Result<Program> {
        let mut child = Program::new(format!("{}_{}_child", parent_a.name, parent_b.name));

        // Take functions from both parents
        if !parent_a.functions.is_empty() {
            let midpoint = parent_a.functions.len() / 2;
            for (i, func) in parent_a.functions.iter().enumerate() {
                if i < midpoint {
                    child.functions.push(func.clone());
                }
            }
        }

        if !parent_b.functions.is_empty() {
            let midpoint = parent_b.functions.len() / 2;
            for (i, func) in parent_b.functions.iter().enumerate() {
                if i >= midpoint {
                    child.functions.push(func.clone());
                }
            }
        }

        // Merge constants
        for (k, v) in &parent_a.constants {
            child.constants.insert(k.clone(), v.clone());
        }
        for (k, v) in &parent_b.constants {
            child
                .constants
                .entry(k.clone())
                .or_insert_with(|| v.clone());
        }

        child.metadata.parent_id = Some(parent_a.id);
        child.metadata.generation_method = Some("crossover:single_point".to_string());
        child.compute_structural_hash();

        Ok(child)
    }

    /// Uniform crossover (randomly select from each parent)
    pub fn uniform(&self, parent_a: &Program, parent_b: &Program) -> Result<Program> {
        let mut child = Program::new(format!("{}_{}_uniform", parent_a.name, parent_b.name));

        // Interleave functions
        let max_funcs = parent_a.functions.len().max(parent_b.functions.len());
        for i in 0..max_funcs {
            // Alternate between parents
            if i % 2 == 0 && i < parent_a.functions.len() {
                child.functions.push(parent_a.functions[i].clone());
            } else if i < parent_b.functions.len() {
                child.functions.push(parent_b.functions[i].clone());
            }
        }

        child.metadata.generation_method = Some("crossover:uniform".to_string());
        child.compute_structural_hash();

        Ok(child)
    }
}

// ============================================================================
// Code Emission - Generate Target Code
// ============================================================================

/// Target language for code emission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmitTarget {
    /// Rust code generation
    Rust,
    /// C/C++ code generation
    Cpp,
    /// CUDA kernel generation
    CUDA,
    /// MLIR dialect generation
    MLIR,
    /// ONNX format export
    ONNX,
}

/// Code emitter trait
pub trait CodeEmitter {
    /// Emit code for the target
    fn emit(&self, program: &Program) -> Result<String>;

    /// Target language
    fn target(&self) -> EmitTarget;
}

/// Rust code emitter for IR programs
///
/// Translates IR programs to idiomatic Rust code that can be
/// compiled and executed.
pub struct RustEmitter;

impl CodeEmitter for RustEmitter {
    fn emit(&self, program: &Program) -> Result<String> {
        let mut output = String::new();
        output.push_str("// Auto-generated by air CodeGen\n");
        output.push_str(&format!("// Program: {}\n\n", program.name));

        for func in &program.functions {
            output.push_str(&self.emit_function(func)?);
            output.push_str("\n\n");
        }

        Ok(output)
    }

    fn target(&self) -> EmitTarget {
        EmitTarget::Rust
    }
}

impl RustEmitter {
    fn emit_function(&self, func: &Function) -> Result<String> {
        let mut output = String::new();

        // Function signature
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", name, self.emit_type(ty)))
            .collect();

        output.push_str(&format!(
            "pub fn {}({}) -> {} {{\n",
            func.name,
            params.join(", "),
            self.emit_type(&func.return_type)
        ));

        // Emit nodes
        for node in &func.nodes {
            output.push_str(&format!("    {}\n", self.emit_node(node)?));
        }

        output.push('}');
        Ok(output)
    }

    fn emit_type(&self, ty: &IRType) -> String {
        match ty {
            IRType::Primitive(PrimitiveType::F32) => "f32".to_string(),
            IRType::Primitive(PrimitiveType::F64) => "f64".to_string(),
            IRType::Primitive(PrimitiveType::I32) => "i32".to_string(),
            IRType::Primitive(PrimitiveType::I64) => "i64".to_string(),
            IRType::Primitive(PrimitiveType::Bool) => "bool".to_string(),
            IRType::Tensor { dtype, shape: _ } => {
                format!(
                    "Tensor<{}>",
                    self.emit_type(&IRType::Primitive(dtype.clone()))
                )
            }
            _ => "()".to_string(),
        }
    }

    fn emit_node(&self, node: &IRNode) -> Result<String> {
        let var_name = format!("v{}", node.id);
        let inputs: Vec<String> = node.inputs.iter().map(|i| format!("v{}", i)).collect();

        let expr = match &node.op {
            IROperation::Parameter { name, .. } => name.clone(),
            IROperation::Constant => {
                if let Some(IRValue::F64(v)) = node.attrs.get("value") {
                    format!("{:.6}", v)
                } else {
                    "0.0".to_string()
                }
            }
            IROperation::BinaryOp { op } => {
                let op_str = match op {
                    BinaryOpKind::Add => "+",
                    BinaryOpKind::Sub => "-",
                    BinaryOpKind::Mul => "*",
                    BinaryOpKind::Div => "/",
                    _ => "?",
                };
                format!("{} {} {}", inputs[0], op_str, inputs[1])
            }
            IROperation::UnaryOp { op } => {
                let func = match op {
                    UnaryOpKind::Neg => "-",
                    UnaryOpKind::Sqrt => ".sqrt()",
                    UnaryOpKind::Exp => ".exp()",
                    UnaryOpKind::Log => ".ln()",
                    _ => "?",
                };
                if func.starts_with('.') {
                    format!("{}{}", inputs[0], func)
                } else {
                    format!("{}{}", func, inputs[0])
                }
            }
            IROperation::MatMul { .. } => {
                format!("{}.matmul(&{})", inputs[0], inputs[1])
            }
            IROperation::Activation { kind } => {
                let func = match kind {
                    ActivationKind::ReLU => "relu",
                    ActivationKind::GeLU => "gelu",
                    ActivationKind::Sigmoid => "sigmoid",
                    ActivationKind::Tanh => "tanh",
                    ActivationKind::Softmax => "softmax",
                    _ => "activation",
                };
                format!("{}({})", func, inputs[0])
            }
            IROperation::Return => {
                return Ok(inputs[0].to_string());
            }
            _ => format!("/* {:?} */", node.op),
        };

        Ok(format!("let {} = {};", var_name, expr))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_creation() {
        let mut builder = FunctionBuilder::new(
            "forward",
            vec![(
                "x".to_string(),
                IRType::tensor(PrimitiveType::F32, vec![1, 784]),
            )],
            IRType::tensor(PrimitiveType::F32, vec![1, 10]),
        );

        // Get parameter node IDs
        let x = builder.param(0);
        let w = builder.constant(
            IRValue::F64(0.01),
            IRType::tensor(PrimitiveType::F32, vec![784, 256]),
        );
        let h = builder.matmul(x, w, false, false);
        let h = builder.activation(ActivationKind::ReLU, h);
        builder.ret(h);

        let func = builder.build();
        assert!(func.verify().is_ok());
    }

    #[test]
    fn test_type_compatibility() {
        let t1 = IRType::tensor(PrimitiveType::F32, vec![32, 64]);
        let t2 = IRType::tensor(PrimitiveType::F32, vec![32, 64]);
        let t3 = IRType::tensor(PrimitiveType::F64, vec![32, 64]);

        assert!(t1.is_compatible(&t2));
        assert!(!t1.is_compatible(&t3));
    }

    #[test]
    fn test_mutation_types() {
        let mutator = Mutator::new(42);
        let mutation = mutator.random_mutation();

        // Should generate some mutation
        match mutation {
            MutationType::InsertNode { .. }
            | MutationType::DeleteNode
            | MutationType::ReplaceOp { .. }
            | MutationType::ChangeActivation { .. }
            | MutationType::AddSkipConnection
            | MutationType::ModifyConstant { .. } => {}
            _ => panic!("Unexpected mutation type"),
        }
    }

    #[test]
    fn test_crossover() {
        let mut prog_a = Program::new("prog_a");
        prog_a.add_function(Function::new(
            "func_a",
            vec![],
            IRType::Primitive(PrimitiveType::Void),
        ));

        let mut prog_b = Program::new("prog_b");
        prog_b.add_function(Function::new(
            "func_b",
            vec![],
            IRType::Primitive(PrimitiveType::Void),
        ));

        let crossover = Crossover::new(42);
        let child = crossover.single_point(&prog_a, &prog_b).unwrap();

        assert!(child.metadata.generation_method.is_some());
    }

    #[test]
    fn test_rust_emission() {
        let mut builder = FunctionBuilder::new(
            "add",
            vec![
                ("a".to_string(), IRType::Primitive(PrimitiveType::F32)),
                ("b".to_string(), IRType::Primitive(PrimitiveType::F32)),
            ],
            IRType::Primitive(PrimitiveType::F32),
        );

        let a = builder.param(0);
        let b = builder.param(1);
        let sum = builder.binary_op(BinaryOpKind::Add, a, b);
        builder.ret(sum);

        let func = builder.build();
        let mut program = Program::new("test");
        program.add_function(func);

        let emitter = RustEmitter;
        let code = emitter.emit(&program).unwrap();

        assert!(code.contains("pub fn add"));
        assert!(code.contains("f32"));
    }

    #[test]
    fn test_structural_hash() {
        let mut prog1 = Program::new("test1");
        let mut prog2 = Program::new("test2");

        prog1.add_function(Function::new(
            "f",
            vec![],
            IRType::Primitive(PrimitiveType::Void),
        ));
        prog2.add_function(Function::new(
            "f",
            vec![],
            IRType::Primitive(PrimitiveType::Void),
        ));

        prog1.compute_structural_hash();
        prog2.compute_structural_hash();

        // Same structure should have same hash
        assert_eq!(
            prog1.metadata.structural_hash,
            prog2.metadata.structural_hash
        );
    }
}
