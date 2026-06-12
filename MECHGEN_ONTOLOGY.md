# MechGen: SWE Agent Communication Protocol & System Ontology

Version 1.0 — 2026-03-26

This document defines (1) a structured communication protocol for software engineering agents operating over the MechGen compiler, and (2) a complete formal ontology of every concept, type, relation, and invariant in the language and compiler system. Together they enable agents to reason about, navigate, modify, verify, and coordinate work on MechGen programs with full semantic precision.

---

## Part I: SWE Agent Communication Protocol

### 1. Protocol Overview

The MechGen SWE Agent Communication Protocol (SACP) is a structured message-passing protocol that enables autonomous software engineering agents to collaborate on MechGen programs. It operates over three transport layers:

| Layer           | Transport             | Latency | Use Case                                           |
| --------------- | --------------------- | ------- | -------------------------------------------------- |
| **RAP**         | JSON-RPC 2.0 over TCP | ~1 ms   | Compiler queries (parse, check, heal, cost)        |
| **SwarmBus**    | In-process pub/sub    | ~1 µs   | Inter-agent coordination (tasks, CRDTs, consensus) |
| **SemanticVCS** | OpLog commits         | Batch   | Persistent semantic change tracking                |

### 2. Agent Roles

Every agent in a MechGen swarm declares exactly one `Role`:

| Role            | Identifier              | Responsibility                                                      |
| --------------- | ----------------------- | ------------------------------------------------------------------- |
| Analyst         | `Role::Analyst`         | Decompose requirements into tasks; query cost oracle; assess impact |
| Implementer     | `Role::Implementer`     | Write MechGen code; invoke synthesis oracle; apply hot patches      |
| Reviewer        | `Role::Reviewer`        | Verify contracts; audit safety; check effect containment            |
| Verifier        | `Role::Verifier`        | Issue verification certificates; run SKB checks; validate proofs    |
| Orchestrator    | `Role::Orchestrator`    | Schedule tasks; balance load; manage consensus rounds               |
| Documentarian   | `Role::Documentarian`   | Generate manifests; maintain specs; produce token reports           |
| Refactorer      | `Role::Refactorer`      | Rename symbols; restructure modules; apply elision passes           |
| SecurityAuditor | `Role::SecurityAuditor` | Audit capabilities; review sandbox policies; verify FFI safety      |

### 3. Message Envelope

Every inter-agent message is wrapped in a typed envelope:

```
Envelope {
    id:          MessageId,        // u64, monotonically increasing
    sender:      AgentId,          // String, unique agent identifier
    recipient:   Recipient,        // Agent(id) | Broadcast | TopicSubscribers
    topic:       Topic,            // Typed routing key (see §3.1)
    payload:     Payload,          // Text(String) | Binary(Vec<u8>) | Map(BTreeMap) | Empty
    timestamp:   u64,              // Lamport clock tick
    correlation: CorrelationId,    // u64, links request/response pairs
    priority:    u8,               // 0 (lowest) – 255 (highest)
}
```

#### 3.1 Topic Taxonomy

| Topic              | Direction            | Payload               | Purpose                      |
| ------------------ | -------------------- | --------------------- | ---------------------------- |
| `LeaseRequest`     | Agent → LeaseManager | `{region, mode}`      | Request semantic region lock |
| `LeaseGrant`       | LeaseManager → Agent | `{lease_id, expires}` | Confirm lock acquisition     |
| `CrdtOp`           | Agent → All          | `StampedOp`           | Broadcast AST/HIR mutation   |
| `ConsensusPropose` | Agent → Orchestrator | `Proposal`            | Propose interface change     |
| `ConsensusVote`    | Agent → Orchestrator | `Vote`                | Cast vote on proposal        |
| `TaskAssign`       | Orchestrator → Agent | `Task`                | Assign work item             |
| `TaskComplete`     | Agent → Orchestrator | `TaskResult`          | Report task outcome          |
| `Diagnostic`       | Compiler → Agent     | `DiagnosticGraph`     | Emit structured error        |
| `Heartbeat`        | Agent → All          | `Empty`               | Liveness signal              |
| `Custom(String)`   | Any → Any            | Any                   | Domain-specific extension    |

### 4. Agent Lifecycle

```
                    ┌──────────────────────────────────────┐
                    │                                      │
  ┌─────────┐  register  ┌──────────┐  dispatch  ┌──────────────┐  complete  ┌───────────┐
  │ Created │──────────→│ Idle     │──────────→│ Working      │──────────→│ Idle      │
  └─────────┘           └──────────┘           └──────────────┘           └───────────┘
                             │                       │                         │
                         heartbeat              lease/acquire             lease/release
                             │                       │                         │
                          ┌──┴──┐              ┌─────┴──────┐            ┌─────┴──────┐
                          │ Bus │              │ LeaseManager│            │ SemanticVCS│
                          └─────┘              └────────────┘            └────────────┘
```

1. **Register**: Agent publishes `AgentDescriptor` to `Orchestrator` via `swarm_sdk`.
2. **Subscribe**: Agent subscribes to relevant `Topic`s on the `MessageBus`.
3. **Acquire Lease**: Before modifying code, agent requests `SemanticRegion` lease.
4. **Work**: Agent performs tasks, emitting `CrdtOp` mutations.
5. **Commit**: Agent commits `SemanticOp`s to `OpLog` in `SemanticVCS`.
6. **Release Lease**: Agent releases semantic region lock.
7. **Report**: Agent sends `TaskResult` to `Orchestrator`.

### 5. RAP (MechGen Agent Protocol) — Compiler Interface

RAP exposes 24 JSON-RPC 2.0 endpoints. Each request is a JSON object with `method`, `params`, `id`:

#### 5.1 Language Services

| Method            | Params           | Returns        | Description                   |
| ----------------- | ---------------- | -------------- | ----------------------------- |
| `language/tokens` | `{source, mode}` | `Vec<Token>`   | Tokenize source in given mode |
| `language/parse`  | `{source, mode}` | `Module` (AST) | Parse to full AST             |

#### 5.2 Build Services

| Method        | Params     | Returns                 | Description                 |
| ------------- | ---------- | ----------------------- | --------------------------- |
| `build/check` | `{source}` | `Vec<Diagnostic>`       | Full syntax + type check    |
| `build/heal`  | `{source}` | `Vec<HealedDiagnostic>` | Check + auto-fix candidates |

#### 5.3 Cost & Token Services

| Method         | Params                           | Returns          | Description                    |
| -------------- | -------------------------------- | ---------------- | ------------------------------ |
| `cost/query`   | `{construct, target, opt_level}` | `CostEstimate`   | Per-construct cost query       |
| `cost/compare` | `{a, b, target, opt_level}`      | `CostComparison` | Compare two constructs         |
| `token/report` | `{source}`                       | `TokenReport`    | Per-item token budget analysis |

#### 5.4 Safety & Verification Services

| Method             | Params                 | Returns                   | Description                 |
| ------------------ | ---------------------- | ------------------------- | --------------------------- |
| `skb/query`        | `{database, category}` | `Vec<Rule>`               | Query Safety Knowledge Base |
| `skb/rules`        | `{}`                   | `Vec<Rule>`               | List all 255 safety rules   |
| `verify/contracts` | `{fqn, spec, effects}` | `VerificationResult`      | Verify function contracts   |
| `verify/module`    | `{source}`             | `Vec<VerificationResult>` | Verify entire module        |

#### 5.5 Effect Services

| Method          | Params     | Returns            | Description                 |
| --------------- | ---------- | ------------------ | --------------------------- |
| `effects/infer` | `{source}` | `EffectAnalysis`   | Infer all effects bottom-up |
| `effects/check` | `{source}` | `Vec<EffectCheck>` | Check declared vs. inferred |

#### 5.6 Formatting & Elision Services

| Method          | Params     | Returns           | Description                 |
| --------------- | ---------- | ----------------- | --------------------------- |
| `format/agent`  | `{source}` | `String`          | Format to agent mode syntax |
| `format/human`  | `{source}` | `String`          | Format to human mode syntax |
| `elision/apply` | `{source}` | `String`          | Apply safety elision pass   |
| `lint/check`    | `{source}` | `Vec<Diagnostic>` | Lint diagnostics            |

#### 5.7 Agent & Capability Services

| Method             | Params               | Returns           | Description              |
| ------------------ | -------------------- | ----------------- | ------------------------ |
| `capability/check` | `{agent, operation}` | `bool`            | Check agent capability   |
| `sandbox/policy`   | `{agent}`            | `ResourceLimits`  | Get agent sandbox policy |
| `heal/graph`       | `{diagnostic}`       | `DiagnosticGraph` | Rich diagnostic graph    |

#### 5.8 Attribute & Documentation Services

| Method               | Params     | Returns  | Description                  |
| -------------------- | ---------- | -------- | ---------------------------- |
| `attribute/expand`   | `{source}` | `String` | Expand compressed attributes |
| `attribute/compress` | `{source}` | `String` | Compress to sigil attributes |
| `doc/query`          | `{symbol}` | `String` | Symbol documentation lookup  |

### 6. Concurrency Protocol

#### 6.1 Semantic Leases

Agents must acquire a `Lease` before modifying any `SemanticRegion`:

```
LeaseMode:
  SharedRead        — Multiple agents can read concurrently
  ExclusiveWrite    — Single agent writes; blocks all others
  Restructuring     — Exclusive access to rename/move operations
```

Compatibility matrix:

| Held \ Requested | SharedRead | ExclusiveWrite | Restructuring |
| ---------------- | ---------- | -------------- | ------------- |
| SharedRead       | ✓          | ✗              | ✗             |
| ExclusiveWrite   | ✗          | ✗              | ✗             |
| Restructuring    | ✗          | ✗              | ✗             |

The `LeaseManager` performs deadlock detection via wait-for graph analysis.

#### 6.2 CRDT Merge Protocol

When multiple agents mutate the same module concurrently:

1. Each agent emits `StampedOp { clock: LamportClock, agent: AgentId, op: CrdtOp }`.
2. Operations are broadcast via `Topic::CrdtOp`.
3. `CrdtState` merges deterministically using Last-Writer-Wins with Lamport ordering.
4. Conflicts resolve to `MergeOutcome::ResolvedLWW { winner, loser }`.
5. `MergeLog` records all outcomes; agents can query `.conflicts()`.

Available CRDT operations:

| CrdtOp                                                           | Target   | Semantics        |
| ---------------------------------------------------------------- | -------- | ---------------- |
| `InsertItem { name, source }`                                    | Module   | Add new item     |
| `RemoveItem { name }`                                            | Module   | Delete item      |
| `ModifyBody { function_name, new_body }`                         | Function | Replace body     |
| `ModifySignature { function_name, new_params, new_return_type }` | Function | Change signature |
| `AddImpl { target_type, impl_source }`                           | Type     | Add impl block   |
| `Rename { old_name, new_name }`                                  | Symbol   | Rename symbol    |

#### 6.3 Five-Phase Consensus

For breaking changes (interface modifications, shared contract changes):

```
Phase 1: Propose         — Agent submits Proposal with affected_regions
Phase 2: ImpactAnalysis  — Agents submit ImpactReport (breaking? affected regions?)
Phase 3: Vote            — Each voter casts Accept | Reject | Abstain
Phase 4: Resolve         — Quorum check → Decision (Accepted | Rejected | NoQuorum)
Phase 5: Integrate       — Accepted changes are applied atomically
```

### 7. Task Decomposition & Scheduling

The `TaskDag` manages dependency-aware parallel work:

```
Task {
    id:                    TaskId,
    name:                  String,
    cost:                  u64,         // Estimated token cost
    required_capabilities: Vec<String>,
    state:                 TaskState,   // Pending → Ready → InProgress → Completed | Blocked
    assigned_to:           Option<AgentId>,
}
```

The `Orchestrator` provides:
- `dispatch(task, payload)` → assigns to capable agent
- `dispatch_with_review(task, payload)` → assigns + requires `Role::Reviewer` approval
- `parallel_waves()` → extracts independent task groups for parallel execution
- `critical_path()` → identifies longest dependency chain

### 8. Diagnostic Protocol

Every compiler diagnostic is a structured graph:

```
DiagnosticGraph {
    root:              Diagnostic,
    context:           Vec<DiagnosticNode>,  // Note | Help | CausalChain
    fixes:             Vec<Fix>,
    related:           Vec<DiagnosticNode>,
    documentation_url: Option<String>,
}

Fix {
    description:         String,
    applicability:       Applicability,  // MachineApplicable | MaybeIncorrect | HasPlaceholders | Unspecified
    preconditions:       Vec<String>,
    postconditions:      Vec<String>,
    side_effects:        Vec<String>,
    confidence:          f64,           // 0.0–1.0
}
```

Agents process diagnostics in priority order: `Error > Warning > Info`. The `heal` subsystem generates ranked `FixCandidate`s with confidence scores and token costs for each.

### 9. Verification Certificate Exchange

After verification, the `CertificateStore` issues machine-checkable proofs:

```
Certificate {
    id:        CertId,
    kind:      CertKind,      // MemorySafety | DataRaceFreedom | ContractSatisfaction | EffectContainment
    target:    String,         // Fully-qualified function or module name
    verifier:  String,         // Agent that performed verification
    steps:     Vec<ProofStep>, // Axiom(String) | Derivation{rule,premises,conclusion} | Witness{source,claim}
    timestamp: u64,
    valid:     bool,
}
```

Certificates can be queried by target, kind, or verifier. Invalid certificates are revoked immediately upon upstream change detection.

### 10. Hot Reload Protocol

For live function patching without full recompilation:

```
1. Agent creates PatchUnit { function_name, module_path, old_body, new_body }
2. HotReloadEngine.validate(patch_id) → ValidationResult
     Ok
     SignatureMismatch { expected, got }
     ContractViolation(String)
     TypeCheckFailure(String)
     EffectEscalation { old_effects, new_effects }
3. If Ok: HotReloadEngine.apply(patch_id) → updates PatchStatus to Applied
4. On failure: HotReloadEngine.rollback(patch_id) → restores old_body
5. MLIR stub re-lowered for patched function only
```

---

## Part II: Complete System Ontology

### 11. Ontology Structure

The MechGen ontology is organized into 12 interconnected domains. Each domain defines concepts (types), relations (how concepts connect), and invariants (properties that always hold).

```
┌─────────────────────────────────────────────────────────────────────┐
│                        MECHGEN ONTOLOGY                             │
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Lexical  │→│ Syntactic│→│ Semantic │→│ Type     │           │
│  │ Domain   │  │ Domain   │  │ Domain   │  │ Domain   │           │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘           │
│       ↓             ↓             ↓             ↓                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Effect   │  │ Safety   │  │ AI       │  │ Agent    │           │
│  │ Domain   │  │ Domain   │  │ Domain   │  │ Domain   │           │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘           │
│       ↓             ↓             ↓             ↓                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐           │
│  │ Codegen  │  │ Verify   │  │ Tooling  │  │ Package  │           │
│  │ Domain   │  │ Domain   │  │ Domain   │  │ Domain   │           │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 12. Domain 1: Lexical

The lexical domain defines all tokens, their classification, and how source text maps to token streams under two modes.

#### 12.1 Concepts

**Mode** — Syntax encoding mode.
- `Human` — Rust keyword surface (default).
- `Agent` — Compressed sigil surface (`#![syntax(agent)]`).

**Token** — Atomic lexical unit.
```
Token { kind: TokenKind, span: Span, text: String }
Span  { offset: usize, len: usize, line: usize, col: usize }
```

**TokenKind** — 168-variant enumeration, classified into 14 categories:

| Category             | Variants                                                                                                                                                                                                                                                                                                                             | Count |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----- |
| Declaration Keywords | `KwF`, `KwAf`, `KwUf`, `KwV`, `KwM`, `KwC`, `KwS`, `KwE`, `KwT`, `KwI`, `KwMod`, `KwU`, `KwUse`, `KwY`, `KwZ`                                                                                                                                                                                                                        | 15    |
| Visibility Modifiers | `Plus`, `TildePre`                                                                                                                                                                                                                                                                                                                   | 2     |
| Control Flow         | `Question`, `QuestionEq`, `At`, `AtAt`, `AtW`, `KwRet`, `KwYield`, `DoubleArrowR`, `Bang`, `KwOr`                                                                                                                                                                                                                                    | 10    |
| Booleans             | `True`, `False`                                                                                                                                                                                                                                                                                                                      | 2     |
| Result/Option        | `KwOk`, `KwErr`, `KwSome`, `KwNone`                                                                                                                                                                                                                                                                                                  | 4     |
| Loop/Branch          | `KwFor`, `KwLoop`, `KwBreak`, `KwContinue`, `KwOf`                                                                                                                                                                                                                                                                                   | 5     |
| Safety & Modifiers   | `KwUnsafe`, `KwType`, `KwStatic`, `Todo`, `Unimplemented`                                                                                                                                                                                                                                                                            | 5     |
| Effect/Contract      | `KwEffect`, `KwHandle`, `KwSpec`, `KwAgent`, `KwSwarm`, `KwExtern`, `KwReq`, `KwEns`, `KwInv`, `KwFx`, `KwPerf`                                                                                                                                                                                                                      | 11    |
| Neural Keywords      | `KwNet`, `KwLayer`, `KwTensor`, `KwParam`, `KwTrain`, `KwGrad`, `KwForward`                                                                                                                                                                                                                                                          | 7     |
| KB Keywords          | `KwKb`, `KwFact`, `KwRule`, `KwQuery`                                                                                                                                                                                                                                                                                                | 4     |
| Evolution Keywords   | `KwEvolve`, `KwGenome`, `KwMutate`, `KwFitness`, `KwSelect`, `KwCrossover`, `KwPopulation`, `KwGenerations`                                                                                                                                                                                                                          | 8     |
| RL Keywords          | `KwRl`, `KwPolicy`, `KwReward`                                                                                                                                                                                                                                                                                                       | 3     |
| Greek Symbols        | `KwPsi` (Ψ), `KwLambda` (λ), `KwPhi` (Φ), `KwPi` (Π), `KwTheta` (Θ), `KwNabla` (∇), `KwAlpha` (α), `KwSigma` (Σ), `KwKappa` (κ), `KwRho` (ρ), `KwOmega` (Ω), `KwGammaGreek` (Γ), `KwPhiLower` (φ), `KwXi` (Ξ), `KwMu` (μ), `KwChi` (χ)                                                                                               | 16    |
| Tensor Operators     | `TensorMatmul` (⊗), `TensorHadamard` (⊙), `TensorTranspose` (⊤), `TensorFlatten` (⊥), `TensorPipeline` (▸)                                                                                                                                                                                                                           | 5     |
| Swarm Patterns       | `KwSwarmMapReduce`, `KwSwarmPipeline`, `KwSwarmSaga`, `KwSwarmFanOut`, `KwSwarmRace`                                                                                                                                                                                                                                                 | 5     |
| Literals             | `IntLiteral`, `FloatLiteral`, `StringLiteral`, `FormatString`, `PrintString`, `EprintString`, `CharLiteral`, `ByteLiteral`, `ByteStringLiteral`, `Ident`                                                                                                                                                                             | 10    |
| Operators            | `Plus`, `Minus`, `Star`, `Slash`, `Percent`, `Eq`, `Neq`, `Lt`, `Gt`, `Le`, `Ge`, `And`, `Or`, `Not`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Assign`, `PlusEq`, `MinusEq`, `StarEq`, `SlashEq`, `PercentEq`, `BitAndEq`, `BitOrEq`, `BitXorEq`, `ShlEq`, `ShrEq`, `AndNot`, `AndTilde`, `PercentNot`, `HashTilde`, `TildeArrow` | 35    |
| Delimiters           | `LParen`, `RParen`, `LBrace`, `RBrace`, `LBrack`, `RBrack`                                                                                                                                                                                                                                                                           | 6     |
| Punctuation          | `Semi`, `Comma`, `Dot`, `Colon`, `ColonQuestion`, `Arrow`, `FatArrow`, `Hash`, `DotDot`, `DotDotEq`, `Tilde`, `Dollar`                                                                                                                                                                                                               | 12    |
| Special              | `Eof`, `Error`, `Whitespace`, `Comment`, `Underscore`, `UnderscoreT`, `KwPipeline`, `KwGrammarExt`                                                                                                                                                                                                                                   | 8     |

#### 12.2 Relations

- **mode_maps(Mode, String, TokenKind)** — A keyword string maps to a TokenKind under a given Mode.
- **spans(Token, Span)** — A token occupies a source span.

#### 12.3 Mode Mapping (Keyword Bijection)

Human mode uses Rust keywords; Agent mode uses compressed sigils. Both map to the same `TokenKind`:

| TokenKind      | Human Keyword | Agent Sigil |
| -------------- | ------------- | ----------- |
| `KwF`          | `fn`          | `f`         |
| `KwAf`         | `async`       | `af`        |
| `KwUf`         | `unsafe` (fn) | `uf`        |
| `Plus`         | `pub`         | `+`         |
| `KwV`          | `let`         | `v`         |
| `KwM`          | `mut`         | `m`         |
| `KwC`          | `const`       | `c`         |
| `KwS`          | `struct`      | `S`         |
| `KwE`          | `enum`        | `E`         |
| `KwT`          | `trait`       | `T`         |
| `KwI`          | `impl`        | `I`         |
| `KwMod`        | `mod`         | `M`         |
| `KwUse`        | `use`         | `u`         |
| `KwY`          | `type`        | `Y`         |
| `KwZ`          | `static`      | `Z`         |
| `Question`     | `if`          | `?`         |
| `QuestionEq`   | `match`       | `?=`        |
| `At`           | `for`         | `@`         |
| `AtAt`         | `loop`        | `@@`        |
| `AtW`          | `while`       | `@w`        |
| `KwOr`         | `else`        | `:`         |
| `KwOf`         | `in`          | `~`         |
| `KwRet`        | `return`      | `ret`       |
| `Bang`         | `break`       | `!`         |
| `DoubleArrowR` | `continue`    | `>>`        |
| `TildeArrow`   | `where`       | `~>`        |

#### 12.4 Invariants

- **INV-L1**: Every valid source string tokenizes to a finite, deterministic token sequence.
- **INV-L2**: `format_agent(parse(source)) ≡ format_agent(parse(format_human(parse(source))))` (round-trip).
- **INV-L3**: Token span ranges are non-overlapping and cover the entire source.
- **INV-L4**: Mode switching is determined lexically by `#![syntax(agent)]` at file head.

### 13. Domain 2: Syntactic

The syntactic domain defines the abstract syntax tree (AST) — the structural representation of all MechGen programs.

#### 13.1 Concepts

**Module** — Top-level compilation unit.
```
Module { items: Vec<Item> }
```

**Item** — A top-level declaration.
```
Item { visibility: Visibility, attributes: Vec<Attribute>, kind: ItemKind }
```

**Visibility** — Access control.
- `Private` — Module-internal.
- `Public` — Exported.

**Attribute** — Metadata annotation.
```
Attribute { name: String, args: Vec<String>, bang: bool }
```

**ItemKind** — 18 declaration forms, partitioned into three strata:

| Stratum       | Variants                                                                                       | Source             |
| ------------- | ---------------------------------------------------------------------------------------------- | ------------------ |
| Core Language | `Function`, `Struct`, `Enum`, `Trait`, `Impl`, `Module`, `Use`, `TypeAlias`, `Const`, `Static` | Rust heritage      |
| Effect System | `Effect`, `Spec`                                                                               | MechGen extensions |
| AI Constructs | `Agent`, `Net`, `Kb`, `Evolve`, `Train`, `Swarm`                                               | MechGen AI layer   |

**ExprKind** — 31 expression forms:

| Category         | Variants                                                                      |
| ---------------- | ----------------------------------------------------------------------------- |
| Atoms            | `Literal`, `Ident`                                                            |
| Arithmetic/Logic | `Binary`, `Unary`                                                             |
| Invocation       | `Call`, `MethodCall`                                                          |
| Access           | `FieldAccess`, `Index`                                                        |
| Construction     | `StructLit`, `TupleLit`, `ArrayLit`, `ArrayRepeat`                            |
| Functions        | `Closure`                                                                     |
| Control Flow     | `If`, `Match`, `Loop`, `While`, `For`, `Block`, `Return`, `Break`, `Continue` |
| Async            | `Await`, `Try`                                                                |
| Type             | `Cast`                                                                        |
| Mutation         | `Assign`                                                                      |
| Range            | `Range`                                                                       |
| Safety           | `UnsafeBlock`                                                                 |
| Placeholder      | `Todo`, `Unimplemented`, `Error`                                              |

**Statement** — 3 forms:
- `Let { mutable, pattern, ty, value }` — Binding introduction.
- `Expr { expr }` — Expression statement.
- `Item { item }` — Nested item declaration.

**Pattern** — 9 forms: `Ident`, `Literal`, `Wildcard`, `Tuple`, `Struct`, `Enum`, `Slice`, `Or`, `Ref`.

**Type (AST)** — 32 surface-level type expressions:

| Category            | Variants                                                            |
| ------------------- | ------------------------------------------------------------------- |
| Named               | `Path`                                                              |
| References          | `Reference`, `Ptr`                                                  |
| Smart Pointers      | `OwnedPtr`, `Rc`, `Arc`, `Cow`                                      |
| Interior Mutability | `Cell`, `RefCell`, `Mutex`, `RwLock`                                |
| Collections         | `Slice`, `Array`, `Vec`, `Set`, `Map`, `Tuple`                      |
| Error Handling      | `Option`, `Result`                                                  |
| SIMD                | `Simd`                                                              |
| AI                  | `Tensor`, `ParamTy`, `Genome`, `Policy`, `KnowledgeBase`, `LlmType` |
| Functions           | `Fn`                                                                |
| Special             | `Never`, `Inferred`, `SelfType`, `StringType`, `Refined`            |

#### 13.2 AI Construct Definitions

**NetDef** — Neural network declaration.
```
NetDef { name, generics, layers: Vec<LayerDef>, forward: Option<Block> }
LayerDef { name, layer_type, args }
```

**KbDef** — Knowledge base declaration (Datalog).
```
KbDef { name, facts: Vec<FactDef>, rules: Vec<RuleDef> }
FactDef { name, args: Vec<String> }
RuleDef { name, params, conditions, body }
```

**EvolveDef** — Evolutionary computation block.
```
EvolveDef { name, genome_type, population_size, generations, fitness, mutate_fn, crossover_fn, select_fn }
```

**TrainDef** — Training loop declaration.
```
TrainDef { name, net, optimizer, loss, epochs, body }
```

**AgentDef** — Autonomous agent declaration.
```
AgentDef { name, capabilities: Vec<String>, requires_approval: bool }
```

**SwarmDef** — Multi-agent swarm declaration.
```
SwarmDef { name, agent_type, size, topology, consensus, on_dispatch, on_aggregate, on_failure }
```

#### 13.3 Contract & Effect Declarations

**EffectDef** — Algebraic effect declaration.
```
EffectDef { name, operations: Vec<EffectOp> }
EffectOp { name, params, return_type }
```

**SpecDef** — Specification/contract block.
```
SpecDef { name, generics, params, return_type, items: Vec<SpecItem> }
SpecItem: Require(String) | Ensure(String) | Performance(String, String) | Effect(Vec<String>) | Invariant(String)
```

**FunctionDef** — carries contracts and effects inline.
```
FunctionDef {
    name, is_async, is_unsafe, generics, params, return_type,
    where_clause: Vec<WherePredicate>,
    effects: Vec<String>,              // / io, gpu, llm
    contracts: Vec<ContractClause>,    // @req, @ens, @inv
    body: Block,
}
ContractClause { kind: ContractClauseKind, condition, message }
ContractClauseKind: Requires | Ensures | Invariant
```

#### 13.4 Tensor Dimensions

```
TensorDim: Lit(u64) | Var(String)
```

Enables static shape checking: `Tensor[f32; 3, 224, 224]` has literal dims; `Tensor[f32; B, N]` has variable dims resolved during shape inference.

#### 13.5 Invariants

- **INV-S1**: The grammar is LL(1)-parseable — zero backtracking, one-token lookahead.
- **INV-S2**: Every `ItemKind` has a unique leading token to disambiguate.
- **INV-S3**: `parse(format_human(ast)) ≡ ast` (formatting preserves AST identity).
- **INV-S4**: All 18 `ItemKind` variants are structurally distinct at the AST level regardless of mode.

### 14. Domain 3: Semantic (Name Resolution)

#### 14.1 Concepts

**Symbol** — A resolved named entity.
```
Symbol { id: SymbolId, name: String, kind: SymbolKind, ty: Option<Ty> }
```

**SymbolId** — Unique, opaque identifier: `SymbolId(u32)`.

**SymbolKind** — 19 kinds:
- `Function`, `Struct`, `Enum`, `EnumVariant { parent: SymbolId }`, `Trait`, `Module`, `TypeAlias`, `Const`
- `Effect`, `Spec`, `Agent`, `Swarm`, `Net`, `Kb`, `Evolve`, `Train`
- `Variable { mutable: bool }`, `Param`, `GenericParam`

**Scope** — A lexical scope containing name and type mappings.
```
Scope { names: HashMap<String, SymbolId>, types: HashMap<String, SymbolId> }
```

**Resolver** — Name resolution engine.
```
Resolver { symbols: SymbolTable, diagnostics, resolved, scopes: Vec<Scope> }
```

#### 14.2 Relations

- **resolves_to(Name, SymbolId)** — An identifier resolves to a symbol.
- **scoped_in(SymbolId, Scope)** — A symbol is defined in a scope.
- **parent_of(SymbolId, SymbolId)** — Enum variant → parent enum.

#### 14.3 Invariants

- **INV-N1**: Every identifier in a well-formed program resolves to exactly one `SymbolId`.
- **INV-N2**: No two symbols in the same scope share a name (for the same namespace).
- **INV-N3**: `EnumVariant` always has a valid `parent` pointing to an `Enum` symbol.

### 15. Domain 4: Type System

#### 15.1 Concepts

**Ty (HIR)** — 31-variant semantic type after resolution and inference:

| Category       | Variants                                                                                                                           |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Primitives     | `Int(IntTy)`, `Uint(UintTy)`, `Float(FloatTy)`, `Bool`, `Str`, `Char`, `Unit`, `Never`                                             |
| Named          | `Named(SymbolId, Vec<Ty>)`                                                                                                         |
| References     | `Ref(mutable: bool, Box<Ty>)`, `Ptr(Box<Ty>)`                                                                                      |
| Smart Pointers | `OwnedPtr`, `Rc`, `Arc`                                                                                                            |
| Collections    | `Slice`, `Array(Ty, u64)`, `Vec`, `Tuple`, `Map(K, V)`                                                                             |
| Error Handling | `Option`, `Result(Ok, Err)`                                                                                                        |
| Concurrency    | `Simd(Ty, u64)`                                                                                                                    |
| AI Types       | `Tensor(Ty, Vec<TensorDimHir>)`, `Param(Ty, Vec<TensorDimHir>)`, `Genome(Ty)`, `Policy(State, Action)`, `KnowledgeBase`, `LlmType` |
| Functions      | `Fn(Vec<Ty>, Box<Ty>, EffectSet)`                                                                                                  |
| Inference      | `Var(TyVar)`, `Error`                                                                                                              |

**IntTy**: `I8`, `I16`, `I32`, `I64`, `I128`, `Isize`.
**UintTy**: `U8`, `U16`, `U32`, `U64`, `U128`, `Usize`.
**FloatTy**: `F32`, `F64`.
**TyVar**: `TyVar(u32)` — Unification variable.

**TypeChecker** — Bidirectional HM-style type checker.
```
TypeChecker { supply: TyVarSupply, subst: Subst, env: TypeEnv, struct_defs, fn_sigs, diagnostics }
```

**Subst** — Substitution map: `TyVar → Ty`.

#### 15.2 Unification Algorithm

Robinson's algorithm with occurs-check:
1. `unify(Var(a), t)` → bind `a := t` if `a ∉ FV(t)`.
2. `unify(t, Var(a))` → symmetric.
3. `unify(C(args1), C(args2))` → unify pairwise if same constructor.
4. Tensor-specific: dimension variables unify symbolically (`B = B`, `Lit(n) = Lit(n)`).

#### 15.3 Relations

- **has_type(SymbolId, Ty)** — A symbol has a resolved type.
- **subtype_of(Ty, Ty)** — Subtyping relation (structural for generics).
- **unifies(TyVar, Ty)** — Unification variable binds to a type.

#### 15.4 Invariants

- **INV-T1**: After type checking, no `Ty::Var` remains unbound (all variables resolved).
- **INV-T2**: `Tensor[T; d1, ..., dn]` shapes are statically verified by shape inference.
- **INV-T3**: Function types carry their `EffectSet` — `Fn(params, ret, effects)`.
- **INV-T4**: Occurs-check prevents infinite types.

### 16. Domain 5: Effect System

#### 16.1 Concepts

**Effect** — 16 effect kinds:

| Effect           | Trigger                         | Description                |
| ---------------- | ------------------------------- | -------------------------- |
| `IO`             | `print`, `println`, `eprint`    | Console I/O                |
| `FS`             | `read`, `open`, `write`         | File system                |
| `Net`            | Network operations              | Network I/O                |
| `Async`          | `spawn`, `join`, `.await`       | Concurrency                |
| `Alloc`          | `alloc`, `Box::new`, `Vec::new` | Heap allocation            |
| `Panic`          | `panic!`, `unwrap`, `expect`    | Divergence                 |
| `FFI`            | `extern` blocks                 | Foreign function calls     |
| `Env`            | `env::var`, `env::args`         | Environment access         |
| `Time`           | `Instant::now`, `SystemTime`    | Time access                |
| `Gpu`            | GPU dispatch                    | GPU computation            |
| `Npu`            | NPU dispatch                    | Neural processing unit     |
| `Llm`            | LLM inference                   | Large language model calls |
| `Evolve`         | Evolutionary blocks             | Evolutionary computation   |
| `Learn`          | Training loops                  | Model training             |
| `Rng`            | Random number generation        | Non-determinism            |
| `Custom(String)` | User-defined                    | Extensible effects         |

**EffectSet** — `BTreeSet<Effect>` — ordered, deduplicated set of effects.

**EffectInfer** — Bottom-up inference engine.
```
EffectInfer { declared, inferred, call_graph, in_progress, diagnostics }
```

#### 16.2 Inference Algorithm

```
1. Initialize: declared[f] = effects from function annotation (/ io, / gpu)
2. For each function f in reverse call-graph order:
     inferred[f] = local_effects(f) ∪ ⋃{inferred[g] | f calls g}
3. Check: inferred[f] ⊆ declared[f]
     If violation: emit UndeclaredEffect diagnostic
4. Check: declared[f] ⊆ inferred[f]
     If violation: emit UnusedEffect warning
```

#### 16.3 Invariants

- **INV-E1**: Pure functions (no effect annotation) have `inferred = ∅`.
- **INV-E2**: Effect inference is monotone — adding calls can only grow the effect set.
- **INV-E3**: `EffectSet` is closed under union.

### 17. Domain 6: Safety (SKB)

The Safety Knowledge Base encodes 255 rules across 8 databases.

#### 17.1 Concepts

**Rule** — A single safety rule.
```
Rule { id, database: RuleDatabase, category, severity: RuleSeverity, description, rationale, fix_template, fix_confidence, tags }
```

**RuleDatabase** — 8 domains:

| Database       | Rule Count | Scope                                                          |
| -------------- | ---------- | -------------------------------------------------------------- |
| `Ownership`    | 40         | Use-after-move, double-move, partial-move, drop-while-borrowed |
| `Borrow`       | 40         | Exclusive/shared violations, aliasing                          |
| `Lifetime`     | 35         | Dangling references, lifetime mismatch                         |
| `TypeSafety`   | 40         | Type mismatches, unsafe casts                                  |
| `Concurrency`  | 35         | Data races, deadlocks, Send/Sync violations                    |
| `FFI`          | 20         | Pointer safety, ABI correctness                                |
| `AgentElision` | 30         | Rules the compiler handles automatically in agent mode         |
| `SwarmSafety`  | 15         | Consensus, topology, fault tolerance                           |

**RuleSeverity**: `Error`, `Warning`, `Info`, `Hint`.

#### 17.2 Agent Elision Rules

In agent mode, the compiler applies `AgentElision` rules to automatically handle:

| Elided Construct        | SKB Rule | Compiler Action                    |
| ----------------------- | -------- | ---------------------------------- |
| `unsafe { ... }`        | AEL-0001 | Verify via OWN/BOR/FFI databases   |
| `unsafe fn`             | AEL-0002 | Detect from body analysis          |
| Lifetime `'a`           | AEL-0003 | Infer via LIF rules                |
| `&mut T` annotations    | AEL-0004 | Determine mutability automatically |
| `Send` / `Sync` bounds  | AEL-0005 | Derive from type structure         |
| `move` on closures      | AEL-0006 | Infer capture mode                 |
| `Pin<T>` wrapping       | AEL-0007 | Auto-wrap self-referential types   |
| `dyn` / `impl` dispatch | AEL-0008 | Select from call-site analysis     |
| `PhantomData<T>`        | AEL-0012 | Insert marker automatically        |

#### 17.3 Invariants

- **INV-SK1**: Every diagnostic emitted by the compiler references at least one SKB rule ID.
- **INV-SK2**: Fix templates have confidence ∈ [0.0, 1.0].
- **INV-SK3**: The 255 rules are complete — all known Rust safety violations are covered.

### 18. Domain 7: AI Subsystems

Four compiler-integrated AI subsystems, each operating as a pipeline phase.

#### 18.1 Shape Inference

**Concepts**:
- `ShapeDim`: `Lit(u64)` | `Var(String)` — Static or symbolic dimension.
- `Shape`: `Vec<ShapeDim>` — Ordered dimension list.

**Operations**:
- `broadcast(a: &[ShapeDim], b: &[ShapeDim]) -> Result<Vec<ShapeDim>>` — NumPy-style broadcasting.
- `matmul_shape(a, b) -> Result<Vec<ShapeDim>>` — `[M,K] ⊗ [K,N] → [M,N]`.
- `transpose(shape) -> Vec<ShapeDim>` — Reverse dimension order.
- `reshape(shape, target) -> Result<Vec<ShapeDim>>` — Reshape with element count preservation.
- `conv2d_output(input, kernel, stride, padding) -> Vec<ShapeDim>` — Convolution output dimensions.

**Invariants**:
- **INV-SH1**: Broadcasting is right-aligned; mismatched non-1 dims produce error.
- **INV-SH2**: Matmul requires inner dimensions to match.
- **INV-SH3**: Reshape preserves total element count: `∏(old_dims) = ∏(new_dims)`.

#### 18.2 Automatic Differentiation

**Concepts**:
- `DiffOp` — 20+ differentiable operations: `Add`, `Sub`, `Mul`, `Div`, `MatMul`, `Transpose`, `Sum`, `Mean`, `ReLU`, `Sigmoid`, `Tanh`, `Softmax`, `LogSoftmax`, `CrossEntropy`, `MSE`, `L1Loss`, `Conv2d`, `MaxPool2d`, `BatchNorm`, `Dropout`, `LayerNorm`, `Exp`, `Log`, `Neg`, `Abs`, `Pow`.
- `TapeEntry` — Wengert list entry: `{ op, inputs, output, shape }`.
- `Tape` — Forward-pass computation graph.

**Algorithm**:
```
Forward:  Build tape (Wengert list) of operations
Backward: Reverse topological sort → apply chain rule per op
Output:   MLIR MechGen.grad.* operations
```

**Invariants**:
- **INV-AD1**: Every `DiffOp` has a defined adjoint (backward) rule.
- **INV-AD2**: Backward pass visits ops in reverse topological order.
- **INV-AD3**: Gradient shapes match parameter shapes.

#### 18.3 Symbolic Reasoning (Datalog)

**Concepts**:
- `Term`: `Atom(String)` | `Variable(String)` | `Wildcard` — Logical term.
- `Atom` (logical): `{ predicate, args: Vec<Term> }` — A ground or non-ground fact.
- `Rule` (logical): `{ head: Atom, body: Vec<Atom> }` — Horn clause.
- `KnowledgeBase` (runtime): `{ facts, rules }` — Logical KB.

**Algorithm**: Semi-naive bottom-up evaluation.
```
1. Initialize: known = facts
2. Repeat (≤1000 iterations):
     delta = apply_rules(rules, known) − known
     known = known ∪ delta
   Until delta = ∅ (fixpoint reached)
3. Query: filter known by predicate and argument patterns
```

**Invariants**:
- **INV-KB1**: Evaluation terminates in ≤1000 iterations.
- **INV-KB2**: Evaluation is monotone — facts only grow.
- **INV-KB3**: Query results are complete w.r.t. the fixpoint.

#### 18.4 Evolutionary Computation

**Concepts**:
- `Individual` — `{ genome: Vec<f64>, fitness: f64 }`.
- `SelectionMethod`: `Tournament(k)`, `Roulette`, `Rank`, `Elitist(count)`.
- `CrossoverMethod`: `SinglePoint`, `TwoPoint`, `Uniform(rate)`.
- `MutationMethod`: `BitFlip(rate)`, `Gaussian(sigma)`, `Swap(rate)`.
- `EvolutionConfig` — Population size, generations, methods, target fitness.

**Algorithm**:
```
1. Initialize random population of N individuals
2. For each generation:
   a. Evaluate fitness for all individuals
   b. Check termination (target fitness reached?)
   c. Select parents via SelectionMethod
   d. Create offspring via CrossoverMethod
   e. Apply MutationMethod
   f. Replace population
3. Return best individual
```

**Invariants**:
- **INV-EV1**: Population size is constant across generations.
- **INV-EV2**: Elitist selection preserves the top-k individuals unchanged.
- **INV-EV3**: Mutation rates ∈ [0.0, 1.0].

### 19. Domain 8: Agent Runtime

#### 19.1 Concepts

**AgentDescriptor** — Agent metadata.
```
AgentDescriptor { id, name, role: Role, capabilities: Vec<String>, max_concurrent_tasks }
```

**SwarmConfig** — Swarm parameters.
```
SwarmConfig { name, max_agents, task_timeout_ticks, require_review }
```

**Orchestrator** — Central coordinator.
- `add_agent(agent)` — Register an agent.
- `dispatch(task, payload)` → `TaskResult` — Assign work.
- `dispatch_with_review(task, payload)` → `TaskResult` — Assign + review gate.
- `agents_by_role(role)` → agents with that role.
- `agents_with_capability(cap)` → agents with that capability.
- `health_check()` → liveness report.

**TaskResult**: `Success(String)` | `Failure(String)` | `NeedsReview(String)`.

**Sandbox** — Per-agent isolation.
```
Sandbox { id, agent_id, capabilities: Vec<CapabilityToken>, limits: ResourceLimits, usage: ResourceUsage, active }
```

**CapabilityToken** — Fine-grained permission.
```
CapabilityToken { name, scope: CapScope, attenuated_from }
CapScope: Full | Restricted(BTreeSet<String>) | ReadOnly
```

**ResourceLimits** — Per-sandbox constraints.
```
ResourceLimits { max_memory_bytes, max_cpu_ms, max_syscalls, max_file_ops, max_network_ops }
```

**AuditLog** — Immutable event trail.
```
AuditEvent { timestamp, agent_id, kind: AuditEventKind, detail }
AuditEventKind: CapabilityGranted | CapabilityDenied | CapabilityAttenuated | ResourceLimitExceeded | SandboxCreated | SandboxDestroyed | OperationPerformed
```

#### 19.2 Invariants

- **INV-A1**: Every agent has exactly one `Role`.
- **INV-A2**: Capability tokens can only be attenuated (narrowed), never escalated.
- **INV-A3**: `ResourceUsage` cannot exceed `ResourceLimits`; violations trigger `ResourceLimitExceeded`.
- **INV-A4**: The `AuditLog` is append-only — entries are never modified or deleted.

### 20. Domain 9: Code Generation

#### 20.1 MLIR Emission

**Concept**: The `emit()` function transforms a type-checked, effect-annotated AST into textual MLIR in the MechGen dialect.

**MLIR Operations**:
- `MechGen.func` — Function definition with effect attributes.
- `MechGen.call` — Function invocation.
- `MechGen.tensor.*` — Tensor operations (matmul, broadcast, reshape).
- `MechGen.grad.*` — Autograd backward pass operations.
- `MechGen.effect.*` — Effect invocation/handling.
- `MechGen.agent.*` — Agent dispatch.

#### 20.2 Safety Elision

**Concept**: The `elide()` pass strips safety annotations from the AST for agent-mode output.

**Eliminated constructs**:
- Lifetime annotations (`'a`, `'static`).
- `unsafe` blocks and function modifiers.
- `&mut` → inferred mutability.
- `move` keyword on closures.
- `PhantomData<T>` → inner type.
- `Pin<T>` → inner type.
- `Send`, `Sync`, `Unpin` bounds.

#### 20.3 Formatting

Two output modes:
- `format_agent(module: &Module) -> String` — Compressed sigil syntax.
- `format_human(module: &Module) -> String` — Rust-like keyword syntax.

Both are bijective: `format_X(parse(format_X(ast))) ≡ format_X(ast)`.

#### 20.4 FFI Generation

**Targets**: `BindingTarget::C`, `BindingTarget::Python`, `BindingTarget::Wasm`.

**ForeignType** — FFI type mapping:
- `Void`, `Int(bits)`, `UInt(bits)`, `Float(bits)`, `Bool`, `CString`
- `Ptr(ForeignType)`, `Array(ForeignType, size)`, `Struct(name)`, `Opaque(name)`

Each `ForeignType` maps to three representations: `.to_c_type()`, `.to_mechgen_type()`, `.to_python_type()`.

#### 20.5 Invariants

- **INV-CG1**: MLIR output is syntactically valid MLIR.
- **INV-CG2**: Elision preserves semantic equivalence — elided programs have the same runtime behavior.
- **INV-CG3**: FFI wrappers always add null checks for pointer parameters.

### 21. Domain 10: Verification & Contracts

#### 21.1 Concepts

**VerificationResult** — Outcome of contract verification.
```
VerificationResult { fqn, status: VerifyStatus, checks: Vec<ContractCheck>, effect_checks: Vec<EffectCheck> }
VerifyStatus: Verified | Partial | Failed | Trivial
```

**ContractCheck** — Individual pre/postcondition result.
```
ContractCheck { condition, kind: ContractKind, result: CheckResult, explanation }
ContractKind: Requires | Ensures
CheckResult: Verified | Violated | Unknown
```

**EffectCheck** — Effect consistency result.
```
EffectCheck { effect, result: EffectCheckResult, detail }
EffectCheckResult: Consistent | Undeclared | Unused
```

**Certificate** — Machine-checkable verification proof.
```
Certificate { id: CertId, kind: CertKind, target, verifier, steps: Vec<ProofStep>, timestamp, valid }
CertKind: MemorySafety | DataRaceFreedom | ContractSatisfaction | EffectContainment
ProofStep: Axiom(String) | Derivation { rule, premises, conclusion } | Witness { source, claim }
```

#### 21.2 Synthesis Oracle

The synthesis oracle generates candidate implementations from specs:

```
SynthesisSpec { name, params, return_type, preconditions, postconditions, invariants, effects, perf_bounds }
Strategy: Imperative | Recursive | Functional | TableDriven | Speculative
Candidate { id, strategy, body, cost: CostEstimate, verification: VerificationResult }
```

The oracle ranks candidates by `CostEstimate.score()` after verifying each against the spec.

#### 21.3 Invariants

- **INV-V1**: `VerifyStatus::Verified` ⟹ all `ContractCheck`s are `Verified` and all `EffectCheck`s are `Consistent`.
- **INV-V2**: Certificates are revoked when the target function is modified.
- **INV-V3**: `ProofStep::Derivation` premises must be earlier steps in the same certificate.

### 22. Domain 11: Tooling & Infrastructure

#### 22.1 Cost Oracle

```
CostEstimate { construct, target, opt_level: OptLevel, cycles, memory_bytes, allocations, latency_ns, token_count, is_exact, confidence }
OptLevel: Debug | Release | ReleaseLto
```

The cost oracle provides per-construct cost queries. Agents query before emitting code to choose optimal implementations. The `CalibrationSuite` validates estimates against measured values.

#### 22.2 Token Budget

```
TokenReport { items: Vec<TokenMetrics>, total_agent, total_human, overall_ratio }
TokenMetrics { name, kind: ItemMetricKind, agent_tokens, human_tokens, ratio }
ItemMetricKind: Function | Struct | Enum | Trait | Impl | Module | Other
```

Agents use token reports to optimize context window utilization. The `overall_ratio` tracks agent-mode compression (typically ~3×).

#### 22.3 Performance Annotations

```
PerfAnnotation:
    ForceInline          // @pi!
    NoBlock              // @pnb
    Vectorize(u32)       // @pv(N)
    TargetHint(String)   // @pt(target)
    Alignment(u32)       // @pa(N)
    Pure                 // @pp
    ReprTargetOptimal    // #[repr(target_optimal)]
```

#### 22.4 Agentic Compiler Intelligence (ACI)

Four cooperative engines:

| Engine                   | Purpose                                      | Key Methods                                       |
| ------------------------ | -------------------------------------------- | ------------------------------------------------- |
| `DynamicWarningEngine`   | Context-aware warning suppression/escalation | `emit()`, `add_suppression()`, `add_escalation()` |
| `IntelligentDebugEngine` | Pattern-based root-cause analysis            | `diagnose()`, `add_pattern()`                     |
| `PerformanceAdvisor`     | Hotspot detection                            | `analyze()`, `all_hotspots()`                     |
| `SwarmCoordIntelligence` | Agent load balancing                         | `route_task()`, `load_balance_order()`            |

#### 22.5 Grammar Extensions

```
GrammarExtension { sigil, rust_equiv, namespace, usage_count, description }
```

Namespace-scoped sigil registration with frequency-based promotion (threshold: 100 uses) to built-in status.

#### 22.6 Benchmarking

```
MetricSeries { name, unit, samples } → MetricSummary { count, mean, min, max, p50, p99 }
```

Tracks: token throughput, parse error rate, synthesis success rate, swarm latency.

#### 22.7 Invariants

- **INV-TL1**: `CostEstimate.confidence` ∈ [0.0, 1.0].
- **INV-TL2**: Token ratios satisfy `agent_tokens ≤ human_tokens` (agent mode is always more compact).
- **INV-TL3**: Grammar extension promotion requires `usage_count ≥ PROMOTION_THRESHOLD` (100).

### 23. Domain 12: Package & Version Control

#### 23.1 Forge Package Registry

```
ForgePackage { name, version, capabilities, effects, contracts, dependencies }
ForgeRegistry — Central registry with capability-indexed search, semantic search (trigram similarity), and contract-based compatibility checking.
```

**Operations**:
- `publish(package)` — Publish a package.
- `search_by_capability(cap)` — Capability-indexed lookup.
- `semantic_search(query)` — Fuzzy search via trigram similarity.
- `check_compatibility(a, b)` — Contract-based composition validation.
- `dependency_graph(name)` — Transitive dependency analysis.

#### 23.2 Capability Manifests

```
CrateManifest { name, version, agents, functions, types, effects, specs, capability_index }
```

Generated by `manifest::generate()` for every crate. Enables capability-indexed search across the Forge ecosystem.

#### 23.3 Semantic Version Control

```
SemanticOp (18 variants):
    AddFunction | RemoveFunction | RenameFunction | ModifyBody | ModifySignature
    AddField | RemoveField | RenameField
    AddContract | RemoveContract
    AddImport | RemoveImport
    AddStruct | RemoveStruct
    ChangeVisibility
    AddEffect | RemoveEffect

Commit { id, parent, author, message, ops: Vec<SemanticOp>, timestamp }
OpLog — Operation log with branching, merging, rebasing
```

Unlike text-based VCS, semantic VCS operates on structured operations, enabling merge conflict detection at the semantic level.

#### 23.4 Invariants

- **INV-P1**: Forge packages are immutable once published (append-only versioning).
- **INV-P2**: OpLog commits form a DAG — `is_ancestor()` is well-defined.
- **INV-P3**: Three-way merge detects conflicts at the operation level, not text level.

---

## Part III: Cross-Domain Relations

### 24. Pipeline Composition

The 13-phase compiler pipeline threads data through domains:

```
Source Text
    │
    ▼  [Lexical Domain]
  Token Stream ──────────────────────────────── Mode: Human | Agent
    │
    ▼  [Syntactic Domain]
  AST (Module) ──────────────────────────────── 18 ItemKind variants
    │
    ├──▶ [Semantic Domain]
    │    Resolved AST ───────────────────────── SymbolId on every name
    │
    ├──▶ [Type Domain]
    │    Typed AST ──────────────────────────── Ty on every expression
    │
    ├──▶ [AI: Shape Inference]
    │    Shape-checked AST ──────────────────── TensorDim validated
    │
    ├──▶ [Effect Domain]
    │    Effect-annotated AST ───────────────── EffectSet per function
    │
    ├──▶ [AI: Autograd]
    │    Grad-annotated AST ─────────────────── Tape entries for Param types
    │
    ├──▶ [AI: Logic Materialization]
    │    KB-materialized AST ────────────────── Fixpoint facts inlined
    │
    ├──▶ [AI: Evolution Codegen]
    │    Evolve-lowered AST ─────────────────── Genetic loops expanded
    │
    ├──▶ [Safety Domain]
    │    SKB-verified AST ───────────────────── 255 rules checked
    │
    ├──▶ [Codegen Domain: MLIR]
    │    MLIR text ──────────────────────────── MechGen dialect
    │
    ├──▶ [Codegen Domain: Elision] (agent mode only)
    │    Elided AST ─────────────────────────── Safety annotations stripped
    │
    └──▶ [Codegen Domain: Format]
         Source text ────────────────────────── Human or Agent syntax
```

### 25. Cross-Domain Invariants

- **INV-X1**: The pipeline is phase-ordered — each phase adds annotations without modifying prior annotations.
- **INV-X2**: Every `Diagnostic` carries both a `Span` (lexical) and a `DiagnosticCategory` (semantic).
- **INV-X3**: `EffectSet` is preserved through codegen — MLIR function attributes mirror effect annotations.
- **INV-X4**: Agent mode and Human mode produce identical ASTs — mode affects only lexing and formatting.
- **INV-X5**: All 42 source modules are reachable from `main.rs`.
- **INV-X6**: Hot reload patches are validated against the type, effect, and contract domains before application.
- **INV-X7**: Semantic VCS operations correspond 1:1 to AST structural changes.

### 26. Concept Index

Complete alphabetical index of all ontological concepts:

| Concept                 | Domain       | Rust Type                | Section |
| ----------------------- | ------------ | ------------------------ | ------- |
| AgentDescriptor         | Agent        | `AgentDescriptor`        | §19     |
| AgentDef                | Syntactic    | `AgentDef`               | §13.2   |
| AgentId                 | Agent        | `String`                 | §19     |
| AgentLoad               | Tooling      | `AgentLoad`              | §22.4   |
| Applicability           | Verification | enum (4)                 | §8      |
| Atom (logical)          | AI/Logic     | `Atom`                   | §18.3   |
| Attribute               | Syntactic    | `Attribute`              | §13.1   |
| AuditEvent              | Agent        | `AuditEvent`             | §19.1   |
| AuditEventKind          | Agent        | enum (7)                 | §19.1   |
| AuditLog                | Agent        | `AuditLog`               | §19.1   |
| BindingTarget           | Codegen      | enum (3)                 | §20.4   |
| Block                   | Syntactic    | `Block`                  | §13.1   |
| BusStats                | Agent        | `BusStats`               | §3      |
| Candidate               | Verification | `Candidate`              | §21.2   |
| CapScope                | Agent        | enum (3)                 | §19.1   |
| CapabilityToken         | Agent        | `CapabilityToken`        | §19.1   |
| CertId                  | Verification | `u64`                    | §21.1   |
| CertKind                | Verification | enum (4)                 | §21.1   |
| Certificate             | Verification | `Certificate`            | §21.1   |
| CertificateStore        | Verification | `CertificateStore`       | §21.1   |
| CheckResult             | Verification | enum (3)                 | §21.1   |
| Commit (VCS)            | Package      | `Commit`                 | §23.3   |
| CommitId                | Package      | `u64`                    | §23.3   |
| ConsensusEngine         | Agent        | `ConsensusEngine`        | §6.3    |
| ConsensusError          | Agent        | enum (5)                 | §6.3    |
| ConstDef                | Syntactic    | `ConstDef`               | §13.1   |
| ContractCheck           | Verification | `ContractCheck`          | §21.1   |
| ContractClause          | Syntactic    | `ContractClause`         | §13.3   |
| ContractClauseKind      | Syntactic    | enum (3)                 | §13.3   |
| ContractKind            | Verification | enum (2)                 | §21.1   |
| CostComparison          | Tooling      | `CostComparison`         | §22.1   |
| CostEstimate            | Tooling      | `CostEstimate`           | §22.1   |
| CrateManifest           | Package      | `CrateManifest`          | §23.2   |
| CrdtOp                  | Agent        | enum (6)                 | §6.2    |
| CrdtState               | Agent        | `CrdtState`              | §6.2    |
| DebugDiagnosis          | Tooling      | `DebugDiagnosis`         | §22.4   |
| Decision                | Agent        | enum (3)                 | §6.3    |
| DecompError             | Agent        | enum (4)                 | §7      |
| Diagnostic              | Cross-domain | `Diagnostic`             | §8      |
| DiagnosticCategory      | Cross-domain | enum (10)                | §8      |
| DiagnosticGraph         | Cross-domain | `DiagnosticGraph`        | §8      |
| DiagnosticNode          | Cross-domain | `DiagnosticNode`         | §8      |
| DiagnosticNodeKind      | Cross-domain | enum (3)                 | §8      |
| DiffOp                  | AI/Autograd  | enum (26)                | §18.2   |
| DynamicWarningEngine    | Tooling      | `DynamicWarningEngine`   | §22.4   |
| Effect                  | Effect       | enum (16)                | §16     |
| EffectAnalysis          | Verification | `EffectAnalysis`         | §21.1   |
| EffectCheck             | Verification | `EffectCheck`            | §21.1   |
| EffectCheckResult       | Verification | enum (3)                 | §21.1   |
| EffectDef               | Syntactic    | `EffectDef`              | §13.3   |
| EffectInfer             | Effect       | `EffectInfer`            | §16     |
| EffectOp                | Syntactic    | `EffectOp`               | §13.3   |
| EffectSet               | Effect       | `BTreeSet<Effect>`       | §16     |
| EnumDef                 | Syntactic    | `EnumDef`                | §13.1   |
| EnumVariant             | Syntactic    | `EnumVariant`            | §13.1   |
| Envelope                | Agent        | `Envelope`               | §3      |
| EvolveDef               | Syntactic    | `EvolveDef`              | §13.2   |
| EvolutionConfig         | AI/Evolution | `EvolutionConfig`        | §18.4   |
| Expr (ExprKind)         | Syntactic    | enum (31)                | §13.1   |
| FactDef                 | Syntactic    | `FactDef`                | §13.2   |
| FieldInit               | Syntactic    | `FieldInit`              | §13.1   |
| FieldPattern            | Syntactic    | `FieldPattern`           | §13.1   |
| Fix                     | Cross-domain | `Fix`                    | §8      |
| FixCandidate            | Tooling      | `FixCandidate`           | §22     |
| FloatTy                 | Type         | enum (2)                 | §15     |
| ForeignFunction         | Codegen      | `ForeignFunction`        | §20.4   |
| ForeignType             | Codegen      | enum (10)                | §20.4   |
| ForgePackage            | Package      | `ForgePackage`           | §23.1   |
| ForgeRegistry           | Package      | `ForgeRegistry`          | §23.1   |
| FunctionDef             | Syntactic    | `FunctionDef`            | §13.3   |
| GenericParam            | Syntactic    | `GenericParam`           | §13.1   |
| GrammarExtension        | Tooling      | `GrammarExtension`       | §22.5   |
| HealedDiagnostic        | Tooling      | `HealedDiagnostic`       | §22     |
| HotReloadEngine         | Tooling      | `HotReloadEngine`        | §10     |
| ImpactReport            | Agent        | `ImpactReport`           | §6.3    |
| ImplBlock               | Syntactic    | `ImplBlock`              | §13.1   |
| Individual              | AI/Evolution | `Individual`             | §18.4   |
| IntelligentDebugEngine  | Tooling      | `IntelligentDebugEngine` | §22.4   |
| IntTy                   | Type         | enum (6)                 | §15     |
| Item                    | Syntactic    | `Item`                   | §13.1   |
| ItemKind                | Syntactic    | enum (18)                | §13.1   |
| ItemMetricKind          | Tooling      | enum (7)                 | §22.2   |
| KbDef                   | Syntactic    | `KbDef`                  | §13.2   |
| KnowledgeBase (runtime) | AI/Logic     | `KnowledgeBase`          | §18.3   |
| LamportClock            | Agent        | `LamportClock`           | §6.2    |
| LayerDef                | Syntactic    | `LayerDef`               | §13.2   |
| Lease                   | Agent        | `Lease`                  | §6.1    |
| LeaseError              | Agent        | enum (4)                 | §6.1    |
| LeaseManager            | Agent        | `LeaseManager`           | §6.1    |
| LeaseMode               | Agent        | enum (3)                 | §6.1    |
| LiteralKind             | Syntactic    | enum (7)                 | §13.1   |
| MatchArm                | Syntactic    | `MatchArm`               | §13.1   |
| MergeLog                | Agent        | `MergeLog`               | §6.2    |
| MergeOutcome            | Agent        | enum (3)                 | §6.2    |
| MessageBus              | Agent        | `MessageBus`             | §3      |
| MetricSeries            | Tooling      | `MetricSeries`           | §22.6   |
| MetricSummary           | Tooling      | `MetricSummary`          | §22.6   |
| Mode                    | Lexical      | enum (2)                 | §12     |
| Module                  | Syntactic    | `Module`                 | §13.1   |
| ModuleDef               | Syntactic    | `ModuleDef`              | §13.1   |
| NetDef                  | Syntactic    | `NetDef`                 | §13.2   |
| OpLog                   | Package      | `OpLog`                  | §23.3   |
| OptLevel                | Tooling      | enum (3)                 | §22.1   |
| Orchestrator            | Agent        | `Orchestrator`           | §19.1   |
| Param                   | Syntactic    | `Param`                  | §13.1   |
| ParseError              | Tooling      | enum (3)                 | §22.3   |
| PatchStatus             | Tooling      | enum (4)                 | §10     |
| PatchUnit               | Tooling      | `PatchUnit`              | §10     |
| Pattern                 | Syntactic    | enum (9)                 | §13.1   |
| Payload                 | Agent        | enum (4)                 | §3      |
| PerfAnnotation          | Tooling      | enum (7)                 | §22.3   |
| PerfAnnotationSet       | Tooling      | `PerfAnnotationSet`      | §22.3   |
| PerfHotspot             | Tooling      | `PerfHotspot`            | §22.4   |
| PerfRegistry            | Tooling      | `PerfRegistry`           | §22.3   |
| PerformanceAdvisor      | Tooling      | `PerformanceAdvisor`     | §22.4   |
| Phase                   | Agent        | enum (5)                 | §6.3    |
| Proposal                | Agent        | `Proposal`               | §6.3    |
| ProofStep               | Verification | enum (3)                 | §21.1   |
| Recipient               | Agent        | enum (3)                 | §3      |
| Resolver                | Semantic     | `Resolver`               | §14     |
| ResourceLimits          | Agent        | `ResourceLimits`         | §19.1   |
| ResourceUsage           | Agent        | `ResourceUsage`          | §19.1   |
| Role                    | Agent        | enum (8)                 | §2      |
| RollbackEntry           | Tooling      | `RollbackEntry`          | §10     |
| Rule (logical)          | AI/Logic     | `Rule`                   | §18.3   |
| Rule (safety)           | Safety       | `Rule`                   | §17     |
| RuleDatabase            | Safety       | enum (8)                 | §17.1   |
| RuleDef                 | Syntactic    | `RuleDef`                | §13.2   |
| RuleSeverity            | Safety       | enum (4)                 | §17.1   |
| Sandbox                 | Agent        | `Sandbox`                | §19.1   |
| Scope                   | Semantic     | `Scope`                  | §14     |
| SemanticOp              | Package      | enum (18)                | §23.3   |
| SemanticRegion          | Agent        | `SemanticRegion`         | §6.1    |
| Severity (diagnostic)   | Cross-domain | enum (3)                 | §8      |
| Severity (ACI)          | Tooling      | enum (5)                 | §22.4   |
| Shape                   | AI/Shape     | `Vec<ShapeDim>`          | §18.1   |
| ShapeDim                | AI/Shape     | enum (2)                 | §18.1   |
| Span (lexer)            | Lexical      | `Span`                   | §12     |
| Span (HIR)              | Type         | `Span`                   | §15     |
| SpecDef                 | Syntactic    | `SpecDef`                | §13.3   |
| SpecItem                | Syntactic    | enum (5)                 | §13.3   |
| StampedOp               | Agent        | `StampedOp`              | §6.2    |
| StaticDef               | Syntactic    | `StaticDef`              | §13.1   |
| Stmt                    | Syntactic    | enum (3)                 | §13.1   |
| Strategy                | Verification | enum (5)                 | §21.2   |
| StructDef               | Syntactic    | `StructDef`              | §13.1   |
| StructField             | Syntactic    | `StructField`            | §13.1   |
| Subst                   | Type         | `Subst`                  | §15     |
| SwarmAgent (trait)      | Agent        | trait                    | §19.1   |
| SwarmBus                | Agent        | `MessageBus`             | §3      |
| SwarmConfig             | Agent        | `SwarmConfig`            | §19.1   |
| SwarmCoordIntelligence  | Tooling      | `SwarmCoordIntelligence` | §22.4   |
| SwarmDef                | Syntactic    | `SwarmDef`               | §13.2   |
| Symbol                  | Semantic     | `Symbol`                 | §14     |
| SymbolId                | Semantic     | `SymbolId(u32)`          | §14     |
| SymbolKind              | Semantic     | enum (19)                | §14     |
| SymbolTable             | Semantic     | `SymbolTable`            | §14     |
| SynthesisOracle         | Verification | `SynthesisOracle`        | §21.2   |
| SynthesisSpec           | Verification | `SynthesisSpec`          | §21.2   |
| Tape                    | AI/Autograd  | `Tape`                   | §18.2   |
| TapeEntry               | AI/Autograd  | `TapeEntry`              | §18.2   |
| Task                    | Agent        | `Task`                   | §7      |
| TaskDag                 | Agent        | `TaskDag`                | §7      |
| TaskId                  | Agent        | `u64`                    | §7      |
| TaskResult              | Agent        | enum (3)                 | §19     |
| TaskState               | Agent        | enum (5)                 | §7      |
| TensorDim               | Syntactic    | enum (2)                 | §13.4   |
| TensorDimHir            | Type         | enum (2)                 | §15     |
| Term                    | AI/Logic     | enum (3)                 | §18.3   |
| TextEdit                | Tooling      | `TextEdit`               | §22     |
| ThreeWayMerge           | Package      | `ThreeWayMerge`          | §23.3   |
| Token                   | Lexical      | `Token`                  | §12.1   |
| TokenKind               | Lexical      | enum (168)               | §12.1   |
| TokenMetrics            | Tooling      | `TokenMetrics`           | §22.2   |
| TokenReport             | Tooling      | `TokenReport`            | §22.2   |
| Topic                   | Agent        | enum (10)                | §3.1    |
| TrainDef                | Syntactic    | `TrainDef`               | §13.2   |
| TraitDef                | Syntactic    | `TraitDef`               | §13.1   |
| Ty (HIR)                | Type         | enum (31)                | §15     |
| Type (AST)              | Syntactic    | enum (32)                | §13.1   |
| TypeAlias               | Syntactic    | `TypeAlias`              | §13.1   |
| TypeChecker             | Type         | `TypeChecker`            | §15     |
| TypeEnv                 | Type         | `TypeEnv`                | §15     |
| TyVar                   | Type         | `TyVar(u32)`             | §15     |
| TyVarSupply             | Type         | `TyVarSupply`            | §15     |
| UintTy                  | Type         | enum (6)                 | §15     |
| UseDef                  | Syntactic    | `UseDef`                 | §13.1   |
| ValidationResult        | Tooling      | enum (5)                 | §10     |
| VariantKind             | Syntactic    | enum (3)                 | §13.1   |
| VerificationResult      | Verification | `VerificationResult`     | §21.1   |
| VerifyStatus            | Verification | enum (4)                 | §21.1   |
| Visibility              | Syntactic    | enum (2)                 | §13.1   |
| Vote                    | Agent        | enum (3)                 | §6.3    |
| Warning                 | Tooling      | `Warning`                | §22.4   |
| WherePredicate          | Syntactic    | `WherePredicate`         | §13.1   |

**Total concepts: 184**

### 27. Module Map

All 42 source modules and their domain membership:

| Module                | Lines | Domain(s)         | Pipeline Phase           |
| --------------------- | ----- | ----------------- | ------------------------ |
| `lexer.rs`            | ~1200 | Lexical           | 1. Tokenization          |
| `parser.rs`           | ~2200 | Syntactic         | 2. Parsing               |
| `ast.rs`              | ~800  | Syntactic         | 2. (data types)          |
| `resolve.rs`          | ~400  | Semantic          | 3. Name Resolution       |
| `types.rs`            | ~700  | Type              | 4. Type Checking         |
| `hir.rs`              | ~500  | Type              | 4. (data types)          |
| `shape.rs`            | ~250  | AI/Shape          | 5. Shape Inference       |
| `effects.rs`          | ~350  | Effect            | 6. Effect Inference      |
| `autograd.rs`         | ~450  | AI/Autograd       | 7. Autodiff              |
| `logic.rs`            | ~400  | AI/Logic          | 8. Logic Materialization |
| `evolve_gen.rs`       | ~400  | AI/Evolution      | 9. Evolution Codegen     |
| `skb.rs`              | ~600  | Safety            | 10. Safety Verification  |
| `mlir.rs`             | ~900  | Codegen           | 11. MLIR Lowering        |
| `elision.rs`          | ~500  | Codegen           | 12. Safety Elision       |
| `fmt.rs`              | ~855  | Codegen           | 13. Formatting           |
| `verify.rs`           | ~400  | Verification      | Cross-phase              |
| `certs.rs`            | ~250  | Verification      | Cross-phase              |
| `synthesis.rs`        | ~300  | Verification      | Cross-phase              |
| `heal.rs`             | ~400  | Tooling           | Cross-phase              |
| `cost.rs`             | ~250  | Tooling           | Cross-phase              |
| `cost_calibration.rs` | ~250  | Tooling           | Cross-phase              |
| `token_budget.rs`     | ~650  | Tooling           | Cross-phase              |
| `perf_annot.rs`       | ~350  | Tooling           | Cross-phase              |
| `aci.rs`              | ~350  | Tooling           | Cross-phase              |
| `bench.rs`            | ~200  | Tooling           | Cross-phase              |
| `ffi_gen.rs`          | ~300  | Codegen           | Cross-phase              |
| `legacy.rs`           | ~600  | Codegen           | Pre-lex                  |
| `grammar.rs`          | ~250  | Lexical           | Cross-phase              |
| `rap.rs`              | ~200  | Agent (transport) | External API             |
| `swarm_bus.rs`        | ~350  | Agent             | Runtime                  |
| `swarm_sdk.rs`        | ~300  | Agent             | Runtime                  |
| `sandbox.rs`          | ~400  | Agent             | Runtime                  |
| `lease.rs`            | ~350  | Agent             | Runtime                  |
| `crdt.rs`             | ~400  | Agent             | Runtime                  |
| `consensus.rs`        | ~350  | Agent             | Runtime                  |
| `decompose.rs`        | ~300  | Agent             | Runtime                  |
| `hot_reload.rs`       | ~350  | Tooling           | Runtime                  |
| `semantic_vcs.rs`     | ~350  | Package           | Runtime                  |
| `forge.rs`            | ~675  | Package           | Runtime                  |
| `manifest.rs`         | ~200  | Package           | Build                    |
| `stdlib_ext.rs`       | ~150  | Package           | Build                    |
| `main.rs`             | ~200  | —                 | Entry point              |

### 28. Relation Summary

Complete list of inter-domain relations:

| Relation         | From → To                     | Semantics                             |
| ---------------- | ----------------------------- | ------------------------------------- |
| `tokenizes`      | Source → Token Stream         | Lexer maps text to tokens             |
| `parses_to`      | Token Stream → AST            | Parser builds syntax tree             |
| `resolves_to`    | Name → SymbolId               | Resolver binds names to symbols       |
| `has_type`       | SymbolId → Ty                 | Type checker assigns types            |
| `has_shape`      | Tensor Expr → Shape           | Shape inference validates dims        |
| `has_effects`    | Function → EffectSet          | Effect inference computes effects     |
| `generates_grad` | Param Expr → Tape             | Autograd builds computation graph     |
| `materializes`   | KbDef → Facts                 | Logic engine computes fixpoint        |
| `lowers_evolve`  | EvolveDef → Loops             | Evolution codegen expands blocks      |
| `verified_by`    | Function → Certificate        | Verifier issues proofs                |
| `emits_mlir`     | Typed AST → MLIR Text         | MLIR lowering produces IR             |
| `elides`         | AST → Elided AST              | Safety elision strips annotations     |
| `formats_to`     | AST → Source Text             | Formatter renders syntax              |
| `assigns_to`     | Task → Agent                  | Orchestrator dispatches work          |
| `leases`         | Agent → SemanticRegion        | Lease manager grants access           |
| `merges_via`     | CrdtOp → CrdtState            | CRDT engine resolves concurrent edits |
| `commits_to`     | SemanticOp → OpLog            | VCS records semantic changes          |
| `publishes_to`   | CrateManifest → ForgeRegistry | Package manager publishes             |

---

## Part IV: Protocol Quick Reference

### 29. Agent Decision Flowchart

```
Agent receives task
    │
    ▼
Query cost/compare → Choose cheapest approach
    │
    ▼
Acquire SemanticRegion lease (ExclusiveWrite)
    │
    ▼
Write code (MechGen source)
    │
    ▼
Call RAP build/check → Get diagnostics
    │
    ├─ Errors? → Call build/heal → Apply best FixCandidate → Retry
    │
    ▼
Call verify/contracts → Check contracts
    │
    ├─ Violations? → Adjust implementation → Retry
    │
    ▼
Call effects/check → Verify effect containment
    │
    ├─ Undeclared? → Add effect annotation → Retry
    │
    ▼
Emit CrdtOp (ModifyBody / InsertItem)
    │
    ▼
Commit SemanticOp to OpLog
    │
    ▼
Release lease
    │
    ▼
Report TaskResult::Success
```

### 30. Counts Summary

| Category                 | Count |
| ------------------------ | ----- |
| Source modules           | 42    |
| TokenKind variants       | 168   |
| AST ItemKind variants    | 18    |
| AST ExprKind variants    | 31    |
| AST Type variants        | 32    |
| HIR Ty variants          | 31    |
| Effect kinds             | 16    |
| SKB rule databases       | 8     |
| SKB rules total          | 255   |
| RAP endpoints            | 24    |
| Agent roles              | 8     |
| Message topics           | 10    |
| CRDT operations          | 6     |
| Consensus phases         | 5     |
| Semantic VCS operations  | 18    |
| DiffOp (autograd)        | 26    |
| Selection methods        | 4     |
| Crossover methods        | 3     |
| Mutation methods         | 3     |
| Performance annotations  | 7     |
| ACI engines              | 4     |
| Ontological concepts     | 184   |
| Cross-domain relations   | 18    |
| System invariants        | 34    |
| Compiler pipeline phases | 13    |
