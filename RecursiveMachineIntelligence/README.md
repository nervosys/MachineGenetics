# RecursiveMachineIntelligence (RMI)

[![CI](https://github.com/nervosys/MachineGenetics/actions/workflows/ci.yml/badge.svg)](https://github.com/nervosys/MachineGenetics/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

> **The built-in agentic-first AI framework of MAGE (Machine Genetics), by NERVOSYS.**
> A low-level substrate for machine-generating AI systems — crate name `rmi`
> (Recursive Machine Intelligence). Optimized for agents: complete self-describing
> ontologies, token-compact discovery (`manifest()`/`describe()`), deterministic
> output, typed self-correcting errors, and effect-mapped safety.

---

## Philosophy

RMI is **not** another ML framework for humans. It is a low-level system designed for AI agents to generate, compose, and optimize other AI systems — without human intervention. Traditional frameworks encode human biases: sequential thinking, natural language abstractions, and interfaces optimized for human cognition. RMI inverts this paradigm:

- **RMIL**: A compact binary neurosymbolic language (the Recursive Machine Intelligence Language)
- **Machine-Native IR**: u16 opcodes, structural types, algebraic composition — no strings, no parsing
- **Binary-First Communication**: Maximally efficient inter-agent protocols (MessagePack+LZ4)
- **Compositional Architecture**: Everything composes via `>>` (sequential) and `|` (parallel)
- **Self-Describing Systems**: All components carry rich metadata for autonomous discovery
- **Zero Human Abstraction Tax**: No reflection, no dynamic dispatch, no text syntax

## Why Low-Level Matters

High-level frameworks (PyTorch, JAX, TensorFlow) are designed for humans to write code. RMI is designed for **machines to write code**:

| Aspect             | Human Frameworks   | RMI                                          |
| ------------------ | ------------------ | -------------------------------------------- |
| **Target User**    | Human developers   | AI agents                                    |
| **Representation** | Python/text source | Binary RMIL (u16 opcodes)                    |
| **Modification**   | Edit text files    | Mutation operators on AST                    |
| **Composition**    | Import/call        | Algebraic combinators `>>` `\|`              |
| **Communication**  | HTTP/JSON          | Binary protocol (60 bytes/transformer block) |
| **Discovery**      | Documentation      | Ontological queries + `Op::ALL` catalogue    |

## RMIL — Recursive Machine Intelligence Language

The core of RMI is RMIL: a compact, binary, neurosymbolic intermediate representation optimized for machine readability and time efficiency.

### Design Principles

1. **Machine-first** — binary is the canonical form; no text syntax needed
2. **Algebraic composition** — `>>` (sequential) and `|` (parallel) operators
3. **Content-addressed** — every sub-expression has a deterministic u64 hash
4. **Self-describing** — programs carry their own instruction set metadata
5. **Neurosymbolic native** — first-class neural AND symbolic operations
6. **Agent-native** — `SEND`, `RECV`, `SPAWN`, `DELEGATE` are opcodes, not library calls

### Instruction Set (95+ opcodes, 7 families)

| Family       | Prefix   | Examples                                                           |
| ------------ | -------- | ------------------------------------------------------------------ |
| **Neural**   | `0x00xx` | `MATMUL`, `LINEAR`, `CONV2D`, `ATTN`, `RELU`, `GELU`, `LAYER_NORM` |
| **Symbolic** | `0x01xx` | `UNIFY`, `RESOLVE`, `INFER`, `MATCH`, `REWRITE`, `PLAN`            |
| **Control**  | `0x02xx` | `SEQ`, `PAR`, `COND`, `LOOP`, `MAP`, `REDUCE`, `RES_ADD`           |
| **Memory**   | `0x03xx` | `ALLOC`, `FREE`, `LOAD`, `STORE`, `RESHAPE`, `TRANSPOSE`           |
| **Agent**    | `0x04xx` | `SEND`, `RECV`, `SPAWN`, `KILL`, `PUBLISH`, `SUBSCRIBE`            |
| **Meta**     | `0x05xx` | `HASH`, `TYPE_OF`, `SHAPE_OF`, `COMPOSE`, `INTROSPECT`             |
| **Math**     | `0x06xx` | `ADD`, `SUB`, `MUL`, `DIV`, `EXP`, `LOG`, `SQRT`, `SIN`, `COS`     |

### Wire Efficiency

A full **transformer encoder block** encodes in ~60 bytes on the wire. An entire MLP encodes in under 30 bytes. Binary codec round-trips are lossless.

### Example: Building Programs

```rust
use rmi::lang::*;

// A transformer block in 8 lines:
let block =
    Expr::op1(Op::LAYER_NORM)
    >> Expr::op1(Op::ATTN)
    >> Expr::op1(Op::DROP)
    >> Expr::op1(Op::LAYER_NORM)
    >> Expr::op1(Op::LINEAR)
    >> Expr::op1(Op::GELU)
    >> Expr::op1(Op::LINEAR)
    >> Expr::op1(Op::DROP);

// With residual connections:
let attn_path = (Expr::op1(Op::LAYER_NORM) >> Expr::op1(Op::ATTN)
    >> Expr::op1(Op::DROP)).residual();
let ffn_path  = (Expr::op1(Op::LAYER_NORM) >> Expr::op1(Op::LINEAR)
    >> Expr::op1(Op::GELU) >> Expr::op1(Op::LINEAR)
    >> Expr::op1(Op::DROP)).residual();
let transformer = attn_path >> ffn_path;

// Neurosymbolic hybrid (neural ‖ symbolic → merge):
let hybrid = Expr::op1(Op::EMBED)
    >> (Expr::op1(Op::LINEAR) >> Expr::op1(Op::GELU) >> Expr::op1(Op::LINEAR)
        | Expr::op1(Op::INFER) >> Expr::op1(Op::RESOLVE))
    >> Expr::op1(Op::CONCAT);

// Content hash for dedup/caching:
let hash = transformer.content_hash(); // deterministic u64

// Binary encode (< 200 bytes):
let bytes = codec::Encoder::encode_expr_only(&transformer);

// Evaluate math:
let mut vm = Vm::new();
let result = vm.eval(&Expr::op2(Op::ADD, Expr::int(2), Expr::int(3))).unwrap();
assert_eq!(result.as_i64(), Some(5));
```

### Pre-built Architecture Patterns

```rust
use rmi::lang::*;

let block      = patterns::transformer_block();     // pre-norm transformer
let mlp        = patterns::mlp(3);                  // 3-layer MLP
let resnet     = patterns::resnet_block();          // ResNet residual block
let rnn        = patterns::rnn_model();             // embed >> lstm >> linear
let classifier = patterns::image_classifier(4);     // 4-stage conv → classifier
let hybrid     = patterns::neurosymbolic_hybrid();  // neural ‖ symbolic → merge
```

## Core Paradigms

### Neural

Deep learning primitives with differentiable programming support:

- Tensor operations with automatic differentiation
- Architecture search primitives
- Training dynamics introspection

### Symbolic

Logic-based reasoning and knowledge representation:

- First-order and higher-order logic
- Ontological reasoning
- Constraint satisfaction and planning

### Neurosymbolic

Hybrid systems combining the best of both:

- Differentiable logic programming
- Knowledge-guided neural architectures
- Neural ‖ symbolic parallel composition via RMIL

### Program Synthesis

Low-level primitives for machine-generating code:

- **Typed IR**: Intermediate representation with static types
- **Mutation Operators**: Insert, delete, replace, rewire operations
- **Crossover**: Single-point and uniform crossover for evolution
- **Code Emission**: Generate Rust, CUDA, MLIR, ONNX from IR
- **Structural Hashing**: Deduplication of equivalent programs

### Multi-Agent Collaboration

First-class primitives for AI-to-AI collaboration:

- **AgentRuntime**: Central hub wiring agents to MessageBus, ServiceRegistry, and SharedWorkspace
- **SharedWorkspace**: Blackboard-pattern data space for agent coordination
- **TaskDelegator**: Capability-based task routing
- **AgentPipeline**: Composable multi-stage processing pipelines
- **ModelRegistry**: Versioned model store with metric/tag search
- **FederatedTrainer**: Federated learning across agents (FedAvg, FedProx, TrimmedMean)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      RMIL Language                          │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐      │
│  │ Sym  │ │  Ty  │ │  Op  │ │ Expr │ │Codec │ │  VM  │      │
│  │(u32) │ │struct│ │(u16) │ │(AST) │ │(bin) │ │(eval)│      │
│  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘      │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐      │
│  │ JIT  │ │ FFI  │ │ LSP  │ │ Reg  │ │ Grad │ │Quant │      │
│  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘      │
├─────────────────────────────────────────────────────────────┤
│                    Agent Orchestration                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Agent A   │◄─┤  Protocol   ├─►│   Agent B   │          │
│  └─────────────┘  │   Layer     │  └─────────────┘          │
│                   └──────┬──────┘                           │
├──────────────────────────┼──────────────────────────────────┤
│                    Code Generation                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  Typed IR   │  │  Mutation   │  │  Crossover  │          │
│  │  Builder    │  │  Operators  │  │  Operators  │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
├──────────────────────────┼──────────────────────────────────┤
│              Neurosymbolic Integration                      │
│   ┌──────────────────────┴──────────────────────┐           │
│   │         Knowledge-Guided Learning           │           │
│   └──────────────────────┬──────────────────────┘           │
├─────────────┬────────────┴───────────┬──────────────────────┤
│   Neural    │                        │      Symbolic        │
│  ┌────────┐ │                        │ ┌──────────────┐     │
│  │Tensors │ │                        │ │   Ontology   │     │
│  │Networks│ │                        │ │   Reasoner   │     │
│  │Optimize│ │                        │ │   Planner    │     │
│  └────────┘ │                        │ └──────────────┘     │
├─────────────┴────────────────────────┴──────────────────────┤
│                    Compute Backend                          │
│  ┌─────┐  ┌──────┐  ┌───────┐  ┌───────┐  ┌────────┐        │
│  │ CPU │  │ CUDA │  │WebGPU │  │ Metal │  │ Vulkan │        │
│  │+BLAS│  │      │  │       │  │       │  │        │        │
│  └─────┘  └──────┘  └───────┘  └───────┘  └────────┘        │
│  ┌─────┐  ┌──────────┐  ┌───────────────┐  ┌──────┐         │
│  │ ANE │  │ Qualcomm │  │ Kernel Fusion │  │ WASM │         │
│  └─────┘  └──────────┘  └───────────────┘  └──────┘         │
├─────────────────────────────────────────────────────────────┤
│                 Ontology & Knowledgebase                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Primitives  │  │  AI History │  │  Reasoning  │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### RMIL: Compose and Evaluate

```rust
use rmi::lang::*;

fn main() {
    let mut vm = Vm::new();

    // Arithmetic
    let expr = Expr::op2(Op::ADD, Expr::int(10), Expr::int(32));
    println!("{:?}", vm.eval(&expr)); // Ok(I64(42))

    // Let-bindings
    let x = vm.symbols.intern("x");
    let bound = Expr::bind(x, Expr::float(3.15), Expr::sym(x));
    println!("{:?}", vm.eval(&bound)); // Ok(F32(3.15))

    // Parallel fork
    let par = Expr::int(1) | Expr::int(2);
    println!("{:?}", vm.eval(&par)); // Ok(Tuple([I64(1), I64(2)]))

    // Architecture wire size
    let block = patterns::transformer_block();
    println!("transformer block = {} bytes", codec::wire_size(&block));
}
```

### Machine-Generated Neural Network (IR)

```rust
use rmi::core::codegen::{
    FunctionBuilder, IRType, IRPrimitiveType, Program,
    ActivationKind, Mutator, RustEmitter, CodeEmitter,
};

fn main() {
    let mut builder = FunctionBuilder::new(
        "mlp_forward",
        vec![
            ("x".into(), IRType::tensor(IRPrimitiveType::F32, vec![32, 784])),
            ("w1".into(), IRType::tensor(IRPrimitiveType::F32, vec![784, 256])),
            ("w2".into(), IRType::tensor(IRPrimitiveType::F32, vec![256, 10])),
        ],
        IRType::tensor(IRPrimitiveType::F32, vec![32, 10]),
    );

    let h = builder.matmul(0, 1, false, false);
    let h = builder.activation(ActivationKind::GeLU, h);
    let out = builder.matmul(h, 2, false, false);
    builder.ret(out);

    let mut program = Program::new("mlp");
    program.add_function(builder.build());

    // Mutate for evolutionary search
    let mutator = Mutator::new(42);
    let mutation = mutator.random_mutation();
    mutator.apply_mutation(&mut program, &mutation).unwrap();

    // Emit to Rust
    let code = RustEmitter.emit(&program).unwrap();
    println!("{}", code);
}
```

### Federated Multi-Agent Training

```rust
use rmi::neural::federated::{FederatedTrainer, FederatedConfig, AggregationStrategy};
use rmi::neural::training::{Dataset, Trainer, TrainerConfig};
use rmi::neural::{Linear, MSELoss, SGD};
use rmi::neural::layers::Layer;

fn main() {
    let datasets = vec![
        Dataset::new(
            vec![vec![0.0, 0.0], vec![1.0, 1.0]],
            vec![vec![0.0], vec![2.0]],
        ),
        Dataset::new(
            vec![vec![2.0, 2.0], vec![3.0, 3.0]],
            vec![vec![4.0], vec![6.0]],
        ),
    ];

    let make_trainer = || Trainer::new(
        vec![Box::new(Linear::new(2, 1))],
        Box::new(MSELoss::new()),
        Box::new(SGD::new(0.01)),
        TrainerConfig { epochs: 1, batch_size: 2, ..Default::default() },
    );

    let config = FederatedConfig {
        num_rounds: 10,
        local_epochs: 2,
        strategy: AggregationStrategy::FedAvg,
        ..Default::default()
    };

    let mut fed = FederatedTrainer::new(
        vec![make_trainer(), make_trainer()],
        datasets,
        config,
    );

    let history = fed.run();
    println!("Final global loss: {:.6}", history.global_losses.last().unwrap());
}
```

## Knowledgebase

RMI includes comprehensive ontologies:

### AI Primitives Ontology
- Tensor operations and their algebraic properties
- Network architecture patterns and compositionality rules
- Optimization algorithms and convergence properties
- Loss functions and their gradients

### AI History Ontology
A structured knowledge graph of AI progress from McCulloch-Pitts (1943) to present:
- Seminal papers and their contributions
- Concept genealogy and evolution
- Architectural innovations timeline

## Compute Backends

| Backend   | Technology                    | Platform         |
| --------- | ----------------------------- | ---------------- |
| CPU       | ndarray + rayon               | All              |
| CUDA      | cudarc/cuBLAS (feature-gated) | NVIDIA           |
| WebGPU    | wgpu compute shaders          | Browser + native |
| Metal     | Apple Metal + MSL             | macOS/iOS        |
| Vulkan    | SPIR-V compute pipeline       | Cross-platform   |
| Apple ANE | Neural Engine (Core ML)       | Apple Silicon    |
| Qualcomm  | Hexagon DSP/HTP (QNN)         | Snapdragon       |

## Installation

**Minimum Rust version:** 1.75

```bash
# Clone and build
git clone https://github.com/nervosys/MachineGenetics.git
cd RecursiveMachineIntelligence
cargo build --release
```

Or add as a dependency in your `Cargo.toml` (the `rmi` crate is vendored in
the MachineGenetics monorepo and resolved by package name):

```toml
[dependencies]
rmi = { git = "https://github.com/nervosys/MachineGenetics", package = "rmi" }
```

### Feature Flags

| Feature | Description                       | Dependencies         |
| ------- | --------------------------------- | -------------------- |
| `cpu`   | CPU backend (default)             | ndarray, rayon       |
| `cuda`  | NVIDIA GPU via cudarc/cuBLAS      | cudarc, cust         |
| `gpu`   | WebGPU/Vulkan/Metal/DX12 via wgpu | wgpu, pollster       |
| `wasm`  | WebAssembly target                | wasm-bindgen, js-sys |
| `full`  | All GPU backends (`cuda` + `gpu`) | all of the above     |

```toml
# CUDA support
rmi = { git = "https://github.com/nervosys/MachineGenetics", features = ["cuda"] }

# All GPU backends
rmi = { git = "https://github.com/nervosys/MachineGenetics", features = ["full"] }
```

## Project Structure

```shell
src/
├── lib.rs                     # Main library entry point
├── lang/                      # RMIL — Recursive Machine Intelligence Language
│   ├── sym.rs                 # Interned symbols (Sym as u32, O(1) comparison)
│   ├── ty.rs                  # Structural type system (Dtype, Ty, Shape)
│   ├── op.rs                  # u16 opcode instruction set (95+ ops, 7 families)
│   ├── expr.rs                # Expression AST + algebraic composition (>> and |)
│   ├── codec.rs               # Binary wire format (encode/decode, round-trip)
│   ├── vm.rs                  # Tree-walking evaluator (math, control, stubs)
│   ├── grad.rs                # Automatic differentiation over RMIL expression trees
│   ├── tensor_rt.rs           # Tensor runtime — full tensor computation in the VM
│   ├── pattern_match.rs       # RMIL-native MATCH/REWRITE execution engine
│   ├── agent_bridge.rs        # Wire RMIL SEND/RECV to agent messaging
│   ├── incremental.rs         # Content-hash-based caching of compiled sub-expressions
│   ├── debugger.rs            # Step-through execution, breakpoints, expression watch
│   ├── syntax.rs              # Human-readable surface syntax with parser & pretty printer
│   ├── quantize.rs            # F16/BF16/INT8/INT16 symmetric per-tensor quantization
│   ├── sparse.rs              # COO and CSR sparse tensor formats with SpMV/SpMM
│   ├── jit.rs                 # JIT compiler — SSA IR, lowering from Expr, interpreter
│   ├── lsp.rs                 # Language server — diagnostics, hover, completions
│   ├── registry.rs            # Package registry — versioned RMIL module sharing
│   └── ffi.rs                 # Foreign function interface — host function bindings
├── core/                      # Core primitives and abstractions
│   ├── primitives.rs          # Tensor, algebraic properties, type system
│   ├── ontology.rs            # Machine-readable ontologies
│   ├── agent.rs               # Agent system with goals and capabilities
│   ├── protocol.rs            # Binary protocol with MessagePack+LZ4
│   ├── storage.rs             # Persistent storage (KV, Tensor, Checkpoints)
│   ├── message_bus.rs         # Inter-agent pub/sub communication
│   ├── codegen.rs             # Low-level IR and program synthesis
│   ├── emitters.rs            # Code emitters (Rust, CUDA, MLIR, ONNX)
│   ├── optimization.rs        # IR optimizer (O0-O3, 6 passes)
│   ├── verification.rs        # IR verifier (6 analysis passes)
│   ├── introspection.rs       # Runtime component discovery
│   ├── discoverability.rs     # Framework catalog, recipes, capability descriptors
│   ├── swarm.rs               # Multi-agent swarm coordination
│   └── collaboration.rs       # AgentRuntime, SharedWorkspace, TaskDelegator
├── compute/                   # Compute backends (7 + BLAS + fusion)
│   ├── cpu.rs                 # CPU backend with ndarray+rayon
│   ├── cuda.rs                # CUDA backend (feature-gated)
│   ├── webgpu.rs              # WebGPU backend (wgpu)
│   ├── metal.rs               # Metal backend (Apple GPU)
│   ├── vulkan.rs              # Vulkan backend (SPIR-V)
│   ├── apple_ane.rs           # Apple Neural Engine backend (ANE/Core ML)
│   ├── qualcomm.rs            # Qualcomm Hexagon DSP/NPU backend (QNN)
│   ├── blas.rs                # BLAS/LAPACK — matmul, LU, QR, SVD, eigendecomposition
│   └── fusion.rs              # Kernel fusion — fuse adjacent ops for GPU dispatch
├── neural/                    # Neural AI primitives
│   ├── primitives.rs          # Differentiable primitives
│   ├── autodiff.rs            # Automatic differentiation engine
│   ├── architecture.rs        # Network architecture DAGs
│   ├── layers.rs              # Neural layers (Linear, Conv, Attention)
│   ├── extended_layers.rs     # Norm, RNN, Embedding, Dropout
│   ├── loss.rs                # Loss functions (MSE, CrossEntropy)
│   ├── optim.rs               # Optimizers (SGD, Adam)
│   ├── training.rs            # Training loop, dataset, data loader
│   ├── serialization.rs       # Model save/load (binary + JSON)
│   └── federated.rs           # Federated learning (FedAvg, FedProx, TrimmedMean)
├── symbolic/                  # Symbolic AI primitives
│   ├── logic.rs               # First-order logic, clauses, predicates
│   ├── unification.rs         # Unification algorithm
│   ├── inference.rs           # Forward/backward chaining inference
│   └── planner.rs             # STRIPS-style planning
├── neurosymbolic/             # Hybrid reasoning
│   ├── embedding.rs           # Neural-symbolic embeddings
│   ├── constraint.rs          # Differentiable constraints
│   └── hybrid.rs              # Hybrid reasoner with adaptive mode
├── distributed/               # Multi-node communication
│   ├── transport.rs           # TCP/QUIC transport with load balancing
│   ├── discovery.rs           # Service registry and gossip protocol
│   ├── consensus.rs           # Raft + Byzantine fault tolerance
│   └── federation.rs          # Cross-cluster federation
├── evolution/                 # Self-improvement
│   ├── meta_learning.rs       # Architecture search, hyperparameter opt
│   ├── population.rs          # Multi-objective evolutionary engine
│   └── self_modification.rs   # Sandboxed code patching
├── runtime/                   # Production deployment
│   ├── deployment.rs          # Declarative specs, YAML/Compose rendering
│   ├── memory_pool.rs         # Arena allocation, zero-copy tensor buffers
│   └── observability.rs       # Metrics, tracing, structured logging
└── knowledge/                 # AI knowledge base
    ├── history.rs             # AI history from 1943-present (30+ papers)
    └── ai_concepts.rs         # AI concepts ontology
```

## Testing

```bash
# Run all 1,367 tests
cargo test

# Run RMIL language tests
cargo test --lib lang

# Run specific module tests
cargo test --lib neural
cargo test --lib symbolic

# Run property-based (fuzz) tests
cargo test --test proptest_fuzz
cargo test --test proptest_ir
cargo test --test proptest_protocol

# Run benchmarks
cargo bench

# Lint (0 warnings)
cargo clippy --all-targets

# Generate docs (0 warnings)
cargo doc --no-deps --open
```

### Examples

```bash
cargo run --example agent_communication
cargo run --example architecture_search
cargo run --example benchmark_comparison
cargo run --example end_to_end_pipeline
cargo run --example neurosymbolic_reasoning
cargo run --example self_aware_architect
cargo run --example swarm_collaboration
cargo run --example training_pipeline
```

## Documentation

- [Architecture Guide](docs/architecture.md)
- [Ontology Reference](docs/ontology.md)
- [Protocol Specification](docs/protocol.md)
- [API Reference](docs/api.md)

## Contributing

Contributions are welcome. Please ensure:

1. `cargo test` passes all 1,367 tests
2. `cargo clippy --all-targets` reports 0 warnings
3. `cargo doc --no-deps` reports 0 warnings
4. New public APIs have doc comments

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.

## Citation

```bibtex
@software{rmi2026,
  title={RMI: Recursive Machine Intelligence},
  author={Nervosys},
  year={2026},
  url={https://github.com/nervosys/MachineGenetics}
}
```
