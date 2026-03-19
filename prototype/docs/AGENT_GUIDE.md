# Redox Agent Guide

> How to build, deploy, and manage agents in the Redox ecosystem.

---

## 1. What is an Agentic Compiler?

In Redox, the compiler is designed to be operated by AI agents — not just human
developers. Every language construct is:

- **Queryable** — agents can inspect types, contracts, effects, and costs.
- **Costable** — the cost oracle returns exact performance data before codegen.
- **Contract-verified** — preconditions, postconditions, and invariants are
  checked at compile-time (and optionally at runtime).
- **Effect-tracked** — the compiler knows which functions have side effects.

---

## 2. Agent Architecture

### 2.1 Swarm Model

Redox supports multi-agent compilation through a swarm model:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Agent A    │     │   Agent B    │     │   Agent C    │
│  (Lexer/     │────▶│  (Type       │────▶│  (Codegen    │
│   Parser)    │     │   Checker)   │     │   Agent)     │
└──────────────┘     └──────────────┘     └──────────────┘
        │                    │                    │
        └────────────────────┴────────────────────┘
                         │
                   ┌─────▼─────┐
                   │ Swarm Bus │
                   └───────────┘
```

Agents communicate via the **Swarm Bus** — a typed publish/subscribe system.
Each agent has:

- A unique identifier
- A set of capabilities (what it can do)
- Resource limits (memory, CPU, syscalls)
- An isolated sandbox

### 2.2 Agent Lifecycle

1. **Create** — `SandboxManager.create(agent_id, limits)`
2. **Grant capabilities** — `sandbox.grant(CapabilityToken.restricted("fs.read"))`
3. **Execute** — agent runs within its sandbox
4. **Communicate** — via Swarm Bus publish/subscribe
5. **Destroy** — `SandboxManager.destroy(agent_id)`

---

## 3. Swarm Communication

### 3.1 Swarm Bus

The Swarm Bus is the primary communication channel between agents:

```
let bus = SwarmBus.new();

// Publish a message
bus.publish("channel.name", payload);

// Subscribe to messages
bus.subscribe("channel.name", |msg| {
    process(msg.payload);
});
```

### 3.2 Event Channels

Standard channels in the Redox compiler:

| Channel             | Purpose                            |
|---------------------|------------------------------------|
| `parse.complete`    | Parser finished a compilation unit |
| `type.error`        | Type checking found an error       |
| `contract.violated` | A contract check failed            |
| `codegen.ready`     | IR is ready for code generation    |
| `build.complete`    | Build finished                     |
| `cost.query`        | Agent requesting cost information  |
| `heal.suggestion`   | Auto-healing suggestion ready      |

### 3.3 Consensus

For decisions requiring agreement across agents (e.g., optimisation strategy),
Redox provides a consensus module:

```
let consensus = ConsensusManager.new();
consensus.propose("opt_strategy", "aggressive_inline");
// Other agents vote
let decision = consensus.resolve("opt_strategy");
```

---

## 4. Capability System

### 4.1 Capability Tokens

Every agent operates within a capability sandbox:

```
// Full access
CapabilityToken.full("fs")

// Restricted — specific operations only
CapabilityToken.restricted("fs.read")

// Read-only
CapabilityToken.read_only("config")
```

### 4.2 Attenuation

Capabilities can be narrowed (attenuated) but never widened:

```
let full = CapabilityToken.full("fs");
let restricted = full.attenuate("fs.read");     // OK
let widened = restricted.attenuate("fs");        // ERROR: cannot widen
```

### 4.3 Resource Limits

```
ResourceLimits {
    max_memory: 10 * 1024 * 1024,  // 10 MB
    max_cpu_ms: 30_000,              // 30 seconds
    max_syscalls: 1000,
    max_file_ops: 50,
    max_network_ops: 10,
}
```

---

## 5. Cost-Aware Agent Decisions

Agents query the cost oracle before generating code:

```
// Should we use Vec or a stack array?
let vec_cost = cost.query("Vec::push", target, Release);
let arr_cost = cost.query("stack array", target, Release);

?: arr_cost.cycles < vec_cost.cycles && size_known {
    emit_stack_array()
} _ {
    emit_vec()
}
```

### 5.1 Token Budget

Agents operate under a token budget — the maximum tokens they can process
in a single pass:

```
[agent]
token_budget = 8192
```

The token budget module tracks consumption and supports elision — automatically
summarising large code blocks to fit within the budget.

---

## 6. Contract-Driven Development

### 6.1 Writing Contracts

```
f transfer(from: &mut Account, to: &mut Account, amount: u64)
    @req from.balance >= amount "insufficient funds"
    @req amount > 0 "amount must be positive"
    @ens from.balance == old(from.balance) - amount
    @ens to.balance == old(to.balance) + amount
    @fx pure
{
    from.balance -= amount;
    to.balance += amount;
}
```

### 6.2 Agent Contract Verification

Agents verify contracts at multiple stages:

1. **Parse-time** — syntactic well-formedness
2. **Type-check-time** — type compatibility
3. **Synthesis-time** — contract-directed code generation
4. **Runtime** — dynamic checks (debug builds)

### 6.3 Spec-Driven Synthesis

The synthesis oracle generates implementations from specifications:

```
// Specification
f sort(arr: &mut [i32])
    @req arr.len() > 0
    @ens arr.windows(2).all(|w| w[0] <= w[1])
    @ens arr.len() == old(arr.len())
    @fx pure;

// Agent synthesises an implementation that satisfies the contracts
```

---

## 7. Hot Reload

During agent development, code can be patched at function granularity
without restarting:

```
let engine = HotReloadEngine.new();
let patch = PatchUnit {
    function_name: "process_data".into(),
    new_body: updated_ir,
    version: 2,
};

// Validate and apply
engine.validate(&patch, &registry)?;
engine.apply(&mut registry, patch)?;
```

---

## 8. Benchmarking Agent Performance

Use the benchmarking suite to measure agent performance:

```
let mut runner = BenchmarkRunner.new();

runner.register("throughput", || {
    let mut tt = TokenThroughput.new();
    // ... run workload ...
    tt.record(tokens_processed, elapsed_ms);
    BenchmarkResult { ... }
});

runner.run_all();
p"{runner.report()}";
```

Key metrics:

| Metric                | Unit       | Description                    |
|-----------------------|------------|--------------------------------|
| Token throughput      | tokens/ms  | Tokens processed per millisec  |
| Parse error rate      | errors/unit| Parse errors per source unit   |
| Synthesis success     | ratio      | Successful syntheses / total   |
| Swarm dispatch latency| ms         | Time to dispatch a task        |
| Swarm completion      | ms         | Time to complete a task        |

---

## 9. Best Practices

1. **Use contracts everywhere** — they are documentation, tests, and
   verification in one.
2. **Track effects** — mark pure functions as `@fx pure` so agents can
   reason about them.
3. **Query costs before codegen** — don't guess performance.
4. **Use the smallest capability** — prefer `restricted` over `full`.
5. **Set resource limits** — prevent runaway agents.
6. **Monitor via audit log** — every sandbox action is logged.
7. **Use semantic versioning in patches** — hot-reload tracks versions.
8. **Benchmark regularly** — calibrate the cost model against real hardware.
