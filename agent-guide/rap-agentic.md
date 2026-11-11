# RAP Agentic Methods

The MAGE Agent Protocol (RAP) exposes agentic-first services over JSON-RPC.
Beyond the base `language/*` and `build/check` methods, RAP provides:

## Method Reference

| Method             | Purpose                               | Key Params                                                       |
| ------------------ | ------------------------------------- | ---------------------------------------------------------------- |
| `build/heal`       | Parse + generate fix candidates (P22) | `source`                                                         |
| `build/selfheal`   | Intent-guided heal loop + repair      | `source`, `fuel`                                                 |
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
{"jsonrpc":"2.0","id":1,"method":"build/heal","params":{"source":"pub fn add(a: i32, b: i32) -> i32 {\n  a + b\n"}}

// Response (trimmed; `edits` omitted)
{"id":1,"jsonrpc":"2.0","result":{"ok":false,"diagnostics":[{
  "diagnostic":{"category":"SyntaxError","id":"E0001","severity":"Error",
    "message":"expected RBrace, found Eof '' (in Block)","span":{"line":3,"col":1}},
  "fixes":[{"id":"insert-rbrace-before-token","description":"Insert `}` at the parser's error position",
    "confidence":0.55,"token_cost":1,"semantics_preserving":true,"edits":[...]}]}]}}
```

A complete function with no return type, `pub fn add(a: i32, b: i32) { a + b }`, returns `ok: true` with no diagnostics, so `build/heal` does not flag it.

**Agent workflow**: Call `build/heal` instead of `build/check`. If fixes are
returned, apply the highest-`confidence` fix and re-check. Iterate until clean.

> **`confidence` is a ranking prior, not a measured success rate.** All 44
> values are literals in `prototype/src/heal.rs`; nothing computes them and
> nothing calibrates them against whether the fix works. The reliability
> bench measures heal success in aggregate and does not record which pattern
> fired, so a per-pattern rate is derivable and not derived.
>
> Sorting by it is the intended use and a reasonable one — a rename fix
> really is likelier to be right than a guess, and the recovery bench tries
> candidates in this order. **Reading `0.85` as "85% likely correct" is
> not.** Re-check after applying, which this workflow already tells you to
> do, and treat the number as an ordering and nothing more.

## `build/selfheal` and `mage-parse --heal`

`build/heal` lists candidates; `build/selfheal` (and the CLI mode
`mage-parse --heal FILE [--write] [--json] [--fuel N]`) applies them, under
rules taken from the program itself. The code is `prototype/src/intent.rs`
and `prototype/src/self_heal.rs`.

**Intent.** Read from four sources only, and each item records which:
function signatures (parameter names and types, return type, generics,
`where`), declared effects, `@req`/`@ens` clauses (on the function or in a
`sp` block) and struct `@inv`, and `@test` functions: zero-argument
top-level functions carrying `@test` or `@t`. A test passes when the
fuel-bounded evaluator returns without error. Names and comments are not read.

**Compile time.** Every candidate fix comes from where the compiler located
the problem. Type-checker diagnostics carry no source span, so beside each
type mismatch the checker records structured evidence
(`types::check_with_evidence`): the enclosing function, the site (the
returned tail expression, a typed `let` initializer, argument *i* of a call
to a builtin such as `range` or to a user function, the right operand of
arithmetic between two numeric types, an assigned value), the expression
itself, and the expected and found types. For a numeric-to-numeric mismatch
the candidate is a cast of exactly that expression: `expr as T`, or
`(expr) as T` when the expression is not atomic. The expression is found in
the source by re-parsing token ranges of the named function and comparing
the result with the checker's expression (a returned value must end the
body; an operand must end its binary expression; a call argument is searched
inside its located call, and the call's top-level commas decide between
equal arguments). Exactly one range must match, with `(x)` and `x` counted
once; otherwise the mismatch is reported as not located, with how many ranges
matched, and nothing is tried for it. A mismatch between non-numeric types is
reported with no fix. `heal_one`'s patterns are used only for diagnostics that
have a position, and `recover` when the source does not parse. A candidate is
accepted only if the error count strictly drops, every function keeps its
signature, declared effects and contracts, struct invariants are unchanged,
no `@test` is removed, and every `@test` that passed before the round still
passes. Among accepted candidates: semantics-preserving first (a cast is not
claimed preserving: it can truncate or change sign), then fewer remaining
errors, then the `confidence` prior. One fix per round; the loop stops at zero
errors, at a round with nothing accepted, or after 8 rounds.

**Run time.** Every `@test`, and every zero-argument function with contracts,
runs once with a fuel budget (default 200000 per run), a call trace (at most
64 lines) and coverage. Coverage (`Interp::enable_coverage`, off by default;
when off it costs one `is_some` test per binary operator, `if` and index)
records, for each comparison and arithmetic node and each `if` of the
top-level functions, under a stable site id `function/kind#n` (the *n*-th node
of that kind in pre-order): whether it ran, the operands and result of each
comparison (first 16 per run), and the arm each `if` took. When a run fails
with a runtime error raised by an operator or an index (division by zero,
overflow, index out of bounds), the report names that node as `raised at`; a
contract violation names its clause as the failed check.

Sites of non-test functions that at least one failing run executed are ranked
by Ochiai score, failed(s) / sqrt(total_failed × (failed(s) + passed(s))),
where failed(s) and passed(s) count the failing and passing runs that
executed the site; sites no failing run executed get no candidate. Ties go to
a site whose observations motivate a candidate, then to operator sites before
`if`s. Candidates are generated only at ranked sites, best first, and within
a site motivated ones first:

- a comparison that saw equal operands in a failing run: the
  inclusive/exclusive swap (`>`/`>=`, `<`/`<=`);
- `+` or `-` in a function whose `@ens` with `<` or `>` failed: `+`/`-`;
- an `if` whose taken arm was taken only in failing runs: swap the branches,
  or negate the condition when there is no `else`;
- then the remaining small mutations at that site: the other comparison
  changes (negation, reversed operands), `+`/`-`, an integer-literal right
  operand ±1.

A failed `@ens` of a function whose body returns an integer literal adds that
literal ±1 (the direction the clause's `>` or `<` suggests first); such a site
has no Ochiai score and is tried after the scored ones. Each mutation is made
on the tree and run on the mutated tree directly. To write it back, the
node's operator (or literal, or `if`) is mapped to the *k*-th token of the
same text in the function, which requires as many such tokens as such nodes;
the edited source is re-parsed and must give the mutated tree. A mapping that
fails is a rejection, not a guess. A candidate is accepted when it keeps the
intent, the failing runs pass, it adds no checker error, and every run
passes. At most 500 candidates. The repair is always reported as
semantics-changing, with its site, score, failing and passing counts, the
values observed there, why it was tried first, and the runs it turned from
failing to passing; every ranked site lists its rejected candidates with the
reason.

**Output.** The intent summary, the compile-time steps with the compiler
evidence each answers (function, site, expression, line and column, expected
and found types), the mismatches left without a fix and why, the debug report
(failing runs with failed check, raised-at node and trace; the ranked sites
with score, counts, observations and rejections, eight in the text report and
all in JSON), the repair and its evidence, and a unified diff. The file is
written only with `--write`. Exit 0 only when the healed source has no errors
and every run passes. `--json` and `build/selfheal` return the same report as
JSON (`compile.steps[].evidence`, `compile.unfixed`, `debug.sites`,
`debug.runs[].raised_at`, `debug.repair`).

**Limits.**

- Compiler evidence covers the sites listed above. A mismatch elsewhere (an
  `if`/`match` arm, a list element, a contract clause, a nested function) has
  no evidence and gets no cast. Only numeric conversions are proposed; for
  arithmetic the right operand is converted to the left operand's type, which
  may not be the conversion the author meant.
- A type-checker diagnostic still has no line in `--check`; only the evidence
  is located.
- The Ochiai score orders sites; it is not the probability that a site is at
  fault. With few tests most sites tie.
- Coverage sees top-level functions only. Closures and nested functions are
  evaluated from copies and are not recorded, so their sites are never ranked.
  Methods are not recorded either.
- An `if` is mapped to source only as `if c { … } else { … }`; `? c { } : { }`
  and `else if` chains are not swapped or negated. An operator whose token
  count differs from its node count in the function (a unary minus folded into
  a literal, a `<` in a type) is not mapped.
- Repairs are single mutations. The tests are the whole specification, so a
  weak test suite admits a wrong repair; the first accepted candidate is taken.
- If the input does not parse, its intent cannot be read before the first
  fix; the intent kept is that of the first source that parses, and the report
  says so. A parse recovery can therefore delete code the intent never saw.
- A probe with arguments is never invented: contracts on functions with
  parameters are exercised only through the tests that call them.

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

Returns `verdict` at the top level (`proved`, `evidenced`, `unspecified`, `unreached` or `refuted`), and `result` with `status` (`Verified`, `Partial`, `Failed` or `Unspecified`), `checks` and `effect_checks`.

## Agent Memory (not implemented)

`memory/set`, `memory/get` and `memory/promote` are not RAP methods. The server answers each with `-32601 Method not found`, and `rap/methods` lists what it dispatches. The four tiers below are a design only; there is no `std::agent::Memory` type.

| Tier      | Lifetime                | Use Case                            |
| --------- | ----------------------- | ----------------------------------- |
| Ephemeral | Single request          | Scratch state, intermediate results |
| Session   | Current conversation    | Context, conversation history       |
| Project   | Per-project, persistent | Learned patterns, project rules     |
| Global    | Cross-project, shared   | Ecosystem knowledge, shared models  |

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
them. As an item, or as a function name, one is the generic parse error `expected item, found KwSwarmRace`; in an expression the error names the reserved word. Either way they are reserved, so you cannot define a function of that name to fill the gap.

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
