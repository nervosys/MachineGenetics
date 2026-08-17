# RAP Agentic Methods

The MAGE Agent Protocol (RAP) exposes agentic-first services over JSON-RPC.
Beyond the base `language/*` and `build/check` methods, RAP provides:

## Method Reference

| Method             | Purpose                               | Key Params                                                       |
| ------------------ | ------------------------------------- | ---------------------------------------------------------------- |
| `build/heal`       | Parse + generate fix candidates (P22) | `source`                                                         |
| `cost/query`       | Per-construct cost estimate (P19)     | `construct`, `target`, `opt`                                     |
| `cost/compare`     | Compare two constructs                | `a`, `b`, `target`                                               |
| `skb/query`        | Query structured knowledge base (P14) | `by` (fqn/effect/capability/tag/rust_alias/module), `value`      |
| `skb/spec`         | Lookup spec block                     | `fqn`                                                            |
| `verify/contracts` | Verify function contracts (P21)       | `fqn`, `requires`, `ensures`, `declared_effects`, `used_effects` |

## `build/heal`

Self-healing compilation: parse source, collect diagnostics, and generate
ranked fix candidates with confidence scores and token cost.

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"build/heal","params":{"source":"pub fn add(a: i32, b: i32) { a + b }"}}

// Response
{"jsonrpc":"2.0","id":1,"result":{
  "ok": false,
  "diagnostics": [
    {
      "diagnostic": {"severity":"Error","message":"missing return type on `add`","span":{"line":1,"col":1}},
      "fixes": [
        {"id":"missing-return-type-0","description":"add return type annotation `-> _`","confidence":0.85,"token_cost":3}
      ]
    }
  ]
}}
```

**Agent workflow**: Call `build/heal` instead of `build/check`. If fixes are
returned, apply the highest-confidence fix and re-check. Iterate until clean.

## `cost/query`

Query the cost of a language construct on a specific target architecture.

```json
{"jsonrpc":"2.0","id":2,"method":"cost/query","params":{
  "construct": "Vec::push",
  "target": "x86_64",
  "opt": "release"
}}
```

Returns: `cycles`, `memory_bytes`, `allocations`, `latency_ns`, `token_count`,
`confidence`.

**Agent workflow**: Before emitting code, query costs of alternative constructs
and choose the cheapest option that meets requirements.

## `cost/compare`

Compare two constructs side-by-side with a recommendation.

```json
{"jsonrpc":"2.0","id":3,"method":"cost/compare","params":{
  "a": "Box alloc",
  "b": "Rc clone",
  "target": "x86_64"
}}
```

## `skb/query`

Query the structured knowledge base. The `by` parameter selects the index:

| `by` value   | Searches                            | Example `value`      |
| ------------ | ----------------------------------- | -------------------- |
| `fqn`        | Fully qualified name (exact/prefix) | `std::io::read_file` |
| `effect`     | Symbols declaring a given effect    | `io`                 |
| `capability` | Symbols requiring a capability      | `network`            |
| `tag`        | Semantic tags                       | `agent`              |
| `rust_alias` | Rust equivalent symbol name         | `HashMap`            |
| `module`     | All symbols in a module (prefix)    | `std::io`            |

```json
{"jsonrpc":"2.0","id":4,"method":"skb/query","params":{"by":"rust_alias","value":"HashMap"}}
```

**Agent workflow**: When translating Rust code, use `skb/query` with
`by=rust_alias` to find the MAGE equivalent. When checking if a function
needs a capability, query `by=fqn`.

## `skb/spec`

Lookup the spec block (preconditions, postconditions) for a symbol.

```json
{"jsonrpc":"2.0","id":5,"method":"skb/spec","params":{"fqn":"std::io::read_file"}}
```

## `verify/contracts`

Verify that a function's implementation satisfies its contracts.

```json
{"jsonrpc":"2.0","id":6,"method":"verify/contracts","params":{
  "fqn": "my_module.process",
  "requires": ["input.len() > 0"],
  "ensures": ["ret.is_ok()"],
  "declared_effects": ["io"],
  "used_effects": ["io"]
}}
```

Returns `status` (Verified/Partial/Failed/Trivial), individual `checks`, and
`effect_checks` with consistency results.

## Agent Memory (stdlib)

RAP's memory service exposes four tiers. There is no `std::agent::Memory`
type — there is no module system, and the tiers are reached over the
protocol, not through a language type:

| Tier      | Lifetime                | Use Case                            |
| --------- | ----------------------- | ----------------------------------- |
| Ephemeral | Single request          | Scratch state, intermediate results |
| Session   | Current conversation    | Context, conversation history       |
| Project   | Per-project, persistent | Learned patterns, project rules     |
| Global    | Cross-project, shared   | Ecosystem knowledge, shared models  |

```json
// Write a project-tier entry
{"jsonrpc":"2.0","id":1,"method":"memory/set",
 "params":{"tier":"project","key":"convention","value":"use_camelCase","tags":["style"]}}

// Read — searches every tier, nearest first
{"jsonrpc":"2.0","id":2,"method":"memory/get","params":{"key":"convention"}}

// Promote an entry to a longer-lived tier
{"jsonrpc":"2.0","id":3,"method":"memory/promote",
 "params":{"key":"convention","from":"session","to":"project"}}
```

## Swarm Orchestration Patterns

| Pattern    | Description                            | Function           |
| ---------- | -------------------------------------- | ------------------ |
| Map-Reduce | Distribute work, collect, merge        | `swarm_map_reduce` |
| Pipeline   | Chain agents sequentially              | `swarm_pipeline`   |
| Saga       | Multi-step with compensating rollbacks | `swarm_saga`       |
| Fan-Out    | Same task to N agents, collect all     | `swarm_fan_out`    |
| Race       | First successful result wins           | `swarm_race`       |

**None of these five are implemented.** `swarm_map_reduce`, `swarm_pipeline`,
`swarm_saga`, `swarm_fan_out` and `swarm_race` are reserved words in the lexer
that no parser arm and no evaluator consumes, and MAGE_SPEC.md does not mention
them. Writing one is a parse error that says so — and because they are
reserved, you cannot define a function of that name to fill the gap either.

Write the patterns out instead. Fan-out is `map`, fan-in is `fold`:

```MAGE
agent Worker { capabilities: [agent] }

swarm Pool {
    agent: Worker
    size: 4
    topology: mesh
    consensus: majority
}

fn handle_item(item: str) -> i32 / agent {
    agent.spawn(item)
    len(item) as i32
}

// Map-reduce: distribute, then merge.
pub fn map_reduce(items: [str]~) -> i32 / agent {
    fold(map(items, |item| handle_item(item)), 0, |acc, n| acc + n)
}

// Pipeline: chain the stages by composing the calls.
pub fn run_pipeline(item: str) -> i32 / agent {
    handle_item(upper(item))
}
```
