# High-Level Architecture DSL — Composability Design

*The connective tissue for a maximally high-level, composable architecture DSL.
Every claim here is anchored to a measurement (`benchmarks/constructs/`) or to an
existing MechGen subsystem. Status markers: ✅ done · ◻ to build.*

## 1. The premise, corrected by measurement

"Higher level → fewer tokens" is true **only where the construct subsumes
boilerplate**, not in the abstract. Measured (`benchmarks/constructs/run.sh`):

| construct | reduction | why |
|---|--:|---|
| `sum`/`freq`/`scan` vs explicit loop | 60–65 % | names a whole control-flow pattern |
| vocabulary + **custom closure** | 14 % | the closure body is *irreducible payload* |
| Transformer `net` vs PyTorch | 49 % | DSL subsumes the imperative forward |

So the token cost of any program is bounded below:

```
tokens(program) ≈ Σ references-to-named-patterns  +  Σ irreducible-novel-bits
```

A DSL can drive the first term toward **1 token each**; it can do **nothing**
about the second (that is real information). The goal is therefore **not**
"maximally high-level" in the abstract — it is **maximizing the fraction of a
program that is references to reusable patterns**.

## 2. The factorization: a small algebra × a large leaf library

A *massive flat knowledgebase of named compositions* is the wrong shape — it hits
two walls:

- **Tokenizer wall.** The §8 discipline (`vocabulary_audit`) requires each name to
  be a *single BPE token*. There are only ~100–200 K tokens; you cannot have a
  *massive* vocabulary of single-token names.
- **Combinatorial wall.** You cannot name every composition — exponentially many.

Resolution — mirror what §8 did for *data* (`map`/`filter`/`fold` = a handful of
combinators; variety lives in the closures). For *architecture*:

- **Few composition operators** (single-token, orthogonal, closed under
  composition) → arbitrary composability.
- **Many leaf blocks** in a registry, referenced by a **short handle**, not
  carried inline → token efficiency from reuse.

`few operators × many leaf blocks`, **not** `many named compositions`.

## 3. What "precompute results" means — precisely

You **cannot** precompute *outputs* (weight/data-dependent post-training). You
**can** precompute the **lowering/structure**: a parameterized block lowers
deterministically to a fixed byte-sequence. So:

> **content-address the lowering (hash), cache it, dedup identical instances,
> reference by handle.**

First expression of a novel block costs its tokens; every *reuse* is ~1 token;
every *identical* lowering is stored once.

## 4. The build — operators, blocks, registry, binary

### 4.1 Composition operators (the algebra)

| operator | meaning | status |
|---|---|---|
| `stack N { … }` | repeat a layer body N× | ✅ landed |
| `residual { … }` | wrap a body in `x + f(x)` | ◻ |
| `branch { … } { … }` | parallel paths, then combine | ◻ |
| `wrap A { … }` | pre/post a body with op A (e.g. norm) | ◻ |

`stack` is proven: **`stack 12 { block }` = 82 real cl100k tokens vs 839 for the
manual 12× repeat — 10.2× fewer, and flat in depth** (100 layers stays ~83
tokens). Source 271 B vs 2320 B. It expands at parse (`parser.rs`,
`parse_layer_body` + the `stack` arm), so lowering is unchanged.

### 4.2 Named blocks ◻

```
block TransformerBlock(d, h, ff) {
    layer attn: MultiHeadAttention(d, h);
    layer norm1: LayerNorm;
    layer ff1: Linear(d, ff);
    layer act: GELU;
    layer ff2: Linear(ff, d);
    layer norm2: LayerNorm;
}
net GPT { stack 12 { TransformerBlock(256, 8, 1024) } forward { … } }
```

A `block` is a parameterized macro over layers. `stack N { Block(args) }`
references it; the surface cost of a 12-layer net becomes **one block definition +
one reference + a count**. AST: a new `BlockDef { name, params, layers }` item; a
block-reference inside `stack`/`net` bodies that expands with the params
substituted.

### 4.3 Registry handles (the knowledgebase) ◻

The leaf library is the **Forge registry** (`forge/`, capability-indexed,
contract-typed, dedup by SHA-256). A block published to Forge is referenced by a
**single-token handle**; the full definition is fetched on demand via the
ontology's progressive disclosure (`manifest`/`describe`) — so the agent
*discovers* blocks without carrying the KB in context. This is the
already-built `forge` toolchain + registry + the agentic `manifest --json`
surface, connected to the `net` DSL.

### 4.4 Binary dedup — a REPEAT op ◻

Today `stack 12` still lowers to 12× ABL items (binary is O(depth): measured
1180 B for 12 blocks vs 123 B for one, ≈9.6×). Add a **`REPEAT(count, block)` op**
to the Agentic Binary Language container so the *artifact* is O(1) too: store the
block once + a count, content-addressed by the block hash. Then the shipped net is
O(1) in depth in *both* tokens and bytes. (`abl_bridge.rs` op table +
container encoder.)

### 4.5 Typed / contracted composition (safety) ◻

A big block library only helps if composition is *safe*. Gate it with the
existing contract system (`@req`/`@ens` on blocks — shape/dtype pre/postconditions)
and the **SKB** (9,157 rules): `stack`/`residual`/`branch` check that adjacent
blocks' output/input shapes unify, so "arbitrarily selected and composed" is
*checkable*, not just possible.

## 5. Falsifiable targets (verify each piece by measurement)

- `stack` ✅ — token cost O(1) in depth (10.2×, measured).
- named `block` + reference — a 12-layer GPT ≈ `block def + stack ref`; target
  **< 1.3×** the single-block token count.
- registry handle — referencing a published block costs **≤ 3 tokens**; the block
  def is *not* in the agent's context (fetched via `describe`).
- ABL `REPEAT` — 12-block container ≤ **1.5×** the 1-block container (vs 9.6×
  today).
- typed composition — a shape-mismatched `stack`/`branch` is rejected at
  `--check` with an actionable diagnostic.

## 6. Honest limits

- This makes *recombination of known blocks* nearly free. **Genuinely novel
  architecture stays expensive** — the irreducible floor (§1), and correctly so.
- `precompute` helps *reuse and identical-instance dedup*, never *first
  expression* and never *data-dependent outputs*.
- The operators must stay **few and orthogonal**; every new named composition
  that isn't expressible from the algebra is a smell (re-derive it from
  `stack`/`residual`/`branch`/`wrap` + leaf blocks instead).
