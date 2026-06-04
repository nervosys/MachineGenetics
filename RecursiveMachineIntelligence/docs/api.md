# RecursiveMachineIntelligence API Reference

This document provides a comprehensive reference for the RecursiveMachineIntelligence public API.

---

## Table of Contents

1. [Compute Module](#compute-module)
   - [BLAS Operations](#blas-operations)
   - [Kernel Fusion](#kernel-fusion)
2. [Neural Module](#neural-module)
3. [Symbolic Module](#symbolic-module)
4. [Neurosymbolic Module](#neurosymbolic-module)
5. [Core Module](#core-module)
   - [Agent](#agent)
   - [Protocol](#protocol)
   - [Storage](#storage)
   - [Message Bus](#message-bus)
   - [Ontology](#ontology)
   - [Optimization](#optimization)
6. [Lang Module](#lang-module)
   - [JIT Compiler](#jit-compiler)
   - [FFI Bridge](#ffi-bridge)
   - [LSP Server](#lsp-server)
   - [Op Registry](#op-registry)
7. [Knowledge Module](#knowledge-module)

---

## Compute Module

**Module:** `framewerx::compute`

### Types

#### `DType`

Tensor data types.

```rust
pub enum DType {
    F32,   // 32-bit float
    F64,   // 64-bit float
    F16,   // 16-bit float (half precision)
    BF16,  // 16-bit bfloat
    I32,   // 32-bit integer
    I64,   // 64-bit integer
    U8,    // 8-bit unsigned
    Bool,  // Boolean
}
```

#### `TensorHandle`

Opaque handle to tensor data.

```rust
pub struct TensorHandle {
    pub id: u64,
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub backend: BackendType,
    pub size_bytes: usize,
}

impl TensorHandle {
    pub fn numel(&self) -> usize;  // Total elements
    pub fn ndim(&self) -> usize;   // Number of dimensions
}
```

#### `DeviceInfo`

Information about compute device.

```rust
pub struct DeviceInfo {
    pub name: String,
    pub backend_type: BackendType,
    pub total_memory: u64,
    pub available_memory: u64,
    pub compute_capability: Option<(u32, u32)>,
    pub compute_units: u32,
}
```

### Traits

#### `Backend`

Core compute backend interface.

```rust
pub trait Backend: Send + Sync {
    // Info
    fn backend_type(&self) -> BackendType;
    fn device_info(&self) -> &DeviceInfo;
    fn is_available(&self) -> bool;

    // Memory
    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn free(&self, handle: &TensorHandle) -> Result<()>;
    fn copy_to_device(&self, handle: &TensorHandle, data: &[u8]) -> Result<()>;
    fn copy_to_host(&self, handle: &TensorHandle) -> Result<Vec<u8>>;
    fn copy(&self, src: &TensorHandle, dst: &TensorHandle) -> Result<()>;

    // Creation
    fn zeros(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn ones(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn rand(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn randn(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn from_slice_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle>;

    // Arithmetic
    fn add(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn sub(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn mul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn div(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn scale(&self, a: &TensorHandle, scalar: f64) -> Result<TensorHandle>;

    // Reductions
    fn sum(&self, a: &TensorHandle) -> Result<f64>;
    fn sum_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle>;
    fn mean(&self, a: &TensorHandle) -> Result<f64>;
    fn mean_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle>;
    fn max(&self, a: &TensorHandle) -> Result<f64>;
    fn min(&self, a: &TensorHandle) -> Result<f64>;

    // Activations
    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle>;
    fn gelu(&self, a: &TensorHandle) -> Result<TensorHandle>;
    fn sigmoid(&self, a: &TensorHandle) -> Result<TensorHandle>;
    fn tanh(&self, a: &TensorHandle) -> Result<TensorHandle>;
    fn softmax(&self, a: &TensorHandle, axis: i32) -> Result<TensorHandle>;

    // Shape
    fn reshape(&self, a: &TensorHandle, new_shape: &[usize]) -> Result<TensorHandle>;
    fn transpose(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle>;
    fn concat(&self, tensors: &[&TensorHandle], axis: usize) -> Result<TensorHandle>;
    fn split(&self, a: &TensorHandle, axis: usize, sections: usize) -> Result<Vec<TensorHandle>>;

    // Sync
    fn synchronize(&self) -> Result<()>;
}
```

### Functions

```rust
/// Get the best available backend (prefers CUDA if available)
pub fn get_backend() -> Arc<dyn Backend>;

/// Get a specific backend
pub fn get_backend_by_type(backend_type: BackendType) -> Result<Arc<dyn Backend>>;
```

### Backends

#### `CpuBackend`

CPU compute backend using ndarray + rayon.

```rust
impl CpuBackend {
    pub fn new() -> Self;
}
```

#### `CudaBackend` (feature = "cuda")

CUDA GPU backend using cudarc.

```rust
impl CudaBackend {
    pub fn new() -> Result<Self>;
    pub fn with_device(device_id: usize) -> Result<Self>;
}
```

### BLAS Operations

**Module:** `framewerx::compute::blas`

Pure-Rust BLAS (Basic Linear Algebra Subprograms) operating on f64 with tiled algorithms.

```rust
pub struct BlasMatrix { pub rows: usize, pub cols: usize, pub data: Vec<f64> }

impl BlasOps {
    pub fn matmul(a: &BlasMatrix, b: &BlasMatrix) -> Result<BlasMatrix, BlasError>;
    pub fn matvec(a: &BlasMatrix, x: &[f64]) -> Result<Vec<f64>, BlasError>;
    pub fn dot(a: &[f64], b: &[f64]) -> Result<f64, BlasError>;
    pub fn lu(a: &BlasMatrix) -> Result<(BlasMatrix, BlasMatrix, Vec<usize>), BlasError>;
    pub fn cholesky(a: &BlasMatrix) -> Result<BlasMatrix, BlasError>;
    pub fn qr(a: &BlasMatrix) -> Result<(BlasMatrix, BlasMatrix), BlasError>;
    pub fn solve(a: &BlasMatrix, b: &[f64]) -> Result<Vec<f64>, BlasError>;
    pub fn inv(a: &BlasMatrix) -> Result<BlasMatrix, BlasError>;
    pub fn det(a: &BlasMatrix) -> Result<f64, BlasError>;
    pub fn norm2(v: &[f64]) -> f64;
    pub fn outer(a: &[f64], b: &[f64]) -> BlasMatrix;
    pub fn transpose(a: &BlasMatrix) -> BlasMatrix;
}
```

The CPU backend (`CpuBackend`) automatically routes matrices ≥32×32 through BLAS for tiled matmul, and exposes `solve()`, `det()`, `inv()`, and `cholesky()` methods.

### Kernel Fusion

**Module:** `framewerx::compute::fusion`

Detects and rewrites fusible RMIL op sequences into fused kernels.

```rust
pub struct FusionConfig {
    pub max_fusion_length: usize,
    pub fuse_elementwise: bool,
    pub fuse_matmul_act: bool,
    pub fuse_norm_act: bool,
    pub fuse_reduce_ewise: bool,
}

pub struct FusionPass { /* ... */ }
impl FusionPass {
    pub fn new(config: FusionConfig) -> Self;
    pub fn fuse(&self, expr: &Expr) -> FusionResult;
}

pub struct FusionResult {
    pub output: Expr,
    pub fused_count: usize,
    pub ops_before: usize,
    pub ops_after: usize,
    pub kernels: Vec<FusedKernel>,
}

pub enum FusionPattern {
    ElementwiseChain,
    MatmulActivation,
    NormActivation,
    ReduceElementwise,
    GenericSeq,
}
```

Also available via the optimization pipeline as `RmilOptimizer` (see [Optimization](#optimization)).

### JIT Compiler

**Module:** `framewerx::lang::jit`

Compiles RMIL `Expr` trees into native `f64 → f64` functions at runtime.

```rust
pub struct JitConfig {
    pub max_depth: usize,
    pub cache_capacity: usize,
}

pub struct JitCompiler { /* ... */ }
impl JitCompiler {
    pub fn new(config: JitConfig) -> Self;
    pub fn compile(&self, expr: &Expr) -> Result<JitFunction, JitError>;
    pub fn compile_cached(&mut self, expr: &Expr) -> Result<&JitFunction, JitError>;
    pub fn cache_size(&self) -> usize;
    pub fn clear_cache(&mut self);
}

pub struct JitFunction { /* ... */ }
impl JitFunction {
    pub fn call_f64(&self, input: f64) -> f64;
}
```

The VM exposes `eval_jit()` which tries JIT first, then falls back to tree-walking.

### FFI Bridge

**Module:** `framewerx::lang::ffi`

Safe and unsafe foreign function interface for calling external C-ABI functions from RMIL.

```rust
pub struct FfiRegistry { /* ... */ }
impl FfiRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, name: &str, sig: FfiSignature, ptr: FfiFuncPtr) -> Result<(), FfiError>;
    pub fn call(&self, name: &str, args: &[FfiValue]) -> Result<FfiValue, FfiError>;
    pub unsafe fn call_unchecked(&self, name: &str, args: &[FfiValue]) -> FfiValue;
    pub fn list_functions(&self) -> Vec<(&str, &FfiSignature)>;
}

pub enum FfiValue { F64(f64), I64(i64), Bool(bool), Ptr(*mut u8), Void }
pub enum FfiType { F64, I64, Bool, Ptr, Void }
pub struct FfiSignature { pub args: Vec<FfiType>, pub ret: FfiType }
```

### LSP Server

**Module:** `framewerx::lang::lsp`

Language Server Protocol implementation for RMIL source files.

```rust
pub struct RmilLanguageServer { /* ... */ }
impl RmilLanguageServer {
    pub fn new() -> Self;
    pub fn completion(&self, source: &str, pos: Position) -> Vec<CompletionItem>;
    pub fn hover(&self, source: &str, pos: Position) -> Option<HoverInfo>;
    pub fn diagnostics(&self, source: &str) -> Vec<Diagnostic>;
    pub fn definition(&self, source: &str, pos: Position) -> Option<Location>;
    pub fn references(&self, source: &str, pos: Position) -> Vec<Location>;
    pub fn document_symbols(&self, source: &str) -> Vec<DocumentSymbol>;
}
```

### Op Registry

**Module:** `framewerx::lang::registry`

Runtime-extensible operation registry allowing user-defined ops.

```rust
pub struct OpRegistry { /* ... */ }
impl OpRegistry {
    pub fn new() -> Self;
    pub fn with_builtins() -> Self;
    pub fn register(&mut self, meta: OpMeta) -> Result<RegisteredOp, RegistryError>;
    pub fn get(&self, name: &str) -> Option<&RegisteredOp>;
    pub fn lookup_by_tag(&self, tag: &str) -> Vec<&RegisteredOp>;
    pub fn all_ops(&self) -> Vec<&RegisteredOp>;
    pub fn len(&self) -> usize;
}

pub struct RegisteredOp { pub op: Op, pub meta: OpMeta, pub version: u32 }
```

---

## Neural Module

**Module:** `framewerx::neural`

### Core Types

#### `Variable`

Differentiable tensor value.

```rust
pub struct Variable {
    pub id: u64,
    pub data: Vec<f32>,
    pub shape: Vec<usize>,
    pub requires_grad: bool,
}

impl Variable {
    pub fn new(data: Vec<f32>, shape: Vec<usize>, requires_grad: bool) -> Self;
    pub fn numel(&self) -> usize;
    pub fn zeros(shape: &[usize], requires_grad: bool) -> Self;
    pub fn ones(shape: &[usize], requires_grad: bool) -> Self;
}
```

#### `GradientTape`

Records operations for automatic differentiation.

```rust
pub struct GradientTape {
    // Internal
}

impl GradientTape {
    pub fn new() -> Self;
    pub fn register(&mut self, var: Variable) -> VariableId;
}
```

### Layer Trait

```rust
pub trait Layer: Send + Sync {
    fn name(&self) -> &str;
    fn forward(&self, inputs: &[&Variable], tape: &mut GradientTape) -> Variable;
    fn parameters(&self) -> Vec<&Variable>;
    fn parameters_mut(&mut self) -> Vec<&mut Variable>;
    fn num_parameters(&self) -> usize;
    fn set_trainable(&mut self, trainable: bool);
    fn reset_parameters(&mut self);
}
```

### Standard Layers

#### `Linear`

Fully connected layer.

```rust
impl Linear {
    pub fn new(in_features: usize, out_features: usize) -> Self;
    pub fn without_bias(in_features: usize, out_features: usize) -> Self;
    pub fn in_features(&self) -> usize;
    pub fn out_features(&self) -> usize;
}
```

#### `Conv2d`

2D convolution layer.

```rust
impl Conv2d {
    pub fn new(in_channels: usize, out_channels: usize, kernel_size: (usize, usize)) -> Self;
    pub fn with_stride(self, stride: (usize, usize)) -> Self;
    pub fn with_padding(self, padding: (usize, usize)) -> Self;
    pub fn output_height(&self, input_height: usize) -> usize;
    pub fn output_width(&self, input_width: usize) -> usize;
}
```

#### `MultiHeadAttention`

Multi-head self-attention.

```rust
impl MultiHeadAttention {
    pub fn new(embed_dim: usize, num_heads: usize) -> Self;
}
```

### Normalization Layers

#### `LayerNorm`

Layer normalization.

```rust
impl LayerNorm {
    pub fn new(normalized_shape: Vec<usize>) -> Self;
    pub fn with_eps(self, eps: f32) -> Self;
}
```

#### `BatchNorm`

Batch normalization.

```rust
impl BatchNorm {
    pub fn new(num_features: usize) -> Self;
    pub fn with_eps(self, eps: f32) -> Self;
    pub fn with_momentum(self, momentum: f32) -> Self;
    pub fn train(&mut self, mode: bool);
}
```

#### `GroupNorm`

Group normalization.

```rust
impl GroupNorm {
    pub fn new(num_groups: usize, num_channels: usize) -> Self;
    pub fn with_eps(self, eps: f32) -> Self;
}
```

#### `RMSNorm`

RMS normalization (used in LLaMA).

```rust
impl RMSNorm {
    pub fn new(dim: usize) -> Self;
    pub fn with_eps(self, eps: f32) -> Self;
}
```

### Recurrent Layers

#### `LSTMCell`

LSTM cell for a single timestep.

```rust
impl LSTMCell {
    pub fn new(input_size: usize, hidden_size: usize) -> Self;
    pub fn forward_step(&self, x: &Variable, hx: &Variable, cx: &Variable) -> (Variable, Variable);
    pub fn hidden_size(&self) -> usize;
}
```

#### `GRUCell`

GRU cell for a single timestep.

```rust
impl GRUCell {
    pub fn new(input_size: usize, hidden_size: usize) -> Self;
    pub fn forward_step(&self, x: &Variable, hx: &Variable) -> Variable;
    pub fn hidden_size(&self) -> usize;
}
```

### Utility Layers

#### `Dropout`

Dropout regularization.

```rust
impl Dropout {
    pub fn new(p: f32) -> Self;  // p in [0, 1)
    pub fn train(&mut self, mode: bool);
}
```

#### `Embedding`

Token embedding layer.

```rust
impl Embedding {
    pub fn new(num_embeddings: usize, embedding_dim: usize) -> Self;
    pub fn with_padding_idx(self, idx: usize) -> Self;
    pub fn lookup(&self, indices: &[usize]) -> Variable;
}
```

#### `FeedForward`

Transformer feed-forward network.

```rust
impl FeedForward {
    pub fn new(d_model: usize, d_ff: usize, dropout: f32) -> Self;
    pub fn with_activation(self, activation: Activation) -> Self;
}
```

#### `ResidualBlock`

Residual connection with layer norm.

```rust
impl ResidualBlock {
    pub fn new(dim: usize, dropout: f32) -> Self;
    pub fn with_post_norm(self) -> Self;
    pub fn forward_with<F>(&self, x: &Variable, sublayer: F, tape: &mut GradientTape) -> Variable
    where F: FnOnce(&Variable, &mut GradientTape) -> Variable;
}
```

### Architecture

#### `NetworkArchitecture`

DAG representation of neural network.

```rust
impl NetworkArchitecture {
    pub fn new(name: &str) -> Self;
    pub fn name(&self) -> &str;
    pub fn add_node(&mut self, node: ArchitectureNode) -> NodeId;
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, edge: ArchitectureEdge);
    pub fn nodes(&self) -> impl Iterator<Item = &ArchitectureNode>;
    pub fn topological_order(&self) -> Vec<NodeId>;
}
```

#### `ArchitectureBuilder`

Builder pattern for architectures.

```rust
impl ArchitectureBuilder {
    pub fn new(name: &str) -> Self;
    pub fn add_layer(self, layer: LayerSpec) -> Self;
    pub fn add_skip_connection(self, from: usize, to: usize) -> Self;
    pub fn build(self) -> NetworkArchitecture;
}
```

### Functions

```rust
/// Compute gradients via backpropagation
pub fn backward(tape: &GradientTape, loss: &Variable) -> HashMap<VariableId, Variable>;

/// Compute gradient of output w.r.t. specific variable
pub fn grad(tape: &GradientTape, output: &Variable, var: &Variable) -> Variable;
```

---

## Symbolic Module

**Module:** `framewerx::symbolic`

### Logic Types

#### `Term`

Logical term.

```rust
pub enum Term {
    Variable(String),
    Symbol(String),
    Function(String, Vec<Term>),
    List(Vec<Term>),
}

impl Term {
    pub fn variable(name: &str) -> Self;
    pub fn symbol(name: &str) -> Self;
    pub fn function(name: &str, args: Vec<Term>) -> Self;
    pub fn list(terms: Vec<Term>) -> Self;
    pub fn is_variable(&self) -> bool;
    pub fn is_ground(&self) -> bool;
    pub fn variables(&self) -> HashSet<String>;
}
```

#### `Predicate`

Logical predicate.

```rust
pub struct Predicate {
    pub name: String,
    pub args: Vec<Term>,
}

impl Predicate {
    pub fn new(name: &str, args: Vec<Term>) -> Self;
    pub fn arity(&self) -> usize;
    pub fn is_ground(&self) -> bool;
}
```

#### `Literal`

Positive or negated predicate.

```rust
pub struct Literal {
    pub predicate: Predicate,
    pub negated: bool,
}

impl Literal {
    pub fn positive(predicate: Predicate) -> Self;
    pub fn negative(predicate: Predicate) -> Self;
}
```

#### `Clause`

Horn clause (rule or fact).

```rust
pub struct Clause {
    pub head: Option<Predicate>,
    pub body: Vec<Literal>,
}

impl Clause {
    pub fn fact(predicate: Predicate) -> Self;
    pub fn rule(head: Predicate, body: Vec<Literal>) -> Self;
    pub fn is_fact(&self) -> bool;
    pub fn is_rule(&self) -> bool;
}
```

#### `KnowledgeBase`

Collection of clauses.

```rust
pub struct KnowledgeBase {
    // Internal
}

impl KnowledgeBase {
    pub fn new() -> Self;
    pub fn add_fact(&mut self, clause: Clause);
    pub fn add_rule(&mut self, clause: Clause);
    pub fn facts(&self) -> &[Clause];
    pub fn rules(&self) -> &[Clause];
    pub fn query(&self, predicate: &Predicate) -> Vec<&Clause>;
}
```

### Unification

#### `Substitution`

Variable bindings.

```rust
pub struct Substitution {
    pub bindings: HashMap<String, Term>,
}

impl Substitution {
    pub fn empty() -> Self;
    pub fn bind(&mut self, var: &str, term: Term);
    pub fn lookup(&self, var: &str) -> Option<&Term>;
    pub fn apply(&self, term: &Term) -> Term;
    pub fn compose(&self, other: &Substitution) -> Substitution;
}
```

#### Functions

```rust
/// Robinson's unification algorithm
pub fn unify(a: &Term, b: &Term) -> Option<Substitution>;

/// Unify two predicates
pub fn unify_predicates(a: &Predicate, b: &Predicate) -> Option<Substitution>;

/// Anti-unification (least general generalization)
pub fn anti_unify(a: &Term, b: &Term) -> Term;
```

### Inference

#### `InferenceEngine`

Forward and backward chaining.

```rust
pub struct InferenceEngine {
    pub config: InferenceConfig,
}

impl InferenceEngine {
    pub fn new(config: InferenceConfig) -> Self;
    pub fn forward_chain(&mut self, kb: &KnowledgeBase) -> Vec<Clause>;
    pub fn backward_chain(&mut self, kb: &KnowledgeBase, goal: &Predicate) -> bool;
    pub fn query(&mut self, kb: &KnowledgeBase, goal: &Predicate) -> Vec<Substitution>;
}
```

#### `InferenceConfig`

```rust
pub struct InferenceConfig {
    pub max_depth: usize,
    pub max_iterations: usize,
    pub timeout_ms: Option<u64>,
}
```

### Planning

#### `Action`

STRIPS action schema.

```rust
pub struct Action {
    pub name: String,
    pub parameters: Vec<Term>,
    pub preconditions: Vec<Predicate>,
    pub add_effects: Vec<Predicate>,
    pub delete_effects: Vec<Predicate>,
}
```

#### `State`

Planning state.

```rust
pub struct State {
    pub predicates: HashSet<Predicate>,
}

impl State {
    pub fn new() -> Self;
    pub fn add(&mut self, predicate: Predicate);
    pub fn remove(&mut self, predicate: &Predicate);
    pub fn holds(&self, predicate: &Predicate) -> bool;
    pub fn satisfies(&self, goal: &State) -> bool;
}
```

#### Functions

```rust
/// Find a plan from initial to goal state
pub fn plan(
    initial: &State,
    goal: &State,
    actions: &[Action],
    max_depth: usize,
) -> Option<Vec<GroundAction>>;
```

---

## Neurosymbolic Module

**Module:** `framewerx::neurosymbolic`

### Symbol Embedding

#### `SymbolEmbedding`

Maps symbols to vectors.

```rust
pub struct SymbolEmbedding {
    // Internal
}

impl SymbolEmbedding {
    pub fn new(config: EmbeddingConfig) -> Self;
    pub fn embed(&mut self, symbol: &str) -> Vec<f64>;
    pub fn embed_predicate(&mut self, pred: &Predicate) -> Vec<f64>;
    pub fn similarity(&self, a: &str, b: &str) -> f64;
}
```

#### `EmbeddingConfig`

```rust
pub struct EmbeddingConfig {
    pub embedding_dim: usize,
    pub use_position_encoding: bool,
    pub normalize: bool,
}
```

### Differentiable Constraints

#### `SoftConstraint`

Differentiable constraint.

```rust
pub struct SoftConstraint {
    pub formula: ConstraintFormula,
    pub weight: f64,
    pub temperature: f64,
}

impl SoftConstraint {
    pub fn new(formula: ConstraintFormula, weight: f64, temperature: f64) -> Self;
    pub fn evaluate(&self, vars: &HashMap<String, f64>) -> f64;
    pub fn gradient(&self, vars: &HashMap<String, f64>) -> HashMap<String, f64>;
}
```

#### `ConstraintFormula`

Constraint expression DSL.

```rust
pub enum ConstraintFormula {
    Variable(String),
    Constant(f64),
    And(Box<ConstraintFormula>, Box<ConstraintFormula>),
    Or(Box<ConstraintFormula>, Box<ConstraintFormula>),
    Not(Box<ConstraintFormula>),
    Implies(Box<ConstraintFormula>, Box<ConstraintFormula>),
    Equals(Box<ConstraintFormula>, Box<ConstraintFormula>),
    LessThan(Box<ConstraintFormula>, Box<ConstraintFormula>),
    GreaterThan(Box<ConstraintFormula>, Box<ConstraintFormula>),
}
```

#### `ConstraintSolver`

Gradient-based constraint solver.

```rust
pub struct ConstraintSolver {
    pub learning_rate: f64,
    pub max_iterations: usize,
    pub tolerance: f64,
}

impl ConstraintSolver {
    pub fn new() -> Self;
    pub fn solve(
        &self,
        constraints: &[SoftConstraint],
        initial: HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>>;
}
```

### Hybrid Reasoning

#### `ReasoningMode`

```rust
pub enum ReasoningMode {
    Neural,     // Pure neural (embedding similarity)
    Symbolic,   // Pure symbolic (logic inference)
    Hybrid,     // Fixed combination
    Adaptive,   // Dynamic selection
}
```

#### `HybridReasoner`

Combines neural and symbolic reasoning.

```rust
pub struct HybridReasoner {
    pub config: HybridConfig,
}

impl HybridReasoner {
    pub fn new(config: HybridConfig) -> Self;
    pub fn query(&self, kb: &KnowledgeBase, query: &Predicate) -> HybridResult;
    pub fn query_with_embeddings(
        &self,
        kb: &KnowledgeBase,
        query: &Predicate,
        embedder: &mut SymbolEmbedding,
    ) -> HybridResult;
}
```

#### `HybridConfig`

```rust
pub struct HybridConfig {
    pub mode: ReasoningMode,
    pub neural_weight: f64,
    pub symbolic_weight: f64,
    pub temperature: f64,
    pub max_iterations: usize,
}
```

---

## Core Module

**Module:** `framewerx::core`

### Agent

#### `Agent`

Autonomous AI agent.

```rust
pub struct Agent {
    pub id: AgentId,
    pub config: AgentConfig,
    pub state: AgentState,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self;
    pub async fn execute(&self, goal: Goal) -> Result<ExecutionResult>;
    pub async fn send_message(&self, to: &AgentId, msg: Message) -> Result<()>;
    pub async fn receive_message(&self) -> Option<Message>;
}
```

#### `Goal`

Agent objective.

```rust
pub struct Goal {
    pub id: String,
    pub goal_type: GoalType,
    pub target: String,
    pub constraints: HashMap<String, f64>,
    pub priority: f64,
}

pub enum GoalType {
    Minimize,
    Maximize,
    Satisfy,
    Achieve,
}
```

### Protocol

#### `Protocol`

Binary communication protocol.

```rust
pub struct Protocol {
    pub config: ProtocolConfig,
}

impl Protocol {
    pub fn new(config: ProtocolConfig) -> Self;
    pub fn encode<T: Serialize>(&self, msg: &Message<T>) -> Result<Vec<u8>>;
    pub fn decode<T: DeserializeOwned>(&self, data: &[u8]) -> Result<Message<T>>;
}
```

#### `Message`

Protocol message.

```rust
pub struct Message<T> {
    pub id: MessageId,
    pub msg_type: MessageType,
    pub sender: AgentId,
    pub timestamp: DateTime<Utc>,
    pub payload: T,
    pub attachments: Vec<TensorAttachment>,
}

pub enum MessageType {
    Query,
    Result,
    GoalAssignment,
    TensorTransfer,
    CapabilityAdvertisement,
}
```

### Storage

#### `KeyValueStore`

High-performance key-value store with LRU caching and disk persistence.

```rust
pub struct KeyValueStore {
    cache: LruCache<String, Vec<u8>>,
    base_path: PathBuf,
    compression_enabled: bool,
}

impl KeyValueStore {
    pub fn new(base_path: PathBuf, cache_capacity: usize) -> Self;
    pub fn with_compression(base_path: PathBuf, cache_capacity: usize) -> Self;
    pub fn set<T: Serialize>(&mut self, key: &str, value: &T) -> Result<()>;
    pub fn get<T: DeserializeOwned>(&mut self, key: &str) -> Result<Option<T>>;
    pub fn delete(&mut self, key: &str) -> Result<bool>;
    pub fn contains(&self, key: &str) -> bool;
    pub fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>>;
    pub fn clear_cache(&mut self);
}
```

#### `TensorStorage`

Efficient binary tensor storage format (similar to safetensors).

```rust
pub struct TensorStorage {
    index: HashMap<String, TensorIndexEntry>,
    data_file: File,
}

pub struct TensorIndexEntry {
    pub name: String,
    pub shape: Vec<usize>,
    pub dtype: StorageDataType,
    pub offset: u64,
    pub length: u64,
    pub checksum: u64,
}

impl TensorStorage {
    pub fn create(path: &Path) -> Result<Self>;
    pub fn open(path: &Path) -> Result<Self>;
    pub fn write_tensor(&mut self, name: &str, data: &[u8], shape: &[usize], dtype: StorageDataType) -> Result<()>;
    pub fn read_tensor(&self, name: &str) -> Result<(Vec<u8>, Vec<usize>, StorageDataType)>;
    pub fn list_tensors(&self) -> Vec<&TensorIndexEntry>;
    pub fn contains(&self, name: &str) -> bool;
}
```

#### `CheckpointManager`

Model and agent state checkpointing with versioning.

```rust
pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    max_checkpoints: usize,
    checkpoints: Vec<CheckpointMeta>,
}

pub struct CheckpointMeta {
    pub id: String,
    pub checkpoint_type: CheckpointType,
    pub created_at: SystemTime,
    pub size_bytes: u64,
    pub metrics: HashMap<String, f64>,
    pub parent_id: Option<String>,
    pub path: PathBuf,
}

impl CheckpointManager {
    pub fn new(checkpoint_dir: PathBuf, max_checkpoints: usize) -> Result<Self>;
    pub fn save_checkpoint<T: Serialize>(&mut self, checkpoint_type: CheckpointType, data: &T, metrics: HashMap<String, f64>) -> Result<String>;
    pub fn load_checkpoint<T: DeserializeOwned>(&self, id: &str) -> Result<T>;
    pub fn list_checkpoints(&self) -> &[CheckpointMeta];
    pub fn get_latest(&self, checkpoint_type: Option<CheckpointType>) -> Option<&CheckpointMeta>;
    pub fn delete_checkpoint(&mut self, id: &str) -> Result<bool>;
}
```

#### `ConsistentHashRing`

Distributed storage with consistent hashing for horizontal scaling.

```rust
pub struct ConsistentHashRing {
    ring: BTreeMap<u64, String>,
    nodes: HashMap<String, ShardInfo>,
    virtual_nodes: usize,
}

pub struct ShardInfo {
    pub id: String,
    pub address: String,
    pub capacity_bytes: u64,
    pub used_bytes: u64,
    pub status: ShardStatus,
}

impl ConsistentHashRing {
    pub fn new(virtual_nodes: usize) -> Self;
    pub fn add_node(&mut self, info: ShardInfo);
    pub fn remove_node(&mut self, node_id: &str);
    pub fn get_node(&self, key: &str) -> Option<&ShardInfo>;
    pub fn get_nodes_for_replication(&self, key: &str, count: usize) -> Vec<&ShardInfo>;
}
```

### Message Bus

#### `Topic`

Hierarchical topic for pub/sub messaging with wildcard support.

```rust
pub struct Topic {
    segments: Vec<String>,
}

impl Topic {
    pub fn new(path: &str) -> Self;           // "agent.task.compute"
    pub fn matches(&self, pattern: &Topic) -> bool;
    pub fn as_string(&self) -> String;
}

// Wildcard patterns:
// - "*" matches exactly one segment: "agent.*.compute"
// - "#" matches zero or more segments: "agent.#"
```

#### `Envelope`

Message wrapper with routing metadata.

```rust
pub struct Envelope<T> {
    pub id: u64,
    pub topic: Topic,
    pub payload: T,
    pub sender: Option<String>,
    pub timestamp: SystemTime,
    pub priority: u8,           // 0 = lowest, 255 = highest
    pub ttl: Option<Duration>,
    pub correlation_id: Option<u64>,
    pub reply_to: Option<Topic>,
}

impl<T: Serialize + DeserializeOwned> Envelope<T> {
    pub fn new(topic: Topic, payload: T) -> Self;
    pub fn with_priority(mut self, priority: u8) -> Self;
    pub fn with_ttl(mut self, ttl: Duration) -> Self;
    pub fn with_correlation(mut self, correlation_id: u64) -> Self;
    pub fn with_reply_to(mut self, reply_to: Topic) -> Self;
    pub fn is_expired(&self) -> bool;
}
```

#### `MessageBus`

Central pub/sub message bus with request/reply support.

```rust
pub struct MessageBus {
    subscriptions: HashMap<String, Vec<Subscription>>,
    dead_letter_queue: DeadLetterQueue,
    stats: BusStats,
}

impl MessageBus {
    pub fn new() -> Self;
    pub fn subscribe(&mut self, pattern: &str, filter: Option<fn(&[u8]) -> bool>) -> String;
    pub fn unsubscribe(&mut self, subscription_id: &str) -> bool;
    pub async fn publish<T: Serialize>(&mut self, envelope: Envelope<T>) -> Result<()>;
    pub async fn request<T, R>(&mut self, envelope: Envelope<T>, timeout: Duration) -> Result<R>
        where T: Serialize, R: DeserializeOwned;
    pub fn stats(&self) -> &BusStats;
}
```

#### `Communicator` Trait

Interface for agents to communicate via the message bus.

```rust
pub trait Communicator: Send + Sync {
    fn agent_id(&self) -> &str;
    fn send(&self, topic: &Topic, payload: Vec<u8>) -> Result<()>;
    fn receive(&self) -> Result<Option<Envelope<Vec<u8>>>>;
    fn subscribe(&self, pattern: &str) -> Result<String>;
    fn unsubscribe(&self, subscription_id: &str) -> Result<()>;
}
```

#### Standard Topics

```rust
pub mod topics {
    // Agent lifecycle
    pub const AGENT_STARTED: &str = "agent.started";
    pub const AGENT_STOPPED: &str = "agent.stopped";
    pub const AGENT_HEARTBEAT: &str = "agent.heartbeat";
    
    // Task management
    pub const TASK_ASSIGNED: &str = "task.assigned";
    pub const TASK_COMPLETED: &str = "task.completed";
    pub const TASK_FAILED: &str = "task.failed";
    
    // Data exchange
    pub const DATA_UPDATED: &str = "data.updated";
    pub const DATA_REQUESTED: &str = "data.requested";
    pub const DATA_SHARED: &str = "data.shared";
    
    // Consensus
    pub const CONSENSUS_PROPOSE: &str = "consensus.propose";
    pub const CONSENSUS_VOTE: &str = "consensus.vote";
    pub const CONSENSUS_COMMIT: &str = "consensus.commit";
    
    // Monitoring
    pub const MONITOR_METRICS: &str = "monitor.metrics";
    pub const MONITOR_ALERT: &str = "monitor.alert";
}
```

### Ontology

#### `Ontology`

Machine-readable concept graph.

```rust
pub struct Ontology {
    // Internal
}

impl Ontology {
    pub fn new() -> Self;
    pub fn load(path: &str) -> Result<Self>;
    pub fn add_concept(&mut self, concept: Concept);
    pub fn add_relation(&mut self, from: &str, to: &str, relation: Relation);
    pub fn get_concept(&self, name: &str) -> Option<&Concept>;
    pub fn related_concepts(&self, name: &str, relation: Relation) -> Vec<&Concept>;
    pub fn similarity(&self, a: &str, b: &str) -> f64;
}
```

### Optimization

#### `OptimizationPipeline` (IR-level)

Chains IR optimization passes with fixed-point iteration.

```rust
pub struct OptimizationPipeline { /* ... */ }
impl OptimizationPipeline {
    pub fn new() -> Self;
    pub fn level(level: OptimizationLevel) -> Self;   // O0, O1, O2, O3 presets
    pub fn add_pass(&mut self, pass: impl OptimizationPass + 'static);
    pub fn max_iterations(self, n: usize) -> Self;
    pub fn pass_names(&self) -> Vec<String>;
    pub fn optimize(&self, program: Program) -> Program;
}

pub enum OptimizationLevel { O0, O1, O2, O3 }
```

**Included passes:** `ConstantFolding`, `DeadCodeElimination`, `CommonSubexpressionElimination`, `OperatorFusion`, `StrengthReduction`, `AlgebraicSimplification`.

#### `RmilOptimizer` (RMIL-level)

Applies RMIL-level optimizations (kernel fusion, etc.) to `Expr` trees before evaluation or IR lowering.

```rust
pub struct RmilOptimizer { /* ... */ }
impl RmilOptimizer {
    pub fn default() -> Self;                        // fusion with default config
    pub fn with_fusion(config: FusionConfig) -> Self;
    pub fn none() -> Self;                           // identity (no passes)
    pub fn add_pass(&mut self, pass: impl RmilPass + 'static);
    pub fn pass_names(&self) -> Vec<String>;
    pub fn optimize_expr(&self, expr: &Expr) -> (Expr, RmilOptStats);
    pub fn fuse(&self, expr: &Expr) -> Expr;         // convenience
}

pub struct RmilOptStats {
    pub ops_before: usize,
    pub ops_after: usize,
    pub fused_kernels: usize,
    pub fusion_detail: Option<FusionResult>,
}
```

---

## Knowledge Module

**Module:** `framewerx::knowledge`

### AI History

#### `AIHistoryKB`

Database of AI contributions.

```rust
pub struct AIHistoryKB {
    // Internal
}

impl AIHistoryKB {
    pub fn new() -> Self;
    pub fn all_contributions(&self) -> &[AIContribution];
    pub fn by_year(&self, year: u32) -> Vec<&AIContribution>;
    pub fn by_era(&self, era: AIEra) -> Vec<&AIContribution>;
    pub fn by_category(&self, category: ContributionCategory) -> Vec<&AIContribution>;
    pub fn by_concept(&self, concept: &str) -> Vec<&AIContribution>;
    pub fn by_author(&self, author: &str) -> Vec<&AIContribution>;
    pub fn lineage(&self, title: &str) -> Vec<&AIContribution>;
}
```

#### `AIContribution`

Single contribution entry.

```rust
pub struct AIContribution {
    pub title: String,
    pub authors: Vec<String>,
    pub year: u32,
    pub era: AIEra,
    pub category: ContributionCategory,
    pub key_concepts: Vec<String>,
    pub equations: Vec<String>,
    pub builds_on: Vec<String>,
    pub abstract_summary: String,
}
```

#### `AIEra`

Historical eras.

```rust
pub enum AIEra {
    Foundations,           // 1943-1956
    SymbolicAI,           // 1956-1974
    AIWinter1,            // 1974-1980
    ExpertSystems,        // 1980-1987
    AIWinter2,            // 1987-1993
    StatisticalML,        // 1993-2006
    DeepLearning,         // 2006-2017
    TransformerEra,       // 2017-2020
    LargeLanguageModels,  // 2020-2023
    MultimodalAI,         // 2023+
}
```

### AI Concepts

#### `AIConceptsOntology`

Ontology of AI concepts.

```rust
pub struct AIConceptsOntology {
    // Internal
}

impl AIConceptsOntology {
    pub fn new() -> Self;
    pub fn get_concept(&self, name: &str) -> Option<&AIConcept>;
    pub fn by_domain(&self, domain: ConceptDomain) -> Vec<&AIConcept>;
    pub fn related(&self, name: &str, relation: ConceptRelation) -> Vec<&AIConcept>;
    pub fn ancestors(&self, name: &str) -> Vec<&AIConcept>;
    pub fn descendants(&self, name: &str) -> Vec<&AIConcept>;
}
```

#### `AIConcept`

Single concept entry.

```rust
pub struct AIConcept {
    pub name: String,
    pub domain: ConceptDomain,
    pub description: String,
    pub math_notation: Option<String>,
    pub complexity: Option<String>,
    pub implementation_hints: Vec<String>,
}
```

---

## Error Handling

All fallible operations return `Result<T, RecursiveMachineIntelligenceError>`.

```rust
pub enum RecursiveMachineIntelligenceError {
    Compute(String),
    Protocol(String),
    Ontology(String),
    Inference(String),
    Agent(String),
    Io(std::io::Error),
}
```

---

## Feature Flags

| Feature | Description      | Default |
| ------- | ---------------- | ------- |
| `cpu`   | CPU backend      | ✓       |
| `cuda`  | CUDA GPU backend |         |
| `full`  | All features     |         |

---

*Generated for RecursiveMachineIntelligence v1.0.0-rc.1*
