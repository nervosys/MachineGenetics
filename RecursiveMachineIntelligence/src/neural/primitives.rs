//! Neural Primitives
//!
//! Machine-readable definitions of neural operations that AI agents
//! can compose to build, analyze, and optimize neural architectures.

use crate::core::primitives::{AlgebraicProperty, GradientInfo, HardwareAffinity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Neural primitive categories for systematic composition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NeuralPrimitiveKind {
    // === Linear Transformations ===
    /// Matrix multiplication operation
    MatMul,
    /// Linear layer (weight matrix + optional bias)
    Linear,
    /// Bilinear transformation (two inputs)
    Bilinear,
    /// Embedding lookup table
    Embedding,

    // === Convolutions ===
    /// 1D convolution (sequences, time series)
    Conv1d,
    /// 2D convolution (images)
    Conv2d,
    /// 3D convolution (video, volumetric data)
    Conv3d,
    /// Transposed 2D convolution (upsampling)
    ConvTranspose2d,
    /// Depthwise separable convolution (efficient)
    DepthwiseSeparableConv,

    // === Attention Mechanisms ===
    /// Scaled dot-product attention
    ScaledDotProductAttention,
    /// Multi-head attention layer
    MultiHeadAttention,
    /// Cross-attention (encoder-decoder)
    CrossAttention,
    /// Self-attention (same input for Q, K, V)
    SelfAttention,
    /// Linear attention (O(n) complexity)
    LinearAttention,
    /// Flash attention (memory-efficient)
    FlashAttention,

    // === Normalization ===
    /// Layer normalization
    LayerNorm,
    /// Batch normalization
    BatchNorm,
    /// Group normalization
    GroupNorm,
    /// Instance normalization
    InstanceNorm,
    /// RMS normalization
    RMSNorm,

    // === Activations ===
    /// Rectified Linear Unit
    ReLU,
    /// Gaussian Error Linear Unit
    GeLU,
    /// Sigmoid Linear Unit
    SiLU,
    /// Sigmoid function
    Sigmoid,
    /// Hyperbolic tangent
    Tanh,
    /// Softmax function
    Softmax,
    /// Log-softmax function
    LogSoftmax,
    /// Softplus function
    Softplus,
    /// Mish activation
    Mish,

    // === Pooling ===
    /// 2D max pooling
    MaxPool2d,
    /// 2D average pooling
    AvgPool2d,
    /// Adaptive average pooling
    AdaptiveAvgPool,
    /// Global average pooling
    GlobalAvgPool,

    // === Recurrent ===
    /// Simple RNN cell
    RNNCell,
    /// Long Short-Term Memory cell
    LSTMCell,
    /// Gated Recurrent Unit cell
    GRUCell,

    // === Regularization ===
    /// Standard dropout
    Dropout,
    /// Drop path (stochastic depth)
    DropPath,

    // === Positional Encoding ===
    /// Sinusoidal positional encoding
    SinusoidalPositionalEncoding,
    /// Rotary positional encoding (RoPE)
    RotaryPositionalEncoding,
    /// Learned positional encoding
    LearnedPositionalEncoding,
    /// Attention with Linear Biases
    ALiBi,

    // === Residual Connections ===
    /// Residual add connection
    ResidualAdd,
    /// Dense connection (DenseNet-style)
    DenseConnection,
}

/// A neural primitive with full metadata for agent reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralPrimitive {
    /// Unique identifier
    pub id: Uuid,

    /// The kind of neural primitive
    pub kind: NeuralPrimitiveKind,

    /// Human-readable name (for debugging)
    pub name: String,

    /// Input tensor specifications
    pub inputs: Vec<TensorSpec>,

    /// Output tensor specification
    pub output: TensorSpec,

    /// Learnable parameters
    pub parameters: Vec<ParameterSpec>,

    /// Hyperparameters that control behavior
    pub hyperparameters: Vec<HyperparameterSpec>,

    /// Memory requirements (in bytes per batch element)
    pub memory_per_element: MemoryEstimate,

    /// FLOPs per forward pass (parameterized)
    pub flops_formula: ComputeFormula,

    /// Algebraic properties for optimization
    pub properties: Vec<AlgebraicProperty>,

    /// Hardware preferences
    pub hardware_affinity: HardwareAffinity,

    /// Gradient information
    pub gradient_info: GradientInfo,

    /// Fusion opportunities with other primitives
    pub fusable_with: Vec<NeuralPrimitiveKind>,

    /// Known numerical stability issues
    pub stability_notes: Vec<StabilityNote>,
}

/// Specification for a tensor input/output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorSpec {
    /// Symbolic name for this tensor
    pub name: String,

    /// Shape specification (can include symbolic dimensions)
    pub shape: ShapeSpec,

    /// Data type
    pub dtype: TensorDType,

    /// Whether this tensor is optional
    pub optional: bool,

    /// Constraints on tensor values
    pub constraints: Vec<TensorConstraint>,
}

/// Shape specification supporting symbolic and dynamic dimensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeSpec {
    /// Individual dimension specifications
    pub dims: Vec<DimSpec>,
}

impl ShapeSpec {
    /// Create a new shape specification from dimension specs
    pub fn new(dims: Vec<DimSpec>) -> Self {
        Self { dims }
    }

    /// Create from concrete dimension values
    pub fn from_concrete(dims: &[usize]) -> Self {
        Self {
            dims: dims.iter().map(|&d| DimSpec::Fixed(d)).collect(),
        }
    }

    /// Get the rank (number of dimensions)
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Calculate concrete shape given symbol bindings
    pub fn resolve(&self, bindings: &HashMap<String, usize>) -> Option<Vec<usize>> {
        self.dims.iter().map(|d| d.resolve(bindings)).collect()
    }
}

/// Dimension specification for tensor shapes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DimSpec {
    /// Fixed known dimension value
    Fixed(usize),

    /// Symbolic dimension (e.g., "batch_size", "seq_len")
    Symbol(String),

    /// Dynamic dimension (any value at runtime)
    Dynamic,

    /// Computed from an expression
    Computed(DimExpr),
}

impl DimSpec {
    /// Resolve the dimension with given variable bindings
    pub fn resolve(&self, bindings: &HashMap<String, usize>) -> Option<usize> {
        match self {
            DimSpec::Fixed(n) => Some(*n),
            DimSpec::Symbol(s) => bindings.get(s).copied(),
            DimSpec::Dynamic => None,
            DimSpec::Computed(expr) => expr.evaluate(bindings),
        }
    }
}

/// Expression for computing dimensions algebraically
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DimExpr {
    /// Constant integer value
    Const(usize),
    /// Variable reference by name
    Var(String),
    /// Addition (a + b)
    Add(Box<DimExpr>, Box<DimExpr>),
    /// Subtraction (a - b)
    Sub(Box<DimExpr>, Box<DimExpr>),
    /// Multiplication (a * b)
    Mul(Box<DimExpr>, Box<DimExpr>),
    /// Division (a / b)
    Div(Box<DimExpr>, Box<DimExpr>),
    /// Floor division (a // b)
    FloorDiv(Box<DimExpr>, Box<DimExpr>),
    /// Ceiling division (ceil(a / b))
    CeilDiv(Box<DimExpr>, Box<DimExpr>),
}

impl DimExpr {
    /// Evaluate the expression with given variable bindings
    pub fn evaluate(&self, bindings: &HashMap<String, usize>) -> Option<usize> {
        match self {
            DimExpr::Const(n) => Some(*n),
            DimExpr::Var(s) => bindings.get(s).copied(),
            DimExpr::Add(a, b) => Some(a.evaluate(bindings)? + b.evaluate(bindings)?),
            DimExpr::Sub(a, b) => {
                let a_val = a.evaluate(bindings)?;
                let b_val = b.evaluate(bindings)?;
                a_val.checked_sub(b_val)
            }
            DimExpr::Mul(a, b) => Some(a.evaluate(bindings)? * b.evaluate(bindings)?),
            DimExpr::Div(a, b) => {
                let b_val = b.evaluate(bindings)?;
                if b_val == 0 {
                    return None;
                }
                Some(a.evaluate(bindings)? / b_val)
            }
            DimExpr::FloorDiv(a, b) => {
                let b_val = b.evaluate(bindings)?;
                if b_val == 0 {
                    return None;
                }
                Some(a.evaluate(bindings)? / b_val)
            }
            DimExpr::CeilDiv(a, b) => {
                let a_val = a.evaluate(bindings)?;
                let b_val = b.evaluate(bindings)?;
                if b_val == 0 {
                    return None;
                }
                Some(a_val.div_ceil(b_val))
            }
        }
    }
}

/// Tensor data types for neural operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TensorDType {
    /// 32-bit floating point
    F32,
    /// 64-bit floating point
    F64,
    /// 16-bit floating point (IEEE)
    F16,
    /// Brain floating point (16-bit)
    BF16,
    /// 32-bit signed integer
    I32,
    /// 64-bit signed integer
    I64,
    /// 8-bit unsigned integer
    U8,
    /// Boolean
    Bool,
}

/// Constraints on tensor values for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TensorConstraint {
    /// Values must be in range [min, max]
    Range {
        /// Minimum allowed value
        min: f64,
        /// Maximum allowed value
        max: f64,
    },

    /// Values must be non-negative (>= 0)
    NonNegative,

    /// Tensor must be normalized to specified norm
    Normalized {
        /// Type of norm constraint
        norm: NormType,
    },

    /// Tensor represents probability distribution (sums to 1)
    Probability,

    /// Values must be integer (no fractional part)
    Integer,

    /// Matrix must be positive definite
    PositiveDefinite,

    /// Matrix must be orthogonal
    Orthogonal,
}

/// Norm types for tensor normalization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NormType {
    /// L1 norm (sum of absolute values)
    L1,
    /// L2 norm (Euclidean norm)
    L2,
    /// L-infinity norm (maximum absolute value)
    LInf,
}

/// Specification for learnable parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSpec {
    /// Parameter name
    pub name: String,

    /// Shape specification
    pub shape: ShapeSpec,

    /// Data type
    pub dtype: TensorDType,

    /// Initialization strategy
    pub init: InitStrategy,

    /// Whether parameter is frozen
    pub trainable: bool,

    /// Regularization applied
    pub regularization: Option<RegularizationType>,
}

/// Parameter initialization strategies for neural networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InitStrategy {
    /// Initialize all values to zero
    Zeros,

    /// Initialize all values to one
    Ones,

    /// Initialize to a constant value
    Constant(f64),

    /// Uniform random distribution
    Uniform {
        /// Lower bound
        low: f64,
        /// Upper bound
        high: f64,
    },

    /// Normal (Gaussian) distribution
    Normal {
        /// Mean of distribution
        mean: f64,
        /// Standard deviation
        std: f64,
    },

    /// Xavier/Glorot uniform initialization
    XavierUniform,

    /// Xavier/Glorot normal initialization
    XavierNormal,

    /// Kaiming/He uniform initialization
    KaimingUniform {
        /// Nonlinearity type (e.g., "relu", "leaky_relu")
        nonlinearity: String,
    },

    /// Kaiming/He normal initialization
    KaimingNormal {
        /// Nonlinearity type (e.g., "relu", "leaky_relu")
        nonlinearity: String,
    },

    /// Orthogonal initialization
    Orthogonal {
        /// Gain factor for scaling
        gain: f64,
    },

    /// Sparse initialization
    Sparse {
        /// Fraction of zeros (0.0 to 1.0)
        sparsity: f64,
    },
}

/// Regularization types for preventing overfitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegularizationType {
    /// L1 regularization (lasso)
    L1 {
        /// Regularization strength
        lambda: f64,
    },
    /// L2 regularization (ridge/weight decay)
    L2 {
        /// Regularization strength
        lambda: f64,
    },
    /// Elastic net (combined L1 + L2)
    ElasticNet {
        /// Ratio of L1 to L2 (0 = pure L2, 1 = pure L1)
        l1_ratio: f64,
        /// Total regularization strength
        lambda: f64,
    },
    /// Spectral normalization
    Spectral {
        /// Number of power iterations
        iterations: usize,
    },
}

/// Specification for hyperparameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperparameterSpec {
    /// Hyperparameter name
    pub name: String,

    /// Type of the hyperparameter
    pub hparam_type: HyperparameterType,

    /// Default value
    pub default: HyperparameterValue,

    /// Valid range or choices
    pub constraints: HyperparameterConstraint,

    /// Description for agents
    pub description: String,
}

/// Types of hyperparameters for neural primitives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperparameterType {
    /// Integer hyperparameter
    Int,
    /// Floating point hyperparameter
    Float,
    /// Boolean hyperparameter
    Bool,
    /// Categorical choice hyperparameter
    Categorical,
}

/// Concrete hyperparameter values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperparameterValue {
    /// Integer value
    Int(i64),
    /// Floating point value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// Categorical string value
    Categorical(String),
}

/// Constraints on hyperparameter values for search spaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperparameterConstraint {
    /// Integer range constraint
    IntRange {
        /// Minimum value (inclusive)
        min: i64,
        /// Maximum value (inclusive)
        max: i64,
        /// Whether to use logarithmic scale in search
        log_scale: bool,
    },
    /// Float range constraint
    FloatRange {
        /// Minimum value (inclusive)
        min: f64,
        /// Maximum value (inclusive)
        max: f64,
        /// Whether to use logarithmic scale in search
        log_scale: bool,
    },
    /// Categorical choices
    Choices(Vec<String>),
    /// No constraint (any value allowed)
    None,
}

/// Memory estimation for a primitive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEstimate {
    /// Activation memory (forward pass)
    pub activation_bytes: ComputeFormula,

    /// Parameter memory
    pub parameter_bytes: ComputeFormula,

    /// Gradient memory (training)
    pub gradient_bytes: ComputeFormula,

    /// Temporary buffers
    pub workspace_bytes: ComputeFormula,
}

/// Formula for computing FLOPs or memory requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComputeFormula {
    /// Constant value
    Const(usize),

    /// Single variable reference
    Var(String),

    /// Product of terms (a * b * c * ...)
    Product(Vec<ComputeFormula>),

    /// Sum of terms (a + b + c + ...)
    Sum(Vec<ComputeFormula>),

    /// Power expression (base ^ exp)
    Power(Box<ComputeFormula>, u32),

    /// Coefficient multiplier (c * expr)
    Coeff(f64, Box<ComputeFormula>),
}

impl ComputeFormula {
    /// Evaluate the formula with given variable bindings
    pub fn evaluate(&self, bindings: &HashMap<String, usize>) -> Option<f64> {
        match self {
            ComputeFormula::Const(n) => Some(*n as f64),
            ComputeFormula::Var(s) => bindings.get(s).map(|&n| n as f64),
            ComputeFormula::Product(terms) => terms
                .iter()
                .try_fold(1.0, |acc, t| Some(acc * t.evaluate(bindings)?)),
            ComputeFormula::Sum(terms) => terms
                .iter()
                .try_fold(0.0, |acc, t| Some(acc + t.evaluate(bindings)?)),
            ComputeFormula::Power(base, exp) => Some(base.evaluate(bindings)?.powi(*exp as i32)),
            ComputeFormula::Coeff(c, inner) => Some(c * inner.evaluate(bindings)?),
        }
    }
}

/// Numerical stability notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityNote {
    /// Condition that triggers instability
    pub condition: String,

    /// Description of the issue
    pub description: String,

    /// Severity level
    pub severity: StabilitySeverity,

    /// Mitigation strategy
    pub mitigation: String,
}

/// Severity level for numerical stability issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StabilitySeverity {
    /// Informational note (unlikely to cause problems)
    Info,
    /// Warning (may cause issues in some cases)
    Warning,
    /// Critical (will likely cause NaN or overflow)
    Critical,
}

impl NeuralPrimitive {
    /// Create a new neural primitive
    pub fn new(kind: NeuralPrimitiveKind, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            name: name.into(),
            inputs: Vec::new(),
            output: TensorSpec {
                name: "output".to_string(),
                shape: ShapeSpec::new(vec![]),
                dtype: TensorDType::F32,
                optional: false,
                constraints: vec![],
            },
            parameters: Vec::new(),
            hyperparameters: Vec::new(),
            memory_per_element: MemoryEstimate {
                activation_bytes: ComputeFormula::Const(0),
                parameter_bytes: ComputeFormula::Const(0),
                gradient_bytes: ComputeFormula::Const(0),
                workspace_bytes: ComputeFormula::Const(0),
            },
            flops_formula: ComputeFormula::Const(0),
            properties: Vec::new(),
            hardware_affinity: HardwareAffinity::default(),
            gradient_info: GradientInfo::default(),
            fusable_with: Vec::new(),
            stability_notes: Vec::new(),
        }
    }

    /// Estimate FLOPs for given dimension bindings
    pub fn estimate_flops(&self, bindings: &HashMap<String, usize>) -> Option<f64> {
        self.flops_formula.evaluate(bindings)
    }

    /// Estimate total memory for given dimension bindings
    pub fn estimate_memory(&self, bindings: &HashMap<String, usize>) -> Option<f64> {
        let activation = self
            .memory_per_element
            .activation_bytes
            .evaluate(bindings)?;
        let params = self.memory_per_element.parameter_bytes.evaluate(bindings)?;
        let grads = self.memory_per_element.gradient_bytes.evaluate(bindings)?;
        let workspace = self.memory_per_element.workspace_bytes.evaluate(bindings)?;
        Some(activation + params + grads + workspace)
    }

    /// Check if this primitive can be fused with another
    pub fn can_fuse_with(&self, other: &NeuralPrimitive) -> bool {
        self.fusable_with.contains(&other.kind)
    }
}

/// Registry of all neural primitives for discovery and composition
pub struct NeuralPrimitiveRegistry {
    /// Map from primitive kind to primitive definition
    primitives: HashMap<NeuralPrimitiveKind, NeuralPrimitive>,
}

impl NeuralPrimitiveRegistry {
    /// Create a new registry with built-in primitives
    pub fn new() -> Self {
        let mut registry = Self {
            primitives: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register built-in neural primitives
    fn register_builtins(&mut self) {
        self.register_linear();
        self.register_conv2d();
        self.register_attention();
        self.register_layer_norm();
        self.register_activations();
    }

    fn register_linear(&mut self) {
        let mut linear = NeuralPrimitive::new(NeuralPrimitiveKind::Linear, "Linear");

        linear.inputs = vec![TensorSpec {
            name: "input".to_string(),
            shape: ShapeSpec::new(vec![
                DimSpec::Symbol("batch".to_string()),
                DimSpec::Symbol("seq".to_string()),
                DimSpec::Symbol("in_features".to_string()),
            ]),
            dtype: TensorDType::F32,
            optional: false,
            constraints: vec![],
        }];

        linear.output = TensorSpec {
            name: "output".to_string(),
            shape: ShapeSpec::new(vec![
                DimSpec::Symbol("batch".to_string()),
                DimSpec::Symbol("seq".to_string()),
                DimSpec::Symbol("out_features".to_string()),
            ]),
            dtype: TensorDType::F32,
            optional: false,
            constraints: vec![],
        };

        linear.parameters = vec![
            ParameterSpec {
                name: "weight".to_string(),
                shape: ShapeSpec::new(vec![
                    DimSpec::Symbol("out_features".to_string()),
                    DimSpec::Symbol("in_features".to_string()),
                ]),
                dtype: TensorDType::F32,
                init: InitStrategy::KaimingUniform {
                    nonlinearity: "relu".to_string(),
                },
                trainable: true,
                regularization: None,
            },
            ParameterSpec {
                name: "bias".to_string(),
                shape: ShapeSpec::new(vec![DimSpec::Symbol("out_features".to_string())]),
                dtype: TensorDType::F32,
                init: InitStrategy::Zeros,
                trainable: true,
                regularization: None,
            },
        ];

        // FLOPs: 2 * batch * seq * in_features * out_features
        linear.flops_formula = ComputeFormula::Product(vec![
            ComputeFormula::Const(2),
            ComputeFormula::Var("batch".to_string()),
            ComputeFormula::Var("seq".to_string()),
            ComputeFormula::Var("in_features".to_string()),
            ComputeFormula::Var("out_features".to_string()),
        ]);

        linear.properties = vec![AlgebraicProperty::Linear];

        linear.fusable_with = vec![
            NeuralPrimitiveKind::ReLU,
            NeuralPrimitiveKind::GeLU,
            NeuralPrimitiveKind::SiLU,
            NeuralPrimitiveKind::LayerNorm,
        ];

        self.primitives.insert(NeuralPrimitiveKind::Linear, linear);
    }

    fn register_conv2d(&mut self) {
        let mut conv = NeuralPrimitive::new(NeuralPrimitiveKind::Conv2d, "Conv2d");

        conv.inputs = vec![TensorSpec {
            name: "input".to_string(),
            shape: ShapeSpec::new(vec![
                DimSpec::Symbol("batch".to_string()),
                DimSpec::Symbol("in_channels".to_string()),
                DimSpec::Symbol("height".to_string()),
                DimSpec::Symbol("width".to_string()),
            ]),
            dtype: TensorDType::F32,
            optional: false,
            constraints: vec![],
        }];

        conv.hyperparameters = vec![
            HyperparameterSpec {
                name: "kernel_size".to_string(),
                hparam_type: HyperparameterType::Int,
                default: HyperparameterValue::Int(3),
                constraints: HyperparameterConstraint::IntRange {
                    min: 1,
                    max: 11,
                    log_scale: false,
                },
                description: "Spatial extent of the convolutional kernel".to_string(),
            },
            HyperparameterSpec {
                name: "stride".to_string(),
                hparam_type: HyperparameterType::Int,
                default: HyperparameterValue::Int(1),
                constraints: HyperparameterConstraint::IntRange {
                    min: 1,
                    max: 4,
                    log_scale: false,
                },
                description: "Step size of the kernel".to_string(),
            },
            HyperparameterSpec {
                name: "padding".to_string(),
                hparam_type: HyperparameterType::Int,
                default: HyperparameterValue::Int(1),
                constraints: HyperparameterConstraint::IntRange {
                    min: 0,
                    max: 5,
                    log_scale: false,
                },
                description: "Zero-padding added to both sides".to_string(),
            },
        ];

        conv.properties = vec![AlgebraicProperty::Linear];

        self.primitives.insert(NeuralPrimitiveKind::Conv2d, conv);
    }

    fn register_attention(&mut self) {
        let mut attn = NeuralPrimitive::new(
            NeuralPrimitiveKind::ScaledDotProductAttention,
            "ScaledDotProductAttention",
        );

        attn.inputs = vec![
            TensorSpec {
                name: "query".to_string(),
                shape: ShapeSpec::new(vec![
                    DimSpec::Symbol("batch".to_string()),
                    DimSpec::Symbol("heads".to_string()),
                    DimSpec::Symbol("seq_q".to_string()),
                    DimSpec::Symbol("head_dim".to_string()),
                ]),
                dtype: TensorDType::F32,
                optional: false,
                constraints: vec![],
            },
            TensorSpec {
                name: "key".to_string(),
                shape: ShapeSpec::new(vec![
                    DimSpec::Symbol("batch".to_string()),
                    DimSpec::Symbol("heads".to_string()),
                    DimSpec::Symbol("seq_kv".to_string()),
                    DimSpec::Symbol("head_dim".to_string()),
                ]),
                dtype: TensorDType::F32,
                optional: false,
                constraints: vec![],
            },
            TensorSpec {
                name: "value".to_string(),
                shape: ShapeSpec::new(vec![
                    DimSpec::Symbol("batch".to_string()),
                    DimSpec::Symbol("heads".to_string()),
                    DimSpec::Symbol("seq_kv".to_string()),
                    DimSpec::Symbol("head_dim".to_string()),
                ]),
                dtype: TensorDType::F32,
                optional: false,
                constraints: vec![],
            },
        ];

        // FLOPs: 2 * batch * heads * seq_q * seq_kv * head_dim (for QK^T and attn*V)
        attn.flops_formula = ComputeFormula::Product(vec![
            ComputeFormula::Const(4), // 2 matmuls
            ComputeFormula::Var("batch".to_string()),
            ComputeFormula::Var("heads".to_string()),
            ComputeFormula::Var("seq_q".to_string()),
            ComputeFormula::Var("seq_kv".to_string()),
            ComputeFormula::Var("head_dim".to_string()),
        ]);

        attn.stability_notes = vec![StabilityNote {
            condition: "seq_len > 8192".to_string(),
            description: "Attention scores can overflow in FP16".to_string(),
            severity: StabilitySeverity::Warning,
            mitigation: "Use Flash Attention or split into chunks".to_string(),
        }];

        self.primitives
            .insert(NeuralPrimitiveKind::ScaledDotProductAttention, attn);
    }

    fn register_layer_norm(&mut self) {
        let mut ln = NeuralPrimitive::new(NeuralPrimitiveKind::LayerNorm, "LayerNorm");

        ln.inputs = vec![TensorSpec {
            name: "input".to_string(),
            shape: ShapeSpec::new(vec![
                DimSpec::Symbol("batch".to_string()),
                DimSpec::Symbol("seq".to_string()),
                DimSpec::Symbol("hidden".to_string()),
            ]),
            dtype: TensorDType::F32,
            optional: false,
            constraints: vec![],
        }];

        ln.parameters = vec![
            ParameterSpec {
                name: "weight".to_string(),
                shape: ShapeSpec::new(vec![DimSpec::Symbol("hidden".to_string())]),
                dtype: TensorDType::F32,
                init: InitStrategy::Ones,
                trainable: true,
                regularization: None,
            },
            ParameterSpec {
                name: "bias".to_string(),
                shape: ShapeSpec::new(vec![DimSpec::Symbol("hidden".to_string())]),
                dtype: TensorDType::F32,
                init: InitStrategy::Zeros,
                trainable: true,
                regularization: None,
            },
        ];

        ln.hyperparameters = vec![HyperparameterSpec {
            name: "eps".to_string(),
            hparam_type: HyperparameterType::Float,
            default: HyperparameterValue::Float(1e-5),
            constraints: HyperparameterConstraint::FloatRange {
                min: 1e-12,
                max: 1e-3,
                log_scale: true,
            },
            description: "Small constant for numerical stability".to_string(),
        }];

        ln.fusable_with = vec![
            NeuralPrimitiveKind::Linear,
            NeuralPrimitiveKind::ReLU,
            NeuralPrimitiveKind::GeLU,
        ];

        self.primitives.insert(NeuralPrimitiveKind::LayerNorm, ln);
    }

    fn register_activations(&mut self) {
        // ReLU
        let mut relu = NeuralPrimitive::new(NeuralPrimitiveKind::ReLU, "ReLU");
        relu.properties = vec![AlgebraicProperty::Idempotent, AlgebraicProperty::Monotonic];
        relu.gradient_info = GradientInfo {
            is_differentiable: true,
            has_custom_vjp: false,
            has_custom_jvp: false,
            gradient_formula: Some("x > 0 ? 1 : 0".to_string()),
            hessian_structure: None,
            is_twice_differentiable: false,
        };
        self.primitives.insert(NeuralPrimitiveKind::ReLU, relu);

        // GELU
        let mut gelu = NeuralPrimitive::new(NeuralPrimitiveKind::GeLU, "GeLU");
        gelu.properties = vec![AlgebraicProperty::Monotonic, AlgebraicProperty::Continuous];
        gelu.gradient_info = GradientInfo {
            is_differentiable: true,
            has_custom_vjp: false,
            has_custom_jvp: false,
            gradient_formula: Some(
                "0.5 * (1 + erf(x/sqrt(2))) + x * exp(-x^2/2) / sqrt(2*pi)".to_string(),
            ),
            hessian_structure: None,
            is_twice_differentiable: true,
        };
        self.primitives.insert(NeuralPrimitiveKind::GeLU, gelu);

        // Softmax
        let mut softmax = NeuralPrimitive::new(NeuralPrimitiveKind::Softmax, "Softmax");
        softmax.output.constraints = vec![TensorConstraint::Probability];
        softmax.stability_notes = vec![StabilityNote {
            condition: "max(input) > 80".to_string(),
            description: "Exponential overflow in FP32".to_string(),
            severity: StabilitySeverity::Critical,
            mitigation: "Subtract max from input before exp".to_string(),
        }];
        self.primitives
            .insert(NeuralPrimitiveKind::Softmax, softmax);
    }

    /// Get a primitive by kind
    pub fn get(&self, kind: NeuralPrimitiveKind) -> Option<&NeuralPrimitive> {
        self.primitives.get(&kind)
    }

    /// Get all primitives
    pub fn all(&self) -> impl Iterator<Item = &NeuralPrimitive> {
        self.primitives.values()
    }

    /// Find primitives with specific properties
    pub fn find_by_property(&self, property: AlgebraicProperty) -> Vec<&NeuralPrimitive> {
        self.primitives
            .values()
            .filter(|p| p.properties.contains(&property))
            .collect()
    }

    /// Find primitives that can be fused with a given primitive
    pub fn find_fusable(&self, kind: NeuralPrimitiveKind) -> Vec<&NeuralPrimitive> {
        if let Some(primitive) = self.primitives.get(&kind) {
            primitive
                .fusable_with
                .iter()
                .filter_map(|k| self.primitives.get(k))
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for NeuralPrimitiveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = NeuralPrimitiveRegistry::new();
        assert!(registry.get(NeuralPrimitiveKind::Linear).is_some());
        assert!(registry.get(NeuralPrimitiveKind::ReLU).is_some());
    }

    #[test]
    fn test_flops_estimation() {
        let registry = NeuralPrimitiveRegistry::new();
        let linear = registry.get(NeuralPrimitiveKind::Linear).unwrap();

        let mut bindings = HashMap::new();
        bindings.insert("batch".to_string(), 32);
        bindings.insert("seq".to_string(), 512);
        bindings.insert("in_features".to_string(), 768);
        bindings.insert("out_features".to_string(), 3072);

        let flops = linear.estimate_flops(&bindings).unwrap();
        // 2 * 32 * 512 * 768 * 3072 = 77,309,411,328
        assert!(flops > 77e9);
    }

    #[test]
    fn test_shape_resolution() {
        let shape = ShapeSpec::new(vec![
            DimSpec::Symbol("batch".to_string()),
            DimSpec::Fixed(512),
            DimSpec::Symbol("hidden".to_string()),
        ]);

        let mut bindings = HashMap::new();
        bindings.insert("batch".to_string(), 32);
        bindings.insert("hidden".to_string(), 768);

        let resolved = shape.resolve(&bindings).unwrap();
        assert_eq!(resolved, vec![32, 512, 768]);
    }

    #[test]
    fn test_dim_expr_evaluate() {
        let mut bindings = HashMap::new();
        bindings.insert("batch".to_string(), 32);
        bindings.insert("seq".to_string(), 128);

        let expr = DimExpr::Mul(
            Box::new(DimExpr::Var("batch".to_string())),
            Box::new(DimExpr::Var("seq".to_string())),
        );
        assert_eq!(expr.evaluate(&bindings), Some(4096));

        let add = DimExpr::Add(
            Box::new(DimExpr::Const(10)),
            Box::new(DimExpr::Var("batch".to_string())),
        );
        assert_eq!(add.evaluate(&bindings), Some(42));

        let div = DimExpr::FloorDiv(Box::new(DimExpr::Const(100)), Box::new(DimExpr::Const(3)));
        assert_eq!(div.evaluate(&bindings), Some(33));
    }

    #[test]
    fn test_dim_spec_resolve_variants() {
        let bindings: HashMap<String, usize> = [("n".to_string(), 64)].into_iter().collect();

        assert_eq!(DimSpec::Fixed(10).resolve(&bindings), Some(10));
        assert_eq!(
            DimSpec::Symbol("n".to_string()).resolve(&bindings),
            Some(64)
        );
        assert_eq!(DimSpec::Dynamic.resolve(&bindings), None);

        let computed = DimSpec::Computed(DimExpr::Mul(
            Box::new(DimExpr::Var("n".to_string())),
            Box::new(DimExpr::Const(2)),
        ));
        assert_eq!(computed.resolve(&bindings), Some(128));
    }

    #[test]
    fn test_shape_spec_from_concrete() {
        let shape = ShapeSpec::from_concrete(&[32, 128, 768]);
        assert_eq!(shape.rank(), 3);
        let bindings = HashMap::new();
        assert_eq!(shape.resolve(&bindings), Some(vec![32, 128, 768]));
    }

    #[test]
    fn test_compute_formula_evaluate() {
        let bindings: HashMap<String, usize> = [
            ("m".to_string(), 32),
            ("n".to_string(), 64),
            ("k".to_string(), 128),
        ]
        .into_iter()
        .collect();

        let product = ComputeFormula::Product(vec![
            ComputeFormula::Var("m".to_string()),
            ComputeFormula::Var("n".to_string()),
            ComputeFormula::Var("k".to_string()),
        ]);
        assert_eq!(product.evaluate(&bindings), Some(32.0 * 64.0 * 128.0));

        let coeff = ComputeFormula::Coeff(2.0, Box::new(ComputeFormula::Var("m".to_string())));
        assert_eq!(coeff.evaluate(&bindings), Some(64.0));
    }

    #[test]
    fn test_registry_find_by_property() {
        let registry = NeuralPrimitiveRegistry::new();
        let elementwise = registry.find_by_property(AlgebraicProperty::Associative);
        // find_by_property should return a valid vec (possibly empty)
        let _ = elementwise.len();
    }

    #[test]
    fn test_primitive_estimate_memory() {
        let registry = NeuralPrimitiveRegistry::new();
        let linear = registry.get(NeuralPrimitiveKind::Linear).unwrap();

        let mut bindings = HashMap::new();
        bindings.insert("batch".to_string(), 32);
        bindings.insert("seq".to_string(), 512);
        bindings.insert("in_features".to_string(), 768);
        bindings.insert("out_features".to_string(), 3072);

        // estimate_memory should return Some value (may be 0 if no formulas)
        let mem = linear.estimate_memory(&bindings);
        // Just verify it doesn't panic and returns a valid option
        assert!(mem.is_none() || mem.unwrap() >= 0.0);
    }
}
