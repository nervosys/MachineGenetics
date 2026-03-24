# MechGen Compiler Internals

> Architecture and implementation guide for the MechGen prototype compiler.

---

## 1. Pipeline Overview

```
Source (.rx)
    │
    ▼
┌──────────┐      ┌──────────┐     ┌───────────┐      ┌──────────┐
│  Lexer   │────▶│  Parser  │────▶│  Resolve  │────▶│  Types   │
│ lexer.rs │      │ parser.rs│     │resolve.rs │      │ types.rs │
└──────────┘      └──────────┘     └───────────┘      └──────────┘
                                                         │
    ┌────────────────────────────────────────────────────┘
    ▼
┌──────────┐      ┌──────────┐     ┌───────────┐
│   HIR    │────▶│   MLIR   │────▶│  Codegen  │
│  hir.rs  │      │ mlir.rs  │     │  (target) │
└──────────┘      └──────────┘     └───────────┘
```

---

## 2. Modules

### 2.1 Lexer (`lexer.rs`)

The LL(1) lexer tokenises MechGen source into a stream of `Token` values. Each
token has a `TokenKind` and a `Span` (line, column, offset).

Key token kinds:
- `KwF`, `KwAf`, `KwUf` — function keywords
- `KwS`, `KwE`, `KwT`, `KwI`, `KwM` — item keywords
- `KwReq`, `KwEns`, `KwInv`, `KwFx`, `KwPerf` — contract annotations
- `Plus`, `Tilde` — visibility prefixes
- `Ident`, `LitInt`, `LitFloat`, `LitStr` — literals

### 2.2 Parser (`parser.rs`)

Recursive-descent parser producing `ast::Program`. Handles:
- Item parsing (functions, structs, enums, traits, impls, modules, use)
- Expression parsing with operator precedence
- Contract clause parsing (`@req`, `@ens`, `@inv`, `@fx`)
- Generic type parameters and trait bounds
- Legacy keyword fallback

### 2.3 AST (`ast.rs`)

The abstract syntax tree contains:

```
Program { items: Vec<Item> }

Item {
    visibility: Visibility,  // Private | Public
    attributes: Vec<Attribute>,
    kind: ItemKind,
}

ItemKind:
    Function(FunctionDef)    // includes contracts, effects
    Struct(StructDef)        // includes invariant contracts
    Enum(EnumDef)
    Trait(TraitDef)
    Impl(ImplDef)
    Module(ModuleDef)
    Use(UsePath)
    TypeAlias(TypeAlias)
    Const(ConstDef)
    Static(StaticDef)
```

Key types:
- `Type::Path { segments, type_args }` — named types
- `FunctionDef.contracts: Vec<ContractClause>`
- `FunctionDef.effects: Vec<String>`
- `ContractClause { kind, condition, message }`

### 2.4 Name Resolution (`resolve.rs`)

Resolves identifiers to their definitions. Builds scope chains and handles:
- Module scoping
- Use declarations
- Import resolution
- Name collision detection

### 2.5 Type System (`types.rs`)

The type checker verifies:
- Type compatibility
- Trait bound satisfaction
- Generic instantiation
- Lifetime correctness (placeholder)
- Effect compatibility

### 2.6 HIR (`hir.rs`)

High-level IR — a desugared form of the AST:
- All control flow normalised
- Pattern matching desugared
- Visibility resolved
- Contracts preserved as annotations

### 2.7 MLIR (`mlir.rs`)

Multi-Level IR for target-specific optimization:
- Ops: `func`, `call`, `return`, `const`, `add`, etc.
- Attributes: types, contracts, effects
- Lowering from HIR to MLIR

### 2.8 Effects (`effects.rs`)

The effect system tracks and propagates side effects:
- `EffectSet` — set of effects for a function
- `EffectLattice` — partial order on effects
- Effect checking and propagation rules
- Effect polymorphism (placeholder)

### 2.9 Contracts and Verification

- `ast.rs` — `ContractClause` definition
- `verify.rs` — Verification certificate generation
- `certs.rs` — Proof certificate serialisation

### 2.10 Cost Oracle (`cost.rs`)

Per-construct cost queries with:
- Built-in cost database for common constructs
- Per-target, per-OptLevel estimates
- `query_cost()`, `list_costs()`, `compare()`

---

## 3. Agentic Subsystems

### 3.1 Swarm Bus (`swarm_bus.rs`)

Typed publish/subscribe bus for inter-agent communication:
- `SwarmBus` — message routing
- `Channel<T>` — typed channels
- `Subscription` — callback registration

### 3.2 Swarm SDK (`swarm_sdk.rs`)

High-level swarm agent SDK:
- Agent lifecycle management
- Task distribution
- Result aggregation

### 3.3 ACI Subsystem (`aci.rs`)

Agent-Computer Interface — standardised tool interface:
- Tool registration and discovery
- Input/output schemas
- Permission model

### 3.4 Sandbox (`sandbox.rs`)

Per-agent capability-based isolation:
- `CapabilityToken` — scoped permissions
- `ResourceLimits` — memory/CPU/IO bounds
- `SandboxManager` — lifecycle management
- `AuditLog` — action logging

### 3.5 Hot Reload (`hot_reload.rs`)

Function-level hot-patching:
- `PatchUnit` — replacement unit
- `HotReloadEngine` — validate/apply/rollback
- Version tracking and rollback log

### 3.6 Token Budget (`token_budget.rs`)

Budget management for LLM agents:
- Token counting and tracking
- Budget allocation per agent
- Elision support

### 3.7 Elision (`elision.rs`)

Code summarisation for token-constrained agents:
- Collapsible code regions
- Priority-based elision
- Configurable detail levels

### 3.8 Synthesis Oracle (`synthesis.rs`)

Contract-directed code generation:
- Spec-driven synthesis from contracts
- Multiple strategy support
- Verification integration

---

## 4. Supporting Modules

### 4.1 Grammar Extensions (`grammar.rs`)

Extensible syntax via `grammar_extension!`:
- Extension registry with usage tracking
- Promotion from user-defined to built-in
- Namespace scoping

### 4.2 Legacy Compatibility (`legacy.rs`)

Bidirectional MechGen ↔ Rust transpilation:
- AST-level translation
- Contract preservation as attributes
- Effect annotation mapping

### 4.3 Manifest (`manifest.rs`)

`MechGen.toml` parsing:
- Package metadata
- Dependency declarations
- Agent configuration

### 4.4 Formatter (`fmt.rs`)

Canonical code formatter:
- Consistent style enforcement
- Token-minimal output
- Configurable width

### 4.5 RAP — REPL/Agent Protocol (`rap.rs`)

Interactive agent protocol:
- REPL commands
- Agent communication protocol
- Session management

### 4.6 CRDT (`crdt.rs`)

Conflict-free replicated data types for multi-agent collaboration:
- G-Counter, PN-Counter
- LWW-Register
- OR-Set

### 4.7 Consensus (`consensus.rs`)

Distributed decision-making:
- Proposal/vote/resolve
- Quorum-based decisions

### 4.8 Lease (`lease.rs`)

Temporary resource ownership:
- Lease acquisition and release
- Timeout-based expiration
- Transfer support

### 4.9 SKB — Shared Knowledge Base (`skb.rs`)

Persistent, queryable knowledge store:
- Key-value storage
- Pattern matching queries
- Agent-scoped access

### 4.10 Semantic VCS (`semantic_vcs.rs`)

Version control with semantic awareness:
- AST-level diffing
- Contract change tracking
- Effect change detection

### 4.11 FFI Generator (`ffi_gen.rs`)

Foreign function interface binding generation:
- C header generation
- Safe Rust wrappers
- Python stub generation
- WASM WIT interface generation

### 4.12 Forge Registry (`forge.rs`)

Package registry with semantic search:
- Capability-based search
- Trigram fuzzy matching
- Contract compatibility checking
- Dependency graph traversal

### 4.13 Performance Annotations (`perf_annot.rs`)

Compiler hints for optimization:
- `force_inline`, `no_block`, `vectorize(N)`
- `alignment(N)`, `pure`, `target_hint`
- MLIR hint emission

### 4.14 Stdlib Extensions (`stdlib_ext.rs`)

Agent-aware standard library extensions:
- `SwarmVec` — per-agent ownership tracking
- `ArenaVec` — bounded allocation
- `SwarmChannel` — typed MPMC channels
- `AgentArena` — per-agent arena allocators

### 4.15 Benchmarking (`bench.rs`)

Performance measurement infrastructure:
- Token throughput tracking
- Parse error rate monitoring
- Synthesis success rate
- Swarm latency metrics
- Benchmark runner with suite management

### 4.16 Cost Calibration (`cost_calibration.rs`)

Cost model accuracy validation:
- Measured vs. estimated comparisons
- Per-target calibration
- Accuracy grading (Excellent/Good/Fair/Poor)
- Standardised benchmark suite

### 4.17 Decompose (`decompose.rs`)

Code decomposition and refactoring:
- Function extraction
- Module splitting
- Dependency analysis

### 4.18 Heal (`heal.rs`)

Auto-healing and error recovery:
- Diagnostic-based suggestions
- Automatic fix application
- Multi-strategy healing

---

## 5. Testing

All modules include inline tests via `#[cfg(test)]` modules. Run the full
test suite with:

```bash
cd prototype
cargo test
```

Current test count: 640+ tests across 36 modules.

---

## 6. File Layout

```
prototype/
├── Cargo.toml
├── docs/
│   ├── LANGUAGE_SPEC.md     # Formal language specification
│   ├── BOOK.md              # User guide
│   ├── COOKBOOK.md           # Practical recipes
│   ├── AGENT_GUIDE.md       # Agent development guide
│   └── INTERNALS.md         # This file
└── src/
    ├── main.rs              # Entry point, module declarations
    ├── lexer.rs             # LL(1) lexer
    ├── parser.rs            # Recursive descent parser
    ├── ast.rs               # Abstract syntax tree
    ├── resolve.rs           # Name resolution
    ├── types.rs             # Type system
    ├── hir.rs               # High-level IR
    ├── mlir.rs              # MLIR emission
    ├── effects.rs           # Effect system
    ├── cost.rs              # Cost oracle
    ├── cost_calibration.rs  # Cost model calibration
    ├── verify.rs            # Verification
    ├── certs.rs             # Proof certificates
    ├── sandbox.rs           # Capability sandbox
    ├── hot_reload.rs        # Hot reload engine
    ├── swarm_bus.rs         # Swarm message bus
    ├── swarm_sdk.rs         # Swarm agent SDK
    ├── aci.rs               # Agent-Computer Interface
    ├── token_budget.rs      # Token budget management
    ├── elision.rs           # Code elision
    ├── synthesis.rs         # Synthesis oracle
    ├── grammar.rs           # Grammar extensions
    ├── legacy.rs            # Rust ↔ MechGen compat
    ├── manifest.rs          # MechGen.toml parsing
    ├── fmt.rs               # Code formatter
    ├── rap.rs               # REPL/Agent protocol
    ├── crdt.rs              # CRDTs
    ├── consensus.rs         # Consensus protocol
    ├── lease.rs             # Resource leases
    ├── skb.rs               # Shared Knowledge Base
    ├── semantic_vcs.rs      # Semantic version control
    ├── ffi_gen.rs           # FFI binding generator
    ├── forge.rs             # Package registry
    ├── perf_annot.rs        # Performance annotations
    ├── stdlib_ext.rs        # Stdlib extensions
    ├── bench.rs             # Benchmarking suite
    └── decompose.rs         # Code decomposition
```
