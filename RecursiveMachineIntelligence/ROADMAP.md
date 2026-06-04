# RMI Roadmap

> **A Low-Level System for Machine-Generating AI Systems**

This document tracks the development progress and future plans for RMI.

---

## Current Status: v1.0.0 Released

**Last Updated:** April 13, 2026
**Test Coverage:** 1367 tests passing (1215 unit + 28 integration + 35 neural + 48 proptest + 41 doc)
**Compiler Warnings:** 0 (clippy clean)

---

## Completed Features

### RMIL — Recursive Machine Intelligence Language (`lang/`)

- [x] **Symbol Table** (`lang/sym.rs`)
  - Interned symbols with O(1) comparison (`Sym` as `u32`)
  - Bidirectional `HashMap<String, Sym>` + `Vec<String>`
  - Slot 0 reserved for `Sym::NIL`

- [x] **Structural Type System** (`lang/ty.rs`)
  - 13 scalar data types (`Dtype`: Bool, I8-I64, U8-U64, F16, BF16, F32, F64)
  - 9 type variants (`Ty`: Void, Scalar, Tensor, Sym, Fn, Tuple, Union, Var, Opaque)
  - Dynamic dimensions support (`DYN = usize::MAX`)
  - Differentiability, composition compatibility, and size queries

- [x] **Opcode Instruction Set** (`lang/op.rs`)
  - 95+ opcodes across 7 families: Neural (0x00), Symbolic (0x01), Control (0x02), Memory (0x03), Agent (0x04), Meta (0x05), Math (0x06)
  - `Op(u16)` newtype with `#[repr(transparent)]` (2 bytes per instruction)
  - Per-op metadata: arity, differentiability, statefulness, parameter presence
  - Full human-readable names and descriptions via `Op::meta()`
  - `Op::ALL` static slice for instruction set discovery

- [x] **Expression AST** (`lang/expr.rs`)
  - 10-variant AST: Lit, Ref, App, Seq, Par, Cond, Let, Lam, Call, Block
  - Algebraic composition: `>>` (Shr for sequential), `|` (BitOr for parallel)
  - Content-addressed hashing (deterministic u64 for dedup/caching)
  - Introspection: `node_count()`, `depth()`, `opcodes()`, `is_differentiable()`
  - Residual connections: `.residual()` wraps in `RES_ADD`
  - Pre-built patterns: `transformer_block()`, `mlp(n)`, `resnet_block()`, `rnn_model()`, `classifier_head()`, `conv_backbone(stages)`, `image_classifier(stages)`, `neurosymbolic_hybrid()`

- [x] **Binary Codec** (`lang/codec.rs`)
  - Wire format: 12-byte header (magic "RMIL" + version u16 + flags u16 + root count u32)
  - Inline symbol table + recursive expression tree encoding
  - Tag-byte variant dispatch for expressions, values, and types
  - Round-trip encode/decode for all expression and value variants
  - Full-program serialization (header + symbols + expr) and expr-only mode
  - DoS guard: max nesting depth of 512

- [x] **Tree-Walking VM** (`lang/vm.rs`)
  - Math operations: ADD, SUB, MUL, DIV, NEG, ABS, EXP, LOG, SQRT, SIN, COS, POW, MAX, MIN, CLAMP
  - Activations: RELU, SIGMOID, TANH (scalar approximations)
  - Control flow: Seq (pipeline), Par (fork → tuple), Cond (branch), Block, Let/Ref bindings
  - Introspection: HASH, IDENTITY
  - Neural/symbolic/agent ops as stubs (records intent, returns Nil)
  - Stack overflow protection with configurable max depth
  - Scoped environment with lexical binding

### Core Infrastructure

- [x] **Primitives System** (`core/primitives.rs`)
  - Atomic computational operations with algebraic properties
  - Associativity, commutativity, idempotency metadata
  - Gradient information for autodiff
  - Compute cost estimation

- [x] **Ontology System** (`core/ontology.rs`)
  - Machine-readable concept graphs
  - Relation queries (IS_A, HAS_PART, etc.)
  - Semantic similarity computation

- [x] **Agent System** (`core/agent.rs`)
  - Autonomous agent abstraction
  - Goal-driven execution
  - Capability advertisement
  - Agent state management

- [x] **Protocol Layer** (`core/protocol.rs`)
  - Binary serialization (MessagePack + LZ4)
  - Tensor attachments
  - Message types (Query, Result, GoalAssignment, TensorTransfer)
  - Capability discovery

- [x] **Storage System** (`core/storage.rs`)
  - KeyValueStore with LRU caching and disk persistence
  - TensorStorage (safetensors-like binary format)
  - CheckpointManager for model/agent state versioning
  - ConsistentHashRing for distributed storage

- [x] **Message Bus** (`core/message_bus.rs`)
  - Pub/Sub with hierarchical topics
  - Wildcard matching (`*`, `#`)
  - Request/Reply RPC patterns
  - Dead Letter Queue for failed messages
  - Priority and TTL support

- [x] **Code Generation** (`core/codegen.rs`)
  - Typed Intermediate Representation (IR)
  - IRNode, IROperation, IRType system
  - FunctionBuilder fluent API
  - Mutation operators for evolutionary synthesis
  - Crossover operators for genetic programming
  - Rust code emitter
  - Structural hashing for deduplication

- [x] **Multi-Agent Collaboration** (`core/collaboration.rs`, `core/swarm.rs`)
  - AgentRuntime: central hub for MessageBus, ServiceRegistry, SharedWorkspace
  - SharedWorkspace: blackboard-pattern data space with key watching
  - TaskDelegator: capability-based routing
  - AgentPipeline: composable multi-stage processing
  - ModelRegistry: versioned model store with metric/tag search
  - Swarm coordinator with task scheduling and consensus voting
  - Autonomous model builder and collaborative workflows

- [x] **Discoverability** (`core/discoverability.rs`)
  - FrameworkCatalog: unified search across ontology, AI concepts, history, and recipes
  - RecipeRegistry: pre-built architecture templates with composability metadata
  - CapabilityDescriptor: structured component descriptors for agent discovery

### Compute Backends

- [x] **CPU Backend** (`compute/cpu.rs`) — ndarray + rayon
- [x] **CUDA Backend** (`compute/cuda.rs`) — cudarc/cuBLAS (feature-gated)
- [x] **WebGPU Backend** (`compute/webgpu.rs`) — wgpu compute shaders
- [x] **Metal Backend** (`compute/metal.rs`) — Apple Metal + MSL
- [x] **Vulkan Backend** (`compute/vulkan.rs`) — SPIR-V compute pipeline
- [x] **Apple ANE Backend** (`compute/apple_ane.rs`) — Apple Neural Engine via Core ML
- [x] **Qualcomm Backend** (`compute/qualcomm.rs`) — Hexagon DSP/HTP via QNN

### Neural Module

- [x] **Autodiff Engine** (`neural/autodiff.rs`) — reverse-mode AD with gradient tape
- [x] **Layer Library** (`neural/layers.rs`, `neural/extended_layers.rs`) — Linear, Conv2d, MultiHeadAttention, LayerNorm, BatchNorm, GroupNorm, RMSNorm, LSTM, GRU, Embedding, Dropout
- [x] **Architecture System** (`neural/architecture.rs`) — DAG networks, ArchitectureBuilder, topological ordering
- [x] **Training** (`neural/training.rs`) — training loop, dataset, data loader
- [x] **Serialization** (`neural/serialization.rs`) — model save/load (binary + JSON)
- [x] **Federated Learning** (`neural/federated.rs`) — FedAvg, FedProx, TrimmedMean aggregation

### Symbolic Module

- [x] **First-Order Logic** (`symbolic/logic.rs`) — terms, predicates, clauses, CNF/DNF
- [x] **Unification** (`symbolic/unification.rs`) — Robinson's algorithm, occurs check, anti-unification
- [x] **Inference Engine** (`symbolic/inference.rs`) — forward/backward chaining, resolution
- [x] **STRIPS Planner** (`symbolic/planner.rs`) — action schemas, goal-directed planning

### Neurosymbolic Module

- [x] **Symbol Embedding** (`neurosymbolic/embedding.rs`) — neural-symbolic conversion
- [x] **Differentiable Constraints** (`neurosymbolic/constraint.rs`) — soft constraint satisfaction
- [x] **Hybrid Reasoner** (`neurosymbolic/hybrid.rs`) — adaptive mode, temperature-controlled blending

### Knowledge Base

- [x] **AI History** (`knowledge/history.rs`) — 30+ seminal papers (1943-2023)
- [x] **AI Concepts Ontology** (`knowledge/ai_concepts.rs`) — neural, symbolic, neurosymbolic concepts

### Code Generation & IR Toolchain

- [x] **Emitters** — 4 targets: Rust, CUDA/PTX, MLIR, ONNX
- [x] **IR Optimizations** — 6 passes: DCE, constant folding, CSE, operator fusion, strength reduction, algebraic simplification. Pipeline levels O0-O3 with fix-point iteration
- [x] **Verification** — 6 passes: type checking, shape inference, resource checking, termination analysis, bounds checking, dataflow analysis

### Distributed Infrastructure

- [x] **Transport** (`distributed/transport.rs`) — TCP/QUIC/InProcess, connection pooling, load balancing
- [x] **Discovery** (`distributed/discovery.rs`) — service registry, gossip protocol, health monitoring
- [x] **Consensus** (`distributed/consensus.rs`) — Raft + Byzantine fault tolerance
- [x] **Federation** (`distributed/federation.rs`) — hierarchical clusters, resource sharing, cross-cluster routing

### Evolution & Self-Improvement

- [x] **Meta-Learning** (`evolution/meta_learning.rs`) — architecture search, hyperparameter optimization, learning curves
- [x] **Self-Modification** (`evolution/self_modification.rs`) — sandboxed patches, rollback, safety guards
- [x] **Evolutionary Engine** (`evolution/population.rs`) — multi-objective Pareto, selection strategies, crossover/mutation

### Production Runtime

- [x] **Memory Pool** (`runtime/memory_pool.rs`) — arena allocation, zero-copy tensor buffers
- [x] **Observability** (`runtime/observability.rs`) — metrics, distributed tracing, structured logging
- [x] **Deployment** (`runtime/deployment.rs`) — declarative specs, YAML/Compose rendering, multi-cloud

### Documentation & Benchmarks

- [x] Architecture Guide, API Reference, Ontology Reference, Protocol Specification
- [x] 8 benchmark groups with Criterion

---

## Benchmarks

> Measured February 16, 2026 | `cargo bench` with Criterion | Windows x64

| Category            | Benchmark    | Size       | Time    |
| ------------------- | ------------ | ---------- | ------- |
| **Matrix Multiply** | naive        | 64x64      | 461 us  |
|                     | naive        | 128x128    | 4.53 ms |
|                     | naive        | 256x256    | 38.5 ms |
| **Forward Pass**    | linear       | 64->32     | 1.28 us |
|                     | linear       | 256->128   | 19.1 us |
|                     | linear       | 512->256   | 97.1 us |
| **Autodiff**        | backward_mul | 64 elem    | 2.36 us |
|                     | backward_mul | 256 elem   | 1.47 us |
|                     | backward_mul | 1024 elem  | 2.15 us |
| **Loss**            | MSE          | 64 elem    | 72.8 ns |
|                     | MSE          | 256 elem   | 223 ns  |
|                     | MSE          | 1024 elem  | 687 ns  |
| **Optimizer**       | SGD step     | 100 params | 88.7 ns |
|                     | SGD step     | 1K params  | 233 ns  |
|                     | SGD step     | 10K params | 2.05 us |
| **Activation**      | ReLU         | 256 elem   | 924 ns  |
|                     | ReLU         | 1024 elem  | 922 ns  |
|                     | ReLU         | 4096 elem  | 1.47 us |
| **Optimization**    | O0 pipeline  | 1 func     | 1.57 us |
|                     | O1 pipeline  | 1 func     | 12.5 us |
|                     | O2 pipeline  | 1 func     | 18.4 us |
|                     | O3 pipeline  | 1 func     | 60.9 us |
|                     | O0 pipeline  | 16 funcs   | 20.0 us |
|                     | O3 pipeline  | 16 funcs   | 1.11 ms |
| **Verification**    | full         | 1 func     | 4.54 us |
|                     | full         | 16 funcs   | 56.1 us |
|                     | post_opt     | 1 func     | 4.47 us |
|                     | post_opt     | 16 funcs   | 54.4 us |

*Naive matmul is O(n^3); production uses ndarray+BLAS.*

---

## Future Directions

### RMIL Enhancements

- [x] **JIT compilation** — lower RMIL expressions to native code via Cranelift or LLVM (`lang::jit`)
- [x] **Gradient tape on Expr** — automatic differentiation over RMIL expression trees (`lang::grad`)
- [x] **Tensor runtime** — full tensor computation in the VM (`lang::tensor_rt`)
- [x] **Pattern matching engine** — RMIL-native `MATCH`/`REWRITE` execution (`lang::pattern_match`)
- [x] **Agent protocol integration** — wire RMIL `SEND`/`RECV` to agent messaging (`lang::agent_bridge`)
- [x] **Incremental compilation** — content-hash-based caching of compiled sub-expressions (`lang::incremental`)
- [x] **RMIL debugger** — step-through execution, breakpoints, expression watch (`lang::debugger`)

### Compute & Performance

- [x] **BLAS/LAPACK integration** — hardware-accelerated matmul, eigendecomposition, SVD (`compute::blas`)
- [x] **Kernel fusion** — fuse adjacent RMIL ops into single GPU kernels (`compute::fusion`)
- [x] **Quantization** — F16/BF16/INT8/INT16 with symmetric per-tensor quantization (`lang::quantize`)
- [x] **Sparse tensor support** — COO and CSR formats with SpMV/SpMM (`lang::sparse`)

### Language & Ecosystem

- [x] **RMIL text syntax** (optional) — human-readable surface syntax with parser & pretty printer (`lang::syntax`)
- [x] **Language server** — IDE support for RMIL programs (`lang::lsp`)
- [x] **Package registry** — share and discover RMIL modules across agents (`lang::registry`)
- [x] **Foreign function interface** — call external libraries from RMIL programs (`lang::ffi`)

---

## Architecture Evolution

```
v0.1 (Initial)                    v1.0 (Current)
+-------------------+            +-----------------------------------+
|   Single Node     |            |        Distributed Cluster        |
|  +-----------+    |            |  +---------+    +---------+       |
|  |  Agent A  |    |   ---->    |  | Node 1  |<-->| Node 2  |       |
|  |  Agent B  |    |            |  | Agent A |    | Agent C |       |
|  +-----------+    |            |  | Agent B |    | Agent D |       |
+-------------------+            |  +---------+    +---------+       |
                                 |  Raft | BFT | Gossip | Federation |
                                 |  CPU | CUDA | WebGPU | Metal | Vk |
                                 |  +-----------------------------+  |
                                 |  |          RMIL               |  |
                                 |  | 95+ ops | binary codec | VM |  |
                                 |  +-----------------------------+  |
                                 +-----------------------------------+
```

---

## Metrics

| Metric        | Current                                                                                                                                                   |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tests         | 1367 (1215 unit + 28 integration + 35 neural + 48 proptest + 41 doc)                                                                                      |
| Lint Warnings | 0 (clippy clean)                                                                                                                                          |
| RMIL Opcodes  | 95+ across 7 families                                                                                                                                     |
| RMIL Modules  | 19 (codec, expr, ffi, grad, incremental, jit, lsp, op, pattern_match, quantize, registry, sparse, sym, syntax, tensor_rt, ty, vm, debugger, agent_bridge) |
| Emitters      | 4 (Rust, CUDA, MLIR, ONNX)                                                                                                                                |
| Backends      | 7 (CPU, CUDA, WebGPU, Metal, Vulkan, Apple ANE, Qualcomm)                                                                                                 |
| Opt Passes    | 6 (DCE, CF, CSE, Fusion, SR, AlgSimp)                                                                                                                     |
| Verify Passes | 6 (Type, Shape, Resource, Term, Bounds, Dataflow)                                                                                                         |
| Distributed   | 4 (Transport, Discovery, Consensus, Federation)                                                                                                           |
| Evolution     | 3 (Meta-Learning, Population, Self-Modification)                                                                                                          |
| Runtime       | 3 (Deployment, Memory Pool, Observability)                                                                                                                |

---

## Changelog

### v1.0.0 (April 2026)

**Release**
- All features complete — promoted from RC to stable
- 1367 tests passing (1215 unit + 28 integration + 35 neural + 48 proptest + 41 doc)
- 0 clippy warnings
- WASM target support verified
- Supply-chain audit via cargo-deny
- Profile-guided optimization (PGO) support
- Expanded benchmarks (17 groups)

### v1.0.0-rc.1 (February 2026)

**RMIL Language** (NEW)
- Interned symbol table (`Sym` as `u32`, O(1) comparison)
- Structural type system with 13 dtypes and 9 type variants
- 95+ opcodes across 7 families (Neural, Symbolic, Control, Memory, Agent, Meta, Math)
- Expression AST with algebraic composition (`>>` sequential, `|` parallel)
- Binary codec with 12-byte header, inline symbol table, round-trip encode/decode
- Tree-walking VM with math, activations, control flow, and introspection
- Pre-built patterns: transformer_block, mlp, resnet_block, rnn_model, neurosymbolic_hybrid
- 85 new tests (566 total)

**Multi-Agent Collaboration**
- AgentRuntime, SharedWorkspace, TaskDelegator, AgentPipeline, ModelRegistry
- Swarm coordinator with consensus voting and collaborative workflows
- Framework discoverability: FrameworkCatalog, RecipeRegistry, CapabilityDescriptor

**Compute Backends**
- WebGPU backend: wgpu-based compute shaders, cross-platform GPU support
- Metal backend: Apple Metal compute pipeline, MSL shader generation
- Vulkan backend: SPIR-V shaders, descriptor set management, cross-platform
- Apple ANE backend: Neural Engine dispatch via Core ML on Apple Silicon
- Qualcomm backend: Hexagon DSP/HTP dispatch via QNN on Snapdragon

**Testing & Quality**
- 566 tests passing, 0 clippy warnings
- Property-based tests across 7 modules
- 8 benchmark groups with Criterion

### v0.2.0 (February 2026) — Feature Complete

**Code Generation**
- CUDA/PTX emitter with kernel launch wrappers
- MLIR emitter (func, arith, math, linalg dialects)
- ONNX export (opset 18+)
- IR optimization pipeline (O0-O3): 6 passes
- Verification system: 6 analysis passes

**Distributed**
- TCP/QUIC transport with connection pooling and load balancing
- Service discovery via gossip protocol
- Raft consensus + Byzantine fault tolerance
- Federation: hierarchical clusters, resource sharing

**Evolution**
- Meta-learning: architecture search, hyperparameter optimization
- Self-modification: sandboxed patches, rollback, safety guards
- Evolutionary engine: multi-objective Pareto, crossover/mutation

**Runtime**
- Memory pool: arena allocation, zero-copy tensor buffers
- Observability: metrics, distributed tracing, structured logging
- Deployment: declarative specs, YAML/Compose rendering, multi-cloud

### v0.1.0 (January 2026) — Initial Alpha

- Core: primitives, ontology, agent, protocol, storage, message bus, codegen
- Compute: CPU (ndarray+rayon), CUDA (feature-gated)
- Neural: autodiff, layers, architecture, training, serialization
- Symbolic: logic, unification, inference, planner
- Neurosymbolic: embedding, constraints, hybrid reasoner
- Knowledge: AI history (30+ papers), AI concepts ontology
