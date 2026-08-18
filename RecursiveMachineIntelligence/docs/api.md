# RecursiveMachineIntelligence API Reference

This document provides a comprehensive reference for the RecursiveMachineIntelligence public API.

> **Status: not checked against the code, and known to have drifted.** Nothing
> compiles the signatures below or resolves the paths, so treat it as a map
> rather than a contract; `cargo doc` is the authority.
>
> Two problems were found and fixed on 2026-08-18 while looking for something
> else, which is the only reason they were found at all:
>
> - Every **Module:** line named `framewerx::…`. The crate is `rmi`; the name
>   changed and this file did not. All twelve paths were wrong, and all twelve
>   modules do exist — so the fix was mechanical, and an agent following the old
>   paths would have concluded the modules were missing.
> - The **FFI Bridge** section described types that exist nowhere in the crate
>   (`FfiValue`, `FfiFuncPtr`) and marked `call_unchecked` `unsafe` when it is
>   not. See that section for the detail; it is the one part of this file that
>   has now been read line-by-line against the source.
>
> The other sections have **not** been verified. If you rely on one, check it
> against `src/` and fix it here — the drift above suggests more.

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

**Module:** `rmi::compute`

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

**Module:** `rmi::compute::blas`

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

**Module:** `rmi::compute::fusion`

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

**Module:** `rmi::lang::jit`

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

**Module:** `rmi::lang::ffi`

Interface for calling **host closures** from RMIL. Despite the name, no raw
C-ABI pointers are involved and nothing here is `unsafe`.

```rust
pub type FfiFn = Box<dyn Fn(&[Val]) -> Result<Val, String> + Send + Sync>;

pub struct FfiRegistry { /* ... */ }
impl FfiRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, binding: FfiBinding);
    pub fn register_fn(
        &mut self,
        name: impl Into<String>,
        signature: FfiSignature,
        func: impl Fn(&[Val]) -> Result<Val, String> + Send + Sync + 'static,
    );
    pub fn call(&self, name: &str, args: &[Val]) -> Result<Val, FfiError>;
    // Skips the arity and type checks `call` performs. Safe: the callee is a
    // Rust closure, not a foreign pointer.
    pub fn call_unchecked(&self, name: &str, args: &[Val]) -> Result<Val, FfiError>;
}
```

> **Corrected 2026-08-18.** This block previously described an API that does
> not exist: `FfiValue` and `FfiFuncPtr` appear **nowhere in the crate**,
> `register` takes an `FfiBinding` rather than `(name, sig, ptr)`, `call`
> deals in `Val`, and `call_unchecked` is **not `unsafe`** and returns
> `Result<Val, FfiError>`. Code written against the old block would not
> compile — and it painted a more alarming picture than the truth, implying a
> raw-pointer FFI surface (`Ptr(*mut u8)`) and an unsafe entry point where the
> real thing passes RMIL values to safe Rust closures. `unsafe` in a signature
> is a contract; documenting one that isn't there is as much a defect as
> omitting one that is.

### LSP Server

**Module:** `rmi::lang::lsp`

Language Server Protocol implementation for RMIL source files.

```rust
pub struct LanguageServer { /* private: documents */ }
impl LanguageServer {
    pub fn new() -> Self;
    pub fn open(&mut self, uri: &str, source: &str);
    pub fn update(&mut self, uri: &str, source: &str);
    pub fn close(&mut self, uri: &str);
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic>;
    pub fn hover(&self, uri: &str, pos: Position) -> Option<HoverInfo>;
    pub fn completions(&self, uri: &str, pos: Position) -> Vec<CompletionItem>;
    pub fn document_symbols(&self, uri: &str) -> Vec<DocumentSymbol>;
}
```

> **Corrected 2026-08-18.** The type is `LanguageServer`, not
> `RmilLanguageServer`. It is document-oriented — `open`/`update`/`close` keep
> the source, and every query takes a `uri` rather than a `&str` of source, so
> the documented calling convention was wrong for all of them. `completion` is
> `completions`, and `definition`/`references` do not exist.

### Package Registry

**Module:** `rmi::lang::registry`

Versioned registry of RMIL packages, with semver resolution and dependency
resolution.

```rust
pub struct PackageMeta {
    pub name: String,
    pub version: SemVer,
    pub description: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
    /* … */
}

pub struct Registry { /* private: packages, counter */ }
impl Registry {
    pub fn new() -> Self;
    pub fn publish(&mut self, meta: PackageMeta, expr: Expr) -> Result<(), RegistryError>;
    pub fn resolve(&self, name: &str, req: &VersionReq) -> Option<&Package>;
    pub fn versions(&self, name: &str) -> &[Package];
    pub fn get(&self, name: &str, version: &SemVer) -> Option<&Package>;
    pub fn search_by_tag(&self, tag: &str) -> Vec<&Package>;
    pub fn search_by_name(&self, query: &str) -> Vec<&Package>;
    pub fn list(&self) -> Vec<&str>;
    pub fn total_packages(&self) -> usize;
}
```

> **Corrected 2026-08-18.** This section described an *operation* registry —
> `OpRegistry`, `RegisteredOp`, `with_builtins`, `lookup_by_tag`, `all_ops` —
> none of which exists in any source file. The module of that name registers
> **packages**: versioned RMIL expressions with semver resolution. Five
> fictional items in one block, under a heading that named the wrong subject,
> which is why they survived: the module path resolved and the section read
> plausibly.

---

## Neural Module

**Module:** `rmi::neural`

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
    pub fn new(name: impl Into<String>) -> Self;
    pub fn input(self, name: impl Into<String>, shape: ShapeSpec) -> Self;
    pub fn linear(self, name: impl Into<String>, out_features: i64) -> Self;
    pub fn layer_norm(self, name: impl Into<String>) -> Self;
    pub fn relu(self, name: impl Into<String>) -> Self;
    pub fn gelu(self, name: impl Into<String>) -> Self;
    pub fn attention(self, name: impl Into<String>, heads: i64, head_dim: i64) -> Self;
    pub fn dropout(self, name: impl Into<String>, p: f64) -> Self;
    pub fn residual_add(self, name: impl Into<String>, skip_from: Uuid) -> Self;
    pub fn output(self) -> Self;
    pub fn current(&self) -> Option<Uuid>;
    pub fn fork(&self) -> Self;
    pub fn build(self) -> NetworkArchitecture;
}
```

> **Corrected 2026-08-18.** There is no `add_layer(LayerSpec)` — the builder
> has one method per layer kind, each naming the layer, and it threads a
> `Uuid` cursor (`current`) rather than indices. Skip connections are
> `residual_add(name, skip_from: Uuid)`, not
> `add_skip_connection(from: usize, to: usize)`: **a caller following the old
> signature would be passing positions where the API wants node ids.**

### Functions

```rust
/// Compute gradients via backpropagation
pub fn backward(tape: &GradientTape, loss: &Variable) -> HashMap<VariableId, Variable>;

/// Compute gradient of output w.r.t. specific variable
pub fn grad(tape: &GradientTape, output: &Variable, var: &Variable) -> Variable;
```

---

## Symbolic Module

**Module:** `rmi::symbolic`

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
    pub fn add_fact(&mut self, name: impl Into<String>, args: Vec<Term>);
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

**Module:** `rmi::neurosymbolic`

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
    pub fn similarity(emb1: &[f32], emb2: &[f32]) -> f32;   // associated fn, not a method
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

**Module:** `rmi::core`

### Agent

#### `Agent`

Autonomous AI agent.

```rust
pub struct Agent {
    pub identity: AgentIdentity,
    /* private: state, context, current_goal, goal_stack,
       execution_trace, message_tx, message_rx */
}

impl Agent {
    pub fn builder() -> AgentBuilder;
    pub fn state(&self) -> AgentState;
    pub fn has_capability(&self, capability: AgentCapability) -> bool;
    pub fn add_capability(&mut self, capability: AgentCapability);
    pub fn set_goal(&self, goal: Goal);
    pub fn push_subgoal(&self, subgoal: Goal);
    pub fn pop_goal(&self) -> Option<Goal>;
    pub async fn execute(&self, goal: Goal) -> Result<GoalResult>;
}
```

> **Corrected 2026-08-18.** `Agent` has no `id`, `config` or public `state`
> field, and no `new(config)` — it is built through `Agent::builder()`, and
> `state` is a method over an atomic. `execute` returns `GoalResult`, not
> `ExecutionResult`.

#### `Goal`

Agent objective. **An enum**, not a struct with a `goal_type` tag:

```rust
pub enum Goal {
    MinimizeLoss    { metric_name: String, target_value: Option<f64>,
                      constraints: HashMap<String, f64> },
    MaximizeMetric  { metric_name: String, target_value: Option<f64>,
                      constraints: HashMap<String, f64> },
    ArchitectureSearch { task_type: String,
                         input_schema: HashMap<String, String>,
                         output_schema: HashMap<String, String>,
                         resource_constraints: HashMap<String, f64> },
    Inference { model_id: String, input_data: Vec<u8> },
    Train     { model_id: String, data_source: String, epochs: u32, batch_size: u32 },
    Reason    { query: String, context_concepts: Vec<ConceptId> },
    Custom    { goal_type: String, spec: Vec<u8> },
}
```

> **Corrected 2026-08-18.** The previous block described a struct with
> `id`/`goal_type`/`target`/`constraints`/`priority` and a companion
> `GoalType { Minimize, Maximize, Satisfy, Achieve }` enum. Neither exists —
> `GoalType` is in no source file, and the tag it stood for is the enum variant
> itself. A `Goal` *does* exist in `rmi::symbolic::planner` as well, holding
> positive and negative predicate lists, so the name resolves twice and neither
> matches what was written here.

### Protocol

#### `Protocol`

Binary communication protocol.

```rust
pub struct Protocol {
    pub compression: String,
    pub encryption: Option<String>,
    pub schema_format: String,
    pub max_message_size: usize,
    pub stream_chunk_size: usize,
    /* private: schemas */
}

impl Protocol {
    pub fn new() -> Self;
    pub fn binary() -> Self;
    pub fn secure(encryption: &str) -> Self;
    pub fn register_schema(&mut self, schema: MessageSchema);
    pub fn get_schema(&self, name: &str) -> Option<&MessageSchema>;
    pub fn create_message(
        &self,
        sender_id: Uuid,
        recipient_id: Uuid,
        message_type: MessageType,
        payload: HashMap<String, serde_json::Value>,
    ) -> Result<Message>;
}
```

> **Corrected 2026-08-18.** `Protocol` has no `config` field and no
> `encode`/`decode`. Those two belong to `Frame` in
> `rmi::distributed::transport` and have different shapes —
> `Frame::encode(&self) -> Vec<u8>` and
> `Frame::decode(data: &[u8]) -> Result<(Self, usize)>`, an associated function
> rather than a method. `Protocol` is a schema registry with compression and
> encryption settings.

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
pub struct KeyValueStore { /* private: base_path, cache, lru_order, … */ }

impl KeyValueStore {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self>;
    pub fn in_memory() -> Self;
    pub fn with_cache_size(self, bytes: usize) -> Self;      // builder
    pub fn without_compression(self) -> Self;                // builder
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<StorageMetadata>;
    pub fn put_raw(/* … */) -> Result<StorageMetadata>;
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    pub fn get_raw(&self, key: &str) -> Result<Option<(Vec<u8>, StorageMetadata)>>;
    pub fn exists(&self, key: &str) -> bool;
    pub fn delete(&self, key: &str) -> Result<bool>;
    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;
    pub fn metadata(&self, key: &str) -> Result<Option<StorageMetadata>>;
}
```

> **Corrected 2026-08-18.** Four differences worth calling out. Compression is
> **opt-out**, not opt-in: the real builder is `without_compression`, and a
> reader following the old `with_compression` would have enabled something that
> was already on. `set` is `put` and returns `StorageMetadata` rather than
> `()`; `contains` is `exists`; and every method takes `&self`, not `&mut self`
> — the cache is behind an `RwLock`, so a shared handle is the intended usage
> and the documented signatures would have forced needless `mut`.

#### `TensorStorage`

Efficient binary tensor storage format (similar to safetensors).

```rust
pub struct TensorStorage { /* private: path, index, mmap_data */ }

pub struct TensorIndexEntry {
    pub name: String,
    pub shape: Vec<usize>,
    pub dtype: String,
    pub offset: u64,
    pub size: u64,
    pub checksum: u64,
}

impl TensorStorage {
    pub fn create(path: impl AsRef<Path>) -> Result<Self>;
    pub fn open(path: impl AsRef<Path>) -> Result<Self>;
    pub fn add_f32(&mut self, name: &str, shape: &[usize], data: &[f32]) -> Result<()>;
    pub fn add_f64(&mut self, name: &str, shape: &[usize], data: &[f64]) -> Result<()>;
    pub fn add_raw(&mut self, name: &str, shape: &[usize], dtype: &str, data: &[u8]) -> Result<()>;
    pub fn save(&self, tensors_data: &HashMap<String, Vec<u8>>) -> Result<()>;
    pub fn get_f32(&self, name: &str) -> Result<Option<(Vec<usize>, Vec<f32>)>>;
    pub fn tensor_names(&self) -> Vec<&str>;
    pub fn tensor_info(&self, name: &str) -> Option<&TensorIndexEntry>;
}
```

> **Corrected 2026-08-18.** None of `write_tensor`, `read_tensor` or
> `list_tensors` exists; the real API is typed (`add_f32`/`add_f64`/`add_raw`,
> `get_f32`) and names are listed by `tensor_names`. `dtype` is a `String`, not
> a `StorageDataType` enum — that type does not exist either — and the index
> field is `size`, not `length`.

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
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self>;
    pub fn with_max_checkpoints(self, max: usize) -> Self;   // builder
    pub fn save<T: Serialize>(
        &self,
        checkpoint_type: CheckpointType,
        step: u64,
        description: &str,
        data: &T,
        metrics: HashMap<String, f64>,
    ) -> Result<CheckpointMeta>;
    pub fn save_with_tensors(
        &self,
        checkpoint_type: CheckpointType,
        step: u64,
        description: &str,
        state: &HashMap<String, Vec<u8>>,
        tensors: &HashMap<String, (Vec<usize>, Vec<f32>)>,
        metrics: HashMap<String, f64>,
    ) -> Result<CheckpointMeta>;
    pub fn load<T: DeserializeOwned>(&self, id: Uuid) -> Result<Option<(CheckpointMeta, T)>>;
    pub fn load_tensors(&self, id: Uuid) -> Result<Option<TensorStorage>>;
    pub fn list(&self) -> Result<Vec<CheckpointMeta>>;
    pub fn latest(&self) -> Result<Option<CheckpointMeta>>;
    pub fn delete(&self, id: Uuid) -> Result<()>;
}
```

> **Corrected 2026-08-18.** Every method in this block was wrong: the names
> carried a `_checkpoint` suffix the code does not use (`save_checkpoint` →
> `save`), `new` took `(dir, max)` where the real one takes a path and a
> `with_max_checkpoints` builder, ids are `Uuid` rather than `&str`, and the
> return types differ throughout. `CheckpointManager` itself exists — which is
> what made this hard to notice: the type resolves, so only calling a method
> reveals the drift.

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
    pub fn new(namespace: &str) -> Self;
    pub fn load(uri: &str) -> Result<Self>;
    pub fn save(&self, path: &str) -> Result<()>;
    pub fn add_concept(&self, concept: Concept);
    pub fn add_concepts(&self, concepts: Vec<Concept>);
    pub fn add_relation(&self, relation: Relation);
    pub fn get(&self, id: &ConceptId) -> Option<Concept>;
    pub fn get_many(&self, ids: &[ConceptId]) -> Vec<Option<Concept>>;
    pub fn lookup(&self, name: &str) -> Option<Concept>;
    pub fn get_related(&self, id: &ConceptId, rel_type: RelationType) -> Vec<Concept>;
    pub fn get_subgraph(/* … */);
    pub fn query(&self, q: &OntologyQuery) -> Vec<Concept>;
    pub fn find_similar(/* … */);
    pub fn merge(&self, other: &Ontology, strategy: MergeStrategy);
    pub fn to_binary(&self) -> Vec<u8>;
    pub fn from_binary(data: &[u8]) -> Result<Self>;
}
```

> **Corrected 2026-08-18.** `get_concept` and `related_concepts` do not exist:
> lookups are `get(&ConceptId)` or `lookup(&str)` — **ids and names are
> different keys** — and relations are `get_related(&ConceptId, RelationType)`.
> `new` takes a namespace. `add_concept`/`add_relation` take `&self`, not
> `&mut self`, and `add_relation` takes a whole `Relation` rather than
> `(from, to, relation)`. `similarity` is not a method here at all; it belongs
> to `rmi::neurosymbolic::embedding` and is listed under that module.

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

**Module:** `rmi::knowledge`

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

All fallible operations return `Result<T, RmiError>`.

```rust
pub enum RmiError {
    Primitive(String),
    Ontology(String),
    Agent(String),
    Protocol(String),
    Compute(String),
    Serialization(String),
    Io(#[from] std::io::Error),
    ShapeMismatch { /* … */ },
    ResourceExhausted(String),
    InvalidConfig(String),
    Neural(String),
    Symbolic(String),
}
```

> **Corrected 2026-08-18.** The type is `RmiError`. The old name was a
> rename artifact — a global `Rmi` → `RecursiveMachineIntelligence` pass
> rewrote the *type* along with the prose, and no such type ever existed.
> `Inference` is not a variant; six others were missing, including
> `ResourceExhausted`, which the memory pool returns.

---

## Feature Flags

| Feature | Description      | Default |
| ------- | ---------------- | ------- |
| `cpu`   | CPU backend      | ✓       |
| `cuda`  | CUDA GPU backend |         |
| `full`  | All features     |         |

---

*Generated for RecursiveMachineIntelligence v1.0.0-rc.1*
