# RecursiveMachineIntelligence Architecture Guide

> **The First AI Framework Designed for AI Systems, Not Humans**

This document describes the architecture of RecursiveMachineIntelligence, explaining how the different 
components work together to enable autonomous multi-agent AI systems.

---

## Overview

RecursiveMachineIntelligence inverts the traditional ML framework paradigm. Instead of providing APIs for 
human developers, it provides **machine-native primitives** that AI agents can reason 
over, compose, and optimize programmatically.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Agent Layer                                        │
│  ┌─────────────┐  ┌─────────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │  Architect  │  │    Trainer      │  │  Evaluator  │  │  Reasoner   │    │
│  │   Agent     │  │    Agent        │  │    Agent    │  │   Agent     │    │
│  └──────┬──────┘  └────────┬────────┘  └──────┬──────┘  └──────┬──────┘    │
├─────────┴──────────────────┴───────────────────┴───────────────┴────────────┤
│                   Storage & Communication Layer                              │
│  ┌────────────────────────────────┐  ┌──────────────────────────────────┐   │
│  │         Message Bus            │  │          Storage                 │   │
│  │  • Pub/Sub Topics              │  │  • KeyValue Store (LRU+LZ4)      │   │
│  │  • Request/Reply RPC           │  │  • Tensor Storage                │   │
│  │  • Dead Letter Queue           │  │  • Checkpoint Manager            │   │
│  └────────────────────────────────┘  └──────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                       Binary Protocol Layer                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  MessagePack + LZ4 | Tensor Attachments | Capability Discovery       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────────────────┤
│                     Neurosymbolic Integration                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────────────┐   │
│  │   Symbol     │  │ Differentiable│ │     Hybrid Reasoner              │   │
│  │  Embedding   │  │  Constraints  │ │  (Neural + Symbolic + Adaptive)  │   │
│  └──────────────┘  └──────────────┘  └──────────────────────────────────┘   │
├────────────────────────────┬────────────────────────────────────────────────┤
│        Neural Module       │              Symbolic Module                    │
│  ┌──────────────────────┐  │  ┌──────────────────────────────────────────┐  │
│  │ • Autodiff Engine    │  │  │ • First-Order Logic                      │  │
│  │ • Architecture DAGs  │  │  │ • Unification Algorithm                  │  │
│  │ • Layer Library      │  │  │ • Forward/Backward Chaining              │  │
│  │ • Gradient Tape      │  │  │ • STRIPS Planning                        │  │
│  └──────────────────────┘  │  └──────────────────────────────────────────┘  │
├────────────────────────────┴────────────────────────────────────────────────┤
│                        Compute Backend                                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │     CPU      │ │     CUDA     │ │    WebGPU    │ │    Metal     │        │
│  │ ndarray+rayon│ │ cudarc+cuBLAS│ │     wgpu     │ │  Apple GPU   │        │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                         │
│  │    Vulkan    │ │  Apple ANE   │ │   Qualcomm   │                         │
│  │   SPIR-V     │ │  Core ML/NPU │ │ Hexagon DSP  │                         │
│  └──────────────┘ └──────────────┘ └──────────────┘                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                     Ontology & Knowledge Base                                │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────┐   │
│  │   Primitives     │  │   AI History     │  │    AI Concepts           │   │
│  │   Ontology       │  │   1943-2023      │  │    Ontology              │   │
│  └──────────────────┘  └──────────────────┘  └──────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Multi-Agent Collaboration Layer

RecursiveMachineIntelligence provides first-class primitives for AI-to-AI collaboration, making it the
first framework truly designed for multi-agent AI systems to work together autonomously.

### AgentRuntime (`collaboration.rs`)

The central integration hub that wires agents to the framework's infrastructure:

```rust
let runtime = AgentRuntime::new(RuntimeConfig::default());
runtime.spawn(architect_agent).await;
runtime.spawn(trainer_agent).await;

// Agents can now communicate via MessageBus, share data via SharedWorkspace,
// and delegate tasks to each other based on capabilities.
let stats = runtime.stats();
println!("Active agents: {}", stats.agent_count);
```

### SharedWorkspace — Blackboard Pattern

A thread-safe shared data space where agents read/write entries with version tracking
and event watching:

- `put()` / `get()` / `delete()` — Typed key-value entries with automatic versioning
- `put_tensor()` / `get_tensor()` — Efficient tensor sharing between agents
- `watch()` — Subscribe to changes on key prefixes
- Automatic version conflict detection

### TaskDelegator — Capability-Based Routing

Routes tasks to the best available agent based on declared capabilities and current load:

- Queries ServiceRegistry for agents matching requested capabilities
- Selects least-loaded agent for optimal resource utilization
- Publishes task assignments via MessageBus for decoupled execution

### AgentPipeline — Composable Processing

Chain multi-agent processing stages where each stage's output flows to the next:

```rust
let pipeline = AgentPipeline::new("train_and_evaluate")
    .then(PipelineStage::new("preprocess", agent_a_id, "raw_data", "clean_data"))
    .then(PipelineStage::new("train", agent_b_id, "clean_data", "model"))
    .then(PipelineStage::new("evaluate", agent_c_id, "model", "metrics"));

pipeline.validate()?; // Verifies input/output key chains
```

### FederatedTrainer — Distributed Training

Multiple agents each train on local data, then periodically aggregate parameters:

- **FedAvg**: Weighted average by dataset size (McMahan et al., 2017)
- **FedProx**: Proximal regularization for heterogeneous data (Li et al., 2020)
- **TrimmedMean**: Byzantine-robust aggregation trimming outlier parameters
- Automatic rollback when global loss degrades

### ModelRegistry — Versioned Model Store

Agents register, share, and discover models with version tracking:

- Automatic version numbering on registration
- Search by metric thresholds (e.g., "accuracy > 0.95")
- Tag-based filtering for model discovery
- Get latest or specific version

---

## Core Modules

### 1. Core Module (`src/core/`)

The foundation layer providing fundamental abstractions.

#### Primitives (`primitives.rs`)

Defines the basic building blocks with rich algebraic metadata:

```rust
pub struct Primitive {
    pub id: PrimitiveId,
    pub name: String,
    pub category: PrimitiveCategory,
    pub dtype: TensorDType,
    pub algebraic_properties: Vec<AlgebraicProperty>,
    pub gradient_info: GradientInfo,
    pub compute_cost: ComputeCost,
}
```

**Key Features:**
- Algebraic properties (Associative, Commutative, Distributive, etc.)
- Gradient information for autodiff
- Compute cost estimation for optimization

#### Ontology (`ontology.rs`)

Machine-readable knowledge representation:

```rust
pub struct Ontology {
    pub concepts: HashMap<ConceptId, Concept>,
    pub relations: Graph<ConceptId, Relation>,
    pub axioms: Vec<Axiom>,
}
```

**Operations:**
- Concept lookup and traversal
- Relation queries (IS_A, HAS_PART, etc.)
- Semantic similarity computation

#### Agent (`agent.rs`)

Autonomous AI agent abstraction:

```rust
pub struct Agent {
    pub id: AgentId,
    pub capabilities: Vec<Capability>,
    pub goals: Vec<Goal>,
    pub knowledge: Arc<Ontology>,
    pub state: AgentState,
}
```

**Capabilities:**
- Goal-driven execution
- Inter-agent communication
- Resource negotiation

#### Protocol (`protocol.rs`)

Binary communication protocol optimized for AI systems:

```rust
pub struct Protocol {
    pub serialization: Serialization::MessagePack,
    pub compression: Compression::Lz4,
    pub tensor_encoding: TensorEncoding::Native,
}
```

**Features:**
- 3-5x smaller than JSON
- Native tensor attachment support
- Streaming capability

#### Storage (`storage.rs`)

Efficient persistent data storage for agent state and model artifacts:

```rust
pub struct KeyValueStore {
    pub cache: LruCache<String, Vec<u8>>,
    pub base_path: PathBuf,
    pub compression_enabled: bool,
}

pub struct TensorStorage {
    pub index: HashMap<String, TensorIndexEntry>,
    pub data_file: File,
}

pub struct CheckpointManager {
    pub checkpoint_dir: PathBuf,
    pub max_checkpoints: usize,
    pub checkpoints: Vec<CheckpointMeta>,
}
```

**Key Features:**
- **KeyValueStore** - LRU-cached KV store with LZ4 compression and disk persistence
- **TensorStorage** - Efficient binary tensor format (safetensors-like) with XXH64 checksums
- **CheckpointManager** - Model/agent state checkpointing with versioning and retention policies
- **ConsistentHashRing** - Distributed storage with virtual nodes for horizontal scaling

#### Message Bus (`message_bus.rs`)

High-performance inter-agent communication infrastructure:

```rust
pub struct Topic {
    pub segments: Vec<String>,  // Hierarchical: "agent.task.compute"
}

pub struct Envelope<T> {
    pub id: u64,
    pub topic: Topic,
    pub payload: T,
    pub priority: u8,
    pub ttl: Option<Duration>,
    pub correlation_id: Option<u64>,
}

pub struct MessageBus {
    pub subscriptions: HashMap<String, Vec<Subscription>>,
    pub dead_letter_queue: DeadLetterQueue,
}
```

**Communication Patterns:**
- **Pub/Sub** - Topic-based broadcast with wildcard matching (`*`, `#`)
- **Request/Reply** - RPC-style communication with correlation IDs
- **Dead Letter Queue** - Failed message handling with retry support

**Standard Topics:**
| Category      | Topics                                                    |
| ------------- | --------------------------------------------------------- |
| **Lifecycle** | `agent.started`, `agent.stopped`, `agent.heartbeat`       |
| **Tasks**     | `task.assigned`, `task.completed`, `task.failed`          |
| **Data**      | `data.updated`, `data.requested`, `data.shared`           |
| **Consensus** | `consensus.propose`, `consensus.vote`, `consensus.commit` |

---

### 2. Compute Module (`src/compute/`)

Backend-agnostic tensor computation.

#### Backend Trait

```rust
pub trait Backend: Send + Sync {
    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;
    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;
    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle>;
    // ... 30+ operations
}
```

#### CPU Backend (`cpu.rs`)

- Uses `ndarray` with `rayon` for parallelization
- SIMD optimization where available
- Memory-efficient large tensor support

#### CUDA Backend (`cuda.rs`)

- Full `cudarc` integration
- Custom PTX kernels for elementwise ops
- cuBLAS for matrix operations
- Async stream support for overlapping compute/transfer

#### WebGPU Backend (`webgpu.rs`)

- Cross-platform GPU via `wgpu`
- Shader-based dispatch for compute operations
- Browser and native support

#### Metal Backend (`metal.rs`)

- Apple GPU acceleration
- Metal Performance Shaders integration
- macOS and iOS support

#### Vulkan Backend (`vulkan.rs`)

- Cross-vendor GPU via SPIR-V compute shaders
- Explicit memory management
- Multi-GPU support

#### Apple ANE Backend (`apple_ane.rs`)

- Apple Neural Engine for Apple Silicon
- Core ML integration for NPU dispatch
- Optimized for inference workloads on macOS/iOS

#### Qualcomm Backend (`qualcomm.rs`)

- Qualcomm Hexagon DSP/HTP via QNN SDK
- NPU acceleration for Snapdragon SoCs
- Optimized for on-device inference on Android/Linux

---

### 3. Neural Module (`src/neural/`)

Deep learning primitives.

#### Autodiff (`autodiff.rs`)

Reverse-mode automatic differentiation:

```rust
pub struct GradientTape {
    operations: Vec<Operation>,
    gradients: HashMap<VariableId, Gradient>,
}

pub fn backward(tape: &GradientTape, loss: &Variable) -> HashMap<VariableId, Variable>;
```

#### Layers (`layers.rs`, `extended_layers.rs`)

Comprehensive layer library:

| Category           | Layers                                           |
| ------------------ | ------------------------------------------------ |
| **Linear**         | `Linear`, `Embedding`                            |
| **Convolutional**  | `Conv2d`                                         |
| **Attention**      | `Attention`, `MultiHeadAttention`                |
| **Normalization**  | `LayerNorm`, `BatchNorm`, `GroupNorm`, `RMSNorm` |
| **Recurrent**      | `LSTMCell`, `GRUCell`                            |
| **Regularization** | `Dropout`                                        |
| **Composition**    | `FeedForward`, `ResidualBlock`                   |

#### Architecture (`architecture.rs`)

DAG-based architecture representation:

```rust
pub struct NetworkArchitecture {
    pub graph: DiGraph<ArchitectureNode, ArchitectureEdge>,
    pub inputs: Vec<NodeId>,
    pub outputs: Vec<NodeId>,
    pub metadata: ArchitectureMetadata,
}
```

---

### 4. Symbolic Module (`src/symbolic/`)

Logic-based reasoning.

#### Logic (`logic.rs`)

First-order logic primitives:

```rust
pub enum Term {
    Variable(String),
    Symbol(String),
    Function(String, Vec<Term>),
    List(Vec<Term>),
}

pub struct Predicate {
    pub name: String,
    pub args: Vec<Term>,
}

pub struct Clause {
    pub head: Option<Predicate>,  // None for facts
    pub body: Vec<Literal>,
}
```

#### Unification (`unification.rs`)

Robinson's unification algorithm with occurs check:

```rust
pub fn unify(a: &Term, b: &Term) -> Option<Substitution>;
pub fn anti_unify(a: &Term, b: &Term) -> Term;  // Least general generalization
```

#### Inference (`inference.rs`)

Forward and backward chaining:

```rust
pub struct InferenceEngine {
    pub config: InferenceConfig,
}

impl InferenceEngine {
    pub fn forward_chain(&mut self, kb: &KnowledgeBase) -> Vec<Clause>;
    pub fn backward_chain(&mut self, kb: &KnowledgeBase, goal: &Predicate) -> bool;
}
```

#### Planner (`planner.rs`)

STRIPS-style planning:

```rust
pub struct Action {
    pub name: String,
    pub parameters: Vec<Term>,
    pub preconditions: Vec<Predicate>,
    pub add_effects: Vec<Predicate>,
    pub delete_effects: Vec<Predicate>,
}

pub fn plan(initial: &State, goal: &State, actions: &[Action]) -> Option<Vec<GroundAction>>;
```

---

### 5. Neurosymbolic Module (`src/neurosymbolic/`)

Hybrid neural-symbolic integration.

#### Symbol Embedding (`embedding.rs`)

Maps discrete symbols to continuous vectors:

```rust
pub struct SymbolEmbedding {
    pub embeddings: HashMap<String, Vec<f64>>,
    pub dim: usize,
}

impl SymbolEmbedding {
    pub fn embed(&mut self, symbol: &str) -> Vec<f64>;
    pub fn similarity(&self, a: &str, b: &str) -> f64;
}
```

#### Differentiable Constraints (`constraint.rs`)

Soft constraints for optimization:

```rust
pub struct SoftConstraint {
    pub formula: ConstraintFormula,
    pub weight: f64,
    pub temperature: f64,
}

impl SoftConstraint {
    pub fn evaluate(&self, vars: &HashMap<String, f64>) -> f64;
    pub fn gradient(&self, vars: &HashMap<String, f64>) -> HashMap<String, f64>;
}
```

#### Hybrid Reasoner (`hybrid.rs`)

Combines neural and symbolic reasoning:

```rust
pub enum ReasoningMode {
    Neural,     // Use embeddings + similarity
    Symbolic,   // Use logic + unification
    Hybrid,     // Fixed weighted combination
    Adaptive,   // Dynamically choose based on query
}

pub struct HybridReasoner {
    pub mode: ReasoningMode,
    pub neural_weight: f64,
    pub symbolic_weight: f64,
}
```

---

### 6. Knowledge Module (`src/knowledge/`)

AI domain knowledge.

#### AI History (`history.rs`)

Machine-readable database of AI contributions:

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
}
```

**Coverage:** 30+ seminal papers from McCulloch-Pitts (1943) to GPT-4 (2023)

#### AI Concepts (`ai_concepts.rs`)

Ontology of AI concepts:

```rust
pub struct AIConcept {
    pub name: String,
    pub domain: ConceptDomain,
    pub description: String,
    pub math_notation: Option<String>,
    pub complexity: Option<String>,
    pub related: Vec<(String, ConceptRelation)>,
}
```

---

## Design Principles

### 1. Machine-Native Interfaces

Everything is designed for programmatic consumption:
- Structured data over natural language
- Binary protocols over text protocols
- Algebraic metadata for formal reasoning

### 2. Compositional Design

All components compose through well-defined interfaces:
- Layers compose into architectures
- Predicates compose into clauses
- Agents compose into multi-agent systems

### 3. Self-Describing Components

Every component carries rich metadata:
- Compute costs for optimization
- Gradient information for autodiff
- Type information for validation

### 4. Backend Agnosticism

Compute is abstracted behind the `Backend` trait:
- Same code runs on CPU or GPU
- Seven backends: CPU, CUDA, WebGPU, Metal, Vulkan, Apple ANE, Qualcomm
- Automatic backend selection based on resources

---

## Extending RecursiveMachineIntelligence

### Adding a New Layer

1. Implement the `Layer` trait:

```rust
impl Layer for MyLayer {
    fn name(&self) -> &str { &self.name }
    fn forward(&self, inputs: &[&Variable], tape: &mut GradientTape) -> Variable { ... }
    fn parameters(&self) -> Vec<&Variable> { ... }
    fn parameters_mut(&mut self) -> Vec<&mut Variable> { ... }
    fn set_trainable(&mut self, trainable: bool) { ... }
    fn reset_parameters(&mut self) { ... }
}
```

2. Add to `layers.rs` or `extended_layers.rs`
3. Re-export from `mod.rs`

### Adding a New Backend

1. Implement the `Backend` trait
2. Add feature flag in `Cargo.toml`
3. Update `get_backend()` in `mod.rs`

### Adding to Knowledge Base

1. Add new `AIContribution` entries in `history.rs`
2. Add new `AIConcept` entries in `ai_concepts.rs`
3. Update relationships in the ontology

---

## Performance Considerations

### Memory Management

- Tensors use arena allocation on GPU
- Reference counting for automatic cleanup
- Memory pool to reduce allocation overhead

### Parallelism

- CPU: Rayon thread pool for data parallelism
- CUDA: Async streams for overlapping operations
- Agents: Tokio for async coordination

### Optimization

- LZ4 compression for network transfer
- MessagePack for compact serialization
- Lazy evaluation where possible

---

## Future Directions

1. **TPU Backend** - Support for Google TPUs
2. **Distributed Training** - Multi-node gradient aggregation
3. **Neural Architecture Search** - Automated architecture discovery
4. **Theorem Proving** - Deeper symbolic reasoning integration
5. **Continuous Learning** - Online adaptation without forgetting

---

*RecursiveMachineIntelligence - The Foundation for Autonomous AI Systems*
